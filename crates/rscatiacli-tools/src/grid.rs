//! grid —— 图纸网格轴线管理（suites/grid 的 Rust 版）。
//!
//! 命令：list-sheets / list-views / create-line / create-system / edit / delete / delete-all
//! （JSON 入参 `command` 字段分发）。
//!
//! 与 Python 版的差异（有意为之）：
//! - Python 的 `sheet.factory_2d` 在 pycatia 不存在 → create_grid_line 实际
//!   从未画出几何（只打印并返回 True）；这里用 主视图（Views 第 1 项）的
//!   Factory2D 真实画线，并用 Texts.Add 打轴线标号；
//! - Python 的 `view.delete()` 在 pycatia 不存在 → 删除从未生效；这里走
//!   Selection（Clear/Add/Delete/Clear，与蜂窝筋删 Body 同一实测通道）。

use serde_json::{json, Value};

use rscatiacli::objects::DrawingDocument;
use rscatiacli::{attach_with_guard, ComCollection, ComObject as _, ComResult, Progress};

fn active_drawing(app: &rscatiacli::objects::Application) -> ComResult<DrawingDocument> {
    let doc = app
        .active_document()
        .map_err(|_| rscatiacli::ComError::runtime("No active document."))?;
    let drawing = DrawingDocument::new(doc.com().clone());
    drawing
        .sheets()
        .map_err(|_| rscatiacli::ComError::runtime("Active document is not a Drawing."))?;
    Ok(drawing)
}

fn find_sheet(
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

/// 网格标号启发式（对齐 _is_grid_label：1-4 位字母数字）。
fn is_grid_label(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 5
        && name.chars().all(|c| c.is_ascii_alphanumeric())
        && name.chars().count() <= 4
}

fn make_label(prefix: &str, num: i64) -> String {
    let bytes: Vec<char> = prefix.chars().collect();
    if bytes.len() == 1 && bytes[0].is_ascii_alphabetic() {
        let letter_num = (bytes[0].to_ascii_uppercase() as i64 - 'A' as i64) + num - 1;
        if (0..26).contains(&letter_num) {
            return format!("{}{num}", (b'A' + letter_num as u8) as char);
        }
        return format!("{}{num}", (b'A' + (letter_num % 26) as u8) as char);
    }
    format!("{prefix}{num}")
}

/// 在主视图上画一条线 + 标号（真实几何，修正 Python 的 no-op）。
#[allow(clippy::too_many_arguments)]
fn create_grid_line(
    drawing: &DrawingDocument,
    sheet_name: &str,
    axis_label: &str,
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    progress: &dyn Fn(&str),
) -> ComResult<bool> {
    let Some(sheet) = find_sheet(drawing, sheet_name)? else {
        progress(&format!("[ERROR] Sheet '{sheet_name}' not found."));
        return Ok(false);
    };
    // 主视图（Views 第 1 项）承载图纸级 2D 几何
    let main_view = sheet
        .views()?
        .item(1)
        .map_err(|e| rscatiacli::ComError::runtime(format!("主视图不可用: {e}")))?;
    let f2d = main_view
        .factory_2d()
        .map_err(|e| rscatiacli::ComError::runtime(format!("Factory2D 不可用: {e}")))?;
    f2d.create_line(x1, y1, x2, y2)?;
    let _ = main_view
        .texts()
        .and_then(|t| t.add(axis_label, (x1 + x2) / 2.0, (y1 + y2) / 2.0));
    progress(&format!(
        "  Grid line '{axis_label}' created: ({x1:.1},{y1:.1}) -> ({x2:.1},{y2:.1})"
    ));
    Ok(true)
}

/// 删除一个视图（Selection 通道，对齐蜂窝筋删 Body 的实测模式）。
fn delete_view(
    app: &rscatiacli::objects::Application,
    drawing: &DrawingDocument,
    view: &rscatiacli::objects::DrawingView,
) -> bool {
    let _ = drawing;
    let Ok(doc) = app.active_document() else {
        return false;
    };
    let Ok(sel) = doc.selection() else {
        return false;
    };
    let _ = sel.clear();
    if sel.add(view).is_err() {
        return false;
    }
    let deleted = sel.delete().is_ok();
    let _ = sel.clear();
    deleted
}

/// 入口。params: {command, sheet?, pattern?(过滤), label?, new_label?, x1..y2?,
/// horizontal_axes{count,spacing,label_prefix,start}, vertical_axes{...},
/// origin_x, origin_y}。
pub fn run(params: &Value, progress: Progress<'_>) -> ComResult<Value> {
    let command = params
        .get("command")
        .and_then(|v| v.as_str())
        .unwrap_or("list-sheets")
        .to_string();
    let app = attach_with_guard(progress)?;
    let drawing = active_drawing(&app)?;

    match command.as_str() {
        "list-sheets" => {
            let mut sheets = Vec::new();
            for (i, sheet) in drawing.sheets()?.items()?.into_iter().enumerate() {
                sheets.push(json!({
                    "index": i + 1,
                    "name": sheet.name().unwrap_or_default(),
                    "views": sheet.views().and_then(|v| v.count()).unwrap_or(0),
                }));
            }
            for s in &sheets {
                progress(&format!(
                    "  [{}] {} ({} views)",
                    s["index"], s["name"], s["views"]
                ));
            }
            Ok(json!({"success": true, "data": {"sheets": sheets}}))
        }
        "list-views" => {
            let sheet_filter = params.get("sheet").and_then(|v| v.as_str()).unwrap_or("");
            let mut views = Vec::new();
            for sheet in drawing.sheets()?.items()? {
                let sn = sheet.name().unwrap_or_default();
                if !sheet_filter.is_empty() && sn != sheet_filter {
                    continue;
                }
                for view in sheet.views()?.items()? {
                    let name = view.name().unwrap_or_default();
                    views.push(json!({
                        "sheet": sn,
                        "view_name": name,
                        "view_type": "(unknown)",
                        "is_grid": is_grid_label(&name),
                    }));
                }
            }
            for v in &views {
                progress(&format!(
                    "  {} / {} (grid={})",
                    v["sheet"].as_str().unwrap_or(""),
                    v["view_name"].as_str().unwrap_or(""),
                    v["is_grid"].as_bool().unwrap_or(false)
                ));
            }
            Ok(json!({"success": true, "data": {"views": views}}))
        }
        "create-line" => {
            let get_f = |k: &str| params.get(k).and_then(|v| v.as_f64()).unwrap_or(0.0);
            let sheet = params.get("sheet").and_then(|v| v.as_str()).unwrap_or("Sheet.1").to_string();
            let label = params.get("label").and_then(|v| v.as_str()).unwrap_or("1").to_string();
            let ok = create_grid_line(
                &drawing,
                &sheet,
                &label,
                get_f("x1"),
                get_f("y1"),
                get_f("x2"),
                get_f("y2"),
                &progress,
            )?;
            Ok(json!({"success": ok, "data": {"created": ok}}))
        }
        "create-system" => {
            let axes = |k: &str| -> (i64, f64, String, i64) {
                let a = params.get(k).cloned().unwrap_or(Value::Null);
                (
                    a.get("count").and_then(|v| v.as_i64()).unwrap_or(0),
                    a.get("spacing").and_then(|v| v.as_f64()).unwrap_or(50.0),
                    a.get("label_prefix")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    a.get("start").and_then(|v| v.as_i64()).unwrap_or(1),
                )
            };
            let (h_count, h_spacing, h_prefix, h_start) = axes("horizontal_axes");
            let (v_count, v_spacing, v_prefix, v_start) = axes("vertical_axes");
            let sheet = params
                .get("sheet")
                .and_then(|v| v.as_str())
                .unwrap_or("Sheet.1")
                .to_string();
            let ox = params.get("origin_x").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let oy = params.get("origin_y").and_then(|v| v.as_f64()).unwrap_or(0.0);
            progress(&format!("Creating grid on sheet '{sheet}':"));
            progress(&format!(
                "  Horizontal axes: {h_count} lines (spacing={h_spacing}mm)"
            ));
            progress(&format!(
                "  Vertical axes:   {v_count} lines (spacing={v_spacing}mm)"
            ));
            let width = (v_count - 1).max(0) as f64 * v_spacing;
            let height = (h_count - 1).max(0) as f64 * h_spacing;
            let mut created = 0i64;
            for idx in 0..h_count {
                let label = make_label(&h_prefix, h_start + idx);
                let y = oy + idx as f64 * h_spacing;
                if create_grid_line(&drawing, &sheet, &label, ox, y, ox + width, y, &progress)? {
                    created += 1;
                }
            }
            for idx in 0..v_count {
                let label = make_label(&v_prefix, v_start + idx);
                let x = ox + idx as f64 * v_spacing;
                if create_grid_line(&drawing, &sheet, &label, x, oy, x, oy + height, &progress)? {
                    created += 1;
                }
            }
            progress(&format!("Grid system created: {h_count}H x {v_count}V"));
            Ok(json!({"success": true, "data": {"created": created}}))
        }
        "edit" => {
            let sheet_name = params.get("sheet").and_then(|v| v.as_str()).unwrap_or("Sheet.1").to_string();
            let label = params.get("label").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let new_label = params.get("new_label").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let Some(sheet) = find_sheet(&drawing, &sheet_name)? else {
                return Ok(json!({"success": false, "message": format!("Sheet '{sheet_name}' not found.")}));
            };
            for view in sheet.views()?.items()? {
                if view.name().unwrap_or_default() == label {
                    if !new_label.is_empty() {
                        view.set_name(&new_label)?;
                        progress(&format!("  Grid line renamed: '{label}' -> '{new_label}'"));
                    }
                    if let Some(scale) = params.get("scale").and_then(|v| v.as_f64()) {
                        view.set_scale(scale)?;
                        progress(&format!("  Set scale = {scale}"));
                    }
                    return Ok(json!({"success": true}));
                }
            }
            Ok(json!({"success": false, "message": format!("Grid line '{label}' not found.")}))
        }
        "delete" => {
            let sheet_name = params.get("sheet").and_then(|v| v.as_str()).unwrap_or("Sheet.1").to_string();
            let label = params.get("label").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let Some(sheet) = find_sheet(&drawing, &sheet_name)? else {
                return Ok(json!({"success": false, "message": format!("Sheet '{sheet_name}' not found.")}));
            };
            for view in sheet.views()?.items()? {
                if view.name().unwrap_or_default() == label {
                    if delete_view(&app, &drawing, &view) {
                        progress(&format!("  Grid line '{label}' deleted."));
                        return Ok(json!({"success": true}));
                    }
                    return Ok(json!({"success": false, "message": "删除失败(Selection 通道)"}));
                }
            }
            Ok(json!({"success": false, "message": format!("Grid line '{label}' not found.")}))
        }
        "delete-all" => {
            let sheet_filter = params.get("sheet").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let mut count = 0i64;
            for sheet in drawing.sheets()?.items()? {
                let sn = sheet.name().unwrap_or_default();
                if !sheet_filter.is_empty() && sn != sheet_filter {
                    continue;
                }
                let mut to_delete = Vec::new();
                for view in sheet.views()?.items()? {
                    if is_grid_label(&view.name().unwrap_or_default()) {
                        to_delete.push(view);
                    }
                }
                for view in to_delete {
                    if delete_view(&app, &drawing, &view) {
                        count += 1;
                    }
                }
            }
            progress(&format!("Deleted {count} grid line(s)."));
            Ok(json!({"success": true, "data": {"deleted": count}}))
        }
        other => Err(rscatiacli::ComError::runtime(format!(
            "未知的 grid 命令: {other}(可选 list-sheets/list-views/create-line/create-system/edit/delete/delete-all)"
        ))),
    }
}
