//! capture_active_part —— CATIA 截图三模式（capture_active_part.py 的 Rust 版）。
//!
//! 命令（JSON 入参 `mode` 分发）：
//!   iso       Part 隐藏辅助元素(轴系/平面/其他 Body/图形集)后 ISO 白底截图；
//!             Product 整个装配按 ISO 截一张（与 Part 一致）。
//!   direct    保持当前视角/缩放不动，仅隐藏规范树 + 白底。
//!   per-part  装配逐个打开 Part 截 ISO；名字含 "tooling" 的轴系设为 current，
//!             并按其 X/Y/Z/ISO 四方向分别截图。
//!
//! 与 Python 版的差异（有意为之）：
//! - 输出 PNG 而非 WebP（image crate 不带 WebP 编码；前端 data_url 按 mime
//!   适配，显示无差异），管线同口径：近白统一 → 内容裁剪 → 擦指南针 →
//!   方形白底 512×512（measure_capture::process_capture 实测通道）；
//! - 旧 AVIF 管线不保留（--output .avif 的兼容分支）。

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use rscatiacli::objects::Document;
use rscatiacli::{attach_with_guard, ComCollection, ComObject as _, ComResult, Progress};

use crate::batch_analysis;
use crate::measure_capture;

const SHOT_MAX_SIZE: u32 = 512;

/// 一张截图的完整通道：视角 → 白底 → 截图 → 后处理 → PNG 落盘。
#[allow(clippy::too_many_arguments)]
fn capture_one(
    _win: &rscatiacli::objects::Window,
    viewer: &rscatiacli::objects::Viewer3D,
    out_path: &Path,
    sight: Option<[f64; 3]>,
    up: [f64; 3],
    reframe: bool,
    zoom_factor: f64,
    progress: &dyn Fn(&str),
) -> Result<(f64, String), String> {
    if let Some(sight) = sight {
        let vp = viewer
            .viewpoint3d()
            .map_err(|e| format!("无 Viewpoint3D: {e}"))?;
        let _ = vp.put_sight_direction(sight);
        let _ = vp.put_up_direction(up);
    }
    let _ = viewer.put_background_color([1.0, 1.0, 1.0]);
    if reframe {
        let _ = viewer.reframe();
        // Reframe 后缩放微调（Python _reframe_zoom：vp.Zoom *= zoom_factor，默认 1.0 不动）
        if (zoom_factor - 1.0).abs() > f64::EPSILON {
            if let Ok(vp) = viewer.viewpoint3d() {
                if let Ok(current) = vp.zoom() {
                    let _ = vp.set_zoom(current * zoom_factor);
                    progress(&format!("🔍 缩放: Reframe×{zoom_factor} (原={current:.4})"));
                }
            }
        }
    }
    let _ = viewer.update();
    viewer
        .capture_to_file(4, &out_path.to_string_lossy())
        .map_err(|e| format!("CaptureToFile 失败: {e}"))?;
    let _ = viewer.put_background_color([0.2, 0.2, 0.4]);

    // 后处理：process_capture（近白统一/内容裁剪/方形白底 512）→ PNG
    let img = image::open(out_path).map_err(|e| format!("打开截图失败: {e}"))?;
    let rgb = img.to_rgb8();
    let (final_img, _tf) = measure_capture::process_capture(&rgb, SHOT_MAX_SIZE);
    final_img
        .save_with_format(out_path, image::ImageFormat::Png)
        .map_err(|e| format!("保存 PNG 失败: {e}"))?;

    let size_kb = std::fs::metadata(out_path)
        .map(|m| (m.len() as f64 / 1024.0 * 10.0).round() / 10.0)
        .unwrap_or(0.0);
    let name = out_path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    progress(&format!(
        "[PNG] {name} | {size_kb:.1}KB | {SHOT_MAX_SIZE}x{SHOT_MAX_SIZE}"
    ));
    Ok((size_kb, out_path.to_string_lossy().to_string()))
}

fn hide_part_aux_elements(doc: &Document) {
    let Ok(part) = doc.part_document().part() else {
        return;
    };
    let Ok(sel) = doc.selection() else {
        return;
    };
    let hide = |objs: Vec<rscatiacli::objects::Entity>| {
        if objs.is_empty() {
            return;
        }
        let _ = sel.clear();
        for o in &objs {
            let _ = sel.add(o);
        }
        if let Ok(vp) = sel.vis_properties() {
            let _ = vp.set_show(1);
        }
        let _ = sel.clear();
    };
    if let Ok(items) = part.axis_systems().and_then(|c| c.items()) {
        hide(
            items
                .into_iter()
                .map(|a| rscatiacli::objects::Entity::new(a.com().clone()))
                .collect(),
        );
    }
    if let Ok(items) = part.planes().and_then(|c| c.items()) {
        hide(items);
    }
    let main_name = part.main_body().and_then(|b| b.name()).ok();
    if let Ok(items) = part.bodies().and_then(|c| c.items()) {
        let others: Vec<rscatiacli::objects::Entity> = items
            .into_iter()
            .filter(|b| main_name.as_deref() != b.name().ok().as_deref())
            .map(|b| rscatiacli::objects::Entity::new(b.com().clone()))
            .collect();
        hide(others);
    }
    if let Ok(items) = part.hybrid_bodies().and_then(|c| c.items()) {
        hide(
            items
                .into_iter()
                .map(|h| rscatiacli::objects::Entity::new(h.com().clone()))
                .collect(),
        );
    }
}

/// 输出目录：params.output_dir 优先，否则文档所在目录。
fn output_dir_of(params: &Value, doc: &Document) -> PathBuf {
    if let Some(d) = params
        .get("output_dir")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
    {
        return PathBuf::from(d);
    }
    doc.full_name()
        .ok()
        .and_then(|fp| Path::new(&fp).parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default())
}

fn doc_stem(doc: &Document) -> String {
    doc.name()
        .ok()
        .and_then(|n| {
            Path::new(&n)
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
        })
        .unwrap_or_else(|| "capture".into())
}

fn iso_or_direct(params: &Value, progress: Progress<'_>, iso: bool) -> ComResult<Value> {
    let mode = if iso { "iso" } else { "direct" };
    let app = attach_with_guard(progress)?;
    let doc = app
        .active_document()
        .map_err(|_| rscatiacli::ComError::runtime("No active document."))?;
    if iso && doc.part_document().part().is_err() && doc.product_document().product().is_err() {
        return Ok(json!({
            "success": false,
            "message": "不支持的文档类型，仅支持 PartDocument 和 ProductDocument",
        }));
    }

    let win = app
        .active_window()
        .map_err(|e| rscatiacli::ComError::runtime(format!("无活动窗口: {e}")))?;
    let viewer = win
        .active_viewer()
        .map_err(|e| rscatiacli::ComError::runtime(format!("无查看器: {e}")))?;
    let orig_layout = win.layout().ok();
    let _ = win.set_layout(1); // 隐藏规范树

    if iso {
        // Part 隐藏辅助元素（Product 不隐藏，整装配一致口径）
        hide_part_aux_elements(&doc);
    }

    let out_dir = output_dir_of(params, &doc);
    let _ = std::fs::create_dir_all(&out_dir);
    let out_path = out_dir.join(format!("{}_{}.png", doc_stem(&doc), mode));

    // iso: ISO 视角 + Reframe；direct: 保持当前视角（sight=None、不 Reframe）
    let (sight, up, reframe) = if iso {
        (Some([-1.0, -1.0, -1.0]), [0.0, 0.0, 1.0], true)
    } else {
        (None, [0.0, 0.0, 1.0], false)
    };
    let zoom_factor = params
        .get("zoom_factor")
        .and_then(|v| v.as_f64())
        .unwrap_or(1.0);
    let capture = capture_one(
        &win,
        &viewer,
        &out_path,
        sight,
        up,
        reframe,
        zoom_factor,
        &progress,
    );

    if let Some(l) = orig_layout {
        let _ = win.set_layout(l);
    }

    let (size_kb, file) = capture.map_err(rscatiacli::ComError::runtime)?;
    Ok(json!({
        "success": true,
        "file": file,
        "size_kb": size_kb,
        "mode": mode,
    }))
}

/// tooling 轴系：名字含 tooling（忽略大小写/空格/_/-），找到则设为 current
/// 并返回四方向 (X/Y/Z/ISO, sight, up)。
type AxisMeasures = Vec<(String, [f64; 3], [f64; 3])>;

fn tooling_directions(
    part: &rscatiacli::objects::Part,
    ss: &rscatiacli::objects::SystemService,
    progress: &dyn Fn(&str),
) -> Option<AxisMeasures> {
    let axes = part.axis_systems().ok()?;
    for axis in axes.items().ok()? {
        let raw = axis.name().unwrap_or_default();
        let normalized: String = raw
            .to_lowercase()
            .chars()
            .filter(|c| !matches!(c, ' ' | '_' | '-'))
            .collect();
        if normalized.contains("tooling") {
            progress(&format!("🧭 找到 tooling axis: {raw}"));
            let _ = axis.set_current(true);
            let Ok((x, y)) = axis.get_vectors(ss) else {
                progress("⚠️ 读取轴系方向失败, 回退全局方向");
                return None;
            };
            // 正交化（对齐 Python：z = x×y 归一，y = z×x 重正交）
            let norm = |v: [f64; 3]| -> [f64; 3] {
                let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
                [v[0] / len, v[1] / len, v[2] / len]
            };
            let cross = |a: [f64; 3], b: [f64; 3]| -> [f64; 3] {
                [
                    a[1] * b[2] - a[2] * b[1],
                    a[2] * b[0] - a[0] * b[2],
                    a[0] * b[1] - a[1] * b[0],
                ]
            };
            let x = norm(x);
            let mut z = norm(cross(x, y));
            let y = norm(cross(z, x));
            z = norm(z);
            progress(&format!(
                "🧭 轴系向量 X={:?} Y={:?}",
                x.iter()
                    .map(|v| (v * 1000.0).round() / 1000.0)
                    .collect::<Vec<_>>(),
                y.iter()
                    .map(|v| (v * 1000.0).round() / 1000.0)
                    .collect::<Vec<_>>(),
            ));
            let neg = |v: [f64; 3]| [-v[0], -v[1], -v[2]];
            let iso_sight = neg(norm([
                x[0] + y[0] + z[0],
                x[1] + y[1] + z[1],
                x[2] + y[2] + z[2],
            ]));
            return Some(vec![
                ("X".into(), neg(x), z),
                ("Y".into(), neg(y), z),
                ("Z".into(), neg(z), y),
                ("ISO".into(), iso_sight, z),
            ]);
        }
    }
    None
}

fn per_part(params: &Value, progress: Progress<'_>) -> ComResult<Value> {
    let app = attach_with_guard(progress)?;
    let doc = app
        .active_document()
        .map_err(|_| rscatiacli::ComError::runtime("No active document."))?;
    let product = doc
        .product_document()
        .product()
        .map_err(|_| rscatiacli::ComError::runtime("per-part 模式需要活动 Product 文档"))?;
    let docs = app.documents()?;

    let mut part_paths: Vec<(String, String)> = Vec::new();
    batch_analysis::collect_parts(&product, &docs, &mut part_paths, &progress);
    progress(&format!("[列表] 共收集 {} 个 Part 路径", part_paths.len()));

    // 去重
    let mut seen: Vec<String> = Vec::new();
    let mut unique: Vec<(String, String)> = Vec::new();
    for (n, p) in part_paths {
        if !seen.contains(&p) {
            seen.push(p.clone());
            unique.push((n, p));
        }
    }

    let out_dir = output_dir_of(params, &doc);
    let _ = std::fs::create_dir_all(&out_dir);
    let ss = app.system_service()?;
    let zoom_factor = params
        .get("zoom_factor")
        .and_then(|v| v.as_f64())
        .unwrap_or(1.0);

    // 全局方向（无 tooling 轴系时的回退，对齐 Python X/Y/Z/ISO）
    let global_dirs: Vec<(String, [f64; 3], [f64; 3])> = vec![
        ("X".into(), [-1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
        ("Y".into(), [0.0, -1.0, 0.0], [0.0, 0.0, 1.0]),
        ("Z".into(), [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]),
        ("ISO".into(), [-1.0, -1.0, -1.0], [0.0, 0.0, 1.0]),
    ];

    let mut files: Vec<String> = Vec::new();
    let mut failures = 0usize;
    let win = app
        .active_window()
        .map_err(|e| rscatiacli::ComError::runtime(format!("无活动窗口: {e}")))?;
    let viewer = win
        .active_viewer()
        .map_err(|e| rscatiacli::ComError::runtime(format!("无查看器: {e}")))?;
    let orig_layout = win.layout().ok();
    let _ = win.set_layout(1);

    for (idx, (ref_name, target_path)) in unique.iter().enumerate() {
        progress(&format!(
            "\n=== [{}/{}] {ref_name} ===",
            idx + 1,
            unique.len()
        ));
        if docs.open(target_path).is_err() {
            progress(&format!("[跳过] {ref_name}: 打开失败"));
            failures += 1;
            continue;
        }
        let part_doc = match app.active_document() {
            Ok(d) => d,
            Err(_) => {
                failures += 1;
                continue;
            }
        };
        hide_part_aux_elements(&part_doc);
        let part = part_doc.part_document().part();

        let directions = part
            .as_ref()
            .ok()
            .and_then(|p| tooling_directions(p, &ss, &progress))
            .unwrap_or_else(|| {
                progress("🧭 使用全局方向 X/Y/Z/ISO");
                global_dirs.clone()
            });

        for (dir_name, sight, up) in &directions {
            let out_path = out_dir.join(format!("{ref_name}_{dir_name}.png"));
            match capture_one(
                &win,
                &viewer,
                &out_path,
                Some(*sight),
                *up,
                true,
                zoom_factor,
                &progress,
            ) {
                Ok((_, file)) => files.push(file),
                Err(e) => {
                    progress(&format!("  [FAIL] {dir_name}: {e}"));
                    failures += 1;
                }
            }
        }
        let _ = part_doc.close();
    }

    if let Some(l) = orig_layout {
        let _ = win.set_layout(l);
    }

    if files.is_empty() {
        return Err(rscatiacli::ComError::runtime("没有生成任何截图"));
    }
    Ok(json!({
        "success": true,
        "file": files[0],
        "files": files,
        "failures": failures,
        "mode": "per-part",
    }))
}

/// 入口。params: {mode: iso|direct|per-part, output_dir?}。
pub fn run(params: &Value, progress: Progress<'_>) -> ComResult<Value> {
    let mode = params
        .get("mode")
        .and_then(|v| v.as_str())
        .unwrap_or("iso")
        .to_lowercase();
    match mode.as_str() {
        "iso" => iso_or_direct(params, progress, true),
        "direct" => iso_or_direct(params, progress, false),
        "per-part" => per_part(params, progress),
        other => Err(rscatiacli::ComError::runtime(format!(
            "未知模式: {other}(可选 iso/direct/per-part)"
        ))),
    }
}
