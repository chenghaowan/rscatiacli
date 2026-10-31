//! 曲面四件套（join_surface / offset_surface / stitch_surface / trim_boundary）
//! 共用的遍历与类型过滤逻辑，全部对齐各自 Python 版的口径。

use rscatiacli::dispatch::{ComObj, ComObject};
use rscatiacli::objects::{HybridBodies, HybridBody, Part, Selection};
use rscatiacli::ComCollection;

/// 深度优先遍历所有几何图形集（含嵌套），yield COM 包装。
/// 子集合读取失败按叶子处理（与 Python try/except 同口径）。
pub fn iter_geosets(hbs: &HybridBodies) -> Vec<HybridBody> {
    fn walk(hbs: &HybridBodies, out: &mut Vec<HybridBody>) {
        let Ok(count) = hbs.count() else {
            return;
        };
        for i in 1..=count {
            let Ok(hb) = hbs.item(i) else {
                continue;
            };
            out.push(hb.clone());
            if let Ok(sub) = hb.hybrid_bodies() {
                walk(&sub, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(hbs, &mut out);
    out
}

/// COM 类型名（如 `HybridShapeOffset`），失败返回空串（与 Python 同口径）。
pub fn com_type_name(obj: &ComObj) -> String {
    obj.type_name().unwrap_or_default()
}

/// 是否平面（基准平面等）：COM 类型名含 plane。
pub fn is_plane(shape: &ComObj) -> bool {
    com_type_name(shape).to_lowercase().contains("plane")
}

/// 是否 HybridShapeOffset：类型名判定，失败时退化为属性探测（同 Python）。
pub fn is_offset(shape: &ComObj) -> bool {
    let tn = com_type_name(shape).to_lowercase();
    if !tn.is_empty() {
        return tn.contains("offset");
    }
    shape.get("OffsetDirection").is_ok() && shape.get("OffsetValue").is_ok()
}

/// 是否可偏置曲面：SurfaceExplicit / Offset / Extract（原宏 Quick 模式的过滤）。
pub fn is_offsettable(shape: &ComObj) -> bool {
    let tn = com_type_name(shape).to_lowercase();
    if !tn.is_empty() {
        return ["surfaceexplicit", "offset", "extract"]
            .iter()
            .any(|k| tn.contains(k));
    }
    is_offset(shape)
}

/// 是否曲面：类型名含 surface/offset/extract/join 等曲面族关键词，排除平面。
pub fn is_surface(shape: &ComObj) -> bool {
    let tn = com_type_name(shape).to_lowercase();
    if tn.is_empty() {
        return false;
    }
    if tn.contains("plane") {
        return false;
    }
    [
        "surface",
        "offset",
        "extract",
        "join",
        "assemble",
        "fill",
        "loft",
        "sweep",
        "blend",
        "split",
        "trim",
        "fillet",
        "extrude",
        "revolve",
        "symmetry",
        "affinity",
        "scale",
        "translation",
        "rotation",
        "wrap",
    ]
    .iter()
    .any(|k| tn.contains(k))
}

/// 自然排序键：boundary_2 < boundary_10。
pub fn natural_key(name: &str) -> Vec<(u64, String)> {
    fn split_digits(s: &str) -> Vec<(u64, String)> {
        let mut out = Vec::new();
        let mut cur = String::new();
        let mut cur_is_digit = false;
        for ch in s.chars() {
            let d = ch.is_ascii_digit();
            if !cur.is_empty() && d != cur_is_digit {
                out.push(segment(&cur, cur_is_digit));
                cur.clear();
            }
            cur_is_digit = d;
            cur.push(ch);
        }
        if !cur.is_empty() {
            out.push(segment(&cur, cur_is_digit));
        }
        out
    }
    fn segment(s: &str, is_digit: bool) -> (u64, String) {
        if is_digit {
            (s.parse::<u64>().unwrap_or(u64::MAX), String::new())
        } else {
            (0, s.to_lowercase())
        }
    }
    split_digits(name)
}

/// 顶层几何图形集重名时自动追加 _2/_3 后缀（offset_surface 用）。
pub fn unique_geoset_name(base: &str, hbs: &HybridBodies) -> String {
    let existing: Vec<String> = (1..=hbs.count().unwrap_or(0))
        .filter_map(|i| hbs.item(i).ok())
        .filter_map(|hb| hb.name().ok())
        .map(|n| n.to_lowercase())
        .collect();
    if !existing.contains(&base.to_lowercase()) {
        return base.to_string();
    }
    let mut k = 2;
    while existing.contains(&format!("{base}_{k}").to_lowercase()) {
        k += 1;
    }
    format!("{base}_{k}")
}

/// 删除几何图形集及其内容（Selection.Delete，stitch/trim/join 的清理口径），
/// 失败静默（与 Python try/except 一致）。
pub fn cleanup_geoset(doc_sel: &Selection, part: &Part, body: &HybridBody) {
    let _ = doc_sel.clear();
    if doc_sel.add(body).is_ok() {
        let _ = doc_sel.delete();
    }
    let _ = doc_sel.clear();
    let _ = part.update();
}

/// 把形状移动到目标几何图形集（AppendHybridShape 即重挂父级），返回是否成功。
pub fn move_shape(part: &Part, dst: &HybridBody, shape: &impl ComObject) -> bool {
    if dst.append_hybrid_shape(shape).is_err() {
        return false;
    }
    let _ = part.update();
    true
}

/// 给单个特征染指定颜色；`inherit` = 0 不继承父级颜色（与 Python 一致）。
pub fn set_real_color(
    doc_sel: &Selection,
    part: &Part,
    shape: &impl ComObject,
    r: i32,
    g: i32,
    b: i32,
    inherit: i32,
) {
    let _ = doc_sel.clear();
    if let Ok(reference) = part.create_reference_from_object(shape) {
        let _ = doc_sel.add(&reference);
        if let Ok(vis) = doc_sel.vis_properties() {
            let _ = vis.set_real_color(r, g, b, inherit);
        }
    }
    let _ = doc_sel.clear();
}

#[cfg(test)]
mod tests {
    use super::natural_key;

    #[test]
    fn natural_order() {
        let mut names = vec!["boundary_10", "boundary_2", "Boundary_1"];
        names.sort_by_key(|n| natural_key(n));
        assert_eq!(names, vec!["Boundary_1", "boundary_2", "boundary_10"]);
    }
}
