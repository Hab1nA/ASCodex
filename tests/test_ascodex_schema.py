"""Tests for the typed schema normalization registry."""

from __future__ import annotations

import pytest

from scripts.ascodex_schema import (
    FieldSpec,
    SchemaRegistry,
    normalize_object,
)


def test_field_spec_resolves_canonical_and_aliases() -> None:
    spec = FieldSpec(
        name="attempt_id",
        aliases=["attemptId", "attempt", "id"],
        value_type="string",
        required=True,
    )
    assert spec.resolve({"attemptId": "a-1"}) == "a-1"
    assert spec.resolve({"attempt": "a-2"}) == "a-2"
    assert spec.resolve({"id": 7}) == "7"  # numeric coerced to string
    with pytest.raises(ValueError):
        spec.resolve({})  # required missing


def test_field_spec_type_validation() -> None:
    score = FieldSpec(name="effective_score", aliases=["effectiveScore"], value_type="number", required=False)
    assert score.resolve({"effectiveScore": "88.5"}) == 88.5
    with pytest.raises(ValueError):
        score.resolve({"effectiveScore": "abc"})  # not a number
    assert score.resolve({}) is None  # optional missing


def test_normalize_object_uses_registry_specs() -> None:
    registry = SchemaRegistry(
        fields=[
            FieldSpec("attempt_id", ["attemptId"], "string", True),
            FieldSpec("owner", ["credited_owner", "ownerName"], "string", True),
            FieldSpec("effective_score", ["effectiveScore", "score"], "number", False),
        ]
    )
    raw = {"attemptId": "a-1", "ownerName": "owner-1", "score": 88.0}
    normalized = normalize_object(raw, registry)
    assert normalized["attempt_id"] == "a-1"
    assert normalized["owner"] == "owner-1"
    assert normalized["effective_score"] == 88.0


def test_normalize_object_fails_closed_on_unknown_required_field() -> None:
    registry = SchemaRegistry(
        fields=[
            FieldSpec("attempt_id", ["attemptId"], "string", True),
            FieldSpec("owner", ["credited_owner"], "string", True),
        ]
    )
    # The canonical name itself must also be accepted as a key.
    raw = {"attempt_id": "a-1", "owner": "owner-1"}
    normalized = normalize_object(raw, registry)
    assert normalized["attempt_id"] == "a-1"
    # Missing required owner fails closed.
    with pytest.raises(ValueError):
        normalize_object({"attempt_id": "a-1"}, registry)


def test_schema_registry_rejects_duplicate_or_empty_names() -> None:
    with pytest.raises(ValueError):
        SchemaRegistry(
            fields=[
                FieldSpec("attempt_id", [], "string", True),
                FieldSpec("attempt_id", [], "string", True),
            ]
        )
    with pytest.raises(ValueError):
        SchemaRegistry(fields=[FieldSpec("", [], "string", True)])


def test_normalize_object_rejects_unknown_value_types() -> None:
    # An unknown value type is rejected at spec construction time.
    with pytest.raises(ValueError):
        FieldSpec("x", [], "bogus", False)
    with pytest.raises(ValueError):
        SchemaRegistry(fields=[FieldSpec("x", [], "bogus", False)])


def test_attempt_registry_has_declared_aliases() -> None:
    from scripts.ascodex_schema import attempt_registry

    registry = attempt_registry()
    names = {spec.name for spec in registry.fields}
    assert {"attempt_id", "challenge_id", "owner", "effective_score", "raw_score"} <= names
    raw = {
        "attemptId": "a-1",
        "challengeId": "c-1",
        "creditedOwner": "owner-1",
        "effectiveScore": 88.0,
        "rawScore": 89.0,
    }
    normalized = normalize_object(raw, registry)
    assert normalized["attempt_id"] == "a-1"
    assert normalized["challenge_id"] == "c-1"
    assert normalized["owner"] == "owner-1"
    assert normalized["effective_score"] == 88.0
    assert normalized["raw_score"] == 89.0


def test_challenge_registry_requires_challenge_id() -> None:
    from scripts.ascodex_schema import challenge_registry

    registry = challenge_registry()
    normalized = normalize_object({"challenge_id": "c-1", "title": "t"}, registry)
    assert normalized["challenge_id"] == "c-1"
    with pytest.raises(ValueError):
        normalize_object({"title": "t"}, registry)  # challenge_id required
