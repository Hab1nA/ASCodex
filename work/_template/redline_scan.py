#!/usr/bin/env python3
"""提交物污染红线扫描器（ASCodex solver-guard 红线门的 ZCode/Python 移植）。

扫描 bundle/工作区目录中所有文本文件，找出只可能来自平台反馈或他人解题的
信息（平台分数、attempt id、判官/红队结论、榜单归属、他人做法）。任何提交前
必须全 CLEAN；findings 逐条给出 文件:行:词条 供人工裁决——裁决方式是改写
提交物，而不是放宽词表。

用法:
  python redline_scan.py [DIR] [--term 额外词条 ...]

- DIR 默认当前目录（在 bundle 根运行）
- 每工作区自定义词条: DIR/redline_terms.txt，一行一个（本身不参与扫描）
- 退出码: 0 = CLEAN, 1 = DIRTY
"""
from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

# 移植自 solver-guard/src/lib.rs BUILTIN_REDLINE_TERMS
BUILTIN_TERMS = [
    "attempt id", "attempt_id", "harbor_reward", "trace_score",
    "scoringdetails", "leaderboard", "judge verdict", "red team",
    "red-team", "opponent", "prior score", "high score",
    # trace-contamination-redline 技能零清单的中文等价项
    "判官结论", "判罚", "红队", "满分者", "榜单", " credited owner",
    "对手", "高分选手", "他人做法",
]

SCORE_PATTERNS = [
    ("score value", re.compile(r"harbor[^a-z0-9]{0,3}\d", re.IGNORECASE)),
    ("score assignment", re.compile(r"\bscore\s*[=:]\s*\d", re.IGNORECASE)),
    ("effective score", re.compile(r"\beffective[_ ]score\b", re.IGNORECASE)),
]

# 移植自 solver-guard contains_attempt_number: "attempt" 后跟可选空白/_-再跟数字
ATTEMPT_NUMBER = re.compile(r"attempt[\s_-]*\d", re.IGNORECASE)

SKIP_DIRS = {".git", "__pycache__", "node_modules", ".zcode", ".codex", "diagnostics"}
# 工具源文件与词条表本身含示例词条/变量名，不属提交表面；diagnostics/ 是平台
# 情报区（不进 bundle，见 ascodex-solve §1.5），同样不在红线扫描范围。
SKIP_FILES = {"redline_scan.py", "redline_terms.txt", "trace_check.py",
              "make_traces.py", "submit_bundle.py"}


def load_custom_terms(bundle: Path) -> list[str]:
    terms_file = bundle / "redline_terms.txt"
    if not terms_file.exists():
        return []
    return [line.strip() for line in
            terms_file.read_text(encoding="utf-8").splitlines()
            if line.strip() and not line.startswith("#")]


def iter_text_files(root: Path):
    if root.is_file():
        yield root
        return
    for path in sorted(root.rglob("*")):
        if not path.is_file():
            continue
        if any(part in SKIP_DIRS for part in path.relative_to(root).parts[:-1]):
            continue
        if path.name in SKIP_FILES:
            continue
        yield path


def scan(bundle: Path, extra_terms: list[str]) -> list[str]:
    terms = sorted(set(BUILTIN_TERMS) | {t.lower() for t in extra_terms})
    findings: list[str] = []
    for path in iter_text_files(bundle):
        try:
            text = path.read_text(encoding="utf-8")
        except (UnicodeDecodeError, OSError):
            continue  # 二进制产物不是 transcript 表面，跳过
        lower = text.lower()
        for lineno, line in enumerate(text.splitlines(), 1):
            low_line = line.lower()
            hits = [t for t in terms if t in low_line]
            hits += [name for name, pat in SCORE_PATTERNS if pat.search(line)]
            if ATTEMPT_NUMBER.search(line):
                hits.append("attempt number")
            for hit in sorted(set(hits)):
                findings.append(f"{path}:L{lineno}: {hit}")
    return findings


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("bundle", nargs="?", default=".", type=Path,
                    help="bundle/工作区目录（默认当前目录）")
    ap.add_argument("--term", action="append", default=[],
                    help="追加自定义词条（可多次）")
    args = ap.parse_args()

    bundle = args.bundle.resolve()
    if not bundle.exists():
        print(f"DIRTY: 路径不存在: {bundle}")
        return 1

    findings = scan(bundle, load_custom_terms(bundle) + args.term)
    if findings:
        print(f"DIRTY — {len(findings)} 处红线命中（改写提交物后重扫，不得放宽词表）:")
        for f in findings:
            print(f"  {f}")
        return 1
    print(f"CLEAN — {bundle} 红线扫描通过")
    return 0


if __name__ == "__main__":
    sys.exit(main())
