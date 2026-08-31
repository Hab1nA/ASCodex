"""Typed schema normalization for ASCodex platform responses.

Field extraction is declared as a registry of `FieldSpec`s (canonical name, aliases, value
type, required) instead of per-call hardcoded alias probing. `normalize_object` extracts every
declared field through the registry and fails closed when a required field cannot be resolved
or a value does not match its declared type. Unknown raw keys are ignored (they carry no
declared meaning); a required field with no matching key is an error, never a silent default.
"""

from __future__ import annotations

import math
from dataclasses import dataclass, field
from typing import Any, Literal


@dataclass(frozen=True)
class FieldSpec:
    name: str
    aliases: list[str] = field(default_factory=list)
    value_type: Literal["string", "number", "boolean", "raw"] = "string"
    required: bool = False

    def __post_init__(self) -> None:
        if not self.name.strip():
            raise ValueError("field spec name must be non-empty")
        if self.value_type not in {"string", "number", "boolean", "raw"}:
            raise ValueError(f"field spec {self.name!r} has unknown value type {self.value_type!r}")

    def resolve(self, raw: dict[str, Any]) -> Any:
        value = None
        for key in (self.name, *self.aliases):
            if key in raw:
                value = raw[key]
                break
        if value is None or (isinstance(value, str) and value.strip() == ""):
            if self.required:
                raise ValueError(f"required field {self.name!r} is missing")
            return None
        return _coerce(value, self.value_type, self.name)


def _coerce(value: Any, value_type: str, name: str) -> Any:
    if value_type == "string":
        if isinstance(value, bool):
            raise ValueError(f"field {name!r} expected string, got boolean")
        return str(value)
    if value_type == "number":
        if isinstance(value, bool):
            raise ValueError(f"field {name!r} expected number, got boolean")
        if isinstance(value, (int, float)):
            number = float(value)
            if math.isfinite(number):
                return number
            raise ValueError(f"field {name!r} is not a finite number")
        if isinstance(value, str):
            try:
                number = float(value)
            except ValueError as error:
                raise ValueError(f"field {name!r} is not a number") from error
            if math.isfinite(number):
                return number
        raise ValueError(f"field {name!r} is not a number")
    if value_type == "boolean":
        if isinstance(value, bool):
            return value
        if isinstance(value, (int, float)) and not isinstance(value, bool):
            return value != 0
        if isinstance(value, str):
            return value.strip().lower() in {"1", "true", "yes"}
        raise ValueError(f"field {name!r} is not a boolean")
    return value  # raw


@dataclass(frozen=True)
class SchemaRegistry:
    fields: list[FieldSpec]

    def __post_init__(self) -> None:
        seen: set[str] = set()
        for spec in self.fields:
            if spec.name in seen:
                raise ValueError(f"schema registry has duplicate field {spec.name!r}")
            seen.add(spec.name)


def normalize_object(raw: dict[str, Any], registry: SchemaRegistry) -> dict[str, Any]:
    if not isinstance(raw, dict):
        raise ValueError("normalization input must be a JSON object")
    normalized: dict[str, Any] = {}
    for spec in registry.fields:
        normalized[spec.name] = spec.resolve(raw)
    return normalized
