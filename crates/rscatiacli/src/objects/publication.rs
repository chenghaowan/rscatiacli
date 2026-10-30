//! `Publications` / `Publication`（发布）。
//!
//! 跨文档复制（`Selection.Copy` → `Paste`）前对象必须已发布，否则 CATIA
//! 剪贴板传不过去。发布集合挂在 `Product` 上（`Product.Publications`）——
//! PartDocument 同样暴露 `Product` 属性。

use crate::collections::ComCollection;
use crate::dispatch::{Arg, ComObject};
use crate::error::ComResult;

use super::reference::Reference;

com_wrapper!(Publications);
com_wrapper!(Publication);

impl ComCollection for Publications {
    type Item = Publication;
}

impl Publications {
    /// 按名字新增发布（`Add` 返回新建的 Publication）。
    pub fn add(&self, name: &str) -> ComResult<Publication> {
        Ok(Publication::new(
            self.com()
                .call("Add", &[Arg::Str(name.to_string())])?
                .as_obj()?,
        ))
    }

    /// 把发布指向指定 `Reference`（`SetDirect`）。
    pub fn set_direct(&self, name: &str, reference: &Reference) -> ComResult<()> {
        self.com().call(
            "SetDirect",
            &[
                Arg::Str(name.to_string()),
                Arg::Obj(reference.com().clone()),
            ],
        )?;
        Ok(())
    }

    /// 已发布的名字列表。个别发布读不出名字时跳过（与 Python 版口径一致）。
    pub fn names(&self) -> Vec<String> {
        let Ok(count) = self.count() else {
            return Vec::new();
        };
        (1..=count)
            .filter_map(|i| self.item(i).ok())
            .filter_map(|p| p.name().ok())
            .collect()
    }
}
