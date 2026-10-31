//! add-material：给 Part 添加材料（打开材料目录，按名字自动挂材料）。
//!
//! 对应 ltl-manage `resources/sidecars/add_material/add_material.py`，JSON 契约
//! 与消息文案逐字段对齐。动作：
//!   list  → 打开目录读取全部 材料族/材料 清单（读完关闭目录，不改任何文档）
//!   apply → 打开目录按名字找材料，挂到当前 .CATPart：
//!           target=partbody(默认, 主 Body) / part(整个 Part) / bodies(逐 Body)
//!
//! API 口径同 V5Automation 官方示例 applyMaterials.CATScript：
//! link_mode 0=复制材料值(无链接), 1=链接到材料目录；GetMaterialOnPart/Body 的
//! out 参数走 `SystemService.Evaluate` 注入 VBA 取回（与 Python 侧同一条实测路径）。
//!
//! 材料目录解析顺序：
//!   1. 参数 catalog_path（文件或目录，前端可填）
//!   2. 软件内置 FIS 标准库（bundled_dir 指向打包的 sidecars/add_material；
//!      首次使用复制到 %APPDATA%\ltl-manage\materials 保证 link_mode=1 链接稳定）
//!   3. FIS 环境：%FIS_TB_DIR% 相关 MATERIAL_CATALOG 目录下的 *.CATMaterial
//!   4. CATIA 安装目录：SystemService.Environ 读 CATInstallPath/CATDLLPath，
//!      问不到退回进程环境变量

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use rscatiacli::dispatch::Arg;
use rscatiacli::objects::{MaterialDocument, MaterialManager, Part, SystemService};
use rscatiacli::{attach_with_guard, ComCollection, ComObj, ComObject, ComResult, Progress};

const FIS_CATALOG_FILENAME: &str = "MATCV5-OR-V01-MATERIALS_CATALOG.CATMaterial";
const BUNDLED_STABLE_DIRNAME: &str = "ltl-manage";

/// GetMaterialOnPart/GetMaterialOnBody 的 out 参数无法直接从晚绑定侧封送,
/// 沿用 pycatia / Python 版的做法: SystemService.Evaluate 注入一段 VBA 取回。
const VBA_GET_MATERIAL: &str = r#"Public Function get_material(kind, manager, target)
    Dim material
    If kind = "part" Then
        manager.GetMaterialOnPart target, material
    Else
        manager.GetMaterialOnBody target, material
    End If
    Set get_material = material
End Function
"#;

// ── 材料目录解析 ─────────────────────────────────────────────────────

/// 目录下的全部 .CATMaterial，按优先级排序：
/// FIS 标准库 MATCV5 > CATIA 默认库 Catalog > 其余(字母序)。
fn list_catalog_files(folder: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .map(|e| e.to_string_lossy().eq_ignore_ascii_case("CATMaterial"))
                .unwrap_or(false)
        })
        .collect();
    files.sort_by_key(|f| {
        let n = f
            .file_name()
            .map(|s| s.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        if n == FIS_CATALOG_FILENAME.to_lowercase() {
            (0, n)
        } else if n == "catalog.catmaterial" {
            (1, n)
        } else {
            (2, n)
        }
    });
    files
}

/// 内置 FIS 标准材料库的稳定路径：复制到 %APPDATA% 固定位置（link_mode=1 的
/// 材料链接要长期指向稳定路径）。bundled_dir 是应用打包目录，副本比稳定文件
/// 新时刷新。无内置库时 None。
fn bundled_catalog(bundled_dir: Option<&str>) -> Option<PathBuf> {
    let base = std::env::var("APPDATA")
        .unwrap_or_else(|_| std::env::temp_dir().to_string_lossy().to_string());
    let stable_dir = Path::new(&base)
        .join(BUNDLED_STABLE_DIRNAME)
        .join("materials");
    let dst = stable_dir.join(FIS_CATALOG_FILENAME);

    if let Some(dir) = bundled_dir.filter(|s| !s.is_empty()) {
        let src = Path::new(dir).join(FIS_CATALOG_FILENAME);
        if src.is_file() {
            let src_newer = match (src.metadata(), dst.metadata()) {
                (Ok(s), Ok(d)) => match (s.modified().ok(), d.modified().ok()) {
                    (Some(a), Some(b)) => a > b,
                    _ => true,
                },
                (Ok(_), Err(_)) => true,
                _ => false,
            };
            if src_newer {
                let _ = std::fs::create_dir_all(&stable_dir);
                let _ = std::fs::copy(&src, &dst);
            }
        }
    }

    if dst.is_file() {
        Some(dst)
    } else {
        None
    }
}

fn catia_env_bases(system_service: Option<&SystemService>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for env in ["CATInstallPath", "CATDLLPath"] {
        let mut val = system_service
            .and_then(|ss| ss.environ(env).ok())
            .unwrap_or_default();
        if val.is_empty() {
            val = std::env::var(env).unwrap_or_default();
        }
        if !val.is_empty() && !out.contains(&val) {
            out.push(val);
        }
    }
    out
}

fn norm_key(p: &Path) -> String {
    p.canonicalize()
        .map(|c| c.to_string_lossy().to_lowercase())
        .unwrap_or_else(|_| p.to_string_lossy().to_lowercase())
}

/// 按顺序解析候选材料目录，去重保序。返回 [{path, source}]。
/// source: explicit=手填 / bundled=软件内置库 / fis_env=FIS 环境变量 / catia_install=CATIA 安装目录
fn resolve_catalog_paths(
    explicit: Option<&str>,
    bundled_dir: Option<&str>,
    system_service: Option<&SystemService>,
) -> Vec<(PathBuf, &'static str)> {
    if let Some(explicit) = explicit.filter(|s| !s.trim().is_empty()) {
        let p = Path::new(explicit.trim());
        if p.is_file() {
            return vec![(p.to_path_buf(), "explicit")];
        }
        if p.is_dir() {
            return list_catalog_files(p)
                .into_iter()
                .map(|f| (f, "explicit"))
                .collect();
        }
        return Vec::new();
    }

    let mut cands: Vec<(PathBuf, &'static str)> = Vec::new();
    if let Some(bundled) = bundled_catalog(bundled_dir) {
        cands.push((bundled, "bundled"));
    }
    if let Ok(fis_tb) = std::env::var("FIS_TB_DIR") {
        if !fis_tb.is_empty() {
            // <FIS_TB_DIR>/FIS_CATALOG 的上级 MATERIAL_CATALOG；FIS_TB_DIR 不带
            // dev 尾层时退化为 <FIS_TB_DIR>/MATERIAL_CATALOG, 两处都查
            let tb = Path::new(&fis_tb);
            for base in [
                tb.join("MATERIAL_CATALOG"),
                tb.parent().unwrap_or(tb).join("MATERIAL_CATALOG"),
            ] {
                cands.extend(
                    list_catalog_files(&base)
                        .into_iter()
                        .map(|f| (f, "fis_env")),
                );
            }
        }
    }
    for base in catia_env_bases(system_service) {
        let base = Path::new(&base);
        // CATInstallPath 可能指向 平台目录(win_b64)或其上级(B32), 两种布局都试
        for layout in [
            base.join("startup").join("materials"),
            base.join("win_b64").join("startup").join("materials"),
        ] {
            cands.extend(
                list_catalog_files(&layout)
                    .into_iter()
                    .map(|f| (f, "catia_install")),
            );
        }
    }

    let mut seen: Vec<String> = Vec::new();
    let mut out = Vec::new();
    for (f, source) in cands {
        let key = norm_key(&f);
        if !seen.contains(&key) {
            seen.push(key);
            out.push((f, source));
        }
    }
    out
}

// ── 目录文档开合（不抢激活：已开复用，只关自己开的） ────────────────────

fn normcase_full(doc: &impl ComObject) -> String {
    doc.com()
        .get("FullName")
        .and_then(|v| v.as_string())
        .map(|s| s.to_lowercase())
        .unwrap_or_default()
}

/// 会话中已打开该路径的文档则返回，否则 None（不切换激活）。
fn find_open_doc(
    docs: &rscatiacli::objects::Documents,
    path: &Path,
) -> Option<rscatiacli::objects::Document> {
    let key = norm_key(path);
    docs.items().ok()?.into_iter().find(|d| {
        let full = normcase_full(d);
        !full.is_empty() && full == key
    })
}

/// 材料目录文档的获取与回收：已开复用，未开则 Open，Drop 只关自己开的。
struct CatalogDoc {
    doc: rscatiacli::objects::Document,
    opened: bool,
}

impl CatalogDoc {
    fn acquire(docs: &rscatiacli::objects::Documents, path: &Path) -> ComResult<Self> {
        if let Some(d) = find_open_doc(docs, path) {
            return Ok(Self {
                doc: d,
                opened: false,
            });
        }
        Ok(Self {
            doc: docs.open(&path.to_string_lossy())?,
            opened: true,
        })
    }

    fn document(&self) -> &rscatiacli::objects::Document {
        &self.doc
    }
}

impl Drop for CatalogDoc {
    fn drop(&mut self) {
        if self.opened {
            let _ = self.doc.close();
        }
    }
}

/// 遍历材料目录文档的全部 材料族/材料，返回 [(family, name, obj)]。
fn iter_catalog_materials(cat_doc: &MaterialDocument) -> ComResult<Vec<(String, String, ComObj)>> {
    let families = cat_doc.families()?;
    let fam_count = families.count()?;
    let mut out = Vec::new();
    for i in 1..=fam_count {
        let Ok(fam) = families.item(i) else { continue };
        let fam_name = fam.name().unwrap_or_default();
        let Ok(mats) = fam.materials() else { continue };
        let Ok(mat_count) = mats.count() else {
            continue;
        };
        for j in 1..=mat_count {
            let Ok(mat) = mats.item(j) else { continue };
            let mat_name = mat.name().unwrap_or_default();
            if !mat_name.is_empty() {
                out.push((fam_name.clone(), mat_name, mat.com().clone()));
            }
        }
    }
    Ok(out)
}

/// 命中的材料：(对象, 所属族名)。
type MaterialHit = Option<(ComObj, String)>;

/// 按名字在目录中找材料：先精确(忽略大小写)，再唯一包含匹配。
/// 返回 (命中材料, 全部候选名列表[找不到时用于报错])。
fn find_material(cat_doc: &MaterialDocument, name: &str) -> ComResult<(MaterialHit, Vec<String>)> {
    let target = name.trim().to_lowercase();
    let entries = iter_catalog_materials(cat_doc)?;
    let all_names: Vec<String> = entries.iter().map(|(_, n, _)| n.clone()).collect();

    for (family, mat_name, obj) in &entries {
        if mat_name.trim().to_lowercase() == target {
            return Ok((Some((obj.clone(), family.clone())), all_names));
        }
    }
    let partial: Vec<&(String, String, ComObj)> = entries
        .iter()
        .filter(|(_, n, _)| n.to_lowercase().contains(&target))
        .collect();
    if partial.len() == 1 {
        let (family, _, obj) = partial[0];
        return Ok((Some((obj.clone(), family.clone())), all_names));
    }
    Ok((None, all_names))
}

/// 用 VBA Evaluate 读当前已挂材料（GetMaterialOnPart/Body），读不到返回 None。
fn get_applied_material(
    system_service: Option<&SystemService>,
    kind: &str,
    manager: &MaterialManager,
    target: &impl ComObject,
) -> Option<String> {
    let ret = system_service?
        .evaluate(
            VBA_GET_MATERIAL,
            0,
            "get_material",
            &[
                Arg::Str(kind.to_string()),
                Arg::Obj(manager.com().clone()),
                Arg::Obj(target.com().clone()),
            ],
        )
        .ok()?;
    if ret.is_null() {
        return None;
    }
    let mat = ret.as_obj().ok()?;
    mat.get("Name")
        .and_then(|v| v.as_string())
        .ok()
        .filter(|s| !s.is_empty())
}

// ── 动作 ─────────────────────────────────────────────────────────────

/// action=list：读取材料目录的 材料族/材料 清单（只读，不改任何文档）。
fn action_list(params: &Value, progress: Progress<'_>) -> Value {
    let explicit = params
        .get("catalog_path")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let bundled_dir = params.get("bundled_dir").and_then(Value::as_str);

    let inner = || -> ComResult<Value> {
        let app = attach_with_guard(progress)?;
        let docs = app.documents()?;
        let system_service = app.system_service().ok();

        let candidates = resolve_catalog_paths(explicit, bundled_dir, system_service.as_ref());
        if candidates.is_empty() {
            return Ok(json!({
                "success": false,
                "message": match explicit {
                    Some(e) => format!("材料目录不存在: {e}\n请填写有效的 .CATMaterial 文件或所在目录"),
                    None => "未自动找到材料目录(.CATMaterial)。\n已尝试: 软件内置库、%FIS_TB_DIR% 相关 MATERIAL_CATALOG、CATIA 安装目录 startup\\materials。\n请在面板中手动填写材料目录文件路径".to_string(),
                }
            }));
        }

        // 目录开合会切换活动文档, 记住原活动文档读完切回
        let prev_active = app.active_document().ok();
        let _ = app.set_display_file_alerts(false);

        let mut tried: Vec<Value> = Vec::new();
        for (path, source) in &candidates {
            tried.push(json!({"path": path.display().to_string(), "error": null}));

            let cat = match CatalogDoc::acquire(&docs, path) {
                Ok(c) => c,
                Err(e) => {
                    tried.last_mut().expect("刚 push")["error"] = json!(e.to_string());
                    progress(&format!("目录打开失败 {}: {e}", path.display()));
                    continue;
                }
            };
            let entries = match iter_catalog_materials(&MaterialDocument::from_document(
                cat.document().clone(),
            )) {
                Ok(v) => v,
                Err(e) => {
                    tried.last_mut().expect("刚 push")["error"] = json!(e.to_string());
                    progress(&format!("目录读取失败 {}: {e}", path.display()));
                    continue;
                }
            };
            drop(cat);

            let mut families: BTreeMap<String, Vec<String>> = BTreeMap::new();
            for (family, name, _) in &entries {
                families
                    .entry(family.clone())
                    .or_default()
                    .push(name.clone());
            }
            if let Some(prev) = &prev_active {
                let _ = prev.activate();
            }
            let fam_list: Vec<Value> = families
                .into_iter()
                .map(|(family, materials)| json!({"family": family, "materials": materials}))
                .collect();
            let fam_count = fam_list.len();
            let entry_count = entries.len();
            return Ok(json!({
                "success": true,
                "message": format!(
                    "已加载材料目录: {}({fam_count} 个材料族, {entry_count} 个材料)",
                    path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default()
                ),
                "data": {
                    "catalog": path.display().to_string(),
                    "source": source,
                    "catalogs_tried": tried,
                    "families": fam_list,
                    "material_count": entry_count,
                }
            }));
        }

        if let Some(prev) = &prev_active {
            let _ = prev.activate();
        }
        let tried_txt = tried
            .iter()
            .map(|t| {
                format!(
                    "{}: {}",
                    t["path"].as_str().unwrap_or_default(),
                    t["error"].as_str().unwrap_or("null")
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        Ok(json!({"success": false, "message": format!("候选材料目录都打开失败:\n{tried_txt}")}))
    };

    match inner() {
        Ok(v) => v,
        Err(e) => json!({"success": false, "message": e.to_string()}),
    }
}

/// 把材料挂到目标上，返回 (成功名列表, 失败[{name, reason}])。
fn apply_on_targets(
    part: &Part,
    manager: &MaterialManager,
    material: &rscatiacli::objects::Material,
    target: &str,
    link_mode: i32,
) -> ComResult<(Vec<String>, Vec<Value>)> {
    let mut applied: Vec<String> = Vec::new();
    let mut failed: Vec<Value> = Vec::new();

    if target == "bodies" {
        let bodies = part.bodies()?;
        let n = bodies.count()?;
        if n == 0 {
            return Err(rscatiacli::ComError::runtime(
                "当前零件没有 Body, 请改用「PartBody / 整个 Part」模式",
            ));
        }
        for i in 1..=n {
            let Ok(body) = bodies.item(i) else { continue };
            let body_name = body.name().unwrap_or_else(|_| format!("Body.{i}"));
            match manager.apply_material_on_body(&body, material, link_mode) {
                Ok(()) => applied.push(body_name),
                Err(e) => failed.push(json!({"name": body_name, "reason": e.to_string()})),
            }
        }
    } else if target == "partbody" {
        let main_body = part.main_body()?;
        manager.apply_material_on_body(&main_body, material, link_mode)?;
        applied.push(main_body.name().unwrap_or_else(|_| "PartBody".to_string()));
    } else {
        manager.apply_material_on_part(part, material, link_mode)?;
        applied.push(part.name().unwrap_or_default());
    }
    Ok((applied, failed))
}

/// action=apply：按名字找材料并挂到当前 .CATPart。
fn action_apply(params: &Value, progress: Progress<'_>) -> Value {
    let material_name = params
        .get("material")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default();
    if material_name.is_empty() {
        return json!({"success": false, "message": "请填写材料名(可先「加载材料目录」查看可选材料)"});
    }
    let target = params
        .get("target")
        .and_then(Value::as_str)
        .unwrap_or("partbody");
    if !matches!(target, "partbody" | "part" | "bodies") {
        return json!({"success": false, "message": format!("目标无效: {target}(仅支持 partbody / part / bodies)")});
    }
    let link_mode = match params.get("link_mode").and_then(Value::as_i64) {
        Some(v @ 0) | Some(v @ 1) => v as i32,
        _ => 1,
    };
    let explicit = params
        .get("catalog_path")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let bundled_dir = params.get("bundled_dir").and_then(Value::as_str);

    let inner = || -> ComResult<Value> {
        let app = attach_with_guard(progress)?;
        let docs = app.documents()?;
        let system_service = app.system_service().ok();
        let candidates = resolve_catalog_paths(explicit, bundled_dir, system_service.as_ref());
        if candidates.is_empty() {
            return Ok(json!({
                "success": false,
                "message": match explicit {
                    Some(e) => format!("材料目录不存在: {e}\n请填写有效的 .CATMaterial 文件或所在目录"),
                    None => "未自动找到材料目录(.CATMaterial)。\n请在面板中手动填写材料目录文件路径, 或先「加载材料目录」排查".to_string(),
                }
            }));
        }

        // 当前激活文档必须是 CATPart（Python 版 ensure_part_doc 的工程图切换未移植）
        let doc = app.active_document()?;
        let part = doc
            .part_document()
            .part()
            .map_err(|_| rscatiacli::ComError::runtime("请先在 CATIA 中激活一个 CATPart 文档"))?;
        let part_name = part
            .name()
            .unwrap_or_else(|_| doc.name().unwrap_or_default());

        let prev_active = app.active_document().ok();
        let _ = app.set_display_file_alerts(false);

        let mut last_error = String::new();
        for (path, source) in &candidates {
            let cat = match CatalogDoc::acquire(&docs, path) {
                Ok(c) => c,
                Err(e) => {
                    last_error = e.to_string();
                    progress(&format!("{}: {last_error}", path.display()));
                    continue;
                }
            };
            let (hit, all_names) = match find_material(
                &MaterialDocument::from_document(cat.document().clone()),
                material_name,
            ) {
                Ok(v) => v,
                Err(e) => {
                    last_error = e.to_string();
                    progress(&format!("{}: {last_error}", path.display()));
                    continue;
                }
            };
            let Some((mat_obj, family)) = hit else {
                last_error = format!(
                    "目录 {} 中没有材料「{material_name}」",
                    path.file_name()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default()
                );
                progress(&last_error);
                continue;
            };
            let material = rscatiacli::objects::Material::new(mat_obj);

            let manager = MaterialManager::new(part.get_item("CATMatManagerVBExt")?);
            // 原材料/回读校验对象跟随应用目标: part 读 Part 级, partbody/bodies 读主 Body 级
            let read_kind = if target == "part" { "part" } else { "body" };
            let main_body = part.main_body().ok();
            let previous = match target {
                "part" => get_applied_material(system_service.as_ref(), read_kind, &manager, &part),
                _ => main_body.as_ref().and_then(|mb| {
                    get_applied_material(system_service.as_ref(), read_kind, &manager, mb)
                }),
            };

            let apply_result = apply_on_targets(&part, &manager, &material, target, link_mode);
            let (applied, failed) = match apply_result {
                Ok(v) => v,
                Err(e) => {
                    last_error = e.to_string();
                    progress(&format!("{}: {last_error}", path.display()));
                    continue;
                }
            };

            // 尽力回读校验(Evaluate 不可用时只提示, 不判失败 —— Apply 本身未报错即已生效)
            let verified = match target {
                "part" => get_applied_material(system_service.as_ref(), read_kind, &manager, &part),
                _ => main_body.as_ref().and_then(|mb| {
                    get_applied_material(system_service.as_ref(), read_kind, &manager, mb)
                }),
            };
            let verify_note = if target != "bodies" && verified.is_none() {
                "(未能自动回读校验, 可在规格树查看材料节点确认)"
            } else {
                ""
            };

            // 挂完切回零件文档并清选择集, 让视图刷新出材料渲染
            if let Some(prev) = &prev_active {
                let _ = prev.activate();
            }
            if let Ok(sel) = doc.selection() {
                let _ = sel.clear();
            }

            let mode_txt = if link_mode == 1 {
                "链接到目录"
            } else {
                "复制材料值"
            };
            let scope_txt = match target {
                "part" => "整个 Part".to_string(),
                "partbody" => "PartBody".to_string(),
                _ => format!("{} 个 Body", applied.len() + failed.len()),
            };
            let mut message = format!(
                "材料 {} 已挂到 {part_name}({scope_txt}, {mode_txt}){verify_note}",
                material
                    .name()
                    .unwrap_or_else(|_| material_name.to_string())
            );
            if !failed.is_empty() {
                message += &format!(";失败 {} 个", failed.len());
            }
            if link_mode == 1 && *source == "bundled" {
                message += ";链接指向本机内置库副本, 跨机器使用建议改用 FIS 网络目录或复制值模式";
            }
            message += ";请手动保存文档(Ctrl+S)";

            return Ok(json!({
                "success": true,
                "message": message,
                "data": {
                    "material": material.name().unwrap_or_else(|_| material_name.to_string()),
                    "family": family,
                    "catalog": path.display().to_string(),
                    "catalog_source": source,
                    "part": part_name,
                    "target": target,
                    "link_mode": link_mode,
                    "previous_material": previous,
                    "verified_material": verified,
                    "applied": applied,
                    "failed": failed,
                    "applied_count": applied.len(),
                    "failed_count": failed.len(),
                    "catalog_materials": json!(if all_names.is_empty() { Value::Null } else { json!(all_names) }),
                }
            }));
        }

        if let Some(prev) = &prev_active {
            let _ = prev.activate();
        }
        let hint = if last_error.contains("没有材料") {
            "\n可先点「加载材料目录」确认可选材料名"
        } else {
            ""
        };
        Ok(json!({"success": false, "message": format!("{last_error}{hint}")}))
    };

    match inner() {
        Ok(v) => v,
        Err(e) => json!({"success": false, "message": e.to_string()}),
    }
}

// ── 入口 ─────────────────────────────────────────────────────────────

/// 工具主体。**必须在 STA 线程上执行。**
pub fn run(params: &Value, progress: Progress<'_>) -> ComResult<Value> {
    let action = params
        .get("action")
        .and_then(Value::as_str)
        .unwrap_or("apply");
    match action {
        "list" => Ok(action_list(params, progress)),
        "apply" => Ok(action_apply(params, progress)),
        other => Ok(
            json!({"success": false, "message": format!("未知 action: {other}(支持 list / apply)")}),
        ),
    }
}
