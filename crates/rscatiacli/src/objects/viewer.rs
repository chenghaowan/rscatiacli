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

impl Window {
    /// 当前活动查看器。
    pub fn active_viewer(&self) -> ComResult<Viewer3D> {
        Ok(Viewer3D::new(self.com().get("ActiveViewer")?.as_obj()?))
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

    /// 截图（4 = catCaptureFormatPNG）。
    pub fn capture_to_file(&self, format: i32, path: &str) -> ComResult<()> {
        self.com()
            .call(
                "CaptureToFile",
                &[Arg::I32(format), Arg::Str(path.to_string())],
            )
            .map(|_| ())
    }
}
