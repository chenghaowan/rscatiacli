//! hide-show：元素显隐控制。
//!
//! 对应 ltl-manage resources/sidecars/hide_show/hide_show.py（771 行）。
//!
//! 5 个动作：
//!   show-body   只显示实体(Body)，隐藏几何图形集/轴系/参考面/约束
//!   show-csys   显示坐标系，隐藏几何图形集/参考面/约束
//!   hide-all    隐藏全部（只隐藏顶层容器，GS 子元素不逐个处理）
//!   show-named  显示名字含 draft/slide/split 的元素（连带其所在几何图形集）
//!   hide-named  隐藏名字含 draft/slide/split 的元素
//!
//! 隐藏策略与 Python 版一致：几何图形集只隐藏 GS 节点本身，子元素随父级不可见。
//! 装配体：递归遍历 Product 树，对每个 CATPart 执行同一动作；前三个动作还会隐藏
//! 装配体自身的装配约束（属于 Product，不在任何 Part 内）。
//!
//! 本期未移植：活动文档是工程图时自动切换到同名 零件/装配（Python 走
//! drawing_common 的同名文档联动）——当前直接返回明确错误。

use serde_json::{json, Value};

use rscatiacli::dispatch::{ComObj, ComObject};
use rscatiacli::objects::{Document, Part, Product, Selection};
use rscatiacli::{attach_with_guard, ComCollection, ComResult, Progress};

const KEYWORDS: [&str; 3] = ["draft", "slide", "split"];

/// 只命中当前可见的 3D 约束（与原始 FIS 宏同款搜索式）
const PART_CONSTRAINT_SEARCHES: [&str; 1] = ["'Part Design'.Constraint.Visibility=Visible,All"];
/// 装配约束挂在 Product 文档上
const PRODUCT_CONSTRAINT_SEARCHES: [&str; 2] = [
    "CATAsmSearch.MfConstraint.Visibility=Visible,All",
    "CATAsmSearch.MfConstraint,All",
];

const REF_PLANE_LABELS: [&str; 3] = ["xy 平面", "yz 平面", "zx 平面"];

struct ActionResult {
    shown: usize,
    hidden: usize,
    shown_names: Vec<String>,
    hidden_names: Vec<String>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Action {
    ShowBody,
    ShowCsys,
    HideAll,
    ShowNamed,
    HideNamed,
}

impl Action {
    fn parse(s: &str) -> Option<Self> {
        match s {
            "show-body" => Some(Self::ShowBody),
            "show-csys" => Some(Self::ShowCsys),
            "hide-all" => Some(Self::HideAll),
            "show-named" => Some(Self::ShowNamed),
            "hide-named" => Some(Self::HideNamed),
            _ => None,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::ShowBody => "只显示实体",
            Self::ShowCsys => "显示所有坐标系",
            Self::HideAll => "隐藏所有",
            Self::ShowNamed => "显示 draft/slide/split 元素",
            Self::HideNamed => "隐藏 draft/slide/split 元素",
        }
    }

    fn hides_product_constraints(self) -> bool {
        matches!(self, Self::ShowBody | Self::ShowCsys | Self::HideAll)
    }
}

// ── 基础读取 ─────────────────────────────────────────────────────────

fn name_of(obj: &impl ComObject) -> String {
    obj.name().unwrap_or_default()
}

fn collection_items(coll: &ComObj) -> Vec<ComObj> {
    let Ok(count) = coll.get("Count").and_then(|v| v.as_i32()) else {
        return Vec::new();
    };
    (1..=count)
        .filter_map(|i| rscatiacli::item_at(coll, i).ok())
        .collect()
}

fn child_items(obj: &impl ComObject, prop: &str) -> Vec<ComObj> {
    obj.com()
        .get(prop)
        .and_then(|v| v.as_obj())
        .map(|coll| collection_items(&coll))
        .unwrap_or_default()
}

/// Part 的参考坐标系三平面。CATIA 的 Part 没有 ReferenceElements，
/// 参考面实际是 OriginElements 的三个基准平面。
fn origin_planes(part: &Part) -> Vec<ComObj> {
    let Ok(origin) = part.origin_elements() else {
        return Vec::new();
    };
    [origin.plane_xy(), origin.plane_yz(), origin.plane_zx()]
        .into_iter()
        .filter_map(|p| p.ok())
        .map(|p| p.com().clone())
        .collect()
}

fn ref_items(refs: &[ComObj]) -> Vec<(ComObj, String)> {
    refs.iter()
        .enumerate()
        .map(|(i, r)| {
            let name = name_of(r);
            let label = if name.is_empty() {
                REF_PLANE_LABELS.get(i).unwrap_or(&"参考面").to_string()
            } else {
                name
            };
            (r.clone(), label)
        })
        .collect()
}

// ── 选择集操作 ───────────────────────────────────────────────────────

/// 把元素选中并应用 SetShow（1=隐藏，0=显示，与 Python 版口径一致）。
fn apply_show(sel: &Selection, items: &[ComObj], show_val: i32) {
    for obj in items {
        let _ = sel.add(obj);
    }
    if let Ok(vis) = sel.vis_properties() {
        let _ = vis.set_show(show_val);
    }
    let _ = sel.clear();
}

/// 按候选搜索式依次尝试，返回 [(对象, 名字)]；全部失败返回 None。
fn search_items(sel: &Selection, predicates: &[&str]) -> Option<Vec<(ComObj, String)>> {
    let _ = sel.clear();
    let mut hit = false;
    for pred in predicates {
        if sel.search(pred).is_ok() {
            hit = true;
            break;
        }
        let _ = sel.clear();
    }
    if !hit {
        return None;
    }

    let count = sel.count().unwrap_or(0);
    let mut out = Vec::new();
    for i in 1..=count {
        let value = sel.item(i).and_then(|item| item.value());
        if let Ok(obj) = value {
            let mut name = name_of(&obj);
            if name.is_empty() {
                name = "约束".to_string();
            }
            out.push((obj.com().clone(), name));
        }
    }
    let _ = sel.clear();
    Some(out)
}

fn collect_constraints(sel: &Selection, part: &Part) -> Vec<(ComObj, String)> {
    if let Some(items) = search_items(sel, &PART_CONSTRAINT_SEARCHES) {
        return items;
    }
    // Search 不可用（CGR/受限）时退回全量枚举；枚举无法按可见性过滤，但重复隐藏幂等
    child_items(part, "Constraints")
        .into_iter()
        .map(|c| {
            let n = name_of(&c);
            (
                c,
                if n.is_empty() {
                    "约束".to_string()
                } else {
                    n
                },
            )
        })
        .collect()
}

fn collect_product_constraints(sel: &Selection) -> Vec<(ComObj, String)> {
    search_items(sel, &PRODUCT_CONSTRAINT_SEARCHES).unwrap_or_default()
}

/// 手动枚举 Part 全部内容到选择集（Search 不可用时 show-named/hide-named 的兜底）。
fn manual_enum_select(sel: &Selection, part: &Part) -> usize {
    let mut count = 0;
    let bodies = child_items(part, "Bodies");
    for body in &bodies {
        if sel.add(body).is_ok() {
            count += 1;
        }
        for shape in child_items(body, "Shapes") {
            if sel.add(&shape).is_ok() {
                count += 1;
            }
        }
        for sketch in child_items(body, "Sketches") {
            if sel.add(&sketch).is_ok() {
                count += 1;
            }
        }
    }
    for hb in child_items(part, "HybridBodies") {
        if sel.add(&hb).is_ok() {
            count += 1;
        }
        for shape in child_items(&hb, "HybridShapes") {
            if sel.add(&shape).is_ok() {
                count += 1;
            }
        }
    }
    for axis in child_items(part, "AxisSystems") {
        if sel.add(&axis).is_ok() {
            count += 1;
        }
    }
    for plane in origin_planes(part) {
        if sel.add(&plane).is_ok() {
            count += 1;
        }
    }
    count
}

/// Part 全部元素（含 GS 内的点线面），供按名字匹配的动作使用。
fn collect_all_items(sel: &Selection, part: &Part) -> Vec<(ComObj, String)> {
    let _ = sel.clear();
    let _ = sel.search("CatPrtSearch.*,All");
    let count = sel.count().unwrap_or(0);
    if count == 0 {
        let _ = sel.clear();
        manual_enum_select(sel, part);
    }

    let n = sel.count().unwrap_or(0);
    let mut items = Vec::new();
    for i in 1..=n {
        let value = sel.item(i).and_then(|item| item.value());
        if let Ok(obj) = value {
            let name = name_of(&obj);
            items.push((obj.com().clone(), name));
        }
    }
    let _ = sel.clear();
    items
}

/// 查找 HybridShape 所在的 Geometrical Set 名称；不在 GS 中返回 None。
fn parent_gs_name(obj: &ComObj) -> Option<String> {
    let parent = obj.get("Parent").and_then(|v| v.as_obj()).ok()?;
    let pname = name_of(&parent);
    if pname == "HybridBodies" {
        return Some(name_of(obj));
    }
    if pname == "HybridShapes" {
        if let Ok(gs) = parent.get("Parent").and_then(|v| v.as_obj()) {
            let n = name_of(&gs);
            if !n.is_empty() {
                return Some(n);
            }
        }
    }
    None
}

// ── 各动作 ───────────────────────────────────────────────────────────

fn collect_part_containers(part: &Part) -> (Vec<ComObj>, Vec<ComObj>, Vec<ComObj>, Vec<ComObj>) {
    (
        child_items(part, "Bodies"),
        child_items(part, "HybridBodies"),
        child_items(part, "AxisSystems"),
        origin_planes(part),
    )
}

fn action_show_body(sel: &Selection, part: &Part) -> ActionResult {
    let (bodies, gs_list, axes, refs) = collect_part_containers(part);

    let body_items: Vec<(ComObj, String)> = bodies
        .iter()
        .map(|b| (b.clone(), format!("[Body] {}", name_of(b))))
        .collect();
    let mut hide: Vec<ComObj> = Vec::new();
    let mut hidden_names: Vec<String> = Vec::new();
    for g in &gs_list {
        hide.push(g.clone());
        hidden_names.push(name_of(g));
    }
    for a in &axes {
        hide.push(a.clone());
        hidden_names.push(name_of(a));
    }
    for (r, label) in ref_items(&refs) {
        hide.push(r);
        hidden_names.push(label);
    }
    for (c, n) in collect_constraints(sel, part) {
        hide.push(c);
        hidden_names.push(format!("[约束] {n}"));
    }
    let shown: Vec<ComObj> = body_items.iter().map(|(b, _)| b.clone()).collect();
    let shown_names: Vec<String> = body_items.iter().map(|(_, n)| n.clone()).collect();

    apply_show(sel, &hide, 1); // 隐藏 GS/轴系/参考面/约束
    apply_show(sel, &shown, 0); // 显示所有 Body

    ActionResult {
        shown: shown_names.len(),
        hidden: hidden_names.len(),
        shown_names,
        hidden_names,
    }
}

fn action_show_csys(sel: &Selection, part: &Part) -> ActionResult {
    let (bodies, gs_list, axes, refs) = collect_part_containers(part);

    let body_items: Vec<(ComObj, String)> = bodies
        .iter()
        .map(|b| (b.clone(), format!("[Body] {}", name_of(b))))
        .collect();
    let ax_items: Vec<(ComObj, String)> = axes
        .iter()
        .map(|a| (a.clone(), format!("[轴系] {}", name_of(a))))
        .collect();

    let mut hide: Vec<ComObj> = Vec::new();
    let mut hidden_names: Vec<String> = Vec::new();
    for g in &gs_list {
        hide.push(g.clone());
        hidden_names.push(name_of(g));
    }
    for (r, label) in ref_items(&refs) {
        hide.push(r);
        hidden_names.push(label);
    }
    for (c, n) in collect_constraints(sel, part) {
        hide.push(c);
        hidden_names.push(format!("[约束] {n}"));
    }

    let mut shown: Vec<ComObj> = Vec::new();
    let mut shown_names: Vec<String> = Vec::new();
    for (b, n) in body_items {
        shown.push(b);
        shown_names.push(n);
    }
    for (a, n) in ax_items {
        shown.push(a);
        shown_names.push(n);
    }

    apply_show(sel, &hide, 1);
    apply_show(sel, &shown, 0);

    ActionResult {
        shown: shown_names.len(),
        hidden: hidden_names.len(),
        shown_names,
        hidden_names,
    }
}

fn action_hide_all(sel: &Selection, part: &Part) -> ActionResult {
    let (bodies, gs_list, axes, refs) = collect_part_containers(part);

    let mut items: Vec<ComObj> = Vec::new();
    let mut names: Vec<String> = Vec::new();
    for (b, label) in bodies
        .iter()
        .map(|b| (b.clone(), format!("[Body] {}", name_of(b))))
    {
        items.push(b);
        names.push(label);
    }
    for g in &gs_list {
        items.push(g.clone());
        names.push(name_of(g));
    }
    for a in &axes {
        items.push(a.clone());
        names.push(name_of(a));
    }
    for (r, label) in ref_items(&refs) {
        items.push(r);
        names.push(label);
    }
    for (c, n) in collect_constraints(sel, part) {
        items.push(c);
        names.push(format!("[约束] {n}"));
    }

    apply_show(sel, &items, 1);

    ActionResult {
        shown: 0,
        hidden: names.len(),
        shown_names: Vec::new(),
        hidden_names: names,
    }
}

fn action_show_named(sel: &Selection, part: &Part, hide_mode: bool) -> ActionResult {
    let all = collect_all_items(sel, part);
    if all.is_empty() {
        return ActionResult {
            shown: 0,
            hidden: 0,
            shown_names: vec![],
            hidden_names: vec![],
        };
    }

    let mut matched: Vec<(ComObj, String)> = Vec::new();
    let mut parent_gs: Vec<String> = Vec::new();
    for (obj, name) in &all {
        let lower = name.to_lowercase();
        if KEYWORDS.iter().any(|k| lower.contains(k)) {
            matched.push((obj.clone(), name.clone()));
            if let Some(gs) = parent_gs_name(obj) {
                if !gs.is_empty() && !parent_gs.contains(&gs) {
                    parent_gs.push(gs);
                }
            }
        }
    }

    // 显示模式下把上级 Geometrical Set 也纳入（GS 隐藏时子元素不可见）
    if !hide_mode && !parent_gs.is_empty() {
        let matched_names: Vec<String> = matched.iter().map(|(_, n)| n.clone()).collect();
        for (obj, name) in &all {
            if parent_gs.contains(name) && !matched_names.contains(name) {
                matched.push((obj.clone(), name.clone()));
            }
        }
    }

    let targets: Vec<ComObj> = matched.iter().map(|(o, _)| o.clone()).collect();
    let names: Vec<String> = matched.iter().map(|(_, n)| n.clone()).collect();
    if hide_mode {
        apply_show(sel, &targets, 1);
        ActionResult {
            shown: 0,
            hidden: names.len(),
            shown_names: vec![],
            hidden_names: names,
        }
    } else {
        apply_show(sel, &targets, 0);
        ActionResult {
            shown: names.len(),
            hidden: 0,
            shown_names: names,
            hidden_names: vec![],
        }
    }
}

fn apply_action(action: Action, sel: &Selection, part: &Part) -> ActionResult {
    match action {
        Action::ShowBody => action_show_body(sel, part),
        Action::ShowCsys => action_show_csys(sel, part),
        Action::HideAll => action_hide_all(sel, part),
        Action::ShowNamed => action_show_named(sel, part, false),
        Action::HideNamed => action_show_named(sel, part, true),
    }
}

// ── 设计模式 / 终端节点 ───────────────────────────────────────────────

/// 强制设计模式（CatWorkModeType: DESIGN_MODE=2）。可视化模式(CGR)下几何未加载，
/// 显隐/选择会报错；切 DESIGN 后几何自动加载。
fn ensure_design_mode(doc: &Document) {
    if let Ok(product) = doc.product_document().product() {
        let _ = product.apply_work_mode(2);
    }
}

/// 激活终端节点（CATIA 的 Activate Terminal Node）。
fn activate_terminal_node(doc: &Document) {
    if let Ok(product) = doc.product_document().product() {
        let _ = product.activate_default_shape();
    }
}

/// 对单个 Part 文档执行动作：先切设计模式；报错则激活终端节点后重试一次。
fn apply_to_doc(action: Action, doc: &Document) -> (Option<ActionResult>, Option<String>) {
    ensure_design_mode(doc);
    let attempt = || -> ComResult<ActionResult> {
        let sel = doc.selection()?;
        let part = doc.part_document().part()?;
        Ok(apply_action(action, &sel, &part))
    };

    match attempt() {
        Ok(result) => (Some(result), None),
        Err(first) => {
            activate_terminal_node(doc);
            match attempt() {
                Ok(result) => (
                    Some(result),
                    Some(format!("首次报错: {first}(激活终端节点后重试成功)")),
                ),
                Err(second) => (
                    None,
                    Some(format!("{first};激活终端节点后重试仍失败: {second}")),
                ),
            }
        }
    }
}

fn update_part_quiet(doc: &Document) -> Option<String> {
    let part = doc.part_document().part().ok()?;
    match part.update() {
        Ok(()) => None,
        Err(e) => Some(format!("Update 失败: {e}")),
    }
}

// ── 装配体 ───────────────────────────────────────────────────────────

/// 递归遍历 Product 树，收集所有 CATPart 文档（按路径去重）。
fn collect_product_part_docs(root: &Product, progress: Progress<'_>) -> Vec<(Document, String)> {
    fn walk(
        prod: &Product,
        out: &mut Vec<(Document, String)>,
        seen: &mut Vec<String>,
        progress: Progress<'_>,
    ) {
        let Ok(products) = prod.products() else {
            return;
        };
        let Ok(count) = products.count() else {
            return;
        };
        for i in 1..=count {
            let Ok(child) = products.item(i) else {
                continue;
            };
            let child_name = {
                let n = name_of(&child);
                if n.is_empty() {
                    format!("#{i}")
                } else {
                    n
                }
            };

            // 可视化模式(CGR)实例先切设计模式，否则后续操作报错
            let _ = child.apply_work_mode(2);

            let ref_doc = child
                .reference_product()
                .and_then(|rp| rp.parent())
                .map(Document::new);

            match ref_doc {
                Ok(doc) => {
                    let doc_name = doc.name().unwrap_or_default();
                    let lower = doc_name.to_lowercase();
                    if lower.ends_with(".catpart") {
                        let full = doc.full_name().unwrap_or_default();
                        let key = if full.is_empty() {
                            doc_name.clone()
                        } else {
                            full
                        };
                        if !seen.contains(&key) {
                            seen.push(key);
                            out.push((doc, doc_name));
                        }
                    } else if lower.ends_with(".catproduct") {
                        walk(&child, out, seen, progress);
                    } else {
                        progress(&format!(
                            "  [跳过] {child_name}: 未找到其 CATPart 文档(可能未加载或文件缺失)"
                        ));
                    }
                }
                Err(e) => progress(&format!("  [跳过] {child_name}: {e}")),
            }
        }
    }

    let mut out: Vec<(Document, String)> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    walk(root, &mut out, &mut seen, progress);
    out
}

fn hide_product_constraints(doc: &Document, progress: Progress<'_>) -> Vec<String> {
    let Ok(sel) = doc.selection() else {
        return Vec::new();
    };
    let items = collect_product_constraints(&sel);
    if items.is_empty() {
        return Vec::new();
    }
    let targets: Vec<ComObj> = items.iter().map(|(o, _)| o.clone()).collect();
    apply_show(&sel, &targets, 1);
    let names: Vec<String> = items.iter().map(|(_, n)| n.clone()).collect();
    let preview: Vec<String> = names.iter().take(8).cloned().collect();
    progress(&format!(
        "  ✅ [装配约束] 隐藏 {} 个: {}{}",
        names.len(),
        preview.join(", "),
        if names.len() > 8 { " ..." } else { "" }
    ));
    names
}

fn run_product_mode(
    doc: &Document,
    action: Action,
    dry_run: bool,
    progress: Progress<'_>,
) -> Value {
    progress("[装配] 检测到 .CATProduct,依次对其下所有 Part 执行 ...");
    let Ok(root) = doc.product_document().product() else {
        return json!({"success": false, "message": "无法读取装配体根节点"});
    };
    let parts = collect_product_part_docs(&root, progress);
    let total = parts.len();
    if total == 0 {
        return json!({
            "success": false,
            "message": "Product 下未找到可处理的 CATPart(零件可能未加载为设计模式或文件缺失)"
        });
    }
    let asm_cst = action.hides_product_constraints();

    if dry_run {
        for (i, (_, n)) in parts.iter().enumerate() {
            progress(&format!("  ({}/{total}) {n}", i + 1));
        }
        let n_cst = if asm_cst {
            doc.selection()
                .ok()
                .map(|sel| collect_product_constraints(&sel).len())
                .unwrap_or(0)
        } else {
            0
        };
        let extra = if asm_cst {
            format!("; 装配约束 {n_cst} 个")
        } else {
            String::new()
        };
        return json!({
            "success": true,
            "message": format!("[DRY RUN] {}: 装配体下共 {total} 个 Part{extra}", action.label()),
            "data": {
                "action": action_id(action),
                "mode": "product",
                "total": total,
                "success_count": total,
                "assembly_constraints": n_cst,
                "items": parts.iter().map(|(_, n)| json!({"name": n, "success": true})).collect::<Vec<_>>(),
            }
        });
    }

    let mut items: Vec<Value> = Vec::new();
    let mut shown_names: Vec<String> = Vec::new();
    let mut hidden_names: Vec<String> = Vec::new();
    let mut total_shown = 0usize;
    let mut total_hidden = 0usize;

    for (idx, (pdoc, pname)) in parts.iter().enumerate() {
        let disp = pname
            .to_lowercase()
            .strip_suffix(".catpart")
            .map(|s| pname[..s.len()].to_string())
            .unwrap_or_else(|| pname.clone());
        progress(&format!("[装配] ({}/{total}) {disp} ...", idx + 1));

        let (result, note) = apply_to_doc(action, pdoc);
        match result {
            None => {
                let note = note.unwrap_or_else(|| "未知错误".to_string());
                progress(&format!("  ❌ {note}"));
                items.push(json!({"name": disp, "success": false, "error": note}));
            }
            Some(r) => {
                let update_err = update_part_quiet(pdoc);
                let mut item =
                    json!({"name": disp, "success": true, "shown": r.shown, "hidden": r.hidden});
                if let Some(u) = &update_err {
                    item["note"] = json!(u);
                }
                shown_names.extend(r.shown_names.iter().map(|n| format!("[{disp}] {n}")));
                hidden_names.extend(r.hidden_names.iter().map(|n| format!("[{disp}] {n}")));
                total_shown += r.shown;
                total_hidden += r.hidden;
                let tail: Vec<String> = [note.unwrap_or_default(), update_err.unwrap_or_default()]
                    .into_iter()
                    .filter(|s| !s.is_empty())
                    .collect();
                progress(&format!(
                    "  ✅ 显示 {}, 隐藏 {}{}",
                    r.shown,
                    r.hidden,
                    if tail.is_empty() {
                        String::new()
                    } else {
                        format!("; {}", tail.join("; "))
                    }
                ));
                items.push(item);
            }
        }
    }

    // 恢复 Product 文档激活（过程中切到过各 Part 文档）
    let _ = doc.activate();

    let asm_names = if asm_cst {
        hide_product_constraints(doc, progress)
    } else {
        Vec::new()
    };
    if !asm_names.is_empty() {
        hidden_names.extend(asm_names.iter().map(|n| format!("[装配约束] {n}")));
        total_hidden += asm_names.len();
    }

    let ok = items
        .iter()
        .filter(|it| it.get("success").and_then(|v| v.as_bool()).unwrap_or(false))
        .count();
    let failed = total - ok;
    let mut msg_parts = vec![format!("成功 {ok}/{total} 个 Part")];
    if total_shown > 0 {
        msg_parts.push(format!("显示 {total_shown}"));
    }
    if total_hidden > 0 {
        msg_parts.push(format!("隐藏 {total_hidden}"));
    }
    if !asm_names.is_empty() {
        msg_parts.push(format!("其中装配约束 {} 个", asm_names.len()));
    }
    if failed > 0 {
        msg_parts.push(format!("{failed} 个失败(见日志)"));
    }

    json!({
        "success": ok > 0,
        "message": format!("{}(装配体): {}", action.label(), msg_parts.join("; ")),
        "data": {
            "action": action_id(action),
            "mode": "product",
            "total": total,
            "success_count": ok,
            "shown": total_shown,
            "hidden": total_hidden,
            "assembly_constraints": asm_names.len(),
            "shown_names": shown_names,
            "hidden_names": hidden_names,
            "items": items,
        }
    })
}

fn action_id(action: Action) -> &'static str {
    match action {
        Action::ShowBody => "show-body",
        Action::ShowCsys => "show-csys",
        Action::HideAll => "hide-all",
        Action::ShowNamed => "show-named",
        Action::HideNamed => "hide-named",
    }
}

// ── 入口 ─────────────────────────────────────────────────────────────

/// 工具主体。**必须在 STA 线程上执行。**
pub fn run(params: &Value, progress: Progress<'_>) -> ComResult<Value> {
    let dry_run = params
        .get("dry_run")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let action_str = params
        .get("action")
        .and_then(|v| v.as_str())
        .unwrap_or("show-body");
    let Some(action) = Action::parse(action_str) else {
        return Ok(json!({
            "success": false,
            "message": format!("未知操作: {action_str},可选: show-body/show-csys/hide-all/show-named/hide-named")
        }));
    };

    let app = attach_with_guard(progress)?;
    if app.document_count()? == 0 {
        return Ok(
            json!({"success": false, "message": "CATIA 没有打开的文档,请先打开 CATPart 或 CATProduct"}),
        );
    }

    let doc = app.active_document()?;
    let doc_name = name_of(&doc);
    let lower = doc_name.to_lowercase();

    if lower.ends_with(".catdrawing") {
        return Ok(json!({
            "success": false,
            "message": format!("当前是工程图({doc_name}): 本期 Rust 版尚未移植同名零件/装配自动切换, 请先切换到零件或装配后重试")
        }));
    }

    if lower.ends_with(".catproduct") {
        return Ok(run_product_mode(&doc, action, dry_run, progress));
    }

    if !lower.ends_with(".catpart") {
        return Ok(
            json!({"success": false, "message": format!("请打开 CATPart 或 CATProduct(当前: {doc_name})")}),
        );
    }

    let part = doc.part_document().part()?;

    if dry_run {
        let (bodies, gs_list, axes, refs) = collect_part_containers(&part);
        let Some(sel) = doc.selection().ok() else {
            return Ok(json!({"success": false, "message": "无法获取选择集"}));
        };
        let csts = collect_constraints(&sel, &part);
        return Ok(json!({
            "success": true,
            "message": format!(
                "[DRY RUN] {}: Body {} / 几何图形集 {} / 轴系 {} / 参考面 {} / 约束 {}",
                action.label(), bodies.len(), gs_list.len(), axes.len(), refs.len(), csts.len()
            ),
            "data": {
                "action": action_id(action),
                "mode": "part",
                "bodies": bodies.len(),
                "geometrical_sets": gs_list.len(),
                "axis_systems": axes.len(),
                "references": refs.len(),
                "constraints": csts.len(),
            }
        }));
    }

    let (result, note) = apply_to_doc(action, &doc);
    let Some(r) = result else {
        return Ok(json!({
            "success": false,
            "message": format!("操作失败: {}", note.unwrap_or_default())
        }));
    };

    let update_err = update_part_quiet(&doc);
    let cst_count = r
        .hidden_names
        .iter()
        .filter(|n| n.starts_with("[约束]"))
        .count();

    let mut msg_parts: Vec<String> = Vec::new();
    if r.shown > 0 {
        msg_parts.push(format!("显示 {} 个", r.shown));
    }
    if r.hidden > 0 {
        msg_parts.push(format!("隐藏 {} 个", r.hidden));
    }
    if msg_parts.is_empty() {
        msg_parts.push("无变化".to_string());
    }
    for extra in [note, update_err].into_iter().flatten() {
        msg_parts.push(extra);
    }

    Ok(json!({
        "success": true,
        "message": format!("{}: {}", action.label(), msg_parts.join("; ")),
        "data": {
            "action": action_id(action),
            "mode": "part",
            "shown": r.shown,
            "hidden": r.hidden,
            "constraints": cst_count,
            "shown_names": r.shown_names,
            "hidden_names": r.hidden_names,
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::Action;

    #[test]
    fn parses_all_actions() {
        for (s, expected) in [
            ("show-body", Action::ShowBody),
            ("show-csys", Action::ShowCsys),
            ("hide-all", Action::HideAll),
            ("show-named", Action::ShowNamed),
            ("hide-named", Action::HideNamed),
        ] {
            assert_eq!(Action::parse(s), Some(expected));
        }
        assert_eq!(Action::parse("nope"), None);
    }

    #[test]
    fn product_constraints_only_for_three_actions() {
        assert!(Action::ShowBody.hides_product_constraints());
        assert!(Action::ShowCsys.hides_product_constraints());
        assert!(Action::HideAll.hides_product_constraints());
        assert!(!Action::ShowNamed.hides_product_constraints());
        assert!(!Action::HideNamed.hides_product_constraints());
    }
}
