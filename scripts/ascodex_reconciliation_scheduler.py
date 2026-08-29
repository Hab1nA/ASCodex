#!/usr/bin/env python3
"""Run bounded, sequential read-only reconciliation cycles with a durable cursor.

The scheduler composes the audited single-cycle runner.  It never writes to the platform and
never submits.  A successful local ledger batch advances a persistent cursor/event-version
pair; failures leave those authoritative values unchanged and stop the loop.  When facts are
applied, the scheduler writes a deterministic, typed Chief wake request.  It does not start,
resume, or inject into a Chief process.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
import time
from pathlib import Path
from typing import Any

from scripts.ascodex_platform_client import safe_error_message, write_atomic_bytes
from scripts.ascodex_reconciliation_runner import _load_monitor_context, run_cycle


SCHEMA_VERSION = "ascodex-reconciliation-scheduler/v1"
WAKE_SCHEMA_VERSION = "ascodex-chief-wake-request/v1"
MAX_UINT63 = (1 << 63) - 1


def _absolute_required(value: Path, name: str) -> Path:
    resolved = value.expanduser().resolve()
    if not resolved.is_absolute():
        raise ValueError(f"{name} must be an absolute path")
    return resolved


def _json_bytes(value: Any) -> bytes:
    return (
        json.dumps(value, ensure_ascii=True, sort_keys=True, indent=2) + "\n"
    ).encode("utf-8")


def _load_state(path: Path) -> dict[str, Any] | None:
    if not path.exists():
        return None
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise ValueError("scheduler state must be a JSON object")
    return value


def _validate_state(
    state: dict[str, Any],
    *,
    challenge_id: str,
    route: str,
    campaign_id: str,
    monitor_context_sha256: str,
) -> None:
    if state.get("schema_version") != SCHEMA_VERSION:
        raise ValueError("scheduler state schema is unsupported")
    if state.get("challenge_id") != challenge_id:
        raise ValueError("scheduler state is bound to another challenge")
    if state.get("route") != route:
        raise ValueError("scheduler state is bound to another route")
    if state.get("campaign_id") != campaign_id:
        raise ValueError("scheduler state is bound to another campaign")
    if state.get("monitor_context_sha256") != monitor_context_sha256:
        raise ValueError("monitor context changed; rotate the scheduler state explicitly")
    cursor = state.get("cursor_position")
    version = state.get("event_version")
    last_wake = state.get("last_wake_event_version")
    cycles = state.get("cycles_completed")
    if (
        not isinstance(cursor, int)
        or isinstance(cursor, bool)
        or not 0 <= cursor <= MAX_UINT63
        or not isinstance(version, int)
        or isinstance(version, bool)
        or not 0 <= version <= MAX_UINT63
        or not isinstance(last_wake, int)
        or isinstance(last_wake, bool)
        or not 0 <= last_wake <= MAX_UINT63
        or last_wake > version
        or not isinstance(cycles, int)
        or isinstance(cycles, bool)
        or cycles < 0
    ):
        raise ValueError("scheduler state counters are invalid")


def _new_state(
    *,
    challenge_id: str,
    route: str,
    campaign_id: str,
    monitor_context_sha256: str,
    cursor_position: int,
    event_version: int,
    now_ms: int,
) -> dict[str, Any]:
    return {
        "schema_version": SCHEMA_VERSION,
        "challenge_id": challenge_id,
        "route": route,
        "campaign_id": campaign_id,
        "monitor_context_sha256": monitor_context_sha256,
        "cursor_position": cursor_position,
        "event_version": event_version,
        "last_wake_event_version": 0,
        "cycles_completed": 0,
        "updated_at_ms": now_ms,
        "last_status": None,
        "last_summary_path": None,
        "last_error": None,
    }


def _next_cursor(cursor: int, item_count: int) -> int:
    next_cursor = cursor + max(1, item_count)
    if next_cursor > MAX_UINT63:
        raise ValueError("scheduler cursor overflow")
    return next_cursor


def _write_chief_wake(
    *,
    output_dir: Path,
    summary: dict[str, Any],
    campaign_id: str,
    challenge_id: str,
    stream_id: str,
    event_version: int,
) -> Path:
    response_sha256 = summary["response_sha256"]
    wake_id_material = (
        f"{campaign_id}\0{challenge_id}\0{stream_id}\0{event_version}\0{response_sha256}"
    )
    wake_id = hashlib.sha256(wake_id_material.encode("utf-8")).hexdigest()[:16]
    wake = {
        "schema_version": WAKE_SCHEMA_VERSION,
        "wake_id": wake_id,
        "campaign_id": campaign_id,
        "challenge_id": challenge_id,
        "stream_id": stream_id,
        "event_version": event_version,
        "cursor_position": summary["cursor_position"],
        "response_sha256": response_sha256,
        "summary_sha256": hashlib.sha256(
            _json_bytes({key: value for key, value in summary.items() if key != "summary_path"})
        ).hexdigest(),
        "summary_path": summary.get("summary_path"),
        "reason": "platform_reconciliation_applied",
        "platform_write_attempted": False,
    }
    path = output_dir / "wakes" / f"chief-{wake_id}.json"
    write_atomic_bytes(path, _json_bytes(wake))
    return path


def run_scheduled_cycles(
    *,
    challenge_id: str,
    route: str,
    output_dir: Path,
    state_file: Path,
    admin_path: Path,
    ledger_path: Path,
    monitor_context_path: Path,
    cursor_position: int,
    event_version: int,
    base_url: str,
    timeout_seconds: int,
    query: list[tuple[str, str]],
    expected_owner: str | None,
    interval_ms: int,
    max_cycles: int,
    sleeper=time.sleep,
    clock=time.time,
) -> dict[str, Any]:
    if challenge_id.strip() == "" or route.strip() == "":
        raise ValueError("challenge_id and route are required")
    if not 0 <= cursor_position <= MAX_UINT63 or not 0 <= event_version <= MAX_UINT63:
        raise ValueError("cursor position and event version must be non-negative 63-bit values")
    if max_cycles <= 0:
        raise ValueError("max_cycles must be positive")
    if max_cycles > 1 and not 1_000 <= interval_ms <= 3_600_000:
        raise ValueError("interval_ms must be between 1000 and 3600000 for repeated cycles")

    output_dir = _absolute_required(output_dir, "output directory")
    state_file = _absolute_required(state_file, "state file")
    admin_path = _absolute_required(admin_path, "admin path")
    ledger_path = _absolute_required(ledger_path, "ledger path")
    monitor_context_path = _absolute_required(
        monitor_context_path, "monitor context path"
    )
    monitor_context = _load_monitor_context(monitor_context_path, challenge_id)
    campaign_id = monitor_context["campaign_id"]
    monitor_context_sha256 = hashlib.sha256(
        monitor_context_path.read_bytes()
    ).hexdigest()

    state = _load_state(state_file)
    if state is None:
        state = _new_state(
            challenge_id=challenge_id,
            route=route,
            campaign_id=campaign_id,
            monitor_context_sha256=monitor_context_sha256,
            cursor_position=cursor_position,
            event_version=event_version,
            now_ms=int(clock() * 1000),
        )
    else:
        _validate_state(
            state,
            challenge_id=challenge_id,
            route=route,
            campaign_id=campaign_id,
            monitor_context_sha256=monitor_context_sha256,
        )

    results: list[dict[str, Any]] = []
    failed = False
    for _ in range(max_cycles):
        if results:
            sleeper(interval_ms / 1000)
        cursor = state["cursor_position"]
        version = state["event_version"]
        try:
            summary = run_cycle(
                challenge_id=challenge_id,
                route=route,
                cursor_position=cursor,
                observed_at_ms=int(clock() * 1000),
                output_dir=output_dir,
                response_path=None,
                owned_only=True,
                base_url=base_url,
                timeout_seconds=timeout_seconds,
                query=query,
                expected_owner=expected_owner,
                admin_path=admin_path,
                ledger_path=ledger_path,
                monitor_context_path=monitor_context_path,
                starting_event_version=version,
            )
        except (OSError, RuntimeError, ValueError, json.JSONDecodeError) as error:
            failed = True
            state["updated_at_ms"] = int(clock() * 1000)
            state["last_status"] = "failed"
            state["last_summary_path"] = None
            state["last_error"] = safe_error_message(error)
            write_atomic_bytes(state_file, _json_bytes(state))
            results.append(
                {
                    "status": "failed",
                    "error": state["last_error"],
                    "cursor_position": cursor,
                    "event_version": version,
                }
            )
            break

        item_count = summary.get("item_count")
        next_version_value = summary.get("next_expected_version", version)
        if (
            not isinstance(item_count, int)
            or isinstance(item_count, bool)
            or item_count < 0
            or not isinstance(next_version_value, int)
            or isinstance(next_version_value, bool)
            or not version <= next_version_value <= MAX_UINT63
        ):
            raise ValueError("runner returned invalid cycle counters")
        if summary.get("applied_count", 0) > 0 and next_version_value <= version:
            raise ValueError("applied reconciliation must advance the campaign event version")

        next_cursor = _next_cursor(cursor, item_count)
        next_version = next_version_value
        wake_path = None
        if summary.get("chief_event_recorded") is True:
            wake_path = _write_chief_wake(
                output_dir=output_dir,
                summary=summary,
                campaign_id=campaign_id,
                challenge_id=challenge_id,
                stream_id=f"{challenge_id}/attempts",
                event_version=next_version,
            )

        state["cursor_position"] = next_cursor
        state["event_version"] = next_version
        state["cycles_completed"] += 1
        state["updated_at_ms"] = int(clock() * 1000)
        state["last_status"] = summary.get("status")
        state["last_summary_path"] = summary.get("summary_path")
        state["last_error"] = None
        if wake_path is not None:
            state["last_wake_event_version"] = next_version
        write_atomic_bytes(state_file, _json_bytes(state))
        result = {
            "status": summary.get("status"),
            "applied_count": summary.get("applied_count", 0),
            "cursor_position": cursor,
            "next_cursor_position": next_cursor,
            "event_version": version,
            "next_event_version": next_version,
            "chief_wake_path": str(wake_path) if wake_path is not None else None,
        }
        results.append(result)

    return {
        "schema_version": SCHEMA_VERSION,
        "read_only": True,
        "platform_write_attempted": False,
        "state_path": str(state_file),
        "cycles_requested": max_cycles,
        "cycles_attempted": len(results),
        "failed": failed,
        "results": results,
        "state": state,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--challenge-id", required=True)
    parser.add_argument("--route", required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--state-file", type=Path, required=True)
    parser.add_argument("--admin", type=Path, required=True)
    parser.add_argument("--ledger", type=Path, required=True)
    parser.add_argument("--monitor-context", type=Path, required=True)
    parser.add_argument("--cursor-position", type=int, required=True)
    parser.add_argument("--event-version", type=int, required=True)
    parser.add_argument("--base-url", default="https://play.bohrium.com/api")
    parser.add_argument("--timeout-seconds", type=int, default=30)
    parser.add_argument(
        "--query",
        action="append",
        default=[],
        help="repeatable key=value query parameter for challenge_attempts",
    )
    parser.add_argument("--expected-owner")
    parser.add_argument("--interval-ms", type=int, default=1_000)
    parser.add_argument("--max-cycles", type=int, default=1)
    args = parser.parse_args()

    try:
        query = []
        for value in args.query:
            if "=" not in value or value.startswith("=") or value.endswith("="):
                raise ValueError("query parameters must use non-empty key=value")
            key, item = value.split("=", 1)
            query.append((key, item))
        result = run_scheduled_cycles(
            challenge_id=args.challenge_id,
            route=args.route,
            output_dir=args.output_dir,
            state_file=args.state_file,
            admin_path=args.admin,
            ledger_path=args.ledger,
            monitor_context_path=args.monitor_context,
            cursor_position=args.cursor_position,
            event_version=args.event_version,
            base_url=args.base_url,
            timeout_seconds=args.timeout_seconds,
            query=query,
            expected_owner=args.expected_owner,
            interval_ms=args.interval_ms,
            max_cycles=args.max_cycles,
        )
        print(json.dumps(result, ensure_ascii=True, sort_keys=True))
        return 2 if result["failed"] else 0
    except (OSError, RuntimeError, ValueError, json.JSONDecodeError) as error:
        print(
            json.dumps(
                {
                    "error": safe_error_message(error),
                    "read_only": True,
                    "platform_write_attempted": False,
                },
                ensure_ascii=True,
                sort_keys=True,
            )
        )
        return 2


if __name__ == "__main__":
    sys.exit(main())
