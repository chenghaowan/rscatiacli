//! 蜂窝筋 —— honeycomb_rib.py 的 Rust 版（在零件上直接生成六边形蜂窝筋板）。
//!
//! 背景：FIS 的 TPLC2360 模板无法自动化实例化（PlacePowerCopy 同路径也失败，
//! 模板不在任何 catalog），故直接生成几何。几何构造：
//!   1. 六边形格点用整数格表示（顶点 = (√3R/2·i, R/2·j)），共享顶点严格相等；
//!   2. 只被 1 个单元使用的边即外边界，串成闭环 = 外轮廓；
//!   3. 外轮廓向外米特偏置 t/2，每单元放内缩六边形孔（壁厚处处 t）；
//!   4. 单草图 = 1 外轮廓 + N 孔，一次 Pad 拉筋高。
//!
//! 注意：Pad 必须建在实体 Body 里（InWorkObject 默认是几何图形集，不切会失败）。

use serde_json::{json, Value};

use rscatiacli::{attach_with_guard, ComCollection, ComObject, ComResult, Progress};

/// 尖顶六边形 6 个顶点在整数格上的偏移。
const VERT_OFF: [(i64, i64); 6] = [(0, 2), (1, 1), (1, -1), (0, -2), (-1, -1), (-1, 1)];

// ── 几何（纯计算，不依赖 CATIA）──

/// 六边形中心（整数格）：行距 1.5R、列距 √3R，奇数行右移半格。
fn hex_cell_centers(cols: i64, rows: i64) -> Vec<(i64, i64)> {
    (0..rows)
        .flat_map(|r| (0..cols).map(move |c| (2 * c + r % 2, 3 * r)))
        .collect()
}

/// 蜂窝外轮廓顶点序列（整数格）：只用 1 次的边串成闭环。
fn outer_loop_cells(cols: i64, rows: i64) -> Vec<(i64, i64)> {
    type Vtx = (i64, i64);
    type Edge = (Vtx, Vtx);
    let mut use_count: std::collections::HashMap<Edge, i32> = std::collections::HashMap::new();
    for (ci, cj) in hex_cell_centers(cols, rows) {
        let vs: Vec<Vtx> = VERT_OFF.iter().map(|(di, dj)| (ci + di, cj + dj)).collect();
        for k in 0..6 {
            let mut e = [vs[k], vs[(k + 1) % 6]];
            e.sort();
            *use_count.entry((e[0], e[1])).or_insert(0) += 1;
        }
    }
    let boundary_edges: Vec<Edge> = use_count
        .iter()
        .filter(|(_, n)| **n == 1)
        .map(|(k, _)| *k)
        .collect();
    let mut adj: std::collections::HashMap<Vtx, Vec<Vtx>> = std::collections::HashMap::new();
    for (a, b) in &boundary_edges {
        adj.entry(*a).or_default().push(*b);
        adj.entry(*b).or_default().push(*a);
    }
    let start = boundary_edges[0].0;
    let mut prev: Option<Vtx> = None;
    let mut cur = start;
    let mut loop_pts = vec![start];
    loop {
        let empty = Vec::new();
        let nxt = adj
            .get(&cur)
            .unwrap_or(&empty)
            .iter()
            .find(|x| Some(**x) != prev);
        match nxt {
            Some(x) if *x != start => {
                loop_pts.push(*x);
                prev = Some(cur);
                cur = *x;
            }
            _ => break,
        }
    }
    loop_pts
}

/// 整数格 -> 实际坐标；scale 为外接半径 R 或孔半径。
fn lattice_to_xy(p: (i64, i64), scale: f64) -> (f64, f64) {
    (
        (3.0f64).sqrt() * scale / 2.0 * p.0 as f64,
        scale / 2.0 * p.1 as f64,
    )
}

/// 多边形向外米特（miter）偏置 d。外法线方向由环绕方向决定。
fn offset_polygon_outward(pts: &[(f64, f64)], d: f64) -> Vec<(f64, f64)> {
    let n = pts.len();
    let mut signed = 0.0;
    for i in 0..n {
        let (x1, y1) = pts[i];
        let (x2, y2) = pts[(i + 1) % n];
        signed += x1 * y2 - x2 * y1;
    }
    let sign = if signed > 0.0 { 1.0 } else { -1.0 };
    let mut lines: Vec<(f64, f64, f64, f64)> = Vec::with_capacity(n);
    for i in 0..n {
        let (x1, y1) = pts[i];
        let (x2, y2) = pts[(i + 1) % n];
        let (dx, dy) = (x2 - x1, y2 - y1);
        let length = (dx * dx + dy * dy).sqrt();
        let (ux, uy) = (dx / length, dy / length);
        lines.push((x1 + sign * uy * d, y1 - sign * ux * d, ux, uy));
    }
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let (x1, y1, ux1, uy1) = lines[(i + n - 1) % n];
        let (x2, y2, ux2, uy2) = lines[i];
        let den = ux1 * uy2 - uy1 * ux2;
        if den.abs() < 1e-12 {
            out.push((x2, y2));
            continue;
        }
        let t = ((x2 - x1) * uy2 - (y2 - y1) * ux2) / den;
        out.push((x1 + ux1 * t, y1 + uy1 * t));
    }
    out
}

/// 草图全部闭合轮廓：[(外轮廓, 孔1, 孔2, ...)]。
type HexProfile = Result<(Vec<(f64, f64)>, Vec<Vec<(f64, f64)>>), String>;

fn build_profile(cols: i64, rows: i64, diameter: f64, thickness: f64) -> HexProfile {
    let r = diameter / 2.0;
    // 孔的顶点按 R 定位、按孔半径缩放 —— 混用会让相邻孔重叠，Pad 直接失败
    let hole_r = r - (thickness / 2.0) / (30f64.to_radians().cos());
    if hole_r <= 0.0 {
        return Err(format!(
            "壁厚 {thickness}mm 相对直径 {diameter}mm 过大, 六边形孔已消失"
        ));
    }
    let loop_pts: Vec<(f64, f64)> = outer_loop_cells(cols, rows)
        .iter()
        .map(|p| lattice_to_xy(*p, r))
        .collect();
    let outer = offset_polygon_outward(&loop_pts, thickness / 2.0);
    let holes: Vec<Vec<(f64, f64)>> = hex_cell_centers(cols, rows)
        .iter()
        .map(|(ci, cj)| {
            let (cx, cy) = lattice_to_xy((*ci, *cj), r);
            VERT_OFF
                .iter()
                .map(|(di, dj)| {
                    let (dx, dy) = lattice_to_xy((*di, *dj), hole_r);
                    (cx + dx, cy + dy)
                })
                .collect()
        })
        .collect();
    Ok((outer, holes))
}

fn polygon_area(pts: &[(f64, f64)]) -> f64 {
    let n = pts.len();
    let mut s = 0.0;
    for i in 0..n {
        let (x1, y1) = pts[i];
        let (x2, y2) = pts[(i + 1) % n];
        s += x1 * y2 - x2 * y1;
    }
    s.abs() / 2.0
}

/// 把整组轮廓的包围盒中心移到原点。
fn center_profile(mut polys: Vec<Vec<(f64, f64)>>) -> Vec<Vec<(f64, f64)>> {
    let (mut xmin, mut xmax, mut ymin, mut ymax) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for poly in &polys {
        for (x, y) in poly {
            xmin = xmin.min(*x);
            xmax = xmax.max(*x);
            ymin = ymin.min(*y);
            ymax = ymax.max(*y);
        }
    }
    let (cx, cy) = ((xmin + xmax) / 2.0, (ymin + ymax) / 2.0);
    for poly in &mut polys {
        for p in poly {
            *p = (p.0 - cx, p.1 - cy);
        }
    }
    polys
}

fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

// ── CATIA ──

pub fn run(params: &Value, progress: Progress<'_>) -> ComResult<Value> {
    let get = |k: &str, dflt: Value| params.get(k).cloned().unwrap_or(dflt);
    let cols = get("cols", json!(6)).as_i64().unwrap_or(6);
    let rows = get("rows", json!(6)).as_i64().unwrap_or(6);
    let d = get("diameter", json!(68.0)).as_f64().unwrap_or(68.0);
    let t = get("thickness", json!(2.0)).as_f64().unwrap_or(2.0);
    let height = get("height", json!(5.0)).as_f64().unwrap_or(5.0);
    let body_name = get("body_name", json!("Honeycomb_Rib"))
        .as_str()
        .unwrap_or("Honeycomb_Rib")
        .to_string();
    let _plane_key = get("plane", json!("xy"))
        .as_str()
        .unwrap_or("xy")
        .to_lowercase();
    let _offset = get("offset", json!(0.0)).as_f64().unwrap_or(0.0);
    let _center = get("center", json!(true)).as_bool().unwrap_or(true);
    let dry_run = get("dry_run", json!(false)).as_bool().unwrap_or(false);

    let mut empty_data = json!({
        "cols": cols, "rows": rows, "diameter": d, "thickness": t, "height": height,
        "cells": cols * rows, "body_name": body_name,
        "wall_area_mm2": Value::Null, "volume_mm3": Value::Null, "area_mm2": Value::Null,
    });

    if cols < 1 || rows < 1 {
        return Ok(json!({"success": false, "message": "行列数必须 >= 1", "data": empty_data}));
    }
    if d <= 0.0 || t <= 0.0 || height <= 0.0 {
        return Ok(
            json!({"success": false, "message": "直径/壁厚/筋高必须 > 0", "data": empty_data}),
        );
    }

    let (outer, holes) = match build_profile(cols, rows, d, t) {
        Ok(v) => v,
        Err(e) => {
            return Ok(
                json!({"success": false, "message": format!("轮廓计算失败: {e}"), "data": empty_data}),
            )
        }
    };
    let wall_area = polygon_area(&outer) - holes.iter().map(|h| polygon_area(h)).sum::<f64>();
    empty_data["wall_area_mm2"] = json!(round3(wall_area));

    if dry_run {
        empty_data["dry_run"] = json!(true);
        return Ok(json!({
            "success": true,
            "message": format!(
                "[DRY RUN] 蜂窝筋 {cols}x{rows} 格, 直径 {d}mm, 壁厚 {t}mm, 筋高 {height}mm; \
                 外轮廓 {} 点 + {} 孔, 筋板截面积 {wall_area:.1}mm², \
                 预计体积 {:.1}mm³",
                outer.len(), holes.len(), wall_area * height
            ),
            "data": empty_data,
        }));
    }

    // 非 dry_run：COM 流程在 run_catia 内重算轮廓（纯计算成本可忽略），保持单一入口。
    run_catia(params, progress)
}

/// 实际 CATIA 流程（把 center 平移后的轮廓画进草图再 Pad）。
fn run_catia(params: &Value, progress: Progress<'_>) -> ComResult<Value> {
    let get = |k: &str, dflt: Value| params.get(k).cloned().unwrap_or(dflt);
    let cols = get("cols", json!(6)).as_i64().unwrap_or(6);
    let rows = get("rows", json!(6)).as_i64().unwrap_or(6);
    let d = get("diameter", json!(68.0)).as_f64().unwrap_or(68.0);
    let t = get("thickness", json!(2.0)).as_f64().unwrap_or(2.0);
    let height = get("height", json!(5.0)).as_f64().unwrap_or(5.0);
    let body_name = get("body_name", json!("Honeycomb_Rib"))
        .as_str()
        .unwrap_or("Honeycomb_Rib")
        .to_string();
    let plane_key = get("plane", json!("xy"))
        .as_str()
        .unwrap_or("xy")
        .to_lowercase();
    let offset = get("offset", json!(0.0)).as_f64().unwrap_or(0.0);
    let center = get("center", json!(true)).as_bool().unwrap_or(true);

    let mut empty_data = json!({
        "cols": cols, "rows": rows, "diameter": d, "thickness": t, "height": height,
        "cells": cols * rows, "body_name": body_name,
        "wall_area_mm2": Value::Null, "volume_mm3": Value::Null, "area_mm2": Value::Null,
    });

    let (outer, holes) = match build_profile(cols, rows, d, t) {
        Ok(v) => v,
        Err(e) => {
            return Ok(
                json!({"success": false, "message": format!("轮廓计算失败: {e}"), "data": empty_data}),
            )
        }
    };
    let wall_area = polygon_area(&outer) - holes.iter().map(|h| polygon_area(h)).sum::<f64>();
    empty_data["wall_area_mm2"] = json!(round3(wall_area));

    let (outer, holes) = if center {
        let all: Vec<Vec<(f64, f64)>> = center_profile(
            std::iter::once(outer)
                .chain(holes)
                .collect::<Vec<Vec<(f64, f64)>>>(),
        );
        let mut it = all.into_iter();
        let outer = it.next().unwrap_or_default();
        let holes: Vec<Vec<(f64, f64)>> = it.collect();
        (outer, holes)
    } else {
        (outer, holes)
    };

    let app = attach_with_guard(progress)?;
    let alerts_prev = app.display_file_alerts().unwrap_or(true);
    let _ = app.set_display_file_alerts(false);

    let result = (|| -> ComResult<Value> {
        let doc = app
            .active_document()
            .map_err(|e| rscatiacli::ComError::runtime(format!("CATIA 无活动文档: {e}")))?;
        let part = doc.part_document().part().map_err(|_| {
            rscatiacli::ComError::runtime(
                "当前活动文档不是零件(.CATPart), 请先激活要操作的零件文档",
            )
        })?;
        let part_name = part.name().unwrap_or_default();
        empty_data["part_name"] = json!(part_name);

        // 幂等：同名 Body 先删掉，重复运行不叠加
        if let Ok(sel) = doc.selection() {
            let bodies = part.bodies()?;
            for b in bodies.items()?.into_iter().rev() {
                if b.name().unwrap_or_default() == body_name {
                    let _ = sel.clear();
                    let _ = sel.add(&b);
                    let _ = sel.delete();
                    let _ = sel.clear();
                }
            }
        }

        let body = part.bodies()?.add()?;
        body.set_name(&body_name)?;
        // Pad 必须建在实体 Body 里：InWorkObject 默认是几何图形集，不切会导致 AddNewPad 失败
        let _ = part.set_in_work_object(&body);

        let oe = part.origin_elements()?;
        let plane_entity = match plane_key.as_str() {
            "xy" => oe.plane_xy(),
            "yz" => oe.plane_yz(),
            "zx" => oe.plane_zx(),
            other => {
                return Ok(json!({
                    "success": false,
                    "message": format!("不支持的平面 {other}(可选 xy/yz/zx)"),
                    "data": empty_data,
                }))
            }
        }
        .map_err(|e| rscatiacli::ComError::runtime(format!("取基准平面失败: {e}")))?;

        let sketch_plane_ref = if offset != 0.0 {
            let plane_ref = part.create_reference_from_object(&plane_entity)?;
            let hsf = part.hybrid_shape_factory()?;
            let off_plane = hsf.add_new_plane_offset(&plane_ref, offset, false)?;
            let gs = part.hybrid_bodies()?.add()?;
            let _ = gs.set_name(&format!("{body_name}_Plane"));
            let _ = gs.append_hybrid_shape(&off_plane);
            let _ = part.update_object(&off_plane);
            part.create_reference_from_object(&off_plane)?
        } else {
            part.create_reference_from_object(&plane_entity)?
        };

        let sketch = body.sketches()?.add(&sketch_plane_ref)?;
        let f2d = sketch.open_edition()?;
        for poly in std::iter::once(&outer).chain(holes.iter()) {
            let n = poly.len();
            for j in 0..n {
                let (x1, y1) = poly[j];
                let (x2, y2) = poly[(j + 1) % n];
                f2d.create_line(x1, y1, x2, y2)?;
            }
        }
        let _ = sketch.close_edition();
        let _ = part.update_object(&sketch);

        let sf = part.shape_factory()?;
        let pad = sf.add_new_pad(&sketch, height)?;
        let _ = part.update_object(&pad);

        // 量测（尽力而为）
        let mut volume = None;
        let mut area = None;
        if let Ok(swb) = doc.part_document().spa_workbench() {
            if let Ok(body_ref) = part.create_reference_from_object(&body) {
                if let Ok(m) = swb.get_measurable(&body_ref) {
                    volume = m.volume().ok().map(round3);
                    area = m.area().ok().map(round3);
                }
            }
        }

        let mut msg = format!(
            "蜂窝筋完成: {cols}x{rows} 共 {} 格, 直径 {d}mm, 壁厚 {t}mm, 筋高 {height}mm\
             ;外轮廓 {} 点 + {} 孔;\
             新建 Body「{body_name}」",
            cols * rows,
            outer.len(),
            holes.len()
        );
        if let Some(v) = volume {
            msg.push_str(&format!(",体积 {v}mm³"));
        }
        msg.push_str(";请手动保存文档(Ctrl+S)");

        let mut data = empty_data;
        data["volume_mm3"] = json!(volume);
        data["area_mm2"] = json!(area);
        data["outer_points"] = json!(outer.len());
        data["holes"] = json!(holes.len());

        Ok(json!({
            "success": true,
            "message": msg,
            "data": data,
        }))
    })();

    let _ = app.set_display_file_alerts(alerts_prev);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outer_loop_unique_vertices_and_area() {
        let pts = outer_loop_cells(3, 3);
        // 3x3 蜂窝外轮廓：顶点唯一（闭环构造保证首尾相接）
        let set: std::collections::HashSet<_> = pts.iter().collect();
        assert_eq!(set.len(), pts.len(), "外轮廓顶点不应重复");
        assert!(pts.len() >= 12);
        // 面积为正且大于单孔面积
        let poly: Vec<(f64, f64)> = pts.iter().map(|p| lattice_to_xy(*p, 34.0)).collect();
        let hole = build_profile(3, 3, 68.0, 2.0).unwrap().1;
        assert!(polygon_area(&poly) > polygon_area(&hole[0]));
    }

    #[test]
    fn profile_counts_and_hole_no_overlap() {
        let (outer, holes) = build_profile(2, 2, 68.0, 2.0).unwrap();
        assert!(!outer.is_empty());
        assert_eq!(holes.len(), 4);
        // 孔面积和 < 外轮廓面积（孔互不重叠的必要条件）
        let outer_area = polygon_area(&outer);
        let holes_area: f64 = holes.iter().map(|h| polygon_area(h)).sum();
        assert!(holes_area < outer_area);
    }

    #[test]
    fn wall_thickness_too_big_errors() {
        assert!(build_profile(2, 2, 4.0, 10.0).is_err());
    }
}
