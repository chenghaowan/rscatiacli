//! `Documents` 集合、通用 `Document`，以及 pycatia 对应的
//! `PartDocument` / `ProductDocument`。

use std::ops::Deref;

use crate::collections::ComCollection;
use crate::dispatch::{Arg, ComObj, ComObject, FromComObj};
use crate::error::ComResult;

use super::part::Part;
use super::product::Product;
use super::selection::Selection;
use super::spa_workbench::SPAWorkbench;

com_wrapper!(Documents);
com_wrapper!(Document);

impl ComCollection for Documents {
    type Item = Document;
}

impl Documents {
    /// 新建指定类型文档（`Add("Part" / "Product")`）。
    pub fn add(&self, doc_type: &str) -> ComResult<Document> {
        Ok(Document::new(
            self.com()
                .call("Add", &[Arg::Str(doc_type.to_string())])?
                .as_obj()?,
        ))
    }

    /// 打开指定路径的文档。
    pub fn open(&self, path: &str) -> ComResult<Document> {
        Ok(Document::new(
            self.com()
                .call("Open", &[Arg::Str(path.to_string())])?
                .as_obj()?,
        ))
    }
}

impl Document {
    /// 文档完整路径。
    pub fn full_name(&self) -> ComResult<String> {
        self.com().get("FullName")?.as_string()
    }

    pub fn is_read_only(&self) -> ComResult<bool> {
        self.com().get("IsReadOnly")?.as_bool()
    }

    pub fn selection(&self) -> ComResult<Selection> {
        Ok(Selection::new(self.com().get("Selection")?.as_obj()?))
    }

    /// 把文档切回前台（装配处理过程中会切到各 Part 文档）。
    pub fn activate(&self) -> ComResult<()> {
        self.com().call("Activate", &[]).map(|_| ())
    }

    /// 保存文档（按当前路径）。
    pub fn save(&self) -> ComResult<()> {
        self.com().call("Save", &[]).map(|_| ())
    }

    /// 另存为指定路径。
    pub fn save_as(&self, path: &str) -> ComResult<()> {
        self.com()
            .call("SaveAs", &[Arg::Str(path.to_string())])
            .map(|_| ())
    }

    /// 关闭文档。
    pub fn close(&self) -> ComResult<()> {
        self.com().call("Close", &[]).map(|_| ())
    }

    /// 触发重算 / 重链。
    pub fn update(&self) -> ComResult<()> {
        self.com().call("Update", &[]).map(|_| ())
    }

    /// 构造零件文档包装。等价于 pycatia 的 `PartDocument(doc.com_object)`；
    /// 非零件文档在调用 `.part()` 时才报错。
    pub fn part_document(&self) -> PartDocument {
        PartDocument::from_document(self.clone())
    }

    /// 构造装配文档包装。等价于 pycatia 的 `ProductDocument(doc.com_object)`；
    /// 非装配文档在调用 `.product()` 时才报错。
    pub fn product_document(&self) -> ProductDocument {
        ProductDocument::from_document(self.clone())
    }
}

/// 零件文档（对应 pycatia `PartDocument`）。
#[derive(Clone)]
pub struct PartDocument(Document);

impl PartDocument {
    pub fn new(obj: ComObj) -> Self {
        Self(Document::new(obj))
    }

    pub fn from_document(document: Document) -> Self {
        Self(document)
    }

    /// 取回通用 `Document`。
    pub fn document(&self) -> &Document {
        &self.0
    }

    /// `Part` 根对象。装配 / 工程图会返回错误。
    pub fn part(&self) -> ComResult<Part> {
        Ok(Part::new(self.0.com().get("Part")?.as_obj()?))
    }

    /// Part 文档同样暴露 `Product` 属性（发布 / 零件号用）。
    pub fn product(&self) -> ComResult<Product> {
        Ok(Product::new(self.0.com().get("Product")?.as_obj()?))
    }

    /// 空间分析工作台（体积等测量）。
    pub fn spa_workbench(&self) -> ComResult<SPAWorkbench> {
        Ok(SPAWorkbench::new(
            self.0.com().get("SPAWorkbench")?.as_obj()?,
        ))
    }
}

impl ComObject for PartDocument {
    fn com(&self) -> &ComObj {
        self.0.com()
    }
}

impl FromComObj for PartDocument {
    fn from_com(obj: ComObj) -> Self {
        Self(Document::new(obj))
    }
}

impl From<Document> for PartDocument {
    fn from(document: Document) -> Self {
        Self(document)
    }
}

impl Deref for PartDocument {
    type Target = Document;

    fn deref(&self) -> &Document {
        &self.0
    }
}

/// 装配文档（对应 pycatia `ProductDocument`）。
#[derive(Clone)]
pub struct ProductDocument(Document);

impl ProductDocument {
    pub fn new(obj: ComObj) -> Self {
        Self(Document::new(obj))
    }

    pub fn from_document(document: Document) -> Self {
        Self(document)
    }

    /// 取回通用 `Document`。
    pub fn document(&self) -> &Document {
        &self.0
    }

    /// `Product` 根对象。零件 / 工程图会返回错误。
    pub fn product(&self) -> ComResult<Product> {
        Ok(Product::new(self.0.com().get("Product")?.as_obj()?))
    }
}

impl ComObject for ProductDocument {
    fn com(&self) -> &ComObj {
        self.0.com()
    }
}

impl FromComObj for ProductDocument {
    fn from_com(obj: ComObj) -> Self {
        Self(Document::new(obj))
    }
}

impl From<Document> for ProductDocument {
    fn from(document: Document) -> Self {
        Self(document)
    }
}

impl Deref for ProductDocument {
    type Target = Document;

    fn deref(&self) -> &Document {
        &self.0
    }
}
