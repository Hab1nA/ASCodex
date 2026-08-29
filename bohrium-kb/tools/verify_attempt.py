#!/usr/bin/env python3
"""Verify Playground attempt completion using fixture data or HTTPS GET only.

This tool never submits, scores, deletes, or mutates a Playground object. Live
mode requires an explicit HTTPS base URL on play.bohrium.com and reads the
credential from PLAYGROUND_TOKEN or BOHRIUM_TOKEN in the current process.
"""

from __future__ import annotations

import argparse
import json
import math
import os
import sys
from pathlib import Path
from typing import Any
from urllib.error import HTTPError, URLError
from urllib.parse import quote, urljoin, urlparse
from urllib.request import Request, urlopen


VERIFIED_STATUSES = {"scored", "late_scored", "settled", "completed", "complete"}
PENDING_STATUSES = {"submitted", "queued", "pending", "draft", "processing", "running"}
MAX_RESPONSE_BYTES = 8 * 1024 * 1024
EVIDENCE_STATUSES = {"present", "redacted", "pending", "unavailable", "not_applicable"}


def first_value(payload: dict[str, Any], *names: str) -> Any:
    for name in names:
        if name in payload and payload[name] not in (None, ""):
            return payload[name]
    return None


def normalize_attempt_response(payload: Any) -> dict[str, Any]:
    if not isinstance(payload, dict):
        raise ValueError("attempt response must be a JSON object")
    if first_value(payload, "id", "attemptId", "attempt_id") is not None:
        return payload
    for key in ("attempt", "data", "result"):
        nested = payload.get(key)
        if isinstance(nested, dict):
            try:
                return normalize_attempt_response(nested)
            except ValueError:
                continue
    raise ValueError("attempt response does not contain an attempt object")


def extract_challenge_id(attempt: dict[str, Any]) -> str | None:
    challenge = first_value(attempt, "challengeId", "challenge_id", "challenge")
    if isinstance(challenge, dict):
        challenge = first_value(challenge, "id", "slug", "challengeId", "challenge_id")
    return str(challenge) if challenge not in (None, "") else None


def extract_results(attempt: dict[str, Any]) -> Any:
    value = first_value(attempt, "resultsJson", "results_json", "results")
    if isinstance(value, str):
        try:
            return json.loads(value)
        except json.JSONDecodeError:
            return None
    return value


def extract_scorecard(attempt: dict[str, Any]) -> Any:
    value = first_value(attempt, "scorecard", "scoreCard", "score_card")
    if isinstance(value, str):
        try:
            value = json.loads(value)
        except json.JSONDecodeError:
            return None
    return value if value not in (None, {}, [], "") else None


def _nested_mapping_value(mapping: Any, *names: str) -> Any:
    if not isinstance(mapping, dict):
        return None
    return first_value(mapping, *names)


def _parse_replay(value: Any) -> bool:
    if isinstance(value, bool):
        return value
    if isinstance(value, (int, float)) and not isinstance(value, bool):
        return value == 1
    if isinstance(value, str):
        return value.strip().lower() in {"1", "true"}
    if isinstance(value, dict):
        nested = first_value(value, "executed", "value", "status")
        return _parse_replay(nested)
    return False


def _parse_evidence(value: Any) -> str | None:
    if value is None:
        return None
    if isinstance(value, bool):
        return "present" if value else "unavailable"
    if isinstance(value, (int, float)):
        return "present" if value == 1 else "unavailable"
    if isinstance(value, str):
        normalized = value.strip().lower()
        if normalized in EVIDENCE_STATUSES:
            return normalized
        if normalized in {"1", "true"}:
            return "present"
        if normalized in {"0", "false"}:
            return "unavailable"
    if isinstance(value, dict):
        nested = first_value(value, "status", "availability", "value", "present")
        return _parse_evidence(nested)
    return None


def _valid_anti_cheat(value: Any) -> bool:
    if not isinstance(value, dict):
        return False
    if value.get("mode") not in {"weighted_three_signals", "weightedThreeSignals"}:
        return False
    signals = value.get("signals")
    if not isinstance(signals, list) or len(signals) != 3:
        return False
    names: set[str] = set()
    for signal in signals:
        if not isinstance(signal, dict):
            return False
        name = signal.get("name")
        weight = signal.get("weight")
        if not isinstance(name, str) or not name.strip() or name in names:
            return False
        if (
            isinstance(weight, bool)
            or not isinstance(weight, (int, float))
            or not math.isfinite(float(weight))
            or weight < 0
        ):
            return False
        if _parse_evidence(signal.get("availability")) is None:
            return False
        names.add(name)
    return True


def _find_harbor_reward(attempt: dict[str, Any], scorecard: Any) -> Any:
    candidates = [
        attempt,
        scorecard,
        attempt.get("scoringDetails"),
        attempt.get("scoring_details"),
    ]
    for candidate in candidates:
        value = _nested_mapping_value(candidate, "harbor_reward", "harborReward")
        if value is not None:
            return value
    return None


def _find_value(attempt: dict[str, Any], scorecard: Any, *names: str) -> Any:
    for candidate in (attempt, scorecard, attempt.get("scoringDetails"), attempt.get("scoring_details")):
        if isinstance(candidate, str):
            try:
                candidate = json.loads(candidate)
            except json.JSONDecodeError:
                continue
        value = _nested_mapping_value(candidate, *names)
        if value is not None:
            return value
    return None


def _finite_number(value: Any) -> float | None:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        return None
    value = float(value)
    return value if math.isfinite(value) else None


def attempt_evidence(attempt: dict[str, Any]) -> dict[str, Any]:
    results = extract_results(attempt)
    scorecard = extract_scorecard(attempt)
    replay_value = first_value(
        attempt,
        "harbor_replay_executed",
        "harborReplayExecuted",
        "replay_executed",
        "replayExecuted",
    )
    if replay_value is None:
        replay_value = _nested_mapping_value(
            scorecard,
            "harbor_replay_executed",
            "harborReplayExecuted",
            "replay_executed",
            "replayExecuted",
        )
    replay = _parse_replay(replay_value)
    harbor_reward = _find_harbor_reward(attempt, scorecard)
    raw_score = _finite_number(_find_value(attempt, scorecard, "raw_score", "rawScore", "original_score", "originalScore"))
    effective_score = _finite_number(_find_value(attempt, scorecard, "effective_score", "effectiveScore", "credited_score", "creditedScore"))
    penalty = _finite_number(_find_value(attempt, scorecard, "penalty", "penaltyDelta", "penalty_delta"))
    penalty_applied = _find_value(attempt, scorecard, "penalty_applied", "penaltyApplied", "penalized")
    if isinstance(penalty_applied, str):
        penalty_applied = penalty_applied.strip().lower() in {"1", "true", "yes", "applied", "penalized"}
    else:
        penalty_applied = bool(penalty_applied)
    owner = _find_value(attempt, scorecard, "credited_owner", "creditedOwner", "owner", "agent", "user")
    if isinstance(owner, dict):
        owner = _nested_mapping_value(owner, "id", "name", "username", "handle")
    bundle_revision = _find_value(attempt, scorecard, "bundle_revision", "bundleRevision", "bundle_hash", "bundleHash")
    rescore_status = _find_value(attempt, scorecard, "rescore_status", "rescoreStatus", "bundle_rescore_status")
    anti_cheat = _find_value(attempt, scorecard, "anti_cheat", "antiCheat", "anticheat")
    trace_evidence = _parse_evidence(
        _find_value(
            attempt,
            scorecard,
            "trace_evidence",
            "traceEvidence",
            "execution_trace",
            "executionTrace",
        )
    )
    status = str(first_value(attempt, "status", "state", "outcome") or "").strip().lower().replace("-", "_")
    reward_valid = (
        isinstance(harbor_reward, (int, float))
        and not isinstance(harbor_reward, bool)
        and math.isfinite(float(harbor_reward))
        and 0 <= float(harbor_reward) <= 1
    )
    results_populated = isinstance(results, (dict, list)) and bool(results)
    scorecard_populated = isinstance(scorecard, (dict, list)) and bool(scorecard)
    return {
        "status": status,
        "replay_executed": replay,
        "results_populated": results_populated,
        "scorecard_populated": scorecard_populated,
        "harbor_reward_present": reward_valid,
        "harbor_reward": harbor_reward if reward_valid else None,
        "raw_score": raw_score,
        "effective_score": effective_score,
        "penalty": penalty,
        "penalty_applied": penalty_applied,
        "credited_owner": str(owner) if owner not in (None, "") else None,
        "bundle_revision": str(bundle_revision) if bundle_revision not in (None, "") else None,
        "rescore_status": str(rescore_status).lower() if rescore_status not in (None, "") else None,
        "anti_cheat": anti_cheat,
        "trace_evidence": trace_evidence,
    }


def verify_payload(
    attempt: dict[str, Any],
    expected_challenge_id: str | None = None,
    leaderboard: Any = None,
) -> dict[str, Any]:
    try:
        attempt = normalize_attempt_response(attempt)
    except ValueError as error:
        return {"verified": False, "decision": "invalid_payload", "reasons": [str(error)]}
    attempt_id = first_value(attempt, "id", "attemptId", "attempt_id")
    challenge_id = extract_challenge_id(attempt)
    evidence = attempt_evidence(attempt)
    reasons: list[str] = []
    if attempt_id in (None, ""):
        reasons.append("attempt id is missing")
    if not challenge_id:
        reasons.append("challengeId is missing")
    if expected_challenge_id and challenge_id != expected_challenge_id:
        reasons.append("challengeId does not match the requested challenge")
    if evidence["status"] in PENDING_STATUSES or evidence["status"] not in VERIFIED_STATUSES:
        reasons.append("status is not a terminal scored state")
    if not evidence["replay_executed"]:
        reasons.append("harbor replay was not executed")
    if not evidence["results_populated"]:
        reasons.append("resultsJson is empty")
    if not evidence["scorecard_populated"]:
        reasons.append("scorecard is empty")
    if not evidence["harbor_reward_present"]:
        reasons.append("harbor_reward is missing")
    raw = evidence["raw_score"]
    effective = evidence["effective_score"]
    if raw is not None and not 0 <= raw <= 100:
        reasons.append("raw_score is outside [0, 100]")
    if effective is not None and not -1 <= effective <= 100:
        reasons.append("effective_score is outside [-1, 100]")
    if evidence["penalty_applied"]:
        if raw is None or effective is None or evidence["penalty"] != -1 or abs(effective - (raw - 1)) > 1e-9:
            reasons.append("penalty must preserve raw score and subtract exactly one point")
    if not evidence["bundle_revision"]:
        reasons.append("bundle revision is missing")
    elif evidence["rescore_status"] != "completed":
        reasons.append("bundle revision has no fresh completed rescore")
    if not evidence["credited_owner"]:
        reasons.append("credited owner is missing")
    if not _valid_anti_cheat(evidence["anti_cheat"]):
        reasons.append("anti-cheat evidence is not the weighted three-signal form")
    if evidence["trace_evidence"] != "present":
        reasons.append("execution trace evidence is not present")
    anonymous_access = _find_value(
        attempt,
        extract_scorecard(attempt),
        "anonymous_other_submission_access",
        "anonymousOtherSubmissionAccess",
    )
    if anonymous_access is True or (isinstance(anonymous_access, str) and anonymous_access.lower() == "open"):
        reasons.append("anonymous access to other submissions is closed")
    leaderboard_seen = None
    if leaderboard is not None:
        leaderboard_seen = leaderboard_contains(leaderboard, str(attempt_id))
        if not leaderboard_seen:
            reasons.append("attempt is not present in the supplied leaderboard response")
    return {
        "verified": not reasons,
        "decision": "verified" if not reasons else "unverified",
        "attempt_id": str(attempt_id) if attempt_id not in (None, "") else None,
        "challenge_id": challenge_id,
        "evidence": evidence,
        "leaderboard_seen": leaderboard_seen,
        "credited_owner": evidence["credited_owner"],
        "reasons": reasons,
    }


def leaderboard_contains(payload: Any, attempt_id: str) -> bool:
    if isinstance(payload, dict):
        candidate = first_value(payload, "id", "attemptId", "attempt_id")
        if candidate not in (None, "") and str(candidate) == attempt_id:
            return True
        nested_attempt = payload.get("attempt")
        if nested_attempt is not None and leaderboard_contains(nested_attempt, attempt_id):
            return True
    if isinstance(payload, list):
        return any(leaderboard_contains(entry, attempt_id) for entry in payload)
    if isinstance(payload, dict):
        return any(
            leaderboard_contains(value, attempt_id)
            for key, value in payload.items()
            if key in {"attempts", "data", "results", "items", "entries", "leaderboard"}
        )
    return False


def validate_live_base(base_url: str) -> str:
    parsed = urlparse(base_url)
    try:
        allowed = (
            parsed.scheme.lower() == "https"
            and parsed.hostname == "play.bohrium.com"
            and parsed.port in (None, 443)
            and parsed.username is None
            and parsed.password is None
            and not parsed.query
            and not parsed.fragment
        )
    except ValueError:
        allowed = False
    if not allowed:
        raise ValueError("live base URL must be https://play.bohrium.com")
    return base_url.rstrip("/")


def get_json(url: str, token: str) -> Any:
    request = Request(url, headers={"Accept": "application/json", "Authorization": f"Bearer {token}"}, method="GET")
    with urlopen(request, timeout=30) as response:
        body = response.read(MAX_RESPONSE_BYTES + 1)
        if len(body) > MAX_RESPONSE_BYTES:
            raise ValueError("live response exceeds the 8 MiB safety limit")
        return json.loads(body.decode("utf-8"))


def next_page_url(payload: Any, current_url: str) -> str | None:
    if not isinstance(payload, dict):
        return None
    candidates = [payload.get("next"), payload.get("next_url"), payload.get("nextPage"), payload.get("next_page")]
    links = payload.get("links")
    if isinstance(links, dict):
        candidates.append(links.get("next"))
    pagination = payload.get("pagination")
    if isinstance(pagination, dict):
        candidates.append(pagination.get("next"))
    for candidate in candidates:
        if isinstance(candidate, str) and candidate:
            return urljoin(current_url, candidate)
    for key in ("data", "result"):
        nested = payload.get(key)
        if isinstance(nested, dict):
            candidate = next_page_url(nested, current_url)
            if candidate:
                return candidate
    return None


def safe_error_message(error: BaseException) -> str:
    message = str(error)
    for env_name in ("PLAYGROUND_TOKEN", "BOHRIUM_TOKEN"):
        token = os.environ.get(env_name)
        if token:
            message = message.replace(token, "[redacted-token]")
    return message


def current_process_token() -> str | None:
    return os.environ.get("PLAYGROUND_TOKEN") or os.environ.get("BOHRIUM_TOKEN")


def get_paginated_leaderboard(base_url: str, challenge_id: str, token: str, max_pages: int = 20) -> list[Any]:
    current_url = f"{base_url}/challenges/{quote(challenge_id, safe='')}/attempts"
    responses: list[Any] = []
    visited: set[str] = set()
    for _ in range(max_pages):
        if current_url in visited:
            break
        visited.add(current_url)
        response = get_json(current_url, token)
        responses.append(response)
        candidate = next_page_url(response, current_url)
        if not candidate:
            break
        parsed = urlparse(candidate)
        try:
            allowed = (
                parsed.scheme.lower() == "https"
                and parsed.hostname == "play.bohrium.com"
                and parsed.port in (None, 443)
                and parsed.username is None
                and parsed.password is None
                and not parsed.fragment
            )
        except ValueError:
            allowed = False
        if not allowed:
            break
        current_url = candidate
    return responses


def load_fixture(path: Path) -> tuple[dict[str, Any], Any]:
    payload = json.loads(path.read_text(encoding="utf-8"))
    if isinstance(payload, dict) and isinstance(payload.get("attempt"), dict):
        return payload["attempt"], payload.get("leaderboard")
    if not isinstance(payload, dict):
        raise ValueError("fixture must be an attempt object or {attempt, leaderboard}")
    return payload, None


def main() -> int:
    parser = argparse.ArgumentParser(description="Verify one Playground attempt with fixture data or read-only HTTPS GET")
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument("--fixture", type=Path, help="local JSON fixture; no network")
    source.add_argument("--live", action="store_true", help="perform explicit read-only GET")
    parser.add_argument("--attempt-id", help="attempt id required by --live")
    parser.add_argument(
        "--owned-only",
        action="store_true",
        help="assert that live attempt/leaderboard reads are limited to the current operator's objects",
    )
    parser.add_argument("--challenge-id", help="expected challenge id")
    parser.add_argument("--base-url", default="https://play.bohrium.com/api")
    parser.add_argument("--check-leaderboard", action="store_true")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--root", type=Path, default=Path.cwd())
    args = parser.parse_args()
    try:
        leaderboard = None
        if args.fixture:
            attempt, leaderboard = load_fixture(args.fixture)
        else:
            if not args.attempt_id:
                raise ValueError("--attempt-id is required with --live")
            if not args.owned_only:
                raise ValueError(
                    "--live attempt reads require --owned-only to assert the object belongs to the current operator"
                )
            token = current_process_token()
            if not token:
                raise ValueError("PLAYGROUND_TOKEN or BOHRIUM_TOKEN is required in the current process")
            base_url = validate_live_base(args.base_url)
            attempt = get_json(f"{base_url}/attempts/{quote(args.attempt_id, safe='')}", token)
            if args.check_leaderboard:
                normalized_attempt = normalize_attempt_response(attempt)
                challenge_id = args.challenge_id or extract_challenge_id(normalized_attempt)
                if not challenge_id:
                    raise ValueError("challenge id is required to check leaderboard")
                leaderboard = get_paginated_leaderboard(base_url, challenge_id, token)
        report = verify_payload(attempt, args.challenge_id, leaderboard)
        report["network_used"] = bool(args.live)
        report["network_write_attempted"] = False
        report["read_only"] = True
        report["ownership_asserted"] = bool(args.live and args.owned_only)
        rendered = json.dumps(report, ensure_ascii=False, indent=2)
        print(rendered)
        if args.output:
            root = args.root.resolve()
            output_path = args.output.resolve()
            if root not in output_path.parents:
                raise ValueError("output path must remain inside --root")
            output_path.write_text(rendered + "\n", encoding="utf-8")
        return 0 if report["verified"] else 2
    except (OSError, ValueError, json.JSONDecodeError, HTTPError, URLError) as error:
        report = {
            "verified": False,
            "decision": "verification_error",
            "network_used": bool(args.live),
            "network_write_attempted": False,
            "read_only": True,
            "ownership_asserted": bool(args.live and args.owned_only),
            "reasons": [safe_error_message(error)],
        }
        print(json.dumps(report, ensure_ascii=False, indent=2))
        return 2


if __name__ == "__main__":
    sys.exit(main())

