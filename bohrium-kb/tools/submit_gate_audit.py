#!/usr/bin/env python3
"""Run the six Playground submission gates locally without network access.

The input is a JSON object with channel, identity, cadence, redline, trace,
and model sections. The command only reads the input and listed artifacts. It
never calls the Playground API and always behaves as dry-run.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


DEFAULT_BANNED_PATTERNS = (
    r"\b(?:attempt|trace_score|harbor_reward|scorecard|judge|red[ -]?team)\b",
    r"\b(?:prior|previous|last)\s+(?:attempt|score|submission)\b",
    r"\b(?:team|competitor|leaderboard|榜单|判官|分数|attempt)\b",
)
ALLOWED_CHANNELS = {"harbor_track", "harbor_only", "cli_no_script"}
ALLOWED_MODELS = {"gpt-5.6-luna", "gpt-5.6-sol", "gpt-5.6-terra", "DeepSeek-V4-Flash"}


def parse_timestamp(value: Any) -> datetime | None:
    if not isinstance(value, str) or not value:
        return None
    normalized = value.replace("Z", "+00:00")
    try:
        parsed = datetime.fromisoformat(normalized)
    except ValueError:
        return None
    if parsed.tzinfo is None:
        return parsed.replace(tzinfo=timezone.utc)
    return parsed.astimezone(timezone.utc)


def result(name: str, passed: bool, detail: str) -> dict[str, Any]:
    return {"name": name, "passed": passed, "detail": detail}


def audit_channel(payload: dict[str, Any]) -> dict[str, Any]:
    channel_section = payload.get("channel")
    if isinstance(channel_section, dict):
        channel = channel_section.get("track", channel_section.get("form"))
        script = bool(channel_section.get("script_present", False))
        official = channel_section.get("expected_official", True)
    else:
        channel = channel_section
        script = bool(payload.get("script", False))
        official = True
    passed = channel in ALLOWED_CHANNELS and not script and official is not False
    detail = "harbor channel without script" if passed else "channel must be harbor-only and script=false"
    return result("channel", passed, detail)


def audit_identity(payload: dict[str, Any]) -> dict[str, Any]:
    identity = payload.get("identity")
    if not isinstance(identity, dict):
        return result("identity", False, "identity object is required")
    name = identity.get("name")
    status = identity.get("status")
    remaining = identity.get("remaining")
    passed = bool(name) and status == "ACTIVE" and isinstance(remaining, int) and remaining > 0
    detail = "active identity has remaining quota" if passed else "identity must be named, ACTIVE, and have remaining quota"
    return result("identity", passed, detail)


def audit_cadence(payload: dict[str, Any], now: datetime) -> dict[str, Any]:
    cadence = payload.get("cadence")
    if not isinstance(cadence, dict):
        return result("cadence", False, "cadence object is required")
    timestamps = cadence.get("last_submits", [])
    if not isinstance(timestamps, list):
        return result("cadence", False, "last_submits must be a list")
    parsed_times = [parse_timestamp(value) for value in timestamps]
    if any(value is None for value in parsed_times):
        return result("cadence", False, "all last_submits values must be ISO timestamps")
    recent = [value for value in parsed_times if value and (now - value).total_seconds() < 600]
    same_content = cadence.get("same_content_seconds", 0)
    content_hash = cadence.get("content_sha256")
    prior_hash = cadence.get("last_content_sha256")
    duplicate_too_soon = bool(content_hash and content_hash == prior_hash and same_content < 3600)
    passed = len(recent) < 2 and not duplicate_too_soon
    detail = "minimum interval and burst limits satisfied" if passed else "cadence or same-content interval is violated"
    return result("cadence", passed, detail)


def audit_redline(payload: dict[str, Any], root: Path) -> dict[str, Any]:
    redline = payload.get("redline")
    if not isinstance(redline, dict):
        return result("redline", False, "redline object is required")
    artifact_names = redline.get("artifacts", [])
    if not isinstance(artifact_names, list) or not artifact_names:
        return result("redline", False, "redline.artifacts must list submission files")
    patterns = [re.compile(pattern, re.IGNORECASE) for pattern in redline.get("patterns", DEFAULT_BANNED_PATTERNS)]
    for artifact_name in artifact_names:
        artifact_path = (root / artifact_name).resolve()
        if root.resolve() not in artifact_path.parents and artifact_path != root.resolve():
            return result("redline", False, f"artifact escapes audit root: {artifact_name}")
        if not artifact_path.is_file():
            return result("redline", False, f"artifact is missing: {artifact_name}")
        try:
            content = artifact_path.read_text(encoding="utf-8")
        except UnicodeDecodeError:
            continue
        for pattern in patterns:
            if pattern.search(content):
                return result("redline", False, f"banned pattern in {artifact_name}: {pattern.pattern}")
    return result("redline", True, "all listed text artifacts are clean")


def audit_trace(payload: dict[str, Any], root: Path) -> dict[str, Any]:
    trace = payload.get("trace")
    if not isinstance(trace, dict):
        return result("trace", False, "trace object is required")
    if trace.get("real_execution") is not True:
        return result("trace", False, "trace.real_execution must be true")
    predicted_score = trace.get("predicted_score")
    if predicted_score is not None and (not isinstance(predicted_score, (int, float)) or predicted_score < 70):
        return result("trace", False, "predicted trace score is below the current full-credit gate")
    cost = trace.get("cost_usd")
    if cost is not None and (not isinstance(cost, (int, float)) or cost < 0.01):
        return result("trace", False, "trace cost_usd is below the minimum real-execution signal")
    trace_path = (root / str(trace.get("path", ""))).resolve()
    if root.resolve() not in trace_path.parents or not trace_path.is_file():
        return result("trace", False, "trace path is missing or outside audit root")
    provenance = trace.get("provenance_paths", [])
    if not isinstance(provenance, list) or not provenance:
        return result("trace", False, "trace provenance_paths must be non-empty")
    for provenance_name in provenance:
        provenance_path = (root / str(provenance_name)).resolve()
        if root.resolve() not in provenance_path.parents or not provenance_path.is_file():
            return result("trace", False, f"missing provenance artifact: {provenance_name}")
    calls: set[str] = set()
    results: set[str] = set()
    timestamps: list[datetime] = []
    try:
        lines = trace_path.read_text(encoding="utf-8").splitlines()
        events = [json.loads(line) for line in lines if line.strip()]
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        return result("trace", False, f"trace is not valid JSONL: {error}")
    if not events:
        return result("trace", False, "trace must contain events")
    for event in events:
        if not isinstance(event, dict):
            return result("trace", False, "trace events must be objects")
        event_type = event.get("step_type", event.get("type"))
        event_id = event.get("tool_call_id", event.get("id"))
        if event_type in {"tool_call", "call"} and event_id:
            calls.add(str(event_id))
        elif event_type in {"tool_result", "result"} and event_id:
            if not event.get("body") and not event.get("stdout"):
                return result("trace", False, "each tool_result must include body/stdout")
            results.add(str(event_id))
        timestamp = parse_timestamp(event.get("timestamp", event.get("created_at")))
        if timestamp is None:
            return result("trace", False, "each trace event needs an ISO timestamp")
        timestamps.append(timestamp)
    if calls != results:
        return result("trace", False, "tool_call and tool_result ids are not one-to-one")
    if timestamps != sorted(timestamps):
        return result("trace", False, "trace timestamps are not monotonic")
    return result("trace", True, f"real trace validated with {len(events)} events")


def audit_model(payload: dict[str, Any]) -> dict[str, Any]:
    model = payload.get("model")
    if not isinstance(model, dict):
        return result("model", False, "model object is required")
    model_name = model.get("name")
    effort = model.get("reasoning_effort")
    passed = model_name in ALLOWED_MODELS and effort in {"max", "xhigh", "high"}
    detail = "model and reasoning effort are permitted" if passed else "model/effort is not in the approved set"
    return result("model", passed, detail)


def load_payload(path: Path) -> dict[str, Any]:
    payload = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(payload, dict):
        raise ValueError("audit input must be a JSON object")
    return payload


def main() -> int:
    parser = argparse.ArgumentParser(description="Local dry-run audit for six Playground submission gates")
    parser.add_argument("input", type=Path, help="JSON audit description")
    parser.add_argument("--root", type=Path, default=Path.cwd(), help="root for relative artifacts")
    parser.add_argument("--output", type=Path, help="optional JSON report path")
    args = parser.parse_args()
    try:
        payload = load_payload(args.input)
        now = parse_timestamp(payload.get("now")) or datetime.now(timezone.utc)
        root = args.root.resolve()
        gates = [
            audit_channel(payload),
            audit_identity(payload),
            audit_cadence(payload, now),
            audit_redline(payload, root),
            audit_trace(payload, root),
            audit_model(payload),
        ]
    except (OSError, ValueError, json.JSONDecodeError) as error:
        report = {
            "dry_run": True,
            "network_used": False,
            "network_write_attempted": False,
            "passed": False,
            "decision": "blocked",
            "error": str(error),
            "gates": [],
        }
        print(json.dumps(report, ensure_ascii=False, indent=2))
        return 2
    requested_write = bool(payload.get("requested_write", False))
    report = {
        "dry_run": True,
        "network_used": False,
        "network_write_attempted": False,
        "passed": all(gate["passed"] for gate in gates) and not requested_write,
        "decision": "write_not_permitted" if requested_write else ("dry_run_pass" if all(gate["passed"] for gate in gates) else "blocked"),
        "content_sha256": hashlib.sha256(args.input.read_bytes()).hexdigest(),
        "gates": gates,
    }
    rendered = json.dumps(report, ensure_ascii=False, indent=2)
    print(rendered)
    if args.output:
        output_path = args.output.resolve()
        if root not in output_path.parents and output_path != root:
            print("output path must remain inside --root", file=sys.stderr)
            return 2
        output_path.write_text(rendered + "\n", encoding="utf-8")
    return 0 if report["passed"] else 2


if __name__ == "__main__":
    sys.exit(main())
