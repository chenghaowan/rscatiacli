# Roadmap（路线图）

本路线图遵守项目的抽取规则：

> **只封装应用真正用到的成员。** 绑定库跟随应用层的迁移进度增长，不做 pycatia 的
> 1:1 全量移植。

状态：✅ 已完成 · 🔜 下一步 · 💭 以后 / 探索中 · ⛔ 非目标

## ✅ 已完成 —— v0.0.1 基础

- IDispatch 晚绑定：`get` / `call` / `get_with` / `put`、VARIANT 转换、`EXCEPINFO`
  错误文案、所有路径上的 `VariantClear`。
- 会话纪律：专用 STA 线程、ROT attach、多实例守卫、后台空会话恢复、每次操作重新 attach。
- Typed 对象模型：`Application`、`Documents`、`Document`、`Part`、`Bodies` / `Body`、
  `HybridBodies` / `HybridBody`、`AxisSystems`、`OriginElements`、`Product` / `Products`、
  `Selection` / `SelectionItem` / `VisProperties`、通用 `ComCollection`。
- 诊断 CLI：`status`、`probe`、`put-selftest`、`ps`。
- 工具：`colorize`、`body_rename`、`hide_show`，以及 JSON 工具注册表。
- 应用集成：path 依赖、薄适配层、`LTL_CATIA_RUST` A/B 开关。
- 真实 CATIA V5 会话验证（probe、属性写回、dry-run、应用与独立工具 CLI 输出逐字一致）。

## 🔜 下一步 —— 结构类工具

优先级跟随应用层的迁移顺序。候选：

1. **材质赋值**（`add_material`）—— 需要 `Part` 材质 / 材质管理器相关 API。
2. **活动零件抓取**（`capture_active_part`）—— 导出或快照当前零件。
3. **Body 转零件**（`body_to_part`）—— 从 Body 创建零件文档。
4. **曲面操作**（`join_surface`、`offset_surface`、`stitch_surface`、`trim_boundary`）
   —— 需要 `Reference` 与 `HybridShapeFactory` 封装。
5. **链接修复**（`link_fixer`）—— 检查并修复文档断链。
6. **产品分析 / 命名**（`product_analysis`、`batch_product_analysis`）——
   Product 树遍历、命名规则、批量处理。
7. **工程图自动切换** —— 活动文档是工程图时，联动到同名零件 / 装配文档
   （目前仍只有 Python 版）。

每一项只补它实际需要的对象模型成员；集合与会话的通用能力已经覆盖大部分遍历代码。

## 💭 之后 —— 几何与测量

这些工具故意放在最后，因为它们依赖 SAFEARRAY 出参、大数据量和几何计算，
而不是简单的树遍历：

- 包围盒 / 模仁盒 / 包装盒
- 投影面积
- 蜂窝筋
- 文字转草图
- 测量工具集
- 实体 dump

前置条件：

- 可靠的 VARIANT 数组层（读写、嵌套、所有权）；
- `Reference`、`HybridShape`、`HybridShapeFactory`、`Measurable`、`SPAWorkbench`、
  `Distances` 的 typed 封装；
- 明确数值后处理放在绑定库还是工具层。

## 💭 收尾 —— 退役 Python sidecar

- Rust 路径转为默认，移除 `LTL_CATIA_RUST` 开关。
- 维护逐工具的 parity 对照表（Python vs Rust：输入、结果 JSON、错误文案）。
- 迁完剩余工具后，从安装包里移除 Python 运行时。
- 高风险工具可选独立进程运行（`rscatiacli-tools` 作为 sidecar），避免卡死的 COM 调用
  拖垮 UI 进程。
- 工具面稳定后，按版本策略发布版本化 API。

## 💭 API 加固

- 错误分类：不用字符串匹配就能区分“成员不存在”“CATIA 忙”“未连接”“文档类型不对”。
- `ComCollection::item_by_name`。
- Parameters / Knowledge、AxisSystem、Drawing 接口。
- 更多示例与手工 CATIA 测试清单。

## 版本策略

- **0.0.x（当前）：** 绑定和工具仍在演进，允许破坏性变更。
- **0.1.0：** 需要 pycatia 的功能面至少覆盖 **50%**（以 typed 绑定计），且已迁移工具稳定。
- **1.0.0：** pycatia 自己都还没到 1.0，本项目又只按需封装，暂不考虑。

## ⛔ 非目标

- pycatia 1:1 对齐或 CATIA 类型库全覆盖。
- 跨平台（CATIA 自动化只在 Windows）。
- 自动拉起 CATIA。
- GUI —— 本项目只提供库和 CLI，界面由应用层负责。
