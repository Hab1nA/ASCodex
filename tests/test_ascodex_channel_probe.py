from __future__ import annotations

import hashlib
import json
import sys
from pathlib import Path

import pytest

from scripts.ascodex_channel_probe import build_channel_probe, main


def challenge() -> dict[str, object]:
    return {
        "challengeId": "challenge-1",
        "contract": {
            "graderName": "harbor-grader",
            "s2": False,
        },
    }


def attempts() -> dict[str, object]:
    return {
        "attempts": [
            {
                "attemptId": "attempt-1",
                "challengeId": "challenge-1",
                "harborReward": 0.91,
                "updated_at": "2026-08-29T00:00:00Z",
            }
        ]
    }


def _raw(value: object) -> bytes:
    return json.dumps(value, sort_keys=True).encode()


def test_probe_binds_responses_and_preserves_observed_channel_facts() -> None:
    challenge_raw = _raw(challenge())
    attempts_raw = _raw(attempts())
    probe = build_channel_probe(
        challenge_id="challenge-1",
        challenge_response=challenge(),
        attempts_response=attempts(),
        challenge_raw=challenge_raw,
        attempts_raw=attempts_raw,
        probe_at_ms=1_000_000,
    )

    assert probe["schema_version"] == "ascodex-channel-probe/v1"
    assert probe["challenge_response_sha256"] == hashlib.sha256(challenge_raw).hexdigest()
    assert probe["attempts_response_sha256"] == hashlib.sha256(attempts_raw).hexdigest()
    assert probe["grader_name"] == "harbor-grader"
    assert probe["s2"] is False
    assert probe["grader_registered"] is True
    assert probe["harbor_active"] is True
    assert probe["observed_attempt_count"] == 1
    assert probe["worker_queue_ok"] is True
    assert probe["method"] == "GET"
    assert probe["platform_write_attempted"] is False


def test_missing_signals_remain_unknown_instead_of_false() -> None:
    challenge_raw = _raw({"challengeId": "challenge-1"})
    attempts_raw = _raw({"attempts": []})
    probe = build_channel_probe(
        challenge_id="challenge-1",
        challenge_response={"challengeId": "challenge-1"},
        attempts_response={"attempts": []},
        challenge_raw=challenge_raw,
        attempts_raw=attempts_raw,
        probe_at_ms=1_000_000,
    )

    assert probe["grader_name"] is None
    assert probe["s2"] is None
    assert probe["grader_registered"] is None
    assert probe["harbor_active"] is False
    assert probe["worker_queue_ok"] is None


def test_worker_queue_becomes_stale_after_three_hours() -> None:
    stale_attempts = attempts()
    stale_attempts["attempts"][0]["updated_at"] = "1970-01-01T00:00:00Z"
    probe = build_channel_probe(
        challenge_id="challenge-1",
        challenge_response=challenge(),
        attempts_response=stale_attempts,
        challenge_raw=_raw(challenge()),
        attempts_raw=_raw(stale_attempts),
        probe_at_ms=1_000 + 3 * 60 * 60 * 1000,
    )

    assert probe["worker_queue_ok"] is False


def test_mixed_challenge_page_and_unbound_challenge_are_rejected() -> None:
    with pytest.raises(ValueError, match="another challenge"):
        build_channel_probe(
            challenge_id="challenge-1",
            challenge_response=challenge(),
            attempts_response={"attempts": [{"challengeId": "other"}]},
            challenge_raw=b"{}",
            attempts_raw=b"{}",
            probe_at_ms=1,
        )

    with pytest.raises(ValueError, match="does not match"):
        build_channel_probe(
            challenge_id="challenge-1",
            challenge_response={"challengeId": "other"},
            attempts_response=None,
            challenge_raw=b"{}",
            attempts_raw=None,
            probe_at_ms=1,
        )


def test_live_cli_requires_ownership_assertion(
    tmp_path: Path, monkeypatch, capsys
) -> None:
    monkeypatch.setattr(
        sys,
        "argv",
        [
            "ascodex-channel-probe",
            "--challenge-id",
            "challenge-1",
            "--output",
            str(tmp_path / "probe.json"),
        ],
    )
    assert main() == 2
    result = json.loads(capsys.readouterr().out)
    assert result["read_only"] is True
    assert result["platform_write_attempted"] is False
    assert "owned-only" in result["error"]


def test_live_cli_uses_get_only_client_and_saves_responses(
    tmp_path: Path, monkeypatch, capsys
) -> None:
    challenge_raw = _raw(challenge())
    attempts_raw = _raw(attempts())
    seen: dict[str, object] = {}

    def fake_get_json(url, token, *, timeout_seconds):
        seen["urls"] = [*seen.get("urls", []), url]
        seen["token"] = token
        seen["timeout"] = timeout_seconds
        if url.endswith("/challenges/challenge-1"):
            return challenge(), challenge_raw, 200
        return attempts(), attempts_raw, 200

    monkeypatch.setattr(
        "scripts.ascodex_channel_probe.current_process_token",
        lambda: "sentinel-token",
    )
    monkeypatch.setattr(
        "scripts.ascodex_channel_probe.get_json", fake_get_json
    )
    output = tmp_path / "probe.json"
    monkeypatch.setattr(
        sys,
        "argv",
        [
            "ascodex-channel-probe",
            "--challenge-id",
            "challenge-1",
            "--owned-only",
            "--timeout-seconds",
            "17",
            "--probe-at-ms",
            "1000",
            "--output",
            str(output),
            "--artifact-dir",
            str(tmp_path / "evidence"),
        ],
    )

    assert main() == 0
    probe = json.loads(output.read_text(encoding="utf-8"))
    assert seen["urls"] == [
        "https://play.bohrium.com/api/challenges/challenge-1",
        "https://play.bohrium.com/api/challenges/challenge-1/attempts",
    ]
    assert seen["token"] == "sentinel-token"
    assert seen["timeout"] == 17
    assert probe["platform_write_attempted"] is False
    assert (tmp_path / "evidence" / "responses" / "challenge-challenge-1.json").read_bytes() == challenge_raw
    assert (tmp_path / "evidence" / "responses" / "attempts-challenge-1.json").read_bytes() == attempts_raw
    assert capsys.readouterr().out.strip()
