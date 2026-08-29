from __future__ import annotations

import json
import sys

import pytest

from scripts.ascodex_platform_client import (
    build_request_url,
    get_json,
    main,
    validate_base_url,
)


def test_base_url_is_restricted_to_playground_https_api() -> None:
    assert validate_base_url("https://play.bohrium.com/api") == "https://play.bohrium.com/api"
    assert validate_base_url("https://play.bohrium.com") == "https://play.bohrium.com/api"
    for value in (
        "http://play.bohrium.com/api",
        "https://example.com/api",
        "https://play.bohrium.com:8443/api",
        "https://user:pass@play.bohrium.com/api",
        "https://play.bohrium.com/api?x=1",
        "https://play.bohrium.com./api",
    ):
        try:
            validate_base_url(value)
        except ValueError:
            pass
        else:
            raise AssertionError(value)


def test_request_urls_are_allow_listed_and_path_safe() -> None:
    assert build_request_url("whoami") == "https://play.bohrium.com/api/auth/me"
    assert (
        build_request_url("attempt", attempt_id="abc")
        == "https://play.bohrium.com/api/attempts/abc"
    )
    assert (
        build_request_url(
            "challenge_attempts",
            challenge_id="ch-1",
            query=[("outcome", "stuck"), ("page", "2")],
        )
        == "https://play.bohrium.com/api/challenges/ch-1/attempts?outcome=stuck&page=2"
    )
    assert (
        build_request_url("attempts", query=[("author", "agent-1"), ("limit", "20")])
        == "https://play.bohrium.com/api/attempts?author=agent-1&limit=20"
    )
    for kwargs in (
        {"endpoint": "attempt"},
        {"endpoint": "challenge_attempts"},
        {"endpoint": "attempts"},
        {"endpoint": "attempts", "query": [("limit", "20")]},
        {"endpoint": "attempt", "attempt_id": "../secret"},
        {"endpoint": "attempt", "attempt_id": "a/b"},
    ):
        try:
            build_request_url(**kwargs)
        except ValueError:
            pass
        else:
            raise AssertionError(kwargs)


def test_get_json_uses_get_and_does_not_send_a_body(monkeypatch) -> None:
    seen: dict[str, object] = {}

    class FakeResponse:
        status = 200

        def __enter__(self):
            return self

        def __exit__(self, *args):
            return False

        def read(self, limit):
            seen["limit"] = limit
            return b'{"ok": true}'

    def fake_urlopen(request, timeout):
        seen["method"] = request.method
        seen["data"] = request.data
        seen["timeout"] = timeout
        seen["accept"] = request.headers["Accept"]
        seen["auth"] = request.headers["Authorization"]
        return FakeResponse()

    monkeypatch.setattr(
        "scripts.ascodex_platform_client.build_opener",
        lambda handler: type("Opener", (), {"open": staticmethod(fake_urlopen)}),
    )
    payload, raw, status = get_json(
        "https://play.bohrium.com/api/attempts/a%2Fb",
        "sentinel-token",
        timeout_seconds=30,
    )
    assert payload == {"ok": True}
    assert status == 200
    assert seen["method"] == "GET"
    assert seen["data"] is None
    assert seen["limit"] == 8 * 1024 * 1024 + 1
    assert seen["accept"] == "application/json"
    assert seen["auth"] == "Bearer sentinel-token"
    assert json.dumps(payload)


def _run_cli(monkeypatch, arguments: list[str]) -> int:
    monkeypatch.setattr(sys, "argv", ["ascodex-platform-client", *arguments])
    return main()


def test_cli_dry_run_does_not_require_credentials(monkeypatch, capsys) -> None:
    code = _run_cli(
        monkeypatch,
        ["whoami", "--dry-run", "--output", "unused.json"],
    )
    summary = json.loads(capsys.readouterr().out)
    assert code == 0
    assert summary["dry_run"] is True
    assert summary["method"] == "GET"
    assert summary["read_only"] is True
    assert summary["network_write_attempted"] is False
    assert summary["url"] == "https://play.bohrium.com/api/auth/me"


def test_cli_attempt_scope_requires_ownership_assertion(
    monkeypatch, capsys
) -> None:
    code = _run_cli(
        monkeypatch,
        ["attempt", "--attempt-id", "abc", "--output", "unused.json"],
    )
    summary = json.loads(capsys.readouterr().out)
    assert code == 2
    assert summary["read_only"] is True
    assert summary["network_write_attempted"] is False
    assert "owned-only" in summary["error"]


def test_cli_attempt_scope_honors_ownership_assertion_dry_run(
    monkeypatch, capsys
) -> None:
    code = _run_cli(
        monkeypatch,
        [
            "attempt",
            "--attempt-id",
            "abc",
            "--owned-only",
            "--dry-run",
            "--output",
            "unused.json",
        ],
    )
    summary = json.loads(capsys.readouterr().out)
    assert code == 0
    assert summary["dry_run"] is True
    assert summary["url"] == "https://play.bohrium.com/api/attempts/abc"


def test_cli_challenge_attempts_requires_ownership_assertion(
    monkeypatch, capsys
) -> None:
    code = _run_cli(
        monkeypatch,
        [
            "challenge_attempts",
            "--challenge-id",
            "challenge-1",
            "--output",
            "unused.json",
        ],
    )
    summary = json.loads(capsys.readouterr().out)
    assert code == 2
    assert summary["read_only"] is True
    assert summary["network_write_attempted"] is False
    assert "owned-only" in summary["error"]


def test_cli_challenge_attempts_honors_ownership_assertion_dry_run(
    monkeypatch, capsys
) -> None:
    code = _run_cli(
        monkeypatch,
        [
            "challenge_attempts",
            "--challenge-id",
            "challenge-1",
            "--owned-only",
            "--dry-run",
            "--output",
            "unused.json",
        ],
    )
    summary = json.loads(capsys.readouterr().out)
    assert code == 0
    assert summary["dry_run"] is True
    assert summary["url"] == (
        "https://play.bohrium.com/api/challenges/challenge-1/attempts"
    )


def test_cli_attempts_requires_author_and_ownership(
    monkeypatch, capsys
) -> None:
    code = _run_cli(
        monkeypatch,
        ["attempts", "--query", "limit=20", "--output", "unused.json"],
    )
    summary = json.loads(capsys.readouterr().out)
    assert code == 2
    assert summary["read_only"] is True
    assert "owned-only" in summary["error"]

    code = _run_cli(
        monkeypatch,
        [
            "attempts",
            "--owned-only",
            "--output",
            "unused.json",
        ],
    )
    summary = json.loads(capsys.readouterr().out)
    assert code == 2
    assert "author" in summary["error"]


def test_cli_attempts_honors_author_and_ownership_dry_run(
    monkeypatch, capsys
) -> None:
    code = _run_cli(
        monkeypatch,
        [
            "attempts",
            "--query",
            "author=agent-1",
            "--query",
            "limit=20",
            "--owned-only",
            "--dry-run",
            "--output",
            "unused.json",
        ],
    )
    summary = json.loads(capsys.readouterr().out)
    assert code == 0
    assert summary["dry_run"] is True
    assert summary["url"] == (
        "https://play.bohrium.com/api/attempts?author=agent-1&limit=20"
    )
