//! IPP —— 外部参考隔离/批处理工具（suites/ipp 的 Rust 版）。
//!
//! 命令：info / isolate / batch / data / config / config-init
//! （JSON 入参 `command` 字段分发）。
//!
//! 与 Python 版的差异（有意为之）：
//! - Python `_find_external_references` 的 `shape.parent.ref` 探测恒为 False、
//!   `part.isolate(dict)` 传 dict 必然 TypeError 被吞 —— isolated_count 实际恒 0；
//!   这里按意图实现：收集名字含 "External" 的 shape → `Part.Isolate(引用)`，
//!   成功计数（对象不支持时按 Python 口径吞错保持 0）；
//! - 数据文件 xlsx 用 calamine（替代 openpyxl/pandas 回退），txt 分隔符探测同口径。

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use calamine::{DataType as _, Reader as _};
use rscatiacli::objects::Document;
use rscatiacli::{attach_with_guard, ComCollection, ComResult, Progress};

// ── 配置（对齐 config.py 默认值）──

struct IppConfig {
    isolate_external_refs: bool,
    external_ref_search_patterns: Vec<String>,
    output_dir: String,
    output_suffix: String,
    batch_folder: String,
}

impl Default for IppConfig {
    fn default() -> Self {
        Self {
            isolate_external_refs: true,
            external_ref_search_patterns: vec!["*.CATPart".into(), "*.CATProduct".into()],
            output_dir: String::new(),
            output_suffix: "_IPP".into(),
            batch_folder: String::new(),
        }
    }
}

impl IppConfig {
    fn from_params(params: &Value) -> Self {
        let mut cfg = Self::default();
        if let Some(path) = params.get("config_path").and_then(|v| v.as_str()) {
            if let Ok(raw) = std::fs::read_to_string(path) {
                if let Ok(data) = serde_json::from_str::<Value>(&raw) {
                    if let Some(b) = data.get("isolate_external_refs").and_then(|v| v.as_bool()) {
                        cfg.isolate_external_refs = b;
                    }
                    if let Some(p) = data.get("external_ref_search_patterns").and_then(|v| v.as_array()) {
                        cfg.external_ref_search_patterns = p
                            .iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect();
                    }
                    for (k, dst) in [
                        ("output_dir", &mut cfg.output_dir),
                        ("batch_folder", &mut cfg.batch_folder),
                    ] {
                        if let Some(v) = data.get(k).and_then(|v| v.as_str()) {
                            *dst = v.to_string();
                        }
                    }
                    if let Some(v) = data.get("output_suffix").and_then(|v| v.as_str()) {
                        cfg.output_suffix = v.to_string();
                    }
                }
            }
        }
        if let Some(b) = params.get("no_isolate").and_then(|v| v.as_bool()) {
            cfg.isolate_external_refs = !b;
        }
        cfg
    }
}

// ── 数据文件（对齐 load_data_file：xlsx 首表 / txt 分隔符探测）──

fn load_data_file(file_path: &Path) -> Result<Vec<std::collections::BTreeMap<String, String>>, String> {
    let ext = file_path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "xlsx" | "xls" => {
            let mut book =
                calamine::open_workbook_auto(file_path).map_err(|e| e.to_string())?;
            let first_sheet = book
                .sheet_names()
                .first()
                .cloned()
                .ok_or("数据文件没有工作表")?;
            let range = book.worksheet_range(&first_sheet).map_err(|e| e.to_string())?;
            let mut rows = range.rows();
            let header: Vec<String> = rows
                .next()
                .map(|h| {
                    h.iter()
                        .enumerate()
                        .map(|(i, c)| {
                            if c.is_empty() {
                                format!("Col{i}")
                            } else {
                                c.to_string()
                            }
                        })
                        .collect()
                })
                .unwrap_or_default();
            let mut out = Vec::new();
            for row in rows {
                let mut rec = std::collections::BTreeMap::new();
                for (i, val) in row.iter().enumerate() {
                    if let Some(h) = header.get(i) {
                        rec.insert(h.clone(), val.to_string());
                    }
                }
                out.push(rec);
            }
            Ok(out)
        }
        "txt" => {
            let raw = std::fs::read_to_string(file_path).map_err(|e| e.to_string())?;
            // utf-8-sig BOM
            let raw = raw.strip_prefix('\u{feff}').unwrap_or(&raw);
            let lines: Vec<&str> = raw.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
            if lines.is_empty() {
                return Ok(Vec::new());
            }
            let delimiter = if lines[0].contains('\t') { '\t' } else { ',' };
            let headers: Vec<String> = lines[0].split(delimiter).map(|h| h.trim().to_string()).collect();
            let mut out = Vec::new();
            for line in &lines[1..] {
                let parts: Vec<&str> = line.split(delimiter).collect();
                let mut rec = std::collections::BTreeMap::new();
                for (i, h) in headers.iter().enumerate() {
                    rec.insert(h.clone(), parts.get(i).map(|s| s.trim().to_string()).unwrap_or_default());
                }
                out.push(rec);
            }
            Ok(out)
        }
        other => Err(format!("Unsupported file type: .{other}")),
    }
}

// ── 文档操作 ──

fn part_number_of(doc: &Document) -> String {
    if let Ok(part) = doc.part_document().part() {
        if let Ok(pn) = part.part_number() {
            return pn;
        }
    }
    if let Ok(product) = doc.product_document().product() {
        if let Ok(pn) = product.part_number() {
            return pn;
        }
    }
    doc.name().unwrap_or_default()
}

fn set_part_number(doc: &Document, pn: &str) -> bool {
    if let Ok(part) = doc.part_document().part() {
        if part.set_part_number(pn).is_ok() {
            let _ = part.update();
            return true;
        }
    }
    if let Ok(product) = doc.product_document().product() {
        if product.set_part_number(pn).is_ok() {
            return true;
        }
    }
    false
}

/// 外部参考探测（名字含 "External" 的 shape；对齐 _find_external_references 意图）。
fn find_external_references(doc: &Document) -> Vec<Value> {
    let mut refs = Vec::new();
    if let Ok(part) = doc.part_document().part() {
        // Part 顶层 shape 挂在主 Body 上（Python 的 part.shapes 同样不成立）
        if let Ok(shapes) = part.main_body().and_then(|b| b.shapes()) {
            for shape in shapes.items().unwrap_or_default() {
                if let Ok(name) = shape.name() {
                    if name.contains("External") {
                        refs.push(json!({"name": name}));
                    }
                }
            }
        }
    }
    refs
}

/// 隔离外部参考（对齐 isolate_external_refs，隔离走 Part.Isolate(引用)）。
fn isolate_external_refs(doc: &Document, progress: &dyn Fn(&str)) -> Value {
    let mut result = json!({
        "document": doc.name().unwrap_or_else(|_| "Unknown".into()),
        "isolated_count": 0,
        "external_refs": [],
    });
    let part = match doc.part_document().part() {
        Ok(p) => p,
        Err(_) => {
            result["error"] = json!("Not a Part document");
            return result;
        }
    };
    let refs = find_external_references(doc);
    result["external_refs"] = json!(refs);
    if refs.is_empty() {
        progress("[OK] No external references found or isolate not supported");
        return result;
    }
    let mut isolated = 0;
    for r in &refs {
        let name = r.get("name").and_then(|v| v.as_str()).unwrap_or("");
        // 按 shape 名取引用再隔离；对象不支持 Isolate 时按 Python 口径吞错
        if let Ok(shapes) = part.main_body().and_then(|b| b.shapes()) {
            for shape in shapes.items().unwrap_or_default() {
                if shape.name().unwrap_or_default() == name {
                    if let Ok(reference) = part.create_reference_from_object(&shape) {
                        if part.isolate(&reference).is_ok() {
                            isolated += 1;
                        }
                    }
                    break;
                }
            }
        }
    }
    let _ = part.update();
    result["isolated_count"] = json!(isolated);
    progress(&format!("[OK] Isolated {isolated} external reference(s)"));
    result
}

fn glob_match_dir(folder: &Path, patterns: &[String]) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(folder) {
        for entry in entries.flatten() {
            let p = entry.path();
            if !p.is_file() {
                continue;
            }
            let name = p
                .file_name()
                .map(|s| s.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            for pat in patterns {
                // 仅支持 Python 用到的 "*.ext" 形式（后缀匹配）
                if let Some(suffix) = pat.strip_prefix("*.") {
                    if name.ends_with(&suffix.to_lowercase()) {
                        files.push(p.clone());
                        break;
                    }
                }
            }
        }
    }
    files.sort();
    files
}

fn print_results(results: &[Value], progress: &dyn Fn(&str)) {
    if results.is_empty() {
        progress("[INFO] No results.");
        return;
    }
    let ok = results
        .iter()
        .filter(|r| r.get("status").and_then(|v| v.as_str()) == Some("ok"))
        .count();
    let errors = results.len() - ok;
    progress("");
    progress("==================================================");
    progress(&format!(
        "Results: {ok} succeeded, {errors} failed (total: {})",
        results.len()
    ));
    progress("==================================================");
    for r in results {
        let is_ok = r.get("status").and_then(|v| v.as_str()) == Some("ok");
        let tag = r
            .get("file")
            .or_else(|| r.get("part"))
            .or_else(|| r.get("document"))
            .and_then(|v| v.as_str())
            .unwrap_or("?");
        progress(&format!(
            "  {} {tag}",
            if is_ok { "[OK]" } else { "[!!]" }
        ));
        if let Some(pn) = r.get("part_number").and_then(|v| v.as_str()) {
            progress(&format!("         Part Number: {pn}"));
        }
        if let Some(pn) = r.get("new_part_number").and_then(|v| v.as_str()) {
            progress(&format!("         New Part No: {pn}"));
        }
        if let Some(n) = r.get("isolated").and_then(|v| v.as_i64()) {
            progress(&format!("         Isolated:    {n} ref(s)"));
        }
        if let Some(e) = r.get("error").and_then(|v| v.as_str()) {
            progress(&format!("         Error:       {e}"));
        }
        if let Some(o) = r.get("output").and_then(|v| v.as_str()) {
            progress(&format!("         Output:      {o}"));
        }
    }
}

/// 入口。params: {command, config_path?, folder?, data_file?, output?, no_isolate?, output_suffix?}。
pub fn run(params: &Value, progress: Progress<'_>) -> ComResult<Value> {
    let command = params
        .get("command")
        .and_then(|v| v.as_str())
        .unwrap_or("info")
        .to_string();
    let cfg = IppConfig::from_params(params);

    match command.as_str() {
        "config" => {
            return Ok(json!({
                "success": true,
                "data": {
                    "isolate_external_refs": cfg.isolate_external_refs,
                    "external_ref_search_patterns": cfg.external_ref_search_patterns,
                    "output_dir": cfg.output_dir,
                    "output_suffix": cfg.output_suffix,
                    "batch_folder": cfg.batch_folder,
                },
            }));
        }
        "config-init" => {
            let path = params
                .get("output")
                .and_then(|v| v.as_str())
                .unwrap_or("ipp_config.json");
            let data = json!({
                "isolate_external_refs": cfg.isolate_external_refs,
                "external_ref_search_patterns": cfg.external_ref_search_patterns,
                "output_dir": cfg.output_dir,
                "output_suffix": cfg.output_suffix,
                "batch_folder": cfg.batch_folder,
            });
            if let Some(parent) = Path::new(path).parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            std::fs::write(path, serde_json::to_string_pretty(&data).unwrap_or_default())
                .map_err(|e| rscatiacli::ComError::runtime(e.to_string()))?;
            progress(&format!("[OK] Config saved to: {path}"));
            return Ok(json!({"success": true, "data": {"path": path}}));
        }
        "data" => {
            let data_file = params
                .get("data_file")
                .and_then(|v| v.as_str())
                .ok_or_else(|| rscatiacli::ComError::runtime("请提供 data_file 路径"))?;
            let records = load_data_file(Path::new(data_file))
                .map_err(|e| rscatiacli::ComError::runtime(e))?;
            if records.is_empty() {
                progress("[WARN] No records found in data file");
                return Ok(json!({"success": true, "data": {"results": []}}));
            }
            progress(&format!(
                "[INFO] Loaded {} record(s) from: {data_file}",
                records.len()
            ));

            let app = attach_with_guard(progress)?;
            let docs = app.documents()?;
            let mut results = Vec::new();
            for (i, record) in records.iter().enumerate() {
                let part_id = record
                    .get("FileName")
                    .or_else(|| record.get("PartNumber"))
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| (i + 1).to_string());
                let new_pn = record.get("NewPartNumber").cloned().unwrap_or_default();
                progress(&format!("\n[{}/{}] Processing: {part_id}", i + 1, records.len()));

                let mut file_path = PathBuf::from(&part_id);
                if !file_path.is_file() && !cfg.batch_folder.is_empty() {
                    file_path = Path::new(&cfg.batch_folder).join(&part_id);
                }

                let doc = if file_path.is_file() {
                    match docs.open(&file_path.to_string_lossy()) {
                        Ok(d) => {
                            progress(&format!("[OK] Opened: {}", file_path.display()));
                            d
                        }
                        Err(e) => {
                            progress(&format!("[ERROR] Failed to open {}: {e}", file_path.display()));
                            results.push(json!({"part": part_id, "status": "error", "error": "Failed to open"}));
                            continue;
                        }
                    }
                } else {
                    match app.active_document() {
                        Ok(d) => d,
                        Err(_) => {
                            results.push(json!({"part": part_id, "status": "error", "error": "No active document"}));
                            continue;
                        }
                    }
                };

                let mut entry = json!({"part": part_id, "status": "ok"});
                let current_pn = part_number_of(&doc);
                entry["current_part_number"] = json!(current_pn);
                if !new_pn.is_empty() && new_pn != "null" {
                    if set_part_number(&doc, &new_pn) {
                        entry["new_part_number"] = json!(new_pn);
                        progress(&format!("[OK] Part number changed: {current_pn} -> {new_pn}"));
                    } else {
                        entry["new_part_number"] = json!("FAILED");
                    }
                }
                if cfg.isolate_external_refs {
                    let iso = isolate_external_refs(&doc, &progress);
                    entry["isolated"] = iso["isolated_count"].clone();
                }
                results.push(entry);
            }
            print_results(&results, &progress);
            return Ok(json!({"success": true, "data": {"results": results}}));
        }
        _ => {}
    }

    let app = attach_with_guard(progress)?;
    let doc = app
        .active_document()
        .map_err(|_| rscatiacli::ComError::runtime("No active document"))?;

    match command.as_str() {
        "info" => {
            let name = doc.name().unwrap_or_else(|_| "Unknown".into());
            let path = doc.full_name().unwrap_or_else(|_| "Unknown".into());
            let dtype = if doc.part_document().part().is_ok() {
                "DocumentType.part_document"
            } else if doc.product_document().product().is_ok() {
                "DocumentType.product_document"
            } else {
                "Unknown"
            };
            progress(&format!("Name:  {name}"));
            progress(&format!("Path:  {path}"));
            progress(&format!("Type:  {dtype}"));
            Ok(json!({"success": true, "data": {"name": name, "path": path, "type": dtype}}))
        }
        "isolate" => {
            let result = isolate_external_refs(&doc, &progress);
            let count = result["isolated_count"].as_i64().unwrap_or(0);
            progress(&format!("External Refs:    {}", result["external_refs"].as_array().map(|a| a.len()).unwrap_or(0)));
            progress(&format!("Isolated:         {count}"));
            for r in result["external_refs"].as_array().unwrap_or(&Vec::new()) {
                progress(&format!(
                    "  - {}",
                    r.get("name").and_then(|v| v.as_str()).unwrap_or("?")
                ));
            }
            Ok(json!({"success": true, "data": result}))
        }
        "batch" => {
            let folder = params
                .get("folder")
                .and_then(|v| v.as_str())
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(&cfg.batch_folder));
            if !folder.is_dir() {
                return Err(rscatiacli::ComError::runtime(format!(
                    "Folder not found: {}",
                    folder.display()
                )));
            }
            let files = glob_match_dir(&folder, &cfg.external_ref_search_patterns);
            if files.is_empty() {
                progress(&format!(
                    "[WARN] No matching files found in: {}",
                    folder.display()
                ));
                return Ok(json!({"success": true, "data": {"results": []}}));
            }
            progress(&format!(
                "[INFO] Found {} file(s) to process in batch mode",
                files.len()
            ));

            let docs = app.documents()?;
            let mut results = Vec::new();
            for (i, fp) in files.iter().enumerate() {
                progress(&format!(
                    "\n[{}/{}] Processing: {}",
                    i + 1,
                    files.len(),
                    fp.display()
                ));
                let doc = match docs.open(&fp.to_string_lossy()) {
                    Ok(d) => {
                        progress(&format!("[OK] Opened: {}", fp.display()));
                        d
                    }
                    Err(e) => {
                        progress(&format!("[ERROR] Failed to open {}: {e}", fp.display()));
                        results.push(json!({"file": fp.to_string_lossy(), "status": "error", "error": "Failed to open"}));
                        continue;
                    }
                };
                let mut entry = json!({"file": fp.to_string_lossy(), "status": "ok"});
                entry["part_number"] = json!(part_number_of(&doc));
                if cfg.isolate_external_refs {
                    let iso = isolate_external_refs(&doc, &progress);
                    entry["isolated"] = iso["isolated_count"].clone();
                }
                if !cfg.output_suffix.is_empty() {
                    let parent = if cfg.output_dir.is_empty() {
                        fp.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| PathBuf::from("."))
                    } else {
                        PathBuf::from(&cfg.output_dir)
                    };
                    let out = parent.join(format!(
                        "{}{}{}",
                        fp.file_stem().map(|s| s.to_string_lossy()).unwrap_or_default(),
                        cfg.output_suffix,
                        fp.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default()
                    ));
                    if let Some(dir) = out.parent() {
                        let _ = std::fs::create_dir_all(dir);
                    }
                    match doc.save_as(&out.to_string_lossy()) {
                        Ok(()) => {
                            progress(&format!("[OK] Saved as: {}", out.display()));
                            entry["output"] = json!(out.to_string_lossy());
                        }
                        Err(e) => progress(&format!("[ERROR] Failed to save: {e}")),
                    }
                }
                results.push(entry);
                let _ = doc.close();
            }
            print_results(&results, &progress);
            Ok(json!({"success": true, "data": {"results": results}}))
        }
        other => Err(rscatiacli::ComError::runtime(format!(
            "未知的 ipp 命令: {other}(可选 info/isolate/batch/data/config/config-init)"
        ))),
    }
}
