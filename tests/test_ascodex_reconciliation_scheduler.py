from __future__ import annotations

import json
from pathlib import Path

import pytest

from scripts.ascodex_reconciliation_scheduler import (
    SCHEMA_VERSION,
    run_scheduled_cycles,
)


def monitor_context() -> dict[str, object]:
    return {
        "role": "monitor",
        "campaign_id": "campaign-1",
        "challenge_id": "challenge-1",
        "agent_id": "thread-monitor",
        "session_id": "session-monitor",
        "thread_id": "thread-monitor",
    }


def _context_path(tmp_path: Path) -> Path:
    path = tmp_path / "monitor.json"
    path.write_text(json.dumps(monitor_context()), encoding="utf-8")
    return path


def _summary(
    *,
    applied: int,
    item_count: int = 2,
    version: int = 3,
) -> dict[str, object]:
    return {
        "status": "batch_complete",
        "item_count": item_count,
        "applied_count": applied,
        "chief_event_recorded": applied > 0,
        "next_expected_version": version,
        "response_sha256": "a" * 64,
        "cursor_position": 10,
        "summary_path": "/tmp/summary.json",
    }


def _base_kwargs(tmp_path: Path, context_path: Path, **overrides: object) -> dict[str, object]:
    values = {
        "challenge_id": "challenge-1",
        "route": "/api/challenges/challenge-1/attempts",
        "output_dir": tmp_path / "artifacts",
        "state_file": tmp_path / "state.json",
        "admin_path": tmp_path / "admin.exe",
        "ledger_path": tmp_path / "ledger.sqlite",
        "monitor_context_path": context_path,
        "cursor_position": 10,
        "event_version": 1,
        "base_url": "https://play.bohrium.com/api",
        "timeout_seconds": 30,
        "query": [],
        "expected_owner": "agent-1",
        "interval_ms": 1_000,
        "max_cycles": 2,
        "sleeper": lambda seconds: None,
        "clock": lambda: 1.0,
    }
    values.update(overrides)
    return values


def test_scheduler_advances_state_and_emits_only_applied_wakes(
    tmp_path: Path, monkeypatch
) -> None:
    context_path = _context_path(tmp_path)
    summaries = [
        _summary(applied=2, version=3),
        _summary(applied=0, version=3),
    ]

    def fake_run_cycle(**kwargs):
        assert kwargs["owned_only"] is True
        assert kwargs["response_path"] is None
        assert kwargs["cursor_position"] == (10 if len(summaries) == 2 else 12)
        assert kwargs["starting_event_version"] == (1 if len(summaries) == 2 else 3)
        return summaries.pop(0)

    monkeypatch.setattr(
        "scripts.ascodex_reconciliation_scheduler.run_cycle", fake_run_cycle
    )
    state_file = tmp_path / "state.json"
    result = run_scheduled_cycles(**_base_kwargs(tmp_path, context_path, state_file=state_file))

    assert result["failed"] is False
    assert [item["applied_count"] for item in result["results"]] == [2, 0]
    assert result["results"][0]["chief_wake_path"] is not None
    assert result["results"][1]["chief_wake_path"] is None
    state = json.loads(state_file.read_text(encoding="utf-8"))
    assert state["schema_version"] == SCHEMA_VERSION
    assert state["cursor_position"] == 14
    assert state["event_version"] == 3
    assert state["last_wake_event_version"] == 3
    assert state["cycles_completed"] == 2
    wake_path = Path(result["results"][0]["chief_wake_path"])
    wake = json.loads(wake_path.read_text(encoding="utf-8"))
    assert wake["schema_version"] == "ascodex-chief-wake-request/v1"
    assert wake["campaign_id"] == "campaign-1"
    assert wake["stream_id"] == "challenge-1/attempts"
    assert wake["reason"] == "platform_reconciliation_applied"
    assert wake["platform_write_attempted"] is False


def test_scheduler_failure_stops_and_preserves_authoritative_counters(
    tmp_path: Path, monkeypatch
) -> None:
    context_path = _context_path(tmp_path)

    def fake_run_cycle(**kwargs):
        if kwargs["starting_event_version"] == 1:
            return _summary(applied=1, version=2)
        raise ValueError("platform unavailable")

    monkeypatch.setattr(
        "scripts.ascodex_reconciliation_scheduler.run_cycle", fake_run_cycle
    )
    state_file = tmp_path / "state.json"
    result = run_scheduled_cycles(**_base_kwargs(tmp_path, context_path, state_file=state_file))

    assert result["failed"] is True
    assert result["results"][-1]["status"] == "failed"
    state = json.loads(state_file.read_text(encoding="utf-8"))
    assert state["cursor_position"] == 12
    assert state["event_version"] == 2
    assert state["cycles_completed"] == 1
    assert state["last_error"] == "platform unavailable"


def test_scheduler_rejects_changed_monitor_context(tmp_path: Path) -> None:
    context_path = _context_path(tmp_path)
    state_file = tmp_path / "state.json"
    state_file.write_text(
        json.dumps(
            {
                "schema_version": SCHEMA_VERSION,
                "challenge_id": "challenge-1",
                "route": "/api/challenges/challenge-1/attempts",
                "campaign_id": "campaign-1",
                "monitor_context_sha256": "0" * 64,
                "cursor_position": 10,
                "event_version": 1,
                "last_wake_event_version": 0,
                "cycles_completed": 0,
                "updated_at_ms": 1,
                "last_status": None,
                "last_summary_path": None,
                "last_error": None,
            }
        ),
        encoding="utf-8",
    )
    with pytest.raises(ValueError, match="monitor context changed"):
        run_scheduled_cycles(**_base_kwargs(tmp_path, context_path, state_file=state_file))


def test_scheduler_requires_safe_repeat_interval(tmp_path: Path) -> None:
    context_path = _context_path(tmp_path)
    with pytest.raises(ValueError, match="interval_ms"):
        run_scheduled_cycles(
            **_base_kwargs(tmp_path, context_path, interval_ms=999, max_cycles=2)
        )
