# ASCLocal-Codex（ASCodex）

面向 Codex 的 Bohrium Playground 解题工程工作区。它将源自 `ASCLocal` 实战与 DeepSeek Harness 的解题经验、评分真相和提交纪律迁移到 Codex 生态，并把其中关键能力（提交前 Guard 门控、审计账本、多角色协作、确定性 Trace 构建、平台 ARM bundle 组装）沉淀为**可本地编译、可测试、可审计**的运行时组件。

> AI 协作代理的操作规则见 [AGENTS.md](AGENTS.md)；本文档介绍项目本身。原始来源工作区的 README 见 [README.source.md](README.source.md)。

## 1. 项目定位

本仓库同时承载两件事：

**① 知识资产镜像（只读基线）**
- `ASCLocal`：Bohrium/Playground 解题经验、平台/API/ARM 协议文档、Round-3 作战复盘与工具脚本。
- DeepSeek Harness：32 个技能原文与 7 份 Cordis preset（`skills/deepseek-harness/`、`agents/harness-presets/source/`，仅作审计与追溯，不可直接加载）。
- Codex 侧适配产物：活跃技能集（`.agents/skills/`）、协作角色定义（`agents/codex-roles/`）、脱敏配置与迁移清单（`config/`）。

**② ASCodex 运行时试验台（工程资产）**
- 以 Rust 在固定 commit 的官方 Codex 源码树（`codex/`，见 `codex/SOURCE_COMMIT.txt`）之上实现：`solver-guard`（门控与审计）、`ascodex-coordination`（协作协议）、`ascodex-runtime`（后台常驻），并在 `core` 注册 `solver_guard_*` 工具。
- 辅以 Python 只读观测/对账脚本（`scripts/`）与 SQLite 事件账本。
- 镜像**不复制**源侧任何凭据、会话、日志与运行态。

## 2. 设计原则

1. 凭据只从进程环境变量读取；`private/` 仅存放本地示例，token/密钥永不入库。
2. 写操作默认 dry-run；真实提交前核对题目、身份额度、challengeId、通道、Trace、红线与模型。
3. 只接受可追溯证据：`submitted`/`queued` 不是成功，必须继续核实 replay、`resultsJson`、scorecard、harbor_reward 与官方榜状态。
4. Trace 必须来自真实执行；tool call/result 一一对应、stdout body、时间戳与 artifact provenance 自洽，禁止 prior score/attempt/团队信息污染。
5. 重型计算优先 Bohrium 云端；本地仅做短时 smoke test 与结果分析。
6. 不声明未实现的运行时能力——角色文档不等于运行时权限。

## 3. 仓库布局

| 路径 | 内容 |
|---|---|
| `AGENTS.md` | 工作区规范（AI 代理必读：布局、安全纪律、Codex 适配边界） |
| `bohrium-kb/` | 平台/API/ARM 协议文档（`docs/`）、作战手册与评分真相（`round3_prep/`）、源工作区工具（`tools/`） |
| `work/` | 题目工作区：`_template/` 为模板，进行中题目按 slug 建目录，完成后归档 |
| `skills/deepseek-harness/` | Harness 技能原文（仅追溯）；活跃适配技能在 `.agents/skills/` |
| `agents/` | `codex-roles/`（Codex 角色定义）、`harness-presets/source/`（Cordis preset 审计副本） |
| `codex/` | 官方 Codex 固定 commit 源码 + `codex-rs/` 工作区（含 ASCodex crates 与对 core 的补丁） |
| `codex/codex-rs/` | Rust crates：`solver-guard/`、`ascodex-coordination/`、`ascodex-runtime/`、`core/`（工具注册） |
| `config/` | 架构/部署/策略文档、`codex-capability-map.md`、`baselines/`（只读基线 SHA-256）、脱敏配置参考 |
| `scripts/` | ASCodex 运维/观测脚本与 `ascodex.ps1` 一键入口 |
| `docs/` | 项目报告，如 `E2E-REPORT.md`（真实 LLM 端到端验证证据链） |
| `private/` | 本地示例目录（凭据/运行态不入库） |
| `tests/`、`out/`、`archive/` | 测试、产物与归档 |

## 4. ASCodex 运行时组件

### 4.1 Guard 门控与审计账本（`solver-guard`）
- 类型化策略门：通道/身份/间隔/红线/Trace/模型 + `Gate::Bohr` 云算力门（机型白名单、每题预算、本地 smoke 上限）。
- SQLite 账本：reservation / event / actor lease registry；identity 级额度与池生命周期（`ascodex-pool-admin`）。
- workspace/CLI 完整性校验、trace/artifact/execution 校验、Guard 派生 redline。
- 信任链：编译期 Ed25519 策略信任锚（`<policy>.sig` 验签）、solver-profile 网络 egress allowlist（typed `EgressPolicy`）、solver child OS 网络隔离（Restricted）。
- 三处 preflight：Core Tool Registry、app-server RPC、AgentControl lineage。

### 4.2 确定性 Trace 构建
- Rust 确定性 trace builder（`solver-guard/src/trace_builder.rs`）+ Core 工具 `solver_guard_build_trace`。
- 软件接管 Trace 构建后**生成即过** Guard 门：real_execution + paired + provenance 全通过；字段与平台 anti-fraud 六信号兼容（E2E 实测为平台门的严格超集），一次达标平台 `ready` 与 `trace_quality=1.0`。

### 4.3 协作协议（`ascodex-coordination`）
- `ActorContext`/lease、OODA/封板契约、typed `PlatformObservation`、四类成功证据约束。
- StageBrief 将技能/能力图 policy root 与角色 challenge workspace root **分开签名验证**——solver 读取 `.agents/skills` 不会因此获得仓库根写权限。
- 自动推送编排（Push 轮次、冷却 Wait、Chief 窗口内/外 force 规则）以集成测试验证（auto_push 7 项全过）。
- 后台常驻组件以独立二进制提供（非模型工具）：`ascodex-supervisor`（周期拾取 + 指数退避）、`ascodex-canary-runner`（两回合隔离子线程探针）、`ascodex-wake-admin`/`ascodex-wake-consumer`（消费成功文件改名 `*.json.consumed` 收敛）。

### 4.4 平台组装与只读观测（`scripts/`）
- `ascodex_arm_bundle.py`：把 solver evidence 组装为平台 ARM v1.1 bundle（arm_manifest、characterization.json、trace 等）；缺 `solve.py` 时自动生成 reproduce stub。已在真实题验证：bundleStatus=ready。
- `ascodex_monitor.py` / `ascodex_leaderboard_check.py` / `ascodex_schema.py`：消费**已保存的只读 JSON 响应**，校验归属/replay/results/scorecard/榜单后原子写出带 SHA-256 的 typed observation / confirmation。不联网、不执行提交。
- `ascodex_reconciliation*.py`：对账调度与执行。
- `ascodex.ps1`：Windows 一键入口（check / build / test / run）。

## 5. 现状与启用边界

**已本地验证**
- `solver-guard` 85 项测试、`ascodex-coordination`（含 auto_push）、`ascodex-runtime` 与 Python 单测全过。
- 真实 LLM 端到端（chief→solver 派发、StageBrief 注入、门控拦截、真实执行 Trace、恢复路径）跑通，E2E 暴露的 8 个问题已修复；证据链见 [docs/E2E-REPORT.md](docs/E2E-REPORT.md)。

**已做平台对齐验证（只读 / 授权内 draft）**
- 从 `play.bohrium.com` 拉取权威 live 协议并核对评分契约：评分器只读 `characterization.json` 的 `deviations_from_paper[]`，Trace 是 anti-fraud 门槛不进分数。
- 以注册 agent 身份完成 draft attempt 创建 + ARM bundle 上传，达 `bundleStatus=ready`（human_review 题上传即结构性满分 1.0/1.0/1.0）。

**仍为边界（不宣称已启用）**
- 正式 executor 提交与评分器高分验证仍需平台写授权；本地 Guard/控制面二进制只服务 dry-run 与账本，不代理平台写通道。
- agnes 多步稳定性仍在改善：子代理偶发误解任务或超长推理不落工具，E2E 报告记录了规避措施。
- `ASCODEX_SOLVER_MODE=1` 仅是 P3 过渡开关：启用 solver profile 必须显式提供本地 typed policy（示例 policy 仅用于 schema/dry-run），且依赖合法 Chief lease 签发的 cycle/brief 与 recovery canary 记录。

## 6. 构建与验证（Windows）

Rust 验证需显式 stable 工具链（本机已由环境指定）：

```powershell
$env:RUSTC = 'C:\Users\XKZ\.rustup\toolchains\stable-x86_64-pc-windows-msvc\bin\rustc.exe'
& 'C:\Users\XKZ\.rustup\toolchains\stable-x86_64-pc-windows-msvc\bin\cargo.exe' test -p codex-ascodex-coordination -p codex-ascodex-runtime -p codex-solver-guard --locked --offline
& 'C:\Users\XKZ\.rustup\toolchains\stable-x86_64-pc-windows-msvc\bin\cargo.exe' check -p codex-core --locked --offline
```

或使用一键入口 `scripts/ascodex.ps1`（离线 Cargo 构建，不写凭据，不触发真实平台写）：

```powershell
./scripts/ascodex.ps1 -Action check
./scripts/ascodex.ps1 -Action build
./scripts/ascodex.ps1 -Action test
./scripts/ascodex.ps1 -Action run -- exec --help
```

启用 solver profile 必须显式提供本地 typed policy 与账本（示例 policy 不能直接启用）：

```powershell
./scripts/ascodex.ps1 -Action run -SolverMode -PolicyFile C:/private/ascodex-policy.yaml -LedgerFile C:/private/ascodex.sqlite -CycleId <cycle-id> -CycleEventVersion <cycle-event-version> -CampaignId <campaign-id> -ChallengeId <challenge-id> -- exec
```

## 7. 运维与控制面

只读平台响应固化为证据（工具离线，输入须为只读客户端已保存的 JSON）：

```powershell
python scripts/ascodex_monitor.py `
  --response .\private\attempt-response.json `
  --challenge-id <challenge-id> --attempt-id <attempt-id> `
  --route /api/attempts/<attempt-id> `
  --output .\private\platform-observation.json
```

管理侧控制面是独立二进制（未注册为模型工具，要求绝对 SQLite 路径与本地 context JSON，单一事务落账，仅服务 dry-run）：

- `ascodex-lease-admin`：lease provision/revoke 与审计（`cargo run -p codex-solver-guard --bin ascodex-lease-admin`）。
- `ascodex-stage-admin`：Chief 循环与阶段简报签发（`issue`）/ 换代（`supersede`，须显式 predecessor，旧 cycle 与全部 brief 同事务撤销）/ 停止（`revoke`）。
- `ascodex-observation-admin`：在 Monitor lease 下把 typed observation 原子写入事实账本。

各命令的完整参数可用 `-Action help` / `--help` 查看；涉及提交或删除的脚本只能先 dry-run、获明确授权后使用。

## 8. 入手指南与参考文档

1. **代理规则**：先读 [AGENTS.md](AGENTS.md)（安全与证据纪律、Codex 适配边界）。
2. **作战与评分真相**：`bohrium-kb/round3_prep/INDEX.md`、`OPERATIONS_PLAYBOOK.md`、`IDENTITY_POOL.md`。
3. **工具映射**：遇到 `solver-guard_*`、`dsh-tool-*`、Lark 或 Playground CLI 名称，查 `config/codex-capability-map.md` 的 Codex 映射/降级。
4. **架构与部署**：`config/ascodex-coordination-architecture.md`（设计基线）、`config/ascodex-deployment-plan.md`（部署边界）。
5. **验证证据**：[docs/E2E-REPORT.md](docs/E2E-REPORT.md)（真实 LLM 全链路 + 真实平台只读/评分契约对齐 + draft bundle `ready`）。
6. **角色协作**：按 `agents/codex-roles/` 执行；research-scientist 只编排，solver 才能提出写操作，monitor/judge/intel/red-team 默认只读。任何真实提交或删除前先报告目标、身份、预算、dry-run 结果与回滚方案。
