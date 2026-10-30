//! 集合的通用遍历：`Count` + `Item(i)`。

use crate::dispatch::{item_at, ComObj, ComObject, FromComObj};
use crate::error::ComResult;

/// CATIA 集合（Documents / Bodies / HybridBodies / Products / Selection ...）。
///
/// 默认实现负责 `Item` 的 METHOD → propget 回退，类型化包装只需声明 `Item` 类型。
pub trait ComCollection: ComObject {
    type Item: FromComObj;

    fn count(&self) -> ComResult<i32> {
        self.com().get("Count")?.as_i32()
    }

    fn raw_item(&self, index: i32) -> ComResult<ComObj> {
        item_at(self.com(), index)
    }

    fn item(&self, index: i32) -> ComResult<Self::Item> {
        Ok(Self::Item::from_com(self.raw_item(index)?))
    }

    fn items(&self) -> ComResult<Vec<Self::Item>> {
        let count = self.count()?;
        let mut out = Vec::with_capacity(count.max(0) as usize);
        for i in 1..=count {
            out.push(self.item(i)?);
        }
        Ok(out)
    }

    fn is_empty(&self) -> ComResult<bool> {
        Ok(self.count()? == 0)
    }
}
