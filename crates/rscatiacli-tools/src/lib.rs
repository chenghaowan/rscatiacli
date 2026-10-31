//! 工具业务层：脚本注册表 + 各工具的 CATIA 操作实现。
//!
//! 这里只放“做什么”（业务规则、参数解析、结果 JSON），COM 对象模型访问全部来自
//! `rscatiacli`。新工具迁移时在 `json_tool` 注册即可，ltl-manage 无需改注册表。

pub mod add_material;
pub mod apply_naming;
pub mod body_rename;
pub mod body_to_part;
pub mod colorize;
pub mod dump_solid;
pub mod find_near_parts;
pub mod hide_show;
pub mod join_surface;
pub mod link_fixer;
pub mod naming_rules;
pub mod offset_surface;
pub mod save_product_without_history;
pub mod stitch_surface;
pub mod surface_common;
pub mod trim_boundary;
pub mod vba_bbox_generate;

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
        "link_fixer/catia_link_fixer_bridge.py" => Some(link_fixer::run),
        "body_to_part/catia_body_to_part_bridge.py" => Some(body_to_part::run),
        "add_material/add_material.py" => Some(add_material::run),
        // 曲面四件套
        "join_surface/join_surface.py" => Some(join_surface::run),
        "offset_surface/offset_surface.py" => Some(offset_surface::run),
        "stitch_surface/stitch_surface.py" => Some(stitch_surface::run),
        "trim_boundary/trim_boundary.py" => Some(trim_boundary::run),
        // product_analysis 一批
        "product_analysis/apply_naming.py" => Some(apply_naming::run),
        "product_analysis/find_near_parts.py" => Some(find_near_parts::run),
        "product_analysis/save_product_without_history.py" => {
            Some(save_product_without_history::run)
        }
        "dump_solid/dump_solid.py" => Some(dump_solid::run),
        "product_analysis/vba_bbox_generate.py" => Some(vba_bbox_generate::run),
        _ => None,
    }
}

/// 按脚本分配执行时限：body_to_part 逐 Body SaveAs + 建装配，link_fixer
/// 的兜底重开全量文档、add_material 反复开关材料目录都远超默认 120s；
/// 曲面四件套逐面偏置/缝合 + 反复 Update，同样放宽；未列出的脚本维持默认。
fn json_tool_timeout(script: &str) -> Duration {
    match script {
        "link_fixer/catia_link_fixer_bridge.py" => Duration::from_secs(900),
        "body_to_part/catia_body_to_part_bridge.py" => Duration::from_secs(3600),
        "add_material/add_material.py" => Duration::from_secs(300),
        "join_surface/join_surface.py" => Duration::from_secs(300),
        "offset_surface/offset_surface.py" => Duration::from_secs(300),
        "stitch_surface/stitch_surface.py" => Duration::from_secs(600),
        "trim_boundary/trim_boundary.py" => Duration::from_secs(600),
        "product_analysis/apply_naming.py" => Duration::from_secs(900),
        "product_analysis/find_near_parts.py" => Duration::from_secs(600),
        "product_analysis/save_product_without_history.py" => Duration::from_secs(1200),
        "dump_solid/dump_solid.py" => Duration::from_secs(600),
        "product_analysis/vba_bbox_generate.py" => Duration::from_secs(900),
        _ => Duration::from_secs(120),
    }
}

/// 该脚本是否已由 Rust 实现（未迁移时调用方应走原 Python 路径）。
pub fn is_rust_json_script(script: &str) -> bool {
    json_tool(script).is_some()
}

/// 执行 JSON 工具。**阻塞调用**，内部投递到 STA 线程。
pub fn run_json_script(script: &str, params: &str) -> JsonScriptOutcome {
    run_json_script_with_timeout(script, params, json_tool_timeout(script))
}

/// 执行 JSON 工具，显式指定执行时限。**阻塞调用**，内部投递到 STA 线程。
pub fn run_json_script_with_timeout(
    script: &str,
    params: &str,
    timeout: Duration,
) -> JsonScriptOutcome {
    let Some(tool) = json_tool(script) else {
        return JsonScriptOutcome {
            result: Err(ComError::runtime(format!("脚本未迁移到 Rust: {script}"))),
            logs: Vec::new(),
        };
    };

    // 参数解析失败按空对象处理（Python 版会返回参数错误，这里由各工具自行兜底）
    let params: Value = serde_json::from_str(params).unwrap_or(Value::Null);

    match run_on_sta(timeout, move || {
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
