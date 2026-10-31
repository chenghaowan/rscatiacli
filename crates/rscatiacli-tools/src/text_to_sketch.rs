//! text-to-sketch：建模·文字/二维码转草图（轮廓 → 拉伸 → AxisToAxis）。
//!
//! 对应 ltl-manage resources/sidecars/text_to_sketch/text_to_sketch.py。
//! action=fonts 列系统 TrueType 字体; action=qr-info 只算二维码版本/模块数;
//! kind=text/qr 排出轮廓几何(GDI 字形 / QR 并集边界) → 激活零件里新建 Body →
//! 绝对 XY 平面草图(直线 CreateLine, 曲线 CreateSpline 极点置构造元素) →
//! AddNewPad 拉伸 → AddNewAxisToAxis2 从绝对轴系转到 tooling 轴系。
//! 重跑覆盖同名 Body(删前把 in-work 挪回 PartBody)。

use std::time::Instant;

use rscatiacli::objects::{
    Application, Factory2D, Part, PartDocument, SPAWorkbench, Selection, SystemService,
};
use rscatiacli::{attach_with_guard, ComCollection, ComObject, ComResult, Progress};
use serde_json::{json, Map, Value};

use crate::doc_link::ensure_part_doc;
use crate::qr_outline;
use crate::text_outline::{self, Segment, TextOutline, TextOutlineError};
use crate::vba_bbox_generate::{command_selection, restore_selection, snapshot_selection};

const DEFAULT_FONT: &str = "Arial";
const DEFAULT_HEIGHT_MM: f64 = 5.0;
const DEFAULT_THICKNESS_MM: f64 = 1.0;
const DEFAULT_AXIS_KEY: &str = "tooling";
const DEFAULT_QR_MODULE_MM: f64 = 0.6; // 单模块(方格)边长缺省值: 0.6mm(识别下限)
const DEFAULT_QR_EC: &str = "M";

fn terr(msg: impl Into<String>) -> TextOutlineError {
    TextOutlineError(msg.into())
}

/// 参数取数（数字或数字字符串 → f64）。
fn f64_of(v: &Value) -> Option<f64> {
    v.as_f64()
        .or_else(|| v.as_str().and_then(|s| s.trim().parse().ok()))
}

fn param_f64(params: &Value, key: &str, default: Value) -> Value {
    params.get(key).cloned().unwrap_or(default)
}

fn is_blank(v: &Value) -> bool {
    v.is_null() || v.as_str().map(|s| s.trim().is_empty()).unwrap_or(false)
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

/// Python `{:g}` 的紧凑数字格式（去尾零; 1.0 → "1", 0.6 → "0.6"）。
fn g_num(v: f64) -> String {
    let s = format!("{v:.6}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

// ── Body 名清理 / 轴系查找 ───────────────────────────────────────────

/// Body 名: 只留字母/数字/中文/空格/-/_/., 其余换成下划线并压掉重复, 限长 24。
fn sanitize_body_name(text: &str, prefix: &str) -> String {
    let replaced: String = text
        .replace(['\r', '\n', '\t'], " ")
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || " .-_".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect();
    let mut s = replaced;
    while s.contains("__") {
        s = s.replace("__", "_");
    }
    let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut s = s
        .trim_matches(|c: char| c == ' ' || c == '.' || c == '-' || c == '_')
        .to_string();
    if s.is_empty() {
        s = prefix.to_string();
    }
    if s.chars().count() > 24 {
        s = s
            .chars()
            .take(24)
            .collect::<String>()
            .trim_matches(|c: char| c == ' ' || c == '.' || c == '-' || c == '_')
            .to_string();
    }
    format!("{prefix}_{s}")
}

fn norm_key(s: &str) -> String {
    s.to_lowercase().replace([' ', '_', '-'], "")
}

/// 按名字关键字找轴系（去空格/下划线/连字符后包含）。
fn find_axis_system(
    part: &Part,
    key: &str,
) -> Result<Option<(rscatiacli::objects::AxisSystem, String)>, String> {
    let key_n = {
        let k = norm_key(key);
        if k.is_empty() {
            "tooling".to_string()
        } else {
            k
        }
    };
    let axes = match part.axis_systems() {
        Ok(a) => a,
        Err(_) => return Ok(None),
    };
    let Ok(items) = axes.items() else {
        return Ok(None);
    };
    for ax in items.iter() {
        let name = ax.name().unwrap_or_default();
        if key_n.contains(&norm_key(&name)) || norm_key(&name).contains(&key_n) {
            return Ok(Some((ax.clone(), name)));
        }
    }
    Ok(None)
}

/// 绝对轴系: 按名字找(Absolute Axis System / 绝对轴系), 找不到退回集合第 1 个
/// —— CATPart 自带绝对轴系, 实测新建零件 axis_systems.count==1 就是它。
fn find_absolute_axis(
    part: &Part,
) -> Result<Option<(rscatiacli::objects::AxisSystem, String)>, String> {
    let axes = match part.axis_systems() {
        Ok(a) => a,
        Err(_) => return Ok(None),
    };
    let Ok(items) = axes.items() else {
        return Ok(None);
    };
    let mut first: Option<(rscatiacli::objects::AxisSystem, String)> = None;
    for ax in items.iter() {
        let name = ax.name().unwrap_or_default();
        let low = name.to_lowercase();
        if low.contains("absolute") || name.contains("绝对") {
            return Ok(Some((ax.clone(), name)));
        }
        if first.is_none() {
            first = Some((ax.clone(), name));
        }
    }
    Ok(first)
}

/// 删掉同名 Body(重跑覆盖旧结果)。删前先把 in-work object 挪回 PartBody ——
/// 上次运行留下的 in-work body 正是要删的这一个, 直接删可能导致 CATIA 报错。
fn delete_same_named_body(part: &Part, cmd_sel: &Selection, name: &str) -> bool {
    // in-work 保护
    if let Ok(iw) = part.com().get("InWorkObject").and_then(|v| v.as_obj()) {
        let iw_name = iw
            .get("Name")
            .and_then(|n| n.as_string())
            .unwrap_or_default();
        if iw_name == name {
            if let Ok(bodies) = part.bodies() {
                if let Ok(items) = bodies.items() {
                    if let Some(first) = items.first() {
                        let _ = part.set_in_work_object(first);
                    }
                }
            }
        }
    }
    let Ok(bodies) = part.bodies() else {
        return false;
    };
    let Ok(items) = bodies.items() else {
        return false;
    };
    for b in items.iter() {
        if b.name().unwrap_or_default() != name {
            continue;
        }
        let _ = cmd_sel.clear();
        if cmd_sel.add(b).is_ok() {
            let _ = cmd_sel.delete();
        }
        let _ = cmd_sel.clear();
        return true;
    }
    false
}

// ── 草图几何 ─────────────────────────────────────────────────────────

/// 把轮廓图元写进草图: 直线段 CreateLine, 折线 CreateSpline(极点设构造元素)。
/// 返回 (直线数, 样条数, 极点总数)。
fn add_segments(f2d: &Factory2D, segments: &[Segment]) -> Result<(usize, usize, usize), String> {
    let mut n_line = 0usize;
    let mut n_curve = 0usize;
    let mut n_pole = 0usize;
    for seg in segments {
        match seg {
            Segment::Line((x0, y0), (x1, y1)) => {
                f2d.create_line(*x0, *y0, *x1, *y1)
                    .map_err(|e| e.to_string())?;
                n_line += 1;
            }
            Segment::Curve(poly) => {
                let mut poles = Vec::with_capacity(poly.len());
                for (x, y) in poly {
                    let p = f2d.create_point(*x, *y).map_err(|e| e.to_string())?;
                    // 极点只是样条控制点, 不参与轮廓
                    let _ = p.set_construction(true);
                    poles.push(p);
                }
                f2d.create_spline(&poles).map_err(|e| e.to_string())?;
                n_pole += poles.len();
                n_curve += 1;
            }
        }
    }
    Ok((n_line, n_curve, n_pole))
}

/// Body 重心(mm); 量不到返回 None。
fn body_cog(
    part: &Part,
    body: &rscatiacli::objects::Body,
    spa: &SPAWorkbench,
    ss: &SystemService,
) -> Option<Vec<f64>> {
    let r = part.create_reference_from_object(body).ok()?;
    let m = spa.get_measurable(&r).ok()?;
    let cog = m.get_cog(ss).ok()?;
    Some(cog.iter().map(|v| round3(*v)).collect())
}

// ── 建几何 ───────────────────────────────────────────────────────────

/// 在激活零件里建 Body → 草图 → 拉伸 → AxisToAxis（Python _build_geometry）。
#[allow(clippy::too_many_arguments)]
fn build_geometry(
    app: &Application,
    prt_doc: &PartDocument,
    outline: &TextOutline,
    thickness: f64,
    body_name: &str,
    axis_key: &str,
    cmd_sel: &Selection,
    ss: &SystemService,
    progress: Progress<'_>,
) -> Result<Map<String, Value>, String> {
    let part = prt_doc.part().map_err(|e| e.to_string())?;
    let part_name = part.name().unwrap_or_else(|_| "Part".to_string());
    let _ = prt_doc.activate();

    // 目标轴系: tooling axis(结果要按它定位); 绝对轴系作 AxisToAxis 的参考
    let Some((target_ax, target_name)) =
        find_axis_system(&part, axis_key).map_err(|e| e.to_string())?
    else {
        return Err(format!(
            "零件「{part_name}」里没有名字含「{axis_key}」的轴系, 无法把文字转到 tooling 坐标; \
             请先用 建模→边界框/投影面积 工具生成 tooling axis(或手动建一个名字含 tooling 的轴系)"
        ));
    };
    let Some((abs_ax, abs_name)) = find_absolute_axis(&part).map_err(|e| e.to_string())? else {
        return Err(format!(
            "零件「{part_name}」里取不到绝对轴系, 无法做 AxisToAxis"
        ));
    };

    let spa = prt_doc.spa_workbench().map_err(|e| e.to_string())?;

    // 1) 覆盖同名 Body
    if delete_same_named_body(&part, cmd_sel, body_name) {
        progress(&format!(
            "已删除同名 Body「{body_name}」(重跑覆盖上一次结果)"
        ));
        let _ = part.update();
    }

    // 2) 新建 Body + 绝对 XY 平面草图
    let body = part
        .bodies()
        .and_then(|b| b.add())
        .map_err(|e| e.to_string())?;
    body.set_name(body_name).map_err(|e| e.to_string())?;
    let _ = part.set_in_work_object(&body);
    let plane_xy = part
        .origin_elements()
        .and_then(|o| o.plane_xy())
        .map_err(|e| e.to_string())?;
    let plane_ref = part
        .create_reference_from_object(&plane_xy)
        .map_err(|e| e.to_string())?;
    let sketch = body
        .sketches()
        .and_then(|s| s.add(&plane_ref))
        .map_err(|e| e.to_string())?;
    let f2d = sketch.open_edition().map_err(|e| e.to_string())?;
    let (n_line, n_curve, n_pole) =
        add_segments(&f2d, &outline.segments).map_err(|e| e.to_string())?;
    let _ = sketch.close_edition();
    let _ = part.update();
    let sketch_name = sketch.name().unwrap_or_default();
    progress(&format!(
        "草图已生成: {sketch_name}(直线 {n_line}, 样条 {n_curve}, 极点 {n_pole})"
    ));

    // 3) 拉伸(沿绝对 +Z)
    let sf = part.shape_factory().map_err(|e| e.to_string())?;
    let pad = sf.add_new_pad(&sketch, thickness).map_err(|e| {
        format!(
            "文字轮廓已写进草图「{sketch_name}」, 但拉伸失败: {e} —— \
             多为字体轮廓笔画自相交(中文/手写体常见), 可换字体(如 仿宋)后重试, \
             或手动进草图用 Trim 去掉重叠笔画"
        )
    })?;
    let _ = part.update();
    let pad_name = pad.name().unwrap_or_default();
    progress(&format!("已拉伸 {}mm: {pad_name}", g_num(thickness)));

    let cog_before = body_cog(&part, &body, &spa, ss);

    // 4) AxisToAxis: 绝对轴系 → tooling axis
    //    成功时 COM 返回值也是 None(动态调度), 用「Body 内形状数 + 重心」验证结果
    let shapes_before = body.shapes().and_then(|s| s.count()).unwrap_or(0);
    progress(&format!(
        "正在执行 AxisToAxis: 绝对轴系「{abs_name}」→ 工具轴系「{target_name}」..."
    ));
    let ref_abs = part
        .create_reference_from_object(&abs_ax)
        .map_err(|e| e.to_string())?;
    let ref_target = part
        .create_reference_from_object(&target_ax)
        .map_err(|e| e.to_string())?;
    let a2a_err = match sf.add_new_axis_to_axis2(&ref_abs, &ref_target) {
        Ok(()) => String::new(),
        Err(e) => format!("{e}"),
    };
    let _ = part.update();
    let shapes_after = body.shapes().and_then(|s| s.count()).unwrap_or(0);
    let transferred = shapes_after > shapes_before;
    let cog_after = body_cog(&part, &body, &spa, ss);
    let mut a2a_name = String::new();
    if transferred {
        if let Ok(sh) = body.shapes().and_then(|s| s.item(shapes_after)) {
            a2a_name = sh.name().unwrap_or_default();
        }
        progress(&format!(
            "AxisToAxis 完成: {a2a_name}(重心 {cog_before:?} → {cog_after:?})"
        ));
    } else {
        let err_note = if a2a_err.is_empty() {
            String::new()
        } else {
            format!("({a2a_err})")
        };
        progress(&format!(
            "⚠ AxisToAxis 未生效{err_note}: 文字仍在绝对坐标, 请手动 Insert→Transformation→Axis to Axis"
        ));
    }

    // 5) 视图刷新(看得见结果)
    if let Ok(window) = app.active_window() {
        if let Ok(viewer) = window.active_viewer() {
            let _ = viewer.reframe();
            let _ = viewer.zoom_in();
        }
    }

    let mut m = Map::new();
    m.insert("part_name".into(), json!(part_name));
    m.insert("body".into(), json!(body.name().unwrap_or_default()));
    m.insert("sketch".into(), json!(sketch_name));
    m.insert("pad".into(), json!(pad_name));
    m.insert("sketch_lines".into(), json!(n_line));
    m.insert("sketch_curves".into(), json!(n_curve));
    m.insert("sketch_poles".into(), json!(n_pole));
    m.insert("axis_reference".into(), json!(abs_name));
    m.insert("axis_target".into(), json!(target_name));
    m.insert("transferred".into(), json!(transferred));
    m.insert("transform_feature".into(), json!(a2a_name));
    m.insert("transform_error".into(), json!(a2a_err));
    m.insert(
        "cog_before".into(),
        cog_before.map(|c| json!(c)).unwrap_or(Value::Null),
    );
    m.insert(
        "cog_after".into(),
        cog_after.map(|c| json!(c)).unwrap_or(Value::Null),
    );
    Ok(m)
}

// ── 二维码模块边长 ───────────────────────────────────────────────────

/// 取单模块边长(mm/格): 优先 `module`; 兼容旧参数 `size`(按内容模块数反推);
/// 都没给就用缺省 0.6mm。
fn qr_module_param(params: &Value, content: &str, ec: &str) -> Result<f64, TextOutlineError> {
    let raw = params.get("module").unwrap_or(&Value::Null);
    if !is_blank(raw) {
        return f64_of(raw).ok_or_else(|| terr(format!("单模块边长不是有效数字: {raw:?}")));
    }
    let legacy = params.get("size").unwrap_or(&Value::Null);
    if !is_blank(legacy) {
        let Some(sz) = f64_of(legacy) else {
            return Ok(DEFAULT_QR_MODULE_MM);
        };
        let Ok((_, cols)) = qr_outline::qr_metrics(content, ec) else {
            return Ok(DEFAULT_QR_MODULE_MM);
        };
        return Ok(if cols > 0 {
            sz / cols as f64
        } else {
            DEFAULT_QR_MODULE_MM
        });
    }
    Ok(DEFAULT_QR_MODULE_MM)
}

// ── 主流程 ───────────────────────────────────────────────────────────

pub fn run(params: &Value, progress: Progress<'_>) -> ComResult<Value> {
    let action = params
        .get("action")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_lowercase();
    if action == "fonts" || action == "list-fonts" {
        let fonts = text_outline::available_fonts();
        return Ok(json!({
            "success": true,
            "message": format!("可用 TrueType 字体 {} 个", fonts.len()),
            "data": { "fonts": fonts },
        }));
    }
    if action == "qr-info" || action == "qr-metrics" {
        // 只算版本/模块数/整体尺寸(不做轮廓追踪), 供前端实时提示
        let content = params.get("content").and_then(|v| v.as_str()).unwrap_or("");
        let ec = params
            .get("ec")
            .and_then(|v| v.as_str())
            .unwrap_or(DEFAULT_QR_EC);
        let module_mm = match qr_module_param(params, content, ec) {
            Ok(v) => v,
            Err(e) => return Ok(json!({ "success": false, "message": e.0, "data": {} })),
        };
        let (version, cols) = match qr_outline::qr_metrics(content, ec) {
            Ok(v) => v,
            Err(e) => return Ok(json!({ "success": false, "message": e.0, "data": {} })),
        };
        let size_mm = cols as f64 * module_mm;
        let mut message =
            format!("v{version} {cols}×{cols} 模块; 单模块 {module_mm:.3}mm → 整体 {size_mm:.1}mm");
        if module_mm < qr_outline::MIN_MODULE_MM {
            message.push_str(&format!(
                "(单模块 < {}mm, 不好扫)",
                qr_outline::MIN_MODULE_MM
            ));
        }
        return Ok(json!({
            "success": true,
            "message": message,
            "data": {
                "kind": "qr",
                "version": version,
                "module_count": cols,
                "modules": format!("{cols}×{cols}"),
                "module_mm": round3(module_mm),
                "size_mm": round3(size_mm),
                "min_module_mm": qr_outline::MIN_MODULE_MM,
                "quiet_zone_mm": round3(module_mm * qr_outline::QUIET_MODULES as f64),
                "ec": ec.trim().to_uppercase().chars().next().unwrap_or('M').to_string(),
                "too_small": module_mm < qr_outline::MIN_MODULE_MM - 1e-9,
            },
        }));
    }

    let kind = params
        .get("kind")
        .and_then(|v| v.as_str())
        .unwrap_or("text")
        .trim()
        .to_lowercase();
    let is_qr = matches!(kind.as_str(), "qr" | "qrcode" | "二维码");
    let text = params.get("text").and_then(|v| v.as_str()).unwrap_or("");
    let font = params
        .get("font")
        .and_then(|v| v.as_str())
        .unwrap_or(DEFAULT_FONT)
        .trim()
        .to_string();
    let font = if font.is_empty() {
        DEFAULT_FONT.to_string()
    } else {
        font
    };
    let height_v = param_f64(params, "height", json!(DEFAULT_HEIGHT_MM));
    let letter_spacing = f64_of(&param_f64(params, "letter_spacing", json!(0))).unwrap_or(0.0);
    let line_spacing_v = params.get("line_spacing").cloned().unwrap_or(Value::Null);
    let line_spacing = if is_blank(&line_spacing_v) {
        None
    } else {
        f64_of(&line_spacing_v)
    };
    let content = params
        .get("content")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(text)
        .to_string();
    let ec = params
        .get("ec")
        .and_then(|v| v.as_str())
        .unwrap_or(DEFAULT_QR_EC);
    let thickness = f64_of(&param_f64(params, "thickness", json!(DEFAULT_THICKNESS_MM)))
        .unwrap_or(DEFAULT_THICKNESS_MM);
    let bold = params
        .get("bold")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let axis_key = params
        .get("axis_key")
        .and_then(|v| v.as_str())
        .unwrap_or(DEFAULT_AXIS_KEY)
        .trim()
        .to_string();
    let axis_key = if axis_key.is_empty() {
        DEFAULT_AXIS_KEY.to_string()
    } else {
        axis_key
    };
    let dry_run = params
        .get("dry_run")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let body_name = params
        .get("body_name")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| sanitize_body_name(&content, if is_qr { "QR" } else { "TEXT" }));

    let t0 = Instant::now();
    let kind_str = if is_qr { "qr" } else { "text" };
    let empty_data = json!({
        "kind": kind_str, "text": text, "content": content, "font": if is_qr { "" } else { &font },
        "part_name": "", "body": "", "sketch": "", "pad": "",
        "axis_reference": "", "axis_target": "", "transferred": false,
        "width_mm": 0.0, "height_mm": 0.0, "char_height_mm": 0.0,
        "contours": 0, "segments": 0, "sketch_lines": 0, "sketch_curves": 0, "sketch_poles": 0,
        "qr_version": null, "qr_ec": null, "qr_modules": null, "qr_module_mm": null,
        "qr_module_count": null, "qr_size_mm": null,
        "qr_quiet_zone_mm": null, "warning": null, "note": null,
        "dry_run": false,
    });
    let empty_data: Map<String, Value> = match empty_data {
        Value::Object(m) => m,
        _ => unreachable!(),
    };

    // 轮廓计算（CATIA 之前, dry_run 也不连 CATIA）
    let outline = (|| -> Result<TextOutline, String> {
        if is_qr {
            let module = qr_module_param(params, &content, ec).map_err(|e| e.0)?;
            qr_outline::build_qr_outline(&content, module, ec).map_err(|e| e.0)
        } else {
            let Some(height_mm) = f64_of(&height_v) else {
                return Err(format!("字高不是有效数字: {height_v:?}"));
            };
            // 离散容差随字高走(小字细容差、大字粗容差, 控制极点数量)
            let tol_mm = (height_mm * 0.01).clamp(0.01, 0.3);
            text_outline::build_text_outline(
                text,
                &font,
                height_mm,
                letter_spacing,
                line_spacing,
                bold,
                false,
                tol_mm,
            )
            .map_err(|e| e.0)
        }
    })();
    let outline = match outline {
        Ok(o) => o,
        Err(e) => return Ok(json!({ "success": false, "message": e, "data": empty_data })),
    };

    let info = outline.info();
    let mut base = empty_data.clone();
    for key in [
        "kind",
        "font",
        "width_mm",
        "height_mm",
        "char_height_mm",
        "contours",
        "segments",
    ] {
        if let Some(v) = info.get(key) {
            base.insert(key.to_string(), v.clone());
        }
    }
    base.insert("thickness_mm".into(), json!(thickness));
    base.insert("letter_spacing_mm".into(), json!(letter_spacing));
    base.insert("body_name_planned".into(), json!(body_name));
    base.insert(
        "outline_seconds".into(),
        json!(round2(t0.elapsed().as_secs_f64())),
    );
    base.insert("dry_run".into(), json!(dry_run));
    for key in [
        "version",
        "ec",
        "modules",
        "module_mm",
        "module_count",
        "size_mm",
        "quiet_zone_mm",
    ] {
        if let Some(v) = info.get(key) {
            base.insert(format!("qr_{key}"), v.clone());
        }
    }
    base.insert(
        "warning".into(),
        info.get("warning").cloned().unwrap_or(Value::Null),
    );
    base.insert(
        "note".into(),
        info.get("note").cloned().unwrap_or(Value::Null),
    );

    let short_content: String = content.chars().take(40).collect();
    let desc = if is_qr {
        format!(
            "二维码「{short_content}」({} 模块, 模块 {}mm, 纠错 {})",
            info.get("modules").and_then(|v| v.as_str()).unwrap_or("?"),
            info.get("module_mm")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0),
            info.get("ec").and_then(|v| v.as_str()).unwrap_or("?"),
        )
    } else {
        format!("文字「{text}」用字体「{}」", outline.font)
    };

    if let Some(warning) = info.get("warning").and_then(|w| w.as_str()) {
        progress(&format!("⚠ {warning}"));
    }

    if dry_run {
        let message = format!(
            "[dry-run] 未修改模型: {desc} 排成 {}×{}mm, {} 个闭合轮廓 / {} 段几何(直线 {}, 曲线 {})",
            info.get("width_mm").and_then(|v| v.as_f64()).unwrap_or(0.0),
            info.get("height_mm").and_then(|v| v.as_f64()).unwrap_or(0.0),
            info.get("contours").and_then(|v| v.as_u64()).unwrap_or(0),
            info.get("segments").and_then(|v| v.as_u64()).unwrap_or(0),
            info.get("lines").and_then(|v| v.as_u64()).unwrap_or(0),
            info.get("curves").and_then(|v| v.as_u64()).unwrap_or(0),
        );
        return Ok(json!({ "success": true, "message": message, "data": base }));
    }

    let result = (|| -> ComResult<Value> {
        let app = attach_with_guard(progress)?;
        // 目标文档: 激活的 .CATPart(图纸/装配时按同名联动切换)
        let prt_doc: PartDocument = match app.active_document() {
            Ok(doc) => {
                if doc
                    .name()
                    .unwrap_or_default()
                    .to_lowercase()
                    .ends_with(".catpart")
                {
                    doc.part_document()
                } else {
                    match ensure_part_doc(&app, progress) {
                        Ok(d) => d.part_document(),
                        Err(e) => {
                            return Ok(json!({
                                "success": false, "message": format!("请先打开零件(.CATPart): {e}"), "data": base
                            }))
                        }
                    }
                }
            }
            Err(e) => {
                return Ok(json!({
                    "success": false, "message": format!("获取活动文档失败: {e}"), "data": base
                }))
            }
        };

        // 命令类操作(删同名 Body)落在激活编辑器的选中集上
        let (cmd_sel, borrowed_sel) = command_selection(&app, &prt_doc)?;
        let sel_snapshot = if borrowed_sel {
            snapshot_selection(&cmd_sel)
        } else {
            Vec::new()
        };

        let alerts_prev = app.display_file_alerts().unwrap_or(true);
        let _ = app.set_display_file_alerts(false);
        progress(&format!(
            "{desc} {}×{}mm, 轮廓 {} 个",
            info.get("width_mm").and_then(|v| v.as_f64()).unwrap_or(0.0),
            info.get("height_mm")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0),
            info.get("contours").and_then(|v| v.as_u64()).unwrap_or(0),
        ));
        let ss = app.system_service()?;
        let build = build_geometry(
            &app, &prt_doc, &outline, thickness, &body_name, &axis_key, &cmd_sel, &ss, progress,
        );
        let _ = app.set_display_file_alerts(alerts_prev);
        if borrowed_sel {
            restore_selection(&cmd_sel, &sel_snapshot);
        }
        let res = match build {
            Ok(v) => v,
            Err(e) => return Ok(json!({ "success": false, "message": e, "data": base })),
        };
        for (k, v) in &res {
            base.insert(k.clone(), v.clone());
        }
        let transferred = res
            .get("transferred")
            .and_then(|b| b.as_bool())
            .unwrap_or(false);
        let axis_target = res
            .get("axis_target")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let a2a_note = if transferred {
            format!("已 AxisToAxis 到「{axis_target}」")
        } else {
            "⚠ AxisToAxis 未生效(结果仍在绝对坐标)".to_string()
        };
        let what = if is_qr {
            format!(
                "二维码「{short_content}」({} 模块, 模块 {}mm, 纠错 {})",
                info.get("modules").and_then(|v| v.as_str()).unwrap_or("?"),
                info.get("module_mm")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0),
                info.get("ec").and_then(|v| v.as_str()).unwrap_or("?"),
            )
        } else {
            format!(
                "文字「{text}」({}, 字高 {}mm, 字间距 {}mm)",
                outline.font,
                g_num(
                    info.get("char_height_mm")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0)
                ),
                g_num(letter_spacing),
            )
        };
        let body = res.get("body").and_then(|v| v.as_str()).unwrap_or("");
        let note_suffix = match info.get("note").and_then(|v| v.as_str()) {
            Some(n) if !n.is_empty() => format!("{n}。"),
            _ => String::new(),
        };
        let message = format!(
            "{}转草图完成: {what} 已生成 {} 个轮廓 + {}mm 拉伸, 存放在新建 Body「{body}」; {a2a_note}。{note_suffix}文档未保存, 需要时请自行另存",
            if is_qr { "二维码" } else { "文字" },
            info.get("contours").and_then(|v| v.as_u64()).unwrap_or(0),
            g_num(thickness),
        );
        Ok(json!({ "success": true, "message": message, "data": base }))
    })();

    match result {
        Ok(v) => Ok(v),
        Err(e) => Ok(json!({
            "success": false,
            "message": format!("text_to_sketch failed: {e}"),
            "data": empty_data,
        })),
    }
}
