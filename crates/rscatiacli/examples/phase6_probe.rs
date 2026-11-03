//! 阶段 6 只读真机探针：验证 GSD 方向对象的 VBA 中转读数与
//! `Part.hybrid_shape_factory` 的工厂获取。不写任何文档状态。
//!
//! 注意：GSD 特征创建会写文档，本探针只测「创建 HybridShapeDirection
//! 所需的 Part/工厂获取路径 + 方向按坐标读 X 分量」这条无副作用链——
//! AddNewDirection 本身不落树（纯构造对象），可以安全调用。
//!
//! ```text
//! cargo run -p rscatiacli --example phase6_probe
//! ```

use std::time::Duration;

use rscatiacli::{objects::HybridShapeDirection, with_application, ComObject, FromComObj};

fn main() {
    let result = with_application(
        Duration::from_secs(120),
        |msg| eprintln!("[probe] {msg}"),
        |app, _log| {
            let doc = app.active_document()?;
            let part_doc = doc.part_document();
            let part = match part_doc.part() {
                Ok(p) => p,
                Err(_) => {
                    println!("active doc is not a Part; factory probe skipped (OK path)");
                    return Ok(());
                }
            };

            let hsf = part.hybrid_shape_factory()?;
            println!("HybridShapeFactory acquired");

            // 纯构造对象：AddNewDirectionByCoord 不进规格树（与既有工具用法一致）。
            let raw = hsf.add_new_direction_by_coord(0.0, 0.0, 1.0)?;
            let dir = HybridShapeDirection::from_com(raw.com().clone());
            println!("HybridShapeDirection created (coord 0,0,1)");

            let ss = app.system_service()?;
            let x = dir.get_x(&ss);
            let y = dir.get_y(&ss);
            let z = dir.get_z(&ss);
            println!("Direction.get_x = {x:?}");
            println!("Direction.get_y = {y:?}");
            println!("Direction.get_z = {z:?}");

            let t = dir.direction_type();
            println!("Direction.type  = {t:?}");

            Ok(())
        },
    );
    match result {
        Ok(_) => println!("phase6 probe: OK"),
        Err(e) => {
            eprintln!("phase6 probe FAILED: {e}");
            std::process::exit(1);
        }
    }
}
