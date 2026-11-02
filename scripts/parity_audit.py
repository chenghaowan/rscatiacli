#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""pycatia -> rscatiacli 翻译进度审计。

扫描 pycatia 源码包（模块族 -> 类清单）与 rscatiacli 的 `objects/mod.rs`
导出面，按类粒度对齐，产出 docs/index.html 进度页所需的数据块。

口径:
- 类: pycatia 各模块目录下非 `__init__.py` 文件中的顶层 `class X` 定义，跨族去重
      （同一类以字母序最先定义的族为准）。
- 功能范围(分母): 14 个规划内模块族，剔除 `*SettingAtt` 设置面板类与 COM
      基础设施类（IDispatch/IUnknown/AnyObject/Collection 等），枚举常量族
      (enumeration) 单列。
- 已封装: rscatiacli 有同名类型化包装；少量 COM 惯用名与 pycatia 命名不一致的
      通过 ALIAS 映射（如 VisProperties <-> VisPropertySet）。

用法:
  python scripts/parity_audit.py                     # 打印摘要
  python scripts/parity_audit.py --write index.html  # 回写 docs/index.html 数据块
  PYCATIA_PATH=<site-packages>/pycatia 可覆盖默认 pycatia 路径
"""
from __future__ import annotations

import argparse
import datetime
import json
import os
import re
import sys

DEFAULT_PYCATIA = r"D:\claw.forvia\ltl-manage\frontR\.venv\Lib\site-packages\pycatia"
REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
MOD_RS = os.path.join(REPO, "crates", "rscatiacli", "src", "objects", "mod.rs")
INDEX_HTML = os.path.join(REPO, "docs", "index.html")

# rscatiacli 包装名 -> pycatia 类名（命名差异白名单）
ALIAS = {
    "VisProperties": "VisPropertySet",
    "SelectionItem": "SelectedElement",
    "Viewpoint3D": "ViewPoint3D",
    "Extract": "HybridShapeExtract",
}

# 规划内的 pycatia 模块族（枚举常量族单独统计）
SCOPE_FAMILIES = [
    ("product_structure_interfaces", "产品结构"),
    ("part_interfaces", "零件特征（Part Design 特征/布尔/阵列）"),
    ("mec_mod_interfaces", "机械模型（实体体/几何集/拓扑）"),
    ("sketcher_interfaces", "草绘器"),
    ("in_interfaces", "文档 / 会话 / 文件 / 相机"),
    ("system_interfaces", "系统服务与设置"),
    ("space_analyses_interfaces", "空间分析（量测/公差/断面）"),
    ("knowledge_interfaces", "知识工程（参数/关系/公式）"),
    ("drafting_interfaces", "制图（2D 图纸）"),
    ("cat_mat_interfaces", "材料库"),
    ("cat_tps_interfaces", "3D 标注（FTA / TPS）"),
    ("navigator_interfaces", "导航器（组 / DMU 标注）"),
    ("hybrid_shape_interfaces", "创成式外形设计（GSD）"),
]

# COM 基础设施类：永不包装，也不计入分母
INFRA_CLASSES = {
    "AnyObject", "Collection", "IDispatch", "IUnknown",
    "CATBaseDispatch", "CATBaseUnknown",
    "SettingController", "SettingRepository", "SettingControllers",
    "Workbench",
}


def scan_pycatia(pycatia_dir: str) -> dict[str, list[str]]:
    """返回 {模块族: [类名, ...]}，类名跨族去重（首见族保留）。"""
    fams: dict[str, set[str]] = {}
    seen: dict[str, str] = {}
    for d in sorted(os.listdir(pycatia_dir)):
        fdir = os.path.join(pycatia_dir, d)
        if not os.path.isdir(fdir) or d in ("__pycache__", "types"):
            continue
        names: set[str] = set()
        for f in sorted(os.listdir(fdir)):
            if not f.endswith(".py") or f == "__init__.py":
                continue
            with open(os.path.join(fdir, f), encoding="utf-8", errors="ignore") as fh:
                for line in fh:
                    m = re.match(r"^class (\w+)", line)
                    if m and m.group(1) not in seen:
                        seen[m.group(1)] = d
                        names.add(m.group(1))
        if names:
            fams[d] = names
    # 二次遍历：把只在后续族出现的类归还给首个定义族（上面已通过 seen 保证）
    return {d: sorted(ns) for d, ns in fams.items()}


def read_exports(mod_rs: str) -> set[str]:
    """从 objects/mod.rs 的 pub use 导出面提取类型名（跨行块兼容）。"""
    with open(mod_rs, encoding="utf-8") as fh:
        text = fh.read()
    exports: set[str] = set()
    for block in re.findall(r"pub use\b[^;]+;", text, re.S):
        for tok in re.findall(r"\b[A-Z][A-Za-z0-9_]*\b", block):
            if not tok.startswith("CAT_"):
                exports.add(tok)
    return exports


def functional(names: list[str]) -> list[str]:
    """剔除设置面板类与 COM 基础设施类后的功能类。"""
    return [n for n in names if not n.endswith("SettingAtt") and n not in INFRA_CLASSES]


def build_data(pycatia_dir: str) -> dict:
    fams = scan_pycatia(pycatia_dir)
    exports = read_exports(MOD_RS)

    pycatia_owner: dict[str, str] = {}
    for fam in sorted(fams):
        for n in fams[fam]:
            pycatia_owner.setdefault(n, fam)

    covered_by_fam: dict[str, list[str]] = {}
    extras: list[str] = []
    for e in sorted(exports):
        target = ALIAS.get(e, e)
        owner = pycatia_owner.get(target)
        if owner:
            covered_by_fam.setdefault(owner, []).append(e)
        else:
            extras.append(e)

    families = []
    scope_total = covered_total = raw_scope_total = 0
    for fam, zh in SCOPE_FAMILIES:
        names = fams.get(fam, [])
        cov = covered_by_fam.get(fam, [])
        func = functional(names)
        missing = [n for n in names if n not in cov]
        missing_out = [n for n in missing if n not in func]
        missing_in = [n for n in missing if n in func]
        families.append({
            "module": fam,
            "zh": zh,
            "raw_total": len(names),
            "functional_total": len(func),
            "covered": cov,
            "missing_in_scope": missing_in,
            "excluded": sorted(missing_out),
        })
        scope_total += len(func)
        covered_total += len(cov)
        raw_scope_total += len(names)

    enums = fams.get("enumeration", [])
    consts = sorted(
        m
        for root, _dirs, files in os.walk(os.path.dirname(MOD_RS))
        for f in files if f.endswith(".rs")
        for m in re.findall(
            r"pub const (CAT_[A-Z0-9_]+|PAPER_[A-Z0-9_]+)",
            open(os.path.join(root, f), encoding="utf-8", errors="ignore").read(),
        )
    )

    out_scope = []
    for fam in sorted(fams):
        if fam in dict(SCOPE_FAMILIES) or fam == "enumeration":
            continue
        out_scope.append({"module": fam, "classes": len(fams[fam])})

    return {
        "generated": datetime.date.today().isoformat(),
        "pycatia_version": read_pycatia_version(pycatia_dir),
        "rscatiacli_version": read_crate_version(),
        "totals": {
            "pycatia_all_classes": sum(len(v) for v in fams.values()),
            "pycatia_families": len(fams),
            "scope_raw_classes": raw_scope_total,
            "scope_functional_classes": scope_total,
            "covered": covered_total,
            "percent": round(covered_total * 100.0 / scope_total, 1) if scope_total else 0.0,
            "enum_pool": len(enums),
            "rust_constants": len(consts),
        },
        "families": families,
        "enumeration": {"total": len(enums)},
        "extras": extras,  # rscatiacli 有而 pycatia 无同名类的扩展封装
        "out_of_scope": out_scope,
    }


def read_pycatia_version(pycatia_dir: str) -> str:
    vp = os.path.join(pycatia_dir, "version.py")
    try:
        with open(vp, encoding="utf-8") as fh:
            m = re.search(r'version\s*=\s*"([^"]+)"', fh.read())
        return m.group(1) if m else "unknown"
    except OSError:
        return "unknown"


def read_crate_version() -> str:
    cargo = os.path.join(REPO, "Cargo.toml")
    with open(cargo, encoding="utf-8") as fh:
        m = re.search(r'^version\s*=\s*"([^"]+)"', fh.read(), re.M)
    return m.group(1) if m else "0.0.1"


BEGIN, END = "<!--PARITY-DATA:BEGIN-->", "<!--PARITY-DATA:END-->"


def write_index(data: dict, path: str) -> None:
    with open(path, encoding="utf-8") as fh:
        html = fh.read()
    block = f"{BEGIN}\nwindow.__PARITY_DATA__ = {json.dumps(data, ensure_ascii=False, indent=2)};\n{END}"
    if BEGIN in html and END in html:
        pre, rest = html.split(BEGIN, 1)
        _, post = rest.split(END, 1)
        html = pre + block + post
    else:
        sys.exit(f"{path} 中找不到 {BEGIN} ... {END} 标记")
    with open(path, "w", encoding="utf-8", newline="\n") as fh:
        fh.write(html)
    print(f"已回写 {path}")


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--pycatia", default=os.environ.get("PYCATIA_PATH", DEFAULT_PYCATIA))
    ap.add_argument("--write", metavar="INDEX_HTML", default=None,
                    help="把数据回写到 docs/index.html 的标记块")
    args = ap.parse_args()

    data = build_data(args.pycatia)
    t = data["totals"]
    print(f"pycatia {data['pycatia_version']}: {t['pycatia_all_classes']} 类 / {t['pycatia_families']} 族")
    print(f"规划范围功能类: {t['covered']}/{t['scope_functional_classes']} = {t['percent']}%"
          f"（原始 {t['scope_raw_classes']}，另含 {t['enum_pool']} 个枚举常量按需引入，Rust 常量 {t['rust_constants']}）")
    for fam in data["families"]:
        pct = round(len(fam["covered"]) * 100.0 / fam["functional_total"], 1) if fam["functional_total"] else 0.0
        print(f"  {fam['module']:<32} {len(fam['covered']):>3}/{fam['functional_total']:<3} {pct:>5}%  {fam['zh']}")
    if args.write:
        write_index(data, args.write)


if __name__ == "__main__":
    main()
