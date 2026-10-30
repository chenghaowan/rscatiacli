//! `HybridShapeFactory`：几何 datum（去参结果）的创建与清理。
//!
//! 方法名与 CATIA 接口一致（AddNewPointDatum / AddNewLineDatum / ...），不做泛化命名。
//! `GetGeometricalFeatureType` 返回值与 CATIA 一致：1=点, 2=曲线, 3=曲面, 8=体积；
//! CATIA 偶尔误判类型（曲线被判成曲面），调用方应按候选顺序逐个尝试而不是依赖它。

use crate::dispatch::{Arg, ComObject};
use crate::error::ComResult;

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
