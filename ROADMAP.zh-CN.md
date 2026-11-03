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

### ✅ 阶段 3 —— sketcher_interfaces 补全（已完成，2026-11-03）

该族 14/14（此前已有 `Sketch` / `Factory2D` / `Point2D` / `Line2D`，新增十类）：

- `GeometricElement` / `Geometry2D` 基类（`GeometricType`、`Construction`、
  `ReportName`）、`Axis2D`、`Curve2D`（参数区间/端点/包围盒/曲率/导数/切向/
  按参数取点，出参走 VBA 中转；`GetLengthAtParam` / `GetParamAtLength` /
  `IsPeriodic`）、`Point2D.GetCoordinates` / `SetData`、`ControlPoint2D`
  （曲率/切向/unset）、`Circle2D`、`Ellipse2D`、`Hyperbola2D`、`Parabola2D`、
  `Spline2D`（控制点/插点）。
- `Factory2D` 补 `CreateClosedCircle` / `CreateCircle` / `CreateClosedEllipse` /
  `CreateEllipse` / `CreateHyperbola` / `CreateParabola` / `CreateControlPoint` /
  `CreateLineFromVector` / `CreateIntersection(s)` / `CreateProjection(s)`，
  返回类型化曲线；`create_spline` 返回值 Entity→Spline2D（唯一调用点本就丢弃返回值）。
- `Sketch` 补 `AbsoluteAxis` / `CenterLine`(读+写) / `Factory2D` 属性 /
  `Evaluate` / `InverseOrientation` / `Get·SetAbsoluteAxisData`。
- `SystemService` 增加通用出参中转（`numeric_out_param` / `object_out_param` /
  `objects_out_param`），与 `direction_out_param` 并列。
- 曲线读取待真机草图验证。

### ✅ 阶段 4 —— in_interfaces 文件 / 相机 / 窗口（已完成，2026-11-03）

该族 40/40 全部有类型化包装：

- 相机（`Cameras`/`Camera`/`Camera2D`/`Camera3D` + `Viewpoint2D`）、文件系统
  （`FileSystem`/`FileComponent`/`File`/`Files`/`Folder`/`Folders`/`TextStream`
  ——新建/复制/删除/存在性 + 文本流读写）、定位（`Move`/`Position`）、
  `References`、打印（`PageSetup` 含约 22 个打印属性、`DraftingPageSetup`、
  `Printer(s)`）、`SelectionSets` / `SendToService` / `SystemConfiguration`、
  查看器基族（`Viewer`/`Viewer2D`/`Viewers`/`SpecsViewer`/`SpecsAndGeomWindow`/
  `Windows` + `LightSource(s)`）。
- `Window` / `Viewer3D` / `Viewpoint3D` 补齐 pycatia 成员面
  （Viewer3D 几何/灯光/导航，Window 打印/标题/几何，Viewpoint3D 视场/焦距/
  视线读数走 VBA 中转）。
- `Application` 补 `file_system` / `printers` / `system_configuration` /
  `windows`；`Document` 补 `cameras`。
- **真机只读验证已过**（文件分隔符、临时目录、系统配置、窗口/打印机/相机
  计数），并按实测修正两处与 pycatia 标注不同的 COM 事实：
  `TemporaryDirectory` 返回 Folder 对象（非字符串）、`Release`/`Version`
  返回整数（VT_I4）。
- `SelectionSets` / `SendToService` / `SystemConfiguration` 的出参列表读取
  走 `com()` 逃生舱。

### ✅ 阶段 5 —— knowledge_interfaces（已完成，2026-11-03；0.1.0 覆盖率门槛达标）

该族 35/35：`Relation` 基类（注释/上下文/输入输出参数/`Modify`/`Rename`）与
`Formula` / `Rule` / `Check` / `DesignTable` / `SetOfEquation` /
`ConstraintSatisfaction` / `Law` / `Loop`（`UseSymbolcTransformations` 为
CAA/COM 原拼写）；类型化参数 `RealParam` / `IntParam` / `StrParam` / `BoolParam` /
`EnumParam` / `Length` / `Angle` / `Dimension` / `ListParameter`（值/量程/容差/
枚举值，字符串枚举走新的 `strings_out_param` 中转）；`List` / `FreeParameters` /
`ParameterSet(s)` / `Units`（MKS 换算）；`Optimization(s)` /
`OptimizationConstraint(s)` / `FeatureGenerator`。`Parameters` 补
`CreateReal` / `CreateInteger` / `CreateString` / `CreateBoolean` / `CreateList`
等；`Parameter` 补备注/只读/重命名/字符串赋值；`Part` 与 `Product` 补
`relations` / `parameters`。

真机只读验证已过（产品级 `Parameters` / `Relations` / `RootParameterSet`）。
实测记录：产品身份参数（`Product1\Part Number`）上 `ValueAsString`
返回"找不到成员"——改用专用访问器。

### ✅ 阶段 6 —— hybrid_shape_interfaces（已完成，2026-11-03）

该族 111/111 全部有类型化包装——分母最大的一族：

- 基类 `Point` / `Line` / `HybridShapeCircle` / `HybridShapeSweep`；方向
  （元素/轴系参考 + 分量读数走 VBA 中转）。
- 点构造器 8 类（坐标/平面/曲线/曲面/中点/圆心/切点/显式）、线构造器 8 类
  （点方向/角度/法线/双切/切线/平分/轴线/显式）、平面构造器 12 类、圆族 10 类
  （圆心半径/圆心过点/三点/两点半径/双切半径/双切点/三切/中心轴/中心切线/显式）。
- 曲线曲面生成：样条/多段线/拉伸/旋转/圆柱/球/螺旋/平面螺旋/脊线/填充/截面/放样。
- 操作：投影/相交/极值（含极坐标）/最近点/反转/结合/桥接/圆角/曲线光顺/外插/
  多重提取/分割/修剪/接合/边界/凸起/中面。
- 变换：旋转/缩放/对称/轴到轴/3D 曲线偏置；展开/转移/开发/包裹曲线/包裹曲面/
  积分法则/投影距离法则/定位变换/反射线/滚动偏置/厚度/平行曲线；
  四类扫掠（圆/线/二次曲线/显式）与圆锥曲线、双切/三切圆角、修复、显式 datum。
- `HybridShapeFactory` 新增约 35 个 `add_new_*`（全部返回类型化特征）；
  既有 6 类（Join/Offset/Extract/LinePtPt/Translate/Affinity）与 `Plane` 基类
  补齐读写面。
- 特大类按序号出参网格（SweepExplicit 位置数组等）走 `com()` 逃生舱；
  方向 VBA 中转读数待 CATIA 会话复核（通道与阶段 2/4/5 相同）。

### 阶段 7 —— drafting_interfaces 剩余

尺寸标注、表格、引出线、GDT、图片、图框组件（约 30 类）。

### ✅ 阶段 8 —— space_analyses / navigator 收尾（已完成，2026-11-03——规划范围收官）

两族均 14/14：

- 空间分析（9 类）：`Clashes`/`Clash`（分组/`Compute`/冲突列表/`Export`）、
  `ClashResults`（+XML 导入）、`Conflict(s)`（类型/状态/备注，干涉点坐标走
  VBA 中转）、`Inertias`（元素复用既有 `Inertia`）、`Sections`/`Section`
  （类型/切模式/12 分量定位读写、`IsEmpty`、导出）。`SPAWorkbench` 补
  `clashes()` / `sections()`。
- 导航器（12 类）：`NavigatorWorkbench`（批注视图/组/超链接/3D 标记入口、
  `AdvancedView`、显示顺序 `Get/SetOrder`、`View`）、`DMUReview(s)`
  （`Current`/`Add`/`ImportFrom`）、`DMUDataFlow`（缓存导出/导入、CGR 替换、
  冻结另存）、`AnnotatedView(s)`（原点/视线/上方向中转、`AddFromViewpoint`）、
  `Marker2D(s)`（七种创建：箭头/圆/手绘/线/图片/矩形/文字）、`Marker3D(s)`
  （对象锚点 + 文字锚点双通道，带序号中转）、`Hyperlink(s)`（对象级 URL 列表）。
- 保留字改名按惯例：`Conflict::conflict_type`、`Section::section_type`、
  `Marker2D/3D::marker_type`。
- 出参网格与设置面板类（`*SettingAtt`）走 `com()` 逃生舱。干涉/标记链路待
  真机复核（`Group(s)` 与 `Distances` 链路已验证）。

**规划范围收官**：阶段 1–8 全部完成，395/446 ≈ 88.6%，11/14 族 100%。
刻意未覆盖：cat_tps 语义 GDT（按需）、cat_mat 2 类、进度页列出的范围外
桌面工作台族。

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
- **0.1.0：** 需要规划范围功能类覆盖至少 **50%**，且已迁移工具稳定。
  **两个条件均已满足**（阶段 1–8 后 395/446 ≈ 88.6%，规划范围收官；28 个 JSON 工具 + colorize
  已稳定）。由维护者决定切版时机。
- **1.0.0：** pycatia 自己都还没到 1.0，本项目又只按需封装，暂不考虑。

## ⛔ 非目标

- 与 pycatia 全部 84 个模块族 1:1 对齐或 CATIA 类型库全覆盖
  （CAM / 加工 / DMU / 系统集成等族保持范围外，见进度页的清单）。
- 跨平台（CATIA 自动化只在 Windows）。
- 自动拉起 CATIA。
- GUI —— 本项目只提供库和 CLI，界面由应用层负责。
