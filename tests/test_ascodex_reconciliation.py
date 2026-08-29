from __future__ import annotations

import sys
import hashlib
import json

import pytest

from scripts.ascodex_reconciliation import build_reconciliation_item, build_reconciliation_items, main


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
        "traceEvidence": True,
        "rawScore": 91,
        "effectiveScore": 91,
        "penaltyApplied": False,
        "creditedOwner": {"id": "agent-1"},
        "bundleRevision": "bundle-v1",
        "rescoreStatus": "completed",
        "leaderboardScope": "unified_overall_and_season",
        "antiCheat": {
            "mode": "weighted_three_signals",
            "signals": [
                {"name": "a", "weight": 0.4},
                {"name": "b", "weight": 0.3},
                {"name": "c", "weight": 0.3},
            ],
        },
        "anonymousOtherSubmissionAccess": False,
    }


def _build(payload: dict[str, object], raw: bytes):
    return build_reconciliation_item(
        payload,
        raw_bytes=raw,
        challenge_id="challenge-1",
        attempt_id="attempt-1",
        route="/api/attempts/attempt-1",
        cursor_position=17,
        observed_at_ms=200,
    )


def test_complete_response_becomes_observation_item() -> None:
    payload = response()
    raw = json.dumps(payload, sort_keys=True).encode()
    item = _build(payload, raw)

    assert item["cursor"] == {
        "stream_id": "challenge-1/attempts",
        "position": 17,
    }
    assert item["response_sha256"] == hashlib.sha256(raw).hexdigest()
    assert item["state"]["kind"] == "observation"
    assert item["state"]["observation"]["response_sha256"] == item["response_sha256"]
    assert item["facts"]["leaderboard_scope"] == "unified_overall_and_season"
    assert item["facts"]["rescore_status"] == "completed"
    assert item["facts"]["anti_cheat"]["mode"] == "weighted_three_signals"
    assert item["facts"]["anonymous_other_submission_access"] == "closed"


def test_matching_expected_owner_remains_confirmed() -> None:
    payload = response()
    raw = json.dumps(payload, sort_keys=True).encode()
    item = build_reconciliation_item(
        payload,
        raw_bytes=raw,
        challenge_id="challenge-1",
        attempt_id="attempt-1",
        route="/api/attempts/attempt-1",
        cursor_position=17,
        observed_at_ms=200,
        expected_owner="agent-1",
    )

    assert item["state"]["kind"] == "observation"
    assert item["facts"]["credited_owner"] == "agent-1"


def test_owner_mismatch_forces_reconciliation() -> None:
    payload = response()
    raw = json.dumps(payload, sort_keys=True).encode()
    item = build_reconciliation_item(
        payload,
        raw_bytes=raw,
        challenge_id="challenge-1",
        attempt_id="attempt-1",
        route="/api/attempts/attempt-1",
        cursor_position=17,
        observed_at_ms=200,
        expected_owner="agent-2",
    )

    assert item["state"]["kind"] == "unknown_needs_reconcile"
    assert "credited owner does not match the expected operator" in item["state"]["reason"]
    assert item["facts"]["credited_owner"] == "agent-1"


def test_manifest_keys_match_rust_deny_unknown_fields_contract() -> None:
    payload = response()
    raw = json.dumps(payload, sort_keys=True).encode()
    item = _build(payload, raw)

    assert set(item) == {
        "cursor",
        "challenge_id",
        "attempt_id",
        "route",
        "observed_at_ms",
        "response_sha256",
        "facts",
        "state",
    }
    assert set(item["cursor"]) == {"stream_id", "position"}
    assert set(item["state"]) == {"kind", "observation"}
    assert set(item["state"]["observation"]) == {
        "attempt_id",
        "challenge_id",
        "route",
        "observed_at_ms",
        "response_sha256",
        "replay_status",
        "results_status",
        "scorecard_status",
        "leaderboard_status",
        "harbor_reward",
        "trace_score",
    }
    allowed_facts = {
        "raw_score",
        "effective_score",
        "penalty",
        "penalty_applied",
        "penalty_basis",
        "credited_owner",
        "bundle_revision",
        "rescore_status",
        "trace_evidence",
        "score_evidence",
        "penalty_evidence",
        "credited_owner_evidence",
        "bundle_evidence",
        "leaderboard_scope",
        "anti_cheat",
        "anonymous_other_submission_access",
        "challenge_page",
    }
    assert set(item["facts"]) <= allowed_facts
    anti_cheat = item["facts"]["anti_cheat"]
    assert set(anti_cheat) == {"mode", "signals"}
    assert all(set(signal) == {"name", "weight", "availability"} for signal in anti_cheat["signals"])


def test_missing_schema_fields_remain_unknown_and_preserve_valid_facts() -> None:
    payload = response()
    payload.pop("antiCheat")
    payload["bundleRevision"] = "bundle-v2"
    payload["rescoreStatus"] = "pending"
    raw = json.dumps(payload, sort_keys=True).encode()
    item = _build(payload, raw)

    assert item["state"]["kind"] == "unknown_needs_reconcile"
    assert "weighted anti-cheat evidence unverified" in item["state"]["reason"]
    assert "bundle revision or fresh rescore unverified" in item["state"]["reason"]
    assert item["facts"]["bundle_revision"] == "bundle-v2"
    assert item["facts"]["rescore_status"] == "pending"


def test_partial_penalty_is_not_fabricated() -> None:
    payload = response()
    payload.update(rawScore=88, effectiveScore=87, penalty=-1, penaltyApplied=True)
    payload.pop("penaltyBasis", None)
    raw = json.dumps(payload, sort_keys=True).encode()
    item = _build(payload, raw)

    assert item["state"]["kind"] == "unknown_needs_reconcile"
    assert "penalty delta and basis unverified" in item["state"]["reason"]
    assert "penalty" not in item["facts"]
    assert "penalty_applied" not in item["facts"]
    assert "penalty_basis" not in item["facts"]


def test_missing_execution_trace_forces_unknown() -> None:
    payload = response()
    payload["traceEvidence"] = False
    raw = json.dumps(payload, sort_keys=True).encode()
    item = _build(payload, raw)

    assert item["state"]["kind"] == "unknown_needs_reconcile"
    assert item["facts"]["trace_evidence"] == "unavailable"


def test_unknown_trace_evidence_status_blocks_confirmation() -> None:
    payload = response()
    payload["traceEvidence"] = "yes"
    raw = json.dumps(payload, sort_keys=True).encode()
    item = _build(payload, raw)

    assert item["state"]["kind"] == "unknown_needs_reconcile"
    assert "execution trace evidence unverified" in item["state"]["reason"]
    assert "trace_evidence" not in item["facts"]


def test_consistent_challenge_page_evidence_is_preserved() -> None:
    payload = response()
    payload["challengePage"] = {
        "challengeSection": "present",
        "mySubmissionsSection": True,
        "leaderboardSection": "present",
        "shareRoute": "/challenge/challenge-1",
        "shareRouteStatus": "present",
        "attachmentStatus": "present",
    }
    raw = json.dumps(payload, sort_keys=True).encode()
    item = _build(payload, raw)

    assert item["state"]["kind"] == "observation"
    assert item["facts"]["challenge_page"] == {
        "challenge_section": "present",
        "my_submissions_section": "present",
        "leaderboard_section": "present",
        "share_route": "/challenge/challenge-1",
        "share_route_status": "present",
        "attachment_status": "present",
    }


def test_inconsistent_share_route_keeps_item_in_reconciliation() -> None:
    payload = response()
    payload["challengePage"] = {
        "challengeSection": "present",
        "mySubmissionsSection": True,
        "leaderboardSection": "present",
        "shareRoute": "/challenge/challenge-1",
        "shareRouteStatus": "unavailable",
        "attachmentStatus": "present",
    }
    raw = json.dumps(payload, sort_keys=True).encode()
    item = _build(payload, raw)

    assert item["state"]["kind"] == "unknown_needs_reconcile"
    assert "challenge page evidence inconsistent" in item["state"]["reason"]
    assert "challenge_page" not in item["facts"]


def test_challenge_attempts_page_becomes_one_item_per_attempt() -> None:
    first = response()
    first["attemptId"] = "attempt-1"
    second = response()
    second["attemptId"] = "attempt-2"
    raw = json.dumps({"attempts": [first, second]}, sort_keys=True).encode()
    items = build_reconciliation_items(
        {"attempts": [first, second]},
        raw_bytes=raw,
        challenge_id="challenge-1",
        route="/api/challenges/challenge-1/attempts",
        cursor_position=20,
        observed_at_ms=200,
    )

    assert [item["attempt_id"] for item in items] == ["attempt-1", "attempt-2"]
    assert [item["cursor"]["position"] for item in items] == [20, 21]
    assert all(item["state"]["kind"] == "observation" for item in items)


def test_challenge_attempts_page_marks_owner_mismatch_for_reconciliation() -> None:
    first = response()
    first["attemptId"] = "attempt-1"
    second = response()
    second["attemptId"] = "attempt-2"
    second["creditedOwner"] = {"id": "agent-2"}
    raw = json.dumps({"attempts": [first, second]}, sort_keys=True).encode()
    items = build_reconciliation_items(
        {"attempts": [first, second]},
        raw_bytes=raw,
        challenge_id="challenge-1",
        route="/api/challenges/challenge-1/attempts",
        cursor_position=20,
        observed_at_ms=200,
        expected_owner="agent-1",
    )

    assert items[0]["state"]["kind"] == "observation"
    assert items[1]["state"]["kind"] == "unknown_needs_reconcile"
    assert (
        "credited owner does not match the expected operator"
        in items[1]["state"]["reason"]
    )


def test_list_response_page_without_attempt_ids_is_rejected() -> None:
    raw = json.dumps({"attempts": [{"challengeId": "challenge-1"}]}, sort_keys=True).encode()
    try:
        build_reconciliation_items(
            {"attempts": [{"challengeId": "challenge-1"}]},
            raw_bytes=raw,
            challenge_id="challenge-1",
            route="/api/challenges/challenge-1/attempts",
            cursor_position=20,
            observed_at_ms=200,
        )
    except ValueError as error:
        assert "attempt_id" in str(error)
    else:
        raise AssertionError("missing attempt_id should fail closed")


def test_challenge_attempts_page_rejects_mixed_challenges() -> None:
    other = response()
    other["attemptId"] = "attempt-other"
    other["challengeId"] = "challenge-other"
    raw = json.dumps({"attempts": [other]}, sort_keys=True).encode()
    try:
        build_reconciliation_items(
            {"attempts": [other]},
            raw_bytes=raw,
            challenge_id="challenge-1",
            route="/api/challenges/challenge-1/attempts",
            cursor_position=40,
            observed_at_ms=200,
        )
    except ValueError as error:
        assert "another challenge" in str(error)
    else:
        raise AssertionError("mixed challenge pages should fail closed")


def test_challenge_attempts_page_rejects_duplicate_attempt_ids() -> None:
    first = response()
    first["attemptId"] = "attempt-duplicate"
    second = response()
    second["attemptId"] = "attempt-duplicate"
    raw = json.dumps({"attempts": [first, second]}, sort_keys=True).encode()
    try:
        build_reconciliation_items(
            {"attempts": [first, second]},
            raw_bytes=raw,
            challenge_id="challenge-1",
            route="/api/challenges/challenge-1/attempts",
            cursor_position=40,
            observed_at_ms=200,
        )
    except ValueError as error:
        assert "duplicate attempt id" in str(error)
    else:
        raise AssertionError("duplicate attempt ids should fail closed")


def _write_response(tmp_path, payload: object):
    path = tmp_path / "response.json"
    path.write_text(json.dumps(payload, sort_keys=True), encoding="utf-8")
    return path


def _run_cli(monkeypatch, arguments: list[str]) -> None:
    monkeypatch.setattr(sys, "argv", ["ascodex-reconciliation", *arguments])
    main()


def test_cli_batch_writes_manifest_without_attempt_id(
    tmp_path, monkeypatch, capsys
) -> None:
    first = response()
    first["attemptId"] = "attempt-1"
    second = response()
    second["attemptId"] = "attempt-2"
    response_path = _write_response(
        tmp_path,
        {"attempts": [first, second]},
    )
    output_path = tmp_path / "items.json"
    _run_cli(
        monkeypatch,
        [
            "--response",
            str(response_path),
            "--challenge-id",
            "challenge-1",
            "--route",
            "/api/challenges/challenge-1/attempts",
            "--cursor-position",
            "20",
            "--observed-at-ms",
            "200",
            "--batch",
            "--output",
            str(output_path),
        ],
    )

    items = json.loads(output_path.read_text(encoding="utf-8"))
    assert [item["attempt_id"] for item in items] == ["attempt-1", "attempt-2"]
    assert [item["cursor"]["position"] for item in items] == [20, 21]
    assert capsys.readouterr().out.strip()


def test_cli_single_attempt_still_requires_attempt_id(
    tmp_path, monkeypatch
) -> None:
    response_path = _write_response(tmp_path, response())
    output_path = tmp_path / "item.json"
    monkeypatch.setattr(
        sys,
        "argv",
        [
            "ascodex-reconciliation",
            "--response",
            str(response_path),
            "--challenge-id",
            "challenge-1",
            "--route",
            "/api/attempts/attempt-1",
            "--cursor-position",
            "17",
            "--observed-at-ms",
            "200",
            "--output",
            str(output_path),
        ],
    )
    with pytest.raises(ValueError, match="attempt_id is required"):
        main()


def test_cli_list_page_requires_explicit_batch_flag(
    tmp_path, monkeypatch
) -> None:
    page_attempt = response()
    page_attempt["attemptId"] = "attempt-page"
    response_path = _write_response(
        tmp_path,
        {"attempts": [page_attempt]},
    )
    output_path = tmp_path / "items.json"
    with pytest.raises(ValueError, match="list-shaped response requires --batch"):
        _run_cli(
            monkeypatch,
            [
                "--response",
                str(response_path),
                "--challenge-id",
                "challenge-1",
                "--route",
                "/api/challenges/challenge-1/attempts",
                "--cursor-position",
                "30",
                "--observed-at-ms",
                "200",
                "--output",
                str(output_path),
            ],
        )
    assert not output_path.exists()


def test_cli_batch_accepts_single_object_response(
    tmp_path, monkeypatch
) -> None:
    response_path = _write_response(tmp_path, response())
    output_path = tmp_path / "items.json"
    _run_cli(
        monkeypatch,
        [
            "--response",
            str(response_path),
            "--challenge-id",
            "challenge-1",
            "--attempt-id",
            "attempt-1",
            "--route",
            "/api/attempts/attempt-1",
            "--cursor-position",
            "40",
            "--observed-at-ms",
            "200",
            "--batch",
            "--output",
            str(output_path),
        ],
    )

    items = json.loads(output_path.read_text(encoding="utf-8"))
    assert len(items) == 1
    assert items[0]["attempt_id"] == "attempt-1"
    assert items[0]["cursor"]["position"] == 40
