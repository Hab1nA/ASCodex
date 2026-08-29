# 完整性盘点

## 32 个 Harness 技能

`bio-reproduce`、`bohrium-bohr`、`checkpoint`、`closure-evidence-standard`、`competition-coordinate`、`dft-convergence`、`differential-scoring`、`distill`、`ears-session-lifecycle`、`generate-grader`、`grade-reproduction`、`judge-field-audit`、`llm-reproduce`、`materials-dft-pipeline`、`mp-r-family-solve`、`multi-agent-reproduce`、`oracle-probe`、`platform-scorecard-analyze`、`playground-solve-optimal`、`proof-pipeline`、`proof-verify`、`real-trace-capture`、`red-team-review`、`reproduce-paper`、`reproduce-validate`、`resume`、`score-difficulty`、`sequence-align`、`submit-attempt`、`trace-contamination-redline`、`trace-maximize`、`unstuck-switch-angle`。

每项同时存在于 `skills/deepseek-harness/<name>/SKILL.md` 和 Codex 发现入口 `.agents/skills/<name>/SKILL.md`。

## 7 个 Harness Agent preset

`research-scientist`、`bohrium-solver`、`bohrium-monitor`、`bohrium-intel`、`bohrium-judge-analyst`、`bohrium-red-team`、`minimal-win`。

每个 preset 的当前 `preset.yml`、`agent.cordis.yml` 与历史 `.bak-*` 均保存在 `agents/harness-presets/source/`；可执行角色边界对应 `agents/codex-roles/` 中的 6 个角色，`minimal-win` 因其 DSH 专用工具组合没有单独执行版。

## 核心知识资产

- `bohrium-kb/round3_prep/INDEX.md` 与 `OPERATIONS_PLAYBOOK.md`
- `HARBOR_LAW.md`、`SCORING_TRUTH.md`、`SUBMISSION_PARADIGM.md`
- `TRACE_LAW.md`、`TRACE_99_RECIPE.md`、`TRACE69_VERDICT.md`
- `JARVIS_METHOD.md`、`IDENTITY_POOL.md`、`HARNESS_GUARD_PLUGIN_DESIGN.md`
- `PLUGIN_TEST_FINDINGS.md`、`SKILLS_AGENT_IMPROVEMENTS.md`、`DECISION_LOG.md`
