# ASCodex 并行解题架构（RoundChief + 全并行派发）

状态：已实施（本文档描述设计与代码落点）。每轮比赛最多 10 道题；开轮时首席对全部题目**一次性原子派发**每题一个 bohrium-solver 子代理，全并行解题，各自走六门提交，首席汇总。

## 设计原则

1. **软件边界优先于模型纪律**：派发不是让首席模型循环调 10 次 spawn_agent（agnes 在多步派发上不稳定），而是一个确定性工具 `solver_round_dispatch` 读 RoundPlan 契约后逐题原子 spawn。模型只做一个调用，软件保证 N 个子代理全部按既定角色/工作区/租约派出。
2. **台账权威按题隔离**：chief 一个线程持有 N 条 `(campaign, challenge, cycle)` binding（每题一条，由 cycle 签发自动创建）；子代理 binding 按 `(父线程, campaign, challenge)` 精确解析，杜绝跨题串绑。
3. **单题纪律不放松**：六门（trace/execution/cross-ref/redline/channel/contract）、StageBrief 注入、workspace ACL、identity pool 按题校验全部保持；只是把"进程级单值 env 身份"改为"binding 行优先、env 兜底"。

## 数据流

```
运维员(脚本)                          首席会话(RoundChief, root)
────────────                          ─────────────────────────
lease_admin provision ×N     ──┐
stage_admin issue ×N  (每题     │   turn 1: 调 solver_round_dispatch(plan)
  cycle+brief+chief binding)  ──┼─▶   └─ 按题: 按题授权→contract→brief→ACL
pool_admin provision ×N      ──┤        →spawn(bohrium-solver, depth1, clean-room)
contract×N + challenge 工作区 ──┘        →bind_child(campaign,challenge)
                                      └─ 返回 N 条 spawn 回执（失败逐题上报，不整批失败）
                                      子代理并发运行（各自 tokio task）
子代理×N: 解题→run.log→build_trace→solver_guard_submit(dry_run 六门)
首席: wait_agent ×N → 汇总 round 报告（AutoPush 决策逐题评估）
```

## 组件与代码落点

### 1. RoundPlan 契约（ascodex-coordination/src/round_plan.rs）

`ascodex-round-plan/v1`：
```json
{
  "schema_version": "ascodex-round-plan/v1",
  "round_id": "round-1",
  "campaign_id": "camp-round-1",
  "task_message_template": "…{challenge_id}…{challenge_workspace}…",
  "challenges": [
    {"challenge_id": "ch-01", "lease_id": "lease-round-1-ch-01",
     "workspace_root": "…/solver-ws/ch-01", "task_name": "solver_ch_01"}
  ]
}
```
校验：1..=10 题、challenge/lease/task_name 唯一、id 无路径分隔符、模板含工作区占位符。逐题可用 `message` 覆盖模板。

### 2. 按题 binding 解析（solver-guard/src/lib.rs）

- `resolve_thread_cycle_binding_for_challenge(thread, session, Role::Chief, campaign, challenge, now)`：在 `thread_cycle_bindings` 上按 `(thread_id, role, campaign_id, challenge_id, revoked IS NULL)` 取**恰好一行**（0 或 >1 都 fail-closed），再复用与 `resolve_thread_cycle_binding` 相同的 cycle/brief 校验。
- `bind_child_thread_to_cycle_for_challenge(...)`：`bind_child_thread_to_cycle` 的按题变体——父行查询带 campaign/challenge 过滤。
- 复用事实：`issue_research_cycle_internal` 签发 cycle 时**原子插入 chief root binding**（`chief:{thread}:{cycle}`），所以 N 次 `stage_admin issue` 天然产生 N 条 chief binding，无需新写入方。

### 3. spawn 链路按题授权（core/src/agent/control.rs + spawn.rs）

- `SpawnAgentOptions.solver_round_challenge: Option<SolverSpawnChallenge { campaign_id, challenge_id, chief_lease_id }>`。
- `spawn_agent_internal` solver 分支：有 override 时走 `authorize_solver_spawn_from_chief_for_challenge`（按题 binding 解析 + `verify_chief_spawn_lease`（per-challenge lease）+ 若设置了 env `ASCODEX_CHIEF_LEASE_ID` 则要求一致），child binding 走按题变体；无 override 时保持原 env 路径（单题模式完全兼容）。
- contract 门 `validate_contract_for_spawn`：contract 路径解析顺序 = `ASCODEX_CONTRACT_FILE/INPUT_FILE`（单题遗留）→ `ASCODEX_CONTRACT_DIR/<challenge_id>.json` + `<challenge_id>.fingerprint-input.json`（round）。challenge 匹配、fingerprint、formal admission 全部不变。

### 4. solver_round_dispatch 工具（core/src/tools/handlers/solver_round_dispatch.rs）

- 仅 root 会话 + solver 模式注册（`spec_plan.rs`：`!session_source.is_non_root_agent() && solver_mode_enabled()`）。
- 参数：`plan_path`（绝对路径）。逐题：构造 clean-room（fork_turns=none）bohrium-solver spawn（与 spawn_agent 相同的 config/role/communication 管线，depth=1）→ 按题授权 → spawn → 按题 bind。
- 逐题失败**继续派发其余题**，回执 `{dispatched, failed, receipts[], errors[]}`。子代理 spawn 后立即并发运行（每线程独立 tokio task），派发循环串行仅是 sqlite 写开销。

### 5. 提交门按题身份（core/src/tools/handlers/solver_guard.rs）

- 先解析该线程的 `thread_cycle_bindings` 行（并发安全，每子代理一行），binding 的 campaign/challenge 成为默认身份（参数仍可显式传但必须与 binding 一致）→ 10 个并发 submit 互不依赖进程 env。
- contract 门同样走第 3 点的按题解析。
- identity pool：ledger 存在活跃条目时按 `(identity, challenge, owner, class)` 精确匹配 → round 供给脚本按题 provision N 条。

### 6. round 级 turn 门（app-server/src/request_processors/turn_processor.rs）

turn/start 的 contract 门在设置 `ASCODEX_ROUND_PLAN_FILE` 时改为校验 RoundPlan（round 契约：题集+租约+模板）；未设置时保持单题 contract 门。挑战级 contract 门仍在 spawn（每题）与 submit（每题）强制。

### 7. 并发容量

`max_concurrent_threads_per_session` 是**并发活跃子代理数**（registry 计数、execution limiter、V2 residency 三道闸都由它派生）。`.codex/config.toml` 5 → 12（10 solver + 2 余量；代码内部 +1 给 chief）。无需改 Rust。

### 8. 监督与汇总（既有组件组合）

- 怠惰检测/推送决策：`ascodex-coordination::auto_push` 逐题输入（worker report/closure），`red_team=true` 升级由首席在收尾回合按题评估；`detect_stuck` 已按 challenge 作用域化。
- 首席收尾：`wait_agent` 逐题等子代理 → 汇总报告（每题 spawn 回执、六门 submit 结果、AutoPush 决策）。
- 监控/红队/判官分析子代理：同一派发机制按题/按阶段可扩展（cycle.stage_briefs 支持多角色 brief；monitor/intel/judge/red-team 为只读角色，`enforce_solver_role_permissions_by_name` 自动收窄）。本迭代 E2E 聚焦 solver 全并行。

## E2E（真实 LLM）

`out/e2e/round_provision.py`（供给）+ `out/e2e/round_driver.py`（驱动）：
1. 生成 10 道确定性本地挑战（每题独立工作区/挑战说明/可验证解）。
2. 按 round plan provision：N lease、N pool 条目、N cycle+brief（含 chief binding）、N contract。
3. 真实 agnes 会话：首席 turn 1 一次 `solver_round_dispatch` → 10 子代理并发解题 → build_trace → 六门 dry-run submit。
4. 断言：N 条 spawn 回执、N 个子代理真实运行、N 份过门 trace、首席收尾报告。
