//! stitch_surface：缝合曲面（按集合图文集批量缝合，边界环数校验 + 自动降精度）。
//!
//! 对应 ltl-manage resources/sidecars/stitch_surface/stitch_surface.py。
//! 递归遍历全部几何图形集，每个集合内全部曲面 AddNewJoin(Connex=0)，
//! AddNewBoundaryOfSurface + AddNewDatums 数边界环数：1 环正常；2 环（内部
//! 缝隙）按精度序列 [0.001, 0.01, 0.1] 降精度重试，降到最低仍 2 环则保留；
//! 结果放入 Stitch_<集合名> 并染随机色。action=isolate/unisolate 隔离显示
//! 缝合结果。out_prefix 开头的旧输出集先删（幂等）。
//!
//! 与 Python 版差异：同名 .CATPart 自动切换未移植（与 hide_show 同口径）。

use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};

use rscatiacli::objects::{Document, HybridBody, Part, Selection};
use rscatiacli::{attach_with_guard, ComCollection, ComObject, ComResult, Progress};

use crate::surface_common::{cleanup_geoset, is_surface, iter_geosets};

const DEFAULT_TOLERANCES: [f64; 3] = [0.001, 0.01, 0.1];
const OUT_PREFIX: &str = "Stitch_";
const GAP_BOUNDARY_COUNT: i64 = 2;

fn name_of(obj: &impl ComObject) -> String {
    obj.name().unwrap_or_default()
}

/// 鲜艳随机 RGB（HSV 随机色相，S=0.85 V=0.95，与 Python colorsys 同参）。
fn random_color() -> (i32, i32, i32) {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let seed = COUNTER
        .fetch_add(1, Ordering::Relaxed)
        .wrapping_mul(0x9E37_79B9_7F4A_7C15);
    let h = (seed % 3600) as f64 / 3600.0;
    let (s, v) = (0.85f64, 0.95f64);
    let i = (h * 6.0).floor() as i64;
    let f = h * 6.0 - i as f64;
    let p = v * (1.0 - s);
    let q = v * (1.0 - f * s);
    let t = v * (1.0 - (1.0 - f) * s);
    let (r, g, b) = match i.rem_euclid(6) {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    (
        (r * 255.0).round() as i32,
        (g * 255.0).round() as i32,
        (b * 255.0).round() as i32,
    )
}

/// 给单个特征染随机色（Selection.Add 特征本体），返回所用 RGB。
fn color_shape(sel: &Selection, shape: &impl ComObject) -> Option<(i32, i32, i32)> {
    let rgb = random_color();
    let _ = sel.clear();
    let ok = sel
        .add(shape)
        .is_ok()
        .then_some(())
        .and_then(|_| sel.vis_properties().ok())
        .map(|vis| vis.set_real_color(rgb.0, rgb.1, rgb.2, 1))
        .is_some_and(|r| r.is_ok());
    let _ = sel.clear();
    ok.then_some(rgb)
}

/// 把集合内全部曲面 Join 到一起，返回 Join 特征。
///
/// Connex 用 0（不限制连通性）：Connex=1 在集合内存在不相连曲面时 Update
/// 直接失败（实测），Connex=0 保持多域但稳定成功；偏差太小同样可能失败，
/// 由调用方按精度序列用全新集合重试。
fn join_surfaces(
    part: &Part,
    hsf: &rscatiacli::objects::HybridShapeFactory,
    body: &HybridBody,
    surfs: &[rscatiacli::objects::Entity],
    tol: f64,
) -> ComResult<rscatiacli::objects::Entity> {
    let mut refs = Vec::with_capacity(surfs.len());
    for s in surfs {
        refs.push(part.create_reference_from_object(s)?);
    }
    let j = hsf.add_new_join(&refs[0], &refs[1])?;
    j.set_connex(0)?;
    j.set_healing_mode(0)?;
    j.set_simplify(1)?;
    j.set_suppress_mode(1)?;
    j.set_deviation(tol)?;
    j.set_federation_propagation(0)?;
    j.set_manifold(1)?;
    for r in &refs[2..] {
        j.add_element(r)?;
    }
    body.append_hybrid_shape(&j)?;
    if let Err(e) = part.update() {
        // 更新失败时清理刚创建的 Join
        let _ = body.remove(&j);
        let _ = part.update();
        return Err(e);
    }
    Ok(rscatiacli::objects::Entity::new(j.com().clone()))
}

/// 统计缝合结果的边界环数（临时集合数完即删）。
fn count_boundaries(
    part: &Part,
    hsf: &rscatiacli::objects::HybridShapeFactory,
    join: &rscatiacli::objects::Entity,
    sel: &Selection,
) -> ComResult<usize> {
    let hbs = part.hybrid_bodies()?;
    let tmp = hbs.add()?;
    tmp.set_name("_stitch_cnt")?;
    let result = (|| -> ComResult<usize> {
        let rj = part.create_reference_from_object(join)?;
        let bnd = hsf.add_new_boundary_of_surface(&rj)?;
        tmp.append_hybrid_shape(&bnd)?;
        if part.update_object(&bnd).is_err() {
            let _ = part.update();
        }
        let rb = part.create_reference_from_object(&bnd)?;
        let datums = hsf.add_new_datums(&rb)?;
        Ok(datums.len())
    })();
    cleanup_geoset(sel, part, &tmp);
    result
}

/// 收集 Part 顶层容器（Body / 顶层几何图形集 / 轴系 / 参考面）。
fn collect_top_containers(part: &Part) -> Vec<rscatiacli::dispatch::ComObj> {
    let mut out = Vec::new();
    for prop in ["Bodies", "HybridBodies", "AxisSystems"] {
        if let Ok(coll) = part.com().get(prop).and_then(|v| v.as_obj()) {
            if let Ok(count) = coll.get("Count").and_then(|v| v.as_i32()) {
                for i in 1..=count {
                    if let Ok(item) = rscatiacli::item_at(&coll, i) {
                        out.push(item);
                    }
                }
            }
        }
    }
    if let Ok(origin) = part.origin_elements() {
        for plane in [origin.plane_xy(), origin.plane_yz(), origin.plane_zx()]
            .into_iter()
            .flatten()
        {
            out.push(plane.com().clone());
        }
    }
    out
}

/// 隔离显示 / 取消隔离：只显示 Stitch_ 前缀的缝合结果，隐藏其他顶层容器。
fn isolate_stitch(
    part: &Part,
    sel: &Selection,
    dry_run: bool,
    restore: bool,
    progress: Progress<'_>,
) -> Value {
    let hbs = part.hybrid_bodies();

    if restore {
        let targets = collect_top_containers(part);
        if dry_run {
            return json!({
                "success": true,
                "message": format!("[dry-run] 将恢复显示 {} 个顶层容器(取消隔离)。未做任何修改", targets.len()),
                "data": {"action": "unisolate", "count": 0, "dry_run": true},
            });
        }
        let _ = sel.clear();
        for v in &targets {
            let _ = sel.add(v);
        }
        if let Ok(vis) = sel.vis_properties() {
            let _ = vis.set_show(0); // 0 = 显示
        }
        let _ = sel.clear();
        let _ = part.update();
        progress(&format!("已恢复显示 {} 个顶层容器", targets.len()));
        return json!({
            "success": true,
            "message": format!("已取消隔离, 恢复显示 {} 个顶层容器", targets.len()),
            "data": {"action": "unisolate", "count": 0, "restored": targets.len()},
        });
    }

    let stitch_geosets: Vec<HybridBody> = match &hbs {
        Ok(h) => h
            .items()
            .unwrap_or_default()
            .into_iter()
            .filter(|hb| name_of(hb).starts_with(OUT_PREFIX))
            .collect(),
        Err(_) => Vec::new(),
    };

    if stitch_geosets.is_empty() {
        return json!({
            "success": false,
            "message": format!("未找到以「{OUT_PREFIX}」开头的缝合结果集合, 请先执行「开始缝合」"),
            "data": {"action": "isolate", "isolated": [], "count": 0},
        });
    }

    let names: Vec<String> = stitch_geosets
        .iter()
        .map(|hb| {
            let n = name_of(hb);
            if n.is_empty() {
                "(未命名)".to_string()
            } else {
                n
            }
        })
        .collect();

    if dry_run {
        return json!({
            "success": true,
            "message": format!(
                "[dry-run] 将隔离显示 {} 个缝合结果集合: {}; 其他元素隐藏。未做任何修改",
                stitch_geosets.len(), names.join(", ")
            ),
            "data": {"action": "isolate", "isolated": names, "count": names.len(), "dry_run": true},
        });
    }

    // 1. 隐藏所有顶层容器
    let hide_targets = collect_top_containers(part);
    let _ = sel.clear();
    for v in &hide_targets {
        let _ = sel.add(v);
    }
    if let Ok(vis) = sel.vis_properties() {
        let _ = vis.set_show(1); // 1 = 隐藏
    }
    let _ = sel.clear();

    // 2. 显示缝合结果集合
    for hb in &stitch_geosets {
        let _ = sel.add(hb);
    }
    if let Ok(vis) = sel.vis_properties() {
        let _ = vis.set_show(0); // 0 = 显示
    }
    let _ = sel.clear();

    let _ = part.update();

    progress(&format!(
        "已隐藏 {} 个容器, 显示 {} 个缝合结果集合",
        hide_targets.len(),
        names.len()
    ));
    json!({
        "success": true,
        "message": format!(
            "已隔离显示 {} 个缝合结果: {}; 其他元素已隐藏(再点一次「取消隔离」可恢复)",
            stitch_geosets.len(), names.join(", ")
        ),
        "data": {"action": "isolate", "isolated": names, "count": names.len(), "hidden": hide_targets.len()},
    })
}

/// 工具主体。**必须在 STA 线程上执行。**
pub fn run(params: &Value, progress: Progress<'_>) -> ComResult<Value> {
    let empty_data = json!({
        "tolerances": DEFAULT_TOLERANCES,
        "geosets": [], "total_stitched": 0, "total_gap_kept": 0,
        "total_skipped": 0, "total_failed": 0, "dry_run": false,
    });

    let dry_run = params
        .get("dry_run")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let action = params
        .get("action")
        .and_then(|v| v.as_str())
        .unwrap_or("stitch")
        .to_lowercase();
    let tolerances: Vec<f64> = match params.get("tolerances").and_then(|v| v.as_array()) {
        Some(arr) if !arr.is_empty() => arr.iter().filter_map(|v| v.as_f64()).collect(),
        _ => DEFAULT_TOLERANCES.to_vec(),
    };
    let tolerances = if tolerances.is_empty() {
        DEFAULT_TOLERANCES.to_vec()
    } else {
        tolerances
    };

    let app = attach_with_guard(progress)?;
    let doc = match app.active_document() {
        Ok(d) => d,
        Err(e) => {
            return Ok(
                json!({"success": false, "message": format!("获取活动文档失败: {e}"), "data": empty_data}),
            );
        }
    };
    if name_of(&doc).is_empty() && doc.full_name().is_err() {
        return Ok(json!({"success": false, "message": "CATIA 无活动文档", "data": empty_data}));
    }
    let doc_name = name_of(&doc);
    if !doc_name.to_lowercase().ends_with(".catpart") {
        let kind = if doc_name.to_lowercase().ends_with(".catproduct") {
            "装配"
        } else if doc_name.to_lowercase().ends_with(".catdrawing") {
            "工程图"
        } else {
            "非零件文档"
        };
        return Ok(json!({
            "success": false,
            "message": format!("当前是{kind}({doc_name}): Rust 版尚未移植同名零件自动切换, 请先打开对应 .CATPart 后重试"),
            "data": empty_data,
        }));
    }

    let alerts_prev = app.display_file_alerts().unwrap_or(true);
    let _ = app.set_display_file_alerts(false);

    let result = run_inner(&doc, dry_run, &action, &tolerances, empty_data, progress);
    let _ = app.set_display_file_alerts(alerts_prev);
    result
}

fn run_inner(
    doc: &Document,
    dry_run: bool,
    action: &str,
    tolerances: &[f64],
    empty_data: Value,
    progress: Progress<'_>,
) -> ComResult<Value> {
    let prt_doc = doc.part_document();
    let part: Part = prt_doc.part()?;
    let part_name = part.name().unwrap_or_else(|_| "Part".to_string());
    let hbs = part.hybrid_bodies()?;
    let hsf = part.hybrid_shape_factory()?;
    let sel = doc.selection()?;

    // 隔离显示 / 取消隔离：只显示 Stitch_ 前缀的缝合结果（先于 Update 校验，同 Python）
    if action == "isolate" || action == "unisolate" {
        return Ok(isolate_stitch(
            &part,
            &sel,
            dry_run,
            action == "unisolate",
            progress,
        ));
    }

    // 零件本身必须无更新错误
    if let Err(e) = part.update() {
        let brief = e.to_string().chars().take(120).collect::<String>();
        return Ok(
            json!({"success": false, "message": format!("零件存在更新错误,请先修复后再运行: {brief}"), "data": empty_data}),
        );
    }

    // 幂等：删除上次输出集合
    if !dry_run {
        let old_outputs: Vec<HybridBody> = iter_geosets(&hbs)
            .into_iter()
            .filter(|hb| name_of(hb).starts_with(OUT_PREFIX))
            .collect();
        for hb in old_outputs {
            let n = name_of(&hb);
            cleanup_geoset(&sel, &part, &hb);
            progress(&format!("已删除旧输出集合: {n}"));
        }
    }

    // 收集待处理集合（跳过输出集）
    let geosets: Vec<HybridBody> = iter_geosets(&hbs)
        .into_iter()
        .filter(|hb| !name_of(hb).starts_with(OUT_PREFIX))
        .collect();

    let mut results: Vec<Value> = Vec::new();
    let mut stitched_n = 0;
    let mut gap_kept_n = 0;
    let mut skipped_n = 0;
    let mut failed_n = 0;

    for gs in &geosets {
        let gs_name = {
            let n = name_of(gs);
            if n.is_empty() {
                "(未命名集合)".to_string()
            } else {
                n
            }
        };
        progress(&format!("\n=== 集合: {gs_name} ==="));

        // 收集集合内曲面（类型名过滤，按名字排序）
        let mut surfs: Vec<(String, rscatiacli::objects::Entity)> = Vec::new();
        if let Ok(shapes) = gs.hybrid_shapes() {
            if let Ok(count) = shapes.count() {
                for j in 1..=count {
                    let Ok(sh) = shapes.item(j) else {
                        continue;
                    };
                    if !is_surface(sh.com()) {
                        continue;
                    }
                    let name = sh.name().unwrap_or_else(|_| format!("shape_{j}"));
                    surfs.push((name, sh));
                }
            }
        }
        surfs.sort_by(|a, b| a.0.cmp(&b.0));

        if surfs.len() < 2 {
            progress(&format!("  曲面数量 {} < 2, 跳过", surfs.len()));
            skipped_n += 1;
            results.push(json!({
                "name": gs_name, "surface_count": surfs.len(), "tolerance": Value::Null,
                "boundary_count": Value::Null, "status": "skipped", "result_name": Value::Null,
            }));
            continue;
        }

        let preview = surfs
            .iter()
            .take(8)
            .map(|(n, _)| n.clone())
            .collect::<Vec<_>>()
            .join(", ");
        progress(&format!(
            "  曲面 {} 个: {}{}",
            surfs.len(),
            preview,
            if surfs.len() > 8 { "..." } else { "" }
        ));

        let out_name = format!("{OUT_PREFIX}{gs_name}");
        if dry_run {
            progress(&format!(
                "  [dry-run] 将创建输出集合 {out_name}, 精度序列 {tolerances:?}"
            ));
            results.push(json!({
                "name": gs_name, "surface_count": surfs.len(), "tolerance": Value::Null,
                "boundary_count": Value::Null, "status": "planned", "result_name": out_name,
            }));
            continue;
        }

        // 每次精度尝试都用全新集合，上次失败的 Join 残留不会污染下次尝试
        let mut best: Option<(f64, Option<i64>, rscatiacli::objects::Entity)> = None;
        for (idx, &tol) in tolerances.iter().enumerate() {
            let is_last = idx == tolerances.len() - 1;
            progress(&format!("  尝试缝合精度 {tol} ..."));
            let trial_body = match hbs.add().and_then(|b| {
                b.set_name("_stitch_trial")?;
                Ok(b)
            }) {
                Ok(b) => b,
                Err(_) => continue,
            };
            let trial_surfs: Vec<rscatiacli::objects::Entity> =
                surfs.iter().map(|(_, s)| s.clone()).collect();
            let join_com = match join_surfaces(&part, &hsf, &trial_body, &trial_surfs, tol) {
                Ok(j) => j,
                Err(e) => {
                    let brief = e.to_string().chars().take(80).collect::<String>();
                    progress(&format!("    ✗ 缝合失败: {brief}"));
                    cleanup_geoset(&sel, &part, &trial_body);
                    continue;
                }
            };
            let bcount = match count_boundaries(&part, &hsf, &join_com, &sel) {
                Ok(n) => Some(n as i64),
                Err(e) => {
                    let brief = e.to_string().chars().take(80).collect::<String>();
                    progress(&format!("    ⚠ 边界计数失败: {brief}"));
                    None
                }
            };
            progress(&format!(
                "    缝合结果边界环数: {}",
                bcount
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| "未知".into())
            ));
            // 2 环 = 曲面内部有缝隙(不希望)：非最低精度时降精度重试
            if bcount == Some(GAP_BOUNDARY_COUNT) && !is_last {
                progress("    → 内部有缝隙(2 环), 降低精度重试");
                cleanup_geoset(&sel, &part, &trial_body);
                continue;
            }
            // 其它情况(0/1/3+ 环, 或已降到最低精度的 2 环)一律保留
            if bcount == Some(GAP_BOUNDARY_COUNT) {
                progress("    → 已到最低精度仍为 2 环, 按需求保留");
            }
            trial_body.set_name(&out_name)?;
            best = Some((tol, bcount, join_com));
            break;
        }

        match best {
            None => {
                progress("  ✗ 全部精度缝合失败, 集合跳过");
                failed_n += 1;
                results.push(json!({
                    "name": gs_name, "surface_count": surfs.len(), "tolerance": Value::Null,
                    "boundary_count": Value::Null, "status": "failed", "result_name": out_name,
                }));
            }
            Some((tol_used, bcount, join_com)) => {
                stitched_n += 1;
                let rgb = color_shape(&sel, &join_com);
                let rgb_txt = rgb
                    .map(|(r, g, b)| format!("RGB({r}, {g}, {b})"))
                    .unwrap_or_else(|| "上色失败".to_string());
                let (status, boundary_value) = match bcount {
                    Some(GAP_BOUNDARY_COUNT) => {
                        gap_kept_n += 1;
                        progress(&format!(
                            "  ✅ 保留 2 环结果(精度 {tol_used}, {rgb_txt}), 输出 {out_name}"
                        ));
                        ("gap_kept", json!(bcount))
                    }
                    other => {
                        progress(&format!(
                            "  ✅ 缝合完成(精度 {tol_used}, {} 环, {rgb_txt}), 输出 {out_name}",
                            other
                                .map(|n| n.to_string())
                                .unwrap_or_else(|| "未知".into())
                        ));
                        ("ok", json!(bcount))
                    }
                };
                let color = rgb.map(|(r, g, b)| json!([r, g, b])).unwrap_or(Value::Null);
                results.push(json!({
                    "name": gs_name, "surface_count": surfs.len(), "tolerance": tol_used,
                    "boundary_count": boundary_value, "status": status, "result_name": out_name,
                    "color": color,
                }));
            }
        }
    }

    // 收尾更新
    if let Err(e) = part.update() {
        let brief = e.to_string().chars().take(150).collect::<String>();
        progress(&format!("⚠ 最终整体 Update 失败,请检查模型: {brief}"));
    }

    let data = json!({
        "tolerances": tolerances,
        "geosets": results,
        "total_stitched": stitched_n,
        "total_gap_kept": gap_kept_n,
        "total_skipped": skipped_n,
        "total_failed": failed_n,
        "dry_run": dry_run,
        "part_name": part_name,
    });

    if dry_run {
        return Ok(json!({
            "success": true,
            "message": format!(
                "[dry-run] 零件「{part_name}」共 {} 个集合将执行缝合: 精度序列 {tolerances:?}; 边界环数 1 视为正常, 2 环(内部缝隙)自动降精度重试, 降到 {} 仍 2 环则保留。未做任何修改",
                results.len(),
                tolerances.last().map(|t| t.to_string()).unwrap_or_default()
            ),
            "data": data,
        }));
    }

    let mut msg = format!("缝合曲面完成: 缝合 {stitched_n} 个集合");
    if gap_kept_n > 0 {
        msg.push_str(&format!(",其中 {gap_kept_n} 个为 2 环保留"));
    }
    msg.push_str(&format!(",跳过 {skipped_n} 个(曲面不足)"));
    if failed_n > 0 {
        msg.push_str(&format!(",失败 {failed_n} 个"));
    }
    msg.push_str(&format!(
        ";精度序列 {tolerances:?};输出集合前缀 {OUT_PREFIX};请手动保存文档(Ctrl+S)"
    ));

    Ok(json!({"success": true, "message": msg, "data": data}))
}
