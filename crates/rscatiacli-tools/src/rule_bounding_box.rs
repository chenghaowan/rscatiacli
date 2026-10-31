//! rule-bounding-box：规则驱动的边界框分析（独立 CLI 契约工具，UI 面板走
//! vba-bbox-generate）。
//!
//! 对应 ltl-manage resources/sidecars/product_analysis/rule_bounding_box.py。
//! find_draftline → Z、halfline/角度扫描 → X（量测核心在 rule_axis.rs）；
//! 建参数化 'tooling axis' 轴系（suppress_geometry 可关）；可选 Rough Stock
//! 绿色线框；导出 {零件名}_bbox.json；截图（隐藏树/轴系 → 白底 → Reframe →
//! ×0.7 缩放 → CaptureToFile(PNG) → 裁剪/缩放/256 色量化，见 image_post.rs）。

use std::path::{Path, PathBuf};

use rscatiacli::dispatch::Arg;
use rscatiacli::objects::{Application, Part, Selection, SystemService};
use rscatiacli::{attach_with_guard, ComCollection, ComObject, ComResult, Progress};
use serde_json::{json, Value};

use crate::image_post;
use crate::rule_axis::{create_coordinate_system, measure_axis_facts};
use crate::vba_bbox_generate::{cleanup_old, cross, dot, norm, Vec3};

type Vec3f = Vec3;

fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

/// UTC 时间 ISO8601（与 rule_projected_surface 同口径，仅记录用途）。
fn local_now_iso() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let days = secs.div_euclid(86400);
    let sod = secs.rem_euclid(86400);
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        sod / 3600,
        (sod % 3600) / 60,
        sod % 60
    )
}

/// 天数 → (年, 月, 日)（Howard Hinnant civil_from_days）。
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

// ── Rough Stock 线框 ─────────────────────────────────────────────────

/// 按包围盒中心 + 三向轴画绿色线框盒（8 角点 + 12 棱线；Python create_rough_stock）。
#[allow(clippy::too_many_arguments)]
fn create_rough_stock(
    part: &Part,
    cmd_sel: &Selection,
    length: f64,
    width: f64,
    height: f64,
    center: Vec3,
    x_axis: Vec3,
    y_axis: Vec3,
    z_axis: Vec3,
    progress: Progress<'_>,
) -> bool {
    if length == 0.0 || width == 0.0 || height == 0.0 {
        return false;
    }
    let attempt = (|| -> Result<(), String> {
        // 清旧（RghStockWire/RghStockBox/RoughStock）
        cleanup_old(
            part,
            cmd_sel,
            &["rghstockwire", "rghstockbox", "roughstock"],
        );
        let hsf = part.hybrid_shape_factory().map_err(|e| e.to_string())?;
        let wb = part
            .hybrid_bodies()
            .and_then(|h| h.add())
            .map_err(|e| e.to_string())?;
        wb.set_name("RghStockWire").map_err(|e| e.to_string())?;

        let (cx, cy, cz) = (center[0], center[1], center[2]);
        let (h_l, h_w, h_h) = (length / 2.0, width / 2.0, height / 2.0);
        // 8 角点（itertools.product([-1,1], 3) 顺序：sx 外层、sz 内层）
        let mut corners: Vec<rscatiacli::objects::Reference> = Vec::new();
        for sx in [-1.0f64, 1.0] {
            for sy in [-1.0f64, 1.0] {
                for sz in [-1.0f64, 1.0] {
                    let x = cx + sx * h_l * x_axis[0] + sy * h_w * y_axis[0] + sz * h_h * z_axis[0];
                    let y = cy + sx * h_l * x_axis[1] + sy * h_w * y_axis[1] + sz * h_h * z_axis[1];
                    let z = cz + sx * h_l * x_axis[2] + sy * h_w * y_axis[2] + sz * h_h * z_axis[2];
                    let pt = hsf
                        .add_new_point_coord(x, y, z)
                        .map_err(|e| e.to_string())?;
                    wb.append_hybrid_shape(&pt).map_err(|e| e.to_string())?;
                    corners.push(
                        part.create_reference_from_object(&pt)
                            .map_err(|e| e.to_string())?,
                    );
                }
            }
        }
        part.update().map_err(|e| e.to_string())?;

        // 12 条棱（底面 4 + 顶面 4 + 竖棱 4，与 Python edges_idx 一致）
        const EDGES: [(usize, usize); 12] = [
            (0, 1),
            (1, 3),
            (3, 2),
            (2, 0),
            (4, 5),
            (5, 7),
            (7, 6),
            (6, 4),
            (0, 4),
            (1, 5),
            (2, 6),
            (3, 7),
        ];
        for (i, j) in EDGES {
            let ln = hsf
                .add_new_line_pt_pt(&corners[i], &corners[j])
                .map_err(|e| e.to_string())?;
            wb.append_hybrid_shape(&ln).map_err(|e| e.to_string())?;
        }
        part.update().map_err(|e| e.to_string())?;

        // 绿色（best-effort）
        let _ = cmd_sel.clear();
        let _ = cmd_sel.add(&wb);
        if let Ok(vp) = cmd_sel.vis_properties() {
            let _ = vp.set_real_color(0, 255, 0, 1);
        }
        let _ = cmd_sel.clear();
        Ok(())
    })();
    if let Err(e) = attempt {
        progress(&format!("[rough_stock] 创建失败(忽略): {e}"));
        return false;
    }
    true
}

// ── 截图 ─────────────────────────────────────────────────────────────

/// 截图：隐藏树/轴系 → 白底 → Reframe → ×0.7 缩放 → CaptureToFile(PNG)
/// → 裁剪/缩放/量化（Python capture_screenshot）。
fn capture_screenshot(
    app: &Application,
    part: &Part,
    cmd_sel: &Selection,
    ss: &SystemService,
    part_name: &str,
    output_dir: &Path,
    progress: Progress<'_>,
) -> Result<String, String> {
    let safe_name = part_name.replace([' ', '/'], "_");
    let png_path: PathBuf = output_dir.join(format!("{safe_name}_bbox.png"));
    let attempt = (|| -> Result<(), String> {
        let window = app.active_window().map_err(|e| e.to_string())?;
        let viewer = window.active_viewer().map_err(|e| e.to_string())?;
        // 隐藏规格树
        viewer.set_tree_visible(false).map_err(|e| e.to_string())?;

        // 隐藏轴系（best-effort：借用/自身选中集 + VisProperties.SetShow(1)）
        if let Ok(axes) = part.axis_systems() {
            if let Ok(items) = axes.items() {
                if !items.is_empty() {
                    let _ = cmd_sel.clear();
                    for ax in items.iter() {
                        let _ = cmd_sel.add(ax);
                    }
                    if let Ok(vp) = cmd_sel.vis_properties() {
                        let _ = vp.set_show(1);
                    }
                    let _ = cmd_sel.clear();
                }
            }
        }

        // 白背景 + 全屏适配
        viewer
            .put_background_color([1.0, 1.0, 1.0])
            .map_err(|e| e.to_string())?;
        viewer.reframe().map_err(|e| e.to_string())?;

        // 小幅缩小视角（Python 经 SystemService.Evaluate 注入的 zoom_out）
        let code = r#"Public Function rscatiacli_zoom_out(v)
    Dim vp
    Set vp = v.Viewpoint3D
    vp.Zoom = vp.Zoom * 0.7
    v.Update
    rscatiacli_zoom_out = "done"
End Function"#;
        match ss.evaluate(
            code,
            0,
            "rscatiacli_zoom_out",
            &[Arg::Obj(viewer.com().clone())],
        ) {
            Ok(_) => {}
            Err(e) => progress(&format!("[screenshot] zoom_out 失败(忽略): {e}")),
        }

        // Capture PNG（4 = catCaptureFormatPNG）+ 后处理
        viewer
            .capture_to_file(4, &png_path.to_string_lossy())
            .map_err(|e| e.to_string())?;
        image_post::postprocess_png(&png_path, 800)?;
        Ok(())
    })();
    match attempt {
        Ok(()) => Ok(png_path.to_string_lossy().into_owned()),
        Err(e) => {
            // 失败清掉半成品文件（与 Python 一致）
            let _ = std::fs::remove_file(&png_path);
            Err(e)
        }
    }
}

// ── 主流程 ───────────────────────────────────────────────────────────

pub fn run(params: &Value, progress: Progress<'_>) -> ComResult<Value> {
    let file_path = params
        .get("file_path")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.replace('/', "\\"));
    let suppress_geometry = params
        .get("suppress_geometry")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let rough_stock = params
        .get("rough_stock")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let suppress_screenshot = params
        .get("suppress_screenshot")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let output_dir_param = params
        .get("output_dir")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let app = attach_with_guard(progress)?;
    let doc = match &file_path {
        Some(p) => app.documents()?.open(p)?,
        None => app.active_document()?,
    };
    let doc_name = doc.name().unwrap_or_default();
    if !doc_name.to_lowercase().ends_with(".catpart") {
        return Ok(json!({
            "success": false,
            "message": format!("期望 PartDocument, 得到 {doc_name}")
        }));
    }
    let prt_doc = doc.part_document();

    // 强制 DESIGN 模式（CGR 可视化模式下 Measurable/extract/extremum 全部失效）
    match doc
        .product_document()
        .product()
        .and_then(|p| p.apply_work_mode(2))
    {
        Ok(()) => progress("[workmode] 已强制 DESIGN 模式"),
        Err(e) => progress(&format!("[workmode] ApplyWorkMode 失败(继续尝试测量): {e}")),
    }

    let part = prt_doc.part()?;
    let ss = app.system_service()?;
    let spa = prt_doc.spa_workbench()?;

    let part_name = part.name().unwrap_or_default();
    // 零件号优先取 Product.PartNumber（与 apply_naming 写入同源；不做命名规则精炼，
    // rule_bounding_box 与 Python 版一致导出原文）
    let catia_pn = doc
        .product_document()
        .product()
        .and_then(|p| p.part_number())
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    progress(&format!(
        "[naming] CATIA 零件号={catia_pn:?} 零件名={part_name:?}"
    ));

    // 命令类操作落在激活编辑器的选中集上
    let (cmd_sel, borrowed_sel) = crate::vba_bbox_generate::command_selection(&app, &prt_doc)?;
    let sel_snapshot = if borrowed_sel {
        progress("[target] 零件在装配中打开: 命令操作改用激活装配的选中集");
        crate::vba_bbox_generate::snapshot_selection(&cmd_sel)
    } else {
        Vec::new()
    };

    let result = (|| -> ComResult<Value> {
        // Step 1-3: draftline/halfline 定向 + 三向量测（rule_axis 量测核心）
        let facts = match measure_axis_facts(&prt_doc, &ss, &spa, &cmd_sel, borrowed_sel, progress)
        {
            Ok(v) => v,
            Err(e) => return Ok(json!({ "success": false, "message": e })),
        };
        let measure = &facts.measure;
        let z = norm(facts.dl.direction);
        let length_axis = measure.length_axis;

        // JSON 记录的坐标系与 Python 一致: X=长边方向, Y=draftline 初始正交(不随 X 重算)
        let ref_vec = if dot(z, [0.0, 0.0, 1.0]).abs() > 0.99 {
            [1.0, 0.0, 0.0]
        } else {
            [0.0, 0.0, 1.0]
        };
        let y_export = norm(cross(z, norm(cross(z, ref_vec))));
        let cog = measure.cog;

        // Step 4: 建 'tooling axis' 轴系（suppress_geometry 时跳过）
        if !suppress_geometry {
            match create_coordinate_system(&part, &cmd_sel, z, length_axis, cog) {
                Ok(_) => progress(&format!("[axis] create_coordinate_system origin={cog:?}")),
                Err(e) => progress(&format!("[axis] WARN create_coordinate_system failed: {e}")),
            }
        }

        // Step 4.5: Rough Stock 绿色线框（默认关）
        if rough_stock && !suppress_geometry {
            create_rough_stock(
                &part,
                &cmd_sel,
                measure.length_mm,
                measure.width_mm,
                measure.height_mm,
                cog,
                length_axis,
                measure.width_axis,
                z,
                progress,
            );
        }

        // Step 4.6: 若脚本打开了文件, 保存文档以持久化新建的坐标系
        let mut saved = false;
        if file_path.is_some() {
            saved = doc
                .save()
                .or_else(|_| part.update().and_then(|_| doc.save()))
                .is_ok();
        }

        // Step 5: 导出 JSON
        let output_dir: PathBuf = match output_dir_param.as_deref() {
            Some(d) if !d.is_empty() => PathBuf::from(d),
            _ => doc
                .full_name()
                .ok()
                .and_then(|p| PathBuf::from(&p).parent().map(|x| x.to_path_buf()))
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or_else(|| PathBuf::from("C:\\temp")),
        };
        let json_path = match export_json(
            &part_name,
            &catia_pn,
            measure,
            &z,
            &y_export,
            &cog,
            &facts.dl,
            &output_dir,
        ) {
            Ok(p) => p,
            Err(e) => return Ok(json!({ "success": false, "message": e })),
        };

        let bbox_data = json!({
            "length_mm": round3(measure.length_mm),
            "width_mm": round3(measure.width_mm),
            "height_mm": round3(measure.height_mm),
            "volume_mm3": round1(measure.length_mm * measure.width_mm * measure.height_mm),
            "bbox_center_mm": cog,
            "dims": {
                "length_axis": length_axis,
                "width_axis": measure.width_axis,
                "height_axis": z,
            },
        });

        if borrowed_sel {
            crate::vba_bbox_generate::restore_selection(
                &cmd_sel,
                &crate::vba_bbox_generate::snapshot_selection(&cmd_sel),
            );
        }

        // Step 6: 截图（suppress_screenshot 用于批量模式）
        if suppress_screenshot {
            return Ok(json!({
                "success": true,
                "message": "Bounding box analysis complete (screenshot suppressed)",
                "data": bbox_data,
                "json_path": json_path,
                "image_path": null,
                "saved": saved,
                "used_halfline": facts.used_halfline,
                "part_number": catia_pn,
                "part_name": part_name,
            }));
        }
        match capture_screenshot(
            &app,
            &part,
            &cmd_sel,
            &ss,
            &part_name,
            &output_dir,
            progress,
        ) {
            Ok(png) => Ok(json!({
                "success": true,
                "message": "Bounding box analysis complete",
                "data": bbox_data,
                "json_path": json_path,
                "image_path": png,
                "saved": saved,
                "used_halfline": facts.used_halfline,
                "part_number": catia_pn,
                "part_name": part_name,
            })),
            Err(e) => Ok(json!({
                "success": true,
                "message": "Bounding box computed, but screenshot failed",
                "data": bbox_data,
                "json_path": json_path,
                "image_path": null,
                "screenshot_error": e,
                "saved": saved,
                "used_halfline": facts.used_halfline,
                "part_number": catia_pn,
                "part_name": part_name,
            })),
        }
    })();

    match result {
        Ok(v) => Ok(v),
        Err(e) => {
            if borrowed_sel {
                crate::vba_bbox_generate::restore_selection(&cmd_sel, &sel_snapshot);
            }
            Ok(json!({
                "success": false,
                "message": format!("rule_bounding_box failed: {e}")
            }))
        }
    }
}

/// 结果写 {output_dir}/{part_name}_bbox.json（Python export_json 同形）。
#[allow(clippy::too_many_arguments)]
fn export_json(
    part_name: &str,
    part_number: &str,
    measure: &crate::rule_axis::AxisMeasure,
    z: &Vec3,
    y_export: &Vec3,
    cog: &Vec3,
    dl: &crate::vba_bbox_generate::LineInfo,
    output_dir: &Path,
) -> Result<String, String> {
    std::fs::create_dir_all(output_dir).map_err(|e| format!("export_json failed: {e}"))?;
    let safe_name = part_name.replace([' ', '/'], "_");
    let filepath = output_dir.join(format!("{safe_name}_bbox.json"));
    let payload = json!({
        "part_name": part_name,
        "part_number": part_number,
        "analysis_time": local_now_iso(),
        "bounding_box": {
            "length_mm": round3(measure.length_mm),
            "width_mm": round3(measure.width_mm),
            "height_mm": round3(measure.height_mm),
            "volume_mm3": round1(measure.length_mm * measure.width_mm * measure.height_mm),
        },
        "coordinate_system": {
            "origin_mm": cog,
            "x_axis": measure.length_axis,
            "y_axis": y_export,
            "z_axis": z,
        },
        "draftline": {
            "name": dl.name,
            "start_mm": dl.start_mm,
            "end_mm": dl.end_mm,
        },
    });
    let text =
        serde_json::to_string_pretty(&payload).map_err(|e| format!("export_json failed: {e}"))?;
    std::fs::write(&filepath, text).map_err(|e| format!("export_json failed: {e}"))?;
    Ok(filepath.to_string_lossy().into_owned())
}

/// 保留 Vec3f 别名引用（跨模块共享 vba_bbox_generate 的 Vec3）。
#[allow(dead_code)]
type Vec3Alias = Vec3f;
