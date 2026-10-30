//! IDispatch 晚绑定封装。
//!
//! 为什么不用生成的类型绑定：CATIA 类型库缺部分成员（`Document.Product`、`Sheets` 等），
//! 且 SAFEARRAY 出参行为与 makepy 静态绑定不一致 —— Python 侧因此统一走
//! `win32com.client.dynamic`（见 ltl-manage resources/tools/drawing_common.py 的实测记录）。
//! 这里走同一条路径：GetIDsOfNames + Invoke，全部按成员名派发。

use std::mem::ManuallyDrop;

use windows::core::{BSTR, GUID, PCWSTR};
use windows::Win32::Foundation::{SysFreeString, VARIANT_FALSE, VARIANT_TRUE};
use windows::Win32::System::Com::{
    IDispatch, DISPATCH_FLAGS, DISPATCH_METHOD, DISPATCH_PROPERTYGET, DISPATCH_PROPERTYPUT,
    DISPPARAMS, EXCEPINFO,
};
use windows::Win32::System::Variant::{
    VariantClear, VARENUM, VARIANT, VARIANT_0, VARIANT_0_0, VARIANT_0_0_0, VT_BOOL, VT_BSTR,
    VT_DISPATCH, VT_EMPTY, VT_I2, VT_I4, VT_R8,
};

// 兼容旧路径 `dispatch::ComError` / `dispatch::ComResult`（原代码把错误类型定义在这里）。
pub use crate::error::{ComError, ComResult};

/// OLEAUTO.H: 属性赋值时 rgdispidNamedArgs 必须指向这个伪 DISPID。
const DISPID_PROPERTYPUT: i32 = -3;

/// 一个可调用的 COM 对象（持有一份 IDispatch 引用）。
#[derive(Clone)]
pub struct ComObj(pub IDispatch);

/// 调用实参。
///
/// 目前只用到 I32/F64/Bool/Str/Obj；空参用 `&[]` 表达。
pub enum Arg {
    I32(i32),
    F64(f64),
    Bool(bool),
    Str(String),
    Obj(ComObj),
}

/// 绑定库所有类型化对象的公共接口。
///
/// `com()` 是逃生舱：对象模型没封装的成员可以取回原始 [`ComObj`] 继续按名字派发。
/// `name` / `set_name` / `parent` 与 pycatia 的 `AnyObject` 对齐，所有 CATIA 对象通用。
pub trait ComObject {
    fn com(&self) -> &ComObj;

    fn name(&self) -> ComResult<String> {
        self.com().get("Name")?.as_string()
    }

    fn set_name(&self, name: &str) -> ComResult<()> {
        self.com().put("Name", Arg::Str(name.to_string()))
    }

    fn parent(&self) -> ComResult<ComObj> {
        self.com().get("Parent")?.as_obj()
    }
}

impl ComObject for ComObj {
    fn com(&self) -> &ComObj {
        self
    }
}

/// 从原始 COM 对象构造类型化包装（供集合默认 `item` 使用）。
pub trait FromComObj: Sized {
    fn from_com(obj: ComObj) -> Self;
}

impl FromComObj for ComObj {
    fn from_com(obj: ComObj) -> Self {
        obj
    }
}

/// 集合 `Item` 的派发：CATIA 有些版本把它暴露成方法，有些暴露成带参属性。
pub fn item_at(collection: &ComObj, index: i32) -> ComResult<ComObj> {
    collection
        .call("Item", &[Arg::I32(index)])
        .or_else(|e| {
            if e.is_member_missing() {
                collection.get_with("Item", &[Arg::I32(index)])
            } else {
                Err(e)
            }
        })
        .and_then(|v| v.as_obj())
}

fn variant(vt: VARENUM, body: VARIANT_0_0_0) -> VARIANT {
    VARIANT {
        Anonymous: VARIANT_0 {
            Anonymous: ManuallyDrop::new(VARIANT_0_0 {
                vt,
                wReserved1: 0,
                wReserved2: 0,
                wReserved3: 0,
                Anonymous: body,
            }),
        },
    }
}

fn to_variant(arg: &Arg) -> VARIANT {
    match arg {
        Arg::I32(v) => variant(VT_I4, VARIANT_0_0_0 { lVal: *v }),
        Arg::F64(v) => variant(VT_R8, VARIANT_0_0_0 { dblVal: *v }),
        Arg::Bool(v) => variant(
            VT_BOOL,
            VARIANT_0_0_0 {
                boolVal: if *v { VARIANT_TRUE } else { VARIANT_FALSE },
            },
        ),
        Arg::Str(v) => variant(
            VT_BSTR,
            VARIANT_0_0_0 {
                bstrVal: ManuallyDrop::new(BSTR::from(v.as_str())),
            },
        ),
        Arg::Obj(v) => variant(
            VT_DISPATCH,
            VARIANT_0_0_0 {
                pdispVal: ManuallyDrop::new(Some(v.0.clone())),
            },
        ),
    }
}

/// 实参数组。COM 约定 rgvarg 逆序存放；Drop 时统一 VariantClear，
/// 释放我们为 BSTR / IDispatch 持有（AddRef）的引用。
struct Args {
    vars: Vec<VARIANT>,
}

impl Args {
    fn new(args: &[Arg]) -> Self {
        Self {
            vars: args.iter().rev().map(to_variant).collect(),
        }
    }

    /// `named` 用于属性赋值：COM 要求 cNamedArgs=1 且 rgdispidNamedArgs 指向
    /// DISPID_PROPERTYPUT(-3)，否则被调用方会忽略新值。
    fn dispparams(&mut self, named: Option<&mut i32>) -> DISPPARAMS {
        match named {
            Some(dispid) => DISPPARAMS {
                rgvarg: self.vars.as_mut_ptr(),
                rgdispidNamedArgs: dispid as *mut i32,
                cArgs: self.vars.len() as u32,
                cNamedArgs: 1,
            },
            None => DISPPARAMS {
                rgvarg: self.vars.as_mut_ptr(),
                rgdispidNamedArgs: std::ptr::null_mut(),
                cArgs: self.vars.len() as u32,
                cNamedArgs: 0,
            },
        }
    }
}

impl Drop for Args {
    fn drop(&mut self) {
        for v in self.vars.iter_mut() {
            unsafe {
                let _ = VariantClear(v);
            }
        }
    }
}

/// 调用返回值。Drop 时 VariantClear，避免 BSTR / IDispatch 泄漏。
pub struct Ret(VARIANT);

impl Drop for Ret {
    fn drop(&mut self) {
        unsafe {
            let _ = VariantClear(&mut self.0);
        }
    }
}

impl Ret {
    fn vt(&self) -> u16 {
        // SAFETY: VARIANT 始终以 VARIANT_0/VARIANT_0_0 布局构造，vt 恒可读。
        unsafe { self.0.Anonymous.Anonymous.vt.0 }
    }

    pub fn is_null(&self) -> bool {
        self.vt() == VT_EMPTY.0
    }

    pub fn as_i32(&self) -> ComResult<i32> {
        match self.vt() {
            v if v == VT_I4.0 => Ok(unsafe { self.0.Anonymous.Anonymous.Anonymous.lVal }),
            v if v == VT_I2.0 => Ok(i32::from(unsafe {
                self.0.Anonymous.Anonymous.Anonymous.iVal
            })),
            v if v == VT_R8.0 => Ok(unsafe { self.0.Anonymous.Anonymous.Anonymous.dblVal } as i32),
            v if v == VT_BOOL.0 => Ok(
                if unsafe { self.0.Anonymous.Anonymous.Anonymous.boolVal } != VARIANT_FALSE {
                    1
                } else {
                    0
                },
            ),
            other => Err(ComError::runtime(format!(
                "期望整数，CATIA 返回 VARIANT 类型 {other}"
            ))),
        }
    }

    pub fn as_f64(&self) -> ComResult<f64> {
        match self.vt() {
            v if v == VT_R8.0 => Ok(unsafe { self.0.Anonymous.Anonymous.Anonymous.dblVal }),
            v if v == VT_I4.0 => Ok(f64::from(unsafe {
                self.0.Anonymous.Anonymous.Anonymous.lVal
            })),
            other => Err(ComError::runtime(format!(
                "期望浮点数，CATIA 返回 VARIANT 类型 {other}"
            ))),
        }
    }

    pub fn as_bool(&self) -> ComResult<bool> {
        match self.vt() {
            v if v == VT_BOOL.0 => {
                Ok(unsafe { self.0.Anonymous.Anonymous.Anonymous.boolVal } != VARIANT_FALSE)
            }
            v if v == VT_I4.0 => Ok(unsafe { self.0.Anonymous.Anonymous.Anonymous.lVal } != 0),
            other => Err(ComError::runtime(format!(
                "期望布尔值，CATIA 返回 VARIANT 类型 {other}"
            ))),
        }
    }

    pub fn as_string(&self) -> ComResult<String> {
        if self.vt() != VT_BSTR.0 {
            return Err(ComError::runtime(format!(
                "期望字符串，CATIA 返回 VARIANT 类型 {}",
                self.vt()
            )));
        }
        // SAFETY: vt == VT_BSTR 时 bstrVal 有效；Display 会做 UTF-16 解码。
        let s = unsafe { self.0.Anonymous.Anonymous.Anonymous.bstrVal.to_string() };
        Ok(s)
    }

    /// 取出对象。克隆一份 IDispatch（AddRef），Drop 时 VariantClear 释放原引用。
    pub fn as_obj(&self) -> ComResult<ComObj> {
        if self.vt() != VT_DISPATCH.0 {
            return Err(ComError::runtime(format!(
                "期望 COM 对象，CATIA 返回 VARIANT 类型 {}",
                self.vt()
            )));
        }
        // SAFETY: vt == VT_DISPATCH 时 pdispVal 有效。
        let opt: &Option<IDispatch> = unsafe { &self.0.Anonymous.Anonymous.Anonymous.pdispVal };
        opt.clone()
            .map(ComObj)
            .ok_or_else(|| ComError::runtime("CATIA 返回了空对象(Nothing)"))
    }
}

/// 从 EXCEPINFO 取可读描述，并按 COM 规则释放被调用方分配的字符串。
fn take_excep_message(info: &mut EXCEPINFO) -> String {
    let text = info.bstrDescription.to_string();
    unsafe {
        if !info.bstrDescription.is_empty() {
            let desc = ManuallyDrop::take(&mut info.bstrDescription);
            SysFreeString(&desc);
        }
        if !info.bstrSource.is_empty() {
            let src = ManuallyDrop::take(&mut info.bstrSource);
            SysFreeString(&src);
        }
        if !info.bstrHelpFile.is_empty() {
            let help = ManuallyDrop::take(&mut info.bstrHelpFile);
            SysFreeString(&help);
        }
    }
    text
}

impl ComObj {
    fn dispid(&self, name: &str) -> ComResult<i32> {
        let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        let ptr = PCWSTR(wide.as_ptr());
        let mut id = 0i32;
        // SAFETY: wide 在调用期间存活；riid 用 IID_NULL。
        unsafe {
            self.0
                .GetIDsOfNames(&GUID::from_u128(0), &ptr, 1, 0, &mut id)
                .map_err(|e| {
                    ComError::new(
                        e.code().0,
                        format!("CATIA 不认识成员 `{name}`: {}", e.message()),
                    )
                })?;
        }
        Ok(id)
    }

    fn invoke(&self, id: i32, flags: DISPATCH_FLAGS, args: &[Arg]) -> ComResult<Ret> {
        let mut packed = Args::new(args);
        let params = packed.dispparams(None);
        self.invoke_with(id, flags, &params)
    }

    fn invoke_put(&self, id: i32, value: Arg) -> ComResult<Ret> {
        let mut packed = Args::new(&[value]);
        let mut dispid_named: i32 = DISPID_PROPERTYPUT;
        let params = packed.dispparams(Some(&mut dispid_named));
        self.invoke_with(id, DISPATCH_PROPERTYPUT, &params)
    }

    fn invoke_with(&self, id: i32, flags: DISPATCH_FLAGS, params: &DISPPARAMS) -> ComResult<Ret> {
        let mut result = VARIANT::default();
        let mut info = EXCEPINFO::default();
        let mut arg_err = 0u32;

        // SAFETY: params/result/info 都在本栈帧存活；指针指向连续 Vec 缓冲区。
        let hr = unsafe {
            self.0.Invoke(
                id,
                &GUID::from_u128(0),
                0,
                flags,
                params,
                Some(&mut result),
                Some(&mut info),
                Some(&mut arg_err),
            )
        };
        let excep = take_excep_message(&mut info);

        match hr {
            Ok(()) => Ok(Ret(result)),
            Err(e) => {
                unsafe {
                    let _ = VariantClear(&mut result);
                }
                let msg = if excep.trim().is_empty() {
                    e.message()
                } else {
                    excep
                };
                Err(ComError::new(e.code().0, msg))
            }
        }
    }

    /// 读属性（DISPATCH_PROPERTYGET）。
    pub fn get(&self, name: &str) -> ComResult<Ret> {
        let id = self.dispid(name)?;
        self.invoke(id, DISPATCH_PROPERTYGET, &[])
    }

    /// 调方法（DISPATCH_METHOD）。CATIA 的集合 `Item`、`Add`、`Clear` 等走这里。
    pub fn call(&self, name: &str, args: &[Arg]) -> ComResult<Ret> {
        let id = self.dispid(name)?;
        self.invoke(id, DISPATCH_METHOD, args)
    }

    /// 带参属性（如部分集合的 `Item` 在类型库里是 propget）。
    /// 仅在 `call` 报“成员/参数不匹配”时使用，避免对有副作用的方法盲目重试。
    pub fn get_with(&self, name: &str, args: &[Arg]) -> ComResult<Ret> {
        let id = self.dispid(name)?;
        self.invoke(id, DISPATCH_PROPERTYGET, args)
    }

    /// 写属性（DISPATCH_PROPERTYPUT）。CATIA 的 `Body.Name`、`Part.Name` 等走这里。
    pub fn put(&self, name: &str, value: Arg) -> ComResult<()> {
        let id = self.dispid(name)?;
        self.invoke_put(id, value).map(|_| ())
    }
}
