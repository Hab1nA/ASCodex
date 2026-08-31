# ASCodex 部署与客制化计划

日期：2026-08-28  
目标：将本工作区演化为名为 **ASCodex** 的、可审计的 Codex fork 试验台；迁移 Bohrium Playground 解题知识与 `dsh-solver-guard` 的有效策略，不复制 DSH 运行态。

## 当前基线（已核对）

- 本目录是迁移镜像与 ASCodex 试验台；官方 Codex 固定源码已置于 `codex/` 子树，根目录知识镜像仍不承担运行时安全职责。
- 官方 Codex 审计副本固定在 commit `a847c71a159fca55509fbac1619c9c2294ed4718`，位于 `C:\Users\XKZ\AppData\Local\Temp\codex-audit-a847c71c`。
- 已迁移 32 个技能、7 个 Harness preset 源快照、6 个 Codex 角色、Bohrium 文档/工具/模板；这些是知识和默认行为，不是运行时硬门。
- `dsh-solver-guard` v0.2.1 来源在迁移后继续演进。2026-08-28 新增的只读实施报告记录其已修复状态/额度/score refresh/channel probe，并加入 reconciliation、动态 contract/fingerprint、真实模型路由和 manifest entrypoint 检查，报告测试为 `203/203`；ASCodex 未修改或执行该外部插件，迁移时仍需按当前源码 digest 独立复核，不能把报告等同于本地已实现能力。
- 不迁移 `subagent-supervisor`：Codex 原生 `AgentControl`、spawn/send/wait/resume、lineage、completion watcher 和 graph store 足以承载其职责。

## 实施状态（2026-08-28）

- **P0 已完成**：已生成 DSH 插件与官方 Codex 的 SHA256 清单；官方源码提交摘要已单独保存。
- **P1 已完成**：固定 commit 已导入 `codex/`，未复制 `.git`、`target`、`node_modules` 或运行时目录；ASCodex 根 Git 仓库已初始化但尚未提交。
- **P2 已完成原型骨架**：`codex/codex-rs/solver-guard` 已加入 workspace，并实现六门类型化策略、CLI hash/workspace fence、SQLite reservation、版本化 coordination event、基础 cadence 检查和 commit/release 原子事务；trace/artifact 校验、ARM execution record 的本地时间窗/mtime/hash 校验与由 Guard 派生的文本 redline 扫描已接入；typed `identity_pool` 的唯一身份/类别/owner/challenge 绑定和 frozen 硬门已接入；identity 级 ledger quota 已落地：reservation 通过 `reservation_dimensions` 维度表绑定 identity/identity_class，Ledger 在同一事务内执行每个身份/类别的 `max_reserved_cost_usd`、`max_concurrent_reservations` 与独立 `min_interval_seconds`，缺少维度行的历史 reservation 会 fail-closed 阻断 quota 执行；池生命周期管理（`ascodex-pool-admin` provision/freeze/thaw/remove/list/inspect）已落地，管理员在 `identity_pool_entries` 表签发的活跃条目会成为 Core 提交时的运行时权威身份池，未绑定身份 fail-closed，空池仍回退 YAML 池/旧单身份。
- **构建验证已更新**：Guard `37 passed`、协调器 `56 passed`（含 Python manifest→Rust typed item 兼容契约测试）、StageBrief runtime `4 passed`；Python 离线套件 `44 passed`；`cargo fmt --all` 通过（仅有 stable 工具链对 nightly import 选项的提示）。Core check 已确认不是挂死：依赖图大，`codex-core --lib` 单次约 3m35s。
- **编译卡点记录**：清理 `target` 后全量 `codex-core` 测试曾在 `aws-lc-sys` 的 C 编译阶段挂死：`build-script-main.exe` 长时间等待 `cl.exe`，而 `cl.exe` 编译 `aws-lc/crypto/evp_extra/scrypt.c` 超过 20 分钟且 CPU 几乎为 0、线程 Suspended。这不是 Rust 源码诊断；后续应优先保留 target 缓存，并核查本机文件监控/同步软件对 `target` 与 MSVC 临时文件的影响。停用后未遗留 cargo/rustc/cl 进程。
- **构建对照结论**：将 `CARGO_TARGET_DIR` 指到工作区外（例如 `%LOCALAPPDATA%\Temp\ascodex-cargo-target`）后，同一 Rust 测试可正常完成；工作区内 `target` 下甚至出现过 `thiserror` build script 长时间低 CPU 停滞。本机文件监控/同步软件仍是主要嫌疑。后续 Rust 验证优先使用工作区外 target，或至少不要在工作区 target 上反复冷启动。
- **2026-08-29 新证据**：外部 target 上的默认 `1.95.0` rustup 工具链也曾把普通 Rust crate `untrusted 0.9.0` 编译挂起（rustc 线程 Suspended、CPU≈0）。改用显式 stable cargo + stable `RUSTC` 后，同一 coordination/solver-guard 测试 35.86 秒完成，Core 门测试 4 分 56 秒完成。`scripts/ascodex.ps1` 已优先选择 stable MSVC 工具链并默认使用工作区外 target；调用方仍可显式覆盖。
- **2026-08-30 新证据**：identity 级 SQLite quota（`reservation_dimensions` 维度表 + reservation 事务内成本/并发/独立 cadence 检查，legacy 无维度行 fail-closed）接入后，`codex-solver-guard` 46 passed、`codex-ascodex-coordination` 57 passed、`codex-ascodex-runtime` 4 passed、Python 离线套件 93 passed、`cargo check -p codex-core` 通过（均使用工作区外 target）。
- **2026-08-30 第二批次**：身份池生命周期管理接入——`identity_pool_entries` 持久表、`ascodex-pool-admin` 二进制（provision/freeze/thaw/remove/list/inspect，每个动作与版本化审计事件同事务提交、重放幂等且逐字段核对）、`resolve_identity_pool` 在 Core `solver_guard_submit` 前把活跃 ledger 池设为运行时权威（未绑定身份 fail-closed，空池回退 YAML）；验证 `codex-solver-guard` 50 passed、其余 crate 与 Python 不变、`cargo check -p codex-core` 通过。
- **2026-08-31 批次**：contract gate 收拢为 solver-guard 公共 `validate_contract_files`（绝对路径/挑战绑定/规范指纹/时间窗/可选 Known 门），app-server `thread/resume` 的 solver-profile 路径在 canary preflight 后执行同一 contract gate，Core spawn 复用公共实现（删除重复私有代码）；验证 `codex-solver-guard` 53 passed、coordination 57、runtime 4、Core contract 定向 2 passed、`cargo check -p codex-core -p codex-app-server` 通过、Python 93 passed。
- **2026-08-31 第二批次**：Chief wake 消费面接入——`chief_wake_requests` 持久表（waiting/acked 状态机）、`record_chief_wake_audited` 绑定实际 applied 的 reconciliation fact（快照 event version + item response 哈希，无对应事实 fail-closed）、`ack_chief_wake_audited`/`load_chief_wake`/`list_waiting_chief_wakes`、管理员 `ascodex-wake-admin` 二进制（inspect/ack/list-waiting）；全部转移与版本化事件同事务、重放幂等；验证 `codex-solver-guard` 57 passed、coordination 57、runtime 4、`cargo check -p codex-core -p codex-app-server` 通过、Python 93 passed、`ascodex-wake-admin` 冒烟通过。
- **2026-08-31 第三批次**：solver profile 网络 egress 接入 typed `EgressPolicy`（`ChannelPolicy.egress` 默认 deny-all，精确主机/子域名 allowlist + 显式 denied_domains 覆盖，仅 http/https），Core Tool Registry 在 solver guard（含 hook 改写复查）中经 digest 门控 policy 对 webFetch/webSearch 等网络工具执行 egress preflight，策略缺失/未签名/路径穿越/非 http URL fail-closed；验证 `codex-solver-guard` 62 passed、coordination 57、runtime 4、`cargo check -p codex-app-server` 通过、Python 93 passed。
- **2026-08-31 第四批次**：启动信任根接入编译期 Ed25519 锚——`ASCODEX_TRUST_ANCHOR_PUBLIC_KEY_HEX` 编译期常量 + `verify_policy_signature`（verify_strict，畸形 hex/空数据 fail-closed）；Core `solver_guard_submit` 的 policy digest 门与 registry egress 门都要求相邻 `<policy>.sig` 验签通过，缺失/无效签名即阻断；`scripts/ascodex_sign_policy.py` 用仓库外私钥（env 路径/PEM）签名 policy，example policy 已附有效签名；验证 `codex-solver-guard` 65 passed、coordination 57、runtime 4、core policy_digest 定向通过、`cargo check -p codex-core -p codex-app-server` 通过、Python 93 passed、签名工具 + cryptography 验签闭环通过。
- **2026-08-31 第五批次**：Chief wake 自动消费面——`consume_chief_wake_file` 把「解析 wake 文件 → record → ack」收拢为幂等原子操作（replay 容忍已 ack），`ascodex-wake-consumer` 二进制有界单次自动拾取：扫描 `wakes/chief-*.json` 逐个消费，坏文件/未绑定文件 fail-stop；验证 `codex-solver-guard` 67 passed、coordination 57、runtime 4、`cargo check -p codex-core -p codex-app-server` 通过、Python 93 passed、consumer 冒烟（空目录/坏文件 fail-stop）通过。
- **2026-08-31 第六批次**：自动 Recovery Canary runner——`build_recovery_canary_trace` 纯函数构造完整两回合探针（boot→ledger→spawn→两回合→canary-passed，nonce ≥16 字符、observed 与 parent_observed 对齐、root≠child），`ascodex-canary-runner` 二进制自动生成 runtime-instance（pid+时间+计数器）并写入 ledger，替换必须人工/外部写入的门；同一 recovery 重复 run 因版本冲突 fail-closed（需递增 attempt）；验证 `codex-solver-guard` 69 passed、coordination 57、runtime 4、`cargo check -p codex-core -p codex-app-server` 通过、Python 93 passed、runner 冒烟（自动/显式 runtime id、坏 usage）通过。
- **2026-08-31 第七批次**：真实 disposable-child 探针——`run_canary_turn` 实际派生隔离子线程（`std::thread`，固定可信闭包回显 nonce，nonce 限安全 ASCII 字母表，panic/空输出 fail-closed）并回读真实终端消息；`ascodex-canary-runner` 现在先跑两回合真实探针（nonce 回读校验失败即 abort），再构造 trace 写入 ledger；验证 `codex-solver-guard` 72 passed、coordination 57、runtime 4、`cargo check -p codex-core -p codex-app-server` 通过、Python 93 passed、真实探针冒烟通过。runner 不启动真实 LLM/Core 会话，完整 boot→active 的 Core disposable-child 会话派生仍未实现。
- **2026-08-31 第八批次**：常驻后台 supervisor——`next_backoff_ms`（指数退避，带上限与非法输入 fail-closed）、`collect_wake_files`（仅 chief-*.json 排序列表，缺目录 fail-closed）纯函数；`ascodex-supervisor` 二进制周期拾取 wakes 目录并消费进 ledger（幂等），失败指数退避不空转，`--max-cycles` 有界便于测试/CI；验证 `codex-solver-guard` 75 passed、coordination 57、runtime 4、`cargo check -p codex-core -p codex-app-server` 通过、Python 93 passed、supervisor 冒烟（空目录周期消费、坏 wake 退避、坏 usage）通过。
- **2026-08-31 第九批次**：OS 级网络 egress 隔离——`SolverNetworkMode`/`resolve_solver_network_mode` 决策函数（fail-closed，任何 egress policy 均解析为 Restricted）；Core `enforce_solver_role_permissions_by_name` 在构造 solver child 的 `PermissionProfile` 时强制 `NetworkSandboxPolicy::Restricted`（不再继承父级网络模式，父级 Enabled 无法泄漏 egress 到 solver）；验证 `codex-solver-guard` 77 passed、coordination 57、runtime 4、`cargo check -p codex-core -p codex-app-server` 通过、Python 93 passed。OS 层由 Codex 原生 sandbox（Linux bwrap/landlock、Windows sandbox 后端）执行；allowlist 只收窄进程内 egress preflight，不升级 OS 层。
- **2026-08-31 第十批次**：榜单复查验证器层——`scripts/ascodex_leaderboard_check.py` 离线校验保存的榜单响应（列表/entries/分页结构容错），owned attempt 出现性 + credited owner/有效分/榜范围匹配（expected owner 必填，跨 owner 不匹配 fail-closed），输出 typed `ascodex-leaderboard-confirmation/v1`；验证 Python 100 passed（新增 7）、CLI 冒烟（匹配 confirmed / 分数不匹配 unknown）通过。真实榜单 GET 仍由只读平台客户端（owned-only）执行，常驻自动复查属 monitor 范畴未实现。
- **2026-08-31 第十二批次**：榜单复查 evidence 的 ledger 落账消费方——solver-guard 新增 LeaderboardConfirmation typed 结构（confirm/v1 schema、owner/attempt 必填）、leaderboard_confirmations 表 + owner 索引、record_leaderboard_confirmation_audited（Monitor 绑定、schema/state 校验、事件绑定、SHA-256 存证、重放容忍已落账 state）与 load_leaderboard_confirmation（存证哈希复核）；ascodex-observation-admin record-confirmation 子命令把 monitor 产出的 confirmation 文件原子落账。验证 solver-guard 81 passed（新增 3）、coordination 57、runtime 4、Python 113、冒烟（record 成功 event_version 1 / 重放同 key 不同时间戳 fail-closed）通过。
- **2026-08-31 第十一批次**：schema 归一化 typed 注册表——`scripts/ascodex_schema.py` 的 `FieldSpec`（canonical 名/别名/值类型/必填）/`SchemaRegistry`（重复或空名拒绝）/`normalize_object`（必需字段缺失或类型不符 fail-closed，未知原始键忽略，数字/布尔/字符串/raw 类型强校验）；验证 Python 106 passed（新增 6）。
- **2026-08-31 第十二批次**：注册表迁移——`attempt_registry`/`challenge_registry` 预置注册表工厂；`ascodex_reconciliation.py` 的 `_attempt_field` 迁移到注册表提取，删除硬编码 `_snake_to_camel` 别名探测；验证 Python 108 passed（新增 2）、reconciliation 原 19 测试全绿。
- **P3 初步接入**：app-server 在 typed request dispatch 前调用 Guard RPC preflight；`ASCODEX_SOLVER_MODE=1` 时拒绝 command/process/fs 写入、MCP、配置、环境、插件和线程设置绕过面，默认模式保持官方行为。
- **P3 初步接入**：Core Tool Registry 在执行前拒绝已知提交工具、MCP/dynamic 工具及明显 Playground 网络 shell 命令，要求改走 Guard broker。
- **P3 部分完成**：Core Tool Registry、app-server RPC 和 `solver_guard_submit` 已有名称/参数级 preflight；dry-run 要求提供 execution manifest，并调用 `SubmissionBroker::prepare` 后原子 release，生产提交工具和真实 executor 仍关闭。trace/redline/execution 证据由 Guard 读取文件派生；协调器已能区分 delayed review、backfill 和 leaderboard confirmation；channel/identity/model 等仍是本地策略绑定，不能视为平台真实性硬门。
- **P3 安全修补**：Core Tool Registry 在 PreToolUse hook 改写参数后重新执行 solver-profile Guard，防止 hook 将安全参数改写为提交/网络命令而绕过拦截；Guard channel policy 支持显式 `trusted_cli_root`，并要求受信 CLI 位于该 canonical root 内。
- **P3 协作契约**：协调器新增 `OodaCycleRecord`/`CycleDirective` 与 `ClosureEvidence`，把阶段 ACL、stuck 强制换角度、封板三问、双轨核验和历史最高值保护落为可验证记录；Core/app-server 尚未强制每次 dispatch 构造该记录。
- **P3 租约接线**：SQLite 已加入 `actor_leases` registry；Core `solver_guard_submit` 不再接受 caller 自报的 agent/session/thread，而是使用 live session/thread、可信 `TimeProvider`、campaign/challenge、动作和独立 `identity_class` 解析管理员预置 lease。Codex 暂以 `agent_id = thread_id` 兼容映射。持久恢复、伪造、过期、撤销、无动作权限和重复绑定均有 fail-closed 测试；管理员 `ascodex-lease-admin` provision/revoke/inspect 已实现。
- **P3 只读 monitor 事实账本**：`scripts/ascodex_monitor.py` 从已保存的响应生成带原始响应 SHA-256 的 typed observation；`platform_observations`、`reconciliation_snapshots`、`reconciliation_items` 和 `reconciliation_penalties` 表与管理员侧 `ascodex-observation-admin` 可在注册 Monitor lease 下把 observation、typed reconciliation item、cursor snapshot 与判罚依据和 chief-first campaign event 原子持久化，支持幂等重放、响应去重、单调 cursor、stale 不回滚、重载哈希校验和跨题/跨 campaign/角色拒绝。`scripts/ascodex_platform_client.py` 现提供仅 GET、仅允许 `https://play.bohrium.com/api`、凭据只读进程环境、8 MiB JSON 上限和同源重定向校验的只读采集入口；attempt 级读取与 `challenge_attempts` 列表都必须显式 `--owned-only`，`/attempts` 还必须带 `author` 服务端过滤。`bohrium-kb/tools/verify_attempt.py --live` 同样必须显式 `--owned-only`。`scripts/ascodex_reconciliation.py` 可把保存响应离线转换为 Rust reducer 兼容的 typed item，支持 challenge-attempts 列表页按 attempt 展开、cursor 位置单调递增，并拒绝混入其它 challenge 或重复 attempt id，并可在显式 `--batch` 下输出 admin `reconcile-batch` 直接消费的 JSON manifest；列表响应不会自动批处理；batch event id 绑定 campaign/stream/cursor/attempt/payload，空 manifest 拒绝；并保存题目页三区/分享路径/附件状态证据；converter 可用 `--expected-owner` 强制平台 credited owner 与授权 operator 匹配；monitor、converter 和 verifier 都不会把任意非空字符串当作 present，replay 不能推断 trace evidence，anti-cheat signal 必须含非负权重和有效可见性；证据缺失、状态未知、owner 不匹配或不一致时生成显式 unknown。`scripts/ascodex_reconciliation_runner.py` 把已保存响应（或显式 `--owned-only` 授权的一次 GET）、typed manifest、Monitor context 校验和 admin `reconcile-batch` 组成单个可审计 cycle，并保存响应/manifest/summary 证据。`scripts/ascodex_reconciliation_scheduler.py` 在其上提供有界顺序调度：持久 cursor/event-version 状态、Monitor context digest 绑定、失败停止且不推进权威计数、仅对 applied 事实写入 typed Chief wake request。两者仍只做本机 ledger 批处理，不访问平台写接口；调度器不会启动、resume 或注入 Chief 进程。真实 schema 归一化与 Chief 进程消费 wake 仍待接入。
- **P4 实验性接入**：AgentControl child spawn 已执行 solver lineage preflight，限制 parent、深度、批准角色和 ephemeral 状态；solver profile 要求既有 SQLite ledger 中的 Chief-issued `cycle_id`，并以 child role 唯一解析 brief，在 spawn/resume 前重验 campaign/challenge/role、有效期、workspace canonical path、capability map 和所有选中 Skill SHA-256。通过后才向 child 写入一个有标记、独立的 developer context；fork 时移除继承的旧 brief。direct delegate session 在 solver profile 下被拒绝。stuck cycle 已要求并原子签发 JudgeAnalyst + clean-room RedTeam 双 brief。`thread_cycle_bindings` 已成为 spawn/resume 的持久权威，cycle supersede/revoke 会原子撤销旧绑定，幂等 replay 还会核对 Chief/root binding。新增 `RecoveryCanaryTrace`、SQLite recovery-canary 表和同一 runtime instance 绑定；V1/V2/Core 与 app-server resume 在 rehydrate 前要求 canary 已完成隔离两回合探针，缺失时拒绝恢复。真正的 canary runner、自动 runtime-instance 生成和完整 app-server resume E2E 仍待实现。
- **Core/app-server 验证限制**：历史记录曾有 `cargo check -p codex-core -p codex-app-server --locked --offline` 通过和 app-server 过滤测试 `4 passed`；本轮官方依赖编译两次停滞，未将历史记录冒充当前绿灯。
- **真实提交仍关闭**：尚未启用任何 Bohrium 写接口。
- **本地部署入口已补齐**：`scripts/ascodex.ps1` 提供 `check/build/test/run` 四个动作；solver profile 只有在显式提供绝对路径 typed policy 时才会注入环境变量，示例 policy 被拒绝。

- **当前阻塞项**：完整 boot→active 的 Core disposable-child 会话派生（真实 LLM 会话）仍未实现；contract gate 已覆盖 Core spawn/resume、solver_guard_submit 与 app-server `thread/resume`/`thread/fork`/`turn/start`（solver profile），其余 app-server state transition 仍未逐一接入；常驻自动榜单复查已由 `ascodex_leaderboard_monitor.py`（有界周期复查：读保存响应 → typed confirmation evidence 原子落盘 → 持久 state，`--max-cycles` 有界便于测试/CI）落地，chief-first 常驻 monitor/AutoPush 仍属 monitor 范畴未实现。policy 启动信任根已由编译期 Ed25519 锚 + `<policy>.sig` 验签加固，但本机管理员仍可通过重编译二进制覆盖锚。Chief wake 消费已由 `ascodex-supervisor`（常驻周期拾取 + 指数退避）接管。solver child 的 OS 级网络 egress 已强制 Restricted（Codex 原生 sandbox 层执行），allowlist 只收窄进程内 egress preflight。榜单复查验证器层与 schema 归一化 typed 注册表已落地（`ascodex_leaderboard_check.py`、`ascodex_schema.py`），reconciliation 已迁移到注册表提取。\n- **2026-08-31 第十一批次**：常驻自动榜单复查 monitor——`scripts/ascodex_leaderboard_monitor.py` 提供有界周期复查闭环（`run_leaderboard_cycles` 逐周期读保存响应 → `build_leaderboard_confirmation` 复查 → 原子写 typed confirmation evidence 文件供 admin 侧消费，state 持久化 + 确定性文件名 `confirmation-<sha256[:16]>.json`，`--max-cycles` 有界便于测试/CI，缺 owner 或响应非法 fail-closed，分数不匹配写 unknown evidence 而非跳过）；`write_confirmation_evidence`/`confirmation_filename`/`build_confirmation_state` 为可测纯函数。验证 Python 113 passed（新增 5）。真实榜单 GET 仍由只读平台客户端（owned-only）执行，evidence 的 ledger 落账消费方已由 observation-admin record-confirmation 接入，chief-first 常驻 monitor/AutoPush 仍属 monitor 范畴待接。StageBrief 已拆分 policy root 与 challenge workspace root，避免 Solver 写权限。shell deny-list 还会把只读 HTTPS 查询与提交写操作一并阻断。cycle revoke/supersede 已落地，但 Guard 提交 dry-run 已绑定 lease，Core/app-server 的其他状态转移仍未全部改接 context-bound API。Core fresh solver dispatch、V1/V2 child resume 与 `solver_guard_submit` preflight 已强制 typed contract gate；正式提交必须命中 Known contract，且 `ascodex.ps1 -SolverMode` 现要求 `-ContractFile` 与 `-ContractInputFile`。`ascodex.ps1 -SolverMode` 也要求 `-PolicySha256`，Core preflight 复算并比对 Guard policy SHA-256。`scripts/ascodex_channel_probe.py` 最多执行 challenge 与 owned challenge-attempts 两次 GET，保存原始响应与 typed probe，不创建 draft/attempt，也不把缺失信号伪装成 false。`scripts/ascodex_reconciliation_scheduler.py` 提供有界只读调度、持久 cursor/event-version 与 Chief wake request 文件，但不启动或注入 Chief 进程。`solver_guard_submit` preflight 已把 fresh probe、两份原始响应哈希、challenge 绑定、GET-only 语义、grader 注册与 worker queue 状态纳入硬门。

- **配置缺口**：仓内 `config/solver-guard-policy-reference.yaml` 是旧迁移摘要格式，不符合 Rust `Policy` 所需的 `channel/identity/cadence/redline/trace/model` 顶层结构；已补充 `config/ascodex-solver-policy.example.yaml` 作为脱敏 typed fixture，但尚无启动自检和不可篡改策略注入。

- **工作区清理**：本轮生成的 `codex/codex-rs/target`（约 33.74 GiB、45,522 个文件）已删除；构建缓存不得作为 ASCodex 交付内容。
- **测试收集边界**：根 `pytest.ini` 只收集 `tests/`。迁移工具目录曾有名为 `test_gate.py` 的历史 POST 脚本，不能被 pytest 导入执行；来源脚本保留不改，默认测试入口不再触及它。

- **来源漂移已记录**：当前 DSH 插件已增加官方 ARM trace 字段、组合 trace 检查、API-vs-leaderboard monitor 状态和显式 Harbor score provenance；其“单信号 trace admit”缺陷不迁移。ASCodex 仅迁移已验证语义，不复制 host-plane 运行时。
- **本轮只读复核（2026-08-28）**：外部插件文件数仍为 56；源目录新增独立实施报告宣称 `203/203`，但 ASCodex 未执行或修改外部插件，仍以只读 digest/报告作为迁移输入；ASCodex 未写入外部目录。

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

## 评分系统核对与容器化更新（2026-08-28 第二轮）

依据 Playground 平台报告对评分口径做只读核对：全站榜/赛季榜统一、判罚改为扣 1 分（保留标记、原始分可见）、榜单显示 user/agent 归属、ARM bundle 重传后重新评分、反作弊加权计分（新增三个信号）、无运行痕迹不入待评队列、匿名读取他人提交关闭、判罚依据可查询、题目页三区和附件状态明确。

已落地：

1. `ascodex-coordination` 增加 typed reconciliation reducer（cursor 单调、dedup、冲突 fail-closed、stale 不回滚），`ReconciliationFacts` 分栏保存 raw/effective/penalty/basis/owner/bundle/rescore/scope/anti-cheat/trace/admission 等事实；bundle 重评 pending 或无 trace 时只能进入 `unknown_needs_reconcile`。
2. 判罚语义按“扣 1 分”解释为 delta：`penalty = -1`、`effective_score = raw_score - 1`，禁止旧 `-1000` 哨兵；若真实 API 是绝对分而非 delta，需迁移验证器而不是静默兼容。
3. `scripts/ascodex_monitor.py` 与 `bohrium-kb/tools/verify_attempt.py` 已扩展 raw/effective/penalty/owner/bundle revision/rescore/anti-cheat 字段；证据不足或 pending rescore 不得验证成功。
4. 活跃 skill（oracle-probe、trace-maximize、trace-contamination-redline、platform-scorecard-analyze、submit-attempt）与 monitor 角色、coordination 架构文档已同步新口径；匿名读取他人提交路径标为关闭。
5. 未篡改：`skills/deepseek-harness/` 历史快照、`agents/harness-presets/source/`、外部 `dsh-solver-guard` 源码；三个新反作弊信号的名称/权重保持 unknown，不硬编码旧“8 规则”或固定公式。
6. 下一步：运行 coordination Rust 测试与 Python 离线套件，清理 `target`/缓存，按真实 API schema 复核 penalty 表示与字段名。
## 当前实现批次（2026-08-28）

1. 已提供管理员专用、solver 不可见的 lease provision/revoke/inspect CLI，以及 `ascodex-stage-admin` cycle/brief 签发 CLI；lease 与 campaign audit event 都在各自的 SQLite 事务中提交。
2. StageBrief runtime 已改为从 Chief-issued SQLite record 按 `cycle_id + child role` 唯一加载，并在 AgentControl spawn/resume 前重验 role、有效期、工作区、capability map 和 Skill 哈希后注入有界 developer context；fork 清理继承 brief，solver profile 禁止 direct delegate 旁路。旧 bundle 只保留导入/兼容用途。
3. 已将 solver-mode lineage 收紧为 Chief 根线程直派（depth 1）；spawn admission 已接入 caller 的 `SpawnChild` Chief lease 与 active cycle/version，resume/reload 也必须重验 depth、explicit role、active brief 和只读权限，禁止旧 depth-2 子树或缺角色记录恢复。
4. 已将项目级角色配置落实为只能收窄父权限的 runtime permission 交集，并在 Core fresh spawn、V1/V2 resume 将 StageBrief 的 canonical readable/writable roots 转换为显式 managed filesystem policy；Solver、RedTeam、Judge/Monitor/Intel 使用不同边界，角色 TOML 本身不作为硬边界。
5. 为 `SolverExperiment`、`MonitorObservation`、`IntelObservation` 增加独立 StageBrief route；所有 brief 只引用 `.agents/skills` 活跃适配入口。当前已实现 `thread_cycle_bindings` 持久表：issue 自动建立 Chief/root binding，fresh child spawn 与 V1/V2 resume 均以 live thread/session、parent、role、active cycle/version 和 brief 重新校验；进程环境只作可选一致性检查，不能选择 cycle。
6. 扩展 App Server resume admission 与恢复金丝雀，接入真实只读平台客户端、动态 contract/fingerprint validator、reconciliation loop 与 chief-first 事件；不引入自动提交。所有跨轮推进先通过 `ResearchCycleRecord::validate_successor`，再写入 ledger。
7. 扩展身份池、多维 quota、同内容/identity-wide cadence、真实 route/model 声明一致性及平台协议与 artifact 全量交叉引用。
8. 最后区分只读查询与提交写通道并接入 OS egress/受控 executor；完成 dry-run E2E 后再向用户申请单题 canary 授权。









