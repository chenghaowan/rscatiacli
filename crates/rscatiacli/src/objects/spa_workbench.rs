//! `SPAWorkbench` / `Measurable`（空间分析工作台：测量）。
//!
//! `Document.SPAWorkbench` 取工作台，`GetMeasurable(reference)` 取测量对象。
//! 数值成员按需补充（当前工具只用 `Volume`）。

use crate::dispatch::{Arg, ComObject};
use crate::error::ComResult;

use super::reference::Reference;

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
}
