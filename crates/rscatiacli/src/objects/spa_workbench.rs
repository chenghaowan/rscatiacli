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

    /// 面积（单位 m²；×1e6 转 mm²，测非面特征时 CATIA 可能报错）。
    pub fn area(&self) -> ComResult<f64> {
        self.com().get("Area")?.as_f64()
    }

    /// 两测量对象最小距离（mm；Func 直接返回 double，无需 VBA 中转）。
    pub fn get_minimum_distance(&self, other: &Reference) -> ComResult<f64> {
        self.com()
            .call("GetMinimumDistance", &[Arg::Obj(other.com().clone())])?
            .as_f64()
    }

    fn measure_triple(&self, ss: &SystemService, member: &str) -> ComResult<[f64; 3]> {
        let function = "rscatiacli_meas_triple";
        let code = format!(
            r#"
        Public Function {function}(oMeasurable As Measurable)
            Dim aReturn(2) as Variant
            oMeasurable.{member} aReturn
            {function} = aReturn
        End Function
        "#
        );
        let values = ss
            .evaluate(&code, 0, function, &[Arg::Obj(self.com().clone())])?
            .as_f64_array()?;
        if values.len() < 3 {
            return Err(ComError::runtime(format!(
                "{member} 返回了 {} 个分量，期望 3",
                values.len()
            )));
        }
        Ok([values[0], values[1], values[2]])
    }

    /// 点坐标（重合 PointCoord 极值读取路径；VBA 中转）。
    pub fn get_point(&self, ss: &SystemService) -> ComResult<[f64; 3]> {
        self.measure_triple(ss, "GetPoint")
    }

    /// 重心（体积加权质心用；VBA 中转）。
    pub fn get_cog(&self, ss: &SystemService) -> ComResult<[f64; 3]> {
        self.measure_triple(ss, "GetCOG")
    }

    /// 曲线上三点（起点/中点/终点，9 分量；直线端点读取用，VBA 中转）。
    pub fn get_points_on_curve(&self, ss: &SystemService) -> ComResult<Vec<f64>> {
        let function = "rscatiacli_meas_curve_pts";
        let code = format!(
            r#"
        Public Function {function}(oMeasurable As Measurable)
            Dim aReturn(8) as Variant
            oMeasurable.GetPointsOnCurve aReturn
            {function} = aReturn
        End Function
        "#
        );
        let values = ss
            .evaluate(&code, 0, function, &[Arg::Obj(self.com().clone())])?
            .as_f64_array()?;
        if values.len() < 9 {
            return Err(ComError::runtime(format!(
                "GetPointsOnCurve 返回了 {} 个分量，期望 9",
                values.len()
            )));
        }
        Ok(values)
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
