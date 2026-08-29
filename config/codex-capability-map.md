# Codex 适配矩阵

| Harness 能力 | 本镜像/当前 Codex 状态 | 处理方式 |
|---|---|---|
| `playground-solve-optimal` 等 32 个技能 | 技能文本已复制；DSH 工具名不会自动出现 | 读取后使用 shell、浏览器和本地脚本等价实现 |
| `bohrium-bohr` / `submit-attempt` | Codex 已有通用 Bohrium job/file 等技能；Playground challenge API 需本地工具 | token 只走环境变量；提交默认 dry-run |
| `verify_attempt` | 已实现本地 fixture 与受限 HTTPS GET | 只读核对 challengeId、终态、replay、`resultsJson`、`scorecard`、Harbor 分数和可选榜单 |
| solver-guard 六门 | Core/app-server 已有 fail-closed preflight；真实 executor 仍关闭 | dry-run 统一走 `solver_guard_submit`；不得把文档当成强制门 |
| ScoreWatcher / AutoPush | 没有持久后台 watcher | 仅在用户要求时运行一次性、可见的状态检查 |
| SkillInjector | 没有动态注入器；协调器已有 OODA 阶段模型 | 当前按阶段选择最小 Skill brief；未来由协调服务生成有大小上限的 typed brief |
| subagent-supervisor | AgentControl 已有 lineage、消息、等待与恢复基础；无 ASCodex 持久循环服务 | 不迁移独立 supervisor；在 AgentControl 之上补 ledger 驱动的协调服务 |
| Cordis presets | Codex 不加载 `agent.cordis.yml` | 使用 `agents/codex-roles/*.md`；source 目录只用于对照 |
| Lark Intel / 事件长连接 | 当前会话未连接 Lark | 保留角色边界，缺少连接时明确降级为人工/只读查询 |

## 版本漂移

源技能中仍有个别旧阈值（例如 trace ≥80）；当前经验文档的实证门槛为 ≥70。执行时以最新 `TRACE_LAW.md`、`SCORING_TRUTH.md` 和 live attempt 证据为准。

`worker-submit-chain` 已被 `round3_prep/INDEX.md` 标记为实证推翻并清理。`OPERATIONS_PLAYBOOK.md` 和迁移版 `submit-attempt` 中残留的 worker 路径只作历史追溯，ASCodex 阶段路由器必须拒绝把它作为默认或降级提交路径；在新的、可验证的平台证据和用户明确授权出现前，不得重新启用。
