//! `Application`：CATIA 根对象。

use crate::collections::ComCollection;
use crate::dispatch::ComObject;
use crate::error::ComResult;

use super::documents::{Document, Documents};

com_wrapper!(Application);

impl Application {
    /// 已打开文档集合。
    pub fn documents(&self) -> ComResult<Documents> {
        Ok(Documents::new(self.com().get("Documents")?.as_obj()?))
    }

    /// 活动文档。没有文档时 CATIA 会返回错误。
    pub fn active_document(&self) -> ComResult<Document> {
        Ok(Document::new(self.com().get("ActiveDocument")?.as_obj()?))
    }

    /// 已打开文档数量。
    pub fn document_count(&self) -> ComResult<i32> {
        self.documents()?.count()
    }
}
