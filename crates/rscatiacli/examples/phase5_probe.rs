//! 阶段 5 只读真机探针：验证产品级 `Parameters` / `Relations` 获取与读数。
//! 不写任何状态。
//!
//! ```text
//! cargo run -p rscatiacli --example phase5_probe
//! ```

use std::time::Duration;

use rscatiacli::{with_application, ComCollection, ComObject};

fn main() {
    let result = with_application(
        Duration::from_secs(120),
        |msg| eprintln!("[probe] {msg}"),
        |app, _log| {
            let doc = app.active_document()?;
            let product = doc.product_document().product()?;

            let params = product.parameters()?;
            let count = params.count()?;
            println!("Product.Parameters.count = {count}");
            for (i, p) in params.items()?.into_iter().take(3).enumerate() {
                println!(
                    "  Param[{}] name={:?} value_str={:?} read_only={}",
                    i + 1,
                    p.name(),
                    p.value_as_string(),
                    p.read_only().unwrap_or(false)
                );
            }

            let relations = product.relations()?;
            println!("Product.Relations.count = {:?}", relations.count());

            let ps = params.root_parameter_set()?;
            println!("RootParameterSet.name = {:?}", ps.name());

            Ok(())
        },
    );
    match result {
        Ok(_) => println!("phase5 probe: OK"),
        Err(e) => {
            eprintln!("phase5 probe FAILED: {e}");
            std::process::exit(1);
        }
    }
}
