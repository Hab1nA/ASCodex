"""Tests for the offline leaderboard confirmation tool."""

from __future__ import annotations

import json
from pathlib import Path

import pytest

from scripts.ascodex_leaderboard_check import (
    build_leaderboard_confirmation,
    find_attempt_in_leaderboard,
)


def _entries(*items: dict) -> list[dict]:
    return list(items)


def _entry(
    attempt_id: str = "attempt-1",
    owner: str = "owner-1",
    score: float = 88.0,
    scope: str = "overall",
    **extra: object,
) -> dict:
    entry = {
        "attempt_id": attempt_id,
        "attemptId": attempt_id,
        "credited_owner": owner,
        "owner": owner,
        "raw_score": score,
        "effective_score": score,
        "leaderboard_scope": scope,
        "scope": scope,
        **extra,
    }
    return entry


def test_find_attempt_by_id_and_owner() -> None:
    entries = _entries(
        _entry("attempt-1", "owner-1", 88.0),
        _entry("attempt-2", "owner-2", 95.0),
    )
    hit = find_attempt_in_leaderboard(entries, "attempt-2", owner="owner-2")
    assert hit is not None
    assert hit["attempt_id"] == "attempt-2"
    assert hit["credited_owner"] == "owner-2"


def test_find_attempt_ignores_other_owners() -> None:
    entries = _entries(_entry("attempt-1", "owner-1", 88.0))
    # The same attempt id under a different owner must not match (no anonymous cross-owner read).
    assert find_attempt_in_leaderboard(entries, "attempt-1", owner="owner-2") is None


def test_find_attempt_accepts_flat_response_with_entries() -> None:
    response = {"entries": _entries(_entry("attempt-1", "owner-1", 88.0)), "scope": "overall"}
    hit = find_attempt_in_leaderboard(response, "attempt-1", owner="owner-1")
    assert hit is not None


def test_find_attempt_accepts_paginated_response() -> None:
    response = {
        "data": {"leaderboard": _entries(_entry("attempt-1", "owner-1", 88.0))},
        "page": 1,
    }
    hit = find_attempt_in_leaderboard(response, "attempt-1", owner="owner-1")
    assert hit is not None


def test_confirmation_requires_owner_and_score_match() -> None:
    response = {"entries": _entries(_entry("attempt-1", "owner-1", 88.0))}
    ok = build_leaderboard_confirmation(
        response,
        attempt_id="attempt-1",
        expected_owner="owner-1",
        expected_effective_score=88.0,
        scope="overall",
    )
    assert ok["state"] == "confirmed"
    assert ok["owner_match"] is True
    assert ok["score_match"] is True

    mismatch = build_leaderboard_confirmation(
        response,
        attempt_id="attempt-1",
        expected_owner="owner-1",
        expected_effective_score=99.0,
        scope="overall",
    )
    assert mismatch["state"] == "unknown_needs_reconcile"
    assert mismatch["score_match"] is False


def test_confirmation_rejects_absent_attempt_and_bad_scope() -> None:
    response = {"entries": _entries(_entry("attempt-1", "owner-1", 88.0)), "scope": "season-1"}
    absent = build_leaderboard_confirmation(
        response,
        attempt_id="attempt-missing",
        expected_owner="owner-1",
        expected_effective_score=88.0,
        scope="overall",
    )
    assert absent["state"] == "unknown_needs_reconcile"
    assert absent["found"] is False

    bad_scope = build_leaderboard_confirmation(
        response,
        attempt_id="attempt-1",
        expected_owner="owner-1",
        expected_effective_score=88.0,
        scope="season-2",
    )
    assert bad_scope["state"] == "unknown_needs_reconcile"
    assert bad_scope["scope_match"] is False


def test_confirmation_requires_explicit_owner() -> None:
    response = {"entries": _entries(_entry("attempt-1", "owner-1", 88.0))}
    with pytest.raises(ValueError):
        build_leaderboard_confirmation(
            response,
            attempt_id="attempt-1",
            expected_owner="",
            expected_effective_score=88.0,
            scope="overall",
        )
