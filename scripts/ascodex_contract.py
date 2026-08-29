#!/usr/bin/env python3
"""Build and validate a typed ASCodex challenge contract from a saved response.

This tool is offline and read-only.  It does not decide that a challenge is usable merely
because a page exists: both `contract_version` and `required_submission` must be observed in
the saved response before the contract can be marked Known.  Optional schema/scoring files are
included in the canonical fingerprint when the operator explicitly supplies them.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path
from typing import Any

from scripts.ascodex_monitor import write_atomic
from scripts.ascodex_platform_client import write_atomic_bytes


SCHEMA_VERSION = "ascodex-coordination/v1"
FINGERPRINT_BYTES = 8


def _decode_json(raw: bytes, description: str) -> Any:
    try:
        return json.loads(raw.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ValueError(f"{description} is not UTF-8 JSON") from error


def _canonical_bytes(value: Any) -> bytes:
    return json.dumps(
        value,
        ensure_ascii=True,
        sort_keys=True,
        separators=(",", ":"),
    ).encode("utf-8")


def _pick(mapping: dict[str, Any], *keys: str) -> Any:
    for key in keys:
        if key in mapping and mapping[key] not in (None, ""):
            return mapping[key]
    return None


def _contract_field(response: dict[str, Any], *keys: str) -> Any:
    value = _pick(response, *keys)
    if value is not None:
        return value
    for parent_key in ("contract", "data", "challenge"):
        parent = response.get(parent_key)
        if isinstance(parent, dict):
            value = _pick(parent, *keys)
            if value is not None:
                return value
    return None


def _optional_int(value: Any, name: str) -> int | None:
    if value is None or value == "":
        return None
    if isinstance(value, bool) or not isinstance(value, int):
        raise ValueError(f"{name} must be an integer")
    if value < 0:
        raise ValueError(f"{name} must be non-negative")
    return value


def _optional_json_hash(path: Path | None, name: str) -> tuple[str | None, Any]:
    if path is None:
        return None, None
    raw = path.read_bytes()
    value = _decode_json(raw, name)
    if isinstance(value, dict):
        return hashlib.sha256(_canonical_bytes(value)).hexdigest(), value
    return hashlib.sha256(raw).hexdigest(), value


def fingerprint_contract(
    *,
    challenge_id: str,
    contract_version: str,
    required_submission: str,
    round_start_ms: int | None,
    round_end_ms: int | None,
    schema: Any = None,
    scoring_contract: Any = None,
) -> tuple[str, dict[str, Any]]:
    if not challenge_id.strip() or not contract_version.strip() or not required_submission.strip():
        raise ValueError("challenge id, contract version, and required submission are required")
    if round_start_ms is not None and round_end_ms is not None and round_end_ms <= round_start_ms:
        raise ValueError("contract round end must be after its start")
    canonical: dict[str, Any] = {
        "challenge_id": challenge_id,
        "contract_version": contract_version,
        "required_submission": required_submission,
    }
    if round_start_ms is not None:
        canonical["round_start_ms"] = round_start_ms
    if round_end_ms is not None:
        canonical["round_end_ms"] = round_end_ms
    if schema is not None:
        canonical["schema"] = schema
    if scoring_contract is not None:
        canonical["scoring_contract"] = scoring_contract
    encoded = _canonical_bytes(canonical)
    fingerprint = hashlib.sha256(encoded).hexdigest()[: FINGERPRINT_BYTES * 2]
    return fingerprint, {
        "fingerprint_input_bytes": encoded,
        "fingerprint_input": canonical,
        "fingerprint_input_sha256": hashlib.sha256(encoded).hexdigest(),
    }


def build_contract(
    *,
    response: dict[str, Any],
    raw_response: bytes,
    challenge_id: str,
    adapter_id: str | None,
    override_version: str | None,
    override_required_submission: str | None,
    override_round_start_ms: int | None,
    override_round_end_ms: int | None,
    schema: Any,
    scoring_contract: Any,
) -> tuple[dict[str, Any], dict[str, Any]]:
    if not isinstance(response, dict):
        raise ValueError("challenge response must be a JSON object")
    response_challenge = _contract_field(response, "challenge_id", "challengeId", "id")
    if response_challenge is not None and str(response_challenge) != challenge_id:
        raise ValueError("challenge response does not match the requested challenge id")

    observed_version = _contract_field(
        response, "contract_version", "contractVersion"
    )
    observed_required = _contract_field(
        response, "required_submission", "requiredSubmission"
    )
    version = str(observed_version if observed_version is not None else override_version or "")
    required = str(
        observed_required
        if observed_required is not None
        else override_required_submission
        or ""
    )
    if observed_version is None or observed_required is None:
        if adapter_id is not None:
            raise ValueError(
                "Known adapter binding requires both contract_version and required_submission "
                "to be observed in the saved response"
            )
        status = "unknown"
    else:
        status = "known"

    round_start = _optional_int(
        _contract_field(response, "round_start_ms", "roundStartMs")
        if _contract_field(response, "round_start_ms", "roundStartMs") is not None
        else override_round_start_ms,
        "round_start_ms",
    )
    round_end = _optional_int(
        _contract_field(response, "round_end_ms", "roundEndMs")
        if _contract_field(response, "round_end_ms", "roundEndMs") is not None
        else override_round_end_ms,
        "round_end_ms",
    )
    if (round_start is None) != (round_end is None):
        raise ValueError("round_start_ms and round_end_ms must be supplied together")

    fingerprint, evidence = fingerprint_contract(
        challenge_id=challenge_id,
        contract_version=version,
        required_submission=required,
        round_start_ms=round_start,
        round_end_ms=round_end,
        schema=schema,
        scoring_contract=scoring_contract,
    )
    contract = {
        "schema_version": SCHEMA_VERSION,
        "challenge_id": challenge_id,
        "contract_version": version,
        "fingerprint": fingerprint,
        "required_submission": required,
        "status": status,
        "adapter_id": adapter_id if status == "known" else None,
        "round_start_ms": round_start,
        "round_end_ms": round_end,
    }
    evidence.update(
        {
            "response_sha256": hashlib.sha256(raw_response).hexdigest(),
            "observed_contract_version": observed_version is not None,
            "observed_required_submission": observed_required is not None,
        }
    )
    return contract, evidence


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--response", type=Path, required=True)
    parser.add_argument("--challenge-id", required=True)
    parser.add_argument("--adapter-id")
    parser.add_argument("--contract-version")
    parser.add_argument("--required-submission")
    parser.add_argument("--round-start-ms", type=int)
    parser.add_argument("--round-end-ms", type=int)
    parser.add_argument("--schema", type=Path)
    parser.add_argument("--scoring-contract", type=Path)
    parser.add_argument("--expected-fingerprint")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--fingerprint-input-output", type=Path, required=True)
    args = parser.parse_args()

    try:
        raw = args.response.read_bytes()
        response = _decode_json(raw, "challenge response")
        if not isinstance(response, dict):
            raise ValueError("challenge response must be a JSON object")
        schema_hash, schema = _optional_json_hash(args.schema, "submission schema")
        scoring_hash, scoring = _optional_json_hash(
            args.scoring_contract, "scoring contract"
        )
        contract, evidence = build_contract(
            response=response,
            raw_response=raw,
            challenge_id=args.challenge_id,
            adapter_id=args.adapter_id,
            override_version=args.contract_version,
            override_required_submission=args.required_submission,
            override_round_start_ms=args.round_start_ms,
            override_round_end_ms=args.round_end_ms,
            schema=schema,
            scoring_contract=scoring,
        )
        if args.expected_fingerprint and contract["fingerprint"] != args.expected_fingerprint:
            raise ValueError("contract fingerprint does not match the expected value")
        fingerprint_input = evidence.pop("fingerprint_input_bytes")
        write_atomic_bytes(args.fingerprint_input_output, fingerprint_input)
        evidence["fingerprint_input_sha256"] = hashlib.sha256(
            fingerprint_input
        ).hexdigest()
        evidence.update(
            {
                "submission_schema_sha256": schema_hash,
                "scoring_contract_sha256": scoring_hash,
            }
        )
        write_atomic(args.output, contract)
        print(
            json.dumps(
                {
                    "contract": contract,
                    "evidence": evidence,
                },
                ensure_ascii=True,
                sort_keys=True,
            )
        )
        return 0
    except (OSError, ValueError, json.JSONDecodeError) as error:
        print(
            json.dumps(
                {
                    "error": str(error),
                    "read_only": True,
                    "platform_write_attempted": False,
                },
                ensure_ascii=True,
                sort_keys=True,
            )
        )
        return 2


if __name__ == "__main__":
    sys.exit(main())
