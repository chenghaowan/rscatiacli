//! `SystemService`：CATIA 全局服务（环境变量读取 / VBA 注入执行）。
//!
//! `Evaluate` 的第 4 个参数是 `SAFEARRAY of VARIANT`（可混合字符串与 COM 对象），
//! 是绑定库目前唯一用到数组实参的接口。out 参数型 API（如 GetMaterialOnPart）
//! 在晚绑定下取不到出参，走 Evaluate 注入 VBA 取回 —— 与 ltl-manage Python 侧
//! （drawing_common / add_material）同一条实测路径。

use crate::dispatch::{Arg, ComObj, ComObject, Ret};
use crate::error::{ComError, ComResult};

com_wrapper!(SystemService);

impl SystemService {
    /// VBA 中转读取 3 分量出参数组（`GetXXX(CATSafeArrayVariant o...)` 型成员，
    /// 与 `Inertia::get_cog_position` 同路径；晚绑定下出参数组取不到，须注入
    /// VBA 转交）。
    pub(crate) fn direction_out_param(&self, obj: &ComObj, member: &str) -> ComResult<[f64; 3]> {
        let function = format!("rscatiacli_{}", member.to_lowercase());
        let code = format!(
            r#"
        Public Function {function}(oFeature As Object)
            Dim oValues(2) As Variant
            oFeature.{member} oValues
            {function} = oValues
        End Function
        "#
        );
        let values = self
            .evaluate(&code, 0, &function, &[Arg::Obj(obj.clone())])?
            .as_f64_array()?;
        if values.len() < 3 {
            return Err(ComError::runtime(format!(
                "{member} 返回了 {} 个分量，期望 3",
                values.len()
            )));
        }
        Ok([values[0], values[1], values[2]])
    }

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
