# Roadmap

This roadmap follows the project's extraction rule:

> **Wrap only what the application actually needs.** Bindings grow with the application
> layer's migration; there is no plan to mirror pycatia's 84 module families 1:1.

Live coverage numbers: [docs/index.html](docs/index.html) (regenerate with
`python scripts/parity_audit.py --write docs/index.html`).

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
- Application integration: path dependency, thin adapter, `LTL_CATIA_RUST` A/B switch.

## ✅ Done — application batches, tools fully migrated

- Materials (`CATMatManagerVBExt` via `SystemService.Evaluate`, SAFEARRAY in/out),
  surface four-pack (`join_surface` / `offset_surface` / `stitch_surface` /
  `trim_boundary`), product analysis (`apply_naming`, `find_near_parts`,
  `save_product_without_history`, `dump_solid`).
- Measurement chain: `vba_bbox_generate`, `rule_projected_surface` (+ rule-axis core),
  `rule_bounding_box`, `batch_bbox` / `product_measure` wrappers, `batch_product_analysis`.
- Geometry-heavy batches: text/QR to sketch (GDI glyph outlines + QR union),
  `honeycomb_rib`, `measure_harness` (FTA annotation + screenshot markup), `wcc`
  (inertia mass properties + recursive tree + CSV/text reports), IPP external-reference
  isolation/batch, drafting three-pack (`drawlink` / `grid` / `expose2d`),
  `capture_active_part` (three modes) — the last Python tool.
- As of commit `a66d566` every tool lives in Rust: **28 JSON tools** (registry keeps the
  original Python script names) + the `colorize` CLI. The Python sidecar is retired;
  `LTL_CATIA_RUST` stays until the Rust path becomes the default.
- Live verification on a real CATIA V5 session for the major tools.

## 🔜 Next — pycatia → Rust translation phases

The tool migration is complete; the remaining Python→Rust work is the **pycatia API
surface itself**. Progress is tracked per class against a defined denominator:

> **Denominator:** classes in the 14 in-scope module families, excluding
> `*SettingAtt` settings panels and COM infrastructure classes
> (`IDispatch`/`IUnknown`/`AnyObject`/`Collection`/…). The `enumeration` family
> (339 CAT enums) is tracked separately and imported on demand.
> Current: **141 / 446 functional classes ≈ 32%**.

### ✅ Phase 1 — part_interfaces + product_structure_interfaces (done 2026-11-03)

The user-set starting point; both families are at 100% class coverage:

- `product_structure_interfaces` 7/7: added `Analyze` (product-level mass properties;
  `GetGravityCenter` / `GetInertia` out-arrays via the `SystemService.Evaluate` VBA
  relay, same path as `Inertia`) and `AssemblyConvertor`
  (`Product.GetItem("BillOfMaterial")`, `Print` via the VBA relay, `SetCurrentFormat` /
  `SetSecondaryFormat` SAFEARRAY columns).
- `part_interfaces` 68/68: typed wrappers for every class across the five waves
  (sketch-based `Prism`/`Pad`/`Pocket`/`Revolution`/`Shaft`/`Groove`/`Sweep`/`Rib`/
  `Slot`/`Hole`/`Stiffener`/`SolidCombine`/`Loft`; dress-up `Fillet` family/`Chamfer`/
  `Shell`/`Thickness`/`Thread`/`Draft` family/`AutoFillet`/`AutoDraft`/Defeaturing;
  booleans `Add`/`Remove`/`Intersect`/`Assemble`/`Trim`; face-based `Split`/
  `ThickSurface`/`SewSurface`/`RemoveFace`/`ReplaceFace`/`CloseSurface`; transforms
  `Mirror`/`Rotate`/`Translate`/`Scaling`/`Scaling2`/`Affinity`/`Symmetry`/`AxisToAxis`;
  patterns `RectPattern`/`CircPattern`/`UserPattern` + `Repartition` family + `Limit`).
- `ShapeFactory` grew ~45 `add_new_*` creation methods returning the typed features
  (`add_new_pad` now returns `Pad`, `add_new_split` returns `Split`).
- Read-only live verification on a real session: `Product.Analyze` properties +
  VBA-relay gravity center / inertia, `GetItem("BillOfMaterial")`. Feature-creation
  write paths still need a real part-document run.
- Members pycatia does not expose stay reachable through the `com()` escape hatch.

### ✅ Phase 2 — mec_mod_interfaces completion (done 2026-11-03)

All 27 remaining classes wrapped (41→42/42 incl. the earlier work):

- Topology markers `Boundary` / `Edge` / `Face` / `Vertex` (+ `BiDimFeatEdge`,
  `MonoDimFeatEdge`, `TriDimFeatEdge`, `NotWireBoundaryMonoDimFeatVertex`,
  `TriDimFeatVertexOrBiDimFeatVertex`, `ZeroDimFeatVertexOrWireBoundaryMonoDimFeatVertex`),
  measurable topology `CylindricalFace`, `PlanarFace`, `Rectilinear{Bi,Mono,Tri}DimFeatEdge`
  (direction/origin out-arrays via the VBA relay).
- `Shape`, `ShapeInstance`, `HybridShape`, `HybridShapeInstance` (input/output/parameter
  put-get surface), `Solid`, `Factory`, `GeometricElements`, `OrderedGeometricalSet(s)`,
  `FixTogether(s)`, `PartServices`.
- `Part` gained `analyze` / `density` (get+put) / `geometric_elements` /
  `ordered_geometrical_sets` / `in_work_object`; `Body` gained
  `ordered_geometrical_sets`; `HybridBody` gained `geometric_elements`; shape
  collections gained `GetBoundary` (B-Rep label → `Boundary`).
- `direction_via_vba` moved into `SystemService::direction_out_param` (shared by
  part_interfaces and mec_mod).
- Topology reads still need live verification on a real part document.

### Phase 3 — sketcher_interfaces completion

`Circle2D`, `Ellipse2D`, `Spline2D`, `Curve2D`, `Geometry2D`, `Axis2D`,
`ControlPoint2D`, `Hyperbola2D`, `Parabola2D` — supports the text-to-sketch and
honeycomb Factory2D batch scenarios.

### Phase 4 — in_interfaces files / cameras / windows

`References`, `Cameras`, `FileSystem` / `Files` / `Folders` / `TextStream`,
`Windows`, `Move` / `Position`. `system_interfaces` is effectively done
(`SystemService` wrapped; the rest is infrastructure, not planned).

### Phase 5 — knowledge_interfaces

Typed parameters (`IntParam` / `RealParam` / `StrParam` / `BoolParam` / `Length` /
`Angle` / `Dimension`) and `Relations` / `Formula` / `Rule` / `SetOfEquation` /
`ParameterSets` / `Law` / `List` / `Units`. `DesignTable` later if needed.

### Phase 6 — hybrid_shape_interfaces, on demand

GSD's 111 classes are the largest chunk of the denominator. Prioritize the
constructor families the toolchain already exercises (point/line/plane constructors,
`Project` / `Intersection` / `Extremum`, `Sweep` / `Loft` / `Blend` / `Rotate` /
`Scaling` / `Symmetry`); the rest grows with the application.

### Phase 7 — drafting_interfaces remainder

Dimensions, tables, leaders, GDT, pictures, view frames (~30 classes).

### Phase 8 — space_analyses / navigator wrap-up

`Clashes` / `Sections` / `Inertias`; navigator `Marker2D/3D`, `Hyperlink`.
Semantic GDT in `cat_tps_interfaces` follows on demand.

## 💭 Consolidation — flip the default

- Make the Rust path the default and remove the `LTL_CATIA_RUST` switch.
- Keep a per-tool parity matrix (Python vs Rust: inputs, result JSON, error text).
- Remove the Python runtime from the installer.
- Optional out-of-process runner (`rscatiacli-tools` as a sidecar) for high-risk tools
  that may hang inside CATIA, so a stuck COM call cannot take down the UI process.

## 💭 API hardening

- Error taxonomy: distinguish "member missing", "CATIA busy", "not connected",
  "wrong document type" without string matching.
- `ComCollection::item_by_name` for named collections.
- More examples and a manual CATIA test checklist.

## Versioning

- **0.0.x (current):** bindings and tools are still moving; breaking changes are allowed.
- **0.1.0:** requires at least **50% of the in-scope functional class surface** covered
  by typed bindings (currently 168/446; the gate is ≈223 classes), plus stable migrated
  tools. Phases 1–2 cleared ≈ +96 classes; remaining estimates: P3 ≈ +10, P4 ≈ +15,
  P5 ≈ +20, P6 ≈ +30, P7 ≈ +25, P8 ≈ +8 — roughly 240–250 cumulative, clearing the gate.
- **1.0.0:** not planned while pycatia itself is pre-1.0 and this project intentionally
  wraps only what the application needs.

## ⛔ Non-goals

- 1:1 parity with all 84 pycatia module families or full CATIA type-library coverage
  (CAM / machining / DMU / systems-integration families stay out; see the out-of-scope
  list on the progress page).
- Cross-platform support (CATIA automation is Windows-only).
- Launching CATIA automatically.
- A GUI — this is a library and a CLI; the application layer owns the UI.
