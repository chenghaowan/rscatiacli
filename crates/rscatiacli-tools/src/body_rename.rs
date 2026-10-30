//! optimize-body-names：优化 Body / 几何图形集名字。
//!
//! 对应 ltl-manage resources/sidecars/body_rename/rename_bodies.py（233 行）。
//! 规则完全一致：
//!   1. 名字含 `\` 时按层切开，保留最后 keep_last 段，多段用空格连接；
//!   2. 残留非法字符换成下划线，连续下划线压缩，首尾空格/下划线去掉；
//!   3. 与现有对象重名时追加 _2/_3。
//!
//! 差异：Python 版在活动文档不是 .CATPart 时会调 `ensure_part_doc()` 自动打开同名零件；
//! 本期未移植，直接返回明确错误。

use std::collections::HashSet;

use serde_json::{json, Value};

use rscatiacli::{attach_with_guard, ComCollection, ComObject, ComResult, Progress};

/// CATIA 特征名中的非法字符（与 Python 版 ILLEGAL_CHARS 完全一致）。
const ILLEGAL_CHARS: &str = "!\"§$%&/=?`>^°{[]}ßüä*+~'#,;:@<>|";

/// 把原始 body/图形集名转换为目标合法名。
fn clean_name(name: &str, keep_last: i64) -> String {
    let mut s = name.to_string();

    // 1. 按 \ 分层取名字核心：保留最后 keep_last 段
    if s.contains('\\') {
        let segs: Vec<&str> = s
            .split('\\')
            .map(str::trim)
            .filter(|seg| !seg.is_empty())
            .collect();
        if segs.is_empty() {
            return String::new();
        }
        let keep = keep_last.max(1) as usize;
        let start = segs.len().saturating_sub(keep);
        s = segs[start..].join(" ");
    }

    // 2. 非法字符 → 下划线；连续下划线压缩为一个
    let mut out = String::with_capacity(s.len());
    let mut prev_underscore = false;
    for ch in s.chars() {
        let mapped = if ILLEGAL_CHARS.contains(ch) { '_' } else { ch };
        if mapped == '_' {
            if !prev_underscore {
                out.push('_');
            }
            prev_underscore = true;
        } else {
            out.push(mapped);
            prev_underscore = false;
        }
    }

    // 3. 去掉首尾的空格/下划线（等价 Python str.strip(" _")）
    out.trim_matches(|c| c == ' ' || c == '_').to_string()
}

/// 重名时追加 _2/_3 序号。
fn unique_name(base: &str, used: &HashSet<String>) -> String {
    if !used.contains(base) {
        return base.to_string();
    }
    let mut i = 2;
    loop {
        let candidate = format!("{base}_{i}");
        if !used.contains(&candidate) {
            return candidate;
        }
        i += 1;
    }
}

fn fail(message: impl Into<String>) -> Value {
    json!({ "success": false, "message": message.into() })
}

/// 工具主体。**必须在 STA 线程上执行。**
pub fn run(params: &Value, progress: Progress<'_>) -> ComResult<Value> {
    let dry_run = params
        .get("dry_run")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let keep_last = params
        .get("keep_last")
        .and_then(|v| v.as_i64())
        .unwrap_or(1)
        .max(1);

    let app = attach_with_guard(progress)?;

    if app.document_count()? == 0 {
        return Ok(fail(
            "CATIA 没有打开的文档,请先在 CATIA 中打开生成的 .CATPart 再运行",
        ));
    }

    let doc = app.active_document()?;
    let part = match doc.part() {
        Ok(part) => part,
        Err(_) => {
            return Ok(fail(
                "当前活动文档不是零件(.CATPart): 请把要处理的零件设为活动文档后重试",
            ))
        }
    };

    // 收集 Bodies 与几何图形集（顺序与 Python 版一致：先 Body 后图形集）。
    // Body 与 HybridBody 只在名字读写上不同，用 ComObject trait 对象统一处理。
    let mut objects: Vec<(Box<dyn ComObject>, &'static str)> = Vec::new();
    if let Ok(bodies) = part.bodies() {
        if let Ok(items) = bodies.items() {
            objects.extend(
                items
                    .into_iter()
                    .map(|b| (Box::new(b) as Box<dyn ComObject>, "Body")),
            );
        }
    }
    if let Ok(hybrid_bodies) = part.hybrid_bodies() {
        if let Ok(items) = hybrid_bodies.items() {
            objects.extend(
                items
                    .into_iter()
                    .map(|h| (Box::new(h) as Box<dyn ComObject>, "几何图形集")),
            );
        }
    }

    let total = objects.len();
    if total == 0 {
        return Ok(fail("当前零件没有 Body / 几何图形集,无需处理"));
    }

    // 已占用名 = 所有对象当前名（改名不能互相撞名）
    let mut used: HashSet<String> = HashSet::new();
    for (obj, _) in &objects {
        if let Ok(name) = obj.name() {
            used.insert(name);
        }
    }

    progress(&format!(
        "开始分析 {total} 个 Body/几何图形集(dry_run={}, 保留最后 {keep_last} 段)...",
        if dry_run { "是" } else { "否" }
    ));

    let mut items: Vec<Value> = Vec::new();
    let mut errors: Vec<String> = Vec::new();

    for (idx, (obj, kind)) in objects.iter().enumerate() {
        let idx = idx + 1;
        let old = match obj.name() {
            Ok(name) => name,
            Err(e) => {
                errors.push(format!("{kind}[{idx}]: 读取名字失败 {e}"));
                continue;
            }
        };

        let target_base = clean_name(&old, keep_last);
        if target_base.is_empty() || target_base == old {
            progress(&format!("  [{idx}/{total}] [{kind}] {old} — 无需修改"));
            continue;
        }

        // 排除自身旧名后查重
        used.remove(&old);
        let new = unique_name(&target_base, &used);
        used.insert(new.clone());

        if dry_run {
            progress(&format!(
                "  [{idx}/{total}] [{kind}] {old}  ->  (预览) {new}"
            ));
            items.push(json!({ "old": old, "new": new, "kind": kind }));
        } else {
            match obj.set_name(&new) {
                Ok(()) => {
                    progress(&format!("  [{idx}/{total}] [{kind}] {old}  ->  {new}"));
                    items.push(json!({ "old": old, "new": new, "kind": kind }));
                }
                Err(e) => {
                    // 目标名被 CATIA 拒绝：记录错误，不中断整体
                    errors.push(format!("{old}: 改名失败 {e}"));
                    used.remove(&new);
                }
            }
        }
    }

    // 与 Python 版一致：结束时清空选择集
    if let Ok(sel) = doc.selection() {
        let _ = sel.clear();
    }

    let renamed_count = items.len();
    let unchanged = total.saturating_sub(renamed_count + errors.len());
    let prefix = if dry_run { "(预览) " } else { "" };
    let mut parts = vec![
        format!("{prefix}共 {total} 个 Body/几何图形集"),
        format!(
            "{} {renamed_count} 个",
            if dry_run {
                "将重命名"
            } else {
                "已重命名"
            }
        ),
    ];
    if unchanged > 0 {
        parts.push(format!("{unchanged} 个名字已合法"));
    }
    if !errors.is_empty() {
        parts.push(format!("{} 个失败", errors.len()));
    }
    let mut message = parts.join(", ");
    if !dry_run && renamed_count > 0 {
        message.push_str(";请手动保存文档(Ctrl+S)");
    }

    Ok(json!({
        "success": true,
        "message": message,
        "data": {
            "total": total,
            "renamed_count": renamed_count,
            "unchanged_count": unchanged,
            "error_count": errors.len(),
            "preview": dry_run,
            "keep_last": keep_last,
            "items": items,
            "errors": errors,
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_last_segments_and_strips_prefix() {
        assert_eq!(
            clean_name(r"5778995BP OUTLET  SEALING.1\5778995BP OUTLET  SEALING", 1),
            "5778995BP OUTLET  SEALING"
        );
        assert_eq!(clean_name(r"a\b\c", 2), "b c");
        assert_eq!(clean_name(r"a\b\c", 1), "c");
        assert_eq!(clean_name(r"a\b\c", 9), "a b c");
    }

    #[test]
    fn replaces_illegal_chars_and_collapses() {
        assert_eq!(clean_name("a//b", 1), "a_b");
        assert_eq!(clean_name("  x  ", 1), "x");
        assert_eq!(clean_name("a**b", 1), "a_b");
        assert_eq!(clean_name("valid name.1", 1), "valid name.1");
    }

    #[test]
    fn unique_appends_suffix() {
        let mut used = HashSet::new();
        used.insert("a".to_string());
        assert_eq!(unique_name("a", &used), "a_2");
        used.insert("a_2".to_string());
        assert_eq!(unique_name("a", &used), "a_3");
        assert_eq!(unique_name("b", &used), "b");
    }
}
