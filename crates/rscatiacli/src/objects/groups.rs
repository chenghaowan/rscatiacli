//! `Groups` / `Group`：DMU 组（挂根产品的技术对象，`Product.GetTechnologicalObject("Groups")`）。
//! find_near_parts 用它包住目标/候选零件做 SPA Distance 测量。

use crate::collections::ComCollection;
use crate::dispatch::{Arg, ComObject};
use crate::error::ComResult;

com_wrapper!(Groups);
com_wrapper!(Group);

impl ComCollection for Groups {
    type Item = Group;
}

impl Groups {
    /// 新建空组。
    pub fn add(&self) -> ComResult<Group> {
        Ok(Group::new(self.com().call("Add", &[])?.as_obj()?))
    }

    /// 按名字删除组（AddExplicit 过的组用名字移除，与 Python 版一致）。
    pub fn remove(&self, name: &str) -> ComResult<()> {
        self.com()
            .call("Remove", &[Arg::Str(name.to_string())])
            .map(|_| ())
    }
}

impl Group {
    /// 把产品实例加进组（显式成员）。
    pub fn add_explicit(&self, product: &impl ComObject) -> ComResult<()> {
        self.com()
            .call("AddExplicit", &[Arg::Obj(product.com().clone())])
            .map(|_| ())
    }
}
