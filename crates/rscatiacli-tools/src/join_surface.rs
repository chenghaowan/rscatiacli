//! join_surface：面缝合（稳妥版分组 + Join + datum 去参）。
//!
//! 对应 ltl-manage resources/sidecars/join_surface/join_surface.py（+bridge）。
//! 流程：取面数最多的几何图形集为源 → 每组以一个种子面出发，距离 ≤ 容差的
//! 后续面（最多看 10 个）移入新组 partsurface_N → 组内全部面 Join(Connex=1)
//! → AddNewSurfaceDatum 去参 → 删除 Join 原件 → 染色。
//! 结果 JSON 与 Python 版同形（扁平：groups/total_joined/total_skipped/
//! dparameterized，无 data 包装）。
//!
//! 注：本工具未接入前端（offset/stitch/trim 才是 UI 三件），契约按 CLI 口径对齐。

use serde_json::{json, Value};
use std::time::Duration;

use rscatiacli::objects::{HybridBody, HybridShapeFactory, Part, SPAWorkbench};
use rscatiacli::{attach_with_guard, ComCollection, ComObject, ComResult, Progress};

use crate::surface_common::move_shape;

/// 每组往后查找的窗口大小（Python SEEK = 10）。
const SEEK: i32 = 10;

const COLORS: [(i32, i32, i32); 10] = [
    (255, 0, 0),
    (0, 255, 0),
    (0, 0, 255),
    (255, 255, 0),
    (255, 0, 255),
    (0, 255, 255),
    (255, 128, 0),
    (128, 0, 255),
    (0, 128, 0),
    (0, 0, 128),
];

fn name_of(obj: &impl ComObject) -> String {
    obj.name().unwrap_or_else(|_| "<unknown>".to_string())
}

/// 找到源体（直属 HybridShapes 最多的图形集，含一级子集，与 Python 口径一致）。
fn find_source_geoset(hbs: &rscatiacli::objects::HybridBodies) -> Option<HybridBody> {
    let mut src: Option<HybridBody> = None;
    let mut max_n = 0;
    for hb in hbs.items().ok()? {
        if let Ok(subs) = hb.hybrid_bodies() {
            for sub in subs.items().ok()?.into_iter() {
                let n = sub.hybrid_shapes().and_then(|s| s.count()).unwrap_or(0);
                if n > max_n {
                    max_n = n;
                    src = Some(sub);
                }
            }
        }
        let n = hb.hybrid_shapes().and_then(|s| s.count()).unwrap_or(0);
        if n > max_n {
            max_n = n;
            src = Some(hb);
        }
    }
    src
}

/// 两元素最小距离（mm）；失败返回一个大数（与 Python 的 999999 同口径）。
fn min_distance(spa: &SPAWorkbench, part: &Part, a: &impl ComObject, b: &impl ComObject) -> f64 {
    let Ok(ra) = part.create_reference_from_object(a) else {
        return 999_999.0;
    };
    let Ok(rb) = part.create_reference_from_object(b) else {
        return 999_999.0;
    };
    let Ok(m) = spa.get_measurable(&ra) else {
        return 999_999.0;
    };
    m.get_minimum_distance(&rb).unwrap_or(999_999.0)
}

/// Join 组内所有面 → SurfaceDatum(去参) → 删 Join。失败返回 Err 信息。
fn join_group(
    part: &Part,
    hsf: &HybridShapeFactory,
    body: &HybridBody,
    tol: f64,
) -> ComResult<rscatiacli::objects::Entity> {
    let shapes = body.hybrid_shapes()?;
    let count = shapes.count()?;
    let mut surfs = Vec::with_capacity(count.max(0) as usize);
    for i in 1..=count {
        surfs.push(shapes.item(i)?);
    }

    let ra = part.create_reference_from_object(&surfs[0])?;
    let rb = part.create_reference_from_object(&surfs[1])?;
    let j = hsf.add_new_join(&ra, &rb)?;
    j.set_connex(1)?;
    j.set_healing_mode(0)?;
    j.set_simplify(1)?;
    j.set_suppress_mode(1)?;
    j.set_deviation(tol)?;
    j.set_federation_propagation(0)?;
    j.set_manifold(1)?;
    for s in &surfs[2..] {
        j.add_element(&part.create_reference_from_object(s)?)?;
    }
    body.append_hybrid_shape(&j)?;
    if let Err(e) = part.update() {
        // 更新失败清理刚创建的 Join（多次尝试，容忍失败，与 Python 一致）
        for _ in 0..3 {
            if body.remove(&j).is_ok() {
                break;
            }
        }
        let _ = part.update();
        return Err(e);
    }

    // 手动去参 = Create Datum（AddNewSurfaceDatum + DeleteObjectForDatum）；
    // CreateDatum/NoKeepMode 已在会话开头打开，这里与 Python 同步双保险。
    let rj = part.create_reference_from_object(&j)?;
    let d = hsf.add_new_surface_datum(&rj)?;
    body.append_hybrid_shape(&d)?;
    part.update()?;
    hsf.delete_object_for_datum(&rj)?;
    part.update()?;
    Ok(d)
}

/// 工具主体。**必须在 STA 线程上执行。**
pub fn run(params: &Value, progress: Progress<'_>) -> ComResult<Value> {
    let tol = params
        .get("tolerance")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.1);
    let max_groups = params
        .get("max_groups")
        .and_then(|v| v.as_i64())
        .unwrap_or(10)
        .max(0) as i32;

    let app = attach_with_guard(progress)?;
    let doc = app.active_document()?;
    progress(&format!("文档: {}", name_of(&doc)));
    let part = doc.part_document().part()?;
    let hsf = part.hybrid_shape_factory()?;
    let hbs = part.hybrid_bodies()?;
    let sel = doc.selection()?;
    let spa = doc.part_document().spa_workbench()?;

    // 激活 Create Datum + No Keep Mode（datum 化去参的前置开关，尽力而为）
    match part.set_create_datum(true) {
        Ok(()) => progress("CreateDatum=True ✅"),
        Err(e) => progress(&format!("CreateDatum设置失败: {e}")),
    }
    match part.set_no_keep_mode(true) {
        Ok(()) => progress("NoKeepMode=True ✅"),
        Err(e) => progress(&format!("NoKeepMode设置失败: {e}")),
    }

    // 找到源体（最多面的那个）
    let Some(src) = find_source_geoset(&hbs) else {
        return Ok(
            json!({"success": true, "message": "无源体", "groups": [], "total_joined": 0, "total_skipped": 0, "dparameterized": false}),
        );
    };
    let src_shapes = src.hybrid_shapes()?;
    let total_src = src_shapes.count()?;
    progress(&format!("源体: '{}' ({total_src}面)", name_of(&src)));
    progress(&format!(
        "每组只查后面{SEEK}个面，本次最多处理{max_groups}组"
    ));

    let mut joined_n = 0;
    let mut failed_n = 0;
    let mut ci = 0usize;
    let mut results: Vec<Value> = Vec::new();
    let mut i: i32 = 1; // 当前 src 中的位置

    for group_no in 1..=max_groups {
        if i > total_src {
            progress("没有更多面了");
            break;
        }

        // 找下一个未处理的面（跳过已被移走的空位）
        while i <= total_src && src_shapes.item(i).is_err() {
            i += 1;
        }
        if i > total_src {
            break;
        }

        // 新建 partsurface_N 体
        let body = hbs.add()?;
        body.set_name(&format!("partsurface_{group_no}"))?;
        progress(&format!("\n=== 组{group_no}: '{}' ===", name_of(&body)));

        // 种子：当前面
        let seed = src_shapes.item(i)?;
        progress(&format!("种子[{i}]: {}", name_of(&seed)));
        move_shape(&part, &body, &seed);
        i += 1;

        // 找后面最多 SEEK 个面
        let limit = (i + SEEK).min(total_src + 1);
        while i < limit {
            let s = match src_shapes.item(i) {
                Ok(s) => s,
                Err(_) => {
                    i += 1;
                    continue;
                }
            };
            let sn = name_of(&s);
            let d = min_distance(&spa, &part, &seed, &s);
            progress(&format!("  [{i}] {sn} d={d:.4}"));

            if d <= tol {
                progress("    → 入组");
                move_shape(&part, &body, &s);
            } else {
                progress("    → 跳过");
            }
            i += 1;
        }

        // Join 组中所有面
        let cnt = body.hybrid_shapes().and_then(|s| s.count()).unwrap_or(0);
        progress(&format!("  组内 {cnt} 面，尝试 Join..."));

        if cnt < 2 {
            progress("  单面跳过");
            // Python 走 hbs.Remove(body)（部分版本不支持则留下）；这里用
            // Selection.Delete（stitch/trim 同款已验证通道），失败同样不致命
            let _ = sel.clear();
            if sel.add(&body).is_ok() {
                let _ = sel.delete();
            }
            let _ = sel.clear();
            failed_n += cnt;
            continue;
        }

        match join_group(&part, &hsf, &body, tol) {
            Ok(result) => {
                let result_name = name_of(&result);
                progress(&format!("  ✅ Join成功: {result_name}"));
                // 染色（带 3 次重试，间隔 50ms，与 Python 一致）
                let c = COLORS[ci % COLORS.len()];
                for attempt in 0..3 {
                    let _ = sel.clear();
                    let ok = part
                        .create_reference_from_object(&result)
                        .and_then(|r| sel.add(&r))
                        .and_then(|_| sel.vis_properties())
                        .and_then(|vis| vis.set_real_color(c.0, c.1, c.2, 0));
                    let _ = sel.clear();
                    if ok.is_ok() {
                        break;
                    }
                    if attempt < 2 {
                        std::thread::sleep(Duration::from_millis(50));
                    }
                }
                ci += 1;
                joined_n += cnt;

                // 清理 body 中除结果外的面（Remove 不被支持时保留，与 Python 同口径）
                let clean = body.hybrid_shapes()?;
                let clean_cnt = clean.count()?;
                for ti in (1..=clean_cnt).rev() {
                    if let Ok(s) = clean.item(ti) {
                        if name_of(&s) != result_name {
                            let _ = body.remove(&s);
                        }
                    }
                }
                let _ = part.update();

                results.push(json!({"joined": cnt, "name": result_name, "body": name_of(&body)}));
            }
            Err(e) => {
                let brief = e.to_string().chars().take(60).collect::<String>();
                progress(&format!("  ✗ Join失败: {brief}"));
                // body 留在原地（含原始面），用户可检查
                failed_n += cnt;
            }
        }
    }

    let remain = src.hybrid_shapes().and_then(|s| s.count()).unwrap_or(0);
    progress(&format!(
        "\n=== 本次: {joined_n}缝合/{failed_n}跳过({max_groups}组) ==="
    ));
    progress(&format!("源体剩余: {remain}面"));

    Ok(json!({
        "success": true,
        "message": format!("本次: {joined_n}缝合/{failed_n}跳过"),
        "groups": results,
        "total_joined": joined_n,
        "total_skipped": failed_n,
        "dparameterized": joined_n > 0,
    }))
}
