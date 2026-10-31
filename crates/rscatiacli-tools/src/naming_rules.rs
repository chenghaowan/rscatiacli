//! 命名规则解析（对应 ltl-manage resources/sidecars/product_analysis/naming_rules.py）。
//!
//! 文件名 / 任意文本 → part_number / part_name，两种模式：
//! position（1-based 起始 + 长度，0 = 取到末尾）/ delimiter（分隔符 + 0-based
//! 索引，零件名 -1 = 剩余全部）。与前端 parsePartsFromFilename 同口径。

use serde_json::Value;

fn substring(s: &str, start: i64, length: i64) -> String {
    if s.is_empty() {
        return String::new();
    }
    let start = start.max(1) as usize; // 1-based
    let chars: Vec<char> = s.chars().collect();
    if start > chars.len() {
        return String::new();
    }
    let end = if length > 0 {
        (start + length as usize - 1).min(chars.len())
    } else {
        chars.len()
    };
    chars[start - 1..end].iter().collect()
}

/// 按配置从任意文本提取 part_number / part_name（不处理扩展名）。
pub fn parse_text(text: &str, config: &Value) -> (String, String) {
    let mode = config
        .get("mode")
        .and_then(|v| v.as_str())
        .unwrap_or("position");
    if mode == "delimiter" {
        let delim = config
            .get("delimiter")
            .and_then(|v| v.as_str())
            .unwrap_or("-");
        let parts: Vec<&str> = text.split(delim).collect();
        let idx_pn = config
            .get("part_number_index")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let idx_pname = config
            .get("part_name_index")
            .and_then(|v| v.as_i64())
            .unwrap_or(-1);
        let mut pn = String::new();
        let mut pname = String::new();
        if idx_pn >= 0 && (idx_pn as usize) < parts.len() {
            pn = parts[idx_pn as usize].trim().to_string();
        }
        if idx_pname == -1 {
            if ((idx_pn + 1) as usize) < parts.len() {
                let start = (idx_pn + 1) as usize;
                pname = parts[start..].join(delim).trim().to_string();
            }
        } else if idx_pname >= 0 && (idx_pname as usize) < parts.len() {
            pname = parts[idx_pname as usize].trim().to_string();
        }
        (pn, pname)
    } else {
        let pn = substring(
            text,
            config
                .get("part_number_start")
                .and_then(|v| v.as_i64())
                .unwrap_or(1),
            config
                .get("part_number_len")
                .and_then(|v| v.as_i64())
                .unwrap_or(0),
        );
        let pname = substring(
            text,
            config
                .get("part_name_start")
                .and_then(|v| v.as_i64())
                .unwrap_or(1),
            config
                .get("part_name_len")
                .and_then(|v| v.as_i64())
                .unwrap_or(0),
        );
        (pn, pname)
    }
}

/// 从文件名解析（剥掉最后一个 `.` 后的扩展名后按 parse_text）。
pub fn parse_parts(filename: &str, config: &Value) -> (String, String) {
    let base = match filename.rsplit_once('.') {
        Some((b, _)) if !b.is_empty() => b,
        _ => filename,
    };
    parse_text(base, config)
}

/// 按命名配置从 CATIA Product.PartNumber 原文提取纯净零件号（无配置或解析
/// 不出时返回去空白原文）。
pub fn refine_part_number(raw: &str, config: &Value) -> String {
    if raw.is_empty() {
        return String::new();
    }
    let text = raw.trim();
    let (pn, _) = parse_text(text, config);
    if pn.is_empty() {
        text.to_string()
    } else {
        pn
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn position_mode() {
        let cfg = json!({"mode": "position", "part_number_start": 1, "part_number_len": 7,
                         "part_name_start": 8, "part_name_len": 0});
        assert_eq!(
            parse_parts("5866262IP DV VANE.CATPart", &cfg),
            ("5866262".into(), "IP DV VANE".into())
        );
    }

    #[test]
    fn delimiter_mode() {
        let cfg = json!({"mode": "delimiter", "delimiter": "-", "part_number_index": 0, "part_name_index": -1});
        assert_eq!(
            parse_parts("ABC-123-bracket plate.CATPart", &cfg),
            ("ABC".into(), "123-bracket plate".into())
        );
    }

    #[test]
    fn substring_edges() {
        assert_eq!(substring("", 1, 3), "");
        assert_eq!(substring("ab", 5, 2), "");
        assert_eq!(substring("abc", 2, 0), "bc");
        assert_eq!(substring("abc", 2, 99), "bc");
    }

    #[test]
    fn refine_keeps_raw_when_unparseable() {
        let cfg = json!({"mode": "position", "part_number_start": 1, "part_number_len": 7});
        assert_eq!(refine_part_number("5866262IP DV VANE", &cfg), "5866262");
        assert_eq!(refine_part_number("  plain  ", &cfg), "plain");
        assert_eq!(refine_part_number("", &cfg), "");
    }
}
