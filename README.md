# ASCLocal-Codex Bohrium 能力迁移镜像

本工作区把 `C:\Users\XKZ\Documents\VSCode Projects\ASCLocal` 的 Bohrium/Playground 解题经验，以及 `C:\Users\XKZ\.dsh` / DeepSeek Harness 的技能与 Agent 预设，整理为 Codex 可审计、可渐进启用的本地资产。

## ASCodex 试验台

官方 Codex 固定 commit 已导入 `codex/`，项目根已初始化独立 Git（尚未提交）。客制化 Guard 位于 `codex/codex-rs/solver-guard/`，当前提供六门类型化策略、workspace/CLI 完整性校验、trace/artifact/execution 校验、Guard 派生 redline、SQLite reservation/event ledger/actor lease registry、identity 级 quota 与池生命周期管理（`ascodex-pool-admin`）、Chief wake 消费面与有界自动拾取（`ascodex-wake-admin`/`ascodex-wake-consumer`）、solver-profile 网络 egress allowlist（typed `EgressPolicy`）、编译期 Ed25519 策略信任锚（`<policy>.sig` 验签）、Core Tool Registry preflight、app-server RPC preflight 与 AgentControl lineage preflight。协作器位于 `codex/codex-rs/ascodex-coordination/`，提供 `ActorContext`/lease、OODA/封板契约、typed `PlatformObservation` 和四类成功证据约束；StageBrief 将技能/能力图 policy root 与角色 challenge workspace root 分开签名验证，Solver 不因读取 `.agents/skills` 获得仓库根写权限。Core dry-run 已使用实时 session/thread、可信时钟和持久 lease 校验 campaign/challenge/action/identity class；调用者不能自报 runtime identity。已保存的平台只读响应可由 `scripts/ascodex_monitor.py` 生成 typed observation，再由管理员侧 `ascodex-observation-admin` 在 Monitor lease 下原子写入事实账本；这不是联网 watcher，也不会执行提交。生产 `solver_guard_submit` 仍未接入真实 Playground executor、平台榜单客户端或 OS 网络隔离。正式架构见 `config/ascodex-coordination-architecture.md`，部署边界见 `config/ascodex-deployment-plan.md`。

启用当前 app-server/Core 的 solver profile 预检需在进程启动前设置 `ASCODEX_SOLVER_MODE=1`；该开关仅是 P3 过渡机制。solver child 必须指向现有 SQLite ledger 中由有效 Chief lease 签发的 `cycle_id` 与非零 `cycle_event_version`，运行时再用 child 角色解析该 active cycle 内唯一的 StageBrief，并重新核对 Chief 的 `SpawnChild` lease、campaign、challenge、角色、有效期、分离的 policy/challenge roots、capability map 和全部选中 Skill 的 SHA-256，随后才写入独立 developer context。恢复路径还要求同一进程实例的 `ASCODEX_RECOVERY_ID` + `ASCODEX_RUNTIME_INSTANCE_ID` 对应一条已记录的、完成隔离子线程两回合探针的 recovery canary；缺失或过期时 V1/V2 与 app-server `thread/resume` 均 fail-closed。`config/ascodex-solver-policy.example.yaml` 仅用于 schema/dry-run 测试，不能据此宣称完整身份、cadence、trace 和提交账本已启用。

外部 dsh-solver-guard 的最终只读审计基线为 `config/baselines/dsh-solver-guard-final-2026-08-28.sha256.json`（77 个文件）；ASCodex 只吸收其稳定的 contract/reconcile/status/strategy 设计，不复制源侧运行态、审计探针或凭据。

最小验证命令（Windows）：

```powershell
$env:RUSTC = 'C:\Users\XKZ\.rustup\toolchains\stable-x86_64-pc-windows-msvc\bin\rustc.exe'
& 'C:\Users\XKZ\.rustup\toolchains\stable-x86_64-pc-windows-msvc\bin\cargo.exe' test -p codex-ascodex-coordination -p codex-ascodex-runtime -p codex-solver-guard --locked --offline
& 'C:\Users\XKZ\.rustup\toolchains\stable-x86_64-pc-windows-msvc\bin\cargo.exe' check -p codex-core --locked --offline
```

可重复的本地部署入口是 `scripts/ascodex.ps1`：

```powershell
./scripts/ascodex.ps1 -Action check
./scripts/ascodex.ps1 -Action build
./scripts/ascodex.ps1 -Action test
./scripts/ascodex.ps1 -Action run -- exec --help
```

启用 solver profile 必须显式提供本地 typed policy；示例 policy 不能直接启用：

```powershell
./scripts/ascodex.ps1 -Action run -SolverMode -PolicyFile C:/private/ascodex-policy.yaml -LedgerFile C:/private/ascodex.sqlite -CycleId <cycle-id> -CycleEventVersion <cycle-event-version> -CampaignId <campaign-id> -ChallengeId <challenge-id> -- exec
```

脚本只使用离线 Cargo 构建，不写入凭据，不开启真实 Bohrium 提交。

只读平台响应可用 `scripts/ascodex_monitor.py` 固化为协调器观测证据。该工具不联网，要求输入已由只读客户端保存的 JSON 响应；它会校验 challenge/attempt 归属、replay、results、scorecard、leaderboard 和分数范围，并原子写出带响应 SHA-256 的 observation：

```powershell
python scripts/ascodex_monitor.py `
  --response .\private\attempt-response.json `
  --challenge-id <challenge-id> `
  --attempt-id <attempt-id> `
  --route /api/attempts/<attempt-id> `
  --output .\private\platform-observation.json
```

`private/` 仅作本地示例目录，不能提交凭据或运行态；观测文件仍需经过协调器的 typed `PlatformObservation` 校验。

管理员 lease 控制面是独立二进制，未注册为模型工具。它要求绝对 SQLite 路径和本地 `ActorContext` JSON，provision/revoke 与审计事件以单一事务写入；当前仍只服务 dry-run，不开启任何平台写通道：

```powershell
cargo run -p codex-solver-guard --bin ascodex-lease-admin -- inspect `
  --ledger C:\private\ascodex-ledger.sqlite --lease-id <lease-id>
```

Chief 的循环与阶段简报也只能经独立控制面签发。该命令要求已预置的 Chief context、完整 `ResearchCycleRecord`、规范工作区和 capability map；cycle 事件与不可变 brief issuance 在同一 SQLite 事务内提交。它不执行模型、网络或平台写操作：

```powershell
cargo run -p codex-solver-guard --bin ascodex-stage-admin -- issue `
  --ledger C:\private\ascodex-ledger.sqlite `
  --chief-context C:\private\chief-context.json `
  --cycle C:\private\research-cycle.json `
  --workspace C:\Users\XKZ\Documents\VSCode Projects\ASCLocal-Codex `
  --capability-map config\codex-capability-map.md `
  --event-id <event-id> --idempotency-key <idempotency-key>
```

新一轮取代旧 cycle 时必须显式提供 predecessor；旧 cycle 与其所有 brief 会在同一事务中撤销：

```powershell
cargo run -p codex-solver-guard --bin ascodex-stage-admin -- supersede `
  --ledger C:\private\ascodex-ledger.sqlite `
  --chief-context C:\private\chief-context.json `
  --cycle C:\private\next-research-cycle.json `
  --predecessor-cycle-id <old-cycle-id> `
  --workspace C:\Users\XKZ\Documents\VSCode Projects\ASCLocal-Codex `
  --capability-map config\codex-capability-map.md `
  --event-id <event-id> --idempotency-key <idempotency-key>
```

停止一个 cycle 使用 `revoke`，需要 Chief lease、campaign 版本和审计事件；撤销后任何新建或恢复的 solver child 都会拒绝该 brief。

## 已迁移内容

- 复制平台文档、ARM 协议、Round-3 作战复盘和工具源码到 `bohrium-kb/`。
- 复制源工作区的 `_template` 到 `work/_template/`。
- 复制 Harness 的 32 个技能文本到 `skills/deepseek-harness/`。
- 复制 7 个 Harness preset 的 `preset.yml`、`agent.cordis.yml` 及其备份到 `agents/harness-presets/source/`，并提供 Codex 角色适配版。
- 记录脱敏后的配置、规则、能力边界与迁移差异到 `config/`。

## 关键范式

1. 先读题面和官方协议，把第 5/6 节逐字翻译成独立 verifier（JARVIS 方法）。
2. 先判定 Harbor 是字段硬匹配、LLM judge 还是数值/图像 verifier；确定性 oracle 才做单字段 A/B。
3. 每次用新 attempt；提交前做通道、身份、cadence、redline、trace、模型六门检查。
4. Trace 只用真实执行历史；提交后必须核 challengeId、replay、`resultsJson`、`scorecard`、Harbor 分数和榜单收录。
5. 卡死、429、旧 draft 或 trace 低分时止损并切换角度，不重复烧同一身份配额。

## 使用方式

- 先阅读 `bohrium-kb/round3_prep/INDEX.md` 与 `OPERATIONS_PLAYBOOK.md`。
- 按任务加载 `.agents/skills/*/SKILL.md` 活跃适配入口；`skills/deepseek-harness/*/SKILL.md` 仅用于原文追溯。遇到 `solver-guard_*`、`dsh-tool-*`、Lark 或 Playground CLI 名称，先查 `config/codex-capability-map.md` 的 Codex 映射/降级说明。
- `worker-submit-chain` 已被最新 `INDEX.md` 实证清理；迁移 Skill 或旧手册中的残留引用仅作历史追溯，不得作为 ASCodex 提交路径。
- 角色协作按 `agents/codex-roles/` 执行；`research-scientist` 只做编排，`solver` 才能提出写操作，monitor/judge/intel/red-team 默认只读。
- 任何真实提交或删除操作都要先向用户报告目标、身份、预算、dry-run 结果和回滚方案。

## 明确未迁移

没有复制任何 DSH token、凭据、sessions、attachments、ledger、node_modules、桌面运行时或正在运行的模型状态。原工作区的 archive/README 与实际目录存在漂移，镜像以可见文件为准，不宣称历史归档完整。
