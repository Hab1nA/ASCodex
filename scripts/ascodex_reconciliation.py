#!/usr/bin/env python3
"""Convert one saved read-only platform response into a typed reconciliation item.

The tool is offline and never mutates a platform object.  It prefers explicit
unknown reconciliation over fabricated evidence: fields are emitted only when
they are present and structurally valid for the Rust reducer.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import time
from pathlib import Path
from typing import Any

from scripts.ascodex_monitor import (
    _flag,
    _first,
    _nested_first,
    _number,
    _status,
    write_atomic,
)


SCHEMA_VERSION = "ascodex-platform-reconciliation/v1"
EVIDENCE_VALUES = {"present", "redacted", "pending", "unavailable", "not_applicable"}
RESCORE_VALUES = {
    "not_applicable": "not_applicable",
    "pending": "pending",
    "completed": "completed",
    "failed": "failed",
}


def _owner(value: Any) -> str | None:
    if isinstance(value, dict):
        value = _first(value, "id", "name", "username", "handle")
    if value is None or value == "":
        return None
    return str(value)


def _scope(value: Any) -> str | None:
    if isinstance(value, str) and value.strip().lower() in {
        "unified_overall_and_season",
        "unifiedoverallandseason",
    }:
        return "unified_overall_and_season"
    return None


def _rescore(value: Any) -> str | None:
    if not isinstance(value, str):
        return None
    return RESCORE_VALUES.get(value.strip().lower())


def _evidence(value: Any) -> str | None:
    if isinstance(value, bool):
        return "present" if value else "unavailable"
    if isinstance(value, str) and value.strip().lower() in EVIDENCE_VALUES:
        return value.strip().lower()
    return None


def _valid_anti_cheat(value: Any) -> dict[str, Any] | None:
    if not isinstance(value, dict):
        return None
    mode = value.get("mode")
    signals = value.get("signals")
    if mode not in {"weighted_three_signals", "weightedThreeSignals"} or not isinstance(
        signals, list
    ):
        return None
    normalized: list[dict[str, Any]] = []
    names: set[str] = set()
    for signal in signals:
        if not isinstance(signal, dict):
            return None
        name = signal.get("name")
        weight = _number(signal.get("weight"))
        if not isinstance(name, str) or not name.strip() or weight is None or weight < 0:
            return None
        key = name.strip()
        if key in names:
            return None
        names.add(key)
        availability = _evidence(signal.get("availability")) or "unavailable"
        normalized.append(
            {
                "name": key,
                "weight": weight,
                "availability": availability,
            }
        )
    if len(normalized) != 3 or all(item["weight"] == 0 for item in normalized):
        return None
    return {"mode": "weighted_three_signals", "signals": normalized}


def _challenge_page_evidence(response: Any) -> tuple[dict[str, Any] | None, bool]:
    """Extract optional challenge-page evidence without inventing section state.

    Missing section values become explicit `unavailable`; a share route and its
    status must agree, otherwise the caller receives an inconsistent marker and
    must keep the item in reconciliation.
    """
    value = _nested_first(response, "challenge_page", "challengePage")
    if not isinstance(value, dict):
        return None, True

    def section(*names: str) -> str:
        return _evidence(_nested_first(value, *names)) or "unavailable"

    share_route_value = _nested_first(value, "share_route", "shareRoute")
    share_route = (
        str(share_route_value).strip()
        if share_route_value not in (None, "")
        else None
    )
    share_route_status = "present" if share_route else "unavailable"
    provided_share_route_status = _evidence(
        _nested_first(value, "share_route_status", "shareRouteStatus")
    )
    consistent = (
        provided_share_route_status is None
        or provided_share_route_status == share_route_status
    )
    if not consistent:
        return None, False

    return (
        {
            "challenge_section": section(
                "challenge_section",
                "challengeSection",
            ),
            "my_submissions_section": section(
                "my_submissions_section",
                "mySubmissionsSection",
            ),
            "leaderboard_section": section(
                "leaderboard_section",
                "leaderboardSection",
            ),
            "share_route": share_route,
            "share_route_status": share_route_status,
            "attachment_status": section(
                "attachment_status",
                "attachmentStatus",
            ),
        },
        True,
    )


def _complete_penalty(
    raw_score: float | None,
    effective_score: float | None,
    penalty: float | None,
    penalty_applied: bool,
    basis_value: Any,
) -> tuple[bool, dict[str, Any] | None]:
    if not penalty_applied or raw_score is None or effective_score is None:
        return False, None
    if penalty != -1.0 or abs(effective_score - (raw_score - 1.0)) > 1e-9:
        return False, None
    if not isinstance(basis_value, dict):
        return False, None
    object_name = basis_value.get("object")
    reason = basis_value.get("reason")
    rewritten = _number(basis_value.get("rewritten_score"))
    if (
        not isinstance(object_name, str)
        or not object_name.strip()
        or not isinstance(reason, str)
        or not reason.strip()
        or rewritten is None
        or abs(rewritten - effective_score) > 1e-9
    ):
        return False, None
    return True, {
        "object": object_name.strip(),
        "reason": reason.strip(),
        "rewritten_score": rewritten,
    }


def build_reconciliation_item(
    response: Any,
    *,
    raw_bytes: bytes,
    challenge_id: str,
    attempt_id: str,
    route: str,
    cursor_position: int,
    observed_at_ms: int,
    expected_owner: str | None = None,
) -> dict[str, Any]:
    if not challenge_id.strip() or not attempt_id.strip() or not route.strip():
        raise ValueError("challenge_id, attempt_id, and route are required")
    if cursor_position < 0 or observed_at_ms < 0:
        raise ValueError("cursor position and observed_at_ms must be non-negative")
    if not isinstance(response, dict):
        raise ValueError("platform response must be a JSON object")

    response_challenge = _first(response, "challengeId", "challenge_id")
    response_attempt = _first(response, "attemptId", "attempt_id", "id")
    if response_challenge is not None and str(response_challenge) != challenge_id:
        raise ValueError("response challengeId does not match the requested challenge")
    if response_attempt is not None and str(response_attempt) != attempt_id:
        raise ValueError("response attempt id does not match the requested attempt")

    response_sha256 = hashlib.sha256(raw_bytes).hexdigest()
    replay_value = _nested_first(
        response,
        "harbor_replay_executed",
        "harborReplayExecuted",
        "replay_executed",
        "replayExecuted",
        "replay",
    )
    replay_status = _status(replay_value)
    results_status = _status(_first(response, "resultsJson", "results_json", "results"))
    scorecard_status = _status(
        _first(response, "scorecard", "scoringDetails", "scoring_details")
    )
    leaderboard_status = _status(
        _first(response, "leaderboard", "leaderboard_status", "rank", "leaderboardEntry")
    )
    reward = _nested_first(response, "harbor_reward", "harborReward", "reward")
    reward_number = _number(reward)
    trace_score = _number(_nested_first(response, "trace_score", "traceScore"))
    raw_score = _number(_nested_first(response, "raw_score", "rawScore", "original_score", "originalScore"))
    effective_score = _number(
        _nested_first(response, "effective_score", "effectiveScore", "credited_score", "creditedScore")
    )
    penalty = _number(_nested_first(response, "penalty", "penaltyDelta", "penalty_delta"))
    penalty_applied = _flag(
        _nested_first(response, "penalty_applied", "penaltyApplied", "penalized")
    )
    penalty_valid, penalty_basis = _complete_penalty(
        raw_score,
        effective_score,
        penalty,
        penalty_applied,
        _nested_first(response, "penalty_basis", "penaltyBasis", "penalty_reason"),
    )
    owner_value = _nested_first(response, "credited_owner", "creditedOwner", "owner", "agent", "user")
    owner = _owner(owner_value)
    scope = _scope(_nested_first(response, "leaderboard_scope", "leaderboardScope", "scope"))
    bundle_revision = _nested_first(
        response, "bundle_revision", "bundleRevision", "bundle_hash", "bundleHash"
    )
    bundle_revision = str(bundle_revision) if bundle_revision not in (None, "") else None
    rescore_status = _rescore(
        _nested_first(response, "rescore_status", "rescoreStatus", "bundle_rescore_status")
    )
    trace_evidence = _evidence(
        _nested_first(response, "trace_evidence", "traceEvidence", "execution_trace", "executionTrace")
    )
    anti_cheat = _valid_anti_cheat(
        _nested_first(response, "anti_cheat", "antiCheat", "anticheat")
    )
    challenge_page, challenge_page_consistent = _challenge_page_evidence(response)
    no_penalty = not penalty_applied and penalty in (None, 0.0)
    normalized_expected_owner = (
        expected_owner.strip() if expected_owner is not None else None
    )
    owner_matches = normalized_expected_owner is None or owner == normalized_expected_owner

    observation_complete = (
        replay_status == "present"
        and results_status in {"present", "redacted"}
        and scorecard_status in {"present", "redacted"}
        and leaderboard_status == "present"
        and reward_number is not None
        and 0 <= reward_number <= 1
        and (trace_score is None or 0 <= trace_score <= 100)
        and trace_evidence == "present"
    )
    facts: dict[str, Any] = {}
    if raw_score is not None:
        if not 0 <= raw_score <= 100:
            raise ValueError("raw_score must be within [0, 100]")
        facts["raw_score"] = raw_score
    if effective_score is not None:
        if not -1 <= effective_score <= 100:
            raise ValueError("effective_score must be within [-1, 100]")
        facts["effective_score"] = effective_score
    if penalty_valid and penalty_basis is not None:
        facts["penalty"] = penalty
        facts["penalty_applied"] = True
        facts["penalty_basis"] = penalty_basis
    if owner is not None and scope is not None:
        facts["credited_owner"] = owner
        facts["leaderboard_scope"] = scope
    if bundle_revision is not None and rescore_status is not None:
        facts["bundle_revision"] = bundle_revision
        facts["rescore_status"] = rescore_status
    if trace_evidence is not None:
        facts["trace_evidence"] = trace_evidence
    if anti_cheat is not None:
        facts["anti_cheat"] = anti_cheat
    if challenge_page is not None:
        facts["challenge_page"] = challenge_page
    if _nested_first(
        response,
        "anonymous_other_submission_access",
        "anonymousOtherSubmissionAccess",
    ) is False:
        facts["anonymous_other_submission_access"] = "closed"

    complete = (
        observation_complete
        and raw_score is not None
        and effective_score is not None
        and (penalty_valid or no_penalty)
        and owner is not None
        and scope is not None
        and owner_matches
        and bundle_revision is not None
        and rescore_status == "completed"
        and anti_cheat is not None
        and challenge_page_consistent
    )
    if complete:
        facts["score_evidence"] = "present"
        facts["penalty_evidence"] = "present"
        facts["credited_owner_evidence"] = "present"
        facts["bundle_evidence"] = "present"
        state = {
            "kind": "observation",
            "observation": {
                "attempt_id": attempt_id,
                "challenge_id": challenge_id,
                "route": route,
                "observed_at_ms": observed_at_ms,
                "response_sha256": response_sha256,
                "replay_status": replay_status,
                "results_status": results_status,
                "scorecard_status": scorecard_status,
                "leaderboard_status": leaderboard_status,
                "harbor_reward": reward,
                "trace_score": trace_score,
            },
        }
    else:
        reasons: list[str] = []
        if not observation_complete:
            reasons.append("replay/results/scorecard/leaderboard evidence incomplete")
        if raw_score is None or effective_score is None:
            reasons.append("raw/effective scores unverified")
        if not (penalty_valid or no_penalty):
            reasons.append("penalty delta and basis unverified")
        if owner is None or scope is None:
            reasons.append("credited owner or leaderboard scope unverified")
        elif not owner_matches:
            reasons.append("credited owner does not match the expected operator")
        if bundle_revision is None or rescore_status != "completed":
            reasons.append("bundle revision or fresh rescore unverified")
        if anti_cheat is None:
            reasons.append("weighted anti-cheat evidence unverified")
        if trace_evidence != "present":
            reasons.append("execution trace evidence unverified")
        if not challenge_page_consistent:
            reasons.append("challenge page evidence inconsistent")
        state = {
            "kind": "unknown_needs_reconcile",
            "reason": "; ".join(reasons),
        }

    return {
        "cursor": {
            "stream_id": f"{challenge_id}/attempts",
            "position": cursor_position,
        },
        "challenge_id": challenge_id,
        "attempt_id": attempt_id,
        "route": route,
        "observed_at_ms": observed_at_ms,
        "response_sha256": response_sha256,
        "facts": facts,
        "state": state,
    }


def _attempt_page(response: Any) -> list[Any]:
    if isinstance(response, list):
        return response
    if not isinstance(response, dict):
        raise ValueError("platform response must be a JSON object")
    for key in ("attempts", "data", "items", "results", "entries"):
        value = response.get(key)
        if isinstance(value, list):
            return value
    return [response]


def _attempt_field(attempt: Any, field: str, default: str | None = None) -> str:
    if not isinstance(attempt, dict):
        raise ValueError("attempt page entries must be JSON objects")
    from scripts.ascodex_schema import attempt_registry, normalize_object

    try:
        normalized = normalize_object(attempt, attempt_registry())
    except ValueError as error:
        raise ValueError(str(error)) from error
    if field not in normalized:
        raise ValueError(f"attempt entry is missing {field}")
    value = normalized[field]
    if value in (None, ""):
        value = default
    if value in (None, ""):
        raise ValueError(f"attempt entry is missing {field}")
    return str(value)


def build_reconciliation_items(
    response: Any,
    *,
    raw_bytes: bytes,
    challenge_id: str,
    route: str,
    cursor_position: int,
    observed_at_ms: int,
    expected_owner: str | None = None,
) -> list[dict[str, Any]]:
    """Convert one saved challenge-attempts page into one item per attempt.

    Page positions are expanded as `cursor_position + index` because the reducer
    requires a unique cursor position per item while a page response can contain
    several attempt objects sharing the same response hash.
    """
    if cursor_position < 0:
        raise ValueError("cursor position must be non-negative")
    attempts = _attempt_page(response)
    seen_attempt_ids: set[str] = set()
    for attempt in attempts:
        attempt_challenge = _attempt_field(attempt, "challenge_id", challenge_id)
        if attempt_challenge != challenge_id:
            raise ValueError(
                "challenge-attempts page contains an entry bound to another challenge"
            )
        attempt_id = _attempt_field(attempt, "attempt_id")
        if attempt_id in seen_attempt_ids:
            raise ValueError(
                "challenge-attempts page contains a duplicate attempt id"
            )
        seen_attempt_ids.add(attempt_id)
    return [
        build_reconciliation_item(
            attempt,
            raw_bytes=raw_bytes,
            challenge_id=challenge_id,
            attempt_id=_attempt_field(attempt, "attempt_id"),
            route=_attempt_field(attempt, "route", route),
            cursor_position=cursor_position + index,
            observed_at_ms=observed_at_ms,
            expected_owner=expected_owner,
        )
    for index, attempt in enumerate(attempts)
    ]


def _looks_like_attempt_page(payload: Any) -> bool:
    return isinstance(payload, list) or (
        isinstance(payload, dict)
        and any(
            isinstance(payload.get(key), list)
            for key in ("attempts", "data", "items", "results", "entries")
        )
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--response", type=Path, required=True)
    parser.add_argument("--challenge-id", required=True)
    parser.add_argument("--attempt-id")
    parser.add_argument("--route", required=True)
    parser.add_argument("--cursor-position", type=int, required=True)
    parser.add_argument("--observed-at-ms", type=int)
    parser.add_argument("--expected-owner")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument(
        "--batch",
        action="store_true",
        help="write a JSON array of items suitable for ascodex-observation-admin reconcile-batch",
    )
    args = parser.parse_args()
    raw = args.response.read_bytes()
    response = json.loads(raw.decode("utf-8"))
    observed_at_ms = (
        args.observed_at_ms
        if args.observed_at_ms is not None
        else int(time.time() * 1000)
    )
    if args.batch:
        items = build_reconciliation_items(
            response,
            raw_bytes=raw,
            challenge_id=args.challenge_id,
            route=args.route,
            cursor_position=args.cursor_position,
            observed_at_ms=observed_at_ms,
            expected_owner=args.expected_owner,
        )
        write_atomic(args.output, items)
        print(json.dumps(items, ensure_ascii=False, sort_keys=True))
        return 0

    if _looks_like_attempt_page(response):
        raise ValueError("list-shaped response requires --batch")

    if args.attempt_id is None:
        raise ValueError("attempt_id is required for single-attempt responses")
    item = build_reconciliation_item(
        response,
        raw_bytes=raw,
        challenge_id=args.challenge_id,
        attempt_id=args.attempt_id,
        route=args.route,
        cursor_position=args.cursor_position,
        observed_at_ms=observed_at_ms,
        expected_owner=args.expected_owner,
    )
    write_atomic(args.output, item)
    print(json.dumps(item, ensure_ascii=False, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
