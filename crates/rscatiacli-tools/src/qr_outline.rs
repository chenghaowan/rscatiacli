//! 二维码轮廓生成: QR 矩阵 → **连通区域并集轮廓** → 草图图元(mm)。
//!
//! 对应 ltl-manage resources/sidecars/text_to_sketch/qr_outline.py。
//! 为什么要先做并集: CATIA 的 Pad 要求轮廓之间不相交 —— 逐块画方块时相邻黑块
//! 共边、对角相接的黑块共点, 草图里就成了"交叉/重合线"。这里把黑块按四连通合并
//! 成区域, 再沿格点追踪边界, 得到一组互不相交的闭合轮廓。
//!
//! 追踪规则:
//!   - 黑块四连通分块(只有对角相接算两块), 每块一条外轮廓;
//!   - 走法固定「黑块在左」, 外轮廓逆时针、洞顺时针;
//!   - 顶点分叉(对角相接的"掐点")按「优先同一黑块的出边」选边;
//!   - 自检: Σ 各轮廓有向面积 == 黑块个数(顺逆时针相消), 不成立即报错。
//!
//! QR 矩阵由 `qrcode` crate 生成(segno 的 Rust 等价物; 模式选择策略略有差异,
//! 个别内容的版本号可能不同, 扫码结果一致)。

use std::collections::{HashMap, HashSet};

use serde_json::json;
use serde_json::Map as JsonMap;
use serde_json::Value;

use crate::text_outline::{Segment, TextOutline, TextOutlineError};

// ── 常量 ─────────────────────────────────────────────────────────────

pub const EC_LEVELS: [&str; 4] = ["L", "M", "Q", "H"];
/// 单模块(黑点)尺寸下限: <0.6mm 蚀刻/雕刻后基本扫不出来(用户口径)。
pub const MIN_MODULE_MM: f64 = 0.6;
/// 标准要求二维码四周留 4 个模块的静区(白色), 生成时不含几何, 仅提示。
pub const QUIET_MODULES: i64 = 4;
/// 掐点分离量(单位=模块): 对角相接的黑块在格点处相碰, 实测这类共点会让 Pad 报错,
/// 故把该点各往自己那一块内部挪这个量(模块 0.8mm 时约 0.06mm 间隙, 不影响扫码)。
pub const PINCH_GAP_MODULE: f64 = 0.08;

// 方向: N=+Y, E=+X; 边界按"黑块在左"定向
const DIR_N: (i64, i64) = (0, 1);
const DIR_E: (i64, i64) = (1, 0);
const DIR_S: (i64, i64) = (0, -1);
const DIR_W: (i64, i64) = (-1, 0);

fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

fn round4(v: f64) -> f64 {
    (v * 10000.0).round() / 10000.0
}

fn terr(msg: impl Into<String>) -> TextOutlineError {
    TextOutlineError(msg.into())
}

type Cell = (i64, i64);
type Vertex = (i64, i64);
/// 原始轮廓: [(整格格点, 入方向, 出方向)]。
type RawLoop = Vec<(Vertex, u8, u8)>;
/// 输出轮廓: [(顶点(掐点可带小数位移), 入方向, 出方向)]。
type LoopPts = Vec<((f64, f64), u8, u8)>;

fn dir_of(d: u8) -> (i64, i64) {
    match d {
        0 => DIR_N,
        1 => DIR_E,
        2 => DIR_S,
        _ => DIR_W,
    }
}

/// 行进方向左转 90°(黑块在左手边, 即指向形状内部)。
fn rot90(d: u8) -> (i64, i64) {
    let (dx, dy) = dir_of(d);
    (-dy, dx)
}

/// 黑块集合 → (闭合轮廓, 掐点格点集合)。
/// 轮廓元素 = (格点, 入方向, 出方向); 顶点分叉(掐点)时优先沿同一个黑块继续,
/// 每条轮廓严格停在自己的四连通块内。
fn trace_loops(
    black: &HashSet<Cell>,
    rows: i64,
    cols: i64,
) -> (Vec<RawLoop>, HashSet<Vertex>) {
    // 出边表: 顶点 → [(方向, 下一顶点, 黑块)]
    let mut out_edges: HashMap<Vertex, Vec<(u8, Vertex, Cell)>> = HashMap::new();
    let add_edge =
        |m: &mut HashMap<Vertex, Vec<(u8, Vertex, Cell)>>, v: Vertex, d: u8, cell: Cell| {
            let (dx, dy) = dir_of(d);
            m.entry(v)
                .or_default()
                .push((d, (v.0 + dx, v.1 + dy), cell));
        };

    for &(r, c) in black {
        let y = rows - 1 - r; // 矩阵行号 → 草图 Y(第 0 行在最上 → Y 最大)
        if !black.contains(&(r + 1, c)) {
            add_edge(&mut out_edges, (c, y), 1, (r, c)); // E
        }
        if !black.contains(&(r, c + 1)) {
            add_edge(&mut out_edges, (c + 1, y), 0, (r, c)); // N
        }
        if !black.contains(&(r - 1, c)) {
            add_edge(&mut out_edges, (c + 1, y + 1), 3, (r, c)); // W
        }
        if !black.contains(&(r, c - 1)) {
            add_edge(&mut out_edges, (c, y + 1), 2, (r, c)); // S
        }
    }

    let saddles: HashSet<Vertex> = out_edges
        .iter()
        .filter(|(_, edges)| {
            edges
                .iter()
                .map(|(d, _, _)| *d)
                .collect::<HashSet<_>>()
                .len()
                > 1
        })
        .map(|(v, _)| *v)
        .collect();

    let mut loops: Vec<Vec<(Vertex, u8, u8)>> = Vec::new();
    let mut used: HashSet<(Vertex, u8)> = HashSet::new();
    let mut starts: Vec<(Vertex, u8, Cell)> = out_edges
        .iter()
        .flat_map(|(v, es)| es.iter().map(move |(d, _, cl)| (*v, *d, *cl)))
        .collect();
    starts.sort();
    for (v0, d0, cell0) in starts {
        if used.contains(&(v0, d0)) {
            continue;
        }
        let mut pts: Vec<(Vertex, u8, u8)> = Vec::new(); // (v, 入方向, 出方向)
        let (mut v, mut d, mut cell) = (v0, d0, cell0);
        let mut din: u8 = 255; // 到达当前点时的行进方向(起点闭合后补)
        while !used.contains(&(v, d)) {
            used.insert((v, d));
            let nv = out_edges[&v]
                .iter()
                .find(|(cd, _, _)| *cd == d)
                .map(|(_, n, _)| *n);
            let Some(nv) = nv else { break };
            // 分叉(掐点)时优先沿同一个黑块继续 → 轮廓不外窜
            let nxt = out_edges
                .get(&nv)
                .map(|cands| {
                    cands
                        .iter()
                        .filter(|(cd, _, _)| !used.contains(&(nv, *cd)))
                        .find(|(_, _, cl)| *cl == cell)
                        .or_else(|| cands.iter().find(|(cd, _, _)| !used.contains(&(nv, *cd))))
                })
                .unwrap_or(None)
                .map(|(cd, _, cl)| (*cd, *cl));
            pts.push((v, din, d));
            match nxt {
                None => {
                    // 闭合: 起点入方向 = 最后一条边方向
                    if let Some(first) = pts.first_mut() {
                        first.1 = d;
                    }
                    break;
                }
                Some((nd, ncell)) => {
                    din = d;
                    v = nv;
                    d = nd;
                    cell = ncell;
                }
            }
        }
        if pts.len() >= 3 && pts.iter().all(|(_, din, _)| *din != 255) {
            loops.push(pts);
        }
    }
    let _ = cols;
    (loops, saddles)
}

fn signed_area(pts: &[(f64, f64)]) -> f64 {
    let n = pts.len();
    let mut s = 0.0f64;
    for i in 0..n {
        let (x1, y1) = pts[i];
        let (x2, y2) = pts[(i + 1) % n];
        s += x1 * y2 - x2 * y1;
    }
    s / 2.0
}

// ── QR 矩阵(qrcode crate) ───────────────────────────────────────────

fn qr_matrix(content: &str, level: &str) -> Result<(u16, Vec<Vec<bool>>), TextOutlineError> {
    use qrcode::{EcLevel, QrCode};
    let ec = match level {
        "L" => EcLevel::L,
        "Q" => EcLevel::Q,
        "H" => EcLevel::H,
        _ => EcLevel::M,
    };
    let code = QrCode::with_error_correction_level(content.as_bytes(), ec).map_err(|e| {
        terr(format!(
            "生成二维码失败: {e:?} —— 内容太长时可缩短内容或把纠错等级降到 L"
        ))
    })?;
    let cols = code.width() as i64;
    let version = ((cols - 17) / 4) as u16;
    let colors = code.to_colors();
    let mut matrix = vec![vec![false; cols as usize]; cols as usize];
    for (i, color) in colors.iter().enumerate() {
        let r = i / cols as usize;
        let c = i % cols as usize;
        matrix[r][c] = matches!(color, qrcode::types::Color::Dark);
    }
    Ok((version, matrix))
}

/// 只算版本与模块数(不做轮廓追踪) —— 供前端实时提示"当前内容需要的最小边长"。
pub fn qr_metrics(content: &str, ec: &str) -> Result<(u16, i64), TextOutlineError> {
    let text = content.trim();
    if text.is_empty() {
        return Err(terr("二维码内容为空"));
    }
    let level = ec_or_default(ec)?;
    let (version, matrix) = qr_matrix(text, &level)?;
    Ok((version, matrix.len() as i64))
}

fn ec_or_default(ec: &str) -> Result<String, TextOutlineError> {
    let level = ec.trim().to_uppercase();
    let level = if level.is_empty() {
        "M".to_string()
    } else {
        level.chars().next().unwrap_or('M').to_string()
    };
    if !EC_LEVELS.contains(&level.as_str()) {
        return Err(terr(format!("纠错等级只能是 L/M/Q/H: {ec:?}")));
    }
    Ok(level)
}

/// 二维码 → 草图轮廓几何(mm)。
///
/// 尺寸口径: 给的是**单个模块(方格)的边长**, 整体尺寸 = 模块数 × 单模块边长。
/// 刻字场景真正要控的是"一格够不够大"(≥0.6mm 才好扫), 整体尺寸是派生结果。
pub fn build_qr_outline(
    content: &str,
    module_mm: f64,
    ec: &str,
) -> Result<TextOutline, TextOutlineError> {
    let text = content.trim();
    if text.is_empty() {
        return Err(terr("二维码内容为空"));
    }
    if !(0.05..=50.0).contains(&module_mm) {
        return Err(terr(format!("单模块边长需在 0.05~50mm 之间: {module_mm}")));
    }
    let level = ec_or_default(ec)?;

    let (version, matrix) = qr_matrix(text, &level)?;
    let rows = matrix.len() as i64;
    let cols = if rows > 0 { matrix[0].len() as i64 } else { 0 };
    if rows == 0 || cols == 0 {
        return Err(terr("二维码矩阵为空"));
    }
    let mut black: HashSet<Cell> = HashSet::new();
    for (r, row) in matrix.iter().enumerate() {
        for (c, &v) in row.iter().enumerate() {
            if v {
                black.insert((r as i64, c as i64));
            }
        }
    }
    if black.is_empty() {
        return Err(terr("二维码矩阵全白(内容可能只有空白字符)"));
    }

    let (raw_loops, saddles) = trace_loops(&black, rows, cols);
    // 自检: 有向面积之和 == 黑块数(外轮廓正、洞为负, 正好相消)
    let area_sum: f64 = raw_loops
        .iter()
        .map(|loop_pts| {
            signed_area(
                &loop_pts
                    .iter()
                    .map(|(v, _, _)| (v.0 as f64, v.1 as f64))
                    .collect::<Vec<_>>(),
            )
        })
        .sum();
    if (area_sum - black.len() as f64).abs() > 1e-6 {
        let shown: String = text.chars().take(40).collect();
        return Err(terr(format!(
            "二维码轮廓追踪自检失败(面积 {area_sum} != 黑块 {}), 请反馈这个内容: {shown:?}",
            black.len()
        )));
    }

    // 掐点(对角相接处)各往自己那一块挪一点 → 轮廓之间既不共边也不共点。
    // 顶点用整数格点×1e6 定点存放位移, 输出时再除回去。
    let gap_units = PINCH_GAP_MODULE;
    let mut loops: Vec<LoopPts> = Vec::new();
    for loop_pts in &raw_loops {
        let mut pts = Vec::with_capacity(loop_pts.len());
        for (v, din, dout) in loop_pts {
            let mut vx = v.0 as f64;
            let mut vy = v.1 as f64;
            if saddles.contains(v) {
                let a = rot90(*din);
                let b = rot90(*dout);
                let (ax, ay) = (a.0 + b.0, a.1 + b.1);
                let norm = ((ax * ax + ay * ay) as f64).sqrt();
                if norm > 0.0 {
                    vx += ax as f64 / norm * gap_units;
                    vy += ay as f64 / norm * gap_units;
                }
            }
            pts.push(((vx, vy), *din, *dout));
        }
        loops.push(pts);
    }

    let mut out = TextOutline::new_text();
    out.kind = "qr";
    let mut pinched = 0usize;
    for loop_pts in &loops {
        // 去掉直穿点(入方向 == 出方向)
        let kept: Vec<&((f64, f64), u8, u8)> = loop_pts
            .iter()
            .filter(|(_, din, dout)| din != dout)
            .collect();
        let pts: Vec<&((f64, f64), u8, u8)> = if kept.len() >= 3 {
            kept
        } else {
            loop_pts.iter().collect()
        };
        // 自触检查: 同一(取整)格点出现两次 → 该轮廓自相碰(已被掐点分离则不计)
        let mut seen: HashSet<(i64, i64)> = HashSet::new();
        let mut dup = false;
        for ((vx, vy), _, _) in &pts {
            let key = ((vx * 1e6).round() as i64, (vy * 1e6).round() as i64);
            if !seen.insert(key) {
                dup = true;
            }
        }
        if dup {
            pinched += 1;
        }
        let n = pts.len();
        for i in 0..n {
            let (x1, y1) = pts[i].0;
            let (x2, y2) = pts[(i + 1) % n].0;
            out.segments.push(Segment::Line(
                (x1 * module_mm, y1 * module_mm),
                (x2 * module_mm, y2 * module_mm),
            ));
            out.n_lines += 1;
        }
        out.n_contours += 1;
    }

    out.width = cols as f64 * module_mm;
    out.height = rows as f64 * module_mm;
    let too_small = module_mm < MIN_MODULE_MM - 1e-9;
    let warning = if too_small {
        Some(format!(
            "单模块边长 {module_mm:.3}mm < {MIN_MODULE_MM}mm(识别下限), 刻出来不好扫 —— 建议改用 ≥ {MIN_MODULE_MM}mm/格, 或把纠错等级降到 L(同内容模块更少); 当前 {cols}×{rows} 模块"
        ))
    } else {
        None
    };
    let mut extra: JsonMap<String, Value> = JsonMap::new();
    extra.insert("kind".into(), json!("qr"));
    extra.insert("version".into(), json!(version));
    extra.insert("ec".into(), json!(level));
    extra.insert("modules".into(), json!(format!("{cols}×{rows}")));
    extra.insert("module_count".into(), json!(cols));
    extra.insert("module_mm".into(), json!(round4(module_mm)));
    extra.insert("size_mm".into(), json!(round3(cols as f64 * module_mm)));
    extra.insert("min_module_mm".into(), json!(MIN_MODULE_MM));
    extra.insert(
        "quiet_zone_mm".into(),
        json!(round3(module_mm * QUIET_MODULES as f64)),
    );
    extra.insert("pinch_points".into(), json!(saddles.len()));
    extra.insert("self_touching".into(), json!(pinched));
    extra.insert(
        "warning".into(),
        warning.map(Value::String).unwrap_or(Value::Null),
    );
    extra.insert(
        "note".into(),
        json!(format!(
            "生成的是黑白模块几何(不含四周静区): 外围请留 ≥ {:.1}mm 空位不雕刻",
            module_mm * QUIET_MODULES as f64
        )),
    );
    out.extra = extra;
    Ok(out)
}

// ── 测试 ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qr_metrics_basic() {
        let (version, cols) = qr_metrics("HELLO", "M").unwrap();
        assert!(version >= 1 && cols >= 21, "v{version} {cols}×{cols}");
        let empty = qr_metrics("  ", "M");
        assert!(empty.is_err());
        let bad_ec = qr_metrics("HELLO", "X");
        assert!(bad_ec.is_err());
    }

    #[test]
    fn qr_outline_geometry() {
        let out = build_qr_outline("https://forvia.example/abc", 0.8, "M").unwrap();
        assert_eq!(out.kind, "qr");
        assert!(
            out.width > 0.0 && (out.height - out.width).abs() < 1e-6,
            "QR 应为正方形"
        );
        assert!(out.n_contours >= 10, "至少有定位角等轮廓");
        // 自检已保证: Σ有向面积 == 黑块数(在 build 内部断言)
        // 全部为直线段
        assert!(out.segments.iter().all(|s| matches!(s, Segment::Line(..))));
        // 单模块过小提示
        let small = build_qr_outline("abc", 0.1, "L").unwrap();
        assert!(small
            .extra
            .get("warning")
            .and_then(|w| w.as_str())
            .is_some());
    }
}
