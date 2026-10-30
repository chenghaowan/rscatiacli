//! rscatiacli-tools CLI：不进 UI 调试工具。
//!
//! 用法：
//!   rscatiacli-tools json <script> <json>
//!   rscatiacli-tools colorize
//!
//! 例：
//!   rscatiacli-tools json body_rename/rename_bodies.py '{"dry_run":true}'
//!   rscatiacli-tools json hide_show/hide_show.py '{"action":"show-body","dry_run":true}'

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match args.first().map(String::as_str) {
        Some("json") => {
            let script = args.get(1).map(String::as_str).unwrap_or("");
            let params = args.get(2).map(String::as_str).unwrap_or("{}");
            if script.is_empty() {
                eprintln!(
                    "缺少脚本名，例如: json body_rename/rename_bodies.py '{{\"dry_run\":true}}'"
                );
                2
            } else {
                rscatiacli_tools::json_cli(script, params)
            }
        }
        Some("colorize") => rscatiacli_tools::colorize::run_cli(),
        Some("-h") | Some("--help") | None => {
            usage();
            0
        }
        Some(other) => {
            eprintln!("未知命令: `{other}`");
            usage();
            2
        }
    };
    std::process::exit(code);
}

fn usage() {
    eprintln!(
        "用法: rscatiacli-tools <命令>\n\
         \n\
         命令:\n\
           json <script> <json>  执行已迁移的 JSON 工具（阻塞，需 CATIA 已启动）\n\
           colorize              给当前零件所有 Body 着色\n\
           --help                显示本帮助\n\
         \n\
         内置脚本:\n\
           body_rename/rename_bodies.py\n\
           hide_show/hide_show.py"
    );
}
