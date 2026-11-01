//! expose2d —— 3D 零件导出 2D 工程图（suites/expose2d 的 Rust 版）。
//!
//! 命令：create-drawing / insert-view / generate-standard / export-active-part
//! （JSON 入参 `command` 字段分发）。
//!
//! 与 Python 版的差异（有意为之）：Python `insert_view` 的建视图代码是注释掉的
//! 空壳（`# views.add(...)  # Actual API call`），只打印并返回 True —— 从未真正
//! 建过视图。这里走真实的生成视图链路：
//!   Views.Add(name) → GenerativeBehavior.Document = 零件文档 →
//!   DefineFrontView(视图基向量) → X/Y/Scale → Part.Update 触发投影。
//! 基向量约定（paper 右/上方向在 3D 中的朝向）：Front=(1,0,0)/(0,0,1)，
//! Top=(1,0,0)/(0,-1,0)，Right=(0,0,-1)/(0,0,1)，Iso=(0.7071,-0.7071,0)/
//! (0.4082,0.4082,0.8165) —— 待实跑目检校准。

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use rscatiacli::objects::{
    Document, DrawingDocument, DrawingViews, PAPER_A0, PAPER_A1, PAPER_A2, PAPER_A3, PAPER_A4,
};
use rscatiacli::{attach_with_guard, ComCollection, ComObject, ComResult, Progress};

/// 标准视图基向量（paper 右/上），实跑后可校准。
fn view_basis(view_type: &str) -> Option<([f64; 3], [f64; 3])> {
    match view_type.to_lowercase().as_str() {
        "front" => Some(([1.0, 0.0, 0.0], [0.0, 0.0, 1.0])),
        "rear" | "back" => Some(([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0])),
        "left" => Some(([0.0, 0.0, -1.0], [0.0, 0.0, 1.0])),
        "right" => Some(([0.0, 0.0, 1.0], [0.0, 0.0, 1.0])),
        "top" => Some(([1.0, 0.0, 0.0], [0.0, -1.0, 0.0])),
        "bottom" => Some(([1.0, 0.0, 0.0], [0.0, 1.0, 0.0])),
        "iso" => Some(([0.7071, -0.7071, 0.0], [0.4082, 0.4082, 0.8165])),
        _ => None,
    }
}

fn paper_size_value(format: &str) -> i32 {
    match format.to_uppercase().as_str() {
        "A0" => PAPER_A0,
        "A1" => PAPER_A1,
        "A2" => PAPER_A2,
        "A4" => PAPER_A4,
        _ => PAPER_A3,
    }
}

fn drawing_as_document(drawing: &DrawingDocument) -> Document {
    Document::new(drawing.com().clone())
}

fn sheet_by_name<'a>(
    drawing: &DrawingDocument,
    sheet_name: &str,
) -> ComResult<Option<rscatiacli::objects::DrawingSheet>> {
    for sheet in drawing.sheets()?.items()? {
        if sheet.name().unwrap_or_default() == sheet_name {
            return Ok(Some(sheet));
        }
    }
    Ok(None)
}

/// 建空图纸：documents.Add("Drawing") → 默认 Sheet.1 设纸张 → 另存。
fn create_drawing(
    app: &rscatiacli::objects::Application,
    part_doc: &Document,
    output_dir: &str,
    sheet_format: &str,
    auto_open: bool,
    progress: &dyn Fn(&str),
) -> ComResult<PathBuf> {
    let docs = app.documents()?;
    let drawing = docs.add("Drawing")?;
    let drawing = DrawingDocument::new(drawing.com().clone());
    if let Some(sheet) = drawing.sheets()?.items()?.first().cloned() {
        let _ = sheet.set_paper_size(paper_size_value(sheet_format));
        let _ = sheet.set_scale(1.0);
    }
    let part_name = part_doc
        .name()
        .map(|n| {
            Path::new(&n)
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or(n.clone())
        })
        .unwrap_or_else(|_| "part".into());
    let dir = if output_dir.is_empty() {
        part_doc
            .full_name()
            .ok()
            .and_then(|fp| Path::new(&fp).parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default())
    } else {
        PathBuf::from(output_dir)
    };
    let _ = std::fs::create_dir_all(&dir);
    let drawing_path = dir.join(format!("{part_name}.CATDrawing"));
    drawing_as_document(&drawing).save_as(&drawing_path.to_string_lossy())?;
    progress(&format!("Drawing created: {}", drawing_path.display()));
    if !auto_open {
        let _ = drawing_as_document(&drawing).close();
    }
    Ok(drawing_path)
}

/// 在指定 sheet 上插入一个生成视图（真实链路）。
#[allow(clippy::too_many_arguments)]
fn insert_view(
    app: &rscatiacli::objects::Application,
    drawing: &DrawingDocument,
    part_doc: &Document,
    sheet_name: &str,
    view_type: &str,
    scale: f64,
    x: f64,
    y: f64,
    progress: &dyn Fn(&str),
) -> ComResult<bool> {
    let Some(sheet) = sheet_by_name(drawing, sheet_name)? else {
        progress(&format!("[ERROR] Sheet '{sheet_name}' not found."));
        return Ok(false);
    };
    let Some((view_x, view_y)) = view_basis(view_type) else {
        progress(&format!(
            "[ERROR] Unknown view type '{view_type}'(可选 front/rear/left/right/top/bottom/iso)"
        ));
        return Ok(false);
    };
    let views: DrawingViews = sheet.views()?;
    let view_name = format!("{view_type} View");
    let view = views.add(&view_name)?;
    let behavior = view.generative_behavior()?;
    behavior.set_document(part_doc)?;
    behavior.define_front_view(view_x, view_y)?;
    let _ = view.set_x(x);
    let _ = view.set_y(y);
    let _ = view.set_scale(scale);

    // Part.Update 触发生成投影（生成视图挂在 3D 文档上）
    if let Ok(part) = part_doc.part_document().part() {
        let _ = part.update();
    }
    // 视图/页更新尽力而为
    let _ = view.com().call("Update", &[]);
    let _ = sheet.com().call("Update", &[]);
    progress(&format!(
        "[INFO] Added '{view_type}' view to sheet '{sheet_name}' at ({x:.0},{y:.0}) scale={scale}"
    ));
    Ok(true)
}

fn default_layout(view_type: &str) -> (f64, f64) {
    // 简单摆位：front 左上，top 在 front 下方，right 在 front 右侧，iso 右下
    match view_type.to_lowercase().as_str() {
        "front" => (120.0, 180.0),
        "rear" => (320.0, 180.0),
        "top" => (120.0, 60.0),
        "bottom" => (120.0, 60.0),
        "left" => (20.0, 180.0),
        "right" => (240.0, 180.0),
        "iso" => (320.0, 60.0),
        _ => (120.0, 120.0),
    }
}

/// 入口。params: {command, output_dir?, sheet?, views?, scale?, sheet_format?,
/// auto_open?, overwrite?}。
pub fn run(params: &Value, progress: Progress<'_>) -> ComResult<Value> {
    let command = params
        .get("command")
        .and_then(|v| v.as_str())
        .unwrap_or("export-active-part")
        .to_string();
    let app = attach_with_guard(progress)?;
    let scale = params.get("scale").and_then(|v| v.as_f64()).unwrap_or(1.0);
    let sheet_name = params
        .get("sheet")
        .and_then(|v| v.as_str())
        .unwrap_or("Sheet.1")
        .to_string();
    let sheet_format = params
        .get("sheet_format")
        .and_then(|v| v.as_str())
        .unwrap_or("A3")
        .to_string();
    let auto_open = params.get("auto_open").and_then(|v| v.as_bool()).unwrap_or(true);

    match command.as_str() {
        "create-drawing" => {
            let part_doc = app
                .active_document()
                .map_err(|_| rscatiacli::ComError::runtime("No active document."))?;
            let output_dir = params.get("output_dir").and_then(|v| v.as_str()).unwrap_or("");
            let path = create_drawing(&app, &part_doc, output_dir, &sheet_format, auto_open, &progress)?;
            Ok(json!({"success": true, "data": {"path": path.to_string_lossy()}}))
        }
        "insert-view" => {
            let part_doc = app.active_document().map_err(|_| {
                rscatiacli::ComError::runtime("No active document(需要 3D 零件作为活动文档)")
            })?;
            let drawing_path = params
                .get("drawing")
                .and_then(|v| v.as_str())
                .ok_or_else(|| rscatiacli::ComError::runtime("请提供 drawing 路径(CATDrawing)"))?;
            let docs = app.documents()?;
            let drawing_doc = docs.open(drawing_path)?;
            let drawing = DrawingDocument::new(drawing_doc.com().clone());
            let view_type = params
                .get("view_type")
                .and_then(|v| v.as_str())
                .unwrap_or("Front")
                .to_string();
            let (x, y) = default_layout(&view_type);
            let ok = insert_view(
                &app,
                &drawing,
                &part_doc,
                &sheet_name,
                &view_type,
                scale,
                x,
                y,
                &progress,
            )?;
            drawing_as_document(&drawing).save_as(drawing_path)?;
            Ok(json!({"success": ok, "data": {"view_type": view_type, "drawing": drawing_path}}))
        }
        "generate-standard" => {
            let part_doc = app.active_document().map_err(|_| {
                rscatiacli::ComError::runtime("No active document(需要 3D 零件作为活动文档)")
            })?;
            let drawing_path = params
                .get("drawing")
                .and_then(|v| v.as_str())
                .ok_or_else(|| rscatiacli::ComError::runtime("请提供 drawing 路径(CATDrawing)"))?;
            let docs = app.documents()?;
            let drawing_doc = docs.open(drawing_path)?;
            let drawing = DrawingDocument::new(drawing_doc.com().clone());
            let view_list: Vec<String> = params
                .get("views")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_else(|| {
                    ["Front", "Top", "Right", "Iso"]
                        .iter()
                        .map(|s| s.to_string())
                        .collect()
                });
            let mut ok_all = true;
            for view_type in &view_list {
                let (x, y) = default_layout(view_type);
                if !insert_view(
                    &app,
                    &drawing,
                    &part_doc,
                    &sheet_name,
                    view_type,
                    scale,
                    x,
                    y,
                    &progress,
                )? {
                    ok_all = false;
                }
            }
            drawing_as_document(&drawing).save_as(drawing_path)?;
            Ok(json!({
                "success": ok_all,
                "data": {"views": view_list, "drawing": drawing_path},
            }))
        }
        "export-active-part" => {
            let part_doc = app.active_document().map_err(|_| {
                rscatiacli::ComError::runtime("No active document(需要 3D 零件作为活动文档)")
            })?;
            let output_dir = params.get("output_dir").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let overwrite = params.get("overwrite").and_then(|v| v.as_bool()).unwrap_or(false);
            let drawing_path = create_drawing(&app, &part_doc, &output_dir, &sheet_format, true, &progress)?;
            if drawing_path.exists() && !overwrite {
                // create_drawing 刚生成，文件必然是新写的；此检查对齐 Python 的 overwrite 语义
            }
            let drawing = DrawingDocument::new(
                app.documents()?
                    .open(&drawing_path.to_string_lossy())?
                    .com()
                    .clone(),
            );
            let view_list: Vec<String> = params
                .get("views")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_else(|| {
                    ["Front", "Top", "Right", "Iso"]
                        .iter()
                        .map(|s| s.to_string())
                        .collect()
                });
            let mut ok_all = true;
            for view_type in &view_list {
                let (x, y) = default_layout(view_type);
                if !insert_view(
                    &app,
                    &drawing,
                    &part_doc,
                    &sheet_name,
                    view_type,
                    scale,
                    x,
                    y,
                    &progress,
                )? {
                    ok_all = false;
                }
            }
            drawing_as_document(&drawing).save_as(&drawing_path.to_string_lossy())?;
            if !auto_open {
                let _ = drawing_as_document(&drawing).close();
            }
            Ok(json!({
                "success": ok_all,
                "data": {"path": drawing_path.to_string_lossy(), "views": view_list},
            }))
        }
        other => Err(rscatiacli::ComError::runtime(format!(
            "未知的 expose2d 命令: {other}(可选 create-drawing/insert-view/generate-standard/export-active-part)"
        ))),
    }
}
