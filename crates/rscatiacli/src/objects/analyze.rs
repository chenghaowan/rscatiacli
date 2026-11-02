//! `Analyze`（产品级质量属性，`Product.Analyze` 属性）。
//!
//! 口径（CAA）：Mass 单位 kg；WetArea 用 mm²（非 CATIA 默认单位，pycatia 文档同注）。
//! 出参数组（GetGravityCenter / GetInertia）经 VBA 中转（与
//! [`Inertia`](super::inertia::Inertia) 同路径）。零件级质量属性优先用
//! `GetTechnologicalObject("Inertia")`（见 [`super::inertia`]）。

use crate::dispatch::{Arg, ComObject};
use crate::error::ComResult;

use super::system_service::SystemService;

com_wrapper!(Analyze);

impl Analyze {
    /// 质量（kg）。
    pub fn mass(&self) -> ComResult<f64> {
        self.com().get("Mass")?.as_f64()
    }

    /// 体积（CATIA 内部单位口径）。
    pub fn volume(&self) -> ComResult<f64> {
        self.com().get("Volume")?.as_f64()
    }

    /// 湿面积（mm²；CAA 注明用 mm² 而非默认单位）。
    pub fn wet_area(&self) -> ComResult<f64> {
        self.com().get("WetArea")?.as_f64()
    }

    /// 重心坐标（`GetGravityCenter`，VBA 中转；单位 m，相对产品坐标系）。
    pub fn get_gravity_center(&self, ss: &SystemService) -> ComResult<[f64; 3]> {
        let function = "rscatiacli_analyze_gravity_center";
        let code = format!(
            r#"
        Public Function {function}(oAnalyze As Analyze)
            Dim oCoordinates(2) As Variant
            oAnalyze.GetGravityCenter oCoordinates
            {function} = oCoordinates
        End Function
        "#
        );
        let values = ss
            .evaluate(&code, 0, function, &[Arg::Obj(self.com().clone())])?
            .as_f64_array()?;
        if values.len() < 3 {
            return Err(crate::error::ComError::runtime(format!(
                "GetGravityCenter 返回了 {} 个分量，期望 3",
                values.len()
            )));
        }
        Ok([values[0], values[1], values[2]])
    }

    /// 惯量矩 9 分量 Ixx/Ixy/Ixz/Iyx/Iyy/Iyz/Izx/Izy/Izz（`GetInertia`，VBA 中转）。
    pub fn get_inertia(&self, ss: &SystemService) -> ComResult<Vec<f64>> {
        let function = "rscatiacli_analyze_inertia";
        let code = format!(
            r#"
        Public Function {function}(oAnalyze As Analyze)
            Dim oMatrix(8) As Variant
            oAnalyze.GetInertia oMatrix
            {function} = oMatrix
        End Function
        "#
        );
        let values = ss
            .evaluate(&code, 0, function, &[Arg::Obj(self.com().clone())])?
            .as_f64_array()?;
        if values.len() < 9 {
            return Err(crate::error::ComError::runtime(format!(
                "GetInertia 返回了 {} 个分量，期望 9",
                values.len()
            )));
        }
        Ok(values)
    }
}
