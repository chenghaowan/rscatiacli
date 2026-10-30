//! `Product` 与 `Products`（装配结构）。

use crate::collections::ComCollection;
use crate::dispatch::{Arg, ComObject};
use crate::error::ComResult;

use super::documents::Document;
use super::publication::Publications;

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

    /// 零件号（`PartNumber`）。
    pub fn part_number(&self) -> ComResult<String> {
        self.com().get("PartNumber")?.as_string()
    }

    pub fn set_part_number(&self, value: &str) -> ComResult<()> {
        self.com().put("PartNumber", Arg::Str(value.to_string()))
    }

    /// 发布集合（跨文档复制前需要先发布）。Part 文档同样暴露 Product。
    pub fn publications(&self) -> ComResult<Publications> {
        Ok(Publications::new(self.com().get("Publications")?.as_obj()?))
    }
}

impl Products {
    /// 把外部组件（已打开的文档）加进装配，返回新增实例。
    pub fn add_external_component(&self, document: &Document) -> ComResult<Product> {
        Ok(Product::new(
            self.com()
                .call("AddExternalComponent", &[Arg::Obj(document.com().clone())])?
                .as_obj()?,
        ))
    }

    /// 把组件的引用重指向另一个 Product（断链重连）。
    /// PartDocument / ProductDocument 均暴露 Product 属性，未保存的内存文档也能作为目标。
    pub fn replace_product(
        &self,
        component: &impl ComObject,
        new_reference: &impl ComObject,
        keep_data: bool,
    ) -> ComResult<()> {
        self.com()
            .call(
                "ReplaceProduct",
                &[
                    Arg::Obj(component.com().clone()),
                    Arg::Obj(new_reference.com().clone()),
                    Arg::Bool(keep_data),
                ],
            )
            .map(|_| ())
    }
}
