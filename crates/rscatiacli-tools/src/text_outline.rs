//! 文字轮廓提取与排版（Windows GDI，无第三方依赖）。
//!
//! 对应 ltl-manage resources/sidecars/text_to_sketch/text_outline.py。
//! 用 1000 逻辑单位的字号取未 hint 的原始轮廓（GGO_NATIVE|GGO_UNHINTED），
//! 解析 TTPOLYGONHEADER + TTPOLYCURVE 序列（16.16 定点）：直线原样保留，
//! 二次/三次贝塞尔按 mm 容差自适应离散成折线；排版按「基线相对的字形外包高
//! = 字高 mm」定标，整体左下角落在 (0,0)。缩放/离散的口径与 Python 版逐行对齐。
//!
//! GDI 句柄纪律：内存 DC / 字体对象用完必须释放（FontDc 的 Drop 负责）。

use std::cell::RefCell;
use std::fmt;

use serde_json::{Map, Value};
use windows::Win32::Foundation::LPARAM;
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateFontIndirectW, DeleteDC, DeleteObject, EnumFontFamiliesExW,
    GetFontUnicodeRanges, GetGlyphOutlineW, GetTextFaceW, SelectObject, CLIP_DEFAULT_PRECIS,
    DEFAULT_CHARSET, FIXED, GET_GLYPH_OUTLINE_FORMAT, GLYPHMETRICS, GLYPHSET, HDC, HGDIOBJ,
    LOGFONTW, MAT2, OUT_TT_ONLY_PRECIS, PROOF_QUALITY, TEXTMETRICW, TRUETYPE_FONTTYPE,
};

// ── GDI 常量 ─────────────────────────────────────────────────────────
const GGO_NATIVE: u32 = 2;
const GGO_UNHINTED: u32 = 0x0100;
const GDI_ERROR: u32 = 0xFFFF_FFFF;
const TT_PRIM_LINE: u16 = 1;
const TT_PRIM_QSPLINE: u16 = 2;
const TT_PRIM_CSPLINE: u16 = 3;

/// 取轮廓用的字号（逻辑单位）；越大精度越高（Python EM_UNITS）。
const EM_UNITS: i32 = 1000;
const FW_NORMAL: i32 = 400;
const FW_BOLD: i32 = 700;

/// 报错提示里优先推荐的常用字体（刻字/中英混排都够用；只列系统里真有的）。
pub const COMMON_FONTS: [&str; 6] = [
    "Arial",
    "Monospac821 BT",
    "仿宋",
    "宋体",
    "MS Gothic",
    "Times New Roman",
];

// ── 错误 ─────────────────────────────────────────────────────────────

/// 字体/字形不可用等可展示给用户的错误。
#[derive(Debug, Clone)]
pub struct TextOutlineError(pub String);

impl fmt::Display for TextOutlineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for TextOutlineError {}

fn terr(msg: impl Into<String>) -> TextOutlineError {
    TextOutlineError(msg.into())
}

// ── 数据结构 ─────────────────────────────────────────────────────────

pub type Pt = (f64, f64);

/// 草图图元：精确直线段 / ≥2 点折线（曲线离散结果）。
#[derive(Clone, Debug)]
pub enum Segment {
    Line(Pt, Pt),
    Curve(Vec<Pt>),
}

/// 原始字形图元（GDI 逻辑单位）。
#[derive(Clone, Debug)]
enum Prim {
    Line(Pt),
    Quad(Pt, Pt),      // 控制点, 终点
    Cubic(Pt, Pt, Pt), // 控制点1, 控制点2, 终点
}

/// 单条闭合轮廓：起点 + 图元序列。
type Contour = (Pt, Vec<Prim>);

/// 排版结果：segments(图元) + 统计。
pub struct TextOutline {
    pub kind: &'static str, // "text" / "qr"
    pub segments: Vec<Segment>,
    pub width: f64,
    pub height: f64,
    pub char_height: f64,
    pub n_contours: usize,
    pub n_lines: usize,
    pub n_curves: usize,
    pub font: String,
    pub missing_chars: Vec<String>,
    /// 生成类型附加信息（qr 的版本/模块数等；文字模式为空）。
    pub extra: Map<String, Value>,
}

impl TextOutline {
    pub(crate) fn new_text() -> Self {
        Self {
            kind: "text",
            segments: Vec::new(),
            width: 0.0,
            height: 0.0,
            char_height: 0.0,
            n_contours: 0,
            n_lines: 0,
            n_curves: 0,
            font: String::new(),
            missing_chars: Vec::new(),
            extra: Map::new(),
        }
    }

    /// 统计信息（Python TextOutline.to_dict 同形，供工具层组装结果）。
    pub fn info(&self) -> Map<String, Value> {
        let mut m = Map::new();
        m.insert("kind".into(), Value::String(self.kind.to_string()));
        m.insert("width_mm".into(), json_f(round3(self.width)));
        m.insert("height_mm".into(), json_f(round3(self.height)));
        m.insert("char_height_mm".into(), json_f(round3(self.char_height)));
        m.insert("contours".into(), Value::from(self.n_contours as u64));
        m.insert("lines".into(), Value::from(self.n_lines as u64));
        m.insert("curves".into(), Value::from(self.n_curves as u64));
        m.insert("segments".into(), Value::from(self.segments.len() as u64));
        m.insert("font".into(), Value::String(self.font.clone()));
        m.insert(
            "missing_chars".into(),
            Value::Array(
                self.missing_chars
                    .iter()
                    .map(|c| Value::String(c.clone()))
                    .collect(),
            ),
        );
        for (k, v) in &self.extra {
            m.insert(k.clone(), v.clone());
        }
        m
    }
}

fn json_f(v: f64) -> Value {
    serde_json::Number::from_f64(v)
        .map(Value::Number)
        .unwrap_or(Value::Null)
}

fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

fn dist2(a: Pt, b: Pt) -> f64 {
    (a.0 - b.0) * (a.0 - b.0) + (a.1 - b.1) * (a.1 - b.1)
}

fn normalize_font_name(s: &str) -> String {
    s.split_whitespace()
        .collect::<String>()
        .replace('-', "")
        .to_lowercase()
}

// ── GDI 句柄包装 ─────────────────────────────────────────────────────

/// 字体覆盖的 Unicode 区间缓存（[(first, last)]）。
struct FontDc {
    hdc: HDC,
    _hfont: HGDIOBJ,
    old: HGDIOBJ,
    actual_name: String,
    unicode_ranges: Option<Vec<(u32, u32)>>,
}

impl Drop for FontDc {
    fn drop(&mut self) {
        unsafe {
            if !self.old.0.is_null() {
                let _ = SelectObject(self.hdc, self.old);
            }
            if !self._hfont.0.is_null() {
                let _ = DeleteObject(self._hfont);
            }
            if !self.hdc.is_invalid() {
                let _ = DeleteDC(self.hdc);
            }
        }
    }
}

impl FontDc {
    /// 选定字体 + 内存 DC（取轮廓/字宽用）。
    fn new(font_name: &str, bold: bool, italic: bool) -> Result<Self, TextOutlineError> {
        let hdc = unsafe { CreateCompatibleDC(None) };
        if hdc.is_invalid() {
            return Err(terr("无法创建 GDI 内存 DC"));
        }
        let mut face = [0u16; 32];
        for (i, ch) in font_name.encode_utf16().take(31).enumerate() {
            face[i] = ch;
        }
        let lf = LOGFONTW {
            lfHeight: -EM_UNITS, // 负 = 字符高度(em), 正 = 含行距的单元格高
            lfWeight: if bold { FW_BOLD } else { FW_NORMAL },
            lfItalic: if italic { 1 } else { 0 },
            lfCharSet: DEFAULT_CHARSET,
            lfOutPrecision: OUT_TT_ONLY_PRECIS,
            lfClipPrecision: CLIP_DEFAULT_PRECIS,
            lfQuality: PROOF_QUALITY,
            lfFaceName: face,
            ..Default::default()
        };
        let hfont = unsafe { CreateFontIndirectW(&lf) };
        if hfont.0.is_null() {
            return Err(terr(format!("创建字体失败: {font_name}")));
        }
        let old = unsafe { SelectObject(hdc, HGDIOBJ(hfont.0)) };
        if old.0.is_null() {
            return Err(terr(format!("选定字体失败: {font_name}")));
        }
        let mut name_buf = [0u16; 96];
        let _ = unsafe { GetTextFaceW(hdc, Some(&mut name_buf)) };
        let len = name_buf.iter().position(|c| *c == 0).unwrap_or(0);
        let actual_name = String::from_utf16_lossy(&name_buf[..len]);
        if actual_name.is_empty() {
            return Err(terr(format!("创建字体失败: {font_name}")));
        }
        Ok(Self {
            hdc,
            _hfont: HGDIOBJ(hfont.0),
            old,
            actual_name,
            unicode_ranges: None,
        })
    }

    /// 字形轮廓：(轮廓列表, 步进宽度)；取不到(字体无此字形)返回 None。
    /// 空格等无轮廓字符返回 (空列表, 步进)。
    fn glyph_outline(&self, ch: char) -> Option<(Vec<Contour>, i32)> {
        let mut gm = GLYPHMETRICS::default();
        let mat = identity_mat2();
        let mut fmt = GET_GLYPH_OUTLINE_FORMAT(GGO_NATIVE | GGO_UNHINTED);
        let mut size =
            unsafe { GetGlyphOutlineW(self.hdc, ch as u32, fmt, &mut gm, 0, None, &mat) };
        if size == GDI_ERROR {
            // 个别驱动不接受 UNHINTED, 退回带 hint 的原生轮廓
            fmt = GET_GLYPH_OUTLINE_FORMAT(GGO_NATIVE);
            size = unsafe { GetGlyphOutlineW(self.hdc, ch as u32, fmt, &mut gm, 0, None, &mat) };
        }
        if size == GDI_ERROR {
            return None;
        }
        let raw: Vec<u8> = if size > 0 {
            let mut buf = vec![0u8; size as usize];
            let got = unsafe {
                GetGlyphOutlineW(
                    self.hdc,
                    ch as u32,
                    fmt,
                    &mut gm,
                    size,
                    Some(buf.as_mut_ptr().cast()),
                    &mat,
                )
            };
            if got == GDI_ERROR {
                return None;
            }
            buf.truncate(got as usize);
            buf
        } else {
            Vec::new()
        };
        Some((parse_outline(&raw), gm.gmCellIncX as i32))
    }

    /// 字体 cmap 覆盖的 Unicode 区间 [(first, last)]；取不到返回空表。
    fn ranges(&mut self) -> Vec<(u32, u32)> {
        if let Some(r) = &self.unicode_ranges {
            return r.clone();
        }
        let out = unsafe {
            let need = GetFontUnicodeRanges(self.hdc, None);
            if need == 0 || need == GDI_ERROR {
                Vec::new()
            } else {
                let mut buf = vec![0u8; need as usize + 64];
                let got = GetFontUnicodeRanges(self.hdc, Some(buf.as_mut_ptr().cast()));
                if got == 0 || got == GDI_ERROR {
                    Vec::new()
                } else {
                    let gs = &*(buf.as_ptr() as *const GLYPHSET);
                    let n = (gs.cRanges as usize).min(4096);
                    let mut v = Vec::with_capacity(n);
                    // awRanges 紧跟 GLYPHSET 头部(16 字节): {wcLow: u16, cGlyphs: u16}
                    for i in 0..n {
                        let off = 16 + i * 4;
                        let lo = u16::from_le_bytes([buf[off], buf[off + 1]]) as u32;
                        let count = u16::from_le_bytes([buf[off + 2], buf[off + 3]]) as u32;
                        if count > 0 {
                            v.push((lo, lo + count - 1));
                        }
                    }
                    v
                }
            }
        };
        self.unicode_ranges = Some(out.clone());
        out
    }
}

fn identity_mat2() -> MAT2 {
    MAT2 {
        eM11: FIXED { fract: 0, value: 1 },
        eM12: FIXED { fract: 0, value: 0 },
        eM21: FIXED { fract: 0, value: 0 },
        eM22: FIXED { fract: 0, value: 1 },
    }
}

// ── 字体枚举 ─────────────────────────────────────────────────────────

thread_local! {
    static FONT_NAMES: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// 枚举回调：只收 TrueType 字体族名（跳过 @ 开头的竖排变体）。
unsafe extern "system" fn enum_font_cb(
    lpelfe: *const LOGFONTW,
    _ntme: *const TEXTMETRICW,
    font_type: u32,
    _lparam: LPARAM,
) -> i32 {
    if font_type & TRUETYPE_FONTTYPE == 0 {
        return 1;
    }
    if lpelfe.is_null() {
        return 1;
    }
    let face = (*lpelfe).lfFaceName;
    let len = face.iter().position(|c| *c == 0).unwrap_or(face.len());
    let name = String::from_utf16_lossy(&face[..len]);
    let name = name.trim().to_string();
    if !name.is_empty() && !name.starts_with('@') {
        FONT_NAMES.with(|c| c.borrow_mut().push(name));
    }
    1
}

/// 系统里可用的 TrueType 字体族名（去重排序）。取不到时返回空表。
pub fn available_fonts() -> Vec<String> {
    let hdc = unsafe { CreateCompatibleDC(None) };
    if hdc.is_invalid() {
        return Vec::new();
    }
    FONT_NAMES.with(|c| c.borrow_mut().clear());
    let lf = LOGFONTW {
        lfCharSet: DEFAULT_CHARSET,
        ..Default::default()
    };
    unsafe {
        EnumFontFamiliesExW(hdc, &lf, Some(enum_font_cb), LPARAM(0), 0);
        let _ = DeleteDC(hdc);
    }
    let mut names = FONT_NAMES.with(|c| std::mem::take(&mut *c.borrow_mut()));
    names.sort_by_key(|a| a.to_lowercase());
    names.dedup();
    names
}

/// 按名字给出相近的可用字体（子串命中优先, 都没命中就列常用字体）。
pub fn suggest_fonts(name: &str, fonts: &[String], limit: usize) -> Vec<String> {
    let key = normalize_font_name(name);
    let sub: Vec<String> = fonts
        .iter()
        .filter(|f| !key.is_empty() && normalize_font_name(f).contains(&key))
        .cloned()
        .collect();
    let fallback = || -> Vec<String> {
        let have: Vec<String> = COMMON_FONTS
            .iter()
            .filter(|f| fonts.iter().any(|x| x == *f))
            .map(|f| f.to_string())
            .collect();
        if have.is_empty() {
            fonts.to_vec()
        } else {
            have
        }
    };
    let list = if sub.is_empty() { fallback() } else { sub };
    list.into_iter().take(limit).collect()
}

// ── 轮廓解析 ─────────────────────────────────────────────────────────

/// 读一个 16.16 定点数（字节序: fract 在前, 有符号 value 在后）。
fn fixed(raw: &[u8], off: usize) -> f64 {
    let fract = u16::from_le_bytes([raw[off], raw[off + 1]]) as f64;
    let value = i16::from_le_bytes([raw[off + 2], raw[off + 3]]) as f64;
    value + fract / 65536.0
}

/// TTPOLYGONHEADER/TTPOLYCURVE 序列 → 轮廓图元列表。
fn parse_outline(raw: &[u8]) -> Vec<Contour> {
    let mut contours: Vec<Contour> = Vec::new();
    let n = raw.len();
    let mut off = 0usize;
    while off + 16 <= n {
        let cb = u32::from_le_bytes([raw[off], raw[off + 1], raw[off + 2], raw[off + 3]]) as usize;
        if cb < 16 || off + cb > n {
            break;
        }
        let start = (fixed(raw, off + 8), fixed(raw, off + 12));
        let mut prims: Vec<Prim> = Vec::new();
        let mut i = off + 16;
        let end = off + cb;
        while i + 4 <= end {
            let wtype = u16::from_le_bytes([raw[i], raw[i + 1]]);
            let cpfx = u16::from_le_bytes([raw[i + 2], raw[i + 3]]) as usize;
            i += 4;
            let mut pts: Vec<Pt> = Vec::with_capacity(cpfx);
            for _ in 0..cpfx {
                if i + 8 > end {
                    break;
                }
                pts.push((fixed(raw, i), fixed(raw, i + 4)));
                i += 8;
            }
            if pts.is_empty() {
                continue;
            }
            match wtype {
                TT_PRIM_LINE => {
                    for p in pts {
                        prims.push(Prim::Line(p));
                    }
                }
                TT_PRIM_QSPLINE => {
                    // 中间点是控制点: 相邻控制点取中点作为上一段终点, 末点为终点
                    for k in 0..pts.len().saturating_sub(1) {
                        let ctrl = pts[k];
                        let nxt = pts[k + 1];
                        let p_end = if k == pts.len() - 2 {
                            nxt
                        } else {
                            ((ctrl.0 + nxt.0) / 2.0, (ctrl.1 + nxt.1) / 2.0)
                        };
                        prims.push(Prim::Quad(ctrl, p_end));
                    }
                }
                TT_PRIM_CSPLINE => {
                    let mut k = 0;
                    while k + 3 <= pts.len() {
                        prims.push(Prim::Cubic(pts[k], pts[k + 1], pts[k + 2]));
                        k += 3;
                    }
                }
                _ => {}
            }
        }
        contours.push((start, prims));
        off += cb;
    }
    contours
}

// ── 贝塞尔自适应离散 ─────────────────────────────────────────────────

/// 二次贝塞尔自适应离散（按控制点到弦的距离判断是否需要细分）。
fn flatten_quad(p0: Pt, c: Pt, p1: Pt, tol: f64, out: &mut Vec<Pt>) {
    let dx = p1.0 - p0.0;
    let dy = p1.1 - p0.1;
    let chord = (dx * dx + dy * dy).sqrt();
    let dev = if chord <= 1e-12 {
        ((c.0 - p0.0).powi(2) + (c.1 - p0.1).powi(2)).sqrt()
    } else {
        ((c.0 - p0.0) * dy - (c.1 - p0.1) * dx).abs() / chord
    };
    if dev <= tol || chord <= tol {
        out.push(p1);
        return;
    }
    // de Casteljau 一分为二
    let a = ((p0.0 + c.0) / 2.0, (p0.1 + c.1) / 2.0);
    let b = ((c.0 + p1.0) / 2.0, (c.1 + p1.1) / 2.0);
    let m = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
    flatten_quad(p0, a, m, tol, out);
    flatten_quad(m, b, p1, tol, out);
}

/// 三次贝塞尔自适应离散。
fn flatten_cubic(p0: Pt, c1: Pt, c2: Pt, p1: Pt, tol: f64, out: &mut Vec<Pt>) {
    let dx = p1.0 - p0.0;
    let dy = p1.1 - p0.1;
    let chord = (dx * dx + dy * dy).sqrt();
    let mut dev = 0.0f64;
    for c in [c1, c2] {
        if chord <= 1e-12 {
            dev = dev.max(((c.0 - p0.0).powi(2) + (c.1 - p0.1).powi(2)).sqrt());
        } else {
            dev = dev.max(((c.0 - p0.0) * dy - (c.1 - p0.1) * dx).abs() / chord);
        }
    }
    if dev <= tol || chord <= tol {
        out.push(p1);
        return;
    }
    let a = ((p0.0 + c1.0) / 2.0, (p0.1 + c1.1) / 2.0);
    let b = ((c1.0 + c2.0) / 2.0, (c1.1 + c2.1) / 2.0);
    let c = ((c2.0 + p1.0) / 2.0, (c2.1 + p1.1) / 2.0);
    let d = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
    let e = ((b.0 + c.0) / 2.0, (b.1 + c.1) / 2.0);
    let m = ((d.0 + e.0) / 2.0, (d.1 + e.1) / 2.0);
    flatten_cubic(p0, a, d, m, tol, out);
    flatten_cubic(m, e, c, p1, tol, out);
}

fn in_ranges(code: u32, ranges: &[(u32, u32)]) -> bool {
    ranges.iter().any(|(lo, hi)| *lo <= code && code <= *hi)
}

// ── 排版 ─────────────────────────────────────────────────────────────

/// 把文字排成轮廓几何（mm；草图坐标 X 右 / Y 上，整体左下角落在 (0,0)）。
///
/// * `height_mm`: 字高 = 文字整体高度（基线对齐后的字形外包高, 不含行距）
/// * `letter_spacing_mm`: 相邻字之间额外增加的间距（可负）
/// * `line_spacing_mm`: 行距, None = 1.5×字高
/// * `tol_mm`: 曲线离散容差
#[allow(clippy::too_many_arguments)]
pub fn build_text_outline(
    text: &str,
    font_name: &str,
    height_mm: f64,
    letter_spacing_mm: f64,
    line_spacing_mm: Option<f64>,
    bold: bool,
    italic: bool,
    tol_mm: f64,
) -> Result<TextOutline, TextOutlineError> {
    // 统一换行; 保留空行(占一行高度)
    let text_norm = text.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<&str> = text_norm
        .split('\n')
        .map(|ln| ln.trim_end_matches('\r'))
        .collect();
    if lines.is_empty() || lines.iter().all(|ln| ln.is_empty()) {
        return Err(terr("文字为空"));
    }
    let font_name = font_name.trim();
    if font_name.is_empty() {
        return Err(terr("字体名为空"));
    }
    if !(0.05..=500.0).contains(&height_mm) {
        return Err(terr(format!("字高需在 0.05~500mm 之间: {height_mm}")));
    }
    let mut line_spacing = line_spacing_mm.unwrap_or(height_mm * 1.5);
    if line_spacing < height_mm * 0.5 {
        line_spacing = height_mm * 0.5; // 防止行重叠到无法辨识
    }

    // 1) 逐字取轮廓（去重保序）
    let mut chars: Vec<char> = Vec::new();
    for ln in &lines {
        for ch in ln.chars() {
            if !chars.contains(&ch) {
                chars.push(ch);
            }
        }
    }

    let mut out = TextOutline::new_text();
    let mut fdc = FontDc::new(font_name, bold, italic)?;
    out.font = fdc.actual_name.clone();
    if normalize_font_name(&out.font) != normalize_font_name(font_name) {
        let fonts = available_fonts();
        let sugg = suggest_fonts(font_name, &fonts, 8);
        let hint = if sugg.is_empty() {
            String::new()
        } else {
            format!("。可用字体: {}", sugg.join("、"))
        };
        return Err(terr(format!(
            "系统里没有字体「{font_name}」(GDI 会把请求替换成别的字体, 结果不可控); 当前被替换为「{}」{hint}",
            out.font
        )));
    }

    // 字体不含的字符(如英文字体里的中文)→ 直接报错, 免得画出一堆方块
    let ranges = fdc.ranges();
    if !ranges.is_empty() {
        let missing: Vec<char> = chars
            .iter()
            .copied()
            .filter(|c| !c.is_whitespace() && !in_ranges(*c as u32, &ranges))
            .collect();
        if !missing.is_empty() {
            let shown: Vec<String> = missing.iter().take(6).map(|c| format!("「{c}」")).collect();
            let more = if missing.len() > 6 {
                format!(" 等 {} 个", missing.len())
            } else {
                String::new()
            };
            out.missing_chars = missing.iter().take(20).map(|c| c.to_string()).collect();
            return Err(terr(format!(
                "字体「{}」不含字符 {}{more}; 中文请换 仿宋/宋体/MS Gothic 这类中文字体",
                out.font,
                shown.join("、")
            )));
        }
    }

    let mut glyphs: std::collections::HashMap<char, (Vec<Contour>, i32)> =
        std::collections::HashMap::new();
    for ch in &chars {
        let got = fdc.glyph_outline(*ch).ok_or_else(|| {
            terr(format!(
                "字体「{}」的字符「{ch}」取不到轮廓 —— 该字体可能是点阵/OpenType(CFF)字体, \
                 不支持取轮廓, 请换 TrueType 字体(如 Arial/仿宋/宋体)",
                out.font
            ))
        })?;
        glyphs.insert(*ch, got);
    }

    // 2) 排版（先按 GDI 单位, 含字间距/行距; GDI 的 Y 向下, 但轮廓数据 y 向上）
    // 缩放基准要先粗算一次: 离散容差按 mm 给, 换算成单位需要 scale
    let mut ys0: Vec<f64> = Vec::new();
    for ch in &chars {
        if let Some((contours, _)) = glyphs.get(ch) {
            for (_start, prims) in contours {
                ys0.push(_start.1);
                for pr in prims {
                    match pr {
                        Prim::Line(p) => ys0.push(p.1),
                        Prim::Quad(c, p) => {
                            ys0.push(c.1);
                            ys0.push(p.1);
                        }
                        Prim::Cubic(c1, c2, p) => {
                            ys0.push(c1.1);
                            ys0.push(c2.1);
                            ys0.push(p.1);
                        }
                    }
                }
            }
        }
    }
    if ys0.is_empty() {
        return Err(terr(format!(
            "文字「{text}」在字体「{}」里没有任何可见字形",
            out.font
        )));
    }
    let ys0_min = ys0.iter().copied().fold(f64::INFINITY, f64::min);
    let ys0_max = ys0.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let span0 = ys0_max - ys0_min;
    if span0 <= 0.0 {
        return Err(terr(format!(
            "文字「{text}」在字体「{}」里没有高度(可能全是空白字符)",
            out.font
        )));
    }
    let tol_units = tol_mm * span0 / height_mm;

    // 排版（单位制）: 每行一行轮廓, 后面的行 y 更小（行距向下）
    let line_letters = letter_spacing_mm * span0 / height_mm; // mm → 单位
    let line_step = line_spacing * span0 / height_mm; // mm → 单位
    let (mut min_x, mut max_x) = (f64::INFINITY, f64::NEG_INFINITY);
    let (mut min_y, mut max_y) = (f64::INFINITY, f64::NEG_INFINITY);
    let (mut base_min_y, mut base_max_y) = (f64::INFINITY, f64::NEG_INFINITY);
    let mut raw_contours: Vec<Vec<Segment>> = Vec::new();
    for (li, ln) in lines.iter().enumerate() {
        let mut pen_x = 0.0f64;
        let base_y = -(li as f64) * line_step;
        for ch in ln.chars() {
            let Some((contour_list, adv)) = glyphs.get(&ch) else {
                continue;
            };
            for (start, prims) in contour_list {
                if prims.is_empty() {
                    continue;
                }
                let (sx, sy) = (start.0 + pen_x, start.1 + base_y);
                let off = |p: Pt| (p.0 + pen_x, p.1 + base_y);
                let mut p0 = (sx, sy);
                let mut cur: Vec<Segment> = Vec::new();
                for pr in prims {
                    match pr {
                        Prim::Line(p1) => {
                            let p1 = off(*p1);
                            cur.push(Segment::Line(p0, p1));
                            p0 = p1;
                        }
                        Prim::Quad(c, p1) => {
                            let c = off(*c);
                            let p1 = off(*p1);
                            let mut chain = Vec::new();
                            flatten_quad(p0, c, p1, tol_units, &mut chain);
                            let mut poly = vec![p0];
                            poly.extend(chain);
                            cur.push(Segment::Curve(poly));
                            p0 = p1;
                        }
                        Prim::Cubic(c1, c2, p1) => {
                            let c1 = off(*c1);
                            let c2 = off(*c2);
                            let p1 = off(*p1);
                            let mut chain = Vec::new();
                            flatten_cubic(p0, c1, c2, p1, tol_units, &mut chain);
                            let mut poly = vec![p0];
                            poly.extend(chain);
                            cur.push(Segment::Curve(poly));
                            p0 = p1;
                        }
                    }
                }
                // 闭合(TrueType 轮廓必闭合): 末点回到起点
                if dist2(p0, (sx, sy)) > 1e-12 {
                    cur.push(Segment::Line(p0, (sx, sy)));
                }
                for seg in &cur {
                    let pts: Vec<Pt> = match seg {
                        Segment::Line(a, b) => vec![*a, *b],
                        Segment::Curve(poly) => poly.clone(),
                    };
                    for p in pts {
                        min_x = min_x.min(p.0);
                        max_x = max_x.max(p.0);
                        min_y = min_y.min(p.1);
                        max_y = max_y.max(p.1);
                        // 基线相对的字形上下极值（定标基准: 字高不含行距）
                        base_min_y = base_min_y.min(p.1 - base_y);
                        base_max_y = base_max_y.max(p.1 - base_y);
                    }
                }
                raw_contours.push(cur);
            }
            pen_x += *adv as f64 + line_letters;
        }
    }
    if min_x == f64::INFINITY {
        return Err(terr(format!("文字「{text}」没有轮廓点")));
    }

    // 3) 单位 → mm: 定标用基线相对的离散极值（多行时每行都是「字高」, 行距不参与
    //    缩放）; 再整体平移使左下角落在 (0, 0)。轮廓数据本身 y 向上, 只缩放+平移。
    let span_flat = base_max_y - base_min_y;
    if span_flat <= 0.0 {
        return Err(terr(format!("文字「{text}」没有高度(可能全是空白字符)")));
    }
    let scale = height_mm / span_flat;
    let mk = |p: Pt| ((p.0 - min_x) * scale, (p.1 - min_y) * scale);

    for cur in &raw_contours {
        for seg in cur {
            match seg {
                Segment::Line(a, b) => {
                    let (p0, p1) = (mk(*a), mk(*b));
                    if dist2(p0, p1) <= 1e-12 {
                        continue;
                    }
                    out.segments.push(Segment::Line(p0, p1));
                    out.n_lines += 1;
                }
                Segment::Curve(poly) => {
                    let pts: Vec<Pt> = poly.iter().map(|p| mk(*p)).collect();
                    // 去掉相邻重复点, 少于 2 点退化为空、2 点退化为直线段
                    let mut dedup: Vec<Pt> = vec![pts[0]];
                    for p in &pts[1..] {
                        if dist2(*p, *dedup.last().unwrap()) > 1e-12 {
                            dedup.push(*p);
                        }
                    }
                    match dedup.len() {
                        0 | 1 => {}
                        2 => {
                            out.segments.push(Segment::Line(dedup[0], dedup[1]));
                            out.n_lines += 1;
                        }
                        _ => {
                            out.segments.push(Segment::Curve(dedup));
                            out.n_curves += 1;
                        }
                    }
                }
            }
        }
        out.n_contours += 1;
    }

    out.width = (max_x - min_x) * scale;
    out.height = (max_y - min_y) * scale; // 整体高度(多行含行距)
    out.char_height = height_mm;
    Ok(out)
}

// ── 测试（真机 Windows 有 GDI 与系统字体, 可直接跑） ────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fonts_available() {
        let fonts = available_fonts();
        assert!(!fonts.is_empty(), "系统里应能枚举到 TrueType 字体");
        assert!(fonts.iter().any(|f| f == "Arial"), "Arial 应在系统字体里");
    }

    #[test]
    fn text_layout_basic() {
        let out = build_text_outline("AB", "Arial", 5.0, 0.5, None, false, false, 0.02)
            .expect("Arial 取轮廓应成功");
        assert_eq!(out.char_height, 5.0);
        assert!(
            (out.height - 5.0).abs() < 0.05,
            "字高应定标到 5mm: {}",
            out.height
        );
        assert!(out.width > 0.0);
        assert!(out.n_lines > 0);
        assert!(out.n_curves > 0, "Arial 的曲线字应离散出样条");
        // 左下角在原点
        let min_x = out
            .segments
            .iter()
            .map(|s| match s {
                Segment::Line(a, _) => a.0,
                Segment::Curve(p) => p[0].0,
            })
            .fold(f64::INFINITY, f64::min);
        assert!(min_x.abs() < 1e-6, "整体应平移到 x=0: {min_x}");
    }

    #[test]
    fn missing_font_rejected() {
        let err = build_text_outline("A", "NoSuchFont_XYZ", 5.0, 0.0, None, false, false, 0.02)
            .err()
            .expect("不存在的字体应报错");
        assert!(err.0.contains("系统里没有字体"), "{err}");
    }

    #[test]
    fn missing_char_rejected() {
        // 英文字体排中文 → 字体不含字符报错
        let err = build_text_outline("刻字", "Arial", 5.0, 0.0, None, false, false, 0.02)
            .err()
            .expect("Arial 排中文应报错");
        assert!(err.0.contains("不含字符"), "{err}");
    }
}
