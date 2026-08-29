# Bohrium Monitor（只读看门狗）

只做 GET/只读查询：榜单、attempt、判词/判罚依据、身份额度、评分器健康和题目详情。每条增量都带 feed cursor、观测时间、响应 SHA-256 及证据链接，并先写入协调账本，再由 Chief 决策；代理不得直接覆盖 `STATUS.md` 或共享状态文件。

## 评分事实卡（必须分栏）

监视器不得把“页面上显示的分数”当作唯一事实。按平台报告契约分别记录：

- `raw_score`：判罚前原始分；`effective_score`：榜单计分值。
- `penalty_applied`、`penalty_basis`（对象、原因、改写分）和 `penalty`。当前依据“扣 1 分”的报告措辞，临时解释为 `penalty = -1`、`effective_score = raw_score - 1`；真实 API 字段/舍入规则未确认，禁止猜测或把历史 `-1000` 当现行规则。
- `credited_owner`：榜单显示的 user/agent 归属；同时记录 `leaderboard_scope`（全站/赛季/题目）与 `season_id`，不能用单一 `leaderboard_present` 代替。
- `bundle_revision`/bundle hash 与 `rescore_status`。重传后旧评分必须标为 stale，只有对应新 revision 的 fresh completed rescore 才能进入 confirmed。
- `anti_cheat`：加权信号及其可见性。新增三个信号的名称/权重尚未获官方 schema，记录为 unknown，不硬编码旧“8 规则”或固定公式。
- `trace_admission`：完全没有运行痕迹的轨迹不进待评队列；无 trace 的对象应为 `unknown_needs_reconcile`，不算失败或成功。

公开页面目前可核对 user/agent 归属及 `-1/100` 展示；原始分、判罚依据和 API 字段仍需登录后授权查询才能确认。匿名读取他人提交已关闭，监视器只能读取自有或明确授权的 attempt/artifact。

禁止提交、删除、注册身份或修改平台对象。Codex 没有持久 ScoreWatcher；长时间监控必须由用户明确授权，并使用可停止的一次性检查或外部调度。
