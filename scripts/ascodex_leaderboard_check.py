"""Offline leaderboard confirmation for the ASCodex monitor loop.

This tool performs no network I/O. A caller must first save the leaderboard response through an
approved read-only client (owned-only), then pass the response file here. It verifies that a
specific owned attempt actually appears in the official leaderboard, that the credited owner and
effective score match, and that the leaderboard scope agrees. It never reads another user's
attempts: the expected owner is required and the attempt id must be present.

Output is a typed confirmation suitable for the coordination ledger:

    {
      "schema_version": "ascodex-leaderboard-confirmation/v1",
      "attempt_id": ...,
      "found": bool,
      "owner_match": bool,
      "score_match": bool,
      "scope_match": bool,
      "scope": ...,
      "state": "confirmed" | "unknown_needs_reconcile",
      "reason": ...,
      "entry": {...} | null
    }
"""

from __future__ import annotations

import argparse
import json
import math
import sys
from pathlib import Path
from typing import Any

from scripts.ascodex_monitor import _first, _nested_first, _number

SCHEMA_VERSION = "ascodex-leaderboard-confirmation/v1"


def _entries_from_response(response: Any) -> list[Any]:
    """Extract leaderboard entries from a saved response, tolerating common shapes."""
    if isinstance(response, list):
        return response
    if not isinstance(response, dict):
        return []
    for key in ("entries", "leaderboard", "leaderboardEntries", "items", "rows"):
        value = response.get(key)
        if isinstance(value, list):
            return value
    nested = _nested_first(response, "data", "result", "payload")
    if isinstance(nested, dict):
        for key in ("entries", "leaderboard", "leaderboardEntries", "items", "rows"):
            value = nested.get(key)
            if isinstance(value, list):
                return value
    return []


def _entry_attempt_id(entry: Any) -> str | None:
    if not isinstance(entry, dict):
        return None
    value = _first(entry, "attempt_id", "attemptId", "attempt", "id")
    return str(value) if value not in (None, "") else None


def _entry_owner(entry: Any) -> str | None:
    if not isinstance(entry, dict):
        return None
    value = _first(entry, "credited_owner", "creditedOwner", "owner", "user", "agent")
    return str(value) if value not in (None, "") else None


def _entry_score(entry: Any) -> float | None:
    if not isinstance(entry, dict):
        return None
    value = _first(entry, "effective_score", "effectiveScore", "score", "points")
    return _number(value)


def find_attempt_in_leaderboard(response: Any, attempt_id: str, owner: str) -> dict[str, Any] | None:
    """Return the leaderboard entry for an owned attempt, or None.

    The owner is mandatory: an attempt id appearing under a different owner is never matched, so
    a misattributed or anonymous entry cannot be mistaken for our own submission.
    """
    owner = (owner or "").strip()
    if not owner or not attempt_id.strip():
        return None
    for entry in _entries_from_response(response):
        if _entry_attempt_id(entry) == attempt_id and _entry_owner(entry) == owner:
            return entry
    return None


def build_leaderboard_confirmation(
    response: Any,
    *,
    attempt_id: str,
    expected_owner: str,
    expected_effective_score: float | None,
    scope: str | None,
) -> dict[str, Any]:
    expected_owner = (expected_owner or "").strip()
    if not expected_owner or not attempt_id.strip():
        raise ValueError("leaderboard confirmation requires an attempt id and explicit owner")
    entry = find_attempt_in_leaderboard(response, attempt_id, expected_owner)
    found = entry is not None
    owner_match = found
    score_match = False
    scope_match = False
    if found:
        entry_score = _entry_score(entry)
        if expected_effective_score is not None and entry_score is not None:
            score_match = abs(entry_score - expected_effective_score) <= 1e-9
        elif expected_effective_score is None:
            # No expected score supplied: presence with an owner match is not enough to confirm
            # a score, so score_match stays False unless a score was actually read.
            score_match = entry_score is not None
        observed_scope = _first(entry, "leaderboard_scope", "leaderboardScope", "scope")
        if scope is None:
            scope_match = True
        else:
            observed = str(observed_scope) if observed_scope not in (None, "") else ""
            scope_match = observed == scope
    confirmed = found and owner_match and score_match and scope_match
    return {
        "schema_version": SCHEMA_VERSION,
        "attempt_id": attempt_id,
        "found": found,
        "owner_match": owner_match,
        "score_match": score_match,
        "scope_match": scope_match,
        "scope": scope,
        "state": "confirmed" if confirmed else "unknown_needs_reconcile",
        "reason": None if confirmed else "attempt absent, owner mismatch, score mismatch, or scope mismatch",
        "entry": entry if found else None,
    }


def write_atomic(path: Path, value: dict[str, Any]) -> None:
    import os
    import tempfile

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
    parser.add_argument("--attempt-id", required=True)
    parser.add_argument("--expected-owner", required=True)
    parser.add_argument("--expected-effective-score", type=float, default=None)
    parser.add_argument("--scope", default=None)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    raw = args.response.read_bytes()
    response = json.loads(raw.decode("utf-8"))
    confirmation = build_leaderboard_confirmation(
        response,
        attempt_id=args.attempt_id,
        expected_owner=args.expected_owner,
        expected_effective_score=args.expected_effective_score,
        scope=args.scope,
    )
    write_atomic(args.output, confirmation)
    print(json.dumps(confirmation, ensure_ascii=True, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    sys.exit(main())
