//! 工具业务层：脚本注册表 + 各工具的 CATIA 操作实现。
//!
//! 这里只放“做什么”（业务规则、参数解析、结果 JSON），COM 对象模型访问全部来自
//! `rscatiacli`。新工具迁移时在 [`json_tool`] 注册即可，ltl-manage 无需改注册表。

pub mod body_rename;
pub mod colorize;
pub mod hide_show;

use std::cell::RefCell;
use std::time::Duration;

use serde_json::Value;

use rscatiacli::{run_on_sta, ComError, ComResult};

/// JSON 工具执行结果：即使失败也保留日志（与 Python 版返回一致）。
pub struct JsonScriptOutcome {
    pub result: ComResult<Value>,
    pub logs: Vec<String>,
}

type JsonToolFn = fn(&Value, &dyn Fn(&str)) -> ComResult<Value>;

/// 已迁移到 Rust 的 JSON 工具注册表：脚本名 → 实现。
fn json_tool(script: &str) -> Option<JsonToolFn> {
    match script {
        "body_rename/rename_bodies.py" => Some(body_rename::run),
        "hide_show/hide_show.py" => Some(hide_show::run),
        _ => None,
    }
}

/// 该脚本是否已由 Rust 实现（未迁移时调用方应走原 Python 路径）。
pub fn is_rust_json_script(script: &str) -> bool {
    json_tool(script).is_some()
}

/// 执行 JSON 工具。**阻塞调用**，内部投递到 STA 线程。
pub fn run_json_script(script: &str, params: &str) -> JsonScriptOutcome {
    let Some(tool) = json_tool(script) else {
        return JsonScriptOutcome {
            result: Err(ComError::runtime(format!("脚本未迁移到 Rust: {script}"))),
            logs: Vec::new(),
        };
    };

    // 参数解析失败按空对象处理（Python 版会返回参数错误，这里由各工具自行兜底）
    let params: Value = serde_json::from_str(params).unwrap_or(Value::Null);

    match run_on_sta(Duration::from_secs(120), move || {
        let logs = RefCell::new(Vec::<String>::new());
        let result = tool(&params, &|msg| logs.borrow_mut().push(msg.to_string()));
        (result, logs.into_inner())
    }) {
        Ok((result, logs)) => JsonScriptOutcome { result, logs },
        Err(e) => JsonScriptOutcome {
            result: Err(e),
            logs: Vec::new(),
        },
    }
}

/// CLI 入口：`--catia-poc json <script> <json>` / `rscatiacli-tools json ...`。
pub fn json_cli(script: &str, params: &str) -> i32 {
    let outcome = run_json_script(script, params);
    for line in &outcome.logs {
        println!("{line}");
    }
    match outcome.result {
        Ok(v) => {
            println!("{v}");
            if v.get("success").and_then(|x| x.as_bool()).unwrap_or(false) {
                0
            } else {
                1
            }
        }
        Err(e) => {
            eprintln!("[catia-poc] 失败: {e}");
            1
        }
    }
}
