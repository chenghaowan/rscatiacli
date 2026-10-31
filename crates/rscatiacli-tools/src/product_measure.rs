//! product-measure：遍历当前激活 CATProduct 下所有 CATPart 逐件测量。
//!
//! 对应 ltl-manage resources/sidecars/product_analysis/product_measure.py（薄壳）：
//! - measure=bbox : vba_bbox_generate（keep_geometry=False，测量后清理临时几何）
//! - measure=area : rule_projected_surface（suppress_save/suppress_json，不改零件文件）
//! - measure=both : 先边界框（顺带建立 tooling axis），再投影面积（复用已有
//!   tooling axis），两组数据合并；两项都成功才算成功
//!
//! 每个 Part 打开→测量→关闭，Product 文档保持激活；零件文件不被保存/修改。
//!
//! ITEM:{json} 行按 Python 协议经 progress 输出（Python 路径下前端实时加进
//! 表格；Rust 进程内执行时随日志一起返回，前端以最终 data.items 兜底覆盖）。

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use rscatiacli::dispatch::Arg;
use rscatiacli::objects::{Application, Document, Documents, Product};
use rscatiacli::{attach_with_guard, ComCollection, ComObject, ComResult, Progress};
use serde_json::{json, Value};

use crate::naming_rules::parse_parts;
use crate::rule_projected_surface::run as area_run;
use crate::vba_bbox_generate::run as bbox_run;

fn empty_data() -> Value {
    json!({ "items": [], "total": 0, "success_count": 0 })
}

/// 单件结果的消息合并（both 模式两步消息拼接；COM 级失败取错误文本）。
fn msg_of(r: &Result<Value, impl std::fmt::Display>) -> String {
    match r {
        Ok(v) => v
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("")
            .to_string(),
        Err(e) => e.to_string(),
    }
}

// ── 同名文档联动（drawing_common 的 ensure_product_doc 子集） ──────────

/// 路径归一（小写 + 反斜杠，Windows 文件系统本就不分大小写）。
fn norm_path(p: &str) -> String {
    p.replace('/', "\\").to_lowercase()
}

/// 在 root 下按 同目录 → 同名子目录 → 递归(≤3 层) 找 <stem><ext>（对应
/// drawing_common._same_named_in_dir；跳过 . 开头隐藏目录）。
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
fn find_same_named_file(folder: &Path, stem: &str, ext: &str) -> Option<PathBuf> {
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
/// 兜底按文件名匹配）。对应 drawing_common._open_document。
fn open_document(app: &Application, path: &Path) -> Result<Document, String> {
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

/// 活动文档为 .CATProduct 则直接返回; 为 工程图/Part 时查找并打开同名装配。
/// 对应 drawing_common.ensure_product_doc。
fn ensure_product_doc(app: &Application, progress: Progress<'_>) -> Result<Document, String> {
    let doc = app
        .active_document()
        .map_err(|_| "CATIA 中没有打开的文档".to_string())?;
    let name = doc.name().unwrap_or_default();
    let lower = name.to_lowercase();
    if lower.ends_with(".catproduct") {
        return Ok(doc);
    }
    if lower.ends_with(".catpart") || lower.ends_with(".catdrawing") {
        let stem = name
            .rsplit_once('.')
            .map(|(b, _)| b)
            .unwrap_or("")
            .to_string();
        let full = doc.full_name().unwrap_or_default();
        let folder = Path::new(&full).parent().map(|p| p.to_path_buf());
        let cand = folder
            .as_deref()
            .and_then(|f| find_same_named_file(f, &stem, ".CATProduct"));
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
                    "当前打开的是 {name}, 未找到同名 .CATProduct —— {hint}。请先打开对应文档后重试"
                ))
            }
        };
    }
    Err(format!(
        "请先在 CATIA 中打开 装配(.CATProduct)(当前: {})",
        if name.is_empty() { "无文档" } else { &name }
    ))
}

/// 强制产品树进入设计模式并逐节点激活终端节点（对应 drawing_common.force_design_mode；
/// 可视化模式 CGR 下零件文档未加载, 遍历产品树会一无所获）。
fn force_design_mode(product: &Product, progress: Progress<'_>) {
    let mut n = 0usize;
    if product.apply_work_mode(2).is_ok() {
        n += 1;
    }
    fn walk(prod: &Product, depth: usize, n: &mut usize) {
        if depth > 12 {
            return;
        }
        let Ok(children) = prod.products() else {
            return;
        };
        let Ok(items) = children.items() else {
            return;
        };
        for ch in items.iter() {
            if ch.com().call("ActivateDefaultShape", &[]).is_ok() {
                *n += 1;
            }
            let _ = ch.apply_work_mode(2);
            walk(ch, depth + 1, n);
        }
    }
    walk(product, 0, &mut n);
    if n > 0 {
        progress(&format!("已切换设计模式并激活终端节点(共 {n} 个节点)"));
    }
}

// ── 产品树遍历 ───────────────────────────────────────────────────────

/// 递归遍历产品树, 收集所有已加载 CATPart 的 (name, path)（对应 _walk_parts）。
fn walk_parts(product: &Product, all_docs: &Documents) -> Vec<(String, String)> {
    // 已打开文档按名匹配(产品打开时其零件文档已加载; 忽略大小写, 去扩展名)
    let mut docs_index: HashMap<String, String> = HashMap::new();
    if let Ok(items) = all_docs.items() {
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
    let mut seen: HashSet<String> = HashSet::new();
    let mut out: Vec<(String, String)> = Vec::new();

    fn walk(
        prod: &Product,
        docs_index: &HashMap<String, String>,
        seen: &mut HashSet<String>,
        out: &mut Vec<(String, String)>,
    ) {
        let Ok(children) = prod.products() else {
            return;
        };
        let Ok(items) = children.items() else {
            return;
        };
        for ch in items.iter() {
            let ref_name = ch
                .reference_product()
                .and_then(|r| r.name())
                .unwrap_or_default();
            // 与已打开文档按名匹配(参考产品名即零件文档名 stem, 忽略大小写)
            let found_path = docs_index.get(&ref_name.to_lowercase()).cloned();
            match found_path {
                Some(path) => {
                    if seen.insert(path.clone()) {
                        out.push((ref_name, path));
                    }
                }
                None => walk(ch, docs_index, seen, out), // 子 Product, 继续递归
            }
        }
    }
    walk(product, &docs_index, &mut seen, &mut out);
    out
}

/// 关闭刚打开的 Part 文档避免窗口堆积（Product 文档保持激活）。
fn close_active_catpart(app: &Application) {
    if let Ok(doc) = app.active_document() {
        let full = doc.full_name().unwrap_or_default();
        let base = full
            .rsplit(['\\', '/'])
            .next()
            .unwrap_or(&full)
            .to_lowercase();
        if base.ends_with(".catpart") {
            let _ = doc.close();
        }
    }
}

// ── 主流程 ───────────────────────────────────────────────────────────

pub fn run(params: &Value, progress: Progress<'_>) -> ComResult<Value> {
    let measure = params
        .get("measure")
        .and_then(|v| v.as_str())
        .unwrap_or("bbox");
    if !["bbox", "area", "both"].contains(&measure) {
        return Ok(json!({
            "success": false,
            "message": format!("未知 measure: {measure}(仅支持 bbox / area / both)"),
            "data": empty_data(),
        }));
    }
    let config = params.get("config").cloned().unwrap_or(Value::Null);

    let app = match attach_with_guard(progress) {
        Ok(a) => a,
        Err(e) => {
            return Ok(json!({
                "success": false, "message": format!("连接 CATIA 失败: {e}"), "data": empty_data()
            }))
        }
    };
    let _ = app.com().put("DisplayFileAlerts", Arg::Bool(false));
    let all_docs = match app.documents() {
        Ok(d) => d,
        Err(e) => {
            return Ok(json!({
                "success": false, "message": format!("连接 CATIA 失败: {e}"), "data": empty_data()
            }))
        }
    };

    // 当前为图纸/零件时自动切换到同名 .CATProduct
    let doc = match ensure_product_doc(&app, progress) {
        Ok(d) => d,
        Err(e) => {
            return Ok(json!({
                "success": false, "message": e, "data": empty_data()
            }))
        }
    };
    let product = match doc.product_document().product() {
        Ok(p) => p,
        Err(e) => {
            return Ok(json!({
                "success": false, "message": format!("读取 Product 失败: {e}"), "data": empty_data()
            }))
        }
    };
    // 可视化模式(CGR)下零件文档未加载 → 遍历前先切设计模式并激活终端节点
    force_design_mode(&product, progress);
    let part_paths = walk_parts(&product, &all_docs);
    progress(&format!(
        "[列表] Product 下共收集 {} 个 CATPart",
        part_paths.len()
    ));
    if part_paths.is_empty() {
        return Ok(json!({
            "success": false,
            "message": "Product 下未找到已加载的 CATPart(已自动尝试切设计模式加载, 仍为空; 请确认零件文件存在且可打开)",
            "data": empty_data(),
        }));
    }

    let total = part_paths.len();
    let mut items: Vec<Value> = Vec::new();
    for (idx, (name, path)) in part_paths.iter().enumerate() {
        progress(&format!("PROGRESS:{}/{} {name}", idx + 1, total));

        // 打开/聚焦零件文档, 再测量
        let (res_v, res_err) = if let Err(e) = all_docs.open(path) {
            (None, format!("测量异常: {e}"))
        } else {
            match measure_one(measure, path, &config, progress) {
                Ok(v) => (Some(v), String::new()),
                Err(e) => (None, e),
            }
        };

        let mut item = json!({
            "name": name,
            "path": path,
            "success": false,
            "message": "",
            "part_number": "",
            "part_name": "",
            "material": "",
            "density_g_per_cm3": null,
            "weight_g": null,
        });
        let success = res_v
            .as_ref()
            .and_then(|v| v.get("success"))
            .and_then(|b| b.as_bool())
            .unwrap_or(false);
        match &res_v {
            Some(v) => {
                item["message"] = v
                    .get("message")
                    .cloned()
                    .unwrap_or(Value::String(String::new()));
                item["part_number"] = v
                    .get("part_number")
                    .cloned()
                    .unwrap_or(Value::String(String::new()));
                item["part_name"] = v
                    .get("part_name")
                    .cloned()
                    .unwrap_or(Value::String(String::new()));
                item["material"] = v
                    .get("material")
                    .cloned()
                    .unwrap_or(Value::String(String::new()));
                item["density_g_per_cm3"] =
                    v.get("density_g_per_cm3").cloned().unwrap_or(Value::Null);
                item["weight_g"] = v.get("weight_g").cloned().unwrap_or(Value::Null);
                if success {
                    if let Some(data) = v.get("data") {
                        for k in [
                            "length_mm",
                            "width_mm",
                            "height_mm",
                            "volume_mm3",
                            "projected_area_xy_mm2",
                        ] {
                            if let Some(x) = data.get(k) {
                                item[k] = x.clone();
                            }
                        }
                    }
                }
            }
            None => {
                item["message"] = Value::String(res_err);
            }
        }

        // 零件号/名回退: 从文件名按命名规则解析
        let filename = Path::new(path)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let (pn, pname) = parse_parts(&filename, &config);
        if item["part_number"]
            .as_str()
            .map(|s| s.is_empty())
            .unwrap_or(true)
        {
            item["part_number"] = json!(pn);
        }
        if item["part_name"]
            .as_str()
            .map(|s| s.is_empty())
            .unwrap_or(true)
        {
            item["part_name"] = json!(pname);
        }

        // 单件失败跳过, 不中断后续零件（失败原因按 error 级别上屏）
        if !success {
            let msg = item["message"].as_str().unwrap_or("未知原因");
            progress(&format!(
                "✗ [{}/{}] {name} 测量失败(已跳过): {}",
                idx + 1,
                total,
                if msg.is_empty() { "未知原因" } else { msg }
            ));
        }
        progress(&format!(
            "ITEM:{}",
            serde_json::to_string(&item).unwrap_or_default()
        ));
        items.push(item);

        close_active_catpart(&app);
    }

    let success_count = items
        .iter()
        .filter(|it| it.get("success").and_then(|b| b.as_bool()).unwrap_or(false))
        .count();
    // 恢复 Product 文档激活（批量过程中关闭了各 Part, 激活文档可能被切走）
    let _ = doc.activate();
    Ok(json!({
        "success": true,
        "message": format!("Product 测量完成: 成功 {success_count}/{total}"),
        "data": { "items": items, "total": total, "success_count": success_count },
        "measure": measure,
    }))
}

/// 单件测量（对应 core_logic 里 measure 分支）。
fn measure_one(
    measure: &str,
    path: &str,
    config: &Value,
    progress: Progress<'_>,
) -> Result<Value, String> {
    match measure {
        "bbox" => {
            let p = json!({ "file_path": path, "keep_geometry": false, "config": config });
            bbox_run(&p, progress).map_err(|e| format!("测量异常: {e}"))
        }
        "area" => {
            let p = json!({
                "file_path": path,
                "suppress_screenshot": true,
                "establish_tooling_axis": true,
                "suppress_save": true,
                "suppress_json": true,
            });
            area_run(&p, progress).map_err(|e| format!("测量异常: {e}"))
        }
        // both: 先边界框(顺带建立 tooling axis), 再投影面积(复用已有 tooling axis);
        // 两步各自独立容错, 数据合并, 两步都成功该项才算成功
        _ => {
            let p_b = json!({ "file_path": path, "keep_geometry": false, "config": config });
            let p_a = json!({
                "file_path": path,
                "suppress_screenshot": true,
                "establish_tooling_axis": true,
                "suppress_save": true,
                "suppress_json": true,
            });
            let res_b = bbox_run(&p_b, progress);
            let res_a = area_run(&p_a, progress);
            let b_ok = res_b
                .as_ref()
                .ok()
                .and_then(|v| v.get("success"))
                .and_then(|b| b.as_bool())
                .unwrap_or(false);
            let a_ok = res_a
                .as_ref()
                .ok()
                .and_then(|v| v.get("success"))
                .and_then(|b| b.as_bool())
                .unwrap_or(false);
            let mut data = serde_json::Map::new();
            if let Ok(v) = &res_b {
                if let Some(d) = v.get("data").and_then(|d| d.as_object()) {
                    for (k, val) in d {
                        data.insert(k.clone(), val.clone());
                    }
                }
            }
            if let Ok(v) = &res_a {
                if let Some(d) = v.get("data").and_then(|d| d.as_object()) {
                    for (k, val) in d {
                        data.insert(k.clone(), val.clone());
                    }
                }
            }
            let msgs = [msg_of(&res_b), msg_of(&res_a)]
                .into_iter()
                .filter(|m| !m.is_empty())
                .collect::<Vec<_>>()
                .join("; ");
            let pick = |key: &str| -> Value {
                res_b
                    .as_ref()
                    .ok()
                    .and_then(|v| v.get(key))
                    .filter(|x| !x.is_null())
                    .cloned()
                    .or_else(|| {
                        res_a
                            .as_ref()
                            .ok()
                            .and_then(|v| v.get(key))
                            .filter(|x| !x.is_null())
                            .cloned()
                    })
                    .unwrap_or(Value::Null)
            };
            Ok(json!({
                "success": b_ok && a_ok,
                "message": msgs,
                "part_number": pick("part_number"),
                "part_name": pick("part_name"),
                "material": pick("material"),
                "density_g_per_cm3": pick("density_g_per_cm3"),
                "weight_g": pick("weight_g"),
                "data": Value::Object(data),
            }))
        }
    }
}
