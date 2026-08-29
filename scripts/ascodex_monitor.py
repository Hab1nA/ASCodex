"""Build a typed, read-only platform observation from a saved API response.

This tool deliberately performs no network I/O. A caller must first save the response through
an approved read-only client, then pass the response file here. The resulting observation is an
audit artifact for the ASCodex coordination ledger, not proof that a write was accepted.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import tempfile
import time
import math
from pathlib import Path
from typing import Any


STATUSES = {"present", "redacted", "pending", "unavailable", "not_applicable"}


def _status(value: Any, *, default: str = "unavailable") -> str:
    if isinstance(value, bool):
        return "present" if value else default
    if isinstance(value, str) and value.lower() in STATUSES:
        return value.lower()
    if value is not None:
        return "present"
    return default


def _evidence_status(value: Any) -> str:
    """Normalize boolean-like evidence without treating unknown text as proof."""
    if value is None:
        return "unavailable"
    if isinstance(value, bool):
        return "present" if value else "unavailable"
    if isinstance(value, (int, float)):
        return "present" if value == 1 else "unavailable"
    if isinstance(value, str):
        normalized = value.strip().lower()
        if normalized in STATUSES:
            return normalized
        if normalized in {"1", "true"}:
            return "present"
        if normalized in {"0", "false", "no"}:
            return "unavailable"
    return "unavailable"


def _first(mapping: dict[str, Any], *keys: str) -> Any:
    for key in keys:
        if key in mapping:
            return mapping[key]
    return None


def _number(value: Any) -> float | None:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        return None
    value = float(value)
    return value if math.isfinite(value) else None


def _flag(value: Any) -> bool:
    if isinstance(value, bool):
        return value
    if isinstance(value, (int, float)) and not isinstance(value, bool):
        return value != 0
    if isinstance(value, str):
        return value.strip().lower() in {"1", "true", "yes", "applied", "penalized"}
    return False


def _nested_first(response: dict[str, Any], *keys: str) -> Any:
    value = _first(response, *keys)
    if value is not None:
        return value
    for parent in (response.get("scorecard"), response.get("scoringDetails"), response.get("scoring_details")):
        if isinstance(parent, str):
            try:
                parent = json.loads(parent)
            except json.JSONDecodeError:
                continue
        if isinstance(parent, dict):
            value = _first(parent, *keys)
            if value is not None:
                return value
    return None


def _nested_has(response: dict[str, Any], *keys: str) -> bool:
    if any(key in response for key in keys):
        return True
    for parent in (response.get("scorecard"), response.get("scoringDetails"), response.get("scoring_details")):
        if isinstance(parent, str):
            try:
                parent = json.loads(parent)
            except json.JSONDecodeError:
                continue
        if isinstance(parent, dict) and any(key in parent for key in keys):
            return True
    return False


def build_observation(
    response: Any,
    *,
    raw_bytes: bytes,
    challenge_id: str,
    attempt_id: str,
    route: str,
    observed_at_ms: int,
) -> dict[str, Any]:
    if not challenge_id.strip() or not attempt_id.strip() or not route.strip():
        raise ValueError("challenge_id, attempt_id, and route are required")
    if observed_at_ms < 0:
        raise ValueError("observed_at_ms must be non-negative")
    if not isinstance(response, dict):
        raise ValueError("platform response must be a JSON object")

    response_challenge = _first(response, "challengeId", "challenge_id")
    response_attempt = _first(response, "attemptId", "attempt_id", "id")
    if response_challenge is not None and str(response_challenge) != challenge_id:
        raise ValueError("response challengeId does not match the requested challenge")
    if response_attempt is not None and str(response_attempt) != attempt_id:
        raise ValueError("response attempt id does not match the requested attempt")

    replay_value = _nested_first(response, "harbor_replay_executed", "harborReplayExecuted", "replay_executed", "replayExecuted", "replay")
    replay_status = _evidence_status(replay_value)
    results = _first(response, "resultsJson", "results_json", "results")
    scorecard = _first(response, "scorecard", "scoringDetails", "scoring_details")
    leaderboard = _first(response, "leaderboard", "leaderboard_status", "rank", "leaderboardEntry")
    reward = _nested_first(response, "harbor_reward", "harborReward", "reward")
    trace_score = _number(_nested_first(response, "trace_score", "traceScore"))
    raw_score = _number(_nested_first(response, "raw_score", "rawScore", "original_score", "originalScore"))
    effective_score = _number(_nested_first(response, "effective_score", "effectiveScore", "credited_score", "creditedScore"))
    penalty_value = _number(_nested_first(response, "penalty", "penaltyDelta", "penalty_delta"))
    penalty_declared = _nested_has(
        response,
        "penalty",
        "penaltyDelta",
        "penalty_delta",
        "penalty_applied",
        "penaltyApplied",
        "penalized",
        "penalty_basis",
        "penaltyBasis",
        "penalty_reason",
    )
    penalty_applied = _flag(_nested_first(response, "penalty_applied", "penaltyApplied", "penalized"))
    penalty_basis = _nested_first(response, "penalty_basis", "penaltyBasis", "penalty_reason")
    if penalty_basis is not None and not isinstance(penalty_basis, dict):
        penalty_basis = {"reason": str(penalty_basis)}
    owner = _nested_first(response, "credited_owner", "creditedOwner", "owner", "agent", "user")
    if isinstance(owner, dict):
        owner = _first(owner, "id", "name", "username", "handle")
    bundle_revision = _nested_first(response, "bundle_revision", "bundleRevision", "bundle_hash", "bundleHash")
    rescore_status = _nested_first(response, "rescore_status", "rescoreStatus", "bundle_rescore_status")
    anti_cheat = _nested_first(response, "anti_cheat", "antiCheat", "anticheat")
    trace_evidence_value = _nested_first(response, "trace_evidence", "traceEvidence", "execution_trace", "executionTrace")
    trace_evidence = _evidence_status(trace_evidence_value)

    observation = {
        "schema_version": "ascodex-platform-observation/v1",
        "attempt_id": attempt_id,
        "challenge_id": challenge_id,
        "route": route,
        "observed_at_ms": observed_at_ms,
        "response_sha256": hashlib.sha256(raw_bytes).hexdigest(),
        "replay_status": replay_status,
        "results_status": _status(results),
        "scorecard_status": _status(scorecard),
        "leaderboard_status": _status(leaderboard),
        "harbor_reward": reward,
        "trace_score": trace_score,
        "raw_score": raw_score,
        "effective_score": effective_score,
        "penalty": penalty_value,
        "penalty_applied": penalty_applied or (penalty_value is not None and penalty_value < 0),
        "penalty_basis": penalty_basis,
        "credited_owner": owner,
        "bundle_revision": bundle_revision,
        "rescore_status": rescore_status,
        "anti_cheat": anti_cheat,
        "trace_evidence": trace_evidence,
        "leaderboard_scope": _nested_first(response, "leaderboard_scope", "leaderboardScope", "scope"),
        "season_id": _nested_first(response, "season_id", "seasonId"),
        "admission_status": "admitted" if trace_evidence == "present" else "not_queued_no_execution_trace",
    }
    if reward is not None and (_number(reward) is None or not 0 <= float(reward) <= 1):
        raise ValueError("harbor_reward must be a finite value in [0, 1]")
    if trace_score is not None and (
        _number(trace_score) is None
        or not 0 <= trace_score <= 100
    ):
        raise ValueError("trace_score must be a finite value in [0, 100]")
    if raw_score is not None and not 0 <= raw_score <= 100:
        raise ValueError("raw_score must be within [0, 100]")
    if effective_score is not None and not -1 <= effective_score <= 100:
        raise ValueError("effective_score must be within [-1, 100]")
    if penalty_applied:
        if raw_score is None or effective_score is None or penalty_value != -1:
            raise ValueError("an applied penalty requires raw/effective scores and a -1 point delta")
        if abs(effective_score - (raw_score - 1)) > 1e-9:
            raise ValueError("effective_score must equal raw_score minus one for a penalty")
    complete = (
        observation["replay_status"] == "present"
        and observation["results_status"] in {"present", "redacted"}
        and observation["scorecard_status"] in {"present", "redacted"}
        and observation["leaderboard_status"] == "present"
        and observation["admission_status"] == "admitted"
        and raw_score is not None
        and effective_score is not None
        and penalty_declared
        and owner not in (None, "")
        and bundle_revision not in (None, "")
        and str(rescore_status or "").lower() == "completed"
        and isinstance(anti_cheat, dict)
        and anti_cheat.get("mode") in {"weighted_three_signals", "weightedThreeSignals"}
        and isinstance(anti_cheat.get("signals"), list)
        and len(anti_cheat["signals"]) == 3
    )
    observation["state"] = "confirmed" if complete else "unknown_needs_reconcile"
    observation["unknown_reason"] = None if complete else "incomplete platform evidence or fresh rescore pending"
    return observation


def write_atomic(path: Path, value: dict[str, Any]) -> None:
    path = path.resolve()
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, temporary = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
    try:
        with os.fdopen(fd, "w", encoding="utf-8", newline="\n") as handle:
            json.dump(value, handle, ensure_ascii=True, indent=2, sort_keys=True)
            handle.write("\n")
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
    parser.add_argument("--challenge-id", required=True)
    parser.add_argument("--attempt-id", required=True)
    parser.add_argument("--route", required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--observed-at-ms", type=int, default=None)
    args = parser.parse_args()
    raw = args.response.read_bytes()
    response = json.loads(raw.decode("utf-8"))
    observed_at_ms = args.observed_at_ms
    if observed_at_ms is None:
        observed_at_ms = int(time.time() * 1000)
    observation = build_observation(
        response,
        raw_bytes=raw,
        challenge_id=args.challenge_id,
        attempt_id=args.attempt_id,
        route=args.route,
        observed_at_ms=observed_at_ms,
    )
    write_atomic(args.output, observation)
    print(json.dumps(observation, ensure_ascii=True, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
