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

- `Application`, `Documents`, `Document`, `PartDocument`, `ProductDocument`
- `Part`, `Bodies` / `Body`, `HybridBodies` / `HybridBody`, `AxisSystems`, `OriginElements`
- `Selection`, `SelectionItem`, `VisProperties`
- `Product`, `Products`, `Publications` / `Publication`
- `Reference`, `HybridShapeFactory` (datum creation / cleanup), `SPAWorkbench` / `Measurable`
- `ComCollection` (`count` / `item` / `items`) with method → property-get fallback
- Session diagnostics: `status`, `probe`, `put-selftest`, `ps`

Tools implemented in `rscatiacli-tools`:

| Tool | Script key | What it does |
| --- | --- | --- |
| colorize | `colorize-bodies` | Assigns a distinct palette color to every body of the active part. |
| body rename | `body_rename/rename_bodies.py` | Cleans body / geometrical-set names: keeps the last `keep_last` `\`-separated segments, replaces illegal characters, de-duplicates with `_2`, `_3`, ... |
| hide-show | `hide_show/hide_show.py` | Shows/hides bodies, geometrical sets, axis systems, reference planes and constraints; recurses into the part documents of a product. |
| link-fixer | `link_fixer/catia_link_fixer_bridge.py` | Broken-link check & repair for CATProducts (`status` / `scan` / `fix`): relink to open documents via `ReplaceProduct` first, then add directories to FileSearchOrder, finally save-and-reopen everything as a fallback. |
| body-to-part | `body_to_part/catia_body_to_part_bridge.py` | Splits every body of a CATPart into its own CATPart (publish → copy → paste → datum-ize related geometry → SaveAs), assembles a Product and writes an Excel report. |
| add-material | `add_material/add_material.py` | Applies a material from a catalog to bodies / the main body (`CATMatManagerVBExt` via `SystemService.Evaluate`; SAFEARRAY in/out). |
| join-surface | `join_surface/join_surface.py` | Groups faces of the densest geometrical set by minimum distance (SPA), joins each group (`AddNewJoin`, Connex=1) and datum-izes the result (`AddNewSurfaceDatum` + `DeleteObjectForDatum`). Not wired into the ltl-manage UI; CLI contract only. |
| offset-surfaces | `offset_surface/offset_surface.py` | Batch-offsets every offsettable surface (explicit / offset / extract) of the geometrical set that holds the `target_1` marker, in the marker's direction; failed faces are copied to a NON_OFFSET compensation set. |
| stitch-surface | `stitch_surface/stitch_surface.py` | Joins every geometrical set (`Connex=0`) and counts boundary rings (`AddNewBoundaryOfSurface` + `AddNewDatums`); 2 rings = internal gap → retry down the tolerance ladder 0.001/0.01/0.1, keep at the last rung. `action=isolate/unisolate` shows only the `Stitch_` results. |
| trim-boundary | `trim_boundary/trim_boundary.py` | Splits solids (`ShapeFactory.AddNewSplit`) and surfaces (`AddNewHybridSplit`) with every plane named `boundary*`, keeping the side between the plane and the tooling-axis origin. |
| apply-naming | `product_analysis/apply_naming.py` | Walks a directory (or an explicit item list), parses part number/name from each filename (position/delimiter rules) and writes `Product.PartNumber/PartName` + save. |
| find-near-parts | `product_analysis/find_near_parts.py` | Leaf instances of the active product; minimum distance to a seed part via SPA `Distance` (temp groups, BetweenTwo + Minimum, removed after each read). `action=list` lists leaves. |
| save-without-history | `product_analysis/save_product_without_history.py` | FPRM010: converts each first-level CATPart of a product into a history-free part (visible non-empty bodies + wireframe pasted `CATPrtResultWithOutLink`), saves them under `<out>/<part>/<part>.CATPart` and reassembles a new product. |
| dump-solid | `dump_solid/dump_solid.py` | Same body-dump conversion for the active part only (`<src dir>/Result/<part>.CATPart`), optional single `body_name`, wireframe optional. |
| vba-bbox-generate | `product_analysis/vba_bbox_generate.py` | Generates the full bounding-box geometry (extremum points → constrained sketches → sweep) along the shared `tooling axis`, reads exact L/W/H, real volume, material/density/weight, writes `Bbox_dx/dy/dz` parameters and the `x…y…z…` description. |


All tools support `dry_run` where applicable. link_fixer, body_to_part, add_material
and the surface tools passed live verification on a real CATIA session
(`status` / `scan` / dry-run / contract runs); write-path runs follow the
per-batch verification notes in ltl-manage's gitrecord.

## Versioning

The current line is **0.0.x** on purpose:

- **0.0.x** — bindings and tools are still moving; breaking changes are allowed.
- **0.1.0** — only after at least **50% of pycatia's functional surface** is covered by
  typed bindings and the migrated tools are stable.
- **1.0.0** — not planned while pycatia itself is pre-1.0 and this project intentionally
  wraps only what the application needs.

This is not a promise of pycatia parity; it is a guard against overstating maturity.

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
cargo run -p rscatiacli-tools -- json link_fixer/catia_link_fixer_bridge.py '{"action":"scan","dry_run":true}'
cargo run -p rscatiacli-tools -- json body_to_part/catia_body_to_part_bridge.py '{"action":"run","dry_run":true}'
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
| `PartDocument.part` | `PartDocument::part()` |
| `part.bodies` / `.item(i)` | `Part::bodies()` / `Bodies::item(i)` |
| `Body.name` | `Body::name()` / `Body::set_name()` |
| `part.hybrid_bodies` | `Part::hybrid_bodies()` |
| `HybridBody.hybrid_shapes` | `HybridBody::hybrid_shapes()` |
| `part.origin_elements.PlaneXY` | `Part::origin_elements()?.plane_xy()` |
| `selection.clear()` / `.add(obj)` / `.search(q)` | `Selection::clear()` / `add()` / `search()` |
| `selection.copy()` / `.paste()` / `.paste_special(f)` / `.delete()` | `Selection::copy()` / `paste()` / `paste_special()` / `delete()` |
| `selection.vis_properties` | `Selection::vis_properties()` |
| `vis_properties.set_show(n)` | `VisProperties::set_show(i32)` |
| `vis_properties.set_real_color(r, g, b, 0)` | `VisProperties::set_real_color(r, g, b, 0)` |
| `document.save()` / `.close()` / `.save_as(p)` | `Document::save()` / `close()` / `save_as()` |
| `documents.add("Part")` / `.open(p)` | `Documents::add()` / `open()` |
| `PartDocument.spa_workbench().get_measurable(ref).volume` | `PartDocument::spa_workbench()?.get_measurable(&r)?.volume()` |
| `part.create_reference_from_object(obj)` | `Part::create_reference_from_object()` |
| `part.hybrid_shape_factory` | `Part::hybrid_shape_factory()` |
| `hybrid_shape_factory.AddNewLineDatum(ref)` | `HybridShapeFactory::add_new_line_datum()` (also Point/Circle/Curve/Surface/Volume) |
| `hybrid_bodies.add()` | `HybridBodies::add()` |
| `hybrid_body.AppendHybridShape(datum)` | `HybridBody::append_hybrid_shape()` |
| `product.part_number` | `Product::part_number()` / `set_part_number()` |
| `product.publications.Add(name)` / `.SetDirect(name, ref)` | `Publications::add()` / `set_direct()` |
| `products.add_external_component(doc)` | `Products::add_external_component()` |
| `products.replace_product(comp, ref, False)` | `Products::replace_product()` |
| `application.FileSearchOrder` | `Application::file_search_order()` / `set_file_search_order()` |
| `application.DisplayFileAlerts = False` | `Application::set_display_file_alerts(false)` |
| `ProductDocument.product` | `ProductDocument::product()` |
| `product.products` | `Product::products()` |
| `product.reference_product` | `Product::reference_product()` |
| `AnyObject.name` / `.parent` | `ComObject::name()` / `.parent()` |

Collections implement `ComCollection`: `count()`, `item(i)`, `items()`, `is_empty()`.
`Item` is dispatched as a method first and falls back to a parameterized property get,
which covers differences between CATIA versions.

Typed documents keep the pycatia class names. Construct them from a generic `Document`
the same way pycatia constructs `PartDocument(doc.com_object)`:

```rust
let doc: Document = app.active_document()?;
let part_doc: PartDocument = doc.part_document(); // or PartDocument::from(doc)
let part = part_doc.part()?;
```

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

## Naming and licensing

Type names (`Application`, `Documents`, `Document`, `PartDocument`, `ProductDocument`,
`Part`, `Body`, `Selection`, ...) follow CATIA's COM automation object model and pycatia's
public API. They are kept identical on purpose so code and documentation stay portable
between the Python and Rust worlds.

- No pycatia source code is copied into this project. The COM layer is an independent
  implementation built on `IDispatch`.
- pycatia itself is MIT-licensed.
- CATIA is a trademark of Dassault Systèmes. This project is not affiliated with or
  endorsed by Dassault Systèmes or the pycatia project.

## Roadmap

See [ROADMAP.md](ROADMAP.md) for the full plan. Short version:

- **Done:** bindings foundation, session discipline, three structural tools, live verification.
- **Done:** bindings foundation, session discipline, structural tools, materials,
  surface tools (join / offset / stitch / trim), SAFEARRAY in- and out-parameters
  (`as_f64_array` / `as_obj_array`), COM type-name reads (`GetTypeInfo`).
- **Next:** rule_projected_surface (908) then the batch_bbox / product_measure
  wrappers on top of vba_bbox_generate; measure harness (screenshot + FTA
  annotation, partially non-COM); text-to-sketch.
- **Later:** honeycomb ribs (UDF-based, not automatable — generate directly instead),
  capture/screenshot tooling.
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
