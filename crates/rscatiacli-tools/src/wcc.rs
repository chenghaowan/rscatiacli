//! WCC —— CATIA 重量重心计算/校验（suites/wcc 的 Rust 版）。
//!
//! 命令：info / weight / check / report / csv / tree / config / config-init
//! （JSON 入参 `command` 字段分发，输出文本与 Python 版逐行对齐，经 progress 回传）。
//!
//! 与 Python 版的差异（有意为之，注释标明）：
//! - Python 的 `inertia.volume/surface_area` 调用是 AttributeError（pycatia 的
//!   Inertia 没有这两个属性），实际一直读到 0 —— 这里体积/面积走 SPAWorkbench
//!   Measurable 补全意图（仅 Part 文档；Product 文档维持 0）；
//! - COG 用 pycatia 同款 GetCOGPosition VBA 中转（Python 版因异常从未读到过）；
//! - 单位：Mass=kg，COG=m（CAA 口径），Volume=m³，Area=m²。

use std::io::Write as _;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use rscatiacli::objects::Document;
use rscatiacli::{attach_with_guard, ComCollection, ComResult, Progress};

/// 配置（对齐 config.py 默认值）。
struct WccConfig {
    warning_kg: f64,
    critical_kg: f64,
    decimal_places: usize,
    output_dir: String,
    include_children: bool,
}

impl Default for WccConfig {
    fn default() -> Self {
        Self {
            warning_kg: 10.0,
            critical_kg: 50.0,
            decimal_places: 3,
            output_dir: String::new(),
            include_children: true,
        }
    }
}

impl WccConfig {
    /// params.config_path 读 JSON（字段同 config.py），再被 params 直传字段覆盖。
    fn from_params(params: &Value) -> Self {
        let mut cfg = Self::default();
        if let Some(path) = params.get("config_path").and_then(|v| v.as_str()) {
            if let Ok(raw) = std::fs::read_to_string(path) {
                if let Ok(data) = serde_json::from_str::<Value>(&raw) {
                    if let Some(t) = data.get("thresholds") {
                        if let Some(w) = t.get("warning_kg").and_then(|v| v.as_f64()) {
                            cfg.warning_kg = w;
                        }
                        if let Some(c) = t.get("critical_kg").and_then(|v| v.as_f64()) {
                            cfg.critical_kg = c;
                        }
                    }
                    if let Some(d) = data.get("report_decimal_places").and_then(|v| v.as_u64()) {
                        cfg.decimal_places = d as usize;
                    }
                    if let Some(o) = data.get("output_dir").and_then(|v| v.as_str()) {
                        cfg.output_dir = o.to_string();
                    }
                    if let Some(c) = data.get("report_include_children").and_then(|v| v.as_bool()) {
                        cfg.include_children = c;
                    }
                }
            }
        }
        if let Some(w) = params.get("warning_kg").and_then(|v| v.as_f64()) {
            cfg.warning_kg = w;
        }
        if let Some(c) = params.get("critical_kg").and_then(|v| v.as_f64()) {
            cfg.critical_kg = c;
        }
        if let Some(d) = params.get("decimal_places").and_then(|v| v.as_u64()) {
            cfg.decimal_places = d as usize;
        }
        cfg
    }
}

/// 单行重量结果（对齐 calculate_weight 返回 dict）。
struct WeightRow {
    part_number: String,
    mass_kg: f64,
    volume_m3: f64,
    surface_area_m2: f64,
    density_raw: Option<f64>,
    cog: Option<[f64; 3]>,
    status: &'static str,
    depth: usize,
}

fn status_for(mass_kg: f64, cfg: &WccConfig) -> &'static str {
    if mass_kg >= cfg.critical_kg {
        "CRITICAL"
    } else if mass_kg >= cfg.warning_kg {
        "WARNING"
    } else {
        "OK"
    }
}

/// 文档类型推断（对齐 pycatia str(doc.document_type) 的常见取值）。
fn document_type_name(doc: &Document) -> &'static str {
    if doc.part_document().part().is_ok() {
        "DocumentType.part_document"
    } else if doc.product_document().product().is_ok() {
        "DocumentType.product_document"
    } else {
        "(unknown)"
    }
}

/// 读取单文档质量属性（对齐 get_mass_properties，差异见模块注释）。
fn get_mass_properties(
    app: &rscatiacli::objects::Application,
    doc: &Document,
) -> Option<WeightRow> {
    let name = doc.name().unwrap_or_default();
    let part_number = if name.is_empty() {
        "UNKNOWN".to_string()
    } else {
        Path::new(&name)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or(name.clone())
    };

    let mut mass_kg = 0.0f64;
    let mut density_raw = None;
    let mut cog = None;

    if let Ok(product) = doc.product_document().product() {
        if let Ok(inertia) = product.get_inertia() {
            mass_kg = inertia.mass().unwrap_or(0.0);
            density_raw = inertia.density().ok();
            if let Ok(ss) = app.system_service() {
                cog = inertia.get_cog_position(&ss).ok();
            }
        }
    }

    // 体积/面积：SPA Measurable（仅 Part 文档；Product 文档保持 0，同 Python 实际行为）
    let mut volume_m3 = 0.0f64;
    let mut surface_area_m2 = 0.0f64;
    let pd = doc.part_document();
    if let (Ok(part), Ok(swb)) = (pd.part(), pd.spa_workbench()) {
        if let Ok(reference) = part.create_reference_from_object(&part) {
            if let Ok(m) = swb.get_measurable(&reference) {
                volume_m3 = m.volume().unwrap_or(0.0);
                surface_area_m2 = m.area().unwrap_or(0.0);
            }
        }
    }

    let cfg_placeholder = WccConfig::default();
    let _ = &cfg_placeholder;
    Some(WeightRow {
        part_number,
        mass_kg,
        volume_m3,
        surface_area_m2,
        density_raw,
        cog,
        status: "",
        depth: 0,
    })
}

/// 计算重量 + 阈值状态（对齐 calculate_weight）。
fn calculate_weight(
    app: &rscatiacli::objects::Application,
    doc: &Document,
    cfg: &WccConfig,
    depth: usize,
) -> Option<WeightRow> {
    let mut row = get_mass_properties(app, doc)?;
    row.status = status_for(row.mass_kg, cfg);
    row.depth = depth;
    Some(row)
}

/// 递归遍历（对齐 calculate_weight_recursive：产品树逐子件文档）。
fn calculate_recursive(
    app: &rscatiacli::objects::Application,
    doc: &Document,
    cfg: &WccConfig,
    depth: usize,
    rows: &mut Vec<WeightRow>,
    progress: &dyn Fn(&str),
) {
    if let Some(row) = calculate_weight(app, doc, cfg, depth) {
        progress(&format!(
            "  [{depth}] {} = {:.3} kg ({})",
            row.part_number, row.mass_kg, row.status
        ));
        rows.push(row);
    }
    if !cfg.include_children {
        return;
    }
    if let Ok(product) = doc.product_document().product() {
        if let Ok(children) = product.products() {
            let self_name = doc.name().unwrap_or_default();
            for child in children.items().unwrap_or_default() {
                let child_doc = child
                    .reference_product()
                    .and_then(|r| r.parent_document());
                if let Ok(child_doc) = child_doc {
                    if child_doc.name().unwrap_or_default() != self_name {
                        calculate_recursive(app, &child_doc, cfg, depth + 1, rows, progress);
                    }
                }
            }
        }
    }
}

fn fmt_fixed(v: f64, dp: usize) -> String {
    format!("{:.*}", dp, v)
}

fn fmt_exp(v: f64, dp: usize) -> String {
    // 对齐 Python {:.dp e} 的两位指数带符号形式：1.235e-03 / 0.000e+00
    let s = format!("{:.*e}", dp, v);
    let Some((mantissa, exp)) = s.split_once('e') else {
        return s;
    };
    let Ok(n) = exp.parse::<i32>() else {
        return s;
    };
    format!("{mantissa}e{}{:02}", if n < 0 { '-' } else { '+' }, n.abs())
}

/// 文本报表（对齐 report.generate_text_report，逐行一致）。
fn generate_text_report(rows: &[WeightRow], cfg: &WccConfig) -> String {
    let dp = cfg.decimal_places;
    let mut lines: Vec<String> = Vec::new();
    lines.push("=".repeat(70));
    lines.push("  WEIGHT CALCULATION & CHECK REPORT".to_string());
    lines.push("=".repeat(70));
    lines.push(String::new());
    lines.push(format!(
        "{:<6} {:<30} {:<12} {:<10}",
        "Depth", "Part Number", "Mass (kg)", "Status"
    ));
    lines.push("-".repeat(70));

    let mut total_mass = 0.0f64;
    let mut warnings = 0usize;
    let mut criticals = 0usize;
    for r in rows {
        total_mass += r.mass_kg;
        match r.status {
            "WARNING" => warnings += 1,
            "CRITICAL" => criticals += 1,
            _ => {}
        }
        let indent = "  ".repeat(r.depth);
        let pn: String = r.part_number.chars().take(28).collect();
        lines.push(format!(
            "{:<6} {:<30} {:>w$.dp$}  {:<10}",
            indent,
            pn,
            r.mass_kg,
            r.status,
            w = dp + 4,
            dp = dp
        ));
    }

    lines.push("-".repeat(70));
    lines.push(format!(
        "{:<36} {:>w$.dp$}",
        "TOTAL",
        total_mass,
        w = dp + 4,
        dp = dp
    ));
    lines.push(String::new());
    lines.push("Summary:".to_string());
    lines.push(format!("  Total parts/products  : {}", rows.len()));
    lines.push(format!("  Total mass            : {:.dp$} kg", total_mass, dp = dp));
    lines.push(format!("  Warnings              : {warnings}"));
    lines.push(format!("  Criticals             : {criticals}"));
    lines.push(String::new());
    lines.push("Thresholds:".to_string());
    lines.push(format!("  Warning  >= {} kg", cfg.warning_kg));
    lines.push(format!("  Critical >= {} kg", cfg.critical_kg));
    lines.push("=".repeat(70));
    lines.join("\n")
}

/// CSV 导出（对齐 export_weight_csv：utf-8-sig BOM + TOTAL 行；返回 (行数, 总质量)）。
fn export_csv(rows: &[WeightRow], cfg: &WccConfig, output_path: &Path) -> Result<(usize, f64), String> {
    if rows.is_empty() {
        return Err("No weight data to export.".to_string());
    }
    if let Some(parent) = output_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let dp = cfg.decimal_places;
    let mut f = std::fs::File::create(output_path).map_err(|e| e.to_string())?;
    f.write_all(&[0xEF, 0xBB, 0xBF]).map_err(|e| e.to_string())?; // utf-8-sig BOM
    let mut write_row = |fields: &[String]| -> Result<(), String> {
        writeln!(f, "{}", fields.join(",")).map_err(|e| e.to_string())
    };
    write_row(&[
        "Depth".into(),
        "Part Number".into(),
        "Mass (kg)".into(),
        "Volume (m3)".into(),
        "Surface Area (m2)".into(),
        "Density (g/cm3)".into(),
        "Status".into(),
        "COG X".into(),
        "COG Y".into(),
        "COG Z".into(),
    ])?;
    let mut total_mass = 0.0f64;
    for r in rows {
        total_mass += r.mass_kg;
        let (cx, cy, cz) = match r.cog {
            Some([x, y, z]) => (format!("{x:.3}"), format!("{y:.3}"), format!("{z:.3}")),
            None => (String::new(), String::new(), String::new()),
        };
        write_row(&[
            r.depth.to_string(),
            r.part_number.clone(),
            fmt_fixed(r.mass_kg, dp),
            fmt_exp(r.volume_m3, dp),
            fmt_exp(r.surface_area_m2, dp),
            r.density_raw.map(|d| fmt_fixed(d, dp)).unwrap_or_default(),
            r.status.to_string(),
            cx,
            cy,
            cz,
        ])?;
    }
    write_row(&[String::new()])?;
    write_row(&["TOTAL".into(), String::new(), fmt_fixed(total_mass, dp)])?;
    Ok((rows.len(), total_mass))
}

/// 入口（json_tool / json_cli）。params: {command, config_path?, output?,
/// warning_kg?, critical_kg?, decimal_places?, include_children?}。
pub fn run(params: &Value, progress: Progress<'_>) -> ComResult<Value> {
    let command = params
        .get("command")
        .and_then(|v| v.as_str())
        .unwrap_or("weight")
        .to_string();
    let cfg = WccConfig::from_params(params);

    // 纯配置命令不需要 CATIA
    match command.as_str() {
        "config" => {
            return Ok(json!({
                "success": true,
                "data": {
                    "thresholds": {"warning_kg": cfg.warning_kg, "critical_kg": cfg.critical_kg},
                    "report_decimal_places": cfg.decimal_places,
                    "report_include_children": cfg.include_children,
                    "output_dir": cfg.output_dir,
                },
            }));
        }
        "config-init" => {
            let path = params.get("output").and_then(|v| v.as_str()).unwrap_or("wcc_config.json");
            if let Some(parent) = Path::new(path).parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let data = json!({
                "thresholds": {"warning_kg": cfg.warning_kg, "critical_kg": cfg.critical_kg},
                "report_decimal_places": cfg.decimal_places,
                "report_include_children": cfg.include_children,
                "output_dir": cfg.output_dir,
            });
            let text = serde_json::to_string_pretty(&data).unwrap_or_default();
            std::fs::write(path, text).map_err(|e| rscatiacli::ComError::runtime(e.to_string()))?;
            progress(&format!("Configuration saved to: {path}"));
            return Ok(json!({"success": true, "data": {"path": path}}));
        }
        _ => {}
    }

    let app = attach_with_guard(progress)?;
    let doc = app
        .active_document()
        .map_err(|_| rscatiacli::ComError::runtime("No active document."))?;

    match command.as_str() {
        "info" => {
            let name = doc.name().unwrap_or_else(|_| "(unknown)".into());
            let path = doc
                .full_name()
                .unwrap_or_else(|_| "(not saved)".into());
            let dtype = document_type_name(&doc);
            progress(&format!("Name:  {name}"));
            progress(&format!("Path:  {path}"));
            progress(&format!("Type:  {dtype}"));
            Ok(json!({"success": true, "data": {"name": name, "path": path, "type": dtype}}))
        }
        "weight" => {
            let row = calculate_weight(&app, &doc, &cfg, 0)
                .ok_or_else(|| rscatiacli::ComError::runtime("Failed to calculate weight."))?;
            let dp = cfg.decimal_places;
            progress(&format!("Part Number:     {}", row.part_number));
            progress(&format!("Mass:            {:.dp$} kg", row.mass_kg, dp = dp));
            progress(&format!("Volume:          {} m3", fmt_exp(row.volume_m3, dp)));
            progress(&format!("Surface Area:    {} m2", fmt_exp(row.surface_area_m2, dp)));
            progress(&format!(
                "Density:         {} g/cm3",
                row.density_raw.map(|d| fmt_fixed(d, dp)).unwrap_or_default()
            ));
            progress(&format!("Status:          {}", row.status));
            Ok(json!({
                "success": true,
                "data": {
                    "part_number": row.part_number, "mass_kg": row.mass_kg,
                    "volume_m3": row.volume_m3, "surface_area_m2": row.surface_area_m2,
                    "density_raw": row.density_raw, "status": row.status,
                    "cog": row.cog,
                },
            }))
        }
        "check" => {
            let row = calculate_weight(&app, &doc, &cfg, 0)
                .ok_or_else(|| rscatiacli::ComError::runtime("Failed to calculate weight"))?;
            let name = doc.name().unwrap_or_else(|_| "(unknown)".into());
            let path = doc.full_name().unwrap_or_else(|_| "(unknown)".into());
            let dp = cfg.decimal_places;
            progress(&format!("Document:        {name}"));
            progress(&format!("Part Number:     {}", row.part_number));
            progress(&format!("Mass:            {:.dp$} kg", row.mass_kg, dp = dp));
            progress(&format!("Status:          {}", row.status));
            progress(&format!(
                "Thresholds:      Warning >= {} kg, Critical >= {} kg",
                cfg.warning_kg, cfg.critical_kg
            ));
            match row.status {
                "CRITICAL" => progress("[!!] CRITICAL: Weight exceeds critical threshold!"),
                "WARNING" => progress("[!] WARNING: Weight exceeds warning threshold."),
                _ => {}
            }
            Ok(json!({
                "success": true,
                "data": {
                    "document": name, "path": path, "type": document_type_name(&doc),
                    "part_number": row.part_number, "mass_kg": row.mass_kg,
                    "status": row.status,
                    "threshold_warning": cfg.warning_kg, "threshold_critical": cfg.critical_kg,
                },
            }))
        }
        "report" | "tree" => {
            progress("Calculating weight tree ...");
            let mut rows = Vec::new();
            calculate_recursive(&app, &doc, &cfg, 0, &mut rows, &progress);
            if rows.is_empty() {
                return Err(rscatiacli::ComError::runtime("No weight data."));
            }
            let report = generate_text_report(&rows, &cfg);
            progress(String::new().as_str());
            progress(&report);
            progress(String::new().as_str());
            let mut data = json!({"total": rows.len(), "report": report});
            if let Some(output) = params.get("output").and_then(|v| v.as_str()).filter(|s| !s.is_empty()) {
                if let Some(parent) = Path::new(output).parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                match std::fs::write(output, &report) {
                    Ok(()) => {
                        progress(&format!("Report saved: {output}"));
                        data["saved_to"] = json!(output);
                    }
                    Err(e) => progress(&format!("[ERROR] Failed to save report: {e}")),
                }
            }
            Ok(json!({"success": true, "data": data}))
        }
        "csv" => {
            let mut rows = Vec::new();
            calculate_recursive(&app, &doc, &cfg, 0, &mut rows, &progress);
            if rows.is_empty() {
                return Err(rscatiacli::ComError::runtime("No weight data."));
            }
            let doc_name = doc.name().unwrap_or_else(|_| "document".into());
            let stem = Path::new(&doc_name)
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or(doc_name.clone());
            let output = params
                .get("output")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(PathBuf::from)
                .unwrap_or_else(|| {
                    let base = if cfg.output_dir.is_empty() {
                        std::env::current_dir().unwrap_or_default()
                    } else {
                        PathBuf::from(&cfg.output_dir)
                    };
                    base.join(format!("{stem}_weight.csv"))
                });
            let (count, total) = export_csv(&rows, &cfg, &output)
                .map_err(|e| rscatiacli::ComError::runtime(e))?;
            let out_str = output.to_string_lossy().to_string();
            progress(&format!("Weight report saved: {out_str}"));
            progress(&format!("Total assemblies: {count}, Total mass: {total:.3} kg"));
            Ok(json!({
                "success": true,
                "data": {"path": out_str, "count": count, "total_mass_kg": total},
            }))
        }
        other => Err(rscatiacli::ComError::runtime(format!(
            "未知的 wcc 命令: {other}(可选 info/weight/check/report/csv/tree/config/config-init)"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(pn: &str, mass: f64, status: &'static str, depth: usize) -> WeightRow {
        WeightRow {
            part_number: pn.into(),
            mass_kg: mass,
            volume_m3: 1.23456e-3,
            surface_area_m2: 5.6789e-1,
            density_raw: Some(7850.0),
            cog: Some([0.1, 0.2, 0.3]),
            status,
            depth,
        }
    }

    #[test]
    fn report_format_matches_python_layout() {
        let cfg = WccConfig::default();
        let rows = vec![
            row("TOP-ASSY", 42.0, "WARNING", 0),
            row("LEAF-1", 2.5, "OK", 1),
        ];
        let text = generate_text_report(&rows, &cfg);
        assert!(text.contains("  WEIGHT CALCULATION & CHECK REPORT"));
        assert!(text.contains(&format!(
            "{:<6} {:<30} {:<12} {:<10}",
            "Depth", "Part Number", "Mass (kg)", "Status"
        )));
        assert!(text.contains("TOP-ASSY"));
        assert!(text.contains("TOTAL"));
        assert!(text.contains("  Total parts/products  : 2"));
        assert!(text.contains("  Total mass            : 44.500 kg"));
        assert!(text.contains("  Warnings              : 1"));
        assert!(text.contains("  Criticals             : 0"));
        assert!(text.contains("  Warning  >= 10 kg"));
    }

    #[test]
    fn status_thresholds() {
        let cfg = WccConfig::default();
        assert_eq!(status_for(9.99, &cfg), "OK");
        assert_eq!(status_for(10.0, &cfg), "WARNING");
        assert_eq!(status_for(50.0, &cfg), "CRITICAL");
    }

    #[test]
    fn exp_format_two_digit_exponent() {
        // 对齐 Python {:.3e}: 1.234e-05
        assert_eq!(fmt_exp(1.23456e-3, 3), "1.235e-03");
        assert_eq!(fmt_exp(0.0, 3), "0.000e+00");
    }
}
