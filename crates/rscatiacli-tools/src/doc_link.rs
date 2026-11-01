//! 同名文档联动（对应 ltl-manage resources/tools/drawing_common.py 的
//! `_ensure_doc_of_type` / `find_same_named_file` / `_open_document`）。
//!
//! 活动文档不是目标类型时，按 同目录 → 同名子目录(建图向导结构) → 下级递归(≤3 层)
//! → 上级目录(同套) 查找同名目标文件；已在会话中的直接切换激活，否则打开。
//! Part/Product/Drawing 三类同名联动共用。

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use rscatiacli::objects::{Application, Document};
use rscatiacli::{ComCollection, Progress};

/// 路径归一（小写 + 反斜杠，Windows 文件系统本就不分大小写）。
pub(crate) fn norm_path(p: &str) -> String {
    p.replace('/', "\\").to_lowercase()
}

/// 在 root 下按 同目录 → 同名子目录 → 递归(≤3 层) 找 <stem><ext>（跳过 . 开头隐藏目录）。
fn collect_same_named(dir: &Path, depth: usize, target: &str, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().map(|n| n.to_string_lossy().into_owned()) else {
            continue;
        };
        if path.is_dir() {
            if !name.starts_with('.') && depth < 3 {
                collect_same_named(&path, depth + 1, target, out);
            }
        } else if name.to_lowercase() == target {
            out.push(path);
        }
    }
}

/// 同目录 → 同名子目录 → 下级递归(≤3 层) → 上级目录(同套) 查找同名文件。
pub(crate) fn find_same_named_file(folder: &Path, stem: &str, ext: &str) -> Option<PathBuf> {
    if stem.is_empty() {
        return None;
    }
    let mut roots = vec![folder.to_path_buf()];
    if let Some(parent) = folder.parent() {
        if parent != folder {
            roots.push(parent.to_path_buf());
        }
    }
    let target = format!("{stem}{ext}").to_lowercase();
    for root in &roots {
        let mut candidates = vec![
            root.join(format!("{stem}{ext}")),
            root.join(stem).join(format!("{stem}{ext}")),
        ];
        collect_same_named(root, 0, &target, &mut candidates);
        if let Some(hit) = candidates.into_iter().find(|c| c.is_file()) {
            return Some(hit);
        }
    }
    None
}

/// 打开文档并返回其 COM 对象；已在会话中则直接切换激活（先按全路径匹配，
/// 兜底按文件名匹配 —— 不同目录同名文件各归各）。
pub(crate) fn open_document(app: &Application, path: &Path) -> Result<Document, String> {
    let key = norm_path(&path.to_string_lossy());
    let base = path
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let docs = app.documents().map_err(|e| e.to_string())?;
    let mut fallback: Option<Document> = None;
    if let Ok(items) = docs.items() {
        for d in items.iter() {
            let full = d.full_name().unwrap_or_default();
            let name = d.name().unwrap_or_default();
            if !full.is_empty() && norm_path(&full) == key {
                let _ = d.activate();
                return Ok(d.clone());
            }
            if fallback.is_none() && !name.is_empty() && name.to_lowercase() == base {
                fallback = Some(d.clone());
            }
        }
    }
    if let Some(f) = fallback {
        let _ = f.activate();
        return Ok(f);
    }
    app.documents()
        .and_then(|ds| ds.open(path.to_string_lossy().as_ref()))
        .map_err(|e| e.to_string())
}

/// 活动文档已是 target_ext 类型则原样返回; 否则按同名联动查找并切换/打开。
pub(crate) fn ensure_doc_of_type(
    app: &Application,
    target_ext: &str,
    target_desc: &str,
    progress: Progress<'_>,
) -> Result<Document, String> {
    let doc = app
        .active_document()
        .map_err(|_| "CATIA 中没有打开的文档".to_string())?;
    let name = doc.name().unwrap_or_default();
    let lower = name.to_lowercase();
    if lower.ends_with(&target_ext.to_lowercase()) {
        return Ok(doc);
    }
    // 三类同名联动范围: 图纸/零件/装配
    if lower.ends_with(".catpart")
        || lower.ends_with(".catproduct")
        || lower.ends_with(".catdrawing")
    {
        let stem = name
            .rsplit_once('.')
            .map(|(b, _)| b)
            .unwrap_or("")
            .to_string();
        let full = doc.full_name().unwrap_or_default();
        let folder = Path::new(&full).parent().map(|p| p.to_path_buf());
        let cand = folder
            .as_deref()
            .and_then(|f| find_same_named_file(f, &stem, target_ext));
        return match cand {
            Some(c) => {
                progress(&format!(
                    "当前文档为 {name}, 已按同名联动切换: {}",
                    c.display()
                ));
                open_document(app, &c)
            }
            None => {
                let hint = folder
                    .map(|f| format!("已查 <{}> 及其下级/上级目录", f.display()))
                    .unwrap_or_else(|| "文档未保存, 无路径可查找".to_string());
                Err(format!(
                    "当前打开的是 {name}, 未找到同名 {target_ext} —— {hint}。请先打开对应文档后重试"
                ))
            }
        };
    }
    Err(format!(
        "请先在 CATIA 中打开 {target_desc}(当前: {})",
        if name.is_empty() { "无文档" } else { &name }
    ))
}

/// 活动文档为 .CATProduct 则直接返回; 为 工程图/Part 时查找并打开同名装配。
pub(crate) fn ensure_product_doc(
    app: &Application,
    progress: Progress<'_>,
) -> Result<Document, String> {
    ensure_doc_of_type(app, ".CATProduct", "装配(.CATProduct)", progress)
}

/// 活动文档为 .CATPart 则直接返回; 为 工程图/Product 时查找并打开同名零件。
pub(crate) fn ensure_part_doc(
    app: &Application,
    progress: Progress<'_>,
) -> Result<Document, String> {
    ensure_doc_of_type(app, ".CATPart", "零件(.CATPart)", progress)
}

/// 已打开文档索引: 文档名 stem(小写) → FullName（产品树遍历匹配用）。
pub(crate) fn open_docs_index(app: &Application) -> HashMap<String, String> {
    let mut docs_index: HashMap<String, String> = HashMap::new();
    if let Ok(docs) = app.documents() {
        if let Ok(items) = docs.items() {
            for d in items.iter() {
                let name = d.name().unwrap_or_default();
                let full = d.full_name().unwrap_or_default();
                if name.is_empty() || full.is_empty() {
                    continue;
                }
                let stem = name
                    .rsplit_once('.')
                    .map(|(b, _)| b)
                    .unwrap_or(&name)
                    .to_lowercase();
                docs_index.insert(stem, full);
            }
        }
    }
    docs_index
}
