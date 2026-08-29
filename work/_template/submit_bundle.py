#!/usr/bin/env python3
"""Generic ARM v1.1 bundle submission for one challenge.

Usage: python submit_bundle.py --challenge <id> [--method "..." ] [--outcome success|partial|stuck|failed] [--stuck-at "..."] [--skill-ids '["..."]']

Reads BOHRIUM_TOKEN from the current process only. Assumes the standard
bundle layout in the current directory: src/reproduce.py, outputs/, trace/,
characterization.json, arm_manifest.json, ...
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
TOKEN = os.environ.get("BOHRIUM_TOKEN")


def build_zip(bundle_dir: Path, exclude: set[str]) -> bytes:
    buf = io.BytesIO()
    with zipfile.ZipFile(buf, "w", zipfile.ZIP_DEFLATED) as z:
        for p in sorted(bundle_dir.rglob("*")):
            if p.is_file() and p.name not in exclude:
                z.write(p, p.relative_to(bundle_dir).as_posix())
    return buf.getvalue()


def main() -> None:
    if not TOKEN:
        raise SystemExit("BOHRIUM_TOKEN is required in the current process; no credential fallback is permitted")
    headers = {"Authorization": f"Bearer {TOKEN}"}
    ap = argparse.ArgumentParser()
    ap.add_argument("--challenge", required=True)
    ap.add_argument("--method", default="Reproduction via ARM v1.1 bundle")
    ap.add_argument("--model", default="DeepSeek-V4")
    ap.add_argument("--harness", default="DeepSeek Harness")
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
                       trace_path.read_text(encoding="utf-8").splitlines()]

    data = {
        "method": args.method,
        "model": args.model,
        "harness": args.harness,
        "type": "agent",
        "status": "draft",
        "outcome": args.outcome,
        "stuck_at": args.stuck_at,
        "skill_ids": args.skill_ids,
        "agent_ids": args.agent_ids,
        "trace": json.dumps(trace_steps),
    }
    files = {}
    if (bundle_dir / "src" / "reproduce.py").exists():
        files["script"] = ("reproduce.py",
                           (bundle_dir / "src" / "reproduce.py").open("rb"))

    print(f"[1/4] creating draft attempt for {args.challenge} ...")
    r = requests.post(f"{BASE}/challenges/{args.challenge}/attempts",
                      headers=headers, data=data, files=files or None, timeout=120)
    if r.status_code >= 400:
        print(f"FAILED: {r.status_code} {r.text[:600]}")
        sys.exit(1)
    attempt = r.json()
    aid = attempt["id"]
    print(f"   attempt id = {aid}")

    print("[2/4] uploading ARM bundle ...")
    bundle = build_zip(bundle_dir, {"submit_bundle.py", "make_traces.py"})
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
    print(f"view: https://play.bohrium.com/#challenge/{args.challenge}/attempts")


if __name__ == "__main__":
    main()
