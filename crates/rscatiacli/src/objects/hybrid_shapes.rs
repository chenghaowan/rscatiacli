//! `HybridShapeJoin` / `HybridShapeOffset`：曲面缝合与偏置特征。
//!
//! 参数取值与 CATIA/pycatia 完全一致（SetConnex(0/1)、SetDeviation(mm) 等），
//! 语义记录来自 ltl-manage 实测：Connex=1 在集合内存在不相连曲面时 Update
//! 直接失败；偏差太小（如 0.001）同样可能失败，需按精度序列降级重试。

use crate::dispatch::{Arg, ComObject};
use crate::error::ComResult;

use super::reference::Reference;

com_wrapper!(HybridShapeJoin);
com_wrapper!(HybridShapeOffset);

impl HybridShapeJoin {
    /// 只缝合有公共边的曲面（1 = 开启）。集合内存在不相连曲面时必须用 0。
    pub fn set_connex(&self, value: i32) -> ComResult<()> {
        self.flag("SetConnex", value)
    }

    pub fn set_healing_mode(&self, value: i32) -> ComResult<()> {
        self.flag("SetHealingMode", value)
    }

    pub fn set_simplify(&self, value: i32) -> ComResult<()> {
        self.flag("SetSimplify", value)
    }

    pub fn set_suppress_mode(&self, value: i32) -> ComResult<()> {
        self.flag("SetSuppressMode", value)
    }

    /// 缝合偏差（mm）。
    pub fn set_deviation(&self, value: f64) -> ComResult<()> {
        self.com().call("SetDeviation", &[Arg::F64(value)])?;
        Ok(())
    }

    pub fn set_federation_propagation(&self, value: i32) -> ComResult<()> {
        self.flag("SetFederationPropagation", value)
    }

    pub fn set_manifold(&self, value: i32) -> ComResult<()> {
        self.flag("SetManifold", value)
    }

    /// 追加第 3 个及以后的元素（构造时 AddNewJoin 只带前两个）。
    pub fn add_element(&self, reference: &Reference) -> ComResult<()> {
        self.com()
            .call("AddElement", &[Arg::Obj(reference.com().clone())])
            .map(|_| ())
    }

    fn flag(&self, method: &str, value: i32) -> ComResult<()> {
        self.com().call(method, &[Arg::I32(value)]).map(|_| ())
    }
}

impl HybridShapeOffset {
    /// 偏置方向（CATIA 布尔语义，true/false 原样）。
    pub fn offset_direction(&self) -> ComResult<bool> {
        self.com().get("OffsetDirection")?.as_bool()
    }

    /// 偏置值（mm）。
    pub fn offset_value(&self) -> ComResult<f64> {
        self.com().get("OffsetValue")?.as_f64()
    }
}
