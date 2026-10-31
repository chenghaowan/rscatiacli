//! dump-solid：零件去参数保存（翻译自 cplugin CONVERT_PARAMETERIC_PART_TO_NON_PARAMETRIC_PART, DumpSolid）。
//!
//! 对应 ltl-manage resources/sidecars/dump_solid/dump_solid.py。
//! 把激活 .CATPart 的可见非零实体 Body 以 CATPrtResultWithOutLink(无链接)
//! 粘贴进新 Part —— 新零件只有结果实体、无草图/参数/设计历史；可视线/面
//! 同样无链接粘贴进 Dump_Elements（include_wireframe 控制）；同名重复 Body
//! 去重；零件号与源一致，另存 `<源目录>/Result/<零件名>.CATPart`，源文件不动。
//!
//! 与 Python 版差异：同名 .CATPart 自动切换未移植（与 hide_show 同口径）。

use serde_json::{json, Value};

use rscatiacli::objects::PartDocument;
use rscatiacli::{attach_with_guard, ComCollection, ComObject, ComResult, Progress};

use crate::save_product_without_history::{dedupe_bodies, qualify_bodies};

/// 搜索可见的 Part Design 线/曲线/曲面（屏幕范围）。
const WIREFRAME_SEARCH: &str = "'Part Design'.Line + 'Part Design'.Curve + 'Part Design'.Surface + 'Part Design'.'Volume geometry',scr";
/// 无链接结果粘贴 = 去参数的关键。
const PASTE_NO_LINK: &str = "CATPrtResultWithOutLink";

/// 把当前零件转成无参零件文件。返回结果 dict。
fn convert_current_part(
    prt_doc: &PartDocument,
    out_root: &std::path::Path,
    body_name: &str,
    include_wireframe: bool,
    app: &rscatiacli::objects::Application,
    progress: Progress<'_>,
) -> ComResult<Value> {
    let prt = prt_doc.part()?;
    let _ = prt_doc.activate(); // 本环境为 no-op(r123 实测)，保留调用与 Python 对齐

    let sel = prt_doc.selection()?;
    let bodies = qualify_bodies(prt_doc, &sel)?;
    let bodies = if body_name.is_empty() {
        bodies
    } else {
        let named: Vec<_> = bodies
            .iter()
            .filter(|b| b.name().unwrap_or_default() == body_name)
            .collect();
        if named.is_empty() {
            return Err(rscatiacli::ComError::runtime(format!(
                "零件中未找到名为「{body_name}」的可见非空 Body(请核对大小写与名称)"
            )));
        }
        named.into_iter().cloned().collect()
    };

    // 实体复制
    let _ = sel.clear();
    for body in &bodies {
        let _ = sel.add(body);
    }
    let has_solids = !bodies.is_empty();
    if has_solids {
        sel.copy()?;
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
    let mut has_wireframe = false;
    if include_wireframe {
        let _ = sel.clear();
        let search_ok = sel.search(WIREFRAME_SEARCH).is_ok(); // 无线面时 Search 抛错视为空
        has_wireframe = search_ok && sel.count().unwrap_or(0) > 0;
        if has_wireframe {
            sel.copy()?;
            let _ = sel.clear();
            let _ = part2_doc.activate();
            let _ = sel_new.clear();
            sel_new.add(&dump_gs)?;
            sel_new.paste_special(PASTE_NO_LINK)?;
        }
    }
    let _ = sel.clear();

    part2.update()?;
    let deduped = dedupe_bodies(&part2_doc, &sel_new);
    part2.update()?;

    // 零件号与源零件一致（与原宏 product2.partnumber = PartName 一致）
    let part_name = {
        let n = prt.name().unwrap_or_default();
        if n.is_empty() {
            "Part".to_string()
        } else {
            n
        }
    };
    if let Err(e) = part2_doc
        .product_document()
        .product()?
        .set_part_number(&part_name)
    {
        progress(&format!("⚠ 设置新零件号失败(忽略): {e}"));
    }

    // 保存为 <输出根>/<零件名>.CATPart
    std::fs::create_dir_all(out_root)
        .map_err(|e| rscatiacli::ComError::runtime(format!("创建输出目录失败: {e}")))?;
    let part_file = out_root.join(format!("{part_name}.CATPart"));
    part2_doc.save_as(&part_file.to_string_lossy())?;
    part2_doc.close()?;

    Ok(json!({
        "part_name": part_name,
        "part_file": part_file.to_string_lossy(),
        "solids": bodies.len(),
        "body_names": bodies.iter().filter_map(|b| b.name().ok()).collect::<Vec<_>>(),
        "wireframe": has_wireframe,
        "deduped_bodies": deduped,
    }))
}

/// 工具主体。**必须在 STA 线程上执行。**
pub fn run(params: &Value, progress: Progress<'_>) -> ComResult<Value> {
    let empty_data = json!({
        "part_name": "", "part_file": "", "solids": 0, "body_names": [],
        "wireframe": false, "deduped_bodies": 0,
    });
    let dry_run = params
        .get("dry_run")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let body_name = params
        .get("body_name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let include_wireframe = params
        .get("include_wireframe")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let output_dir = params
        .get("output_dir")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();

    let app = attach_with_guard(progress)?;
    let doc = app.active_document()?;
    let doc_name = doc.name().unwrap_or_default();
    if !doc_name.to_lowercase().ends_with(".catpart") {
        return Ok(json!({
            "success": false,
            "message": format!("当前是{}({doc_name}): Rust 版尚未移植同名零件自动切换, 请先打开对应 .CATPart 后重试",
                if doc_name.to_lowercase().ends_with(".catproduct") { "装配" } else { "非零件文档" }),
            "data": empty_data,
        }));
    }

    let prt_doc = doc.part_document();
    let prt = prt_doc.part()?;
    let part_name = {
        let n = prt.name().unwrap_or_default();
        if n.is_empty() {
            "Part".to_string()
        } else {
            n
        }
    };

    // 输出目录：默认 <源文件目录>/Result（与宏一致）
    let src_full = prt_doc.full_name().unwrap_or_default();
    let src_path = std::path::PathBuf::from(&src_full);
    let saved_on_disk = src_path.exists();
    if !saved_on_disk && output_dir.is_empty() {
        return Ok(json!({
            "success": false,
            "message": format!("零件尚未保存到磁盘({doc_name}),请先保存后再执行去参数保存"),
            "data": empty_data,
        }));
    }
    let out_root = if output_dir.is_empty() {
        src_path
            .parent()
            .map(|p| p.join("Result"))
            .unwrap_or_else(|| std::path::PathBuf::from("Result"))
    } else {
        std::path::PathBuf::from(&output_dir)
    };

    // 预览：只统计将导出的 Body，不实际转换
    if dry_run {
        let bodies = qualify_bodies(&prt_doc, &prt_doc.selection()?)?;
        let bodies = if body_name.is_empty() {
            bodies
        } else {
            bodies
                .into_iter()
                .filter(|b| b.name().unwrap_or_default() == body_name)
                .collect::<Vec<_>>()
        };
        let info = json!({
            "part_name": part_name,
            "source_file": if saved_on_disk { json!(src_full) } else { json!("(未保存)") },
            "output_dir": out_root.to_string_lossy(),
            "solids": bodies.len(),
            "body_names": bodies.iter().filter_map(|b| b.name().ok()).collect::<Vec<_>>(),
            "wireframe": include_wireframe,
            "deduped_bodies": 0,
            "dry_run": true,
            "target_body": if body_name.is_empty() { json!("(全部可见非空 Body)") } else { json!(body_name) },
        });
        return Ok(json!({
            "success": true,
            "message": format!(
                "[dry-run] 零件「{part_name}」将导出 {} 个实体 Body{} ,输出到 {}",
                info["solids"].as_i64().unwrap_or(0),
                if include_wireframe { " + 可见线/曲面" } else { "" },
                out_root.to_string_lossy()
            ),
            "data": info,
        }));
    }

    let alerts_prev = app.display_file_alerts().unwrap_or(true);
    let _ = app.set_display_file_alerts(false);

    progress(&format!("正在为零件「{part_name}」生成无参数副本..."));
    let result = convert_current_part(
        &prt_doc,
        &out_root,
        &body_name,
        include_wireframe,
        &app,
        progress,
    );
    let _ = app.set_display_file_alerts(alerts_prev);

    let mut res = match result {
        Ok(v) => v,
        Err(e) => {
            return Ok(json!({
                "success": false,
                "message": e.to_string(),
                "data": {"part_name": part_name},
            }));
        }
    };
    res["source_file"] = if saved_on_disk {
        json!(src_full)
    } else {
        json!("(未保存)")
    };
    res["output_dir"] = json!(out_root.to_string_lossy());
    Ok(json!({
        "success": true,
        "message": format!(
            "去参数保存完成: 零件「{part_name}」已导出 {} 个实体{} ,无参数副本保存在 {}",
            res["solids"].as_i64().unwrap_or(0),
            if res["wireframe"].as_bool().unwrap_or(false) { " + 可视线/曲面" } else { "" },
            res["part_file"].as_str().unwrap_or_default()
        ),
        "data": res,
    }))
}
