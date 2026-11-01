//! colorize-bodies：给当前零件的每个 Body 设不同颜色。
//!
//! 对应 ltl-manage resources/tools/colorize_bodies.py。差异说明：
//! - Python 版活动文档不是 .CATPart 时会调 `ensure_part_doc()` 自动查找并打开同名零件；
//!   本期未移植该兜底，直接给出明确错误（下一批工具再补）。
//! - 连接纪律完全一致：ROT attach、多实例守卫、空会话自动退出（见 rscatiacli::attach_with_guard）。

use std::time::Duration;

use rscatiacli::{
    attach_with_guard, run_on_sta, ComCollection, ComError, ComResult, Progress,
};

/// 经典 16 色调色板（已避开红色系）。与 Python 版 COLORS 完全一致。
const COLORS: [(i32, i32, i32); 16] = [
    (0, 255, 0),
    (0, 0, 255),
    (255, 255, 0),
    (255, 0, 255),
    (0, 255, 255),
    (255, 128, 0),
    (128, 0, 255),
    (0, 128, 0),
    (0, 0, 128),
    (128, 128, 0),
    (128, 0, 128),
    (0, 128, 128),
    (128, 128, 128),
    (192, 192, 192),
    (0, 255, 128),
    (128, 255, 0),
];

/// 超时上限：超时后判定 CATIA 无响应，丢弃当前 STA 线程。
const RUN_TIMEOUT: Duration = Duration::from_secs(120);

/// 对外入口：投递到 STA 线程执行。
pub fn run_in_worker<F>(progress: F) -> ComResult<String>
where
    F: Fn(&str) + Send + 'static,
{
    run_on_sta(RUN_TIMEOUT, move || run(&progress))?
}

/// 工具主体。**必须在 STA 线程上执行。**
pub fn run(progress: Progress<'_>) -> ComResult<String> {
    let app = attach_with_guard(progress)?;

    let doc_count = app.document_count()?;
    if doc_count == 0 {
        return Err(ComError::runtime(
            "CATIA 中没有打开的文档: 请先打开一个 .CATPart 再使用本工具",
        ));
    }

    let doc = app.active_document()?;

    // Part 属性只存在于 PartDocument；工程图/装配取不到。
    let part = doc.part_document().part().map_err(|_| {
        ComError::runtime("当前活动文档不是零件(.CATPart): 请把要着色的零件设为活动文档后重试")
    })?;

    let bodies = part.bodies()?;
    let count = bodies.count()?;
    if count == 0 {
        return Err(ComError::runtime("当前零件没有 body"));
    }

    let sel = doc.selection()?;
    let vis = sel.vis_properties()?;

    for i in 1..=count {
        let body = bodies.item(i)?;
        let name = body.name().unwrap_or_else(|_| "?".to_string());

        let (r, g, b) = COLORS[(i as usize - 1) % COLORS.len()];

        sel.clear()?;
        sel.add(&body)?;
        // 第 4 个参数 0 = 不继承父级颜色，对象自身使用该颜色
        vis.set_real_color(r, g, b, 0)?;

        progress(&format!("  body[{i}] {name} -> RGB({r},{g},{b})"));
    }

    sel.clear()?;

    Ok(format!("已为 {count} 个 body 着色。请查看 CATIA 视图。"))
}

/// CLI 入口：`ltl-manage-tauri.exe --catia-poc colorize` / `rscatiacli-tools colorize`。
///
/// 注意：release 构建带 `windows_subsystem = "windows"`，没有控制台，
/// 请用 `cargo run -- --catia-poc colorize`（debug）验证。
pub fn run_cli() -> i32 {
    println!("[catia-poc] 连接 CATIA ...");
    match run_in_worker(|msg| println!("{msg}")) {
        Ok(message) => {
            println!("{message}");
            0
        }
        Err(e) => {
            eprintln!("[catia-poc] 失败: {e}");
            1
        }
    }
}
