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
