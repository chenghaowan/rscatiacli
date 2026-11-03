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

impl Plane {
    /// 是否参考平面（`IsARefPlane`）。
    pub fn is_a_ref_plane(&self) -> ComResult<bool> {
        self.com().call("IsARefPlane", &[])?.as_bool()
    }

    /// 位置六分量（`GetPosition`，VBA 中转：原点 xy? z + 轴分量，按 CAA 顺序）。
    pub fn get_position(&self, ss: &SystemService) -> ComResult<Vec<f64>> {
        ss.numeric_out_param(self.com(), "GetPosition")
    }

    /// 设置位置（`SetPosition(x, y, z)`）。
    pub fn set_position(&self, x: f64, y: f64, z: f64) -> ComResult<()> {
        self.com()
            .call("SetPosition", &[Arg::F64(x), Arg::F64(y), Arg::F64(z)])
            .map(|_| ())
    }

    /// 写原点（`PutOrigin`；3 分量）。
    pub fn put_origin(&self, origin: [f64; 3]) -> ComResult<()> {
        self.com()
            .call(
                "PutOrigin",
                &[Arg::SafeArray(
                    origin.iter().map(|v| Arg::F64(*v)).collect(),
                )],
            )
            .map(|_| ())
    }

    /// 写第一轴（`PutFirstAxis`；3 分量）。
    pub fn put_first_axis(&self, axis: [f64; 3]) -> ComResult<()> {
        self.com()
            .call(
                "PutFirstAxis",
                &[Arg::SafeArray(axis.iter().map(|v| Arg::F64(*v)).collect())],
            )
            .map(|_| ())
    }

    /// 写第二轴（`PutSecondAxis`；3 分量）。
    pub fn put_second_axis(&self, axis: [f64; 3]) -> ComResult<()> {
        self.com()
            .call(
                "PutSecondAxis",
                &[Arg::SafeArray(axis.iter().map(|v| Arg::F64(*v)).collect())],
            )
            .map(|_| ())
    }

    /// 移除位置（`RemovePosition`）。
    pub fn remove_position(&self) -> ComResult<()> {
        self.com().call("RemovePosition", &[]).map(|_| ())
    }
}
