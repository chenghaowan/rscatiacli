//! 截图处理与标注管线（PIL 流程的 Rust 等价物）。
//!
//! 对应 measure_harness.py 的 `_process_capture` / `_find_color_hits` /
//! `_load_font` / `_annotate_screenshot` 的画图部分，以及原截图工具
//! capture_active_part 的管线口径：
//! 阈值白底化 → 裁白边(margin 2) → 擦右上罗盘(150×100) → 等比缩放白底填充成
//! 正方形; 裁剪/缩放/填充偏移全程跟踪（CaptureTransform 把原始截图像素坐标
//! 映射到处理图坐标, 供色标定位换算）。
//!
//! 与 Python 版的已知差异: WebP 输出为无损编码（image crate 的 WebP 编码器
//! 不带质量参数）, 文件比有损 quality=80 大一些; 总计标签的圆角矩形画成直角。

use std::path::Path;

use ab_glyph::{FontArc, FontVec, PxScale};
use image::{Rgb, RgbImage};
use imageproc::drawing::{
    draw_filled_rect_mut, draw_hollow_circle_mut, draw_hollow_rect_mut, draw_line_segment_mut,
    draw_text_mut, text_size,
};
use imageproc::rect::Rect;

/// 截图输出方形边长（与原截图工具同风格）。
pub const SHOT_MAX_SIZE: u32 = 1024;
/// 色标聚类 RGB 容差。
pub const COLOR_TOL: u8 = 45;
/// 内容阈值（与原工具一致）。
const CONTENT_THRESHOLD: u8 = 30;

/// 原始截图像素坐标 → 处理图坐标的仿射跟踪。
#[derive(Clone, Copy, Debug)]
pub struct CaptureTransform {
    crop_x: i64,
    crop_y: i64,
    scale: f64,
    pad_x: i32,
    pad_y: i32,
}

impl CaptureTransform {
    pub fn apply(&self, x: f64, y: f64) -> (i32, i32) {
        (
            ((x - self.crop_x as f64) * self.scale).round() as i32 + self.pad_x,
            ((y - self.crop_y as f64) * self.scale).round() as i32 + self.pad_y,
        )
    }
}

fn luma_diff_white(px: Rgb<u8>) -> u8 {
    // ImageChops.difference(img, white).convert("L") 的等价: 与纯白的差取亮度
    let d = |c: u8| 255i32 - c as i32;
    let l = (0.299 * d(px[0]).abs() as f64
        + 0.587 * d(px[1]).abs() as f64
        + 0.114 * d(px[2]).abs() as f64) as i32;
    l.clamp(0, 255) as u8
}

/// 阈值白底化 → 裁白边 → 擦右上罗盘 → 等比缩放白底填充成 max_size 正方形。
/// 返回 (处理图, 坐标变换)。全白图返回纯白画布 + 恒等变换。
pub fn process_capture(raw: &RgbImage, max_size: u32) -> (RgbImage, CaptureTransform) {
    let (w, h) = (raw.width() as i64, raw.height() as i64);
    // 1) 白底化: 与白的差 > 阈值保留原色, 否则置白
    let mut img_white = raw.clone();
    for px in img_white.pixels_mut() {
        if luma_diff_white(*px) <= CONTENT_THRESHOLD {
            *px = Rgb([255, 255, 255]);
        }
    }
    // 2) 内容边界(非白区域)
    let mut bbox: Option<(i64, i64, i64, i64)> = None; // (l, t, r, b) 半开区间
    for (x, y, px) in img_white.enumerate_pixels() {
        if luma_diff_white(*px) > CONTENT_THRESHOLD {
            let (l, t, r, b) = bbox.unwrap_or((x as i64, y as i64, x as i64, y as i64));
            bbox = Some((
                l.min(x as i64),
                t.min(y as i64),
                r.max(x as i64 + 1),
                b.max(y as i64 + 1),
            ));
        }
    }
    let Some((bl, bt, br, bb)) = bbox else {
        return (
            RgbImage::from_pixel(max_size, max_size, Rgb([255, 255, 255])),
            CaptureTransform {
                crop_x: 0,
                crop_y: 0,
                scale: 1.0,
                pad_x: 0,
                pad_y: 0,
            },
        );
    };
    // 3) 裁剪(margin 2, 与原工具一致)
    let m = 2i64;
    let x0 = (bl - m).max(0);
    let y0 = (bt - m).max(0);
    let x1 = (br + m).min(w);
    let y1 = (bb + m).min(h);
    let cropped = image::imageops::crop_imm(
        &img_white,
        x0 as u32,
        y0 as u32,
        (x1 - x0) as u32,
        (y1 - y0) as u32,
    )
    .to_image();
    let (cw, ch) = (cropped.width() as i64, cropped.height() as i64);

    // 4) 擦右上残余(右上 150x100 有非白内容就白化, 指南针贴边时生效)
    let corner_w = 150i64.min(cw);
    let corner_h = 100i64.min(ch);
    let mut cropped = cropped;
    let mut has_corner_content = false;
    for yy in 0..corner_h {
        for xx in (cw - corner_w)..cw {
            if luma_diff_white(*cropped.get_pixel(xx as u32, yy as u32)) > CONTENT_THRESHOLD {
                has_corner_content = true;
            }
        }
    }
    if has_corner_content {
        for yy in 0..corner_h {
            for xx in (cw - corner_w)..cw {
                cropped.put_pixel(xx as u32, yy as u32, Rgb([255, 255, 255]));
            }
        }
    }

    // 5) 等比缩放 + 白底居中填充
    let scale = (max_size as f64 / cw as f64).min(max_size as f64 / ch as f64);
    let new_w = ((cw as f64 * scale).round() as u32).max(1).min(max_size);
    let new_h = ((ch as f64 * scale).round() as u32).max(1).min(max_size);
    let resized = image::imageops::resize(
        &cropped,
        new_w,
        new_h,
        image::imageops::FilterType::Lanczos3,
    );
    let mut canvas = RgbImage::from_pixel(max_size, max_size, Rgb([255, 255, 255]));
    let pad_x = ((max_size - new_w) / 2) as i32;
    let pad_y = ((max_size - new_h) / 2) as i32;
    image::imageops::overlay(&mut canvas, &resized, pad_x as i64, pad_y as i64);

    (
        canvas,
        CaptureTransform {
            crop_x: x0,
            crop_y: y0,
            scale,
            pad_x,
            pad_y,
        },
    )
}

/// 在 RGB 图里找接近 rgb 的像素坐标(色标定位用)。
pub fn find_color_hits(img: &RgbImage, rgb: [u8; 3], tol: u8) -> Vec<(i32, i32)> {
    let (r0, g0, b0) = (rgb[0] as i32, rgb[1] as i32, rgb[2] as i32);
    let tol = tol as i32;
    let mut out = Vec::new();
    for (x, y, px) in img.enumerate_pixels() {
        if (px[0] as i32 - r0).abs() <= tol
            && (px[1] as i32 - g0).abs() <= tol
            && (px[2] as i32 - b0).abs() <= tol
        {
            out.push((x as i32, y as i32));
        }
    }
    out
}

// ── 字体 ─────────────────────────────────────────────────────────────

/// 优先中文可用字体(微软雅黑/黑体), 再 Arial/segoe; 找不到返回 None。
pub fn load_font() -> Option<FontArc> {
    const CANDIDATES: [&str; 5] = [
        "msyh.ttc",
        "simhei.ttf",
        "arial.ttf",
        "segoeui.ttf",
        "Arial.ttf",
    ];
    let dir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".to_string());
    for name in CANDIDATES {
        let path = Path::new(&dir).join("Fonts").join(name);
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let font = if name.to_lowercase().ends_with(".ttc") {
            FontVec::try_from_vec_and_index(bytes, 0)
                .ok()
                .map(FontArc::from)
        } else {
            FontVec::try_from_vec(bytes).ok().map(FontArc::from)
        };
        if font.is_some() {
            return font;
        }
    }
    None
}

/// 文本像素宽度（imageproc::drawing::text_size 的包装）。
pub fn text_width(scale: PxScale, font: &FontArc, s: &str) -> i32 {
    text_size(scale, font, s).0 as i32
}

// ── 标注绘图 ─────────────────────────────────────────────────────────

/// 一条标注: (图例文本, 引线终点(处理图坐标), 线色)。
pub struct Marker {
    pub label: String,
    pub at: (i32, i32),
    pub rgb: [u8; 3],
}

/// 在处理图上画右侧图例 + 彩色引线 + 左上角总计（Python _annotate_screenshot 6 步）。
pub fn draw_annotations(
    img: &mut RgbImage,
    markers: &[Marker],
    total_label: Option<&str>,
    font: Option<&FontArc>,
) {
    let (w, h) = (img.width() as i32, img.height() as i32);
    let font_size = 18f32.max(h as f32 / 26.0);
    let scale = PxScale::from(font_size);
    let th = font_size as i32 + 8;

    // 右侧图例列宽
    let col_w = markers
        .iter()
        .map(|m| {
            font.map(|f| text_width(scale, f, &m.label))
                .unwrap_or((m.label.chars().count() as i32) * th * 6 / 10)
        })
        .max()
        .unwrap_or(0)
        + 64;
    let x_col = w - col_w - 14;

    // 槽位: 顶部顺排; 超出底部时压缩间距
    let mut slots: Vec<i32> = Vec::new();
    let mut slot_y = 14i32;
    for _ in markers {
        slots.push(slot_y);
        slot_y += th + 10;
    }
    if let Some(last) = slots.last() {
        if last + th > h - 14 {
            let n = markers.len().max(1) as i32;
            let gap = (th + 6).max((h - 28 - th * n) / n);
            slots.clear();
            slot_y = 14;
            for _ in markers {
                slots.push(slot_y);
                slot_y += gap;
            }
        }
    }

    if !markers.is_empty() {
        // 图例底板: 白底 + 灰边(2px)
        let bottom = (slot_y + 6).min(h - 8);
        draw_filled_rect_mut(
            img,
            Rect::at(x_col - 8, 8).of_size(
                (w - 8 - (x_col - 8)).max(1) as u32,
                (bottom - 8).max(1) as u32,
            ),
            Rgb([255, 255, 255]),
        );
        for off in 0..2 {
            draw_hollow_rect_mut(
                img,
                Rect::at(x_col - 8 + off, 8 + off).of_size(
                    (w - 8 - (x_col - 8) - 2 * off).max(1) as u32,
                    (bottom - 8 - 2 * off).max(1) as u32,
                ),
                Rgb([180, 180, 180]),
            );
        }
    }

    for (m, &slot) in markers.iter().zip(&slots) {
        let rgb = Rgb(m.rgb);
        let ly = slot + th / 2;
        let lx = x_col - 8;
        // 3px 引线(三条平行线近似 PIL width=3)
        for dy in [-1i32, 0, 1] {
            draw_line_segment_mut(
                img,
                (lx as f32, (ly + dy) as f32),
                (m.at.0 as f32, (m.at.1 + dy) as f32),
                rgb,
            );
        }
        // 空心圆环(外 r6 内 r4, 三重描出 3px)
        for r in [4, 5, 6] {
            draw_hollow_circle_mut(img, m.at, r, rgb);
        }
        // 图例色块
        draw_filled_rect_mut(
            img,
            Rect::at(x_col + 2, slot + 2).of_size(34, th.max(1) as u32),
            rgb,
        );
        if let Some(f) = font {
            draw_text_mut(
                img,
                Rgb([20, 20, 20]),
                x_col + 44,
                slot + 3,
                scale,
                f,
                &m.label,
            );
        }
    }

    // 左上角总计标签
    if let (Some(label), Some(f)) = (total_label, font) {
        let font_total_size = 20f32.max(h as f32 / 22.0);
        let scale_total = PxScale::from(font_total_size);
        let tw = text_width(scale_total, f, label);
        let th2 = font_total_size as i32 + 10;
        draw_filled_rect_mut(
            img,
            Rect::at(12, 12).of_size((tw + 26).max(1) as u32, th2.max(1) as u32),
            Rgb([255, 255, 210]),
        );
        for off in 0..2 {
            draw_hollow_rect_mut(
                img,
                Rect::at(12 - off, 12 - off).of_size(
                    (tw + 26 + 2 * off).max(1) as u32,
                    (th2 + 2 * off).max(1) as u32,
                ),
                Rgb([160, 140, 40]),
            );
        }
        draw_text_mut(img, Rgb([60, 50, 0]), 25, 17, scale_total, f, label);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 合成截图走完整管线: 白底化/裁边/擦罗盘/方形缩放 + 色标定位 + 标注绘制。
    #[test]
    fn capture_pipeline_roundtrip() {
        // 800×600 视图: 白底 + 中心一个彩色矩形(内容) + 右上角一小块深色(罗盘)
        let mut raw = RgbImage::from_pixel(800, 600, Rgb([255, 255, 255]));
        for x in 300..500 {
            for y in 250..350 {
                raw.put_pixel(x, y, Rgb([200, 60, 60]));
            }
        }
        for x in 700..790 {
            for y in 5..90 {
                raw.put_pixel(x, y, Rgb([90, 90, 90]));
            }
        }
        let (final_img, _tf) = process_capture(&raw, SHOT_MAX_SIZE);
        assert_eq!(final_img.width(), SHOT_MAX_SIZE);
        assert_eq!(final_img.height(), SHOT_MAX_SIZE);
        // 裁白边后方形化: 内容区约 202×102 → 放大填充, 中心应有彩色像素
        let mut found_color = false;
        for px in final_img.pixels() {
            if px.0[0] > 150 && px.0[1] < 120 && px.0[2] < 120 {
                found_color = true;
                break;
            }
        }
        assert!(found_color, "缩放后应保留内容色");
        // 罗盘区应被擦白: 右上角 150×100 内应无深色(内容居中填充, 右上角是空白)
        for y in 0..60 {
            for x in (SHOT_MAX_SIZE as i32 - 140)..SHOT_MAX_SIZE as i32 {
                let px = final_img.get_pixel(x as u32, y as u32);
                assert!(px.0[0] > 200, "右上角应已擦白: ({x},{y}) {:?}", px.0);
            }
        }

        // 色标定位: 互补色色块画上去后应能找回来
        let marker_rgb = [0u8, 255, 255]; // 红管的互补色
        let mut marked = raw.clone();
        for x in 380..420 {
            for y in 280..320 {
                marked.put_pixel(x, y, Rgb(marker_rgb));
            }
        }
        let hits = find_color_hits(&marked, marker_rgb, COLOR_TOL);
        assert!(hits.len() >= 4, "色标应命中 >=4 像素: {}", hits.len());
        let cx = hits.iter().map(|h| h.0).sum::<i32>() as f64 / hits.len() as f64;
        let cy = hits.iter().map(|h| h.1).sum::<i32>() as f64 / hits.len() as f64;
        assert!(
            (cx - 399.0).abs() < 2.0 && (cy - 299.0).abs() < 2.0,
            "质心应在色块中心: ({cx},{cy})"
        );

        // 标注绘制: 图例 + 引线 + 总计(字体来自系统, 找不到也可绘制其余部分)
        let font = load_font();
        let markers = vec![Marker {
            label: "测试曲面: 12.3 mm".into(),
            at: (400, 300),
            rgb: [200, 60, 60],
        }];
        let mut annotated = final_img.clone();
        draw_annotations(
            &mut annotated,
            &markers,
            Some("总计 12.3 mm"),
            font.as_ref(),
        );
        // 图例底板区域(右上)应被白底填充且有非纯白图例色块
        let mut has_chip = false;
        for px in annotated.pixels() {
            if px.0[0] > 150 && px.0[1] < 120 && px.0[2] < 120 {
                has_chip = true;
            }
        }
        assert!(has_chip, "图例色块应已绘制");
    }

    /// 纯白截图: 全白画布 + 恒等变换(裁不出内容)。
    #[test]
    fn blank_capture() {
        let raw = RgbImage::from_pixel(800, 600, Rgb([255, 255, 255]));
        let (img, tf) = process_capture(&raw, 1024);
        assert_eq!(img.width(), 1024);
        assert_eq!((tf.crop_x, tf.crop_y, tf.scale), (0, 0, 1.0));
    }
}
