//! `SPAWorkbench` / `Measurable`（空间分析工作台：测量）。
//!
//! `Document.SPAWorkbench` 取工作台，`GetMeasurable(reference)` 取测量对象。
//! 数值成员按需补充。单位混合制（ltl-manage 实测标定）：长度/周长 = mm，
//! 面积 = m²(×1e6 转 mm²)，体积 = m³(×1e9 转 mm³)。

use crate::dispatch::{Arg, ComObject};
use crate::error::{ComError, ComResult};

use super::reference::Reference;
use super::system_service::SystemService;

com_wrapper!(SPAWorkbench);
com_wrapper!(Measurable);

impl SPAWorkbench {
    /// 为 `Reference` 创建测量对象。
    pub fn get_measurable(&self, reference: &Reference) -> ComResult<Measurable> {
        Ok(Measurable::new(
            self.com()
                .call("GetMeasurable", &[Arg::Obj(reference.com().clone())])?
                .as_obj()?,
        ))
    }
}

impl Measurable {
    /// 体积（单位 m³；测非实体时 CATIA 可能报错或返回 0）。
    pub fn volume(&self) -> ComResult<f64> {
        self.com().get("Volume")?.as_f64()
    }

    /// 两测量对象最小距离（mm；Func 直接返回 double，无需 VBA 中转）。
    pub fn get_minimum_distance(&self, other: &Reference) -> ComResult<f64> {
        self.com()
            .call("GetMinimumDistance", &[Arg::Obj(other.com().clone())])?
            .as_f64()
    }

    /// 平面九分量：原点 + 第一轴 + 第二轴（`GetPlane` 是 SAFEARRAY 出参，
    /// 经 VBA 中转取回，与 pycatia measurable.get_plane 同一条实测路径）。
    /// 返回 [ox, oy, oz, ux, uy, uz, vx, vy, vz]。
    pub fn get_plane(&self, ss: &SystemService) -> ComResult<Vec<f64>> {
        let function = "rscatiacli_meas_plane";
        let code = format!(
            r#"
        Public Function {function}(oMeasurable As Measurable)
            Dim aReturn(8) as Variant
            oMeasurable.GetPlane aReturn
            {function} = aReturn
        End Function
        "#
        );
        let values = ss
            .evaluate(&code, 0, function, &[Arg::Obj(self.com().clone())])?
            .as_f64_array()?;
        if values.len() < 9 {
            return Err(ComError::runtime(format!(
                "GetPlane 返回了 {} 个分量，期望 9",
                values.len()
            )));
        }
        Ok(values)
    }
}
