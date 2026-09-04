from pathlib import Path
import re


ROOT = Path(__file__).resolve().parents[1]

# These are intentionally synthetic upstream test fixtures imported with Codex. They contain
# PEM-shaped marker text to exercise parser/error paths, but are not credentials. Keep this
# allowlist exact so a real secret or a new fixture still fails the migration scan.
UPSTREAM_MARKER_FIXTURES = {
    Path("codex/codex-rs/agent-identity/src/lib.rs"),
    Path("codex/codex-rs/login/src/auth/auth_tests.rs"),
    Path("codex/codex-rs/login/src/auth/agent_identity.rs"),
    Path("codex/codex-rs/http-client/tests/fixtures/test-ca-trusted.pem"),
    Path("codex/codex-rs/http-client/tests/fixtures/test-ca.pem"),
    Path("codex/codex-rs/http-client/tests/fixtures/test-intermediate.pem"),
    Path("codex/codex-rs/app-server-protocol/schema/typescript/v2/CliAuthCredentialsStoreMode.ts"),
}


def test_harness_skill_inventory_is_complete() -> None:
    skills = sorted((ROOT / "skills" / "deepseek-harness").glob("*/SKILL.md"))
    assert len(skills) == 32
    active_skills = sorted((ROOT / ".agents" / "skills").glob("*/SKILL.md"))
    # 32 个迁移技能 + ascodex-solve（2026-09-04 ZCode 单会话迁移新增入口）
    assert len(active_skills) == 33
    assert (ROOT / ".agents" / "skills" / "ascodex-solve" / "SKILL.md").is_file()
    assert (ROOT / "skills" / "deepseek-harness" / "playground-solve-optimal" / "SKILL.md").is_file()
    assert (ROOT / "skills" / "deepseek-harness" / "trace-contamination-redline" / "SKILL.md").is_file()


def test_no_runtime_credentials_or_private_keys_are_migrated() -> None:
    forbidden_name = re.compile(r"credentials|\.pem$|\.key$", re.I)
    forbidden_content = re.compile(
        r"BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY|asp_[A-Za-z0-9]{24,}",
        re.I,
    )
    for path in ROOT.rglob("*"):
        # ".mimosa" is local plugin hook state (it snapshots repo files under hashed names);
        # it is runtime residue, not migrated content, and must never gate this scan.
        # "private"/".codex"/"out" 是 gitignored 运行态（凭据、会话转录、E2E 产物），
        # 不属迁移入库内容；本断言的对象是迁移物本身。
        if (
            not path.is_file()
            or ".git" in path.parts
            or "__pycache__" in path.parts
            or ".mimosa" in path.parts
            or "private" in path.parts
            or ".codex" in path.parts
            or "out" in path.parts
        ):
            continue
        relative = path.relative_to(ROOT)
        if "codex" in relative.parts and any(
            part.startswith("target") for part in relative.parts
        ):
            continue
        if forbidden_name.search(path.name) and relative not in UPSTREAM_MARKER_FIXTURES:
            raise AssertionError(path)
        if path.stat().st_size > 2_000_000:
            continue
        try:
            text = path.read_text(encoding="utf-8")
        except UnicodeDecodeError:
            continue
        marker_match = forbidden_content.search(text)
        if marker_match and relative not in UPSTREAM_MARKER_FIXTURES:
            raise AssertionError(path)


def test_codex_role_boundaries_are_present() -> None:
    roles = {p.stem for p in (ROOT / "agents" / "codex-roles").glob("*.md")}
    assert roles == {
        "research-scientist",
        "bohrium-solver",
        "bohrium-monitor",
        "bohrium-judge-analyst",
        "bohrium-red-team",
        "bohrium-intel",
    }
