# ASCodex 协作架构

日期：2026-08-28  
状态：试验台设计与可测试策略层，未启用 Bohrium 真实写通道

本文把 `bohrium-kb/round3_prep` 的作战规则落成 Codex 可实现的主代理-子代理协作契约。它是 ASCodex 的设计基线，不把 Harness 的 preset、skill 或 prompt 误认为运行时安全边界。

## 1. 运行时分层

```text
Codex Core / app-server
        |
        v
Coordination protocol + ACL + state transitions
        |
        +--> AgentControl lineage (chief / solver / monitor / intel / judge / red-team)
        +--> Guard broker (six gates, lease, quota reservation, audit)
        +--> SQLite event ledger (versioned, idempotent, replayable)
        |
        v
Controlled executor (future) --> Bohrium read/write APIs (write disabled in P0-P5)
```

知识资产在 `skills/`、`agents/` 和 `bohrium-kb/`；权限、状态和额度只能由进程内协议与 broker 决定。`subagent-supervisor` 不迁移为独立外壳，因为 Codex 原生 AgentControl 已提供派生、消息、等待和恢复基础；需要补的是可审计协议和强制 ACL。项目级 worker profiles 关闭全量 skill 注入及不必要的 apps/plugins/memory/request-permission 能力；这只是角色配置层的缩限，最终权限仍由父线程交集、Guard 和 OS sandbox 决定。

## 2. 三层状态机

### Campaign

```text
new -> intake -> oriented -> planned -> executing -> evidence_ready
     -> preflight -> submitting -> awaiting_score -> observed
     -> score_verified -> closure_review -> closed
                                  \-> replan -> planned
```

任一活动状态都可 `abort`。`transport_accepted` 只表示 broker/通道接受，`submission_dispatched` 才能进入 `awaiting_score`；二者不可复用。每次事件携带 `expected_version`，版本冲突拒绝覆盖。

`closure_review_started` 与 `closure_approved` 是两个独立事件；进入复核不等于已经通过封板三问。

### Agent

```text
registered -> briefed -> active
active -> waiting_input | waiting_score | handoff_pending
handoff_pending -> end_proposed -> end_accepted
active/waiting_* -> suspended | crashed -> recovering -> active
```

每个 agent dossier 还必须带 `runtime_instance_id`、`turn_generation`、`last_seen`、`lease_epoch` 和 `recovery_attempt`。恢复顺序固定为 `boot -> ledger_check -> canary -> rehydrate -> active`；金丝雀失败时不得恢复正式作战。子代理只能提出结束或交接，不能自行接受结束。chief 在窗口内裁决继续、换方向或收工；AutoPush（未来实现）只能在窗口超时后按预置 lease 兜底。

### Attempt

```text
candidate -> preflight -> reserved -> transport_sent -> pending_parse
          -> pending_review -> score_observed -> backfilled
          -> leaderboard_confirmed -> settled -> committed
```

失败分为 `failed_retryable`、`failed_terminal`、`stuck`、`released`、`filtered`。`submitted`、`queued`、`transport_sent`、`api_scored` 均不是成功；平台延迟、`gradable=false`、批量重评和旧 draft 停摆分别进入 `pending_review`、`backfilled` 或 `retry_new_attempt`，不能无限等待旧对象。成功证据分层核验：题目/attempt 归属和 Harbor replay/reward（适用题型）是必备；`resultsJson`、scorecard、scoringDetails 和排行榜按通道及可见性分别记录，`resultsJson=null` 不单独否决；重评后必须再次确认稳定性。协调器的 `PlatformObservation` 固化 attempt、challenge、路由、观测时间、响应 SHA-256 以及 replay/results/scorecard/leaderboard 五个独立状态；只有排行榜已确认才允许官方确认。成功 worker 回报必须引用 attempt、trace、artifact、score 四类证据，引用使用相对路径和 64 位 SHA-256。

旧的布尔型 `OfficialEvidence` 仅用于迁移诊断，兼容确认函数已禁用，不能推进 attempt；正式路径必须使用 typed `PlatformObservation`。

### Message receipt

```text
accepted -> delivered -> acked -> applied
       \-> rejected | expired
```

消息状态是审计事件，不依赖 `trigger_turn` 推断是否已执行。重复 `message_id/idempotency_key` 必须幂等，乱序或过期命令拒绝。

## 3. 消息契约

Codex 原生 inter-agent content 承载版本化 JSON `MessageEnvelope`，关键字段包括：

- `schema_version/message_id/message_type/topic`；
- `campaign_id/challenge_id/correlation_id/causation_id/stream_seq`；
- sender/recipient 的 `session_id/thread_id/role`；
- `issued_at_ms/expires_at_ms/priority/requires_ack/trigger_turn`；
- `lease_id/authority/expected_state_version/idempotency_key`；
- payload 与带 `artifact_id/sha256/path/provenance` 的证据索引。

`trigger_turn=true` 仅允许 user 或 chief 发出的 command/control。payload 不得包含 token、AccessKey、密码或未脱敏凭据。score 事件先到 chief，再按脱敏规则复制给 solver；submission-clean 产物禁止写入判官结论、历史分数或他人做法。

## 4. ACL 与可见性

| 角色 | 允许 | 禁止 |
|---|---|---|
| chief | 建 campaign、分配角色、批准/撤销 lease、replan/close | 直接执行、直接调用平台写接口 |
| solver | 读题面、写自己的 workspace、smoke、请求提交 | 直接 REST/CLI 写入、兄弟 workspace、未授权身份 |
| monitor | 只读 attempt/榜单/状态，维护 STATUS 快照 | submit/delete、改 solver 产物 |
| intel | 读公开公告并保留 cursor/原文 | 决策、提交、把未核验消息当事实 |
| judge analyst | 只读 scorecard/判词，设计差分实验 | submit、改 solver workspace |
| red team | 原始题面和独立 clean-room | 旧 solver 报告/答案、submit |
| Guard broker | 六门检查、lease、reservation、审计、唯一提交入口 | 被 caller/skill/MCP/RPC 覆盖 |

chief 可见 campaign 的脱敏状态和证据索引；parent 可见直属 child；child 只见自己的档案与 sanitized brief；sibling 默认不可见；red team 额外禁止旧结论。路径必须经过 canonical workspace fence。

## 5. Lease、额度与恢复

写权限、身份白名单、AutoPush 窗口、Bohrium job 和 workspace 都使用 parent-issued lease；lease 包含 owner、campaign/challenge、role、过期时间、允许动作和撤销版本，以及不可覆盖的 `authorized_identity_classes`、`operator_id`、`pool_epoch`、`registration_allowed=false`。身份类别必须由用户先指定；FROZEN、错误 operator、跨类别身份和新增注册均 fail-closed，429 只能在已授权类别内顺延。solver 的普通请求不能提升为 submit capability。

协调器现在提供 `ActorContext` / `Lease` 运行时契约：每次上下文绑定的状态转移都核对 `agent_id/session_id/thread_id`、campaign/challenge、lease owner/role、有效时间窗、允许动作、冻结身份池和 operator。非 chief 只能操作自己的 agent；chief 也必须通过自己的有效 lease。旧的 role-only API 仅保留给快照迁移和兼容测试，Core 接入时应统一改走 `*_with_context`。

`solver-guard` SQLite ledger 当前提供 reservation/commit/release、`coordination_events` 事件追加、`actor_leases` 注册表，以及 Chief 绑定的 `research_cycle_issuances/stage_brief_issuances`。管理员预置的 solver/chief lease 可跨 SQLite 重开恢复；Chief 只能以有效 `Decide` lease、匹配 campaign optimistic version 和完整文件哈希在单一事务内签发周期与 brief。每次签发同时建立唯一的 `thread_cycle_bindings` Chief/root 行；fresh child spawn 与 V1/V2 resume 通过 live thread/session、parent、role、active cycle/version 和 StageBrief 重新解析该绑定。supersede/revoke 会原子撤销旧 cycle、brief 与 thread binding，幂等 replay 也必须证明 Chief/root 行仍存在且逐字段一致。Core dry-run 提交按 `lease_id` 查询并绑定实时 session/thread、campaign/challenge、动作和 identity class；调用者不能提交自造的 ActorContext。Codex 当前没有独立于 thread 的 agent id，因此 Core 内部暂以 `agent_id = thread_id` 兼容映射，模型参数不能覆盖。注册、重复绑定、伪造、过期、撤销和动作缺失均 fail-closed。Guard policy 现在支持 typed `identity_pool`：非空时它是权威来源，提交身份必须唯一绑定 challenge/owner/identity class，frozen 条目阻断；空池仅保留旧单身份迁移兼容。identity 级 ledger quota 已由 `reservation_dimensions` 维度表在 reservation 事务内强制执行：每个 identity/identity_class 的 `max_reserved_cost_usd`、`max_concurrent_reservations` 与独立 `min_interval_seconds`，缺少维度行的历史 reservation 会整体阻断 quota 执行而不是被绕过；`identity_pool_entries` 持久表与管理员 `ascodex-pool-admin`（provision/freeze/thaw/remove/list/inspect，动作与版本化审计事件同事务、重放幂等）提供池生命周期管理，Core `solver_guard_submit` 在 admission 前以 `resolve_identity_pool` 将活跃 ledger 池设为运行时权威：未绑定身份 fail-closed，无活跃条目时回退 YAML 池。重启恢复只认 ledger，不认内存状态或聊天记录；真实平台观察客户端仍待接入，恢复 admission 已接入但自动 runner 未完成。

## 6. OODA 与卡死协议

Observe（monitor/intel）→ Orient（题级 channel probe + 判官信号卡）→ Decide（chief 按收益×概率×耗时派活）→ Act（solver 实验）。实验计划必须写假设、轴、变更字段、字段类型覆盖（数值+字符串/枚举）、baseline hash、quota 成本语义、预期响应、判别标准和证据锚点；多字段变更需显式耦合组。`ascodex_channel_probe.py` 已提供题级只读 probe：仅 GET challenge 与 owned challenge-attempts，保存响应哈希和 typed channel evidence；缺失 grader/queue 信号保持 unknown，quota 成本保持 unknown，不创建 diagnostic draft。`solver_guard_submit` preflight 现在要求 fresh typed probe 和两份原始 GET 响应，校验哈希、challenge 绑定、GET-only 语义、grader 注册和 worker queue；失败/过期/unknown grader 一律阻断。出现同轴三次无进展、判词重复或两档差距且两小时无进展，进入 `stuck -> replan`，并派 judge analyst + clean-room red team；不得继续无证据轮询烧额度。stuck 检测按最近活动的 `challenge_id` 隔离历史，避免跨题 attempt 或判词串线。

每轮 worker 回报只能是 `success`、`blocked`、`env_failure`、`timeout`、`inconclusive` 五类，并携带 attempt、身份、Harbor、trace、scoringDetails/判词证据。monitor 每周期原子覆盖 `STATUS.md` 的 `scoreboard/in-flight/since-last/blocked/next-checkpoint` 五字段；事件 cursor 和快照均可从 ledger 恢复。

### 6.1 可执行 OODA 周期

`codex-ascodex-coordination` 暴露 `OodaCycleRecord`、`StageBrief` 和 `ResearchCycleRecord`。前者只记录 Chief 在 `decide/review` 阶段签发的 directive；observer、judge、red-team 和 solver 可以产生报告或观察，但不能以 OODA directive 改变下一轮。后两者把阶段化技能引用、verifier/baseline、实验、报告、观测、事实/推断、额度和收关证据固化为一条离线可验证的循环记录。stuck 时只能 `EscalateStuckReview`（原子要求 judge 分析和 clean-room red team）或 abort；不能继续、预提交或封板。`BeginClosureReview` 与 `ApproveClosure` 必须是两个独立记录，且都绑定 `ClosureEvidence`。

这不是新的后台 supervisor，也不代替 Codex 的 AgentControl。Core/app-server 应在派发下一轮前构造并验证该记录，再使用 context-bound transition 写入 ledger。这样主代理的自然语言消息只能成为候选意图，不能直接越过阶段 ACL、版本冲突或证据门。

### 6.2 封板三问的机器前置

`ClosureEvidence` 将封板三问和“真天花板”核对转为前置条件：场上更高结果已查、至少两个独立证伪者已完成、历史最高值已查、Harbor/Trace 双轨已确认，并且所有依据带相对路径和 SHA-256。`budget_stop_requested` 不能覆盖高于当前结果的历史记录。该对象只允许进入 `closure_review`，不能直接把 campaign 标记为 `closed`。

### 6.3 科研协作循环服务

目标循环固定为：

```text
intake -> contract/verifier -> role brief -> solver experiment
       -> read-only observation -> evidence normalization -> chief decision
       -> continue | replan | clean-room red-team | closure review
```

Codex AgentControl 只负责 thread 生命周期、lineage、消息、等待与恢复；ASCodex coordination service 负责 campaign 状态、lease、阶段 ACL、证据归一化和下一轮 directive。二者不能互相替代，也不新增第二套 supervisor。项目级 `.codex/agents/*.toml` 只提供原生角色说明与默认配置；当前 AgentControl 的 role override 不把 TOML 中的 `sandbox_mode`/MCP 配置当作不可绕过的权限边界，因此实际读写和网络限制必须继续由 parent-derived permission profile、RPC/tool preflight、Guard broker 与后续 OS egress 实现，不能仅凭角色文件宣称只读。

solver profile 的 child dispatch 进一步收紧为 `depth=1`：只有 Chief/root 直接派发 worker，普通 worker 不得再派生第二层 solver child。Core 在资源预留前还会以 live parent/session、可信时钟和 Guard ledger 校验 Chief `SpawnChild` lease 与 active `cycle_id + cycle_event_version`；深度门、租约门和 cycle/brief 门缺一不可。

2026-08-31 补充：solver profile 的网络 egress 已接入 typed `EgressPolicy`（`ChannelPolicy.egress`，默认 `deny_all=true` 即空策略全部拒绝）。`validate_egress` 做精确主机名/子域名 allowlist 匹配、`denied_domains` 显式覆盖、`extract_network_host` 只接受 http/https；Core Tool Registry 在 `solver_guard_blocks_invocation`（含 hook 改写后复查）中解析 digest 门控的 policy 文件并对 `webFetch`/`webSearch` 等网络工具执行 egress preflight，策略缺失/未签名/路径穿越/非 http URL 一律 fail-closed。这仍是进程内策略层，不是 OS 级网络沙箱/防火墙隔离。

每轮必须落盘 `ResearchCycleRecord`，至少包含 verifier/spec hash、实验假设、单字段或显式耦合轴、baseline hash、quota 成本、角色 brief hash、worker report、平台观测 hash、事实/推断分栏以及下一步 directive。缺 verifier、baseline、brief 或相应阶段证据时只能 `blocked` 或 `inconclusive`，不得自然语言跳转到提交或封板。

跨轮推进还必须通过 `ResearchCycleRecord::validate_successor`：后继记录只能引用同一
campaign/challenge，`expected_state_version` 必须严格递增一位；已 abort 或已批准封板的记录
没有后继。封板候选、封板审查和批准收关分别是独立记录，不能用一条自然语言消息或旧 cycle
重放跳过中间阶段。

SkillInjector 迁移为阶段化、受限大小（最多 1,229 bytes）的 `StageBrief`，不把全部技能塞入上下文：开题选 `playground-solve-optimal/platform-scorecard-analyze`，提交前选 trace/redline/submit，卡死判官分析选 oracle/differential/field audit，卡死红队选 `unstuck-switch-angle/red-team-review`，封板选 closure，重计算选 bohr，交接选 competition-coordinate。Skill 只提供知识，不授予 Action/lease；brief 记录来源 digest、选择原因、大小上限和有效阶段。来源路径必须是规范 workspace 相对路径，`worker-submit-chain` 永不进入 brief；clean-room brief 仅派给 RedTeam。

StageBrief 只能引用 Codex 活跃适配入口 `.agents/skills/<name>/SKILL.md`；`skills/deepseek-harness/` 是追溯用原文，包含 DSH 专用工具名或已被修正的旧行为，禁止作为运行时 brief 来源。两份副本的 digest 不要求一致，但 ledger 必须锁定实际执行入口的 digest。

事件默认 chief-first：平台分数、stuck、trace 风险、quota 和 lease 变更先写 ledger，再唤醒 chief；solver 只收脱敏副本。未来 AutoPush 不使用关键词猜测“收工”，而由 `end_proposed` 和结构化 `ClosureEvidence` 驱动；chief 的自然语言同意不能覆盖机器前置。

### 6.4 动态契约与事实对账面

最新只读审计表明，Guard 不能只充当静态六门集合，还必须成为确定性事实源。每题在 intake 和 pre-submit 读取并缓存 `challenge_id + contract_version + contract_fingerprint`，按服务端 `required_submission`、schema、scoring contract、round window 和 ARM manifest 生成验证计划。题目专用 adapter 只能精确命中 challenge/version/fingerprint；未知契约继续执行通用安全、trace、artifact、有限数值和路径检查，但默认阻断正式提交，且 Chief override 只能放开 `unknown_contract` 这一门，不能覆盖身份、污染或真实性门。

2026-08-29 补充：`scripts/ascodex_contract.py` 可从保存的 challenge 响应生成 typed `ChallengeContract`，并只对显式选中的 contract_version、required_submission、round window、submission schema 和 scoring contract 做规范 JSON SHA-256 指纹；只有这两项服务端字段都在响应中观察到，且显式提供 adapter id，才允许 `Known`。`ChallengeContract::verify_fingerprint_input` 在 Rust 侧复算同一 canonical 指纹。Core 的 fresh solver dispatch、V1/V2 child resume 和 `solver_guard_submit` preflight 现在都要求 typed contract 与 canonical fingerprint input，校验 challenge 绑定、指纹、时间窗和状态；Solver dispatch 与 formal admission 进一步要求 `Known` contract。2026-08-31 补充：公共 `validate_contract_files` 收拢到 solver-guard，app-server 的 `thread/resume` solver-profile 路径在 canary preflight 后同样执行 contract gate（要求 Known 且 challenge 绑定环境中的 `ASCODEX_CHALLENGE_ID`），Core spawn 复用同一实现；这仍未覆盖全部 app-server state transition，不能宣称端到端 contract gate 已完成。

平台 observation、attempt、quota、cadence、Bohr job 与排行榜必须由独立 reconciliation loop 周期重建。无 attempt id 的本地失败不得消耗平台额度或 cadence；无法确认的对象进入 `unknown_needs_reconcile`，不能永久算作 in-flight。`refresh=true`、channel probe 等接口必须执行其声明的只读探测并保存响应哈希，禁止用静态规则或旧缓存冒充实时事实。模型门绑定 `Chief task config == effective child route == submission declaration`，具体 provider/model/effort 由任务配置决定，不在通用源码中写死。

该事实对账面只产生 typed observation/event，不直接驱动提交。当前 `platform_observations`、`reconciliation_snapshots`、`reconciliation_items` 与 `reconciliation_penalties` 表，以及管理员侧 `ascodex-observation-admin` 已把完整 `PlatformObservation`、typed reconciliation item、单调 cursor snapshot、不可变 item 审计与判罚依据原子持久化；同一 attempt/response 重复导入、幂等冲突、跨题/跨 campaign 写入、非 Monitor 写入、cursor 冲突和账本 JSON 篡改均 fail-closed。事件先落 ledger 再由 Chief 决策；网络不可达时保留 last-known fact 与观测时间并进入 degraded/unknown，不把”暂时无数据”误判为失败或成功。只读、白名单域的平台 GET client、离线 reconciliation converter、单周期 runner 和有界顺序 scheduler 已落地；runner 只做一次”采集/读取保存响应 → typed manifest → Monitor context 校验 → admin ledger 批处理”，不访问平台写接口。scheduler 持久绑定 Monitor context digest、cursor/event-version，失败时不推进权威计数；applied 事实产生 typed Chief wake request 文件。2026-08-31 补充：Chief wake 消费面已落地——`chief_wake_requests` 持久表把 wake 文件登记为等待/已确认状态机，`record_chief_wake_audited` 要求 wake 必须绑定实际 applied 的 reconciliation fact（campaign 快照 event version + 某 item 的 response 哈希）且 `platform_write_attempted=false`，`ack_chief_wake_audited`/`list_waiting_chief_wakes` 与管理员 `ascodex-wake-admin`（inspect/ack/list-waiting）提供消费与控制面，所有转移与版本化事件同事务、重放幂等 fail-closed。真实 schema 归一化和 Chief 进程自动消费 wake 仍未实现。

## 7. 实现状态与验收矩阵

## 7.1 评分系统变更适配（2026-08-28）

本节是对平台报告的本地契约映射，不宣称已经取得官方 API schema。平台报告明确的行为为：全站榜与赛季总榜统一口径；判罚改为扣 1 分且保留判罚标记、原始分可见；榜单显示 user/agent 归属；ARM bundle 重传后重新评分；反作弊改为加权计分并新增三个信号；无运行痕迹的 trace 不进入待评队列；匿名读取他人提交关闭；判罚依据可查询；题目页面的概览/题面/资源区和缺失附件状态明确。

ASCodex 的 typed reconciliation facts 因此必须至少分栏保存：

| 事实 | 本地字段/门 | 处理原则 |
|---|---|---|
| 判罚前分数 | `raw_score` | 永不因判罚被覆盖或“归还” |
| 榜单计分 | `effective_score` | 与原始分分离；不得拿有效分反推历史原始分 |
| 判罚 | `penalty`, `penalty_applied`, `penalty_basis` | 当前最佳解释为 delta `-1`，即 `effective = raw - 1`；对象、原因、改写分必须可审计 |
| 归属/榜单 | `credited_owner`, `leaderboard_scope`, `season_id` | 全站/赛季/题目范围分别记录；多 agent 不合并为匿名布尔值 |
| bundle 重评 | `bundle_revision`, `rescore_status` | 新 revision 只有 fresh completed rescore 才可确认；旧结论标 stale |
| 反作弊 | `anti_cheat` | 记录 weighted signals 的可见部分；新增三个信号的名称/权重未知时保持 unknown，不猜固定公式 |
| trace 入队 | `trace_evidence`, admission state | 无运行痕迹只能 `unknown_needs_reconcile`，不能当失败、成功或可评 |

`penalty = -1` 是依据“扣 1 分”措辞的本地临时解释，而不是已核实的 API 表示；若后续 schema 明确该字段是绝对有效分而非 delta，必须迁移 reducer 与 verifier，不得静默兼容两种语义。公开页面只读核对到 user/agent 归属和 `-1/100` 展示，尚未核对原始分、判罚依据或真实字段名。匿名他人提交读取路径不得用于 oracle probe；仅允许自有或明确授权对象。

监视、验证和提交前检查必须把“平台报告契约”“公开页证据”“本地推断”三者分栏，任何一栏缺失都进入 `unknown_needs_reconcile`，而不是以旧教程中的单一 reward/leaderboard 布尔值推进 `settled`。

已实现并测试：角色 ACL、`ActorContext`/lease 绑定、实验计划基础校验、worker 四类证据报告、campaign/agent/attempt/receipt 转移、delayed-review/backfill/leaderboard-confirmed attempt 事件、typed `PlatformObservation`、消息 envelope 校验、stuck 检测、Guard 六门基础策略、SQLite reservation、actor lease registry 与版本化事件日志 replay，以及 thread→active cycle/role 的持久绑定和 revoke/supersede 失效。`ResearchCycleRecord::validate_successor` 还提供跨轮 reducer 边界：版本严格递增、campaign/challenge 不可漂移、abort/已批准封板不可继续。Core fresh spawn、V1/V2 resume 已把经签发 StageBrief 的 canonical readable/writable roots 转换为显式 managed `FileSystemSandboxPolicy`；不再依赖继承的 `:workspace_roots`，RedTeam clean-room 与 parent roots 重叠时拒绝。提交入口现在从 trace/run.log/manifest-listed text artifacts 派生 redline 结果，并拒绝平台反馈、attempt 编号及外部解题者引用；caller 自报的 redline 布尔值不再进入工具契约。当前 Core dry-run 已用可信 `TimeProvider` 校验持久 lease 后执行 broker 原子 reserve/release，但不产生真实 attempt。

Trace admission 的当前硬条件是：JSON/JSONL 仅允许 1--10,000 个对象且不超过 8 MiB；每步必须有唯一 `step_id`、连续 `step_order`、时间戳、非负 duration/cost/tokens；至少三条 80 字符 thought、总 `cost_usd >= 0.01`；tool call/result 必须按同一 ID 紧邻成对；至少一段 16 字符以上 result body 必须出现在 run.log；manifest 中的文件必须在 workspace 内、SHA-256 匹配且不能把 trace/run.log/manifest 伪装成业务 artifact。若提交入口提供 ARM manifest，还会强制校验 `execution` 的 run/session/agent、成功 exit/status、cwd、entrypoint、run-log hash、时间窗和文件 mtime，并要求 execution.log_path 与实际 run.log 一致。redline 扫描对明确的二进制输出只依赖 manifest hash，其他读取错误一律 fail-closed。

已实现并测试：管理员专用 `ascodex-lease-admin` 与 `ascodex-stage-admin` 二进制、原子 lease provision/revoke、Chief 绑定的 cycle/brief 签发和版本化审计事件、StageBrief 路由契约和离线 ResearchCycleRecord。`codex-ascodex-runtime` 从既有 SQLite issuance 按 `cycle_id + child role` 唯一读取 brief，重新验证 campaign/challenge/role、有效期、canonical workspace boundary、capability map 与每个 Codex 活跃 Skill SHA-256，并把不超过声明预算的引用卡注入独立 developer context；policy root（技能/能力图校验）与 challenge workspace root（角色 ACL）已在 StageBrief 中分离，避免 Solver 因技能根而获得仓库根写权限；fork 会移除继承 brief，solver profile 拒绝 direct delegate session。`supersede_research_cycle_audited` 会在撤销前驱前读取其持久 JSON/hash，并通过 `ResearchCycleRecord::validate_successor` 拒绝版本跳跃、题目漂移和终态后继。旧 JSON bundle 仅保留导入/离线兼容格式，已不作为 spawn 权威。管理员命令不是模型 tool，只接受绝对 SQLite/JSON 路径并且不会输出 operator、lease context 或凭据。

尚未实现：真实 `solver_guard_submit` executor、不可由本机管理员覆盖的策略与启动配置、常驻后台 supervisor、artifact 与 execution block 的全量交叉引用、OS 级网络 egress（本机沙箱/防火墙层）、真实平台 monitor/榜单复查、chief-first 常驻 monitor/AutoPush，以及 Core/app-server 全部状态转移对 `ResearchCycleRecord` 的强制接入。cycle revoke/supersede 已具备控制面原语，但尚未扩展为所有 app-server 状态事件的统一生命周期服务。execution 校验仍是本地文件/时间窗证明，不是平台侧真实性证明；redline 仍是本地文本证据扫描；恢复金丝雀已实现持久化、runtime 绑定、两回合证据与 resume admission 门，并有篡改/冲突/过期负向测试，但真实 Core disposable-child runner 尚未接入，不能宣称完整 boot→active 自动闭环；故当前所有真实 Bohrium 写操作保持关闭。

策略完整性现状：`solver_guard_submit` 会复算 Guard policy 文件 SHA-256 并与 `ASCODEX_POLICY_SHA256` 比较，启动脚本也要求管理员提供 approved digest。这只防止 policy 文件在启动后被静默替换，不等于不可篡改的 OS/签名信任根；issuance selector 仍受进程环境影响。

能力等级：P0 本地 fail-closed 预检；P1 SQLite 预约与事件 replay；P2 真实 trace/artifact/redline admission；P3 只读平台 monitor 与榜单复查；P4 受控真实写 executor。没有 E2E 证据的能力只能标为设计或实验。

本地验收命令：

```powershell
$env:RUSTC = 'C:\Users\XKZ\.rustup\toolchains\stable-x86_64-pc-windows-msvc\bin\rustc.exe'
& 'C:\Users\XKZ\.rustup\toolchains\stable-x86_64-pc-windows-msvc\bin\cargo.exe' test -p codex-ascodex-coordination --locked --offline
& 'C:\Users\XKZ\.rustup\toolchains\stable-x86_64-pc-windows-msvc\bin\cargo.exe' test -p codex-solver-guard --locked --offline
python -m pytest -q tests/test_migration_integrity.py tests/test_submit_gate_audit.py tests/test_verify_attempt.py
```

Python 迁移扫描必须在清理 `codex/codex-rs/target` 后运行；target 是临时构建产物，不属于交付内容。


