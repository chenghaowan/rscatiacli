//! `Product` 与 `Products`（装配结构）。

use crate::collections::ComCollection;
use crate::dispatch::{Arg, ComObject};
use crate::error::ComResult;

com_wrapper!(Product);
com_wrapper!(Products);

impl ComCollection for Products {
    type Item = Product;
}

impl Product {
    /// 子装配 / 零件实例集合。
    pub fn products(&self) -> ComResult<Products> {
        Ok(Products::new(self.com().get("Products")?.as_obj()?))
    }

    /// 实例对应的参考装配（`ReferenceProduct`）。
    pub fn reference_product(&self) -> ComResult<Product> {
        Ok(Product::new(self.com().get("ReferenceProduct")?.as_obj()?))
    }

    /// CatWorkModeType：DESIGN_MODE = 2。CGR 可视化实例必须先切设计模式。
    pub fn apply_work_mode(&self, mode: i32) -> ComResult<()> {
        self.com().call("ApplyWorkMode", &[Arg::I32(mode)])?;
        Ok(())
    }

    /// 激活终端节点（CATIA 的 Activate Terminal Node）。
    pub fn activate_default_shape(&self) -> ComResult<()> {
        self.com().call("ActivateDefaultShape", &[])?;
        Ok(())
    }
}
