//! rscatiacli —— CATIA V5 COM 晚绑定绑定库。
//!
//! 设计目标与 pycatia 一致：按 CATIA 对象模型分层封装（Application / Documents /
//! Document / Part / Body / Selection ...），但只覆盖 ltl-manage 已迁移工具实际用到的
//! 成员；未封装的成员通过 [`dispatch::ComObject::com`] 取回原始 [`dispatch::ComObj`]
//! 继续按名字派发。新增工具时按需补封装，不做 pycatia 全量移植。
//!
//! 三条连接纪律（与 ltl-manage Python 侧 get_catia_com 一致）：
//! 1. 所有 COM 调用都投递到专用 STA 线程（[`session::run_on_sta`]）；
//! 2. 只用 ROT（GetActiveObject）连接，绝不自动拉起 CATIA；
//! 3. 每次操作重新 attach，不缓存 Application / Document。

#![cfg(windows)]

pub mod collections;
pub mod dispatch;
pub mod error;
pub mod objects;
pub mod session;

pub use collections::ComCollection;
pub use dispatch::{item_at, Arg, ComObj, ComObject, FromComObj, Ret};
pub use error::{ComError, ComResult};
pub use objects::*;
pub use session::{
    attach_with_guard, catia_is_running, cnext_count, probe, process_names_containing,
    put_selftest, run_on_sta, status, with_application, Progress,
};
