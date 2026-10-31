# Roadmap

This roadmap follows the project's extraction rule:

> **Wrap only what the application actually needs.** Bindings grow with the application
> layer's migration; there is no plan to mirror pycatia 1:1.

Status legend: ✅ done · 🔜 next · 💭 later / exploring · ⛔ non-goal

## ✅ Done — v0.0.1 foundation

- IDispatch late binding: `get` / `call` / `get_with` / `put`, VARIANT conversion,
  `EXCEPINFO` messages, `VariantClear` on every path.
- Session discipline: dedicated STA thread, ROT attach, multi-instance guard,
  empty-background-session recovery, per-operation attach.
- Typed object model: `Application`, `Documents`, `Document`, `Part`, `Bodies` / `Body`,
  `HybridBodies` / `HybridBody`, `AxisSystems`, `OriginElements`, `Product` / `Products`,
  `Publications` / `Publication`, `Reference`, `HybridShapeFactory`, `SPAWorkbench` /
  `Measurable`, `Selection` / `SelectionItem` / `VisProperties`, generic `ComCollection`.
- Diagnostics CLI: `status`, `probe`, `put-selftest`, `ps`.
- Tools: `colorize`, `body_rename`, `hide_show`, `link_fixer` (status/scan/fix),
  `body_to_part` (incl. Excel report), plus the JSON tool registry with per-script timeouts.
- Application integration: path dependency, thin adapter, `LTL_CATIA_RUST` A/B switch.
- Live verification on a real CATIA V5 session (probe, property write, dry runs,
  byte-identical output between the app and the standalone tools CLI; link_fixer and
  body_to_part passed read-only live verification, write paths pending a real run).

## ✅ Done — application batches (r123–r128)

- Materials: `add_material` (`CATMatManagerVBExt` via `SystemService.Evaluate`,
  SAFEARRAY in/out).
- Surface tools: `join_surface`, `offset_surface`, `stitch_surface`, `trim_boundary`
  (Evaluate out-parameter arrays, COM type-name reads, tolerance ladders).
- Product analysis: `apply_naming`, `find_near_parts`, `save_product_without_history`,
  `dump_solid`.
- Measurement: `vba_bbox_generate` (constrained sketches + sweep + `Bbox_*` parameters),
  `rule_projected_surface` with the `rule_axis` establishment core (extremum → plane
  → SPA min-distance spans, angle scan), `InstanceFactory` template instantiation,
  `Measurable.Area`, and the `batch_bbox` / `product_measure` thin wrappers.

## 🔜 Next — structural tools

Priority is driven by the application layer's migration order. Candidates:

1. **Measure harness** (`measure_harness`) — screenshot + FTA/annotation, partially
   non-COM (Win32 capture), a separate batch.
2. **Rule bounding box standalone** (`rule_bounding_box`) — the axis core already
   landed in `rule_axis`; remaining: JSON export + screenshot post-processing.
3. **Text/QR to sketch** (`text_to_sketch`, `text_outline`, `qr_outline`) —
   large Factory2D batches.
4. **Drawing auto-switch** — when the active document is a drawing, resolve the same-name
   part / product document (still Python-only today; the product-measure wrapper carries
   a compact same-name lookup already).

Each item adds only the object-model members it needs. Shared collection and session
helpers already cover most traversal code.

## 💭 Deferred / covered elsewhere

- **Active part capture** (`capture_active_part`) — screenshot capability is covered by
  the application's pointtofixcraft plugin, so this sidecar is not being migrated for now;
  if it ever is, it needs `Window` / `Viewer3D` / `Viewpoint3D` wrappers plus an
  image-encoding pipeline.

## 💭 Later — geometry and measurement

These tools are deliberately last because they depend on SAFEARRAY out-parameters,
large result sets, and geometric math rather than simple tree traversal:

- bounding box / moulding box / packaging box
- projected surface area
- honeycomb ribs
- text-to-sketch
- measure harness
- solid dump

Prerequisites:

- a robust VARIANT array layer (read/write, nesting, ownership);
- typed wrappers for `Reference`, `HybridShape`, `HybridShapeFactory`, `Measurable`,
  `SPAWorkbench`, `Distances`;
- a decision on where numeric post-processing lives (bindings vs tools).

## 💭 Consolidation — retire the Python sidecar

- Make the Rust path the default and remove the `LTL_CATIA_RUST` switch.
- Keep a per-tool parity matrix (Python vs Rust: inputs, result JSON, error text).
- Port the remaining tools, then remove the Python runtime from the installer.
- Optional out-of-process runner (`rscatiacli-tools` as a sidecar) for high-risk tools
  that may hang inside CATIA, so a stuck COM call cannot take down the UI process.
- Publish a versioned API once the tool surface stops moving (see Versioning).

## 💭 API hardening

- Error taxonomy: distinguish "member missing", "CATIA busy", "not connected",
  "wrong document type" without string matching.
- `ComCollection::item_by_name` for named collections.
- Parameters / Knowledge, AxisSystem, Drawing interfaces.
- More examples and a manual CATIA test checklist.

## Versioning

- **0.0.x (current):** bindings and tools are still moving; breaking changes are allowed.
- **0.1.0:** requires at least **50% of pycatia's functional surface** covered by typed
  bindings, plus stable migrated tools.
- **1.0.0:** not planned while pycatia itself is pre-1.0 and this project intentionally
  wraps only what the application needs.

## ⛔ Non-goals

- 1:1 pycatia parity or full CATIA type-library coverage.
- Cross-platform support (CATIA automation is Windows-only).
- Launching CATIA automatically.
- A GUI — this is a library and a CLI; the application layer owns the UI.
