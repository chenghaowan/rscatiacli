//! `DrawingDocument` / `DrawingSheets` / `DrawingSheet` / `DrawingViews` /
//! `DrawingView` / `DrawingTexts` / `DrawingText` / `DrawingViewGenerativeBehavior`
//! （工程图家族；drawlink / grid / expose2d 三套件共用）。
//!
//! 成员名对齐 pycatia drafting_interfaces（Sheets/Views/Name/Scale/X/Y/
//! Factory2D/Texts/GenerativeBehavior.Document/DefineFrontView）。
//! Factory2D 复用 sketch 的包装（drawing 与 sketch 的 CreateLine 同签名）。
//! CATIAPaperSize 数值按 V5Automation typelib 顺序（Letter=0, Legal=1, A0=2,
//! A1=3, A2=4, A3=5, A4=6...）——实跑时如遇纸张不符在此校准。

use crate::collections::ComCollection;
use crate::dispatch::{Arg, ComObject};
use crate::error::ComResult;

use super::sketch::Factory2D;

com_wrapper!(DrawingDocument);
com_wrapper!(DrawingSheets);
com_wrapper!(DrawingSheet);
com_wrapper!(DrawingViews);
com_wrapper!(DrawingView);
com_wrapper!(DrawingTexts);
com_wrapper!(DrawingText);
com_wrapper!(DrawingViewGenerativeBehavior);

/// CATIAPaperSize（typelib 顺序）。
pub const PAPER_A0: i32 = 2;
pub const PAPER_A1: i32 = 3;
pub const PAPER_A2: i32 = 4;
pub const PAPER_A3: i32 = 5;
pub const PAPER_A4: i32 = 6;

impl DrawingDocument {
    /// 图纸页集合（`Sheets`）。
    pub fn sheets(&self) -> ComResult<DrawingSheets> {
        Ok(DrawingSheets::new(self.com().get("Sheets")?.as_obj()?))
    }
}

impl ComCollection for DrawingSheets {
    type Item = DrawingSheet;
}

impl DrawingSheets {
    /// 新建图纸页（`Add(name)`）。
    pub fn add(&self, name: &str) -> ComResult<DrawingSheet> {
        Ok(DrawingSheet::new(
            self.com()
                .call("Add", &[Arg::Str(name.to_string())])?
                .as_obj()?,
        ))
    }
}

impl DrawingSheet {
    pub fn name(&self) -> ComResult<String> {
        self.com().get("Name")?.as_string()
    }

    /// 页内视图集合（`Views`；第 1 个是主视图，图纸级 2D 几何挂它上面）。
    pub fn views(&self) -> ComResult<DrawingViews> {
        Ok(DrawingViews::new(self.com().get("Views")?.as_obj()?))
    }

    pub fn paper_size(&self) -> ComResult<i32> {
        self.com().get("PaperSize")?.as_i32()
    }

    pub fn set_paper_size(&self, value: i32) -> ComResult<()> {
        self.com().put("PaperSize", Arg::I32(value))
    }

    pub fn scale(&self) -> ComResult<f64> {
        self.com().get("Scale")?.as_f64()
    }

    pub fn set_scale(&self, value: f64) -> ComResult<()> {
        self.com().put("Scale", Arg::F64(value))
    }
}

impl ComCollection for DrawingViews {
    type Item = DrawingView;
}

impl DrawingViews {
    /// 新建视图（`Add(name)`；生成视图配 GenerativeBehavior 用）。
    pub fn add(&self, name: &str) -> ComResult<DrawingView> {
        Ok(DrawingView::new(
            self.com()
                .call("Add", &[Arg::Str(name.to_string())])?
                .as_obj()?,
        ))
    }
}

impl DrawingView {
    pub fn name(&self) -> ComResult<String> {
        self.com().get("Name")?.as_string()
    }

    pub fn set_name(&self, value: &str) -> ComResult<()> {
        self.com().put("Name", Arg::Str(value.to_string()))
    }

    pub fn scale(&self) -> ComResult<f64> {
        self.com().get("Scale")?.as_f64()
    }

    pub fn set_scale(&self, value: f64) -> ComResult<()> {
        self.com().put("Scale", Arg::F64(value))
    }

    pub fn angle(&self) -> ComResult<f64> {
        self.com().get("Angle")?.as_f64()
    }

    pub fn set_angle(&self, value: f64) -> ComResult<()> {
        self.com().put("Angle", Arg::F64(value))
    }

    /// 视图在图纸上的位置（mm）。
    pub fn x(&self) -> ComResult<f64> {
        self.com().get("X")?.as_f64()
    }

    pub fn set_x(&self, value: f64) -> ComResult<()> {
        self.com().put("X", Arg::F64(value))
    }

    pub fn y(&self) -> ComResult<f64> {
        self.com().get("Y")?.as_f64()
    }

    pub fn set_y(&self, value: f64) -> ComResult<()> {
        self.com().put("Y", Arg::F64(value))
    }

    /// 2D 几何工厂（drawing 与 sketch 的 Factory2D.CreateLine 同签名，复用包装）。
    pub fn factory_2d(&self) -> ComResult<Factory2D> {
        Ok(Factory2D::new(self.com().get("Factory2D")?.as_obj()?))
    }

    /// 视图文字集合（`Texts`）。
    pub fn texts(&self) -> ComResult<DrawingTexts> {
        Ok(DrawingTexts::new(self.com().get("Texts")?.as_obj()?))
    }

    /// 生成视图行为（expose2d 建生成视图用）。
    pub fn generative_behavior(&self) -> ComResult<DrawingViewGenerativeBehavior> {
        Ok(DrawingViewGenerativeBehavior::new(
            self.com().get("GenerativeBehavior")?.as_obj()?,
        ))
    }
}

impl DrawingTexts {
    /// 新建文字（`Add(text, x, y)`）。
    pub fn add(&self, text: &str, x: f64, y: f64) -> ComResult<DrawingText> {
        Ok(DrawingText::new(
            self.com()
                .call(
                    "Add",
                    &[
                        Arg::Str(text.to_string()),
                        Arg::F64(x),
                        Arg::F64(y),
                    ],
                )?
                .as_obj()?,
        ))
    }
}

impl DrawingText {
    pub fn name(&self) -> ComResult<String> {
        self.com().get("Name")?.as_string()
    }

    /// 文字内容（`Text`）。
    pub fn text(&self) -> ComResult<String> {
        self.com().get("Text")?.as_string()
    }

    pub fn set_text(&self, value: &str) -> ComResult<()> {
        self.com().put("Text", Arg::Str(value.to_string()))
    }
}

impl DrawingViewGenerativeBehavior {
    /// 生成视图的 3D 源文档（`Document` put；须是已加载的 Part/Product 文档）。
    pub fn set_document(&self, document: &impl ComObject) -> ComResult<()> {
        self.com()
            .put("Document", Arg::Obj(document.com().clone()))
    }

    /// 读取 3D 源文档（drawlink 链接探测；无链接时 COM 报错）。
    pub fn document(&self) -> ComResult<crate::objects::documents::Document> {
        Ok(crate::objects::documents::Document::new(
            self.com().get("Document")?.as_obj()?,
        ))
    }

    /// 定义投影（视图 X/Y 轴的 3D 方向向量；如 Front = (1,0,0)/(0,0,1)）。
    pub fn define_front_view(&self, view_x: [f64; 3], view_y: [f64; 3]) -> ComResult<()> {
        self.com()
            .call(
                "DefineFrontView",
                &[
                    Arg::F64(view_x[0]),
                    Arg::F64(view_x[1]),
                    Arg::F64(view_x[2]),
                    Arg::F64(view_y[0]),
                    Arg::F64(view_y[1]),
                    Arg::F64(view_y[2]),
                ],
            )
            .map(|_| ())
    }
}
