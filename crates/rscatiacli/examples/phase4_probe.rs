//! 阶段 4 只读真机探针：验证 `Application.file_system` / `system_configuration`
//! / `windows` / `printers` 与 `Document.cameras` 的获取与读数。不写任何状态。
//!
//! ```text
//! cargo run -p rscatiacli --example phase4_probe
//! ```

use std::time::Duration;

use rscatiacli::{with_application, ComCollection};

fn main() {
    let result = with_application(
        Duration::from_secs(120),
        |msg| eprintln!("[probe] {msg}"),
        |app, _log| {
            // ── SystemConfiguration ──
            let sc = app.system_configuration()?;
            println!(
                "SystemConfiguration: release={:?} version={:?} os={:?} sp={:?} products={}",
                sc.release(),
                sc.version(),
                sc.operating_system(),
                sc.service_pack().unwrap_or_default(),
                sc.product_count().unwrap_or(-1)
            );

            // ── FileSystem ──
            let fs = app.file_system()?;
            println!(
                "FileSystem: file_sep={:?} path_sep={:?}",
                fs.file_separator(),
                fs.path_separator()
            );
            println!(
                "TemporaryDirectory = {:?}",
                fs.temporary_directory().and_then(|f| f.path())
            );
            println!("FileSystem: 拼接={:?}", fs.concatenate_paths("a", "b"));

            // ── Windows / Printers / Cameras ──
            let windows = app.windows()?;
            println!("Windows.count = {:?}", windows.count());
            let printers = app.printers()?;
            println!("Printers.count = {:?}", printers.count());
            let doc = app.active_document()?;
            let cameras = doc.cameras()?;
            println!("Cameras.count = {:?}", cameras.count());
            if cameras.count()? > 0 {
                let cam = cameras.item(1)?;
                println!("Camera[1].type = {:?}", cam.camera_type());
            }

            Ok(())
        },
    );
    match result {
        Ok(_) => println!("phase4 probe: OK"),
        Err(e) => {
            eprintln!("phase4 probe FAILED: {e}");
            std::process::exit(1);
        }
    }
}
