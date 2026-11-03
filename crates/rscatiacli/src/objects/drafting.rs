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
use crate::dispatch::{Arg, ComObj, ComObject};
use crate::error::ComResult;

use super::reference::Reference;
use super::sketch::Factory2D;
use super::system_service::SystemService;

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

    /// 图片集合（`Pictures`）。
    pub fn pictures(&self) -> ComResult<DrawingPictures> {
        Ok(DrawingPictures::new(self.com().get("Pictures")?.as_obj()?))
    }

    /// 表格集合（`Tables`）。
    pub fn tables(&self) -> ComResult<DrawingTables> {
        Ok(DrawingTables::new(self.com().get("Tables")?.as_obj()?))
    }

    /// 打印区域（`PrintArea`）。
    pub fn print_area(&self) -> ComResult<PrintArea> {
        Ok(PrintArea::new(self.com().get("PrintArea")?.as_obj()?))
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

    /// 尺寸标注集合（`Dimensions`）。
    pub fn dimensions(&self) -> ComResult<DrawingDimensions> {
        Ok(DrawingDimensions::new(
            self.com().get("Dimensions")?.as_obj()?,
        ))
    }

    /// 箭头集合（`Arrows`）。
    pub fn arrows(&self) -> ComResult<DrawingArrows> {
        Ok(DrawingArrows::new(self.com().get("Arrows")?.as_obj()?))
    }

    /// 坐标尺寸集合（`CoordDims`）。
    pub fn coord_dims(&self) -> ComResult<DrawingCoordDims> {
        Ok(DrawingCoordDims::new(
            self.com().get("CoordDims")?.as_obj()?,
        ))
    }

    /// 螺纹标注集合（`Threads`）。
    pub fn threads(&self) -> ComResult<DrawingThreads> {
        Ok(DrawingThreads::new(self.com().get("Threads")?.as_obj()?))
    }

    /// 生成链接（`GenerativeLinks`）。
    pub fn generative_links(&self) -> ComResult<DrawingViewGenerativeLinks> {
        Ok(DrawingViewGenerativeLinks::new(
            self.com().get("GenerativeLinks")?.as_obj()?,
        ))
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
                    &[Arg::Str(text.to_string()), Arg::F64(x), Arg::F64(y)],
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

    /// 引出线集合（`Leaders`）。
    pub fn leaders(&self) -> ComResult<DrawingLeaders> {
        Ok(DrawingLeaders::new(self.com().get("Leaders")?.as_obj()?))
    }

    /// 文字属性（`TextProperties`）。
    pub fn text_properties(&self) -> ComResult<DrawingTextProperties> {
        Ok(DrawingTextProperties::new(
            self.com().get("TextProperties")?.as_obj()?,
        ))
    }
}

impl DrawingViewGenerativeBehavior {
    /// 生成视图的 3D 源文档（`Document` put；须是已加载的 Part/Product 文档）。
    pub fn set_document(&self, document: &impl ComObject) -> ComResult<()> {
        self.com().put("Document", Arg::Obj(document.com().clone()))
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

// ── 阶段 7：图纸标注/表格/图片/尺寸/焊接/螺纹/打印区域（29 类） ──────

com_wrapper!(DrawingRoot);
com_wrapper!(DrawingPageSetup);
com_wrapper!(DrawingDimensions);
com_wrapper!(DrawingDimension);
com_wrapper!(DrawingDimLine);
com_wrapper!(DrawingDimExtLine);
com_wrapper!(DrawingDimValue);
com_wrapper!(DrawingTextProperties);
com_wrapper!(DrawingTextRange);
com_wrapper!(DrawingLeaders);
com_wrapper!(DrawingLeader);
com_wrapper!(DrawingArrows);
com_wrapper!(DrawingArrow);
com_wrapper!(DrawingPictures);
com_wrapper!(DrawingPicture);
com_wrapper!(DrawingTables);
com_wrapper!(DrawingTable);
com_wrapper!(DrawingGDTs);
com_wrapper!(DrawingGDT);
com_wrapper!(DrawingComponents);
com_wrapper!(DrawingComponent);
com_wrapper!(DrawingThreads);
com_wrapper!(DrawingThread);
com_wrapper!(DrawingWeldings);
com_wrapper!(DrawingWelding);
com_wrapper!(DrawingViewGenerativeLinks);
com_wrapper!(DrawingCoordDims);
com_wrapper!(DrawingCoordDim);
com_wrapper!(PrintArea);

impl DrawingRoot {
    /// 活动图纸（`ActiveSheet`）。
    pub fn active_sheet(&self) -> ComResult<DrawingSheet> {
        Ok(DrawingSheet::new(self.com().get("ActiveSheet")?.as_obj()?))
    }

    /// 设置活动图纸（`ActiveSheet`）。
    pub fn set_active_sheet(&self, sheet: &DrawingSheet) -> ComResult<()> {
        self.com().put("ActiveSheet", Arg::Obj(sheet.com().clone()))
    }

    /// 按顺序重排图纸（`ReorderSheets`；SAFEARRAY of sheets）。
    pub fn reorder_sheets(&self, ordered: &[&DrawingSheet]) -> ComResult<()> {
        let args = Arg::SafeArray(ordered.iter().map(|s| Arg::Obj(s.com().clone())).collect());
        self.com().call("ReorderSheets", &[args]).map(|_| ())
    }
}

impl DrawingPageSetup {
    prop_pair!(
        choose_best_orientation,
        set_choose_best_orientation,
        "ChooseBestOrientation",
        bool
    );
    prop_pair!(
        fit_to_printer_format,
        set_fit_to_printer_format,
        "FitToPrinterFormat",
        bool
    );
    prop_pair!(
        fit_to_sheet_format,
        set_fit_to_sheet_format,
        "FitToSheetFormat",
        bool
    );
}

impl ComCollection for DrawingDimensions {
    type Item = DrawingDimension;
}

impl ComCollection for DrawingLeaders {
    type Item = DrawingLeader;
}

impl ComCollection for DrawingArrows {
    type Item = DrawingArrow;
}

impl ComCollection for DrawingPictures {
    type Item = DrawingPicture;
}

impl ComCollection for DrawingTables {
    type Item = DrawingTable;
}

impl ComCollection for DrawingComponents {
    type Item = DrawingComponent;
}

impl ComCollection for DrawingThreads {
    type Item = DrawingThread;
}

impl ComCollection for DrawingWeldings {
    type Item = DrawingWelding;
}

impl DrawingDimensions {
    /// 新建尺寸标注（`Add(类型, 几何元素数组, 选点坐标数组, 基本表示模式)`；
    /// geom_elem 为 Reference 数组、pt_coord_elem 为与元素一一对应的 x,y 坐标）。
    pub fn add(
        &self,
        type_dim: i32,
        geom_elems: &[Reference],
        pt_coord_elems: &[f64],
        line_rep: i32,
    ) -> ComResult<DrawingDimension> {
        let geoms = Arg::SafeArray(
            geom_elems
                .iter()
                .map(|r| Arg::Obj(r.com().clone()))
                .collect(),
        );
        let pts = Arg::SafeArray(pt_coord_elems.iter().map(|v| Arg::F64(*v)).collect());
        Ok(DrawingDimension::new(
            self.com()
                .call("Add", &[Arg::I32(type_dim), geoms, pts, Arg::I32(line_rep)])?
                .as_obj()?,
        ))
    }
}

impl DrawingDimension {
    prop_pair!(dual_value, set_dual_value, "DualValue", i32);
    prop_pair!(forshortened, set_forshortened, "Forshortened", bool);
    prop_pair!(half_dim_mode, set_half_dim_mode, "HalfDimMode", bool);
    prop_pair!(symbols_side, set_symbols_side, "SymbolsSide", i32);
    prop_pair!(value_angle, set_value_angle, "ValueAngle", f64);
    prop_pair!(value_auto_mode, set_value_auto_mode, "ValueAutoMode", i32);
    prop_pair!(value_display, set_value_display, "ValueDisplay", i32);
    prop_pair!(value_frame, set_value_frame, "ValueFrame", i32);
    prop_pair!(value_in_out, set_value_in_out, "ValueInOut", i32);
    prop_pair!(
        value_orientation,
        set_value_orientation,
        "ValueOrientation",
        i32
    );
    prop_pair!(value_reference, set_value_reference, "ValueReference", i32);
    prop_get!(dim_status, "DimStatus", i32);
    prop_get!(dim_type, "DimType", i32);
    prop_get!(cumulate_mode, "CumulateMode", i32);
    prop_get!(nb_ext_line, "NbExtLine", i32);
    prop_get!(nb_symb, "NbSymb", i32);

    /// 是否真尺寸模式（`TrueDimMode`；读失败多为未启用）。
    pub fn true_dim_mode(&self) -> ComResult<bool> {
        self.com().get("TrueDimMode")?.as_bool()
    }

    /// 是否被裁剪（`IsClipped`）。
    pub fn is_clipped(&self) -> ComResult<bool> {
        self.com().call("IsClipped", &[])?.as_bool()
    }

    /// 尺寸线（`GetDimLine`）。
    pub fn get_dim_line(&self) -> ComResult<DrawingDimLine> {
        Ok(DrawingDimLine::new(
            self.com().call("GetDimLine", &[])?.as_obj()?,
        ))
    }

    /// 尺寸界线（`GetDimExtLine`）。
    pub fn get_dim_ext_line(&self) -> ComResult<DrawingDimExtLine> {
        Ok(DrawingDimExtLine::new(
            self.com().call("GetDimExtLine", &[])?.as_obj()?,
        ))
    }

    /// 尺寸值对象（`GetValue`）。
    pub fn get_value(&self) -> ComResult<DrawingDimValue> {
        Ok(DrawingDimValue::new(
            self.com().call("GetValue", &[])?.as_obj()?,
        ))
    }

    /// 移动尺寸值（`MoveValue(x, y, sub_part, angle_behavior)`）。
    pub fn move_value(
        &self,
        x: f64,
        y: f64,
        sub_part: i32,
        dim_angle_behavior: i32,
    ) -> ComResult<()> {
        self.com()
            .call(
                "MoveValue",
                &[
                    Arg::F64(x),
                    Arg::F64(y),
                    Arg::I32(sub_part),
                    Arg::I32(dim_angle_behavior),
                ],
            )
            .map(|_| ())
    }

    /// 恢复尺寸值位置（`RestoreValuePosition`）。
    pub fn restore_value_position(&self) -> ComResult<()> {
        self.com().call("RestoreValuePosition", &[]).map(|_| ())
    }
}

impl DrawingDimLine {
    prop_pair!(color, set_color, "Color", i32);
    prop_pair!(thickness, set_thickness, "Thickness", f64);
    prop_pair!(
        dim_line_graph_rep,
        set_dim_line_graph_rep,
        "DimLineGraphRep",
        i32
    );
    prop_pair!(
        dim_line_orientation,
        set_dim_line_orientation,
        "DimLineOrientation",
        i32
    );
    prop_pair!(
        dim_line_reference,
        set_dim_line_reference,
        "DimLineReference",
        i32
    );
    prop_pair!(dim_line_type, set_dim_line_type, "DimLineType", i32);

    /// 尺寸线方向分量（`GetDimLineDir`，VBA 中转；2 分量 [x, y]）。
    pub fn get_dim_line_dir(&self, ss: &SystemService) -> ComResult<Vec<f64>> {
        ss.numeric_out_param(self.com(), "GetDimLineDir")
    }
}

impl DrawingDimExtLine {
    prop_pair!(color, set_color, "Color", i32);
    prop_pair!(thickness, set_thickness, "Thickness", f64);
    prop_pair!(ext_line_slant, set_ext_line_slant, "ExtLineSlant", f64);
    prop_pair!(ext_line_type, set_ext_line_type, "ExtLineType", i32);
}

impl DrawingDimValue {
    prop_pair!(fake_dim_type, set_fake_dim_type, "FakeDimType", i32);
    prop_pair!(scoring_mode, set_scoring_mode, "ScoringMode", i32);
    prop_pair!(
        value_framed_element,
        set_value_framed_element,
        "ValueFramedElement",
        i32
    );
    prop_pair!(
        value_framed_group,
        set_value_framed_group,
        "ValueFramedGroup",
        i32
    );

    /// 尺寸数值（`Value`；只读）。
    pub fn value(&self) -> ComResult<f64> {
        self.com().get("Value")?.as_f64()
    }

    /// 前后缀文本（`SetPsText(index, prefix, suffix)`）。
    pub fn set_ps_text(&self, index: i32, prefix: &str, suffix: &str) -> ComResult<()> {
        self.com()
            .call(
                "SetPsText",
                &[
                    Arg::I32(index),
                    Arg::Str(prefix.to_string()),
                    Arg::Str(suffix.to_string()),
                ],
            )
            .map(|_| ())
    }

    /// 假尺寸文本（`SetFakeDimValue(index, text)`）。
    pub fn set_fake_dim_value(&self, index: i32, text: &str) -> ComResult<()> {
        self.com()
            .call(
                "SetFakeDimValue",
                &[Arg::I32(index), Arg::Str(text.to_string())],
            )
            .map(|_| ())
    }
}

impl DrawingTextProperties {
    prop_pair!(anchor_point, set_anchor_point, "AnchorPoint", i32);
    prop_pair!(blanking, set_blanking, "Blanking", i32);
    prop_pair!(bold, set_bold, "Bold", i32);
    prop_pair!(color, set_color, "Color", i32);
    prop_pair!(font_name, set_font_name, "FontName", str);
    prop_pair!(font_size, set_font_size, "FontSize", f64);
    prop_pair!(frame_type, set_frame_type, "FrameType", i32);
    prop_pair!(italic, set_italic, "Italic", i32);
    prop_pair!(justification, set_justification, "Justification", i32);
    prop_pair!(kerning, set_kerning, "Kerning", i32);
    prop_pair!(mirror, set_mirror, "Mirror", i32);
    prop_pair!(overline, set_overline, "Overline", i32);
    prop_pair!(strike_thru, set_strike_thru, "StrikeThru", i32);
    prop_pair!(subscript, set_subscript, "Subscript", i32);
    prop_pair!(superscript, set_superscript, "Superscript", i32);
    prop_pair!(underline, set_underline, "Underline", i32);

    /// 激活文字边框（`ActivateFrame(type)`）。
    pub fn activate_frame(&self, frame_type: i32) -> ComResult<()> {
        self.com()
            .call("ActivateFrame", &[Arg::I32(frame_type)])
            .map(|_| ())
    }

    /// 刷新文字属性（`Update`）。
    pub fn update(&self) -> ComResult<()> {
        self.com().call("Update", &[]).map(|_| ())
    }
}

impl DrawingTextRange {
    /// 文本长度（`Length`）。
    pub fn length(&self) -> ComResult<i32> {
        self.com().get("Length")?.as_i32()
    }

    /// 起始位置（`Start`）。
    pub fn start(&self) -> ComResult<i32> {
        self.com().get("Start")?.as_i32()
    }

    prop_pair!(text, set_text, "Text", str);

    /// 文字属性（`TextProperties`）。
    pub fn text_properties(&self) -> ComResult<DrawingTextProperties> {
        Ok(DrawingTextProperties::new(
            self.com().get("TextProperties")?.as_obj()?,
        ))
    }

    /// 取子区间（`GetTextRange(start, end)`）。
    pub fn get_text_range(&self, start: i32, end: i32) -> ComResult<DrawingTextRange> {
        Ok(DrawingTextRange::new(
            self.com()
                .call("GetTextRange", &[Arg::I32(start), Arg::I32(end)])?
                .as_obj()?,
        ))
    }

    /// 之后插入文本（`InsertAfter`）。
    pub fn insert_after(&self, text: &str) -> ComResult<()> {
        self.com()
            .call("InsertAfter", &[Arg::Str(text.to_string())])
            .map(|_| ())
    }

    /// 之前插入文本（`InsertBefore`）。
    pub fn insert_before(&self, text: &str) -> ComResult<()> {
        self.com()
            .call("InsertBefore", &[Arg::Str(text.to_string())])
            .map(|_| ())
    }
}

impl DrawingLeaders {
    /// 新建引出线（`Add(head_x, head_y)`）。
    pub fn add(&self, head_x: f64, head_y: f64) -> ComResult<DrawingLeader> {
        Ok(DrawingLeader::new(
            self.com()
                .call("Add", &[Arg::F64(head_x), Arg::F64(head_y)])?
                .as_obj()?,
        ))
    }

    /// 删除（`Remove(index)`）。
    pub fn remove(&self, index: i32) -> ComResult<()> {
        self.com().call("Remove", &[Arg::I32(index)]).map(|_| ())
    }
}

impl DrawingLeader {
    prop_pair!(all_around, set_all_around, "AllAround", bool);
    prop_pair!(anchor_point, set_anchor_point, "AnchorPoint", i32);
    prop_pair!(anchor_symbol, set_anchor_symbol, "AnchorSymbol", i32);
    prop_pair!(head_symbol, set_head_symbol, "HeadSymbol", i32);
    prop_get!(nb_interruption, "NbInterruption", i32);
    prop_get!(nb_point, "NbPoint", i32);
}

impl DrawingArrows {
    /// 新建箭头（`Add(head_x, head_y, tail_x, tail_y)`）。
    pub fn add(
        &self,
        head_x: f64,
        head_y: f64,
        tail_x: f64,
        tail_y: f64,
    ) -> ComResult<DrawingArrow> {
        Ok(DrawingArrow::new(
            self.com()
                .call(
                    "Add",
                    &[
                        Arg::F64(head_x),
                        Arg::F64(head_y),
                        Arg::F64(tail_x),
                        Arg::F64(tail_y),
                    ],
                )?
                .as_obj()?,
        ))
    }
}

impl DrawingArrow {
    prop_pair!(head_symbol, set_head_symbol, "HeadSymbol", i32);
    prop_pair!(tail_symbol, set_tail_symbol, "TailSymbol", i32);
    prop_pair!(
        scale_on_extremities,
        set_scale_on_extremities,
        "ScaleOnExtremities",
        bool
    );
    prop_get!(nb_interruption, "NbInterruption", i32);
    prop_get!(nb_point, "NbPoint", i32);
}

impl DrawingPictures {
    /// 插入图片（`Add(path, x, y)`）。
    pub fn add(&self, path: &str, x: f64, y: f64) -> ComResult<DrawingPicture> {
        Ok(DrawingPicture::new(
            self.com()
                .call(
                    "Add",
                    &[Arg::Str(path.to_string()), Arg::F64(x), Arg::F64(y)],
                )?
                .as_obj()?,
        ))
    }

    /// 删除（`Remove(index)`）。
    pub fn remove(&self, index: i32) -> ComResult<()> {
        self.com().call("Remove", &[Arg::I32(index)]).map(|_| ())
    }
}

impl DrawingPicture {
    prop_pair!(x, set_x, "X", f64);
    prop_pair!(y, set_y, "Y", f64);
    prop_pair!(width, set_width, "Width", f64);
    prop_pair!(height, set_height, "Height", f64);
    prop_pair!(crop_bottom, set_crop_bottom, "CropBottom", f64);
    prop_pair!(crop_left, set_crop_left, "CropLeft", f64);
    prop_pair!(crop_right, set_crop_right, "CropRight", f64);
    prop_pair!(crop_top, set_crop_top, "CropTop", f64);
    prop_pair!(format, set_format, "Format", i32);
    prop_pair!(ratio_lock, set_ratio_lock, "RatioLock", bool);
    prop_get!(get_original_width, "GetOriginalWidth", f64);
    prop_get!(get_original_height, "GetOriginalHeight", f64);
}

impl DrawingTables {
    /// 新建表格（`Add(x, y, 行数, 列数, 行高, 列宽)`）。
    #[allow(clippy::too_many_arguments)]
    pub fn add(
        &self,
        x: f64,
        y: f64,
        rows: i32,
        columns: i32,
        row_height: f64,
        column_width: f64,
    ) -> ComResult<DrawingTable> {
        Ok(DrawingTable::new(
            self.com()
                .call(
                    "Add",
                    &[
                        Arg::F64(x),
                        Arg::F64(y),
                        Arg::I32(rows),
                        Arg::I32(columns),
                        Arg::F64(row_height),
                        Arg::F64(column_width),
                    ],
                )?
                .as_obj()?,
        ))
    }
}

impl DrawingTable {
    prop_pair!(anchor_point, set_anchor_point, "AnchorPoint", i32);
    prop_pair!(angle, set_angle, "Angle", f64);
    prop_pair!(compute_mode, set_compute_mode, "ComputeMode", i32);
    prop_pair!(
        orientation_reference,
        set_orientation_reference,
        "OrientationReference",
        i32
    );
    prop_pair!(x, set_x, "X", f64);
    prop_pair!(y, set_y, "Y", f64);
    prop_get!(number_of_rows, "NumberOfRows", i32);
    prop_get!(number_of_columns, "NumberOfColumns", i32);

    /// 文字属性（`TextProperties`）。
    pub fn text_properties(&self) -> ComResult<DrawingTextProperties> {
        Ok(DrawingTextProperties::new(
            self.com().get("TextProperties")?.as_obj()?,
        ))
    }

    /// 追加行（`AddRow(row)`）。
    pub fn add_row(&self, row: i32) -> ComResult<()> {
        self.com().call("AddRow", &[Arg::I32(row)]).map(|_| ())
    }

    /// 追加列（`AddColumn(col)`）。
    pub fn add_column(&self, column: i32) -> ComResult<()> {
        self.com()
            .call("AddColumn", &[Arg::I32(column)])
            .map(|_| ())
    }

    /// 读单元格文本（`GetCellString(row, col)`）。
    pub fn get_cell_string(&self, row: i32, column: i32) -> ComResult<String> {
        self.com()
            .call("GetCellString", &[Arg::I32(row), Arg::I32(column)])?
            .as_string()
    }

    /// 写单元格文本（`SetCellString(row, col, text)`；CAA 原名 SetCellString/
    /// ModifyCellString 视版本，此为 SetCellString 口径）。
    pub fn set_cell_string(&self, row: i32, column: i32, text: &str) -> ComResult<()> {
        self.com()
            .call(
                "SetCellString",
                &[Arg::I32(row), Arg::I32(column), Arg::Str(text.to_string())],
            )
            .map(|_| ())
    }

    /// 读列宽（`GetColumnSize(col)`）。
    pub fn get_column_size(&self, column: i32) -> ComResult<f64> {
        self.com()
            .call("GetColumnSize", &[Arg::I32(column)])?
            .as_f64()
    }
}

impl DrawingGDTs {
    /// 删除（`Remove(index)`）。
    pub fn remove(&self, index: i32) -> ComResult<()> {
        self.com().call("Remove", &[Arg::I32(index)]).map(|_| ())
    }
}

impl DrawingGDT {
    prop_pair!(angle, set_angle, "Angle", f64);
    prop_pair!(x, set_x, "X", f64);
    prop_pair!(y, set_y, "Y", f64);
    prop_get!(row_number, "RowNumber", i32);

    /// 引出线集合（`Leaders`）。
    pub fn leaders(&self) -> ComResult<DrawingLeaders> {
        Ok(DrawingLeaders::new(self.com().get("Leaders")?.as_obj()?))
    }

    /// 文字属性（`TextProperties`）。
    pub fn text_properties(&self) -> ComResult<DrawingTextProperties> {
        Ok(DrawingTextProperties::new(
            self.com().get("TextProperties")?.as_obj()?,
        ))
    }

    /// 设形位公差符号（`SetToleranceType(row, symbol)`）。
    pub fn set_tolerance_type(&self, row: i32, symbol: i32) -> ComResult<()> {
        self.com()
            .call("SetToleranceType", &[Arg::I32(row), Arg::I32(symbol)])
            .map(|_| ())
    }

    /// 读形位公差符号（`GetToleranceType(row)`）。
    pub fn get_tolerance_type(&self, row: i32) -> ComResult<i32> {
        self.com()
            .call("GetToleranceType", &[Arg::I32(row)])?
            .as_i32()
    }
}

impl DrawingComponents {
    /// 插入组件（`Add(参考视图, x, y)`）。
    pub fn add(&self, reference: &DrawingView, x: f64, y: f64) -> ComResult<DrawingComponent> {
        Ok(DrawingComponent::new(
            self.com()
                .call(
                    "Add",
                    &[Arg::Obj(reference.com().clone()), Arg::F64(x), Arg::F64(y)],
                )?
                .as_obj()?,
        ))
    }

    /// 删除（`Remove(index)`）。
    pub fn remove(&self, index: i32) -> ComResult<()> {
        self.com().call("Remove", &[Arg::I32(index)]).map(|_| ())
    }
}

impl DrawingComponent {
    prop_pair!(angle, set_angle, "Angle", f64);
    prop_pair!(x, set_x, "X", f64);
    prop_pair!(y, set_y, "Y", f64);
    prop_pair!(scale2, set_scale2, "Scale2", f64);

    /// 组件参考视图（`CompRef`）。
    pub fn comp_ref(&self) -> ComResult<DrawingView> {
        Ok(DrawingView::new(self.com().get("CompRef")?.as_obj()?))
    }

    /// 翻转（`Flip`）。
    pub fn flip(&self) -> ComResult<()> {
        self.com().call("Flip", &[]).map(|_| ())
    }

    /// 是否翻转（`GetFlip`）。
    pub fn get_flip(&self) -> ComResult<bool> {
        self.com().call("GetFlip", &[])?.as_bool()
    }
}

impl DrawingThreads {
    /// 新建螺纹标注（`Add(几何元素数组)`；SAFEARRAY of Reference）。
    pub fn add(&self, geom_elems: &[Reference]) -> ComResult<DrawingThread> {
        let args = Arg::SafeArray(
            geom_elems
                .iter()
                .map(|r| Arg::Obj(r.com().clone()))
                .collect(),
        );
        Ok(DrawingThread::new(
            self.com().call("Add", &[args])?.as_obj()?,
        ))
    }

    /// 删除（`Remove(index)`）。
    pub fn remove(&self, index: i32) -> ComResult<()> {
        self.com().call("Remove", &[Arg::I32(index)]).map(|_| ())
    }
}

impl DrawingThread {
    /// 螺纹类型（`Type`；Rust 保留字故带 thread 前缀）。
    pub fn thread_type(&self) -> ComResult<i32> {
        self.com().get("Type")?.as_i32()
    }

    /// 设置螺纹类型（`Type`）。
    pub fn set_thread_type(&self, value: i32) -> ComResult<()> {
        self.com().put("Type", Arg::I32(value))
    }

    /// 是否关联 3D（`IsLinkedTo`）。
    pub fn is_linked_to(&self) -> ComResult<bool> {
        self.com().call("IsLinkedTo", &[])?.as_bool()
    }
}

impl DrawingWeldings {
    /// 新建焊接标注（`Add(symbol, x, y)`）。
    pub fn add(&self, symbol: i32, x: f64, y: f64) -> ComResult<DrawingWelding> {
        Ok(DrawingWelding::new(
            self.com()
                .call("Add", &[Arg::I32(symbol), Arg::F64(x), Arg::F64(y)])?
                .as_obj()?,
        ))
    }

    /// 删除（`Remove(index)`）。
    pub fn remove(&self, index: i32) -> ComResult<()> {
        self.com().call("Remove", &[Arg::I32(index)]).map(|_| ())
    }
}

impl DrawingWelding {
    prop_pair!(angle, set_angle, "Angle", f64);
    prop_pair!(x, set_x, "X", f64);
    prop_pair!(y, set_y, "Y", f64);
    prop_pair!(
        identification_line_side,
        set_identification_line_side,
        "IdentificationLineSide",
        i32
    );
    prop_pair!(welding_side, set_welding_side, "WeldingSide", i32);
    prop_pair!(welding_tail, set_welding_tail, "WeldingTail", i32);

    /// 引出线集合（`Leaders`）。
    pub fn leaders(&self) -> ComResult<DrawingLeaders> {
        Ok(DrawingLeaders::new(self.com().get("Leaders")?.as_obj()?))
    }

    /// 文字属性（`TextProperties`）。
    pub fn text_properties(&self) -> ComResult<DrawingTextProperties> {
        Ok(DrawingTextProperties::new(
            self.com().get("TextProperties")?.as_obj()?,
        ))
    }

    /// 设焊接符号（`SetSymbol(symbol, weld)`）。
    pub fn set_symbol(&self, symbol: i32, weld: i32) -> ComResult<()> {
        self.com()
            .call("SetSymbol", &[Arg::I32(symbol), Arg::I32(weld)])
            .map(|_| ())
    }

    /// 读焊接符号（`GetSymbol(weld)`）。
    pub fn get_symbol(&self, weld: i32) -> ComResult<i32> {
        self.com().call("GetSymbol", &[Arg::I32(weld)])?.as_i32()
    }
}

impl DrawingViewGenerativeLinks {
    /// 追加链接（`AddLink(obj)`）。
    pub fn add_link(&self, link: &impl ComObject) -> ComResult<()> {
        self.com()
            .call("AddLink", &[Arg::Obj(link.com().clone())])
            .map(|_| ())
    }

    /// 清空链接（`RemoveAllLinks`）。
    pub fn remove_all_links(&self) -> ComResult<()> {
        self.com().call("RemoveAllLinks", &[]).map(|_| ())
    }

    /// 首个链接（`FirstLink`；遍历用 `NextLink`，走 com()）。
    pub fn first_link(&self) -> ComResult<ComObj> {
        self.com().call("FirstLink", &[])?.as_obj()
    }
}

impl DrawingCoordDims {
    /// 删除（`Remove(index)`）。
    pub fn remove(&self, index: i32) -> ComResult<()> {
        self.com().call("Remove", &[Arg::I32(index)]).map(|_| ())
    }
}

impl DrawingCoordDim {
    prop_pair!(angle, set_angle, "Angle", f64);
    prop_pair!(x, set_x, "X", f64);
    prop_pair!(y, set_y, "Y", f64);

    /// 文字属性（`TextProperties`）。
    pub fn text_properties(&self) -> ComResult<DrawingTextProperties> {
        Ok(DrawingTextProperties::new(
            self.com().get("TextProperties")?.as_obj()?,
        ))
    }
}

impl PrintArea {
    prop_pair!(
        activation_state,
        set_activation_state,
        "ActivationState",
        bool
    );
    prop_pair!(area_height, set_area_height, "AreaHeight", f64);
    prop_pair!(area_width, set_area_width, "AreaWidth", f64);
    prop_pair!(area_low_x, set_area_low_x, "AreaLowX", f64);
    prop_pair!(area_low_y, set_area_low_y, "AreaLowY", f64);

    /// 一次性设打印区域（`SetArea(x, y, w, h)`）。
    pub fn set_area(&self, x: f64, y: f64, width: f64, height: f64) -> ComResult<()> {
        self.com()
            .call(
                "SetArea",
                &[Arg::F64(x), Arg::F64(y), Arg::F64(width), Arg::F64(height)],
            )
            .map(|_| ())
    }
}
