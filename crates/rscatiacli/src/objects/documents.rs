//! `Documents` 集合与 `Document`。

use crate::collections::ComCollection;
use crate::dispatch::ComObject;
use crate::error::ComResult;

use super::part::Part;
use super::product::Product;
use super::selection::Selection;

com_wrapper!(Documents);
com_wrapper!(Document);

impl ComCollection for Documents {
    type Item = Document;
}

impl Document {
    /// 文档完整路径。
    pub fn full_name(&self) -> ComResult<String> {
        self.com().get("FullName")?.as_string()
    }

    pub fn is_read_only(&self) -> ComResult<bool> {
        self.com().get("IsReadOnly")?.as_bool()
    }

    /// 零件文档的 `Part` 根对象。装配 / 工程图会返回错误。
    pub fn part(&self) -> ComResult<Part> {
        Ok(Part::new(self.com().get("Part")?.as_obj()?))
    }

    /// 装配文档的 `Product` 根对象。零件 / 工程图会返回错误。
    pub fn product(&self) -> ComResult<Product> {
        Ok(Product::new(self.com().get("Product")?.as_obj()?))
    }

    pub fn selection(&self) -> ComResult<Selection> {
        Ok(Selection::new(self.com().get("Selection")?.as_obj()?))
    }

    /// 把文档切回前台（装配处理过程中会切到各 Part 文档）。
    pub fn activate(&self) -> ComResult<()> {
        self.com().call("Activate", &[]).map(|_| ())
    }
}
