//! rule-bounding-box 轴系建立核心（rule_projected_surface 建系共用）。
//!
//! 对应 ltl-manage resources/sidecars/product_analysis/rule_bounding_box.py 的
//! find_draftline / find_halfline / measure_bounding_box(轴系所需部分) /
//! create_coordinate_system：draftline → Z；halfline(长边)/平面内角度扫描 → X；
//! Y = Z×X。三向尺寸用「极值点 → 法平面 → 过极值点偏移平面 → SPA 最小距离」
//! 精确量测（CATIA 内核量平面间距，无离散扫描的余弦误差，如 82 而非 81.98）。
//! 原点 = 主实体 COG；最后在零件中创建 'tooling axis' 轴系（按坐标去参）并读回。
//!
//! 量测用临时体 "RuleBbox"（内含 extremum_points 几何集）在成功/失败路径都清理。
//! 坐标读取走重合 PointCoord（Extremum 自身 get_point/get_cog 常以脚本错误失败）。

use rscatiacli::objects::{
    Entity, HybridBody, HybridShapeFactory, Part, PartDocument, Reference, SPAWorkbench, Selection,
    SystemService,
};
use rscatiacli::{ComCollection, ComObject, ComResult, Progress};

use crate::vba_bbox_generate::{
    cleanup_old, cross, dot, find_line_by_keyword, find_tooling_axis, norm, LineInfo, Vec3,
};

/// 建系结果（读回轴系的权威值）。
pub(crate) struct AxisFrame {
    pub origin_mm: Vec3,
    pub x_axis: Vec3,
    pub y_axis: Vec3,
    pub z_axis: Vec3,
}

/// 量测输出：长边方向 + 主实体 COG（create_coordinate_system 与建系所需）。
struct MeasureFrame {
    length_axis: Vec3,
    #[allow(dead_code)]
    width_axis: Vec3,
    cog: Vec3,
}

/// a 到 b 沿单位方向 dir 的投影距离（极值点是全球坐标，须投影到自定义轴上）。
fn projected_dist(a: Vec3, b: Vec3, dir: Vec3) -> f64 {
    let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    (d[0] * dir[0] + d[1] * dir[1] + d[2] * dir[2]).abs()
}

/// COM 错误统一带上 Python 版的函数名前缀（UI 上按关键字提示）。
fn cm<T>(r: ComResult<T>) -> Result<T, String> {
    r.map_err(|e| format!("measure_bounding_box failed: {e}"))
}

/// 量测上下文（extract 已建好，量测特征全部挂在临时几何集 hb 上）。
struct MeasureCtx<'a> {
    part: &'a Part,
    hsf: &'a HybridShapeFactory,
    hb: &'a HybridBody,
    spa: &'a SPAWorkbench,
    ss: &'a SystemService,
    extract_ref: &'a Reference,
    cog: Vec3,
    progress: Progress<'a>,
}

/// 极值点 → 重合 PointCoord → 读坐标（get_point 失败回退 get_cog，均失败返回 None）。
fn axis_extremum_pt(
    ctx: &MeasureCtx<'_>,
    d_vec: Vec3,
    is_max: bool,
    axis_name: &str,
) -> Option<Vec3> {
    let tmp_dir = ctx
        .hsf
        .add_new_direction_by_coord(d_vec[0], d_vec[1], d_vec[2])
        .ok()?;
    let dir_ref = ctx.part.create_reference_from_object(&tmp_dir).ok()?;
    let extr = ctx
        .hsf
        .add_new_extremum(ctx.extract_ref, &dir_ref, if is_max { 1 } else { 0 })
        .ok()?;
    let _ = ctx.hb.append_hybrid_shape(&extr);
    let _ = ctx.part.set_in_work_object(&extr);
    let _ = extr.set_name(axis_name);
    let _ = ctx.hsf.gsm_visibility(&extr, 0);
    ctx.part.update().ok()?;
    let ref_extr = ctx.part.create_reference_from_object(&extr).ok()?;
    let pt_feat = ctx
        .hsf
        .add_new_point_coord_with_reference(0.0, 0.0, 0.0, &ref_extr)
        .ok()?;
    let _ = ctx.hb.append_hybrid_shape(&pt_feat);
    let _ = ctx.part.set_in_work_object(&pt_feat);
    ctx.part.update().ok()?;
    let ref_pt = ctx.part.create_reference_from_object(&pt_feat).ok()?;
    let m = ctx.spa.get_measurable(&ref_pt).ok()?;
    let pt = match m.get_point(ctx.ss) {
        Ok(p) => p,
        Err(_) => match m.get_cog(ctx.ss) {
            Ok(c) => c,
            Err(e) => {
                (ctx.progress)(&format!(
                    "[extremum] {axis_name}: 坐标读取失败(重合 PointCoord get_point/get_cog 均异常): {e}"
                ));
                return None;
            }
        },
    };
    (ctx.progress)(&format!(
        "[extremum] {axis_name} = ({:.4}, {:.4}, {:.4})",
        pt[0], pt[1], pt[2]
    ));
    Some(pt)
}

/// 沿平面内方向 d_vec 的投影尺寸（扫描用：极值点 + 投影距离）。
fn plane_span(ctx: &MeasureCtx<'_>, d_vec: Vec3) -> Result<f64, String> {
    let pt_max = axis_extremum_pt(ctx, d_vec, true, "scan_max");
    let pt_min = axis_extremum_pt(ctx, d_vec, false, "scan_min");
    match (pt_max, pt_min) {
        (Some(a), Some(b)) => Ok(projected_dist(a, b, d_vec)),
        (Some(p), None) | (None, Some(p)) => {
            (ctx.progress)(&format!("[scan] WARN 单侧极值缺失, 用 COG 估算: d={d_vec:?}"));
            Ok(2.0 * projected_dist(p, ctx.cog, d_vec))
        }
        (None, None) => Err(format!(
            "极值点坐标读取失败(方向 {d_vec:?}): extract/extremum 未解析, 测量链路异常, 不再静默返回 0"
        )),
    }
}

/// 沿方向 d_vec 的尺寸（VBA 核心方法：法平面 + 过极值点偏移平面 + SPA 最小距离）。
fn span_via_planes(ctx: &MeasureCtx<'_>, d_vec: Vec3) -> Result<f64, String> {
    (ctx.progress)(&format!("[span] start d_vec={d_vec:?}"));
    let pt_max = axis_extremum_pt(ctx, d_vec, true, "vba_max");
    let pt_min = axis_extremum_pt(ctx, d_vec, false, "vba_min");
    (ctx.progress)(&format!(
        "[span] extrema pt_max={pt_max:?} pt_min={pt_min:?}"
    ));
    let (p_max, p_min) = match (pt_max, pt_min) {
        (None, None) => {
            return Err(format!(
                "极值点坐标读取失败(方向 {d_vec:?}): extract/extremum 未解析, 测量链路异常, 不再静默返回 0"
            ));
        }
        (Some(p), None) | (None, Some(p)) => {
            (ctx.progress)(&format!(
                "[span] WARN 单侧极值缺失, 用 COG 估算: d={d_vec:?}"
            ));
            return Ok(2.0 * projected_dist(p, ctx.cog, d_vec));
        }
        (Some(a), Some(b)) => (a, b),
    };

    // 过原点、方向=d_vec 的有限线（必须非零长度），作 AddNewPlaneNormal 的法向参考
    let origin_pt = cm(ctx
        .hsf
        .add_new_point_coord(ctx.cog[0], ctx.cog[1], ctx.cog[2]))?;
    let normal_dir = cm(ctx
        .hsf
        .add_new_direction_by_coord(d_vec[0], d_vec[1], d_vec[2]))?;
    let normal_line = cm(ctx.hsf.add_new_line_pt_dir(
        &cm(ctx.part.create_reference_from_object(&origin_pt))?,
        &cm(ctx.part.create_reference_from_object(&normal_dir))?,
        -100.0,
        100.0,
        false,
    ))?;
    cm(ctx.hb.append_hybrid_shape(&normal_line))?;
    cm(ctx.part.set_in_work_object(&normal_line))?;
    cm(ctx.part.update())?;
    let ref_normal = cm(ctx.part.create_reference_from_object(&normal_line))?;
    let ref_origin = cm(ctx.part.create_reference_from_object(&origin_pt))?;
    let base_plane = cm(ctx.hsf.add_new_plane_normal(&ref_normal, &ref_origin))?;
    let ref_base_plane = cm(ctx.part.create_reference_from_object(&base_plane))?;
    cm(ctx.hb.append_hybrid_shape(&base_plane))?;
    cm(ctx.hsf.gsm_visibility(&base_plane, 0))?;
    cm(ctx.part.set_in_work_object(&base_plane))?;
    cm(ctx.part.update())?;

    // 过极值点各建一个法向=d_vec 的偏移平面（AddNewPlaneOffsetPt 移植）
    let plane_at = |pt: Vec3| -> Result<Reference, String> {
        let p = cm(ctx.hsf.add_new_point_coord(pt[0], pt[1], pt[2]))?;
        cm(ctx.hb.append_hybrid_shape(&p))?;
        cm(ctx.part.set_in_work_object(&p))?;
        cm(ctx.part.update())?;
        let ref_p = cm(ctx.part.create_reference_from_object(&p))?;
        let plane = cm(ctx.hsf.add_new_plane_offset_pt(&ref_base_plane, &ref_p))?;
        cm(ctx.hb.append_hybrid_shape(&plane))?;
        cm(ctx.hsf.gsm_visibility(&plane, 0))?;
        cm(ctx.part.set_in_work_object(&plane))?;
        cm(ctx.part.update())?;
        cm(ctx.part.create_reference_from_object(&plane))
    };
    let ref_pl_max = plane_at(p_max)?;
    (ctx.progress)("[span] plane_max ok");
    let ref_pl_min = plane_at(p_min)?;
    (ctx.progress)("[span] plane_min ok");

    // SPA 测两偏移平面最小距离（mm 原值，实测并非 m，勿再 ×1000）
    let m = cm(ctx.spa.get_measurable(&ref_pl_max))?;
    let value = cm(m.get_minimum_distance(&ref_pl_min))?;
    (ctx.progress)(&format!("[span] min distance value={value}"));
    Ok(value)
}

/// measure_bounding_box 的轴系子集：主实体 extract → 角度扫描/半线定向 →
/// 三向精确量测 → 零尺寸守卫；临时 RuleBbox 体成功/失败路径都清理。
#[allow(clippy::too_many_arguments)]
fn measure_axis_frame(
    part: &Part,
    hsf: &HybridShapeFactory,
    cmd_sel: &Selection,
    ss: &SystemService,
    spa: &SPAWorkbench,
    z: Vec3,
    x: Vec3,
    y: Vec3,
    x_axis_override: Option<Vec3>,
    progress: Progress<'_>,
) -> Result<MeasureFrame, String> {
    let attempt = (|| -> Result<MeasureFrame, String> {
        cm(part.update())?;
        let main_body = cm(part.main_body())?;
        let ref_body = cm(part.create_reference_from_object(&main_body))?;

        // SPA 只用于取主实体 COG（包围盒中心 = 坐标系原点）
        let body_cog = spa
            .get_measurable(&ref_body)
            .ok()
            .and_then(|m| m.get_cog(ss).ok())
            .unwrap_or([0.0; 3]);

        // 遗留 RuleBbox 可能是当前 in-work（上次中断所致），先切回主实体再删
        if part.set_in_work_object(&main_body).is_ok() {
            let _ = part.update();
        }
        cleanup_old(part, cmd_sel, &["rulebbox"]);

        let bbox_body = cm(part.bodies().and_then(|b| b.add()))?;
        let _ = bbox_body.set_name("RuleBbox");
        let hb = cm(bbox_body.hybrid_bodies().and_then(|h| h.add()))?;
        let _ = hb.set_name("extremum_points");

        let ex = cm(hsf.add_new_extract(&ref_body))?;
        let _ = ex.set_propagation_type(1);
        let _ = ex.set_complementary_extract(false);
        let _ = ex.set_is_federated(false);
        cm(hb.append_hybrid_shape(&ex))?;
        cm(part.set_in_work_object(&ex))?;
        cm(hsf.gsm_visibility(&ex, 0))?;
        cm(part.update())?;
        let extract_ref = cm(part.create_reference_from_object(&ex))?;

        let ctx = MeasureCtx {
            part,
            hsf,
            hb: &hb,
            spa,
            ss,
            extract_ref: &extract_ref,
            cog: body_cog,
            progress,
        };

        let (length, width, l_axis, w_axis) = if let Some(hl_dir) = x_axis_override {
            // 有 halfline(或显式 X)：直接用其方向作为长边，不做角度扫描
            let x_dir = norm(hl_dir);
            let y_dir = norm(cross(z, x_dir));
            let length = span_via_planes(&ctx, x_dir)?;
            let width = span_via_planes(&ctx, y_dir)?;
            (length, width, x_dir, y_dir)
        } else {
            let dir_of = |deg: f64| -> Vec3 {
                let rad = deg.to_radians();
                norm([
                    rad.cos() * x[0] + rad.sin() * y[0],
                    rad.cos() * x[1] + rad.sin() * y[1],
                    rad.cos() * x[2] + rad.sin() * y[2],
                ])
            };
            // 粗扫 15° 定位最长边区间
            let mut best_angle = 0.0f64;
            let mut best_span = -1.0f64;
            for deg in (0..180).step_by(15) {
                let span = plane_span(&ctx, dir_of(deg as f64))
                    .map_err(|e| format!("measure_bounding_box failed: {e}"))?;
                if span > best_span {
                    best_span = span;
                    best_angle = deg as f64;
                }
            }
            // 细扫：峰值附近 ±5°，1° 步长
            let coarse = best_angle as i32;
            best_angle = coarse as f64;
            for off in -5..=5 {
                let deg = coarse + off;
                if !(0..180).contains(&deg) || deg == coarse {
                    continue;
                }
                let span = plane_span(&ctx, dir_of(deg as f64))
                    .map_err(|e| format!("measure_bounding_box failed: {e}"))?;
                if span > best_span {
                    best_span = span;
                    best_angle = deg as f64;
                }
            }
            // 最终角度上精确量测长边（无余弦误差），宽 = 平面内正交方向
            let l_axis = dir_of(best_angle);
            let length = span_via_planes(&ctx, l_axis)?;
            let w_axis = dir_of(best_angle + 90.0);
            let width = span_via_planes(&ctx, w_axis)?;
            (length, width, l_axis, w_axis)
        };
        // 高 = 沿拔模方向 Z 的尺寸
        let height = span_via_planes(&ctx, z)?;

        // 尺寸守卫：任一方向尺寸为 0 说明链路有环节静默失败，显式失败不返回 0 值
        let dims = [("长", length), ("宽", width), ("高", height)];
        let zero_dims: Vec<&str> = dims
            .iter()
            .filter(|item| item.1 <= 0.0)
            .map(|item| item.0)
            .collect();
        if !zero_dims.is_empty() {
            return Err(format!(
                "尺寸测量为 0 ({}), 测量链路异常, 不返回 0 值结果",
                zero_dims.join("+")
            ));
        }

        cleanup_old(part, cmd_sel, &["rulebbox"]);
        progress(&format!(
            "[bbox] 长 L={length:.3}mm  宽 W={width:.3}mm  高 H={height:.3}mm  体积 V={:.1}mm³",
            length * width * height
        ));
        Ok(MeasureFrame {
            length_axis: l_axis,
            width_axis: w_axis,
            cog: body_cog,
        })
    })();

    match attempt {
        Ok(v) => Ok(v),
        Err(e) => {
            // 失败路径同样清掉临时 RuleBbox 体，不在零件里留下半成品测量几何
            cleanup_old(part, cmd_sel, &["rulebbox"]);
            Err(e)
        }
    }
}

/// 创建 'tooling axis' 轴系（rule_bounding_box 原值：origin_type=0 纯坐标方式，
/// 不依赖几何点/方向线引用；建前删除同名旧轴系）。
fn create_coordinate_system(
    part: &Part,
    cmd_sel: &Selection,
    z: Vec3,
    x: Vec3,
    origin: Vec3,
) -> ComResult<Entity> {
    let y = norm(cross(z, x));
    if let Ok(axes) = part.axis_systems() {
        if let Ok(items) = axes.items() {
            for ax in items.iter().rev() {
                if ax
                    .name()
                    .unwrap_or_default()
                    .to_lowercase()
                    .contains("tooling axis")
                {
                    let _ = cmd_sel.clear();
                    if cmd_sel.add(ax).is_ok() {
                        let _ = cmd_sel.delete();
                    }
                }
            }
        }
    }
    let _ = cmd_sel.clear();

    let axis_sys = part.axis_systems()?.add()?;
    axis_sys.set_name("tooling axis")?;
    axis_sys.set_origin_type(0)?; // catAxisSystemOriginByCoordinates=0
    axis_sys.set_x_axis_type(1)?; // catAxisSystemAxisByCoordinates=1
    axis_sys.set_y_axis_type(1)?;
    axis_sys.set_z_axis_type(1)?;
    axis_sys.put_origin(origin)?;
    axis_sys.put_x_axis(x)?;
    axis_sys.put_y_axis(y)?;
    axis_sys.put_z_axis(z)?;
    let _ = part.update_object(&axis_sys);
    let _ = part.update_object(&axis_sys);
    axis_sys.set_is_current(true)?;
    let _ = part.update_object(&axis_sys);
    part.update()?;
    Ok(Entity::new(axis_sys.com().clone()))
}

/// 无已有 'tooling axis' 时的建立流程（对应 rule_projected_surface
/// `_establish_tooling_axis`）：draftline → Z、halfline/扫描 → X、量测 → 建系 → 读回。
pub(crate) fn establish_tooling_axis(
    prt_doc: &PartDocument,
    ss: &SystemService,
    spa: &SPAWorkbench,
    cmd_sel: &Selection,
    skip_search: bool,
    suppress_geometry: bool,
    progress: Progress<'_>,
) -> Result<AxisFrame, String> {
    // Step 1: draftline
    let dl: LineInfo = find_line_by_keyword(prt_doc, "draftline", skip_search, ss, spa, progress)?;
    // Step 1.5: halfline 方向直接作为 X 轴(长边)，否则角度扫描
    let hl = find_line_by_keyword(prt_doc, "halfline", skip_search, ss, spa, progress).ok();

    let part = prt_doc
        .part()
        .map_err(|e| format!("_establish_tooling_axis failed: {e}"))?;
    let hsf = part
        .hybrid_shape_factory()
        .map_err(|e| format!("_establish_tooling_axis failed: {e}"))?;

    // Step 2: 由 draftline 计算初始坐标系（Z=draftline 方向；Python 版把
    // 起点作初始原点, 量测后原点一律换成 body COG, 故此处无需保留）
    let z = norm(dl.direction);
    let _origin_pt = dl.start_mm.unwrap_or([0.0; 3]);
    let global_z = [0.0, 0.0, 1.0];
    let ref_vec = if dot(z, global_z).abs() > 0.99 {
        [1.0, 0.0, 0.0]
    } else {
        global_z
    };
    let x0 = norm(cross(z, ref_vec));
    let y0 = norm(cross(z, x0));

    // Step 3: 量测边界框，得到长边方向(X)与 body COG
    let measured = measure_axis_frame(
        &part,
        &hsf,
        cmd_sel,
        ss,
        spa,
        z,
        x0,
        y0,
        hl.map(|l| l.direction),
        progress,
    )?;
    let length_axis = measured.length_axis;
    let cog = measured.cog;

    // Step 4: 在零件中创建 'tooling axis' 轴系（suppress_geometry 时跳过）
    if !suppress_geometry {
        match create_coordinate_system(&part, cmd_sel, z, length_axis, cog) {
            Ok(_) => progress(&format!("[axis] create_coordinate_system origin={cog:?}")),
            Err(e) => progress(&format!("[axis] WARN create_coordinate_system failed: {e}")),
        }
    }

    // 读回实际建立的轴系（更权威）
    if let Some(ax) = find_tooling_axis(&part, ss, progress) {
        return Ok(AxisFrame {
            origin_mm: ax.origin_mm,
            x_axis: norm(ax.x_axis),
            y_axis: norm(ax.y_axis),
            z_axis: norm(ax.z_axis),
        });
    }
    Ok(AxisFrame {
        origin_mm: cog,
        x_axis: length_axis,
        y_axis: norm(cross(z, length_axis)),
        z_axis: z,
    })
}
