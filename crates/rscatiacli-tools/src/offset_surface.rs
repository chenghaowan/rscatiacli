//! offset_surface：曲面偏置（按名字约定批量偏置几何体集曲面）。
//!
//! 对应 ltl-manage resources/sidecars/offset_surface/offset_surface.py。
//! 约定：源几何体集内先用 CATIA 偏置命令建一个标记特征并命名为 target_1，
//! 本工具读它的方向作为整批方向，把集合内全部可偏置曲面（SurfaceExplicit/
//! Offset/Extract）批量偏置为给定值；失败面原样复制到 NON_OFFSET_B_SURFACES_
//! <源集>_<值>mm 补偿集；成功面进入 B_SURFACES_<源集>_<值>mm 并染绿；
//! 结束删除 target 标记（keep_target 可保留）。
//!
//! 与 Python 版差异：活动文档为图纸/装配时的同名 .CATPart 自动切换未移植，
//! 返回明确错误（与 hide_show 同口径）。

use serde_json::{json, Value};

use rscatiacli::objects::{HybridBody, Part};
use rscatiacli::{attach_with_guard, ComCollection, ComObject, ComResult, Progress};

use crate::surface_common::{is_offset, is_offsettable, iter_geosets, unique_geoset_name};

/// Python `%g` 的常用近似：整数去小数尾，其余按默认浮点显示。
fn g_format(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

fn name_of(obj: &impl ComObject) -> String {
    obj.name().unwrap_or_default()
}

/// 给容器（HybridBody 本体）染色（Python 对容器直接 Add 对象，不加 Reference）。
fn color_container(
    sel: &rscatiacli::objects::Selection,
    container: &HybridBody,
    r: i32,
    g: i32,
    b: i32,
    inherit: i32,
) {
    let _ = sel.clear();
    if sel.add(container).is_ok() {
        if let Ok(vis) = sel.vis_properties() {
            let _ = vis.set_real_color(r, g, b, inherit);
        }
    }
    let _ = sel.clear();
}

/// 选中并删除对象（Selection.Delete，失败静默）。
fn select_delete(sel: &rscatiacli::objects::Selection, obj: &impl ComObject) -> bool {
    let _ = sel.clear();
    let ok = sel.add(obj).is_ok() && sel.delete().is_ok();
    let _ = sel.clear();
    ok
}

/// 复制对象到目标容器（Selection Copy/Paste，与原宏同通道）。
fn copy_to(sel: &rscatiacli::objects::Selection, obj: &impl ComObject, dst: &HybridBody) -> bool {
    let _ = sel.clear();
    if sel.add(obj).is_err() {
        let _ = sel.clear();
        return false;
    }
    if sel.copy().is_err() {
        let _ = sel.clear();
        return false;
    }
    let _ = sel.clear();
    if sel.add(dst).is_err() || sel.paste().is_err() {
        let _ = sel.clear();
        return false;
    }
    let _ = sel.clear();
    true
}

/// 工具主体。**必须在 STA 线程上执行。**
pub fn run(params: &Value, progress: Progress<'_>) -> ComResult<Value> {
    let offset_value = params.get("offset_value").and_then(|v| v.as_f64());
    let Some(offset_value) = offset_value else {
        return Ok(
            json!({"success": false, "message": format!("偏置值无效: {:?}", params.get("offset_value"))}),
        );
    };
    if offset_value <= 0.0 {
        return Ok(json!({"success": false, "message": "偏置值必须大于 0"}));
    }
    let keep_target = params
        .get("keep_target")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let target_prefix = params
        .get("target_prefix")
        .and_then(|v| v.as_str())
        .unwrap_or("target_")
        .to_lowercase();

    let app = attach_with_guard(progress)?;
    let doc = app.active_document()?;
    let doc_name = name_of(&doc);
    let lower = doc_name.to_lowercase();
    if !lower.ends_with(".catpart") {
        let kind = if lower.ends_with(".catproduct") {
            "装配"
        } else if lower.ends_with(".catdrawing") {
            "工程图"
        } else {
            "非零件文档"
        };
        return Ok(json!({
            "success": false,
            "message": format!(
                "当前是{kind}({doc_name}): Rust 版尚未移植同名零件自动切换, 请先打开对应 .CATPart 后重试"
            )
        }));
    }

    let alerts_prev = app.display_file_alerts().unwrap_or(true);
    let _ = app.set_display_file_alerts(false);

    let result = run_inner(&doc, offset_value, keep_target, &target_prefix, progress);
    let _ = app.set_display_file_alerts(alerts_prev);
    result
}

fn run_inner(
    doc: &rscatiacli::objects::Document,
    offset_value: f64,
    keep_target: bool,
    target_prefix: &str,
    progress: Progress<'_>,
) -> ComResult<Value> {
    let part: Part = doc.part_document().part()?;
    let hsf = part.hybrid_shape_factory()?;
    let hbs = part.hybrid_bodies()?;
    let sel = doc.selection()?;

    // 零件本身必须无更新错误（照原宏先整体 Update 校验）
    if let Err(e) = part.update() {
        let brief = e.to_string().chars().take(120).collect::<String>();
        return Ok(
            json!({"success": false, "message": format!("零件存在更新错误,请先修复后再运行: {brief}")}),
        );
    }

    // ── 1. 按名字找 target 标记特征 ──
    // [(geoset, shape, name)]
    let mut target_hits: Vec<(HybridBody, rscatiacli::dispatch::ComObj, String)> = Vec::new();
    for hb in iter_geosets(&hbs) {
        let Ok(shapes) = hb.hybrid_shapes() else {
            continue;
        };
        let Ok(count) = shapes.count() else {
            continue;
        };
        for i in 1..=count {
            let Ok(sh) = shapes.item(i) else {
                continue;
            };
            let Ok(name) = sh.name() else {
                continue;
            };
            if name.to_lowercase().starts_with(target_prefix) {
                target_hits.push((hb.clone(), sh.com().clone(), name));
            }
        }
    }

    if target_hits.is_empty() {
        return Ok(json!({"success": false, "message": format!(
            "未找到以 '{target_prefix}' 开头的特征。请先在源几何体集内:选中基准面 → 插入>曲面>偏置(朝目标方向偏置,距离任意)→ 将该偏置特征改名为 target_1,再运行本工具"
        )}));
    }

    let mut hit = None;
    for (hb, sh, name) in &target_hits {
        if is_offset(sh) {
            hit = Some((hb.clone(), sh.clone(), name.clone()));
            break;
        }
    }
    let Some((target_hb, target_sh, target_name)) = hit else {
        let names = target_hits
            .iter()
            .take(5)
            .map(|(_, _, n)| n.clone())
            .collect::<Vec<_>>()
            .join(", ");
        return Ok(json!({"success": false, "message": format!(
            "target 特征({names})不是偏置面。必须先用 CATIA 偏置命令生成偏置特征并命名 {target_prefix}1,程序靠它读取整批偏置方向"
        )}));
    };

    let other_names: Vec<String> = target_hits
        .iter()
        .filter(|(_, sh, _)| !sh.same_object(&target_sh))
        .map(|(_, _, n)| n.clone())
        .collect();
    if !other_names.is_empty() {
        progress(&format!(
            "⚠ 发现多个 {target_prefix} 标记,本次使用 '{target_name}',忽略: {}",
            other_names
                .iter()
                .take(5)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    let b_direction = match target_sh.get("OffsetDirection").and_then(|v| v.as_bool()) {
        Ok(d) => d,
        Err(e) => {
            return Ok(
                json!({"success": false, "message": format!("读取 target 偏置方向失败: {e}")}),
            );
        }
    };
    let src_name = target_hb
        .name()
        .unwrap_or_else(|_| "源几何体集".to_string());
    progress(&format!(
        "target 标记: '{target_name}'(位于 '{src_name}'),整批方向 orientation={b_direction}"
    ));

    // ── 2. 收集源集中可偏置的曲面(跳过 target 标记) ──
    let mut src_shapes: Vec<(String, rscatiacli::dispatch::ComObj)> = Vec::new();
    let mut skipped_other = 0;
    let total_in_set = target_hb
        .hybrid_shapes()
        .and_then(|s| s.count())
        .unwrap_or(0);
    let shapes = target_hb.hybrid_shapes()?;
    for i in 1..=total_in_set {
        let Ok(sh) = shapes.item(i) else {
            continue;
        };
        let name = sh.name().unwrap_or_else(|_| format!("shape_{i}"));
        if name.to_lowercase().starts_with(target_prefix) {
            continue;
        }
        if is_offsettable(sh.com()) {
            src_shapes.push((name, sh.com().clone()));
        } else {
            skipped_other += 1;
        }
    }

    let total = src_shapes.len() as i32;
    if total == 0 {
        return Ok(json!({"success": false, "message": format!(
            "源几何体集 '{src_name}' 内没有可偏置的曲面(支持显式曲面/偏置面/提取面)"
        )}));
    }
    progress(&format!(
        "源集 '{src_name}' 共 {total_in_set} 个元素,可偏置曲面 {total} 个(跳过非曲面 {skipped_other} 个)"
    ));

    // ── 3. 建输出集(重名自动加后缀) ──
    let val_txt = g_format(offset_value);
    let out_name = unique_geoset_name(&format!("B_SURFACES_{src_name}_{val_txt}mm"), &hbs);
    let out_body = hbs.add()?;
    out_body.set_name(&out_name)?;
    let non_name = unique_geoset_name(
        &format!("NON_OFFSET_B_SURFACES_{src_name}_{val_txt}mm"),
        &hbs,
    );
    let mut non_off_body = Some(hbs.add()?);
    non_off_body.as_ref().unwrap().set_name(&non_name)?;
    progress(&format!("输出集: '{out_name}',补偿集: '{non_name}'"));

    // ── 4. 批量偏置 ──
    let mut failed_items: Vec<Value> = Vec::new();
    for (idx, (sname, shape)) in src_shapes.iter().enumerate() {
        let reference = part.create_reference_from_object(shape);
        let off = reference.and_then(|r| hsf.add_new_offset(&r, offset_value, b_direction, 0.01));
        match off {
            Ok(off) => {
                if let Err(e) = (|| -> ComResult<()> {
                    out_body.append_hybrid_shape(&off)?;
                    part.set_in_work_object(&off)?;
                    let _ = off.set_name(sname); // 改名失败不致命(重名等)
                    part.update_object(&off)
                })() {
                    let reason = e.to_string().chars().take(120).collect::<String>();
                    progress(&format!("  [{}/{total}] {sname} ✗ {reason}", idx + 1));
                    failed_items.push(json!({"name": sname, "reason": reason}));
                    // 删除刚创建的失败偏置特征(照原宏 Selection.Delete)
                    select_delete(&sel, &off);
                    // 原面原样复制进补偿集(照原宏 Copy/Paste)
                    if let Some(nb) = non_off_body.as_ref() {
                        if !copy_to(&sel, shape, nb) {
                            progress("    ⚠ 原面复制到补偿集失败");
                        } else {
                            let pasted = nb
                                .hybrid_shapes()
                                .and_then(|s| Ok((s.count()?, s)))
                                .and_then(|(c, s)| s.item(c));
                            if let Ok(p) = pasted {
                                let _ = p.set_name(sname);
                            }
                        }
                    }
                } else {
                    progress(&format!("  [{}/{total}] {sname} ✓", idx + 1));
                }
            }
            Err(e) => {
                let reason = e.to_string().chars().take(120).collect::<String>();
                progress(&format!("  [{}/{total}] {sname} ✗ {reason}", idx + 1));
                failed_items.push(json!({"name": sname, "reason": reason}));
                if let Some(nb) = non_off_body.as_ref() {
                    if !copy_to(&sel, shape, nb) {
                        progress("    ⚠ 原面复制到补偿集失败");
                    }
                }
            }
        }
    }

    // ── 5. 收尾:删 target 标记、着色、空补偿集删除 ──
    let target_deleted = if keep_target {
        false
    } else {
        let deleted = select_delete(&sel, &target_sh);
        if deleted {
            progress(&format!("已删除 target 标记 '{target_name}'"));
        } else {
            progress("⚠ target 标记删除失败(可手动删除)");
        }
        deleted
    };

    color_container(&sel, &out_body, 0, 100, 0, 1);

    let non_off_count = non_off_body
        .as_ref()
        .and_then(|nb| nb.hybrid_shapes().and_then(|s| s.count()).ok())
        .unwrap_or(0);
    if non_off_count == 0 {
        if let Some(nb) = non_off_body.as_ref() {
            select_delete(&sel, nb);
        }
        non_off_body = None;
    } else if let Some(nb) = non_off_body.as_ref() {
        color_container(&sel, nb, 148, 0, 211, 1);
    }

    if let Err(e) = part.update() {
        let brief = e.to_string().chars().take(150).collect::<String>();
        progress(&format!("⚠ 最终整体 Update 失败,请检查模型: {brief}"));
    }

    let failed_count = failed_items.len() as i32;
    let ok_count = total - failed_count;
    let mut message =
        format!("曲面偏置完成: 成功 {ok_count}/{total},失败 {failed_count};输出集 '{out_name}'");
    if non_off_body.is_some() {
        message.push_str(&format!(";失败原面已复制到 '{non_name}'"));
    }
    if skipped_other > 0 {
        message.push_str(&format!(";跳过非曲面元素 {skipped_other} 个"));
    }
    message.push_str(";请手动保存文档(Ctrl+S)");

    Ok(json!({
        "success": true,
        "message": message,
        "data": {
            "source_geoset": src_name,
            "target_name": target_name,
            "direction": b_direction,
            "offset_value": offset_value,
            "total": total,
            "offsetted_count": ok_count,
            "failed_count": failed_count,
            "output_geoset": out_name,
            "non_offset_geoset": if non_off_body.is_some() { json!(non_name) } else { Value::Null },
            "failed_items": failed_items,
            "keep_target": keep_target,
            "target_deleted": target_deleted,
        }
    }))
}
