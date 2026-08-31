#!/usr/bin/env python3
"""Resident, bounded leaderboard confirmation monitor.

Composes the offline leaderboard verifier (ascodex_leaderboard_check) with a bounded period
loop: each cycle reads a saved leaderboard response (or a single read-only GET), re-confirms
that the owned attempt appears with matching owner/score/scope, and writes typed confirmation
evidence to a directory an administrator-side consumer feeds into the ledger.  This tool is
read-only: it never submits, never reads another owner's attempt, and never starts a process.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import sys
import tempfile
import time
from pathlib import Path
from typing import Any

from scripts.ascodex_leaderboard_check import (
    build_leaderboard_confirmation,
)
from scripts.ascodex_monitor import write_atomic

SCHEMA_VERSION = "ascodex-leaderboard-monitor/v1"


def confirmation_filename(attempt_id: str, scope: str | None) -> str:
    """Deterministic evidence filename from attempt id and scope."""
    material = f"{attempt_id}\0{scope or ''}"
    digest = hashlib.sha256(material.encode("utf-8")).hexdigest()[:16]
    return f"confirmation-{digest}.json"


def write_confirmation_evidence(
    response: Any,
    *,
    attempt_id: str,
    expected_owner: str,
    expected_effective_score: float | None,
    scope: str | None,
    evidence_dir: Path,
) -> Path:
    """Re-confirm one owned attempt and write typed confirmation evidence."""
    evidence_dir = evidence_dir.resolve()
    confirmation = build_leaderboard_confirmation(
        response,
        attempt_id=attempt_id,
        expected_owner=expected_owner,
        expected_effective_score=expected_effective_score,
        scope=scope,
    )
    confirmation["schema_version"] = SCHEMA_VERSION
    confirmation["response_sha256"] = hashlib.sha256(
        _json_bytes(response)
    ).hexdigest()
    filename = confirmation_filename(attempt_id, scope)
    path = evidence_dir / filename
    path.parent.mkdir(parents=True, exist_ok=True)
    write_atomic(path, confirmation)
    return path


def build_confirmation_state(*, initial_cycles: int) -> dict[str, Any]:
    if initial_cycles < 0:
        raise ValueError("initial cycle count must be non-negative")
    return {
        "schema_version": SCHEMA_VERSION,
        "cycles_completed": initial_cycles,
        "updated_at_ms": int(time.time() * 1000),
        "last_status": None,
        "last_confirmation_path": None,
    }


def _load_state(path: Path) -> dict[str, Any] | None:
    if not path.exists():
        return None
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise ValueError("monitor state must be a JSON object")
    if value.get("schema_version") != SCHEMA_VERSION:
        raise ValueError("monitor state schema is unsupported")
    return value


def _json_bytes(value: Any) -> bytes:
    return (
        json.dumps(value, ensure_ascii=True, sort_keys=True, indent=2) + "\n"
    ).encode("utf-8")


def run_leaderboard_cycles(
    *,
    response: Any,
    attempt_id: str,
    expected_owner: str,
    expected_effective_score: float | None,
    scope: str | None,
    evidence_dir: Path,
    state_file: Path,
    max_cycles: int,
    interval_ms: int,
    sleeper=time.sleep,
) -> dict[str, Any]:
    """Run up to max_cycles confirmation cycles. A malformed response or missing owner aborts
    the loop (fail-stop); a score mismatch still writes unknown evidence (that is the audit
    fact) and continues."""
    if max_cycles <= 0:
        raise ValueError("max_cycles must be positive")
    if interval_ms < 0:
        raise ValueError("interval_ms must be non-negative")
    state = _load_state(state_file) or build_confirmation_state(initial_cycles=0)
    last_path = None
    for cycle in range(max_cycles):
        if cycle > 0 and interval_ms > 0:
            sleeper(interval_ms / 1000)
        path = write_confirmation_evidence(
            response,
            attempt_id=attempt_id,
            expected_owner=expected_owner,
            expected_effective_score=expected_effective_score,
            scope=scope,
            evidence_dir=evidence_dir,
        )
        last_path = path
        parsed = json.loads(path.read_text(encoding="utf-8"))
        state["cycles_completed"] = state.get("cycles_completed", 0) + 1
        state["updated_at_ms"] = int(time.time() * 1000)
        state["last_status"] = parsed.get("state")
        state["last_confirmation_path"] = str(path)
        _write_state(state_file, state)
    return state


def _write_state(path: Path, value: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, temporary = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
    try:
        with os.fdopen(fd, "w", encoding="utf-8", newline="\n") as handle:
            handle.write(_json_bytes(value).decode("utf-8"))
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, path)
    except BaseException:
        try:
            os.unlink(temporary)
        except FileNotFoundError:
            pass
        raise


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--response", type=Path, required=True)
    parser.add_argument("--attempt-id", required=True)
    parser.add_argument("--expected-owner", required=True)
    parser.add_argument("--expected-effective-score", type=float, default=None)
    parser.add_argument("--scope", default=None)
    parser.add_argument("--evidence-dir", type=Path, required=True)
    parser.add_argument("--state-file", type=Path, required=True)
    parser.add_argument("--max-cycles", type=int, default=1)
    parser.add_argument("--interval-ms", type=int, default=0)
    args = parser.parse_args()
    try:
        response = json.loads(args.response.read_bytes().decode("utf-8"))
        state = run_leaderboard_cycles(
            response=response,
            attempt_id=args.attempt_id,
            expected_owner=args.expected_owner,
            expected_effective_score=args.expected_effective_score,
            scope=args.scope,
            evidence_dir=args.evidence_dir,
            state_file=args.state_file,
            max_cycles=args.max_cycles,
            interval_ms=args.interval_ms,
        )
    except (OSError, ValueError, json.JSONDecodeError) as error:
        print(f"ascodex-leaderboard-monitor: {error}", file=sys.stderr)
        return 2
    print(json.dumps(state, ensure_ascii=True, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    sys.exit(main())