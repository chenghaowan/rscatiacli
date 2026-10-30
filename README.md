# rscatiacli

**Late-bound COM bindings for CATIA V5 in Rust — a pycatia-style object model, plus a small tool layer.**

English | [简体中文](README.zh-CN.md)

`rscatiacli` talks to a running CATIA V5 session through `IDispatch` late binding
(`GetIDsOfNames` + `Invoke`). It exposes a typed, pycatia-style object model
(`Application`, `Document`, `Part`, `Body`, `Selection`, `Product`, ...) and keeps an
escape hatch to raw dispatch for members that are not wrapped yet.

The project was extracted from a production Tauri-based CAD management application.
The extraction rule is deliberately simple:

> **Wrap only what the application actually needs.** Where the app's migration goes,
> the bindings follow. This is not a 1:1 port of pycatia.

## Why another CATIA binding?

- **Rust, no Python runtime.** The original tools depended on a large Python sidecar
  (pycatia + `win32com`). This crate removes that dependency from the hot path.
- **Late binding, like `win32com.client.dynamic`.** CATIA's type library is incomplete
  (`Document.Product`, `Sheets`, ...) and SAFEARRAY out-parameters behave differently
  under static bindings. Name-based dispatch avoids those traps.
- **Production connection discipline.** STA-only calls, ROT attach, multi-instance
  guard, and no cached `Application` / `Document` pointers.

## Workspace

```
crates/rscatiacli        bindings library + `rscatiacli` diagnostics CLI
crates/rscatiacli-tools  tool business logic + `rscatiacli-tools` CLI
```

An application layer depends on both crates by path or git:

```toml
rscatiacli = { path = "../rscatiacli/crates/rscatiacli" }
rscatiacli-tools = { path = "../rscatiacli/crates/rscatiacli-tools" }
```

## Status

Bindings implemented and verified against a live CATIA V5 session:

- `Application`, `Documents`, `Document`
- `Part`, `Bodies` / `Body`, `HybridBodies` / `HybridBody`, `AxisSystems`, `OriginElements`
- `Selection`, `SelectionItem`, `VisProperties`
- `Product`, `Products`
- `ComCollection` (`count` / `item` / `items`) with method → property-get fallback
- Session diagnostics: `status`, `probe`, `put-selftest`, `ps`

Tools implemented in `rscatiacli-tools`:

| Tool | Script key | What it does |
| --- | --- | --- |
| colorize | `colorize-bodies` | Assigns a distinct palette color to every body of the active part. |
| body rename | `body_rename/rename_bodies.py` | Cleans body / geometrical-set names: keeps the last `keep_last` `\`-separated segments, replaces illegal characters, de-duplicates with `_2`, `_3`, ... |
| hide-show | `hide_show/hide_show.py` | Shows/hides bodies, geometrical sets, axis systems, reference planes and constraints; recurses into the part documents of a product. |

All three support `dry_run` where applicable and were exercised on real CATIA documents.

## Requirements

- Windows (COM / OLE through the `windows` crate).
- CATIA V5 already running. **The library never launches CATIA** — auto-launch creates a
  hidden background instance that breaks later automation.
- Rust stable (edition 2021).

## Quick start

```bash
cargo test --workspace

# Diagnostics (`status` works without CATIA; the others need a running session)
cargo run -p rscatiacli -- status
cargo run -p rscatiacli -- probe
cargo run -p rscatiacli -- put-selftest
cargo run -p rscatiacli -- ps

# Run a migrated tool outside the UI
cargo run -p rscatiacli-tools -- json hide_show/hide_show.py '{"action":"show-body","dry_run":true}'
cargo run -p rscatiacli-tools -- json body_rename/rename_bodies.py '{"dry_run":true}'
cargo run -p rscatiacli-tools -- colorize
```

## Library usage

```rust
use std::time::Duration;

use rscatiacli::{with_application, ComCollection, ComObject, ComResult};

fn main() -> ComResult<()> {
    let names = with_application(
        Duration::from_secs(120),
        |msg| eprintln!("{msg}"),
        |app, _log| {
            let doc = app.active_document()?;
            let part = doc.part()?;

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
```

For members that are not wrapped yet, use the escape hatch:

```rust
use rscatiacli::{Arg, ComObject};

let raw = body.com(); // &ComObj
let count = raw.get("Something")?.as_i32()?;
raw.call("SomeMethod", &[Arg::I32(1)])?;
```

## pycatia → rscatiacli

| pycatia | rscatiacli |
| --- | --- |
| `Application` | `Application` |
| `application.documents` / `.item(i)` | `Application::documents()` / `Documents::item(i)` |
| `PartDocument.part` | `Document::part()` |
| `part.bodies` / `.item(i)` | `Part::bodies()` / `Bodies::item(i)` |
| `Body.name` | `Body::name()` / `Body::set_name()` |
| `part.hybrid_bodies` | `Part::hybrid_bodies()` |
| `HybridBody.hybrid_shapes` | `HybridBody::hybrid_shapes()` |
| `part.origin_elements.PlaneXY` | `Part::origin_elements()?.plane_xy()` |
| `selection.clear()` / `.add(obj)` / `.search(q)` | `Selection::clear()` / `add()` / `search()` |
| `selection.vis_properties` | `Selection::vis_properties()` |
| `vis_properties.set_show(n)` | `VisProperties::set_show(i32)` |
| `vis_properties.set_real_color(r, g, b, 0)` | `VisProperties::set_real_color(r, g, b, 0)` |
| `ProductDocument.product` | `Document::product()` |
| `product.products` | `Product::products()` |
| `product.reference_product` | `Product::reference_product()` |
| `AnyObject.name` / `.parent` | `ComObject::name()` / `.parent()` |

Collections implement `ComCollection`: `count()`, `item(i)`, `items()`, `is_empty()`.
`Item` is dispatched as a method first and falls back to a parameterized property get,
which covers differences between CATIA versions.

## Design notes

- **One STA thread owns every COM call.** `run_on_sta` / `with_application` post work to a
  dedicated `CoInitializeEx(COINIT_APARTMENTTHREADED)` thread. A timed-out call discards the
  worker (COM cannot cancel in-flight calls); the next call creates a fresh apartment.
- **ROT attach only.** `GetActiveObject` on `CATIA.Application`; never `CoCreateInstance`.
  The latter can silently start a windowless background instance.
- **Attach per operation.** `Application` / `Document` handles are never cached, so a CATIA
  restart or a closed document cannot leave dangling pointers.
- **Multi-instance guard.** If several `CNEXT.exe` processes are running and the automation
  cannot tell which one the user is working in, it fails loudly instead of writing into the
  wrong session. Empty background sessions are quit and retried.
- **Raw `SetShow` semantics.** `VisProperties::set_show(i32)` keeps CATIA's native
  `1 = hide`, `0 = show`; it is intentionally not wrapped in a bool.

## Roadmap

See [ROADMAP.md](ROADMAP.md) for the full plan. Short version:

- **Done:** bindings foundation, session discipline, three structural tools, live verification.
- **Next:** more structural tools (materials, capture, body-to-part, surface join/offset/stitch/trim)
  and the object-model members they need.
- **Later:** geometry / measurement tools (bounding box, projected area, honeycomb ribs,
  text-to-sketch) — these need SAFEARRAY out-parameter handling.
- **Consolidation:** make Rust the default path, retire the Python sidecar, optional process
  isolation for risky tools.

## Development

```bash
cargo fmt --all
cargo test --workspace
```

CATIA-dependent paths cannot run in CI (they need a licensed, running CATIA session).
Changes to bindings should be verified manually with `probe` / `put-selftest` / dry runs
against a real document. New tools are registered in
`crates/rscatiacli-tools/src/lib.rs` and follow the "wrap only what is needed" rule.

## License

MIT. See [LICENSE](LICENSE).
