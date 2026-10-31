//! `Distances` / `Distance`：SPA 距离测量（`Product.GetTechnologicalObject("Distances")`）。
//!
//! Measurable.GetMinimumDistance 官方注明 Bodies 之间无法直接测量；零件间最小
//! 距离必须走 Distance（BetweenTwo + Minimum，Compute 后读 Value，单位 mm）——
//! 与 ltl-manage find_near_parts.py 同一条实测通道。

use crate::collections::ComCollection;
use crate::dispatch::{Arg, ComObject};
use crate::error::ComResult;

use super::groups::Group;

com_wrapper!(Distances);
com_wrapper!(Distance);

/// CatDistanceComputationType：两对象之间。
pub const CAT_DISTANCE_BETWEEN_TWO: i32 = 2;
/// CatDistanceMeasureType：最小距离。
pub const CAT_DISTANCE_MEASURE_MINIMUM: i32 = 0;

impl ComCollection for Distances {
    type Item = Distance;
}

impl Distances {
    /// 新建距离测量（配置后需 Compute）。
    pub fn add(&self) -> ComResult<Distance> {
        Ok(Distance::new(self.com().call("Add", &[])?.as_obj()?))
    }

    /// 按名字删除测量（临时测量读完即删）。
    pub fn remove(&self, name: &str) -> ComResult<()> {
        self.com()
            .call("Remove", &[Arg::Str(name.to_string())])
            .map(|_| ())
    }
}

impl Distance {
    /// 参与测量的第一组。
    pub fn set_first_group(&self, group: &Group) -> ComResult<()> {
        self.com().put("FirstGroup", Arg::Obj(group.com().clone()))
    }

    /// 参与测量的第二组。
    pub fn set_second_group(&self, group: &Group) -> ComResult<()> {
        self.com().put("SecondGroup", Arg::Obj(group.com().clone()))
    }

    /// 计算类型（CAT_DISTANCE_BETWEEN_TWO = 2）。
    pub fn set_computation_type(&self, value: i32) -> ComResult<()> {
        self.com().put("ComputationType", Arg::I32(value))
    }

    /// 测量类型（CAT_DISTANCE_MEASURE_MINIMUM = 0）。
    pub fn set_measure_type(&self, value: i32) -> ComResult<()> {
        self.com().put("MeasureType", Arg::I32(value))
    }

    /// 执行计算。
    pub fn compute(&self) -> ComResult<()> {
        self.com().call("Compute", &[]).map(|_| ())
    }

    /// 测量结果（mm；Compute 后有效）。
    pub fn value(&self) -> ComResult<f64> {
        self.com().get("Value")?.as_f64()
    }

    /// 结果是否有效（1 = 有值；0 = 未定义）。
    pub fn is_defined(&self) -> ComResult<i32> {
        self.com().get("IsDefined")?.as_i32()
    }
}
