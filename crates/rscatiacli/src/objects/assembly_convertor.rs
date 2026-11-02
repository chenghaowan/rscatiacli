//! `AssemblyConvertor`（装配 BOM / 清单输出）。
//!
//! 获取路径：`Product.GetItem("BillOfMaterial")`（与 CATIA 录制宏一致，
//! pycatia issue #110 同口径；`GetItem` 返回 AnyObject，这里直接包成
//! [`AssemblyConvertor`]，见 [`Product::assembly_convertor`](super::product::Product)）。
//!
//! `Print` 经 VBA 中转（与 pycatia 同：动态派发直调 Print 会失败）。
//! `SetCurrentFormat` / `SetSecondaryFormat` 的列名为产品属性关键字
//! （"Quantity"、"Part Number"、"Nomenclature"、"Revision"...）或用户属性名。
//! 注意（pycatia issue #110 实测）：文件路径必须是 Windows 反斜杠，正斜杠会触发
//! `com_error -2147467259` 且错误会粘住到 CATIA 重启。

use crate::dispatch::{Arg, ComObject};
use crate::error::ComResult;

use super::product::Product;
use super::system_service::SystemService;

com_wrapper!(AssemblyConvertor);

/// `SetCurrentFormat` / `SetSecondaryFormat` 的列名数组（SAFEARRAY of BSTR）。
pub type FormatProps = Vec<String>;

impl AssemblyConvertor {
    /// 导出装配内容（`Print`；file_type: "TXT" / "HTML" / "XLS"，路径用 `\\`）。
    pub fn print(
        &self,
        ss: &SystemService,
        file_type: &str,
        file_path: &str,
        product: &Product,
    ) -> ComResult<()> {
        let function = "rscatiacli_assembly_convertor_print";
        let code = format!(
            r#"
        Public Function {function}(oConvertor As Object, iFileType As String, iFile As String, iProduct As Product)
            oConvertor.Print iFileType, iFile, iProduct
            {function} = iFileType
        End Function
        "#
        );
        ss.evaluate(
            &code,
            0,
            function,
            &[
                Arg::Obj(self.com().clone()),
                Arg::Str(file_type.to_string()),
                Arg::Str(file_path.to_string()),
                Arg::Obj(product.com().clone()),
            ],
        )
        .map(|_| ())
    }

    /// 设 Print 主列（`SetCurrentFormat`）。
    pub fn set_current_format(&self, props: &[&str]) -> ComResult<()> {
        self.set_format("SetCurrentFormat", props)
    }

    /// 设 Print 次列（`SetSecondaryFormat`）。
    pub fn set_secondary_format(&self, props: &[&str]) -> ComResult<()> {
        self.set_format("SetSecondaryFormat", props)
    }

    fn set_format(&self, member: &str, props: &[&str]) -> ComResult<()> {
        let items: Vec<Arg> = props.iter().map(|p| Arg::Str(p.to_string())).collect();
        self.com()
            .call(member, &[Arg::SafeArray(items)])
            .map(|_| ())
    }
}
