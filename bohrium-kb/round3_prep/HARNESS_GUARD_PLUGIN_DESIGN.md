# DSH Harness 作战纪律守卫插件 — 设计文档

> **2026-08-28 覆盖声明**：本文描述的是 DeepSeek Harness 侧历史实现现状；ASCodex 迁移只在 `.agents/skills/` 与本地 `codex/codex-rs` 中落地，且判罚/反作弊契约以 `config/playground-scoring-audit-2026-08-28.md` 为准。本文中的 -1000 翻账与固定 trace 门槛不适用于当前平台。

> 版本：v0.2（与实现对齐）
> 目的：把解题作战中反复依赖提示词工程的纪律，固化为 **DeepSeek Harness 插件的软件边界**。纪律不该靠"记得"，要靠"做不到"。
> 状态：**已实现**（`dsh-solver-guard` v0.2.0，host 平面插件；本文档描述的是实现现状而非蓝图）。实现拆分见文末 §12。

---

## 0. 为什么需要这个插件（纪律失效的反模式）

实战中反复出现的纪律失效形态（attempt 级证据已归档清理，保留模式结论）：

| # | 纪律域 | 失效模式 | 代价 |
|---|---|---|---|
| 1 | 提交间隔 | 同身份短窗高频提交 → N16_BURST → 翻账 | 高分被清零，身份冻结 |
| 2 | 提交间隔 | 间隔口径反复变更靠广播通知 | 各 solver 执行不一致 |
| 3 | 身份配额 | 超发（>10 上限）；429 后才顺延 | 身份额度耗尽才发现 |
| 4 | 身份配额 | 误用非池内/冻结凭据 | 纪律反复重申仍漏 |
| 5 | 提交方法 | 四步链（带 script）出高分但**不进官方榜**——通道选错 | 大量"高分"实际 0 收益 |
| 6 | 提交方法 | 旧 draft 卡死数小时，代理还在"等队列恢复" | 浪费半天无产出 |
| 7 | 提交方法 | CLI 裸提交 traceCount=0 → 0 分 | 每题烧多发 |
| 8 | trace 质量 | 构造 trace 落 29 档；时间轴伪造落 69 档；只有真实执行 84+ | 落袋分被重评回滚 |
| 9 | 轮询阻塞 | 子代理 sleep 轮询出分，干等 | 有效工作时间浪费 |
| 10 | 污染红线 | banned 扫描靠手工，偶尔漏扫 | 泄漏 = 判官翻账风险 |
| 11 | 模型路由 | 子代理默认继承错误模型 | 全部路由错误 |
| 12 | 消息纪律 | send 不 interrupt → 消息堆积不消费 | 代理执行旧任务 |
| 13 | 收工纪律 | 子代理弱证据收工（封板三问不过即停） | 场上还有空间却提前止损 |

**结论**：纪律失效 100% 是"人/模型记得做"型约束，不是"做不到"型约束。软件边界应让违规在物理上不可行。

---

## 1. 设计目标与非目标

### 目标
1. **提交前门禁（Gate）**：任何 attempt 提交前，自动校验六道门（channel → identity → cadence → redline → trace → model），任一不过即拒绝执行。
2. **中央状态机（State）**：身份额度、提交历史、子代理档案、通道状态集中持久化（JSON 台账），多会话共享但**按会话隔离可见性**。
3. **异步出分监控（Async）**：出分轮询从子代理剥离，由插件后台守护执行，事件推送**主代理优先 + 子代理副本**。
4. **就地纠偏（Self-heal）**：检测到违规意图（超频提交、越权身份、污染提交物）时，插件主动拦截并给出正确路径（**拒绝即教学**：注入对应技能卡）。
5. **强制续推（AutoPush）**：子代理弱证据收工时，插件经主代理裁决窗口后 cold-resume 强制其继续或派红队攻击其结论。
6. **身份授权（Per-agent whitelist）**：每个子代理可用哪些身份由主代理设定（自然语言 → 主代理 → 插件工具），提交门只在白名单内选。

### 非目标
- 不替代解题智能（不判断数值对错）。
- 不做平台侧修复（评分器故障检测到就上报，不硬解）。
- 不替主代理裁决（AutoPush 只兜底：主代理窗口内回复 = 主代理拥有裁决权，插件让路）。

---

## 2. 总体架构

```
┌────────────────────────────────────────────────────────────┐
│                 dsh-solver-guard 插件（host 平面）             │
├──────────────┬──────────────┬──────────────┬───────────────┤
│  Gate 层      │  State 层     │  Monitor 层   │  Guard 层      │
│ (同步门禁)     │ (JSON 台账)   │ (异步守护)     │ (子代理治理)    │
├──────────────┼──────────────┼──────────────┼───────────────┤
│ ChannelGate  │ agents.db    │ ScoreWatcher │ AutoPush      │
│ IdentityGate │ quota.db     │ DiskMon      │ SkillInjector │
│ CadenceGate  │ submits.db   │ TraceFeedback│ exec 策略面    │
│ RedlineGate  │ bohr_jobs.db │              │ 会话隔离       │
│ TraceGate    │ events.db    │              │ Web 作战台账   │
│ ModelGate    │ state.db     │              │               │
└──────────────┴──────────────┴──────────────┴───────────────┘
        │              │              │              │
        └──────────────┴──────┬───────┴──────────────┘
           工具注入（tool 前缀 solver-guard_*，见 §5）
           事件注入（score/ready · trace/feedback · submit/created ·
                    agent/registered · inject/sent · autopush/*）
```

**关键设计决策**：
- **门禁是同步的、不可绕过的**：`solver-guard_build-submit` 是唯一提交入口（Gate + 执行一体），token 由插件持有注入，命令与输出不含明文，solver 不直接接触凭据明文。
- **状态是中央的、文件持久化的**：`$DSH_HOME/solver-guard/` 下 JSON 台账（JsonStore 原子写），多会话/多子代理共享；**可见性按会话隔离**（每个会话只见自己 spawn 的子代理档案）。
- **监控是独立的、永不阻塞**：ScoreWatcher 是插件后台定时任务（60s 轮询），不是子代理 turn 的一部分；推送用 `agent.inject`（不唤醒、不打断）。
- **执行者是子代理，裁决者是主代理**：出分/卡死/trace 反馈等决策触发事件推给主代理（primary）+ 子代理（副本）；AutoPush 在弱收工时先给主代理裁决窗口。

---

## 3. 模块详设

### 3.1 ChannelGate（提交方法/通道门）—— 第一道

**规则源**（从 SCORING_TRUTH.md / SUBMISSION_PARADIGM.md 固化）：
```yaml
channels:
  harbor_track:                     # 唯一进官方榜
    allowed_forms:
      - cli_no_script
      - rest_no_script
    forbidden: [script_in_draft, four_step_with_script]
  judge_track:                      # 出分但不收录（仅记录）
    allowed_forms: [four_step_with_script]
    purpose: "证据留存，不计入官方分"
per_challenge_overrides: {}         # 逐题通道覆盖（如 abacus 判分器状态）
```

**实现**：
- 提交必须走 `solver-guard_build-submit`（form 默认 cli_no_script），插件校验：
  - draft 无 script 字段（脚本级检查）；
  - trace 已挂载（提交后 traceCount>0，否则告警"CLI 裸提交风险"）；
  - challengeId 归属（提交前 GET challenge 200 + 提交后 GET attempt 核实）。
- 通道状态探测：`solver-guard_channel-probe <challenge>`（只读 1 发成本，返回通道矩阵）。

### 3.2 IdentityGate（身份配额门）—— 第二道

**规则源**：
```yaml
identity_pool:                     # 来自 IDENTITY_POOL.md，只读导入
  <identity>: { account: ..., status: ACTIVE|FROZEN, per_challenge_limit: 10, cred_file: ... }
model_declaration: "DeepSeek V4 Flash"   # 提交声明模型锁定
harness_declaration: "DeepSeek Harness"  # 锁定
```

**实现**：
- 插件持有全部凭据（从 `~/.dsh/*credentials.txt` 读取一次，内存持有，**solver 永不接触 token 明文**）。
- **身份选择三层**：
  1. **per-agent 白名单**：主代理用 `solver-guard_agent-identities set <agent_id> <names...>` 为每个子代理设定可用身份（只有该子代理所属会话可操作，set 时校验身份在池且非 FROZEN）。白名单非空 → 自动选择只在白名单内按**主代理配置顺序**选；显式 `--identity` 出白名单即拒（提示扩权）；429 顺延也限白名单内。
  2. **全局池兜底**：未设定白名单 → 按池声明顺序选第一个 ACTIVE 且 <10 的身份；全满 → 拒绝"身份池耗尽，需总负责人裁决"。
  3. **FROZEN 物理拒绝**：插件层直接拒绝（token 在插件手里，手写凭据无效）。
- 每次提交成功/失败（429）后自动更新 quota（**不依赖子代理自觉登记**）；429 自动顺延下一身份重试一次。

### 3.3 CadenceGate（提交间隔/防作弊门）—— 第三道

**规则源**（rules.yaml，用户可调）：
```yaml
cadence:
  min_interval_sec: 600            # 同一身份相邻提交 ≥10min（用户口径，可配）
  same_content_min_interval_sec: 3600  # 同内容（sha256 相同）≥60min
  max_identical_resubmits: 1       # 同内容最多提交 1 次（原样重交禁止）
  burst_window_sec: 300            # N16 检测窗口
  burst_max_submits: 2             # 窗口内最多 2 发
```

**实现**：
- 提交内容先算 sha256（outputs + trace + manifest 归一化），查 submits 台账：
  - **间隔按 solver 计**：每个子代理保持自己的提交节奏（同题兄弟不互相拖累）；
  - **burst 按身份计**：窗口内该身份第 3 发 → 拒绝（N16 是全身份序列惩罚）；
  - 同内容 <3600s 或已提交过 → 拒绝。
- 主代理可用 `solver-guard_cadence-override --identity <name> --reason "..."` 临时放行（**覆盖必须留痕**，events 表）。

### 3.4 RedlineGate（污染红线扫描）—— 第四道

**实现**：
- banned 词库（分数格式、attempt id、对方队名、判词短语、内部代号如 N16/quota/banned），可配置增补。
- 提交前对**全部提交物**（trace/report/outputs 目录内所有文本）递归扫描，任何命中 → 拒绝 + 报告命中位置（`solver-guard_redline-scan <dir>` 可单跑）。
- 测试句内置："没看过答案或分数的人能写出这句话吗？"

**经验映射**：trace-contamination-redline 技能固化为代码。

### 3.5 TraceGate（trace 质量门）—— 第五道

**规则源**（从 TRACE_LAW.md / TRACE_99_RECIPE.md 固化）：
```yaml
trace_gate:
  min_trace_score: 70              # factor 1.0 门槛（实证 ≥70 而非 80）
  must_be_real_execution: true     # 构造/模板 trace 落 29 档
  machine_layer_checks: [typed_step_type, tool_call_pairing, timestamp_window,
                         artifact_existence, cost_floor, stdout_anchor]
  time_axis_check: { span_approx_sum_duration: true, per_step_advance: true }
  banned_sources: [score_mentions, other_team_mentions, judge_conclusions]
```

**实现**：
- 提交前自动跑机器层 6 条 + 时间轴自洽 + banned 扫描（`solver-guard_trace-validate <trace>` 可单跑，返回通过/拒绝 + 档位预测 29/69/84+）。
- **真实性标记**：trace 必须携带 provenance（真实会话 JSONL 或显式 execution 记录），校验 artifact 真实存在且 mtime 合理；无真实执行来源 → 拒绝。

### 3.6 ModelGate（模型/路由锁定）—— 第六道

**实现**：
- 提交命令强制携带 `--model "DeepSeek V4 Flash" --harness "DeepSeek Harness"`（rules.model_declaration / harness_declaration）。
- 子代理模型/推理路由由 registerContinuableSetup waterfall 强制（provider/model/reasoningEffort），偏离即修正 + 事件告警。

### 3.7 ScoreWatcher（异步出分监控——核心非阻塞设计）

**设计要点**：
- **独立后台守护**（插件进程内定时任务，每 60s 轮询），**绝不占用子代理 turn**。
- 状态机：`submitted → pending_parse → scored | backfilled | stuck`；回填窗口 30-50min 预期，stuck 阈值 2h（避免误报）；平台 stuckAt 字段直接识别。
- **推送路由（定案）**：决策触发事件 → **主代理优先（parentSession），子代理副本**，notifyList 配置名单兜底：
  - 出分回填（score/ready）：`attempt N 已确认进官方榜：harbor=… ts=… → …`；
  - 卡死告警（stuck）：建议放弃并新开 attempt；
  - **trace 反馈（trace/feedback）**：仅当 trace 分低于 accept 档（<70）推送修复方向（band 分类：blocked/review），accept 档不推（避免噪声）。
- 推送机制：`createPusher` — 子代理在线则 `agent.inject`（不打断），不在线则 cold-resume 后注入。
- 子代理提交后**立即返回继续工作**（不再 sleep 轮询）。

**设计论证（为什么主代理优先）**：出分消息不是"知会事件"而是"决策触发事件"——继续攻/换方向/换人/接受封板/调身份配额都取决于它。子代理只有局部信息（自己的题、自己的分），主代理持有全局态势（场上分数横截面、身份池余量、多子代理并行状态）；把引信放在决策者手里是对"执行者自决事故"（超发额度、误用身份、N16 burst、干等卡死）的系统性修复。成本极低（低频 + 异步 inject 不占回合），子代理保留副本没有被剥夺信息。AutoPush 的主代理裁决窗口（§3.9）是同一信息流的上下游。

### 3.8 BohriumGuard（云算力纪律门）

**规则源**：
```yaml
bohr:
  local_smoke_limit_sec: 120        # 本地单次计算 ≤2min（纪律卡材料）
  local_mem_limit_gb: 2
  heavy_must_cloud: true            # 重型一律云
  job_timeout_default_sec: 3600
  cost_budget_per_challenge: 50     # 每题预算（美元，可配）
  allowed_machines: [c32_m128_cpu, c128_m256_cpu]  # 机型白名单
```

**实现**：
- `solver-guard_bohr submit/status/download`：提交前校验机型白名单 + 每题累计预算（checkBohrBudget），ACCESS_KEY 由插件持有注入。
- job 生命周期自动跟踪（submit → running → finished/failed），状态变化推送。
- 本地长跑命令的**硬拦截未实现**（原草案"直接拒绝本地执行"）：当前由 `solver-guard_exec` 的命令面策略（python_only：只接受 python 脚本形态，禁 shell 元字符）间接限制 + `heavy_must_cloud` 规则 + bohrium-bohr 技能引导承担。

### 3.9 AutoPush（强制续推——收工纪律的软件边界）

**问题**：子代理弱证据收工（"我尽力了"式报告、封板三问不过）。对应 §0 反模式 #13。

**实现**（lib/autopush.js）：
- `subagent/end` 时对收工报告做封板三问判定（`closure-evidence-standard` 技能标准）：
  - **有 attempt 证据 + 场上无人更高 + 本地/历史无更高值** → 正常放行；
  - **弱收工**（无 attempt 证据 / 场上有人更高 / 只有自己一种解法）→ 进入续推流程。
- **主代理裁决窗口**（rules.auto_push.chief_window_sec，默认 90s）：插件先等主代理对收工报告回复（coordinator 消息）：
  - 主代理回复继续/换方向 → **插件让路**（主代理拥有裁决权）；
  - 主代理接受收工 → 插件让路；
  - 窗口内无回复 → 插件兜底强制续推（cold-resume 带新信息 + closure 卡片）。
- 续推节奏：`max_pushes: 2` / `min_interval_min: 30`；第二次弱收工 → **红队模式**（buildRedTeamPrompt 攻击其结论）。
- 主代理推理超时场景：窗口超时后插件 push 先行；主代理后续消息到达时以**主代理为准**（插件不重复 push 已由主代理接管的子代理）。

**与主代理意图不冲突的保证**：所有 push 消息带"指令层级"声明（总负责人指令 > 插件纪律）；主代理在窗口内回复 = 主代理接管。

### 3.10 会话隔离与子代理档案（State 层可见性）

- 每个子代理档案（dossier）记录：label / challengeId / workspace / sandboxMode / **parentSession（spawn 它的会话）** / identities（白名单）/ attempts / diskEvents / writesOutside / workspaceUsage。
- **可见性规则**：任何会话只见 (a) 自己的档案 + (b) parentSession == 自己的子代理档案。工具与 Web 端点均强制（`?session=<id>` 参数）。
- 工作区托管：`solver-guard_agent-register` 自动创建标准结构工作区；DiskMon 定期扫描工作区大小/磁盘剩余/越界写。
- **exec 命令面**：`solver-guard_exec` 只接受 python 脚本形态（python_only 策略，禁 shell 元字符与 -c）；工具面剔除 pwsh/bash（deny_shell_tools）+ 解题聚焦白名单（focus_mode）。

---

## 4. 数据模型（State 层）

`$DSH_HOME/solver-guard/` 下 JSON 台账（JsonStore：原子写、文件锁、多会话安全）：

| 集合 | 内容 |
|---|---|
| `agents` | 子代理档案：agentId → { label, challengeId, workspace, sandboxMode, parentSession, **identities(白名单)**, status, createdAtMs, settledAtMs, diskEvents[], writesOutside[], workspaceUsage } |
| `quota` | 身份 × 题 → { used, lastSubmitAtMs, lastContentSha }（每次提交自动更新）|
| `submits` | attemptId → { identity, challengeId, channel, form, contentSha, status(submitted/pending_parse/scored/backfilled/stuck), createdAtMs, requestedBy, score 字段, traceFeedbackSent } |
| `bohr_jobs` | jobId → { challengeId, jobName, machine, status, cost, submittedAtMs, finishedAtMs, requestedBy } |
| `events` | 审计事件流：submit/created · score/ready · trace/feedback · agent/registered · agent/identities · inject/sent · autopush/* · cadence/override |
| `state` | bootedAt / diskFreeBytes / overrides（cadence 放行）|
| `credentials` | 身份 → token（内存持有，不落明文日志）|

---

## 5. 工具接口（注入 DSH 的 tool 前缀 solver-guard_*）

| 工具 | 用途 | 状态 |
|---|---|---|
| `solver-guard_status` | 全量状态（身份余量/在途 attempt/通道/出分窗口/事件尾） | ✅ |
| `solver-guard_build-submit` | **唯一提交入口**（六门 + 执行）：`challenge_id / form / track / outputs / trace / report / identity / dry_run` | ✅ |
| `solver-guard_trace-validate` | trace 质量门单跑（6 条 + 时间轴 + 档位预测） | ✅ |
| `solver-guard_redline-scan` | 提交物污染扫描 | ✅ |
| `solver-guard_score` | 查询单条出分状态（只读） | ✅ |
| `solver-guard_channel-probe` | 判分器状态探测（只读 1 发成本） | ✅ |
| `solver-guard_cadence-override` | 主代理级临时放行（留痕） | ✅ |
| `solver-guard_bohr` | 云算力门禁化（submit/status/download） | ✅ |
| `solver-guard_agent-register` | 派活登记：档案/题目/工作区创建/沙箱模式 | ✅ |
| `solver-guard_agent-show` | 查看档案（题目/attempts/磁盘行为/白名单） | ✅ |
| `solver-guard_agent-identities` | **身份白名单**：set/clear/show，仅主代理会话可操作 | ✅ |
| `solver-guard_skill-inject` | 手动注入技能卡（主代理指定 agent+skill+reason） | ✅ |
| `solver-guard_workspace-create` | 插件代管创建标准工作区 | ✅ |
| `solver-guard_exec` | 命令面（python_only 策略） | ✅ |
| `solver-guard_disk-scan` / `disk-events` | 磁盘状态快照 / 行为流 | ✅ |
| `solver-guard_trace-feedback` | trace 修复方向查询 | ✅ |

**Web 端点**：`/plugins/dsh-solver-guard/agents`（作战台账，`?session=<id>` 会话隔离）、`/plugins/dsh-solver-guard/status`。

**钩子**：
- `subagent/end` → AutoPush 收工判定；
- 子代理创建 → dossier 骨架（parentSession 归属）+ 沙箱模式强制 + exec/工具面策略；
- 提交被门拒 → 拒绝即教学（GATE_TEACHING 映射注入技能卡）。

---

## 6. 与现有资产集成

| 现有文件 | 集成方式 |
|---|---|
| `IDENTITY_POOL.md` | 启动时导入 → identity_pool（rules.yaml，只读）；**per-agent 授权走 agent-identities 工具，不再手工登记** |
| `SCORING_TRUTH.md` / `SUBMISSION_PARADIGM.md` | 固化进 ChannelGate 规则（harbor 轨唯一、无 script、trace≥70）|
| `TRACE_LAW.md` / `TRACE_99_RECIPE.md` | 固化进 TraceGate 检查器 |
| 技能 `submit-attempt` / `bohrium-bohr` 等 | 技能是知识卡；**执行一律走插件工具**（build-submit/bohr）|
| `.dsh/skills/`（项目根，33 个） | SkillInjector 编译来源（rules.injector.skillRoots）|

---

## 7. 实施状态对照（原路线图 → 现状）

| 阶段 | 内容 | 状态 |
|---|---|---|
| **P0** | CadenceGate + IdentityGate + 台账落盘 + build-submit | ✅ 已实现（超频被拒/身份自动顺延/429 顺延有测试锁定）|
| **P1** | TraceGate + RedlineGate + ChannelGate | ✅ 已实现（构造 trace 被拒/banned 命中被拒/四步链被拒）|
| **P2** | ScoreWatcher 异步守护 + 事件推送 + 回填状态机 | ✅ 已实现（主代理优先推送 + trace 反馈）|
| **P3** | BohriumGuard + 预算 + 任务生命周期事件 | ✅ 已实现（机型白名单 + 每题预算）|
| **P4** | ModelGate 自动修正 + 通道状态探测 | ✅ 已实现（提交声明锁定 + 路由 waterfall）|
| **P1b-P4b** | SkillInjector（编译/阶段检测/纪律刷新/拒绝即教学）| ✅ 已实现（stage→skill 映射可配置，judge 四卡）|
| **P5** | AutoPush 强制续推 + 主代理裁决窗口 + 红队模式 | ✅ 已实现 |
| **P6** | 会话隔离（parentSession 可见性）+ exec python_only + 工具面聚焦 | ✅ 已实现 |
| **P7** | per-agent 身份白名单（agent-identities）+ 台账 UI 会话作用域 + judge 注入补卡 | ✅ 已实现 |

**测试**：`plugins/dsh-solver-guard/test/`（98 例，node --test）：gates / exec / ledger / autopush / injector-monitor / trace-redline / state-submit-bohr / identity-whitelist。

---

## 8. 边界与风险

1. **用户覆盖权**：所有门禁提供 `--reason` 留痕放行（cadence-override），但默认拒绝。
2. **平台规则变化**：通道/档位规则随平台变化，rules.yaml 可热更新（status 工具触发 refreshRules）。
3. **不成为瓶颈**：门禁同步但毫秒级；ScoreWatcher 独立定时任务，不阻塞 solver turn。
4. **凭据安全**：token 只在插件内存，子代理通过 build-submit 间接使用；命令与输出不含明文。
5. **不新增身份**：IdentityGate 只消费 identity_pool 已有身份，工具层拒绝注册新身份。
6. **白名单不是提额**：白名单只限定"能用哪些"，额度仍是每题 10 次上限；耗尽后需主代理扩权（扩的是名单，不是额度）。

---

## 9. 一句话总结

> **把"记得做"变成"做不到"**：六道提交门（通道/身份/间隔/红线/trace/模型）+ 两道算力门（机型/预算）+ 一个永不阻塞的异步出分守护（主代理优先推送）+ 强制续推（AutoPush）+ 按需技能注入（SkillInjector）+ 身份白名单（agent-identities）——全部以软件边界固化，提示词只负责智能，插件负责纪律。

---

## 10. SkillInjector（按需 Skill 注入——防长上下文记忆衰减）

### 10.1 问题定义

子代理在长上下文作战（数百步）后，**最初注入的 Skill 内容被稀释遗忘**（实证：多个子代理在长会话后忘记提交纪律/trace 门槛类早期注入内容）。

### 10.2 注入位置（5 个，全部已实现）

| # | 位置 | 触发 | 目标 |
|---|---|---|---|
| 1 | 周期扫描 scanOnce（90s） | 阶段检测（最近 60 条事件关键词 + 工具名） | 所有 live 子代理 |
| 2 | 纪律刷新卡 refreshDiscipline（10min） | 30min TTL | 所有 live 子代理 |
| 3 | 提交门拒绝教学 | GATE_TEACHING 映射（identity→competition-coordinate、channel→submit-attempt、trace→trace-maximize、redline→trace-contamination-redline）| 被拒的调用者 |
| 4 | 手动工具 `solver-guard_skill-inject` | 主代理指定 | 指定子代理 |
| 5 | AutoPush 续推消息内嵌 | 弱收工续推/红队/覆盖 | 被续推的子代理 |

### 10.3 阶段 → 技能映射（engine 默认 + rules.yaml stage_skills 覆盖）

| 阶段 | 检测信号（保守关键词/工具名） | 注入 Skill |
|---|---|---|
| pre_submit | 准备语境 + 提交类动词（准备/即将/下一步/先…提交/trace-validate/build-submit 调用）| trace-contamination-redline + trace-maximize + submit-attempt |
| stuck | 卡住/卡死/等出分/轮询/重试 N 次（**优先于 pre_submit 匹配**）| unstuck-switch-angle |
| closing | 收关/封板/封顶/收工/止损/天花板/ceiling | closure-evidence-standard |
| cloud | bohr/bohrium/DFT/GPU/长跑/训练 | bohrium-bohr |
| judge | 判词/scorecard/harbor/差分/分量/判分器 + solver-guard_score 调用 | platform-scorecard-analyze + oracle-probe + **differential-scoring + judge-field-audit** |
| handover | 接管/换人/交接 | competition-coordinate |

**匹配精确化**：pre_submit 需要"准备提交"语境（"提交后我会…"不触发）；stuck 优先于 pre_submit（"提交完了等出分"判 stuck 不刷提交卡组）；closing 去掉泛词"最优"。

### 10.4 纪律卡（时间衰减兜底）—— 每 30 分钟刷新

```
【纪律刷新 · solver-guard】当前题 {challenge}：
· 身份 {identity} 余 {n}/10（禁 FROZEN、禁新增、限主代理白名单）
· 通道 {channel_state}：只走 harbor 轨（无 script），CLI 形态已证触发
· trace ≥70 满额；构造 trace=29 档，真实执行=84+
· 提交间隔 ≥10min + 同内容禁重交（N16 红线）
· 出分由 ScoreWatcher 守护，你提交后立即继续工作，勿 sleep 轮询
· 场上分数/attempt id 禁入提交物（banned 扫描在提交门自动执行）
· 工作区由插件代管：需要目录用 solver-guard_workspace-create
```

### 10.5 限流与去重

- TTL 30min 内不重复注入同一 skill；单子代理每小时 ≤6 卡；注入历史写 events 表（何时/给谁/注入什么/为何）。
- 精华卡 ≤1.2KB（长技能截断，附"完整版见 skills/<name>/SKILL.md"）。
- 注入不是命令：子代理可忽略（总负责人/用户指令优先）；强制走 Gate 层。

---

## 11. 最终架构总览（Gate + Monitor + Injector + AutoPush）

```
                 ┌──────────────────────────────────────────┐
                 │          dsh-solver-guard 插件              │
                 ├────────────┬────────────┬─────────────────┤
  提交路径 ──▶ 六道 Gate（拒绝即教学）│ ScoreWatcher │  SkillInjector   │
                 │            │ (异步出分)   │  (按需刷新)       │
  云算力 ──▶  BohriumGuard    │            │  AutoPush        │
  弱收工 ──▶  AutoPush        │            │  (强制续推)       │
                 │            └───┬────────┴────────┬────────┤
                 │                │ 事件             │ inject  │
                 └────────────────┼────────────────┼─────────┘
                                  ▼                ▼
                      主代理（裁决 + 副本给子代理）   子代理 inbox
                      (score-ready / stuck / trace反馈)(Skill 卡/纪律刷新/续推)
```

> **纪律四支柱**：Gate 让违规**做不到**，Monitor 让等待**不阻塞**，Injector 让知识**不失忆**，AutoPush 让收工**不轻率**。

---

## 12. 实现拆分（与仓库对应）

| 文件 | 职责 |
|---|---|
| `lib/index.js` | 插件入口：工具注册、门禁编排、端点、生命周期 |
| `lib/submit.js` | 六门编排 + 执行唯一入口（runGates/runSubmission）|
| `lib/gates/identity.js` | 身份池/白名单选择（selectIdentityFrom）/额度 |
| `lib/gates/cadence.js` · `channel.js` · `redline.js` · `trace-check.js` | 各门纯逻辑 |
| `lib/monitor.js` | ScoreWatcher：轮询/状态机/**主代理优先推送**/trace 反馈 |
| `lib/autopush.js` | AutoPush：封板三问判定/主代理裁决窗口/红队 |
| `lib/injector/` | compiler（SKILL.md→精华卡）/ detector（阶段）/ engine（映射+限流）|
| `lib/ledger.js` | JSON 台账 + 会话可见性 + 白名单字段 |
| `lib/exec.js` | 命令面策略（python_only/工具面聚焦）|
| `lib/bohr.js` · `diskmon.js` · `workspace.js` · `rules.js` · `state.js` · `cli.js` | 各支撑模块 |
| `lib/client.js` | Web 作战台账（会话作用域，`?session=`）|
| `test/*.test.mjs` | 98 例单元测试 |
