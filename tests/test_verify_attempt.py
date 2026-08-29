import json
import sys
from pathlib import Path


TOOLS = Path(__file__).resolve().parents[1] / "bohrium-kb" / "tools"
sys.path.insert(0, str(TOOLS))

from verify_attempt import (  # noqa: E402
    attempt_evidence,
    get_json,
    get_paginated_leaderboard,
    main,
    validate_live_base,
    verify_payload,
)


def scored_attempt() -> dict:
    return {
        "id": 123,
        "challengeId": "challenge-a",
        "status": "scored",
        "harbor_replay_executed": 1,
        "resultsJson": {"value": 1},
        "scorecard": {"trace_score": 88},
        "harbor_reward": 0.91,
        "rawScore": 91,
        "effectiveScore": 91,
        "traceEvidence": True,
        "penaltyApplied": False,
        "creditedOwner": {"id": "agent-1"},
        "bundleRevision": "sha256:bundle-v1",
        "rescoreStatus": "completed",
        "antiCheat": {
            "mode": "weighted_three_signals",
            "signals": [
                {"name": "a", "weight": 0.4, "availability": "present"},
                {"name": "b", "weight": 0.3, "availability": "present"},
                {"name": "c", "weight": 0.3, "availability": "present"},
            ],
        },
    }


def test_scored_attempt_requires_replay_and_results() -> None:
    report = verify_payload(scored_attempt(), "challenge-a", [{"id": 123}])
    assert report["verified"] is True
    assert report["leaderboard_seen"] is True
    assert report["evidence"]["replay_executed"] is True


def test_nested_attempt_response_is_normalized() -> None:
    report = verify_payload({"data": {"attempt": scored_attempt()}}, "challenge-a")
    assert report["verified"] is True


def test_nested_scorecard_reward_and_string_json_are_supported() -> None:
    attempt = scored_attempt()
    attempt["resultsJson"] = '{"value": 1}'
    attempt["scorecard"] = '{"trace_score": 88, "harbor_reward": 0.91}'
    attempt.pop("harbor_reward")
    evidence = attempt_evidence(attempt)
    assert evidence["results_populated"] is True
    assert evidence["scorecard_populated"] is True
    assert evidence["harbor_reward"] == 0.91


def test_replay_can_be_read_from_scorecard() -> None:
    attempt = scored_attempt()
    attempt["scorecard"] = {"trace_score": 88, "harbor_replay_executed": " TRUE ", "harbor_reward": 0.91}
    attempt.pop("harbor_replay_executed")
    evidence = attempt_evidence(attempt)
    assert evidence["replay_executed"] is True


def test_redacted_results_remain_unverified_but_preserve_score_evidence() -> None:
    attempt = {
        "id": 18053,
        "challengeId": "challenge-a",
        "status": "scored",
        "resultsJson": None,
        "scorecard": {"harbor_replay_executed": 1, "harbor_reward": 1.0, "trace_score": 98.75},
        "rawScore": 100,
        "effectiveScore": 100,
        "penaltyApplied": False,
        "creditedOwner": {"id": "agent-1"},
        "bundleRevision": "sha256:bundle-v1",
        "rescoreStatus": "completed",
        "antiCheat": {
            "mode": "weighted_three_signals",
            "signals": [{"name": "a"}, {"name": "b"}, {"name": "c"}],
        },
    }
    report = verify_payload(attempt, "challenge-a")
    assert report["verified"] is False
    assert report["evidence"]["replay_executed"] is True
    assert report["evidence"]["harbor_reward"] == 1.0
    assert "resultsJson is empty" in report["reasons"]


def test_invalid_or_nonfinite_evidence_is_not_populated() -> None:
    attempt = scored_attempt()
    attempt.update(resultsJson="not-json", scorecard="{}", harbor_reward=float("nan"))
    evidence = attempt_evidence(attempt)
    assert evidence["results_populated"] is False
    assert evidence["scorecard_populated"] is False
    assert evidence["harbor_reward_present"] is False


def test_zero_attempt_id_and_normalized_status_are_handled() -> None:
    attempt = scored_attempt()
    attempt.update(id=0, status=" SCORED ", challenge={"slug": "challenge-a"})
    attempt.pop("challengeId")
    report = verify_payload(attempt, "challenge-a")
    assert report["verified"] is True
    assert report["attempt_id"] == "0"


def test_submitted_attempt_is_not_verified() -> None:
    attempt = scored_attempt()
    attempt.update(status="queued", resultsJson=None, scorecard=None, harbor_replay_executed=0)
    report = verify_payload(attempt, "challenge-a")
    assert report["verified"] is False
    assert "harbor replay was not executed" in report["reasons"]
    assert "resultsJson is empty" in report["reasons"]
    assert "scorecard is empty" in report["reasons"]


def test_challenge_mismatch_and_leaderboard_absence_block() -> None:
    report = verify_payload(scored_attempt(), "challenge-b", [{"id": 999}])
    assert report["verified"] is False
    assert "challengeId does not match the requested challenge" in report["reasons"]
    assert "attempt is not present in the supplied leaderboard response" in report["reasons"]


def test_live_url_is_restricted_to_playground_https() -> None:
    assert validate_live_base("https://play.bohrium.com/api") == "https://play.bohrium.com/api"
    for value in (
        "http://play.bohrium.com/api",
        "https://example.com/api",
        "https://47.92.88.121/api",
        "https://play.bohrium.com:8443/api",
        "https://user:pass@play.bohrium.com/api",
        "https://play.bohrium.com/api?x=1",
        "https://play.bohrium.com./api",
    ):
        try:
            validate_live_base(value)
        except ValueError:
            pass
        else:
            raise AssertionError(value)


def test_get_json_uses_get_and_does_not_send_a_body(monkeypatch) -> None:
    seen = {}

    class FakeResponse:
        def __enter__(self):
            return self

        def __exit__(self, *args):
            return False

        def read(self, limit):
            seen["limit"] = limit
            return b'{"ok": true}'

    def fake_urlopen(request, timeout):
        seen["request"] = request
        seen["timeout"] = timeout
        return FakeResponse()

    monkeypatch.setattr("verify_attempt.urlopen", fake_urlopen)
    assert get_json("https://play.bohrium.com/api/attempts/a%2Fb", "sentinel-token") == {"ok": True}
    request = seen["request"]
    assert request.method == "GET"
    assert request.data is None
    assert request.headers["Accept"] == "application/json"
    assert request.headers["Authorization"] == "Bearer sentinel-token"


def test_leaderboard_pagination_stays_on_allowed_host(monkeypatch) -> None:
    responses = [
        {"data": [{"id": 999}], "next": "/api/challenges/challenge-a/attempts?page=2"},
        {"data": [{"id": 123}]},
    ]
    seen = []

    def fake_get(url, token):
        seen.append(url)
        return responses.pop(0)

    monkeypatch.setattr("verify_attempt.get_json", fake_get)
    pages = get_paginated_leaderboard("https://play.bohrium.com/api", "challenge-a", "secret")
    assert len(pages) == 2
    assert seen[1].startswith("https://play.bohrium.com/")


def test_fixture_round_trip_is_local(tmp_path: Path) -> None:
    fixture = tmp_path / "fixture.json"
    fixture.write_text(json.dumps({"attempt": scored_attempt(), "leaderboard": [{"id": 123}]}), encoding="utf-8")
    assert json.loads(fixture.read_text(encoding="utf-8"))["attempt"]["status"] == "scored"


def test_live_attempt_requires_ownership_assertion(monkeypatch, capsys) -> None:
    def fail_network(*args, **kwargs):
        raise AssertionError("network must not be used without ownership assertion")

    monkeypatch.setattr("verify_attempt.get_json", fail_network)
    monkeypatch.setattr("verify_attempt.current_process_token", lambda: "sentinel-token")
    monkeypatch.setattr(
        sys,
        "argv",
        [
            "verify_attempt.py",
            "--live",
            "--attempt-id",
            "123",
        ],
    )
    code = main()
    summary = json.loads(capsys.readouterr().out)
    assert code == 2
    assert summary["network_used"] is True
    assert summary["network_write_attempted"] is False
    assert summary["ownership_asserted"] is False
    assert "owned-only" in summary["reasons"][0]


def test_live_attempt_honors_ownership_assertion(monkeypatch, capsys) -> None:
    attempt = scored_attempt()
    leaderboard = [{"id": 123}]

    def fake_get_json(url, token):
        assert url == "https://play.bohrium.com/api/attempts/123"
        assert token == "sentinel-token"
        return attempt

    def fake_leaderboard(base_url, challenge_id, token):
        assert base_url == "https://play.bohrium.com/api"
        assert challenge_id == "challenge-a"
        assert token == "sentinel-token"
        return leaderboard

    monkeypatch.setattr("verify_attempt.current_process_token", lambda: "sentinel-token")
    monkeypatch.setattr("verify_attempt.get_json", fake_get_json)
    monkeypatch.setattr("verify_attempt.get_paginated_leaderboard", fake_leaderboard)
    monkeypatch.setattr(
        sys,
        "argv",
        [
            "verify_attempt.py",
            "--live",
            "--attempt-id",
            "123",
            "--owned-only",
            "--challenge-id",
            "challenge-a",
            "--check-leaderboard",
        ],
    )
    code = main()
    report = json.loads(capsys.readouterr().out)
    assert code == 0
    assert report["verified"] is True
    assert report["network_used"] is True
    assert report["network_write_attempted"] is False
    assert report["ownership_asserted"] is True


def test_penalty_preserves_raw_score_and_subtracts_one() -> None:
    attempt = scored_attempt()
    attempt.update(rawScore=88, effectiveScore=87, penalty=-1, penaltyApplied=True, penaltyBasis={"reason": "weighted anti-cheat"})
    report = verify_payload(attempt, "challenge-a")
    assert report["verified"] is True
    assert report["evidence"]["raw_score"] == 88
    assert report["evidence"]["effective_score"] == 87
    assert report["evidence"]["penalty"] == -1


def test_bundle_upload_without_fresh_rescore_is_unverified() -> None:
    attempt = scored_attempt()
    attempt.update(bundleRevision="sha256:new", rescoreStatus="pending")
    report = verify_payload(attempt, "challenge-a")
    assert report["verified"] is False
    assert "bundle revision has no fresh completed rescore" in report["reasons"]


def test_unknown_anti_cheat_schema_blocks_verification() -> None:
    attempt = scored_attempt()
    attempt["antiCheat"] = {"mode": "legacy_eight_rules", "signals": []}
    report = verify_payload(attempt, "challenge-a")
    assert report["verified"] is False
    assert "anti-cheat evidence is not the weighted three-signal form" in report["reasons"]


def test_replay_textual_yes_is_not_evidence() -> None:
    attempt = scored_attempt()
    attempt["harbor_replay_executed"] = "yes"
    report = verify_payload(attempt, "challenge-a")
    assert report["verified"] is False
    assert "harbor replay was not executed" in report["reasons"]


def test_missing_or_unknown_trace_evidence_blocks_verification() -> None:
    missing = scored_attempt()
    missing.pop("traceEvidence")
    report = verify_payload(missing, "challenge-a")
    assert report["verified"] is False
    assert "execution trace evidence is not present" in report["reasons"]

    unknown = scored_attempt()
    unknown["traceEvidence"] = "yes"
    report = verify_payload(unknown, "challenge-a")
    assert report["verified"] is False
    assert "execution trace evidence is not present" in report["reasons"]


def test_anti_cheat_signals_require_complete_typed_evidence() -> None:
    attempt = scored_attempt()
    attempt["antiCheat"]["signals"][0].pop("weight")
    report = verify_payload(attempt, "challenge-a")
    assert report["verified"] is False
    assert "anti-cheat evidence is not the weighted three-signal form" in report["reasons"]


def test_generic_score_is_not_treated_as_raw_score() -> None:
    attempt = scored_attempt()
    attempt.pop("rawScore")
    attempt["score"] = 88
    evidence = attempt_evidence(attempt)
    assert evidence["raw_score"] is None
