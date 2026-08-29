from __future__ import annotations

import hashlib
import json
from pathlib import Path

import pytest

from scripts.ascodex_monitor import build_observation, write_atomic


def response() -> dict[str, object]:
    return {
        "challengeId": "challenge-1",
        "attemptId": "attempt-1",
        "harbor_replay_executed": True,
        "resultsJson": {"ok": True},
        "scorecard": {"trace_score": 88},
        "leaderboard": {"rank": 1},
        "harbor_reward": 0.91,
        "trace_score": 88,
        "rawScore": 91,
        "effectiveScore": 91,
        "penaltyApplied": False,
        "creditedOwner": {"id": "agent-1"},
        "bundleRevision": "sha256:bundle-v1",
        "rescoreStatus": "completed",
        "traceEvidence": True,
        "leaderboardScope": "season_4",
        "seasonId": "season-4",
        "antiCheat": {
            "mode": "weighted_three_signals",
            "signals": [{"name": "a"}, {"name": "b"}, {"name": "c"}],
        },
    }


def test_observation_hashes_raw_response_and_binds_ids() -> None:
    raw = json.dumps(response(), sort_keys=True).encode()
    observation = build_observation(
        response(),
        raw_bytes=raw,
        challenge_id="challenge-1",
        attempt_id="attempt-1",
        route="/api/attempts/attempt-1",
        observed_at_ms=100,
    )
    assert observation["response_sha256"] == hashlib.sha256(raw).hexdigest()
    assert observation["leaderboard_status"] == "present"
    assert observation["state"] == "confirmed"
    assert observation["credited_owner"] == "agent-1"


@pytest.mark.parametrize(
    ("field", "value"),
    [
        ("challengeId", "other"),
        ("attemptId", "other"),
    ],
)
def test_observation_rejects_unbound_response(field: str, value: object) -> None:
    payload = response()
    payload[field] = value
    raw = json.dumps(payload, sort_keys=True).encode()
    with pytest.raises(ValueError):
        build_observation(
            payload,
            raw_bytes=raw,
            challenge_id="challenge-1",
            attempt_id="attempt-1",
            route="/api/attempts/attempt-1",
            observed_at_ms=100,
        )


@pytest.mark.parametrize(
    ("field", "value"),
    [("harbor_replay_executed", False), ("leaderboard", None), ("antiCheat", None), ("traceEvidence", False)],
)
def test_incomplete_response_is_retained_for_reconciliation(field: str, value: object) -> None:
    payload = response()
    payload[field] = value
    raw = json.dumps(payload, sort_keys=True).encode()
    observation = build_observation(
        payload,
        raw_bytes=raw,
        challenge_id="challenge-1",
        attempt_id="attempt-1",
        route="/api/attempts/attempt-1",
        observed_at_ms=100,
    )
    assert observation["state"] == "unknown_needs_reconcile"


def test_bundle_reupload_waits_for_fresh_rescore() -> None:
    payload = response()
    payload.update(bundleRevision="sha256:new", rescoreStatus="pending")
    raw = json.dumps(payload, sort_keys=True).encode()
    observation = build_observation(payload, raw_bytes=raw, challenge_id="challenge-1", attempt_id="attempt-1", route="/api/attempts/attempt-1", observed_at_ms=100)
    assert observation["state"] == "unknown_needs_reconcile"


def test_penalty_preserves_raw_score_and_subtracts_one() -> None:
    payload = response()
    payload.update(rawScore=88, effectiveScore=87, penalty=-1, penaltyApplied=True, penaltyBasis={"reason": "weighted anti-cheat"})
    raw = json.dumps(payload, sort_keys=True).encode()
    observation = build_observation(payload, raw_bytes=raw, challenge_id="challenge-1", attempt_id="attempt-1", route="/api/attempts/attempt-1", observed_at_ms=100)
    assert observation["raw_score"] == 88
    assert observation["effective_score"] == 87


@pytest.mark.parametrize(
    ("field", "value"),
    [("harbor_replay_executed", "maybe"), ("traceEvidence", "yes")],
)
def test_unknown_text_is_not_treated_as_evidence(field: str, value: object) -> None:
    payload = response()
    payload[field] = value
    raw = json.dumps(payload, sort_keys=True).encode()
    observation = build_observation(
        payload,
        raw_bytes=raw,
        challenge_id="challenge-1",
        attempt_id="attempt-1",
        route="/api/attempts/attempt-1",
        observed_at_ms=100,
    )
    assert observation["state"] == "unknown_needs_reconcile"


def test_replay_alone_does_not_infer_trace_evidence() -> None:
    payload = response()
    payload.pop("traceEvidence")
    raw = json.dumps(payload, sort_keys=True).encode()
    observation = build_observation(
        payload,
        raw_bytes=raw,
        challenge_id="challenge-1",
        attempt_id="attempt-1",
        route="/api/attempts/attempt-1",
        observed_at_ms=100,
    )
    assert observation["replay_status"] == "present"
    assert observation["trace_evidence"] == "unavailable"
    assert observation["state"] == "unknown_needs_reconcile"


def test_observation_write_is_atomic_and_json(tmp_path: Path) -> None:
    target = tmp_path / "ledger" / "observation.json"
    value = {"schema_version": "ascodex-platform-observation/v1", "ok": True}
    write_atomic(target, value)
    assert json.loads(target.read_text(encoding="utf-8")) == value
    assert list(target.parent.glob(".*")) == []
