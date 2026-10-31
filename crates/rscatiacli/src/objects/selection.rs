//! `Selection` / `SelectionItem` / `VisProperties`。

use crate::collections::ComCollection;
use crate::dispatch::{Arg, ComObj, ComObject};
use crate::error::ComResult;

use super::part::Entity;
use super::reference::Reference;

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

    /// 复制选中对象到 CATIA 剪贴板（跨文档复制前对象需已发布）。
    pub fn copy(&self) -> ComResult<()> {
        self.com().call("Copy", &[]).map(|_| ())
    }

    /// 粘贴到当前选中的容器。
    pub fn paste(&self) -> ComResult<()> {
        self.com().call("Paste", &[]).map(|_| ())
    }

    /// 特殊格式粘贴（如 "CATPrtResult"）。
    pub fn paste_special(&self, format: &str) -> ComResult<()> {
        self.com()
            .call("PasteSpecial", &[Arg::Str(format.to_string())])
            .map(|_| ())
    }

    /// 删除选中对象。
    pub fn delete(&self) -> ComResult<()> {
        self.com().call("Delete", &[]).map(|_| ())
    }

    /// 从选择集移除第 index(1-based) 项（不移除对象本身）。
    pub fn remove(&self, index: i32) -> ComResult<()> {
        self.com().call("Remove", &[Arg::I32(index)]).map(|_| ())
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

    /// 选择项对应的 `Reference`（发布 SetDirect 用）。
    pub fn reference(&self) -> ComResult<Reference> {
        Ok(Reference::new(self.com().get("Reference")?.as_obj()?))
    }
}

impl VisProperties {
    /// 读当前显隐状态：0 = 显示，1 = 隐藏（保留 CATIA 原始语义）。
    pub fn get_show(&self) -> ComResult<i32> {
        self.com().call("GetShow", &[])?.as_i32()
    }

    /// 显隐：保留 CATIA 原始语义（1 = 隐藏，0 = 显示），不包装成 bool，避免口径反转。
    pub fn set_show(&self, value: i32) -> ComResult<()> {
        self.com().call("SetShow", &[Arg::I32(value)])?;
        Ok(())
    }

    /// 线宽（包围盒 dz 约束加粗用）。
    pub fn set_real_width(&self, width: i32, inherit: i32) -> ComResult<()> {
        self.com()
            .call("SetRealWidth", &[Arg::I32(width), Arg::I32(inherit)])?;
        Ok(())
    }

    /// 不透明度（0-255，扫掠体半透明观感）。
    pub fn set_real_opacity(&self, opacity: i32, inherit: i32) -> ComResult<()> {
        self.com()
            .call("SetRealOpacity", &[Arg::I32(opacity), Arg::I32(inherit)])?;
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
