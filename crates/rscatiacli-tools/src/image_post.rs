//! 截图后处理（对应 Python 版 PIL 流程）。
//!
//! rule_bounding_box.capture_screenshot 与 measure_harness 共用：
//! 打开 PNG → 裁剪中心区（15%,10% → 90%,90%）→ 最长边 >800 时等比缩小 →
//! 量化 256 色（NeuQuant，PIL MEDIANCUT 的等价物）→ 写回 8-bit 调色板 PNG8。
//! 量化把截图压到几十 KB，前端 base64 内嵌/预览不吃带宽。

use std::io::BufWriter;
use std::path::Path;

/// 按 Python capture_screenshot 的后处理规则就地改写 PNG。
pub fn postprocess_png(path: &Path, max_size: u32) -> Result<(), String> {
    let img = image::open(path).map_err(|e| format!("打开截图失败: {e}"))?;
    let rgba = img.to_rgba8();
    let (w, h) = (rgba.width(), rgba.height());

    // 裁剪中心区：left=15%, top=10%, right=90%, bottom=90%
    let cx0 = (w as f64 * 0.15) as u32;
    let cy0 = (h as f64 * 0.10) as u32;
    let cx1 = (w as f64 * 0.90) as u32;
    let cy1 = (h as f64 * 0.90) as u32;
    let cropped = if cx0 < cx1 && cy0 < cy1 {
        image::imageops::crop_imm(&rgba, cx0, cy0, cx1 - cx0, cy1 - cy0).to_image()
    } else {
        rgba
    };
    let (w, h) = (cropped.width(), cropped.height());

    // 最长边 > max_size 时等比缩小（LANCZOS，对齐 PIL LANCZOS）
    let resized = if w.max(h) > max_size {
        let ratio = max_size as f64 / w.max(h) as f64;
        let nw = ((w as f64 * ratio) as u32).max(1);
        let nh = ((h as f64 * ratio) as u32).max(1);
        image::imageops::resize(&cropped, nw, nh, image::imageops::FilterType::Lanczos3)
    } else {
        cropped
    };
    let (w, h) = (resized.width(), resized.height());
    if w == 0 || h == 0 {
        return Err("截图尺寸异常(空图像)".to_string());
    }

    // 量化 256 色 → PNG8（调色板 + 索引）
    let rgba_bytes: Vec<u8> = resized.into_raw();
    let nq = color_quant::NeuQuant::new(1, 256, &rgba_bytes);
    let indices: Vec<u8> = rgba_bytes
        .chunks_exact(4)
        .map(|px| nq.index_of(px) as u8)
        .collect();
    let palette_rgba = nq.color_map_rgba();
    let palette_rgb: Vec<u8> = palette_rgba
        .chunks_exact(4)
        .flat_map(|px| [px[0], px[1], px[2]])
        .collect();

    let file = std::fs::File::create(path).map_err(|e| format!("写截图失败: {e}"))?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), w, h);
    encoder.set_color(png::ColorType::Indexed);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_palette(palette_rgb);
    let mut writer = encoder
        .write_header()
        .map_err(|e| format!("写截图头失败: {e}"))?;
    writer
        .write_image_data(&indices)
        .map_err(|e| format!("写截图数据失败: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 合成 PNG 走一遍后处理：1000×600 → 裁剪(150,60)-(900,540)=750×480，
    /// 最长边 750 ≤ 800 不缩放；量化后仍可解码且尺寸不变。
    #[test]
    fn crop_resize_quantize_roundtrip() {
        let dir = std::env::temp_dir().join("rscatiacli_image_post_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("post_test.png");

        let img = image::RgbaImage::from_fn(1000, 600, |x, y| {
            image::Rgba([(x % 256) as u8, (y % 256) as u8, ((x + y) % 256) as u8, 255])
        });
        img.save(&path).unwrap();

        postprocess_png(&path, 800).unwrap();

        let out = image::open(&path).unwrap();
        assert_eq!((out.width(), out.height()), (750, 480));
        std::fs::remove_file(&path).ok();

        // 超大图触发等比缩小: 1600×800 → 裁剪(240,80)-(1440,720)=1200×640 → 缩到 800×426
        let path2 = dir.join("post_test2.png");
        let big = image::RgbaImage::from_fn(1600, 800, |x, y| {
            image::Rgba([(x / 8) as u8, (y / 8) as u8, 128, 255])
        });
        big.save(&path2).unwrap();
        postprocess_png(&path2, 800).unwrap();
        let out2 = image::open(&path2).unwrap();
        assert_eq!(out2.width(), 800);
        assert_eq!(out2.height(), 426);
        std::fs::remove_file(&path2).ok();
    }
}
