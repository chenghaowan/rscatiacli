//! find-near-parts：查找装配体中与目标零件 A 最小距离小于阈值的零件。
//!
//! 对应 ltl-manage resources/sidecars/product_analysis/find_near_parts.py。
//! Measurable.GetMinimumDistance 官方注明 Bodies 之间测不了 —— 零件间距离走
//! SPA Distance（临时 Group ×2 + BetweenTwo + Minimum，Compute 读 Value，
//! 读完即删）。目标定位优先级：seed_part（零件号→实例名→文件名）→
//! target_name/target_part_number → 当前选中项；action=list 只列叶零件。
//!
//! 与 Python 版差异：同名 .CATProduct 自动切换未移植（与 hide_show 同口径）。

use serde_json::{json, Value};

use rscatiacli::objects::{Distances, Group, Groups, Product};
use rscatiacli::{attach_with_guard, ComCollection, ComObject, ComResult, Progress};

/// 距离 < 该值视为相切/干涉。
const TOUCH_EPS: f64 = 1e-6;

fn name_of(obj: &impl ComObject) -> String {
    obj.name().unwrap_or_default()
}

struct Leaf {
    path: String,
    product: Product,
}

impl Leaf {
    fn instance_name(&self) -> String {
        name_of(&self.product)
    }

    fn info(&self) -> Value {
        json!({
            "instance_name": self.instance_name(),
            "part_number": self.product.part_number().unwrap_or_default(),
            "file_name": self.product.file_name().unwrap_or_default(),
            "path": self.path,
        })
    }
}

/// 递归收集叶级零件实例（Products.Count == 0）。
fn collect_leaf_parts(root: &Product) -> Vec<Leaf> {
    fn walk(p: &Product, path: &str, out: &mut Vec<Leaf>) {
        let children = p.products().and_then(|c| c.items()).unwrap_or_default();
        if children.is_empty() {
            out.push(Leaf {
                path: path.to_string(),
                product: p.clone(),
            });
        } else {
            for c in &children {
                let cname = name_of(c);
                walk(c, &format!("{path}/{cname}"), out);
            }
        }
    }
    let mut out = Vec::new();
    let root_name = name_of(root);
    walk(root, &root_name, &mut out);
    out
}

/// 叶零件字段匹配（先精确[忽略大小写]后子串，与 Python 一致）。
fn match_leaves<'a>(leaves: &'a [Leaf], key: &str, value: &str) -> Vec<&'a Leaf> {
    let value_l = value.trim().to_lowercase();
    if value_l.is_empty() {
        return Vec::new();
    }
    let field = |l: &Leaf| -> String {
        match key {
            "part_number" => l.product.part_number().unwrap_or_default(),
            "file_name" => l.product.file_name().unwrap_or_default(),
            _ => l.instance_name(),
        }
    };
    let exact: Vec<&Leaf> = leaves
        .iter()
        .filter(|l| field(l).trim().to_lowercase() == value_l)
        .collect();
    if !exact.is_empty() {
        return exact;
    }
    leaves
        .iter()
        .filter(|l| field(l).to_lowercase().contains(&value_l))
        .collect()
}

/// 定位目标 A。返回 Ok(leaf) 或 Err(错误信息)。
fn resolve_target<'a>(
    leaves: &'a [Leaf],
    params: &Value,
    doc: &rscatiacli::objects::Document,
) -> Result<&'a Leaf, String> {
    let seed = params
        .get("seed_part")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    if !seed.is_empty() {
        for key in ["part_number", "name", "file_name"] {
            let matches = match_leaves(leaves, key, seed);
            if !matches.is_empty() {
                return Ok(matches[0]);
            }
        }
        return Err(format!("未找到种子零件: {seed},可用零件见 data.leaves"));
    }

    let tname = params
        .get("target_name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let tpn = params
        .get("target_part_number")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if !tname.is_empty() || !tpn.is_empty() {
        let matches = if !tname.is_empty() {
            match_leaves(leaves, "name", &tname)
        } else {
            match_leaves(leaves, "part_number", &tpn)
        };
        if matches.is_empty() {
            return Err(format!(
                "未找到目标零件 A({}),可用零件见 data.leaves",
                if tname.is_empty() { &tpn } else { &tname }
            ));
        }
        return Ok(matches[0]);
    }

    // 未指定 → 当前选中项
    let sel = doc.selection().map_err(|e| e.to_string())?;
    if sel.count().unwrap_or(0) == 0 {
        return Err("未指定 target_name/target_part_number,且当前未选中任何零件".to_string());
    }
    let item = sel.item(1).map_err(|e| e.to_string())?;
    let Ok(value) = item.value() else {
        return Err("当前选中项不是装配体中的叶级零件实例".to_string());
    };
    let sel_name = value.name().unwrap_or_default();
    for l in leaves {
        if l.instance_name() == sel_name {
            return Ok(l);
        }
    }
    Err("当前选中项不是装配体中的叶级零件实例".to_string())
}

/// 目标 A 组 + 候选 B 组的临时 Distance 测量。返回 (value_mm, is_defined)。
fn measure_min_distance(
    group_a: &Group,
    candidate: &Leaf,
    groups: &Groups,
    distances: &Distances,
    seq: usize,
) -> ComResult<(f64, i32)> {
    let g = groups.add()?;
    g.set_name(&format!("__near_g_{seq}"))?;
    let d = distances.add()?;
    d.set_name(&format!("__near_d_{seq}"))?;
    let result = (|| -> ComResult<(f64, i32)> {
        g.add_explicit(&candidate.product)?;
        d.set_first_group(group_a)?;
        d.set_second_group(&g)?;
        d.set_computation_type(rscatiacli::objects::CAT_DISTANCE_BETWEEN_TWO)?;
        d.set_measure_type(rscatiacli::objects::CAT_DISTANCE_MEASURE_MINIMUM)?;
        d.compute()?;
        Ok((d.value()?, d.is_defined()?))
    })();
    // finally 兜底清理：临时 Distance / Group 读删不留孤儿
    let _ = distances.remove(&format!("__near_d_{seq}"));
    let _ = groups.remove(&format!("__near_g_{seq}"));
    result
}

/// 工具主体。**必须在 STA 线程上执行。**
pub fn run(params: &Value, progress: Progress<'_>) -> ComResult<Value> {
    let empty_data = json!({"items": [], "total": 0, "match_count": 0});
    let max_distance = params
        .get("max_distance")
        .and_then(|v| v.as_f64())
        .unwrap_or(5.0);
    let dry_run = params
        .get("dry_run")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let app = attach_with_guard(progress)?;
    let _ = app.set_display_file_alerts(false);
    let doc = app.active_document()?;
    let doc_name = name_of(&doc);
    if !doc_name.to_lowercase().ends_with(".catproduct") {
        return Ok(json!({
            "success": false,
            "message": format!("当前是{}({doc_name}): Rust 版尚未移植同名装配自动切换, 请先打开对应 .CATProduct 后重试",
                if doc_name.to_lowercase().ends_with(".catpart") { "零件" } else { "非装配文档" }),
            "data": empty_data,
        }));
    }

    let root = doc.product_document().product()?;
    // 可视化模式(CGR)下零件几何未加载 → 测量前先切设计模式并激活终端节点
    let _ = root.apply_work_mode(2);
    let _ = root.activate_default_shape();
    let leaves = collect_leaf_parts(&root);

    // action=list: 只列出装配中所有叶零件（前端种子下拉）
    if params.get("action").and_then(|v| v.as_str()) == Some("list") {
        return Ok(json!({
            "success": true,
            "message": format!("装配中共 {} 个叶零件", leaves.len()),
            "data": {
                "items": [],
                "total": leaves.len(),
                "match_count": 0,
                "leaves": leaves.iter().map(|l| l.info()).collect::<Vec<_>>(),
            },
        }));
    }

    if leaves.len() < 2 {
        return Ok(json!({
            "success": false,
            "message": format!("装配体中只有 {} 个叶零件,无法比较", leaves.len()),
            "data": {"items": [], "total": leaves.len(), "match_count": 0},
        }));
    }

    let target = match resolve_target(&leaves, params, &doc) {
        Ok(t) => t,
        Err(err) => {
            return Ok(json!({
                "success": false,
                "message": err,
                "data": {
                    "items": [], "total": leaves.len(), "match_count": 0,
                    "leaves": leaves.iter().map(|l| l.info()).collect::<Vec<_>>(),
                },
            }));
        }
    };
    let target_info = target.info();
    let total = leaves.len() - 1;

    if dry_run {
        return Ok(json!({
            "success": true,
            "message": format!(
                "[dry-run] 目标 A={};共 {total} 个零件待测量,阈值 {max_distance} mm",
                target.instance_name()
            ),
            "data": {
                "items": [], "total": total, "match_count": 0,
                "target": target_info,
                "max_distance": max_distance,
                "leaves": leaves.iter().filter(|l| l.path != target.path)
                    .map(|l| l.info()).collect::<Vec<_>>(),
            },
        }));
    }

    // ── 正式测量 ──
    let _ = root.update();

    // Groups / Distances 挂在根产品的技术对象上（缺 SPA 许可时这里会失败）
    let (groups, distances) = match (|| -> ComResult<(Groups, Distances)> {
        let g = Groups::new(root.get_technological_object("Groups")?.com().clone());
        let d = Distances::new(root.get_technological_object("Distances")?.com().clone());
        Ok((g, d))
    })() {
        Ok(v) => v,
        Err(e) => {
            return Ok(json!({
                "success": false,
                "message": format!("获取 Groups/Distances 失败(可能缺少 SPA/Space Analysis 许可): {e}"),
                "data": {"items": [], "total": total, "match_count": 0},
            }));
        }
    };

    // 目标 A 的 Group（复用，finally 清理）
    let group_a = groups.add()?;
    group_a.set_name("__near_target_a")?;
    group_a.add_explicit(&target.product)?;

    let mut items: Vec<Value> = Vec::new();
    let mut failures = 0usize;
    let mut match_count = 0usize;
    let mut seq = 0usize;
    let mut idx = 0usize;
    for leaf in &leaves {
        if leaf.path == target.path {
            continue;
        }
        idx += 1;
        progress(&format!("PROGRESS:{idx}/{total} {}", leaf.instance_name()));
        seq += 1;
        let mut item = json!({
            "instance_name": leaf.instance_name(),
            "part_number": leaf.product.part_number().unwrap_or_default(),
            "file_name": leaf.product.file_name().unwrap_or_default(),
            "path": leaf.path,
            "distance_mm": Value::Null,
            "touching": false,
            "status": "ok",
            "note": "",
        });
        match measure_min_distance(&group_a, leaf, &groups, &distances, seq) {
            Ok((value, 1)) => {
                item["distance_mm"] = json!((value * 10000.0).round() / 10000.0);
                item["touching"] = json!(value.abs() < TOUCH_EPS);
                if value < max_distance {
                    match_count += 1;
                    item["status"] = json!("match");
                }
            }
            Ok((_, is_defined)) => {
                item["status"] = json!("error");
                item["note"] = json!(format!("距离未定义(IsDefined={is_defined})"));
                failures += 1;
            }
            Err(e) => {
                item["status"] = json!("error");
                item["note"] = json!(e.to_string());
                failures += 1;
            }
        }
        items.push(item);
    }
    // 只清理本次创建的目标 Group
    let _ = groups.remove("__near_target_a");

    Ok(json!({
        "success": true,
        "message": format!(
            "分析完成: {match_count}/{total} 个零件距 A({}) < {max_distance} mm,失败 {failures} 个",
            target.instance_name()
        ),
        "data": {
            "items": items,
            "total": total,
            "match_count": match_count,
            "failures": failures,
            "target": target_info,
            "max_distance": max_distance,
        },
    }))
}
