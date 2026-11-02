//! 文档 / 会话 / 文件族（pycatia `in_interfaces` 阶段 4 补全）。
//!
//! 相机（`Cameras`/`Camera` 系）、文件系统（`FileSystem`/`File`/`Folder`/
//! `TextStream`）、定位（`Move`/`Position`）、引用集合（`References`）、
//! 打印（`PageSetup`/`DraftingPageSetup`/`Printer(s)`）、选择集 / 发送服务 /
//! 系统配置、查看器基族（`Viewer`/`Viewer2D`/`Viewers`/`SpecsViewer`/
//! `SpecsAndGeomWindow`/`Windows`）与 `Viewpoint2D`。
//!
//! 已在其它模块的：`Application`/`Documents`/`Document`/`Reference`/
//! `Selection`/`Window`/`Viewer3D`/`Viewpoint3D`。
//! 个别出参数组取列表的成员（`GetListOfSelectionSet` 等）未封装，
//! 走 `com()` 逃生舱。

use crate::collections::ComCollection;
use crate::dispatch::{Arg, ComObj, ComObject};
use crate::error::ComResult;

use super::reference::Reference;
use super::system_service::SystemService;
use super::viewer::Window;

com_wrapper!(Viewpoint2D);
com_wrapper!(Camera);
com_wrapper!(Camera2D);
com_wrapper!(Camera3D);
com_wrapper!(Cameras);
com_wrapper!(FileComponent);
com_wrapper!(File);
com_wrapper!(Files);
com_wrapper!(Folder);
com_wrapper!(Folders);
com_wrapper!(FileSystem);
com_wrapper!(TextStream);
com_wrapper!(Move);
com_wrapper!(Position);
com_wrapper!(References);
com_wrapper!(PageSetup);
com_wrapper!(DraftingPageSetup);
com_wrapper!(Printer);
com_wrapper!(Printers);
com_wrapper!(SelectionSets);
com_wrapper!(SendToService);
com_wrapper!(SystemConfiguration);
com_wrapper!(Viewer);
com_wrapper!(Viewer2D);
com_wrapper!(Viewers);
com_wrapper!(SpecsViewer);
com_wrapper!(SpecsAndGeomWindow);
com_wrapper!(Windows);
com_wrapper!(LightSource);
com_wrapper!(LightSources);

impl ComCollection for Cameras {
    type Item = Camera;
}

impl ComCollection for Files {
    type Item = File;
}

impl ComCollection for Folders {
    type Item = Folder;
}

impl ComCollection for References {
    type Item = Reference;
}

impl ComCollection for Printers {
    type Item = Printer;
}

impl ComCollection for Viewers {
    type Item = Viewer;
}

impl ComCollection for Windows {
    type Item = Window;
}

impl ComCollection for LightSources {
    type Item = LightSource;
}

// ── 视点 / 相机 ─────────────────────────────────────────────────────

impl Viewpoint2D {
    prop_pair!(zoom, set_zoom, "Zoom", f64);

    /// 原点（`GetOrigin`，VBA 中转；2 分量 [x, y]）。
    pub fn get_origin(&self, ss: &SystemService) -> ComResult<Vec<f64>> {
        ss.numeric_out_param(self.com(), "GetOrigin")
    }

    /// 设置原点（`PutOrigin`；2 分量）。
    pub fn put_origin(&self, origin: [f64; 2]) -> ComResult<()> {
        self.com()
            .call(
                "PutOrigin",
                &[Arg::SafeArray(vec![
                    Arg::F64(origin[0]),
                    Arg::F64(origin[1]),
                ])],
            )
            .map(|_| ())
    }
}

impl Camera {
    /// 相机类型（`Type`，CatCameraType；Rust 保留字故方法名带 camera 前缀）。
    pub fn camera_type(&self) -> ComResult<i32> {
        self.com().get("Type")?.as_i32()
    }
}

impl Camera2D {
    prop_pair!(viewpoint2d, set_viewpoint2d, "Viewpoint2D", Viewpoint2D);
}

impl Camera3D {
    prop_pair!(viewpoint3d, set_viewpoint3d, "Viewpoint3D", Viewpoint3D);
}

impl Cameras {
    /// 按序号删除相机（`Remove`）。
    pub fn remove(&self, index: i32) -> ComResult<()> {
        self.com().call("Remove", &[Arg::I32(index)]).map(|_| ())
    }
}

// ── 文件系统 ────────────────────────────────────────────────────────

impl FileComponent {
    prop_pair!(parent_folder, set_parent_folder, "ParentFolder", Folder);
    prop_get!(path, "Path", str);
}

impl File {
    /// 完整路径（继承自 `FileComponent.Path`）。
    pub fn path(&self) -> ComResult<String> {
        self.com().get("Path")?.as_string()
    }

    /// 大小（字节）。
    pub fn size(&self) -> ComResult<f64> {
        self.com().get("Size")?.as_f64()
    }

    /// 类型（`Type`；Rust 保留字故方法名带 file 前缀）。
    pub fn file_type(&self) -> ComResult<String> {
        self.com().get("Type")?.as_string()
    }

    /// 按模式打开文本流（`OpenAsTextStream`；mode: "r"/"w"/"a"，同 VBA 口径）。
    pub fn open_as_text_stream(&self, mode: &str) -> ComResult<TextStream> {
        Ok(TextStream::new(
            self.com()
                .call("OpenAsTextStream", &[Arg::Str(mode.to_string())])?
                .as_obj()?,
        ))
    }
}

impl Folder {
    /// 完整路径（继承自 `FileComponent.Path`）。
    pub fn path(&self) -> ComResult<String> {
        self.com().get("Path")?.as_string()
    }

    /// 内含文件集合（`Files`）。
    pub fn files(&self) -> ComResult<Files> {
        Ok(Files::new(self.com().get("Files")?.as_obj()?))
    }

    /// 子文件夹集合（`SubFolders`）。
    pub fn sub_folders(&self) -> ComResult<Folders> {
        Ok(Folders::new(self.com().get("SubFolders")?.as_obj()?))
    }
}

impl FileSystem {
    prop_get!(file_separator, "FileSeparator", str);
    prop_get!(path_separator, "PathSeparator", str);

    /// 临时目录（`TemporaryDirectory`；真机实测返回 Folder 对象而非字符串，
    /// 与 pycatia 标注不同——读路径用 `.path()`）。
    pub fn temporary_directory(&self) -> ComResult<Folder> {
        Ok(Folder::new(self.com().get("TemporaryDirectory")?.as_obj()?))
    }

    /// 拼接路径（`ConcatenatePaths`）。
    pub fn concatenate_paths(&self, chunk1: &str, chunk2: &str) -> ComResult<String> {
        self.com()
            .call(
                "ConcatenatePaths",
                &[Arg::Str(chunk1.to_string()), Arg::Str(chunk2.to_string())],
            )?
            .as_string()
    }

    /// 复制文件（`CopyFile`；overwrite 控制是否覆盖目标）。
    pub fn copy_file(&self, source: &str, destination: &str, overwrite: bool) -> ComResult<()> {
        self.com()
            .call(
                "CopyFile",
                &[
                    Arg::Str(source.to_string()),
                    Arg::Str(destination.to_string()),
                    Arg::Bool(overwrite),
                ],
            )
            .map(|_| ())
    }

    /// 复制文件夹（`CopyFolder`）。
    pub fn copy_folder(&self, source: &str, destination: &str) -> ComResult<()> {
        self.com()
            .call(
                "CopyFolder",
                &[
                    Arg::Str(source.to_string()),
                    Arg::Str(destination.to_string()),
                ],
            )
            .map(|_| ())
    }

    /// 新建文件（`CreateFile`；overwrite 控制是否覆盖已有文件）。
    pub fn create_file(&self, path: &str, overwrite: bool) -> ComResult<File> {
        Ok(File::new(
            self.com()
                .call(
                    "CreateFile",
                    &[Arg::Str(path.to_string()), Arg::Bool(overwrite)],
                )?
                .as_obj()?,
        ))
    }

    /// 新建文件夹（`CreateFolder`）。
    pub fn create_folder(&self, path: &str) -> ComResult<Folder> {
        Ok(Folder::new(
            self.com()
                .call("CreateFolder", &[Arg::Str(path.to_string())])?
                .as_obj()?,
        ))
    }

    /// 删除文件（`DeleteFile`）。
    pub fn delete_file(&self, path: &str) -> ComResult<()> {
        self.com()
            .call("DeleteFile", &[Arg::Str(path.to_string())])
            .map(|_| ())
    }

    /// 删除文件夹（`DeleteFolder`）。
    pub fn delete_folder(&self, path: &str) -> ComResult<()> {
        self.com()
            .call("DeleteFolder", &[Arg::Str(path.to_string())])
            .map(|_| ())
    }

    /// 文件是否存在（`FileExists`）。
    pub fn file_exists(&self, path: &str) -> ComResult<bool> {
        self.com()
            .call("FileExists", &[Arg::Str(path.to_string())])?
            .as_bool()
    }

    /// 文件夹是否存在（`FolderExists`）。
    pub fn folder_exists(&self, path: &str) -> ComResult<bool> {
        self.com()
            .call("FolderExists", &[Arg::Str(path.to_string())])?
            .as_bool()
    }

    /// 取文件对象（`GetFile`）。
    pub fn get_file(&self, path: &str) -> ComResult<File> {
        Ok(File::new(
            self.com()
                .call("GetFile", &[Arg::Str(path.to_string())])?
                .as_obj()?,
        ))
    }

    /// 取文件夹对象（`GetFolder`）。
    pub fn get_folder(&self, path: &str) -> ComResult<Folder> {
        Ok(Folder::new(
            self.com()
                .call("GetFolder", &[Arg::Str(path.to_string())])?
                .as_obj()?,
        ))
    }
}

impl TextStream {
    /// 是否行尾（`AtEndOfLine`）。
    pub fn at_end_of_line(&self) -> ComResult<bool> {
        self.com().get("AtEndOfLine")?.as_bool()
    }

    /// 是否流尾（`AtEndOfStream`）。
    pub fn at_end_of_stream(&self) -> ComResult<bool> {
        self.com().get("AtEndOfStream")?.as_bool()
    }

    /// 关闭流（`Close`）。
    pub fn close(&self) -> ComResult<()> {
        self.com().call("Close", &[]).map(|_| ())
    }

    /// 读 N 个字符（`Read`）。
    pub fn read(&self, num_of_char: i32) -> ComResult<String> {
        self.com()
            .call("Read", &[Arg::I32(num_of_char)])?
            .as_string()
    }

    /// 读一行（`ReadLine`）。
    pub fn read_line(&self) -> ComResult<String> {
        self.com().call("ReadLine", &[])?.as_string()
    }

    /// 写字符串（`Write`）。
    pub fn write(&self, text: &str) -> ComResult<()> {
        self.com()
            .call("Write", &[Arg::Str(text.to_string())])
            .map(|_| ())
    }
}

// ── 定位 ────────────────────────────────────────────────────────────

impl Move {
    /// 被移动对象（`MovableObject`，原始对象）。
    pub fn movable_object(&self) -> ComResult<ComObj> {
        self.com().get("MovableObject")?.as_obj()
    }

    /// 应用变换矩阵（`Apply`；CAA 口径 12 分量：3x3 旋转 + 平移列）。
    pub fn apply(&self, transformation: &[f64]) -> ComResult<()> {
        let args = Arg::SafeArray(transformation.iter().map(|v| Arg::F64(*v)).collect());
        self.com().call("Apply", &[args]).map(|_| ())
    }
}

impl Position {
    /// 定位分量（`GetComponents`，VBA 中转；CAA 口径 12 分量）。
    pub fn get_components(&self, ss: &SystemService) -> ComResult<Vec<f64>> {
        ss.numeric_out_param(self.com(), "GetComponents")
    }

    /// 设置定位分量（`SetComponents`）。
    pub fn set_components(&self, components: &[f64]) -> ComResult<()> {
        let args = Arg::SafeArray(components.iter().map(|v| Arg::F64(*v)).collect());
        self.com().call("SetComponents", &[args]).map(|_| ())
    }
}

// ── 打印 ────────────────────────────────────────────────────────────

impl PageSetup {
    prop_pair!(banner, set_banner, "Banner", str);
    prop_pair!(banner_position, set_banner_position, "BannerPosition", i32);
    prop_pair!(banner_size, set_banner_size, "BannerSize", f64);
    prop_pair!(bottom, set_bottom, "Bottom", f64);
    prop_pair!(bottom_margin, set_bottom_margin, "BottomMargin", f64);
    prop_pair!(color, set_color, "Color", i32);
    prop_pair!(dpi, set_dpi, "DPI", f64);
    prop_pair!(gamma, set_gamma, "Gamma", f64);
    prop_pair!(left, set_left, "Left", f64);
    prop_pair!(left_margin, set_left_margin, "LeftMargin", f64);
    prop_pair!(line_cap, set_line_cap, "LineCap", i32);
    prop_pair!(orientation, set_orientation, "Orientation", i32);
    prop_pair!(paper_height, set_paper_height, "PaperHeight", f64);
    prop_pair!(paper_size, set_paper_size, "PaperSize", i32);
    prop_pair!(paper_width, set_paper_width, "PaperWidth", f64);
    prop_pair!(quality, set_quality, "Quality", i32);
    prop_pair!(right_margin, set_right_margin, "RightMargin", f64);
    prop_pair!(rotation, set_rotation, "Rotation", i32);
    prop_pair!(scaling1_to1, set_scaling1_to1, "Scaling1To1", bool);
    prop_pair!(top_margin, set_top_margin, "TopMargin", f64);
    prop_pair!(
        white_vectors_in_black,
        set_white_vectors_in_black,
        "WhiteVectorsInBlack",
        bool
    );
    prop_pair!(zoom, set_zoom, "Zoom", f64);
}

impl DraftingPageSetup {
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

impl Printer {
    prop_get!(device_name, "DeviceName", str);
    prop_get!(orientation, "Orientation", i32);
    prop_get!(paper_height, "PaperHeight", f64);
    prop_get!(paper_size, "PaperSize", i32);
    prop_get!(paper_width, "PaperWidth", f64);
}

// ── 选择集 / 发送 / 系统配置 ────────────────────────────────────────

impl SelectionSets {
    /// 新建选择集（`CreateSelectionSet`）。
    pub fn create_selection_set(&self, name: &str) -> ComResult<()> {
        self.com()
            .call("CreateSelectionSet", &[Arg::Str(name.to_string())])
            .map(|_| ())
    }

    /// 删除选择集（`DeleteSelectionSet`）。
    pub fn delete_selection_set(&self, name: &str) -> ComResult<()> {
        self.com()
            .call("DeleteSelectionSet", &[Arg::Str(name.to_string())])
            .map(|_| ())
    }

    /// 把当前选择集（CSO）存入指定选择集（`AddCSOIntoSelectionSet`）。
    pub fn add_cso_into_selection_set(&self, name: &str) -> ComResult<()> {
        self.com()
            .call("AddCSOIntoSelectionSet", &[Arg::Str(name.to_string())])
            .map(|_| ())
    }

    /// 把指定选择集载入 CSO（`PutSelectionSetIntoCSO`）。
    pub fn put_selection_set_into_cso(&self, name: &str) -> ComResult<()> {
        self.com()
            .call("PutSelectionSetIntoCSO", &[Arg::Str(name.to_string())])
            .map(|_| ())
    }

    /// 重命名选择集（`RenameSelectionSet`）。
    pub fn rename_selection_set(&self, old_name: &str, new_name: &str) -> ComResult<()> {
        self.com()
            .call(
                "RenameSelectionSet",
                &[
                    Arg::Str(old_name.to_string()),
                    Arg::Str(new_name.to_string()),
                ],
            )
            .map(|_| ())
    }
    // `GetListOfSelectionSet` 出参为字符串数组，未封装——走 com() 逃生舱。
}

impl SendToService {
    /// 追加待发送文件（`AddFile`）。
    pub fn add_file(&self, path: &str) -> ComResult<()> {
        self.com()
            .call("AddFile", &[Arg::Str(path.to_string())])
            .map(|_| ())
    }

    /// 移除待发送文件（`RemoveFile`）。
    pub fn remove_file(&self, file: &str) -> ComResult<()> {
        self.com()
            .call("RemoveFile", &[Arg::Str(file.to_string())])
            .map(|_| ())
    }

    /// 是否保留目录结构（`KeepDirectory`）。
    pub fn keep_directory(&self, keep: bool) -> ComResult<()> {
        self.com()
            .call("KeepDirectory", &[Arg::Bool(keep)])
            .map(|_| ())
    }

    /// 执行发送（`Run`）。
    pub fn run(&self) -> ComResult<()> {
        self.com().call("Run", &[]).map(|_| ())
    }

    /// 设多文件目标目录（`SetDirectoryFile`）。
    pub fn set_directory_file(&self, directory: &str) -> ComResult<()> {
        self.com()
            .call("SetDirectoryFile", &[Arg::Str(directory.to_string())])
            .map(|_| ())
    }

    /// 设单文件目标目录（`SetDirectoryOneFile`）。
    pub fn set_directory_one_file(&self, file: &str, directory: &str) -> ComResult<()> {
        self.com()
            .call(
                "SetDirectoryOneFile",
                &[Arg::Str(file.to_string()), Arg::Str(directory.to_string())],
            )
            .map(|_| ())
    }

    /// 设初始文件（`SetInitialFile`）。
    pub fn set_initial_file(&self, path: &str) -> ComResult<()> {
        self.com()
            .call("SetInitialFile", &[Arg::Str(path.to_string())])
            .map(|_| ())
    }

    /// 重命名待发送文件（`SetRenameFile`）。
    pub fn set_rename_file(&self, old_name: &str, new_name: &str) -> ComResult<()> {
        self.com()
            .call(
                "SetRenameFile",
                &[
                    Arg::Str(old_name.to_string()),
                    Arg::Str(new_name.to_string()),
                ],
            )
            .map(|_| ())
    }
    // `GetListOfDependantFile` / `GetListOfToBeCopiedFiles` /
    // `GetLastSendToMethodError` 出参未封装——走 com() 逃生舱。
}

impl SystemConfiguration {
    prop_get!(operating_system, "OperatingSystem", str);
    prop_get!(service_pack, "ServicePack", str);

    /// 版本年份（`Release`；真机实测返回整数 VT_I4，与 pycatia 字符串标注不同）。
    pub fn release(&self) -> ComResult<i32> {
        self.com().get("Release")?.as_i32()
    }

    /// 服务包级别（`Version`；真机实测返回整数 VT_I4）。
    pub fn version(&self) -> ComResult<i32> {
        self.com().get("Version")?.as_i32()
    }
    prop_get!(product_count, "ProductCount", i32);

    /// 许可是否授权（`IsProductAuthorized`）。
    pub fn is_product_authorized(&self, product_name: &str) -> ComResult<bool> {
        self.com()
            .call("IsProductAuthorized", &[Arg::Str(product_name.to_string())])?
            .as_bool()
    }
    // `GetProductNames` 出参为字符串数组，未封装——走 com() 逃生舱。
}

// ── 查看器基族 ──────────────────────────────────────────────────────

impl Viewer {
    prop_pair!(full_screen, set_full_screen, "FullScreen", bool);
    prop_get!(height, "Height", i32);
    prop_get!(width, "Width", i32);

    /// 激活查看器（`Activate`）。
    pub fn activate(&self) -> ComResult<()> {
        self.com().call("Activate", &[]).map(|_| ())
    }

    /// 截图到文件（`CaptureToFile`；format 为 CatCaptureFormat）。
    pub fn capture_to_file(&self, format: i32, file: &str) -> ComResult<()> {
        self.com()
            .call(
                "CaptureToFile",
                &[Arg::I32(format), Arg::Str(file.to_string())],
            )
            .map(|_| ())
    }

    /// 背景色（`GetBackgroundColor`，VBA 中转；3 分量 0..1 RGB）。
    pub fn get_background_color(&self, ss: &SystemService) -> ComResult<Vec<f64>> {
        ss.numeric_out_param(self.com(), "GetBackgroundColor")
    }

    /// 背景色（0..1 的 RGB）。
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

    /// 新建相机（`NewCamera`）。
    pub fn new_camera(&self) -> ComResult<Camera> {
        Ok(Camera::new(self.com().call("NewCamera", &[])?.as_obj()?))
    }

    /// 视图重画（`Reframe`）。
    pub fn reframe(&self) -> ComResult<()> {
        self.com().call("Reframe", &[]).map(|_| ())
    }

    /// 视图刷新（`Update`）。
    pub fn update(&self) -> ComResult<()> {
        self.com().call("Update", &[]).map(|_| ())
    }

    /// 放大（`ZoomIn`）。
    pub fn zoom_in(&self) -> ComResult<()> {
        self.com().call("ZoomIn", &[]).map(|_| ())
    }

    /// 缩小（`ZoomOut`）。
    pub fn zoom_out(&self) -> ComResult<()> {
        self.com().call("ZoomOut", &[]).map(|_| ())
    }
}

impl Viewer2D {
    prop_pair!(viewpoint2d, set_viewpoint2d, "Viewpoint2D", Viewpoint2D);
}

impl SpecsViewer {
    prop_pair!(layout, set_layout, "Layout", i32);
}

impl SpecsAndGeomWindow {
    prop_pair!(layout, set_layout, "Layout", i32);

    /// 规格树查看器（`SpecsViewer`）。
    pub fn specs_viewer(&self) -> ComResult<SpecsViewer> {
        Ok(SpecsViewer::new(self.com().get("SpecsViewer")?.as_obj()?))
    }
}

impl Windows {
    /// 排列窗口（`Arrange`；style 为 CatArrangeStyle）。
    pub fn arrange(&self, style: i32) -> ComResult<()> {
        self.com().call("Arrange", &[Arg::I32(style)]).map(|_| ())
    }
}
