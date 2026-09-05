# SolverGuard → Codex 深度审计

审计对象：官方 `openai/codex`，源码基线 `a847c71a159fca55509fbac1619c9c2294ed4718`（2026-08-27 本地审计副本）；业务规则来源为本目录的 `dsh-solver-guard` 设计与 Playground 作战文档。

## 结论

可行，但不能把 `dsh-solver-guard` 作为普通 Codex 插件原样搬运。推荐在 Codex fork 中新增 Rust `solver-guard` policy crate，并把它接入核心工具 broker、app-server RPC 入口和持久化 state store。普通插件只承载技能、MCP、应用和 hook 声明，不能提供进程内策略 ABI（`codex-rs/plugin/src/lib.rs:51-80`、`plugin/src/manifest.rs:8-38`）。

`subagent-supervisor` 不需要迁移：Codex 已有 `AgentControl`、spawn/send/wait/resume、父子路径、角色、完成 watcher、graph store 和 rollout 恢复；只需把 SolverGuard 的身份、预算、提交状态附着到现有 agent/thread lineage。

## 原生能力复用

| 能力 | Codex 原生挂点 | 判断 |
|---|---|---|
| 主代理—子代理循环 | `AgentControl` 与 `multi_agents_common` 的 spawn/send/wait/resume；`ThreadManager` 可注入 SQLite-backed `AgentGraphStore` | `native_reuse`；绑定 `bohrium-solver`、`red-team` 等角色即可 |
| 角色/模型/推理设置 | agent role 配置和 spawn 时的有效运行时策略 | `native_reuse` 作为默认；提交时仍需 SolverGuard 再核验，不能仅信任 caller 参数 |
| 工具前置阻断 | `core/src/tools/registry.rs` 的 PreToolUse dispatch；同步 managed hook 可返回 blocked/updated input | `native_extension`；最终硬门应在统一 broker，避免工具新增或 hook 异步化绕过 |
| 工作区/程序/网络基础隔离 | sandbox、Windows sandbox、`execpolicy`、managed network | `native_reuse`/纵深防御；`execpolicy` 只有程序、host、protocol 规则，不能表达 challenge/identity/cadence |
| 真实 trace 记录 | rollout-trace 的 turn、inference、tool started/runtime/ended、child result、payload provenance | `native_reuse` 记录；提交真实性、配对、污染和 artifact provenance 仍需 admission |
| 跨重启 attempt/quota/cadence | 现有 thread/rollout store；rollout budget 仅进程内 root-tree 计数 | `new_crate_required`；新增 SQLite ledger 和原子 reserve/commit/release |

## 六门迁移矩阵

| Gate | 原生覆盖 | 必须新增的策略/数据 | 不能声称的内容 |
|---|---|---|---|
| `channel` | managed domain/protocol、PreToolUse 工具名匹配 | Harbor-only endpoint/method/form、challengeId 归属、提交前后核验 | 网络允许 `play.bohrium.com` 不等于提交通道正确 |
| `identity` | 无 Bohrium 身份概念；agent registry 仅运行态计数 | 身份白名单、冻结状态、per-challenge quota、attempt owner | 角色文件或环境变量不构成身份授权 |
| `cadence` | token rollout budget（进程内） | 持久提交时间窗、同内容间隔、burst/429 账本和时钟策略 | prompt/skill 提醒不能防止重启后超发 |
| `redline` | 同步 PreToolUse 可调用扫描器 | 对提交 bundle、trace、stdout、代码/变量名的全量扫描；失败即拒 | PostToolUse 不能撤销已发生写入；异步 executor hook 不能硬阻断 |
| `trace` | rollout-trace 事件和原始 payload | 真实执行证明、tool call/result 一一配对、单调时间、artifact hash/provenance、最低分门槛 | 有 trace 文件不等于 trace 真实或可提交 |
| `model` | thread/role 有 model/reasoning；turn 可由 caller 覆盖 | approved provider/model/effort 白名单、provider 隔离、提交时 snapshot/证明 | 默认模型或角色 preset 不等于提交模型锁定 |

## 新发现的绕过面（高优先级）

1. **app-server 直接执行命令**：`message_processor.rs:1616-1655` 将 `one_off_command_exec`、`process/spawn` 等 RPC 直接交给专用 processor；`command_exec_processor.rs:91-334` 接受 caller 的 command、cwd、env、sandbox/permission override 后构造并启动进程。这条路径不经过 `ToolRouter.dispatch_any` 的 PreToolUse/统一 SolverGuard。Solver 模式必须禁用这些 RPC，或在 processor 构造执行请求前调用同一个 guard，并拒绝 caller 提升权限。
2. **app-server 直接文件写入/删除**：`message_processor.rs:1066-1100` 暴露 `fs/writeFile`、`fs/createDirectory`、`fs/remove`、`fs/copy`；`fs_processor.rs:79-173` 直接调用 `ExecutorFileSystem`，sandbox 参数为 `None`。提交物、trace 和工作区保护不能只依赖模型工具 hook。
3. **动态工具在外部进程执行**：`thread/start` 只做 schema/name 校验并保存 caller 提供的 `dynamic_tools`（`thread_processor.rs:1401-1458`）；`dynamic.rs:108-216` 将调用转发给 app-server 客户端，副作用发生在 Codex 外部。Solver 模式应默认 `dynamic_tools=[]`，或要求签名/托管 executor，并在 core dispatch 前置检查。
4. **MCP 是外部副作用边界**：MCP handler 将调用交给 MCP server；read-only/destructive 标注和记忆的 approval 不能作为提交安全证明。提交相关 server/tool 必须显式 allowlist，并在 `prepare_mcp_call` 之前做 guard preflight。
5. **运行时设置可被 caller 改写**：`turn/start` 与 `thread/settings/update` 接收 model、effort、sandbox/permissions、cwd 等覆盖（`turn_processor.rs:499-650,895-940`）。SolverGuard 必须在更新前后核验，禁止从 solver profile 降级到 full-access、未批准模型或未批准环境。
6. **环境和插件能力可动态改变**：`environment/add`、`plugin/install`、`config/*write` 均在 app-server dispatch 表中直接可用（`message_processor.rs:1001-1059,1415-1420`）。Solver 模式应将其列入管理员操作，默认拒绝或要求独立授权；插件安装后必须重新计算 guard capability snapshot。
7. **子代理深度/持久化存在边界**：V2 配置会忽略 `agent_max_depth`，不能用它限制 worker→subworker 深度；ephemeral child 的 spawn edge 也会跳过 SQLite 持久化。SolverGuard 必须在 `spawn_agent` preflight 自己执行 depth 上限，并禁止提交代理使用 ephemeral，或另写独立 owner ledger。
8. **并非每个 CoreToolRuntime 都产生 hook payload**：部分控制类工具（例如等待/标准输入控制）可返回 `pre_tool_use_payload=None`。因此 guard 不能只挂在“有 payload 的 hook”上，应在 registry 的全局 dispatch 或提交工具专用 wrapper 中无条件执行。

## 推荐实现边界

```text
codex-core
  ├─ AgentControl / roles / AgentGraphStore
  ├─ ToolRouter dispatch broker  ── SolverGuard::preflight(tool, context)
  ├─ rollout-trace               ── SolverGuard::admit_trace(bundle)
  └─ state SQLite                ── identity/quota/cadence/attempt ledger
                 ▲
app-server       ───┴─ RPC admission（command/process/fs/mcp/dynamic/settings/plugin）
managed sync hook ───── redline/格式检查（纵深防御，不作为唯一门）
solver-guard crate ───── 六门策略、凭据句柄、原子账本、审计事件
```

建议的唯一写入口是 `solver_guard.submit`（或等价 core-native tool），而不是让 solver 直接调用 HTTP/CLI。入口流程：

1. 解析并冻结 challenge、channel、identity、model、thread/agent owner。
2. SQLite 事务中 reserve cadence/quota/attempt；任一冲突 fail-closed。
3. 读取 rollout-trace 和 artifact manifest，执行 redline、真实性、配对、时间和 provenance 校验。
4. 仅由 guard 持有凭据句柄调用 Harbor；提交后立即只读核验 challengeId、终态、replay、`resultsJson`、`scorecard`、`harbor_reward` 和榜状态。
5. commit/release ledger，写入不可变 audit event；网络错误或核验不完整不得标记成功。

## 主代理—子代理循环设计

- 主代理创建 `solver`、`intel`、`judge`、`red-team`、`monitor` 子代理；每个 child 继承 parent 的 sandbox/cwd 基线，并由 role 约束允许工具和模型。
- 子代理只能产出候选 artifact、trace 和结构化报告；只有主代理或 guard broker 能 reserve/submit。
- 监控不需要独立 supervisor 插件：使用 Codex completion watcher、thread events 和外部一次性 monitor；跨重启状态放 SQLite，不放内存队列。
- full-history fork 不能依赖 caller 指定新角色；需要角色隔离时使用受控 spawn 或在 guard 层覆盖。
- Solver worker 的最大深度、是否允许 ephemeral、是否可继续派生子代理，必须成为 guard 的显式策略字段；不能依赖 V2 的旧配置项。

## Fail-closed 验证矩阵

至少加入以下 integration tests：

| 测试 | 预期 |
|---|---|
| 六门任一字段缺失/冲突 | 不调用网络、不消耗 quota、返回结构化 blocked |
| 同身份 burst、重复内容、429 后重试 | SQLite 原子账本只允许合法 reserve；重启后仍拒绝超频 |
| trace 缺 call/result、无 stdout、时间倒退、provenance 缺失 | 提交入口拒绝 |
| bundle 含 prior score/attempt/team/credential 词 | redline 拒绝，且不产生 attempt |
| caller 通过 `command/exec`、`process/spawn`、`fs/writeFile` | solver profile 下统一拒绝或进入显式管理员授权 |
| dynamic tool/MCP 未在 allowlist | 注册或调用前拒绝 |
| `turn/start`/settings 更新请求 full-access、未批准 model/provider | 更新前拒绝，原设置保持不变 |
| plugin/config/environment RPC | solver profile 下拒绝；管理员变更触发 capability snapshot 失效 |
| child 直接调用 submit | 无 owner lease/主代理签名则拒绝 |
| guard 数据库损坏、时钟不可用、网络返回不完整 | fail-closed，不标记成功 |

## 最小改动顺序

1. 新增 `codex-rs/solver-guard` crate：纯策略、类型和 SQLite migration，先用本地 fixture 测试。
2. 在 `core/src/tools/registry.rs` 的统一 dispatch 前后接入 preflight/admission，复用 dispatch trace。
3. 在 app-server 的 command/process/fs/MCP/dynamic/settings/plugin/environment RPC 入口接入同一 broker；solver profile 默认关闭绕过面。
4. 接入 `ThreadManager` 的 graph store 与 rollout-trace，保存 owner、model snapshot、artifact hash 和 gate 结果。
5. 最后再做 UI/CLI 映射；不要先把 Node 插件名或 prompt 当成“已启用硬门”。

## 审计限制

- 本报告基于固定 commit 和本地源码审计；未宣称 Windows sandbox、managed hook 部署或真实 Harbor 写操作已经 E2E 验证。
- `ASCLocal-Codex` 当前是迁移镜像，不包含 DSH 凭据、sessions、运行时 node_modules，也不应把它们复制进 Codex fork。
- 普通 Codex plugin manifest、skill、role、AGENTS.md 只能提供默认行为和人机协作提示；硬性六门必须由 core/app-server broker + 持久 ledger 实现。

## DSH 插件复核结论

本机 `C:\Users\XKZ\dsh-plugins\dsh-solver-guard` 为 `0.2.1`，已增加 ARM bundle/protocol-first admission、并发 quota reservation、身份白名单、Bohrium job 管理和更完整的 trace/redline 检查；`CAPABILITIES.md` 明确仍是 host-plane 插件，不修改 DSH core，Windows ACL 强隔离默认关闭。

- 来源基线存在测试契约失败（`harborScoreOf` 导出契约），阻止来源达到 clean-green。
- 因此当前插件不能直接作为 ASCodex 的安全基线；应先修复测试契约，再将规则/函数迁移为 Rust policy tests，而不是复制 Node 运行时。

### 新增高风险（迁移前必须处理）

- `traceAdmission` 当前“任一 signal 即 admit”，可用无 tool call、无 artifact、零 cost 的 synthetic steps 获得 `admit`；`runGates` 还会把非数组 trace 当作 native session 并合成 provenance。Rust 版必须要求真实 execution record、工具配对和 artifact 证据的最小集合，禁止合成 provenance。
- 红线扫描对不可读 report、outputs 目录遍历错误采用跳过策略；提交输入也未强制位于分配工作区。迁移版必须对 required artifact、canonical path 和读取失败统一 fail-closed。
- `cadence-override` 只检查 reason，缺少 chief/parent-session/managed authorization；Bohrium job 的 submit/download/kill/describe 也缺少 owner、role 和 workspace path 约束。
- `loadCredentials` 会把 token 写入普通 `credentials.json`，代码未证明 600/ACL；规则文件损坏时继续使用默认规则；关键 hook 安装失败仅 warning 后继续运行。这三类都与“硬门”目标冲突。
- web `/status`、`/agents` 读取接口缺少鉴权/严格 session scope；若 app-server 或远端环境暴露，会泄露身份、路径和作战台账。
- `serverQuotaExceeded`、`challengeMeta` 请求失败时当前策略倾向继续提交，属于 best-effort，不是硬 quota/channel gate。
- 当前 ModelGate 也不是硬门：`model.js:12-18` 的 waterfall 只记录/审计，`runGates` 的 model gate 在 `submit.js:330-335` 始终返回 `ok:true`，未核对 effective provider/model；Codex 版必须在 spawn 和 submit admission 同时锁定。
- channel 仍有语义缺口：`rest_no_script` 可被声明为允许形态，但 `runSubmission` 实际统一调用 CLI；迁移版必须把声明 channel 与实际 transport/method 绑定，或暂时只允许已验证的 CLI route。
- `traceAdmission` 的“单信号通过”不仅是实现问题，插件 red-team 规格文档也明确如此；ASCodex 应先改规格，再写 Rust 测试，至少要求 provenance 与 tool/artifact/log/cost 中的组合证据。
- ASCodex 当前约 381 个文件、约 8.3 MB，无 `.git`、无官方 Codex `Cargo.toml`/构建入口；它是知识镜像，不是可运行 fork。官方源码只能以固定 commit 导入为独立 `codex/` 子树。

## recheck 补充发现

对 `C:\Users\XKZ\dsh-plugins\dsh-solver-guard` v0.2.1 和迁移镜像再次核对的结论：

- 插件没有 `.git` 或 lockfile；digest 已生成并核对通过，后续源码变化仍需显式重生成并核对。
- 正确测试入口是 `node --test test/*.test.mjs`；`npm test` 没有定义，不能把“无脚本”当成通过。
- `checkBohrBudget` 只累计已回填的 `job.cost`，pending/running 任务未按估算成本 reservation；存在并发超预算窗口。ASCodex 必须在 SQLite 中原子预留估算额度，再允许启动任务。
- `spawnCli`/`spawnBohr` 将整个 `process.env` 传给子进程；迁移版必须使用最小环境白名单，只注入单次凭据句柄，禁止跨 provider 泄漏。
- `python_only` 仅检查启动命令，脚本内部仍可发起 `requests`/`curl` 等网络写操作；不能把现有 exec 视为提交隔离。必须采用 OS 网络 egress/sandbox 或统一执行 broker。
- `JsonStore` 写盘异常被 warning 并吞掉；磁盘满/权限错误时账本退化为内存，重启可重复提交。关键 reservation/commit/override 写失败必须 fail-closed，并将系统标记为 degraded。
- rules 中的 `cli_path`/`playground_js` 与 PATH 上的 `bohr` 无签名或 hash 校验；ASCodex 应固定受信任绝对路径与摘要，配置变更走管理员流程。

以上问题均列为 ASCodex P0/P1 验收前置条件；不能通过复制 Node 文件或角色文档宣称已启用硬门。

## ASCodex runtime hardening 已落地项

- Core Tool Registry 现在在 PreToolUse hook 改写参数后重新执行同一 `tool_preflight_with_input`。因此 hook 不能把原本安全的参数改写成提交/网络命令后绕过 solver profile；该检查仍是 Guard broker 之外的纵深防御。
- `ChannelPolicy` 新增可选 `trusted_cli_root`。若未显式配置，受信 CLI 必须位于 `workspace_root`；配置了该字段时，CLI 路径必须位于其 canonical 子树并通过 SHA-256 校验。
- `solver_guard_submit` 现在保留并检查 `ExecutionRecord`，要求其 `session_id` 与 live invocation 一致、`agent_id` 与请求身份一致；仅凭 trace 布尔值或模型自报上下文仍不能通过。
- 新增 `OodaCycleRecord`/`CycleDirective` 与 `ClosureEvidence`，将阶段 ACL、stuck 强制 replan/异质复核、封板三问、双轨证据和历史最高值保护落为协议层校验。Core/app-server 尚未把每次 dispatch 全部绑定到该协议，因此仍不能宣称运行时全链路硬约束已完成。
