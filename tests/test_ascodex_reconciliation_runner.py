from __future__ import annotations

import hashlib
import json
import subprocess
from pathlib import Path

import pytest

from scripts.ascodex_reconciliation_runner import run_cycle


def response(attempt_id: str = "attempt-1") -> dict[str, object]:
    return {
        "challengeId": "challenge-1",
        "attemptId": attempt_id,
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
    }


def monitor_context() -> dict[str, object]:
    return {
        "role": "monitor",
        "campaign_id": "campaign-1",
        "challenge_id": "challenge-1",
        "agent_id": "thread-monitor",
        "session_id": "session-monitor",
        "thread_id": "thread-monitor",
    }


def _write_monitor_context(tmp_path: Path) -> Path:
    path = tmp_path / "monitor.json"
    path.write_text(json.dumps(monitor_context()), encoding="utf-8")
    return path


def _write_response(tmp_path: Path, payload: object) -> Path:
    path = tmp_path / "response.json"
    path.write_text(json.dumps(payload, sort_keys=True), encoding="utf-8")
    return path


def test_saved_response_creates_manifest_and_records_local_batch(
    tmp_path: Path, monkeypatch
) -> None:
    payload = response()
    response_path = _write_response(tmp_path, payload)
    context_path = _write_monitor_context(tmp_path)
    raw = json.dumps(payload, sort_keys=True).encode()
    response_hash = hashlib.sha256(raw).hexdigest()
    admin_result = {
        "status": "batch-complete",
        "campaign_id": "campaign-1",
        "challenge_id": "challenge-1",
        "items": [{"status": "applied"}],
        "next_expected_version": 2,
    }
    seen: dict[str, object] = {}

    def fake_run(command, **kwargs):
        seen["command"] = command
        seen["kwargs"] = kwargs
        return subprocess.CompletedProcess(command, 0, json.dumps(admin_result), "")

    monkeypatch.setattr("scripts.ascodex_reconciliation_runner.subprocess.run", fake_run)
    summary = run_cycle(
        challenge_id="challenge-1",
        route="/api/challenges/challenge-1/attempts",
        cursor_position=20,
        observed_at_ms=200,
        output_dir=tmp_path / "artifacts",
        response_path=response_path,
        owned_only=False,
        base_url="https://play.bohrium.com/api",
        timeout_seconds=30,
        query=[],
        expected_owner="agent-1",
        admin_path=tmp_path / "admin.exe",
        ledger_path=tmp_path / "ledger.sqlite",
        monitor_context_path=context_path,
        starting_event_version=1,
    )

    assert summary["status"] == "batch_complete"
    assert summary["item_count"] == 1
    assert summary["applied_count"] == 1
    assert summary["chief_event_recorded"] is True
    assert summary["next_expected_version"] == 2
    assert summary["platform_write_attempted"] is False
    manifest = json.loads(Path(summary["manifest_path"]).read_text(encoding="utf-8"))
    assert manifest[0]["response_sha256"] == response_hash
    assert manifest[0]["state"]["kind"] == "observation"
    command = seen["command"]
    assert command[:2] == [str(tmp_path / "admin.exe"), "reconcile-batch"]
    assert "--manifest" in command
    assert seen["kwargs"]["check"] is False
    assert seen["kwargs"]["capture_output"] is True


def test_live_fetch_requires_explicit_ownership_assertion(tmp_path: Path) -> None:
    with pytest.raises(ValueError, match="owned-only"):
        run_cycle(
            challenge_id="challenge-1",
            route="/api/challenges/challenge-1/attempts",
            cursor_position=20,
            observed_at_ms=200,
            output_dir=tmp_path,
            response_path=None,
            owned_only=False,
            base_url="https://play.bohrium.com/api",
            timeout_seconds=30,
            query=[],
            expected_owner=None,
            admin_path=None,
            ledger_path=None,
            monitor_context_path=None,
            starting_event_version=None,
        )


def test_live_fetch_uses_get_only_client_and_stops_at_manifest(
    tmp_path: Path, monkeypatch
) -> None:
    payload = {"attempts": [response()]}
    raw = json.dumps(payload, sort_keys=True).encode()
    seen: dict[str, object] = {}

    def fake_get_json(url, token, *, timeout_seconds):
        seen["url"] = url
        seen["token"] = token
        seen["timeout"] = timeout_seconds
        return payload, raw, 200

    monkeypatch.setattr(
        "scripts.ascodex_reconciliation_runner.current_process_token",
        lambda: "sentinel-token",
    )
    monkeypatch.setattr(
        "scripts.ascodex_reconciliation_runner.get_json", fake_get_json
    )
    summary = run_cycle(
        challenge_id="challenge-1",
        route="/api/challenges/challenge-1/attempts",
        cursor_position=20,
        observed_at_ms=200,
        output_dir=tmp_path,
        response_path=None,
        owned_only=True,
        base_url="https://play.bohrium.com/api",
        timeout_seconds=17,
        query=[("page", "2")],
        expected_owner="agent-1",
        admin_path=None,
        ledger_path=None,
        monitor_context_path=None,
        starting_event_version=None,
    )

    assert summary["status"] == "manifest_only"
    assert seen["url"] == (
        "https://play.bohrium.com/api/challenges/challenge-1/attempts?page=2"
    )
    assert seen["token"] == "sentinel-token"
    assert seen["timeout"] == 17


def test_empty_page_does_not_call_admin(tmp_path: Path, monkeypatch) -> None:
    response_path = _write_response(tmp_path, {"attempts": []})
    called = False

    def fake_run(*args, **kwargs):
        nonlocal called
        called = True
        raise AssertionError("empty page must not invoke admin")

    monkeypatch.setattr("scripts.ascodex_reconciliation_runner.subprocess.run", fake_run)
    summary = run_cycle(
        challenge_id="challenge-1",
        route="/api/challenges/challenge-1/attempts",
        cursor_position=20,
        observed_at_ms=200,
        output_dir=tmp_path,
        response_path=response_path,
        owned_only=False,
        base_url="https://play.bohrium.com/api",
        timeout_seconds=30,
        query=[],
        expected_owner=None,
        admin_path=tmp_path / "admin.exe",
        ledger_path=tmp_path / "ledger.sqlite",
        monitor_context_path=_write_monitor_context(tmp_path),
        starting_event_version=1,
    )

    assert summary["status"] == "no_items"
    assert summary["chief_event_recorded"] is False
    assert called is False


def test_admin_result_is_bound_to_monitor_campaign_and_challenge(
    tmp_path: Path, monkeypatch
) -> None:
    response_path = _write_response(tmp_path, {"attempts": [response()]})
    context_path = _write_monitor_context(tmp_path)
    admin_result = {
        "status": "batch-complete",
        "campaign_id": "other-campaign",
        "challenge_id": "challenge-1",
        "items": [{"status": "applied"}],
        "next_expected_version": 2,
    }

    def fake_run(command, **kwargs):
        return subprocess.CompletedProcess(command, 0, json.dumps(admin_result), "")

    monkeypatch.setattr("scripts.ascodex_reconciliation_runner.subprocess.run", fake_run)
    with pytest.raises(ValueError, match="campaign"):
        run_cycle(
            challenge_id="challenge-1",
            route="/api/challenges/challenge-1/attempts",
            cursor_position=20,
            observed_at_ms=200,
            output_dir=tmp_path,
            response_path=response_path,
            owned_only=False,
            base_url="https://play.bohrium.com/api",
            timeout_seconds=30,
            query=[],
            expected_owner="agent-1",
            admin_path=tmp_path / "admin.exe",
            ledger_path=tmp_path / "ledger.sqlite",
            monitor_context_path=context_path,
            starting_event_version=1,
        )
