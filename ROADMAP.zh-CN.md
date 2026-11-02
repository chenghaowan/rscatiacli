# Roadmap（路线图）

本路线图遵守项目的抽取规则：

> **只封装应用真正用到的成员。** 绑定库跟随应用层的迁移进度增长，不做 pycatia
> 84 个模块族的 1:1 全量移植。

实时覆盖数字见 [docs/index.html](docs/index.html)（可用
`python scripts/parity_audit.py --write docs/index.html` 重新生成）。

状态：✅ 已完成 · 🔜 下一步 · 💭 以后 / 探索中 · ⛔ 非目标

## ✅ 已完成 —— v0.0.1 基础

- IDispatch 晚绑定：`get` / `call` / `get_with` / `put`、VARIANT 转换、`EXCEPINFO`
  错误文案、所有路径上的 `VariantClear`。
- 会话纪律：专用 STA 线程、ROT attach、多实例守卫、后台空会话恢复、每次操作重新 attach。
- Typed 对象模型：`Application`、`Documents`、`Document`、`Part`、`Bodies` / `Body`、
  `HybridBodies` / `HybridBody`、`AxisSystems`、`OriginElements`、`Product` / `Products`、
  `Publications` / `Publication`、`Reference`、`HybridShapeFactory`、`SPAWorkbench` /
  `Measurable`、`Selection` / `SelectionItem` / `VisProperties`、通用 `ComCollection`。
- 诊断 CLI：`status`、`probe`、`put-selftest`、`ps`。
- 应用集成：path 依赖、薄适配层、`LTL_CATIA_RUST` A/B 开关。

## ✅ 已完成 —— 应用批次，工具全量迁移

- 材料赋值（`CATMatManagerVBExt` 经 `SystemService.Evaluate`，SAFEARRAY 进出）、
  曲面四件套（`join_surface` / `offset_surface` / `stitch_surface` / `trim_boundary`）、
  产品分析（`apply_naming`、`find_near_parts`、`save_product_without_history`、
  `dump_solid`）。
- 量测链路：`vba_bbox_generate`、`rule_projected_surface`（含 rule-axis 建系核心）、
  `rule_bounding_box`、`batch_bbox` / `product_measure` 薄壳、`batch_product_analysis`。
- 重几何批次：文字/二维码转草图（GDI 字形轮廓 + QR 并集）、`honeycomb_rib`、
  `measure_harness`（FTA 注解 + 截图标注）、`wcc`（Inertia 质量属性 + 递归树 + CSV/文本报表）、
  IPP 外部参考隔离/批处理、图纸三件套（`drawlink` / `grid` / `expose2d`）、
  `capture_active_part`（三模式）—— 最后一个 Python 工具。
- 截至 `a66d566`，所有工具均为 Rust 实现：**28 个 JSON 工具**（注册表保留原 Python
  脚本名）+ `colorize` 独立 CLI。Python sidecar 已退役；`LTL_CATIA_RUST` 保留到
  Rust 路径转为默认为止。
- 主要工具均已在真实 CATIA V5 会话完成验证。

## 🔜 下一步 —— pycatia → Rust 翻译阶段

工具迁移已完成；剩余的 Python→Rust 工作是 **pycatia API 面本身**。以类为粒度、
按明确的分母跟踪进度：

> **分母：** 14 个规划内模块族的类，剔除 `*SettingAtt` 设置面板类与 COM 基础设施类
> （`IDispatch` / `IUnknown` / `AnyObject` / `Collection` 等）；`enumeration` 族
> （339 个 CAT 枚举）单列、按需引入。
> 当前：**141 / 446 个功能类 ≈ 32%**。

### ✅ 阶段 1 —— part_interfaces + product_structure_interfaces（已完成，2026-11-03）

用户指定的起点，两族类覆盖均已 100%：

- `product_structure_interfaces` 7/7：补 `Analyze`（产品级质量属性；
  `GetGravityCenter` / `GetInertia` 出参数组走 `SystemService.Evaluate` VBA 中转，
  与 `Inertia` 同路径）与 `AssemblyConvertor`
  （`Product.GetItem("BillOfMaterial")`；`Print` 走 VBA 中转；
  `SetCurrentFormat` / `SetSecondaryFormat` SAFEARRAY 列配置）。
- `part_interfaces` 68/68：五波全部有类型化包装（基于草图 `Prism`/`Pad`/`Pocket`/
  `Revolution`/`Shaft`/`Groove`/`Sweep`/`Rib`/`Slot`/`Hole`/`Stiffener`/`SolidCombine`/
  `Loft`；修饰 `Fillet` 族/`Chamfer`/`Shell`/`Thickness`/`Thread`/`Draft` 族/
  `AutoFillet`/`AutoDraft`/Defeaturing 族；布尔 `Add`/`Remove`/`Intersect`/`Assemble`/
  `Trim`；面基于 `Split`/`ThickSurface`/`SewSurface`/`RemoveFace`/`ReplaceFace`/
  `CloseSurface`；变换 `Mirror`/`Rotate`/`Translate`/`Scaling`/`Scaling2`/`Affinity`/
  `Symmetry`/`AxisToAxis`；阵列 `RectPattern`/`CircPattern`/`UserPattern` +
  `Repartition` 族 + `Limit`）。
- `ShapeFactory` 新增约 45 个 `add_new_*` 创建方法、全部返回类型化特征
  （`add_new_pad` 现在返回 `Pad`，`add_new_split` 返回 `Split`）。
- 真机只读验证已过：`Product.Analyze` 三属性 + VBA 中转重心/惯量、
  `GetItem("BillOfMaterial")`。特征创建/参数写入路径待真机建特征验证。
- pycatia 未暴露的成员走 `com()` 逃生舱按名派发。

### ✅ 阶段 2 —— mec_mod_interfaces 补全（已完成，2026-11-03）

剩余 27 类全部有类型化包装（该族 42/42）：

- 拓扑标记族 `Boundary` / `Edge` / `Face` / `Vertex`（+ `BiDimFeatEdge`、
  `MonoDimFeatEdge`、`TriDimFeatEdge`、`NotWireBoundaryMonoDimFeatVertex`、
  `TriDimFeatVertexOrBiDimFeatVertex`、`ZeroDimFeatVertexOrWireBoundaryMonoDimFeatVertex`），
  可量测拓扑 `CylindricalFace`、`PlanarFace`、`Rectilinear{Bi,Mono,Tri}DimFeatEdge`
  （方向/原点出参数组走 VBA 中转）。
- `Shape`、`ShapeInstance`、`HybridShape`、`HybridShapeInstance`（输入/输出/参数
  读写面）、`Solid`、`Factory`、`GeometricElements`、`OrderedGeometricalSet(s)`、
  `FixTogether(s)`、`PartServices`。
- `Part` 补 `analyze` / `density`（读+写）/ `geometric_elements` /
  `ordered_geometrical_sets` / `in_work_object`；`Body` 补 `ordered_geometrical_sets`；
  `HybridBody` 补 `geometric_elements`；形状集合补 `GetBoundary`
  （B-Rep 标签 → `Boundary`）。
- `direction_via_vba` 上移为 `SystemService::direction_out_param`
  （part_interfaces 与 mec_mod 共用）。
- 拓扑读数待真机零件文档验证。

### 阶段 3 —— sketcher_interfaces 补全

`Circle2D`、`Ellipse2D`、`Spline2D`、`Curve2D`、`Geometry2D`、`Axis2D`、
`ControlPoint2D`、`Hyperbola2D`、`Parabola2D` —— 支撑文字转草图与蜂窝肋的
Factory2D 批量场景。

### 阶段 4 —— in_interfaces 文件 / 相机 / 窗口

`References`、`Cameras`、`FileSystem` / `Files` / `Folders` / `TextStream`、
`Windows`、`Move` / `Position`。`system_interfaces` 实际已就绪
（`SystemService` 已封装；其余为基础设施，不计划）。

### 阶段 5 —— knowledge_interfaces

类型化参数（`IntParam` / `RealParam` / `StrParam` / `BoolParam` / `Length` /
`Angle` / `Dimension`）与 `Relations` / `Formula` / `Rule` / `SetOfEquation` /
`ParameterSets` / `Law` / `List` / `Units`；`DesignTable` 视需要后置。

### 阶段 6 —— hybrid_shape_interfaces 按需扩展

GSD 的 111 类是分母大头。优先补工具链已用到的构造器族（点/线/平面构造、
`Project` / `Intersection` / `Extremum`、`Sweep` / `Loft` / `Blend` / `Rotate` /
`Scaling` / `Symmetry`）；其余随应用需要生长。

### 阶段 7 —— drafting_interfaces 剩余

尺寸标注、表格、引出线、GDT、图片、图框组件（约 30 类）。

### 阶段 8 —— space_analyses / navigator 收尾

`Clashes` / `Sections` / `Inertias`；导航器 `Marker2D/3D`、`Hyperlink`。
`cat_tps_interfaces` 的语义 GDT 按需跟进。

## 💭 收尾 —— Rust 路径转默认

- Rust 路径转为默认，移除 `LTL_CATIA_RUST` 开关。
- 维护逐工具的 parity 对照表（Python vs Rust：输入、结果 JSON、错误文案）。
- 从安装包里移除 Python 运行时。
- 高风险工具可选独立进程运行（`rscatiacli-tools` 作为 sidecar），避免卡死的 COM 调用
  拖垮 UI 进程。

## 💭 API 加固

- 错误分类：不用字符串匹配就能区分"成员不存在""CATIA 忙""未连接""文档类型不对"。
- `ComCollection::item_by_name`。
- 更多示例与手工 CATIA 测试清单。

## 版本策略

- **0.0.x（当前）：** 绑定和工具仍在演进，允许破坏性变更。
- **0.1.0：** 需要规划范围功能类覆盖至少 **50%**（当前 168/446；门槛约 223 类），
  且已迁移工具稳定。阶段 1–2 已过 ≈ +96 类；剩余预估：P3 ≈ +10，P4 ≈ +15，
  P5 ≈ +20，P6 ≈ +30，P7 ≈ +25，P8 ≈ +8 —— 累计约 240–250 类，可过门槛。
- **1.0.0：** pycatia 自己都还没到 1.0，本项目又只按需封装，暂不考虑。

## ⛔ 非目标

- 与 pycatia 全部 84 个模块族 1:1 对齐或 CATIA 类型库全覆盖
  （CAM / 加工 / DMU / 系统集成等族保持范围外，见进度页的清单）。
- 跨平台（CATIA 自动化只在 Windows）。
- 自动拉起 CATIA。
- GUI —— 本项目只提供库和 CLI，界面由应用层负责。
