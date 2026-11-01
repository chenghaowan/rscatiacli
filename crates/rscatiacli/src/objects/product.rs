//! `Product` 与 `Products`（装配结构）。

use crate::collections::ComCollection;
use crate::dispatch::{Arg, ComObject};
use crate::error::ComResult;

use super::documents::Document;
use super::publication::Publications;

com_wrapper!(Product);
com_wrapper!(Products);

impl ComCollection for Products {
    type Item = Product;
}

impl Product {
    /// 子装配 / 零件实例集合。
    pub fn products(&self) -> ComResult<Products> {
        Ok(Products::new(self.com().get("Products")?.as_obj()?))
    }

    /// 按类型名取挂载的技术对象（如 `GetTechnologicalObject("Groups")` /
    /// `"Distances"`，SPA 测距用）。
    pub fn get_technological_object(
        &self,
        application_type: &str,
    ) -> ComResult<crate::objects::part::Entity> {
        Ok(crate::objects::part::Entity::new(
            self.com()
                .call(
                    "GetTechnologicalObject",
                    &[Arg::Str(application_type.to_string())],
                )?
                .as_obj()?,
        ))
    }

    /// 实例参考文档的文件名（`ReferenceProduct.Parent.Name`，如 `xxx.CATPart`；
    /// pycatia Product.file_name 同口径）。
    pub fn file_name(&self) -> ComResult<String> {
        let doc = self.reference_product()?.parent()?;
        doc.get("Name")?.as_string()
    }

    /// 实例对应的参考装配（`ReferenceProduct`）。
    pub fn reference_product(&self) -> ComResult<Product> {
        Ok(Product::new(self.com().get("ReferenceProduct")?.as_obj()?))
    }

    /// CatWorkModeType：DESIGN_MODE = 2。CGR 可视化实例必须先切设计模式。
    pub fn apply_work_mode(&self, mode: i32) -> ComResult<()> {
        self.com().call("ApplyWorkMode", &[Arg::I32(mode)])?;
        Ok(())
    }

    /// 激活终端节点（CATIA 的 Activate Terminal Node）。
    pub fn activate_default_shape(&self) -> ComResult<()> {
        self.com().call("ActivateDefaultShape", &[])?;
        Ok(())
    }

    /// 零件号（`PartNumber`）。
    pub fn part_number(&self) -> ComResult<String> {
        self.com().get("PartNumber")?.as_string()
    }

    /// 实例名（`Name`，如 "Part1.1"；与已开文档 Name 匹配用）。
    pub fn name(&self) -> ComResult<String> {
        self.com().get("Name")?.as_string()
    }

    /// 更新产品树（断链/几何变化后重算，尽力而为）。
    pub fn update(&self) -> ComResult<()> {
        self.com().call("Update", &[]).map(|_| ())
    }

    /// 描述属性（属性页「描述」字段；实例与参考两份，边界框尺寸写入用）。
    pub fn set_description_inst(&self, value: &str) -> ComResult<()> {
        self.com()
            .put("DescriptionInst", Arg::Str(value.to_string()))
    }

    pub fn set_description_ref(&self, value: &str) -> ComResult<()> {
        self.com()
            .put("DescriptionRef", Arg::Str(value.to_string()))
    }

    pub fn set_part_number(&self, value: &str) -> ComResult<()> {
        self.com().put("PartNumber", Arg::Str(value.to_string()))
    }

    /// 零件名（`PartName`，与 PartNumber 不同：这是显示名）。
    pub fn part_name(&self) -> ComResult<String> {
        self.com().get("PartName")?.as_string()
    }

    pub fn set_part_name(&self, value: &str) -> ComResult<()> {
        self.com().put("PartName", Arg::Str(value.to_string()))
    }

    /// 发布集合（跨文档复制前需要先发布）。Part 文档同样暴露 Product。
    pub fn publications(&self) -> ComResult<Publications> {
        Ok(Publications::new(self.com().get("Publications")?.as_obj()?))
    }
}

impl Products {
    /// 把外部组件（已打开的文档）加进装配，返回新增实例。
    pub fn add_external_component(&self, document: &Document) -> ComResult<Product> {
        Ok(Product::new(
            self.com()
                .call("AddExternalComponent", &[Arg::Obj(document.com().clone())])?
                .as_obj()?,
        ))
    }

    /// 按文件路径批量插入组件（`AddComponentsFromFiles(paths, docType)`，
    /// docType 用 "All"；save_product_without_history 建新装配用）。
    pub fn add_components_from_files(&self, paths: &[&str], doc_type: &str) -> ComResult<()> {
        let args: Vec<Arg> = paths
            .iter()
            .map(|p| Arg::Str((*p).to_string()))
            .chain(std::iter::once(Arg::Str(doc_type.to_string())))
            .collect();
        self.com().call("AddComponentsFromFiles", &args).map(|_| ())
    }

    /// 把组件的引用重指向另一个 Product（断链重连）。
    /// PartDocument / ProductDocument 均暴露 Product 属性，未保存的内存文档也能作为目标。
    pub fn replace_product(
        &self,
        component: &impl ComObject,
        new_reference: &impl ComObject,
        keep_data: bool,
    ) -> ComResult<()> {
        self.com()
            .call(
                "ReplaceProduct",
                &[
                    Arg::Obj(component.com().clone()),
                    Arg::Obj(new_reference.com().clone()),
                    Arg::Bool(keep_data),
                ],
            )
            .map(|_| ())
    }
}
