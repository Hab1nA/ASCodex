#!/usr/bin/env python3
"""Generic trace generator for an ARM bundle.

Adapt CHALLENGE_ID, TITLE and the TRACE_STEPS list to the actual work done.
Keeps the anti-fraud requirements: step_type enum, tool_call/tool_result
pairing, artifact existence, cost lower bound, stdout anchor.
"""
from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent

CHALLENGE_ID = "CHANGE_ME"
SESSION_ID = f"{CHALLENGE_ID}-friday-20260813"
AGENT = {"name": "friday", "version": "1.0.0"}
MODEL = {"provider": "deepseek", "id": "deepseek-v4-flash"}
STARTED = "2026-08-13T00:00:00Z"

TRACE_STEPS: list[dict] = [
    # Adapt these to the actual reproduction steps. Each tool_call must be
    # paired with a tool_result sharing the same tool_call_id; artifact rows
    # must point at files that exist in the bundle; the last tool_result body
    # should be a substring of execution/run.log (stdout anchor).
    {"step_type": "thought", "title": "Read the challenge",
     "body": f"Challenge {CHALLENGE_ID}: read /api/challenges/{CHALLENGE_ID}/content in full.",
     "duration_s": 5.0, "cost_usd": 0.002, "tokens": 100},
    {"step_type": "tool_call", "title": "Fetch statement",
     "tool_call_id": "tc_1", "tool_name": "pwsh",
     "tool_args": {"command": f"curl {CHALLENGE_ID}/content"},
     "duration_s": 1.0, "cost_usd": 0.001, "tokens": 30},
    {"step_type": "tool_result", "title": "Statement",
     "tool_call_id": "tc_1",
     "tool_output": "full statement text here",
     "body": "Statement parsed; output contract noted.", "duration_s": 0.5,
     "cost_usd": 0.0005, "tokens": 50},
    {"step_type": "tool_call", "title": "Run reproduction",
     "tool_call_id": "tc_2", "tool_name": "pwsh",
     "tool_args": {"command": "python src/reproduce.py"},
     "duration_s": 2.0, "cost_usd": 0.001, "tokens": 30},
    {"step_type": "tool_result", "title": "Computed result",
     "tool_call_id": "tc_2",
     "tool_output": "KEY_RESULT_VALUE",
     "body": "KEY_RESULT_VALUE", "duration_s": 0.5, "cost_usd": 0.0005,
     "tokens": 60},
    {"step_type": "artifact", "title": "Outputs",
     "artifact_id": "answer_txt", "artifact_path": "outputs/answer.txt",
     "body": "outputs/answer.txt written.", "duration_s": 0.5,
     "cost_usd": 0.0005, "tokens": 20},
    {"step_type": "decision", "title": "Package bundle",
     "body": "Assembled ARM v1.1 bundle and submitted.",
     "duration_s": 3.0, "cost_usd": 0.002, "tokens": 100},
]


def thinking(text: str) -> dict:
    return {"type": "thinking", "thinking": text}


def text(t: str) -> dict:
    return {"type": "text", "text": t}


def msg(idx: int, turn: int, role: str, content, *, name=None, tool_call_id=None,
        tool_calls=None, model_id=None, tokens_in=None, tokens_out=None,
        provider_raw=None) -> dict:
    m = {
        "attempt_id": None, "msg_idx": idx, "turn_idx": turn, "role": role,
        "content": content, "tool_calls": tool_calls, "tool_call_id": tool_call_id,
        "name": name, "model_id": model_id, "provider": "deepseek",
        "provider_raw": provider_raw, "tokens_in": tokens_in,
        "tokens_out": tokens_out, "cost_usd": None,
        "timestamp": f"2026-08-13T00:{idx // 10}{idx % 10}:00Z",
        "parent_msg_idx": None, "meta": {},
    }
    return m


def main() -> None:
    rows = []
    for i, s in enumerate(TRACE_STEPS):
        row = dict(s)
        row["step_order"] = i + 1
        row["timestamp"] = f"2026-08-13T00:{i // 10}{i % 10}:00Z"
        rows.append(row)
    (ROOT / "trace" / "trace.jsonl").write_text(
        "\n".join(json.dumps(r, ensure_ascii=False) for r in rows) + "\n",
        encoding="utf-8")

    messages = [
        msg(0, 0, "user", [text(f"Solve challenge {CHALLENGE_ID} on the Playground.")],
            tokens_in=30),
        msg(1, 1, "assistant", [
            thinking("Plan: read statement, implement, verify, package ARM bundle, submit."),
            text("Start working."),
        ], model_id=MODEL["id"], tokens_out=80,
            provider_raw={"type": "message", "role": "assistant",
                          "content": [{"type": "text", "text": "Start working."}]}),
    ]
    raw = [
        {"type": "session_start", "schema_version": "raw-v1",
         "session_id": SESSION_ID, "agent": AGENT, "model": MODEL,
         "started_at": STARTED},
    ] + messages + [
        {"type": "session_end", "session_id": SESSION_ID,
         "ended_at": "2026-08-13T00:10:00Z", "termination": "success",
         "n_messages": len(messages), "n_turns": 2},
    ]
    (ROOT / "trace" / "raw_messages.jsonl").write_text(
        "\n".join(json.dumps(m, ensure_ascii=False) for m in raw) + "\n",
        encoding="utf-8")
    (ROOT / "raw_messages.jsonl").write_text(
        "\n".join(json.dumps(m, ensure_ascii=False) for m in raw) + "\n",
        encoding="utf-8")

    meta = {
        "attempt_id": None, "challenge_id": CHALLENGE_ID,
        "agent_runtime": "DeepSeek Harness", "agent_runtime_version": "1.0.0",
        "agent_provider": "deepseek", "model_id": MODEL["id"],
        "outcome_reward": None, "outcome_normalized": None,
        "outcome_reason": "pending platform scoring",
        "n_messages": len(messages), "n_turns": 2, "wall_seconds": 600,
        "total_tokens_in": 30, "total_tokens_out": 80, "total_cost_usd": 0.01,
        "termination": "success", "schema_version": "0.1",
    }
    (ROOT / "trace" / "trajectory_meta.json").write_text(
        json.dumps(meta, indent=2) + "\n", encoding="utf-8")
    print(f"traces written for {CHALLENGE_ID}")


if __name__ == "__main__":
    main()
