//! 批量产品分析 —— batch_product_analysis.py 的 Rust 版。
//!
//! 流程：活动 Product 树递归收集 Part 路径（按已开文档名匹配）→ 去重 →
//! 逐件 打开 / 边界框(vba_bbox_generate) / 投影面积(rule_projected_surface) /
//! ISO 截图(隐藏轴系平面辅助几何 + 白底 + 480px 内容裁剪) / 关闭。
//!
//! 与 Python 版的差异（有意为之）：
//! - 子进程调用改为同进程直调（STA 会话内），无 RESULT: 解析层；
//! - CaptureToFile(4) 直接出 PNG（rule_bounding_box 已验证的通道），不走 BMP 中转；
//! - Excel 汇总由应用层 rust_xlsxwriter 基于返回 JSON 生成，本模块只出数据。

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use rscatiacli::objects::{Application, Documents};
use rscatiacli::{attach_with_guard, ComCollection, ComObject, ComResult, Progress};

use crate::rule_projected_surface;
use crate::vba_bbox_generate;

/// 截图最长边（Python 版 max_size=480）。
const SHOT_MAX_EDGE: u32 = 480;
/// 近白阈值（RGB 各通道与 255 的最大差 ≤ 30 视为背景白）。
const WHITE_THRESHOLD: i32 = 30;

pub fn run(params: &Value, progress: Progress<'_>) -> ComResult<Value> {
    let dry_run = params
        .get("dry_run")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let app = attach_with_guard(progress)?;
    // 禁止 CATIA 弹任何对话框（保存提醒等会卡住批量流程）。
    let _ = app.set_display_file_alerts(false);
    let docs = app.documents()?;

    let doc = app
        .active_document()
        .map_err(|_| rscatiacli::ComError::runtime("无打开文档"))?;
    let product = doc
        .product_document()
        .product()
        .map_err(|_| rscatiacli::ComError::runtime("当前文档不是 Product,请激活一个 .CATProduct"))?;

    // ── 收集所有 Part 路径（递归遍历整棵产品树） ──
    progress("[批量] 递归遍历产品树收集 Part ...");
    let mut part_paths: Vec<(String, String)> = Vec::new();
    collect_parts(&product, &docs, &mut part_paths, &progress);
    progress(&format!("[列表] 共收集 {} 个 Part 路径", part_paths.len()));

    // 去重：同一个文件只处理一次
    let mut seen: Vec<String> = Vec::new();
    let mut unique: Vec<(String, String)> = Vec::new();
    for (name, path) in part_paths {
        if !seen.contains(&path) {
            seen.push(path.clone());
            unique.push((name, path));
        }
    }
    progress(&format!("[去重] {} 个唯一文件", unique.len()));

    let root_children = product
        .products()
        .and_then(|p| p.count())
        .unwrap_or(0);

    if dry_run {
        for (idx, (name, path)) in unique.iter().enumerate() {
            progress(&format!("  [{}] {name} -> {path}", idx + 1));
        }
        return Ok(json!({
            "success": true,
            "message": format!("DRY RUN: 将处理 {} 个 Part", unique.len()),
            "batch": [],
            "batch_total": root_children,
            "batch_success": 0,
            "excel": Value::Null,
        }));
    }

    // Excel 输出目录：与 Product 同目录
    let product_dir = doc
        .full_name()
        .ok()
        .and_then(|fp| PathBuf::from(&fp).parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("D:/claw.forvia/cplugin"));
    let _ = std::fs::create_dir_all(&product_dir);
    // Excel 文件名基（应用层 rust_xlsxwriter 用）：文档名去扩展名
    let product_name = doc
        .name()
        .ok()
        .and_then(|n| PathBuf::from(&n).file_stem().map(|s| s.to_string_lossy().to_string()))
        .unwrap_or_else(|| "product".into());

    // ── 逐个处理（核心循环） ──
    let mut batch: Vec<Value> = Vec::new();
    let total = unique.len();
    for (idx, (ref_name, target_path)) in unique.iter().enumerate() {
        progress(&format!("\n=== [{}/{}] {ref_name} ===", idx + 1, total));
        let file_name = Path::new(target_path)
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| ref_name.clone());

        let mut item = json!({"name": ref_name, "path": target_path});

        // ── 1. 打开文档 ──
        if let Err(e) = docs.open(target_path) {
            progress(&format!("[跳过] {ref_name}: 打开失败 {e}"));
            item["success"] = json!(false);
            item["message"] = json!(format!("打开失败: {e}"));
            item["bbox"] = json!({"success": false, "message": "未打开"});
            item["area"] = json!({"success": false, "message": "未打开"});
            item["screenshot"] = json!({"success": false, "message": "未打开"});
            batch.push(item);
            continue;
        }
        progress(&format!("[打开] {ref_name}"));

        // ── 2. 边界框分析（测量后删除临时几何） ──
        progress(&format!("[边框] {file_name} ..."));
        let bbox = vba_bbox_generate::run(
            &json!({"file_path": target_path, "keep_geometry": false}),
            progress,
        )
        .unwrap_or_else(|e| json!({"success": false, "message": e.to_string()}));
        if bbox.get("success").and_then(|v| v.as_bool()) == Some(true) {
            if let Some(d) = bbox.get("data") {
                progress(&format!(
                    "  -> L={} W={} H={} V={}",
                    d.get("length_mm").and_then(|v| v.as_f64()).map_or("?".into(), |v| format!("{v:.2}")),
                    d.get("width_mm").and_then(|v| v.as_f64()).map_or("?".into(), |v| format!("{v:.2}")),
                    d.get("height_mm").and_then(|v| v.as_f64()).map_or("?".into(), |v| format!("{v:.2}")),
                    d.get("volume_mm3").and_then(|v| v.as_f64()).map_or("?".into(), |v| format!("{v:.1}")),
                ));
            }
        } else {
            let msg = bbox.get("message").and_then(|v| v.as_str()).unwrap_or("跳过");
            progress(&format!("  -> {}", &msg.chars().take(80).collect::<String>()));
        }
        item["bbox"] = bbox;

        // ── 3. 投影面积分析 ──
        progress(&format!("[投影] {file_name} ..."));
        let area = rule_projected_surface::run(&json!({"file_path": target_path}), progress)
            .unwrap_or_else(|e| json!({"success": false, "message": e.to_string()}));
        if area.get("success").and_then(|v| v.as_bool()) == Some(true) {
            if let Some(d) = area.get("data") {
                progress(&format!(
                    "  -> XY投影={}",
                    d.get("projected_area_xy_mm2").and_then(|v| v.as_f64()).map_or("?".into(), |v| format!("{v:.2}"))
                ));
            }
        } else {
            let msg = area.get("message").and_then(|v| v.as_str()).unwrap_or("跳过");
            progress(&format!("  -> {}", &msg.chars().take(80).collect::<String>()));
        }
        item["area"] = area;

        // ── 4. 截图（PNG，输出到 Product 同目录；bbox/area 可能切走了活动文档） ──
        progress(&format!("[截图] {file_name} ..."));
        let png_path = product_dir.join(format!("{ref_name}.png"));
        let _ = docs.open(target_path); // 重新激活
        let shot = screenshot(&app, &png_path, &progress)
            .unwrap_or_else(|e| json!({"success": false, "message": e}));
        if shot.get("success").and_then(|v| v.as_bool()) == Some(true) {
            progress(&format!(
                "  截图 OK: {} ({}KB)",
                Path::new(shot.get("file").and_then(|v| v.as_str()).unwrap_or(""))
                    .file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default(),
                shot.get("size_kb").and_then(|v| v.as_f64()).unwrap_or(0.0),
            ));
        } else {
            progress(&format!(
                "  截图 FAIL: {}",
                shot.get("message").and_then(|v| v.as_str()).unwrap_or("")
            ));
        }
        item["screenshot"] = shot;

        // ── 5. 关闭文档（释放内存） ──
        if let Ok(cur) = app.active_document() {
            let is_part = cur
                .full_name()
                .ok()
                .and_then(|fp| Path::new(&fp).extension().map(|e| e.to_string_lossy().to_lowercase()))
                .map(|ext| ext == "catpart")
                .unwrap_or(false);
            if is_part {
                let _ = cur.close();
                progress(&format!("[关闭] {ref_name}"));
            }
        }

        // 综合成功标志：任一分析成功即认为该 part 处理成功
        let ok = ["screenshot", "bbox", "area"]
            .iter()
            .any(|k| item[*k].get("success").and_then(|v| v.as_bool()) == Some(true));
        item["success"] = json!(ok);
        progress(&format!("[完成] {ref_name} (success={ok})"));
        batch.push(item);
    }

    let success_count = batch
        .iter()
        .filter(|it| it.get("success").and_then(|v| v.as_bool()) == Some(true))
        .count();

    // ── 6. 恢复 Product 文档为活动文档 ──
    if let Ok(product_path) = root_product_reference_path(&app, &docs) {
        let _ = docs.open(&product_path);
    }

    progress(&format!(
        "\n[完成] {success_count}/{} 处理成功",
        unique.len()
    ));

    Ok(json!({
        "success": true,
        "message": format!("批量分析完成: {success_count}/{}", unique.len()),
        "batch": batch,
        "batch_total": unique.len(),
        "batch_success": success_count,
        "product_name": product_name,
        "product_dir": product_dir.to_string_lossy(),
        // Excel 由应用层生成后回填
        "excel": Value::Null,
    }))
}

/// Product 文档的恢复路径：优先活动文档若是 ProductDocument 直接取 FullName，
/// 否则遍历已开文档按 .CATProduct 找。
fn root_product_reference_path(app: &Application, docs: &Documents) -> ComResult<String> {
    if let Ok(cur) = app.active_document() {
        if cur.product_document().product().is_ok() {
            if let Ok(fp) = cur.full_name() {
                return Ok(fp);
            }
        }
    }
    for d in docs.items()? {
        if let Ok(name) = d.name() {
            if name.to_lowercase().ends_with(".catproduct") {
                return d.full_name();
            }
        }
    }
    Err(rscatiacli::ComError::runtime("未找到 Product 文档"))
}

/// 递归遍历产品树，收集所有 Part 引用路径（按已开文档名匹配）。
fn collect_parts(
    product: &rscatiacli::objects::Product,
    docs: &Documents,
    out: &mut Vec<(String, String)>,
    progress: &dyn Fn(&str),
) {
    let Ok(children) = product.products() else {
        return;
    };
    let Ok(items) = children.items() else {
        return;
    };
    let opened = docs.items().unwrap_or_default();
    for child in &items {
        // 子产品的参考名（= 文档 stem，如 "Part1"）
        let ref_name = match child.reference_product().and_then(|r| r.name()) {
            Ok(n) => n,
            Err(_) => continue,
        };
        // 尝试从已打开文档中匹配（dn 形如 "Part1.CATPart"）
        let mut found: Option<String> = None;
        for d in &opened {
            if let Ok(dn) = d.name() {
                if dn.replace(".CATPart", "").replace(".catpart", "") == ref_name {
                    if let Ok(fp) = d.full_name() {
                        found = Some(fp);
                    }
                    break;
                }
            }
        }
        match found {
            Some(path) => {
                progress(&format!("  [找到] {ref_name} -> {path}"));
                out.push((ref_name, path));
            }
            None => {
                // 可能是子 Product，递归
                collect_parts(child, docs, out, progress);
            }
        }
    }
}

/// 截图：ISO 视角 + 平行投影 + 白底 + 隐藏轴系/平面/辅助 Body/几何图形集
/// + Reframe + CaptureToFile(4=PNG) + 内容裁剪/擦指南针/480px 缩放。
fn screenshot(
    app: &Application,
    png_path: &Path,
    progress: &dyn Fn(&str),
) -> Result<Value, String> {
    let win = app
        .active_window()
        .map_err(|e| format!("无活动窗口: {e}"))?;
    let orig_layout = win.layout().ok();
    let _ = win.set_layout(1); // catWindowGeomOnly - 隐藏规范树

    let viewer = win.active_viewer().map_err(|e| format!("无查看器: {e}"))?;

    // 等轴侧视图 + 平行投影 + 白色背景
    if let Ok(vp) = viewer.viewpoint3d() {
        let _ = vp.put_sight_direction([-1.0, -1.0, -1.0]);
        let _ = vp.put_up_direction([0.0, 0.0, 1.0]);
        let _ = vp.set_projection_mode(1);
    }
    let _ = viewer.put_background_color([1.0, 1.0, 1.0]);

    // 隐藏轴系、参考平面、其他 Body、几何图形集（仅 PartDocument）
    if let Ok(doc) = app.active_document() {
        if let Ok(part) = doc.part_document().part() {
            if let Ok(sel) = doc.selection() {
                let hide = |objs: Vec<rscatiacli::objects::Entity>| {
                    if objs.is_empty() {
                        return;
                    }
                    let _ = sel.clear();
                    for o in &objs {
                        let _ = sel.add(o);
                    }
                    if let Ok(vp) = sel.vis_properties() {
                        let _ = vp.set_show(1);
                    }
                    let _ = sel.clear();
                };
                if let Ok(items) = part.axis_systems().and_then(|c| c.items()) {
                    hide(items.into_iter().map(|a| rscatiacli::objects::Entity::new(a.com().clone())).collect());
                }
                if let Ok(items) = part.planes().and_then(|c| c.items()) {
                    hide(items);
                }
                let main_name = part.main_body().and_then(|b| b.name()).ok();
                if let Ok(items) = part.bodies().and_then(|c| c.items()) {
                    let others: Vec<rscatiacli::objects::Entity> = items
                        .into_iter()
                        .filter(|b| {
                            main_name.as_deref()
                                != b.name().ok().as_deref()
                        })
                        .map(|b| rscatiacli::objects::Entity::new(b.com().clone()))
                        .collect();
                    hide(others);
                }
                if let Ok(items) = part.hybrid_bodies().and_then(|c| c.items()) {
                    hide(items.into_iter().map(|h| rscatiacli::objects::Entity::new(h.com().clone())).collect());
                }
            }
        }
    }

    let _ = viewer.reframe();
    let _ = viewer.update();

    viewer
        .capture_to_file(4, &png_path.to_string_lossy())
        .map_err(|e| format!("CaptureToFile 失败: {e}"))?;

    // 恢复 Layout 和背景色
    if let Some(l) = orig_layout {
        let _ = win.set_layout(l);
    }
    let _ = viewer.put_background_color([0.2, 0.2, 0.4]);

    // ── 后处理：近白统一 → 内容裁剪 → 擦指南针 → 480px ──
    postprocess_shot(png_path)?;

    let size_kb = std::fs::metadata(png_path)
        .map(|m| (m.len() as f64 / 1024.0 * 10.0).round() / 10.0)
        .unwrap_or(0.0);
    progress(&format!(
        "[PNG] {} | {size_kb:.1}KB",
        png_path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default()
    ));
    Ok(json!({
        "success": true,
        "file": png_path.to_string_lossy(),
        "size_kb": size_kb,
    }))
}

fn pixel_white_diff(p: image::Rgb<u8>) -> i32 {
    let [r, g, b] = p.0;
    (255 - r as i32).max((255 - g as i32).max(255 - b as i32))
}

/// 后处理（对齐 _screenshot 的 PIL 管线）：
/// 近白统一为纯白 → 中心扩展内容裁剪(±2px) → 擦右上指南针 → 最长边 480。
fn postprocess_shot(path: &Path) -> Result<(), String> {
    let img = image::open(path).map_err(|e| format!("打开截图失败: {e}"))?;
    let mut img = img.to_rgb8();

    // 近白统一为纯白
    for p in img.pixels_mut() {
        if pixel_white_diff(*p) <= WHITE_THRESHOLD {
            *p = image::Rgb([255, 255, 255]);
        }
    }

    // 中心扩展裁剪：从中心向四边找内容边界（±2px 余量）
    let (w, h) = (img.width() as i64, img.height() as i64);
    let (cx, cy) = (w / 2, h / 2);
    let content = |x: i64, y: i64| pixel_white_diff(*img.get_pixel(x as u32, y as u32)) > WHITE_THRESHOLD;
    let row_has_content = |y: i64| (0..w).any(|x| content(x, y));
    let col_has_content = |x: i64| (0..h).any(|y| content(x, y));
    let mut top = cy;
    for y in (0..=cy).rev() {
        if row_has_content(y) { top = y; } else { break; }
    }
    let mut bottom = cy;
    for y in cy..h {
        if row_has_content(y) { bottom = y; } else { break; }
    }
    let mut left = cx;
    for x in (0..=cx).rev() {
        if col_has_content(x) { left = x; } else { break; }
    }
    let mut right = cx;
    for x in cx..w {
        if col_has_content(x) { right = x; } else { break; }
    }
    let top = (top - 2).max(0);
    let bottom = (bottom + 2).min(h - 1);
    let left = (left - 2).max(0);
    let right = (right + 2).min(w - 1);
    if right > left && bottom > top {
        img = image::imageops::crop_imm(&img, left as u32, top as u32, (right - left + 1) as u32, (bottom - top + 1) as u32).to_image();
    }

    // 擦除右上残余（指南针可能在边缘附近）
    let (cw, ch) = (img.width(), img.height());
    if cw > 150 && ch > 100 {
        let mut dirty = false;
        for y in 0..100u32 {
            for x in (cw - 150)..cw {
                if pixel_white_diff(*img.get_pixel(x, y)) > WHITE_THRESHOLD {
                    dirty = true;
                    break;
                }
            }
            if dirty { break; }
        }
        if dirty {
            for y in 0..100u32 {
                for x in (cw - 150)..cw {
                    *img.get_pixel_mut(x, y) = image::Rgb([255, 255, 255]);
                }
            }
        }
    }

    // 缩放最长边到 480px
    let (w2, h2) = (img.width(), img.height());
    let img = if w2.max(h2) > SHOT_MAX_EDGE {
        let scale = SHOT_MAX_EDGE as f64 / w2.max(h2) as f64;
        let nw = (w2 as f64 * scale).round() as u32;
        let nh = (h2 as f64 * scale).round() as u32;
        image::imageops::resize(&img, nw.max(1), nh.max(1), image::imageops::FilterType::Lanczos3)
    } else {
        img
    };

    img.save_with_format(path, image::ImageFormat::Png)
        .map_err(|e| format!("保存 PNG 失败: {e}"))
}
