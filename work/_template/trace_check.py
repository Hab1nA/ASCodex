#!/usr/bin/env python3
"""trace.jsonl 校验器（real-trace-capture 13 条提交前校验清单的脚本化）。

只校验、不生成：trace 必须先从真实执行记录转录（ZCode 会话记录 / run.log），
脚本合成 trace 违反污染红线，任何生成器产物都过不了本检查与提交门。

用法:
  python trace_check.py <trace.jsonl> --run-log <run.log> [--root <bundle根>]

退出码: 0 = PASS, 1 = FAIL。
"""
from __future__ import annotations

import argparse
import json
import re
import sys
from datetime import datetime
from pathlib import Path

STEP_TYPES = {"thought", "tool_call", "tool_result", "artifact",
              "decision", "error", "observation"}

# real-trace-capture 校验清单第 1 条：论文引用应为空
PAPER_CITATION = re.compile(r"Maliar|Paper \[|Table \d|Equation \(|et al\.")

# 校验清单第 11 条：artifacts 只列业务产物，禁止列证据文件本身
EVIDENCE_FILES = {"trace.jsonl", "run.log", "artifacts.json",
                  "execution.json", "channel-probe.json"}


def parse_ts(value: str) -> datetime:
    return datetime.fromisoformat(str(value).replace("Z", "+00:00"))


def check(trace_path: Path, run_log_path: Path | None, root: Path):
    errors: list[str] = []
    warnings: list[str] = []

    try:
        raw_lines = trace_path.read_text(encoding="utf-8").splitlines()
    except OSError as e:
        return [f"无法读取 trace: {e}"], []

    rows: list[dict] = []
    for lineno, line in enumerate(raw_lines, 1):
        if not line.strip():
            continue
        try:
            row = json.loads(line)
        except json.JSONDecodeError as e:
            errors.append(f"L{lineno}: 不是合法 JSON: {e}")
            continue
        if not isinstance(row, dict):
            errors.append(f"L{lineno}: 行不是 JSON 对象")
            continue
        rows.append((lineno, row))  # type: ignore[arg-type]

    seen_ids: set[str] = set()
    call_ids: set[str] = set()
    result_ids: set[str] = set()
    thought_bodies: list[str] = []
    anchored_bodies = 0
    prev_ts = None
    expect_result_for: str | None = None
    cost_total = 0.0
    full_text = json.dumps([r for _, r in rows], ensure_ascii=False)

    for i, (lineno, row) in enumerate(rows):
        loc = f"L{lineno}"
        # --- 通用字段 ---
        for field in ("step_order", "step_id", "step_type", "timestamp",
                      "duration_s", "cost_usd", "tokens"):
            if field not in row:
                errors.append(f"{loc}: 缺字段 {field}")
        if row.get("step_order") != i + 1:
            errors.append(f"{loc}: step_order 应为 {i + 1}，实际 {row.get('step_order')}")
        sid = row.get("step_id")
        if sid in seen_ids:
            errors.append(f"{loc}: step_id 重复: {sid}")
        seen_ids.add(sid)
        if row.get("step_type") not in STEP_TYPES:
            errors.append(f"{loc}: step_type 非法: {row.get('step_type')}")
        for field in ("duration_s", "cost_usd", "tokens"):
            v = row.get(field)
            if isinstance(v, (int, float)) and not isinstance(v, bool) and v >= 0:
                continue
            errors.append(f"{loc}: {field} 必须是非负数字，实际 {v!r}")
        try:
            ts = parse_ts(row["timestamp"])
            if prev_ts is not None and ts < prev_ts:
                errors.append(f"{loc}: timestamp 递减")
            prev_ts = ts
        except (KeyError, ValueError, TypeError) as e:
            errors.append(f"{loc}: timestamp 非 RFC3339: {row.get('timestamp')!r} ({e})")
        cost_total += row.get("cost_usd") if isinstance(row.get("cost_usd"), (int, float)) else 0

        stype = row.get("step_type")

        # --- thought ---
        if stype == "thought":
            body = row.get("body")
            if not isinstance(body, str):
                errors.append(f"{loc}: thought.body 必须是字符串")
            else:
                thought_bodies.append(body)
                if len(body) < 80:
                    errors.append(f"{loc}: thought.body <80 字符 ({len(body)})")

        # --- tool_call / tool_result 1:1 紧邻配对 ---
        if stype == "tool_call":
            if expect_result_for is not None:
                errors.append(f"{loc}: tool_call {expect_result_for} 的 tool_result 缺失")
            tc = row.get("tool_call_id")
            if not tc:
                errors.append(f"{loc}: tool_call 缺 tool_call_id")
            call_ids.add(tc)
            expect_result_for = tc
            if not isinstance(row.get("tool_args"), dict) or not row.get("tool_name"):
                errors.append(f"{loc}: tool_call 需含 tool_name 与 tool_args 对象")
        elif stype == "tool_result":
            tc = row.get("tool_call_id")
            result_ids.add(tc)
            if expect_result_for is None:
                errors.append(f"{loc}: tool_result 无前导 tool_call")
            elif tc != expect_result_for:
                errors.append(f"{loc}: tool_result.tool_call_id={tc!r} 与前导 {expect_result_for!r} 不一致")
            expect_result_for = None
            body = row.get("body")
            if not isinstance(body, str):
                errors.append(f"{loc}: tool_result.body 必须是字符串（真实 stdout 文本）")
            elif len(body) >= 16:
                log_text = run_log_path.read_text(encoding="utf-8", errors="replace") \
                    if run_log_path and run_log_path.exists() else ""
                if (body in log_text) or (body.strip() and body.strip() in log_text):
                    anchored_bodies += 1
                else:
                    warnings.append(f"{loc}: ≥16 字符 tool_result.body 未锚定在 run.log")

        # --- artifact 行 ---
        if stype == "artifact":
            apath = row.get("artifact_path")
            if not apath:
                errors.append(f"{loc}: artifact 行缺 artifact_path")
            elif Path(apath).name in EVIDENCE_FILES:
                errors.append(f"{loc}: artifact 不得列证据文件本身: {apath}")
            elif not (root / apath).exists():
                errors.append(f"{loc}: artifact 文件不存在: {apath}")

    if expect_result_for is not None:
        errors.append(f"结尾: tool_call {expect_result_for} 的 tool_result 缺失")
    if call_ids != result_ids:
        errors.append(f"tool_call/tool_result 集合不一致: 缺 result 的={sorted(call_ids - result_ids)}"
                      f" 缺 call 的={sorted(result_ids - call_ids)}")
    if len([b for b in thought_bodies if len(b) >= 80]) < 3:
        errors.append(f"≥80 字符 thought 不足 3 条（实际 {len([b for b in thought_bodies if len(b) >= 80])}）")
    if cost_total < 0.01:
        errors.append(f"cost_usd 总和 {cost_total:.4f} < 0.01")
    if run_log_path and not run_log_path.exists():
        errors.append(f"run.log 不存在: {run_log_path}")
    if anchored_bodies < 1:
        errors.append("没有一条 ≥16 字符 tool_result.body 能锚定在 run.log（禁止合成证据）")
    if PAPER_CITATION.search(full_text):
        errors.append("trace 含论文引用模式（Maliar/Paper[/Table N/Equation(/et al.）——全部删除")
    if thought_bodies and "answer" in thought_bodies[0].lower()[:120] and len(thought_bodies[0]) > 0:
        warnings.append("首条 thought 疑似含结论（answer 字样）——人工确认首条是过程叙述")

    return errors, warnings


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("trace", type=Path)
    ap.add_argument("--run-log", type=Path, default=None)
    ap.add_argument("--root", type=Path, default=Path("."),
                    help="bundle 根（artifact 相对路径的基准，默认当前目录）")
    args = ap.parse_args()

    errors, warnings = check(args.trace, args.run_log, args.root.resolve())
    for w in warnings:
        print(f"WARN: {w}")
    if errors:
        print(f"FAIL — {len(errors)} 处不通过:")
        for e in errors:
            print(f"  {e}")
        return 1
    print(f"PASS — {args.trace} 满足 real-trace-capture 提交前校验"
          f"（{len(warnings)} 条警告需人工确认）")
    return 0


if __name__ == "__main__":
    sys.exit(main())
