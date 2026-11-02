//! `Parameters` / `Parameter`：知识工程参数（`Part.Parameters`）。
//!
//! vba_bbox_generate 用 `CreateDimension(name, "Length", value)` 建存
//! Bbox_dx/dy/dz 三个长度参数。

use crate::collections::ComCollection;
use crate::dispatch::{Arg, ComObj, ComObject};
use crate::error::ComResult;

use super::knowledge::{
    BoolParam, IntParam, ListParameter, ParameterSet, RealParam, StrParam, Units,
};

com_wrapper!(Parameters);
com_wrapper!(Parameter);

impl ComCollection for Parameters {
    type Item = Parameter;
}

impl Parameters {
    /// 新建实数参数（`CreateReal`）。
    pub fn create_real(&self, name: &str, value: f64) -> ComResult<RealParam> {
        Ok(RealParam::new(
            self.com()
                .call("CreateReal", &[Arg::Str(name.to_string()), Arg::F64(value)])?
                .as_obj()?,
        ))
    }

    /// 新建整数参数（`CreateInteger`）。
    pub fn create_integer(&self, name: &str, value: i32) -> ComResult<IntParam> {
        Ok(IntParam::new(
            self.com()
                .call(
                    "CreateInteger",
                    &[Arg::Str(name.to_string()), Arg::I32(value)],
                )?
                .as_obj()?,
        ))
    }

    /// 新建字符串参数（`CreateString`）。
    pub fn create_string(&self, name: &str, value: &str) -> ComResult<StrParam> {
        Ok(StrParam::new(
            self.com()
                .call(
                    "CreateString",
                    &[Arg::Str(name.to_string()), Arg::Str(value.to_string())],
                )?
                .as_obj()?,
        ))
    }

    /// 新建布尔参数（`CreateBoolean`）。
    pub fn create_boolean(&self, name: &str, value: bool) -> ComResult<BoolParam> {
        Ok(BoolParam::new(
            self.com()
                .call(
                    "CreateBoolean",
                    &[Arg::Str(name.to_string()), Arg::Bool(value)],
                )?
                .as_obj()?,
        ))
    }

    /// 新建列表参数（`CreateList`）。
    pub fn create_list(&self, name: &str) -> ComResult<ListParameter> {
        Ok(ListParameter::new(
            self.com()
                .call("CreateList", &[Arg::Str(name.to_string())])?
                .as_obj()?,
        ))
    }

    /// 根参数集（`RootParameterSet`）。
    pub fn root_parameter_set(&self) -> ComResult<ParameterSet> {
        Ok(ParameterSet::new(
            self.com().get("RootParameterSet")?.as_obj()?,
        ))
    }

    /// 单位集合（`Units`）。
    pub fn units(&self) -> ComResult<Units> {
        Ok(Units::new(self.com().get("Units")?.as_obj()?))
    }

    /// 全部参数（含子集，`AllParameters`）。
    pub fn all_parameters(&self) -> ComResult<Parameters> {
        Ok(Parameters::new(self.com().get("AllParameters")?.as_obj()?))
    }

    /// 关系表达式里的参数名写法（`GetNameToUseInRelation`，如 `Part1\Body1\Sketch1\Radius`）。
    pub fn get_name_to_use_in_relation(&self, object: &impl ComObject) -> ComResult<String> {
        self.com()
            .call("GetNameToUseInRelation", &[Arg::Obj(object.com().clone())])?
            .as_string()
    }

    /// 是否含参数（`HasParameters`）。
    pub fn has_parameters(&self) -> ComResult<bool> {
        self.com().call("HasParameters", &[])?.as_bool()
    }

    /// 序号处是否参数（`IsParameter`）。
    pub fn is_parameter(&self, index: i32) -> ComResult<bool> {
        self.com()
            .call("IsParameter", &[Arg::I32(index)])?
            .as_bool()
    }

    /// 序号处是否列表参数（`IsListParameter`）。
    pub fn is_list_parameter(&self, index: i32) -> ComResult<bool> {
        self.com()
            .call("IsListParameter", &[Arg::I32(index)])?
            .as_bool()
    }

    /// 按序号删除参数（`Remove`）。
    pub fn remove(&self, index: i32) -> ComResult<()> {
        self.com().call("Remove", &[Arg::I32(index)]).map(|_| ())
    }

    /// 取对象下的参数子集（`SubList`）。
    pub fn sub_list(&self, object: &impl ComObject, recursively: bool) -> ComResult<Vec<ComObj>> {
        self.com()
            .call(
                "SubList",
                &[Arg::Obj(object.com().clone()), Arg::Bool(recursively)],
            )?
            .as_obj_array()
    }

    /// 新建带量纲参数（magnitude 如 "Length"）。
    pub fn create_dimension(
        &self,
        name: &str,
        magnitude: &str,
        value: f64,
    ) -> ComResult<Parameter> {
        Ok(Parameter::new(
            self.com()
                .call(
                    "CreateDimension",
                    &[
                        Arg::Str(name.to_string()),
                        Arg::Str(magnitude.to_string()),
                        Arg::F64(value),
                    ],
                )?
                .as_obj()?,
        ))
    }
}

impl Parameter {
    /// 备注（`Comment`）。
    pub fn comment(&self) -> ComResult<String> {
        self.com().get("Comment")?.as_string()
    }

    /// 设备注（`Comment`）。
    pub fn set_comment(&self, value: &str) -> ComResult<()> {
        self.com().put("Comment", Arg::Str(value.to_string()))
    }

    /// 所属上下文（`Context`，原始对象）。
    pub fn context(&self) -> ComResult<ComObj> {
        self.com().get("Context")?.as_obj()
    }

    /// 是否只读（`ReadOnly`）。
    pub fn read_only(&self) -> ComResult<bool> {
        self.com().get("ReadOnly")?.as_bool()
    }

    /// 是否被重命名过（`Renamed`）。
    pub fn renamed(&self) -> ComResult<bool> {
        self.com().get("Renamed")?.as_bool()
    }

    /// 用户访问模式（`UserAccessMode`；0 = 未设置等，见 CAA CatParameterUserAccessMode）。
    pub fn user_access_mode(&self) -> ComResult<i32> {
        self.com().get("UserAccessMode")?.as_i32()
    }

    /// 是否真参数（`IsTrueParameter`；NonCDDerived 参数为 false）。
    pub fn is_true_parameter(&self) -> ComResult<bool> {
        self.com().get("IsTrueParameter")?.as_bool()
    }

    /// 可选关系（`OptionalRelation`；无则错误）。
    pub fn optional_relation(&self) -> ComResult<crate::objects::knowledge::Relation> {
        Ok(crate::objects::knowledge::Relation::new(
            self.com().get("OptionalRelation")?.as_obj()?,
        ))
    }

    /// 重命名（`Rename`）。
    pub fn rename(&self, name: &str) -> ComResult<()> {
        self.com()
            .call("Rename", &[Arg::Str(name.to_string())])
            .map(|_| ())
    }

    /// 按字符串赋值（`ValuateFromString`）。
    pub fn valuate_from_string(&self, value: &str) -> ComResult<()> {
        self.com()
            .call("ValuateFromString", &[Arg::Str(value.to_string())])
            .map(|_| ())
    }

    /// 值的格式化字符串（`ValueAsString`；CAA 口径。真机实测：产品身份参数
    /// 如 `Product1\Part Number` 上返回"找不到成员"——此类参数改用
    /// `Product::part_number()` 等专用访问器）。
    pub fn value_as_string(&self) -> ComResult<String> {
        self.com().get("ValueAsString")?.as_string()
    }

    /// 参数值。
    pub fn value(&self) -> ComResult<f64> {
        self.com().get("Value")?.as_f64()
    }

    pub fn set_value(&self, value: f64) -> ComResult<()> {
        self.com().put("Value", Arg::F64(value))
    }
}
