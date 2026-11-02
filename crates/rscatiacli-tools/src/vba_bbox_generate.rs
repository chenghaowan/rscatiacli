//! vba-bbox-generate：按已验证的 VBA（VBAProject1.catvba / Module1.bas）方法
//! 生成包围盒几何图形集并读出精确长/宽/高。
//!
//! 对应 ltl-manage resources/sidecars/product_analysis/vba_bbox_generate.py。
//! 生成并保留整套几何：body `Boundingbox.<n>` + 几何集 "definition_points"
//! （6 个极值点/上下平面/引导线）+ sketch1/2/3（矩形+距离约束）+ 扫掠实体 +
//! `Bbox_dx/dy/dz` 参数 + 描述属性 x{}y{}z{}。
//!
//! 移植方法而非照抄录制宏：极值点 → sketch 矩形+约束 → 扫掠 → 取尺寸。
//! 轴系：复用已有 tooling axis；否则 Z=draftline / X=halfline / Y=Z×X 新建，
//! 原点 = 全部实体 body 合并质心（体积加权）。
//!
//! 命令类操作（Delete/Search）落在**激活编辑器**的选中集上：零件在装配中打开
//! 时零件文档不是激活文档，其 Selection 连 Clear() 都会 RPC_E_SERVERFAULT，
//! Delete 报 E_FAIL —— 此时借用激活装配的选中集（用完还原）。
//! 多 body：每 body 一个 extract，6 向极值取全局最值（跳过 Boundingbox/RuleBbox
//! 临时体，防自指污染）。体积 = 各 body 真实体积之和（m³→mm³）。

use serde_json::{json, Value};

use rscatiacli::dispatch::ComObj;
use rscatiacli::objects::{
    Constraints, Entity, HybridBody, PartDocument, SPAWorkbench, Selection, SystemService,
};
use rscatiacli::{attach_with_guard, ComCollection, ComObject, ComResult, Progress};

use crate::naming_rules::refine_part_number;
use crate::surface_common::com_type_name;

pub(crate) type Vec3 = [f64; 3];

/// 临时几何名关键字（清理与跳过的依据）。
const TEMP_NAMES: [&str; 2] = ["boundingbox", "rulebbox"];
/// 无链接材料读取的 VBA 模板（out 参数无法晚绑定取回，与 add_material 同通道）。
const VBA_GET_MATERIAL: &str = r#"Public Function get_material(kind, manager, target)
    Dim material
    If kind = "part" Then
        manager.GetMaterialOnPart target, material
    Else
        manager.GetMaterialOnBody target, material
    End If
    Set get_material = material
End Function
"#;

fn is_temp_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    TEMP_NAMES.iter().any(|k| lower.contains(k))
}

pub(crate) fn dot(a: Vec3, b: Vec3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub(crate) fn cross(a: Vec3, b: Vec3) -> Vec3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

pub(crate) fn norm(v: Vec3) -> Vec3 {
    let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if n < 1e-12 {
        return [0.0; 3];
    }
    [v[0] / n, v[1] / n, v[2] / n]
}

// ── 目标解析：激活文档为 Product 时解析"当前激活的 part" ────────────────

fn is_part_document_com(obj: &ComObj) -> bool {
    obj.get("Part").is_ok()
}

fn parent_walk_to_part_doc(start: &ComObj, hops: usize) -> Option<ComObj> {
    let mut cur = start.clone();
    for _ in 0..hops {
        if is_part_document_com(&cur) {
            return Some(cur);
        }
        cur = cur.get("Parent").and_then(|p| p.as_obj()).ok()?;
    }
    None
}

/// 产品选中集里恰好 1 个元素 → 沿 Parent 链上溯到所属 PartDocument。
fn part_doc_from_selection(product_doc: &rscatiacli::objects::Document) -> Option<PartDocument> {
    let sel = product_doc.selection().ok()?;
    if sel.count().ok()? != 1 {
        return None;
    }
    let value = sel.item(1).ok()?.value().ok()?;
    let vcom = value.com().clone();
    // 选中零件部件: ReferenceProduct.Parent 即宿主文档
    if let Ok(cand) = vcom.get("ReferenceProduct").and_then(|r| r.as_obj()) {
        if let Ok(host) = cand.get("Parent").and_then(|p| p.as_obj()) {
            if is_part_document_com(&host) {
                return Some(PartDocument::new(host));
            }
            // 子装配(宿主为 ProductDocument)不算
            return None;
        }
    }
    // 选中零件内几何: Parent 链上溯
    parent_walk_to_part_doc(&vcom, 30).map(PartDocument::new)
}

/// 零件文档匹配键: 文档名去扩展名 + 主 Product 的 PartNumber(大写)。
fn part_doc_keys(doc: &PartDocument) -> Vec<String> {
    let mut keys = Vec::new();
    if let Ok(name) = doc.name() {
        let stem = name.rsplit_once('.').map(|(b, _)| b).unwrap_or(&name);
        if !stem.is_empty() {
            keys.push(stem.to_uppercase());
        }
    }
    if let Ok(pn) = doc
        .product_document()
        .product()
        .and_then(|p| p.part_number())
    {
        let pn = pn.trim().to_uppercase();
        if !pn.is_empty() {
            keys.push(pn);
        }
    }
    keys
}

/// 产品树下所有零件的 PartNumber 与参考产品名(大写)。
fn collect_product_part_keys(root: &rscatiacli::objects::Product) -> Vec<String> {
    fn walk(prod: &rscatiacli::objects::Product, keys: &mut Vec<String>) {
        let Ok(children) = prod.products() else {
            return;
        };
        let Ok(items) = children.items() else {
            return;
        };
        for ch in &items {
            if let Ok(pn) = ch.part_number() {
                let pn = pn.trim().to_uppercase();
                if !pn.is_empty() {
                    keys.push(pn);
                }
            }
            if let Ok(name) = ch.reference_product().and_then(|r| r.name()) {
                let name = name.trim().to_uppercase();
                if !name.is_empty() {
                    keys.push(name);
                }
            }
            walk(ch, keys);
        }
    }
    let mut keys = Vec::new();
    walk(root, &mut keys);
    keys
}

/// 解析边界框目标零件文档。返回 (PartDocument, note)；失败 Err(错误消息)。
fn resolve_part_document(
    app: &rscatiacli::objects::Application,
    doc: &rscatiacli::objects::Document,
) -> Result<(PartDocument, String), String> {
    let doc_name = doc.name().unwrap_or_default();
    let lower = doc_name.to_lowercase();
    if lower.ends_with(".catpart") {
        return Ok((doc.part_document(), String::new()));
    }
    if !lower.ends_with(".catproduct") {
        return Err(format!("期望 PartDocument, 得到 {doc_name}"));
    }

    if let Some(pd) = part_doc_from_selection(doc) {
        return Ok((pd, "目标零件取自产品中选中的元素".to_string()));
    }

    // 会话中已打开的全部 PartDocument
    let mut open_parts: Vec<PartDocument> = Vec::new();
    if let Ok(docs) = app.documents() {
        if let Ok(items) = docs.items() {
            for d in &items {
                if d.name()
                    .map(|n| n.to_lowercase().ends_with(".catpart"))
                    .unwrap_or(false)
                {
                    open_parts.push(d.part_document());
                }
            }
        }
    }
    if open_parts.is_empty() {
        return Err(
            "激活文档是产品(CATProduct), 未能确定要测量的零件: 请在产品树中单击选中目标零件后重试, \
             或先双击打开该零件; 若要测量产品下全部零件请用「批量分析」功能"
                .to_string(),
        );
    }
    if open_parts.len() == 1 {
        let note = format!(
            "目标零件取自已打开的零件: {}",
            open_parts[0].name().unwrap_or_default()
        );
        return Ok((open_parts.remove(0), note));
    }

    // 多个时取属于该产品结构的唯一一个
    let keys = doc
        .product_document()
        .product()
        .map(|p| collect_product_part_keys(&p))
        .unwrap_or_default();
    let members: Vec<PartDocument> = open_parts
        .iter()
        .filter(|p| {
            let ks = part_doc_keys(p);
            ks.iter().any(|k| keys.contains(k))
        })
        .cloned()
        .collect();
    if members.len() == 1 {
        let note = format!(
            "目标零件取自已打开的零件: {}",
            members[0].name().unwrap_or_default()
        );
        return Ok((members.into_iter().next().unwrap(), note));
    }

    let names = open_parts
        .iter()
        .filter_map(|p| p.name().ok())
        .collect::<Vec<_>>()
        .join(", ");
    Err(format!(
        "当前打开了多个零件({names}), 无法确定要测量的零件: 请在产品树中单击选中目标零件后重试, 或关闭多余零件窗口"
    ))
}

// ── 命令类操作的选中集（激活编辑器规则） ────────────────────────────────

/// 激活产品的组件树里是否含目标零件文档。
fn product_contains_part(
    product_doc: &rscatiacli::objects::Document,
    part_doc: &PartDocument,
) -> bool {
    let target = part_doc.name().unwrap_or_default().trim().to_uppercase();
    if target.is_empty() {
        return false;
    }
    fn walk(prod: &rscatiacli::objects::Product, target: &str, depth: usize, found: &mut bool) {
        if *found || depth > 12 {
            return;
        }
        let Ok(children) = prod.products() else {
            return;
        };
        let Ok(items) = children.items() else {
            return;
        };
        for ch in &items {
            if let Ok(host) = ch
                .reference_product()
                .and_then(|r| r.parent())
                .and_then(|p| p.get("Name").and_then(|n| n.as_string()))
            {
                if host.trim().to_uppercase() == target {
                    *found = true;
                    return;
                }
            }
            walk(ch, target, depth + 1, found);
        }
    }
    let mut found = false;
    if let Ok(prd) = product_doc.product_document().product() {
        walk(&prd, &target, 0, &mut found);
    }
    found
}

/// 命令类操作(Sel.Delete/Sel.Search)真正落地的选中集。返回 (选中集, 是否借用)。
pub(crate) fn command_selection(
    app: &rscatiacli::objects::Application,
    part_doc: &PartDocument,
) -> ComResult<(Selection, bool)> {
    if let Ok(active) = app.active_document() {
        let same = active.name().unwrap_or_default().trim().to_uppercase()
            == part_doc.name().unwrap_or_default().trim().to_uppercase();
        if !same && product_contains_part(&active, part_doc) {
            if let Ok(sel) = active.selection() {
                return Ok((sel, true));
            }
        }
    }
    Ok((part_doc.selection()?, false))
}

/// 记下选中集当前内容（借用前保存）。
pub(crate) fn snapshot_selection(sel: &Selection) -> Vec<ComObj> {
    let mut items = Vec::new();
    if let Ok(n) = sel.count() {
        for i in 1..=n {
            if let Ok(v) = sel.item(i).and_then(|it| it.value()) {
                items.push(v.com().clone());
            }
        }
    }
    items
}

/// 把选中集还原为快照内容（元素已删等异常静默忽略）。
pub(crate) fn restore_selection(sel: &Selection, items: &[ComObj]) {
    let _ = sel.clear();
    for it in items {
        let _ = sel.add(it);
    }
}

// ── 直线查找（draftline / halfline） ─────────────────────────────────

pub(crate) struct LineInfo {
    pub name: String,
    #[allow(dead_code)]
    pub start_mm: Option<Vec3>,
    #[allow(dead_code)]
    pub end_mm: Option<Vec3>,
    pub direction: Vec3,
}

/// 直线方向（GetDirection 是出参数组，VBA 中转；非直线会失败）。
fn line_direction(obj: &ComObj, ss: &SystemService) -> ComResult<Vec3> {
    let function = "rscatiacli_line_dir";
    let code = format!(
        r#"
        Public Function {function}(oLine)
            Dim aReturn(2) as Variant
            oLine.GetDirection aReturn
            {function} = aReturn
        End Function
        "#
    );
    let values = ss
        .evaluate(
            &code,
            0,
            function,
            &[rscatiacli::dispatch::Arg::Obj(obj.clone())],
        )?
        .as_f64_array()?;
    if values.len() < 3 {
        return Err(rscatiacli::ComError::runtime(format!(
            "GetDirection 返回了 {} 个分量，期望 3",
            values.len()
        )));
    }
    Ok([values[0], values[1], values[2]])
}

/// evaluate 参数的小包装（ComObj → Arg::Obj）。
struct ComObjArg<'a>(&'a ComObj);
impl From<ComObjArg<'_>> for rscatiacli::dispatch::Arg {
    fn from(c: ComObjArg<'_>) -> Self {
        rscatiacli::dispatch::Arg::Obj(c.0.clone())
    }
}

/// 直线端点（GetPointsOnCurve 9 分量；只影响日志）。
fn line_endpoints(
    part: &rscatiacli::objects::Part,
    spa: &SPAWorkbench,
    ss: &SystemService,
    shape: &ComObj,
) -> Option<(Vec3, Vec3)> {
    let r = part.create_reference_from_object(shape).ok()?;
    let m = spa.get_measurable(&r).ok()?;
    let pts = m.get_points_on_curve(ss).ok()?;
    if pts.len() < 6 {
        return None;
    }
    Some(([pts[0], pts[1], pts[2]], [pts[3], pts[4], pts[5]]))
}

/// 直接遍历几何图形集/Body 按名字找直线（不依赖激活编辑器）。
fn find_line_by_name_walk(
    prt_doc: &PartDocument,
    keyword: &str,
    ss: &SystemService,
    spa: &SPAWorkbench,
) -> Result<LineInfo, String> {
    let part = prt_doc
        .part()
        .map_err(|e| format!("find line '{keyword}' failed: {e}"))?;
    let mut hit: Option<(String, ComObj, Vec3)> = None;

    fn walk_shapes(
        shapes: &rscatiacli::objects::Entities,
        keyword: &str,
        ss: &SystemService,
        hit: &mut Option<(String, ComObj, Vec3)>,
    ) {
        if hit.is_some() {
            return;
        }
        let Ok(count) = shapes.count() else {
            return;
        };
        for k in 1..=count {
            let Ok(sh) = shapes.item(k) else {
                continue;
            };
            let Ok(name) = sh.name() else {
                continue;
            };
            if !name.to_lowercase().contains(keyword) {
                continue;
            }
            match line_direction(sh.com(), ss) {
                Ok(direction) => {
                    *hit = Some((name, sh.com().clone(), direction));
                    return;
                }
                Err(_) => continue, // 非直线：跳过继续找
            }
        }
    }

    fn walk_bodies(
        hbs: &rscatiacli::objects::HybridBodies,
        keyword: &str,
        ss: &SystemService,
        hit: &mut Option<(String, ComObj, Vec3)>,
        depth: usize,
    ) {
        if hit.is_some() || depth > 8 {
            return;
        }
        let Ok(items) = hbs.items() else {
            return;
        };
        for hb in &items {
            if let Ok(shapes) = hb.hybrid_shapes() {
                walk_shapes(&shapes, keyword, ss, hit);
            }
            if hit.is_some() {
                return;
            }
            if let Ok(sub) = hb.hybrid_bodies() {
                walk_bodies(&sub, keyword, ss, hit, depth + 1);
            }
        }
    }

    // 先 Part 顶层几何图形集递归，再实体 Body 下的几何图形集
    if let Ok(hbs) = part.hybrid_bodies() {
        walk_bodies(&hbs, keyword, ss, &mut hit, 0);
    }
    if hit.is_none() {
        if let Ok(bodies) = part.bodies() {
            for b in bodies.items().unwrap_or_default() {
                if let Ok(hbs) = b.hybrid_bodies() {
                    walk_bodies(&hbs, keyword, ss, &mut hit, 0);
                }
                if hit.is_some() {
                    break;
                }
            }
        }
    }
    let (name, shape, direction) =
        hit.ok_or_else(|| format!("No line with '{keyword}' in name found"))?;
    let (start_mm, end_mm) = line_endpoints(&part, spa, ss, &shape)
        .map(|(s, e)| (Some(s), Some(e)))
        .unwrap_or((None, None));
    let _ = com_type_name(&shape); // 保留类型名探测引用（诊断用）
    Ok(LineInfo {
        name,
        start_mm,
        end_mm,
        direction,
    })
}

/// 找名字含 keyword 的第一条直线：先 Selection.Search，未命中回退按名遍历。
pub(crate) fn find_line_by_keyword(
    prt_doc: &PartDocument,
    keyword: &str,
    skip_search: bool,
    ss: &SystemService,
    spa: &SPAWorkbench,
    progress: Progress<'_>,
) -> Result<LineInfo, String> {
    if skip_search {
        let walk = find_line_by_name_walk(prt_doc, keyword, ss, spa);
        if let Ok(w) = &walk {
            progress(&format!(
                "[line] 装配上下文: 跳过 Search, 按名字遍历找到 {}",
                w.name
            ));
        }
        return walk;
    }
    // Search 路径
    let search_result = (|| -> Result<LineInfo, String> {
        let sel = prt_doc.selection().map_err(|e| e.to_string())?;
        let _ = sel.clear();
        sel.search("CatPrtSearch.Line,All")
            .map_err(|e| format!("find line '{keyword}' failed: {e}"))?;
        let count = sel.count().unwrap_or(0);
        for i in 1..=count {
            let Ok(item) = sel.item(i) else {
                continue;
            };
            let Ok(value) = item.value() else {
                continue;
            };
            let name = value.name().unwrap_or_default();
            if !name.to_lowercase().contains(keyword) {
                continue;
            }
            let direction = line_direction(value.com(), ss)
                .map_err(|e| format!("find line '{keyword}' failed: {e}"))?;
            let _ = sel.clear();
            // 端点读回（rule_bounding_box 的 JSON 导出需要；失败仅缺日志信息）
            let (start_mm, end_mm) = line_endpoints(
                &prt_doc.part().map_err(|e| e.to_string())?,
                spa,
                ss,
                value.com(),
            )
            .map(|(s, e)| (Some(s), Some(e)))
            .unwrap_or((None, None));
            return Ok(LineInfo {
                name,
                start_mm,
                end_mm,
                direction,
            });
        }
        let _ = sel.clear();
        Err(format!("No line with '{keyword}' in name found"))
    })();
    match search_result {
        Ok(v) => Ok(v),
        Err(search_err) => {
            let fallback = find_line_by_name_walk(prt_doc, keyword, ss, spa);
            match fallback {
                Ok(f) => {
                    progress(&format!(
                        "[line] Search 未命中(零件在装配中打开时该命令返回空), 已按名字遍历找到: {}",
                        f.name
                    ));
                    Ok(f)
                }
                Err(_) => Err(search_err), // 保留原 Search 失败文案
            }
        }
    }
}

// ── tooling axis ─────────────────────────────────────────────────────

pub(crate) struct AxisInfo {
    pub origin_mm: Vec3,
    pub x_axis: Vec3,
    pub y_axis: Vec3,
    pub z_axis: Vec3,
    pub origin_type: i32,
}

/// 查找已有 'tooling axis' 基准坐标系（与投影面积工具共用）。
pub(crate) fn find_tooling_axis(
    part: &rscatiacli::objects::Part,
    ss: &SystemService,
    progress: Progress<'_>,
) -> Option<AxisInfo> {
    let axes = part.axis_systems().ok()?;
    for ax in axes.items().ok()? {
        let name = ax.name().unwrap_or_default();
        if !name.to_lowercase().contains("tooling axis") {
            continue;
        }
        let origin = ax.get_origin(ss).ok()?;
        let x = ax.get_x_axis(ss).ok()?;
        let y = ax.get_y_axis(ss).ok()?;
        let z = ax.get_z_axis(ss).ok()?;
        let ot = ax.origin_type().unwrap_or(0);
        return Some(AxisInfo {
            origin_mm: origin,
            x_axis: x,
            y_axis: y,
            z_axis: z,
            origin_type: ot,
        });
    }
    let _ = progress;
    None
}

/// 创建 'tooling axis' 共享基准坐标系（按坐标去参，固定基准）。
pub(crate) fn build_axis_system(
    part: &rscatiacli::objects::Part,
    x_axis: Vec3,
    y_axis: Vec3,
    z_axis: Vec3,
    origin: Vec3,
) -> ComResult<Entity> {
    let axis_sys = part.axis_systems()?.add()?;
    axis_sys.set_name("tooling axis")?;
    axis_sys.set_origin_type(1)?; // catAxisSystemOriginByCoordinates
    axis_sys.set_x_axis_type(1)?; // catAxisSystemAxisByCoordinates
    axis_sys.set_y_axis_type(1)?;
    axis_sys.set_z_axis_type(1)?;
    axis_sys.put_origin(origin)?;
    axis_sys.put_x_axis(x_axis)?;
    axis_sys.put_y_axis(y_axis)?;
    axis_sys.put_z_axis(z_axis)?;
    let _ = part.update_object(&axis_sys);
    let _ = part.update_object(&axis_sys);
    let _ = part.update_object(&axis_sys);
    axis_sys.set_is_current(true)?;
    let _ = part.update_object(&axis_sys);
    part.update()?;
    Ok(Entity::new(axis_sys.com().clone()))
}

// ── 清理 ─────────────────────────────────────────────────────────────

/// 删除名字含 keywords 的 Bodies / HybridBodies（幂等清理）。
pub(crate) fn cleanup_old(part: &rscatiacli::objects::Part, sel: &Selection, names: &[&str]) {
    let _ = sel.clear();
    if let Ok(bodies) = part.bodies() {
        if let Ok(items) = bodies.items() {
            for b in items.iter().rev() {
                let name = b.name().unwrap_or_default();
                if names
                    .iter()
                    .any(|kw| name.to_lowercase().contains(&kw.to_lowercase()))
                    && sel.add(b).is_ok()
                {
                    let _ = sel.delete();
                }
            }
        }
    }
    if let Ok(hbs) = part.hybrid_bodies() {
        if let Ok(items) = hbs.items() {
            for hb in items.iter().rev() {
                let name = hb.name().unwrap_or_default();
                if names
                    .iter()
                    .any(|kw| name.to_lowercase().contains(&kw.to_lowercase()))
                    && sel.add(hb).is_ok()
                {
                    let _ = sel.delete();
                }
            }
        }
    }
    let _ = sel.clear();
    let _ = part.update();
}

/// 删除名字含 keywords 的轴系。
fn delete_axis_systems(part: &rscatiacli::objects::Part, sel: &Selection, names: &[&str]) {
    if let Ok(axes) = part.axis_systems() {
        if let Ok(items) = axes.items() {
            for ax in items.iter().rev() {
                let name = ax.name().unwrap_or_default();
                if names
                    .iter()
                    .any(|k| name.to_lowercase().contains(&k.to_lowercase()))
                {
                    let _ = sel.clear();
                    if sel.add(ax).is_ok() {
                        let _ = sel.delete();
                    }
                }
            }
        }
    }
    let _ = sel.clear();
    let _ = part.update();
}

/// 删除临时几何(Body/HybridBody)与旧版轴系 + Bbox_ 参数；tooling axis 保留。
fn cleanup_temp(part: &rscatiacli::objects::Part, sel: &Selection, extra_names: &[&str]) {
    let mut body_names: Vec<&str> = vec!["boundingbox", "definition_points"];
    body_names.extend_from_slice(extra_names);
    cleanup_old(part, sel, &body_names);
    // 旧版 Box_Axis_System 轴系（现统一为 tooling axis）
    if let Ok(axes) = part.axis_systems() {
        if let Ok(items) = axes.items() {
            for ax in items.iter().rev() {
                let name = ax.name().unwrap_or_default();
                if name.to_lowercase().contains("box_axis_system") {
                    let _ = sel.clear();
                    if sel.add(ax).is_ok() {
                        let _ = sel.delete();
                    }
                }
            }
        }
    }
    // Bbox_* 参数
    if let Ok(params) = part.parameters() {
        let _ = sel.clear();
        if let Ok(items) = params.items() {
            for p in &items {
                if let Ok(name) = p.name() {
                    if name.contains("Bbox_") {
                        let _ = sel.add(p);
                    }
                }
            }
        }
        if sel.count().unwrap_or(0) > 0 {
            let _ = sel.delete();
        }
        let _ = sel.clear();
    }
    let _ = part.update();
}

/// 对象是否位于本工具生成的临时几何内（Parent 链 8 级内出现关键字）。
fn in_temp_geometry(start: &ComObj) -> bool {
    let mut cur = Some(start.clone());
    for _ in 0..8 {
        let Some(c) = cur else { break };
        let name = c
            .get("Name")
            .and_then(|n| n.as_string())
            .unwrap_or_default();
        if is_temp_name(&name) {
            return true;
        }
        match c.get("Parent").and_then(|p| p.as_obj()) {
            Ok(p) => cur = Some(p),
            Err(_) => break,
        }
    }
    false
}

// ── 核心：按 VBA 方法生成包围盒几何并读出 dx/dy/dz ──────────────────────

struct BboxDims {
    length_mm: f64,
    width_mm: f64,
    height_mm: f64,
}

/// 删除失败/落选的临时特征（尽力而为）。
fn drop_feature(sel: &Selection, feature: Option<&Entity>) {
    let Some(f) = feature else { return };
    let _ = sel.clear();
    if sel.add(f).is_ok() {
        let _ = sel.delete();
    }
    let _ = sel.clear();
}

/// 对每个 body 的 extract 求极值，返回候选 [(投影值, 极值特征, 点特征)]。
#[allow(clippy::too_many_arguments)]
fn extremum_candidates(
    hsf: &rscatiacli::objects::HybridShapeFactory,
    part: &rscatiacli::objects::Part,
    hybrid_body2: &HybridBody,
    extracts: &[Entity],
    direction_feat: &Entity,
    axis_vec: Vec3,
    is_max: bool,
    spa: &SPAWorkbench,
    ss: &SystemService,
    sel: &Selection,
) -> Vec<(f64, Entity, Entity)> {
    let mut out = Vec::new();
    for ex in extracts {
        let mut extr: Option<Entity> = None;
        let mut pt: Option<Entity> = None;
        let attempt = (|| -> ComResult<(Entity, Entity, f64)> {
            let dir_ref = part.create_reference_from_object(direction_feat)?;
            let e = hsf.add_new_extremum(
                &part.create_reference_from_object(ex)?,
                &dir_ref,
                if is_max { 1 } else { 0 },
            )?;
            extr = Some(e.clone());
            hybrid_body2.append_hybrid_shape(&e)?;
            part.set_in_work_object(&e)?;
            part.update()?;
            let ref_extr = part.create_reference_from_object(&e)?;
            let p = hsf.add_new_point_coord_with_reference(0.0, 0.0, 0.0, &ref_extr)?;
            pt = Some(p.clone());
            hybrid_body2.append_hybrid_shape(&p)?;
            part.set_in_work_object(&p)?;
            part.update()?;
            let m = spa.get_measurable(&part.create_reference_from_object(&p)?)?;
            let xyz = match m.get_point(ss) {
                Ok(v) => v,
                Err(_) => m.get_cog(ss)?,
            };
            Ok((e, p, dot(xyz, axis_vec)))
        })();
        match attempt {
            Ok((e, p, proj)) => out.push((proj, e, p)),
            Err(_) => {
                // 该 body 无实体或极值/坐标读取失败: 清掉半成品再跳过
                drop_feature(sel, pt.as_ref());
                drop_feature(sel, extr.as_ref());
            }
        }
    }
    out
}

/// 取所有 body 里的全局极值，返回胜出的 (极值特征, PointCoord 特征)。
#[allow(clippy::too_many_arguments)]
fn global_extremum(
    hsf: &rscatiacli::objects::HybridShapeFactory,
    part: &rscatiacli::objects::Part,
    hybrid_body2: &HybridBody,
    extracts: &[Entity],
    direction_feat: &Entity,
    axis_vec: Vec3,
    is_max: bool,
    name: &str,
    spa: &SPAWorkbench,
    ss: &SystemService,
    sel: &Selection,
) -> ComResult<(Entity, Entity)> {
    let cands = extremum_candidates(
        hsf,
        part,
        hybrid_body2,
        extracts,
        direction_feat,
        axis_vec,
        is_max,
        spa,
        ss,
        sel,
    );
    if cands.is_empty() {
        return Err(rscatiacli::ComError::runtime(format!(
            "极值点测量失败({name}): 所有 body 的 extract/extremum 均未解析"
        )));
    }
    let (bi, best) = cands
        .iter()
        .enumerate()
        .max_by(|a, b| {
            let (av, bv) = (a.1 .0, b.1 .0);
            if is_max {
                av.partial_cmp(&bv).unwrap_or(std::cmp::Ordering::Equal)
            } else {
                bv.partial_cmp(&av).unwrap_or(std::cmp::Ordering::Equal)
            }
        })
        .map(|(i, c)| (i, (c.1.clone(), c.2.clone())))
        .unwrap();
    let _ = bi;
    let (extr, pt) = best;
    let _ = extr.set_name(name);
    let _ = hsf.gsm_visibility(&extr, 0);
    // 删除落选候选（每方向只留 1 个极值点）
    for (_, l_extr, l_pt) in &cands {
        if l_extr.com().same_object(extr.com()) {
            continue;
        }
        drop_feature(sel, Some(l_pt));
        drop_feature(sel, Some(l_extr));
    }
    Ok((extr, pt))
}

/// 按 VBA 原方法生成几何图形集并读出包围盒三向尺寸。
#[allow(clippy::too_many_arguments)]
fn generate_bbox(
    prt_doc: &PartDocument,
    x_axis: Vec3,
    y_axis: Vec3,
    z_axis: Vec3,
    cmd_sel: &Selection,
    ss: &SystemService,
    spa: &SPAWorkbench,
    progress: Progress<'_>,
) -> ComResult<BboxDims> {
    let part = prt_doc.part()?;
    part.update()?;
    let hsf = part.hybrid_shape_factory()?;

    let d1 = hsf.add_new_direction_by_coord(x_axis[0], x_axis[1], x_axis[2])?;
    let d2 = hsf.add_new_direction_by_coord(y_axis[0], y_axis[1], y_axis[2])?;
    let d3 = hsf.add_new_direction_by_coord(z_axis[0], z_axis[1], z_axis[2])?;

    cleanup_old(&part, cmd_sel, &["Boundingbox", "BoundingBox"]);

    let j = part.bodies()?.count()?;
    let body1 = part.bodies()?.add()?;
    body1.set_name(&format!("Boundingbox.{j}"))?;
    let hybrid_bodies1 = body1.hybrid_bodies()?;
    let hybrid_body1 = hybrid_bodies1.add()?;
    hybrid_body1.set_name("definition_points")?;

    // 多 body: 每 body 一个 extract（跳过临时体防自指污染）
    let mut extracts: Vec<Entity> = Vec::new();
    let bodies = part.bodies()?;
    let body_items = bodies.items()?;
    for bdy in &body_items {
        let nm = bdy.name().unwrap_or_default().to_lowercase();
        if TEMP_NAMES.iter().any(|k| nm.contains(k)) {
            continue;
        }
        let ex: Option<Entity> = None;
        let attempt = (|| -> ComResult<Entity> {
            let ref_b = part.create_reference_from_object(bdy)?;
            let e = hsf.add_new_extract(&ref_b)?;
            e.set_propagation_type(1)?;
            e.set_complementary_extract(false)?;
            e.set_is_federated(false)?;
            hybrid_body1.append_hybrid_shape(&e)?;
            part.set_in_work_object(&e)?;
            hsf.gsm_visibility(&e, 0)?;
            part.update()?;
            Ok(Entity::new(e.com().clone()))
        })();
        match attempt {
            Ok(e) => extracts.push(e),
            Err(ex_err) => {
                progress(&format!(
                    "[bbox] 跳过不可测量 body「{}」: {ex_err}",
                    bdy.name().unwrap_or_default()
                ));
                drop_feature(cmd_sel, ex.as_ref());
            }
        }
    }
    if extracts.is_empty() {
        return Err(rscatiacli::ComError::runtime(
            "零件中所有 body 的 extract 均失败, 无可测量实体",
        ));
    }

    let hybrid_body2 = hybrid_bodies1.item_by_name("definition_points")?;

    // ---- 6 个极值点（多 body 取全局最值）----
    let (extremum1, pt1) = global_extremum(
        &hsf,
        &part,
        &hybrid_body2,
        &extracts,
        &d1,
        x_axis,
        true,
        "max_X",
        spa,
        ss,
        cmd_sel,
    )?;
    let (extremum2, pt2) = global_extremum(
        &hsf,
        &part,
        &hybrid_body2,
        &extracts,
        &d1,
        x_axis,
        false,
        "min_X",
        spa,
        ss,
        cmd_sel,
    )?;
    let (extremum3, pt3) = global_extremum(
        &hsf,
        &part,
        &hybrid_body2,
        &extracts,
        &d2,
        y_axis,
        true,
        "max_Y",
        spa,
        ss,
        cmd_sel,
    )?;
    let (extremum4, pt4) = global_extremum(
        &hsf,
        &part,
        &hybrid_body2,
        &extracts,
        &d2,
        y_axis,
        false,
        "min_Y",
        spa,
        ss,
        cmd_sel,
    )?;
    let (extremum5, pt5) = global_extremum(
        &hsf,
        &part,
        &hybrid_body2,
        &extracts,
        &d3,
        z_axis,
        true,
        "max_Z",
        spa,
        ss,
        cmd_sel,
    )?;
    // 原 Python: 重新建 d3b 再求 min_Z
    let d3b = hsf.add_new_direction_by_coord(z_axis[0], z_axis[1], z_axis[2])?;
    let (_extremum6, pt6) = global_extremum(
        &hsf,
        &part,
        &hybrid_body2,
        &extracts,
        &d3b,
        z_axis,
        false,
        "min_Z",
        spa,
        ss,
        cmd_sel,
    )?;
    let _ = (extremum1, extremum2, extremum3, extremum4, extremum5);
    part.update()?;

    // ---- 每个极值点的参考点引用 ----
    let point_ref11 = part.create_reference_from_object(&pt1)?;
    let point_ref12 = part.create_reference_from_object(&pt2)?;
    let point_ref13 = part.create_reference_from_object(&pt3)?;
    let point_ref14 = part.create_reference_from_object(&pt4)?;
    let point_ref5 = part.create_reference_from_object(&pt5)?;
    let point_ref6 = part.create_reference_from_object(&pt6)?;
    part.update()?;

    // ---- sketch1: 底面正方形（4 条距离约束贴住极值点）----
    let sketches1 = hybrid_body1.hybrid_sketches()?;
    // 草图建在共享基准坐标系 tooling axis 的 XY 平面上
    let tooling_axis = part.axis_systems()?.items()?.into_iter().find(|ax| {
        ax.name()
            .unwrap_or_default()
            .to_lowercase()
            .contains("tooling axis")
    });
    let Some(tooling_axis) = tooling_axis else {
        return Err(rscatiacli::ComError::runtime(
            "缺少 tooling axis 共享基准坐标系",
        ));
    };
    let tname = tooling_axis.name()?;
    let reference_axis_syst = part.create_reference_from_name(&format!(
        "Selection_RSur:(Face:(Brp:({tname};1);None:());{tname})"
    ))?;
    let sketch1 = sketches1.add(&reference_axis_syst)?;
    let factory2d1 = sketch1.open_edition()?;
    let geometric_elements1 = sketch1.geometric_elements()?;
    let axis2d1 = geometric_elements1.item_by_name("AbsoluteAxis")?;
    let line_h = rscatiacli::objects::Line2D::new(axis2d1.get_item("HDirection")?);
    line_h.set_report_name(1)?;
    let line_v = rscatiacli::objects::Line2D::new(axis2d1.get_item("VDirection")?);
    line_v.set_report_name(2)?;

    let ponto = 20000.0;
    let p1 = factory2d1.create_point(-ponto, -ponto)?;
    p1.set_report_name(3)?;
    let p2 = factory2d1.create_point(ponto, -ponto)?;
    p2.set_report_name(4)?;
    let p3 = factory2d1.create_point(ponto, ponto)?;
    p3.set_report_name(5)?;
    let p4 = factory2d1.create_point(-ponto, ponto)?;
    p4.set_report_name(6)?;

    let l12 = factory2d1.create_line(-ponto, -ponto, ponto, -ponto)?;
    l12.set_report_name(7)?;
    l12.set_start_point(&p1)?;
    l12.set_end_point(&p2)?;
    let l23 = factory2d1.create_line(ponto, -ponto, ponto, ponto)?;
    l23.set_report_name(8)?;
    l23.set_start_point(&p2)?;
    l23.set_end_point(&p3)?;
    let l34 = factory2d1.create_line(-ponto, ponto, ponto, ponto)?;
    l34.set_report_name(9)?;
    l34.set_start_point(&p4)?;
    l34.set_end_point(&p3)?;
    let l41 = factory2d1.create_line(-ponto, -ponto, -ponto, ponto)?;
    l41.set_report_name(10)?;
    l41.set_start_point(&p1)?;
    l41.set_end_point(&p4)?;

    let r12 = part.create_reference_from_object(&l12)?;
    let r23 = part.create_reference_from_object(&l23)?;
    let r34 = part.create_reference_from_object(&l34)?;
    let r41 = part.create_reference_from_object(&l41)?;

    let constraints1 = sketch1.constraints()?;
    let _ = constraints1.add_mono_elt_cst(rscatiacli::objects::CAT_CST_TYPE_HORONTALITY, &r12)?;
    let _ = constraints1.add_mono_elt_cst(rscatiacli::objects::CAT_CST_TYPE_VERTICALITY, &r23)?;
    let _ =
        constraints1.add_bi_elt_cst(rscatiacli::objects::CAT_CST_TYPE_PARALLELISM, &r12, &r34)?;
    let _ =
        constraints1.add_bi_elt_cst(rscatiacli::objects::CAT_CST_TYPE_PARALLELISM, &r23, &r41)?;
    // catCstTypeDistance=1：矩形 4 条边贴到 4 个极值点（距离=0）
    let cst2 = constraints1.add_bi_elt_cst(
        rscatiacli::objects::CAT_CST_TYPE_DISTANCE,
        &point_ref11,
        &r23,
    )?;
    let cst3 = constraints1.add_bi_elt_cst(
        rscatiacli::objects::CAT_CST_TYPE_DISTANCE,
        &point_ref13,
        &r34,
    )?;
    let cst4 = constraints1.add_bi_elt_cst(
        rscatiacli::objects::CAT_CST_TYPE_DISTANCE,
        &r41,
        &point_ref12,
    )?;
    let cst1 = constraints1.add_bi_elt_cst(
        rscatiacli::objects::CAT_CST_TYPE_DISTANCE,
        &r12,
        &point_ref14,
    )?;
    for cst in [&cst1, &cst2, &cst3, &cst4] {
        cst.dimension()?.set_value(0.0)?;
    }
    sketch1.close_edition()?;
    part.update()?;

    // ---- 上下平面（贴合极值点）----
    let origin_line_1 = part.create_reference_from_object(&line_h)?;
    let origin_line_2 = part.create_reference_from_object(&line_v)?;
    let plan_origin = hsf.add_new_plane2_lines(&origin_line_1, &origin_line_2)?;
    let ref_plan_origin = part.create_reference_from_object(&plan_origin)?;
    let plan_inferieur = hsf.add_new_plane_offset_pt(&ref_plan_origin, &point_ref6)?;
    hybrid_body2.append_hybrid_shape(&plan_inferieur)?;
    part.set_in_work_object(&plan_inferieur)?;
    let plan_superieur = hsf.add_new_plane_offset_pt(&ref_plan_origin, &point_ref5)?;
    hybrid_body2.append_hybrid_shape(&plan_superieur)?;
    part.set_in_work_object(&plan_superieur)?;
    let _ = hsf.gsm_visibility(&plan_inferieur, 0);
    let _ = hsf.gsm_visibility(&plan_superieur, 0);
    part.update()?;

    // ---- Z 方向引导线（底点投到顶面）----
    let point_inf = hsf.add_new_point_coord_with_reference(0.0, 0.0, 0.0, &point_ref6)?;
    hybrid_body2.append_hybrid_shape(&point_inf)?;
    let ref_point_inf = part.create_reference_from_object(&point_inf)?;
    let _ = hsf.gsm_visibility(&ref_point_inf, 0);
    let proj_pt_inf = hsf.add_new_project(&point_ref6, &plan_superieur)?;
    hybrid_body2.append_hybrid_shape(&proj_pt_inf)?;
    part.set_in_work_object(&proj_pt_inf)?;
    let _ = hsf.gsm_visibility(&proj_pt_inf, 0);
    let point_sup = hsf.add_new_point_coord_with_reference(0.0, 0.0, 0.0, &proj_pt_inf)?;
    hybrid_body2.append_hybrid_shape(&point_sup)?;
    let _ = hsf.gsm_visibility(&point_sup, 0);
    let ref_point_sup = part.create_reference_from_object(&point_sup)?;
    let line_guide = hsf.add_new_line_pt_pt(&ref_point_sup, &ref_point_inf)?;
    hybrid_body2.append_hybrid_shape(&line_guide)?;
    part.set_in_work_object(&line_guide)?;
    let _ = hsf.gsm_visibility(&line_guide, 0);
    let ref_guideline = part.create_reference_from_object(&line_guide)?;
    line_guide.begin_offset()?.set_value(0.0)?;
    line_guide.end_offset()?.set_value(0.0)?;

    // dz 长度约束（挂在零件级约束上）
    let part_constraints: Constraints = part.constraints()?;
    let constraint_dz = part_constraints
        .add_mono_elt_cst(rscatiacli::objects::CAT_CST_TYPE_LENGTH, &ref_guideline)?;
    let length_dz = constraint_dz.dimension()?;
    part.update()?;
    // 约束线宽（纯观感，失败不中断）
    let _ = cmd_sel.clear();
    if cmd_sel.add(&constraint_dz).is_ok() {
        if let Ok(vis) = cmd_sel.vis_properties() {
            let _ = vis.set_real_width(4, 1);
        }
    }
    let _ = cmd_sel.clear();
    part.update()?;

    // ---- sketch2: 顶面正方形（投影 sketch1 的线）----
    let sketches2 = hybrid_body1.hybrid_sketches()?;
    let sketch2 = sketches2.add(&plan_superieur)?;
    let factory2d2 = sketch2.open_edition()?;

    let pont = 200000.0;
    let q1 = factory2d2.create_point(-pont, -pont)?;
    let q2 = factory2d2.create_point(pont, -pont)?;
    let q3 = factory2d2.create_point(pont, pont)?;
    let q4 = factory2d1.create_point(-pont, pont)?; // 保留与 Python 相同的 factory（2d1 亦可用）
    let _ = &q4;
    let q4 = factory2d2.create_point(-pont, pont)?;
    let _ = q4;

    let m12 = factory2d2.create_line(-pont, -pont, pont, -pont)?;
    m12.set_start_point(&q1)?;
    m12.set_end_point(&q2)?;
    let m23 = factory2d2.create_line(pont, -pont, pont, pont)?;
    m23.set_start_point(&q2)?;
    m23.set_end_point(&q3)?;
    let m34 = factory2d2.create_line(-pont, pont, pont, pont)?;
    m34.set_start_point(&q4)?;
    m34.set_end_point(&q3)?;
    let m41 = factory2d2.create_line(-pont, -pont, -pont, pont)?;
    m41.set_start_point(&q1)?;
    m41.set_end_point(&q4)?;

    let rm12 = part.create_reference_from_object(&m12)?;
    let rm23 = part.create_reference_from_object(&m23)?;
    let rm34 = part.create_reference_from_object(&m34)?;
    let rm41 = part.create_reference_from_object(&m41)?;

    // 投影 sketch1 的 4 条边到 sketch2
    let pr12 = factory2d2.create_projection(&r12)?;
    let pr23 = factory2d2.create_projection(&r23)?;
    let pr34 = factory2d2.create_projection(&r34)?;
    let pr41 = factory2d2.create_projection(&r41)?;
    let rp12 = part.create_reference_from_object(&pr12)?;
    let rp23 = part.create_reference_from_object(&pr23)?;
    let rp34 = part.create_reference_from_object(&pr34)?;
    let rp41 = part.create_reference_from_object(&pr41)?;

    let constraints2 = sketch2.constraints()?;
    let _ = constraints2.add_mono_elt_cst(rscatiacli::objects::CAT_CST_TYPE_HORONTALITY, &rm12)?;
    let _ = constraints2.add_mono_elt_cst(rscatiacli::objects::CAT_CST_TYPE_VERTICALITY, &rm23)?;
    let _ =
        constraints2.add_bi_elt_cst(rscatiacli::objects::CAT_CST_TYPE_PARALLELISM, &rm12, &rm34)?;
    let _ =
        constraints2.add_bi_elt_cst(rscatiacli::objects::CAT_CST_TYPE_PARALLELISM, &rm23, &rm41)?;
    let c11 =
        constraints2.add_bi_elt_cst(rscatiacli::objects::CAT_CST_TYPE_DISTANCE, &rm12, &rp12)?;
    let c12 =
        constraints2.add_bi_elt_cst(rscatiacli::objects::CAT_CST_TYPE_DISTANCE, &rp23, &rm23)?;
    let c13 =
        constraints2.add_bi_elt_cst(rscatiacli::objects::CAT_CST_TYPE_DISTANCE, &rp34, &rm34)?;
    let c14 =
        constraints2.add_bi_elt_cst(rscatiacli::objects::CAT_CST_TYPE_DISTANCE, &rm41, &rp41)?;
    for cst in [&c11, &c12, &c13, &c14] {
        cst.dimension()?.set_value(0.0)?;
    }
    sketch2.close_edition()?;
    part.update()?;

    // ---- sketch3: 底面（测 dx/dy）----
    let sketches3 = hybrid_body1.hybrid_sketches()?;
    let sketch3 = sketches3.add(&plan_inferieur)?;
    let factory2d3 = sketch3.open_edition()?;

    let pj12 = factory2d3.create_projection(&rm12)?;
    let pj23 = factory2d3.create_projection(&rm23)?;
    let pj34 = factory2d3.create_projection(&rm34)?;
    let pj41 = factory2d3.create_projection(&rm41)?;
    let rpj12 = part.create_reference_from_object(&pj12)?;
    let rpj23 = part.create_reference_from_object(&pj23)?;
    let _rpj34 = part.create_reference_from_object(&pj34)?;
    let _rpj41 = part.create_reference_from_object(&pj41)?;

    let constraints3 = sketch3.constraints()?;
    let constraint_dx =
        constraints3.add_mono_elt_cst(rscatiacli::objects::CAT_CST_TYPE_LENGTH, &rpj12)?;
    let constraint_dy =
        constraints3.add_mono_elt_cst(rscatiacli::objects::CAT_CST_TYPE_LENGTH, &rpj23)?;
    let length_dx = constraint_dx.dimension()?;
    let dx_value = length_dx.value()?;
    let length_dy = constraint_dy.dimension()?;
    let dy_value = length_dy.value()?;
    sketch3.close_edition()?;
    part.update()?;

    // ---- 扫掠 ----
    let sweepref_1 = part.create_reference_from_object(&sketch3)?;
    let guide1 = part.create_reference_from_object(&line_guide)?;
    let sweep1 = hsf.add_new_sweep_explicit(&sweepref_1, &guide1)?;
    hybrid_body2.append_hybrid_shape(&sweep1)?;
    part.set_in_work_object(&sweep1)?;
    part.update()?;

    // 隐藏辅助几何 + 扫掠着色（观感，失败不中断）
    let _ = cmd_sel.clear();
    let vis_ok = (|| -> ComResult<()> {
        cmd_sel.add(&sketch1)?;
        cmd_sel.add(&sketch2)?;
        cmd_sel.add(&sketch3)?;
        cmd_sel.add(&body1)?;
        cmd_sel.vis_properties()?.set_show(0)?; // catVisPropertyNoShowAttr=0
        cmd_sel.add(&sweep1)?;
        let vis = cmd_sel.vis_properties()?;
        vis.set_real_color(153, 204, 255, 1)?;
        vis.set_real_opacity(160, 1)?;
        Ok(())
    })();
    if vis_ok.is_err() {
        progress("[bbox] 辅助几何隐藏/着色失败(仅观感, 继续)");
    }
    let _ = cmd_sel.clear();

    Ok(BboxDims {
        length_mm: (dx_value * 1000.0).round() / 1000.0,
        width_mm: (dy_value * 1000.0).round() / 1000.0,
        height_mm: (length_dz.value()? * 1000.0).round() / 1000.0,
    })
}

// ── 材料 / 密度 / 重量 ───────────────────────────────────────────────

/// 从 Product 参数表读已挂材料名（回退路径；"...\\Material" 参数才可靠）。
fn read_material_from_params(doc: &rscatiacli::objects::Document) -> String {
    let Ok(prod) = doc.product_document().product() else {
        return String::new();
    };
    let Ok(params) = prod.com().get("Parameters").and_then(|v| v.as_obj()) else {
        return String::new();
    };
    let Ok(count) = params.get("Count").and_then(|v| v.as_i32()) else {
        return String::new();
    };
    for i in 1..=count {
        let Ok(p) = rscatiacli::item_at(&params, i) else {
            continue;
        };
        let nm = p
            .get("Name")
            .and_then(|v| v.as_string())
            .unwrap_or_default();
        let low = nm.to_lowercase();
        if low == "material" || low.ends_with("\\material") || low.ends_with("/material") {
            let v = p
                .get("Value")
                .and_then(|v| v.as_string())
                .unwrap_or_default()
                .trim()
                .to_string();
            if !v.is_empty() {
                return v;
            }
        }
    }
    String::new()
}

/// 读材料名 + 密度/质量（kg/m³、kg → g/cm³、g；失败返回 None 不阻断测量）。
fn read_material_and_weight(
    app: &rscatiacli::objects::Application,
    doc: &rscatiacli::objects::Document,
    spa: &SPAWorkbench,
    ss: &SystemService,
    body_refs: &[rscatiacli::objects::Reference],
    progress: Progress<'_>,
) -> (Option<String>, Option<f64>, Option<f64>) {
    let mut material: Option<String> = None;
    let part = match doc.part_document().part() {
        Ok(p) => p,
        Err(_) => return (None, None, None),
    };
    // 材料名: CATMatManagerVBExt → GetMaterialOnPart（VBA 中转）
    let _ = app;
    if let Ok(manager) = part.get_item("CATMatManagerVBExt") {
        let args = [
            rscatiacli::dispatch::Arg::Str("part".to_string()),
            rscatiacli::dispatch::Arg::Obj(manager.com().clone()),
            rscatiacli::dispatch::Arg::Obj(part.com().clone()),
        ];
        match ss.evaluate(VBA_GET_MATERIAL, 0, "get_material", &args) {
            Ok(ret) => {
                if let Ok(mat) = ret.as_obj() {
                    let name = mat.name().unwrap_or_default().trim().to_string();
                    if !name.is_empty() {
                        material = Some(name);
                    }
                }
            }
            Err(e) => progress(&format!("[material] 读取材料名失败(忽略): {e}")),
        }
    }
    if material.is_none() {
        let from_params = read_material_from_params(doc);
        if !from_params.is_empty() {
            material = Some(from_params);
        }
    }

    // 密度/质量: 首选 Product 级 Inertia 技术对象
    let mut density_kg_m3 = 0.0f64;
    let mut total_mass_kg = 0.0f64;
    let prod = doc.product_document().product();
    let inertia = prod
        .as_ref()
        .ok()
        .and_then(|p| p.get_technological_object("Inertia").ok());
    match inertia {
        Some(i) => {
            density_kg_m3 = i
                .com()
                .get("Density")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            total_mass_kg = i.com().get("Mass").and_then(|v| v.as_f64()).unwrap_or(0.0);
        }
        None => {
            progress("[weight] Product Inertia 读取失败, 回退逐 body");
            for rb in body_refs {
                if let Ok(m) = spa.get_measurable(rb) {
                    if let Ok(inertia) = m.com().get("Inertia").and_then(|v| v.as_obj()) {
                        if density_kg_m3 <= 0.0 {
                            density_kg_m3 = inertia
                                .get("Density")
                                .and_then(|v| v.as_f64())
                                .unwrap_or(0.0);
                        }
                        total_mass_kg +=
                            inertia.get("Mass").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    }
                }
            }
        }
    }
    let density_g_per_cm3 = if density_kg_m3 > 0.0 {
        Some((density_kg_m3 / 1000.0 * 10000.0).round() / 10000.0)
    } else {
        None
    };
    let weight_g = if total_mass_kg > 0.0 {
        Some((total_mass_kg * 1000.0 * 100.0).round() / 100.0)
    } else {
        None
    };
    (material, density_g_per_cm3, weight_g)
}

/// 在 part 里创建 Bbox_dx/dy/dz 三个 Length 参数并赋值。
fn create_bbox_parameters(
    part: &rscatiacli::objects::Part,
    length_mm: f64,
    width_mm: f64,
    height_mm: f64,
) -> ComResult<Value> {
    let params = part.parameters()?;
    let mut out = serde_json::Map::new();
    for (key, val) in [
        ("Bbox_dx.1", length_mm),
        ("Bbox_dy.1", width_mm),
        ("Bbox_dz.1", height_mm),
    ] {
        match params.create_dimension(key, "Length", val) {
            Ok(dim) => {
                out.insert(key.to_string(), json!(dim.value().unwrap_or(val)));
            }
            Err(_) => {
                out.insert(key.to_string(), json!(val));
            }
        }
    }
    part.update()?;
    Ok(Value::Object(out))
}

/// 把尺寸写入文档根 Product 的 DescriptionInst / DescriptionRef。
fn write_description(doc: &rscatiacli::objects::Document, text: &str, progress: Progress<'_>) {
    let prod = doc.product_document().product();
    let mut errors = Vec::new();
    if let Ok(p) = prod {
        if p.set_description_inst(text).is_err() {
            errors.push("DescriptionInst");
        }
        if p.set_description_ref(text).is_err() {
            errors.push("DescriptionRef");
        }
    } else {
        errors.push("Product");
    }
    if !errors.is_empty() {
        progress(&format!(
            "[desc] 写入 description 部分失败(不影响测量结果): {}",
            errors.join("; ")
        ));
    }
    progress(&format!("[desc] 已写入零件描述: {text}"));
}

// ── 主流程 ───────────────────────────────────────────────────────────

/// 还原工具借用的零件编辑上下文：装配选中集 + 原始 in-work 对象。
fn restore_context(
    cmd_sel: &Selection,
    borrowed: bool,
    snapshot: &[ComObj],
    part: &rscatiacli::objects::Part,
    orig_in_work: Option<&ComObj>,
    orig_in_work_name: Option<&str>,
    progress: Progress<'_>,
) {
    if borrowed {
        restore_selection(cmd_sel, snapshot);
    }
    if let (Some(orig), Some(orig_name)) = (orig_in_work, orig_in_work_name) {
        match part.com().get("InWorkObject").and_then(|v| v.as_obj()) {
            Ok(cur) => {
                let cur_name = cur
                    .get("Name")
                    .and_then(|n| n.as_string())
                    .unwrap_or_default();
                if cur_name == orig_name {
                    return; // 未被改写
                }
                if part.set_in_work_object(&Entity::new(orig.clone())).is_ok() {
                    progress(&format!("[target] 已还原 in-work 对象: {orig_name}"));
                    return;
                }
                progress("[target] 还原 in-work 对象失败, 改切实体 body");
            }
            Err(e) => progress(&format!(
                "[target] 还原 in-work 对象失败, 改切实体 body: {e}"
            )),
        }
    }
    // 无原对象可还原: 切回第一个非临时实体 body
    if let Ok(bodies) = part.bodies() {
        if let Ok(items) = bodies.items() {
            for b in &items {
                let name = b.name().unwrap_or_default();
                if is_temp_name(&name) {
                    continue;
                }
                let _ = part.set_in_work_object(b);
                progress(&format!("[target] in-work 切回实体 body: {name}"));
                break;
            }
        }
    }
}

/// 工具主体。**必须在 STA 线程上执行。**
pub fn run(params: &Value, progress: Progress<'_>) -> ComResult<Value> {
    let file_path = params
        .get("file_path")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.replace('/', "\\"));
    let keep_geometry = params
        .get("keep_geometry")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let config = params.get("config").cloned().unwrap_or(Value::Null);

    let app = attach_with_guard(progress)?;
    let doc = match &file_path {
        Some(p) => app.documents()?.open(p)?,
        None => app.active_document()?,
    };

    let (doc, resolve_note) = match resolve_part_document(&app, &doc) {
        Ok(v) => v,
        Err(msg) => return Ok(json!({ "success": false, "message": msg })),
    };
    if !resolve_note.is_empty() {
        progress(&format!("[target] {resolve_note}"));
        let _ = doc.activate(); // 零件在装配中未独立开窗时无效(实测)，保留调用
    }

    // 命令类操作(Delete)落在激活编辑器的选中集上
    let (cmd_sel, borrowed_sel) = command_selection(&app, &doc)?;
    let sel_snapshot = if borrowed_sel {
        progress("[target] 零件在装配中打开: 命令删除改用激活装配的选中集");
        snapshot_selection(&cmd_sel)
    } else {
        Vec::new()
    };

    // 强制 DESIGN 模式（CGR 下 Measurable/extract/extremum 失效）
    match doc
        .product_document()
        .product()
        .and_then(|p| p.apply_work_mode(2))
    {
        Ok(()) => progress("[workmode] 已强制 DESIGN 模式"),
        Err(e) => progress(&format!("[workmode] ApplyWorkMode 失败(继续尝试): {e}")),
    }

    let part = doc.part_document().part()?;
    let ss = app.system_service()?;
    let spa = doc.part_document().spa_workbench()?;

    // 记录原 in-work 对象（结束后还原；位于临时几何内的不还原）
    let orig_in_work_raw = part.com().get("InWorkObject").and_then(|v| v.as_obj()).ok();
    let mut orig_in_work: Option<ComObj> = None;
    let mut orig_in_work_name: Option<String> = None;
    if let Some(orig) = &orig_in_work_raw {
        let name = orig
            .get("Name")
            .and_then(|n| n.as_string())
            .unwrap_or_default();
        if in_temp_geometry(orig) {
            progress(&format!(
                "[target] 原 in-work「{name}」位于临时几何内(上次运行遗留), 本次不还原"
            ));
        } else {
            orig_in_work = Some(orig.clone());
            orig_in_work_name = Some(name);
        }
    }

    // 零件号/零件名取自零件本体
    let part_name = part.name().unwrap_or_default();
    let mut catia_pn = String::new();
    match doc
        .product_document()
        .product()
        .and_then(|p| p.part_number())
    {
        Ok(pn) => {
            catia_pn = refine_part_number(pn.trim(), &config);
            if catia_pn.is_empty() {
                catia_pn = pn.trim().to_string();
            }
        }
        Err(e) => progress(&format!("[naming] 读取 Product.PartNumber 失败: {e}")),
    }
    progress(&format!(
        "[naming] CATIA 零件号={catia_pn:?} 零件名={part_name:?}"
    ));

    // 幂等：先清掉上一次(可能半成品)生成的临时几何
    cleanup_temp(&part, &cmd_sel, &[]);

    let result = (|| -> ComResult<Value> {
        // 1) 全部实体 body 的真实体积/质心（跳过临时体）
        let mut body_refs: Vec<rscatiacli::objects::Reference> = Vec::new();
        let mut real_volume_m3 = 0.0f64;
        let mut cog_w = [0.0f64; 3];
        let mut cog_vsum = 0.0f64;
        let mut cogs_plain: Vec<Vec3> = Vec::new();
        for bdy in part.bodies()?.items()? {
            let nm = bdy.name().unwrap_or_default().to_lowercase();
            if TEMP_NAMES.iter().any(|k| nm.contains(k)) {
                continue;
            }
            let Ok(rb) = part.create_reference_from_object(&bdy) else {
                continue;
            };
            body_refs.push(rb.clone());
            let vol_m3 = spa
                .get_measurable(&rb)
                .and_then(|m| m.volume())
                .unwrap_or(0.0);
            if vol_m3 <= 0.0 {
                if let Ok(m) = spa.get_measurable(&rb) {
                    if let Ok(c) = m.get_cog(&ss) {
                        cogs_plain.push(c);
                    }
                }
                continue;
            }
            real_volume_m3 += vol_m3;
            if let Ok(c) = spa.get_measurable(&rb).and_then(|m| m.get_cog(&ss)) {
                cog_w[0] += vol_m3 * c[0];
                cog_w[1] += vol_m3 * c[1];
                cog_w[2] += vol_m3 * c[2];
                cog_vsum += vol_m3;
            }
        }
        let merged_cog: Option<Vec3> = if cog_vsum > 0.0 {
            Some([
                cog_w[0] / cog_vsum,
                cog_w[1] / cog_vsum,
                cog_w[2] / cog_vsum,
            ])
        } else if !cogs_plain.is_empty() {
            let n = cogs_plain.len() as f64;
            Some([
                cogs_plain.iter().map(|c| c[0]).sum::<f64>() / n,
                cogs_plain.iter().map(|c| c[1]).sum::<f64>() / n,
                cogs_plain.iter().map(|c| c[2]).sum::<f64>() / n,
            ])
        } else {
            None
        };
        progress(&format!(
            "[volume] 实体 body {} 个, 合并真实体积={:.1}mm³",
            body_refs.len(),
            real_volume_m3 * 1e9
        ));

        // 2) 轴系：优先复用已有 tooling axis
        let existing_axis = find_tooling_axis(&part, &ss, progress);
        let (x_axis, y_axis, z_axis, cog): (Vec3, Vec3, Vec3, Vec3) = match &existing_axis {
            Some(ax) => {
                let x = norm(ax.x_axis);
                let y = norm(ax.y_axis);
                let z = norm(ax.z_axis);
                let cog = ax.origin_mm;
                if ax.origin_type != 1 {
                    delete_axis_systems(&part, &cmd_sel, &["tooling axis", "Box_Axis_System"]);
                    if borrowed_sel {
                        restore_selection(&cmd_sel, &sel_snapshot);
                    }
                    build_axis_system(&part, x, y, z, cog)?;
                    progress(&format!(
                        "[axis] 重建 tooling axis 为去参基准(沿用已有方向): 原点={cog:?}"
                    ));
                } else {
                    progress(&format!("[axis] 复用已有 tooling axis 基准: 原点={cog:?}"));
                }
                (x, y, z, cog)
            }
            None => {
                // 无已有 tooling axis: 按 draftline/halfline 新建
                let dl = match find_line_by_keyword(
                    &doc.part_document(),
                    "draftline",
                    borrowed_sel,
                    &ss,
                    &spa,
                    progress,
                ) {
                    Ok(v) => v,
                    Err(e) => {
                        restore_context(
                            &cmd_sel,
                            borrowed_sel,
                            &sel_snapshot,
                            &part,
                            orig_in_work.as_ref(),
                            orig_in_work_name.as_deref(),
                            progress,
                        );
                        return Ok(
                            json!({ "success": false, "message": format!("零件无已有 tooling axis, 且缺少 draftline 直线: {e}") }),
                        );
                    }
                };
                let hl = match find_line_by_keyword(
                    &doc.part_document(),
                    "halfline",
                    borrowed_sel,
                    &ss,
                    &spa,
                    progress,
                ) {
                    Ok(v) => v,
                    Err(e) => {
                        restore_context(
                            &cmd_sel,
                            borrowed_sel,
                            &sel_snapshot,
                            &part,
                            orig_in_work.as_ref(),
                            orig_in_work_name.as_deref(),
                            progress,
                        );
                        return Ok(
                            json!({ "success": false, "message": format!("零件无已有 tooling axis, 且缺少 halfline 直线: {e}") }),
                        );
                    }
                };
                let z = norm(dl.direction);
                let x = norm(hl.direction);
                let y = norm(cross(z, x));
                let cog = merged_cog.unwrap_or_else(|| {
                    progress("[cog] 取所有 body 合并质心失败, 退回数值原点 (0,0,0)");
                    [0.0; 3]
                });
                build_axis_system(&part, x, y, z, cog)?;
                progress(&format!(
                    "[axis] 新建 tooling axis 基准: 原点={cog:?} X={x:?} Y={y:?} Z={z:?}"
                ));
                (x, y, z, cog)
            }
        };

        // 5) 按 VBA 方法生成几何并读尺寸
        let dims = generate_bbox(
            &doc.part_document(),
            x_axis,
            y_axis,
            z_axis,
            &cmd_sel,
            &ss,
            &spa,
            progress,
        )?;
        let bbox_vol = dims.length_mm * dims.width_mm * dims.height_mm;
        let volume_mm3 = if real_volume_m3 > 0.0 {
            (real_volume_m3 * 1e9 * 10.0).round() / 10.0
        } else {
            progress("[volume] WARN 读取真实体积失败, 回退用包围盒容积 L×W×H");
            (bbox_vol * 10.0).round() / 10.0
        };
        progress(&format!(
            "[bbox] 长={} 宽={} 高={} 真实体积={volume_mm3}mm³ (包围盒容积={:.1}mm³)",
            dims.length_mm, dims.width_mm, dims.height_mm, bbox_vol
        ));

        // 5.2) 材料 / 密度 / 重量
        let (material, density_g_per_cm3, weight_g) =
            read_material_and_weight(&app, &doc, &spa, &ss, &body_refs, progress);
        progress(&format!(
            "[mat] 材料={} 密度={}g/cm³ 重量={}g",
            material.as_deref().unwrap_or("-"),
            density_g_per_cm3
                .map(|v| v.to_string())
                .unwrap_or_else(|| "-".into()),
            weight_g
                .map(|v| v.to_string())
                .unwrap_or_else(|| "-".into()),
        ));

        // 6) Bbox_dx/dy/dz 参数
        let bbox_parameters =
            create_bbox_parameters(&part, dims.length_mm, dims.width_mm, dims.height_mm)?;

        // 6.5) 描述属性（保存文档才落盘）
        let description = format!(
            "x{:.1}y{:.1}z{:.1}",
            dims.length_mm, dims.width_mm, dims.height_mm
        );
        write_description(&doc, &description, progress);

        // 7) 是否保留生成的包围盒集合体
        let kept_geometry = keep_geometry;
        if !keep_geometry {
            cleanup_temp(&part, &cmd_sel, &["rough stock"]);
            progress("[bbox] 未勾选保留集合体, 已删除包围盒集合体/Rough stock(保留 tooling axis 基准, 仅保留测量结果)");
        }

        // 8) 保存文档（描述落盘；失败只提示）
        let mut saved = false;
        match part.update().and_then(|_| doc.save()) {
            Ok(()) => saved = true,
            Err(e) => progress(&format!("[bbox] 保存文档失败, 描述未落盘(请手动保存): {e}")),
        }

        restore_context(
            &cmd_sel,
            borrowed_sel,
            &sel_snapshot,
            &part,
            orig_in_work.as_ref(),
            orig_in_work_name.as_deref(),
            progress,
        );

        Ok(json!({
            "success": true,
            "message": format!(
                "包围盒几何生成完成, 零件描述已写入{}{}",
                if kept_geometry { "" } else { " (集合体已清理, 保留 tooling axis 基准坐标系)" },
                if saved { "; 文档已保存" } else { "; 文档未保存, 请手动保存" }
            ),
            "data": {
                "length_mm": dims.length_mm,
                "width_mm": dims.width_mm,
                "height_mm": dims.height_mm,
                "volume_mm3": volume_mm3,
                "bbox_volume_mm3": (bbox_vol * 10.0).round() / 10.0,
                "bbox_center_mm": cog.iter().map(|v| (v * 1000.0).round() / 1000.0).collect::<Vec<_>>(),
                "description": description,
                "material": material,
                "density_g_per_cm3": density_g_per_cm3,
                "weight_g": weight_g,
                "bbox_parameters": bbox_parameters,
            },
            "part_number": catia_pn,
            "part_name": part_name,
            "material": material,
            "density_g_per_cm3": density_g_per_cm3,
            "weight_g": weight_g,
            "saved": saved,
            "kept_geometry": kept_geometry,
        }))
    })();

    match result {
        Ok(v) => Ok(v),
        Err(e) => {
            // 失败也还原零件，不留半成品几何
            if let Ok(p) = doc.part_document().part() {
                cleanup_temp(&p, &cmd_sel, &[]);
            }
            if let Ok(p) = doc.part_document().part() {
                restore_context(
                    &cmd_sel,
                    borrowed_sel,
                    &sel_snapshot,
                    &p,
                    orig_in_work_raw.as_ref(),
                    None,
                    progress,
                );
            }
            Ok(json!({
                "success": false,
                "message": format!("生成包围盒几何失败: {e}"),
            }))
        }
    }
}
