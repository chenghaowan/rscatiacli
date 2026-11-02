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

### ✅ Phase 3 — sketcher_interfaces completion (done 2026-11-03)

14/14 (the earlier `Sketch` / `Factory2D` / `Point2D` / `Line2D` plus the ten new):

- `GeometricElement` / `Geometry2D` bases (`GeometricType`, `Construction`,
  `ReportName`), `Axis2D`, `Curve2D` (param extents / end points / range box /
  curvature / derivatives / tangent / point-at-param via the VBA relay,
  `GetLengthAtParam` / `GetParamAtLength` / `IsPeriodic`), `Point2D.GetCoordinates`
  / `SetData`, `ControlPoint2D` (curvature / tangent / unset), `Circle2D`,
  `Ellipse2D`, `Hyperbola2D`, `Parabola2D`, `Spline2D` (control points, insert-after).
- `Factory2D` gained `CreateClosedCircle` / `CreateCircle` / `CreateClosedEllipse` /
  `CreateEllipse` / `CreateHyperbola` / `CreateParabola` / `CreateControlPoint` /
  `CreateLineFromVector` / `CreateIntersection(s)` / `CreateProjection(s)`, returning
  typed curves; `create_spline` now returns `Spline2D` (the one caller discarded the
  result).
- `Sketch` gained `AbsoluteAxis` / `CenterLine` (get+put) / `Factory2D` property /
  `Evaluate` / `InverseOrientation` / `Get·SetAbsoluteAxisData`.
- `SystemService` gained the generic out-param relays (`numeric_out_param`,
  `object_out_param`, `objects_out_param`) alongside `direction_out_param`.
- Curve reads need live verification on a real sketch.

### ✅ Phase 4 — in_interfaces files / cameras / windows (done 2026-11-03)

40/40 — the whole family is now typed:

- Cameras (`Cameras`/`Camera`/`Camera2D`/`Camera3D` + `Viewpoint2D`), file system
  (`FileSystem`/`FileComponent`/`File`/`Files`/`Folder`/`Folders`/`TextStream` —
  create/copy/delete/exists plus text read/write), positioning (`Move`/`Position`),
  `References`, printing (`PageSetup` incl. ~22 print properties,
  `DraftingPageSetup`, `Printer(s)`), `SelectionSets` / `SendToService` /
  `SystemConfiguration`, viewer base family (`Viewer`/`Viewer2D`/`Viewers`/
  `SpecsViewer`/`SpecsAndGeomWindow`/`Windows` + `LightSource(s)`).
- `Window` / `Viewer3D` / `Viewpoint3D` completed to the pycatia surface
  (geometry/lights/navigation on Viewer3D, print/caption/geometry on Window,
  field-of-view/focus/sight reads via the VBA relay on Viewpoint3D).
- `Application` gained `file_system` / `printers` / `system_configuration` /
  `windows`; `Document` gained `cameras`.
- **Live read-only verification passed** (file separators, temp directory,
  system configuration, window/printer/camera counts), which corrected two
  COM facts vs pycatia's annotations: `TemporaryDirectory` returns a Folder
  object (not a string) and `Release`/`Version` return integers (VT_I4).
- Out-param list getters on `SelectionSets` / `SendToService` /
  `SystemConfiguration` stay on the `com()` escape hatch.

### ✅ Phase 5 — knowledge_interfaces (done 2026-11-03; 0.1.0 coverage gate met)

35/35 — `Relation` base (comment/context/in-out parameters/`Modify`/`Rename`) with
`Formula` / `Rule` / `Check` / `DesignTable` / `SetOfEquation` /
`ConstraintSatisfaction` / `Law` / `Loop` (`UseSymbolcTransformations` spelled as
CAA/COM has it); typed parameters `RealParam` / `IntParam` / `StrParam` /
`BoolParam` / `EnumParam` / `Length` / `Angle` / `Dimension` / `ListParameter`
(values, ranges, tolerances, enumerate values — string arrays via the new
`strings_out_param` relay); `List` / `FreeParameters` / `ParameterSet(s)` /
`Units` (MKS conversions); `Optimization(s)` / `OptimizationConstraint(s)` /
`FeatureGenerator`. `Parameters` gained `CreateReal` / `CreateInteger` /
`CreateString` / `CreateBoolean` / `CreateList` and friends; `Parameter` gained
comment/read-only/rename/valuate-from-string; `Part` and `Product` gained
`relations` / `parameters`.

Live read-only verification passed (product-level `Parameters` /
`Relations` / `RootParameterSet`). Live note: `ValueAsString` is member-missing
on product identity parameters (`Product1\Part Number`) — use the dedicated
accessors instead.

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
  by typed bindings, plus stable migrated tools. **Both conditions are now met**
  (241/446 ≈ 54.0% after phases 1–5; the 28 JSON tools + colorize have been stable).
  Remaining phases P6–P8 continue to grow coverage but are not gate-blocking.
  Cut the release when the maintainer decides.
- **1.0.0:** not planned while pycatia itself is pre-1.0 and this project intentionally
  wraps only what the application needs.

## ⛔ Non-goals

- 1:1 parity with all 84 pycatia module families or full CATIA type-library coverage
  (CAM / machining / DMU / systems-integration families stay out; see the out-of-scope
  list on the progress page).
- Cross-platform support (CATIA automation is Windows-only).
- Launching CATIA automatically.
- A GUI — this is a library and a CLI; the application layer owns the UI.
