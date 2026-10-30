//! COM 错误类型。

use std::fmt;

use windows::Win32::Foundation::{DISP_E_MEMBERNOTFOUND, DISP_E_PARAMNOTFOUND};

/// COM 调用错误：保留 HRESULT 便于判断（如成员不存在），同时带可读信息。
#[derive(Debug, Clone)]
pub struct ComError {
    pub hresult: i32,
    pub message: String,
}

impl ComError {
    pub fn new(hresult: i32, message: impl Into<String>) -> Self {
        Self {
            hresult,
            message: message.into(),
        }
    }

    /// 非 COM 层的业务错误（CATIA 未运行、多实例守卫等）。
    pub fn runtime(message: impl Into<String>) -> Self {
        Self {
            hresult: 0,
            message: message.into(),
        }
    }

    /// 成员不存在 / 参数不匹配 —— 调用方可据此重试别的派发方式。
    pub fn is_member_missing(&self) -> bool {
        self.hresult == DISP_E_MEMBERNOTFOUND.0 || self.hresult == DISP_E_PARAMNOTFOUND.0
    }
}

impl fmt::Display for ComError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ComError {}

pub type ComResult<T> = Result<T, ComError>;
