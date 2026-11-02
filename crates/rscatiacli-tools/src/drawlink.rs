//! drawlink —— 图纸链接管理（suites/drawlink 的 Rust 版）。
//!
//! 命令：info / list-sheets / list-views / links / detail / rename / update-link / export-report
//! （JSON 入参 `command` 字段分发）。
//!
//! 链接探测说明：Python 版 `hasattr(view, "generates_from")` 恒 False（pycatia
//! DrawingView 无此属性）→ 链接列表实际恒空；这里用 `GenerativeBehavior.Document`
//! 读取 3D 源文档，无链接时 COM 报错按无链接处理 —— 按意图实现。

use std::path::Path;

use serde_json::{json, Value};

use rscatiacli::objects::{Document, DrawingDocument, DrawingView};
use rscatiacli::{attach_with_guard, ComCollection, ComObject as _, ComResult, Progress};

fn active_drawing(app: &rscatiacli::objects::Application) -> ComResult<DrawingDocument> {
    let doc = app
        .active_document()
        .map_err(|_| rscatiacli::ComError::runtime("No active document."))?;
    // DrawingDocument 校验：取 Sheets 成功即认为是图纸文档
    let drawing = DrawingDocument::new(doc.com().clone());
    drawing
        .sheets()
        .map_err(|_| rscatiacli::ComError::runtime("Active document is not a Drawing."))?;
    Ok(drawing)
}

/// 视图信息（含 3D 链接探测，对齐 _get_view_info 意图）。
fn view_info(view: &DrawingView) -> Value {
    let name = view.name().unwrap_or_default();
    let scale = view.scale().unwrap_or(1.0);
    let mut info = json!({
        "name": name,
        "view_type": "(unknown)",
        "scale": scale,
        "has_linked_doc": false,
        "linked_doc_name": "",
    });
    if let Ok(behavior) = view.generative_behavior() {
        if let Ok(doc) = behavior.document() {
            info["has_linked_doc"] = json!(true);
            info["linked_doc_name"] = json!(doc
                .name()
                .unwrap_or_else(|_| "(unknown 3D document)".into()));
        }
    }
    info
}

fn find_view(
    drawing: &DrawingDocument,
    sheet_name: &str,
    view_name: &str,
) -> ComResult<Option<(String, DrawingView)>> {
    for sheet in drawing.sheets()?.items()? {
        if sheet.name().unwrap_or_default() != sheet_name {
            continue;
        }
        for view in sheet.views()?.items()? {
            if view.name().unwrap_or_default() == view_name {
                return Ok(Some((sheet_name.to_string(), view)));
            }
        }
    }
    Ok(None)
}

fn list_sheets(drawing: &DrawingDocument) -> ComResult<Vec<Value>> {
    let mut out = Vec::new();
    for (i, sheet) in drawing.sheets()?.items()?.into_iter().enumerate() {
        out.push(json!({
            "index": i + 1,
            "name": sheet.name().unwrap_or_default(),
            "view_count": sheet.views().and_then(|v| v.count()).unwrap_or(0),
        }));
    }
    Ok(out)
}

fn list_all_links(drawing: &DrawingDocument) -> ComResult<Vec<Value>> {
    let mut links = Vec::new();
    for sheet in drawing.sheets()?.items()? {
        let sheet_name = sheet.name().unwrap_or_default();
        for view in sheet.views()?.items()? {
            let info = view_info(&view);
            if info["has_linked_doc"].as_bool() == Some(true) {
                links.push(json!({
                    "sheet": sheet_name,
                    "view_name": info["name"],
                    "view_type": info["view_type"],
                    "linked_doc": info["linked_doc_name"],
                }));
            }
        }
    }
    Ok(links)
}

fn document_name(drawing: &DrawingDocument) -> String {
    Document::new(drawing.com().clone())
        .name()
        .unwrap_or_else(|_| "(unknown)".into())
}

/// 入口。params: {command, sheet?, view?, name?, output?}。
pub fn run(params: &Value, progress: Progress<'_>) -> ComResult<Value> {
    let command = params
        .get("command")
        .and_then(|v| v.as_str())
        .unwrap_or("info")
        .to_string();
    let app = attach_with_guard(progress)?;
    let drawing = active_drawing(&app)?;

    match command.as_str() {
        "info" => {
            let name = document_name(&drawing);
            let path = Document::new(drawing.com().clone())
                .full_name()
                .unwrap_or_else(|_| "(not saved)".into());
            let sheet_count = drawing.sheets().and_then(|s| s.count()).unwrap_or(0);
            progress(&format!("Name:  {name}"));
            progress(&format!("Path:  {path}"));
            progress(&format!("Sheets:     {sheet_count}"));
            Ok(json!({"success": true, "data": {"name": name, "path": path, "sheet_count": sheet_count}}))
        }
        "list-sheets" => {
            let sheets = list_sheets(&drawing)?;
            progress(&format!("{:<6} {:<40} {:<6}", "Index", "Name", "Views"));
            for s in &sheets {
                progress(&format!(
                    "{:<6} {:<40} {:<6}",
                    s["index"].as_i64().unwrap_or(0),
                    s["name"].as_str().unwrap_or(""),
                    s["view_count"].as_i64().unwrap_or(0)
                ));
            }
            Ok(json!({"success": true, "data": {"sheets": sheets}}))
        }
        "list-views" => {
            let sheet_name = params.get("sheet").and_then(|v| v.as_str()).unwrap_or("");
            let mut views = Vec::new();
            for sheet in drawing.sheets()?.items()? {
                let sn = sheet.name().unwrap_or_default();
                if !sheet_name.is_empty() && sn != sheet_name {
                    continue;
                }
                for view in sheet.views()?.items()? {
                    let mut info = view_info(&view);
                    info["sheet"] = json!(sn);
                    views.push(info);
                }
            }
            progress(&format!("Views in sheet '{}':", if sheet_name.is_empty() { "*" } else { sheet_name }));
            for v in &views {
                progress(&format!(
                    "  {:<30} scale={} linked={}",
                    v["name"].as_str().unwrap_or(""),
                    v["scale"].as_f64().unwrap_or(1.0),
                    v["has_linked_doc"].as_bool().unwrap_or(false)
                ));
            }
            Ok(json!({"success": true, "data": {"views": views}}))
        }
        "links" => {
            let links = list_all_links(&drawing)?;
            for l in &links {
                progress(&format!(
                    "  {} / {} -> {}",
                    l["sheet"].as_str().unwrap_or(""),
                    l["view_name"].as_str().unwrap_or(""),
                    l["linked_doc"].as_str().unwrap_or("")
                ));
            }
            if links.is_empty() {
                progress("  (no linked views)");
            }
            Ok(json!({"success": true, "data": {"links": links}}))
        }
        "detail" => {
            let sheet_name = params.get("sheet").and_then(|v| v.as_str()).unwrap_or("");
            let view_name = params.get("view").and_then(|v| v.as_str()).unwrap_or("");
            match find_view(&drawing, sheet_name, view_name)? {
                Some((_, view)) => {
                    let info = view_info(&view);
                    Ok(json!({"success": true, "data": info}))
                }
                None => Err(rscatiacli::ComError::runtime(format!(
                    "View '{view_name}' not found in sheet '{sheet_name}'."
                ))),
            }
        }
        "rename" => {
            let sheet_name = params.get("sheet").and_then(|v| v.as_str()).unwrap_or("");
            let view_name = params.get("view").and_then(|v| v.as_str()).unwrap_or("");
            let new_name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
            if new_name.is_empty() {
                return Err(rscatiacli::ComError::runtime("新视图名不能为空"));
            }
            match find_view(&drawing, sheet_name, view_name)? {
                Some((_, view)) => {
                    view.set_name(new_name)?;
                    progress(&format!("  View renamed: '{view_name}' -> '{new_name}'"));
                    Ok(json!({"success": true}))
                }
                None => Err(rscatiacli::ComError::runtime(format!(
                    "View '{view_name}' not found in sheet '{sheet_name}'."
                ))),
            }
        }
        "update-link" => {
            // 对齐 update_link_info：支持 name/scale/x/y/angle 字段更新
            let sheet_name = params.get("sheet").and_then(|v| v.as_str()).unwrap_or("");
            let view_name = params.get("view").and_then(|v| v.as_str()).unwrap_or("");
            let Some((_, view)) = find_view(&drawing, sheet_name, view_name)? else {
                return Ok(json!({"success": false, "message": format!("View '{view_name}' not found")}));
            };
            let mut updated = Vec::new();
            for key in ["scale", "x", "y", "angle"] {
                if let Some(v) = params.get(key).and_then(|v| v.as_f64()) {
                    match key {
                        "scale" => view.set_scale(v)?,
                        "x" => view.set_x(v)?,
                        "y" => view.set_y(v)?,
                        "angle" => view.set_angle(v)?,
                        _ => {}
                    }
                    updated.push(format!("{key}={v}"));
                    progress(&format!("  Set {key} = {v}"));
                }
            }
            if let Some(v) = params.get("name").and_then(|v| v.as_str()).filter(|s| !s.is_empty()) {
                view.set_name(v)?;
                updated.push(format!("name={v}"));
                progress(&format!("  Set name = {v}"));
            }
            Ok(json!({"success": true, "data": {"updated": updated}}))
        }
        "export-report" => {
            let output = params
                .get("output")
                .and_then(|v| v.as_str())
                .ok_or_else(|| rscatiacli::ComError::runtime("请提供 output 路径"))?;
            let links = list_all_links(&drawing)?;
            let sheets = list_sheets(&drawing)?;
            let name = document_name(&drawing);
            let mut text = String::new();
            text.push_str("Drawing Link Report\n");
            text.push_str(&format!("{}\n", "=".repeat(60)));
            text.push_str(&format!("Document: {name}\n"));
            text.push_str(&format!("Sheets: {}\n", sheets.len()));
            text.push_str(&format!("Total Links: {}\n", links.len()));
            text.push_str(&format!("{}\n\n", "=".repeat(60)));
            for sheet_info in &sheets {
                let sname = sheet_info["name"].as_str().unwrap_or("");
                text.push_str(&format!(
                    "\nSheet: {sname} ({} views)\n",
                    sheet_info["view_count"].as_i64().unwrap_or(0)
                ));
                text.push_str(&format!("{}\n", "-".repeat(40)));
                let sheet_links: Vec<&Value> = links
                    .iter()
                    .filter(|l| l["sheet"].as_str() == Some(sname))
                    .collect();
                if sheet_links.is_empty() {
                    text.push_str("  (no linked views)\n");
                } else {
                    for l in sheet_links {
                        text.push_str(&format!(
                            "  View: {:<30} -> {}\n",
                            l["view_name"].as_str().unwrap_or(""),
                            l["linked_doc"].as_str().unwrap_or("")
                        ));
                    }
                }
            }
            if let Some(parent) = Path::new(output).parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            std::fs::write(output, text)
                .map_err(|e| rscatiacli::ComError::runtime(format!("Failed to export report: {e}")))?;
            progress(&format!("Report saved to: {output}"));
            Ok(json!({"success": true, "data": {"path": output, "links": links.len()}}))
        }
        other => Err(rscatiacli::ComError::runtime(format!(
            "未知的 drawlink 命令: {other}(可选 info/list-sheets/list-views/links/detail/rename/update-link/export-report)"
        ))),
    }
}
