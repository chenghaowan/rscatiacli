# rscatiacli

CATIA V5 的 Rust COM 绑定库（定位同 pycatia），以及基于它的工具业务层。

- `crates/rscatiacli` —— **绑定库**：IDispatch 晚绑定、STA 会话、ROT 连接、多实例守卫、
  CATIA 对象模型（Application / Documents / Document / Part / Body / Selection / Product ...）。
- `crates/rscatiacli-tools` —— **工具业务层**：colorize / body_rename / hide_show 等
  “做什么” 的逻辑，只通过 `rscatiacli` 访问 CATIA。
- `ltl-manage`（`D:\claw.forvia\ltl-manage`）作为应用层，以 path 依赖引用这两个 crate。

## 快速开始

```bash
# 编译 + 单测（纯逻辑，不需要 CATIA）
cargo test --workspace

# 连接诊断（需要 CATIA 已经启动）
cargo run -p rscatiacli -- status
cargo run -p rscatiacli -- probe
cargo run -p rscatiacli -- put-selftest
cargo run -p rscatiacli -- ps

# 不进 UI 调试工具
cargo run -p rscatiacli-tools -- json body_rename/rename_bodies.py '{"dry_run":true}'
cargo run -p rscatiacli-tools -- json hide_show/hide_show.py '{"action":"show-body","dry_run":true}'
cargo run -p rscatiacli-tools -- colorize
```

## 连接纪律

与 ltl-manage Python 侧 `get_catia_com()` 完全一致：

1. **只在专用 STA 线程上调用 COM**：`session::run_on_sta` / `with_application` 负责投递；
   超时后丢弃卡住的 worker，下次调用自动重建。
2. **只用 ROT（`GetActiveObject`）连接**，绝不自动拉起 CATIA —— 自动拉起的是无窗口后台
   实例，会让后续工具误报“检测到多个 CATIA 实例”。
3. **每次操作重新 attach**，不缓存 `Application` / `Document`。
4. **多实例守卫**：本机有多个 `CNEXT.exe` 且无法判断用户正在用哪个时，明确报错而不是
   把操作写进错误窗口。

## 对象模型映射（pycatia → rscatiacli）

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
| `vis_properties.set_show(n)` | `VisProperties::set_show(i32)`（1=隐藏，0=显示，保留 CATIA 原始语义） |
| `vis_properties.set_real_color(r,g,b,0)` | `VisProperties::set_real_color(r,g,b,0)` |
| `ProductDocument.product` | `Document::product()` |
| `product.products` | `Product::products()` |
| `product.reference_product` | `Product::reference_product()` |
| `AnyObject.name` / `.parent` | `ComObject::name()` / `.parent()`（所有包装类型通用） |

集合统一实现 `ComCollection`：`count()`、`item(i)`、`items()`、`is_empty()`；
`Item` 先按方法派发，失败时按带参属性回退，兼容不同 CATIA 版本。

**未封装的成员走逃生舱**：

```rust
use rscatiacli::ComObject;

let raw = body.com();              // &ComObj
let n = raw.get("Something")?.as_i32()?;
raw.call("SomeMethod", &[rscatiacli::Arg::I32(1)])?;
```

## 封装范围约定

**ltl-manage 做到哪里，rscatiacli 就封装到哪里。** 不追求 pycatia 全量：

- 当前只覆盖已迁移工具（colorize、body_rename、hide_show）与 `status` 探测实际用到的成员；
- 新增工具时才补它需要的对象/成员，先走 `com()` 验证，再固化成 typed API；
- 业务规则（命名清洗、显隐动作、参数解析、结果 JSON）放 `rscatiacli-tools`，
  绑定库不掺业务语义。

## 新增一个工具

1. 在 `crates/rscatiacli-tools/src/` 新建模块，实现
   `pub fn run(params: &Value, progress: &dyn Fn(&str)) -> ComResult<Value>`；
2. 在 `crates/rscatiacli-tools/src/lib.rs` 的 `json_tool()` 里注册脚本名
   （如 `"xxx/xxx.py"`），ltl-manage 会自动识别，无需改注册表；
3. 若需要新的 CATIA 对象/成员，先在 `crates/rscatiacli/src/objects/` 里补 typed 封装
   （或先用 `ComObject::com()` 原样派发）；
4. 真机验证：`cargo run -p rscatiacli-tools -- json xxx/xxx.py '{...}'`，
   `dry_run` 优先。

## 依赖方（ltl-manage）

`frontR/src-tauri/Cargo.toml`：

```toml
rscatiacli = { path = "../../../rscatiacli/crates/rscatiacli" }
rscatiacli-tools = { path = "../../../rscatiacli/crates/rscatiacli-tools" }
```

要求构建机上 `D:\claw.forvia\rscatiacli` 与 `ltl-manage` 平级存在。

## 平台

仅 Windows：CATIA 自动化依赖 `windows` crate 的 COM / OLE / Variant 接口，
绑定库在非 Windows 下不编译（`#![cfg(windows)]`）。
