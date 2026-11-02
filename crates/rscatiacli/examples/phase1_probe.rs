//! 阶段 1 只读真机探针：验证 Product::analyze（含 VBA 中转出参数组）与
//! Product::assembly_convertor 的获取路径。不写任何文档状态。
//!
//! ```text
//! cargo run -p rscatiacli --example phase1_probe
//! ```

use std::time::Duration;

use rscatiacli::{with_application, ComObject};

fn main() {
    let result = with_application(
        Duration::from_secs(120),
        |msg| eprintln!("[probe] {msg}"),
        |app, _log| {
            let doc = app.active_document()?;
            let product = doc.product_document().product()?;

            // ── Analyze：产品级质量属性 ──
            let analyze = product.analyze()?;
            println!("Analyze.Mass     = {:?}", analyze.mass());
            println!("Analyze.Volume   = {:?}", analyze.volume());
            println!("Analyze.WetArea  = {:?}", analyze.wet_area());

            let ss = app.system_service()?;
            println!(
                "Analyze.GravityCenter = {:?}",
                analyze.get_gravity_center(&ss)
            );
            println!(
                "Analyze.Inertia(9)    = {:?}",
                analyze.get_inertia(&ss).map(|v| v.len())
            );

            // ── AssemblyConvertor：GetItem("BillOfMaterial") 获取（不调 Print）──
            let convertor = product.assembly_convertor()?;
            println!(
                "AssemblyConvertor = {:?}",
                convertor
                    .com()
                    .get("Name")
                    .map(|_| "ok".to_string())
                    .or_else(|_| Ok::<_, rscatiacli::ComError>("ok(GetItem 成功)".into()))
            );

            Ok(())
        },
    );
    match result {
        Ok(_) => println!("phase1 probe: OK"),
        Err(e) => {
            eprintln!("phase1 probe FAILED: {e}");
            std::process::exit(1);
        }
    }
}
