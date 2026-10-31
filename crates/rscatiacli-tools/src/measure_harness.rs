//! measure-harness：线束测长（按 measureharness 集合内曲面 L = 2×面积/边界周长）。
//!
//! 对应 ltl-manage resources/sidecars/measure_harness/measure_harness.py。
//! action=measure(缺省): 测集合内全部曲面(跳过平面), 面积×周长 → 等效长度,
//! 可选边界环分析(环数/环长/截面直径, 只影响判定标签); annotate3d: FTA 文字
//! 注解(MH_LEN_* 幂等, 注解集整删连带清 UserSurface); screenshot: 色标点放曲面
//! COG 截图定位 → 删点重截干净底图 → 白底化/裁边/擦罗盘/方形管线 → 图例+引线
//! 标注(WebP); annotate_all: 清旧注解+换色+截图标注+3D 注解全流水线。
//!
//! 注: Python 版的 _DialogAutoDismiss 弹窗自动关闭线程在当前 Python 里已无
//! 调用点(死代码), 未移植。

use std::path::{Path, PathBuf};

use base64::Engine;
use rscatiacli::objects::{
    Annotation, Application, Entity, HybridBody, Part, PartDocument, Reference, SPAWorkbench,
    Selection, SystemService, UserSurface, Viewer3D,
};
use rscatiacli::{attach_with_guard, ComCollection, ComObject, ComResult, Progress};
use serde_json::{json, Map, Value};

use crate::doc_link::ensure_part_doc;
use crate::measure_capture::{
    draw_annotations, find_color_hits, load_font, process_capture, Marker, COLOR_TOL, SHOT_MAX_SIZE,
};
use crate::surface_common::{com_type_name, is_plane, iter_geosets};
use crate::vba_bbox_generate::command_selection;

// ── 常量 ─────────────────────────────────────────────────────────────

/// 约定的集合图文集名(大小写不敏感)。
const DEFAULT_GEOSET_NAME: &str = "measureharness";
/// 边界环分析/截图色标临时集合(用完即删)。
const TMP_GEOSET_NAME: &str = "_measure_harness_tmp";
/// 两环长度差 <2% 视为等截面。
const LOOP_EQUAL_TOL: f64 = 0.02;
/// 3D 标注命名前缀(幂等清理依据)。
const ANN_NAME_PREFIX: &str = "MH_LEN_";
/// 注解集标准(Add 参数)。
const ANN_STANDARD: &str = "ISO";
/// 标注配色(每张曲面一色, 截图标注用)。
const LABEL_COLORS: [[u8; 3]; 12] = [
    [230, 0, 230],
    [0, 145, 255],
    [0, 175, 80],
    [255, 140, 0],
    [0, 190, 190],
    [200, 0, 0],
    [150, 75, 0],
    [90, 0, 180],
    [0, 90, 160],
    [230, 120, 60],
    [60, 120, 0],
    [180, 0, 120],
];

/// 截图标注结果（annotate_screenshot 的 JSON 对象）。
type ShotOut = Map<String, Value>;

fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

/// 本地时间戳(文件名用; GetLocalTime)。
fn local_timestamp() -> String {
    use windows::Win32::System::SystemInformation::GetLocalTime;
    let st = unsafe { GetLocalTime() };
    format!(
        "{:04}{:02}{:02}_{:02}{:02}{:02}",
        st.wYear, st.wMonth, st.wDay, st.wHour, st.wMinute, st.wSecond
    )
}

// ── 行结构 ───────────────────────────────────────────────────────────

/// 一张曲面的测量行（内部引用不进 JSON）。
#[derive(Default)]
struct SurfRow {
    name: String,
    shape_type: String,
    area_mm2: Option<f64>,
    perimeter_mm: Option<f64>,
    length_mm: Option<f64>,
    loop_count: Option<i64>,
    loop_lengths_mm: Vec<f64>,
    loop_diameters_mm: Vec<f64>,
    status: Option<String>,
    reason: Option<String>,
    shape: Option<Entity>,
    reference: Option<Reference>,
    color: Option<[u8; 3]>,
}

impl SurfRow {
    fn to_json(&self) -> Value {
        json!({
            "name": self.name,
            "shape_type": self.shape_type,
            "area_mm2": self.area_mm2,
            "perimeter_mm": self.perimeter_mm,
            "length_mm": self.length_mm,
            "loop_count": self.loop_count,
            "loop_lengths_mm": self.loop_lengths_mm,
            "loop_diameters_mm": self.loop_diameters_mm,
            "status": self.status,
            "reason": self.reason,
        })
    }
}

// ── 几何图形集查找 ───────────────────────────────────────────────────

fn find_geoset<'a>(all: &'a [HybridBody], name: &str) -> Option<(&'a HybridBody, String)> {
    let want = name.trim().to_lowercase();
    for hb in all {
        let actual = hb.name().unwrap_or_default();
        if actual.trim().to_lowercase() == want {
            return Some((hb, actual));
        }
    }
    None
}

// ── 测量核心 ─────────────────────────────────────────────────────────

/// 拆曲面边界环: AddNewBoundaryOfSurface → AddNewDatums 按域拆 datum 曲线,
/// datum 数量 = 边界环数(与缝合曲面的环数口径一致)。
fn analyze_loops(
    part: &Part,
    hsf: &rscatiacli::objects::HybridShapeFactory,
    spa: &SPAWorkbench,
    tmp_body: &HybridBody,
    surf_ref: &Reference,
) -> Result<(i64, Vec<f64>), String> {
    let bnd = hsf
        .add_new_boundary_of_surface(surf_ref)
        .map_err(|e| e.to_string())?;
    let _ = tmp_body.append_hybrid_shape(&bnd);
    if part.update_object(&bnd).is_err() {
        let _ = part.update();
    }
    let bnd_ref = part
        .create_reference_from_object(&bnd)
        .map_err(|e| e.to_string())?;
    let datums = hsf.add_new_datums(&bnd_ref).map_err(|e| e.to_string())?;
    let mut lengths = Vec::new();
    for d in &datums {
        if let Ok(dref) = part.create_reference_from_object(d) {
            if let Ok(m) = spa.get_measurable(&dref) {
                if let Ok(v) = m.length() {
                    lengths.push(round3(v));
                }
            }
        }
    }
    Ok((datums.len() as i64, lengths))
}

/// 按环数与环长差给出曲面类型说明。
fn classify(loop_count: Option<i64>, loop_lengths: &[f64]) -> String {
    let Some(count) = loop_count else {
        return "未分析(参考值)".to_string();
    };
    if count >= 3 {
        return "合并过渡面(等效长度)".to_string();
    }
    if count == 2 {
        if loop_lengths.len() == 2 {
            let min = loop_lengths.iter().cloned().fold(f64::INFINITY, f64::min);
            let max = loop_lengths
                .iter()
                .cloned()
                .fold(f64::NEG_INFINITY, f64::max);
            if min > 0.0 && (max - min) / min < LOOP_EQUAL_TOL {
                return "等截面(精确)".to_string();
            }
            return "变截面(等效长度)".to_string();
        }
        return "等效长度".to_string();
    }
    "边界不完整(参考值)".to_string()
}

fn clip_err(msg: &str) -> String {
    msg.chars().take(66).collect()
}

/// 测量集合内全部曲面, 返回 (surfaces_rows, skipped_rows)。
fn measure_surfaces(
    part: &Part,
    spa: &SPAWorkbench,
    hsf: &rscatiacli::objects::HybridShapeFactory,
    geoset: &HybridBody,
    analyze: bool,
    tmp_body: Option<&HybridBody>,
    progress: Progress<'_>,
) -> (Vec<SurfRow>, Vec<SurfRow>) {
    let mut items: Vec<(String, Entity, String)> = Vec::new();
    if let Ok(shapes) = geoset.hybrid_shapes() {
        if let Ok(list) = shapes.items() {
            for (i, sh) in list.iter().enumerate() {
                if is_plane(sh.com()) {
                    continue; // 平面(基准平面等)不测
                }
                let nm = sh.name().unwrap_or_else(|_| format!("shape_{}", i + 1));
                items.push((nm, sh.clone(), com_type_name(sh.com())));
            }
        }
    }

    let mut surfaces: Vec<SurfRow> = Vec::new();
    let mut skipped: Vec<SurfRow> = Vec::new();
    for (name, shape, type_name) in &items {
        let mut row = SurfRow {
            name: name.clone(),
            shape_type: type_name.clone(),
            shape: Some(shape.clone()),
            ..Default::default()
        };
        let reference = match part.create_reference_from_object(shape) {
            Ok(r) => r,
            Err(e) => {
                row.reason = Some(clip_err(&format!("建引用失败: {e}")));
                progress(&format!(
                    "  ✗ {name}: {}",
                    row.reason.clone().unwrap_or_default()
                ));
                skipped.push(row);
                continue;
            }
        };
        row.reference = Some(reference.clone());
        let Ok(m) = spa.get_measurable(&reference) else {
            row.reason = Some("无法创建测量对象".to_string());
            skipped.push(row);
            continue;
        };

        // 面积(m² → mm²)
        let area_m2 = match m.area() {
            Ok(a) if a > 0.0 => a,
            Ok(_) => {
                row.reason = Some("无法测面积(非曲面?): 面积非正".to_string());
                progress(&format!(
                    "  ✗ {name}: {}",
                    row.reason.clone().unwrap_or_default()
                ));
                skipped.push(row);
                continue;
            }
            Err(e) => {
                row.reason = Some(clip_err(&format!("无法测面积(非曲面?): {e}")));
                progress(&format!(
                    "  ✗ {name}: {}",
                    row.reason.clone().unwrap_or_default()
                ));
                skipped.push(row);
                continue;
            }
        };
        row.area_mm2 = Some(round3(area_m2 * 1e6));

        // 边界周长(mm) = 全部边界环长度之和
        match m.perimeter() {
            Ok(p) if p > 1e-9 => row.perimeter_mm = Some(round3(p)),
            Ok(_) => {
                row.reason = Some("无法测边界周长(闭合曲面?): 周长为 0".to_string());
                progress(&format!(
                    "  ✗ {name}: {}",
                    row.reason.clone().unwrap_or_default()
                ));
                skipped.push(row);
                continue;
            }
            Err(e) => {
                row.reason = Some(clip_err(&format!("无法测边界周长(闭合曲面?): {e}")));
                progress(&format!(
                    "  ✗ {name}: {}",
                    row.reason.clone().unwrap_or_default()
                ));
                skipped.push(row);
                continue;
            }
        }

        // L = 2A/P
        let length_mm = 2.0 * (area_m2 * 1e6) / row.perimeter_mm.unwrap_or(1.0);
        row.length_mm = Some(round3(length_mm));

        // 环分析(可选): 只影响判定标签与截面直径展示, 不影响长度值
        if analyze {
            if let Some(tmp) = tmp_body {
                match analyze_loops(part, hsf, spa, tmp, &reference) {
                    Ok((count, lengths)) => {
                        row.loop_count = Some(count);
                        row.loop_lengths_mm = lengths.clone();
                        row.loop_diameters_mm = lengths
                            .iter()
                            .map(|v| (v / std::f64::consts::PI * 100.0).round() / 100.0)
                            .collect();
                    }
                    Err(e) => progress(&format!(
                        "  ⚠ {name}: 环分析失败({}), 按未分析处理",
                        e.chars().take(60).collect::<String>()
                    )),
                }
            }
        }
        row.status = Some(classify(row.loop_count, &row.loop_lengths_mm));
        let loops_txt = if row.loop_diameters_mm.is_empty() {
            String::new()
        } else {
            format!(", 截面直径 {:?} mm", row.loop_diameters_mm)
        };
        progress(&format!(
            "  ✅ {name}: 面积 {} mm², 周长 {} mm({} 环, {}{loops_txt}), 长度 {} mm",
            row.area_mm2.unwrap_or(0.0),
            row.perimeter_mm.unwrap_or(0.0),
            row.loop_count
                .map(|c| c.to_string())
                .unwrap_or_else(|| "?".into()),
            row.status.clone().unwrap_or_default(),
            row.length_mm.unwrap_or(0.0),
        ));
        surfaces.push(row);
    }
    (surfaces, skipped)
}

/// 删除临时几何图形集（幂等 best-effort）。
fn delete_body(cmd_sel: &Selection, part: &Part, body: &HybridBody) {
    let _ = cmd_sel.clear();
    if cmd_sel.add(body).is_ok() {
        let _ = cmd_sel.delete();
    }
    let _ = cmd_sel.clear();
    let _ = part.update();
}

// ── 3D 标注 ──────────────────────────────────────────────────────────

/// 删除本工具此前建的 MH_LEN_* 注解(幂等), 返回删除数。
/// 注解集里全是我们的注解时整删注解集(连带清 UserSurface, 否则孤儿引用会让
/// 后续 update 反复弹「Error Copying Data」); 有用户注解时逐个删我们的。
fn clear_len_annotations(part: &Part, cmd_sel: &Selection, progress: Progress<'_>) -> usize {
    let Ok(sets) = part.annotation_sets() else {
        return 0;
    };
    let Ok(count) = sets.count() else {
        return 0;
    };
    if count == 0 {
        return 0;
    }
    let Ok(s1) = sets.item(1) else {
        return 0;
    };
    let Ok(anns) = s1.annotations() else {
        return 0;
    };
    let Ok(total) = anns.count() else {
        return 0;
    };
    let mut ours: Vec<Annotation> = Vec::new();
    for i in 1..=total {
        if let Ok(a) = anns.item(i) {
            if a.name().unwrap_or_default().starts_with(ANN_NAME_PREFIX) {
                ours.push(a);
            }
        }
    }
    if ours.is_empty() {
        return 0;
    }
    if total as usize == ours.len() {
        // 整个注解集都是本工具建的: 整删(连带清掉 UserSurface)
        let _ = cmd_sel.clear();
        if cmd_sel.add(&s1).is_ok() {
            let _ = cmd_sel.delete();
        }
        let _ = cmd_sel.clear();
        let _ = part.update();
        return ours.len();
    }
    let mut removed = 0;
    for a in &ours {
        let _ = cmd_sel.clear();
        if cmd_sel.add(a).is_ok() {
            let _ = cmd_sel.delete();
            removed += 1;
        }
    }
    let _ = cmd_sel.clear();
    let _ = part.update();
    let _ = progress;
    removed
}

/// 按 MH_US_<idx> 找/建 UserSurface（跨运行复用, 防无限累积）。
fn get_or_create_user_surface(
    us_raw: &rscatiacli::objects::UserSurfaces,
    row: &SurfRow,
    idx: usize,
) -> Result<UserSurface, String> {
    let want = format!("MH_US_{idx}");
    if let Ok(items) = us_raw.items() {
        for u in items.iter() {
            if u.name().unwrap_or_default() == want {
                return Ok(u.clone());
            }
        }
    }
    let reference = row
        .reference
        .as_ref()
        .ok_or_else(|| "无曲面引用".to_string())?;
    let us = us_raw.generate(reference).map_err(|e| e.to_string())?;
    let _ = us.set_name(&want);
    Ok(us)
}

/// 在注解集里为每张曲面建 FTA 文字注解(内容 = 长度)。
fn create_len_annotations(
    part: &Part,
    surf_rows: &[SurfRow],
    progress: Progress<'_>,
) -> Result<Map<String, Value>, String> {
    let sets = part.annotation_sets().map_err(|e| e.to_string())?;
    let mut created_set = false;
    let s1 = if sets.count().map_err(|e| e.to_string())? == 0 {
        created_set = true;
        let s = sets.add(ANN_STANDARD).map_err(|e| e.to_string())?;
        progress(&format!("已创建注解集(标准 {ANN_STANDARD})"));
        s
    } else {
        sets.item(1).map_err(|e| e.to_string())?
    };

    let us_raw = part.user_surfaces().map_err(|e| e.to_string())?;
    let factory = s1.annotation_factory().map_err(|e| e.to_string())?;
    let mut made = 0usize;
    for (idx, row) in surf_rows.iter().enumerate() {
        if row.length_mm.is_none() || row.reference.is_none() {
            continue;
        }
        let make = |us: &UserSurface| -> Result<(), String> {
            let ann = factory.create_text(us).map_err(|e| e.to_string())?;
            let t = ann.text_object().map_err(|e| e.to_string())?;
            t.set_text(&format!("{:.1} mm", row.length_mm.unwrap_or(0.0)))
                .map_err(|e| e.to_string())?;
            let _ = ann.set_xy(0.0, 0.0);
            let _ = ann.set_name(&format!("{ANN_NAME_PREFIX}{}", idx + 1));
            Ok(())
        };
        let attempt = (|| {
            let us = get_or_create_user_surface(&us_raw, row, idx + 1)?;
            make(&us)
        })();
        match attempt {
            Ok(()) => {
                made += 1;
                progress(&format!(
                    "  ✅ {}: 注解 {:.1} mm 已创建",
                    row.name,
                    row.length_mm.unwrap_or(0.0)
                ));
            }
            Err(_) => {
                // 复用的 UserSurface 引用可能已失效(曲面被重建过), 兜底新建一个
                let fallback = (|| {
                    let us = us_raw
                        .generate(row.reference.as_ref().unwrap())
                        .map_err(|e| e.to_string())?;
                    make(&us)
                })();
                match fallback {
                    Ok(()) => {
                        made += 1;
                        progress(&format!(
                            "  ✅ {}: 注解 {:.1} mm 已创建(新建锚点)",
                            row.name,
                            row.length_mm.unwrap_or(0.0)
                        ));
                    }
                    Err(e) => progress(&format!(
                        "  ✗ {}: 3D 标注失败: {}",
                        row.name,
                        e.chars().take(80).collect::<String>()
                    )),
                }
            }
        }
    }
    let _ = part.update();
    let _ = s1.set_switch_on(true); // 确保注解显示打开
    let mut m = Map::new();
    m.insert("created".into(), json!(made));
    m.insert("created_set".into(), json!(created_set));
    m.insert(
        "set_name".into(),
        json!(s1.name().unwrap_or_else(|_| "Annotation Set".to_string())),
    );
    Ok(m)
}

// ── 曲面随机换色 ─────────────────────────────────────────────────────

/// 生成 n 个明亮且互相分开的随机色(随机起点 + 金角间隔铺色相)。
fn random_palette(n: usize) -> Vec<[u8; 3]> {
    // 简易 LCG(毫秒时钟播种), 对齐 Python 的「随机但分散」口径
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_millis() as u64 ^ d.as_secs())
        .unwrap_or(42);
    let mut state = seed | 1;
    let mut next = |min: f64, max: f64| {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let unit = ((state >> 33) as f64) / ((1u64 << 31) as f64);
        min + unit * (max - min)
    };
    let start = next(0.0, 360.0);
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let hue = (start + i as f64 * 137.508 + next(-14.0, 14.0)) % 360.0;
        let s = next(0.72, 0.9);
        let v = next(0.9, 1.0);
        let (r, g, b) = hsv_to_rgb(hue / 360.0, s, v);
        out.push([(r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8]);
    }
    out
}

fn hsv_to_rgb(h: f64, s: f64, v: f64) -> (f64, f64, f64) {
    let i = (h * 6.0).floor() as i64;
    let f = h * 6.0 - i as f64;
    let p = v * (1.0 - s);
    let q = v * (1.0 - f * s);
    let t = v * (1.0 - (1.0 - f) * s);
    match (i.rem_euclid(6), v, p, q, t) {
        (0, v, _, _, t) => (v, t, p),
        (1, v, _, q, _) => (q, v, p),
        (2, v, p, _, _) => (p, v, t),
        (3, _, p, q, _) => (p, q, v),
        (4, _, p, _, t) => (t, p, v),
        _ => (v, p, q),
    }
}

/// 给测量的曲面逐个上随机色(与截图标注的引线/图例同色), 返回上色数。
fn color_surfaces(
    cmd_sel: &Selection,
    surf_rows: &mut [SurfRow],
    colors: &[[u8; 3]],
    progress: Progress<'_>,
) -> usize {
    let mut n = 0;
    for (row, rgb) in surf_rows.iter_mut().zip(colors) {
        if row.length_mm.is_none() || row.shape.is_none() {
            continue;
        }
        let _ = cmd_sel.clear();
        if let Some(shape) = &row.shape {
            if cmd_sel.add(shape).is_ok() {
                if let Ok(vp) = cmd_sel.vis_properties() {
                    let _ = vp.set_real_color(rgb[0] as i32, rgb[1] as i32, rgb[2] as i32, 0);
                }
                n += 1;
                row.color = Some(*rgb);
                progress(&format!("  🎨 {}: 换色 RGB{rgb:?}", row.name));
            }
        }
    }
    let _ = cmd_sel.clear();
    n
}

// ── 截图标注 ─────────────────────────────────────────────────────────

fn capture_bmp(viewer: &Viewer3D, path: &Path) -> Result<image::RgbImage, String> {
    let _ = viewer.update();
    viewer
        .capture_to_file(4, &path.to_string_lossy())
        .map_err(|e| format!("CaptureToFile 失败: {e}"))?;
    let meta = std::fs::metadata(path).map_err(|_| "CaptureToFile 失败".to_string())?;
    if meta.len() == 0 {
        return Err("CaptureToFile 失败".to_string());
    }
    image::open(path)
        .map(|d| d.to_rgb8())
        .map_err(|e| format!("读取截图失败: {e}"))
}

/// 截图并标注每段长度。定位原理: 色标点直接放曲面重心 COG 上(点标记不做深度
/// 测试, 屏幕位置就是 COG 的真实投影); 找色点 → 删点重截干净底图 → 同一管线
/// 处理 → 按跟踪的坐标变换把色标位置映射到最终图上画引线。
#[allow(clippy::too_many_arguments)]
fn annotate_screenshot(
    app: &Application,
    part: &Part,
    spa: &SPAWorkbench,
    ss: &SystemService,
    cmd_sel: &Selection,
    surf_rows: &mut [SurfRow],
    total_mm: f64,
    out_base: &Path,
) -> Result<Map<String, Value>, String> {
    let window = app.active_window().map_err(|e| e.to_string())?;
    let viewer = window.active_viewer().map_err(|e| e.to_string())?;
    let hsf = part.hybrid_shape_factory().map_err(|e| e.to_string())?;
    let bmp_path = out_base.with_extension("bmp");

    // 1. 每张曲面一个色标点, 放在 COG 上; 检测色用管色的互补色(高对比:
    //    色标叠在彩色管子上, 同色系会把质心拉偏)
    let tmp = part
        .hybrid_bodies()
        .and_then(|h| h.add())
        .map_err(|e| e.to_string())?;
    let _ = tmp.set_name(TMP_GEOSET_NAME);
    struct MarkerRaw {
        idx: usize,
        rgb: [u8; 3],
        point: Entity,
    }
    let mut markers: Vec<MarkerRaw> = Vec::new();
    for (idx, row) in surf_rows.iter_mut().enumerate() {
        if row.length_mm.is_none() || row.reference.is_none() {
            continue;
        }
        let cog = row
            .reference
            .as_ref()
            .and_then(|r| spa.get_measurable(r).ok())
            .and_then(|m| m.get_cog(ss).ok());
        let Some(cog) = cog else {
            row.reason = Some("重心不可测, 截图未标注".to_string());
            continue;
        };
        let tube = row
            .color
            .unwrap_or_else(|| LABEL_COLORS[idx % LABEL_COLORS.len()]);
        row.color = Some(tube);
        let mk = [255 - tube[0], 255 - tube[1], 255 - tube[2]];
        let pt = hsf
            .add_new_point_coord(cog[0], cog[1], cog[2])
            .map_err(|e| e.to_string())?;
        let _ = tmp.append_hybrid_shape(&pt);
        markers.push(MarkerRaw {
            idx,
            rgb: mk,
            point: pt,
        });
    }
    let _ = part.update();
    for m in &markers {
        let _ = cmd_sel.clear();
        if cmd_sel.add(&m.point).is_ok() {
            if let Ok(vp) = cmd_sel.vis_properties() {
                let _ = vp.set_real_color(m.rgb[0] as i32, m.rgb[1] as i32, m.rgb[2] as i32, 0);
            }
        }
    }
    let _ = cmd_sel.clear();

    // 2. 截图(白底 + Layout=1 相当于 F3 隐藏树 + 注解集 SwitchOn=False 不渲染)
    let orig_layout = window.layout().ok();
    if orig_layout.is_some() {
        let _ = window.set_layout(1); // catWindowGeomOnly
    }
    let mut ann_hidden = false;
    if let Ok(sets) = part.annotation_sets() {
        if let Ok(n) = sets.count() {
            if n > 0 {
                if let Ok(s1) = sets.item(1) {
                    if s1.set_switch_on(false).is_ok() {
                        ann_hidden = true;
                    }
                }
            }
        }
    }
    let _ = viewer.put_background_color([1.0, 1.0, 1.0]);
    let img_markers = capture_bmp(&viewer, &bmp_path).ok();

    // 3. 删色标, 重截干净底图(视图未动, 屏幕位置一致); 恢复底色/布局/注解显示
    delete_body(cmd_sel, part, &tmp);
    let img_clean = match capture_bmp(&viewer, &bmp_path) {
        Ok(c) => Some(c),
        Err(_) => img_markers.clone(),
    };
    let _ = viewer.put_background_color([0.2, 0.2, 0.4]); // 与原工具相同的恢复色
    if let Some(l) = orig_layout {
        let _ = window.set_layout(l);
    }
    if ann_hidden {
        if let Ok(sets) = part.annotation_sets() {
            if let Ok(s1) = sets.item(1) {
                let _ = s1.set_switch_on(true);
            }
        }
    }
    let _ = std::fs::remove_file(&bmp_path);

    let img_markers = img_markers.ok_or_else(|| "截图失败".to_string())?;
    let img_clean = img_clean.ok_or_else(|| "截图失败".to_string())?;

    // 4. 找色点 → 原始截图像素位置(引线/图例用管色, 色标检测色是它的互补色)
    struct PosRaw {
        idx: usize,
        x: f64,
        y: f64,
        rgb: [u8; 3],
    }
    let mut positions_raw: Vec<PosRaw> = Vec::new();
    let mut missed: Vec<String> = Vec::new();
    for m in &markers {
        let hits = find_color_hits(&img_markers, m.rgb, COLOR_TOL);
        if hits.len() < 4 {
            missed.push(surf_rows[m.idx].name.clone());
            continue;
        }
        let cx = hits.iter().map(|h| h.0).sum::<i32>() as f64 / hits.len() as f64;
        let cy = hits.iter().map(|h| h.1).sum::<i32>() as f64 / hits.len() as f64;
        positions_raw.push(PosRaw {
            idx: m.idx,
            x: cx,
            y: cy,
            rgb: surf_rows[m.idx].color.unwrap_or(LABEL_COLORS[0]),
        });
    }

    // 5. 干净底图走管线(白底化/裁边/擦罗盘/方形), 色标坐标同步变换
    let (mut img_final, tf) = process_capture(&img_clean, SHOT_MAX_SIZE);

    // 6. 标注: 大字体, 右侧图例 + 彩色引线, 左上角总计
    let font = load_font();
    let markers_draw: Vec<Marker> = positions_raw
        .iter()
        .map(|p| Marker {
            label: format!(
                "{}: {:.1} mm",
                surf_rows[p.idx].name,
                surf_rows[p.idx].length_mm.unwrap_or(0.0)
            ),
            at: tf.apply(p.x, p.y),
            rgb: p.rgb,
        })
        .collect();
    let total_label = if total_mm > 0.0 {
        Some(format!("总计 {:.1} mm", total_mm))
    } else {
        None
    };
    draw_annotations(
        &mut img_final,
        &markers_draw,
        total_label.as_deref(),
        font.as_ref(),
    );

    // 7. 存 WebP(无损编码, Python 为 quality=80 有损) + data_url
    let out_webp = out_base.with_extension("webp");
    img_final
        .save_with_format(&out_webp, image::ImageFormat::WebP)
        .map_err(|e| format!("写截图失败: {e}"))?;
    let bytes = std::fs::read(&out_webp).map_err(|e| format!("读截图失败: {e}"))?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);

    let mut m = Map::new();
    m.insert("file_path".into(), json!(out_webp.to_string_lossy()));
    m.insert(
        "data_url".into(),
        json!(format!("data:image/webp;base64,{b64}")),
    );
    m.insert("width".into(), json!(img_final.width()));
    m.insert("height".into(), json!(img_final.height()));
    m.insert("labeled".into(), json!(positions_raw.len()));
    m.insert("missed".into(), json!(missed));
    Ok(m)
}

// ── 主流程 ───────────────────────────────────────────────────────────

pub fn run(params: &Value, progress: Progress<'_>) -> ComResult<Value> {
    let geoset_name = params
        .get("geoset_name")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| DEFAULT_GEOSET_NAME.to_string());
    let action = params
        .get("action")
        .and_then(|v| v.as_str())
        .unwrap_or("measure")
        .trim()
        .to_lowercase();
    let analyze = params
        .get("analyze_loops")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let empty_data = json!({
        "geoset_name": geoset_name, "action": action, "surfaces": [], "skipped": [],
        "total_length_mm": null, "measured_count": 0, "skipped_count": 0,
        "formula": "L = 2×面积/边界周长",
    });

    let app = attach_with_guard(progress)?;
    // 命令类操作(删临时集合/注解)落在激活编辑器的选中集上。
    // 注意 prt_doc 在闭包内解析, 这里只挂一个后置还原闭包所需的状态。
    let result = (|| -> ComResult<Value> {
        let doc = app.active_document()?;
        let prt_doc: PartDocument = if doc
            .name()
            .unwrap_or_default()
            .to_lowercase()
            .ends_with(".catpart")
        {
            doc.part_document()
        } else {
            // 当前为图纸/装配时自动切换到同名 .CATPart
            match ensure_part_doc(&app, progress) {
                Ok(d) => d.part_document(),
                Err(e) => return Ok(json!({ "success": false, "message": e, "data": empty_data })),
            }
        };

        let (cmd_sel, borrowed_sel) = command_selection(&app, &prt_doc)?;
        let sel_snapshot = if borrowed_sel {
            crate::vba_bbox_generate::snapshot_selection(&cmd_sel)
        } else {
            Vec::new()
        };
        let restore_sel = |sel: &Selection, snap: &[rscatiacli::dispatch::ComObj]| {
            crate::vba_bbox_generate::restore_selection(sel, snap);
        };
        // 后置还原(所有 return 路径统一走 defer 风格: 结果生成后执行)
        struct SelGuard<'a> {
            sel: &'a Selection,
            snap: Vec<rscatiacli::dispatch::ComObj>,
            borrowed: bool,
            restore: &'a dyn Fn(&Selection, &[rscatiacli::dispatch::ComObj]),
        }
        impl Drop for SelGuard<'_> {
            fn drop(&mut self) {
                if self.borrowed {
                    (self.restore)(self.sel, &self.snap);
                }
            }
        }
        let _guard = SelGuard {
            sel: &cmd_sel,
            snap: sel_snapshot,
            borrowed: borrowed_sel,
            restore: &restore_sel,
        };

        let part = prt_doc.part()?;
        let ss = app.system_service()?;
        let spa = prt_doc.spa_workbench()?;
        let hsf = part.hybrid_shape_factory()?;

        let _ = &cmd_sel;
        // 找集合
        let hbs = part.hybrid_bodies()?;
        let all_geosets = iter_geosets(&hbs);
        let Some((geoset, geoset_actual)) = find_geoset(&all_geosets, &geoset_name) else {
            let names: Vec<String> = all_geosets
                .iter()
                .map(|h| h.name().unwrap_or_else(|_| "(未命名)".to_string()))
                .collect();
            let hint = if names.is_empty() {
                "(无任何集合)".to_string()
            } else {
                let shown: Vec<String> = names.iter().take(12).cloned().collect();
                format!(
                    "{}{}",
                    shown.join("; "),
                    if names.len() > 12 { "..." } else { "" }
                )
            };
            return Ok(json!({
                "success": false,
                "message": format!("未找到名为「{geoset_name}」的集合图文集, 请确认(现有集合: {hint})"),
                "data": empty_data,
            }));
        };

        // 测量(临时集合用于环分析, 用完即删)
        let tmp_body = if analyze {
            let t = part.hybrid_bodies().and_then(|h| h.add())?;
            let _ = t.set_name(TMP_GEOSET_NAME);
            Some(t)
        } else {
            None
        };
        let (mut surfaces, skipped) = measure_surfaces(
            &part,
            &spa,
            &hsf,
            geoset,
            analyze,
            tmp_body.as_ref(),
            progress,
        );
        if let Some(t) = &tmp_body {
            delete_body(&cmd_sel, &part, t);
        }

        let total_mm: f64 = surfaces.iter().filter_map(|r| r.length_mm).sum();
        let measured_n = surfaces.iter().filter(|r| r.length_mm.is_some()).count();

        if measured_n == 0 {
            let reasons: Vec<String> = skipped
                .iter()
                .take(5)
                .map(|r| format!("{}: {}", r.name, r.reason.clone().unwrap_or_default()))
                .collect();
            return Ok(json!({
                "success": false,
                "message": format!("集合「{geoset_actual}」的 {} 个元素均无法测长: {}", skipped.len(), reasons.join("; ")),
                "data": {
                    "geoset_name": geoset_actual,
                    "action": action,
                    "surfaces": [],
                    "skipped": skipped.iter().map(|r| r.to_json()).collect::<Vec<_>>(),
                    "total_length_mm": null,
                    "measured_count": 0,
                    "skipped_count": skipped.len(),
                    "formula": "L = 2×面积/边界周长",
                },
            }));
        }

        let mut data = Map::new();
        data.insert("geoset_name".into(), json!(geoset_actual));
        data.insert("part_name".into(), json!(part.name().unwrap_or_default()));
        data.insert("action".into(), json!(action));
        data.insert(
            "surfaces".into(),
            json!(surfaces.iter().map(|r| r.to_json()).collect::<Vec<_>>()),
        );
        data.insert(
            "skipped".into(),
            json!(skipped.iter().map(|r| r.to_json()).collect::<Vec<_>>()),
        );
        data.insert("total_length_mm".into(), json!(round3(total_mm)));
        data.insert("measured_count".into(), json!(measured_n));
        data.insert("skipped_count".into(), json!(skipped.len()));
        data.insert(
            "formula".into(),
            json!("L = 2×面积/边界周长(等截面精确, 合并过渡面为平均周长等效长度, 不取整)"),
        );
        data.insert(
            "unit_note".into(),
            json!("长度/周长/截面直径单位 mm, 面积 mm²"),
        );

        let suppress = || {
            let prev = app.display_file_alerts().ok();
            let _ = app.set_display_file_alerts(false);
            prev
        };
        let restore = |prev: Option<bool>| {
            if let Some(p) = prev {
                let _ = app.set_display_file_alerts(p);
            }
        };

        // ── 分发 ──
        if action == "annotate3d" {
            let prev = suppress();
            let res = create_len_annotations(&part, &surfaces, progress);
            restore(prev);
            match res {
                Ok(res) => {
                    data.insert("annotate3d".into(), Value::Object(res.clone()));
                    let created = res.get("created").and_then(|v| v.as_u64()).unwrap_or(0);
                    let set_name = res.get("set_name").and_then(|v| v.as_str()).unwrap_or("");
                    return Ok(json!({
                        "success": true,
                        "message": format!(
                            "3D 标注完成: 在注解集「{set_name}」下为 {created} 张曲面建了长度注解(总长 {:.2} mm)。注解锚在对应曲面上, 可在 3D 里拖动微调; 重跑会先删本工具建的旧标注",
                            total_mm
                        ),
                        "data": data,
                    }));
                }
                Err(e) => {
                    return Ok(json!({
                        "success": false,
                        "message": format!("3D 标注失败: {}", e.chars().take(120).collect::<String>()),
                        "data": data,
                    }))
                }
            }
        }

        if action == "screenshot" {
            let prev = suppress();
            let out_dir = out_dir_of(params);
            let out_png = out_dir.join(format!("measure_harness_{}.png", local_timestamp()));
            let res = annotate_screenshot(
                &app,
                &part,
                &spa,
                &ss,
                &cmd_sel,
                &mut surfaces,
                total_mm,
                &out_png,
            );
            restore(prev);
            match res {
                Ok(shot) => {
                    let labeled = shot.get("labeled").and_then(|v| v.as_u64()).unwrap_or(0);
                    let missed = shot
                        .get("missed")
                        .and_then(|v| v.as_array())
                        .cloned()
                        .unwrap_or_default();
                    let file_path = shot
                        .get("file_path")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let miss = if missed.is_empty() {
                        String::new()
                    } else {
                        let names: Vec<String> = missed
                            .iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect();
                        format!(";{} 张被遮挡未标注({})", missed.len(), names.join(", "))
                    };
                    data.insert("screenshot".into(), Value::Object(shot));
                    return Ok(json!({
                        "success": true,
                        "message": format!("截图标注完成: {labeled} 段线束已标注, 总长 {:.2} mm{miss}; 图片: {file_path}", total_mm),
                        "data": data,
                    }));
                }
                Err(e) => {
                    return Ok(json!({
                        "success": false,
                        "message": format!("截图标注失败: {}", e.chars().take(120).collect::<String>()),
                        "data": data,
                    }))
                }
            }
        }

        if action == "annotate_all" {
            // 完整流水线: 清旧 3D 注解(保证截图干净) → 曲面随机换色 → 截图+标注 → 最后建 3D 注解
            let prev = suppress();
            let res = (|| -> Result<(ShotOut, Map<String, Value>, usize), String> {
                let removed = clear_len_annotations(&part, &cmd_sel, progress);
                if removed > 0 {
                    progress(&format!(
                        "已删除旧 3D 标注 {removed} 个(截图前清理, 保证截图干净)"
                    ));
                }
                let palette = random_palette(surfaces.len());
                for (row, rgb) in surfaces.iter_mut().zip(&palette) {
                    row.color = Some(*rgb);
                }
                let colored = color_surfaces(&cmd_sel, &mut surfaces, &palette, progress);

                let out_dir = out_dir_of(params);
                let out_png = out_dir.join(format!("measure_harness_{}.png", local_timestamp()));
                let shot = annotate_screenshot(
                    &app,
                    &part,
                    &spa,
                    &ss,
                    &cmd_sel,
                    &mut surfaces,
                    total_mm,
                    &out_png,
                )?;
                let ann = create_len_annotations(&part, &surfaces, progress)?;
                Ok((shot, ann, colored))
            })();
            restore(prev);
            match res {
                Ok((shot, ann, colored)) => {
                    let labeled = shot.get("labeled").and_then(|v| v.as_u64()).unwrap_or(0);
                    let missed = shot
                        .get("missed")
                        .and_then(|v| v.as_array())
                        .cloned()
                        .unwrap_or_default();
                    let file_path = shot
                        .get("file_path")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let miss = if missed.is_empty() {
                        String::new()
                    } else {
                        let names: Vec<String> = missed
                            .iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect();
                        format!(";{} 张被遮挡未标注({})", missed.len(), names.join(", "))
                    };
                    data.insert("screenshot".into(), Value::Object(shot));
                    data.insert("annotate3d".into(), Value::Object(ann.clone()));
                    data.insert("colored".into(), json!(colored));
                    let ann_created = ann.get("created").and_then(|v| v.as_u64()).unwrap_or(0);
                    return Ok(json!({
                        "success": true,
                        "message": format!(
                            "标注完成: 换色 {colored} 张曲面, 截图标注 {labeled} 段(总长 {:.2} mm){miss}, 3D 注解 {ann_created} 个;截图: {file_path}。注解在截图之后创建, 不会出现在截图里;请手动保存文档",
                            total_mm
                        ),
                        "data": data,
                    }));
                }
                Err(e) => {
                    return Ok(json!({
                        "success": false,
                        "message": format!("标注失败: {}", e.chars().take(120).collect::<String>()),
                        "data": data,
                    }))
                }
            }
        }

        // measure(缺省)
        let summary = surfaces
            .iter()
            .filter_map(|r| r.length_mm.map(|l| format!("{}→{}", r.name, l)))
            .collect::<Vec<_>>()
            .join(", ");
        let skip_note = if skipped.is_empty() {
            String::new()
        } else {
            format!(";跳过 {} 个", skipped.len())
        };
        Ok(json!({
            "success": true,
            "message": format!(
                "线束测长完成: 集合「{geoset_actual}」测得 {measured_n} 张曲面, 总长度 {:.2} mm({summary}){skip_note}。合并过渡面为等效长度, 未取整",
                total_mm
            ),
            "data": data,
        }))
    })();

    match result {
        Ok(v) => Ok(v),
        Err(e) => Ok(json!({
            "success": false,
            "message": format!("measure_harness failed: {e}")
        })),
    }
}

fn out_dir_of(params: &Value) -> PathBuf {
    let from_params = params
        .get("out_dir")
        .and_then(|v| v.as_str())
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty());
    from_params.unwrap_or_else(|| {
        std::env::var("TEMP")
            .map(PathBuf::from)
            .unwrap_or_else(|_| dirs_home())
    })
}

fn dirs_home() -> PathBuf {
    std::env::var("USERPROFILE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("C:\\temp"))
}
