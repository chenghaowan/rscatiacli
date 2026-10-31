//! `SystemService`：CATIA 全局服务（环境变量读取 / VBA 注入执行）。
//!
//! `Evaluate` 的第 4 个参数是 `SAFEARRAY of VARIANT`（可混合字符串与 COM 对象），
//! 是绑定库目前唯一用到数组实参的接口。out 参数型 API（如 GetMaterialOnPart）
//! 在晚绑定下取不到出参，走 Evaluate 注入 VBA 取回 —— 与 ltl-manage Python 侧
//! （drawing_common / add_material）同一条实测路径。

use crate::dispatch::{Arg, ComObject, Ret};
use crate::error::ComResult;

com_wrapper!(SystemService);

impl SystemService {
    /// 读 CATIA 环境文件变量（如 CATInstallPath；setcatenv 设置的变量通常
    /// 不在系统环境里，问运行中的 CATIA 最可靠）。
    pub fn environ(&self, name: &str) -> ComResult<String> {
        self.com()
            .call("Environ", &[Arg::Str(name.to_string())])?
            .as_string()
    }

    /// 注入一段 VBA/VBScript 并调用其中的函数。
    /// `args` 以 SAFEARRAY 传入；函数返回值原样取回。
    pub fn evaluate(&self, code: &str, flags: i32, function: &str, args: &[Arg]) -> ComResult<Ret> {
        self.com().call(
            "Evaluate",
            &[
                Arg::Str(code.to_string()),
                Arg::I32(flags),
                Arg::Str(function.to_string()),
                Arg::SafeArray(args.to_vec()),
            ],
        )
    }
}
