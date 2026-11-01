//! 工具业务层：脚本注册表 + 各工具的 CATIA 操作实现。
//!
//! 这里只放“做什么”（业务规则、参数解析、结果 JSON），COM 对象模型访问全部来自
//! `rscatiacli`。新工具迁移时在 `json_tool` 注册即可，ltl-manage 无需改注册表。

pub mod add_material;
pub mod apply_naming;
pub mod batch_bbox;
pub mod body_rename;
pub mod body_to_part;
pub mod colorize;
pub mod doc_link;
pub mod dump_solid;
pub mod find_near_parts;
pub mod hide_show;
pub mod honeycomb_rib;
pub mod ipp;
pub mod image_post;
pub mod join_surface;
pub mod link_fixer;
pub mod measure_capture;
pub mod measure_harness;
pub mod naming_rules;
pub mod offset_surface;
pub mod product_measure;
pub mod qr_outline;
pub mod rule_axis;
pub mod rule_bounding_box;
pub mod rule_projected_surface;
pub mod save_product_without_history;
pub mod batch_analysis;
pub mod stitch_surface;
pub mod surface_common;
pub mod text_outline;
pub mod text_to_sketch;
pub mod trim_boundary;
pub mod wcc;
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
        // 批量产品分析: 遍历产品树逐件 bbox/投影/截图, 部件数大时很耗时
        "batch_product_analysis/batch_product_analysis.py" => Some(batch_analysis::run),
        "honeycomb_rib/honeycomb_rib.py" => Some(honeycomb_rib::run),
        "wcc/wcc_cli.py" => Some(wcc::run),
        "ipp/ipp_cli.py" => Some(ipp::run),
        "trim_boundary/trim_boundary.py" => Some(trim_boundary::run),
        // product_analysis 一批
        "product_analysis/apply_naming.py" => Some(apply_naming::run),
        "product_analysis/find_near_parts.py" => Some(find_near_parts::run),
        "product_analysis/save_product_without_history.py" => {
            Some(save_product_without_history::run)
        }
        "dump_solid/dump_solid.py" => Some(dump_solid::run),
        "product_analysis/vba_bbox_generate.py" => Some(vba_bbox_generate::run),
        // 测量三件套（rule_projected_surface 依赖 rule_axis 的轴系建立）
        "product_analysis/rule_projected_surface.py" => Some(rule_projected_surface::run),
        "product_analysis/rule_bounding_box.py" => Some(rule_bounding_box::run),
        "text_to_sketch/text_to_sketch.py" => Some(text_to_sketch::run),
        "measure_harness/measure_harness.py" => Some(measure_harness::run),
        "product_analysis/batch_bbox.py" => Some(batch_bbox::run),
        "product_analysis/product_measure.py" => Some(product_measure::run),
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
        "batch_product_analysis/batch_product_analysis.py" => Duration::from_secs(3600),
        "honeycomb_rib/honeycomb_rib.py" => Duration::from_secs(300),
        "wcc/wcc_cli.py" => Duration::from_secs(600),
        "ipp/ipp_cli.py" => Duration::from_secs(1200),
        "trim_boundary/trim_boundary.py" => Duration::from_secs(600),
        "product_analysis/apply_naming.py" => Duration::from_secs(900),
        "product_analysis/find_near_parts.py" => Duration::from_secs(600),
        "product_analysis/save_product_without_history.py" => Duration::from_secs(1200),
        "dump_solid/dump_solid.py" => Duration::from_secs(600),
        "product_analysis/vba_bbox_generate.py" => Duration::from_secs(900),
        // 投影面积: 无轴系时先做角度扫描建系(几十次极值量测)再压扁/模板实例化
        "product_analysis/rule_projected_surface.py" => Duration::from_secs(900),
        "product_analysis/rule_bounding_box.py" => Duration::from_secs(900),
        // 文字/二维码转草图: 轮廓量大时草图逐线 CreateLine + 拉伸 + 变换都很耗时
        "text_to_sketch/text_to_sketch.py" => Duration::from_secs(900),
        // 截图标注: 色标/两轮截图/WebP 编码 + FTA 注解逐个创建
        "measure_harness/measure_harness.py" => Duration::from_secs(1200),
        "product_analysis/batch_bbox.py" => Duration::from_secs(3600),
        "product_analysis/product_measure.py" => Duration::from_secs(3600),
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
