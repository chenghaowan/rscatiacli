//! 创成式外形设计 GSD 特征族（pycatia `hybrid_shape_interfaces`，阶段 6）。
//!
//! 本文件补全 104 个类：点/线/平面构造器、方向、圆/样条/多段线、拉伸/旋转/
//! 圆柱/球/螺旋、投影/相交/极值/最近点、变换（旋转/缩放/对称/轴到轴）、
//! 分割/修剪/接合/圆角/桥接、展开/包裹等。此前已有的
//! `HybridShapeJoin`/`HybridShapeOffset`/`Extract`/`HybridShapeLinePtPt`/
//! `HybridShapeTranslate`/`HybridShapeAffinity` 在 [`super::hybrid_shapes`]，
//! `Plane` 在 [`super::plane`]，工厂在 [`super::hybrid_shape_factory`]。
//!
//! 特大类的按序号取元素出参网格（`GetPieceCutter` 等）未逐一封装，
//! 走 `ComObject::com()` 逃生舱按名派发。

use crate::dispatch::{Arg, ComObj, ComObject};
use crate::error::ComResult;

use super::reference::Reference;
use super::system_service::SystemService;

// ── 基类 ────────────────────────────────────────────────────────────

com_wrapper!(Point);
com_wrapper!(Line);
com_wrapper!(HybridShapeCircle);
com_wrapper!(HybridShapeSweep);
com_wrapper!(ShapeDesignWorkbench);

impl Point {
    /// 坐标（`GetCoordinates`，VBA 中转；3 分量）。
    pub fn get_coordinates(&self, ss: &SystemService) -> ComResult<Vec<f64>> {
        ss.numeric_out_param(self.com(), "GetCoordinates")
    }

    /// 写坐标（`SetCoordinates`；CAA 签名带出参数组，实为写入）。
    pub fn set_coordinates(&self, coordinates: [f64; 3]) -> ComResult<()> {
        self.com()
            .call(
                "SetCoordinates",
                &[Arg::SafeArray(
                    coordinates.iter().map(|v| Arg::F64(*v)).collect(),
                )],
            )
            .map(|_| ())
    }
}

impl Line {
    prop_pair!(first_upto_elem, set_first_upto_elem, "FirstUptoElem", ref_);
    prop_pair!(
        second_upto_elem,
        set_second_upto_elem,
        "SecondUptoElem",
        ref_
    );

    /// 方向分量（`GetDirection`，VBA 中转；3 分量）。
    pub fn get_direction(&self, ss: &SystemService) -> ComResult<Vec<f64>> {
        ss.numeric_out_param(self.com(), "GetDirection")
    }

    /// 起点分量（`GetOrigin`，VBA 中转；3 分量）。
    pub fn get_origin(&self, ss: &SystemService) -> ComResult<Vec<f64>> {
        ss.numeric_out_param(self.com(), "GetOrigin")
    }

    /// 写方向（`PutDirection`；3 分量）。
    pub fn put_direction(&self, direction: [f64; 3]) -> ComResult<()> {
        self.com()
            .call(
                "PutDirection",
                &[Arg::SafeArray(
                    direction.iter().map(|v| Arg::F64(*v)).collect(),
                )],
            )
            .map(|_| ())
    }
}

impl HybridShapeCircle {
    prop_pair!(
        axis_computation,
        set_axis_computation,
        "AxisComputation",
        bool
    );

    /// 起始参数角（`StartAngle`）。
    pub fn start_angle(&self) -> ComResult<f64> {
        self.com().get("StartAngle")?.as_f64()
    }

    /// 终止参数角（`EndAngle`）。
    pub fn end_angle(&self) -> ComResult<f64> {
        self.com().get("EndAngle")?.as_f64()
    }

    /// 圆心坐标（`GetCenter`，VBA 中转；3 分量）。
    pub fn get_center(&self, ss: &SystemService) -> ComResult<Vec<f64>> {
        ss.numeric_out_param(self.com(), "GetCenter")
    }

    /// 设置限制方式（`SetLimitation`）。
    pub fn set_limitation(&self, limitation: i32) -> ComResult<()> {
        self.com()
            .call("SetLimitation", &[Arg::I32(limitation)])
            .map(|_| ())
    }
}

// ── 方向 ────────────────────────────────────────────────────────────

com_wrapper!(HybridShapeDirection);

impl HybridShapeDirection {
    prop_pair!(object, set_object, "Object", ref_);
    prop_pair!(ref_axis_system, set_ref_axis_system, "RefAxisSystem", ref_);

    /// 方向类型（`Type`，CatHybridShapeDirectionType）。
    pub fn direction_type(&self) -> ComResult<i32> {
        self.com().get("Type")?.as_i32()
    }

    /// 方向规格（`DirectionSpecification`，原始对象）。
    pub fn direction_specification(&self) -> ComResult<ComObj> {
        self.com().get("DirectionSpecification")?.as_obj()
    }

    /// X 分量（`GetX`）。
    pub fn get_x(&self, ss: &SystemService) -> ComResult<f64> {
        ss.numeric_out_param(self.com(), "GetX")
            .map(|v| v.first().copied().unwrap_or(0.0))
    }

    /// Y 分量（`GetY`）。
    pub fn get_y(&self, ss: &SystemService) -> ComResult<f64> {
        ss.numeric_out_param(self.com(), "GetY")
            .map(|v| v.first().copied().unwrap_or(0.0))
    }

    /// Z 分量（`GetZ`）。
    pub fn get_z(&self, ss: &SystemService) -> ComResult<f64> {
        ss.numeric_out_param(self.com(), "GetZ")
            .map(|v| v.first().copied().unwrap_or(0.0))
    }
}

// ── 点构造器 ────────────────────────────────────────────────────────

com_wrapper!(HybridShapePointCoord);
com_wrapper!(HybridShapePointExplicit);
com_wrapper!(HybridShapePointOnCurve);
com_wrapper!(HybridShapePointOnPlane);
com_wrapper!(HybridShapePointOnSurface);
com_wrapper!(HybridShapePointBetween);
com_wrapper!(HybridShapePointCenter);
com_wrapper!(HybridShapePointTangent);

impl HybridShapePointCoord {
    /// X 坐标（`X`）。
    pub fn x(&self) -> ComResult<f64> {
        self.com().get("X")?.as_f64()
    }

    /// Y 坐标（`Y`）。
    pub fn y(&self) -> ComResult<f64> {
        self.com().get("Y")?.as_f64()
    }

    /// Z 坐标（`Z`）。
    pub fn z(&self) -> ComResult<f64> {
        self.com().get("Z")?.as_f64()
    }

    prop_pair!(pt_ref, set_pt_ref, "PtRef", ref_);
    prop_pair!(ref_axis_system, set_ref_axis_system, "RefAxisSystem", ref_);
}

impl HybridShapePointOnCurve {
    prop_pair!(curve, set_curve, "Curve", ref_);
    prop_pair!(direction, set_direction, "Direction", ref_);
    prop_pair!(point, set_point, "Point", ref_);
    prop_pair!(distance_type, set_distance_type, "DistanceType", i32);
    prop_pair!(on_curve_type, set_on_curve_type, "OnCurveType", i32);
    prop_pair!(orientation, set_orientation, "Orientation", i32);
    prop_pair!(offset, set_offset, "Offset", f64);
    prop_pair!(ratio, set_ratio, "Ratio", f64);

    /// 曲线类型（`Type`，原始对象）。
    pub fn on_curve_kind(&self) -> ComResult<ComObj> {
        self.com().get("Type")?.as_obj()
    }
}

impl HybridShapePointOnPlane {
    prop_pair!(plane, set_plane, "Plane", ref_);
    prop_pair!(point, set_point, "Point", ref_);
    prop_pair!(
        projection_surface,
        set_projection_surface,
        "ProjectionSurface",
        ref_
    );
    prop_pair!(first_direction, set_first_direction, "FirstDirection", ref_);
    prop_pair!(x_offset, set_x_offset, "XOffset", f64);
    prop_pair!(y_offset, set_y_offset, "YOffset", f64);
}

impl HybridShapePointOnSurface {
    prop_pair!(surface, set_surface, "Surface", ref_);
    prop_pair!(point, set_point, "Point", ref_);
    prop_pair!(direction, set_direction, "Direction", ref_);
    prop_pair!(offset, set_offset, "Offset", f64);
}

impl HybridShapePointBetween {
    prop_pair!(first_point, set_first_point, "FirstPoint", ref_);
    prop_pair!(second_point, set_second_point, "SecondPoint", ref_);
    prop_pair!(support, set_support, "Support", ref_);
    prop_pair!(orientation, set_orientation, "Orientation", i32);
    prop_pair!(ratio, set_ratio, "Ratio", f64);
}

impl HybridShapePointCenter {
    prop_pair!(element, set_element, "Element", ref_);
}

impl HybridShapePointTangent {
    prop_pair!(curve, set_curve, "Curve", ref_);
    prop_pair!(direction, set_direction, "Direction", ref_);
}

// ── 线构造器 ────────────────────────────────────────────────────────

com_wrapper!(HybridShapeLinePtDir);
com_wrapper!(HybridShapeLineAngle);
com_wrapper!(HybridShapeLineNormal);
com_wrapper!(HybridShapeLineBiTangent);
com_wrapper!(HybridShapeLineTangency);
com_wrapper!(HybridShapeLineBisecting);
com_wrapper!(HybridShapeLineExplicit);
com_wrapper!(HybridShapeAxisLine);

impl HybridShapeLinePtDir {
    prop_pair!(point, set_point, "Point", ref_);
    prop_pair!(dir, set_dir, "Dir", ref_);
    prop_pair!(support, set_support, "Support", ref_);
    prop_pair!(orientation, set_orientation, "Orientation", i32);
    prop_pair!(begin_offset, set_begin_offset, "BeginOffset", f64);
    prop_pair!(end_offset, set_end_offset, "EndOffset", f64);

    /// 长度类型（`GetLengthType`）。
    pub fn get_length_type(&self) -> ComResult<i32> {
        self.com().call("GetLengthType", &[])?.as_i32()
    }

    /// 设置长度类型（`SetLengthType`）。
    pub fn set_length_type(&self, kind: i32) -> ComResult<()> {
        self.com()
            .call("SetLengthType", &[Arg::I32(kind)])
            .map(|_| ())
    }

    /// 对称延伸（`GetSymmetricalExtension`）。
    pub fn get_symmetrical_extension(&self) -> ComResult<bool> {
        self.com().call("GetSymmetricalExtension", &[])?.as_bool()
    }

    /// 设置对称延伸（`SetSymmetricalExtension`）。
    pub fn set_symmetrical_extension(&self, value: bool) -> ComResult<()> {
        self.com()
            .call("SetSymmetricalExtension", &[Arg::Bool(value)])
            .map(|_| ())
    }
}

impl HybridShapeLineAngle {
    prop_pair!(point, set_point, "Point", ref_);
    prop_pair!(curve, set_curve, "Curve", ref_);
    prop_pair!(surface, set_surface, "Surface", ref_);
    prop_pair!(orientation, set_orientation, "Orientation", i32);
    prop_pair!(angle, set_angle, "Angle", f64);
    prop_pair!(begin_offset, set_begin_offset, "BeginOffset", f64);
    prop_pair!(end_offset, set_end_offset, "EndOffset", f64);
    prop_pair!(geodesic, set_geodesic, "Geodesic", bool);

    /// 长度类型（`GetLengthType`）。
    pub fn get_length_type(&self) -> ComResult<i32> {
        self.com().call("GetLengthType", &[])?.as_i32()
    }

    /// 设置长度类型（`SetLengthType`）。
    pub fn set_length_type(&self, kind: i32) -> ComResult<()> {
        self.com()
            .call("SetLengthType", &[Arg::I32(kind)])
            .map(|_| ())
    }

    /// 对称延伸（`GetSymmetricalExtension`）。
    pub fn get_symmetrical_extension(&self) -> ComResult<bool> {
        self.com().call("GetSymmetricalExtension", &[])?.as_bool()
    }

    /// 设置对称延伸（`SetSymmetricalExtension`）。
    pub fn set_symmetrical_extension(&self, value: bool) -> ComResult<()> {
        self.com()
            .call("SetSymmetricalExtension", &[Arg::Bool(value)])
            .map(|_| ())
    }
}

impl HybridShapeLineNormal {
    prop_pair!(point, set_point, "Point", ref_);
    prop_pair!(surface, set_surface, "Surface", ref_);
    prop_pair!(orientation, set_orientation, "Orientation", i32);
    prop_pair!(begin_offset, set_begin_offset, "BeginOffset", f64);
    prop_pair!(end_offset, set_end_offset, "EndOffset", f64);

    /// 长度类型（`GetLengthType`）。
    pub fn get_length_type(&self) -> ComResult<i32> {
        self.com().call("GetLengthType", &[])?.as_i32()
    }

    /// 设置长度类型（`SetLengthType`）。
    pub fn set_length_type(&self, kind: i32) -> ComResult<()> {
        self.com()
            .call("SetLengthType", &[Arg::I32(kind)])
            .map(|_| ())
    }

    /// 对称延伸（`GetSymmetricalExtension`）。
    pub fn get_symmetrical_extension(&self) -> ComResult<bool> {
        self.com().call("GetSymmetricalExtension", &[])?.as_bool()
    }

    /// 设置对称延伸（`SetSymmetricalExtension`）。
    pub fn set_symmetrical_extension(&self, value: bool) -> ComResult<()> {
        self.com()
            .call("SetSymmetricalExtension", &[Arg::Bool(value)])
            .map(|_| ())
    }
}

impl HybridShapeLineBiTangent {
    prop_pair!(curve1, set_curve1, "Curve1", ref_);
    prop_pair!(element2, set_element2, "Element2", ref_);
    prop_pair!(support, set_support, "Support", ref_);

    /// 长度类型（`GetLengthType`）。
    pub fn get_length_type(&self) -> ComResult<i32> {
        self.com().call("GetLengthType", &[])?.as_i32()
    }

    /// 设置长度类型（`SetLengthType`）。
    pub fn set_length_type(&self, kind: i32) -> ComResult<()> {
        self.com()
            .call("SetLengthType", &[Arg::I32(kind)])
            .map(|_| ())
    }
}

impl HybridShapeLineTangency {
    prop_pair!(point, set_point, "Point", ref_);
    prop_pair!(curve, set_curve, "Curve", ref_);
    prop_pair!(support, set_support, "Support", ref_);
    prop_pair!(orientation, set_orientation, "Orientation", i32);
    prop_pair!(begin_offset, set_begin_offset, "BeginOffset", f64);
    prop_pair!(end_offset, set_end_offset, "EndOffset", f64);

    /// 长度类型（`GetLengthType`）。
    pub fn get_length_type(&self) -> ComResult<i32> {
        self.com().call("GetLengthType", &[])?.as_i32()
    }

    /// 设置长度类型（`SetLengthType`）。
    pub fn set_length_type(&self, kind: i32) -> ComResult<()> {
        self.com()
            .call("SetLengthType", &[Arg::I32(kind)])
            .map(|_| ())
    }

    /// 对称延伸（`GetSymmetricalExtension`）。
    pub fn get_symmetrical_extension(&self) -> ComResult<bool> {
        self.com().call("GetSymmetricalExtension", &[])?.as_bool()
    }

    /// 设置对称延伸（`SetSymmetricalExtension`）。
    pub fn set_symmetrical_extension(&self, value: bool) -> ComResult<()> {
        self.com()
            .call("SetSymmetricalExtension", &[Arg::Bool(value)])
            .map(|_| ())
    }
}

impl HybridShapeLineBisecting {
    prop_pair!(elem1, set_elem1, "Elem1", ref_);
    prop_pair!(elem2, set_elem2, "Elem2", ref_);
    prop_pair!(ref_point, set_ref_point, "RefPoint", ref_);
    prop_pair!(support, set_support, "Support", ref_);
    prop_pair!(orientation, set_orientation, "Orientation", i32);
    prop_pair!(solution_type, set_solution_type, "SolutionType", bool);
    prop_pair!(begin_offset, set_begin_offset, "BeginOffset", f64);
    prop_pair!(end_offset, set_end_offset, "EndOffset", f64);

    /// 长度类型（`GetLengthType`）。
    pub fn get_length_type(&self) -> ComResult<i32> {
        self.com().call("GetLengthType", &[])?.as_i32()
    }

    /// 设置长度类型（`SetLengthType`）。
    pub fn set_length_type(&self, kind: i32) -> ComResult<()> {
        self.com()
            .call("SetLengthType", &[Arg::I32(kind)])
            .map(|_| ())
    }

    /// 对称延伸（`GetSymmetricalExtension`）。
    pub fn get_symmetrical_extension(&self) -> ComResult<bool> {
        self.com().call("GetSymmetricalExtension", &[])?.as_bool()
    }

    /// 设置对称延伸（`SetSymmetricalExtension`）。
    pub fn set_symmetrical_extension(&self, value: bool) -> ComResult<()> {
        self.com()
            .call("SetSymmetricalExtension", &[Arg::Bool(value)])
            .map(|_| ())
    }
}

impl HybridShapeAxisLine {
    prop_pair!(element, set_element, "Element", ref_);
    prop_pair!(direction, set_direction, "Direction", ref_);
    prop_pair!(axis_line_type, set_axis_line_type, "AxisLineType", i32);
}

// ── 平面构造器 ──────────────────────────────────────────────────────

com_wrapper!(HybridShapePlane1Curve);
com_wrapper!(HybridShapePlane1Line1Pt);
com_wrapper!(HybridShapePlane2Lines);
com_wrapper!(HybridShapePlane3Points);
com_wrapper!(HybridShapePlaneAngle);
com_wrapper!(HybridShapePlaneBetween);
com_wrapper!(HybridShapePlaneEquation);
com_wrapper!(HybridShapePlaneMean);
com_wrapper!(HybridShapePlaneNormal);
com_wrapper!(HybridShapePlaneOffset);
com_wrapper!(HybridShapePlaneOffsetPt);
com_wrapper!(HybridShapePlaneTangent);

impl HybridShapePlane1Curve {
    prop_pair!(curve, set_curve, "Curve", ref_);
}

impl HybridShapePlane1Line1Pt {
    prop_pair!(line, set_line, "Line", ref_);
    prop_pair!(point, set_point, "Point", ref_);
}

impl HybridShapePlane2Lines {
    prop_pair!(first, set_first, "First", ref_);
    prop_pair!(second, set_second, "Second", ref_);
    prop_pair!(
        forbid_non_coplanar_lines,
        set_forbid_non_coplanar_lines,
        "ForbidNonCoplanarLines",
        bool
    );
}

impl HybridShapePlane3Points {
    prop_pair!(first, set_first, "First", ref_);
    prop_pair!(second, set_second, "Second", ref_);
    prop_pair!(third, set_third, "Third", ref_);
}

impl HybridShapePlaneAngle {
    prop_pair!(ref_plane, set_ref_plane, "RefPlane", ref_);
    prop_pair!(revol_axis, set_revol_axis, "RevolAxis", ref_);
    prop_pair!(orientation, set_orientation, "Orientation", i32);
    prop_pair!(angle, set_angle, "Angle", f64);
    prop_pair!(projection_mode, set_projection_mode, "ProjectionMode", bool);
}

impl HybridShapePlaneBetween {
    prop_pair!(first_element, set_first_element, "FirstElement", ref_);
    prop_pair!(second_element, set_second_element, "SecondElement", ref_);
    prop_pair!(orientation, set_orientation, "Orientation", i32);
    prop_pair!(ratio, set_ratio, "Ratio", f64);
}

impl HybridShapePlaneEquation {
    prop_pair!(ref_axis_system, set_ref_axis_system, "RefAxisSystem", ref_);
    prop_pair!(a, set_a, "A", f64);
    prop_pair!(b, set_b, "B", f64);
    prop_pair!(c, set_c, "C", f64);
    prop_pair!(d, set_d, "D", f64);
}

impl HybridShapePlaneMean {
    /// 追加点（`AddPoint`）。
    pub fn add_point(&self, point: &Reference) -> ComResult<()> {
        self.com()
            .call("AddPoint", &[Arg::Obj(point.com().clone())])
            .map(|_| ())
    }

    /// 点数（`GetSize`）。
    pub fn get_size(&self) -> ComResult<i32> {
        self.com().call("GetSize", &[])?.as_i32()
    }

    /// 移除全部点（`RemoveAll`）。
    pub fn remove_all(&self) -> ComResult<()> {
        self.com().call("RemoveAll", &[]).map(|_| ())
    }
}

impl HybridShapePlaneNormal {
    prop_pair!(curve, set_curve, "Curve", ref_);
    prop_pair!(point, set_point, "Point", ref_);
}

impl HybridShapePlaneOffset {
    prop_pair!(ref_plane, set_ref_plane, "RefPlane", ref_);
    prop_pair!(orientation, set_orientation, "Orientation", i32);
    prop_pair!(offset, set_offset, "Offset", f64);
}

impl HybridShapePlaneOffsetPt {
    prop_pair!(ref_plane, set_ref_plane, "RefPlane", ref_);
    prop_pair!(point, set_point, "Point", ref_);
}

impl HybridShapePlaneTangent {
    prop_pair!(surface, set_surface, "Surface", ref_);
    prop_pair!(point, set_point, "Point", ref_);
}

// ── 圆 ──────────────────────────────────────────────────────────────

com_wrapper!(HybridShapeCircleCtrRad);
com_wrapper!(HybridShapeCircleCtrPt);
com_wrapper!(HybridShapeCircle3Points);
com_wrapper!(HybridShapeCircle2PointsRad);
com_wrapper!(HybridShapeCircleBitangentRadius);
com_wrapper!(HybridShapeCircleBitangentPoint);
com_wrapper!(HybridShapeCircleTritangent);
com_wrapper!(HybridShapeCircleCenterAxis);
com_wrapper!(HybridShapeCircleCenterTangent);
com_wrapper!(HybridShapeCircleExplicit);

impl HybridShapeCircleCtrRad {
    prop_pair!(center, set_center, "Center", ref_);
    prop_pair!(support, set_support, "Support", ref_);
    prop_pair!(first_direction, set_first_direction, "FirstDirection", ref_);
    prop_pair!(diameter_mode, set_diameter_mode, "DiameterMode", bool);
    prop_get!(radius, "Radius", f64);
    prop_get!(diameter, "Diameter", f64);

    /// 几何贴附支撑（`SetGeometryOnSupport`）。
    pub fn set_geometry_on_support(&self) -> ComResult<()> {
        self.com().call("SetGeometryOnSupport", &[]).map(|_| ())
    }

    /// 取消贴附（`UnsetGeometryOnSupport`）。
    pub fn unset_geometry_on_support(&self) -> ComResult<()> {
        self.com().call("UnsetGeometryOnSupport", &[]).map(|_| ())
    }

    /// 是否贴附（`IsGeodesic`）。
    pub fn is_geodesic(&self) -> ComResult<bool> {
        self.com().call("IsGeodesic", &[])?.as_bool()
    }
}

impl HybridShapeCircleCtrPt {
    prop_pair!(center, set_center, "Center", ref_);
    prop_pair!(crossing_point, set_crossing_point, "CrossingPoint", ref_);
    prop_pair!(support, set_support, "Support", ref_);

    /// 几何贴附支撑（`SetGeometryOnSupport`）。
    pub fn set_geometry_on_support(&self) -> ComResult<()> {
        self.com().call("SetGeometryOnSupport", &[]).map(|_| ())
    }

    /// 取消贴附（`UnsetGeometryOnSupport`）。
    pub fn unset_geometry_on_support(&self) -> ComResult<()> {
        self.com().call("UnsetGeometryOnSupport", &[]).map(|_| ())
    }
}

impl HybridShapeCircle3Points {
    prop_pair!(element1, set_element1, "Element1", ref_);
    prop_pair!(element2, set_element2, "Element2", ref_);
    prop_pair!(element3, set_element3, "Element3", ref_);
    prop_pair!(support, set_support, "Support", ref_);
}

impl HybridShapeCircle2PointsRad {
    prop_pair!(pt1, set_pt1, "Pt1", ref_);
    prop_pair!(pt2, set_pt2, "Pt2", ref_);
    prop_pair!(support, set_support, "Support", ref_);
    prop_pair!(orientation, set_orientation, "Orientation", i32);
    prop_pair!(diameter_mode, set_diameter_mode, "DiameterMode", bool);
    prop_get!(radius, "Radius", f64);
    prop_get!(diameter, "Diameter", f64);

    /// 几何贴附支撑（`SetGeometryOnSupport`）。
    pub fn set_geometry_on_support(&self) -> ComResult<()> {
        self.com().call("SetGeometryOnSupport", &[]).map(|_| ())
    }

    /// 取消贴附（`UnsetGeometryOnSupport`）。
    pub fn unset_geometry_on_support(&self) -> ComResult<()> {
        self.com().call("UnsetGeometryOnSupport", &[]).map(|_| ())
    }
}

impl HybridShapeCircleBitangentRadius {
    prop_pair!(curve1, set_curve1, "Curve1", ref_);
    prop_pair!(curve2, set_curve2, "Curve2", ref_);
    prop_pair!(support, set_support, "Support", ref_);
    prop_pair!(orientation1, set_orientation1, "Orientation1", i32);
    prop_pair!(orientation2, set_orientation2, "Orientation2", i32);
    prop_pair!(
        tangent_orientation1,
        set_tangent_orientation1,
        "TangentOrientation1",
        i32
    );
    prop_pair!(
        tangent_orientation2,
        set_tangent_orientation2,
        "TangentOrientation2",
        i32
    );
    prop_pair!(
        discrimination_index,
        set_discrimination_index,
        "DiscriminationIndex",
        i32
    );
    prop_pair!(diameter_mode, set_diameter_mode, "DiameterMode", bool);
    prop_get!(radius, "Radius", f64);
    prop_get!(diameter, "Diameter", f64);
}

impl HybridShapeCircleBitangentPoint {
    prop_pair!(curve1, set_curve1, "Curve1", ref_);
    prop_pair!(curve2, set_curve2, "Curve2", ref_);
    prop_pair!(pt, set_pt, "Pt", ref_);
    prop_pair!(support, set_support, "Support", ref_);
    prop_pair!(orientation1, set_orientation1, "Orientation1", i32);
    prop_pair!(orientation2, set_orientation2, "Orientation2", i32);
    prop_pair!(
        tangent_orientation1,
        set_tangent_orientation1,
        "TangentOrientation1",
        i32
    );
    prop_pair!(
        tangent_orientation2,
        set_tangent_orientation2,
        "TangentOrientation2",
        i32
    );
    prop_pair!(
        discrimination_index,
        set_discrimination_index,
        "DiscriminationIndex",
        i32
    );
}

impl HybridShapeCircleTritangent {
    prop_pair!(curve1, set_curve1, "Curve1", ref_);
    prop_pair!(curve2, set_curve2, "Curve2", ref_);
    prop_pair!(curve3, set_curve3, "Curve3", ref_);
    prop_pair!(support, set_support, "Support", ref_);
    prop_pair!(orientation1, set_orientation1, "Orientation1", i32);
    prop_pair!(orientation2, set_orientation2, "Orientation2", i32);
    prop_pair!(orientation3, set_orientation3, "Orientation3", i32);
    prop_pair!(
        tangent_orientation1,
        set_tangent_orientation1,
        "TangentOrientation1",
        i32
    );
    prop_pair!(
        tangent_orientation2,
        set_tangent_orientation2,
        "TangentOrientation2",
        i32
    );
    prop_pair!(
        tangent_orientation3,
        set_tangent_orientation3,
        "TangentOrientation3",
        i32
    );
    prop_pair!(
        discrimination_index,
        set_discrimination_index,
        "DiscriminationIndex",
        i32
    );
}

impl HybridShapeCircleCenterAxis {
    prop_pair!(axis, set_axis, "Axis", ref_);
    prop_pair!(point, set_point, "Point", ref_);
    prop_pair!(projection_mode, set_projection_mode, "ProjectionMode", bool);
    prop_pair!(diameter_mode, set_diameter_mode, "DiameterMode", bool);
    prop_get!(radius, "Radius", f64);
    prop_get!(diameter, "Diameter", f64);
}

impl HybridShapeCircleCenterTangent {
    prop_pair!(center_elem, set_center_elem, "CenterElem", ref_);
    prop_pair!(tangent_curve, set_tangent_curve, "TangentCurve", ref_);
    prop_pair!(support, set_support, "Support", ref_);
    prop_pair!(orientation1, set_orientation1, "Orientation1", i32);
    prop_pair!(orientation2, set_orientation2, "Orientation2", i32);
    prop_pair!(
        tangent_orientation1,
        set_tangent_orientation1,
        "TangentOrientation1",
        i32
    );
    prop_pair!(
        tangent_orientation2,
        set_tangent_orientation2,
        "TangentOrientation2",
        i32
    );
    prop_pair!(
        discrimination_index,
        set_discrimination_index,
        "DiscriminationIndex",
        i32
    );
    prop_pair!(diameter_mode, set_diameter_mode, "DiameterMode", bool);
    prop_get!(radius, "Radius", f64);
    prop_get!(diameter, "Diameter", f64);
}

// ── 曲线 / 曲面生成 ─────────────────────────────────────────────────

com_wrapper!(HybridShapeSpline);
com_wrapper!(HybridShapePolyline);
com_wrapper!(HybridShapeExtrude);
com_wrapper!(HybridShapeRevol);
com_wrapper!(HybridShapeCylinder);
com_wrapper!(HybridShapeSphere);
com_wrapper!(HybridShapeHelix);
com_wrapper!(HybridShapeSpiral);
com_wrapper!(HybridShapeSpine);
com_wrapper!(HybridShapeFill);
com_wrapper!(HybridShapeSection);
com_wrapper!(HybridShapeLoft);

impl HybridShapeSpline {
    /// 追加控制点（`AddPoint`）。
    pub fn add_point(&self, point: &Reference) -> ComResult<()> {
        self.com()
            .call("AddPoint", &[Arg::Obj(point.com().clone())])
            .map(|_| ())
    }

    /// 控制点数（`GetNbControlPoint`）。
    pub fn get_nb_control_point(&self) -> ComResult<i32> {
        self.com().call("GetNbControlPoint", &[])?.as_i32()
    }

    /// 按位置取控制点（`GetPoint`）。
    pub fn get_point(&self, position: i32) -> ComResult<Reference> {
        Ok(Reference::new(
            self.com()
                .call("GetPoint", &[Arg::I32(position)])?
                .as_obj()?,
        ))
    }

    /// 清空（`RemoveAll`）。
    pub fn remove_all(&self) -> ComResult<()> {
        self.com().call("RemoveAll", &[]).map(|_| ())
    }

    /// 设置支撑面（`SetSupport`）。
    pub fn set_support(&self, support: &Reference) -> ComResult<()> {
        self.com()
            .call("SetSupport", &[Arg::Obj(support.com().clone())])
            .map(|_| ())
    }
}

impl HybridShapePolyline {
    prop_pair!(closure, set_closure, "Closure", bool);

    /// 元素数（`NumberOfElements`）。
    pub fn number_of_elements(&self) -> ComResult<i32> {
        self.com().get("NumberOfElements")?.as_i32()
    }

    /// 插入点（`InsertElement`）。
    pub fn insert_element(&self, point: &Reference, position: i32) -> ComResult<()> {
        self.com()
            .call(
                "InsertElement",
                &[Arg::Obj(point.com().clone()), Arg::I32(position)],
            )
            .map(|_| ())
    }

    /// 替换点（`ReplaceElement`）。
    pub fn replace_element(&self, point: &Reference, position: i32) -> ComResult<()> {
        self.com()
            .call(
                "ReplaceElement",
                &[Arg::Obj(point.com().clone()), Arg::I32(position)],
            )
            .map(|_| ())
    }

    /// 删除点（`RemoveElement`）。
    pub fn remove_element(&self, position: i32) -> ComResult<()> {
        self.com()
            .call("RemoveElement", &[Arg::I32(position)])
            .map(|_| ())
    }
}

impl HybridShapeExtrude {
    prop_pair!(extruded_object, set_extruded_object, "ExtrudedObject", ref_);
    prop_pair!(direction, set_direction, "Direction", ref_);
    prop_pair!(
        first_limit_type,
        set_first_limit_type,
        "FirstLimitType",
        i32
    );
    prop_pair!(
        second_limit_type,
        set_second_limit_type,
        "SecondLimitType",
        i32
    );
    prop_pair!(
        first_upto_element,
        set_first_upto_element,
        "FirstUptoElement",
        ref_
    );
    prop_pair!(
        second_upto_element,
        set_second_upto_element,
        "SecondUptoElement",
        ref_
    );
    prop_pair!(orientation, set_orientation, "Orientation", bool);
    prop_pair!(
        symmetrical_extension,
        set_symmetrical_extension,
        "SymmetricalExtension",
        bool
    );
    prop_pair!(context, set_context, "Context", i32);
    prop_pair!(begin_offset, set_begin_offset, "BeginOffset", f64);
    prop_pair!(end_offset, set_end_offset, "EndOffset", f64);
}

impl HybridShapeRevol {
    prop_pair!(profile, set_profile, "Profile", ref_);
    prop_pair!(axis, set_axis, "Axis", ref_);
    prop_pair!(
        first_limit_type,
        set_first_limit_type,
        "FirstLimitType",
        i32
    );
    prop_pair!(
        second_limit_type,
        set_second_limit_type,
        "SecondLimitType",
        i32
    );
    prop_pair!(
        first_upto_element,
        set_first_upto_element,
        "FirstUptoElement",
        ref_
    );
    prop_pair!(
        second_upto_element,
        set_second_upto_element,
        "SecondUptoElement",
        ref_
    );
    prop_pair!(orientation, set_orientation, "Orientation", bool);
    prop_pair!(context, set_context, "Context", i32);
    prop_pair!(
        begin_angle_offset,
        set_begin_angle_offset,
        "BeginAngleOffset",
        f64
    );
    prop_pair!(
        end_angle_offset,
        set_end_angle_offset,
        "EndAngleOffset",
        f64
    );
    prop_get!(begin_angle, "BeginAngle", f64);
    prop_get!(end_angle, "EndAngle", f64);
}

impl HybridShapeCylinder {
    prop_pair!(center, set_center, "Center", ref_);
    prop_pair!(direction, set_direction, "Direction", ref_);
    prop_pair!(orientation, set_orientation, "Orientation", bool);
    prop_pair!(
        symmetrical_extension,
        set_symmetrical_extension,
        "SymmetricalExtension",
        bool
    );
    prop_get!(radius, "Radius", f64);
    prop_get!(length1, "Length1", f64);
    prop_get!(length2, "Length2", f64);

    /// 写半径（`Radius` 属性）。
    pub fn set_radius(&self, value: f64) -> ComResult<()> {
        self.com().put("Radius", Arg::F64(value))
    }

    /// 写下限长（`Length1` 属性）。
    pub fn set_length1(&self, value: f64) -> ComResult<()> {
        self.com().put("Length1", Arg::F64(value))
    }

    /// 写上限长（`Length2` 属性）。
    pub fn set_length2(&self, value: f64) -> ComResult<()> {
        self.com().put("Length2", Arg::F64(value))
    }

    /// 反转方向（`InvertOrientation`）。
    pub fn invert_orientation(&self) -> ComResult<()> {
        self.com().call("InvertOrientation", &[]).map(|_| ())
    }
}

impl HybridShapeSphere {
    prop_pair!(center, set_center, "Center", ref_);
    prop_pair!(axis, set_axis, "Axis", ref_);
    prop_pair!(limitation, set_limitation, "Limitation", i32);

    /// 写半径（`SetRadius`）。
    pub fn set_radius(&self, value: f64) -> ComResult<()> {
        self.com().call("SetRadius", &[Arg::F64(value)]).map(|_| ())
    }

    /// 写起始子午角（`SetBeginMeridianAngle`）。
    pub fn set_begin_meridian_angle(&self, angle: f64) -> ComResult<()> {
        self.com()
            .call("SetBeginMeridianAngle", &[Arg::F64(angle)])
            .map(|_| ())
    }

    /// 写终止子午角（`SetEndMeridianAngle`）。
    pub fn set_end_meridian_angle(&self, angle: f64) -> ComResult<()> {
        self.com()
            .call("SetEndMeridianAngle", &[Arg::F64(angle)])
            .map(|_| ())
    }

    /// 写起始纬线角（`SetBeginParallelAngle`）。
    pub fn set_begin_parallel_angle(&self, angle: f64) -> ComResult<()> {
        self.com()
            .call("SetBeginParallelAngle", &[Arg::F64(angle)])
            .map(|_| ())
    }

    /// 写终止纬线角（`SetEndParallelAngle`）。
    pub fn set_end_parallel_angle(&self, angle: f64) -> ComResult<()> {
        self.com()
            .call("SetEndParallelAngle", &[Arg::F64(angle)])
            .map(|_| ())
    }
}

impl HybridShapeHelix {
    prop_pair!(profile, set_profile, "Profile", ref_);
    prop_pair!(axis, set_axis, "Axis", ref_);
    prop_pair!(starting_point, set_starting_point, "StartingPoint", ref_);
    prop_pair!(
        clockwise_revolution,
        set_clockwise_revolution,
        "ClockwiseRevolution",
        bool
    );
    prop_pair!(invert_axis, set_invert_axis, "InvertAxis", bool);
    prop_pair!(taper_outward, set_taper_outward, "TaperOutward", bool);
    prop_pair!(pitch_law_type, set_pitch_law_type, "PitchLawType", i32);

    /// 写螺距（`SetPitch`）。
    pub fn set_pitch(&self, pitch: f64) -> ComResult<()> {
        self.com().call("SetPitch", &[Arg::F64(pitch)]).map(|_| ())
    }

    /// 写高度（`SetHeight`）。
    pub fn set_height(&self, height: f64) -> ComResult<()> {
        self.com()
            .call("SetHeight", &[Arg::F64(height)])
            .map(|_| ())
    }

    /// 写圈数（`SetRevolutionNumber`）。
    pub fn set_revolution_number(&self, value: f64) -> ComResult<()> {
        self.com()
            .call("SetRevolutionNumber", &[Arg::F64(value)])
            .map(|_| ())
    }

    /// 写起始角（`SetStartingAngle`）。
    pub fn set_starting_angle(&self, value: f64) -> ComResult<()> {
        self.com()
            .call("SetStartingAngle", &[Arg::F64(value)])
            .map(|_| ())
    }

    /// 写锥角（`SetTaperAngle`）。
    pub fn set_taper_angle(&self, value: f64) -> ComResult<()> {
        self.com()
            .call("SetTaperAngle", &[Arg::F64(value)])
            .map(|_| ())
    }
}

impl HybridShapeSpiral {
    prop_pair!(center_point, set_center_point, "CenterPoint", ref_);
    prop_pair!(axis, set_axis, "Axis", ref_);
    prop_pair!(support, set_support, "Support", ref_);
    prop_pair!(
        clockwise_revolution,
        set_clockwise_revolution,
        "ClockwiseRevolution",
        bool
    );
    prop_pair!(invert_axis, set_invert_axis, "InvertAxis", bool);
    prop_pair!(spiral_type, set_spiral_type, "Type", i32);
}

impl HybridShapeSpine {
    prop_pair!(start_point, set_start_point, "StartPoint", ref_);
    prop_pair!(orientation, set_orientation, "Orientation", i32);

    /// 追加引导线（`AddGuide`）。
    pub fn add_guide(&self, guide: &Reference) -> ComResult<()> {
        self.com()
            .call("AddGuide", &[Arg::Obj(guide.com().clone())])
            .map(|_| ())
    }

    /// 追加截面（`AddSection`）。
    pub fn add_section(&self, section: &Reference) -> ComResult<()> {
        self.com()
            .call("AddSection", &[Arg::Obj(section.com().clone())])
            .map(|_| ())
    }

    /// 截面数（`GetNumberOfSections`）。
    pub fn get_number_of_sections(&self) -> ComResult<i32> {
        self.com().call("GetNumberOfSections", &[])?.as_i32()
    }
}

impl HybridShapeFill {
    prop_pair!(continuity, set_continuity, "Continuity", i32);
    prop_pair!(detection, set_detection, "Detection", i32);
    prop_pair!(plane_only_mode, set_plane_only_mode, "PlaneOnlyMode", bool);
    prop_pair!(tolerant_mode, set_tolerant_mode, "TolerantMode", bool);

    /// 追加边界（`AddBound`）。
    pub fn add_bound(&self, boundary: &Reference) -> ComResult<()> {
        self.com()
            .call("AddBound", &[Arg::Obj(boundary.com().clone())])
            .map(|_| ())
    }

    /// 清空边界（`RemoveAllBound`）。
    pub fn remove_all_bound(&self) -> ComResult<()> {
        self.com().call("RemoveAllBound", &[]).map(|_| ())
    }
}

impl HybridShapeSection {
    prop_pair!(section_plane, set_section_plane, "SectionPlane", ref_);
}

impl HybridShapeLoft {
    prop_pair!(
        section_coupling,
        set_section_coupling,
        "SectionCoupling",
        i32
    );
    prop_pair!(relimitation, set_relimitation, "Relimitation", i32);
    prop_pair!(
        canonical_detection,
        set_canonical_detection,
        "CanonicalDetection",
        i32
    );
    prop_pair!(context, set_context, "Context", i32);
    prop_pair!(
        boolean_operation,
        set_boolean_operation,
        "BooleanOperation",
        i32
    );

    /// 追加引导线（`AddGuide`）。
    pub fn add_guide(&self, guide: &Reference) -> ComResult<()> {
        self.com()
            .call("AddGuide", &[Arg::Obj(guide.com().clone())])
            .map(|_| ())
    }

    /// 设置脊线（`SetSpine`）。
    pub fn set_spine(&self, spine: &Reference) -> ComResult<()> {
        self.com()
            .call("SetSpine", &[Arg::Obj(spine.com().clone())])
            .map(|_| ())
    }
}

// ── 操作 ────────────────────────────────────────────────────────────

com_wrapper!(HybridShapeProject);
com_wrapper!(HybridShapeIntersection);
com_wrapper!(HybridShapeExtremum);
com_wrapper!(HybridShapeExtremumPolar);
com_wrapper!(HybridShapeNear);
com_wrapper!(HybridShapeInverse);
com_wrapper!(HybridShapeCombine);
com_wrapper!(HybridShapeConnect);
com_wrapper!(HybridShapeCorner);
com_wrapper!(HybridShapeCurveSmooth);
com_wrapper!(HybridShapeExtrapol);
com_wrapper!(HybridShapeExtractMulti);
com_wrapper!(HybridShapeSplit);
com_wrapper!(HybridShapeTrim);
com_wrapper!(HybridShapeAssemble);
com_wrapper!(HybridShapeBoundary);
com_wrapper!(HybridShapeBump);
com_wrapper!(HybridShapeMidSurface);

impl HybridShapeProject {
    prop_pair!(elem_to_project, set_elem_to_project, "ElemToProject", ref_);
    prop_pair!(support, set_support, "Support", ref_);
    prop_pair!(direction, set_direction, "Direction", ref_);
    prop_pair!(normal, set_normal, "Normal", bool);
    prop_pair!(
        extrapolation_mode,
        set_extrapolation_mode,
        "ExtrapolationMode",
        i32
    );
    prop_pair!(smoothing_type, set_smoothing_type, "SmoothingType", i32);
    prop_pair!(solution_type, set_solution_type, "SolutionType", i32);
    prop_pair!(
        maximum_deviation_value,
        set_maximum_deviation_value,
        "MaximumDeviationValue",
        f64
    );
    prop_pair!(p_3d_smoothing, set_p_3d_smoothing, "P3DSmoothing", bool);
}

impl HybridShapeIntersection {
    prop_pair!(element1, set_element1, "Element1", ref_);
    prop_pair!(element2, set_element2, "Element2", ref_);
    prop_pair!(extend_mode, set_extend_mode, "ExtendMode", i32);
    prop_pair!(
        extrapolate_mode,
        set_extrapolate_mode,
        "ExtrapolateMode",
        bool
    );
    prop_pair!(intersect_mode, set_intersect_mode, "IntersectMode", bool);
    prop_pair!(point_type, set_point_type, "PointType", i32);
    prop_pair!(solid_mode, set_solid_mode, "SolidMode", bool);
}

impl HybridShapeExtremum {
    prop_pair!(
        reference_element,
        set_reference_element,
        "ReferenceElement",
        ref_
    );
    prop_pair!(direction, set_direction, "Direction", ref_);
    prop_pair!(direction2, set_direction2, "Direction2", ref_);
    prop_pair!(direction3, set_direction3, "Direction3", ref_);
    prop_pair!(extremum_type, set_extremum_type, "ExtremumType", i32);
    prop_pair!(extremum_type2, set_extremum_type2, "ExtremumType2", i32);
    prop_pair!(extremum_type3, set_extremum_type3, "ExtremumType3", i32);
}

impl HybridShapeExtremumPolar {
    prop_pair!(contour, set_contour, "Contour", ref_);
    prop_pair!(origin, set_origin, "Origin", ref_);
    prop_pair!(support, set_support, "Support", ref_);
    prop_pair!(dir, set_dir, "Dir", ref_);
    prop_pair!(extremum_type, set_extremum_type, "ExtremumType", i32);
}

impl HybridShapeNear {
    prop_pair!(
        reference_element,
        set_reference_element,
        "ReferenceElement",
        ref_
    );
    prop_pair!(
        multiple_solution,
        set_multiple_solution,
        "MultipleSolution",
        ref_
    );
}

impl HybridShapeInverse {
    prop_pair!(element, set_element, "Element", ref_);
    prop_pair!(orientation, set_orientation, "Orientation", i32);
}

impl HybridShapeCombine {
    prop_pair!(elem1, set_elem1, "Elem1", ref_);
    prop_pair!(elem2, set_elem2, "Elem2", ref_);
    prop_pair!(direction1, set_direction1, "Direction1", ref_);
    prop_pair!(direction2, set_direction2, "Direction2", ref_);
    prop_pair!(
        nearest_solution,
        set_nearest_solution,
        "NearestSolution",
        i32
    );
    prop_pair!(
        solution_type_combine,
        set_solution_type_combine,
        "SolutionTypeCombine",
        i32
    );
}

impl HybridShapeConnect {
    prop_pair!(first_curve, set_first_curve, "FirstCurve", ref_);
    prop_pair!(second_curve, set_second_curve, "SecondCurve", ref_);
    prop_pair!(first_point, set_first_point, "FirstPoint", ref_);
    prop_pair!(second_point, set_second_point, "SecondPoint", ref_);
    prop_pair!(base_curve, set_base_curve, "BaseCurve", ref_);
    prop_pair!(
        first_orientation,
        set_first_orientation,
        "FirstOrientation",
        i32
    );
    prop_pair!(
        second_orientation,
        set_second_orientation,
        "SecondOrientation",
        i32
    );
    prop_pair!(
        first_continuity,
        set_first_continuity,
        "FirstContinuity",
        i32
    );
    prop_pair!(
        second_continuity,
        set_second_continuity,
        "SecondContinuity",
        i32
    );
    prop_pair!(connect_type, set_connect_type, "ConnectType", i32);
    prop_pair!(trim, set_trim, "Trim", bool);
}

impl HybridShapeCorner {
    prop_pair!(first_elem, set_first_elem, "FirstElem", ref_);
    prop_pair!(second_elem, set_second_elem, "SecondElem", ref_);
    prop_pair!(direction, set_direction, "Direction", ref_);
    prop_pair!(support, set_support, "Support", ref_);
    prop_pair!(orientation1, set_orientation1, "FirstOrientation", i32);
    prop_pair!(orientation2, set_orientation2, "SecondOrientation", i32);
    prop_pair!(
        tangent_orientation1,
        set_tangent_orientation1,
        "FirstTangentOrientation",
        i32
    );
    prop_pair!(
        tangent_orientation2,
        set_tangent_orientation2,
        "SecondTangentOrientation",
        i32
    );
    prop_pair!(
        discrimination_index,
        set_discrimination_index,
        "DiscriminationIndex",
        i32
    );
    prop_pair!(corner_type, set_corner_type, "CornerType", i32);
    prop_pair!(on_vertex, set_on_vertex, "OnVertex", bool);
    prop_pair!(trim, set_trim, "Trim", bool);
    prop_pair!(trim_mode, set_trim_mode, "TrimMode", i32);
    prop_pair!(radius, set_radius, "Radius", f64);

    /// 反转第一元素方向（`InvertFirstOrientation`）。
    pub fn invert_first_orientation(&self) -> ComResult<()> {
        self.com().call("InvertFirstOrientation", &[]).map(|_| ())
    }

    /// 反转第二元素方向（`InvertSecondOrientation`）。
    pub fn invert_second_orientation(&self) -> ComResult<()> {
        self.com().call("InvertSecondOrientation", &[]).map(|_| ())
    }
}

impl HybridShapeCurveSmooth {
    prop_pair!(curve_to_smooth, set_curve_to_smooth, "CurveToSmooth", ref_);
    prop_pair!(support, set_support, "Support", ref_);
    prop_pair!(correction_mode, set_correction_mode, "CorrectionMode", i32);
    prop_pair!(
        end_extremity_continuity,
        set_end_extremity_continuity,
        "EndExtremityContinuity",
        i32
    );
    prop_pair!(
        start_extremity_continuity,
        set_start_extremity_continuity,
        "StartExtremityContinuity",
        i32
    );
    prop_pair!(
        curvature_threshold,
        set_curvature_threshold,
        "CurvatureThreshold",
        f64
    );
    prop_pair!(
        curvature_threshold_activity,
        set_curvature_threshold_activity,
        "CurvatureThresholdActivity",
        bool
    );
    prop_pair!(
        topology_simplification_activity,
        set_topology_simplification_activity,
        "TopologySimplificationActivity",
        bool
    );
}

impl HybridShapeExtrapol {
    prop_pair!(
        elem_to_extrapol,
        set_elem_to_extrapol,
        "ElemToExtrapol",
        ref_
    );
    prop_pair!(boundary, set_boundary, "Boundary", ref_);
    prop_pair!(support, set_support, "Support", ref_);
    prop_pair!(elem_until, set_elem_until, "ElemUntil", ref_);
    prop_pair!(border_type, set_border_type, "BorderType", i32);
    prop_pair!(continuity_type, set_continuity_type, "ContinuityType", i32);
    prop_pair!(limit_type, set_limit_type, "LimitType", i32);
    prop_pair!(
        propagation_mode,
        set_propagation_mode,
        "PropagationMode",
        i32
    );
    prop_pair!(
        constant_length_mode,
        set_constant_length_mode,
        "ConstantLengthMode",
        bool
    );
    prop_pair!(
        extend_edges_mode,
        set_extend_edges_mode,
        "ExtendEdgesMode",
        bool
    );
    prop_pair!(
        extrapol_both_sides_identically,
        set_extrapol_both_sides_identically,
        "ExtrapolBothSidesIdentically",
        bool
    );
}

impl HybridShapeExtractMulti {
    /// 追加约束元素（`AddConstraint`：元素/传播类型/补集/联邦/曲率阈值/位置）。
    #[allow(clippy::too_many_arguments)]
    pub fn add_constraint(
        &self,
        constraint: &Reference,
        kind: i32,
        complementaire: bool,
        is_federated: bool,
        curvature_threshold: f64,
        position: i32,
    ) -> ComResult<()> {
        self.com()
            .call(
                "AddConstraint",
                &[
                    Arg::Obj(constraint.com().clone()),
                    Arg::I32(kind),
                    Arg::Bool(complementaire),
                    Arg::Bool(is_federated),
                    Arg::F64(curvature_threshold),
                    Arg::I32(position),
                ],
            )
            .map(|_| ())
    }

    /// 按位置取元素（`GetElement`）。
    pub fn get_element(&self, position: i32) -> ComResult<Reference> {
        Ok(Reference::new(
            self.com()
                .call("GetElement", &[Arg::I32(position)])?
                .as_obj()?,
        ))
    }

    /// 按位置删除元素（`RemoveElement`）。
    pub fn remove_element(&self, position: i32) -> ComResult<()> {
        self.com()
            .call("RemoveElement", &[Arg::I32(position)])
            .map(|_| ())
    }
}

impl HybridShapeSplit {
    prop_pair!(elem_to_cut, set_elem_to_cut, "ElemToCut", ref_);
    prop_pair!(cutting_elem, set_cutting_elem, "CuttingElem", ref_);
    prop_pair!(support, set_support, "Support", ref_);
    prop_pair!(orientation, set_orientation, "Orientation", i32);
    prop_pair!(volume_result, set_volume_result, "VolumeResult", i32);
    prop_pair!(both_sides_mode, set_both_sides_mode, "BothSidesMode", bool);
    prop_pair!(
        automatic_extrapolation_mode,
        set_automatic_extrapolation_mode,
        "AutomaticExtrapolationMode",
        bool
    );
    prop_pair!(
        intersection_computation,
        set_intersection_computation,
        "IntersectionComputation",
        bool
    );
    prop_pair!(
        extrapolation_type,
        set_extrapolation_type,
        "ExtrapolationType",
        i32
    );

    /// 反转保留侧（`InvertOrientation`）。
    pub fn invert_orientation(&self) -> ComResult<()> {
        self.com().call("InvertOrientation", &[]).map(|_| ())
    }
}

impl HybridShapeTrim {
    prop_pair!(first_elem, set_first_elem, "FirstElem", ref_);
    prop_pair!(second_elem, set_second_elem, "SecondElem", ref_);
    prop_pair!(support, set_support, "Support", ref_);
    prop_pair!(
        first_orientation,
        set_first_orientation,
        "FirstOrientation",
        i32
    );
    prop_pair!(
        second_orientation,
        set_second_orientation,
        "SecondOrientation",
        i32
    );
    prop_pair!(mode, set_mode, "Mode", i32);
    prop_pair!(connex, set_connex, "Connex", bool);
    prop_pair!(manifold, set_manifold, "Manifold", bool);
    prop_pair!(simplify, set_simplify, "Simplify", bool);
    prop_pair!(keep_all_pieces, set_keep_all_pieces, "KeepAllPieces", bool);
    prop_pair!(
        intersection_computation,
        set_intersection_computation,
        "IntersectionComputation",
        bool
    );
    prop_pair!(
        automatic_extrapolation_mode,
        set_automatic_extrapolation_mode,
        "AutomaticExtrapolationMode",
        bool
    );

    /// 反转第一元素方向（`InvertFirstOrientation`）。
    pub fn invert_first_orientation(&self) -> ComResult<()> {
        self.com().call("InvertFirstOrientation", &[]).map(|_| ())
    }

    /// 反转第二元素方向（`InvertSecondOrientation`）。
    pub fn invert_second_orientation(&self) -> ComResult<()> {
        self.com().call("InvertSecondOrientation", &[]).map(|_| ())
    }
}

impl HybridShapeAssemble {
    prop_pair!(invert, set_invert, "Invert", bool);

    /// 追加元素（`AddElement`）。
    pub fn add_element(&self, element: &Reference) -> ComResult<()> {
        self.com()
            .call("AddElement", &[Arg::Obj(element.com().clone())])
            .map(|_| ())
    }

    /// 元素数（`GetElementsSize`）。
    pub fn get_elements_size(&self) -> ComResult<i32> {
        self.com().call("GetElementsSize", &[])?.as_i32()
    }

    /// 按序号取元素（`GetElement`）。
    pub fn get_element(&self, rank: i32) -> ComResult<Reference> {
        Ok(Reference::new(
            self.com().call("GetElement", &[Arg::I32(rank)])?.as_obj()?,
        ))
    }

    /// 按序号移除元素（`RemoveElement`）。
    pub fn remove_element(&self, rank: i32) -> ComResult<()> {
        self.com()
            .call("RemoveElement", &[Arg::I32(rank)])
            .map(|_| ())
    }

    /// 缝合偏差（`SetDeviation`）。
    pub fn set_deviation(&self, deviation: f64) -> ComResult<()> {
        self.com()
            .call("SetDeviation", &[Arg::F64(deviation)])
            .map(|_| ())
    }

    /// 只缝合有公共边的曲面（`SetConnex`）。
    pub fn set_connex(&self, connex: bool) -> ComResult<()> {
        self.com()
            .call("SetConnex", &[Arg::Bool(connex)])
            .map(|_| ())
    }

    /// 简化（`SetSimplify`）。
    pub fn set_simplify(&self, simplify: bool) -> ComResult<()> {
        self.com()
            .call("SetSimplify", &[Arg::Bool(simplify)])
            .map(|_| ())
    }
}

impl HybridShapeBoundary {
    prop_pair!(initial_element, set_initial_element, "InitialElement", ref_);
    prop_pair!(support, set_support, "Support", ref_);
    prop_pair!(propagation, set_propagation, "Propagation", i32);
    prop_pair!(from, set_from, "From", ref_);
    prop_pair!(to, set_to, "To", ref_);
    prop_pair!(
        from_orientation,
        set_from_orientation,
        "FromOrientation",
        i32
    );
    prop_pair!(to_orientation, set_to_orientation, "ToOrientation", i32);
}

impl HybridShapeBump {
    prop_pair!(body_to_bump, set_body_to_bump, "BodyToBump", ref_);
    prop_pair!(
        deformation_center,
        set_deformation_center,
        "DeformationCenter",
        ref_
    );
    prop_pair!(deformation_dir, set_deformation_dir, "DeformationDir", ref_);
    prop_pair!(
        deformation_dist,
        set_deformation_dist,
        "DeformationDist",
        ref_
    );
    prop_pair!(
        deformation_dist_value,
        set_deformation_dist_value,
        "DeformationDistValue",
        f64
    );
    prop_pair!(limit_curve, set_limit_curve, "LimitCurve", ref_);
    prop_pair!(projection_dir, set_projection_dir, "ProjectionDir", ref_);
    prop_pair!(continuity_type, set_continuity_type, "ContinuityType", i32);
}

impl HybridShapeMidSurface {
    prop_pair!(support, set_support, "Support", ref_);
    prop_pair!(creation_mode, set_creation_mode, "CreationMode", i32);
    prop_pair!(threshold, set_threshold, "Threshold", f64);
}

// ── 变换 ────────────────────────────────────────────────────────────

com_wrapper!(HybridShapeRotate);
com_wrapper!(HybridShapeScaling);
com_wrapper!(HybridShapeSymmetry);
com_wrapper!(HybridShapeAxisToAxis);
com_wrapper!(HybridShape3DCurveOffset);
com_wrapper!(HybridShapeBlend);
com_wrapper!(HybridShapeDevelop);
com_wrapper!(HybridShapeUnfold);
com_wrapper!(HybridShapeTransfer);
com_wrapper!(HybridShapeThickness);
com_wrapper!(HybridShapeWrapCurve);
com_wrapper!(HybridShapeWrapSurface);
com_wrapper!(HybridShapeIntegratedLaw);
com_wrapper!(HybridShapeLawDistProj);
com_wrapper!(HybridShapePositionTransfo);
com_wrapper!(HybridShapeReflectLine);
com_wrapper!(HybridShapeRollingOffset);
com_wrapper!(HybridShapeCurvePar);

impl HybridShapeRotate {
    prop_pair!(elem_to_rotate, set_elem_to_rotate, "ElemToRotate", ref_);
    prop_pair!(axis, set_axis, "Axis", ref_);
    prop_pair!(rotation_type, set_rotation_type, "RotationType", i32);
    prop_pair!(angle_value, set_angle_value, "AngleValue", f64);
    prop_pair!(first_element, set_first_element, "FirstElement", ref_);
    prop_pair!(second_element, set_second_element, "SecondElement", ref_);
    prop_pair!(first_point, set_first_point, "FirstPoint", ref_);
    prop_pair!(second_point, set_second_point, "SecondPoint", ref_);
    prop_pair!(third_point, set_third_point, "ThirdPoint", ref_);
    prop_pair!(
        orientation_of_first_element,
        set_orientation_of_first_element,
        "OrientationOfFirstElement",
        bool
    );
    prop_pair!(
        orientation_of_second_element,
        set_orientation_of_second_element,
        "OrientationOfSecondElement",
        bool
    );
    prop_pair!(volume_result, set_volume_result, "VolumeResult", bool);
    prop_get!(angle, "Angle", f64);

    /// 设置创建模式（`SetCreationMode`）。
    pub fn set_creation_mode(&self, creation: bool) -> ComResult<()> {
        self.com()
            .call("SetCreationMode", &[Arg::Bool(creation)])
            .map(|_| ())
    }
}

impl HybridShapeScaling {
    prop_pair!(elem_to_scale, set_elem_to_scale, "ElemToScale", ref_);
    prop_pair!(center, set_center, "Center", ref_);
    prop_pair!(creation_mode, set_creation_mode, "CreationMode", bool);
    prop_pair!(volume_result, set_volume_result, "VolumeResult", bool);
    prop_pair!(ratio_value, set_ratio_value, "RatioValue", f64);
    prop_get!(ratio, "Ratio", f64);
}

impl HybridShapeSymmetry {
    prop_pair!(
        elem_to_symmetry,
        set_elem_to_symmetry,
        "ElemToSymmetry",
        ref_
    );
    prop_pair!(reference, set_reference, "Reference", ref_);
    prop_pair!(creation_mode, set_creation_mode, "CreationMode", bool);
    prop_pair!(volume_result, set_volume_result, "VolumeResult", bool);
}

impl HybridShapeAxisToAxis {
    prop_pair!(
        elem_to_transform,
        set_elem_to_transform,
        "ElemToTransform",
        ref_
    );
    prop_pair!(reference_axis, set_reference_axis, "ReferenceAxis", ref_);
    prop_pair!(target_axis, set_target_axis, "TargetAxis", ref_);
    prop_pair!(creation_mode, set_creation_mode, "CreationMode", bool);
    prop_pair!(volume_result, set_volume_result, "VolumeResult", bool);
}

impl HybridShape3DCurveOffset {
    prop_pair!(curve_to_offset, set_curve_to_offset, "CurveToOffset", ref_);
    prop_pair!(direction, set_direction, "Direction", ref_);
    prop_pair!(
        invert_direction,
        set_invert_direction,
        "InvertDirection",
        bool
    );
    prop_pair!(
        corner_tension_value,
        set_corner_tension_value,
        "CornerTensionValue",
        f64
    );

    /// 偏置值（`OffsetValue`；CAA 为 Length 参数对象，此处读数值）。
    pub fn offset_value(&self) -> ComResult<f64> {
        self.com().get("OffsetValue")?.as_f64()
    }

    /// 写偏置值（经 `OffsetValue` 属性）。
    pub fn set_offset_value(&self, value: f64) -> ComResult<()> {
        self.com().put("OffsetValue", Arg::F64(value))
    }

    /// 圆角半径值（`CornerRadiusValue`）。
    pub fn corner_radius_value(&self) -> ComResult<f64> {
        self.com().get("CornerRadiusValue")?.as_f64()
    }
}

impl HybridShapeBlend {
    prop_pair!(spine, set_spine, "Spine", ref_);
    prop_pair!(coupling, set_coupling, "Coupling", i32);
    prop_pair!(
        ruled_developable_surface,
        set_ruled_developable_surface,
        "RuledDevelopableSurface",
        bool
    );
    prop_pair!(
        smooth_angle_threshold_activity,
        set_smooth_angle_threshold_activity,
        "SmoothAngleThresholdActivity",
        bool
    );
    prop_pair!(
        smooth_deviation_activity,
        set_smooth_deviation_activity,
        "SmoothDeviationActivity",
        bool
    );

    /// 设置曲线（`SetCurve`；blend_limit: 1=首截面, 2=末截面）。
    pub fn set_curve(&self, blend_limit: i32, curve: &Reference) -> ComResult<()> {
        self.com()
            .call(
                "SetCurve",
                &[Arg::I32(blend_limit), Arg::Obj(curve.com().clone())],
            )
            .map(|_| ())
    }

    /// 取曲线（`GetCurve`）。
    pub fn get_curve(&self, blend_limit: i32) -> ComResult<Reference> {
        Ok(Reference::new(
            self.com()
                .call("GetCurve", &[Arg::I32(blend_limit)])?
                .as_obj()?,
        ))
    }
}

impl HybridShapeDevelop {
    prop_pair!(wire_to_develop, set_wire_to_develop, "WireToDevelop", ref_);
    prop_pair!(support, set_support, "Support", ref_);
    prop_pair!(
        point_on_support,
        set_point_on_support,
        "PointOnSupport",
        ref_
    );
    prop_pair!(positioned_wire, set_positioned_wire, "PositionedWire", ref_);
    prop_pair!(mode, set_mode, "Mode", i32);
    prop_pair!(mode_pos, set_mode_pos, "ModePos", i32);
}

impl HybridShapeUnfold {
    prop_pair!(
        surface_to_unfold,
        set_surface_to_unfold,
        "SurfaceToUnfold",
        ref_
    );
    prop_pair!(
        origin_to_unfold,
        set_origin_to_unfold,
        "OriginToUnfold",
        ref_
    );
    prop_pair!(
        direction_to_unfold,
        set_direction_to_unfold,
        "DirectionToUnfold",
        ref_
    );
    prop_pair!(target_plane, set_target_plane, "TargetPlane", ref_);
    prop_pair!(target_origin, set_target_origin, "TargetOrigin", ref_);
    prop_pair!(
        target_direction,
        set_target_direction,
        "TargetDirection",
        ref_
    );
    prop_pair!(surface_type, set_surface_type, "SurfaceType", i32);
    prop_pair!(
        target_orientation_mode,
        set_target_orientation_mode,
        "TargetOrientationMode",
        i32
    );
    prop_pair!(
        edge_to_tear_positioning_orientation,
        set_edge_to_tear_positioning_orientation,
        "EdgeToTearPositioningOrientation",
        i32
    );
}

impl HybridShapeTransfer {
    prop_pair!(
        element_to_transfer,
        set_element_to_transfer,
        "ElementToTransfer",
        ref_
    );
    prop_pair!(
        surface_to_unfold,
        set_surface_to_unfold,
        "SurfaceToUnfold",
        ref_
    );
    prop_pair!(
        unfolded_surface,
        set_unfolded_surface,
        "UnfoldedSurface",
        ref_
    );
    prop_pair!(
        type_of_transfer,
        set_type_of_transfer,
        "TypeOfTransfer",
        i32
    );
    prop_pair!(unfold_type, set_unfold_type, "UnfoldType", i32);
}

impl HybridShapeThickness {
    prop_pair!(orientation, set_orientation, "Orientation", i32);
    prop_get!(thickness1_value, "Thickness1Value", f64);
    prop_get!(thickness2_value, "Thickness2Value", f64);

    /// 厚度 1（`Thickness1`，数值）。
    pub fn thickness1(&self) -> ComResult<f64> {
        self.com().get("Thickness1")?.as_f64()
    }

    /// 厚度 2（`Thickness2`，数值）。
    pub fn thickness2(&self) -> ComResult<f64> {
        self.com().get("Thickness2")?.as_f64()
    }
}

impl HybridShapeWrapCurve {
    prop_pair!(surface, set_surface, "Surface", ref_);
    prop_pair!(
        first_curves_constraint,
        set_first_curves_constraint,
        "FirstCurvesConstraint",
        i32
    );
    prop_pair!(
        last_curves_constraint,
        set_last_curves_constraint,
        "LastCurvesConstraint",
        i32
    );
}

impl HybridShapeWrapSurface {
    prop_pair!(surface, set_surface, "Surface", ref_);
    prop_pair!(
        reference_surface,
        set_reference_surface,
        "ReferenceSurface",
        ref_
    );
    prop_pair!(target_surface, set_target_surface, "TargetSurface", ref_);
    prop_pair!(
        deformation_mode,
        set_deformation_mode,
        "DeformationMode",
        i32
    );
}

impl HybridShapeIntegratedLaw {
    prop_pair!(spine, set_spine, "Spine", ref_);
    prop_pair!(advanced_law, set_advanced_law, "AdvancedLaw", ref_);
    prop_pair!(pitch_law_type, set_pitch_law_type, "PitchLawType", i32);
    prop_pair!(
        implicit_law_interpolation_mode,
        set_implicit_law_interpolation_mode,
        "ImplicitLawInterpolationMode",
        i32
    );
    prop_pair!(
        invert_mapping_law,
        set_invert_mapping_law,
        "InvertMappingLaw",
        bool
    );
}

impl HybridShapeLawDistProj {
    prop_pair!(reference, set_reference, "Reference", ref_);
    prop_pair!(definition, set_definition, "Definition", ref_);
    prop_pair!(
        applied_unit_symbol,
        set_applied_unit_symbol,
        "AppliedUnitSymbol",
        str
    );
    prop_pair!(
        measure_unit_symbol,
        set_measure_unit_symbol,
        "MeasureUnitSymbol",
        str
    );
    prop_pair!(
        positive_direction_orientation,
        set_positive_direction_orientation,
        "PositiveDirectionOrientation",
        i32
    );
    prop_pair!(
        parameter_on_definition,
        set_parameter_on_definition,
        "ParameterOnDefinition",
        bool
    );
    prop_pair!(scaling, set_scaling, "Scaling", f64);
}

impl HybridShapePositionTransfo {
    prop_pair!(profile, set_profile, "Profile", ref_);
    prop_pair!(mode, set_mode, "Mode", i32);
    prop_pair!(
        initial_direction_computation_mode,
        set_initial_direction_computation_mode,
        "InitialDirectionComputationMode",
        i32
    );
}

impl HybridShapeReflectLine {
    prop_pair!(support, set_support, "Support", ref_);
    prop_pair!(origin, set_origin, "Origin", ref_);
    prop_pair!(direction, set_direction, "Direction", ref_);
    prop_pair!(angle, set_angle, "Angle", ref_);
    prop_pair!(source_type, set_source_type, "SourceType", i32);
    prop_pair!(
        orientation_support,
        set_orientation_support,
        "OrientationSupport",
        i32
    );
    prop_pair!(
        orientation_direction,
        set_orientation_direction,
        "OrientationDirection",
        i32
    );
    prop_pair!(type_solution, set_type_solution, "TypeSolution", i32);
}

impl HybridShapeRollingOffset {
    prop_pair!(support, set_support, "Support", ref_);

    /// 偏置（`GetOffset`）。
    pub fn get_offset(&self) -> ComResult<f64> {
        self.com().call("GetOffset", &[])?.as_f64()
    }

    /// 写偏置（`PutOffset`）。
    pub fn put_offset(&self, offset: f64) -> ComResult<()> {
        self.com()
            .call("PutOffset", &[Arg::F64(offset)])
            .map(|_| ())
    }

    /// 曲线数（`GetNBCurve`）。
    pub fn get_nb_curve(&self) -> ComResult<i32> {
        self.com().call("GetNBCurve", &[])?.as_i32()
    }
}

impl HybridShapeCurvePar {
    prop_pair!(curve_offseted, set_curve_offseted, "CurveOffseted", ref_);
    prop_pair!(support, set_support, "Support", ref_);
    prop_pair!(curve_par_law, set_curve_par_law, "CurveParLaw", ref_);
    prop_pair!(curve_par_type, set_curve_par_type, "CurveParType", i32);
    prop_pair!(law_type, set_law_type, "LawType", i32);
    prop_pair!(geodesic, set_geodesic, "Geodesic", bool);
    prop_pair!(
        invert_direction,
        set_invert_direction,
        "InvertDirection",
        bool
    );
    prop_pair!(
        invert_mapping_law,
        set_invert_mapping_law,
        "InvertMappingLaw",
        bool
    );
    prop_pair!(keep_both_sides, set_keep_both_sides, "KeepBothSides", bool);
    prop_pair!(other_side, set_other_side, "OtherSide", bool);
    prop_pair!(smoothing_type, set_smoothing_type, "SmoothingType", i32);
    prop_pair!(
        maximum_deviation_value,
        set_maximum_deviation_value,
        "MaximumDeviationValue",
        f64
    );
    prop_pair!(p_3d_smoothing, set_p_3d_smoothing, "P3DSmoothing", bool);
}

// ── 圆锥曲线 / 扫掠 / 圆角 / 修复 / 显式 datum ──────────────────────

com_wrapper!(HybridShapeConic);
com_wrapper!(HybridShapeFilletBiTangent);
com_wrapper!(HybridShapeFilletTriTangent);
com_wrapper!(HybridShapeHealing);
com_wrapper!(HybridShapeSweepCircle);
com_wrapper!(HybridShapeSweepConic);
com_wrapper!(HybridShapeSweepExplicit);
com_wrapper!(HybridShapeSweepLine);
com_wrapper!(HybridShapeCurveExplicit);
com_wrapper!(HybridShapePlaneExplicit);
com_wrapper!(HybridShapeSurfaceExplicit);
com_wrapper!(HybridShapeVolumeExplicit);

impl HybridShapeConic {
    prop_pair!(start_point, set_start_point, "StartPoint", ref_);
    prop_pair!(end_point, set_end_point, "EndPoint", ref_);
    prop_pair!(start_tangent, set_start_tangent, "StartTangent", ref_);
    prop_pair!(end_tangent, set_end_tangent, "EndTangent", ref_);
    prop_pair!(
        tangent_int_point,
        set_tangent_int_point,
        "TangentIntPoint",
        ref_
    );
    prop_pair!(support_plane, set_support_plane, "SupportPlane", ref_);
    prop_pair!(conic_parameter, set_conic_parameter, "ConicParameter", f64);
}

impl HybridShapeFilletBiTangent {
    prop_pair!(first_elem, set_first_elem, "FirstElem", ref_);
    prop_pair!(second_elem, set_second_elem, "SecondElem", ref_);
    prop_pair!(spine, set_spine, "Spine", ref_);
    prop_pair!(hold_curve, set_hold_curve, "HoldCurve", ref_);
    prop_pair!(
        first_orientation,
        set_first_orientation,
        "FirstOrientation",
        i32
    );
    prop_pair!(
        second_orientation,
        set_second_orientation,
        "SecondOrientation",
        i32
    );
    prop_pair!(radius_type, set_radius_type, "RadiusType", i32);
    prop_pair!(radius_value, set_radius_value, "RadiusValue", f64);
    prop_pair!(section_type, set_section_type, "SectionType", i32);
    prop_pair!(
        conical_section_parameter,
        set_conical_section_parameter,
        "ConicalSectionParameter",
        f64
    );
    prop_pair!(
        supports_trim_mode,
        set_supports_trim_mode,
        "SupportsTrimMode",
        i32
    );
    prop_pair!(
        ribbon_relimitation_mode,
        set_ribbon_relimitation_mode,
        "RibbonRelimitationMode",
        i32
    );
}

impl HybridShapeFilletTriTangent {
    prop_pair!(first_elem, set_first_elem, "FirstElem", ref_);
    prop_pair!(second_elem, set_second_elem, "SecondElem", ref_);
    prop_pair!(remove_elem, set_remove_elem, "RemoveElem", ref_);
    prop_pair!(
        first_orientation,
        set_first_orientation,
        "FirstOrientation",
        i32
    );
    prop_pair!(
        second_orientation,
        set_second_orientation,
        "SecondOrientation",
        i32
    );
    prop_pair!(
        remove_orientation,
        set_remove_orientation,
        "RemoveOrientation",
        i32
    );
    prop_pair!(
        supports_trim_mode,
        set_supports_trim_mode,
        "SupportsTrimMode",
        i32
    );
    prop_pair!(
        ribbon_relimitation_mode,
        set_ribbon_relimitation_mode,
        "RibbonRelimitationMode",
        i32
    );

    /// 反转第一元素方向（`InvertFirstOrientation`）。
    pub fn invert_first_orientation(&self) -> ComResult<()> {
        self.com().call("InvertFirstOrientation", &[]).map(|_| ())
    }

    /// 反转移除元素方向（`InvertRemoveOrientation`）。
    pub fn invert_remove_orientation(&self) -> ComResult<()> {
        self.com().call("InvertRemoveOrientation", &[]).map(|_| ())
    }

    /// 反转第二元素方向（`InvertSecondOrientation`）。
    pub fn invert_second_orientation(&self) -> ComResult<()> {
        self.com().call("InvertSecondOrientation", &[]).map(|_| ())
    }
}

impl HybridShapeHealing {
    prop_pair!(continuity, set_continuity, "Continuity", i32);

    /// 追加待修复体（`AddBodyToHeal`）。
    pub fn add_body_to_heal(&self, body: &Reference) -> ComResult<()> {
        self.com()
            .call("AddBodyToHeal", &[Arg::Obj(body.com().clone())])
            .map(|_| ())
    }

    /// 追加冻结元素（`AddElementsToFreeze`）。
    pub fn add_elements_to_freeze(&self, element: &Reference) -> ComResult<()> {
        self.com()
            .call("AddElementsToFreeze", &[Arg::Obj(element.com().clone())])
            .map(|_| ())
    }

    /// 追加保持尖锐的边（`AddEdgeToKeepSharp`）。
    pub fn add_edge_to_keep_sharp(&self, edge: &Reference) -> ComResult<()> {
        self.com()
            .call("AddEdgeToKeepSharp", &[Arg::Obj(edge.com().clone())])
            .map(|_| ())
    }

    /// 写合并距离（`SetMergingDistance`）。
    pub fn set_merging_distance(&self, value: f64) -> ComResult<()> {
        self.com()
            .call("SetMergingDistance", &[Arg::F64(value)])
            .map(|_| ())
    }

    /// 写距离目标（`SetDistanceObjective`）。
    pub fn set_distance_objective(&self, value: f64) -> ComResult<()> {
        self.com()
            .call("SetDistanceObjective", &[Arg::F64(value)])
            .map(|_| ())
    }

    /// 写相切目标（`SetTangencyObjective`）。
    pub fn set_tangency_objective(&self, value: f64) -> ComResult<()> {
        self.com()
            .call("SetTangencyObjective", &[Arg::F64(value)])
            .map(|_| ())
    }
}

impl HybridShapeSweepCircle {
    prop_pair!(first_guide_crv, set_first_guide_crv, "FirstGuideCrv", ref_);
    prop_pair!(
        second_guide_crv,
        set_second_guide_crv,
        "SecondGuideCrv",
        ref_
    );
    prop_pair!(third_guide_crv, set_third_guide_crv, "ThirdGuideCrv", ref_);
    prop_pair!(reference, set_reference, "Reference", ref_);
    prop_pair!(spine, set_spine, "Spine", ref_);
    prop_pair!(mode, set_mode, "Mode", i32);
    prop_pair!(choice_no, set_choice_no, "ChoiceNo", i32);
    prop_pair!(context, set_context, "Context", i32);
    prop_pair!(
        canonical_detection,
        set_canonical_detection,
        "CanonicalDetection",
        i32
    );
    prop_pair!(trim_option, set_trim_option, "TrimOption", i32);
    prop_pair!(
        guide_deviation_activity,
        set_guide_deviation_activity,
        "GuideDeviationActivity",
        bool
    );
    prop_pair!(smooth_activity, set_smooth_activity, "SmoothActivity", bool);
    prop_pair!(radius_law_type, set_radius_law_type, "RadiusLawType", i32);

    /// 按序号写半径（`SetRadius`）。
    pub fn set_radius(&self, index: i32, radius: f64) -> ComResult<()> {
        self.com()
            .call("SetRadius", &[Arg::I32(index), Arg::F64(radius)])
            .map(|_| ())
    }

    /// 按序号写角度（`SetAngle`）。
    pub fn set_angle(&self, index: i32, angle: f64) -> ComResult<()> {
        self.com()
            .call("SetAngle", &[Arg::I32(index), Arg::F64(angle)])
            .map(|_| ())
    }
}

impl HybridShapeSweepConic {
    prop_pair!(first_guide_crv, set_first_guide_crv, "FirstGuideCrv", ref_);
    prop_pair!(
        second_guide_crv,
        set_second_guide_crv,
        "SecondGuideCrv",
        ref_
    );
    prop_pair!(third_guide_crv, set_third_guide_crv, "ThirdGuideCrv", ref_);
    prop_pair!(
        fourth_guide_crv,
        set_fourth_guide_crv,
        "FourthGuideCrv",
        ref_
    );
    prop_pair!(fifth_guide_crv, set_fifth_guide_crv, "FifthGuideCrv", ref_);
    prop_pair!(spine, set_spine, "Spine", ref_);
    prop_pair!(
        canonical_detection,
        set_canonical_detection,
        "CanonicalDetection",
        i32
    );
    prop_pair!(
        guide_deviation_activity,
        set_guide_deviation_activity,
        "GuideDeviationActivity",
        bool
    );
    prop_pair!(smooth_activity, set_smooth_activity, "SmoothActivity", bool);
    prop_pair!(parameter, set_parameter, "Parameter", f64);
    prop_pair!(
        parameter_law_type,
        set_parameter_law_type,
        "ParameterLawType",
        i32
    );
}

impl HybridShapeSweepExplicit {
    prop_pair!(profile, set_profile, "Profile", ref_);
    prop_pair!(
        positioned_profile,
        set_positioned_profile,
        "PositionedProfile",
        ref_
    );
    prop_pair!(first_guide_crv, set_first_guide_crv, "FirstGuideCrv", ref_);
    prop_pair!(
        second_guide_crv,
        set_second_guide_crv,
        "SecondGuideCrv",
        ref_
    );
    prop_pair!(reference, set_reference, "Reference", ref_);
    prop_pair!(spine, set_spine, "Spine", ref_);
    prop_pair!(
        pulling_direction,
        set_pulling_direction,
        "PullingDirection",
        ref_
    );
    prop_pair!(mode, set_mode, "Mode", i32);
    prop_pair!(position_mode, set_position_mode, "PositionMode", i32);
    prop_pair!(sub_type, set_sub_type, "SubType", i32);
    prop_pair!(solution_no, set_solution_no, "SolutionNo", i32);
    prop_pair!(context, set_context, "Context", i32);
    prop_pair!(
        canonical_detection,
        set_canonical_detection,
        "CanonicalDetection",
        i32
    );
    prop_pair!(
        guide_projection,
        set_guide_projection,
        "GuideProjection",
        bool
    );
    prop_pair!(
        profile_x_axis_computation_mode,
        set_profile_x_axis_computation_mode,
        "ProfileXAxisComputationMode",
        i32
    );
    prop_pair!(angle_law_type, set_angle_law_type, "AngleLawType", i32);
    prop_pair!(
        guide_deviation_activity,
        set_guide_deviation_activity,
        "GuideDeviationActivity",
        bool
    );
    prop_pair!(smooth_activity, set_smooth_activity, "SmoothActivity", bool);

    /// 是否用草图轴作默认（`UseSketchAxisAsDefault`）。
    pub fn use_sketch_axis_as_default(&self, value: bool) -> ComResult<()> {
        self.com()
            .call("UseSketchAxisAsDefault", &[Arg::Bool(value)])
            .map(|_| ())
    }
}

impl HybridShapeSweepLine {
    prop_pair!(first_guide_crv, set_first_guide_crv, "FirstGuideCrv", ref_);
    prop_pair!(
        second_guide_crv,
        set_second_guide_crv,
        "SecondGuideCrv",
        ref_
    );
    prop_pair!(
        first_guide_surf,
        set_first_guide_surf,
        "FirstGuideSurf",
        ref_
    );
    prop_pair!(
        second_guide_surf,
        set_second_guide_surf,
        "SecondGuideSurf",
        ref_
    );
    prop_pair!(spine, set_spine, "Spine", ref_);
    prop_pair!(draft_direction, set_draft_direction, "DraftDirection", ref_);
    prop_pair!(mode, set_mode, "Mode", i32);
    prop_pair!(choice_no, set_choice_no, "ChoiceNo", i32);
    prop_pair!(context, set_context, "Context", i32);
    prop_pair!(
        canonical_detection,
        set_canonical_detection,
        "CanonicalDetection",
        i32
    );
    prop_pair!(trim_option, set_trim_option, "TrimOption", i32);
    prop_pair!(
        second_trim_option,
        set_second_trim_option,
        "SecondTrimOption",
        i32
    );
    prop_pair!(
        draft_computation_mode,
        set_draft_computation_mode,
        "DraftComputationMode",
        i32
    );
    prop_pair!(angle_law_type, set_angle_law_type, "AngleLawType", i32);
    prop_pair!(
        guide_deviation_activity,
        set_guide_deviation_activity,
        "GuideDeviationActivity",
        bool
    );
    prop_pair!(smooth_activity, set_smooth_activity, "SmoothActivity", bool);
    prop_pair!(solution_no, set_solution_no, "SolutionNo", i32);

    /// 按序号写长度（`SetLength`）。
    pub fn set_length(&self, index: i32, length: f64) -> ComResult<()> {
        self.com()
            .call("SetLength", &[Arg::I32(index), Arg::F64(length)])
            .map(|_| ())
    }

    /// 按序号写角度（`SetAngle`）。
    pub fn set_angle(&self, index: i32, angle: f64) -> ComResult<()> {
        self.com()
            .call("SetAngle", &[Arg::I32(index), Arg::F64(angle)])
            .map(|_| ())
    }
}
