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

    /// 拉伸：`AddNewPad(轮廓 Sketch, 高度 mm)`（沿 +Z; 失败多为轮廓自相交）。
    pub fn add_new_pad(&self, profile: &impl ComObject, height: f64) -> ComResult<Entity> {
        Ok(Entity::new(
            self.com()
                .call(
                    "AddNewPad",
                    &[Arg::Obj(profile.com().clone()), Arg::F64(height)],
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
