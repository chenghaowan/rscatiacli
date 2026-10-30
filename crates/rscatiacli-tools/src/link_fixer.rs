//! link-fixer：装配断链检查与修复。
//!
//! 对应 ltl-manage `resources/sidecars/link_fixer/`
//! （catia_link_fixer_bridge.py + product_link_handler.py），JSON 字段与 Python
//! 版逐一对齐，前端无需改动。
//!
//! 三个动作：
//!   status  连接 / 文档概览（只读，不加多实例守卫，口径同 Python 版）
//!   scan    遍历已打开的 CATProduct，报告断链；带搜索路径时写入 FileSearchOrder
//!   fix     断链重连：优先重连到已打开文档（`Products.ReplaceProduct`），其次把
//!           文件所在目录加进 FileSearchOrder；仍有失败则保存并重开全部文档，
//!           触发 CATIA 按搜索顺序自动重链
//!
//! 修复策略（按 Python 版约定）：
//!   1. 断链组件若能在已打开文档中按文件名匹配到 → ReplaceProduct 直接重连
//!      （未保存的内存文档也能连）；重连失败且文档已保存 → 该文档所在目录进
//!      FileSearchOrder；
//!   2. 否则在磁盘搜索路径中找同名文件 → 其所在目录进 FileSearchOrder；
//!   3. 每个文档修完 `doc.Update()` 触发 CATIA 重链。
//!
//! 与 Python 版的差异（收敛性改进）：
//! - 连接只用 ROT 且加多实例守卫（Python 的 Dispatch 会拉起后台空实例）；
//! - 支持 `dry_run`：只报告将要重连的组件，不写 FileSearchOrder、不 ReplaceProduct。

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use rscatiacli::objects::{Application, Document, Documents, Product, Products};
use rscatiacli::{attach_with_guard, ComCollection, ComObj, ComObject, ComResult, Progress};

/// 已打开文档表条目：按文件名（小写）索引，保留 Document 以便重连。
#[derive(Clone)]
struct OpenDoc {
    name_lower: String,
    display_name: String,
    /// FullName；空串表示尚未保存（仅存在于内存）。
    path: String,
    doc: Document,
}

/// 一条断链信息（字段与 Python 版 `_check_component_link` 一致）。
struct BrokenLink {
    name: String,
    expected_path: String,
    status: &'static str,
    parent_path: String,
    filename_hint: String,
    found_in_open: bool,
    open_doc_path: String,
    open_in_memory: bool,
}

// ── 入口 ─────────────────────────────────────────────────────────────

/// 工具主体。**必须在 STA 线程上执行。**
pub fn run(params: &Value, progress: Progress<'_>) -> ComResult<Value> {
    let action = params
        .get("action")
        .and_then(Value::as_str)
        .unwrap_or("status");
    let search_paths: Vec<String> = params
        .get("search_paths")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let dry_run = params
        .get("dry_run")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    match action {
        "status" => Ok(rscatiacli::status_on_sta()),
        "scan" => Ok(scan(&search_paths, dry_run, progress)),
        "fix" => Ok(fix(&search_paths, dry_run, progress)),
        other => Ok(json!({"error": format!("未知 action: {other},可选 status/scan/fix")})),
    }
}

// ── status ───────────────────────────────────────────────────────────

// status 直接复用 rscatiacli::status_on_sta()（字段与 Python action_status 一致）。

// ── 已打开文档表 ─────────────────────────────────────────────────────

fn collect_open_documents(docs: &Documents) -> Vec<OpenDoc> {
    let mut out = Vec::new();
    if let Ok(items) = docs.items() {
        for d in items {
            let Ok(name) = d.name() else { continue };
            let path = d
                .full_name()
                .ok()
                .filter(|s| !s.is_empty())
                .unwrap_or_default();
            out.push(OpenDoc {
                name_lower: name.to_lowercase(),
                display_name: name,
                path,
                doc: d,
            });
        }
    }
    out
}

/// 按文件名线索在已打开文档中查找：先精确匹配（含扩展名），再按主名匹配，
/// 兼容 PartNumber 这类不带扩展名的线索。
fn lookup_open_document<'a>(hint: &str, open_docs: &'a [OpenDoc]) -> Option<&'a OpenDoc> {
    if hint.is_empty() || open_docs.is_empty() {
        return None;
    }
    let base = Path::new(hint)
        .file_name()?
        .to_string_lossy()
        .to_lowercase();
    if let Some(hit) = open_docs.iter().find(|d| d.name_lower == base) {
        return Some(hit);
    }
    let stem = Path::new(&base)
        .file_stem()?
        .to_string_lossy()
        .to_lowercase();
    if stem.is_empty() {
        return None;
    }
    open_docs.iter().find(|d| {
        Path::new(&d.name_lower)
            .file_stem()
            .map(|s| s.to_string_lossy().to_lowercase())
            .as_deref()
            == Some(stem.as_str())
    })
}

// ── 断链检查 ─────────────────────────────────────────────────────────

/// 检查单个组件（Product 实例）的参考链接是否断开。
/// 优先用官方 COM API 读真实引用文档路径：
/// `ReferenceProduct` → `Parent`（文档）→ `FullName`，磁盘上不存在即断链。
/// 返回 None 表示链接正常。
fn check_component_link(
    comp: &ComObj,
    open_docs: &[OpenDoc],
    parent_path: &str,
) -> Option<BrokenLink> {
    let name = comp
        .get("Name")
        .and_then(|v| v.as_string())
        .unwrap_or_else(|_| "Unknown".to_string());

    let mut expected_path = "unknown".to_string();
    let mut status: Option<&'static str> = None;
    let mut filename_hint: Option<String> = None;

    let ref_read: ComResult<()> = (|| {
        let rp = comp.get("ReferenceProduct")?.as_obj()?;
        let ref_doc = rp.get("Parent")?.as_obj()?;
        let full = ref_doc
            .get("FullName")
            .and_then(|v| v.as_string())
            .unwrap_or_default();
        if !full.is_empty() {
            expected_path = full.clone();
            filename_hint = Path::new(&full)
                .file_name()
                .map(|s| s.to_string_lossy().to_string());
            if !Path::new(&full).exists() {
                status = Some("file_missing");
            }
        } else {
            // 引用文档存在但尚未保存（纯内存文档），用文档名作线索
            status = Some("reference_not_saved");
            if let Ok(n) = ref_doc.get("Name").and_then(|v| v.as_string()) {
                filename_hint = Path::new(&n)
                    .file_name()
                    .map(|s| s.to_string_lossy().to_string());
            }
        }
        Ok(())
    })();
    if ref_read.is_err() {
        status = Some("reference_inaccessible");
    }

    let mut status = status?;
    let hint_empty = filename_hint.as_ref().is_none_or(|h| h.is_empty());
    if hint_empty {
        // PartNumber 兜底（减少误报）
        if let Ok(pn) = comp.get("PartNumber").and_then(|v| v.as_string()) {
            let pn = pn.trim().to_string();
            if !pn.is_empty() {
                filename_hint = Some(pn);
            }
        }
        let still_empty = filename_hint.as_ref().is_none_or(|h| h.is_empty());
        if still_empty && status == "reference_inaccessible" {
            status = "file_not_found_in_search_paths";
        }
    }

    let mut found_in_open = false;
    let mut open_doc_path = String::new();
    let mut open_in_memory = false;
    if let Some(hint) = filename_hint.as_ref().filter(|h| !h.is_empty()) {
        if let Some(hit) = lookup_open_document(hint, open_docs) {
            found_in_open = true;
            open_doc_path = hit.path.clone();
            open_in_memory = hit.path.is_empty();
        }
    }

    Some(BrokenLink {
        name,
        expected_path,
        status,
        parent_path: parent_path.to_string(),
        filename_hint: filename_hint.unwrap_or_default(),
        found_in_open,
        open_doc_path,
        open_in_memory,
    })
}

fn link_json(l: &BrokenLink) -> Value {
    json!({
        "type": "reference_part",
        "name": l.name,
        "expected_path": l.expected_path,
        "status": l.status,
        "parent_path": l.parent_path,
        "filename_hint": l.filename_hint,
        "found_in_open": l.found_in_open,
        "open_doc_path": l.open_doc_path,
        "open_in_memory": l.open_in_memory,
    })
}

/// 递归扫描产品树找断链（根产品即文档自身，不检查自指链接）。
fn find_broken_links(root: &Product, open_docs: &[OpenDoc]) -> Vec<BrokenLink> {
    fn walk(
        prod: &Product,
        parent_path: &str,
        open_docs: &[OpenDoc],
        out: &mut Vec<BrokenLink>,
        is_root: bool,
    ) {
        if !is_root {
            if let Some(link) = check_component_link(prod.com(), open_docs, parent_path) {
                out.push(link);
            }
        }
        let Ok(children) = prod.products() else {
            return;
        };
        let Ok(count) = children.count() else {
            return;
        };
        let self_name = prod.name().unwrap_or_default();
        for i in 1..=count {
            let Ok(child) = children.item(i) else {
                continue;
            };
            let child_path = if parent_path.is_empty() {
                self_name.clone()
            } else {
                format!("{parent_path}/{self_name}")
            };
            walk(&child, &child_path, open_docs, out, false);
        }
    }
    let mut out = Vec::new();
    walk(root, "", open_docs, &mut out, true);
    out
}

/// 已打开的 CATProduct 文档（名字后缀判断 + Product 属性可读确认）。
fn product_documents(docs: &Documents) -> Vec<Document> {
    docs.items()
        .map(|items| {
            items
                .into_iter()
                .filter(|d| {
                    d.name()
                        .map(|n| n.to_lowercase().ends_with(".catproduct"))
                        .unwrap_or(false)
                        && d.product_document().product().is_ok()
                })
                .collect()
        })
        .unwrap_or_default()
}

// ── FileSearchOrder ──────────────────────────────────────────────────

fn canonical_dir(path: &str) -> Option<String> {
    if path.is_empty() {
        return None;
    }
    let resolved = std::fs::canonicalize(path).unwrap_or_else(|_| PathBuf::from(path));
    let s = resolved.to_string_lossy().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// 把目录加入搜索顺序（去重）并同步 CATIA FileSearchOrder。
fn push_search_dir(
    app: &Application,
    order: &mut Vec<String>,
    dir: Option<&Path>,
    dry_run: bool,
    progress: Progress<'_>,
) {
    let Some(dir) = dir else { return };
    let Some(s) = canonical_dir(dir.to_string_lossy().as_ref()) else {
        return;
    };
    if order.contains(&s) {
        return;
    }
    order.push(s);
    if !dry_run {
        if let Err(e) = app.set_file_search_order(&order.join(";")) {
            progress(&format!("Error setting search paths: {e}"));
        }
    }
}

/// 磁盘搜索：先目录直配，再递归子目录（rglob 口径，文件名不区分大小写）。
fn find_file_in_search_paths(filename: &str, order: &[String]) -> Option<PathBuf> {
    if filename.is_empty() {
        return None;
    }
    let candidates: Vec<String> = if !filename.contains('.') {
        vec![
            format!("{filename}.CATPart"),
            format!("{filename}.CATPART"),
            format!("{filename}.catpart"),
        ]
    } else {
        vec![filename.to_string()]
    };
    for entry in order {
        let dir = Path::new(entry);
        if !dir.is_dir() {
            continue;
        }
        for name in &candidates {
            let direct = dir.join(name);
            if direct.is_file() {
                return Some(direct);
            }
        }
        for name in &candidates {
            if let Some(hit) = rglob_file(dir, name, 0) {
                return Some(hit);
            }
        }
    }
    None
}

fn rglob_file(dir: &Path, name: &str, depth: usize) -> Option<PathBuf> {
    if depth > 24 {
        return None; // 防御异常深的目录树
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return None;
    };
    for entry in entries.flatten() {
        let Ok(ft) = entry.file_type() else { continue };
        let fname = entry.file_name().to_string_lossy().to_string();
        if ft.is_dir() {
            if let Some(hit) = rglob_file(&entry.path(), name, depth + 1) {
                return Some(hit);
            }
        } else if fname.eq_ignore_ascii_case(name) {
            return Some(entry.path());
        }
    }
    None
}

// ── scan ─────────────────────────────────────────────────────────────

fn scan(search_paths: &[String], dry_run: bool, progress: Progress<'_>) -> Value {
    let mut result = json!({"products": [], "broken_links": [], "total_broken": 0});

    let mut inner = || -> ComResult<()> {
        let app = attach_with_guard(progress)?;
        if !search_paths.is_empty() && !dry_run {
            let mut order = current_search_order(&app);
            for p in search_paths {
                if let Some(s) = canonical_dir(p) {
                    if !order.contains(&s) {
                        order.push(s);
                    }
                }
            }
            if let Err(e) = app.set_file_search_order(&order.join(";")) {
                progress(&format!("Error setting search paths: {e}"));
            }
        }

        let docs = app.documents()?;
        let open_docs = collect_open_documents(&docs);
        let mut total_broken = 0i64;
        for doc in product_documents(&docs) {
            let name = doc.name().unwrap_or_default();
            let path = doc.full_name().unwrap_or_default();
            let links = match doc.product_document().product() {
                Ok(root) => find_broken_links(&root, &open_docs),
                Err(_) => Vec::new(),
            };
            for l in &links {
                result["broken_links"]
                    .as_array_mut()
                    .expect("broken_links 是数组")
                    .push(json!({
                        "document": name,
                        "name": l.name,
                        "expected_path": l.expected_path,
                        "status": l.status,
                    }));
                total_broken += 1;
            }
            result["products"]
                .as_array_mut()
                .expect("products 是数组")
                .push(json!({
                    "name": name,
                    "path": path,
                    "broken_links": links.iter().map(link_json).collect::<Vec<_>>(),
                }));
        }
        result["total_broken"] = json!(total_broken);
        Ok(())
    };

    if let Err(e) = inner() {
        result["error"] = json!(e.to_string());
    }
    result
}

fn current_search_order(app: &Application) -> Vec<String> {
    app.file_search_order()
        .unwrap_or_default()
        .split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect()
}

// ── fix ──────────────────────────────────────────────────────────────

fn fix(search_paths: &[String], dry_run: bool, progress: Progress<'_>) -> Value {
    let mut result = json!({
        "success": false,
        "message": "",
        "details": [],
        "processed_products": 0,
        "total_broken_links": 0,
        "resolved_links": 0,
        "resolved_by_open_doc": 0,
        "resolved_by_search_path": 0,
        "failed_resolutions": 0,
    });

    let mut inner = || -> ComResult<()> {
        let app = attach_with_guard(progress)?;
        if dry_run {
            result["message"] = json!("[DRY RUN] 只扫描, 不做任何修复动作");
        } else if !search_paths.is_empty() {
            // 与 Python 版一致：先写入用户给的搜索路径
            let mut order = current_search_order(&app);
            for p in search_paths {
                if let Some(s) = canonical_dir(p) {
                    if !order.contains(&s) {
                        order.push(s);
                    }
                }
            }
            if let Err(e) = app.set_file_search_order(&order.join(";")) {
                progress(&format!("Error setting search paths: {e}"));
            }
        }

        let docs = app.documents()?;
        let open_docs = collect_open_documents(&docs);
        // 搜索顺序表（含 FileSearchOrder 既有项），fix 过程中可能追加目录
        let mut order = current_search_order(&app);
        for p in search_paths {
            if let Some(s) = canonical_dir(p) {
                if !order.contains(&s) {
                    order.push(s);
                }
            }
        }

        let mut processed = 0i64;
        let mut total_broken = 0i64;
        let mut resolved_open = 0i64;
        let mut resolved_path = 0i64;
        let mut failed = 0i64;

        for doc in product_documents(&docs) {
            let doc_name = doc.name().unwrap_or_default();
            if !dry_run {
                progress(&format!("Processing product document: {doc_name}"));
            }
            let root = doc.product_document().product()?;
            let root_coll = root.products()?;

            // 两遍遍历：先收集全部断链组件，再逐一重链，避免重链过程中集合索引错位
            let mut to_fix: Vec<(ComObj, Products, BrokenLink)> = Vec::new();
            collect_fixables(&root_coll, "", &open_docs, &mut to_fix);
            let found = to_fix.len() as i64;

            for (comp, parent_coll, link) in &to_fix {
                let mut detail = json!({
                    "document": doc_name,
                    "component": link.name,
                    "expected_path": link.expected_path,
                    "result": "failed",
                    "message": "",
                });

                // 修复来源优先级: 已打开的文档 > 磁盘搜索路径
                let open_match = if link.filename_hint.is_empty() {
                    None
                } else {
                    lookup_open_document(&link.filename_hint, &open_docs).cloned()
                };

                if let Some(od) = open_match {
                    if !od.path.is_empty() {
                        push_search_dir(
                            &app,
                            &mut order,
                            Path::new(&od.path).parent(),
                            dry_run,
                            progress,
                        );
                    }
                    let path_note = if od.path.is_empty() {
                        " (未保存)".to_string()
                    } else {
                        format!(" ({})", od.path)
                    };
                    if dry_run {
                        resolved_open += 1;
                        detail["result"] = "resolved_from_open_doc".into();
                        detail["message"] = json!(format!(
                            "[DRY RUN] 将重连到已打开文档 {}{}",
                            od.display_name, path_note
                        ));
                    } else {
                        match relink_to_open_document(parent_coll, comp, &od.doc, progress) {
                            true => {
                                resolved_open += 1;
                                detail["result"] = "resolved_from_open_doc".into();
                                detail["message"] = json!(format!(
                                    "重连到已打开文档 {}{}",
                                    od.display_name, path_note
                                ));
                            }
                            false => {
                                if od.path.is_empty() {
                                    failed += 1;
                                    detail["message"] =
                                        json!("已打开文档未保存, 且 ReplaceProduct 重连失败");
                                } else {
                                    resolved_path += 1;
                                    detail["result"] = "resolved_from_open_doc_path".into();
                                    detail["message"] = json!(format!(
                                        "ReplaceProduct 重连失败, 改用已打开文档所在目录 {} 按搜索顺序重链",
                                        Path::new(&od.path).parent().map(|p| p.display().to_string()).unwrap_or_default()
                                    ));
                                }
                            }
                        }
                    }
                } else {
                    let probe = if link.filename_hint.is_empty() {
                        link.name.as_str()
                    } else {
                        link.filename_hint.as_str()
                    };
                    match find_file_in_search_paths(probe, &order) {
                        Some(found_file) => {
                            push_search_dir(
                                &app,
                                &mut order,
                                found_file.parent(),
                                dry_run,
                                progress,
                            );
                            resolved_path += 1;
                            detail["result"] = "resolved_from_search_path".into();
                            detail["message"] =
                                json!(format!("搜索路径找到文件 {}", found_file.display()));
                        }
                        None => {
                            failed += 1;
                            detail["message"] = json!("已打开文档与搜索路径中都找不到对应文件");
                        }
                    }
                }

                result["details"]
                    .as_array_mut()
                    .expect("details 是数组")
                    .push(detail);
            }

            // 触发 CATIA 按搜索顺序重链
            if !dry_run && found > 0 {
                if let Err(e) = doc.update() {
                    progress(&format!("Error updating document {doc_name}: {e}"));
                } else {
                    progress(&format!("Updated document: {doc_name}"));
                }
            }

            processed += 1;
            total_broken += found;
            progress(&format!(
                "Found {found} broken links in {doc_name}, \
                 resolved {resolved_open} via open docs, \
                 resolved {resolved_path} via search paths"
            ));
        }

        result["processed_products"] = json!(processed);
        result["total_broken_links"] = json!(total_broken);
        result["resolved_by_open_doc"] = json!(resolved_open);
        result["resolved_by_search_path"] = json!(resolved_path);
        result["resolved_links"] = json!(resolved_open + resolved_path);
        result["failed_resolutions"] = json!(failed);
        Ok(())
    };

    match inner() {
        Ok(()) => {
            result["success"] = json!(true);
            result["message"] = json!("Fixing broken links in open documents...");
            let failed = result["failed_resolutions"].as_i64().unwrap_or(0);
            if failed > 0 && !dry_run {
                // 兜底：保存并重开全部文档，触发 CATIA 完整重链（与 Python 版一致）
                result["message"] = json!("Reopening documents for full link restoration...");
                match attach_with_guard(progress) {
                    Ok(app) => result["reopen"] = restore_by_reopening(&app, progress),
                    Err(e) => result["error"] = json!(e.to_string()),
                }
            }
        }
        Err(e) => {
            result["error"] = json!(e.to_string());
        }
    }
    result
}

/// 深度遍历产品树，收集 (组件, 所在 Products 集合, 断链)。
fn collect_fixables(
    coll: &Products,
    path: &str,
    open_docs: &[OpenDoc],
    out: &mut Vec<(ComObj, Products, BrokenLink)>,
) {
    let Ok(count) = coll.count() else { return };
    for i in 1..=count {
        let Ok(comp) = coll.item(i) else { continue };
        if let Some(link) = check_component_link(comp.com(), open_docs, path) {
            out.push((comp.com().clone(), coll.clone(), link));
        }
        if let Ok(child_coll) = comp.products() {
            let name = comp.name().unwrap_or_default();
            let child_path = if path.is_empty() {
                name
            } else {
                format!("{path}/{name}")
            };
            collect_fixables(&child_coll, &child_path, open_docs, out);
        }
    }
}

/// 把断链组件直接重连到已打开的文档（未保存的内存文档也能连）。
fn relink_to_open_document(
    parent_coll: &Products,
    comp: &ComObj,
    open_doc: &Document,
    progress: Progress<'_>,
) -> bool {
    match open_doc
        .product_document()
        .product()
        .and_then(|new_ref| parent_coll.replace_product(comp, &new_ref, false))
    {
        Ok(()) => true,
        Err(e) => {
            progress(&format!("ReplaceProduct failed: {e}"));
            false
        }
    }
}

/// 兜底重链：保存（非只读）并关闭全部文档，再按原路径重开。
fn restore_by_reopening(app: &Application, progress: Progress<'_>) -> Value {
    let mut closed = 0i64;
    let mut reopened = 0i64;
    let mut errors = 0i64;

    let Ok(docs) = app.documents() else {
        return json!({"closed_documents": 0, "reopened_documents": 0, "errors": 0});
    };
    let mut paths: Vec<String> = Vec::new();
    if let Ok(items) = docs.items() {
        for doc in items {
            match doc.is_read_only() {
                Ok(false) | Err(_) => {
                    let _ = doc.save();
                }
                Ok(true) => {}
            }
            if let Ok(full) = doc.full_name() {
                if !full.is_empty() {
                    progress(&format!(
                        "Collected document to reopen: {}",
                        Path::new(&full)
                            .file_name()
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_else(|| full.clone())
                    ));
                    paths.push(full);
                }
            }
        }
    }

    progress("Closing all documents...");
    let total = docs.count().unwrap_or(0);
    for _ in 0..total.max(0) {
        if docs.count().unwrap_or(0) == 0 {
            break;
        }
        match docs.item(1).and_then(|d| d.close()) {
            Ok(()) => {
                closed += 1;
                progress(&format!("Closed document {closed}"));
            }
            Err(_) => errors += 1,
        }
    }

    std::thread::sleep(std::time::Duration::from_secs(1));

    progress("Reopening documents...");
    for path in &paths {
        let name = Path::new(path)
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| path.clone());
        match docs.open(path) {
            Ok(_) => {
                reopened += 1;
                progress(&format!("Reopened: {name}"));
            }
            Err(_) => errors += 1,
        }
    }
    json!({
        "closed_documents": closed,
        "reopened_documents": reopened,
        "errors": errors,
    })
}
