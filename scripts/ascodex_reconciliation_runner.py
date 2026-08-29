#!/usr/bin/env python3
"""Run one audited read-only Playground reconciliation cycle.

The runner composes three already-tested components: the GET-only platform client, the typed
offline converter, and the local observation-admin batch command.  It never writes to the
platform and never marks an incomplete response as scored.  Calling this binary once is one
cycle; a trusted supervisor may schedule repeated invocations, but this script does not run a
resident supervisor or wake the Chief process itself.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import sys
import time
from pathlib import Path
from typing import Any

from scripts.ascodex_platform_client import (
    build_request_url,
    current_process_token,
    get_json,
    safe_error_message,
    validate_base_url,
    write_atomic_bytes,
)
from scripts.ascodex_reconciliation import build_reconciliation_items


def _absolute_required(value: Path, name: str) -> Path:
    resolved = value.expanduser().resolve()
    if not resolved.is_absolute():
        raise ValueError(f"{name} must be an absolute path")
    return resolved


def _load_monitor_context(path: Path, challenge_id: str) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise ValueError("monitor context must be a JSON object")
    if value.get("role") != "monitor":
        raise ValueError("reconciliation requires a monitor actor context")
    if value.get("campaign_id") in (None, ""):
        raise ValueError("monitor context is missing campaign_id")
    if value.get("challenge_id") != challenge_id:
        raise ValueError("monitor context challenge does not match the requested challenge")
    return value


def _json_bytes(value: Any) -> bytes:
    return (json.dumps(value, ensure_ascii=True, sort_keys=True, indent=2) + "\n").encode(
        "utf-8"
    )


def _load_response(path: Path) -> tuple[Any, bytes]:
    raw = path.read_bytes()
    try:
        payload = json.loads(raw.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ValueError("saved response is not UTF-8 JSON") from error
    if not isinstance(payload, (dict, list)):
        raise ValueError("saved response JSON must be an object or array")
    return payload, raw


def run_cycle(
    *,
    challenge_id: str,
    route: str,
    cursor_position: int,
    observed_at_ms: int,
    output_dir: Path,
    response_path: Path | None,
    owned_only: bool,
    base_url: str,
    timeout_seconds: int,
    query: list[tuple[str, str]] | None,
    expected_owner: str | None,
    admin_path: Path | None,
    ledger_path: Path | None,
    monitor_context_path: Path | None,
    starting_event_version: int | None,
) -> dict[str, Any]:
    if challenge_id.strip() == "" or route.strip() == "":
        raise ValueError("challenge_id and route are required")
    if cursor_position < 0 or observed_at_ms < 0:
        raise ValueError("cursor position and observed time must be non-negative")
    if starting_event_version is not None and starting_event_version < 0:
        raise ValueError("starting event version must be non-negative")

    output_dir = output_dir.expanduser().resolve()
    payload: Any
    raw: bytes
    if response_path is None:
        if not owned_only:
            raise ValueError(
                "platform fetch requires --owned-only to assert operator-owned attempts"
            )
        validate_base_url(base_url)
        url = build_request_url(
            "challenge_attempts",
            challenge_id=challenge_id,
            query=query,
        )
        payload, raw, _status = get_json(
            url,
            current_process_token(),
            timeout_seconds=timeout_seconds,
        )
    else:
        payload, raw = _load_response(response_path)

    response_sha256 = hashlib.sha256(raw).hexdigest()
    response_dir = output_dir / "responses"
    response_output = response_dir / f"{response_sha256}.json"
    write_atomic_bytes(response_output, raw)

    items = build_reconciliation_items(
        payload,
        raw_bytes=raw,
        challenge_id=challenge_id,
        route=route,
        cursor_position=cursor_position,
        observed_at_ms=observed_at_ms,
        expected_owner=expected_owner,
    )
    manifest_output = output_dir / "manifests" / f"{response_sha256}-{cursor_position}.json"
    write_atomic_bytes(manifest_output, _json_bytes(items))

    base_summary = {
        "read_only": True,
        "platform_write_attempted": False,
        "challenge_id": challenge_id,
        "response_sha256": response_sha256,
        "response_path": str(response_output),
        "manifest_path": str(manifest_output),
        "item_count": len(items),
        "cursor_position": cursor_position,
        "observed_at_ms": observed_at_ms,
    }
    if not items:
        return {
            **base_summary,
            "status": "no_items",
            "chief_event_recorded": False,
        }

    if admin_path is None:
        return {
            **base_summary,
            "status": "manifest_only",
            "chief_event_recorded": False,
        }

    if ledger_path is None or monitor_context_path is None or starting_event_version is None:
        raise ValueError(
            "ledger batch mode requires --ledger, --monitor-context, and --starting-event-version"
        )

    admin = _absolute_required(admin_path, "admin path")
    ledger = _absolute_required(ledger_path, "ledger path")
    monitor_context_path = _absolute_required(
        monitor_context_path, "monitor context path"
    )
    monitor_context = _load_monitor_context(monitor_context_path, challenge_id)
    command = [
        str(admin),
        "reconcile-batch",
        "--ledger",
        str(ledger),
        "--monitor-context",
        str(monitor_context_path),
        "--manifest",
        str(manifest_output),
        "--starting-event-version",
        str(starting_event_version),
    ]
    completed = subprocess.run(
        command,
        capture_output=True,
        text=True,
        check=False,
    )
    if completed.returncode != 0:
        raise RuntimeError(
            safe_error_message(
                RuntimeError(completed.stderr.strip() or "observation admin failed")
            )
        )
    try:
        admin_result = json.loads(completed.stdout)
    except json.JSONDecodeError as error:
        raise ValueError("observation admin did not return JSON") from error
    if not isinstance(admin_result, dict):
        raise ValueError("observation admin JSON result must be an object")
    if admin_result.get("status") != "batch-complete":
        raise ValueError("unexpected observation admin result status")
    if admin_result.get("campaign_id") != monitor_context["campaign_id"]:
        raise ValueError("observation admin result campaign does not match monitor context")
    if admin_result.get("challenge_id") != challenge_id:
        raise ValueError("observation admin result challenge does not match the request")
    next_version = admin_result.get("next_expected_version")
    if (
        not isinstance(next_version, int)
        or isinstance(next_version, bool)
        or next_version < starting_event_version
    ):
        raise ValueError("observation admin returned an invalid next event version")

    summaries = admin_result.get("items")
    applied = 0
    if not isinstance(summaries, list) or len(summaries) != len(items):
        raise ValueError("observation admin result does not cover every manifest item")
    for summary in summaries:
        if not isinstance(summary, dict) or summary.get("status") not in {
            "applied",
            "duplicate",
            "stale",
        }:
            raise ValueError("observation admin returned an invalid item summary")
        if summary.get("status") == "applied":
            applied += 1

    run_id = f"{observed_at_ms}-{cursor_position}-{response_sha256}"
    summary_output = output_dir / "runs" / f"{run_id}.json"
    summary = {
        **base_summary,
        "status": "batch_complete",
        "chief_event_recorded": applied > 0,
        "applied_count": applied,
        "admin_result": admin_result,
        "next_expected_version": next_version,
    }
    write_atomic_bytes(summary_output, _json_bytes(summary))
    summary["summary_path"] = str(summary_output)
    return summary


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--challenge-id", required=True)
    parser.add_argument("--route", required=True)
    parser.add_argument("--cursor-position", type=int, required=True)
    parser.add_argument("--observed-at-ms", type=int)
    parser.add_argument("--response", type=Path)
    parser.add_argument(
        "--owned-only",
        action="store_true",
        help="assert that challenge_attempts reads are limited to the current operator",
    )
    parser.add_argument("--base-url", default="https://play.bohrium.com/api")
    parser.add_argument("--timeout-seconds", type=int, default=30)
    parser.add_argument(
        "--query",
        action="append",
        default=[],
        help="repeatable key=value query parameter for challenge_attempts",
    )
    parser.add_argument("--expected-owner")
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--admin", type=Path)
    parser.add_argument("--ledger", type=Path)
    parser.add_argument("--monitor-context", type=Path)
    parser.add_argument("--starting-event-version", type=int)
    args = parser.parse_args()

    try:
        query = []
        for value in args.query:
            if "=" not in value or value.startswith("=") or value.endswith("="):
                raise ValueError("query parameters must use non-empty key=value")
            key, item = value.split("=", 1)
            query.append((key, item))
        summary = run_cycle(
            challenge_id=args.challenge_id,
            route=args.route,
            cursor_position=args.cursor_position,
            observed_at_ms=(
                args.observed_at_ms
                if args.observed_at_ms is not None
                else int(time.time() * 1000)
            ),
            output_dir=args.output_dir,
            response_path=args.response,
            owned_only=args.owned_only,
            base_url=args.base_url,
            timeout_seconds=args.timeout_seconds,
            query=query,
            expected_owner=args.expected_owner,
            admin_path=args.admin,
            ledger_path=args.ledger,
            monitor_context_path=args.monitor_context,
            starting_event_version=args.starting_event_version,
        )
        print(json.dumps(summary, ensure_ascii=True, sort_keys=True))
        return 0
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
