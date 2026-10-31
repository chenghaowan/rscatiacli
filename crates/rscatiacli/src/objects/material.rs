//! 材料目录与材料管理：`MaterialDocument` / `MaterialFamilies` / `MaterialFamily` /
//! `Materials` / `Material` / `MaterialManager`。
//!
//! 材料目录是 `.CATMaterial` 文档（CATIA 类型库里的 `MaterialDocument`），打开后按
//! `Families` → `Materials` 两层遍历。挂材料的入口是
//! `Part.GetItem("CATMatManagerVBExt")` 拿到的 `MaterialManager`
//! （API 口径同 V5Automation 官方示例 CAAScdMatUseCases：link_mode 0=复制材料值,
//! 1=链接到材料目录）。

use crate::collections::ComCollection;
use crate::dispatch::{Arg, ComObj, ComObject, FromComObj};
use crate::error::ComResult;

use super::documents::Document;
use super::part::{Body, Part};

com_wrapper!(MaterialFamilies);
com_wrapper!(MaterialFamily);
com_wrapper!(Materials);
com_wrapper!(Material);
com_wrapper!(MaterialManager);

impl ComCollection for MaterialFamilies {
    type Item = MaterialFamily;
}

impl ComCollection for Materials {
    type Item = Material;
}

impl MaterialFamily {
    /// 族内全部材料。
    pub fn materials(&self) -> ComResult<Materials> {
        Ok(Materials::new(self.com().get("Materials")?.as_obj()?))
    }
}

impl MaterialManager {
    /// 把材料挂到整个 Part。
    pub fn apply_material_on_part(
        &self,
        part: &Part,
        material: &Material,
        link_mode: i32,
    ) -> ComResult<()> {
        self.com()
            .call(
                "ApplyMaterialOnPart",
                &[
                    Arg::Obj(part.com().clone()),
                    Arg::Obj(material.com().clone()),
                    Arg::I32(link_mode),
                ],
            )
            .map(|_| ())
    }

    /// 把材料挂到单个 Body。
    pub fn apply_material_on_body(
        &self,
        body: &Body,
        material: &Material,
        link_mode: i32,
    ) -> ComResult<()> {
        self.com()
            .call(
                "ApplyMaterialOnBody",
                &[
                    Arg::Obj(body.com().clone()),
                    Arg::Obj(material.com().clone()),
                    Arg::I32(link_mode),
                ],
            )
            .map(|_| ())
    }
}

/// 材料目录文档（`.CATMaterial`）。
#[derive(Clone)]
pub struct MaterialDocument(Document);

impl MaterialDocument {
    pub fn new(obj: ComObj) -> Self {
        Self(Document::new(obj))
    }

    pub fn from_document(document: Document) -> Self {
        Self(document)
    }

    /// 取回通用 `Document`。
    pub fn document(&self) -> &Document {
        &self.0
    }

    /// 材料族集合。
    pub fn families(&self) -> ComResult<MaterialFamilies> {
        Ok(MaterialFamilies::new(
            self.0.com().get("Families")?.as_obj()?,
        ))
    }
}

impl ComObject for MaterialDocument {
    fn com(&self) -> &ComObj {
        self.0.com()
    }
}

impl FromComObj for MaterialDocument {
    fn from_com(obj: ComObj) -> Self {
        Self(Document::new(obj))
    }
}
