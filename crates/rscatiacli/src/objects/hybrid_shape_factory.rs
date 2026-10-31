//! `HybridShapeFactory`：几何 datum（去参结果）的创建与清理。
//!
//! 方法名与 CATIA 接口一致（AddNewPointDatum / AddNewLineDatum / ...），不做泛化命名。
//! `GetGeometricalFeatureType` 返回值与 CATIA 一致：1=点, 2=曲线, 3=曲面, 8=体积；
//! CATIA 偶尔误判类型（曲线被判成曲面），调用方应按候选顺序逐个尝试而不是依赖它。

use crate::dispatch::{Arg, ComObject};
use crate::error::ComResult;

use super::hybrid_shapes::{HybridShapeJoin, HybridShapeOffset};
use super::part::Entity;
use super::reference::Reference;

com_wrapper!(HybridShapeFactory);

impl HybridShapeFactory {
    /// 几何类型（1=点, 2=曲线, 3=曲面, 8=体积）。
    pub fn get_geometrical_feature_type(&self, reference: &Reference) -> ComResult<i32> {
        self.com()
            .call(
                "GetGeometricalFeatureType",
                &[Arg::Obj(reference.com().clone())],
            )?
            .as_i32()
    }

    /// 缝合前两个曲面，其余用 `HybridShapeJoin::add_element` 追加。
    pub fn add_new_join(&self, ref_a: &Reference, ref_b: &Reference) -> ComResult<HybridShapeJoin> {
        Ok(HybridShapeJoin::new(
            self.com()
                .call(
                    "AddNewJoin",
                    &[Arg::Obj(ref_a.com().clone()), Arg::Obj(ref_b.com().clone())],
                )?
                .as_obj()?,
        ))
    }

    /// 偏置曲面：`AddNewOffset(Reference, OffsetValue, OffsetDirection, Deviation)`。
    pub fn add_new_offset(
        &self,
        reference: &Reference,
        offset_value: f64,
        direction: bool,
        deviation: f64,
    ) -> ComResult<HybridShapeOffset> {
        Ok(HybridShapeOffset::new(
            self.com()
                .call(
                    "AddNewOffset",
                    &[
                        Arg::Obj(reference.com().clone()),
                        Arg::F64(offset_value),
                        Arg::Bool(direction),
                        Arg::F64(deviation),
                    ],
                )?
                .as_obj()?,
        ))
    }

    /// 曲面的整体边界特征（配合 add_new_datums 数边界环数，与 FIS AST 宏同口径）。
    pub fn add_new_boundary_of_surface(&self, reference: &Reference) -> ComResult<Entity> {
        Ok(Entity::new(
            self.com()
                .call(
                    "AddNewBoundaryOfSurface",
                    &[Arg::Obj(reference.com().clone())],
                )?
                .as_obj()?,
        ))
    }

    /// 按域拆分边界特征，返回的 datum 数 = 边界环数。
    pub fn add_new_datums(&self, reference: &Reference) -> ComResult<Vec<Entity>> {
        Ok(self
            .com()
            .call("AddNewDatums", &[Arg::Obj(reference.com().clone())])?
            .as_obj_array()?
            .into_iter()
            .map(Entity::new)
            .collect())
    }

    /// 按坐标建方向特征（包围盒 6 向极值的迭代方向）。
    pub fn add_new_direction_by_coord(&self, x: f64, y: f64, z: f64) -> ComResult<Entity> {
        Ok(Entity::new(
            self.com()
                .call(
                    "AddNewDirectionByCoord",
                    &[Arg::F64(x), Arg::F64(y), Arg::F64(z)],
                )?
                .as_obj()?,
        ))
    }

    /// 提取（把 Body 转成可测的 GSD 特征；多 body 包围盒每 body 一个）。
    pub fn add_new_extract(
        &self,
        reference: &Reference,
    ) -> ComResult<super::hybrid_shapes::Extract> {
        Ok(super::hybrid_shapes::Extract::new(
            self.com()
                .call("AddNewExtract", &[Arg::Obj(reference.com().clone())])?
                .as_obj()?,
        ))
    }

    /// 极值点：`AddNewExtremum(元素, 方向, 极性 1=Max/0=Min)`。
    pub fn add_new_extremum(
        &self,
        element: &Reference,
        direction: &Reference,
        polarity: i32,
    ) -> ComResult<Entity> {
        Ok(Entity::new(
            self.com()
                .call(
                    "AddNewExtremum",
                    &[
                        Arg::Obj(element.com().clone()),
                        Arg::Obj(direction.com().clone()),
                        Arg::I32(polarity),
                    ],
                )?
                .as_obj()?,
        ))
    }

    /// 重合 PointCoord（极值坐标的可靠读取路径，Extremum 自身 get_point 常
    /// 以脚本错误失败 —— ltl-manage 实测）。
    pub fn add_new_point_coord_with_reference(
        &self,
        x: f64,
        y: f64,
        z: f64,
        reference: &impl crate::dispatch::ComObject,
    ) -> ComResult<Entity> {
        Ok(Entity::new(
            self.com()
                .call(
                    "AddNewPointCoordWithReference",
                    &[
                        Arg::F64(x),
                        Arg::F64(y),
                        Arg::F64(z),
                        Arg::Obj(reference.com().clone()),
                    ],
                )?
                .as_obj()?,
        ))
    }

    /// 控制 GSD 特征在树/屏幕上的可见性（0 = 不可见）。
    pub fn gsm_visibility(
        &self,
        object: &impl crate::dispatch::ComObject,
        visibility: i32,
    ) -> ComResult<()> {
        self.com()
            .call(
                "GSMVisibility",
                &[Arg::Obj(object.com().clone()), Arg::I32(visibility)],
            )
            .map(|_| ())
    }

    /// 两线建平面（草图绝对轴 H/V → 基准平面）。
    pub fn add_new_plane2_lines(&self, line1: &Reference, line2: &Reference) -> ComResult<Entity> {
        Ok(Entity::new(
            self.com()
                .call(
                    "AddNewPlane2Lines",
                    &[Arg::Obj(line1.com().clone()), Arg::Obj(line2.com().clone())],
                )?
                .as_obj()?,
        ))
    }

    /// 过点偏移平面（贴合极值点的上下平面）。
    pub fn add_new_plane_offset_pt(
        &self,
        plane: &Reference,
        point: &Reference,
    ) -> ComResult<Entity> {
        Ok(Entity::new(
            self.com()
                .call(
                    "AddNewPlaneOffsetPt",
                    &[Arg::Obj(plane.com().clone()), Arg::Obj(point.com().clone())],
                )?
                .as_obj()?,
        ))
    }

    /// 投影（底点投到顶面；Python 侧混传对象/引用）。
    pub fn add_new_project(
        &self,
        element: &impl crate::dispatch::ComObject,
        support: &impl crate::dispatch::ComObject,
    ) -> ComResult<Entity> {
        Ok(Entity::new(
            self.com()
                .call(
                    "AddNewProject",
                    &[
                        Arg::Obj(element.com().clone()),
                        Arg::Obj(support.com().clone()),
                    ],
                )?
                .as_obj()?,
        ))
    }

    /// 两点连线（Z 向引导线）。
    pub fn add_new_line_pt_pt(
        &self,
        pt1: &Reference,
        pt2: &Reference,
    ) -> ComResult<super::hybrid_shapes::HybridShapeLinePtPt> {
        Ok(super::hybrid_shapes::HybridShapeLinePtPt::new(
            self.com()
                .call(
                    "AddNewLinePtPt",
                    &[Arg::Obj(pt1.com().clone()), Arg::Obj(pt2.com().clone())],
                )?
                .as_obj()?,
        ))
    }

    /// 点 + 方向建直线（投影方向参考线；start/end 为起止参数，orientation 控制朝向）。
    pub fn add_new_line_pt_dir(
        &self,
        pt: &Reference,
        direction: &Reference,
        start: f64,
        end: f64,
        orientation: bool,
    ) -> ComResult<Entity> {
        Ok(Entity::new(
            self.com()
                .call(
                    "AddNewLinePtDir",
                    &[
                        Arg::Obj(pt.com().clone()),
                        Arg::Obj(direction.com().clone()),
                        Arg::F64(start),
                        Arg::F64(end),
                        Arg::Bool(orientation),
                    ],
                )?
                .as_obj()?,
        ))
    }

    /// 按坐标建点（投影方向参考线起点/压扁法 COG 参考点等）。
    pub fn add_new_point_coord(&self, x: f64, y: f64, z: f64) -> ComResult<Entity> {
        Ok(Entity::new(
            self.com()
                .call("AddNewPointCoord", &[Arg::F64(x), Arg::F64(y), Arg::F64(z)])?
                .as_obj()?,
        ))
    }

    /// 方程建平面（Ax+By+Cz+D=0，tooling axis XY 投影面）。
    pub fn add_new_plane_equation(&self, a: f64, b: f64, c: f64, d: f64) -> ComResult<Entity> {
        Ok(Entity::new(
            self.com()
                .call(
                    "AddNewPlaneEquation",
                    &[Arg::F64(a), Arg::F64(b), Arg::F64(c), Arg::F64(d)],
                )?
                .as_obj()?,
        ))
    }

    /// 过曲线、过点的法平面（压扁法量尺寸的基准平面）。
    pub fn add_new_plane_normal(&self, line: &Reference, point: &Reference) -> ComResult<Entity> {
        Ok(Entity::new(
            self.com()
                .call(
                    "AddNewPlaneNormal",
                    &[Arg::Obj(line.com().clone()), Arg::Obj(point.com().clone())],
                )?
                .as_obj()?,
        ))
    }

    /// 空平移（把模板输出平移回 tooling axis XY 平面）。
    pub fn add_new_empty_translate(&self) -> ComResult<super::hybrid_shapes::HybridShapeTranslate> {
        Ok(super::hybrid_shapes::HybridShapeTranslate::new(
            self.com().call("AddNewEmptyTranslate", &[])?.as_obj()?,
        ))
    }

    /// 仿射变形（压扁法：xy 比例 1，z 比例 = scale，把封闭表面压到平面上）。
    pub fn add_new_affinity(
        &self,
        reference: &Reference,
        ratio_x: f64,
        ratio_y: f64,
        ratio_z: f64,
    ) -> ComResult<super::hybrid_shapes::HybridShapeAffinity> {
        Ok(super::hybrid_shapes::HybridShapeAffinity::new(
            self.com()
                .call(
                    "AddNewAffinity",
                    &[
                        Arg::Obj(reference.com().clone()),
                        Arg::F64(ratio_x),
                        Arg::F64(ratio_y),
                        Arg::F64(ratio_z),
                    ],
                )?
                .as_obj()?,
        ))
    }

    /// 显式扫掠（草图沿引导线 → 包围盒实体）。
    pub fn add_new_sweep_explicit(
        &self,
        profile: &Reference,
        guide: &Reference,
    ) -> ComResult<Entity> {
        Ok(Entity::new(
            self.com()
                .call(
                    "AddNewSweepExplicit",
                    &[
                        Arg::Obj(profile.com().clone()),
                        Arg::Obj(guide.com().clone()),
                    ],
                )?
                .as_obj()?,
        ))
    }

    /// GSD 曲面分割：`AddNewHybridSplit(曲面, 分割元素, Orientation(1/-1))`。
    pub fn add_new_hybrid_split(
        &self,
        to_split: &Reference,
        splitting: &Reference,
        orientation: i32,
    ) -> ComResult<Entity> {
        Ok(Entity::new(
            self.com()
                .call(
                    "AddNewHybridSplit",
                    &[
                        Arg::Obj(to_split.com().clone()),
                        Arg::Obj(splitting.com().clone()),
                        Arg::I32(orientation),
                    ],
                )?
                .as_obj()?,
        ))
    }

    pub fn add_new_point_datum(&self, reference: &Reference) -> ComResult<Entity> {
        self.datum("AddNewPointDatum", reference)
    }

    pub fn add_new_line_datum(&self, reference: &Reference) -> ComResult<Entity> {
        self.datum("AddNewLineDatum", reference)
    }

    pub fn add_new_circle_datum(&self, reference: &Reference) -> ComResult<Entity> {
        self.datum("AddNewCircleDatum", reference)
    }

    pub fn add_new_curve_datum(&self, reference: &Reference) -> ComResult<Entity> {
        self.datum("AddNewCurveDatum", reference)
    }

    pub fn add_new_surface_datum(&self, reference: &Reference) -> ComResult<Entity> {
        self.datum("AddNewSurfaceDatum", reference)
    }

    pub fn add_new_volume_datum(&self, reference: &Reference) -> ComResult<Entity> {
        self.datum("AddNewVolumeDatum", reference)
    }

    /// 删除临时副本（datum 化后只留 datum）。
    pub fn delete_object_for_datum(&self, object: &impl ComObject) -> ComResult<()> {
        self.com()
            .call("DeleteObjectForDatum", &[Arg::Obj(object.com().clone())])
            .map(|_| ())
    }

    fn datum(&self, method: &str, reference: &Reference) -> ComResult<Entity> {
        Ok(Entity::new(
            self.com()
                .call(method, &[Arg::Obj(reference.com().clone())])?
                .as_obj()?,
        ))
    }
}
