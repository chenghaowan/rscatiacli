//! `ShapeFactory`：Part Design 实体特征工厂（`Part.ShapeFactory`）。

use crate::dispatch::{Arg, ComObject};
use crate::error::ComResult;

use super::part::Entity;
use super::reference::Reference;

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
    ) -> ComResult<Entity> {
        Ok(Entity::new(
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
}
