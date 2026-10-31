//! `Application`：CATIA 根对象。

use crate::collections::ComCollection;
use crate::dispatch::{Arg, ComObject};
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

    /// 开关文件操作弹框（SaveAs / Open 过程中避免确认对话框卡住自动化）。
    pub fn set_display_file_alerts(&self, enabled: bool) -> ComResult<()> {
        self.com().put("DisplayFileAlerts", Arg::Bool(enabled))
    }

    /// 文件搜索顺序（分号分隔的目录列表，断链重链按此顺序找文件）。
    pub fn file_search_order(&self) -> ComResult<String> {
        self.com().get("FileSearchOrder")?.as_string()
    }

    pub fn set_file_search_order(&self, value: &str) -> ComResult<()> {
        self.com()
            .put("FileSearchOrder", Arg::Str(value.to_string()))
    }

    /// 全局服务（环境变量 / VBA 注入）。
    pub fn system_service(&self) -> ComResult<crate::objects::SystemService> {
        Ok(crate::objects::SystemService::new(
            self.com().get("SystemService")?.as_obj()?,
        ))
    }
}
