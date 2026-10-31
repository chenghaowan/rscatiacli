//! save-product-without-history：装配无参保存（翻译自 FIS PRODUCT_SAVE_WITHOUT_HISTORY.catvba, FPRM010）。
//!
//! 对应 ltl-manage resources/sidecars/product_analysis/save_product_without_history.py。
//! 把激活 .CATProduct 第一层的每个 .CATPart 抽成无设计历史的哑图零件：
//! 可见且体积非 0 的 Body 以 CATPrtResultWithOutLink(无链接)粘贴进新 Part，
//! 可见线/面搜索后同样无链接粘贴进 Dump_Elements，另存
//! `<输出根>/<零件名>/<零件名>.CATPart`；新装配插入这些哑图零件并恢复
//! 实例名/零件号，最后存为 `<输出根>/<原文件名>.CATProduct`。源装配不动。
//!
//! 与 Python 版差异：同名 .CATProduct 自动切换未移植（与 hide_show 同口径）。

use serde_json::{json, Value};

use rscatiacli::objects::{Body, Document, PartDocument};
use rscatiacli::{attach_with_guard, ComCollection, ComObject, ComResult, Progress};

/// CatVisPropertyShow: 0 = 显示。
const SHOW_SHOWN: i32 = 0;
/// 原宏 ApplyWorkMode 实参(0)；hide_show/drawing_common 用 2 —— 两处 Python
/// 口径本就不同，按各自原样移植。
const WORK_MODE_DESIGN_AS_IN_MACRO: i32 = 0;
/// 原宏: 搜索可见的 Part Design 线/曲线/曲面/体素几何(屏幕范围)。
const WIREFRAME_SEARCH: &str = "'Part Design'.Line + 'Part Design'.Curve + 'Part Design'.Surface + 'Part Design'.'Volume geometry',scr";
/// 无链接结果粘贴格式。
const PASTE_NO_LINK: &str = "CATPrtResultWithOutLink";

fn name_of(obj: &impl ComObject) -> String {
    obj.name().unwrap_or_default()
}

/// 剥掉 `.CATProduct` 后缀（大小写不敏感，与 Python `_strip_product_ext` 一致）。
fn strip_product_ext(name: &str) -> &str {
    if name.len() >= 11 && name[name.len() - 11..].eq_ignore_ascii_case(".catproduct") {
        &name[..name.len() - 11]
    } else {
        name
    }
}

/// 挑出可导出的 Body：不在布尔运算中、当前可见、体积非 0（与宏过滤一致）。
fn qualify_bodies(
    prt_doc: &PartDocument,
    sel: &rscatiacli::objects::Selection,
) -> ComResult<Vec<Body>> {
    let prt = prt_doc.part()?;
    let spa = prt_doc.spa_workbench()?;
    let mut result = Vec::new();
    for body in prt.bodies()?.items()? {
        if body.in_boolean_operation().unwrap_or(false) {
            continue;
        }
        let _ = sel.clear();
        if sel.add(&body).is_ok() {
            if let Ok(vis) = sel.vis_properties() {
                // 隐藏的 Body 不导出；读取失败按可见处理
                if vis.get_show().map(|s| s != SHOW_SHOWN).unwrap_or(false) {
                    continue;
                }
            }
        }
        let has_volume = match prt.create_reference_from_object(&body) {
            Ok(r) => match spa.get_measurable(&r) {
                Ok(m) => m.volume().map(|v| v * 1e9).unwrap_or(0.0) != 0.0,
                Err(_) => false, // 测不出体积(无实体)的 Body 不导出
            },
            Err(_) => false,
        };
        if !has_volume {
            continue;
        }
        result.push(body);
    }
    let _ = sel.clear();
    Ok(result)
}

/// 新 Part 里同名 Body 只保留搜索到的第一个，其余删除（与宏清理一致）。
fn dedupe_bodies(part2_doc: &PartDocument, sel: &rscatiacli::objects::Selection) -> usize {
    let mut removed = 0;
    let Ok(bodies) = part2_doc.part().and_then(|p| p.bodies()) else {
        return 0;
    };
    for body in bodies.items().unwrap_or_default() {
        let Ok(name) = body.name() else { continue };
        let _ = sel.clear();
        if sel.search(&format!(".Body.Name={name},all")).is_err() {
            continue; // 搜不到(只有一个自身)属正常
        }
        if sel.count().unwrap_or(0) > 1 && sel.remove(1).is_ok() && sel.delete().is_ok() {
            removed += 1;
        }
    }
    let _ = sel.clear();
    removed
}

/// 把一个源零件转成无参零件文件，插入新装配。返回实例信息 dict。
fn convert_one_part(
    prt_doc: &PartDocument,
    out_root: &std::path::Path,
    new_product_doc: &rscatiacli::objects::ProductDocument,
    app: &rscatiacli::objects::Application,
) -> ComResult<Value> {
    let prt = prt_doc.part()?;
    let _ = prt_doc.activate(); // 本环境为 no-op(r123 实测)，保留调用与 Python 对齐

    let bodies = qualify_bodies(prt_doc, &prt_doc.selection()?)?;

    // 实体复制
    let sel_src = prt_doc.selection()?;
    let _ = sel_src.clear();
    for body in &bodies {
        let _ = sel_src.add(body);
    }
    let has_solids = !bodies.is_empty();
    if has_solids {
        sel_src.copy()?;
    }

    // 新 Part 文档 + Dump_Elements 几何图形集
    let docs = app.documents()?;
    let part2_doc = docs.add("Part")?.part_document();
    let part2 = part2_doc.part()?;
    let dump_gs = part2.hybrid_bodies()?.add()?;
    dump_gs.set_name("Dump_Elements")?;

    let sel_new = part2_doc.selection()?;
    if has_solids {
        let _ = sel_new.clear();
        sel_new.add(&part2)?;
        sel_new.paste_special(PASTE_NO_LINK)?;
    }

    // 可视线/面 → Dump_Elements
    let _ = sel_src.clear();
    let search_ok = sel_src.search(WIREFRAME_SEARCH).is_ok(); // 无线面时 Search 抛错视为空
    let has_wireframe = search_ok && sel_src.count().unwrap_or(0) > 0;
    if has_wireframe {
        sel_src.copy()?;
        let _ = sel_src.clear();
        let _ = part2_doc.activate();
        let _ = sel_new.clear();
        sel_new.add(&dump_gs)?;
        sel_new.paste_special(PASTE_NO_LINK)?;
    }
    let _ = sel_src.clear();

    part2.update()?;
    let deduped = dedupe_bodies(&part2_doc, &sel_new);
    part2.update()?;

    // 保存为 <输出根>/<零件名>/<零件名>.CATPart
    let part_name = {
        let n = prt.name().unwrap_or_default();
        if n.is_empty() {
            "Part".to_string()
        } else {
            n
        }
    };
    let part_dir = out_root.join(&part_name);
    std::fs::create_dir_all(&part_dir)
        .map_err(|e| rscatiacli::ComError::runtime(format!("创建输出目录失败: {e}")))?;
    let part_file = part_dir.join(format!("{part_name}.CATPart"));
    part2_doc.save_as(&part_file.to_string_lossy())?;
    part2_doc.close()?;

    // 插入新装配（实例改名由调用方处理）
    let products_new = new_product_doc.product()?.products()?;
    products_new.add_components_from_files(&[&part_file.to_string_lossy()], "All")?;

    Ok(json!({
        "part_name": part_name,
        "part_file": part_file.to_string_lossy(),
        "solids": bodies.len(),
        "wireframe": has_wireframe,
        "deduped_bodies": deduped,
    }))
}

/// 工具主体。**必须在 STA 线程上执行。**
pub fn run(params: &Value, progress: Progress<'_>) -> ComResult<Value> {
    let empty_data = json!({"items": [], "total": 0, "converted": 0, "skipped": 0});
    let dry_run = params
        .get("dry_run")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let close_source = params
        .get("close_source")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let output_dir = params
        .get("output_dir")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();

    let app = attach_with_guard(progress)?;
    let doc = app.active_document()?;
    let doc_name = name_of(&doc);
    if !doc_name.to_lowercase().ends_with(".catproduct") {
        return Ok(json!({
            "success": false,
            "message": format!("当前是{}({doc_name}): Rust 版尚未移植同名装配自动切换, 请先打开对应 .CATProduct 后重试",
                if doc_name.to_lowercase().ends_with(".catpart") { "零件" } else { "非装配文档" }),
            "data": empty_data,
        }));
    }

    let prd_doc = doc.product_document();
    let prd = prd_doc.product()?;
    let prod_name = strip_product_ext(&doc_name);
    if prod_name.is_empty() {
        return Ok(json!({
            "success": false,
            "message": format!("无法从文档名提取产品名({doc_name})"),
            "data": empty_data,
        }));
    }

    // 输出根目录：默认 <源文件目录>/<产品名>（与宏一致）
    let src_full = prd_doc.full_name().unwrap_or_default();
    let src_path = std::path::PathBuf::from(&src_full);
    if !src_path.exists() {
        return Ok(json!({
            "success": false,
            "message": format!("装配尚未保存到磁盘({doc_name}),请先保存后再执行无参保存"),
            "data": empty_data,
        }));
    }
    let src_dir = src_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_default();
    let out_root = if output_dir.is_empty() {
        src_dir.join(prod_name)
    } else {
        std::path::PathBuf::from(&output_dir).join(prod_name)
    };

    // 收集第一层 .CATPart 实例（与宏一致：不展开子装配）
    let mut targets: Vec<(String, String, PartDocument)> = Vec::new();
    for inst in prd.products()?.items().unwrap_or_default() {
        let Ok(reference) = inst.reference_product() else {
            continue;
        };
        let Ok(parent) = reference.parent() else {
            continue;
        };
        let doc = Document::new(parent);
        let doc_name = name_of(&doc);
        if !doc_name.to_lowercase().contains(".catpart") {
            continue; // 子装配/无几何的组件跳过
        }
        let part_number = reference.part_number().unwrap_or_default();
        targets.push((name_of(&inst), part_number, doc.part_document()));
    }

    let mut info = json!({
        "product_name": prod_name,
        "source_file": src_full,
        "output_dir": out_root.to_string_lossy(),
        "items": [], "total": targets.len(), "converted": 0, "skipped": 0,
    });

    if targets.is_empty() {
        return Ok(json!({
            "success": false,
            "message": "装配第一层没有 .CATPart 零件实例,无可转换内容",
            "data": info,
        }));
    }

    if dry_run {
        info["items"] = json!(targets
            .iter()
            .map(|(n, p, d)| json!({
                "instance_name": n,
                "part_number": p,
                "source_document": d.name().unwrap_or_default(),
                "status": "pending",
                "note": "",
            }))
            .collect::<Vec<_>>());
        info["dry_run"] = json!(true);
        return Ok(json!({
            "success": true,
            "message": format!(
                "[dry-run] 共 {} 个直属零件将转换为无参零件,输出目录: {}",
                targets.len(),
                out_root.to_string_lossy()
            ),
            "data": info,
        }));
    }

    let alerts_prev = app.display_file_alerts().unwrap_or(true);
    let _ = app.set_display_file_alerts(false);

    let result = run_inner(
        &app,
        &prd,
        &out_root,
        prod_name,
        targets,
        &mut info,
        close_source,
        alerts_prev,
        progress,
    );
    let _ = app.set_display_file_alerts(alerts_prev);
    if close_source {
        let _ = prd_doc.document().close();
    }
    result
}

#[allow(clippy::too_many_arguments)]
fn run_inner(
    app: &rscatiacli::objects::Application,
    prd: &rscatiacli::objects::Product,
    out_root: &std::path::Path,
    prod_name: &str,
    targets: Vec<(String, String, PartDocument)>,
    info: &mut Value,
    close_source: bool,
    alerts_prev: bool,
    progress: Progress<'_>,
) -> ComResult<Value> {
    let _ = (close_source, alerts_prev); // 由调用方收尾处理
    let total = targets.len();
    let mut items: Vec<Value> = Vec::new();
    let mut converted = 0usize;
    let mut skipped = 0usize;
    let mut inst_records: Vec<(String, String)> = Vec::new();

    progress(&format!("({total} 个零件) 切换设计模式..."));
    let _ = prd.apply_work_mode(WORK_MODE_DESIGN_AS_IN_MACRO);

    // 新建空装配
    let docs = app.documents()?;
    let prod2_doc = docs.add("Product")?.product_document();
    let _ = prod2_doc
        .product()?
        .apply_work_mode(WORK_MODE_DESIGN_AS_IN_MACRO);

    if let Err(e) = std::fs::create_dir_all(out_root) {
        return Ok(
            json!({"success": false, "message": format!("创建输出目录失败: {e}"), "data": *info}),
        );
    }

    for (idx, (src_inst_name, part_number, prt_doc)) in targets.iter().enumerate() {
        progress(&format!("({}/{total}) 正在转换: {src_inst_name}", idx + 1));
        let mut item = json!({
            "instance_name": src_inst_name,
            "part_number": part_number,
            "source_document": prt_doc.name().unwrap_or_default(),
            "status": "ok",
            "note": "",
        });
        match convert_one_part(prt_doc, out_root, &prod2_doc, app) {
            Ok(res) => {
                if let (Some(obj), Some(src)) = (item.as_object_mut(), res.as_object()) {
                    for (k, v) in src {
                        obj.insert(k.clone(), v.clone());
                    }
                }
                // 重命名新装配中的实例为源实例名（与宏一致）
                let products_new = prod2_doc.product().and_then(|p| p.products())?;
                let count = products_new.count()?;
                let new_inst = products_new.item(count)?;
                match new_inst.set_name(src_inst_name) {
                    Ok(()) => inst_records.push((src_inst_name.clone(), part_number.clone())),
                    Err(e) => {
                        item["note"] = json!(format!("实例改名失败({e}),保留默认名"));
                        inst_records.push((name_of(&new_inst), part_number.clone()));
                    }
                }
                converted += 1;
            }
            Err(e) => {
                item["status"] = json!("error");
                item["note"] = json!(e.to_string());
                skipped += 1;
                progress(&format!("✗ {src_inst_name} 转换失败: {e}"));
            }
        }
        items.push(item);
    }

    if converted == 0 {
        let _ = prod2_doc.close();
        info["items"] = json!(items);
        info["converted"] = json!(0);
        info["skipped"] = json!(skipped);
        return Ok(json!({
            "success": false,
            "message": "所有零件转换失败,未生成输出",
            "data": *info,
        }));
    }

    // 恢复每个实例的零件号（与宏一致：按实例名匹配）
    if let Ok(prod2) = prod2_doc.product() {
        if let Ok(insts) = prod2.products().and_then(|p| p.items()) {
            for inst in &insts {
                let name = name_of(inst);
                if let Some((_, rec_pn)) =
                    inst_records.iter().find(|(rec_name, _)| *rec_name == name)
                {
                    if !rec_pn.is_empty() {
                        let _ = inst.set_part_number(rec_pn);
                    }
                }
            }
            let _ = prod2.set_part_number(prod_name);
        }
    }

    let new_product_file = out_root.join(format!("{prod_name}.CATProduct"));
    progress(&format!(
        "保存新装配: {}",
        new_product_file.to_string_lossy()
    ));
    prod2_doc.save_as(&new_product_file.to_string_lossy())?;
    let _ = prod2_doc.activate();

    info["items"] = json!(items);
    info["converted"] = json!(converted);
    info["skipped"] = json!(skipped);
    info["output_file"] = json!(new_product_file.to_string_lossy());
    Ok(json!({
        "success": true,
        "message": format!(
            "无参保存完成: {converted}/{total} 个零件已转换,新装配保存在 {}",
            new_product_file.to_string_lossy()
        ),
        "data": *info,
    }))
}
