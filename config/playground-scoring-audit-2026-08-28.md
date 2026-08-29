# Playground 评分系统审计（2026-08-28）

依据：用户提供的平台报告、play.bohrium.com 公开页面只读核对、本地镜像文档与 ASCodex 实现。本审计不把未核实的 API schema 当作事实；所有本地契约都在文本中标注依据等级。

## 1. 平台变更核对结果

| 变更 | 公开页/报告证据 | 本地契约映射 | 依据等级 |
|---|---|---|---|
| 全站榜与赛季总榜统一 | 同一榜页提供 All Time / Season 1–4 筛选 | `LeaderboardScope::UnifiedOverallAndSeason`，不再用单一 `leaderboard_present` 布尔值 | 报告+页面 |
| 判罚改为扣 1 分 | Attempts 页出现多条 `-1/100 -1%` | `penalty = -1`（delta），`effective_score = raw_score - 1`，`penalty_applied=true`，`penalty_basis` 保存对象/原因/改写分 | 报告+页面；字段名为推断 |
| 原始分可见、不归还 | 报告明确 | `raw_score` 独立保存，永不因判罚被覆盖；有效分不反推原始分 | 报告 |
| 榜单显示归属 | Attempts 页显示 `丁钊翰's ...`、`wuhan's Elephant` | `credited_owner` + `leaderboard_scope` + `season_id` | 页面 |
| ARM 重传后重新评分 | 报告明确 | `bundle_revision`/hash + `rescore_status`；pending 只能 `unknown_needs_reconcile`，旧结论标 stale | 报告 |
| 反作弊加权+3 信号 | 报告明确 | `AntiCheatMode::WeightedThreeSignals`，信号名称/权重未知时保持 unknown；禁止硬编码旧“8 规则”或 `harbor_reward × trace_factor × 100` | 报告；权重为未知 |
| 无运行痕迹不入待评队列 | 报告明确 | `trace_evidence` 缺失 → `unknown_needs_reconcile`，不算失败/成功 | 报告 |
| 匿名读取他人提交关闭 | 报告明确 | oracle-probe/trace 技能删除该路径；只读自有或明确授权 attempt | 报告 |
| 判罚依据可查询 | 报告明确 | `PenaltyBasis { object, reason, rewritten_score }` 仅供本地诊断账本，禁入提交物 | 报告 |
| 题目页三区+正常链接+附件状态 | 页面 Overview/Guide/Resources/...、Resources (17)、缺失附件显式提示 | `ChallengePageEvidence` 分栏记录三区、share route、attachment status | 页面 |

## 2. 发现的关键语义风险

1. 判罚表示：公开页 `-1` 可解释为 raw=0 的 delta 结果，但未登录页看不到原始分。本地采用“扣 1 分 → delta -1”解释；若真实 API 返回的是替换后的绝对有效分，必须迁移 `reconciliation.rs`、monitor 与 verifier，禁止同时支持两套语义。
2. 反作弊权重：三个新信号的具体名称/权重/阈值未公开，任何固定公式都是臆测；本地只保留 admission 硬门（真实执行、配对工具事件、artifact provenance）作为比平台更严的前置，不冒充平台信号。
3. 旧教程过时点：`bohrium-kb/docs/dev/dev-TUTORIAL.md` 的“7 层反作弊流水线”“全局榜与 Hackathon 独立体系”已过时；`skills/deepseek-harness/` 为不可变追溯快照不改，活跃 `.agents/skills/` 已更新。
4. 匿名读取路径：oracle-probe 旧文说“找满分 attempt 读 outputs”，已改为禁止；`/docs` 返回 403，不能把旧教程当当前官方文档。

## 3. 已落地改动

- Rust `ascodex-coordination`：typed reconciliation reducer（单调 cursor、dedup、冲突 fail-closed、stale 不回滚）；`ReconciliationFacts` 含 raw/effective/penalty/basis/owner/bundle/rescore/trace/anti-cheat/anonymous-access/challenge-page。solver-guard ledger 现已原子持久化 snapshot、不可变 item、campaign event 和判罚 audit row。
- `scripts/ascodex_monitor.py`、`bohrium-kb/tools/verify_attempt.py`：新字段解析与校验；verifier live GET 必须显式 `--owned-only`；monitor、converter 和 verifier 都不会把任意非空字符串当作 present，replay 不能推断 trace evidence；anti-cheat signal 必须含非负权重和有效可见性；pending rescore / 证据不足不验证成功。
- `scripts/ascodex_platform_client.py`：只读、白名单域、GET-only、进程环境凭据、8 MiB 上限和同源重定向校验；attempt 级读取与 `challenge_attempts` 列表都必须显式 `--owned-only`，`/attempts` 还必须带 `author` 服务端过滤。
- `scripts/ascodex_reconciliation.py`：保存响应到 Rust reducer 兼容 item 的离线转换，支持 challenge-attempts 列表页展开并拒绝混入其它 challenge 或重复 attempt id，在显式 `--batch` 下输出 admin `reconcile-batch` manifest；列表响应不会自动批处理；batch event id 绑定 campaign/stream/cursor/attempt/payload，空 manifest 拒绝；可用 `--expected-owner` 校验 credited owner；unknown evidence status、challenge page、missing trace、pending rescore、不完整评分、owner 不匹配或 owner/scope 缺失会显式进入 `unknown_needs_reconcile`。
- `scripts/ascodex_reconciliation_runner.py`：把保存响应（或显式 `--owned-only` 授权的一次 GET）转为 typed manifest，校验 Monitor context 后调用本地 admin `reconcile-batch`；一个进程调用只执行一个 cycle，保存响应、manifest 和 summary，且不访问平台写接口。
- 活跃技能与角色文档、coordination/deployment 配置文档同步。
- 未修改 Harness 历史快照、外部 solver-guard 源码、Harness 运行态。

## 4. 待确认/待办

1. 登录后只读核对真实 API 字段名：penalty 是 delta 还是绝对分、rescore/bundle revision 字段、owner 字段、anti-cheat 信号形态。
2. 三个新反作弊信号获得实时 schema 后再填权重，在此之前 `platform_weighted_anticheat_unknown`。
3. 常驻周期调度、Chief 进程唤醒与真实 schema 归一化接入该 ledger；当前 persistence/reducer、只读 GET client 和单周期 runner 已落地。
4. 后续可归档 `bohrium-kb/docs/dev/dev-TUTORIAL.md` 或加日期化勘误标记，避免新代理误读为现行规则。

## 5. 结论

评分口径变更与本地镜像的主要冲突都已定位并完成第一轮适配：所有成功路径必须持有原始分/有效分/判罚/归属/bundle revision/rescore/榜范围与 trace admission 的分栏证据；缺任何一栏都进入 reconcile/unknown，而不是沿用旧的单一 reward/leaderboard 判断。









