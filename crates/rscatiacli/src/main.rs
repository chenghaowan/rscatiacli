//! rscatiacli 诊断 CLI：status / probe / put-selftest / ps。
//!
//! 工具业务命令在 `rscatiacli-tools` 里。

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match args.first().map(String::as_str) {
        Some("status") => {
            let value = rscatiacli::status();
            match serde_json::to_string_pretty(&value) {
                Ok(text) => println!("{text}"),
                Err(_) => println!("{value}"),
            }
            0
        }
        Some("probe") => print_result("连接 CATIA (只读)", rscatiacli::probe),
        Some("put-selftest") => print_result("属性写入自检", rscatiacli::put_selftest),
        Some("ps") => {
            let (total, hits) = rscatiacli::process_names_containing("cnext");
            println!("sysinfo 进程总数: {total}");
            println!("含 cnext 的进程名: {hits:?}");
            println!("cnext_count() = {}", rscatiacli::cnext_count());
            0
        }
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

fn print_result(label: &str, f: fn() -> rscatiacli::ComResult<String>) -> i32 {
    println!("[rscatiacli] {label} ...");
    match f() {
        Ok(text) => {
            println!("{text}");
            0
        }
        Err(e) => {
            eprintln!("[rscatiacli] 失败: {e}");
            1
        }
    }
}

fn usage() {
    eprintln!(
        "用法: rscatiacli <命令>\n\
         \n\
         命令:\n\
           status        输出连接状态 JSON（无需 CATIA 运行）\n\
           probe         连接 CATIA 并打印活动文档信息（只读）\n\
           put-selftest  把每个 Body 的名字原样写回，验证属性写入通道\n\
           ps            进程诊断（CNEXT 计数）\n\
           --help        显示本帮助"
    );
}
