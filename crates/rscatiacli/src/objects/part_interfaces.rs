//! Part Design 特征族（pycatia `part_interfaces` 对齐）。
//!
//! 覆盖 68 个类：基于草图的特征（Prism/Pad/Pocket、Revolution/Shaft/Groove、
//! Sweep/Rib/Slot、Hole、Stiffener、SolidCombine、Loft）、修饰特征（Fillet 族、
//! Chamfer、Shell、Thickness、Thread、Draft 族、AutoFillet/AutoDraft）、布尔运算
//! （Add/Remove/Intersect/Assemble/Trim）、变换（Mirror/Rotate/Translate/Scaling
//! 族/Affinity/Symmetry/AxisToAxis）、阵列（RectPattern/CircPattern/UserPattern +
//! Repartition 族）。
//!
//! 成员口径与 pycatia 暴露面一致：属性读写 + 关键方法；pycatia 没暴露的成员走
//! `ComObject::com()` 逃生舱按名派发。所有特征经 `ShapeFactory::add_new_*` 创建
//! （见 [`super::shape_factory`]）。
//!
//! 命名备注：`Hole` 的 `Type` 属性是 Rust 保留字，方法名为 `hole_type` /
//! `set_hole_type`；变换类的 `HybridShape` 属性（底层 GSD 对象）返回原始
//! [`ComObj`]，对应 pycatia 的 `hybrid_shape()`，具体类型待阶段 6 补齐。

use crate::collections::ComCollection;
use crate::dispatch::{Arg, ComObj, ComObject};
use crate::error::ComResult;

use super::part::Body;
use super::reference::Reference;
use super::sketch::Sketch;
use super::system_service::SystemService;

/// 成员列表读取：属性优先、方法兜底（CATIA 版本差异，与 `item_at` 同思路）。
fn member_ref_array(obj: &ComObj, name: &str) -> ComResult<Vec<Reference>> {
    let raw = obj.get(name).and_then(|v| v.as_obj_array()).or_else(|e| {
        if e.is_member_missing() {
            obj.call(name, &[]).and_then(|v| v.as_obj_array())
        } else {
            Err(e)
        }
    })?;
    Ok(raw.into_iter().map(Reference::new).collect())
}

/// 成员列表读取（原始对象版）。
fn member_obj_array(obj: &ComObj, name: &str) -> ComResult<Vec<ComObj>> {
    obj.get(name).and_then(|v| v.as_obj_array()).or_else(|e| {
        if e.is_member_missing() {
            obj.call(name, &[]).and_then(|v| v.as_obj_array())
        } else {
            Err(e)
        }
    })
}

/// 底层 GSD 对象（变换类的 `HybridShape` 属性；具体类型阶段 6 补齐）。
fn hybrid_shape(obj: &ComObj) -> ComResult<ComObj> {
    obj.get("HybridShape")?.as_obj()
}

// ---------------------------------------------------------------------------
// 基类
// ---------------------------------------------------------------------------

com_wrapper!(SketchBasedShape);
com_wrapper!(DressUpShape);
com_wrapper!(SurfaceBasedShape);
com_wrapper!(BooleanShape);
com_wrapper!(TransformationShape);
com_wrapper!(Limit);

impl SketchBasedShape {
    /// 轮廓草图（`Sketch`，只读）。
    pub fn sketch(&self) -> ComResult<Sketch> {
        Ok(Sketch::new(self.com().get("Sketch")?.as_obj()?))
    }

    /// 替换轮廓元素（`SetProfileElement`）。
    pub fn set_profile_element(&self, profile: &Reference) -> ComResult<()> {
        self.com()
            .call("SetProfileElement", &[Arg::Obj(profile.com().clone())])
            .map(|_| ())
    }
}

impl SurfaceBasedShape {
    prop_pair!(surface, set_surface, "Surface", ref_);
}

impl BooleanShape {
    /// 运算结果所在的 `Body`（只读）。
    pub fn body(&self) -> ComResult<Body> {
        Ok(Body::new(self.com().get("Body")?.as_obj()?))
    }

    /// 指定被运算对象（`SetOperatedObject`）。
    pub fn set_operated_object(&self, reference: &Reference) -> ComResult<()> {
        self.com()
            .call("SetOperatedObject", &[Arg::Obj(reference.com().clone())])
            .map(|_| ())
    }

    /// 指定运算体（`SetOperatingVolume`）。
    pub fn set_operating_volume(&self, reference: &Reference) -> ComResult<()> {
        self.com()
            .call("SetOperatingVolume", &[Arg::Obj(reference.com().clone())])
            .map(|_| ())
    }
}

impl Limit {
    prop_get!(dimension, "Dimension", i32);
    prop_pair!(limit_mode, set_limit_mode, "LimitMode", i32);
    prop_pair!(
        limiting_element,
        set_limiting_element,
        "LimitingElement",
        ref_
    );
}

// ---------------------------------------------------------------------------
// 波次 1 —— 基于草图的特征
// ---------------------------------------------------------------------------

com_wrapper!(Prism);
com_wrapper!(Pad);
com_wrapper!(Pocket);
com_wrapper!(Revolution);
com_wrapper!(Shaft);
com_wrapper!(Groove);
com_wrapper!(Sweep);
com_wrapper!(Rib);
com_wrapper!(Slot);
com_wrapper!(Hole);
com_wrapper!(Stiffener);
com_wrapper!(SolidCombine);
com_wrapper!(Loft);

impl Prism {
    prop_pair!(
        direction_orientation,
        set_direction_orientation,
        "DirectionOrientation",
        i32
    );
    prop_pair!(direction_type, set_direction_type, "DirectionType", i32);
    prop_pair!(is_symmetric, set_is_symmetric, "IsSymmetric", bool);
    prop_pair!(is_thin, set_is_thin, "IsThin", bool);
    prop_pair!(merge_end, set_merge_end, "MergeEnd", bool);
    prop_pair!(neutral_fiber, set_neutral_fiber, "NeutralFiber", bool);

    /// 第一限制（`FirstLimit`）。
    pub fn first_limit(&self) -> ComResult<Limit> {
        Ok(Limit::new(self.com().get("FirstLimit")?.as_obj()?))
    }

    /// 第二限制（`SecondLimit`）。
    pub fn second_limit(&self) -> ComResult<Limit> {
        Ok(Limit::new(self.com().get("SecondLimit")?.as_obj()?))
    }

    /// 反转内侧面（`ReverseInnerSide`）。
    pub fn reverse_inner_side(&self) -> ComResult<()> {
        self.com().call("ReverseInnerSide", &[]).map(|_| ())
    }

    /// 指定拉伸方向直线（`SetDirection`）。
    pub fn set_direction(&self, line: &Reference) -> ComResult<()> {
        self.com()
            .call("SetDirection", &[Arg::Obj(line.com().clone())])
            .map(|_| ())
    }

    /// 方向分量（`GetDirection`，VBA 中转）。
    pub fn get_direction(&self, ss: &SystemService) -> ComResult<[f64; 3]> {
        ss.direction_out_param(self.com(), "GetDirection")
    }
}

impl Revolution {
    prop_get!(first_angle, "FirstAngle", f64);
    prop_get!(second_angle, "SecondAngle", f64);
    prop_pair!(revolute_axis, set_revolute_axis, "RevoluteAxis", ref_);
    prop_pair!(is_thin, set_is_thin, "IsThin", bool);
    prop_pair!(merge_end, set_merge_end, "MergeEnd", bool);
    prop_pair!(neutral_fiber, set_neutral_fiber, "NeutralFiber", bool);
}

impl Sweep {
    prop_pair!(
        anchor_dir_reverse,
        set_anchor_dir_reverse,
        "AnchorDirReverse",
        bool
    );
    prop_pair!(
        center_curve_element,
        set_center_curve_element,
        "CenterCurveElement",
        ref_
    );
    prop_pair!(is_thin, set_is_thin, "IsThin", bool);
    prop_pair!(merge_end, set_merge_end, "MergeEnd", bool);
    prop_pair!(merge_mode, set_merge_mode, "MergeMode", i32);
    prop_pair!(
        move_profile_to_path,
        set_move_profile_to_path,
        "MoveProfileToPath",
        bool
    );
    prop_pair!(neutral_fiber, set_neutral_fiber, "NeutralFiber", bool);
    prop_pair!(
        normal_axis_dir_reverse,
        set_normal_axis_dir_reverse,
        "NormalAxisDirReverse",
        bool
    );
    prop_pair!(
        pulling_dir_element,
        set_pulling_dir_element,
        "PullingDirElement",
        ref_
    );
    prop_pair!(
        reference_surface_element,
        set_reference_surface_element,
        "ReferenceSurfaceElement",
        ref_
    );

    /// 中心曲线草图（`CenterCurve`，只读）。
    pub fn center_curve(&self) -> ComResult<Sketch> {
        Ok(Sketch::new(self.com().get("CenterCurve")?.as_obj()?))
    }

    /// 保持角度选项（`SetKeepAngleOption`）。
    pub fn set_keep_angle_option(&self) -> ComResult<()> {
        self.com().call("SetKeepAngleOption", &[]).map(|_| ())
    }
}

impl Hole {
    prop_pair!(anchor_mode, set_anchor_mode, "AnchorMode", i32);
    prop_pair!(bottom_type, set_bottom_type, "BottomType", i32);
    prop_pair!(
        counter_drilled_mode,
        set_counter_drilled_mode,
        "CounterDrilledMode",
        i32
    );
    prop_pair!(
        counter_sunk_mode,
        set_counter_sunk_mode,
        "CounterSunkMode",
        i32
    );
    prop_pair!(thread_side, set_thread_side, "ThreadSide", i32);
    prop_pair!(threading_mode, set_threading_mode, "ThreadingMode", i32);
    prop_get!(bottom_angle, "BottomAngle", f64);
    prop_get!(diameter, "Diameter", f64);
    prop_get!(head_angle, "HeadAngle", f64);
    prop_get!(head_depth, "HeadDepth", f64);
    prop_get!(head_diameter, "HeadDiameter", f64);
    prop_get!(thread_depth, "ThreadDepth", f64);
    prop_get!(thread_diameter, "ThreadDiameter", f64);
    prop_get!(thread_pitch, "ThreadPitch", f64);

    /// 孔类型（`Type` 属性；Rust 保留字故方法名带 hole 前缀）。
    pub fn hole_type(&self) -> ComResult<i32> {
        self.com().get("Type")?.as_i32()
    }

    /// 设置孔类型（`Type` 属性）。
    pub fn set_hole_type(&self, value: i32) -> ComResult<()> {
        self.com().put("Type", Arg::I32(value))
    }

    /// 孔底限制（`BottomLimit`）。
    pub fn bottom_limit(&self) -> ComResult<Limit> {
        Ok(Limit::new(self.com().get("BottomLimit")?.as_obj()?))
    }

    /// 建标准螺纹设计表（`CreateStandardThreadDesignTable`）。
    pub fn create_standard_thread_design_table(&self, standard_type: i32) -> ComResult<()> {
        self.com()
            .call(
                "CreateStandardThreadDesignTable",
                &[Arg::I32(standard_type)],
            )
            .map(|_| ())
    }

    /// 建用户标准螺纹设计表（`CreateUserStandardDesignTable`）。
    pub fn create_user_standard_design_table(
        &self,
        standard_name: &str,
        path: &str,
    ) -> ComResult<()> {
        self.com()
            .call(
                "CreateUserStandardDesignTable",
                &[
                    Arg::Str(standard_name.to_string()),
                    Arg::Str(path.to_string()),
                ],
            )
            .map(|_| ())
    }

    /// 孔轴方向分量（`GetDirection`，VBA 中转）。
    pub fn get_direction(&self, ss: &SystemService) -> ComResult<[f64; 3]> {
        ss.direction_out_param(self.com(), "GetDirection")
    }

    /// 孔原点分量（`GetOrigin`，VBA 中转）。
    pub fn get_origin(&self, ss: &SystemService) -> ComResult<[f64; 3]> {
        ss.direction_out_param(self.com(), "GetOrigin")
    }

    /// 反转孔方向（`Reverse`）。
    pub fn reverse(&self) -> ComResult<()> {
        self.com().call("Reverse", &[]).map(|_| ())
    }

    /// 指定孔轴方向直线（`SetDirection`）。
    pub fn set_direction(&self, line: &Reference) -> ComResult<()> {
        self.com()
            .call("SetDirection", &[Arg::Obj(line.com().clone())])
            .map(|_| ())
    }

    /// 指定孔原点（`SetOrigin`）。
    pub fn set_origin(&self, x: f64, y: f64, z: f64) -> ComResult<()> {
        self.com()
            .call("SetOrigin", &[Arg::F64(x), Arg::F64(y), Arg::F64(z)])
            .map(|_| ())
    }
}

impl Stiffener {
    prop_pair!(is_from_top, set_is_from_top, "IsFromTop", bool);
    prop_pair!(is_symmetric, set_is_symmetric, "IsSymmetric", bool);
    prop_get!(thickness, "Thickness", f64);
    prop_get!(thickness_from_top, "ThicknessFromTop", f64);

    /// 反转深度（`ReverseDepth`）。
    pub fn reverse_depth(&self) -> ComResult<()> {
        self.com().call("ReverseDepth", &[]).map(|_| ())
    }

    /// 反转厚度（`ReverseThickness`）。
    pub fn reverse_thickness(&self) -> ComResult<()> {
        self.com().call("ReverseThickness", &[]).map(|_| ())
    }
}

impl SolidCombine {
    prop_pair!(
        first_component_direction,
        set_first_component_direction,
        "FirstComponentDirection",
        ref_
    );
    prop_pair!(
        first_component_profile,
        set_first_component_profile,
        "FirstComponentProfile",
        ref_
    );
    prop_pair!(
        second_component_direction,
        set_second_component_direction,
        "SecondComponentDirection",
        ref_
    );
    prop_pair!(
        second_component_profile,
        set_second_component_profile,
        "SecondComponentProfile",
        ref_
    );
}

impl Loft {
    /// 底层 GSD 放样对象（`HybridShape`）。
    pub fn hybrid_shape(&self) -> ComResult<ComObj> {
        hybrid_shape(self.com())
    }
}

// ---------------------------------------------------------------------------
// 波次 2 —— 修饰特征
// ---------------------------------------------------------------------------

com_wrapper!(Fillet);
com_wrapper!(EdgeFillet);
com_wrapper!(ConstRadEdgeFillet);
com_wrapper!(VarRadEdgeFillet);
com_wrapper!(FaceFillet);
com_wrapper!(TritangentFillet);
com_wrapper!(AutoFillet);
com_wrapper!(Chamfer);
com_wrapper!(Shell);
com_wrapper!(Thickness);
com_wrapper!(Thread);
com_wrapper!(Draft);
com_wrapper!(DraftDomains);
com_wrapper!(DraftDomain);
com_wrapper!(AutoDraft);
com_wrapper!(Defeaturing);
com_wrapper!(DefeaturingFilters);
com_wrapper!(DefeaturingFilter);
com_wrapper!(DefeaturingFilterWithRange);
com_wrapper!(DefeaturingFilletFilter);
com_wrapper!(DefeaturingHoleFilter);

impl Fillet {
    prop_pair!(
        fillet_boundary_relimitation,
        set_fillet_boundary_relimitation,
        "FilletBoundaryRelimitation",
        i32
    );
    prop_pair!(
        fillet_trim_support,
        set_fillet_trim_support,
        "FilletTrimSupport",
        i32
    );
}

impl EdgeFillet {
    prop_pair!(
        edge_propagation,
        set_edge_propagation,
        "EdgePropagation",
        i32
    );

    /// 保留边列表（`EdgesToKeep`）。
    pub fn edges_to_keep(&self) -> ComResult<Vec<Reference>> {
        member_ref_array(self.com(), "EdgesToKeep")
    }

    /// 追加保留边（`AddEdgeToKeep`）。
    pub fn add_edge_to_keep(&self, edge: &Reference) -> ComResult<()> {
        self.com()
            .call("AddEdgeToKeep", &[Arg::Obj(edge.com().clone())])
            .map(|_| ())
    }

    /// 撤销保留边（`WithdrawEdgeToKeep`）。
    pub fn withdraw_edge_to_keep(&self, edge: &Reference) -> ComResult<()> {
        self.com()
            .call("WithdrawEdgeToKeep", &[Arg::Obj(edge.com().clone())])
            .map(|_| ())
    }
}

impl ConstRadEdgeFillet {
    /// 倒圆对象列表（`ObjectsToFillet`）。
    pub fn objects_to_fillet(&self) -> ComResult<Vec<Reference>> {
        member_ref_array(self.com(), "ObjectsToFillet")
    }

    prop_get!(radius, "Radius", f64);

    /// 追加倒圆对象（`AddObjectToFillet`）。
    pub fn add_object_to_fillet(&self, object: &Reference) -> ComResult<()> {
        self.com()
            .call("AddObjectToFillet", &[Arg::Obj(object.com().clone())])
            .map(|_| ())
    }

    /// 撤销倒圆对象（`WithdrawObjectToFillet`）。
    pub fn withdraw_object_to_fillet(&self, object: &Reference) -> ComResult<()> {
        self.com()
            .call("WithdrawObjectToFillet", &[Arg::Obj(object.com().clone())])
            .map(|_| ())
    }

    /// 切换为变半径倒圆（`SwitchToVarFilletType`）。
    pub fn switch_to_var_fillet_type(&self) -> ComResult<()> {
        self.com().call("SwitchToVarFilletType", &[]).map(|_| ())
    }
}

impl VarRadEdgeFillet {
    prop_pair!(bitangency_type, set_bitangency_type, "BitangencyType", i32);
    prop_pair!(fillet_spine, set_fillet_spine, "FilletSpine", ref_);
    prop_pair!(
        fillet_variation,
        set_fillet_variation,
        "FilletVariation",
        i32
    );
    prop_pair!(
        sharp_edge_removal_mode,
        set_sharp_edge_removal_mode,
        "SharpEdgeRemovalMode",
        i32
    );

    /// 倒圆边列表（`EdgesToFillet`）。
    pub fn edges_to_fillet(&self) -> ComResult<Vec<Reference>> {
        member_ref_array(self.com(), "EdgesToFillet")
    }

    /// 施加顶点列表（`ImposedVertices`）。
    pub fn imposed_vertices(&self) -> ComResult<Vec<Reference>> {
        member_ref_array(self.com(), "ImposedVertices")
    }

    /// 追加倒圆边并设半径（`AddEdgeToFillet`）。
    pub fn add_edge_to_fillet(&self, edge: &Reference, radius: f64) -> ComResult<()> {
        self.com()
            .call(
                "AddEdgeToFillet",
                &[Arg::Obj(edge.com().clone()), Arg::F64(radius)],
            )
            .map(|_| ())
    }

    /// 追加施加顶点并设半径（`AddImposedVertex`）。
    pub fn add_imposed_vertex(&self, vertex: &Reference, radius: f64) -> ComResult<()> {
        self.com()
            .call(
                "AddImposedVertex",
                &[Arg::Obj(vertex.com().clone()), Arg::F64(radius)],
            )
            .map(|_| ())
    }

    /// 读施加顶点半径（`ImposedVertexRadius`）。
    pub fn imposed_vertex_radius(&self, vertex: &Reference) -> ComResult<f64> {
        self.com()
            .call("ImposedVertexRadius", &[Arg::Obj(vertex.com().clone())])?
            .as_f64()
    }

    /// 切换为等半径倒圆（`SwitchToConstFilletType`）。
    pub fn switch_to_const_fillet_type(&self) -> ComResult<()> {
        self.com().call("SwitchToConstFilletType", &[]).map(|_| ())
    }

    /// 撤销倒圆边（`WithdrawEdgeToFillet`）。
    pub fn withdraw_edge_to_fillet(&self, edge: &Reference) -> ComResult<()> {
        self.com()
            .call("WithdrawEdgeToFillet", &[Arg::Obj(edge.com().clone())])
            .map(|_| ())
    }

    /// 撤销施加顶点（`WithdrawImposedVertex`）。
    pub fn withdraw_imposed_vertex(&self, vertex: &Reference) -> ComResult<()> {
        self.com()
            .call("WithdrawImposedVertex", &[Arg::Obj(vertex.com().clone())])
            .map(|_| ())
    }
}

impl FaceFillet {
    prop_pair!(first_face, set_first_face, "FirstFace", ref_);
    prop_pair!(second_face, set_second_face, "SecondFace", ref_);
    prop_get!(radius, "Radius", f64);
}

impl TritangentFillet {
    prop_pair!(face_to_remove, set_face_to_remove, "FaceToRemove", ref_);
    prop_pair!(first_face, set_first_face, "FirstFace", ref_);
    prop_pair!(second_face, set_second_face, "SecondFace", ref_);
}

impl AutoFillet {
    prop_get!(curvature_radius, "CurvatureRadius", f64);
    prop_get!(fillet_radius, "FilletRadius", f64);
    prop_get!(round_radius, "RoundRadius", f64);
    prop_pair!(
        round_radius_activation,
        set_round_radius_activation,
        "RoundRadiusActivation",
        bool
    );
    prop_pair!(functional_face, set_functional_face, "FunctionalFace", ref_);
    prop_pair!(
        slivers_and_crack,
        set_slivers_and_crack,
        "SliversAndCrack",
        ref_
    );
    prop_pair!(support_surface, set_support_surface, "SupportSurface", ref_);
    prop_get!(parting_element, "PartingElement", ref_);

    /// 功能面列表（`FunctionalFaces`）。
    pub fn functional_faces(&self) -> ComResult<Vec<Reference>> {
        member_ref_array(self.com(), "FunctionalFaces")
    }

    /// 待倒圆面列表（`FacesToFillets`）。
    pub fn faces_to_fillets(&self) -> ComResult<Vec<Reference>> {
        member_ref_array(self.com(), "FacesToFillets")
    }

    /// 细缝与裂纹列表（`SliversAndCracks`）。
    pub fn slivers_and_cracks(&self) -> ComResult<Vec<Reference>> {
        member_ref_array(self.com(), "SliversAndCracks")
    }
}

impl Chamfer {
    prop_get!(angle, "Angle", f64);
    prop_get!(length1, "Length1", f64);
    prop_get!(length2, "Length2", f64);
    prop_pair!(mode, set_mode, "Mode", i32);
    prop_pair!(orientation, set_orientation, "Orientation", i32);
    prop_pair!(propagation, set_propagation, "Propagation", i32);

    /// 倒角元素列表（`ElementsToChamfer`）。
    pub fn elements_to_chamfer(&self) -> ComResult<Vec<Reference>> {
        member_ref_array(self.com(), "ElementsToChamfer")
    }

    /// 追加倒角元素（`AddElementToChamfer`）。
    pub fn add_element_to_chamfer(&self, element: &Reference) -> ComResult<()> {
        self.com()
            .call("AddElementToChamfer", &[Arg::Obj(element.com().clone())])
            .map(|_| ())
    }

    /// 撤销倒角元素（`WithdrawElementToChamfer`）。
    pub fn withdraw_element_to_chamfer(&self, element: &Reference) -> ComResult<()> {
        self.com()
            .call(
                "WithdrawElementToChamfer",
                &[Arg::Obj(element.com().clone())],
            )
            .map(|_| ())
    }
}

impl Shell {
    prop_get!(external_thickness, "ExternalThickness", f64);
    prop_get!(internal_thickness, "InternalThickness", f64);

    /// 移除面列表（`FacesToRemove`）。
    pub fn faces_to_remove(&self) -> ComResult<Vec<Reference>> {
        member_ref_array(self.com(), "FacesToRemove")
    }

    /// 追加移除面（`AddFaceToRemove`）。
    pub fn add_face_to_remove(&self, face: &Reference) -> ComResult<()> {
        self.com()
            .call("AddFaceToRemove", &[Arg::Obj(face.com().clone())])
            .map(|_| ())
    }

    /// 追加不同厚度的加厚面（`AddFaceWithDifferentThickness`）。
    pub fn add_face_with_different_thickness(&self, face: &Reference) -> ComResult<()> {
        self.com()
            .call(
                "AddFaceWithDifferentThickness",
                &[Arg::Obj(face.com().clone())],
            )
            .map(|_| ())
    }

    /// 移除不同厚度的面（`RemoveFaceWithDifferentThickness`）。
    pub fn remove_face_with_different_thickness(&self, face: &Reference) -> ComResult<()> {
        self.com()
            .call(
                "RemoveFaceWithDifferentThickness",
                &[Arg::Obj(face.com().clone())],
            )
            .map(|_| ())
    }

    /// 指定体支撑（`SetVolumeSupport`）。
    pub fn set_volume_support(&self, volume: &Reference) -> ComResult<()> {
        self.com()
            .call("SetVolumeSupport", &[Arg::Obj(volume.com().clone())])
            .map(|_| ())
    }

    /// 撤销移除面（`WithdrawFaceToRemove`）。
    pub fn withdraw_face_to_remove(&self, face: &Reference) -> ComResult<()> {
        self.com()
            .call("WithdrawFaceToRemove", &[Arg::Obj(face.com().clone())])
            .map(|_| ())
    }
}

impl Thickness {
    prop_get!(offset, "Offset", f64);

    /// 加厚面列表（`FacesToThicken`）。
    pub fn faces_to_thicken(&self) -> ComResult<Vec<Reference>> {
        member_ref_array(self.com(), "FacesToThicken")
    }

    /// 追加加厚面（`AddFaceToThicken`）。
    pub fn add_face_to_thicken(&self, face: &Reference) -> ComResult<()> {
        self.com()
            .call("AddFaceToThicken", &[Arg::Obj(face.com().clone())])
            .map(|_| ())
    }

    /// 追加不同厚度的面（`AddFaceWithDifferentThickness`）。
    pub fn add_face_with_different_thickness(&self, face: &Reference) -> ComResult<()> {
        self.com()
            .call(
                "AddFaceWithDifferentThickness",
                &[Arg::Obj(face.com().clone())],
            )
            .map(|_| ())
    }

    /// 移除不同厚度的面（`RemoveFaceWithDifferentThickness`）。
    pub fn remove_face_with_different_thickness(&self, face: &Reference) -> ComResult<()> {
        self.com()
            .call(
                "RemoveFaceWithDifferentThickness",
                &[Arg::Obj(face.com().clone())],
            )
            .map(|_| ())
    }

    /// 指定体支撑（`SetVolumeSupport`）。
    pub fn set_volume_support(&self, volume: &Reference) -> ComResult<()> {
        self.com()
            .call("SetVolumeSupport", &[Arg::Obj(volume.com().clone())])
            .map(|_| ())
    }

    /// 撤销加厚面（`WithdrawFaceToThicken`）。
    pub fn withdraw_face_to_thicken(&self, face: &Reference) -> ComResult<()> {
        self.com()
            .call("WithdrawFaceToThicken", &[Arg::Obj(face.com().clone())])
            .map(|_| ())
    }
}

impl Thread {
    prop_pair!(depth, set_depth, "Depth", f64);
    prop_pair!(diameter, set_diameter, "Diameter", f64);
    prop_pair!(
        lateral_face_element,
        set_lateral_face_element,
        "LateralFaceElement",
        ref_
    );
    prop_pair!(
        limit_face_element,
        set_limit_face_element,
        "LimitFaceElement",
        ref_
    );
    prop_pair!(pitch, set_pitch, "Pitch", f64);
    prop_pair!(side, set_side, "Side", i32);
    prop_get!(support_depth, "SupportDepth", f64);
    prop_get!(support_diameter, "SupportDiameter", f64);

    /// 螺纹标准描述（`ThreadDescription`）。
    pub fn thread_description(&self) -> ComResult<String> {
        self.com().get("ThreadDescription")?.as_string()
    }

    /// 建标准螺纹设计表（`CreateStandardThreadDesignTable`）。
    pub fn create_standard_thread_design_table(&self, standard_type: i32) -> ComResult<()> {
        self.com()
            .call(
                "CreateStandardThreadDesignTable",
                &[Arg::I32(standard_type)],
            )
            .map(|_| ())
    }

    /// 建用户标准螺纹设计表（`CreateUserStandardDesignTable`）。
    pub fn create_user_standard_design_table(
        &self,
        standard_name: &str,
        path: &str,
    ) -> ComResult<()> {
        self.com()
            .call(
                "CreateUserStandardDesignTable",
                &[
                    Arg::Str(standard_name.to_string()),
                    Arg::Str(path.to_string()),
                ],
            )
            .map(|_| ())
    }

    /// 反转方向（`ReverseDirection`）。
    pub fn reverse_direction(&self) -> ComResult<()> {
        self.com().call("ReverseDirection", &[]).map(|_| ())
    }

    /// 显式指定旋向（`SetExplicitPolarity`）。
    pub fn set_explicit_polarity(&self, thread_polarity: i32) -> ComResult<()> {
        self.com()
            .call("SetExplicitPolarity", &[Arg::I32(thread_polarity)])
            .map(|_| ())
    }
}

impl ComCollection for DraftDomains {
    type Item = DraftDomain;
}

impl Draft {
    /// 拔模域集合（`DraftDomains`）。
    pub fn draft_domains(&self) -> ComResult<DraftDomains> {
        Ok(DraftDomains::new(self.com().get("DraftDomains")?.as_obj()?))
    }

    prop_pair!(mode, set_mode, "Mode", i32);
    prop_pair!(parting_element, set_parting_element, "PartingElement", ref_);
}

impl DraftDomain {
    prop_get!(draft_angle, "DraftAngle", f64);
    prop_pair!(
        multiselection_mode,
        set_multiselection_mode,
        "MultiselectionMode",
        i32
    );
    prop_pair!(neutral_element, set_neutral_element, "NeutralElement", ref_);
    prop_pair!(
        neutral_propagation_mode,
        set_neutral_propagation_mode,
        "NeutralPropagationMode",
        i32
    );
    prop_pair!(
        pulling_direction_element,
        set_pulling_direction_element,
        "PullingDirectionElement",
        ref_
    );

    /// 拔模面列表（`FacesToDraft`）。
    pub fn faces_to_draft(&self) -> ComResult<Vec<Reference>> {
        member_ref_array(self.com(), "FacesToDraft")
    }

    /// 追加拔模面（`AddFaceToDraft`）。
    pub fn add_face_to_draft(&self, face: &Reference) -> ComResult<()> {
        self.com()
            .call("AddFaceToDraft", &[Arg::Obj(face.com().clone())])
            .map(|_| ())
    }

    /// 撤销拔模面（`RemoveFaceToDraft`）。
    pub fn remove_face_to_draft(&self, face: &Reference) -> ComResult<()> {
        self.com()
            .call("RemoveFaceToDraft", &[Arg::Obj(face.com().clone())])
            .map(|_| ())
    }

    /// 拔模方向分量（`GetPullingDirection`，VBA 中转）。
    pub fn get_pulling_direction(&self, ss: &SystemService) -> ComResult<[f64; 3]> {
        ss.direction_out_param(self.com(), "GetPullingDirection")
    }

    /// 指定拔模方向分量（`SetPullingDirection`）。
    pub fn set_pulling_direction(&self, x: f64, y: f64, z: f64) -> ComResult<()> {
        self.com()
            .call(
                "SetPullingDirection",
                &[Arg::F64(x), Arg::F64(y), Arg::F64(z)],
            )
            .map(|_| ())
    }

    /// 指定体支撑（`SetVolumeSupport`）。
    pub fn set_volume_support(&self, volume: &Reference) -> ComResult<()> {
        self.com()
            .call("SetVolumeSupport", &[Arg::Obj(volume.com().clone())])
            .map(|_| ())
    }
}

impl AutoDraft {
    prop_pair!(functional_face, set_functional_face, "FunctionalFace", ref_);
    prop_pair!(
        main_draft_angle,
        set_main_draft_angle,
        "MainDraftAngle",
        ref_
    );
    prop_pair!(parting_element, set_parting_element, "PartingElement", ref_);
    prop_pair!(
        pulling_direction,
        set_pulling_direction,
        "PullingDirection",
        ref_
    );

    /// 功能面列表（`FunctionalFaces`）。
    pub fn functional_faces(&self) -> ComResult<Vec<Reference>> {
        member_ref_array(self.com(), "FunctionalFaces")
    }
}

impl ComCollection for DefeaturingFilters {
    type Item = DefeaturingFilter;
}

impl Defeaturing {
    /// 去特征过滤器集合（`Filters`）。
    pub fn filters(&self) -> ComResult<DefeaturingFilters> {
        Ok(DefeaturingFilters::new(
            self.com().get("Filters")?.as_obj()?,
        ))
    }
}

impl DefeaturingFilters {
    /// 追加过滤器（`Add`，过滤器类型为 CATIA 字符串枚举）。
    pub fn add(&self, filter_type: &str) -> ComResult<DefeaturingFilter> {
        Ok(DefeaturingFilter::new(
            self.com()
                .call("Add", &[Arg::Str(filter_type.to_string())])?
                .as_obj()?,
        ))
    }

    /// 删除过滤器（`Remove`，按标识）。
    pub fn remove(&self, filter_id: &str) -> ComResult<()> {
        self.com()
            .call("Remove", &[Arg::Str(filter_id.to_string())])
            .map(|_| ())
    }
}

impl DefeaturingFilterWithRange {
    /// 读范围上限开关（`GetMaximumActivity`）。
    pub fn get_maximum_activity(&self, range_id: &str) -> ComResult<bool> {
        self.com()
            .call("GetMaximumActivity", &[Arg::Str(range_id.to_string())])?
            .as_bool()
    }

    /// 读范围上限角度（`GetMaximumAngle`）。
    pub fn get_maximum_angle(&self, range_id: &str) -> ComResult<f64> {
        self.com()
            .call("GetMaximumAngle", &[Arg::Str(range_id.to_string())])?
            .as_f64()
    }

    /// 读范围上限长度（`GetMaximumLength`）。
    pub fn get_maximum_length(&self, range_id: &str) -> ComResult<f64> {
        self.com()
            .call("GetMaximumLength", &[Arg::Str(range_id.to_string())])?
            .as_f64()
    }

    /// 读范围上限值（`GetMaximumValue`）。
    pub fn get_maximum_value(&self, range_id: &str) -> ComResult<f64> {
        self.com()
            .call("GetMaximumValue", &[Arg::Str(range_id.to_string())])?
            .as_f64()
    }

    /// 读范围下限开关（`GetMinimumActivity`）。
    pub fn get_minimum_activity(&self, range_id: &str) -> ComResult<bool> {
        self.com()
            .call("GetMinimumActivity", &[Arg::Str(range_id.to_string())])?
            .as_bool()
    }

    /// 读范围下限角度（`GetMinimumAngle`）。
    pub fn get_minimum_angle(&self, range_id: &str) -> ComResult<f64> {
        self.com()
            .call("GetMinimumAngle", &[Arg::Str(range_id.to_string())])?
            .as_f64()
    }

    /// 读范围下限长度（`GetMinimumLength`）。
    pub fn get_minimum_length(&self, range_id: &str) -> ComResult<f64> {
        self.com()
            .call("GetMinimumLength", &[Arg::Str(range_id.to_string())])?
            .as_f64()
    }

    /// 读范围下限值（`GetMinimumValue`）。
    pub fn get_minimum_value(&self, range_id: &str) -> ComResult<f64> {
        self.com()
            .call("GetMinimumValue", &[Arg::Str(range_id.to_string())])?
            .as_f64()
    }

    /// 写范围上限开关（`SetMaximumActivity`）。
    pub fn set_maximum_activity(&self, range_id: &str, value: bool) -> ComResult<()> {
        self.com()
            .call(
                "SetMaximumActivity",
                &[Arg::Str(range_id.to_string()), Arg::Bool(value)],
            )
            .map(|_| ())
    }

    /// 写范围上限值（`SetMaximumValue`）。
    pub fn set_maximum_value(&self, range_id: &str, value: f64) -> ComResult<()> {
        self.com()
            .call(
                "SetMaximumValue",
                &[Arg::Str(range_id.to_string()), Arg::F64(value)],
            )
            .map(|_| ())
    }

    /// 写范围下限开关（`SetMinimumActivity`）。
    pub fn set_minimum_activity(&self, range_id: &str, value: bool) -> ComResult<()> {
        self.com()
            .call(
                "SetMinimumActivity",
                &[Arg::Str(range_id.to_string()), Arg::Bool(value)],
            )
            .map(|_| ())
    }

    /// 写范围下限值（`SetMinimumValue`）。
    pub fn set_minimum_value(&self, range_id: &str, value: f64) -> ComResult<()> {
        self.com()
            .call(
                "SetMinimumValue",
                &[Arg::Str(range_id.to_string()), Arg::F64(value)],
            )
            .map(|_| ())
    }
}

// ---------------------------------------------------------------------------
// 波次 3 —— 布尔运算与面基于特征
// ---------------------------------------------------------------------------

com_wrapper!(Add);
com_wrapper!(Remove);
com_wrapper!(Intersect);
com_wrapper!(Assemble);
com_wrapper!(Trim);
com_wrapper!(Split);
com_wrapper!(ThickSurface);
com_wrapper!(SewSurface);
com_wrapper!(RemoveFace);
com_wrapper!(ReplaceFace);
com_wrapper!(CloseSurface);

impl Trim {
    /// 追加保留面（`AddFaceToKeep`）。
    pub fn add_face_to_keep(&self, face: &Reference) -> ComResult<()> {
        self.com()
            .call("AddFaceToKeep", &[Arg::Obj(face.com().clone())])
            .map(|_| ())
    }

    /// 追加保留面并指定邻接面（`AddFaceToKeep2`）。
    pub fn add_face_to_keep2(&self, face: &Reference, adjacent: &Reference) -> ComResult<()> {
        self.com()
            .call(
                "AddFaceToKeep2",
                &[
                    Arg::Obj(face.com().clone()),
                    Arg::Obj(adjacent.com().clone()),
                ],
            )
            .map(|_| ())
    }

    /// 追加移除面（`AddFaceToRemove`）。
    pub fn add_face_to_remove(&self, face: &Reference) -> ComResult<()> {
        self.com()
            .call("AddFaceToRemove", &[Arg::Obj(face.com().clone())])
            .map(|_| ())
    }

    /// 追加移除面并指定邻接面（`AddFaceToRemove2`）。
    pub fn add_face_to_remove2(&self, face: &Reference, adjacent: &Reference) -> ComResult<()> {
        self.com()
            .call(
                "AddFaceToRemove2",
                &[
                    Arg::Obj(face.com().clone()),
                    Arg::Obj(adjacent.com().clone()),
                ],
            )
            .map(|_| ())
    }

    /// 撤销保留面（`WithdrawFaceToKeep`）。
    pub fn withdraw_face_to_keep(&self, face: &Reference) -> ComResult<()> {
        self.com()
            .call("WithdrawFaceToKeep", &[Arg::Obj(face.com().clone())])
            .map(|_| ())
    }

    /// 撤销保留面并指定邻接面（`WithdrawFaceToKeep2`）。
    pub fn withdraw_face_to_keep2(&self, face: &Reference, adjacent: &Reference) -> ComResult<()> {
        self.com()
            .call(
                "WithdrawFaceToKeep2",
                &[
                    Arg::Obj(face.com().clone()),
                    Arg::Obj(adjacent.com().clone()),
                ],
            )
            .map(|_| ())
    }

    /// 撤销移除面（`WithdrawFaceToRemove`）。
    pub fn withdraw_face_to_remove(&self, face: &Reference) -> ComResult<()> {
        self.com()
            .call("WithdrawFaceToRemove", &[Arg::Obj(face.com().clone())])
            .map(|_| ())
    }

    /// 撤销移除面并指定邻接面（`WithdrawFaceToRemove2`）。
    pub fn withdraw_face_to_remove2(
        &self,
        face: &Reference,
        adjacent: &Reference,
    ) -> ComResult<()> {
        self.com()
            .call(
                "WithdrawFaceToRemove2",
                &[
                    Arg::Obj(face.com().clone()),
                    Arg::Obj(adjacent.com().clone()),
                ],
            )
            .map(|_| ())
    }
}

impl Split {
    prop_pair!(splitting_side, set_splitting_side, "SplittingSide", i32);
}

impl ThickSurface {
    prop_get!(bot_offset, "BotOffset", f64);
    prop_get!(top_offset, "TopOffset", f64);
    prop_get!(offset_side, "OffsetSide", i32);

    /// 交换偏置侧（`SwapOffsetSide`）。
    pub fn swap_offset_side(&self) -> ComResult<()> {
        self.com().call("SwapOffsetSide", &[]).map(|_| ())
    }
}

impl SewSurface {
    prop_pair!(deviation, set_deviation, "Deviation", f64);
    prop_pair!(deviation_mode, set_deviation_mode, "DeviationMode", i32);
    prop_pair!(
        sewing_intersection_mode,
        set_sewing_intersection_mode,
        "SewingIntersectionMode",
        i32
    );
    prop_pair!(sewing_side, set_sewing_side, "SewingSide", i32);

    /// 指定曲面支撑（`SetSurfaceSupport`）。
    pub fn set_surface_support(&self, surface: &Reference) -> ComResult<()> {
        self.com()
            .call("SetSurfaceSupport", &[Arg::Obj(surface.com().clone())])
            .map(|_| ())
    }

    /// 指定体支撑（`SetVolumeSupport`）。
    pub fn set_volume_support(&self, volume: &Reference) -> ComResult<()> {
        self.com()
            .call("SetVolumeSupport", &[Arg::Obj(volume.com().clone())])
            .map(|_| ())
    }
}

impl RemoveFace {
    /// 追加保留面（`KeepFace`）。
    pub fn keep_face(&self, face: &Reference) -> ComResult<()> {
        self.com()
            .call("KeepFace", &[Arg::Obj(face.com().clone())])
            .map(|_| ())
    }

    /// 保留面列表（`KeepFaces`）。
    pub fn keep_faces(&self) -> ComResult<Vec<Reference>> {
        member_ref_array(self.com(), "KeepFaces")
    }

    prop_get!(propagation, "Propagation", i32);

    /// 追加移除面（`RemoveFace`）。
    pub fn remove_face(&self, face: &Reference) -> ComResult<()> {
        self.com()
            .call("RemoveFace", &[Arg::Obj(face.com().clone())])
            .map(|_| ())
    }

    /// 移除面列表（`RemoveFaces`）。
    pub fn remove_faces(&self) -> ComResult<Vec<Reference>> {
        member_ref_array(self.com(), "RemoveFaces")
    }

    /// 撤销保留面（`RemoveKeepFace`）。
    pub fn remove_keep_face(&self, face: &Reference) -> ComResult<()> {
        self.com()
            .call("RemoveKeepFace", &[Arg::Obj(face.com().clone())])
            .map(|_| ())
    }

    /// 撤销移除面（`RemoveRemoveFace`）。
    pub fn remove_remove_face(&self, face: &Reference) -> ComResult<()> {
        self.com()
            .call("RemoveRemoveFace", &[Arg::Obj(face.com().clone())])
            .map(|_| ())
    }
}

impl ReplaceFace {
    /// 被替换面（`RemoveFace`，只读）。
    pub fn remove_face(&self) -> ComResult<Reference> {
        Ok(Reference::new(self.com().get("RemoveFace")?.as_obj()?))
    }

    prop_pair!(splitting_side, set_splitting_side, "SplittingSide", i32);

    /// 追加被替换面（`AddRemoveFace`）。
    pub fn add_remove_face(&self, face: &Reference) -> ComResult<()> {
        self.com()
            .call("AddRemoveFace", &[Arg::Obj(face.com().clone())])
            .map(|_| ())
    }

    /// 追加分割面（`AddSplitPlane`）。
    pub fn add_split_plane(&self, plane: &Reference) -> ComResult<()> {
        self.com()
            .call("AddSplitPlane", &[Arg::Obj(plane.com().clone())])
            .map(|_| ())
    }

    /// 删除被替换面（`DeleteRemoveFace`）。
    pub fn delete_remove_face(&self, face: &Reference) -> ComResult<()> {
        self.com()
            .call("DeleteRemoveFace", &[Arg::Obj(face.com().clone())])
            .map(|_| ())
    }
}

// ---------------------------------------------------------------------------
// 波次 4 —— 变换
// ---------------------------------------------------------------------------

com_wrapper!(Mirror);
com_wrapper!(Translate);
com_wrapper!(Rotate);
com_wrapper!(Symmetry);
com_wrapper!(Affinity);
com_wrapper!(AxisToAxis);
com_wrapper!(Scaling);
com_wrapper!(Scaling2);

impl Mirror {
    /// 镜像对象（`MirroringObject`，原始对象）。
    pub fn mirroring_object(&self) -> ComResult<ComObj> {
        self.com().get("MirroringObject")?.as_obj()
    }

    prop_pair!(mirroring_plane, set_mirroring_plane, "MirroringPlane", ref_);
}

impl Translate {
    /// 底层 GSD 平移对象（`HybridShape`）。
    pub fn hybrid_shape(&self) -> ComResult<ComObj> {
        hybrid_shape(self.com())
    }
}

impl Rotate {
    prop_get!(angle, "Angle", f64);
    prop_pair!(angle_value, set_angle_value, "AngleValue", f64);
    prop_pair!(axis, set_axis, "Axis", ref_);

    /// 底层 GSD 旋转对象（`HybridShape`）。
    pub fn hybrid_shape(&self) -> ComResult<ComObj> {
        hybrid_shape(self.com())
    }
}

impl Symmetry {
    /// 底层 GSD 对称对象（`HybridShape`）。
    pub fn hybrid_shape(&self) -> ComResult<ComObj> {
        hybrid_shape(self.com())
    }
}

impl Affinity {
    /// 底层 GSD 仿射对象（`HybridShape`）。
    pub fn hybrid_shape(&self) -> ComResult<ComObj> {
        hybrid_shape(self.com())
    }
}

impl AxisToAxis {
    /// 底层 GSD 轴到轴对象（`HybridShape`）。
    pub fn hybrid_shape(&self) -> ComResult<ComObj> {
        hybrid_shape(self.com())
    }
}

impl Scaling {
    prop_get!(factor, "Factor", f64);
    prop_pair!(
        scaling_reference,
        set_scaling_reference,
        "ScalingReference",
        ref_
    );
}

impl Scaling2 {
    prop_pair!(center, set_center, "Center", ref_);
    prop_get!(ratio, "Ratio", f64);
    prop_pair!(ratio_value, set_ratio_value, "RatioValue", f64);
}

// ---------------------------------------------------------------------------
// 波次 5 —— 阵列与 Repartition 族
// ---------------------------------------------------------------------------

com_wrapper!(Pattern);
com_wrapper!(RectPattern);
com_wrapper!(CircPattern);
com_wrapper!(UserPattern);
com_wrapper!(Repartition);
com_wrapper!(LinearRepartition);
com_wrapper!(AngularRepartition);
com_wrapper!(UserRepartition);

impl Pattern {
    prop_pair!(item_to_copy, set_item_to_copy, "ItemToCopy", obj);
    prop_get!(rotation_angle, "RotationAngle", f64);

    /// 激活阵列实例（`ActivatePosition`）。
    pub fn activate_position(&self, pos_u: i32, pos_v: i32) -> ComResult<()> {
        self.com()
            .call("ActivatePosition", &[Arg::I32(pos_u), Arg::I32(pos_v)])
            .map(|_| ())
    }

    /// 停用阵列实例（`DesactivatePosition`；CATIA 拼写如此）。
    pub fn desactivate_position(&self, pos_u: i32, pos_v: i32) -> ComResult<()> {
        self.com()
            .call("DesactivatePosition", &[Arg::I32(pos_u), Arg::I32(pos_v)])
            .map(|_| ())
    }
}

impl RectPattern {
    /// 第一方向 Repartition（`FirstDirectionRepartition`）。
    pub fn first_direction_repartition(&self) -> ComResult<Repartition> {
        Ok(Repartition::new(
            self.com().get("FirstDirectionRepartition")?.as_obj()?,
        ))
    }

    /// 第一方向行（`FirstDirectionRow`，原始对象）。
    pub fn first_direction_row(&self) -> ComResult<ComObj> {
        self.com().get("FirstDirectionRow")?.as_obj()
    }

    /// 第二方向 Repartition（`SecondDirectionRepartition`）。
    pub fn second_direction_repartition(&self) -> ComResult<Repartition> {
        Ok(Repartition::new(
            self.com().get("SecondDirectionRepartition")?.as_obj()?,
        ))
    }

    /// 第二方向行（`SecondDirectionRow`，原始对象）。
    pub fn second_direction_row(&self) -> ComResult<ComObj> {
        self.com().get("SecondDirectionRow")?.as_obj()
    }

    prop_pair!(
        first_orientation,
        set_first_orientation,
        "FirstOrientation",
        bool
    );
    prop_pair!(
        first_rectangular_pattern_parameters,
        set_first_rectangular_pattern_parameters,
        "FirstRectangularPatternParameters",
        i32
    );
    prop_pair!(
        second_orientation,
        set_second_orientation,
        "SecondOrientation",
        bool
    );
    prop_pair!(
        second_rectangular_pattern_parameters,
        set_second_rectangular_pattern_parameters,
        "SecondRectangularPatternParameters",
        i32
    );

    /// 第一方向分量（`GetFirstDirection`，VBA 中转）。
    pub fn get_first_direction(&self, ss: &SystemService) -> ComResult<[f64; 3]> {
        ss.direction_out_param(self.com(), "GetFirstDirection")
    }

    /// 第二方向分量（`GetSecondDirection`，VBA 中转）。
    pub fn get_second_direction(&self, ss: &SystemService) -> ComResult<[f64; 3]> {
        ss.direction_out_param(self.com(), "GetSecondDirection")
    }

    /// 指定第一方向（`SetFirstDirection`）。
    pub fn set_first_direction(&self, direction: &Reference) -> ComResult<()> {
        self.com()
            .call("SetFirstDirection", &[Arg::Obj(direction.com().clone())])
            .map(|_| ())
    }

    /// 指定第二方向（`SetSecondDirection`）。
    pub fn set_second_direction(&self, direction: &Reference) -> ComResult<()> {
        self.com()
            .call("SetSecondDirection", &[Arg::Obj(direction.com().clone())])
            .map(|_| ())
    }

    /// 设某方向实例间距（`SetInstanceSpacing`；direction: 1=第一方向, 2=第二方向）。
    pub fn set_instance_spacing(
        &self,
        instance_number: i32,
        spacing: f64,
        direction: i32,
    ) -> ComResult<()> {
        self.com()
            .call(
                "SetInstanceSpacing",
                &[
                    Arg::I32(instance_number),
                    Arg::F64(spacing),
                    Arg::I32(direction),
                ],
            )
            .map(|_| ())
    }

    /// 设某方向不等间距实例数（`SetUnequalInstanceNumber`）。
    pub fn set_unequal_instance_number(
        &self,
        instance_number: i32,
        direction: i32,
    ) -> ComResult<()> {
        self.com()
            .call(
                "SetUnequalInstanceNumber",
                &[Arg::I32(instance_number), Arg::I32(direction)],
            )
            .map(|_| ())
    }
}

impl CircPattern {
    /// 角度方向行（`AngularDirectionRow`，原始对象）。
    pub fn angular_direction_row(&self) -> ComResult<ComObj> {
        self.com().get("AngularDirectionRow")?.as_obj()
    }

    /// 角度方向 Repartition（`AngularRepartition`）。
    pub fn angular_repartition(&self) -> ComResult<AngularRepartition> {
        Ok(AngularRepartition::new(
            self.com().get("AngularRepartition")?.as_obj()?,
        ))
    }

    /// 径向方向行（`RadialDirectionRow`，原始对象）。
    pub fn radial_direction_row(&self) -> ComResult<ComObj> {
        self.com().get("RadialDirectionRow")?.as_obj()
    }

    /// 径向 Repartition（`RadialRepartition`）。
    pub fn radial_repartition(&self) -> ComResult<Repartition> {
        Ok(Repartition::new(
            self.com().get("RadialRepartition")?.as_obj()?,
        ))
    }

    prop_pair!(
        circular_pattern_parameters,
        set_circular_pattern_parameters,
        "CircularPatternParameters",
        i32
    );
    prop_pair!(
        radial_alignment,
        set_radial_alignment,
        "RadialAlignment",
        bool
    );
    prop_pair!(
        rotation_orientation,
        set_rotation_orientation,
        "RotationOrientation",
        bool
    );

    /// 旋转轴分量（`GetRotationAxis`，VBA 中转）。
    pub fn get_rotation_axis(&self, ss: &SystemService) -> ComResult<[f64; 3]> {
        ss.direction_out_param(self.com(), "GetRotationAxis")
    }

    /// 旋转中心分量（`GetRotationCenter`，VBA 中转）。
    pub fn get_rotation_center(&self, ss: &SystemService) -> ComResult<[f64; 3]> {
        ss.direction_out_param(self.com(), "GetRotationCenter")
    }

    /// 设角度间距实例数与间距（`SetInstanceAngularSpacing`）。
    pub fn set_instance_angular_spacing(
        &self,
        instance_number: i32,
        angular_spacing: f64,
    ) -> ComResult<()> {
        self.com()
            .call(
                "SetInstanceAngularSpacing",
                &[Arg::I32(instance_number), Arg::F64(angular_spacing)],
            )
            .map(|_| ())
    }

    /// 指定旋转轴（`SetRotationAxis`）。
    pub fn set_rotation_axis(&self, axis: &Reference) -> ComResult<()> {
        self.com()
            .call("SetRotationAxis", &[Arg::Obj(axis.com().clone())])
            .map(|_| ())
    }

    /// 指定旋转中心（`SetRotationCenter`）。
    pub fn set_rotation_center(&self, center: &Reference) -> ComResult<()> {
        self.com()
            .call("SetRotationCenter", &[Arg::Obj(center.com().clone())])
            .map(|_| ())
    }

    /// 设不等间距实例数（`SetUnequalInstanceNumber`）。
    pub fn set_unequal_instance_number(&self, instance_number: i32) -> ComResult<()> {
        self.com()
            .call("SetUnequalInstanceNumber", &[Arg::I32(instance_number)])
            .map(|_| ())
    }

    /// 设不等间距步长（`SetUnequalStep`）。
    pub fn set_unequal_step(&self, instance_number: i32) -> ComResult<()> {
        self.com()
            .call("SetUnequalStep", &[Arg::I32(instance_number)])
            .map(|_| ())
    }
}

impl UserPattern {
    prop_pair!(anchor_point, set_anchor_point, "AnchorPoint", obj);

    /// 定位特征列表（`FeatureToLocatePositions`）。
    pub fn feature_to_locate_positions(&self) -> ComResult<Vec<ComObj>> {
        member_obj_array(self.com(), "FeatureToLocatePositions")
    }

    /// 追加定位特征（`AddFeatureToLocatePositions`）。
    pub fn add_feature_to_locate_positions(&self, feature: &impl ComObject) -> ComResult<()> {
        self.com()
            .call(
                "AddFeatureToLocatePositions",
                &[Arg::Obj(feature.com().clone())],
            )
            .map(|_| ())
    }
}

impl Repartition {
    prop_get!(instances_count, "InstancesCount", i32);
}

impl LinearRepartition {
    prop_get!(spacing, "Spacing", f64);
}

impl AngularRepartition {
    prop_get!(angular_spacing, "AngularSpacing", f64);
    prop_get!(instance_spacing, "InstanceSpacing", f64);
}

impl UserRepartition {
    /// 定位特征列表（`FeatureToLocatePositions`）。
    pub fn feature_to_locate_positions(&self) -> ComResult<Vec<ComObj>> {
        member_obj_array(self.com(), "FeatureToLocatePositions")
    }

    /// 追加定位特征（`AddFeatureToLocatePositions`）。
    pub fn add_feature_to_locate_positions(&self, feature: &impl ComObject) -> ComResult<()> {
        self.com()
            .call(
                "AddFeatureToLocatePositions",
                &[Arg::Obj(feature.com().clone())],
            )
            .map(|_| ())
    }
}
