#!/usr/bin/env python3
"""Read-only channel probe for one Playground challenge.

The probe performs at most two GET requests: challenge metadata and the
operator-owned challenge-attempts page.  It never creates an attempt, draft,
scoring job, or any other platform object.  Missing channel signals remain
explicit `None`; unlike the historical DSH probe, absent evidence is not
converted into a false negative.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

from scripts.ascodex_monitor import write_atomic
from scripts.ascodex_platform_client import (
    build_request_url,
    current_process_token,
    get_json,
    safe_error_message,
    validate_base_url,
    write_atomic_bytes,
)


SCHEMA_VERSION = "ascodex-channel-probe/v1"
RECENT_ATTEMPT_LIMIT = 30
WORKER_QUEUE_STALE_MS = 3 * 60 * 60 * 1000


def _first(mapping: dict[str, Any], *keys: str) -> Any:
    for key in keys:
        if key in mapping and mapping[key] not in (None, ""):
            return mapping[key]
    return None


def _attempts_page(response: Any) -> list[Any]:
    if isinstance(response, list):
        return response
    if not isinstance(response, dict):
        raise ValueError("attempts response must be a JSON object or array")
    for key in ("attempts", "items", "data", "results", "entries"):
        value = response.get(key)
        if isinstance(value, list):
            return value
    return []


def _strict_bool(value: Any) -> bool | None:
    if isinstance(value, bool):
        return value
    if isinstance(value, int) and not isinstance(value, bool) and value in (0, 1):
        return bool(value)
    if isinstance(value, str):
        normalized = value.strip().lower()
        if normalized in {"1", "true"}:
            return True
        if normalized in {"0", "false"}:
            return False
    return None


def _attempt_time_ms(attempt: dict[str, Any]) -> int | None:
    value = _first(
        attempt,
        "updated_at_ms",
        "updatedAtMs",
        "updated_at",
        "updatedAt",
        "created_at_ms",
        "createdAtMs",
        "created_at",
        "createdAt",
    )
    if isinstance(value, (int, float)) and not isinstance(value, bool):
        number = float(value)
        if number < 0 or value > 9_223_372_036_854_775_807:
            return None
        return int(value)
    if not isinstance(value, str) or not value.strip():
        return None
    try:
        parsed = datetime.fromisoformat(value.strip().replace("Z", "+00:00"))
    except ValueError:
        return None
    if parsed.tzinfo is None:
        parsed = parsed.replace(tzinfo=timezone.utc)
    millis = int(parsed.timestamp() * 1000)
    return millis if millis >= 0 else None


def _challenge_binds_response(
    response: dict[str, Any], challenge_id: str
) -> bool:
    value = _first(response, "challenge_id", "challengeId", "id")
    return value is None or str(value) == challenge_id


def build_channel_probe(
    *,
    challenge_id: str,
    challenge_response: Any,
    attempts_response: Any | None,
    challenge_raw: bytes,
    attempts_raw: bytes | None,
    probe_at_ms: int,
) -> dict[str, Any]:
    if not challenge_id.strip():
        raise ValueError("challenge_id is required")
    if probe_at_ms < 0:
        raise ValueError("probe time must be non-negative")
    if not isinstance(challenge_response, dict):
        raise ValueError("challenge response must be a JSON object")
    if not _challenge_binds_response(challenge_response, challenge_id):
        raise ValueError("challenge response does not match the requested challenge")
    if (attempts_response is None) != (attempts_raw is None):
        raise ValueError("attempts response and raw bytes must be supplied together")

    contract = challenge_response.get("contract")
    if not isinstance(contract, dict):
        contract = challenge_response
    grader_name_value = _first(
        contract, "grader_name", "graderName"
    )
    if grader_name_value is None and isinstance(
        challenge_response.get("scoring"), dict
    ):
        grader_name_value = _first(
            challenge_response["scoring"], "grader_name", "graderName"
        )
    grader_name = (
        str(grader_name_value).strip() if grader_name_value is not None else None
    )
    s2 = _strict_bool(_first(contract, "s2"))

    grader_registered: bool | None = None
    if s2 is False and grader_name is not None:
        grader_registered = True
    elif s2 is False and grader_name is None:
        grader_registered = False

    harbor_active: bool | None = None
    worker_queue_ok: bool | None = None
    newest_attempt_updated_ms: int | None = None
    observed_attempt_count: int | None = None
    if attempts_response is not None:
        attempts = _attempts_page(attempts_response)
        observed_attempt_count = len(attempts)
        recent = attempts[:RECENT_ATTEMPT_LIMIT]
        harbor_signals = []
        for attempt in recent:
            if not isinstance(attempt, dict):
                raise ValueError("challenge-attempts entries must be JSON objects")
            attempt_challenge = _first(attempt, "challenge_id", "challengeId")
            if attempt_challenge is not None and str(attempt_challenge) != challenge_id:
                raise ValueError(
                    "challenge-attempts page contains another challenge"
                )
            harbor_signals.append(
                _first(
                    attempt,
                    "harbor_reward",
                    "harborReward",
                    "harbor_replay_executed",
                    "harborReplayExecuted",
                    "score",
                )
                is not None
            )
        harbor_active = any(harbor_signals)
        for attempt in recent:
            timestamp = _attempt_time_ms(attempt)
            if timestamp is not None and (
                newest_attempt_updated_ms is None
                or timestamp > newest_attempt_updated_ms
            ):
                newest_attempt_updated_ms = timestamp
        if newest_attempt_updated_ms is not None:
            worker_queue_ok = (
                probe_at_ms - newest_attempt_updated_ms < WORKER_QUEUE_STALE_MS
            )

    return {
        "schema_version": SCHEMA_VERSION,
        "challenge_id": challenge_id,
        "probe_at_ms": probe_at_ms,
        "challenge_route": f"/api/challenges/{challenge_id}",
        "attempts_route": f"/api/challenges/{challenge_id}/attempts",
        "challenge_response_sha256": hashlib.sha256(challenge_raw).hexdigest(),
        "attempts_response_sha256": (
            hashlib.sha256(attempts_raw).hexdigest()
            if attempts_raw is not None
            else None
        ),
        "grader_name": grader_name,
        "s2": s2,
        "grader_registered": grader_registered,
        "harbor_active": harbor_active,
        "observed_attempt_count": observed_attempt_count,
        "newest_attempt_updated_ms": newest_attempt_updated_ms,
        "worker_queue_ok": worker_queue_ok,
        "worker_queue_stale_after_ms": WORKER_QUEUE_STALE_MS,
        "recent_attempt_limit": RECENT_ATTEMPT_LIMIT,
        "method": "GET",
        "platform_write_attempted": False,
        "quota_cost": "unknown",
    }


def _fetch(
    challenge_id: str,
    base_url: str,
    timeout_seconds: int,
    output_dir: Path,
) -> tuple[Any, bytes, Any, bytes]:
    validate_base_url(base_url)
    token = current_process_token()
    challenge_url = build_request_url("challenge", challenge_id=challenge_id)
    attempts_url = build_request_url(
        "challenge_attempts", challenge_id=challenge_id
    )
    challenge_payload, challenge_raw, _ = get_json(
        challenge_url, token, timeout_seconds=timeout_seconds
    )
    attempts_payload, attempts_raw, _ = get_json(
        attempts_url, token, timeout_seconds=timeout_seconds
    )
    write_atomic_bytes(
        output_dir / "responses" / f"challenge-{challenge_id}.json", challenge_raw
    )
    write_atomic_bytes(
        output_dir / "responses" / f"attempts-{challenge_id}.json", attempts_raw
    )
    return challenge_payload, challenge_raw, attempts_payload, attempts_raw


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--challenge-id", required=True)
    parser.add_argument("--owned-only", action="store_true")
    parser.add_argument("--base-url", default="https://play.bohrium.com/api")
    parser.add_argument("--timeout-seconds", type=int, default=30)
    parser.add_argument("--challenge-response", type=Path)
    parser.add_argument("--attempts-response", type=Path)
    parser.add_argument("--probe-at-ms", type=int)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--artifact-dir", type=Path)
    args = parser.parse_args()

    try:
        probe_at_ms = (
            args.probe_at_ms
            if args.probe_at_ms is not None
            else int(time.time() * 1000)
        )
        if args.challenge_response is not None:
            challenge_raw = args.challenge_response.read_bytes()
            challenge_payload = json.loads(challenge_raw.decode("utf-8"))
            if args.attempts_response is None:
                attempts_payload = None
                attempts_raw = None
            else:
                attempts_raw = args.attempts_response.read_bytes()
                attempts_payload = json.loads(attempts_raw.decode("utf-8"))
            artifact_dir = (
                args.artifact_dir if args.artifact_dir is not None else args.output.parent
            ).resolve()
            challenge_path = artifact_dir / "responses" / f"challenge-{args.challenge_id}.json"
            attempts_path = artifact_dir / "responses" / f"attempts-{args.challenge_id}.json"
            write_atomic_bytes(challenge_path, challenge_raw)
            write_atomic_bytes(attempts_path, attempts_raw)
        else:
            if not args.owned_only:
                raise ValueError(
                    "live channel probe requires --owned-only because it reads "
                    "the challenge attempts endpoint"
                )
            if args.attempts_response is not None:
                raise ValueError(
                    "--attempts-response cannot be combined with a live challenge GET"
                )
            artifact_dir = (
                args.artifact_dir if args.artifact_dir is not None else args.output.parent
            ).resolve()
            challenge_path = artifact_dir / "responses" / f"challenge-{args.challenge_id}.json"
            attempts_path = artifact_dir / "responses" / f"attempts-{args.challenge_id}.json"
            challenge_payload, challenge_raw, attempts_payload, attempts_raw = _fetch(
                args.challenge_id,
                args.base_url,
                args.timeout_seconds,
                artifact_dir,
            )
        probe = build_channel_probe(
            challenge_id=args.challenge_id,
            challenge_response=challenge_payload,
            attempts_response=attempts_payload,
            challenge_raw=challenge_raw,
            attempts_raw=attempts_raw,
            probe_at_ms=probe_at_ms,
        )
        write_atomic(args.output, probe)
        print(
            json.dumps(
                {
                    **probe,
                    "evidence": {
                        "challenge_response_path": str(challenge_path.resolve()),
                        "attempts_response_path": str(attempts_path.resolve()),
                    },
                },
                ensure_ascii=True,
                sort_keys=True,
            )
        )
        return 0
    except (OSError, ValueError, json.JSONDecodeError) as error:
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
