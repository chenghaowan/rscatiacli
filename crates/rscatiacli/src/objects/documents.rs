//! `Documents` 集合、通用 `Document`，以及 pycatia 对应的
//! `PartDocument` / `ProductDocument`。

use std::ops::Deref;

use crate::collections::ComCollection;
use crate::dispatch::{ComObj, ComObject, FromComObj};
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

    pub fn selection(&self) -> ComResult<Selection> {
        Ok(Selection::new(self.com().get("Selection")?.as_obj()?))
    }

    /// 把文档切回前台（装配处理过程中会切到各 Part 文档）。
    pub fn activate(&self) -> ComResult<()> {
        self.com().call("Activate", &[]).map(|_| ())
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
