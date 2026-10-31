//! apply-naming：把文件名解析出的零件号/零件名写入 CATPart 文档。
//!
//! 对应 ltl-manage resources/sidecars/product_analysis/apply_naming.py。
//! 递归收集目录下 .CATPart，逐个 Open → Product.PartNumber/PartName 赋值 →
//! Save → Close；也接受显式 items 列表。PROGRESS:i/N 行走日志通道（前端按
//! PROGRESS: 前缀识别）。

use serde_json::{json, Value};

use rscatiacli::{attach_with_guard, ComResult, Progress};

use crate::naming_rules::parse_parts;

fn catpart_paths(root: &str) -> Vec<std::path::PathBuf> {
    fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        let Ok(read) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in read.flatten() {
            let p = entry.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.to_lowercase().ends_with(".catpart"))
                .unwrap_or(false)
            {
                out.push(p);
            }
        }
    }
    let mut paths = Vec::new();
    walk(std::path::Path::new(root), &mut paths);
    paths.sort();
    paths
}

/// Open + set PartNumber/PartName + save。返回单件结果 dict。
fn apply_to_document(
    app: &rscatiacli::objects::Application,
    fpath: &str,
    part_number: &str,
    part_name: &str,
) -> Value {
    let file_name = std::path::Path::new(fpath)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(fpath)
        .to_string();
    let mut item = json!({
        "name": file_name,
        "path": fpath,
        "part_number": part_number,
        "part_name": part_name,
        "success": false,
        "message": "",
    });
    let result = (|| -> ComResult<()> {
        let docs = app.documents()?;
        let doc = docs.open(fpath)?;
        let product = doc.product_document().product()?;
        if !part_number.is_empty() {
            product.set_part_number(part_number)?;
        }
        if !part_name.is_empty() {
            product.set_part_name(part_name)?;
        }
        doc.save()?;
        Ok(())
    })();
    match result {
        Ok(()) => {
            item["success"] = json!(true);
            item["message"] = json!("已写入并保存");
        }
        Err(e) => item["message"] = json!(e.to_string()),
    }
    // 关闭刚打开的文档（Open 会切换活动文档；本来就没文档时忽略错误）
    if let Ok(doc) = app.active_document() {
        let _ = doc.close();
    }
    item
}

/// 工具主体。**必须在 STA 线程上执行。**
pub fn run(params: &Value, progress: Progress<'_>) -> ComResult<Value> {
    let config = params.get("config").cloned().unwrap_or(Value::Null);
    let empty_items = json!({"items": [], "total": 0, "success_count": 0});

    // 组装 (path, part_number, part_name) 列表
    let mut targets: Vec<(String, String, String)> = Vec::new();
    if let Some(items) = params.get("items").and_then(|v| v.as_array()) {
        for it in items {
            let Some(p) = it.get("path").and_then(|v| v.as_str()) else {
                continue;
            };
            targets.push((
                p.to_string(),
                it.get("part_number")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                it.get("part_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
            ));
        }
    } else if let Some(directory) = params.get("directory").and_then(|v| v.as_str()) {
        if !std::path::Path::new(directory).is_dir() {
            return Ok(
                json!({"success": false, "message": format!("目录不存在: {directory}"), "data": empty_items}),
            );
        }
        for p in catpart_paths(directory) {
            let name = p
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default()
                .to_string();
            let (pn, pname) = parse_parts(&name, &config);
            targets.push((p.to_string_lossy().into_owned(), pn, pname));
        }
    } else {
        return Ok(
            json!({"success": false, "message": "缺少 directory 或 items 参数", "data": empty_items}),
        );
    }

    let total = targets.len();
    let app = attach_with_guard(progress)?;
    let _ = app.set_display_file_alerts(false);

    let mut items: Vec<Value> = Vec::new();
    let mut success_count = 0usize;
    for (i, (fpath, pn, pname)) in targets.iter().enumerate() {
        let disp = std::path::Path::new(fpath)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(fpath);
        progress(&format!("PROGRESS:{}/{total} {disp}", i + 1));
        let item = apply_to_document(&app, fpath, pn, pname);
        if item
            .get("success")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            success_count += 1;
        }
        items.push(item);
    }

    let _ = app.set_display_file_alerts(true);
    Ok(json!({
        "success": true,
        "message": format!("命名应用完成: {success_count}/{total}"),
        "data": {"items": items, "total": total, "success_count": success_count},
    }))
}
