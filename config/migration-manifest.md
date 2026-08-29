# 迁移清单与来源

- 来源工作区：`C:\Users\XKZ\Documents\VSCode Projects\ASCLocal`
- 来源 Harness home：`C:\Users\XKZ\.dsh`
- 来源插件源码（仅审计结论，未复制运行时）：`C:\Users\XKZ\dsh-plugins\dsh-solver-guard`、`dsh-subagent-supervisor`
- DSH 全局纪律原文：`config/dsh-AGENTS.source.md`
- 迁移日期：2026-08-27
- 最终外部插件审计基线：`config/baselines/dsh-solver-guard-final-2026-08-28.sha256.json`（77 文件，只读采集；不含运行时复制授权）
- 迁移主体：36 篇平台文档、16 篇作战文档、193 个工具源码文件、32 个技能、7 个 preset 目录及 Codex 角色/测试；同时保留一份 `.agents/skills` 发现入口副本。
- 排除项：凭据、token、sessions、attachments、storages、undo 快照、logs、zip/模型/大型数据、node_modules、桌面运行时
- 技能双副本语义：`skills/deepseek-harness/` 是不可变的 Harness 原文快照；`.agents/skills/` 是 Codex 活跃入口，已对提交、云算力、红队、Trace、判官探针和代理协调规则增加安全适配，因此两处不要求 SHA256 相同。

## 已知源侧问题

源仓库无提交记录；`README/AGENTS/tests` 声称存在 `archive/challenges`、`archive/collab`、`archive/historical`，但当前可见树缺失这些目录；`work/` 仍有若干非 `_template` 目录。镜像保留文档作为历史证据，不自动补造归档。
