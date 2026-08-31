"""Tests for the resident leaderboard confirmation monitor."""

from __future__ import annotations

import json
from pathlib import Path

import pytest

from scripts.ascodex_leaderboard_monitor import (
    CONFIRMATION_SCHEMA_VERSION,
    build_confirmation_state,
    confirmation_filename,
    run_leaderboard_cycles,
    write_confirmation_evidence,
)


def _entries(*items: dict) -> list[dict]:
    return list(items)


def _entry(attempt_id: str = "attempt-1", owner: str = "owner-1", score: float = 88.0) -> dict:
    return {
        "attempt_id": attempt_id,
        "attemptId": attempt_id,
        "credited_owner": owner,
        "owner": owner,
        "effective_score": score,
        "leaderboard_scope": "overall",
        "scope": "overall",
    }


LEADERBOARD_RESPONSE = {"entries": _entries(_entry())}


def test_monitor_writes_confirmation_evidence_for_owned_attempt(tmp_path: Path) -> None:
    evidence_dir = tmp_path / "confirmations"
    wake = write_confirmation_evidence(
        LEADERBOARD_RESPONSE,
        attempt_id="attempt-1",
        expected_owner="owner-1",
        expected_effective_score=88.0,
        scope="overall",
        evidence_dir=evidence_dir,
    )
    assert wake.exists(), "confirmation evidence file must be written"
    parsed = json.loads(wake.read_text(encoding="utf-8"))
    assert parsed["schema_version"] == CONFIRMATION_SCHEMA_VERSION
    assert parsed["confirmation_id"] == "conf-attempt-1"
    assert parsed["attempt_id"] == "attempt-1"
    assert parsed["state"] == "confirmed"
    # Deterministic name derived from attempt + response, not random.
    assert wake.name == confirmation_filename("attempt-1", "overall")


def test_monitor_confirmation_unknown_on_score_mismatch(tmp_path: Path) -> None:
    wake = write_confirmation_evidence(
        LEADERBOARD_RESPONSE,
        attempt_id="attempt-1",
        expected_owner="owner-1",
        expected_effective_score=99.0,
        scope="overall",
        evidence_dir=tmp_path / "confirmations",
    )
    parsed = json.loads(wake.read_text(encoding="utf-8"))
    assert parsed["state"] == "unknown_needs_reconcile"
    assert parsed["score_match"] is False


def test_monitor_requires_explicit_owner(tmp_path: Path) -> None:
    with pytest.raises(ValueError):
        write_confirmation_evidence(
            LEADERBOARD_RESPONSE,
            attempt_id="attempt-1",
            expected_owner="",
            expected_effective_score=88.0,
            scope="overall",
            evidence_dir=tmp_path / "confirmations",
        )


def test_monitor_confirmation_state_advances_cycles() -> None:
    state = build_confirmation_state(initial_cycles=0)
    assert state["cycles_completed"] == 0
    state = build_confirmation_state(initial_cycles=2)
    assert state["cycles_completed"] == 2


def test_monitor_cycles_are_bounded_and_persist_state(tmp_path: Path) -> None:
    state_file = tmp_path / "state.json"
    evidence_dir = tmp_path / "confirmations"
    state = run_leaderboard_cycles(
        response=LEADERBOARD_RESPONSE,
        attempt_id="attempt-1",
        expected_owner="owner-1",
        expected_effective_score=88.0,
        scope="overall",
        evidence_dir=evidence_dir,
        state_file=state_file,
        max_cycles=2,
        interval_ms=1,
    )
    assert state["cycles_completed"] == 2
    assert state_file.exists(), "state must be persisted"
    assert len(list(evidence_dir.glob("confirmation-*.json"))) >= 1