# ASCodex 部署与客制化计划

目标：将本工作区演化为名为 **ASCodex** 的、可审计的 Codex fork 试验台；迁移 Bohrium Playground 解题知识与 `dsh-solver-guard` 的有效策略，不复制 DSH 运行态。

## 当前基线（已核对）

- 本目录是迁移镜像与 ASCodex 试验台；官方 Codex 固定源码已置于 `codex/` 子树，根目录知识镜像仍不承担运行时安全职责。
- 官方 Codex 审计副本固定在 commit `a847c71a159fca55509fbac1619c9c2294ed4718`。
- 已迁移 32 个技能、7 个 Harness preset 源快照、6 个 Codex 角色、Bohrium 文档/工具/模板；这些是知识和默认行为，不是运行时硬门。
- `dsh-solver-guard` v0.2.1 来源在迁移后继续演进；ASCodex 未修改或执行该外部插件，迁移时仍需按当前源码 digest 独立复核，不能把外部报告等同于本地已实现能力。
- 不迁移 `subagent-supervisor`：Codex 原生 `AgentControl`、spawn/send/wait/resume、lineage、completion watcher 和 graph store 足以承载其职责。

## 实施状态概要

- P0-P1 已完成：来源 SHA256 清单 + 固定 commit 导入 `codex/`（未复制 `.git`/`target`/`node_modules`/运行时目录）。
- P2 已完成原型骨架：`codex/codex-rs/solver-guard` 六门类型化策略、CLI hash/workspace fence、SQLite reservation、版本化 coordination event、identity 级 ledger quota（`reservation_dimensions` 维度表 + fail-closed）、身份池生命周期管理（`ascodex-pool-admin`）、公共 contract gate 收拢、Chief wake 消费面（`chief_wake_requests` 状态机 + `ascodex-wake-consumer` + 常驻 `ascodex-supervisor` 指数退避）、typed `EgressPolicy` egress preflight、OS 级网络 egress 强制 Restricted、编译期 Ed25519 信任根 + policy 验签、recovery canary、榜单复查验证器（`ascodex_leaderboard_check.py`/`ascodex_leaderboard_monitor.py`）与 schema 归一化 typed 注册表（`ascodex_schema.py`）。
- P3 已初步接入：app-server typed request dispatch 前的 Guard RPC preflight；`ASCODEX_SOLVER_MODE=1` 拒绝 command/process/fs 写入、MCP、配置、环境、插件和线程设置绕过面；PreToolUse hook 改写参数后重执行 Guard 防 hook 绕过；`ChannelPolicy.trusted_cli_root`；生产提交工具与真实 executor 仍关闭。
- P3 协作契约：`OodaCycleRecord`/`CycleDirective`/`ClosureEvidence`、租约接线（`actor_leases` + `ascodex-lease-admin`）、只读 monitor 事实账本（`ascodex_monitor.py` + observation/reconciliation 表 + `ascodex_reconciliation*.py`）。
- P4 实验性接入：AgentControl child spawn lineage preflight、Chief-issued `cycle_id` StageBrief、`thread_cycle_bindings` 持久权威、stuck cycle 双 brief 签发、recovery canary resume 门。
- 收束判定：所有无需真实平台写/真实 LLM 会话的设计项已实现并测试通过；以下项**待授权**（不属未完成项）：真实 `solver_guard_submit` executor（平台写授权）、完整 boot→active 的 Core disposable-child 会话派生（真实 LLM 会话）、chief-first 常驻 monitor/AutoPush 接线、Core/app-server 全部状态转移对 `ResearchCycleRecord` 的强制接入、真实榜单 GET 的常驻自动复查。

**构建纪律（跨轮复用，已实证）**：
- Rust 验证一律使用工作区外 `CARGO_TARGET_DIR` + 显式 stable MSVC 工具链（`scripts/ascodex.ps1` 已默认）；工作区内 target 与 nightly 默认工具链在本机文件监控软件干扰下会编译挂起（rustc 线程 Suspended、CPU≈0）。
- `codex-core --lib` 依赖图大，单次 check 约 3-4 分钟，不是挂死。
- `target`/构建缓存不得进入仓库或交付内容。

## 配置缺口

- 仓内 `config/solver-guard-policy-reference.yaml` 是旧迁移摘要格式，不符合 Rust `Policy` 所需的 `channel/identity/cadence/redline/trace/model` 顶层结构；`config/ascodex-solver-policy.example.yaml` 是脱敏 typed fixture，尚无启动自检和不可篡改策略注入。

## 测试边界

- 根 `pytest.ini` 只收集 `tests/`；来源树中名为 `test_gate.py` 的历史 POST 脚本不能被 pytest 导入执行。

## 来源漂移已记录

- DSH 插件已增加官方 ARM trace 字段、组合 trace 检查、API-vs-leaderboard monitor 状态和显式 Harbor score provenance；其"单信号 trace admit"缺陷不迁移。ASCodex 仅迁移已验证语义，不复制 host-plane 运行时。

## 目标架构

```text
ASCodex/
├─ codex/                         # 固定官方 Codex 源码（独立提交基线）
│  ├─ codex-rs/solver-guard/      # Rust 六门策略、broker、SQLite ledger
│  └─ codex-rs/core + app-server  # 统一接入点
├─ skills/ agents/ bohrium-kb/    # 已迁移知识资产
└─ config/                        # 脱敏策略、迁移记录、验收证据
```

设计上的唯一写入口为受 Guard 保护的 `solver_guard.submit`（或等价 core-native tool）；当前实现尚未接入真实写 executor，生产工具仍是 dry-run。普通 skill、MCP、preset、AGENTS.md 只能提供提示和默认配置，不能替代进程内策略。

## 分阶段执行

### P0：冻结来源与可追溯基线

1. 对 `dsh-solver-guard` v0.2.1 生成文件清单、SHA256、Node 版本和测试输出；源目录不改写。
2. 记录官方 Codex commit、构建方式（Windows 优先 WSL2）及许可证/依赖清单。
3. 建立 ASCodex 自己的 Git 根和迁移清单；凭据、sessions、node_modules、logs、ledger 永不进入仓库。

**验收**：两份来源摘要可重复校验；任何文件漂移都能被检测。

### P1：导入官方 Codex，不覆盖镜像

1. 将固定 commit 导入 `codex/` 子树，保留当前知识镜像目录。
2. 完成官方最小构建与现有测试 smoke test，记录平台限制和失败项。
3. 创建 `solver` profile 的配置入口，但默认仍为 dry-run。

**验收**：源码可构建；未启用 Guard 时行为与官方基线一致。

### P2：实现 Rust `solver-guard` crate

1. 定义 `channel/identity/cadence/redline/trace/model` 六门类型化策略，配置使用 `serde_yaml`，禁止复制 Node YAML fallback。
2. 使用 SQLite 事务实现 identity/quota/cadence/attempt reservation、commit/release 和不可变 audit event；写失败、时钟不可用、数据库损坏均 fail-closed。
3. 凭据只以句柄传递；CLI/Bohr 子进程采用最小环境白名单，固定受信任绝对路径与 hash/签名。
4. 建立 workspace canonical-path fence、owner/challenge 绑定、估算成本 reservation；pending/running 作业也计入预算。
5. trace admission 要求真实 execution record、tool call/result 配对、stdout body、单调时间和 artifact hash/provenance 的最小组合证据，禁止 synthetic provenance。

**验收**：crate 单测覆盖缺字段、冲突、重启恢复、并发 reservation、越界路径、不可读 artifact、规则损坏等场景。

### P3：接入所有 Codex 执行面

1. 在 Core Tool Registry 的统一 dispatch broker 执行 Guard preflight/admission。
2. 在 app-server 的 `command/exec`、`process/spawn`、`fs/writeFile/create/remove/copy`、MCP、dynamic tools、settings、plugin/environment/config RPC 入口复用同一 broker；solver profile 默认拒绝或要求管理员授权。
3. 对 Python/脚本执行施加 OS sandbox 与网络 egress policy；命令字符串 deny-list 只能作为纵深防御。
4. Hook 安装失败、challenge/quota 元数据不可读、提交后核验不完整时阻断，不得 best-effort 放行。

**验收**：逐项 RPC 绕过测试证明不会绕过六门；管理员授权和审计事件可追溯。

### P4：绑定 AgentControl 与解题角色

1. 将 `bohrium-solver/intel/judge/red-team/monitor` 角色映射到受控 spawn；child 只能产出 artifact/报告，不能直接 reserve/submit。
2. Guard 记录 parent/child lineage、owner、model/provider snapshot、最大深度、ephemeral 禁止策略和 workspace。
3. 复用 rollout-trace、AgentGraphStore 和 completion watcher；跨重启状态只认 SQLite ledger。
4. 在 AgentControl 上增加 ledger 驱动的 coordination service：按 `intake -> verifier -> brief -> experiment -> observation -> normalize -> decision` 推进，不另造 supervisor。
5. 生成有大小上限和来源 digest 的阶段 `StageBrief`；只选择当前阶段的 A 类技能，旧 DSH 工具名必须经过 capability map，已清理的 worker 链拒绝路由。
6. 用结构化 `end_proposed + ClosureEvidence` 驱动收关；平台事件 chief-first，solver 只接收脱敏副本。

**验收**：child 直接提交、越深派生、未批准模型/环境覆盖均被阻断；主代理可恢复未完成图。

### P5：端到端 dry-run 与证据矩阵

1. 将 32 个技能按 Codex 工具映射，标记 `native/adapter/unavailable/historical`，不伪造 DSH 工具名；验证 StageBrief 不加载 B/C 类冗余工作流或历史 worker 链。
2. 建立 synthetic trace、红线污染、RPC 绕过、凭据隔离、规则损坏、重启额度恢复、预算并发和 monitor 响应绑定等 integration tests。
3. 只运行 Bohrium 查询和提交 dry-run；验证 challengeId、通道、身份、quota、trace、model 六门输出。

**验收**：所有 P0/P1 测试 clean-green；dry-run 生成完整、可审计但不产生真实 attempt 的证据包。

### P6：受控试运行（需单独授权）

1. 在用户明确批准、确认身份额度和回滚方案后，启用单题目/单身份 canary。
2. 提交后必须核对 replay、`resultsJson`、`scorecard`、`harbor_reward` 与官方榜状态；`submitted`/`queued` 不算成功。
3. 发现任何门失效立即停用 profile，保留审计包，不自动重试或扩大额度。

## 暂不做

- 不复制 DSH sessions、credentials、桌面运行态或独立 supervisor。ScoreWatcher/AutoPush 的目标语义仅在 ASCodex coordination service 内按 typed event/state 重新实现，不复制关键词触发和 host-plane 注入机制。
- 不把普通插件 manifest、skill 文本或角色文档描述为硬性安全能力。
- 不在 P0–P5 阶段访问真实提交写接口，不修改 `C:\Users\XKZ\dsh-plugins\dsh-solver-guard` 源码。

## 评分系统核对（现行口径）

依据 Playground 平台报告对评分口径做只读核对：全站榜/赛季榜统一、判罚改为扣 1 分（保留标记、原始分可见）、榜单显示 user/agent 归属、ARM bundle 重传后重新评分、反作弊加权计分（新增三个信号）、无运行痕迹不入待评队列、匿名读取他人提交关闭、判罚依据可查询、题目页三区和附件状态明确。

已落地：

1. `ascodex-coordination` 增加 typed reconciliation reducer（cursor 单调、dedup、冲突 fail-closed、stale 不回滚），`ReconciliationFacts` 分栏保存 raw/effective/penalty/basis/owner/bundle/rescore/scope/anti-cheat/trace/admission 等事实；bundle 重评 pending 或无 trace 时只能进入 `unknown_needs_reconcile`。
2. 判罚语义按"扣 1 分"解释为 delta：`penalty = -1`、`effective_score = raw_score - 1`，禁止旧 `-1000` 哨兵；若真实 API 是绝对分而非 delta，需迁移验证器而不是静默兼容。
3. `scripts/ascodex_monitor.py` 与 `bohrium-kb/tools/verify_attempt.py` 已扩展 raw/effective/penalty/owner/bundle revision/rescore/anti-cheat 字段；证据不足或 pending rescore 不得验证成功。
4. 活跃 skill（oracle-probe、trace-maximize、trace-contamination-redline、platform-scorecard-analyze、submit-attempt）与 monitor 角色、coordination 架构文档已同步新口径；匿名读取他人提交路径标为关闭。
5. 未篡改：`skills/deepseek-harness/` 历史快照、`agents/harness-presets/source/`、外部 `dsh-solver-guard` 源码；三个新反作弊信号的名称/权重保持 unknown，不硬编码旧"8 规则"或固定公式。
