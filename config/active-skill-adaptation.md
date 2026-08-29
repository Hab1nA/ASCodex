# 活跃技能适配记录

Codex 自动发现 `.agents/skills/`，所以该目录是执行入口；`skills/deepseek-harness/` 仅保留原文以便逐字追溯。已完成的高风险适配如下：

- `submit-attempt`：合并重复 frontmatter；改为当前进程凭据、dry-run；禁止在 Codex 中直接执行 POST/score/legacy worker。
- `bohrium-bohr`：禁止 `setx` 持久化 AccessKey；长任务改用 Codex session/wait；危险 kill/delete 仍需用户授权。
- `red-team-review`：改用 `collaboration.spawn_agent` clean-room 角色。
- `competition-coordinate`：改用 Codex collaboration API，Lark 与 DSH 队列能力缺失时显式降级。
- `oracle-probe`：不再断言 GET 一定不消耗配额；拒绝根据历史示例访问任意 worker IP；Trace 全额门槛以当前实证 ≥70 为准。
- `trace-maximize`：将旧版 ≥80 标为历史，当前门槛改为 ≥70；不存在的外部 validator 改用本地审计器。
- `closure-evidence-standard`：不再把不存在的 `worker-submit-chain` 当作可执行能力。

其余技能保留原文并受根 `AGENTS.md` 约束；遇到 DSH 专用工具名时必须先查 `config/codex-capability-map.md`。
