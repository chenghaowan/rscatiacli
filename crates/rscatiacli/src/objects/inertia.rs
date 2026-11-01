//! `Inertia`（质量属性，`Product.GetTechnologicalObject("Inertia")`）。
//!
//! 单位（CAA 文档）：Mass = kg，Position = m，Density = kg/m³。
//! 注意：Inertia 没有 Volume/WetArea 属性（pycatia 的 inertia.volume 调用
//! 实际是 AttributeError，wcc Python 版的体积/面积一直读到 0）——体积/面积
//! 走 SPAWorkbench Measurable。

use crate::dispatch::{Arg, ComObject};
use crate::error::ComResult;

use super::system_service::SystemService;

com_wrapper!(Inertia);

impl Inertia {
    /// 质量（kg）。
    pub fn mass(&self) -> ComResult<f64> {
        self.com().get("Mass")?.as_f64()
    }

    /// 密度（kg/m³；返回值语义见 CAA：1=默认值被用，-1=不均匀，其它=实际密度）。
    pub fn density(&self) -> ComResult<f64> {
        self.com().get("Density")?.as_f64()
    }

    /// 重心位置（VBA 中转取 SAFEARRAY，与 pycatia get_cog_position 同路径；
    /// 单位 m，相对产品坐标系）。
    pub fn get_cog_position(&self, ss: &SystemService) -> ComResult<[f64; 3]> {
        let function = "rscatiacli_inertia_cog";
        let code = format!(
            r#"
        Public Function {function}(oInertia As Inertia)
            Dim oCoordinates(2) as Variant
            oInertia.GetCOGPosition oCoordinates
            {function} = oCoordinates
        End Function
        "#
        );
        let values = ss
            .evaluate(&code, 0, function, &[Arg::Obj(self.com().clone())])?
            .as_f64_array()?;
        if values.len() < 3 {
            return Err(crate::error::ComError::runtime(format!(
                "GetCOGPosition 返回了 {} 个分量，期望 3",
                values.len()
            )));
        }
        Ok([values[0], values[1], values[2]])
    }
}
