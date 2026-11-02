//! 草图基础设施：`Sketches` / `Sketch` / `Factory2D` / 几何 2D 族
//! （`Geometry2D`/`Curve2D`/`Point2D`/`Line2D`/`Circle2D`/`Ellipse2D`/
//! `Hyperbola2D`/`Parabola2D`/`Spline2D`/`ControlPoint2D`/`Axis2D`）/
//! `Constraints` / `Constraint` / `Dimension`。
//!
//! vba_bbox_generate 的矩形+约束+投影全走这条链：HybridBody.HybridSketches.Add
//! → Sketch.OpenEdition(Factory2D) → 画点线/投影/约束 → CloseEdition。
//! 约束类型常量与 CATIA 枚举一致（catCstTypeDistance=1、Length=5、
//! Horizontality=10、Verticality=13、Parallelism=8，取自 VBA 原宏）。
//! 阶段 3 起几何 2D 族成员面照 pycatia `sketcher_interfaces` 对齐；出参数组
//! （坐标/方向/参数区间等）走 `SystemService` 的 VBA 中转助手。

use crate::collections::ComCollection;
use crate::dispatch::{Arg, ComObject};
use crate::error::ComResult;

use super::part::Entity;
use super::reference::Reference;
use super::system_service::SystemService;

com_wrapper!(Spline2D);
com_wrapper!(Sketches);
com_wrapper!(Sketch);
com_wrapper!(Factory2D);
com_wrapper!(GeometricElement);
com_wrapper!(Geometry2D);
com_wrapper!(Axis2D);
com_wrapper!(Curve2D);
com_wrapper!(Point2D);
com_wrapper!(ControlPoint2D);
com_wrapper!(Line2D);
com_wrapper!(Circle2D);
com_wrapper!(Ellipse2D);
com_wrapper!(Hyperbola2D);
com_wrapper!(Parabola2D);
com_wrapper!(Constraints);
com_wrapper!(Constraint);
com_wrapper!(Dimension);

/// catCstTypeDistance。
pub const CAT_CST_TYPE_DISTANCE: i32 = 1;
/// catCstTypeLength（VBA 原宏用 5 —— 与部分文档的 3 不同，按原宏原样）。
pub const CAT_CST_TYPE_LENGTH: i32 = 5;
/// catCstTypeHorizontality。
pub const CAT_CST_TYPE_HORONTALITY: i32 = 10;
/// catCstTypeVerticality。
pub const CAT_CST_TYPE_VERTICALITY: i32 = 13;
/// catCstTypeParallelism。
pub const CAT_CST_TYPE_PARALLELISM: i32 = 8;

impl ComCollection for Sketches {
    type Item = Sketch;
}

impl Sketches {
    /// 在指定平面/坐标系引用上新建草图（Python 侧混传 Reference 或平面特征）。
    pub fn add(&self, plane: &impl ComObject) -> ComResult<Sketch> {
        Ok(Sketch::new(
            self.com()
                .call("Add", &[Arg::Obj(plane.com().clone())])?
                .as_obj()?,
        ))
    }
}

impl Sketch {
    /// 绝对轴（`AbsoluteAxis`；原点/水平/垂直参考为其中的 Point2D）。
    pub fn absolute_axis(&self) -> ComResult<Axis2D> {
        Ok(Axis2D::new(self.com().get("AbsoluteAxis")?.as_obj()?))
    }

    /// 2D 工厂（`Factory2D` 属性；与 [`Sketch::open_edition`] 同对象）。
    pub fn factory_2d(&self) -> ComResult<Factory2D> {
        Ok(Factory2D::new(self.com().get("Factory2D")?.as_obj()?))
    }

    /// 中心线（`CenterLine`）。
    pub fn center_line(&self) -> ComResult<Line2D> {
        Ok(Line2D::new(self.com().get("CenterLine")?.as_obj()?))
    }

    /// 设置中心线（`CenterLine`）。
    pub fn set_center_line(&self, line: &Line2D) -> ComResult<()> {
        self.com().put("CenterLine", Arg::Obj(line.com().clone()))
    }

    /// 草图几何元素集合（`Item("AbsoluteAxis")` 取绝对轴）。
    pub fn geometric_elements(&self) -> ComResult<super::part::Entities> {
        Ok(super::part::Entities::new(
            self.com().get("GeometricElements")?.as_obj()?,
        ))
    }

    /// 草图约束集合。
    pub fn constraints(&self) -> ComResult<Constraints> {
        Ok(Constraints::new(self.com().get("Constraints")?.as_obj()?))
    }

    /// 反转草图轮廓方向（`InverseOrientation`）。
    pub fn inverse_orientation(&self) -> ComResult<()> {
        self.com().call("InverseOrientation", &[]).map(|_| ())
    }

    /// 重算草图（`Evaluate`）。
    pub fn evaluate(&self) -> ComResult<()> {
        self.com().call("Evaluate", &[]).map(|_| ())
    }

    /// 绝对轴数据（`GetAbsoluteAxisData`，VBA 中转；6 分量：
    /// 原点 xy / 水平方向 xy / 垂直方向 xy）。
    pub fn get_absolute_axis_data(&self, ss: &SystemService) -> ComResult<Vec<f64>> {
        ss.numeric_out_param(self.com(), "GetAbsoluteAxisData")
    }

    /// 设置绝对轴数据（`SetAbsoluteAxisData`；6 分量，同 [`Sketch::get_absolute_axis_data`]）。
    pub fn set_absolute_axis_data(&self, axis_data: &[f64]) -> ComResult<()> {
        let args = Arg::SafeArray(axis_data.iter().map(|v| Arg::F64(*v)).collect());
        self.com().call("SetAbsoluteAxisData", &[args]).map(|_| ())
    }

    /// 打开草图编辑，返回 2D 工厂。
    pub fn open_edition(&self) -> ComResult<Factory2D> {
        Ok(Factory2D::new(
            self.com().call("OpenEdition", &[])?.as_obj()?,
        ))
    }

    /// 关闭草图编辑。
    pub fn close_edition(&self) -> ComResult<()> {
        self.com().call("CloseEdition", &[]).map(|_| ())
    }
}

impl Factory2D {
    /// 样条：极点数组（折线离散点; 极点建议置 Construction 不参与轮廓）。
    pub fn create_spline(&self, poles: &[Point2D]) -> ComResult<Spline2D> {
        let args = Arg::SafeArray(poles.iter().map(|p| Arg::Obj(p.com().clone())).collect());
        Ok(Spline2D::new(
            self.com().call("CreateSpline", &[args])?.as_obj()?,
        ))
    }

    pub fn create_point(&self, x: f64, y: f64) -> ComResult<Point2D> {
        Ok(Point2D::new(
            self.com()
                .call("CreatePoint", &[Arg::F64(x), Arg::F64(y)])?
                .as_obj()?,
        ))
    }

    /// 2D 线段。
    pub fn create_line(&self, x1: f64, y1: f64, x2: f64, y2: f64) -> ComResult<Line2D> {
        Ok(Line2D::new(
            self.com()
                .call(
                    "CreateLine",
                    &[Arg::F64(x1), Arg::F64(y1), Arg::F64(x2), Arg::F64(y2)],
                )?
                .as_obj()?,
        ))
    }

    /// 按起点 + 方向向量画线（`CreateLineFromVector`）。
    pub fn create_line_from_vector(&self, x1: f64, y1: f64, ux: f64, uy: f64) -> ComResult<Line2D> {
        Ok(Line2D::new(
            self.com()
                .call(
                    "CreateLineFromVector",
                    &[Arg::F64(x1), Arg::F64(y1), Arg::F64(ux), Arg::F64(uy)],
                )?
                .as_obj()?,
        ))
    }

    /// 控制点（`CreateControlPoint`；样条/曲面控制用，可置 Construction）。
    pub fn create_control_point(&self, x: f64, y: f64) -> ComResult<ControlPoint2D> {
        Ok(ControlPoint2D::new(
            self.com()
                .call("CreateControlPoint", &[Arg::F64(x), Arg::F64(y)])?
                .as_obj()?,
        ))
    }

    /// 完整圆（`CreateClosedCircle`）。
    pub fn create_closed_circle(&self, x: f64, y: f64, radius: f64) -> ComResult<Circle2D> {
        Ok(Circle2D::new(
            self.com()
                .call(
                    "CreateClosedCircle",
                    &[Arg::F64(x), Arg::F64(y), Arg::F64(radius)],
                )?
                .as_obj()?,
        ))
    }

    /// 圆弧（`CreateCircle`；start/end 为参数角，完整圆传 0 与 2π）。
    pub fn create_circle(
        &self,
        x: f64,
        y: f64,
        radius: f64,
        start_param: f64,
        end_param: f64,
    ) -> ComResult<Circle2D> {
        Ok(Circle2D::new(
            self.com()
                .call(
                    "CreateCircle",
                    &[
                        Arg::F64(x),
                        Arg::F64(y),
                        Arg::F64(radius),
                        Arg::F64(start_param),
                        Arg::F64(end_param),
                    ],
                )?
                .as_obj()?,
        ))
    }

    /// 完整椭圆（`CreateClosedEllipse`）。
    #[allow(clippy::too_many_arguments)]
    pub fn create_closed_ellipse(
        &self,
        x: f64,
        y: f64,
        major_x: f64,
        major_y: f64,
        major_radius: f64,
        minor_radius: f64,
    ) -> ComResult<Ellipse2D> {
        Ok(Ellipse2D::new(
            self.com()
                .call(
                    "CreateClosedEllipse",
                    &[
                        Arg::F64(x),
                        Arg::F64(y),
                        Arg::F64(major_x),
                        Arg::F64(major_y),
                        Arg::F64(major_radius),
                        Arg::F64(minor_radius),
                    ],
                )?
                .as_obj()?,
        ))
    }

    /// 椭圆弧（`CreateEllipse`；start/end 为参数角）。
    #[allow(clippy::too_many_arguments)]
    pub fn create_ellipse(
        &self,
        x: f64,
        y: f64,
        major_x: f64,
        major_y: f64,
        major_radius: f64,
        minor_radius: f64,
        start_param: f64,
        end_param: f64,
    ) -> ComResult<Ellipse2D> {
        Ok(Ellipse2D::new(
            self.com()
                .call(
                    "CreateEllipse",
                    &[
                        Arg::F64(x),
                        Arg::F64(y),
                        Arg::F64(major_x),
                        Arg::F64(major_y),
                        Arg::F64(major_radius),
                        Arg::F64(minor_radius),
                        Arg::F64(start_param),
                        Arg::F64(end_param),
                    ],
                )?
                .as_obj()?,
        ))
    }

    /// 双曲线（`CreateHyperbola`）。
    pub fn create_hyperbola(
        &self,
        x: f64,
        y: f64,
        axis_x: f64,
        axis_y: f64,
        major_radius: f64,
        minor_radius: f64,
    ) -> ComResult<Hyperbola2D> {
        Ok(Hyperbola2D::new(
            self.com()
                .call(
                    "CreateHyperbola",
                    &[
                        Arg::F64(x),
                        Arg::F64(y),
                        Arg::F64(axis_x),
                        Arg::F64(axis_y),
                        Arg::F64(major_radius),
                        Arg::F64(minor_radius),
                    ],
                )?
                .as_obj()?,
        ))
    }

    /// 抛物线（`CreateParabola`）。
    pub fn create_parabola(
        &self,
        x: f64,
        y: f64,
        axis_x: f64,
        axis_y: f64,
        focal_distance: f64,
    ) -> ComResult<Parabola2D> {
        Ok(Parabola2D::new(
            self.com()
                .call(
                    "CreateParabola",
                    &[
                        Arg::F64(x),
                        Arg::F64(y),
                        Arg::F64(axis_x),
                        Arg::F64(axis_y),
                        Arg::F64(focal_distance),
                    ],
                )?
                .as_obj()?,
        ))
    }

    /// 把 3D 参考投影进草图平面。
    pub fn create_projection(&self, element: &Reference) -> ComResult<Entity> {
        Ok(Entity::new(
            self.com()
                .call("CreateProjection", &[Arg::Obj(element.com().clone())])?
                .as_obj()?,
        ))
    }

    /// 投影（结果集版本，`CreateProjections`）。
    pub fn create_projections(&self, element: &Reference) -> ComResult<Entity> {
        Ok(Entity::new(
            self.com()
                .call("CreateProjections", &[Arg::Obj(element.com().clone())])?
                .as_obj()?,
        ))
    }

    /// 3D 参考与草图平面的交线（`CreateIntersection`）。
    pub fn create_intersection(&self, element: &Reference) -> ComResult<Entity> {
        Ok(Entity::new(
            self.com()
                .call("CreateIntersection", &[Arg::Obj(element.com().clone())])?
                .as_obj()?,
        ))
    }

    /// 交线集版本（`CreateIntersections`）。
    pub fn create_intersections(&self, element: &Reference) -> ComResult<Entity> {
        Ok(Entity::new(
            self.com()
                .call("CreateIntersections", &[Arg::Obj(element.com().clone())])?
                .as_obj()?,
        ))
    }
}

impl Point2D {
    /// 控制报告编号（VBA 原宏用 1..10 标注轴/点/线的引用顺序）。
    pub fn set_report_name(&self, value: i32) -> ComResult<()> {
        self.com().put("ReportName", Arg::I32(value))
    }
    /// 极点/构造元素开关（样条控制点不参与轮廓）。
    pub fn set_construction(&self, value: bool) -> ComResult<()> {
        self.com().put("Construction", Arg::Bool(value))
    }
}

impl Line2D {
    pub fn set_report_name(&self, value: i32) -> ComResult<()> {
        self.com().put("ReportName", Arg::I32(value))
    }

    pub fn set_start_point(&self, point: &impl ComObject) -> ComResult<()> {
        self.com().put("StartPoint", Arg::Obj(point.com().clone()))
    }

    pub fn set_end_point(&self, point: &impl ComObject) -> ComResult<()> {
        self.com().put("EndPoint", Arg::Obj(point.com().clone()))
    }
}

impl Constraints {
    /// 单元素约束（水平/垂直/长度等）。
    pub fn add_mono_elt_cst(&self, cst_type: i32, element: &Reference) -> ComResult<Constraint> {
        Ok(Constraint::new(
            self.com()
                .call(
                    "AddMonoEltCst",
                    &[Arg::I32(cst_type), Arg::Obj(element.com().clone())],
                )?
                .as_obj()?,
        ))
    }

    /// 双元素约束（距离/平行等）。
    pub fn add_bi_elt_cst(
        &self,
        cst_type: i32,
        first: &Reference,
        second: &Reference,
    ) -> ComResult<Constraint> {
        Ok(Constraint::new(
            self.com()
                .call(
                    "AddBiEltCst",
                    &[
                        Arg::I32(cst_type),
                        Arg::Obj(first.com().clone()),
                        Arg::Obj(second.com().clone()),
                    ],
                )?
                .as_obj()?,
        ))
    }
}

impl Constraint {
    /// 约束的尺寸对象（距离/长度约束的数值载体）。
    pub fn dimension(&self) -> ComResult<Dimension> {
        Ok(Dimension::new(self.com().get("Dimension")?.as_obj()?))
    }
}

impl Dimension {
    /// 尺寸值（mm；读出 dx/dy 测量结果 / 写 0 贴合极值点）。
    pub fn value(&self) -> ComResult<f64> {
        self.com().get("Value")?.as_f64()
    }

    pub fn set_value(&self, value: f64) -> ComResult<()> {
        self.com().put("Value", Arg::F64(value))
    }
}

// ── 几何 2D 族（阶段 3；基类 → 具体曲线） ─────────────────────────────

impl GeometricElement {
    /// 几何类型（`GeometricType`，CatGeometricType 枚举值）。
    pub fn geometric_type(&self) -> ComResult<i32> {
        self.com().get("GeometricType")?.as_i32()
    }
}

impl Geometry2D {
    prop_pair!(construction, set_construction, "Construction", bool);
    prop_pair!(report_name, set_report_name, "ReportName", i32);
}

impl Axis2D {
    /// 原点（`Origin`）。
    pub fn origin(&self) -> ComResult<Point2D> {
        Ok(Point2D::new(self.com().get("Origin")?.as_obj()?))
    }

    /// 水平参考（`HorizontalReference`）。
    pub fn horizontal_reference(&self) -> ComResult<Point2D> {
        Ok(Point2D::new(
            self.com().get("HorizontalReference")?.as_obj()?,
        ))
    }

    /// 垂直参考（`VerticalReference`）。
    pub fn vertical_reference(&self) -> ComResult<Point2D> {
        Ok(Point2D::new(self.com().get("VerticalReference")?.as_obj()?))
    }
}

impl Curve2D {
    /// 连续性（`Continuity`，CatContinuity 枚举值）。
    pub fn continuity(&self) -> ComResult<i32> {
        self.com().get("Continuity")?.as_i32()
    }

    prop_pair!(start_point, set_start_point, "StartPoint", Point2D);
    prop_pair!(end_point, set_end_point, "EndPoint", Point2D);
    prop_get!(period, "Period", f64);

    /// 是否周期曲线（`IsPeriodic`）。
    pub fn is_periodic(&self) -> ComResult<bool> {
        self.com().call("IsPeriodic", &[])?.as_bool()
    }

    /// 按参数取弧长（`GetLengthAtParam`）。
    pub fn get_length_at_param(&self, from_param: f64, to_param: f64) -> ComResult<f64> {
        self.com()
            .call(
                "GetLengthAtParam",
                &[Arg::F64(from_param), Arg::F64(to_param)],
            )?
            .as_f64()
    }

    /// 按弧长取参数（`GetParamAtLength`）。
    pub fn get_param_at_length(&self, from_param: f64, length: f64) -> ComResult<f64> {
        self.com()
            .call(
                "GetParamAtLength",
                &[Arg::F64(from_param), Arg::F64(length)],
            )?
            .as_f64()
    }

    /// 参数区间（`GetParamExtents`，VBA 中转；2 分量 [min, max]）。
    pub fn get_param_extents(&self, ss: &SystemService) -> ComResult<Vec<f64>> {
        ss.numeric_out_param(self.com(), "GetParamExtents")
    }

    /// 端点坐标（`GetEndPoints`，VBA 中转；4 分量 [x1, y1, x2, y2]）。
    pub fn get_end_points(&self, ss: &SystemService) -> ComResult<Vec<f64>> {
        ss.numeric_out_param(self.com(), "GetEndPoints")
    }

    /// 包围盒（`GetRangeBox`，VBA 中转；4 分量 [xmin, ymin, xmax, ymax]）。
    pub fn get_range_box(&self, ss: &SystemService) -> ComResult<Vec<f64>> {
        ss.numeric_out_param(self.com(), "GetRangeBox")
    }

    /// 曲率（`GetCurvature`，VBA 中转；2 分量 [curvature, radius]）。
    pub fn get_curvature(&self, param: f64, ss: &SystemService) -> ComResult<Vec<f64>> {
        let function = "rscatiacli_get_curvature";
        let code = format!(
            r#"
        Public Function {function}(oCurve As Object, iParam As Double)
            Dim oValues() As Variant
            oCurve.GetCurvature iParam, oValues
            {function} = oValues
        End Function
        "#
        );
        ss.evaluate(
            &code,
            0,
            function,
            &[Arg::Obj(self.com().clone()), Arg::F64(param)],
        )?
        .as_f64_array()
    }

    /// 导数（`GetDerivatives`，VBA 中转；分量数按 CAA，不校验）。
    pub fn get_derivatives(&self, param: f64, ss: &SystemService) -> ComResult<Vec<f64>> {
        let function = "rscatiacli_get_derivatives";
        let code = format!(
            r#"
        Public Function {function}(oCurve As Object, iParam As Double)
            Dim oValues() As Variant
            oCurve.GetDerivatives iParam, oValues
            {function} = oValues
        End Function
        "#
        );
        ss.evaluate(
            &code,
            0,
            function,
            &[Arg::Obj(self.com().clone()), Arg::F64(param)],
        )?
        .as_f64_array()
    }

    /// 切向量（`GetTangent`，VBA 中转；前 4 个为切点 xy + 切向 xy，
    /// 最后一个为方向符号）。
    pub fn get_tangent(&self, param: f64, ss: &SystemService) -> ComResult<Vec<f64>> {
        let function = "rscatiacli_get_tangent";
        let code = format!(
            r#"
        Public Function {function}(oCurve As Object, iParam As Double)
            Dim oValues() As Variant
            oCurve.GetTangent iParam, oValues
            {function} = oValues
        End Function
        "#
        );
        ss.evaluate(
            &code,
            0,
            function,
            &[Arg::Obj(self.com().clone()), Arg::F64(param)],
        )?
        .as_f64_array()
    }

    /// 按参数取点（`GetPointAtParam`，VBA 中转单对象出参）。
    pub fn get_point_at_param(&self, param: f64, ss: &SystemService) -> ComResult<Point2D> {
        Ok(Point2D::new(ss.object_out_param(
            self.com(),
            "GetPointAtParam",
            &[param],
        )?))
    }
}

impl Point2D {
    /// 坐标（`GetCoordinates`，VBA 中转；2 分量 [x, y]）。
    pub fn get_coordinates(&self, ss: &SystemService) -> ComResult<Vec<f64>> {
        ss.numeric_out_param(self.com(), "GetCoordinates")
    }

    /// 写坐标（`SetData`）。
    pub fn set_data(&self, x: f64, y: f64) -> ComResult<()> {
        self.com()
            .call("SetData", &[Arg::F64(x), Arg::F64(y)])
            .map(|_| ())
    }
}

impl ControlPoint2D {
    prop_pair!(curvature, set_curvature, "Curvature", f64);

    /// 切向（`GetTangent`，VBA 中转；2 分量 [tx, ty]）。
    pub fn get_tangent(&self, ss: &SystemService) -> ComResult<Vec<f64>> {
        ss.numeric_out_param(self.com(), "GetTangent")
    }

    /// 写切向（`SetTangent`）。
    pub fn set_tangent(&self, x: f64, y: f64) -> ComResult<()> {
        self.com()
            .call("SetTangent", &[Arg::F64(x), Arg::F64(y)])
            .map(|_| ())
    }

    /// 清除曲率约束（`UnsetCurvature`）。
    pub fn unset_curvature(&self) -> ComResult<()> {
        self.com().call("UnsetCurvature", &[]).map(|_| ())
    }

    /// 清除切向约束（`UnsetTangent`）。
    pub fn unset_tangent(&self) -> ComResult<()> {
        self.com().call("UnsetTangent", &[]).map(|_| ())
    }
}

impl Circle2D {
    prop_pair!(center_point, set_center_point, "CenterPoint", Point2D);
    prop_get!(radius, "Radius", f64);

    /// 圆心坐标（`GetCenter`，VBA 中转；2 分量 [x, y]）。
    pub fn get_center(&self, ss: &SystemService) -> ComResult<Vec<f64>> {
        ss.numeric_out_param(self.com(), "GetCenter")
    }

    /// 写圆数据（`SetData`）。
    pub fn set_data(&self, x: f64, y: f64, radius: f64) -> ComResult<()> {
        self.com()
            .call("SetData", &[Arg::F64(x), Arg::F64(y), Arg::F64(radius)])
            .map(|_| ())
    }
}

impl Ellipse2D {
    prop_pair!(center_point, set_center_point, "CenterPoint", Point2D);
    prop_get!(major_radius, "MajorRadius", f64);
    prop_get!(minor_radius, "MinorRadius", f64);

    /// 圆心（`GetCenter`，VBA 中转；2 分量）。
    pub fn get_center(&self, ss: &SystemService) -> ComResult<Vec<f64>> {
        ss.numeric_out_param(self.com(), "GetCenter")
    }

    /// 长轴方向（`GetMajorAxis`，VBA 中转；2 分量）。
    pub fn get_major_axis(&self, ss: &SystemService) -> ComResult<Vec<f64>> {
        ss.numeric_out_param(self.com(), "GetMajorAxis")
    }

    /// 短轴方向（`GetMinorAxis`，VBA 中转；2 分量）。
    pub fn get_minor_axis(&self, ss: &SystemService) -> ComResult<Vec<f64>> {
        ss.numeric_out_param(self.com(), "GetMinorAxis")
    }

    /// 写椭圆数据（`SetData`：圆心 xy、长轴方向 xy、长/短半径）。
    #[allow(clippy::too_many_arguments)]
    pub fn set_data(
        &self,
        x: f64,
        y: f64,
        major_x: f64,
        major_y: f64,
        major_radius: f64,
        minor_radius: f64,
    ) -> ComResult<()> {
        self.com()
            .call(
                "SetData",
                &[
                    Arg::F64(x),
                    Arg::F64(y),
                    Arg::F64(major_x),
                    Arg::F64(major_y),
                    Arg::F64(major_radius),
                    Arg::F64(minor_radius),
                ],
            )
            .map(|_| ())
    }
}

impl Hyperbola2D {
    prop_get!(imaginary_radius, "ImaginaryRadius", f64);
    prop_get!(radius, "Radius", f64);

    /// 轴方向（`GetAxis`，VBA 中转；2 分量）。
    pub fn get_axis(&self, ss: &SystemService) -> ComResult<Vec<f64>> {
        ss.numeric_out_param(self.com(), "GetAxis")
    }

    /// 中心（`GetCenter`，VBA 中转；2 分量）。
    pub fn get_center(&self, ss: &SystemService) -> ComResult<Vec<f64>> {
        ss.numeric_out_param(self.com(), "GetCenter")
    }

    /// 写双曲线数据（`SetData`）。
    #[allow(clippy::too_many_arguments)]
    pub fn set_data(
        &self,
        x: f64,
        y: f64,
        axis_x: f64,
        axis_y: f64,
        major_radius: f64,
        minor_radius: f64,
    ) -> ComResult<()> {
        self.com()
            .call(
                "SetData",
                &[
                    Arg::F64(x),
                    Arg::F64(y),
                    Arg::F64(axis_x),
                    Arg::F64(axis_y),
                    Arg::F64(major_radius),
                    Arg::F64(minor_radius),
                ],
            )
            .map(|_| ())
    }
}

impl Parabola2D {
    prop_get!(focal_distance, "FocalDistance", f64);

    /// 轴方向（`GetAxis`，VBA 中转；2 分量）。
    pub fn get_axis(&self, ss: &SystemService) -> ComResult<Vec<f64>> {
        ss.numeric_out_param(self.com(), "GetAxis")
    }

    /// 中心（`GetCenter`，VBA 中转；2 分量）。
    pub fn get_center(&self, ss: &SystemService) -> ComResult<Vec<f64>> {
        ss.numeric_out_param(self.com(), "GetCenter")
    }

    /// 写抛物线数据（`SetData`）。
    pub fn set_data(
        &self,
        x: f64,
        y: f64,
        axis_x: f64,
        axis_y: f64,
        focal_distance: f64,
    ) -> ComResult<()> {
        self.com()
            .call(
                "SetData",
                &[
                    Arg::F64(x),
                    Arg::F64(y),
                    Arg::F64(axis_x),
                    Arg::F64(axis_y),
                    Arg::F64(focal_distance),
                ],
            )
            .map(|_| ())
    }
}

impl Spline2D {
    /// 控制点数（`GetNumberOfControlPoints`；CAA 口径返回 double）。
    pub fn get_number_of_control_points(&self) -> ComResult<f64> {
        self.com().call("GetNumberOfControlPoints", &[])?.as_f64()
    }

    /// 控制点（`GetControlPoints`，VBA 中转对象数组；未经真机验证，
    /// 若 COM 拒收动态数组走 `com()` 逃生舱）。
    pub fn get_control_points(&self, ss: &SystemService) -> ComResult<Vec<Point2D>> {
        let raw = ss.objects_out_param(self.com(), "GetControlPoints")?;
        Ok(raw.into_iter().map(Point2D::new).collect())
    }

    /// 在指定序位后插入控制点（`InsertControlPointAfter`）。
    pub fn insert_control_point_after(
        &self,
        control_point: &Point2D,
        position: i32,
    ) -> ComResult<()> {
        self.com()
            .call(
                "InsertControlPointAfter",
                &[Arg::Obj(control_point.com().clone()), Arg::I32(position)],
            )
            .map(|_| ())
    }
}
