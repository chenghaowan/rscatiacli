//! 导航器族（pycatia `navigator_interfaces`，阶段 8）。
//!
//! DMU 审查（`DMUReview(s)`）、批注视图（`AnnotatedView(s)`）、标记
//! （`Marker2D(s)` / `Marker3D(s)`）、超链接（`Hyperlink(s)`）、数据流
//! （`DMUDataFlow`）与工作台入口（`NavigatorWorkbench`）。既有 `Group(s)`
//! 在 [`super::groups`]。
//!
//! 获取路径：`Document::GetItem("NavigatorWorkbench")`（或产品文档侧）；
//! 标记集合挂 `AnnotatedView::marker2_ds()` 与 `NavigatorWorkbench::marker3_ds()`。
//! 出参位置数组走 `SystemService` VBA 中转。

use crate::collections::ComCollection;
use crate::dispatch::{Arg, ComObj, ComObject};
use crate::error::ComResult;

use super::product::Product;
use super::system_service::SystemService;
use super::viewer::Viewpoint3D;

com_wrapper!(NavigatorWorkbench);
com_wrapper!(DMUReview);
com_wrapper!(DMUReviews);
com_wrapper!(DMUDataFlow);
com_wrapper!(AnnotatedView);
com_wrapper!(AnnotatedViews);
com_wrapper!(Marker2D);
com_wrapper!(Marker2Ds);
com_wrapper!(Marker3D);
com_wrapper!(Marker3Ds);
com_wrapper!(Hyperlink);
com_wrapper!(Hyperlinks);

impl ComCollection for DMUReviews {
    type Item = DMUReview;
}

impl ComCollection for AnnotatedViews {
    type Item = AnnotatedView;
}

impl ComCollection for Marker2Ds {
    type Item = Marker2D;
}

impl ComCollection for Marker3Ds {
    type Item = Marker3D;
}

impl ComCollection for Hyperlinks {
    type Item = Hyperlink;
}

impl NavigatorWorkbench {
    /// 批注视图集合（`AnnotatedViews`）。
    pub fn annotated_views(&self) -> ComResult<AnnotatedViews> {
        Ok(AnnotatedViews::new(
            self.com().get("AnnotatedViews")?.as_obj()?,
        ))
    }

    /// 组集合（`Groups`）。
    pub fn groups(&self) -> ComResult<super::groups::Groups> {
        Ok(super::groups::Groups::new(
            self.com().get("Groups")?.as_obj()?,
        ))
    }

    /// 超链接集合（`Hyperlinks`）。
    pub fn hyperlinks(&self) -> ComResult<Hyperlinks> {
        Ok(Hyperlinks::new(self.com().get("Hyperlinks")?.as_obj()?))
    }

    /// 3D 标记集合（`Marker3Ds`）。
    pub fn marker3_ds(&self) -> ComResult<Marker3Ds> {
        Ok(Marker3Ds::new(self.com().get("Marker3Ds")?.as_obj()?))
    }

    /// DMU 数据流（`DMUDataFlow`）。
    pub fn dmu_data_flow(&self) -> ComResult<DMUDataFlow> {
        Ok(DMUDataFlow::new(self.com().get("DMUDataFlow")?.as_obj()?))
    }

    /// 定制批注视图（`AdvancedView(view, option)`）。
    pub fn advanced_view(&self, annotated_view: &AnnotatedView, view_option: i32) -> ComResult<()> {
        self.com()
            .call(
                "AdvancedView",
                &[
                    Arg::Obj(annotated_view.com().clone()),
                    Arg::I32(view_option),
                ],
            )
            .map(|_| ())
    }

    /// 对象显示顺序（`GetOrder`）。
    pub fn get_order(&self, object: &impl ComObject) -> ComResult<i32> {
        self.com()
            .call("GetOrder", &[Arg::Obj(object.com().clone())])?
            .as_i32()
    }

    /// 设置对象显示顺序（`SetOrder`）。
    pub fn set_order(&self, object: &impl ComObject, new_rank: i32) -> ComResult<()> {
        self.com()
            .call(
                "SetOrder",
                &[Arg::Obj(object.com().clone()), Arg::I32(new_rank)],
            )
            .map(|_| ())
    }

    /// 进入批注视图（`View(view)`）。
    pub fn view(&self, annotated_view: &AnnotatedView) -> ComResult<()> {
        self.com()
            .call("View", &[Arg::Obj(annotated_view.com().clone())])
            .map(|_| ())
    }
}

impl DMUReviews {
    /// 当前审查（`Current`）。
    pub fn current(&self) -> ComResult<DMUReview> {
        Ok(DMUReview::new(self.com().call("Current", &[])?.as_obj()?))
    }

    /// 新建审查（`Add`）。
    pub fn add(&self) -> ComResult<DMUReview> {
        Ok(DMUReview::new(self.com().call("Add", &[])?.as_obj()?))
    }

    /// 从产品导入（`ImportFrom(product)`）。
    pub fn import_from(&self, product: &Product) -> ComResult<()> {
        self.com()
            .call("ImportFrom", &[Arg::Obj(product.com().clone())])
            .map(|_| ())
    }

    /// 删除（`Remove(index)`）。
    pub fn remove(&self, index: i32) -> ComResult<()> {
        self.com().call("Remove", &[Arg::I32(index)]).map(|_| ())
    }
}

impl DMUReview {
    prop_pair!(activation, set_activation, "Activation", i32);

    /// 子审查集合（`DMUReviews`）。
    pub fn dmu_reviews(&self) -> ComResult<DMUReviews> {
        Ok(DMUReviews::new(self.com().get("DMUReviews")?.as_obj()?))
    }
}

impl DMUDataFlow {
    /// 缓存导出（`CacheExport(directory, prefix, data)`）。
    pub fn cache_export(&self, directory: &str, prefix: &str, data: i32) -> ComResult<()> {
        self.com()
            .call(
                "CacheExport",
                &[
                    Arg::Str(directory.to_string()),
                    Arg::Str(prefix.to_string()),
                    Arg::I32(data),
                ],
            )
            .map(|_| ())
    }

    /// 缓存导入（`CacheImport(directory)`）。
    pub fn cache_import(&self, directory: &str) -> ComResult<()> {
        self.com()
            .call("CacheImport", &[Arg::Str(directory.to_string())])
            .map(|_| ())
    }

    /// 折叠（`Collapse`）。
    pub fn collapse(&self) -> ComResult<()> {
        self.com().call("Collapse", &[]).map(|_| ())
    }

    /// 替换为 CGR（`ReplaceByCgr(directory, prefix)`）。
    pub fn replace_by_cgr(&self, directory: &str, prefix: &str) -> ComResult<()> {
        self.com()
            .call(
                "ReplaceByCgr",
                &[
                    Arg::Str(directory.to_string()),
                    Arg::Str(prefix.to_string()),
                ],
            )
            .map(|_| ())
    }

    /// 冻结另存（`SaveAsFrozen(directory, prefix, data, cache)`）。
    pub fn save_as_frozen(
        &self,
        directory: &str,
        prefix: &str,
        data: i32,
        cache: i32,
    ) -> ComResult<()> {
        self.com()
            .call(
                "SaveAsFrozen",
                &[
                    Arg::Str(directory.to_string()),
                    Arg::Str(prefix.to_string()),
                    Arg::I32(data),
                    Arg::I32(cache),
                ],
            )
            .map(|_| ())
    }
}

impl AnnotatedViews {
    /// 新建批注视图（`Add`）。
    pub fn add(&self) -> ComResult<AnnotatedView> {
        Ok(AnnotatedView::new(self.com().call("Add", &[])?.as_obj()?))
    }

    /// 从视点新建（`AddFromViewpoint`）。
    pub fn add_from_viewpoint(&self, viewpoint: &Viewpoint3D) -> ComResult<AnnotatedView> {
        Ok(AnnotatedView::new(
            self.com()
                .call("AddFromViewpoint", &[Arg::Obj(viewpoint.com().clone())])?
                .as_obj()?,
        ))
    }

    /// 删除（`Remove(index)`）。
    pub fn remove(&self, index: i32) -> ComResult<()> {
        self.com().call("Remove", &[Arg::I32(index)]).map(|_| ())
    }
}

impl AnnotatedView {
    prop_pair!(behavior_mode, set_behavior_mode, "BehaviorMode", i32);
    prop_pair!(comment, set_comment, "Comment", str);
    prop_pair!(projection_mode, set_projection_mode, "ProjectionMode", i32);
    prop_pair!(sound, set_sound, "Sound", str);

    /// 视野（`FieldOfView`）。
    pub fn field_of_view(&self) -> ComResult<f64> {
        self.com().get("FieldOfView")?.as_f64()
    }

    /// 缩放（`Zoom`）。
    pub fn zoom(&self) -> ComResult<f64> {
        self.com().get("Zoom")?.as_f64()
    }

    /// 2D 标记集合（`Marker2Ds`）。
    pub fn marker2_ds(&self) -> ComResult<Marker2Ds> {
        Ok(Marker2Ds::new(self.com().get("Marker2Ds")?.as_obj()?))
    }

    /// 原点分量（`GetOrigin`，VBA 中转；3 分量）。
    pub fn get_origin(&self, ss: &SystemService) -> ComResult<Vec<f64>> {
        ss.numeric_out_param(self.com(), "GetOrigin")
    }

    /// 视线分量（`GetSightDirection`，VBA 中转；3 分量）。
    pub fn get_sight_direction(&self, ss: &SystemService) -> ComResult<Vec<f64>> {
        ss.numeric_out_param(self.com(), "GetSightDirection")
    }

    /// 上方向分量（`GetUpDirection`，VBA 中转；3 分量）。
    pub fn get_up_direction(&self, ss: &SystemService) -> ComResult<Vec<f64>> {
        ss.numeric_out_param(self.com(), "GetUpDirection")
    }

    /// 刷新（`Update`）。
    pub fn update(&self) -> ComResult<()> {
        self.com().call("Update", &[]).map(|_| ())
    }
}

impl Marker2D {
    prop_pair!(fill, set_fill, "Fill", i32);
    prop_pair!(frame, set_frame, "Frame", i32);
    prop_pair!(picture, set_picture, "Picture", str);
    prop_pair!(text, set_text, "Text", str);
    prop_pair!(text_angle, set_text_angle, "TextAngle", f64);
    prop_pair!(text_font, set_text_font, "TextFont", str);
    prop_pair!(
        text_orientation,
        set_text_orientation,
        "TextOrientation",
        i32
    );
    prop_pair!(text_size, set_text_size, "TextSize", f64);

    /// 标记类型（`Type`；Rust 保留字故带 marker 前缀）。
    pub fn marker_type(&self) -> ComResult<i32> {
        self.com().get("Type")?.as_i32()
    }

    /// 位置分量（`GetPositions`，VBA 中转；分量数按标记类型）。
    pub fn get_positions(&self, ss: &SystemService) -> ComResult<Vec<f64>> {
        ss.numeric_out_param(self.com(), "GetPositions")
    }

    /// 设置位置分量（`SetPositions`）。
    pub fn set_positions(&self, coordinates: &[f64]) -> ComResult<()> {
        let args = Arg::SafeArray(coordinates.iter().map(|v| Arg::F64(*v)).collect());
        self.com().call("SetPositions", &[args]).map(|_| ())
    }
}

impl Marker2Ds {
    fn add_marker(&self, member: &str, extra: &[Arg], coordinates: &[f64]) -> ComResult<Marker2D> {
        let mut args: Vec<Arg> = vec![Arg::SafeArray(
            coordinates.iter().map(|v| Arg::F64(*v)).collect(),
        )];
        args.extend_from_slice(extra);
        Ok(Marker2D::new(self.com().call(member, &args)?.as_obj()?))
    }

    /// 2D 箭头（`Add2DArrow(coords)`；coords 为点序列 x1,y1,x2,y2...）。
    pub fn add2d_arrow(&self, coordinates: &[f64]) -> ComResult<Marker2D> {
        self.add_marker("Add2DArrow", &[], coordinates)
    }

    /// 2D 圆（`Add2DCircle(coords, fill)`）。
    pub fn add2d_circle(&self, coordinates: &[f64], fill_status: i32) -> ComResult<Marker2D> {
        self.add_marker("Add2DCircle", &[Arg::I32(fill_status)], coordinates)
    }

    /// 2D 手绘（`Add2DFreeHand(coords)`）。
    pub fn add2d_free_hand(&self, coordinates: &[f64]) -> ComResult<Marker2D> {
        self.add_marker("Add2DFreeHand", &[], coordinates)
    }

    /// 2D 直线（`Add2DLine(coords)`）。
    pub fn add2d_line(&self, coordinates: &[f64]) -> ComResult<Marker2D> {
        self.add_marker("Add2DLine", &[], coordinates)
    }

    /// 2D 图片（`Add2DPicture(coords, path)`）。
    pub fn add2d_picture(&self, coordinates: &[f64], path: &str) -> ComResult<Marker2D> {
        self.add_marker("Add2DPicture", &[Arg::Str(path.to_string())], coordinates)
    }

    /// 2D 矩形（`Add2DRectangle(coords, fill)`）。
    pub fn add2d_rectangle(&self, coordinates: &[f64], fill_status: i32) -> ComResult<Marker2D> {
        self.add_marker("Add2DRectangle", &[Arg::I32(fill_status)], coordinates)
    }

    /// 2D 文字（`Add2DText(coords, text)`）。
    pub fn add2d_text(&self, coordinates: &[f64], text: &str) -> ComResult<Marker2D> {
        self.add_marker("Add2DText", &[Arg::Str(text.to_string())], coordinates)
    }

    /// 删除（`Remove(index)`）。
    pub fn remove(&self, index: i32) -> ComResult<()> {
        self.com().call("Remove", &[Arg::I32(index)]).map(|_| ())
    }
}

impl Marker3D {
    prop_pair!(fill, set_fill, "Fill", i32);
    prop_pair!(frame, set_frame, "Frame", i32);
    prop_pair!(text, set_text, "Text", str);
    prop_pair!(text_font, set_text_font, "TextFont", str);
    prop_pair!(
        text_orientation,
        set_text_orientation,
        "TextOrientation",
        i32
    );
    prop_pair!(text_size, set_text_size, "TextSize", f64);

    /// 标记类型（`Type`；Rust 保留字故带 marker 前缀）。
    pub fn marker_type(&self) -> ComResult<i32> {
        self.com().get("Type")?.as_i32()
    }

    /// 追加关联对象（`AddObject`）。
    pub fn add_object(&self, object: &impl ComObject) -> ComResult<()> {
        self.com()
            .call("AddObject", &[Arg::Obj(object.com().clone())])
            .map(|_| ())
    }

    /// 关联对象数（`CountObject`）。
    pub fn count_object(&self) -> ComResult<i32> {
        self.com().call("CountObject", &[])?.as_i32()
    }

    /// 按序号取关联对象（`ItemObject`）。
    pub fn item_object(&self, index: i32) -> ComResult<ComObj> {
        self.com().call("ItemObject", &[Arg::I32(index)])?.as_obj()
    }

    /// 移除关联对象（`RemoveObject(index)`）。
    pub fn remove_object(&self, index: i32) -> ComResult<()> {
        self.com()
            .call("RemoveObject", &[Arg::I32(index)])
            .map(|_| ())
    }

    /// 对象上锚点分量（`GetObjectPositions(index)`，VBA 中转）。
    pub fn get_object_positions(&self, index: i32, ss: &SystemService) -> ComResult<Vec<f64>> {
        let function = "rscatiacli_get_object_positions";
        let code = format!(
            r#"
        Public Function {function}(oMarker As Object, iIndex As Long)
            Dim oValues() As Variant
            oMarker.GetObjectPositions iIndex, oValues
            {function} = oValues
        End Function
        "#
        );
        ss.evaluate(
            &code,
            0,
            function,
            &[Arg::Obj(self.com().clone()), Arg::I32(index)],
        )?
        .as_f64_array()
    }

    /// 文字锚点分量（`GetTextPositions`，VBA 中转）。
    pub fn get_text_positions(&self, ss: &SystemService) -> ComResult<Vec<f64>> {
        ss.numeric_out_param(self.com(), "GetTextPositions")
    }

    /// 设置对象锚点（`SetObjectPositions(index, coords)`）。
    pub fn set_object_positions(&self, index: i32, coordinates: &[f64]) -> ComResult<()> {
        let coords = Arg::SafeArray(coordinates.iter().map(|v| Arg::F64(*v)).collect());
        self.com()
            .call("SetObjectPositions", &[Arg::I32(index), coords])
            .map(|_| ())
    }

    /// 设置文字锚点（`SetTextPositions(coords)`）。
    pub fn set_text_positions(&self, coordinates: &[f64]) -> ComResult<()> {
        let coords = Arg::SafeArray(coordinates.iter().map(|v| Arg::F64(*v)).collect());
        self.com().call("SetTextPositions", &[coords]).map(|_| ())
    }

    /// 刷新（`Update`）。
    pub fn update(&self) -> ComResult<()> {
        self.com().call("Update", &[]).map(|_| ())
    }
}

impl Marker3Ds {
    /// 3D 文字标记（`Add3DText(text_coords, text, object_coords, object)`）。
    pub fn add3d_text(
        &self,
        text_coordinates: &[f64],
        text: &str,
        object_coordinates: &[f64],
        object: &impl ComObject,
    ) -> ComResult<Marker3D> {
        let tc = Arg::SafeArray(text_coordinates.iter().map(|v| Arg::F64(*v)).collect());
        let oc = Arg::SafeArray(object_coordinates.iter().map(|v| Arg::F64(*v)).collect());
        Ok(Marker3D::new(
            self.com()
                .call(
                    "Add3DText",
                    &[
                        tc,
                        Arg::Str(text.to_string()),
                        oc,
                        Arg::Obj(object.com().clone()),
                    ],
                )?
                .as_obj()?,
        ))
    }

    /// 删除（`Remove(index)`）。
    pub fn remove(&self, index: i32) -> ComResult<()> {
        self.com().call("Remove", &[Arg::I32(index)]).map(|_| ())
    }
}

impl Hyperlinks {
    /// 对象加入超链接集合（`Add(object)` -> Hyperlink）。
    pub fn add(&self, object: &impl ComObject) -> ComResult<Hyperlink> {
        Ok(Hyperlink::new(
            self.com()
                .call("Add", &[Arg::Obj(object.com().clone())])?
                .as_obj()?,
        ))
    }

    /// 取对象的超链接（`GetHyperlink`）。
    pub fn get_hyperlink(&self, object: &impl ComObject) -> ComResult<Hyperlink> {
        Ok(Hyperlink::new(
            self.com()
                .call("GetHyperlink", &[Arg::Obj(object.com().clone())])?
                .as_obj()?,
        ))
    }

    /// 删除（`Remove(index)`）。
    pub fn remove(&self, index: i32) -> ComResult<()> {
        self.com().call("Remove", &[Arg::I32(index)]).map(|_| ())
    }
}

impl Hyperlink {
    /// 追加 URL（`AddURL(url)`）。
    pub fn add_url(&self, url: &str) -> ComResult<()> {
        self.com()
            .call("AddURL", &[Arg::Str(url.to_string())])
            .map(|_| ())
    }

    /// URL 数（`CountObject`）。
    pub fn count_object(&self) -> ComResult<i32> {
        self.com().call("CountObject", &[])?.as_i32()
    }

    /// 按序号取 URL（`ItemObject`）。
    pub fn item_object(&self, index: i32) -> ComResult<ComObj> {
        self.com().call("ItemObject", &[Arg::I32(index)])?.as_obj()
    }

    /// 移除 URL（`RemoveObject(index)`）。
    pub fn remove_object(&self, index: i32) -> ComResult<()> {
        self.com()
            .call("RemoveObject", &[Arg::I32(index)])
            .map(|_| ())
    }
}
