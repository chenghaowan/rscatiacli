//! rule-projected-surface：tooling axis XY 平面投影面积（排除通孔）。
//!
//! 对应 ltl-manage resources/sidecars/product_analysis/rule_projected_surface.py。
//! 主方法 = PROJECTED_AREA.CATPart 模板（InstanceFactory 实例化，轮廓法，通孔
//! 面积已排除）；模板不可用/失败时回退 Affinity 压扁法（封闭表面压到 XY 平面，
//! 顶面+底面重叠计两次 → 面积 ÷2）。结果曲面（无链接副本）保留在持久体
//! 'ProjSurf_XY'，过程几何（提取/平面/点线/Join/模板实例体）全部清理。
//!
//! 坐标系：优先已有 'tooling axis'；没有则复用 rule_bounding_box 方法建立
//! （draftline → Z、halfline/角度扫描 → X，见 rule_axis.rs）。
//! 命令类操作（Delete/Search/Copy/PasteSpecial）落在激活编辑器的选中集上
//! （零件在装配中打开时自身 Selection 会 RPC_E_SERVERFAULT）。

use std::path::PathBuf;

use rscatiacli::objects::{
    Document, HybridBody, HybridShapeAffinity, HybridShapeFactory, Part, PartDocument, Reference,
    SPAWorkbench, Selection, SystemService,
};
use rscatiacli::{attach_with_guard, ComCollection, ComObject, ComResult, Progress};
use serde_json::{json, Value};

use crate::rule_axis::{establish_tooling_axis, AxisFrame};
use crate::vba_bbox_generate::{
    cleanup_old, command_selection, dot, find_tooling_axis, restore_selection, snapshot_selection,
    Vec3,
};

type Vec3f = Vec3;

/// 投影面积结果。
struct AreaData {
    area_mm2: f64,
    volume_mm3: f64,
}

fn round4(v: f64) -> f64 {
    (v * 10000.0).round() / 10000.0
}

/// 尽力而为的选择集删除（清空 → 加入 → 删除 → 清空）。
fn sel_drop(sel: &Selection, obj: &impl ComObject) {
    let _ = sel.clear();
    if sel.add(obj).is_ok() {
        let _ = sel.delete();
    }
    let _ = sel.clear();
}

// ── 模板定位 ─────────────────────────────────────────────────────────

/// PROJECTED_AREA.CATPart 模板路径：参数显式指定优先，否则按 sidecars 资源
/// 的常见布局试探（dev 工作目录 / 打包安装目录 / catia-tools 运行时目录）。
/// 找不到返回 None（调用方回退 Affinity，与 Python 行为一致）。
fn locate_template(explicit: Option<&str>) -> Option<PathBuf> {
    if let Some(p) = explicit {
        let pb = PathBuf::from(p);
        if pb.is_file() {
            return Some(pb);
        }
    }
    const REL: &str = "resources/sidecars/product_analysis/template/PROJECTED_AREA.CATPart";
    const RUNTIME_REL: &str =
        "catia-tools/src/sidecars/product_analysis/template/PROJECTED_AREA.CATPart";
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join(REL));
            candidates.push(dir.join(RUNTIME_REL));
            if let Some(up) = dir.parent() {
                candidates.push(up.join(REL));
                candidates.push(up.join(RUNTIME_REL));
            }
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join(REL));
        if let Some(up) = cwd.parent() {
            candidates.push(up.join(REL));
        }
    }
    candidates.into_iter().find(|p| p.is_file())
}

// ── 主流程 ───────────────────────────────────────────────────────────

pub fn run(params: &Value, progress: Progress<'_>) -> ComResult<Value> {
    let file_path = params
        .get("file_path")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.replace('/', "\\"));
    let suppress_save = params
        .get("suppress_save")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let suppress_json = params
        .get("suppress_json")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let establish_allowed = params
        .get("establish_tooling_axis")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let suppress_geometry = params
        .get("suppress_geometry")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let output_dir_param = params
        .get("output_dir")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let template_path_param = params
        .get("template_path")
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
    let prt_doc: PartDocument = doc.part_document();

    // 命令类操作(Delete/Search/Copy/PasteSpecial)落在激活编辑器的选中集上
    let (cmd_sel, borrowed_sel) = command_selection(&app, &prt_doc)?;
    let sel_snapshot = if borrowed_sel {
        progress("[target] 零件在装配中打开: 命令操作改用激活装配的选中集");
        snapshot_selection(&cmd_sel)
    } else {
        Vec::new()
    };

    // 强制 DESIGN 模式（CGR 下 Measurable/extract/extremum 失效）
    match doc
        .product_document()
        .product()
        .and_then(|p| p.apply_work_mode(2))
    {
        Ok(()) => progress("[workmode] 已强制 DESIGN 模式"),
        Err(e) => progress(&format!("[workmode] ApplyWorkMode 失败(继续尝试): {e}")),
    }

    let part = prt_doc.part()?;
    let ss = app.system_service()?;
    let spa = prt_doc.spa_workbench()?;

    let part_name = part.name().unwrap_or_default();
    let catia_pn = doc
        .product_document()
        .product()
        .and_then(|p| p.part_number())
        .map(|s| s.trim().to_string())
        .unwrap_or_default();

    let result = (|| -> ComResult<Value> {
        // 坐标系: 优先已有 'tooling axis', 没有则复用边界框方法建立
        let (cs, cs_source) = match find_tooling_axis(&part, &ss, progress) {
            Some(ax) => {
                progress("[axis] 使用零件已有 'tooling axis' 坐标系");
                (
                    AxisFrame {
                        origin_mm: ax.origin_mm,
                        x_axis: ax.x_axis,
                        y_axis: ax.y_axis,
                        z_axis: ax.z_axis,
                    },
                    "existing",
                )
            }
            None => {
                if !establish_allowed {
                    return Ok(json!({
                        "success": false,
                        "message": "零件中没有 'tooling axis' 坐标系,且 establish_tooling_axis=False"
                    }));
                }
                match establish_tooling_axis(
                    &prt_doc,
                    &ss,
                    &spa,
                    &cmd_sel,
                    borrowed_sel,
                    suppress_geometry,
                    progress,
                ) {
                    Ok(frame) => {
                        progress(&format!(
                            "[axis] 零件无 'tooling axis',已按边界框方法建立坐标系 (origin={:?})",
                            frame.origin_mm
                        ));
                        (frame, "bbox_method")
                    }
                    Err(e) => return Ok(json!({ "success": false, "message": e })),
                }
            }
        };

        // 投影面积（投影平面 = tooling axis XY 平面本身, 无偏移）
        let area_data = match projected_surface_analysis(
            &prt_doc,
            &part,
            &spa,
            &ss,
            &cmd_sel,
            &cs,
            template_path_param.as_deref(),
            progress,
        ) {
            Ok(v) => v,
            Err(msg) => return Ok(json!({ "success": false, "message": msg })),
        };

        // 若脚本打开了文件且建立了新轴系, 保存文档持久化
        // (suppress_save 用于 Product 批量测量: 只出数据, 不改零件文件)
        let mut saved = false;
        if file_path.is_some() && cs_source == "bbox_method" && !suppress_save {
            saved = doc
                .save()
                .or_else(|_| part.update().and_then(|_| doc.save()))
                .is_ok();
        }

        // 导出 JSON（suppress_json 用于批量测量, 不逐件落 JSON 文件）
        let json_path = if suppress_json {
            None
        } else {
            match export_json(
                &part_name,
                &area_data,
                &cs,
                output_dir_param.as_deref(),
                &doc,
                cs_source,
            ) {
                Ok(p) => Some(p),
                Err(e) => return Ok(json!({ "success": false, "message": e })),
            }
        };

        if borrowed_sel {
            restore_selection(&cmd_sel, &sel_snapshot);
        }

        Ok(json!({
            "success": true,
            "message": "Projected surface analysis complete",
            "data": {
                "projected_area_xy_mm2": area_data.area_mm2,
                "volume_mm3": area_data.volume_mm3,
            },
            "json_path": json_path,
            "tooling_axis_source": cs_source,
            "saved": saved,
            "part_number": catia_pn,
            "part_name": part_name,
        }))
    })();

    match result {
        Ok(v) => Ok(v),
        Err(e) => {
            if borrowed_sel {
                restore_selection(&cmd_sel, &sel_snapshot);
            }
            Ok(json!({
                "success": false,
                "message": format!("projected_surface_analysis failed: {e}")
            }))
        }
    }
}

/// 计算投影面积（轮廓模板法优先，Affinity 压扁法回退），结果面保留在
/// 持久体 ProjSurf_XY；对应 Python projected_surface_analysis。
#[allow(clippy::too_many_arguments)]
fn projected_surface_analysis(
    _prt_doc: &PartDocument,
    part: &Part,
    spa: &SPAWorkbench,
    ss: &SystemService,
    cmd_sel: &Selection,
    cs: &AxisFrame,
    template_path: Option<&str>,
    progress: Progress<'_>,
) -> Result<AreaData, String> {
    let hsf = part
        .hybrid_shape_factory()
        .map_err(|e| format!("projected_surface_analysis failed: {e}"))?;

    // 主实体为空/不可测时给出明确错误, 避免后续 extract/模板链崩溃或出 0 值
    let main_body = part
        .main_body()
        .map_err(|e| format!("projected_surface_analysis failed: {e}"))?;
    let ref_body = part
        .create_reference_from_object(&main_body)
        .map_err(|e| format!("projected_surface_analysis failed: {e}"))?;
    let volume_m3 = match spa
        .get_measurable(&ref_body)
        .ok()
        .and_then(|m| m.volume().ok())
    {
        Some(v) if v > 0.0 => v,
        _ => {
            return Err(
                "主实体(main_body)为空或不可测, 无法计算投影面积; 请确保 main_body 中有实体"
                    .to_string(),
            )
        }
    };

    // 清理旧几何（含残留的模板实例体）
    cleanup_old(
        part,
        cmd_sel,
        &["ruleprojsurf", "projsurf_xy", "fis_projectedarea"],
    );

    let hybrid_body = part
        .hybrid_bodies()
        .and_then(|h| h.add())
        .map_err(|e| format!("projected_surface_analysis failed: {e}"))?;
    let _ = hybrid_body.set_name("RuleProjSurf");

    // Extract MainBody
    let extract = hsf
        .add_new_extract(&ref_body)
        .map_err(|e| format!("projected_surface_analysis failed: {e}"))?;
    let _ = extract.set_propagation_type(1);
    let _ = extract.set_complementary_extract(false);
    let _ = extract.set_is_federated(false);
    let _ = hybrid_body.append_hybrid_shape(&extract);
    let _ = part.set_in_work_object(&extract);
    let _ = hsf.gsm_visibility(&extract, 0);
    let _ = part.update();
    let ref_extract = part
        .create_reference_from_object(&extract)
        .map_err(|e| format!("projected_surface_analysis failed: {e}"))?;

    // 投影平面 = tooling axis 的 XY 平面本身（无偏移）：Ax+By+Cz+D=0
    let z = cs.z_axis;
    let o = cs.origin_mm;
    let (a, b, c) = (z[0], z[1], z[2]);
    let d = -(a * o[0] + b * o[1] + c * o[2]);
    let proj_plane = hsf
        .add_new_plane_equation(a, b, c, d)
        .map_err(|e| format!("projected_surface_analysis failed: {e}"))?;
    let _ = hybrid_body.append_hybrid_shape(&proj_plane);
    let _ = proj_plane.set_name("ProjPlane_XY");
    let _ = part.set_in_work_object(&proj_plane);
    let _ = part.update();

    // XY 平面投影（排除通孔）: 模板失败回退到 Affinity
    let mut area_xy = silhouette_outline_area(
        part,
        &hsf,
        &hybrid_body,
        &ref_extract,
        cs,
        &proj_plane,
        cmd_sel,
        spa,
        ss,
        template_path,
        progress,
    );
    if area_xy.is_none() {
        // 模板失败后, 清掉可能已损坏的临时几何, 再让 Affinity 自包含重建
        sel_drop(cmd_sel, &hybrid_body);
        let _ = part.update();
        area_xy = affinity_outline_area(part, &hsf, cmd_sel, spa, ss, cs, progress);
    }

    // 清理过程几何体（提取/平面/点线/Join）, 只保留结果面 ProjSurf_XY
    sel_drop(cmd_sel, &hybrid_body);
    let _ = part.update();

    let area = area_xy.ok_or_else(|| {
        "投影面积计算失败:轮廓模板与 Affinity 压扁两种方法都未得到结果".to_string()
    })?;
    Ok(AreaData {
        area_mm2: area,
        volume_mm3: (volume_m3 * 1_000_000_000.0 * 10.0).round() / 10.0,
    })
}

/// 把结果曲面复制到持久体 ProjSurf_XY（无链接 PasteSpecial）。
fn keep_projected_surface(
    part: &Part,
    cmd_sel: &Selection,
    source: &impl ComObject,
    tag: &str,
    progress: Progress<'_>,
) {
    let attempt = (|| -> Result<(), String> {
        let proj_body = part
            .hybrid_bodies()
            .and_then(|h| h.add())
            .map_err(|e| e.to_string())?;
        proj_body
            .set_name("ProjSurf_XY")
            .map_err(|e| e.to_string())?;
        cmd_sel.clear().map_err(|e| e.to_string())?;
        cmd_sel.add(source).map_err(|e| e.to_string())?;
        cmd_sel.copy().map_err(|e| e.to_string())?;
        cmd_sel.clear().map_err(|e| e.to_string())?;
        cmd_sel.add(&proj_body).map_err(|e| e.to_string())?;
        part.set_in_work_object(&proj_body)
            .map_err(|e| e.to_string())?;
        cmd_sel
            .paste_special("CATPrtResultWithOutLink")
            .map_err(|e| e.to_string())?;
        cmd_sel.clear().map_err(|e| e.to_string())?;
        part.update().map_err(|e| e.to_string())?;
        Ok(())
    })();
    if let Err(e) = attempt {
        progress(&format!("[{tag}] 保留投影曲面失败: {e}"));
    }
}

/// 轮廓模板法：InstanceFactory 实例化 PROJECTED_AREA.CATPart，取其
/// Project_Surface 输出测面积（通孔已排除），平移回 tooling axis XY 平面。
/// 任一环节失败返回 None（调用方回退 Affinity）。
#[allow(clippy::too_many_arguments)]
fn silhouette_outline_area(
    part: &Part,
    hsf: &HybridShapeFactory,
    hybrid_body: &HybridBody,
    ref_extract: &Reference,
    cs: &AxisFrame,
    proj_plane: &rscatiacli::objects::Entity,
    cmd_sel: &Selection,
    spa: &SPAWorkbench,
    ss: &SystemService,
    template_path: Option<&str>,
    progress: Progress<'_>,
) -> Option<f64> {
    let Some(template) = locate_template(template_path) else {
        progress("[silhouette] 模板不存在,回退到 Affinity");
        return None;
    };

    let attempt = (|| -> Result<f64, String> {
        let z = cs.z_axis;
        let o = cs.origin_mm;

        // MTD 输入：原 VBA 用曲线作投影方向，这里建一条沿 Z 的线
        let mt_origin = hsf
            .add_new_point_coord(o[0], o[1], o[2])
            .map_err(|e| e.to_string())?;
        let _ = hybrid_body.append_hybrid_shape(&mt_origin);
        let _ = hsf.gsm_visibility(&mt_origin, 0);
        let mt_dir = hsf
            .add_new_direction_by_coord(z[0], z[1], z[2])
            .map_err(|e| e.to_string())?;
        let _ = hybrid_body.append_hybrid_shape(&mt_dir);
        let _ = hsf.gsm_visibility(&mt_dir, 0);
        let mt_line = hsf
            .add_new_line_pt_dir(
                &part
                    .create_reference_from_object(&mt_origin)
                    .map_err(|e| e.to_string())?,
                &part
                    .create_reference_from_object(&mt_dir)
                    .map_err(|e| e.to_string())?,
                -1.0,
                1.0,
                false,
            )
            .map_err(|e| e.to_string())?;
        let _ = hybrid_body.append_hybrid_shape(&mt_line);
        let _ = hsf.gsm_visibility(&mt_line, 0);
        let _ = part.set_in_work_object(&mt_line);
        let _ = part.update();

        // join(extract, extract) 作为 Surface 输入
        let join = hsf
            .add_new_join(ref_extract, ref_extract)
            .map_err(|e| e.to_string())?;
        let _ = join.set_connex(0);
        let _ = join.set_manifold(0);
        let _ = hybrid_body.append_hybrid_shape(&join);
        let _ = hsf.gsm_visibility(&join, 0);
        let _ = part.update_object(&join);
        let _ = part.update();

        // 实例化 ProjectedArea 模板（输入传原始 HybridShape, 非 Reference）
        let factory = part
            .get_customer_factory("InstanceFactory")
            .map_err(|e| e.to_string())?;
        factory
            .begin_instance_factory("ProjectedArea", &template.to_string_lossy())
            .map_err(|e| e.to_string())?;
        factory.begin_instantiate().map_err(|e| e.to_string())?;
        factory
            .put_input_data("Surface", &join)
            .map_err(|e| e.to_string())?;
        factory
            .put_input_data("MTD", &mt_line)
            .map_err(|e| e.to_string())?;
        factory
            .put_input_data("SupportPlane", proj_plane)
            .map_err(|e| e.to_string())?;
        let _ = factory.instantiate();
        factory.end_instantiate().map_err(|e| e.to_string())?;
        factory.end_instance_factory().map_err(|e| e.to_string())?;
        let _ = part.update();

        // 查找模板实例体
        let _ = cmd_sel.clear();
        cmd_sel
            .search("Name=FIS_ProjectedArea*,all")
            .map_err(|e| e.to_string())?;
        if cmd_sel.count().unwrap_or(0) < 1 {
            progress("[silhouette] 未找到 FIS_ProjectedArea,回退到 Affinity");
            return Err("未找到 FIS_ProjectedArea 实例体".to_string());
        }
        let hb_val = cmd_sel
            .item(1)
            .and_then(|it| it.value())
            .map_err(|e| e.to_string())?;
        let hb = HybridBody::new(hb_val.com().clone());

        // 模板输出 4 个形状：External(轮廓线)/Extrenal_Fill(外轮廓填充,含孔)/
        // Internal(孔轮廓线)/Project_Surface(投影曲面,已排除通孔面积)。
        // 取 Project_Surface（优先按名字，回退第 4 个）。
        let shapes = hb.hybrid_shapes().map_err(|e| e.to_string())?;
        let count = shapes.count().unwrap_or(0);
        let mut outline_shape: Option<rscatiacli::objects::Entity> = None;
        for i in 1..=count {
            if let Ok(sh) = shapes.item(i) {
                if sh.name().unwrap_or_default().contains("Project_Surface") {
                    outline_shape = Some(sh);
                    break;
                }
            }
        }
        let outline_shape = match outline_shape {
            Some(s) => s,
            None => shapes
                .item(4)
                .map_err(|_| "模板输出缺少 Project_Surface".to_string())?,
        };

        let ref_outline = part
            .create_reference_from_object(&outline_shape)
            .map_err(|e| e.to_string())?;
        let m = spa
            .get_measurable(&ref_outline)
            .map_err(|e| e.to_string())?;
        let area_mm2 = m.area().map_err(|e| e.to_string())? * 1_000_000.0; // m2 -> mm2

        // 把模板输出平移到 tooling axis XY 平面（面积已测得, 失败只影响摆放位置）
        let mut tr: Option<rscatiacli::objects::HybridShapeTranslate> = None;
        match m.get_cog(ss) {
            Ok(cog) => {
                let dz = dot([cog[0] - o[0], cog[1] - o[1], cog[2] - o[2]], z);
                if dz.abs() > 1e-6 {
                    match (|| -> Result<rscatiacli::objects::HybridShapeTranslate, String> {
                        let t = hsf.add_new_empty_translate().map_err(|e| e.to_string())?;
                        t.set_elem_to_translate(&ref_outline)
                            .map_err(|e| e.to_string())?;
                        t.set_vector_type(0).map_err(|e| e.to_string())?;
                        let dir = hsf
                            .add_new_direction_by_coord(z[0], z[1], z[2])
                            .map_err(|e| e.to_string())?;
                        t.set_direction(&dir).map_err(|e| e.to_string())?;
                        t.set_distance_value(-dz).map_err(|e| e.to_string())?;
                        t.set_volume_result(false).map_err(|e| e.to_string())?;
                        hybrid_body
                            .append_hybrid_shape(&t)
                            .map_err(|e| e.to_string())?;
                        part.set_in_work_object(&t).map_err(|e| e.to_string())?;
                        part.update().map_err(|e| e.to_string())?;
                        Ok(t)
                    })() {
                        Ok(t) => {
                            tr = Some(t);
                            progress(&format!(
                                "[silhouette] 模板输出沿 Z 偏移 {:.2} mm,已平移到 tooling axis XY 平面",
                                dz
                            ));
                        }
                        Err(e) => progress(&format!("[silhouette] 平移输出失败,保留原位置: {e}")),
                    }
                }
            }
            Err(e) => progress(&format!("[silhouette] 平移输出失败,保留原位置: {e}")),
        }

        // 复制（平移后）曲面到持久体 ProjSurf_XY（无链接）
        match &tr {
            Some(t) => keep_projected_surface(part, cmd_sel, t, "silhouette", progress),
            None => keep_projected_surface(part, cmd_sel, &outline_shape, "silhouette", progress),
        }

        // 清理临时平移特征（它引用模板内部形状, 必须先删）
        if let Some(t) = &tr {
            sel_drop(cmd_sel, t);
            let _ = part.update();
        }

        // 清理模板实例体
        sel_drop(cmd_sel, &hb_val);
        let _ = part.update();

        progress(&format!(
            "[silhouette] XY 投影面积(排除通孔) = {:.4} mm²",
            round4(area_mm2)
        ));
        Ok(round4(area_mm2))
    })();

    match attempt {
        Ok(v) => Some(v),
        Err(e) => {
            // 模板法未出结果时回退 Affinity 属正常流程, 不用「失败/错误」字样
            progress(&format!(
                "[silhouette] 模板法未出结果({e}), 自动改用 Affinity 压扁法"
            ));
            // 已创建的 FIS_ProjectedArea 实例体处于损坏状态会让后续所有 update 失败
            let _ = cmd_sel.clear();
            let mut broken = false;
            if cmd_sel.search("Name=FIS_ProjectedArea*,all").is_ok() {
                broken = cmd_sel.count().unwrap_or(0) > 0;
                if broken {
                    let _ = cmd_sel.delete();
                }
            }
            let _ = cmd_sel.clear();
            let _ = part.update();
            if broken {
                progress("[silhouette] 已清理损坏的 FIS_ProjectedArea 实例体");
            }
            None
        }
    }
}

/// Affinity 压扁法回退：把 MainBody 表面压扁到 tooling axis XY 平面并测面积。
/// 完全自包含（自己建 extract/平面/压扁曲面），面积 ÷2（顶面+底面重叠计两次，
/// 与原 FIS VBA bool_area → area_val/2 一致），通孔形状保留（已排除通孔）。
#[allow(clippy::too_many_arguments)]
fn affinity_outline_area(
    part: &Part,
    hsf: &HybridShapeFactory,
    cmd_sel: &Selection,
    spa: &SPAWorkbench,
    ss: &SystemService,
    cs: &AxisFrame,
    progress: Progress<'_>,
) -> Option<f64> {
    let attempt = (|| -> Result<f64, String> {
        // 自己的临时体（旧的 RuleProjSurf 已由调用方清理）
        let hybrid_body = part
            .hybrid_bodies()
            .and_then(|h| h.add())
            .map_err(|e| e.to_string())?;
        hybrid_body
            .set_name("RuleProjSurf_Aff")
            .map_err(|e| e.to_string())?;

        let main_body = part.main_body().map_err(|e| e.to_string())?;
        let ref_body = part
            .create_reference_from_object(&main_body)
            .map_err(|e| e.to_string())?;
        let body_cog = spa
            .get_measurable(&ref_body)
            .and_then(|m| m.get_cog(ss))
            .map_err(|e| e.to_string())?;

        // extract
        let extract = hsf.add_new_extract(&ref_body).map_err(|e| e.to_string())?;
        let _ = extract.set_propagation_type(1);
        let _ = extract.set_complementary_extract(false);
        let _ = extract.set_is_federated(false);
        let _ = hybrid_body.append_hybrid_shape(&extract);
        let _ = part.set_in_work_object(&extract);
        let _ = hsf.gsm_visibility(&extract, 0);
        let _ = part.update();
        let ref_extract = part
            .create_reference_from_object(&extract)
            .map_err(|e| e.to_string())?;

        // tooling axis XY 平面（过原点, 法向 Z）
        let z = cs.z_axis;
        let o = cs.origin_mm;
        let (a, b, c) = (z[0], z[1], z[2]);
        let d = -(a * o[0] + b * o[1] + c * o[2]);
        let proj_plane = hsf
            .add_new_plane_equation(a, b, c, d)
            .map_err(|e| e.to_string())?;
        let _ = hybrid_body.append_hybrid_shape(&proj_plane);
        let _ = proj_plane.set_name("ProjPlane_XY");
        let _ = part.set_in_work_object(&proj_plane);
        let _ = part.update();
        let ref_plane = part
            .create_reference_from_object(&proj_plane)
            .map_err(|e| e.to_string())?;

        let cog_pt = hsf
            .add_new_point_coord(body_cog[0], body_cog[1], body_cog[2])
            .map_err(|e| e.to_string())?;
        let _ = hybrid_body.append_hybrid_shape(&cog_pt);
        let _ = hsf.gsm_visibility(&cog_pt, 0);
        let ref_cog = part
            .create_reference_from_object(&cog_pt)
            .map_err(|e| e.to_string())?;

        // 变形系数默认 1e-5（侧壁折叠带视觉上不可见, 面积误差 ~0.00%）；
        // 复杂零件在过小系数下 update/测量报错 → 逐级放大重试（1e-5 → 0.01 → 0.1）
        const AFFINITY_SCALES: [f64; 3] = [1e-5, 0.01, 0.1];
        let mut affinity: Option<HybridShapeAffinity> = None;
        let mut area_m2: Option<f64> = None;
        let mut last_err = String::new();
        for (si, scale) in AFFINITY_SCALES.iter().enumerate() {
            match try_affinity(
                part,
                hsf,
                &hybrid_body,
                cmd_sel,
                spa,
                &ref_extract,
                &ref_plane,
                &ref_cog,
                *scale,
            ) {
                Ok((area, aff)) => {
                    if si > 0 {
                        progress(&format!("[affinity] 变形系数 {scale} 重试成功"));
                    }
                    affinity = Some(aff);
                    area_m2 = Some(area);
                    break;
                }
                Err(e) => {
                    last_err = e;
                    affinity = None;
                    area_m2 = None;
                    if si < AFFINITY_SCALES.len() - 1 {
                        progress(&format!(
                            "[affinity] 变形系数 {scale} 计算未通过({last_err}),自动改用 {} 重试",
                            AFFINITY_SCALES[si + 1]
                        ));
                    } else {
                        progress(&format!(
                            "[affinity] 变形系数 {scale} 计算未通过({last_err})"
                        ));
                    }
                }
            }
        }

        let (aff, area_m2) = match (affinity, area_m2) {
            (Some(a), Some(v)) => (a, v),
            _ => {
                // 所有变形系数都未成功 → 清掉本方法的临时体, 由调用方给出总错误
                sel_drop(cmd_sel, &hybrid_body);
                let _ = part.update();
                progress(&format!(
                    "[affinity] 压扁法失败: 变形系数 1e-5, 0.01, 0.1 均未成功, 最后错误: {last_err}"
                ));
                return Err(last_err);
            }
        };

        // 保留压扁曲面到持久体 ProjSurf_XY
        keep_projected_surface(part, cmd_sel, &aff, "affinity", progress);

        // 删除临时体（与 silhouette 失败后删除逻辑一致, 保持零件干净）
        sel_drop(cmd_sel, &hybrid_body);
        let _ = part.update();

        let area_mm2 = (area_m2 * 1_000_000.0) / 2.0; // m2 -> mm2, 单面
        progress(&format!(
            "[affinity] XY 投影面积(压扁/2,已排除通孔) = {:.4} mm²",
            round4(area_mm2)
        ));
        Ok(round4(area_mm2))
    })();

    match attempt {
        Ok(v) => Some(v),
        Err(e) => {
            progress(&format!("[affinity] 失败: {e}"));
            None
        }
    }
}

/// 建 affinity 特征并测面积；失败时即时删除该特征（避免损坏状态阻断重试）。
#[allow(clippy::too_many_arguments)]
fn try_affinity(
    part: &Part,
    hsf: &HybridShapeFactory,
    hybrid_body: &HybridBody,
    cmd_sel: &Selection,
    spa: &SPAWorkbench,
    ref_extract: &Reference,
    ref_plane: &Reference,
    ref_cog: &Reference,
    scale: f64,
) -> Result<(f64, HybridShapeAffinity), String> {
    let affinity = hsf
        .add_new_affinity(ref_extract, 1.0, 1.0, scale)
        .map_err(|e| e.to_string())?;
    let finish = (|| -> Result<f64, String> {
        affinity
            .set_axis_plane(ref_plane)
            .map_err(|e| e.to_string())?;
        affinity
            .set_axis_origin(ref_cog)
            .map_err(|e| e.to_string())?;
        affinity
            .set_volume_result(false)
            .map_err(|e| e.to_string())?;
        hybrid_body
            .append_hybrid_shape(&affinity)
            .map_err(|e| e.to_string())?;
        part.set_in_work_object(&affinity)
            .map_err(|e| e.to_string())?;
        hsf.gsm_visibility(&affinity, 0)
            .map_err(|e| e.to_string())?;
        part.update().map_err(|e| e.to_string())?;
        let ref_aff = part
            .create_reference_from_object(&affinity)
            .map_err(|e| e.to_string())?;
        let m = spa.get_measurable(&ref_aff).map_err(|e| e.to_string())?;
        m.area().map_err(|e| e.to_string())
    })();
    match finish {
        Ok(area) => Ok((area, affinity)),
        Err(e) => {
            // 删除本次失败的 affinity 特征, 避免损坏状态阻断后续重试
            sel_drop(cmd_sel, &affinity);
            let _ = part.set_in_work_object(hybrid_body);
            let _ = part.update();
            Err(e)
        }
    }
}

// ── JSON 导出 ────────────────────────────────────────────────────────

/// 结果写 {output_dir}/{part_name}_proj_surface.json（对应 Python export_json）。
#[allow(clippy::too_many_arguments)]
fn export_json(
    part_name: &str,
    area: &AreaData,
    cs: &AxisFrame,
    output_dir_param: Option<&str>,
    doc: &Document,
    cs_source: &str,
) -> Result<String, String> {
    let dir: PathBuf = match output_dir_param {
        Some(d) if !d.is_empty() => PathBuf::from(d),
        _ => {
            // 零件同目录, 回退 C:\temp
            doc.full_name()
                .ok()
                .and_then(|p| PathBuf::from(&p).parent().map(|x| x.to_path_buf()))
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or_else(|| PathBuf::from("C:\\temp"))
        }
    };
    std::fs::create_dir_all(&dir).map_err(|e| format!("export_json failed: {e}"))?;
    let safe_name = part_name.replace([' ', '/'], "_");
    let filepath = dir.join(format!("{safe_name}_proj_surface.json"));
    let payload = json!({
        "part_name": part_name,
        "analysis_time": local_now_iso(),
        "projected_surface": { "plane_xy_mm2": area.area_mm2 },
        "volume_mm3": area.volume_mm3,
        "tooling_axis_source": cs_source,
        "coordinate_system": {
            "origin_mm": cs.origin_mm,
            "x_axis": cs.x_axis,
            "y_axis": cs.y_axis,
            "z_axis": cs.z_axis,
        },
    });
    let text =
        serde_json::to_string_pretty(&payload).map_err(|e| format!("export_json failed: {e}"))?;
    std::fs::write(&filepath, text).map_err(|e| format!("export_json failed: {e}"))?;
    Ok(filepath.to_string_lossy().into_owned())
}

/// UTC 时间 ISO8601（无 chrono 依赖；Python 版为本地 naive 时间，仅记录用途）。
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

/// 保留类型别名引用（跨模块共享 vba_bbox_generate 的 Vec3）。
#[allow(dead_code)]
type Vec3Alias = Vec3f;
