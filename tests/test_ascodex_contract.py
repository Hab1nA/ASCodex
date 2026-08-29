from __future__ import annotations

import hashlib
import json
from pathlib import Path

import pytest

from scripts.ascodex_contract import (
    build_contract,
    fingerprint_contract,
    main,
)


def response() -> dict[str, object]:
    return {
        "challengeId": "challenge-1",
        "contract": {
            "contractVersion": "v3",
            "requiredSubmission": "arm-bundle",
            "roundStartMs": 100,
            "roundEndMs": 900,
        },
    }


def test_complete_response_builds_known_contract_with_canonical_fingerprint() -> None:
    raw = json.dumps(response(), sort_keys=True).encode()
    contract, evidence = build_contract(
        response=response(),
        raw_response=raw,
        challenge_id="challenge-1",
        adapter_id="adapter-v3",
        override_version=None,
        override_required_submission=None,
        override_round_start_ms=None,
        override_round_end_ms=None,
        schema=None,
        scoring_contract=None,
    )
    canonical = json.dumps(
        {
            "challenge_id": "challenge-1",
            "contract_version": "v3",
            "required_submission": "arm-bundle",
            "round_end_ms": 900,
            "round_start_ms": 100,
        },
        ensure_ascii=True,
        sort_keys=True,
        separators=(",", ":"),
    ).encode()
    expected = hashlib.sha256(canonical).hexdigest()[:16]

    assert contract == {
        "schema_version": "ascodex-coordination/v1",
        "challenge_id": "challenge-1",
        "contract_version": "v3",
        "fingerprint": expected,
        "required_submission": "arm-bundle",
        "status": "known",
        "adapter_id": "adapter-v3",
        "round_start_ms": 100,
        "round_end_ms": 900,
    }
    assert evidence["observed_contract_version"] is True
    assert evidence["observed_required_submission"] is True


def test_missing_required_fields_are_unknown_and_cannot_claim_adapter() -> None:
    payload = response()
    del payload["contract"]
    raw = json.dumps(payload, sort_keys=True).encode()
    with pytest.raises(ValueError, match="Known adapter"):
        build_contract(
            response=payload,
            raw_response=raw,
            challenge_id="challenge-1",
            adapter_id="adapter-v3",
            override_version=None,
            override_required_submission=None,
            override_round_start_ms=None,
            override_round_end_ms=None,
            schema=None,
            scoring_contract=None,
        )

    contract, evidence = build_contract(
        response=payload,
        raw_response=raw,
        challenge_id="challenge-1",
        adapter_id=None,
        override_version="v3",
        override_required_submission="arm-bundle",
        override_round_start_ms=None,
        override_round_end_ms=None,
        schema=None,
        scoring_contract=None,
    )
    assert contract["status"] == "unknown"
    assert contract["adapter_id"] is None
    assert evidence["observed_contract_version"] is False
    assert evidence["observed_required_submission"] is False


def test_optional_contract_files_change_the_fingerprint(tmp_path: Path) -> None:
    schema = tmp_path / "schema.json"
    scoring = tmp_path / "scoring.json"
    schema.write_text(json.dumps({"type": "object"}), encoding="utf-8")
    scoring.write_text(json.dumps({"reward": "harbor"}), encoding="utf-8")
    base = fingerprint_contract(
        challenge_id="challenge-1",
        contract_version="v3",
        required_submission="arm-bundle",
        round_start_ms=None,
        round_end_ms=None,
        schema=None,
        scoring_contract=None,
    )[0]
    expanded = fingerprint_contract(
        challenge_id="challenge-1",
        contract_version="v3",
        required_submission="arm-bundle",
        round_start_ms=None,
        round_end_ms=None,
        schema={"type": "object"},
        scoring_contract={"reward": "harbor"},
    )[0]

    assert base != expanded
    assert len(base) == 16
    assert len(expanded) == 16


def test_cli_writes_typed_contract_and_enforces_expected_fingerprint(
    tmp_path: Path, capsys, monkeypatch
) -> None:
    response_path = tmp_path / "challenge.json"
    response_path.write_text(json.dumps(response(), sort_keys=True), encoding="utf-8")
    output = tmp_path / "contract.json"
    input_output = tmp_path / "fingerprint-input.json"
    argv = [
        "ascodex-contract",
        "--response",
        str(response_path),
        "--challenge-id",
        "challenge-1",
        "--adapter-id",
        "adapter-v3",
        "--expected-fingerprint",
        "0" * 16,
        "--output",
        str(output),
        "--fingerprint-input-output",
        str(input_output),
    ]
    monkeypatch.setattr("sys.argv", argv)
    assert main() == 2
    assert json.loads(capsys.readouterr().out)["error"].startswith("contract fingerprint")
    assert not output.exists()

    payload = response()
    payload["contract"]["roundEndMs"] = 1000
    response_path.write_text(json.dumps(payload, sort_keys=True), encoding="utf-8")
    canonical = json.dumps(
        {
            "challenge_id": "challenge-1",
            "contract_version": "v3",
            "required_submission": "arm-bundle",
            "round_end_ms": 1000,
            "round_start_ms": 100,
        },
        ensure_ascii=True,
        sort_keys=True,
        separators=(",", ":"),
    ).encode()
    expected = hashlib.sha256(canonical).hexdigest()[:16]
    argv[argv.index("--expected-fingerprint") + 1] = expected
    monkeypatch.setattr("sys.argv", argv)
    assert main() == 0
    saved = json.loads(output.read_text(encoding="utf-8"))
    assert saved["fingerprint"] == expected
    assert saved["status"] == "known"
    assert input_output.read_bytes() == canonical
