# Harness preset 来源说明

`source/` 保存当前 `~/.dsh/.agent-presets` 的 7 个 preset 文本及历史备份，用于逐项对照，不会被 Codex 自动加载。可执行的 Codex 角色在 `../codex-roles/`，并显式标注只读/写权限边界。

原 preset 依赖 Cordis realms、`solver-guard_*`、`subagent-supervisor`、Lark 和 DSH 专用工具；缺少这些运行时只能按 `config/codex-capability-map.md` 降级，不能宣称已复制其强制能力。
