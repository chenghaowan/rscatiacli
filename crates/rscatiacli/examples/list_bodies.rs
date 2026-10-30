//! List the body names of the active part.
//!
//! Run with a CATIA session open on a `.CATPart`:
//!
//! ```bash
//! cargo run -p rscatiacli --example list_bodies
//! ```

use std::time::Duration;

use rscatiacli::{with_application, ComCollection, ComObject, ComResult};

fn main() -> ComResult<()> {
    let names = with_application(
        Duration::from_secs(120),
        |msg| eprintln!("{msg}"),
        |app, _log| {
            let doc = app.active_document()?;
            let part = doc.part_document().part()?;

            let mut names = Vec::new();
            for body in part.bodies()?.items()? {
                names.push(body.name()?);
            }
            Ok(names)
        },
    )?;

    println!("{names:#?}");
    Ok(())
}
