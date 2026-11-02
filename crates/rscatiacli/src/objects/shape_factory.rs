//! `ShapeFactory`：Part Design 实体特征工厂（`Part.ShapeFactory`）。
//!
//! 创建方法与 pycatia `part_interfaces.shape_factory` 暴露面对齐（实体集；
//! GSD 曲面 / Volume 系列属阶段 6）。返回值均为 [`crate::objects::part_interfaces`]
//! 里的类型化特征。注意 `AddNewAxisToAxis2` 成功时 COM 返回值也是 None
//! （动态调度），故只返回 `()`；结果用「Body 内形状数 + 重心变化」验证
//! （与 Python 版同口径）。

use crate::dispatch::{Arg, ComObject};
use crate::error::ComResult;

use super::part::Body;
use super::part_interfaces::{
    Add, Affinity, Assemble, AutoDraft, AutoFillet, CircPattern, CloseSurface, ConstRadEdgeFillet,
    Defeaturing, FaceFillet, Groove, Hole, Intersect, Loft, Mirror, Pad, Pocket, RectPattern,
    Remove, RemoveFace, ReplaceFace, Rib, Rotate, Scaling, SewSurface, Shaft, Shell, Slot,
    SolidCombine, Split, Stiffener, Symmetry, Thickness, Thread, Translate, Trim, TritangentFillet,
    UserPattern, VarRadEdgeFillet,
};
use super::reference::Reference;
use super::sketch::Sketch;

com_wrapper!(ShapeFactory);

/// CatSplitSide：保留分割元素的哪一侧（与 CATIA 枚举值一致）。
pub const CAT_POSITIVE_SIDE: i32 = 0;
pub const CAT_NEGATIVE_SIDE: i32 = 1;

impl ShapeFactory {
    /// 用分割元素裁剪实体：`AddNewSplit(Reference, CatSplitSide) As Split`。
    pub fn add_new_split(
        &self,
        splitting_element: &Reference,
        split_side: i32,
    ) -> ComResult<Split> {
        Ok(Split::new(
            self.com()
                .call(
                    "AddNewSplit",
                    &[
                        Arg::Obj(splitting_element.com().clone()),
                        Arg::I32(split_side),
                    ],
                )?
                .as_obj()?,
        ))
    }

    /// 拉伸：`AddNewPad(轮廓 Sketch, 高度 mm)`（沿 +Z; 失败多为轮廓自相交）。
    pub fn add_new_pad(&self, profile: &Sketch, height: f64) -> ComResult<Pad> {
        Ok(Pad::new(
            self.com()
                .call(
                    "AddNewPad",
                    &[Arg::Obj(profile.com().clone()), Arg::F64(height)],
                )?
                .as_obj()?,
        ))
    }

    /// 以 Reference 为轮廓拉伸：`AddNewPadFromRef(Reference, 高度 mm)`。
    pub fn add_new_pad_from_ref(&self, profile: &Reference, height: f64) -> ComResult<Pad> {
        Ok(Pad::new(
            self.com()
                .call(
                    "AddNewPadFromRef",
                    &[Arg::Obj(profile.com().clone()), Arg::F64(height)],
                )?
                .as_obj()?,
        ))
    }

    /// 凹槽：`AddNewPocket(轮廓 Sketch, 深度 mm)`。
    pub fn add_new_pocket(&self, profile: &Sketch, depth: f64) -> ComResult<Pocket> {
        Ok(Pocket::new(
            self.com()
                .call(
                    "AddNewPocket",
                    &[Arg::Obj(profile.com().clone()), Arg::F64(depth)],
                )?
                .as_obj()?,
        ))
    }

    /// 以 Reference 为轮廓开槽：`AddNewPocketFromRef(Reference, 深度 mm)`。
    pub fn add_new_pocket_from_ref(&self, profile: &Reference, depth: f64) -> ComResult<Pocket> {
        Ok(Pocket::new(
            self.com()
                .call(
                    "AddNewPocketFromRef",
                    &[Arg::Obj(profile.com().clone()), Arg::F64(depth)],
                )?
                .as_obj()?,
        ))
    }

    /// 旋转体：`AddNewShaft(轮廓 Sketch)`。
    pub fn add_new_shaft(&self, profile: &Sketch) -> ComResult<Shaft> {
        Ok(Shaft::new(
            self.com()
                .call("AddNewShaft", &[Arg::Obj(profile.com().clone())])?
                .as_obj()?,
        ))
    }

    /// 以 Reference 为轮廓旋转体：`AddNewShaftFromRef(Reference)`。
    pub fn add_new_shaft_from_ref(&self, profile: &Reference) -> ComResult<Shaft> {
        Ok(Shaft::new(
            self.com()
                .call("AddNewShaftFromRef", &[Arg::Obj(profile.com().clone())])?
                .as_obj()?,
        ))
    }

    /// 旋转槽：`AddNewGroove(轮廓 Sketch)`。
    pub fn add_new_groove(&self, profile: &Sketch) -> ComResult<Groove> {
        Ok(Groove::new(
            self.com()
                .call("AddNewGroove", &[Arg::Obj(profile.com().clone())])?
                .as_obj()?,
        ))
    }

    /// 以 Reference 为轮廓旋转槽：`AddNewGrooveFromRef(Reference)`。
    pub fn add_new_groove_from_ref(&self, profile: &Reference) -> ComResult<Groove> {
        Ok(Groove::new(
            self.com()
                .call("AddNewGrooveFromRef", &[Arg::Obj(profile.com().clone())])?
                .as_obj()?,
        ))
    }

    /// 肋：`AddNewRib(轮廓 Sketch, 中心曲线 Sketch)`。
    pub fn add_new_rib(&self, profile: &Sketch, center_curve: &Sketch) -> ComResult<Rib> {
        Ok(Rib::new(
            self.com()
                .call(
                    "AddNewRib",
                    &[
                        Arg::Obj(profile.com().clone()),
                        Arg::Obj(center_curve.com().clone()),
                    ],
                )?
                .as_obj()?,
        ))
    }

    /// 以 Reference 建肋：`AddNewRibFromRef(Reference, Reference)`。
    pub fn add_new_rib_from_ref(
        &self,
        profile: &Reference,
        center_curve: &Reference,
    ) -> ComResult<Rib> {
        Ok(Rib::new(
            self.com()
                .call(
                    "AddNewRibFromRef",
                    &[
                        Arg::Obj(profile.com().clone()),
                        Arg::Obj(center_curve.com().clone()),
                    ],
                )?
                .as_obj()?,
        ))
    }

    /// 开槽：`AddNewSlot(轮廓 Sketch, 中心曲线 Sketch)`。
    pub fn add_new_slot(&self, profile: &Sketch, center_curve: &Sketch) -> ComResult<Slot> {
        Ok(Slot::new(
            self.com()
                .call(
                    "AddNewSlot",
                    &[
                        Arg::Obj(profile.com().clone()),
                        Arg::Obj(center_curve.com().clone()),
                    ],
                )?
                .as_obj()?,
        ))
    }

    /// 以 Reference 开槽：`AddNewSlotFromRef(Reference, Reference)`。
    pub fn add_new_slot_from_ref(
        &self,
        profile: &Reference,
        center_curve: &Reference,
    ) -> ComResult<Slot> {
        Ok(Slot::new(
            self.com()
                .call(
                    "AddNewSlotFromRef",
                    &[
                        Arg::Obj(profile.com().clone()),
                        Arg::Obj(center_curve.com().clone()),
                    ],
                )?
                .as_obj()?,
        ))
    }

    /// 加强筋：`AddNewStiffener(轮廓 Sketch)`。
    pub fn add_new_stiffener(&self, profile: &Sketch) -> ComResult<Stiffener> {
        Ok(Stiffener::new(
            self.com()
                .call("AddNewStiffener", &[Arg::Obj(profile.com().clone())])?
                .as_obj()?,
        ))
    }

    /// 以 Reference 建加强筋：`AddNewStiffenerFromRef(Reference)`。
    pub fn add_new_stiffener_from_ref(&self, profile: &Reference) -> ComResult<Stiffener> {
        Ok(Stiffener::new(
            self.com()
                .call("AddNewStiffenerFromRef", &[Arg::Obj(profile.com().clone())])?
                .as_obj()?,
        ))
    }

    /// 在支撑面上钻孔：`AddNewHole(支撑 Reference, 深度 mm)`。
    pub fn add_new_hole(&self, support: &Reference, depth: f64) -> ComResult<Hole> {
        Ok(Hole::new(
            self.com()
                .call(
                    "AddNewHole",
                    &[Arg::Obj(support.com().clone()), Arg::F64(depth)],
                )?
                .as_obj()?,
        ))
    }

    /// 按点钻孔：`AddNewHoleFromPoint(x, y, z, 支撑, 深度)`。
    pub fn add_new_hole_from_point(
        &self,
        x: f64,
        y: f64,
        z: f64,
        support: &Reference,
        depth: f64,
    ) -> ComResult<Hole> {
        Ok(Hole::new(
            self.com()
                .call(
                    "AddNewHoleFromPoint",
                    &[
                        Arg::F64(x),
                        Arg::F64(y),
                        Arg::F64(z),
                        Arg::Obj(support.com().clone()),
                        Arg::F64(depth),
                    ],
                )?
                .as_obj()?,
        ))
    }

    /// 按参考点钻孔：`AddNewHoleFromRefPoint(原点 Reference, 支撑, 深度)`。
    pub fn add_new_hole_from_ref_point(
        &self,
        origin: &Reference,
        support: &Reference,
        depth: f64,
    ) -> ComResult<Hole> {
        Ok(Hole::new(
            self.com()
                .call(
                    "AddNewHoleFromRefPoint",
                    &[
                        Arg::Obj(origin.com().clone()),
                        Arg::Obj(support.com().clone()),
                        Arg::F64(depth),
                    ],
                )?
                .as_obj()?,
        ))
    }

    /// 按草图钻孔：`AddNewHoleFromSketch(轮廓 Sketch, 深度 mm)`。
    pub fn add_new_hole_from_sketch(&self, profile: &Sketch, depth: f64) -> ComResult<Hole> {
        Ok(Hole::new(
            self.com()
                .call(
                    "AddNewHoleFromSketch",
                    &[Arg::Obj(profile.com().clone()), Arg::F64(depth)],
                )?
                .as_obj()?,
        ))
    }

    /// 布尔加：`AddNewAdd(Body)`。
    pub fn add_new_add(&self, body: &Body) -> ComResult<Add> {
        Ok(Add::new(
            self.com()
                .call("AddNewAdd", &[Arg::Obj(body.com().clone())])?
                .as_obj()?,
        ))
    }

    /// 布尔减：`AddNewRemove(Body)`。
    pub fn add_new_remove(&self, body: &Body) -> ComResult<Remove> {
        Ok(Remove::new(
            self.com()
                .call("AddNewRemove", &[Arg::Obj(body.com().clone())])?
                .as_obj()?,
        ))
    }

    /// 布尔交：`AddNewIntersect(Body)`。
    pub fn add_new_intersect(&self, body: &Body) -> ComResult<Intersect> {
        Ok(Intersect::new(
            self.com()
                .call("AddNewIntersect", &[Arg::Obj(body.com().clone())])?
                .as_obj()?,
        ))
    }

    /// 布尔并：`AddNewAssemble(Body)`。
    pub fn add_new_assemble(&self, body: &Body) -> ComResult<Assemble> {
        Ok(Assemble::new(
            self.com()
                .call("AddNewAssemble", &[Arg::Obj(body.com().clone())])?
                .as_obj()?,
        ))
    }

    /// 修剪：`AddNewTrim(Body)`。
    pub fn add_new_trim(&self, body: &Body) -> ComResult<Trim> {
        Ok(Trim::new(
            self.com()
                .call("AddNewTrim", &[Arg::Obj(body.com().clone())])?
                .as_obj()?,
        ))
    }

    /// 镜像：`AddNewMirror(镜像元素 Reference)`。
    pub fn add_new_mirror(&self, mirroring_element: &Reference) -> ComResult<Mirror> {
        Ok(Mirror::new(
            self.com()
                .call("AddNewMirror", &[Arg::Obj(mirroring_element.com().clone())])?
                .as_obj()?,
        ))
    }

    /// 旋转：`AddNewRotate2(轴 Reference, 角度 deg)`。
    pub fn add_new_rotate2(&self, axis: &Reference, angle: f64) -> ComResult<Rotate> {
        Ok(Rotate::new(
            self.com()
                .call(
                    "AddNewRotate2",
                    &[Arg::Obj(axis.com().clone()), Arg::F64(angle)],
                )?
                .as_obj()?,
        ))
    }

    /// 平移：`AddNewTranslate2(距离 mm)`。
    pub fn add_new_translate2(&self, distance: f64) -> ComResult<Translate> {
        Ok(Translate::new(
            self.com()
                .call("AddNewTranslate2", &[Arg::F64(distance)])?
                .as_obj()?,
        ))
    }

    /// 对称：`AddNewSymmetry2(参考 Reference)`。
    pub fn add_new_symmetry_2(&self, reference: &Reference) -> ComResult<Symmetry> {
        Ok(Symmetry::new(
            self.com()
                .call("AddNewSymmetry2", &[Arg::Obj(reference.com().clone())])?
                .as_obj()?,
        ))
    }

    /// 仿射：`AddNewAffinity2(x/y/z 比例)`。
    pub fn add_new_affinity2(
        &self,
        x_ratio: f64,
        y_ratio: f64,
        z_ratio: f64,
    ) -> ComResult<Affinity> {
        Ok(Affinity::new(
            self.com()
                .call(
                    "AddNewAffinity2",
                    &[Arg::F64(x_ratio), Arg::F64(y_ratio), Arg::F64(z_ratio)],
                )?
                .as_obj()?,
        ))
    }

    /// 缩放：`AddNewScaling(参考 Reference, 比例)`。
    pub fn add_new_scaling(
        &self,
        scaling_reference: &Reference,
        factor: f64,
    ) -> ComResult<Scaling> {
        Ok(Scaling::new(
            self.com()
                .call(
                    "AddNewScaling",
                    &[Arg::Obj(scaling_reference.com().clone()), Arg::F64(factor)],
                )?
                .as_obj()?,
        ))
    }

    /// 等半径边倒圆：`AddNewEdgeFilletWithConstantRadius(边, 传播模式, 半径 mm)`。
    pub fn add_new_edge_fillet_with_constant_radius(
        &self,
        edge: &Reference,
        propagation_mode: i32,
        radius: f64,
    ) -> ComResult<ConstRadEdgeFillet> {
        Ok(ConstRadEdgeFillet::new(
            self.com()
                .call(
                    "AddNewEdgeFilletWithConstantRadius",
                    &[
                        Arg::Obj(edge.com().clone()),
                        Arg::I32(propagation_mode),
                        Arg::F64(radius),
                    ],
                )?
                .as_obj()?,
        ))
    }

    /// 变半径边倒圆：`AddNewEdgeFilletWithVaryingRadius(边, 传播, 变化模式, 默认半径)`。
    pub fn add_new_edge_fillet_with_varying_radius(
        &self,
        edge: &Reference,
        propagation_mode: i32,
        variation_mode: i32,
        default_radius: f64,
    ) -> ComResult<VarRadEdgeFillet> {
        Ok(VarRadEdgeFillet::new(
            self.com()
                .call(
                    "AddNewEdgeFilletWithVaryingRadius",
                    &[
                        Arg::Obj(edge.com().clone()),
                        Arg::I32(propagation_mode),
                        Arg::I32(variation_mode),
                        Arg::F64(default_radius),
                    ],
                )?
                .as_obj()?,
        ))
    }

    /// 实体等半径边倒圆：`AddNewSolidEdgeFilletWithConstantRadius`。
    pub fn add_new_solid_edge_fillet_with_constant_radius(
        &self,
        edge: &Reference,
        propagation_mode: i32,
        radius: f64,
    ) -> ComResult<ConstRadEdgeFillet> {
        Ok(ConstRadEdgeFillet::new(
            self.com()
                .call(
                    "AddNewSolidEdgeFilletWithConstantRadius",
                    &[
                        Arg::Obj(edge.com().clone()),
                        Arg::I32(propagation_mode),
                        Arg::F64(radius),
                    ],
                )?
                .as_obj()?,
        ))
    }

    /// 实体变半径边倒圆：`AddNewSolidEdgeFilletWithVaryingRadius`。
    pub fn add_new_solid_edge_fillet_with_varying_radius(
        &self,
        edge: &Reference,
        propagation_mode: i32,
        variation_mode: i32,
        default_radius: f64,
    ) -> ComResult<VarRadEdgeFillet> {
        Ok(VarRadEdgeFillet::new(
            self.com()
                .call(
                    "AddNewSolidEdgeFilletWithVaryingRadius",
                    &[
                        Arg::Obj(edge.com().clone()),
                        Arg::I32(propagation_mode),
                        Arg::I32(variation_mode),
                        Arg::F64(default_radius),
                    ],
                )?
                .as_obj()?,
        ))
    }

    /// 面倒圆：`AddNewFaceFillet(面1, 面2, 半径 mm)`。
    pub fn add_new_face_fillet(
        &self,
        face1: &Reference,
        face2: &Reference,
        radius: f64,
    ) -> ComResult<FaceFillet> {
        Ok(FaceFillet::new(
            self.com()
                .call(
                    "AddNewFaceFillet",
                    &[
                        Arg::Obj(face1.com().clone()),
                        Arg::Obj(face2.com().clone()),
                        Arg::F64(radius),
                    ],
                )?
                .as_obj()?,
        ))
    }

    /// 实体面倒圆：`AddNewSolidFaceFillet`。
    pub fn add_new_solid_face_fillet(
        &self,
        face1: &Reference,
        face2: &Reference,
        radius: f64,
    ) -> ComResult<FaceFillet> {
        Ok(FaceFillet::new(
            self.com()
                .call(
                    "AddNewSolidFaceFillet",
                    &[
                        Arg::Obj(face1.com().clone()),
                        Arg::Obj(face2.com().clone()),
                        Arg::F64(radius),
                    ],
                )?
                .as_obj()?,
        ))
    }

    /// 三切面倒圆：`AddNewTritangentFillet(面1, 面2, 移除面)`。
    pub fn add_new_tritangent_fillet(
        &self,
        face1: &Reference,
        face2: &Reference,
        removed_face: &Reference,
    ) -> ComResult<TritangentFillet> {
        Ok(TritangentFillet::new(
            self.com()
                .call(
                    "AddNewTritangentFillet",
                    &[
                        Arg::Obj(face1.com().clone()),
                        Arg::Obj(face2.com().clone()),
                        Arg::Obj(removed_face.com().clone()),
                    ],
                )?
                .as_obj()?,
        ))
    }

    /// 实体三切面倒圆：`AddNewSolidTritangentFillet`。
    pub fn add_new_solid_tritangent_fillet(
        &self,
        face1: &Reference,
        face2: &Reference,
        removed_face: &Reference,
    ) -> ComResult<TritangentFillet> {
        Ok(TritangentFillet::new(
            self.com()
                .call(
                    "AddNewSolidTritangentFillet",
                    &[
                        Arg::Obj(face1.com().clone()),
                        Arg::Obj(face2.com().clone()),
                        Arg::Obj(removed_face.com().clone()),
                    ],
                )?
                .as_obj()?,
        ))
    }

    /// 抽壳：`AddNewShell(移除面 Reference, 内厚度 mm, 外厚度 mm)`。
    pub fn add_new_shell(
        &self,
        face_to_remove: &Reference,
        internal_thickness: f64,
        external_thickness: f64,
    ) -> ComResult<Shell> {
        Ok(Shell::new(
            self.com()
                .call(
                    "AddNewShell",
                    &[
                        Arg::Obj(face_to_remove.com().clone()),
                        Arg::F64(internal_thickness),
                        Arg::F64(external_thickness),
                    ],
                )?
                .as_obj()?,
        ))
    }

    /// 加厚：`AddNewThickness(面 Reference, 偏置 mm)`。
    pub fn add_new_thickness(
        &self,
        face_to_thicken: &Reference,
        offset: f64,
    ) -> ComResult<Thickness> {
        Ok(Thickness::new(
            self.com()
                .call(
                    "AddNewThickness",
                    &[Arg::Obj(face_to_thicken.com().clone()), Arg::F64(offset)],
                )?
                .as_obj()?,
        ))
    }

    /// 移除面：`AddNewRemoveFace(保留面, 移除面)`。
    pub fn add_new_remove_face(
        &self,
        keep_faces: &Reference,
        remove_faces: &Reference,
    ) -> ComResult<RemoveFace> {
        Ok(RemoveFace::new(
            self.com()
                .call(
                    "AddNewRemoveFace",
                    &[
                        Arg::Obj(keep_faces.com().clone()),
                        Arg::Obj(remove_faces.com().clone()),
                    ],
                )?
                .as_obj()?,
        ))
    }

    /// 替换面：`AddNewReplaceFace(分割面, 被替换面, 保留侧 CatSplitSide)`。
    pub fn add_new_replace_face(
        &self,
        split_plane: &Reference,
        remove_face: &Reference,
        splitting_side: i32,
    ) -> ComResult<ReplaceFace> {
        Ok(ReplaceFace::new(
            self.com()
                .call(
                    "AddNewReplaceFace",
                    &[
                        Arg::Obj(split_plane.com().clone()),
                        Arg::Obj(remove_face.com().clone()),
                        Arg::I32(splitting_side),
                    ],
                )?
                .as_obj()?,
        ))
    }

    /// 缝合曲面：`AddNewSewSurface(缝合元素, 保留侧 CatSplitSide)`。
    pub fn add_new_sew_surface(
        &self,
        sewing_element: &Reference,
        sewing_side: i32,
    ) -> ComResult<SewSurface> {
        Ok(SewSurface::new(
            self.com()
                .call(
                    "AddNewSewSurface",
                    &[
                        Arg::Obj(sewing_element.com().clone()),
                        Arg::I32(sewing_side),
                    ],
                )?
                .as_obj()?,
        ))
    }

    /// 封闭曲面成体：`AddNewCloseSurface(封闭元素 Reference)`。
    pub fn add_new_close_surface(&self, close_element: &Reference) -> ComResult<CloseSurface> {
        Ok(CloseSurface::new(
            self.com()
                .call(
                    "AddNewCloseSurface",
                    &[Arg::Obj(close_element.com().clone())],
                )?
                .as_obj()?,
        ))
    }

    /// 多截面实体组合：`AddNewSolidCombine(第一轮廓, 第二轮廓)`。
    pub fn add_new_solid_combine(
        &self,
        profile_first: &Reference,
        profile_second: &Reference,
    ) -> ComResult<SolidCombine> {
        Ok(SolidCombine::new(
            self.com()
                .call(
                    "AddNewSolidCombine",
                    &[
                        Arg::Obj(profile_first.com().clone()),
                        Arg::Obj(profile_second.com().clone()),
                    ],
                )?
                .as_obj()?,
        ))
    }

    /// 带参考的螺纹：`AddNewThreadWithRef(侧面, 界限面)`。
    pub fn add_new_thread_with_ref(
        &self,
        lateral_face: &Reference,
        limit_face: &Reference,
    ) -> ComResult<Thread> {
        Ok(Thread::new(
            self.com()
                .call(
                    "AddNewThreadWithRef",
                    &[
                        Arg::Obj(lateral_face.com().clone()),
                        Arg::Obj(limit_face.com().clone()),
                    ],
                )?
                .as_obj()?,
        ))
    }

    /// 无参考螺纹：`AddNewThreadWithOutRef()`。
    pub fn add_new_thread_with_out_ref(&self) -> ComResult<Thread> {
        Ok(Thread::new(
            self.com().call("AddNewThreadWithOutRef", &[])?.as_obj()?,
        ))
    }

    /// 去特征：`AddNewDefeaturing()`。
    pub fn add_new_defeaturing(&self) -> ComResult<Defeaturing> {
        Ok(Defeaturing::new(
            self.com().call("AddNewDefeaturing", &[])?.as_obj()?,
        ))
    }

    /// 放样：`AddNewLoft()`。
    pub fn add_new_loft(&self) -> ComResult<Loft> {
        Ok(Loft::new(self.com().call("AddNewLoft", &[])?.as_obj()?))
    }

    /// 自动倒圆：`AddNewAutoFillet(倒圆半径, 圆角半径)`。
    pub fn add_new_auto_fillet(
        &self,
        fillet_radius: f64,
        round_radius: f64,
    ) -> ComResult<AutoFillet> {
        Ok(AutoFillet::new(
            self.com()
                .call(
                    "AddNewAutoFillet",
                    &[Arg::F64(fillet_radius), Arg::F64(round_radius)],
                )?
                .as_obj()?,
        ))
    }

    /// 自动拔模：`AddNewAutoDraft(拔模角 deg)`。
    pub fn add_new_auto_draft(&self, draft_angle: f64) -> ComResult<AutoDraft> {
        Ok(AutoDraft::new(
            self.com()
                .call("AddNewAutoDraft", &[Arg::F64(draft_angle)])?
                .as_obj()?,
        ))
    }

    /// 矩形阵列：`AddNewRectPattern(...)`（参数与 pycatia 同序）。
    #[allow(clippy::too_many_arguments)]
    pub fn add_new_rect_pattern(
        &self,
        shape_to_copy: &impl ComObject,
        nb_of_copies_in_dir1: i32,
        nb_of_copies_in_dir2: i32,
        step_in_dir1: f64,
        step_in_dir2: f64,
        position_along_dir1: i32,
        position_along_dir2: i32,
        dir1: &Reference,
        dir2: &Reference,
        is_reversed_dir1: bool,
        is_reversed_dir2: bool,
        rotation_angle: f64,
    ) -> ComResult<RectPattern> {
        Ok(RectPattern::new(
            self.com()
                .call(
                    "AddNewRectPattern",
                    &[
                        Arg::Obj(shape_to_copy.com().clone()),
                        Arg::I32(nb_of_copies_in_dir1),
                        Arg::I32(nb_of_copies_in_dir2),
                        Arg::F64(step_in_dir1),
                        Arg::F64(step_in_dir2),
                        Arg::I32(position_along_dir1),
                        Arg::I32(position_along_dir2),
                        Arg::Obj(dir1.com().clone()),
                        Arg::Obj(dir2.com().clone()),
                        Arg::Bool(is_reversed_dir1),
                        Arg::Bool(is_reversed_dir2),
                        Arg::F64(rotation_angle),
                    ],
                )?
                .as_obj()?,
        ))
    }

    /// 圆周阵列：`AddNewCircPattern(...)`（参数与 pycatia 同序）。
    #[allow(clippy::too_many_arguments)]
    pub fn add_new_circ_pattern(
        &self,
        shape_to_copy: &impl ComObject,
        nb_of_copies_in_radial_dir: i32,
        nb_of_copies_in_angular_dir: i32,
        step_in_radial_dir: f64,
        step_in_angular_dir: f64,
        position_along_radial_dir: i32,
        position_along_angular_dir: i32,
        rotation_center: &Reference,
        rotation_axis: &Reference,
        is_reversed_rotation_axis: bool,
        rotation_angle: f64,
        is_radius_aligned: bool,
    ) -> ComResult<CircPattern> {
        Ok(CircPattern::new(
            self.com()
                .call(
                    "AddNewCircPattern",
                    &[
                        Arg::Obj(shape_to_copy.com().clone()),
                        Arg::I32(nb_of_copies_in_radial_dir),
                        Arg::I32(nb_of_copies_in_angular_dir),
                        Arg::F64(step_in_radial_dir),
                        Arg::F64(step_in_angular_dir),
                        Arg::I32(position_along_radial_dir),
                        Arg::I32(position_along_angular_dir),
                        Arg::Obj(rotation_center.com().clone()),
                        Arg::Obj(rotation_axis.com().clone()),
                        Arg::Bool(is_reversed_rotation_axis),
                        Arg::F64(rotation_angle),
                        Arg::Bool(is_radius_aligned),
                    ],
                )?
                .as_obj()?,
        ))
    }

    /// 用户阵列：`AddNewUserPattern(复制对象, 实例数)`。
    pub fn add_new_user_pattern(
        &self,
        shape_to_copy: &impl ComObject,
        nb_of_copies: i32,
    ) -> ComResult<UserPattern> {
        Ok(UserPattern::new(
            self.com()
                .call(
                    "AddNewUserPattern",
                    &[
                        Arg::Obj(shape_to_copy.com().clone()),
                        Arg::I32(nb_of_copies),
                    ],
                )?
                .as_obj()?,
        ))
    }

    /// 轴到轴变换：`AddNewAxisToAxis2(参考轴系, 目标轴系)`。
    /// 注意: 成功时 COM 返回值也是 None(动态调度), 故只返回 ();
    /// 结果用「Body 内形状数 + 重心变化」验证（与 Python 版同口径）。
    pub fn add_new_axis_to_axis2(
        &self,
        reference_axis: &Reference,
        target_axis: &Reference,
    ) -> ComResult<()> {
        self.com()
            .call(
                "AddNewAxisToAxis2",
                &[
                    Arg::Obj(reference_axis.com().clone()),
                    Arg::Obj(target_axis.com().clone()),
                ],
            )
            .map(|_| ())
    }
}
