//! `Window` / `Viewer3D`（3D 查看器截图通道）。
//!
//! 对应 rule_bounding_box / capture_active_part 的截图流程：
//! `Application.ActiveWindow` → `Window.ActiveViewer` → 隐藏规格树 /
//! 白底 / Reframe / `CaptureToFile(4=PNG, 路径)`。图像裁剪缩放量化在工具层
//! （image + color_quant + png crate，见 rscatiacli-tools image_post.rs）。

use crate::dispatch::{Arg, ComObject};
use crate::error::ComResult;

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
