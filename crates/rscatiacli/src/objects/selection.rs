//! `Selection` / `SelectionItem` / `VisProperties`。

use crate::collections::ComCollection;
use crate::dispatch::{Arg, ComObj, ComObject};
use crate::error::ComResult;

use super::part::Entity;

com_wrapper!(Selection);
com_wrapper!(SelectionItem);
com_wrapper!(VisProperties);

impl ComCollection for Selection {
    type Item = SelectionItem;
}

impl Selection {
    pub fn clear(&self) -> ComResult<()> {
        self.com().call("Clear", &[])?;
        Ok(())
    }

    /// 把对象加入选择集。
    pub fn add<O: ComObject>(&self, object: &O) -> ComResult<()> {
        self.com().call("Add", &[Arg::Obj(object.com().clone())])?;
        Ok(())
    }

    /// 执行 CATIA 搜索式，结果留在选择集里。
    pub fn search(&self, query: &str) -> ComResult<()> {
        self.com().call("Search", &[Arg::Str(query.to_string())])?;
        Ok(())
    }

    pub fn vis_properties(&self) -> ComResult<VisProperties> {
        Ok(VisProperties::new(
            self.com().get("VisProperties")?.as_obj()?,
        ))
    }
}

impl SelectionItem {
    /// 选择项对应的实际对象。
    pub fn value(&self) -> ComResult<Entity> {
        Ok(Entity::new(self.raw_value()?))
    }

    pub fn raw_value(&self) -> ComResult<ComObj> {
        self.com().get("Value")?.as_obj()
    }
}

impl VisProperties {
    /// 显隐：保留 CATIA 原始语义（1 = 隐藏，0 = 显示），不包装成 bool，避免口径反转。
    pub fn set_show(&self, value: i32) -> ComResult<()> {
        self.com().call("SetShow", &[Arg::I32(value)])?;
        Ok(())
    }

    /// 设置对象自身颜色。`inherit` = 0 表示不继承父级颜色。
    pub fn set_real_color(&self, r: i32, g: i32, b: i32, inherit: i32) -> ComResult<()> {
        self.com().call(
            "SetRealColor",
            &[Arg::I32(r), Arg::I32(g), Arg::I32(b), Arg::I32(inherit)],
        )?;
        Ok(())
    }
}
