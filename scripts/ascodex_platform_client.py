#!/usr/bin/env python3
"""Read-only Playground client for auditable ASCodex monitor inputs.

The client is intentionally narrow: GET only, one allow-listed host, JSON
responses, and an explicit opt-in for attempt-level reads that may return an
object not known to belong to the current operator.  It never submits, uploads,
deletes, or triggers scoring.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import tempfile
import time
from pathlib import Path
from typing import Any
from urllib.error import HTTPError, URLError
from urllib.parse import urlencode, urlparse
from urllib.request import HTTPRedirectHandler, Request, build_opener


ALLOWED_HOST = "play.bohrium.com"
DEFAULT_BASE_URL = "https://play.bohrium.com/api"
MAX_RESPONSE_BYTES = 8 * 1024 * 1024
MAX_TIMEOUT_SECONDS = 60
ENDPOINT_TEMPLATES = {
    "whoami": "/auth/me",
    "challenge": "/challenges/{challenge_id}",
    "challenge_attempts": "/challenges/{challenge_id}/attempts",
    "attempts": "/attempts",
    "attempt": "/attempts/{attempt_id}",
    "attempt_score": "/attempts/{attempt_id}/score",
    "attempt_bundle_status": "/attempts/{attempt_id}/bundle/status",
    "attempt_bundle_manifest": "/attempts/{attempt_id}/bundle/manifest",
}
ATTEMPT_ID_ENDPOINTS = {
    "attempt",
    "attempt_score",
    "attempt_bundle_status",
    "attempt_bundle_manifest",
}
OWNERSHIP_ASSERTED_ENDPOINTS = ATTEMPT_ID_ENDPOINTS | {"challenge_attempts", "attempts"}


def _clean_id(value: str) -> str:
    value = value.strip()
    if not value:
        raise ValueError("endpoint id cannot be empty")
    if "/" in value or "\\" in value or value in {".", ".."}:
        raise ValueError("endpoint id must not contain a path separator")
    return value


def validate_base_url(base_url: str) -> str:
    try:
        parsed = urlparse(base_url)
    except ValueError as error:
        raise ValueError("invalid base URL") from error
    valid = (
        parsed.scheme.lower() == "https"
        and parsed.hostname == ALLOWED_HOST
        and parsed.port in (None, 443)
        and parsed.username is None
        and parsed.password is None
        and parsed.path in ("", "/", "/api")
        and not parsed.query
        and not parsed.fragment
    )
    if not valid:
        raise ValueError(f"base URL must be https://{ALLOWED_HOST}/api")
    return f"https://{ALLOWED_HOST}/api"


def build_request_url(
    endpoint: str,
    *,
    attempt_id: str | None = None,
    challenge_id: str | None = None,
    query: list[tuple[str, str]] | None = None,
) -> str:
    if endpoint not in ENDPOINT_TEMPLATES:
        raise ValueError("endpoint is not allow-listed")
    if endpoint in ATTEMPT_ID_ENDPOINTS and not attempt_id:
        raise ValueError(f"{endpoint} requires --attempt-id")
    if endpoint in {"challenge", "challenge_attempts"} and not challenge_id:
        raise ValueError(f"{endpoint} requires --challenge-id")
    template = ENDPOINT_TEMPLATES[endpoint]
    path = template.format(
        attempt_id=_clean_id(attempt_id) if "{attempt_id}" in template else None,
        challenge_id=_clean_id(challenge_id) if "{challenge_id}" in template else None,
    )
    if endpoint == "attempts" and not query:
        raise ValueError("attempts endpoint requires an author query parameter")
    if query:
        cleaned_query = []
        for key, value in query:
            if not str(key).strip() or str(value) == "":
                raise ValueError("query parameters must have non-empty keys and values")
            cleaned_query.append((str(key), str(value)))
        if endpoint == "attempts" and not any(
            key.strip().lower() == "author" for key, _ in cleaned_query
        ):
            raise ValueError("attempts endpoint requires an author query parameter")
        return f"https://{ALLOWED_HOST}/api{path}?{urlencode(cleaned_query)}"
    return f"https://{ALLOWED_HOST}/api{path}"


def current_process_token() -> str:
    token = os.environ.get("PLAYGROUND_TOKEN") or os.environ.get("BOHRIUM_TOKEN")
    if not token or not token.strip():
        raise ValueError("PLAYGROUND_TOKEN or BOHRIUM_TOKEN is required in the current process")
    return token


class _SameHostRedirectHandler(HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        parsed = urlparse(newurl)
        if (
            parsed.scheme.lower() != "https"
            or parsed.hostname != ALLOWED_HOST
            or parsed.username
            or parsed.password
        ):
            raise ValueError("blocked cross-origin redirect")
        return super().redirect_request(req, fp, code, msg, headers, newurl)


def get_json(
    url: str,
    token: str,
    *,
    timeout_seconds: int = 30,
) -> tuple[Any, bytes, int]:
    if timeout_seconds <= 0 or timeout_seconds > MAX_TIMEOUT_SECONDS:
        raise ValueError(f"timeout must be between 1 and {MAX_TIMEOUT_SECONDS} seconds")
    request = Request(
        url,
        method="GET",
        headers={
            "Accept": "application/json",
            "Authorization": f"Bearer {token}",
            "User-Agent": "ASCodex-ReadOnlyMonitor/1.0",
        },
    )
    opener = build_opener(_SameHostRedirectHandler)
    with opener.open(request, timeout=timeout_seconds) as response:
        body = response.read(MAX_RESPONSE_BYTES + 1)
        if len(body) > MAX_RESPONSE_BYTES:
            raise ValueError("response exceeds the 8 MiB safety limit")
        try:
            payload = json.loads(body.decode("utf-8"))
        except (UnicodeDecodeError, json.JSONDecodeError) as error:
            raise ValueError("response is not UTF-8 JSON") from error
        if not isinstance(payload, (dict, list)):
            raise ValueError("response JSON must be an object or array")
        return payload, body, int(response.status)


def safe_error_message(error: BaseException) -> str:
    message = str(error)
    for env_name in ("PLAYGROUND_TOKEN", "BOHRIUM_TOKEN"):
        token = os.environ.get(env_name)
        if token:
            message = message.replace(token, "[redacted-token]")
    return message


def write_atomic_bytes(path: Path, value: bytes) -> None:
    path = path.resolve()
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, temporary = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
    try:
        with os.fdopen(fd, "wb") as handle:
            handle.write(value)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, path)
    except BaseException:
        try:
            os.unlink(temporary)
        except FileNotFoundError:
            pass
        raise


def parse_query(value: str) -> tuple[str, str]:
    if "=" not in value:
        raise ValueError(f"invalid query parameter `{value}`; expected key=value")
    key, raw_value = value.split("=", 1)
    if not key or not raw_value:
        raise ValueError("query parameter key and value cannot be empty")
    return key, raw_value


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("endpoint", choices=sorted(ENDPOINT_TEMPLATES))
    parser.add_argument("--attempt-id")
    parser.add_argument("--challenge-id")
    parser.add_argument(
        "--query",
        action="append",
        default=[],
        help="repeatable key=value query parameter",
    )
    parser.add_argument("--base-url", default=DEFAULT_BASE_URL)
    parser.add_argument("--timeout-seconds", type=int, default=30)
    parser.add_argument(
        "--owned-only",
        action="store_true",
        help="assert that attempt-bearing reads are limited to the current operator's objects",
    )
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()

    try:
        validate_base_url(args.base_url)
        if args.endpoint in OWNERSHIP_ASSERTED_ENDPOINTS and not args.owned_only:
            raise ValueError(
                "attempt-bearing reads require --owned-only to assert the objects belong to the current operator"
            )
        url = build_request_url(
            args.endpoint,
            attempt_id=args.attempt_id,
            challenge_id=args.challenge_id,
            query=[parse_query(value) for value in args.query],
        )
        token = "dry-run" if args.dry_run else current_process_token()
        if args.dry_run:
            summary = {
                "dry_run": True,
                "method": "GET",
                "url": url,
                "read_only": True,
                "network_write_attempted": False,
            }
        else:
            payload, body, status = get_json(
                url,
                token,
                timeout_seconds=args.timeout_seconds,
            )
            write_atomic_bytes(args.output, body)
            summary = {
                "dry_run": False,
                "method": "GET",
                "url": url,
                "status": status,
                "response_sha256": hashlib.sha256(body).hexdigest(),
                "response_bytes": len(body),
                "output": str(args.output.resolve()),
                "read_only": True,
                "network_write_attempted": False,
                "payload_type": type(payload).__name__,
            }
        print(json.dumps(summary, ensure_ascii=False, sort_keys=True))
        return 0
    except (HTTPError, OSError, URLError, ValueError, json.JSONDecodeError) as error:
        print(
            json.dumps(
                {
                    "error": safe_error_message(error),
                    "read_only": True,
                    "network_write_attempted": False,
                },
                ensure_ascii=False,
                sort_keys=True,
            )
        )
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
