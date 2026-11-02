//! `Window` / `Viewer3D`（3D 查看器截图通道）。
//!
//! 对应 rule_bounding_box / capture_active_part 的截图流程：
//! `Application.ActiveWindow` → `Window.ActiveViewer` → 隐藏规格树 /
//! 白底 / Reframe / `CaptureToFile(4=PNG, 路径)`。图像裁剪缩放量化在工具层
//! （image + color_quant + png crate，见 rscatiacli-tools image_post.rs）。
//! 阶段 4 补齐 pycatia `in_interfaces` 的 Window/Viewer3D/Viewpoint3D 成员面。

use crate::dispatch::{Arg, ComObject};
use crate::error::ComResult;

use super::in_interfaces::LightSources;
use super::system_service::SystemService;

com_wrapper!(Window);
com_wrapper!(Viewer3D);
com_wrapper!(Viewpoint3D);

impl Window {
    /// 当前活动查看器。
    pub fn active_viewer(&self) -> ComResult<Viewer3D> {
        Ok(Viewer3D::new(self.com().get("ActiveViewer")?.as_obj()?))
    }
    /// 窗口布局（1 = catWindowGeomOnly, 相当于 F3 只显示几何; 截图前切换）。
    pub fn layout(&self) -> ComResult<i32> {
        self.com().get("Layout")?.as_i32()
    }

    pub fn set_layout(&self, value: i32) -> ComResult<()> {
        self.com().put("Layout", Arg::I32(value))
    }

    prop_pair!(caption, set_caption, "Caption", str);
    prop_pair!(height, set_height, "Height", i32);
    prop_pair!(width, set_width, "Width", i32);
    prop_pair!(left, set_left, "Left", i32);
    prop_pair!(top, set_top, "Top", i32);
    prop_pair!(window_state, set_window_state, "WindowState", i32);
    prop_pair!(page_setup, set_page_setup, "PageSetup", PageSetup);

    /// 激活下一个窗口（`ActivateNext`）。
    pub fn activate_next(&self) -> ComResult<()> {
        self.com().call("ActivateNext", &[]).map(|_| ())
    }

    /// 激活上一个窗口（`ActivatePrevious`）。
    pub fn activate_previous(&self) -> ComResult<()> {
        self.com().call("ActivatePrevious", &[]).map(|_| ())
    }

    /// 打印（`PrintOut`）。
    pub fn print_out(&self) -> ComResult<()> {
        self.com().call("PrintOut", &[]).map(|_| ())
    }

    /// 打印到文件（`PrintToFile`）。
    pub fn print_to_file(&self, file_name: &str) -> ComResult<()> {
        self.com()
            .call("PrintToFile", &[Arg::Str(file_name.to_string())])
            .map(|_| ())
    }

    /// 新建窗口（`NewWindow`）。
    pub fn new_window(&self) -> ComResult<Window> {
        Ok(Window::new(self.com().call("NewWindow", &[])?.as_obj()?))
    }
}

impl Viewer3D {
    /// 规格树可见性（截图前置 false）。
    pub fn set_tree_visible(&self, value: bool) -> ComResult<()> {
        self.com().put("TreeVisible", Arg::Bool(value))
    }

    /// 背景色（0..1 的 RGB；截图前改纯白）。
    pub fn put_background_color(&self, rgb: [f64; 3]) -> ComResult<()> {
        self.com()
            .call(
                "PutBackgroundColor",
                &[Arg::SafeArray(vec![
                    Arg::F64(rgb[0]),
                    Arg::F64(rgb[1]),
                    Arg::F64(rgb[2]),
                ])],
            )
            .map(|_| ())
    }

    /// 视图重画（全屏适配）。
    pub fn reframe(&self) -> ComResult<()> {
        self.com().call("Reframe", &[]).map(|_| ())
    }

    /// 放大视图（文字转草图后的结果刷新用）。
    pub fn zoom_in(&self) -> ComResult<()> {
        self.com().call("ZoomIn", &[]).map(|_| ())
    }

    /// 视图更新（截图前确保画面最新）。
    pub fn update(&self) -> ComResult<()> {
        self.com().call("Update", &[]).map(|_| ())
    }

    /// 截图（4 = catCaptureFormatPNG）。
    pub fn capture_to_file(&self, format: i32, path: &str) -> ComResult<()> {
        self.com()
            .call(
                "CaptureToFile",
                &[Arg::I32(format), Arg::Str(path.to_string())],
            )
            .map(|_| ())
    }

    /// 3D 视点（sight/up 方向与投影模式；批量分析截图用 ISO 视角）。
    pub fn viewpoint3d(&self) -> ComResult<Viewpoint3D> {
        Ok(Viewpoint3D::new(self.com().get("Viewpoint3D")?.as_obj()?))
    }
}

impl Viewpoint3D {
    /// 视线方向（ISO 视角用 (-1,-1,-1)）。
    pub fn put_sight_direction(&self, dir: [f64; 3]) -> ComResult<()> {
        self.com()
            .call(
                "PutSightDirection",
                &[Arg::SafeArray(vec![
                    Arg::F64(dir[0]),
                    Arg::F64(dir[1]),
                    Arg::F64(dir[2]),
                ])],
            )
            .map(|_| ())
    }

    /// 视图上方向（水平视角用 (0,0,1)）。
    pub fn put_up_direction(&self, dir: [f64; 3]) -> ComResult<()> {
        self.com()
            .call(
                "PutUpDirection",
                &[Arg::SafeArray(vec![
                    Arg::F64(dir[0]),
                    Arg::F64(dir[1]),
                    Arg::F64(dir[2]),
                ])],
            )
            .map(|_| ())
    }

    /// 投影模式（0 = 透视, 1 = 平行投影）。
    pub fn projection_mode(&self) -> ComResult<i32> {
        self.com().get("ProjectionMode")?.as_i32()
    }

    pub fn set_projection_mode(&self, value: i32) -> ComResult<()> {
        self.com().put("ProjectionMode", Arg::I32(value))
    }

    /// 视图缩放系数（capture 的 zoom_factor 用：Reframe 后乘系数微调）。
    pub fn zoom(&self) -> ComResult<f64> {
        self.com().get("Zoom")?.as_f64()
    }

    pub fn set_zoom(&self, value: f64) -> ComResult<()> {
        self.com().put("Zoom", Arg::F64(value))
    }
}

impl Viewer3D {
    prop_pair!(clipping_mode, set_clipping_mode, "ClippingMode", i32);
    prop_pair!(far_limit, set_far_limit, "FarLimit", f64);
    prop_pair!(near_limit, set_near_limit, "NearLimit", f64);
    prop_pair!(foggy, set_foggy, "Foggy", bool);
    prop_pair!(ground, set_ground, "Ground", bool);
    prop_pair!(
        lighting_intensity,
        set_lighting_intensity,
        "LightingIntensity",
        f64
    );
    prop_pair!(lighting_mode, set_lighting_mode, "LightingMode", i32);
    prop_pair!(
        navigation_style,
        set_navigation_style,
        "NavigationStyle",
        i32
    );
    prop_pair!(rendering_mode, set_rendering_mode, "RenderingMode", i32);

    /// 光源集合（`LightSources`）。
    pub fn light_sources(&self) -> ComResult<LightSources> {
        Ok(LightSources::new(self.com().get("LightSources")?.as_obj()?))
    }

    /// 绕轴旋转视图（`Rotate`；axis 为 3 分量，angle 为弧度）。
    pub fn rotate(&self, axis: [f64; 3], angle: f64) -> ComResult<()> {
        self.com()
            .call(
                "Rotate",
                &[
                    Arg::SafeArray(vec![
                        Arg::F64(axis[0]),
                        Arg::F64(axis[1]),
                        Arg::F64(axis[2]),
                    ]),
                    Arg::F64(angle),
                ],
            )
            .map(|_| ())
    }

    /// 平移视图（`Translate`；3 分量）。
    pub fn translate(&self, vector: [f64; 3]) -> ComResult<()> {
        self.com()
            .call(
                "Translate",
                &[Arg::SafeArray(vec![
                    Arg::F64(vector[0]),
                    Arg::F64(vector[1]),
                    Arg::F64(vector[2]),
                ])],
            )
            .map(|_| ())
    }
}

impl Viewpoint3D {
    prop_pair!(field_of_view, set_field_of_view, "FieldOfView", f64);
    prop_pair!(focus_distance, set_focus_distance, "FocusDistance", f64);

    /// 原点（`GetOrigin`，VBA 中转；3 分量）。
    pub fn get_origin(&self, ss: &SystemService) -> ComResult<Vec<f64>> {
        ss.numeric_out_param(self.com(), "GetOrigin")
    }

    /// 视线方向（`GetSightDirection`，VBA 中转；3 分量）。
    pub fn get_sight_direction(&self, ss: &SystemService) -> ComResult<Vec<f64>> {
        ss.numeric_out_param(self.com(), "GetSightDirection")
    }

    /// 上方向（`GetUpDirection`，VBA 中转；3 分量）。
    pub fn get_up_direction(&self, ss: &SystemService) -> ComResult<Vec<f64>> {
        ss.numeric_out_param(self.com(), "GetUpDirection")
    }

    /// 设置原点（`PutOrigin`；3 分量）。
    pub fn put_origin(&self, origin: [f64; 3]) -> ComResult<()> {
        self.com()
            .call(
                "PutOrigin",
                &[Arg::SafeArray(vec![
                    Arg::F64(origin[0]),
                    Arg::F64(origin[1]),
                    Arg::F64(origin[2]),
                ])],
            )
            .map(|_| ())
    }
}
