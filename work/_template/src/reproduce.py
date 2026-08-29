#!/usr/bin/env python3
"""Reproduction entrypoint for challenge CHANGE_ME.

Fill in the actual computation. Contract:
- runnable via `python src/reproduce.py` from the bundle root;
- writes outputs/answer.txt (JSON answer object), outputs/response.md,
  execution/results/tc_result.json (or result.json), results_json.json,
  execution/run.log (stdout captured).
"""
from __future__ import annotations

import json
from pathlib import Path


def compute() -> dict:
    """Return the answer object, e.g. {"Tc_K": 39.35}."""
    raise NotImplementedError("implement compute() for this challenge")


def main() -> int:
    root = Path(__file__).resolve().parents[1]
    (root / "execution" / "results").mkdir(parents=True, exist_ok=True)
    (root / "outputs").mkdir(parents=True, exist_ok=True)

    answer = compute()
    report = {
        "challenge_id": root.name,
        "answer": answer,
        "method": "see REPORT.md",
        "metrics": {},
        "passes": {"answer_file_written": True},
        "artifacts": {"answer": "outputs/answer.txt",
                      "result": "execution/results/result.json",
                      "report": "REPORT.md", "trace": "trace/trace.jsonl"},
        "limitations": [],
    }
    (root / "outputs" / "answer.txt").write_text(
        json.dumps(answer, sort_keys=True) + "\n", encoding="utf-8")
    (root / "outputs" / "response.md").write_text(
        json.dumps(answer, indent=2) + "\n", encoding="utf-8")
    (root / "execution" / "results" / "result.json").write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    (root / "results_json.json").write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(json.dumps(answer, sort_keys=True))
    print("Pipeline complete. Exit code: 0")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
