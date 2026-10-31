//! trim_boundary：用边界裁剪（以 boundary 开头的平面批量 Split 实体与曲面）。
//!
//! 对应 ltl-manage resources/sidecars/trim_boundary/trim_boundary.py。
//! 1. 找名字以 plane_prefix（默认 boundary，不区分大小写）开头的平面
//!    （Selection.Search ".Plane,all" 定位 + COM 类型名含 plane 校验）；
//! 2. 读 tooling axis（名字含 tooling_key）原点作方向基准，平面法向 = 第一轴×
//!    第二轴，原点带符号距离决定保留侧 —— 始终保留「平面与 tooling axis 原点
//!    之间」的一侧（keep_origin_side=false 取反）；
//! 3. 实体：ShapeFactory AddNewSplit（每 Body × 每平面一个 Split）；
//! 4. 曲面：HybridShapeFactory AddNewHybridSplit，结果放回原容器，成功后隐藏
//!    原曲面（keep_originals 保留）；
//! 5. 幂等：Trim_ 开头的既有结果跳过；dry_run 只统计。
//!
//! 与 Python 版差异：同名 .CATPart 自动切换未移植（与 hide_show 同口径）。

use serde_json::{json, Value};
use std::collections::HashMap;

use rscatiacli::dispatch::ComObj;
use rscatiacli::objects::{Body, HybridBody, Part, Plane, Selection};
use rscatiacli::{attach_with_guard, ComCollection, ComObject, ComResult, Progress};

use crate::surface_common::{com_type_name, is_plane, iter_geosets, natural_key};

const CAT_POSITIVE_SIDE: i32 = 0;
const CAT_NEGATIVE_SIDE: i32 = 1;
const TRIM_PREFIX: &str = "Trim_";

fn name_of(obj: &impl ComObject) -> String {
    obj.name().unwrap_or_default()
}

/// COM 对象身份键（IUnknown 指针），用于反查容器（与 Python `_oleobj_` 指针相等同口径）。
fn identity_key(obj: &ComObj) -> usize {
    obj.identity_key()
}

// ── 向量工具 ─────────────────────────────────────────────────────────

type Vec3 = [f64; 3];

fn dot(a: Vec3, b: Vec3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: Vec3, b: Vec3) -> Vec3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn norm(v: Vec3) -> Vec3 {
    let length = dot(v, v).sqrt();
    if length == 0.0 {
        return v;
    }
    [v[0] / length, v[1] / length, v[2] / length]
}

// ── 几何读取 ─────────────────────────────────────────────────────────

/// 读平面局部坐标，返回 (原点, 单位法向)。
/// 法向 = 第一轴 × 第二轴（平面局部 Z，CATIA Split 的"自然法向"）。
/// 先走 Plane 接口（VBA 中转），失败退回 SPAWorkbench Measurable.GetPlane。
fn plane_frame(
    part: &Part,
    spa: &rscatiacli::objects::SPAWorkbench,
    ss: &rscatiacli::objects::SystemService,
    plane: &ComObj,
) -> ComResult<(Vec3, Vec3)> {
    let pl = Plane::new(plane.clone());
    if let (Ok(o), Ok(u), Ok(v)) = (
        pl.get_origin(ss),
        pl.get_first_axis(ss),
        pl.get_second_axis(ss),
    ) {
        return Ok((o, norm(cross(norm(u), norm(v)))));
    }
    let reference = part.create_reference_from_object(plane)?;
    let m = spa.get_measurable(&reference)?;
    let c = m.get_plane(ss)?; // [ox,oy,oz, ux,uy,uz, vx,vy,vz]
    let origin = [c[0], c[1], c[2]];
    let u = norm([c[3], c[4], c[5]]);
    let v = norm([c[6], c[7], c[8]]);
    Ok((origin, norm(cross(u, v))))
}

/// 查找名字含 key 的轴系，返回 (轴系名, 原点(mm))；找不到返回 None。
fn find_tooling_axis(
    part: &Part,
    ss: &rscatiacli::objects::SystemService,
    key: &str,
) -> Option<(String, Vec3)> {
    let key_l = key.to_lowercase();
    let axes = part.axis_systems().ok()?;
    for ax in axes.items().ok()? {
        let name = ax.name().unwrap_or_default();
        if !name.to_lowercase().contains(&key_l) {
            continue;
        }
        let Ok(origin) = ax.get_origin(ss) else {
            continue;
        };
        return Some((name, origin));
    }
    None
}

// ── 收集 ─────────────────────────────────────────────────────────────

struct PlaneInfo {
    name: String,
    com: ComObj,
    geoset: Option<ComObj>,
    frame: (Vec3, Vec3),
}

/// 收集全部 boundary 平面（名字以 prefix 开头、类型为平面），自然排序。
fn find_boundary_planes(
    part: &Part,
    hbs: &rscatiacli::objects::HybridBodies,
    doc_sel: &Selection,
    spa: &rscatiacli::objects::SPAWorkbench,
    ss: &rscatiacli::objects::SystemService,
    prefix: &str,
    progress: Progress<'_>,
) -> Vec<PlaneInfo> {
    // 平面由 CATIA 原生搜索定位（实测语法 .Plane,all）
    let _ = doc_sel.clear();
    if doc_sel.search(".Plane,all").is_err() {
        let _ = doc_sel.clear();
        return Vec::new();
    }

    // 全部容器内的 hybrid shape 指针 → 容器映射
    let mut pointer_to_container: HashMap<usize, ComObj> = HashMap::new();
    for hb in iter_geosets(hbs) {
        if let Ok(shapes) = hb.hybrid_shapes() {
            for j in 1..=shapes.count().unwrap_or(0) {
                if let Ok(sh) = shapes.item(j) {
                    pointer_to_container.insert(identity_key(sh.com()), hb.com().clone());
                }
            }
        }
    }

    let mut found: Vec<PlaneInfo> = Vec::new();
    let count = doc_sel.count().unwrap_or(0);
    for i in 1..=count {
        let Ok(item) = doc_sel.item(i) else {
            continue;
        };
        let Ok(value) = item.value() else {
            continue;
        };
        let name = value.name().unwrap_or_default();
        if !name.to_lowercase().starts_with(prefix) {
            continue;
        }
        if !is_plane(value.com()) {
            let tn = com_type_name(value.com());
            progress(&format!(
                "⚠ 跳过非平面特征: {name}(类型 {})",
                if tn.is_empty() { "未知" } else { &tn }
            ));
            continue;
        }
        // 解析平面所属几何体集（指针相等判定，供裁剪结果放回同集）
        let geoset = pointer_to_container
            .get(&identity_key(value.com()))
            .cloned();
        let frame = match plane_frame(part, spa, ss, value.com()) {
            Ok(f) => f,
            Err(e) => {
                progress(&format!("⚠ 平面「{name}」几何读取失败,跳过: {e}"));
                continue;
            }
        };
        found.push(PlaneInfo {
            name,
            com: value.com().clone(),
            geoset,
            frame,
        });
    }
    let _ = doc_sel.clear();
    found.sort_by_key(|p| natural_key(&p.name));
    found
}

/// 收集可裁剪的实体 Body：不在布尔运算中、当前可见、体积非 0（与 dump_solid 同过滤）。
fn collect_solid_bodies(
    part: &Part,
    spa: &rscatiacli::objects::SPAWorkbench,
    doc_sel: &Selection,
) -> Vec<Body> {
    let Ok(bodies) = part.bodies() else {
        return Vec::new();
    };
    let mut result = Vec::new();
    for body in bodies.items().unwrap_or_default() {
        if body.in_boolean_operation().unwrap_or(false) {
            continue;
        }
        let _ = doc_sel.clear();
        if doc_sel.add(&body).is_ok() {
            if let Ok(vis) = doc_sel.vis_properties() {
                // 0 = 显示；读取失败按可见处理
                if vis.get_show().map(|s| s != 0).unwrap_or(false) {
                    continue;
                }
            }
        }
        let visible = match part.create_reference_from_object(&body) {
            Ok(r) => match spa.get_measurable(&r) {
                Ok(m) => m.volume().map(|v| v * 1e9).unwrap_or(0.0) != 0.0,
                Err(_) => false,
            },
            Err(_) => false,
        };
        if !visible {
            continue;
        }
        result.push(body);
    }
    let _ = doc_sel.clear();
    result
}

struct SurfaceInfo {
    name: String,
    com: ComObj,
    container: Option<ComObj>,
}

/// 用 CATIA 原生 Surface 搜索定位零件中全部曲面（排除平面与既有 Trim_ 结果）。
fn collect_surfaces(
    hbs: &rscatiacli::objects::HybridBodies,
    doc_sel: &Selection,
    bodies: &[Body],
    boundary_names: &std::collections::HashSet<String>,
) -> Vec<SurfaceInfo> {
    let _ = doc_sel.clear();
    if doc_sel.search(".Surface,all").is_err() {
        let _ = doc_sel.clear();
        return Vec::new();
    }

    // 预收集全部容器内的 hybrid shape 指针 → 容器映射
    let mut pointer_to_container: HashMap<usize, ComObj> = HashMap::new();
    for hb in iter_geosets(hbs) {
        if let Ok(shapes) = hb.hybrid_shapes() {
            for j in 1..=shapes.count().unwrap_or(0) {
                if let Ok(sh) = shapes.item(j) {
                    pointer_to_container.insert(identity_key(sh.com()), hb.com().clone());
                }
            }
        }
    }
    for b in bodies {
        if let Ok(shapes) = b.hybrid_shapes() {
            for j in 1..=shapes.count().unwrap_or(0) {
                if let Ok(sh) = shapes.item(j) {
                    pointer_to_container.insert(identity_key(sh.com()), b.com().clone());
                }
            }
        }
    }

    let mut surfaces: Vec<SurfaceInfo> = Vec::new();
    let count = doc_sel.count().unwrap_or(0);
    for i in 1..=count {
        let Ok(item) = doc_sel.item(i) else {
            continue;
        };
        let Ok(value) = item.value() else {
            continue;
        };
        let name = value.name().unwrap_or_default();
        // 平面不是曲面(保险)；boundary 平面与既有裁剪结果一律不动
        if is_plane(value.com()) {
            continue;
        }
        if name.starts_with(TRIM_PREFIX) || boundary_names.contains(&name) {
            continue;
        }
        let container = pointer_to_container
            .get(&identity_key(value.com()))
            .cloned();
        surfaces.push(SurfaceInfo {
            name,
            com: value.com().clone(),
            container,
        });
    }
    let _ = doc_sel.clear();
    surfaces.sort_by_key(|s| natural_key(&s.name));
    surfaces
}

// ── 方向判定 ─────────────────────────────────────────────────────────

/// 按「保留平面与 tooling axis 原点之间」规则计算保留侧。
/// 返回 (split_side, gsd_orientation, signed_distance)。
fn keep_side_for_plane(
    plane_info: &PlaneInfo,
    origin: Vec3,
    keep_origin_side: bool,
) -> (i32, i32, f64) {
    let (po, n) = plane_info.frame;
    let signed = dot([origin[0] - po[0], origin[1] - po[1], origin[2] - po[2]], n);
    let origin_side_positive = signed >= 0.0; // 原点在平面法向侧
    let keep_positive = if keep_origin_side {
        origin_side_positive
    } else {
        !origin_side_positive
    };
    let split_side = if keep_positive {
        CAT_POSITIVE_SIDE
    } else {
        CAT_NEGATIVE_SIDE
    };
    let gsd_orientation = if keep_positive { 1 } else { -1 };
    (split_side, gsd_orientation, signed)
}

// ── 裁剪 ─────────────────────────────────────────────────────────────

/// 对单个实体 Body 执行 Part Design Split。返回错误信息（空串=成功）。
fn split_solid(part: &Part, body: &Body, plane_info: &PlaneInfo, split_side: i32) -> String {
    let plane_ref = match part.create_reference_from_object(&plane_info.com) {
        Ok(r) => r,
        Err(e) => return format!("创建平面引用失败: {e}"),
    };
    let split = match (|| -> ComResult<rscatiacli::objects::Entity> {
        part.set_in_work_object(body)?;
        let factory = part.shape_factory()?;
        factory.add_new_split(&plane_ref, split_side)
    })() {
        Ok(s) => s,
        Err(e) => return format!("Split 创建失败: {e}"),
    };
    let _ = split.set_name(&format!(
        "{TRIM_PREFIX}{}_{}",
        plane_info.name,
        name_of(body)
    ));
    let _ = part.update_object(&split);
    String::new()
}

/// 对单个曲面执行 GSD Split，结果追加回原容器。返回错误信息（空串=成功）。
fn split_surface(
    part: &Part,
    hsf: &rscatiacli::objects::HybridShapeFactory,
    doc_sel: &Selection,
    surf: &SurfaceInfo,
    plane_info: &PlaneInfo,
    gsd_orientation: i32,
    keep_originals: bool,
) -> String {
    let split = match (|| -> ComResult<rscatiacli::objects::Entity> {
        let surf_ref = part.create_reference_from_object(&surf.com)?;
        let plane_ref = part.create_reference_from_object(&plane_info.com)?;
        hsf.add_new_hybrid_split(&surf_ref, &plane_ref, gsd_orientation)
    })() {
        Ok(s) => s,
        Err(e) => return format!("Split 创建失败: {e}"),
    };

    // 追加回原容器（PartDesign Body 用 InsertHybridShape，几何图形集用
    // AppendHybridShape）；容器解析失败时退到 boundary 平面所在几何图形集
    let fallback = surf.container.as_ref().or(plane_info.geoset.as_ref());
    if let Some(c) = fallback {
        let appended = Body::new(c.clone())
            .insert_hybrid_shape(&split)
            .or_else(|_| HybridBody::new(c.clone()).append_hybrid_shape(&split));
        if appended.is_err() {
            // 追加失败则尝试删除刚创建的特征，避免残留
            let _ = split.com().call("Delete", &[]);
            return "Split 加入容器失败".to_string();
        }
    }
    let _ = split.set_name(&format!("{TRIM_PREFIX}{}_{}", plane_info.name, surf.name));
    let _ = part.update_object(&split);

    // 成功后隐藏原曲面（裁剪结果代替原面显示）
    if !keep_originals {
        let _ = doc_sel.clear();
        if doc_sel.add(&surf.com).is_ok() {
            if let Ok(vis) = doc_sel.vis_properties() {
                let _ = vis.set_show(1); // 1=隐藏
            }
        }
        let _ = doc_sel.clear();
    }
    String::new()
}

// ── 主逻辑 ───────────────────────────────────────────────────────────

/// 工具主体。**必须在 STA 线程上执行。**
pub fn run(params: &Value, progress: Progress<'_>) -> ComResult<Value> {
    let empty_data = json!({
        "plane_prefix": "", "planes": [], "tooling_axis": "", "origin_mm": Value::Null,
        "per_plane": [], "bodies": [], "surfaces": [], "solid_splits": 0,
        "surface_splits": 0, "solid_failed": [], "surface_failed": [],
        "dry_run": false,
    });

    let dry_run = params
        .get("dry_run")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let plane_prefix = params
        .get("plane_prefix")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("boundary")
        .to_lowercase();
    let tooling_key = params
        .get("tooling_key")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("tooling")
        .to_string();
    let include_surfaces = params
        .get("include_surfaces")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let keep_origin_side = params
        .get("keep_origin_side")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let keep_originals = params
        .get("keep_originals")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let app = attach_with_guard(progress)?;
    let doc = app.active_document()?;
    let doc_name = name_of(&doc);
    if !doc_name.to_lowercase().ends_with(".catpart") {
        let kind = if doc_name.to_lowercase().ends_with(".catproduct") {
            "装配"
        } else if doc_name.to_lowercase().ends_with(".catdrawing") {
            "工程图"
        } else {
            "非零件文档"
        };
        return Ok(json!({
            "success": false,
            "message": format!("当前是{kind}({doc_name}): Rust 版尚未移植同名零件自动切换, 请先打开对应 .CATPart 后重试"),
            "data": empty_data,
        }));
    }

    let alerts_prev = app.display_file_alerts().unwrap_or(true);
    let _ = app.set_display_file_alerts(false);

    let result = run_inner(
        &app,
        &doc,
        dry_run,
        &plane_prefix,
        &tooling_key,
        include_surfaces,
        keep_origin_side,
        keep_originals,
        empty_data,
        progress,
    );
    let _ = app.set_display_file_alerts(alerts_prev);
    result
}

#[allow(clippy::too_many_arguments)]
fn run_inner(
    app: &rscatiacli::objects::Application,
    doc: &rscatiacli::objects::Document,
    dry_run: bool,
    plane_prefix: &str,
    tooling_key: &str,
    include_surfaces: bool,
    keep_origin_side: bool,
    keep_originals: bool,
    empty_data: Value,
    progress: Progress<'_>,
) -> ComResult<Value> {
    let prt_doc = doc.part_document();
    let part: Part = prt_doc.part()?;
    let part_name = part.name().unwrap_or_else(|_| "Part".to_string());
    let hbs = part.hybrid_bodies()?;
    let hsf = part.hybrid_shape_factory()?;
    let spa = doc.part_document().spa_workbench()?;
    let ss = app.system_service()?;
    let sel = doc.selection()?;

    // 零件本身必须无更新错误（照 offset_surface 先整体 Update 校验）
    if let Err(e) = part.update() {
        let brief = e.to_string().chars().take(120).collect::<String>();
        return Ok(
            json!({"success": false, "message": format!("零件存在更新错误,请先修复后再运行: {brief}"), "data": empty_data}),
        );
    }

    // 1. tooling axis 原点（方向基准）
    let Some((axis_name, origin)) = find_tooling_axis(&part, &ss, tooling_key) else {
        return Ok(json!({
            "success": false,
            "message": format!(
                "未找到名字含「{tooling_key}」的轴系(需作为方向基准)。请在零件中创建 tooling axis(建模→边界框工具会自动生成)后重试"
            ),
            "data": empty_data,
        }));
    };
    progress(&format!(
        "tooling 轴系: 「{axis_name}」,原点 [{:.3}, {:.3}, {:.3}]",
        origin[0], origin[1], origin[2]
    ));

    // 2. boundary 平面
    let planes = find_boundary_planes(&part, &hbs, &sel, &spa, &ss, plane_prefix, progress);
    if planes.is_empty() {
        return Ok(json!({
            "success": false,
            "message": format!("零件中未找到以「{plane_prefix}」开头的平面(不区分大小写)。请先命名 boundary 平面后重试"),
            "data": empty_data,
        }));
    }
    progress(&format!(
        "找到 {} 个 boundary 平面: {}",
        planes.len(),
        planes
            .iter()
            .map(|p| p.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    ));

    // 3. 每个平面的保留方向
    let mut per_plane: Vec<Value> = Vec::new();
    for p in &planes {
        let (split_side, gsd_orientation, signed) =
            keep_side_for_plane(p, origin, keep_origin_side);
        let keep_txt = if (signed >= 0.0) == keep_origin_side {
            "原点一侧"
        } else {
            "远离原点一侧"
        };
        per_plane.push(json!({
            "plane": p.name,
            "signed_distance_mm": (signed * 1000.0).round() / 1000.0,
            "keep": keep_txt,
            "split_side": split_side,
            "orientation": gsd_orientation,
        }));
        progress(&format!(
            "  平面「{}」: 原点带符号距离 {signed:+.3}mm → 保留{keep_txt}",
            p.name
        ));
    }

    // 4. 实体 Body
    let bodies = collect_solid_bodies(&part, &spa, &sel);
    let body_names: Vec<String> = bodies.iter().map(name_of).collect();
    progress(&format!(
        "可裁剪实体 Body {} 个: {}",
        bodies.len(),
        if body_names.is_empty() {
            "(无)".to_string()
        } else {
            body_names.join(", ")
        }
    ));

    // 5. 曲面
    let boundary_names: std::collections::HashSet<String> =
        planes.iter().map(|p| p.name.clone()).collect();
    let surfaces = if include_surfaces {
        collect_surfaces(&hbs, &sel, &bodies, &boundary_names)
    } else {
        Vec::new()
    };
    let surface_preview = surfaces
        .iter()
        .take(20)
        .map(|s| s.name.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    progress(&format!(
        "待裁剪曲面 {} 个: {}{}",
        surfaces.len(),
        if surface_preview.is_empty() {
            "(无)"
        } else {
            &surface_preview
        },
        if surfaces.len() > 20 { "..." } else { "" }
    ));

    let planes_list: Vec<String> = planes.iter().map(|p| p.name.clone()).collect();
    let surface_names: Vec<String> = surfaces.iter().map(|s| s.name.clone()).collect();
    let mut result = json!({
        "plane_prefix": plane_prefix,
        "planes": planes_list,
        "tooling_axis": axis_name,
        "origin_mm": origin.iter().map(|v| (v * 1000.0).round() / 1000.0).collect::<Vec<_>>(),
        "per_plane": per_plane,
        "bodies": body_names,
        "surfaces": surface_names,
        "solid_splits": 0,
        "surface_splits": 0,
        "solid_failed": [],
        "surface_failed": [],
        "dry_run": dry_run,
        "part_name": part_name,
    });

    if dry_run {
        let solid_splits = bodies.len() * planes.len();
        let surface_splits = surfaces.len() * planes.len();
        result["solid_splits"] = json!(solid_splits);
        result["surface_splits"] = json!(surface_splits);
        let mut msg = format!(
            "[dry-run] 零件「{part_name}」将按 {} 个 boundary 平面裁剪: 实体 {} 个 × {} = {solid_splits} 个 Split",
            planes.len(),
            bodies.len(),
            planes.len()
        );
        if include_surfaces {
            msg.push_str(&format!(
                ",曲面 {} 个 × {} = {surface_splits} 个 Split",
                surfaces.len(),
                planes.len()
            ));
        }
        msg.push_str(&format!(
            ";均保留「平面与 tooling axis 原点([{:.3}, {:.3}, {:.3}])之间」一侧。未做任何修改",
            origin[0], origin[1], origin[2]
        ));
        return Ok(json!({"success": true, "message": msg, "data": result}));
    }

    // 6. 实体裁剪
    let mut solid_splits = 0usize;
    let mut solid_failed: Vec<Value> = Vec::new();
    for body in &bodies {
        for p in &planes {
            let side = keep_side_for_plane(p, origin, keep_origin_side).0;
            let err = split_solid(&part, body, p, side);
            if err.is_empty() {
                solid_splits += 1;
                progress(&format!("  ✓ 实体「{}」×「{}」", name_of(body), p.name));
            } else {
                solid_failed.push(json!({"body": name_of(body), "plane": p.name, "reason": err}));
                progress(&format!(
                    "  ✗ 实体「{}」×「{}」: 失败",
                    name_of(body),
                    p.name
                ));
            }
        }
    }

    // 7. 曲面裁剪
    let mut surface_splits = 0usize;
    let mut surface_failed: Vec<Value> = Vec::new();
    if include_surfaces {
        for s in &surfaces {
            for p in &planes {
                let orientation = keep_side_for_plane(p, origin, keep_origin_side).1;
                let err = split_surface(&part, &hsf, &sel, s, p, orientation, keep_originals);
                if err.is_empty() {
                    surface_splits += 1;
                    progress(&format!("  ✓ 曲面「{}」×「{}」", s.name, p.name));
                } else {
                    surface_failed.push(json!({"surface": s.name, "plane": p.name, "reason": err}));
                    progress(&format!("  ✗ 曲面「{}」×「{}」: 失败", s.name, p.name));
                }
            }
        }
    }

    // 8. 收尾更新
    if let Err(e) = part.update() {
        let brief = e.to_string().chars().take(150).collect::<String>();
        progress(&format!("⚠ 最终整体 Update 失败,请检查模型: {brief}"));
    }

    result["solid_splits"] = json!(solid_splits);
    result["surface_splits"] = json!(surface_splits);
    result["solid_failed"] = json!(solid_failed);
    result["surface_failed"] = json!(surface_failed);

    let mut msg = format!(
        "用边界裁剪完成: 实体 Split {solid_splits}/{}",
        bodies.len() * planes.len()
    );
    if include_surfaces {
        msg.push_str(&format!(
            ",曲面 Split {surface_splits}/{}",
            surfaces.len() * planes.len()
        ));
    }
    msg.push_str(&format!(
        ",共 {} 个 boundary 平面;均保留「平面与 tooling axis 原点之间」一侧",
        planes.len()
    ));
    if !solid_failed.is_empty() || !surface_failed.is_empty() {
        msg.push_str(&format!(
            ",失败 {} 处(见日志)",
            solid_failed.len() + surface_failed.len()
        ));
    }
    msg.push_str(";请手动保存文档(Ctrl+S)");

    Ok(json!({"success": true, "message": msg, "data": result}))
}
