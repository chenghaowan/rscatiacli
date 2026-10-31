//! `Part.GetCustomerFactory("InstanceFactory")`：用户特征模板实例化。
//!
//! 对应 rule_projected_surface.py 里绕过 pycatia 的直接 COM 调用，实例化
//! PROJECTED_AREA.CATPart 模板（ProjectedArea）。流程固定：
//! `BeginInstanceFactory(模板名, 模板路径)` → `BeginInstantiate` →
//! `PutInputData(输入名, 原始 HybridShape)`（注意传特征对象而非 Reference）
//! → `Instantiate` → `EndInstantiate` → `EndInstanceFactory`。
//! 实例化产物靠 `Selection.Search("Name=FIS_ProjectedArea*,all")` 找回。

use crate::dispatch::{Arg, ComObject};
use crate::error::ComResult;

use super::part::Entity;

com_wrapper!(InstanceFactory);

impl InstanceFactory {
    /// 载入模板（模板名 + .CATPart 路径）。
    pub fn begin_instance_factory(&self, name: &str, template_path: &str) -> ComResult<()> {
        self.com()
            .call(
                "BeginInstanceFactory",
                &[
                    Arg::Str(name.to_string()),
                    Arg::Str(template_path.to_string()),
                ],
            )
            .map(|_| ())
    }

    pub fn begin_instantiate(&self) -> ComResult<()> {
        self.com().call("BeginInstantiate", &[]).map(|_| ())
    }

    /// 按模板输入名喂入数据（传原始 HybridShape，与原 VBA find_object_by_name 同口径）。
    pub fn put_input_data(&self, name: &str, data: &impl ComObject) -> ComResult<()> {
        self.com()
            .call(
                "PutInputData",
                &[Arg::Str(name.to_string()), Arg::Obj(data.com().clone())],
            )
            .map(|_| ())
    }

    /// 生成实例（返回的实例对象一般不直接使用，实例体靠 Search 找回）。
    pub fn instantiate(&self) -> ComResult<Entity> {
        Ok(Entity::new(self.com().call("Instantiate", &[])?.as_obj()?))
    }

    pub fn end_instantiate(&self) -> ComResult<()> {
        self.com().call("EndInstantiate", &[]).map(|_| ())
    }

    pub fn end_instance_factory(&self) -> ComResult<()> {
        self.com().call("EndInstanceFactory", &[]).map(|_| ())
    }
}
