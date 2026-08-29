from __future__ import annotations

import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]

STALE_SCORING_PATTERNS = {
    "historical -1000 penalty": re.compile(r"(?:-1000|扣\s*1000\s*分|1000\s*分)"),
    "legacy eight anti-cheat rules": re.compile(
        r"(?:反作弊\s*8|8\s*条反作弊|8\s*条规则|8\s*规则)"
    ),
    "legacy trace-score gate": re.compile(r"trace_score\s*(?:≥|>=)\s*70"),
    "legacy fixed scoring formula": re.compile(
        r"harbor_reward\s*(?:×|\*)\s*trace_factor\s*(?:×|\*)\s*100"
    ),
    "legacy linear trace factor": re.compile(
        r"trace_factor\s*=\s*ts\s*(?:/|÷)\s*100"
    ),
}

HISTORICAL_MARKERS = ("历史", "旧", "考古", "禁止", "不得", "不能", "不硬编码")


def active_contract_files() -> list[Path]:
    paths: list[Path] = []
    paths.extend((ROOT / ".agents" / "skills").rglob("SKILL.md"))
    paths.extend((ROOT / "agents" / "codex-roles").glob("*.md"))
    paths.extend((ROOT / "config").glob("*.md"))
    paths.extend((ROOT / "scripts").glob("*.py"))
    return sorted(path for path in paths if path.is_file())


def is_explicitly_historical(line: str) -> bool:
    return any(marker in line for marker in HISTORICAL_MARKERS)


def scoring_contract_violations() -> list[str]:
    violations: list[str] = []
    for path in active_contract_files():
        relative = path.relative_to(ROOT)
        for line_number, line in enumerate(
            path.read_text(encoding="utf-8").splitlines(), start=1
        ):
            for label, pattern in STALE_SCORING_PATTERNS.items():
                if pattern.search(line) and not is_explicitly_historical(line):
                    violations.append(f"{relative}:{line_number}: {label}")
    return violations


def test_active_layers_do_not_reintroduce_stale_scoring_contract() -> None:
    files = active_contract_files()
    assert files, "active scoring contract paths must be discoverable"

    violations = scoring_contract_violations()
    assert violations == []


def test_historical_references_are_allowed_only_with_explicit_marker() -> None:
    assert is_explicitly_historical(
        "历史的 -1000 罚分只能作为旧记录考古，不能作为现行规则。"
    )
    assert not is_explicitly_historical("当前判罚改为 -1000。")
