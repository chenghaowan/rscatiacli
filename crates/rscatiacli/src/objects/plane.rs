//! `Plane`：平面特征的局部坐标（原点 + 两条轴）。
//!
//! `GetOrigin` / `GetFirstAxis` / `GetSecondAxis` 是 SAFEARRAY 出参，晚绑定下
//! 取不到出参 —— 与 pycatia 同一条实测路径：经 `SystemService.Evaluate` 注入
//! VBA 中转取回。`trim_boundary` 用它算平面法向（第一轴 × 第二轴）。

use crate::dispatch::{Arg, ComObject};
use crate::error::ComResult;

use super::system_service::SystemService;

com_wrapper!(Plane);

impl Plane {
    /// 平面原点（模型坐标 mm）。
    pub fn get_origin(&self, ss: &SystemService) -> ComResult<[f64; 3]> {
        self.triple(ss, "GetOrigin")
    }

    /// 平面第一轴（单位向量）。
    pub fn get_first_axis(&self, ss: &SystemService) -> ComResult<[f64; 3]> {
        self.triple(ss, "GetFirstAxis")
    }

    /// 平面第二轴（单位向量）。
    pub fn get_second_axis(&self, ss: &SystemService) -> ComResult<[f64; 3]> {
        self.triple(ss, "GetSecondAxis")
    }

    fn triple(&self, ss: &SystemService, member: &str) -> ComResult<[f64; 3]> {
        // 与 pycatia plane.py 相同的模板：出参数组由 VBA 包装函数原样返回。
        let function = "rscatiacli_plane_triple";
        let code = format!(
            r#"
        Public Function {function}(oPlane As Plane)
            Dim aReturn(2) as Variant
            oPlane.{member} aReturn
            {function} = aReturn
        End Function
        "#
        );
        let values = ss
            .evaluate(&code, 0, function, &[Arg::Obj(self.com().clone())])?
            .as_f64_array()?;
        if values.len() < 3 {
            return Err(crate::error::ComError::runtime(format!(
                "{member} 返回了 {} 个分量，期望 3",
                values.len()
            )));
        }
        Ok([values[0], values[1], values[2]])
    }
}
