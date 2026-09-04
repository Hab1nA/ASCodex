#!/usr/bin/env python3
"""ARM v1.1 bundle submission for one challenge (ZCode single-session track).

Usage:
  python submit_bundle.py --challenge <id> [--dry-run]
                          [--method "..."] [--outcome success|partial|failed|stuck]
                          [--stuck-at "..."] [--skill-ids '["..."]']

Track alignment (bohrium-kb/round3_prep/SCORING_TRUTH.md 方式 B / harbor 轨):
draft attempt is created WITHOUT the `script` field — attaching it switches the
attempt to the bundle/judge track whose scores are not collected by the official
leaderboard. reproduce.py ships inside the bundle zip regardless.

Credentials: PLAYGROUND_TOKEN first, BOHRIUM_TOKEN as documented fallback,
current process environment only. Model/harness provenance defaults to
ASCODEX_MODEL / ASCODEX_HARNESS env ("unspecified" when absent) — never leave
historical defaults in a submission.
"""
from __future__ import annotations

import argparse
import io
import json
import os
import sys
import zipfile
from pathlib import Path

import requests

BASE = os.environ.get("BOHRIUM_BASE", "https://play.bohrium.com/api")
TOKEN = os.environ.get("PLAYGROUND_TOKEN") or os.environ.get("BOHRIUM_TOKEN")

EXCLUDE_FILES = {"submit_bundle.py", "make_traces.py", "redline_scan.py",
                 "trace_check.py", "redline_terms.txt"}
EXCLUDE_DIRS = {"__pycache__", ".git", "diagnostics"}


def build_zip(bundle_dir: Path) -> bytes:
    buf = io.BytesIO()
    with zipfile.ZipFile(buf, "w", zipfile.ZIP_DEFLATED) as z:
        for p in sorted(bundle_dir.rglob("*")):
            if not p.is_file() or p.name.startswith("."):
                continue
            if p.name in EXCLUDE_FILES:
                continue
            rel_parts = set(p.relative_to(bundle_dir).parts[:-1])
            if rel_parts & EXCLUDE_DIRS:
                continue
            z.write(p, p.relative_to(bundle_dir).as_posix())
    return buf.getvalue()


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--challenge", required=True)
    ap.add_argument("--dry-run", action="store_true",
                    help="build the bundle and print its member list; no network, no token needed")
    ap.add_argument("--method", default="Reproduction via ARM v1.1 bundle")
    ap.add_argument("--model", default=None,
                    help="defaults to $ASCODEX_MODEL (else 'unspecified'); must reflect the real session model")
    ap.add_argument("--harness", default=None,
                    help="defaults to $ASCODEX_HARNESS (else 'unspecified')")
    ap.add_argument("--outcome", default="success",
                    choices=["success", "partial", "failed", "stuck"])
    ap.add_argument("--stuck-at", default="")
    ap.add_argument("--skill-ids", default="[]")
    ap.add_argument("--agent-ids", default="[]")
    args = ap.parse_args()

    bundle_dir = Path(__file__).resolve().parent
    trace_path = bundle_dir / "trace" / "trace.jsonl"
    trace_steps = []
    if trace_path.exists():
        trace_steps = [json.loads(line) for line in
                       trace_path.read_text(encoding="utf-8").splitlines() if line.strip()]

    if not trace_steps:
        raise SystemExit(
            "FAIL-CLOSED: trace/trace.jsonl 缺失或为空——无 trace 不入待评队列，"
            "禁止提交。先按 real-trace-capture 转录真实执行并跑 trace_check.py 全绿。")

    bundle = build_zip(bundle_dir)
    members = zipfile.ZipFile(io.BytesIO(bundle)).namelist()
    print(f"bundle members ({len(members)}):")
    for name in members:
        print(f"  {name}")
    print("提醒：arm_manifest.json 的 execution.ran_at/wall_time_s 必须是真实运行时间窗，"
          "trace 时间戳须落在窗内；handoff.status 不接受 'complete'。")

    if args.dry_run:
        print("DRY-RUN OK：未联网、未消耗提交授权。")
        return

    if not TOKEN:
        raise SystemExit("PLAYGROUND_TOKEN（或 BOHRIUM_TOKEN）必须在当前进程环境中；禁止文件回退")

    model = args.model or os.environ.get("ASCODEX_MODEL", "unspecified")
    harness = args.harness or os.environ.get("ASCODEX_HARNESS", "unspecified")
    headers = {"Authorization": f"Bearer {TOKEN}"}

    data = {
        "method": args.method,
        "model": model,
        "harness": harness,
        "type": "agent",
        "status": "draft",
        "outcome": args.outcome,
        "stuck_at": args.stuck_at,
        "skill_ids": args.skill_ids,
        "agent_ids": args.agent_ids,
        "trace": json.dumps(trace_steps),
    }

    print(f"[1/4] creating draft attempt for {args.challenge} (no script field -> harbor track) ...")
    r = requests.post(f"{BASE}/challenges/{args.challenge}/attempts",
                      headers=headers, data=data, timeout=120)
    if r.status_code >= 400:
        print(f"FAILED: {r.status_code} {r.text[:600]}")
        sys.exit(1)
    attempt = r.json()
    aid = attempt["id"]
    print(f"   attempt id = {aid}")

    print("[2/4] uploading ARM bundle ...")
    print(f"   bundle size = {len(bundle)} bytes")
    r = requests.post(f"{BASE}/attempts/{aid}/bundle", headers=headers,
                      files={"bundle": ("bundle.zip", bundle, "application/zip")},
                      timeout=300)
    if r.status_code >= 400:
        print(f"BUNDLE FAILED: {r.status_code} {r.text[:600]}")
        sys.exit(1)
    print(f"   bundle ok: {r.text[:200]}")

    print("[3/4] submitting draft ...")
    r = requests.post(f"{BASE}/attempts/{aid}/submit", headers=headers, timeout=120)
    print(f"   submit: {r.status_code} {r.text[:200]}")

    print("[4/4] triggering scoring ...")
    r = requests.post(f"{BASE}/attempts/{aid}/score", headers=headers, timeout=300)
    print(f"   score: {r.status_code} {r.text[:600]}")

    print(f"\nDONE. attempt_id={aid}")
    print("submitted/queued 不是成功：按 submit-attempt Step 5 只读核实"
          "replay/resultsJson/scorecard/credited owner/fresh rescore。")
    print(f"view: https://play.bohrium.com/#challenge/{args.challenge}/attempts")


if __name__ == "__main__":
    main()
