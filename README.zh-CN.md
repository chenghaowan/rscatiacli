# rscatiacli

**CATIA V5 的 Rust COM 晚绑定库 —— pycatia 风格的对象模型，外加一层工具业务代码。**

[English](README.md) | 简体中文

`rscatiacli` 通过 `IDispatch` 晚绑定（`GetIDsOfNames` + `Invoke`）连接正在运行的
CATIA V5 会话，提供 pycatia 风格的 typed 对象模型（`Application`、`Document`、`Part`、
`Body`、`Selection`、`Product` ...），并保留一个取回原始 `ComObj` 的逃生舱，用于尚未
封装的成员。

项目从一个生产环境的 Tauri CAD 管理应用里抽出来。抽取规则很简单：

> **只封装应用真正用到的成员。** 应用层迁到哪里，绑定库就封装到哪里；
> 不做 pycatia 的 1:1 全量移植。

## 为什么再造一个 CATIA 绑定？

- **Rust，不依赖 Python 运行时。** 原工具链依赖一个约 200 MB 的 Python sidecar
  （pycatia + `win32com`），这个库把热路径从 Python 里拿了出来。
- **晚绑定，等价于 `win32com.client.dynamic`。** CATIA 类型库缺成员
  （`Document.Product`、`Sheets` ...），静态绑定下 SAFEARRAY 出参行为也不一致；
  按名字派发可以绕开这些坑。
- **生产级的连接纪律。** 只在 STA 线程调用 COM、只走 ROT attach、多实例守卫、
  不缓存 `Application` / `Document`。

## 工作区

```
crates/rscatiacli        绑定库 + `rscatiacli` 诊断 CLI
crates/rscatiacli-tools  工具业务层 + `rscatiacli-tools` CLI
```

应用层通过 path 或 git 依赖这两个 crate：

```toml
rscatiacli = { path = "../rscatiacli/crates/rscatiacli" }
rscatiacli-tools = { path = "../rscatiacli/crates/rscatiacli-tools" }
```

## 当前状态

以下绑定已实现，并在真实 CATIA V5 会话中验证：

- `Application`、`Documents`、`Document`、`PartDocument`、`ProductDocument`
- `Part`、`Bodies` / `Body`、`HybridBodies` / `HybridBody`、`AxisSystems`、`OriginElements`
- `Selection`、`SelectionItem`、`VisProperties`
- `Product`、`Products`
- `ComCollection`（`count` / `item` / `items`），`Item` 支持方法 → 带参属性回退
- 会话诊断：`status`、`probe`、`put-selftest`、`ps`

`rscatiacli-tools` 已实现的工具：

| 工具 | 脚本键 | 作用 |
| --- | --- | --- |
| colorize | `colorize-bodies` | 给活动零件的每个 Body 分配一种调色板颜色。 |
| body rename | `body_rename/rename_bodies.py` | 清洗 Body / 几何图形集名字：`\` 分层保留最后 `keep_last` 段、非法字符转下划线、重名追加 `_2`、`_3` ... |
| hide-show | `hide_show/hide_show.py` | 显示/隐藏 Body、几何图形集、轴系、参考面、约束；装配体递归处理其下所有 Part 文档。 |

三个工具在适用处都支持 `dry_run`，并已在真实 CATIA 文档上跑过。

## 版本策略

当前版本线故意停留在 **0.0.x**：

- **0.0.x** —— 绑定和工具都还在演进，允许破坏性变更。
- **0.1.0** —— 只有在 **pycatia 的功能面至少覆盖 50%**、且已迁移工具稳定之后才升。
- **1.0.0** —— pycatia 自己都还没到 1.0，本项目又只按需封装，暂不考虑。

这不是对 pycatia 全量对齐的承诺，而是防止高估成熟度。

## 环境要求

- Windows（通过 `windows` crate 访问 COM / OLE）。
- **CATIA V5 必须已经启动。库绝不自动拉起 CATIA** —— 自动拉起的是无窗口后台实例，
  会让后续自动化出错。
- Rust stable（edition 2021）。

## 快速开始

```bash
cargo test --workspace

# 诊断（status 不需要 CATIA，其余需要已启动的会话）
cargo run -p rscatiacli -- status
cargo run -p rscatiacli -- probe
cargo run -p rscatiacli -- put-selftest
cargo run -p rscatiacli -- ps

# 不进 UI 调试工具
cargo run -p rscatiacli-tools -- json hide_show/hide_show.py '{"action":"show-body","dry_run":true}'
cargo run -p rscatiacli-tools -- json body_rename/rename_bodies.py '{"dry_run":true}'
cargo run -p rscatiacli-tools -- colorize
```

## 库用法

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

尚未封装的成员走逃生舱：

```rust
use rscatiacli::{Arg, ComObject};

let raw = body.com(); // &ComObj
let count = raw.get("Something")?.as_i32()?;
raw.call("SomeMethod", &[Arg::I32(1)])?;
```

## pycatia → rscatiacli 对照

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
| `selection.vis_properties` | `Selection::vis_properties()` |
| `vis_properties.set_show(n)` | `VisProperties::set_show(i32)` |
| `vis_properties.set_real_color(r, g, b, 0)` | `VisProperties::set_real_color(r, g, b, 0)` |
| `ProductDocument.product` | `ProductDocument::product()` |
| `product.products` | `Product::products()` |
| `product.reference_product` | `Product::reference_product()` |
| `AnyObject.name` / `.parent` | `ComObject::name()` / `.parent()` |

集合统一实现 `ComCollection`：`count()`、`item(i)`、`items()`、`is_empty()`。
`Item` 先按方法派发，失败时回退到带参属性，兼容不同 CATIA 版本。

文档类型保留 pycatia 的类名。从通用 `Document` 构造，与 pycatia 的
`PartDocument(doc.com_object)` 同义：

```rust
let doc: Document = app.active_document()?;
let part_doc: PartDocument = doc.part_document(); // 或 PartDocument::from(doc)
let part = part_doc.part()?;
```

## 设计要点

- **所有 COM 调用归一条 STA 线程。** `run_on_sta` / `with_application` 把任务投递到
  `CoInitializeEx(COINIT_APARTMENTTHREADED)` 专用线程。调用超时会丢弃该 worker
  （COM 无法取消正在执行的调用），下一次调用重建 apartment。
- **只走 ROT attach。** 对 `CATIA.Application` 调 `GetActiveObject`，绝不
  `CoCreateInstance` —— 后者会静默拉起无窗口后台实例。
- **每次操作重新 attach。** 不缓存 `Application` / `Document`，CATIA 重启或文档关闭
  都不会留下悬空引用。
- **多实例守卫。** 本机有多个 `CNEXT.exe` 且无法判断用户正在用哪个时，明确报错，
  绝不把操作写进错误会话；后台空会话会被 Quit 后重连。
- **`SetShow` 保留原始语义。** `VisProperties::set_show(i32)` 维持 CATIA 的
  `1 = 隐藏`、`0 = 显示`，不包装成 bool，避免口径反转。

## 命名与许可

类型名（`Application`、`Documents`、`Document`、`PartDocument`、`ProductDocument`、
`Part`、`Body`、`Selection` ...）沿用 CATIA COM 自动化对象模型与 pycatia 的公开 API。
**故意保持同名**，这样 Python 与 Rust 两边的代码和文档可以互相对照、迁移。

- 本项目没有复制 pycatia 源码；COM 层是基于 `IDispatch` 的独立实现。
- pycatia 本身是 MIT 许可。
- CATIA 是 Dassault Systèmes 的商标。本项目与 Dassault Systèmes 及 pycatia 项目
  没有隶属或背书关系。

## Roadmap

完整计划见 [ROADMAP.zh-CN.md](ROADMAP.zh-CN.md)。简版：

- **已完成：** 绑定基础、会话纪律、三个结构类工具、真机验证。
- **下一步：** 更多结构类工具（材质、抓取、Body 转零件、曲面 join/offset/stitch/trim）
  以及它们需要的对象模型成员。
- **之后：** 几何/测量类工具（包围盒、投影面积、蜂窝筋、文字转草图）——需要先解决
  SAFEARRAY 出参。
- **收尾：** Rust 路径转默认、退役 Python sidecar、高风险工具的进程隔离。

## 开发

```bash
cargo fmt --all
cargo test --workspace
```

依赖 CATIA 的路径无法进 CI（需要已授权的、正在运行的 CATIA 会话）。绑定改动应通过
`probe` / `put-selftest` / dry-run 在真实文档上手工验证。新工具在
`crates/rscatiacli-tools/src/lib.rs` 注册，并遵守“只封装需要的成员”。

## 许可

MIT，见 [LICENSE](LICENSE)。
