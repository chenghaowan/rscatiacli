//! batch-bbox：目录递归批量边界框测量。
//!
//! 对应 ltl-manage resources/sidecars/product_analysis/batch_bbox.py（薄壳）：
//! 遍历目录下全部 .CATPart，逐件调 vba_bbox_generate（keep_geometry=False，
//! 测量后删除临时几何，不污染零件文件），收集长/宽/高/体积/材料/重量。
//! PROGRESS:{i}/{N} {name} 行按 Python 协议经 progress 输出（前端过滤显示；
//! Rust 进程内执行时日志随结果一起返回）。

use serde_json::{json, Value};
use std::path::{Path, PathBuf};

use rscatiacli::dispatch::Arg;
use rscatiacli::objects::Application;
use rscatiacli::{attach_with_guard, ComObject, ComResult, Progress};

use crate::naming_rules::parse_parts;
use crate::vba_bbox_generate::run as bbox_run;

/// 递归收集目录下全部 .CATPart（不区分扩展名大小写），按路径排序。
fn catpart_paths(root: &str) -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if path
                .extension()
                .map(|e| e.to_string_lossy().to_lowercase() == "catpart")
                .unwrap_or(false)
            {
                out.push(path);
            }
        }
    }
    let mut out = Vec::new();
    walk(Path::new(root), &mut out);
    out.sort_by(|a, b| a.to_string_lossy().cmp(&b.to_string_lossy()));
    out
}

/// 关闭刚打开的活动 CATPart 文档，避免窗口堆积。
fn close_active_catpart(app: &Application) {
    if let Ok(doc) = app.active_document() {
        let full = doc.full_name().unwrap_or_default();
        if full.to_lowercase().ends_with(".catpart") {
            let _ = doc.close();
        }
    }
}

pub fn run(params: &Value, progress: Progress<'_>) -> ComResult<Value> {
    let empty = json!({ "items": [], "total": 0, "success_count": 0 });
    let directory = params
        .get("directory")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if directory.is_empty() {
        return Ok(json!({
            "success": false, "message": "缺少 directory 参数", "data": empty
        }));
    }
    if !Path::new(directory).is_dir() {
        return Ok(json!({
            "success": false, "message": format!("目录不存在: {directory}"), "data": empty
        }));
    }
    let config = params.get("config").cloned().unwrap_or(Value::Null);

    // 连接一次 CATIA 并禁止保存弹窗，避免逐文件重复连接
    let app = match attach_with_guard(progress) {
        Ok(a) => a,
        Err(e) => {
            return Ok(json!({
                "success": false, "message": format!("无法连接 CATIA: {e}"), "data": empty
            }))
        }
    };
    let _ = app.com().put("DisplayFileAlerts", Arg::Bool(false));

    let files = catpart_paths(directory);
    let total = files.len();
    let mut items: Vec<Value> = Vec::new();
    let mut success_count = 0usize;

    for (i, fpath) in files.iter().enumerate() {
        let name = fpath
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        progress(&format!("PROGRESS:{}/{} {name}", i + 1, total));

        // 每个 Part 单独分析: 复用已验证的 VBA 精确测量(测量后删除临时几何)
        let file_params = json!({
            "file_path": fpath.to_string_lossy(),
            "keep_geometry": false,
            "config": config,
        });
        let (res_v, res_err) = match bbox_run(&file_params, progress) {
            Ok(v) => (Some(v), None),
            Err(e) => (None, Some(e.to_string())),
        };

        let mut item = json!({
            "name": name,
            "path": fpath.to_string_lossy(),
            "success": false,
            "message": "",
            "length_mm": null,
            "width_mm": null,
            "height_mm": null,
            "volume_mm3": null,
            "material": "",
            "density_g_per_cm3": null,
            "weight_g": null,
            "part_number": "",
            "part_name": "",
        });
        let ok = res_v
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
                if ok {
                    if let Some(data) = v.get("data") {
                        for k in ["length_mm", "width_mm", "height_mm", "volume_mm3"] {
                            item[k] = data.get(k).cloned().unwrap_or(Value::Null);
                        }
                    }
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
                    success_count += 1;
                }
            }
            None => {
                item["message"] =
                    Value::String(format!("测量异常: {}", res_err.unwrap_or_default()));
            }
        }
        // 零件号优先用 CATIA 零件本体属性, 解析不到再回退到文件名规则解析
        let (pn, pname) = parse_parts(&name, &config);
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
        items.push(item);

        close_active_catpart(&app);
    }

    Ok(json!({
        "success": true,
        "message": format!("批量边界框分析完成: {success_count}/{total}"),
        "data": { "items": items, "total": total, "success_count": success_count },
    }))
}
