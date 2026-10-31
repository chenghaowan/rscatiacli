//! `Parameters` / `Parameter`：知识工程参数（`Part.Parameters`）。
//!
//! vba_bbox_generate 用 `CreateDimension(name, "Length", value)` 建存
//! Bbox_dx/dy/dz 三个长度参数。

use crate::collections::ComCollection;
use crate::dispatch::{Arg, ComObject};
use crate::error::ComResult;

com_wrapper!(Parameters);
com_wrapper!(Parameter);

impl ComCollection for Parameters {
    type Item = Parameter;
}

impl Parameters {
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
    /// 参数值。
    pub fn value(&self) -> ComResult<f64> {
        self.com().get("Value")?.as_f64()
    }

    pub fn set_value(&self, value: f64) -> ComResult<()> {
        self.com().put("Value", Arg::F64(value))
    }
}
