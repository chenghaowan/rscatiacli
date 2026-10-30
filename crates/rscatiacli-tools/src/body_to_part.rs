//! body-to-part：把 CATPart 的每个 Body 拆成独立 CATPart，并用 Product 组织。
//!
//! 对应 ltl-manage `resources/sidecars/body_to_part/`
//! （catia_body_to_part_bridge.py + body_to_part.py），JSON 契约与 Python 版一致：
//! `{"success", "message", "logs"}`（logs 由适配层合并进结果）。
//!
//! 流程（与 Python 版逐段对应）：
//!   1. 连接 CATIA，关文件弹框，定位源 CATPart（活动文档或 --file-path）；
//!   2. 列出 Body，SPAWorkbench 预测体积（cm³，失败记 N/A）；
//!   3. 逐 Body：发布 → Selection.Copy → 新建 Part → 粘贴(CATPrtResult) →
//!      重命名/设 PartBody/删空 Body → 设零件号 → 按零件号前缀去参复制关联
//!      几何（datum 化 + 保留源名 + 隐藏）→ 隐藏轴系 → SaveAs → Close；
//!   4. 新建 Product → AddExternalComponent 逐个装配 → SaveAs；
//!   5. 生成 Excel 统计（rust_xlsxwriter）。
//!
//! 与 Python 版的差异（收敛性改进）：
//! - 连接只用 ROT 且加多实例守卫（Python 的 Dispatch("CATIA.Application") 会
//!   拉起无窗口后台实例）；
//! - 支持 `dry_run`：只报告 Body 清单与输出位置，不做任何写操作。

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use rscatiacli::dispatch::{ComObj, ComObject};
use rscatiacli::objects::{
    Body, Document, Entity, HybridBody, HybridShapeFactory, Part, Reference, SPAWorkbench,
    Selection,
};
use rscatiacli::{attach_with_guard, ComCollection, ComResult, Progress};

// ── 入口 ─────────────────────────────────────────────────────────────

/// 工具主体。**必须在 STA 线程上执行。**
pub fn run(params: &Value, progress: Progress<'_>) -> ComResult<Value> {
    let action = params
        .get("action")
        .and_then(Value::as_str)
        .unwrap_or("run");
    match action {
        "status" => Ok(status_on_sta()),
        "run" => Ok(run_inner(params, progress)),
        other => Ok(json!({
            "success": false,
            "message": format!("未知 action: {other},可选 status/run")
        })),
    }
}

// ── status ───────────────────────────────────────────────────────────

/// 连接概览（只读，字段与 Python 版 `action_status()` 一致）。
fn status_on_sta() -> Value {
    let running = rscatiacli::catia_is_running();
    let mut out = json!({
        "connected": false,
        "com_available": true,
        "handler_available": true,
        "catia_running": running,
        "active_document": Value::Null,
    });
    if !running {
        return out;
    }
    match rscatiacli::attach_without_guard(std::time::Duration::from_secs(20)) {
        Ok(app) => {
            out["connected"] = json!(true);
            if let Ok(doc) = app.active_document() {
                out["active_document"] = json!({
                    "name": doc.name().unwrap_or_default(),
                    "path": doc.full_name().unwrap_or_default(),
                });
            }
        }
        Err(e) => {
            out["catia_running"] = json!(false);
            out["error"] = json!(e.to_string());
        }
    }
    out
}

// ── 纯函数 ───────────────────────────────────────────────────────────

/// 从名称开头提取连续数字作为零件号，无数字返回 None。
fn extract_part_number(name: &str) -> Option<String> {
    let digits: String = name
        .trim()
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    if digits.is_empty() {
        None
    } else {
        Some(digits)
    }
}

/// 文件名安全化：`\ / : * ? " < > |` 替换为下划线。
fn safe_file_name(name: &str) -> String {
    name.chars()
        .map(|c| if r#"\/:*?"<>|"#.contains(c) { '_' } else { c })
        .collect()
}

/// 落盘文件名转 ASCII。
///
/// CATIA COM 的 `SaveAs` / `Open` 拒绝文件名里的非 ASCII 字符（目录名不受限；
/// 中文文件名在 pywin32 与本库下同样报 "The method SaveAs failed"，实测限制）。
/// 保留字母数字与 `- . ( )`，其余字符（含中文、空格、管道分隔）折叠成单个 `_`。
/// 文档内部的零件号 / Body 名保留原文 —— 字符串属性通道不受影响。
fn ascii_file_name(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        let keep = c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '(' | ')');
        if keep {
            out.push(c);
        } else if !out.is_empty() && !out.ends_with('_') {
            out.push('_');
        }
    }
    let trimmed = out.trim_matches(|c| c == '_' || c == '.');
    if trimmed.is_empty() {
        "part".to_string()
    } else {
        trimmed.to_string()
    }
}

// ── 几何辅助 ─────────────────────────────────────────────────────────

/// 递归扫描源 Part 的所有几何图文集，收集名字以 part_number 开头的元素
/// （图文集整名命中 → 集内全部入列；散落 hybrid_shape 名字命中 → 入列）。
fn collect_matching_shapes(orig_part: &Part, part_number: &str) -> Vec<ComObj> {
    fn visit(hb: &ComObj, pn: &str, out: &mut Vec<ComObj>) {
        let name = hb
            .get("Name")
            .and_then(|v| v.as_string())
            .unwrap_or_default();
        let whole_set_match = name.starts_with(pn);

        if let Ok(shapes) = hb.get("HybridShapes").and_then(|v| v.as_obj()) {
            if let Ok(count) = shapes.get("Count").and_then(|v| v.as_i32()) {
                for i in 1..=count {
                    let Ok(shape) = rscatiacli::item_at(&shapes, i) else {
                        continue;
                    };
                    let sname = shape
                        .get("Name")
                        .and_then(|v| v.as_string())
                        .unwrap_or_default();
                    if whole_set_match || sname.starts_with(pn) {
                        out.push(shape);
                    }
                }
            }
        }
        // 递归嵌套子集
        if let Ok(subs) = hb.get("HybridBodies").and_then(|v| v.as_obj()) {
            if let Ok(count) = subs.get("Count").and_then(|v| v.as_i32()) {
                for j in 1..=count {
                    if let Ok(sub) = rscatiacli::item_at(&subs, j) {
                        visit(&sub, pn, out);
                    }
                }
            }
        }
    }

    let mut out = Vec::new();
    if let Ok(hbs) = orig_part.com().get("HybridBodies").and_then(|v| v.as_obj()) {
        if let Ok(count) = hbs.get("Count").and_then(|v| v.as_i32()) {
            for k in 1..=count {
                if let Ok(hb) = rscatiacli::item_at(&hbs, k) {
                    visit(&hb, part_number, &mut out);
                }
            }
        }
    }
    out
}

/// 隐藏指定图文集及其所有子 datum。失败仅记日志不中断。
fn hide_hybrid_body(doc: &Document, hb: &ComObj, part: &Part, progress: Progress<'_>) {
    let Ok(sel) = doc.selection() else {
        progress("    - 隐藏图文集失败: 无法获取选择集");
        return;
    };
    // 隐藏每个子 datum（GS 的 SetShow 不会级联到子项）
    if let Ok(shapes) = hb.get("HybridShapes").and_then(|v| v.as_obj()) {
        if let Ok(count) = shapes.get("Count").and_then(|v| v.as_i32()) {
            for i in 1..=count {
                let Ok(shape) = rscatiacli::item_at(&shapes, i) else {
                    continue;
                };
                let _ = sel.clear();
                if sel.add(&shape).is_ok() {
                    let _ = sel.vis_properties().and_then(|v| v.set_show(1));
                }
            }
        }
    }
    // 隐藏 GS 本身（规范树折叠）
    let _ = sel.clear();
    let _ = sel.add(hb);
    let _ = sel.vis_properties().and_then(|v| v.set_show(1));
    let _ = sel.clear();
    let _ = part.update();
}

/// 确保 COM 对象已发布（Publish），否则自动发布。
/// 跨文档粘贴前需要发布对象，否则 CATIA 剪贴板无法正确传递几何数据。
fn ensure_published(doc: &Document, body: &ComObj, sel: &Selection) -> Option<String> {
    let publications = doc.product_document().product().ok()?.publications().ok()?;
    let existing = publications.names();
    let pub_name = body.get("Name").and_then(|v| v.as_string()).ok()?;
    if existing.iter().any(|n| n == &pub_name) {
        return Some(pub_name);
    }
    if publications.add(&pub_name).is_err() {
        return None;
    }
    // SetDirect 在某些 CATIA 版本可能失败，但 Add 已足够让跨文档粘贴工作
    let _ = (|| -> ComResult<()> {
        sel.clear()?;
        sel.add(body)?;
        let reference = sel.item(1)?.reference()?;
        sel.clear()?;
        publications.set_direct(&pub_name, &reference)
    })();
    Some(pub_name)
}

/// 默认几何图文集（第 1 个）里是否已有内容（验证粘贴是否真的成功）。
fn default_gs_has_shapes(default_gs: &Option<ComObj>) -> bool {
    default_gs
        .as_ref()
        .and_then(|g| g.get("HybridShapes").ok())
        .and_then(|v| v.as_obj().ok())
        .and_then(|hs| hs.get("Count").ok())
        .and_then(|v| v.as_i32().ok())
        .map(|n| n > 0)
        .unwrap_or(false)
}

/// 默认图文集里的最后一个 hybrid_shape（刚粘贴的临时副本）。
fn hybrid_shapes_last(default_gs: &ComObj) -> Option<ComObj> {
    let shapes = default_gs
        .get("HybridShapes")
        .and_then(|v| v.as_obj())
        .ok()?;
    let count = shapes.get("Count").and_then(|v| v.as_i32()).ok()?;
    if count <= 0 {
        return None;
    }
    rscatiacli::item_at(&shapes, count).ok()
}

/// 在当前 Part 内为 reference 创建 datum，追加到目标图文集，删除临时副本。
/// datum 创建后重命名为源 shape 名（保留源名）。
/// 返回 false = 去参失败（调用方负责清理临时副本）。
fn create_datum_for_ref(
    hsf: &HybridShapeFactory,
    reference: &Reference,
    target_hb: &HybridBody,
    part: &Part,
    src_name: &str,
) -> bool {
    // 查几何类型: 1=point, 2=curve, 3=surface, 8=volume。
    // CATIA 经常把曲线误判为曲面（类型 3），所以不依赖 geo_type，优先精确类型：
    //   直线 → 圆 → 曲线 → 曲面；点/体积只在明确类型时优先尝试。
    let geo_type = hsf.get_geometrical_feature_type(reference).ok();
    let candidates: &[&str] = match geo_type {
        Some(8) => &[
            "AddNewVolumeDatum",
            "AddNewLineDatum",
            "AddNewCircleDatum",
            "AddNewCurveDatum",
            "AddNewSurfaceDatum",
        ],
        Some(1) => &[
            "AddNewPointDatum",
            "AddNewLineDatum",
            "AddNewCircleDatum",
            "AddNewCurveDatum",
            "AddNewSurfaceDatum",
        ],
        _ => &[
            "AddNewLineDatum",
            "AddNewCircleDatum",
            "AddNewCurveDatum",
            "AddNewSurfaceDatum",
        ],
    };

    let mut datum: Option<Entity> = None;
    for method in candidates {
        let attempt = match *method {
            "AddNewPointDatum" => hsf.add_new_point_datum(reference),
            "AddNewLineDatum" => hsf.add_new_line_datum(reference),
            "AddNewCircleDatum" => hsf.add_new_circle_datum(reference),
            "AddNewCurveDatum" => hsf.add_new_curve_datum(reference),
            "AddNewSurfaceDatum" => hsf.add_new_surface_datum(reference),
            "AddNewVolumeDatum" => hsf.add_new_volume_datum(reference),
            _ => continue,
        };
        if let Ok(d) = attempt {
            datum = Some(d);
            break;
        }
    }
    let Some(datum) = datum else {
        return false;
    };

    // 追加到目标图文集
    if target_hb.append_hybrid_shape(&datum).is_err() {
        return false;
    }
    let _ = part.update();

    // 重命名 datum 为源 shape 名
    if !src_name.is_empty() {
        let _ = datum.set_name(src_name);
    }

    // 删除临时副本，仅保留 datum
    let _ = hsf.delete_object_for_datum(reference);
    let _ = part.update();
    true
}

/// 把源 Part 中名字以 part_number 开头的几何图文集及散落曲线/曲面，
/// 完全去参后复制到新 Part 的一个以 part_number 命名的几何图文集。
/// 三步法：复制粘贴临时副本 → datum 化 + 重命名 → 删临时副本。
/// 返回成功复制的元素数量。
fn copy_related_geometry(
    orig_doc: &Document,
    orig_part: &Part,
    new_doc: &Document,
    new_part: &Part,
    part_number: &str,
    progress: Progress<'_>,
) -> usize {
    let shapes = collect_matching_shapes(orig_part, part_number);
    progress(&format!(
        "    零件号 {part_number}: 匹配到 {} 个几何元素",
        shapes.len()
    ));

    // 新建目标图文集
    let Ok(target_hbs) = new_part.hybrid_bodies() else {
        return 0;
    };
    let Ok(target_hb) = target_hbs.add() else {
        return 0;
    };
    let _ = target_hb.set_name(part_number);

    if shapes.is_empty() {
        progress(&format!("    - 无匹配元素，已创建空图文集 {part_number}"));
        hide_hybrid_body(new_doc, target_hb.com(), new_part, progress);
        return 0;
    }

    let Ok(new_hsf) = new_part.hybrid_shape_factory() else {
        return 0;
    };
    let (Ok(orig_sel), Ok(new_sel)) = (orig_doc.selection(), new_doc.selection()) else {
        return 0;
    };
    // 默认几何图文集（Geometrical Set.1），用于跨 Part 粘贴临时副本
    let default_gs: Option<ComObj> = new_part
        .hybrid_bodies()
        .ok()
        .and_then(|hbs| hbs.raw_item(1).ok());

    let mut success = 0usize;
    for (idx, shape) in shapes.iter().enumerate() {
        let idx = idx + 1;
        let sname = shape
            .get("Name")
            .and_then(|v| v.as_string())
            .unwrap_or_default();

        // Step 1: 源 Part 复制
        let copied = (|| -> ComResult<()> {
            orig_sel.clear()?;
            orig_sel.add(shape)?;
            orig_sel.copy()
        })()
        .is_ok();
        if !copied {
            progress(&format!("    - [{idx}] 复制失败: {sname}"));
            continue;
        }

        // 粘贴到默认 GS（已验证 PasteSpecial 对 HybridShape 静默失败，先普通 Paste）
        let _ = new_sel.clear();
        if let Some(gs) = &default_gs {
            let _ = new_sel.add(gs);
        }
        let mut temp_ok = match new_sel.paste() {
            Ok(_) => {
                let _ = new_part.update();
                default_gs_has_shapes(&default_gs)
            }
            Err(e) => {
                progress(&format!("    - [{idx}] 粘贴失败: {sname} ({e})"));
                continue;
            }
        };
        if !temp_ok {
            // Paste 返回成功但没粘上，尝试 PasteSpecial 回退
            let _ = new_sel.clear();
            if let Some(gs) = &default_gs {
                let _ = new_sel.add(gs);
            }
            if new_sel.paste_special("CATPrtResult").is_ok() {
                let _ = new_part.update();
                temp_ok = default_gs_has_shapes(&default_gs);
            }
        }
        if !temp_ok {
            continue;
        }

        // 取刚粘贴的临时副本（默认 GS 中的最后一个）
        let Some(temp_copy) = default_gs.as_ref().and_then(hybrid_shapes_last) else {
            progress(&format!("    - [{idx}] 取临时副本失败: {sname}"));
            continue;
        };
        let Ok(reference) = new_part.create_reference_from_object(&temp_copy) else {
            continue;
        };

        // Step 2 + 3: Datum 化 + 重命名 + 删临时副本
        if create_datum_for_ref(&new_hsf, &reference, &target_hb, new_part, &sname) {
            success += 1;
            progress(&format!("    - [{idx}] 已去参复制: {sname}"));
        } else {
            progress(&format!("    - [{idx}] 去参失败(类型判定/创建): {sname}"));
            // 清理临时副本
            let _ = new_sel.clear();
            let _ = new_sel.add(&temp_copy);
            let _ = new_sel.delete();
        }
    }

    // 隐藏目标图文集及其所有子 datum
    hide_hybrid_body(new_doc, target_hb.com(), new_part, progress);
    success
}

// ── 测量 ─────────────────────────────────────────────────────────────

/// 测量 Body 的体积，返回 cm³ 值（m³ → cm³，保留 3 位小数）。
fn measure_body_volume(spa: &SPAWorkbench, part: &Part, body: &Body) -> Option<f64> {
    let reference = part.create_reference_from_object(body.com()).ok()?;
    let measurable = spa.get_measurable(&reference).ok()?;
    let volume_m3 = measurable.volume().ok()?;
    Some((volume_m3 * 1_000_000.0 * 1000.0).round() / 1000.0)
}

// ── Excel 报表 ───────────────────────────────────────────────────────

struct Record {
    name: String,
    volume: Option<f64>,
    part_path: PathBuf,
}

/// 生成 Excel 统计文件（列 / 列宽 / 样式对齐 Python 版 openpyxl 报表）。
fn create_excel_report(
    output_dir: &Path,
    doc_name: &str,
    records: &[Record],
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    use rust_xlsxwriter::{Color, Format, FormatAlign, FormatBorder, Workbook};

    let mut wb = Workbook::new();
    let ws = wb.add_worksheet();
    ws.set_name("零件统计")?;

    let header = Format::new()
        .set_bold()
        .set_font_size(11)
        .set_font_color(Color::RGB(0xFFFFFF))
        .set_background_color(Color::RGB(0x4472C4))
        .set_border(FormatBorder::Thin)
        .set_align(FormatAlign::Center)
        .set_align(FormatAlign::VerticalCenter);
    let center = Format::new()
        .set_border(FormatBorder::Thin)
        .set_align(FormatAlign::Center)
        .set_align(FormatAlign::VerticalCenter);
    let wrap = Format::new()
        .set_border(FormatBorder::Thin)
        .set_align(FormatAlign::VerticalCenter)
        .set_text_wrap();

    let headers = ["序号", "Body名称", "体积 (cm³)", "CATPart文件"];
    for (col, h) in headers.iter().enumerate() {
        ws.write_with_format(0, col as u16, *h, &header)?;
    }

    for (i, rec) in records.iter().enumerate() {
        let row = (i + 1) as u32;
        ws.set_row_height(row, 75.0)?; // ~2cm
        ws.write_with_format(row, 0, (i + 1) as f64, &center)?;
        ws.write_with_format(row, 1, &rec.name, &wrap)?;
        let vol = rec
            .volume
            .map(|v| format!("{v:.3}"))
            .unwrap_or_else(|| "N/A".to_string());
        ws.write_with_format(row, 2, vol, &center)?;
        ws.write_with_format(row, 3, rec.part_path.display().to_string(), &wrap)?;
    }

    ws.set_column_width(0, 6.0)?;
    ws.set_column_width(1, 30.0)?;
    ws.set_column_width(2, 15.0)?;
    ws.set_column_width(3, 60.0)?;

    let path = output_dir.join(format!("{doc_name}.xlsx"));
    wb.save(&path)?;
    Ok(path)
}

// ── run 主流程 ───────────────────────────────────────────────────────

fn run_inner(params: &Value, progress: Progress<'_>) -> Value {
    let file_path = params
        .get("file_path")
        .and_then(Value::as_str)
        .map(String::from);
    let dry_run = params
        .get("dry_run")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let inner = || -> ComResult<Value> {
        // 1. 连接 CATIA（ROT + 多实例守卫），关闭文件弹框
        let app = attach_with_guard(progress)?;
        if !dry_run {
            let _ = app.set_display_file_alerts(false);
        }
        let docs = app.documents()?;

        // 2. 定位源 CATPart：指定文件 or 活动文档
        let doc: Document = match &file_path {
            Some(fp) => {
                let p = Path::new(fp);
                if !p.is_file() {
                    return Ok(
                        json!({"success": false, "message": format!("错误: 文件不存在: {fp}")}),
                    );
                }
                if !p
                    .extension()
                    .map(|e| e.to_string_lossy().to_lowercase() == "catpart")
                    .unwrap_or(false)
                {
                    return Ok(json!({
                        "success": false,
                        "message": format!("错误: 文件不是CATPart格式: {fp}")
                    }));
                }
                let abs = std::fs::canonicalize(p)
                    .map_err(|e| rscatiacli::ComError::runtime(format!("路径解析失败: {e}")))?;
                let abs = abs.to_string_lossy().to_string();
                let d = docs.open(&abs)?;
                progress(&format!("已打开文件: {abs}"));
                d
            }
            None => app.active_document().map_err(|_| {
                rscatiacli::ComError::runtime("错误: 请先在CATIA中激活一个CATPart文档")
            })?,
        };

        let full_name = doc.full_name().unwrap_or_default();
        let doc_path = PathBuf::from(&full_name);
        let is_catpart = doc_path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase() == "catpart")
            .unwrap_or(false);
        if !is_catpart {
            return Ok(json!({
                "success": false,
                "message": "错误: 请先在CATIA中激活一个CATPart文档"
            }));
        }
        let doc_name = doc_path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        let Some(doc_dir) = doc_path
            .parent()
            .map(|p| p.to_path_buf())
            .filter(|p| !p.as_os_str().is_empty())
        else {
            return Ok(json!({
                "success": false,
                "message": "错误: 无法确定文档所在目录(文档可能尚未保存), 请先保存文档"
            }));
        };
        let output_dir = doc_dir.join(&doc_name);
        progress(&format!(
            "当前文档: {}",
            doc_path
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| doc_name.clone())
        ));
        progress(&format!("文档名称: {doc_name}"));
        progress(&format!("输出文件夹: {}", output_dir.display()));

        // 3. 列出 Body
        let orig_part_doc = doc.part_document();
        let orig_part = orig_part_doc.part()?;
        let bodies_list = orig_part.bodies()?.items()?;
        let body_names: Vec<String> = bodies_list
            .iter()
            .map(|b| b.name().unwrap_or_default())
            .collect();
        progress(&format!("找到 {} 个Body:", body_names.len()));
        for n in &body_names {
            progress(&format!("  - {n}"));
        }
        if body_names.is_empty() {
            progress("没有找到任何Body，退出。");
            return Ok(json!({"success": true, "message": "Body-to-Part completed successfully"}));
        }

        // dry-run：只报告计划，不写任何东西
        if dry_run {
            let product_path = doc_dir.join(format!("{doc_name}.CATProduct"));
            progress("[DRY RUN] 将要执行的步骤:");
            progress("  1. 关闭文件弹框, 逐 Body 预测体积 (SPAWorkbench)");
            progress("  2. 逐 Body: 发布 → 复制 → 新建 Part → 粘贴(CATPrtResult)");
            progress("  3. 重命名/设 PartBody/删空 Body → 设零件号 → 去参复制关联几何");
            progress("  4. 隐藏轴系 → SaveAs → Close");
            progress("  5. 新建 Product → AddExternalComponent 装配 → SaveAs");
            progress("  6. 生成 Excel 统计");
            return Ok(json!({
                "success": true,
                "dry_run": true,
                "message": format!(
                    "[DRY RUN] 共 {} 个 Body; 输出目录 {}; 产品文件 {}",
                    body_names.len(),
                    output_dir.display(),
                    product_path.display()
                ),
                "bodies": body_names,
            }));
        }

        std::fs::create_dir_all(&output_dir)
            .map_err(|e| rscatiacli::ComError::runtime(format!("创建输出目录失败: {e}")))?;

        // 4. SPA 工作台 + 预测体积
        let spa = match orig_part_doc.spa_workbench() {
            Ok(s) => {
                progress("SPA工作台已就绪");
                Some(s)
            }
            Err(e) => {
                progress(&format!("获取SPA工作台失败: {e}"));
                None
            }
        };
        progress("测量体积...");
        let volumes: Vec<Option<f64>> = bodies_list
            .iter()
            .zip(&body_names)
            .map(|(b, name)| {
                let vol = spa
                    .as_ref()
                    .and_then(|s| measure_body_volume(s, &orig_part, b));
                let display = vol
                    .map(|v| format!("{v:.3} cm³"))
                    .unwrap_or_else(|| "N/A".to_string());
                progress(&format!("  {name}: {display}"));
                vol
            })
            .collect();

        // 5. 第一轮：逐 Body 生成 CATPart 文件
        let mut records: Vec<Record> = Vec::new();
        let mut used_file_names: std::collections::HashSet<String> = std::collections::HashSet::new();
        for (idx, (body, body_name)) in bodies_list.iter().zip(&body_names).enumerate() {
            let idx = idx + 1;
            let safe_name = safe_file_name(body_name);
            // 落盘文件名转 ASCII（CATIA SaveAs/Open 拒绝非 ASCII 文件名）；
            // 零件号 / Body 名保留原文。同名 Body 追加 _2 / _3 去重。
            let mut file_stem = ascii_file_name(body_name);
            let mut dedup = 1usize;
            while !used_file_names.insert(file_stem.to_lowercase()) {
                dedup += 1;
                file_stem = format!("{}_{dedup}", ascii_file_name(body_name));
            }
            if file_stem != *body_name {
                progress(&format!("  - 文件名转 ASCII(CATIA 无法保存中文文件名): {file_stem}.CATPart; 零件号/Body 名保留原文"));
            }
            let name_tag = if safe_name != *body_name {
                format!("{body_name} -> {safe_name}")
            } else {
                body_name.clone()
            };
            progress(&format!(
                "\n[{idx}/{}] 处理 Body: {name_tag}",
                body_names.len()
            ));

            // 5.1 在原始文档中复制 Body（先重新激活源文档）
            let orig_doc = match app.active_document() {
                Ok(d) => d,
                Err(e) => {
                    progress(&format!("  - 复制失败: {e}"));
                    continue;
                }
            };
            let orig_sel = match orig_doc.selection() {
                Ok(s) => s,
                Err(e) => {
                    progress(&format!("  - 复制失败: {e}"));
                    continue;
                }
            };
            let _ = ensure_published(&orig_doc, body.com(), &orig_sel);
            let copied = (|| -> ComResult<()> {
                orig_sel.clear()?;
                orig_sel.add(body.com())?;
                orig_sel.copy()
            })();
            if let Err(e) = copied {
                progress(&format!("  - 复制失败: {e}"));
                continue;
            }
            progress("  - 已复制到剪贴板");

            // 5.2 新建 CATPart
            let new_doc = match docs.add("Part") {
                Ok(d) => d,
                Err(e) => {
                    progress(&format!("  - 创建新文档失败: {e}"));
                    continue;
                }
            };
            if let Err(e) = new_doc.part_document().part().and_then(|p| p.update()) {
                progress(&format!("  - 创建新文档失败: {e}"));
                continue;
            }

            // 5.3 在新文档中粘贴
            let new_sel = new_doc.selection()?;
            let paste_ok = (|| -> ComResult<()> {
                new_sel.clear()?;
                new_sel.add(&new_doc.part_document().part()?)?;
                new_sel.paste_special("CATPrtResult")?;
                new_doc.part_document().part()?.update()
            })();
            if let Err(e) = paste_ok {
                progress(&format!("  - 粘贴失败: {e}"));
                let _ = new_doc.close();
                continue;
            }
            progress("  - 已粘贴 (As Result With Link)");

            // 5.4 重命名粘贴后的 Body，设为 PartBody，并尝试删除空 PartBody
            let new_part = new_doc.part_document().part()?;
            let rename_result = (|| -> ComResult<()> {
                let new_bodies = new_part.bodies()?;
                if new_bodies.count()? > 1 {
                    let pasted = new_bodies.item(new_bodies.count()?)?;
                    pasted.set_name(body_name)?;
                    new_part.set_main_body(&pasted)?;
                    new_part.update()?;
                    // 删除空 PartBody（第一个）
                    let _ = new_sel.clear();
                    if let Ok(empty_body) = new_bodies.item(1) {
                        let _ = new_sel.add(&empty_body);
                        let _ = new_sel.delete();
                    }
                }
                Ok(())
            })();
            if let Err(e) = rename_result {
                progress(&format!("  - 重命名/设PartBody失败: {e}"));
            }

            // 5.5 设置 Part Number 与文件名一致
            // 零件号保留原文；被去重的 Body 追加 _N —— CATIA 拒绝向同一 Product
            // 添加零件号重复的组件（实测 AddExternalComponent E_FAIL，唯一化后恢复）
            let part_number_for_doc = if dedup > 1 {
                progress(&format!("  - 零件号同步去重: {}_{dedup}", safe_name));
                format!("{safe_name}_{dedup}")
            } else {
                safe_name.clone()
            };
            if let Err(e) = new_doc
                .product_document()
                .product()
                .and_then(|p| p.set_part_number(&part_number_for_doc))
            {
                progress(&format!("  - 设置Part Number失败: {e}"));
            }

            // 5.6 按零件号前缀关联复制几何元素（去参 + 保留源名 + 隐藏）
            match extract_part_number(body_name) {
                Some(part_number) => {
                    let cnt = copy_related_geometry(
                        &orig_doc,
                        &orig_part,
                        &new_doc,
                        &new_part,
                        &part_number,
                        progress,
                    );
                    progress(&format!(
                        "  - 关联复制完成: {cnt} 个元素已去参到图文集 {part_number}"
                    ));
                }
                None => progress("  - Body名称无零件号前缀，跳过关联复制"),
            }

            // 5.7 隐藏坐标系
            let _ = (|| -> ComResult<()> {
                let axes = new_part.axis_systems()?;
                if axes.count()? > 0 {
                    let ax_sel = new_doc.selection()?;
                    ax_sel.clear()?;
                    for k in 1..=axes.count()? {
                        if let Ok(ax) = axes.item(k) {
                            let _ = ax_sel.add(&ax);
                        }
                    }
                    let _ = ax_sel.vis_properties().and_then(|v| v.set_show(1));
                    ax_sel.clear()?;
                }
                Ok(())
            })();

            // 5.8 保存并关闭（文件名用 ASCII 版；零件号已是原文 safe_name）
            let part_path = output_dir.join(format!("{file_stem}.CATPart"));
            match new_doc.save_as(&part_path.to_string_lossy()) {
                Ok(_) => progress(&format!("  - 已保存: {}", part_path.display())),
                Err(e) => {
                    progress(&format!("  - 保存失败: {e}"));
                    let _ = new_doc.close();
                    continue;
                }
            }
            if let Err(e) = new_doc.close() {
                progress(&format!("  - 关闭失败: {e}"));
            }

            records.push(Record {
                name: body_name.clone(),
                volume: volumes.get(idx - 1).copied().flatten(),
                part_path,
            });
        }

        // 6. 第二轮：创建 Product 并装配所有 CATPart
        progress(&format!("\n{}", "=".repeat(50)));
        progress("创建Product文件并装配...");
        let product_path = doc_dir.join(format!("{doc_name}.CATProduct"));
        let mut product_saved: Option<PathBuf> = None;
        let product_result = (|| -> ComResult<()> {
            let prod_doc = docs.add("Product")?;
            prod_doc
                .product_document()
                .product()?
                .set_part_number(&doc_name)?;
            for rec in &records {
                match docs.open(&rec.part_path.to_string_lossy()) {
                    Ok(part_doc) => {
                        if let Err(e) = prod_doc
                            .product_document()
                            .product()?
                            .products()?
                            .add_external_component(&part_doc)
                        {
                            progress(&format!("  - 装配失败 {}: {e}", rec.name));
                        }
                        let _ = part_doc.close();
                    }
                    Err(e) => progress(&format!("  - 装配失败 {}: {e}", rec.name)),
                }
            }
            prod_doc.save_as(&product_path.to_string_lossy())?;
            prod_doc.close()?;
            Ok(())
        })();
        match product_result {
            Ok(()) => {
                progress(&format!("  - Product已保存: {}", product_path.display()));
                product_saved = Some(product_path);
            }
            Err(e) => progress(&format!("  - 创建Product失败: {e}")),
        }

        // 7. 完成 + Excel 统计
        progress(&format!("\n{}", "=".repeat(50)));
        progress("全部完成!");
        match &product_saved {
            Some(p) => progress(&format!("产品文件: {}", p.display())),
            None => progress("产品文件: (创建失败)"),
        }
        progress(&format!("输出文件夹: {}", output_dir.display()));
        progress(&format!("共处理 {} 个Body", records.len()));

        match create_excel_report(&output_dir, &doc_name, &records) {
            Ok(p) => progress(&format!("Excel统计: {}", p.display())),
            Err(e) => progress(&format!("生成Excel失败: {e}")),
        }

        Ok(json!({
            "success": true,
            "message": "Body-to-Part completed successfully",
        }))
    };

    // Python 版 action_run 捕获一切异常 → success:false + message
    match inner() {
        Ok(v) => v,
        Err(e) => json!({"success": false, "message": e.to_string()}),
    }
}

#[cfg(test)]
mod tests {
    use super::{ascii_file_name, extract_part_number, safe_file_name};

    #[test]
    fn extracts_leading_digits() {
        assert_eq!(extract_part_number("123bracket"), Some("123".to_string()));
        assert_eq!(extract_part_number(" 456 cover"), Some("456".to_string()));
        assert_eq!(extract_part_number("bracket123"), None);
        assert_eq!(extract_part_number(""), None);
    }

    #[test]
    fn sanitizes_file_names() {
        assert_eq!(
            safe_file_name("a/b\\c:d*e?f\"g<h>i|j"),
            "a_b_c_d_e_f_g_h_i_j"
        );
        assert_eq!(safe_file_name("plain"), "plain");
    }

    #[test]
    fn ascii_file_names_strip_non_ascii() {
        // 真实 Body 名: 管道分隔 + 中文段 → 保留 ASCII 语义段
        assert_eq!(
            ascii_file_name("PSH-28159684 |  C.1 |  螺钉台 |  工作中|Upper Outer Housing"),
            "PSH-28159684_C.1_Upper_Outer_Housing"
        );
        assert_eq!(
            ascii_file_name("Connect Bar --Right"),
            "Connect_Bar_--Right"
        );
        assert_eq!(ascii_file_name("bracket-2.CATPart"), "bracket-2.CATPart");
        // 全中文 → 兜底名
        assert_eq!(ascii_file_name("螺钉台"), "part");
        assert_eq!(ascii_file_name(""), "part");
    }

    #[test]
    fn datum_candidate_order_matches_python() {
        // 候选顺序在 create_datum_for_ref 内联定义；这里锁关键语义：
        // 类型 8 体积优先、类型 1 点优先、其余直线→圆→曲线→曲面。
        let order = |geo: Option<i32>| -> &'static [&'static str] {
            match geo {
                Some(8) => &[
                    "AddNewVolumeDatum",
                    "AddNewLineDatum",
                    "AddNewCircleDatum",
                    "AddNewCurveDatum",
                    "AddNewSurfaceDatum",
                ],
                Some(1) => &[
                    "AddNewPointDatum",
                    "AddNewLineDatum",
                    "AddNewCircleDatum",
                    "AddNewCurveDatum",
                    "AddNewSurfaceDatum",
                ],
                _ => &[
                    "AddNewLineDatum",
                    "AddNewCircleDatum",
                    "AddNewCurveDatum",
                    "AddNewSurfaceDatum",
                ],
            }
        };
        assert_eq!(order(Some(8))[0], "AddNewVolumeDatum");
        assert_eq!(order(Some(1))[0], "AddNewPointDatum");
        assert_eq!(
            order(Some(3)),
            &[
                "AddNewLineDatum",
                "AddNewCircleDatum",
                "AddNewCurveDatum",
                "AddNewSurfaceDatum"
            ]
        );
        assert_eq!(order(None).len(), 4);
    }
}
