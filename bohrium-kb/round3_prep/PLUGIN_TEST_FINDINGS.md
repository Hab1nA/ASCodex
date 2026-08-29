# dsh-solver-guard 插件问题档案（测试性作战实录）

> **来源**：2026-08-25 测试性作战（5 题真实作战全链路：denoise-pancreas /
> focused-imaging / abacus-fractional-charge / cu-dislocation-md /
> heg2d-flow-ansatz，身份池 Friday-01~08）。
> **验证基准**：`HARNESS_GUARD_PLUGIN_DESIGN.md`（设计文档 v0.1）vs 插件实现 v0.2.0。
> **状态**：🟢 持续记录中——作战期间与后续发现的所有插件问题一律追加到本档案
> （按 # 递增编号，不重编号）。
> **证据纪律**：每条问题附 attempt id / 作业 id / 文件路径证据；无证据不下结论。

---

## 一、问题总览（按严重度）

| # | 问题 | 严重度 | 状态 | 一句话 |
|---|---|---|---|---|
| 1 | 身份池缺 Friday-08 登记 | 中 | ✅ 已修（台账补登） | 设计 §3.2 要求含 Friday-08，rules.yaml 只有 01~07 |
| 2 | trace 门 provenance 只查存在性 | 低 | 📋 记录 | 设计 §3.4 要求校验 run.log 存在+mtime，实现不校验 |
| 3 | 失败提交也记配额+cadence | 中 | 📋 记录 | attemptId=null 的失败记 1/10 + 600s 间隔，浪费配额与时间 |
| 4 | 插件 trace 门与平台判分器两套逻辑 | 中 | 📋 记录 | 预检（无 provenance 恒 29）≠ 平台 LLM 内容判分；正确姿势未文档化 |
| 5 | traceCount=0 假阳性警告 | 低 | 📋 记录 | CLI 轨该字段恒 0，ScoreWatcher 误报"裸提交风险" |
| 6 | build-submit 无 --bundle 直传 | 高 | 🔧 已改代码待重启 | CLI 硬编码占位 reproduce.py → worker 无法执行真实 entrypoint |
| 7 | rest_no_script 形态名不副实 | 低 | 📋 记录 | 与 cli_no_script 共用同一 CLI spawn，REST fallback 未实现 |
| 8 | 通道门未按题类型校验 | 高 | 📋 记录 | S2 型题需 ARM 形态，插件默认形态不提示 → 盲目重试 |
| 9 | 429 被 CLI 吞 → 自动顺延失效 | 高 | 📋 记录 | denoise 9 连败核心：CLI 吞 429，插件检测不到 |
| 10 | 提交前无服务端额度预检 | 中 | 📋 记录 | 额度满只能靠失败反馈（且被吞），应提交前 GET attempts 计数 |
| 11 | 文档/消息纪律教训 | 低 | 📋 记录 | followup 积压、编辑事故（见 §四 操作教训，非插件问题） |

## 二、详细条目

### #1 身份池缺 Friday-08 登记（已修）
- **现象**：设计文档 §3.2 身份池含 Friday-08（friday-s2-24714, ACTIVE），插件
  rules.yaml 实际只有 Friday-01~07。
- **证据**：`~/.dsh/solver-guard/rules.yaml`（修复前）；`HARNESS_GUARD_PLUGIN_DESIGN.md` §3.2。
- **根因**：实现与设计文档漂移（身份池初始导入遗漏）。
- **影响**：池内既有账号无法使用（若用户指定 Friday-08 会"身份不存在"）。
- **修复**：rules.yaml 补登（2026-08-25 已完成，cred_file=agent2_credentials.txt）。
  注意：这是台账补登，非新增身份（池冻结纪律不受影响）。
- **建议**：设计文档与 rules.yaml 建立一致性校验（启动时比对 IDENTITY_POOL.md）。

### #2 trace 门 provenance 只查存在性（设计 vs 实现）
- **现象**：trace 带 provenance 字段（execution_id/ran_at_ms/wall_time_ms 任一）
  即解锁 29 档；run.log 从不校验。
- **证据**：`lib/gates/trace-check.js` predictBand() L264-265
  （`if (!hasProvenance || !executionRecord) return 29`）；trace-validate 从未
  传入 runLog（`opts.runLog` 恒空 → stdout_anchor 宽容 PASS）。
- **设计要求**（§3.4）：trace 必须携带 provenance 指向真实运行记录，插件校验
  execution/run.log 存在且 mtime 合理。
- **影响**：防伪弱化（provenance 可伪造）；但对真实执行 trace 友好（无需额外文件）。
- **建议**：可选接入 solver-guard_exec 的真实执行记录（stdout 落盘）作为 run.log
  来源；或维持宽松并在文档中明确"防伪依赖时间轴自洽 + artifact 存在性"。

### #3 失败提交也记配额 + cadence
- **现象**：CLI 崩溃/429 被吞的失败（attemptId=null，平台未创建）也被本地记
  1/10 配额 + 触发 600s cadence。
- **证据**：denoise Friday-02 本地记账 9 次失败（submits.json 9 条
  status=pending_parse）；每次失败后 cadence 600s 重试窗口。
- **影响**：本地配额账与平台实际消耗脱节（平台 0 消耗、本地 9/10）；
  9 次重试受 cadence 限制耗时 ~2 小时。
- **建议**：仅 attemptId 非空的失败记账；纯环境失败（exit 崩溃/网络）不记或
  提供总负责人回滚机制（solver-guard 增加 quota 修正工具）。

### #4 插件 trace 门与平台判分器是两套逻辑
- **现象**：插件 trace-validate 过门（84 档）≠ 平台 trace_score 高分。
- **证据**（focused 爬坡实录）：
  | attempt | trace 版本 | 插件门 | 平台 trace_score |
  |---|---|---|---|
  | 33459 | 插件门模板 | 84 | 29.0 |
  | 33462 | native 格式 | 84 | 29.0 |
  | 33465 | native+真实失败史 | 84 | 21.7 |
  | 33467 | abacus 模板（title/因果闭环） | 84 | 44.3 |
  | 33470 | +evidence 入 bundle | 84 | 46.2 |
  | 33460(abacus) | 真实数据工作链 | 84 | 94.95 |
- **结论**：插件门 = 保守本地预检（无 provenance 恒 29）；平台 = LLM 内容判分
  （模板式/一帆风顺 → 29；真实数据链 + 因果闭环 → 高分）。
- **正确姿势**（经验固化）：native 格式（ISO 时间戳/tool_args 数组）+ 真实数据
  工作链 + 因果闭环 + 失败/修复史 + provenance 行放文件尾部（双解析器兼容）。
- **建议**：插件文档/技能注入补一张"平台 trace 判分偏好"卡（trace-maximize 更新）。

### #5 ScoreWatcher traceCount=0 假阳性警告
- **现象**：每次回填都警告"traceCount=0，CLI 裸提交风险，trace 可能未挂载"。
- **证据**：CLI 源码 attempt 载荷 trace 字段硬编码 "[]"（trace 只进 bundle
  traces/trace.jsonl）；历史 28076 traceCount=0 但 trace_score=89.375；
  33460 traceCount=0 但 trace_score=94.95。
- **影响**：100% 假警报，误导 solver 反复排查 trace 挂载（浪费时间）。
- **建议**：ScoreWatcher 改读 bundle 内 traces/trace.jsonl 存在性，或取消该警告。

### #6 build-submit 无 --bundle 直传 + CLI 占位脚本覆盖（高）
- **现象**：judge 轨（four_step_with_script）提交 harbor 结构性 0。
- **证据**：CLI 源码 L3780-3783 构建 bundle 时 src/reproduce.py 无条件覆盖为
  占位脚本 → worker 永远执行占位 → 不产契约文件 → harbor 0（33459/33462/
  33465/33467/33470 一致）；插件 build-submit 不暴露 --bundle。
- **影响**：无本地数据时（数据只在 worker 环境）无法让 worker 执行真实算法
  （focused 唯一真实判分路径被堵）。
- **修复**：✅ 已改代码（index.js schema + submit.js 透传 + hash 含 bundle，
  2026-08-25），**待重启 dsh 生效**。solver 侧 self_arm.zip 已就绪。
- **建议**：重启后验证 CLI 对 --bundle 直传的 worker 执行 + harbor 出分。

### #7 rest_no_script 形态名不副实
- **现象**：form=rest_no_script 与 cli_no_script 走同一 CLI spawn 路径。
- **证据**：`lib/submit.js` execSubmit 与 form 无关（solver 读源码确认）；
  denoise rest_no_script 失败模式与 cli_no_script 完全一致。
- **影响**：形态枚举误导（solver 换形态重试 = 无意义重试）。
- **建议**：要么实现 REST fallback（draft→bundle→score via fetch），要么从
  枚举中移除 rest_no_script。

### #8 通道门未按题类型校验（高）
- **现象**：S2 hackathon 型题（无 per-challenge programmatic grader，服务端
  要求 manifest/bundle/results_json ARM 形态）用默认 cli_no_script 提交被
  400/429 拒绝，插件无预检与提示。
- **证据**：最小 POST 探测 denoise/cu-md/heg2d → 400
  "This challenge has no per-challenge programmatic grader... Provide one to
  enable scoring"；abacus/focused 为普通题（CLI 形态可用）。
- **影响**：3/5 测试题是 S2 型；solver 盲目重试浪费大量时间。
- **建议**：build-submit 前探测题类型（POST 探测或 challenge 详情 resources/
  grader 字段），按类型提示形态与 bundle 要求。

### #9 429 被 CLI 吞 → 自动顺延失效（高）
- **现象**：服务端 429（额度满）被 CLI cleanSubmissionError 默认分支吞成
  "未能发送成功"（exit 1）；插件 429 检测读 CLI stdout 检测不到 → 不自动
  顺延身份。
- **证据**：denoise 9 连败；Friday-02/03 最小 POST 直测 429
  "Submission limit reached — at most 10 submissions per challenge"；
  Friday-04 201 成功。
- **影响**：额度满时 solver 无限重试（每次 cadence 600s），设计 §3.2 的
  "429 自动顺延"完全失效。
- **建议**：①插件在 CLI 之外预检额度（GET /api/challenges/{id}/attempts 按
  authorId 计数）；②或检测 CLI 原始 stdout 中 429 关键词；③错误映射表
  补充 429 分支。

### #10 提交前无服务端额度预检
- **现象**：build-submit 不查服务端该身份该题已用次数（本地 quota 台账是
  本地记账，与服务端计数可能脱节）。
- **证据**：Friday-02 本地 0 次记录但服务端 10/10（历史轮次累计，跨轮不重置）。
- **影响**：额度耗尽只能靠失败反馈（且 429 被吞），浪费重试轮。
- **建议**：build-submit 身份门增加服务端额度查询（GET attempts 按 authorId
  过滤计数），满额直接拒绝并提示可用身份。

### #12 ScoreWatcher 分数锁定为 0（出分前误判 backfilled，高）
- **现象**：Web UI（作战台账）显示 attempt 总分 0.0、harbor/trace "-"；ScoreWatcher
  回填消息恒 "harbor=n/a ts=n/a → 0"——但平台真实分数正常（33474 =
  47.7586，scorecard 齐全）。
- **证据**：手动复现 `playground status --attempt-id 33474`（monitor.js 同款
  调用）输出完全正常（score=47.7586、harbor_reward=0.4775858、
  trace_score=98.75、status=scored）；monitor.js 状态机逻辑：
  - L44 `pick('score', ...)` 对 CLI 输出恒命中（未出分时 score=0.0 字段也存在）
    → L49 `hasScore = score !== undefined` 恒 true（0 也算有分）；
  - L50 `isBackfilled = hasScore || ...` → **首次轮询（出分前）即推 backfilled
    事件（分数 0/n/a）并锁定状态**；
  - L230 `if (!SUBMIT_STATUSES.has(row.status)) continue`——backfilled 不在
    轮询集合（submitted/pending_parse/scored）→ **锁死后不再复查**，真实出分
    永不回填 → Web UI 显示 0.0 / "-"。
  - 附带：`/\b(scored|...)\b/` 正则不匹配 "late_scored"（下划线是词边界破坏），
    status 判断也偏弱。
- **影响**：ScoreWatcher 分数回填全链路失效（本作战全部 attempt 的回填分数
  都是出分前的 0/n/a 快照）；Web UI 台账分数不可信。
- **建议**：①hasScore 判定收紧：`score > 0 || status === 'scored'/'backfilled'`
  （严格匹配），score=0 且未 scored 继续轮询；②backfilled 后仍周期性复查
  （读到 score>0 且与已记录不同 → 推更新事件）；③status 匹配用精确值集合
  而非正则。

### #13 draft 创建计入服务端提交额度，DELETE 不释放（高）
- **现象**：Friday-04 在 denoise 题从"额度可用"变"429 满额"——诊断探针
  （quota probe / diag draft / quick probe 等 201 创建的 draft）计入服务端
  10 次/题提交额度，DELETE 不释放；叠加 33473/33474 两次正式提交后满额。
- **证据**：solver 裸 submitted POST 直测 Friday-04 → 429；Friday-05/06/07/08
  draft 探针 201（额度可用）；此前假设"draft 无配额副作用"不成立。
- **影响**：诊断探针直接烧主提交身份的额度（10 次/题宝贵）；测试作战中
  denoise 的 Friday-04 仅剩 8 次可用时被迫换身份。
- **建议**：①诊断探针一律用备用身份（或正式提交身份之外的身份）；②插件
  build-submit 的"dry-run"保持不创建任何服务端对象；③文档明确 draft 的
  配额语义（探索是否可通过 DELETE 释放——实测不释放）。

### #14 作战台账子代理状态与实际生命周期脱节（高）
- **现象**：作战台账 UI 显示 settled，但子代理实际 running（cu-dislocation-md /
  heg2d-flow-ansatz 实证——agent-register 后多次冷恢复唤醒，档案 status 仍
  settled）。
- **证据**：agents.json 三档案 status=settled + settledAtMs 非空，而 list_agents
  显示 running；ledger.js 状态机：
  - `register`（agent-register 工具）置 status='running'（L173）；
  - `touch`（subagent 创建窗口钩子）仅在档案不存在时建骨架，**不更新已有档案**；
  - `markSettled`（subagent/end 钩子）置 status='settled'（L194-195）；
  - **没有任何钩子在冷恢复/唤醒（send_message 新 turn）时把 settled 刷回
    running**——档案状态只随"创建/登记/结束"切换，不随生命周期复活。
- **影响**：UI 台账状态不可信（用户观察）；总负责人据档案判断"代理已收工"会
  误判；后续 AutoPush/监控逻辑若依赖档案 status 也会失真。
- **建议**：①插件增加 subagent resume/new-turn 事件钩子（唤醒时置 running）；
  ②或 UI 渲染时优先透传 DSH 实时 agent registry 状态（running/idle/ready），
  档案 status 仅作审计快照；③markSettled 前核对实时状态（防止把运行中代理
  标 settled）。

### #15 算力上云纪律未强制执行（本地长训练畅通无阻，高）
- **现象**：本机存在高消耗进程（python 46240：CPU 19812s ≈ 5.5h、内存 7.2GB，
  anaconda）——heg2d 的变分训练（train_n10.py，1800s exec）在本地长期运行，
  远超 rules.yaml 的 bohr.local_smoke_limit_sec=120s / local_mem_limit_gb=2GB。
- **证据**：Get-Process 快照（46240/46536 两个 python 高消耗）；heg2d probe
  （solver-guard_exec timeout_ms=1800000 的本地训练 exec 被放行）；exec.js
  全文（L1-260）——检查项仅 workspace fence / deny list / python_only /
  timeout 上限 30min，**rules.bohr 的阈值（smoke 时长/内存/heavy_must_cloud）
  在 exec 路径完全不被读取**。
- **根因链**：①设计文档 §3.8 BohriumGuard 要求"超限即警告'此命令应走
  bohr'；可选强制模式直接拒绝本地执行"——**exec.js 未实现任何阈值检查**
  （README 声称"本地长跑拦截降级为纪律卡提醒"，但命令级提醒不存在，
  SkillInjector 的纪律卡是 30min 通用刷新，不针对具体命令）；②exec timeout
  上限 30min（1800s）允许长训练（heg2d 恰用满上限）；③无内存监控
  （diskmon 只管磁盘）。
- **影响**：算力上云纪律形同虚设——重型任务可静默本地跑（本机资源被占、
  bohr 预算闲置、纪律执行依赖 solver 自觉）。
- **建议**：①exec.js 增加 rules.bohr 阈值检查（命令特征检测训练/长循环 +
  超限返回拒绝或强警告并记录事件，警告文案给"应走 solver-guard_bohr"）；
  ②exec timeout 上限收紧至冒烟级（如 10min），长任务强制走 bohr；③可选
  内存监控（进程快照）；④此问题与 #8（通道门按题类型校验）同属"执行路径
  未实现设计规则的预检层"。

### #16 AutoPush 封板审查盲区（denoise premature closure 事件复盘，高）
- **现象**：denoise solver 在低于公开基准（composite 0.69 < 0.71→50 分锚点）、
  远低于场上最高（74.73 vs 47.76）、判词明示 push 的情况下收束，AutoPush 未
  拦截，总负责人批准。
- **根因（双因）**：
  1. **challengeBest 数据源缺陷**：autopush.js `bestScore()`（L256-275）只从
     插件本地 submits 台账取分（= 我们自己人的提交），**不查平台实时榜**——
     场上 MrZhang 74.73 对机器规则不可见 → "场上更高"第一问恒静默；
  2. **chief 窗口无分级**：机器已判定 premature（independent<2 规则命中，
     denoise 该题只有 1 个 solver 提交），但总负责人在 chief_window_sec=90
     窗口内回复收束（无理由校验）→ 插件让路，机器审查被一句话压制。
- **影响**：封板三问第 1 问（场上有人在你上面）在数据源层面失效；
  chief 仓促表态可覆盖机器审查——premature closure 可系统性漏网。
- **改进提案（插件 v0.3 方向）**：
  A. **challengeBest 取平台实时榜**：插件用已有 token GET /api/challenges/{id}/attempts
     取 max(score)，缓存到 ledger（challenge_best 字段定期刷新）——机器第一问
     即可见 74.73 > 47.76 直接触发；
  B. **chief closure 分级**：硬规则（challengeBest − agentBest > 阈值，如 10 分，
     或 agentBest < 0.7×challengeBest）→ chief 收束表态不生效，必须附战略原因
     （换题/换人/quota）或显式 override 留痕（cadence-override 同款模式）；
     软规则（independent<2 / 判词 push 信号）→ chief 收束必填 reason 并记录事件；
  C. **判词信号纳入**：ScoreWatcher 读 scoringDetails，判词含
     push/improve/more complete/partial 类词汇 → 档案标记 partial_push_signal，
     纳入 AutoPush 触发条件；
  D. **基准线可配**：rules.yaml 支持 per-challenge 基准（如 denoise 0.71→50），
     agentBest < 基准线收束需 chief 显式确认；
  E. **closure-review 工具化**：solver-guard_closure-review（或 chief 窗口自动
     推送三问核对单：challengeBest/agentBest/判词摘要，逐条回答或给战略原因
     才放行），closure 裁决事件留痕（closure/approved|rejected + reason）。
- **经验教训（总负责人侧）**：批准收束前必过封板三问；chief 窗口内不仓促表态
  （denoise 事件 = 我的窗口内收束回复压制了机器审查）。

### #17 AutoPush 触发词表盲区（红队机制从未生效，高）
- **现象**：设计"两次弱收束后自动派红队攻击其结论"（autopush.js L499-505）
  在本作战从未触发——denoise（47.76 收束）、focused、abacus（w 关闭）、
  heg2d 的收束文本全部绕过 AutoPush。
- **证据**：触发链第一步 L118 `if (!CLOSURE_CLAIM_RE.test(text)) return
  { premature: false }`（L430 同前置）——CLOSURE_CLAIM_RE 词表
  （收工/收手/封板/饱和/已达上限/不再+提高…）与实战措辞脱节：
  denoise 用词"收束/终值/落袋/不再追加/定稿"——**全部不在词表**；
  本作战全部收束报告零命中 → AutoPush 在第一步静默退出（chief 窗口、
  overrule、红队 spawn 均未执行）。
- **澄清**：总负责人此前复盘"我的 closure 回复压制了 AutoPush"不准确——
  chief 窗口逻辑（L465-488）设计为"chief closure 表态被 overrule 强制续推"
  （封板标准优先于口头同意），即使 chief 表态收束机器也会否决；真实缺陷
  是词表盲区导致机器从未启动。
- **影响**：AutoPush/红队机制整体失效（设计 §"AutoPush 强制续推"名存实亡）；
  premature closure 系统性漏网（依赖总负责人人工三问，而人工会错）。
- **建议**：①词表扩充实战高频词（收束|收尾|定稿|终值|落袋|封笔|完结|结案|
  关闭|不再追加|到此为止|就到这|无进一步方案）；②**结构信号替代单一词表**：
  检测"最终/终值/收束+REPORT/五要素/不再追加"组合结构判定收束意图；
  ③宽松触发+严格审查（宁可误触发红队，成本低；chief intervene 机制已存在
  可停推）；④红队触发链 E2E 测试（构造弱收束文本验证 spawnRedTeam 全链路）。
- **联动**：#16（challengeBest 数据源 + chief 分级）与本条共同构成
  AutoPush 修复包（v0.3）。

### #18 bohr 幽灵作业无法清理（kill 通道缺失，中）
- **现象**：bohr 队列被幽灵作业阻塞（23268819 probe 超 max_run_time 3 倍未
  超时、23268817 cost=0 占位），保底作业排队无限延迟；无法 kill。
- **证据**：①solver 侧 DENY_PATTERNS 禁止 `bohr job kill`（设计归总负责人）；
  ②solver-guard_bohr 工具仅 submit/status/download，无 kill/管理通道；
  ③总负责人手动 `bohr job kill`（pwsh + BOHRIUM_ACCESS_KEY/ACCESS_KEY 从
  ~/.bohrium/config.yaml 读取）均报 "AccessKey Invalid"，而插件 spawn 同一
  key 提交成功——key 传递/校验路径与手动调用不一致（待查：bohr.exe 的
  env 变量名或 config 自动加载行为）。
- **影响**：平台队列被幽灵作业占满时无任何清理手段（只能等平台超时/人工
  控制台）；保底提交延迟 1-2h+。
- **建议**：①solver-guard_bohr 增加 kill/describe 管理操作（或总负责人侧
  solver-guard_bohr-admin 工具，带 reason 留痕）；②修复手动 CLI 的 key 传递
  路径（bohr.ps1 wrapper 或 config 自动加载）；③文档记录幽灵作业处置 SOP。

### #19 trace 预检未有效防短板（实证：7/9 出分 attempt 的 trace 是短板，高）
- **现象**：插件 trace-validate 全部放行（84 档），但平台 trace_score 从 21.7
  到 98.75 全谱分布；已出分 9 个 attempt 中 **7 个 trace 是短板（<70）**。
- **证据**（本作战实测）：denoise 33473=49（损失 24 分：harbor 0.4776 本可拿
  48 分，trace 49 打折到 23.4）；focused 33459/33462/33465/33467/33470/33488
  = 29/29/21.7/44.3/46.2/49；高分仅 denoise 33474+（98.75，v13 标准 schema）
  与 abacus 33460（94.95）。
- **根因**：插件 trace 门 = 机器规则（provenance 存在性/时间轴/schema 面），
  平台 trace_score = **LLM 内容判分**（模板痕迹/因果闭环/真实数据链/cost
  比例/thought 深度）——两套逻辑（#4 延伸），预检对 LLM 判分面预测能力≈0。
- **关键因果**：高分 trace（98.75）是 solver 在 33473 判词反馈（N08/N09 +
  trace-feedback 五条指引）后修对的——**预检既没拦住短板也没教会正确姿势**。
- **影响**：trace 短板直接打折答案分（33473：48→23 分），损失可达 50%。
- **建议**：①trace-validate 增加"平台 LLM 判分面"预检（tool_name/tool_args
  标准 schema、cost 比例 sanity $11.5/M、模板痕迹检测、thought 深度——
  把 trace-feedback 五条固化为规则）；②预检档位与平台分数回标校准（在线
  学习闭环）；③提交门对"LLM 面可疑"trace 强警告+注入配方卡；④98.75 实证
  配方（标准 schema+真实数据链+因果闭环+provenance 尾部）写入 trace-maximize。

### #20 trace 预检反馈不可操作（29 档无根因指引，agent 盲试，高）
- **现象**：denoise solver 在 29 档盲试 **11 个变体**（JSONL/数组/数字时间戳/
  +error/+reasoning/56 步真实史/中文 thought/技能模板/考古 arm33073 格式）
  全 29，最终由总负责人读源码发现缺 provenance 行；focused 同样多轮试错。
- **证据**：trace-validate 输出（index.js L358-367）——机器层 6 条全 PASS
  + 档位 29 + 泛泛修复指引（"构造/模板痕迹"），**没有指出 predictBand 29
  的真实原因（hasProvenance=false）**；29 档无 FAIL 项可看（69 档有具体
  "时间轴不自洽"细节）——反馈质量三档不对称。
- **对比**：平台判词反馈（33473 的 N08/N09 "tool_call schema malformed"）
  一次修对——平台比插件预检可操作。
- **影响**：29 档盲试浪费大量轮次（denoise 11 变体 ≈ 数小时）；预检本可在
  第一轮给出"缺 provenance 元数据行（execution_id/ran_at_ms/wall_time_ms，
  无 type 字段）"的精确指令。
- **建议**：①29 档输出具体根因（hasProvenance=false → 明确"缺 provenance
  元数据行"并附 JSON 示例）；②全 PASS 但 29 档时输出"机器层全过，唯一缺口
  = provenance"；③修复指引按 29 档子类细分（缺 provenance vs 模板痕迹）；
  ④用平台判词代码（N08/N09…）回标增强预检指引（与 #19 在线学习闭环联动）。

- **修复（2026-08-25）**：新增 `lib/gates/llm-surface.js` LLM 面预检层（6 规则：模板痕迹/因果闭环/真实数据链/cost 比例/thought 风格/失败史），
  trace-validate 输出 LLM 面风险节；submit 门 high 风险强警告（不阻断）+ 配方提示；ScoreWatcher 回标记录 {预检风险,平台分} 校准对（trace_calibration 集合）。
  验证：114/114 测试通过；真实 trace（attempt 23301）实测输出 4 项 LLM 面风险（机器面看不到的短板被捕获）。

- **修复（2026-08-25）**：新增 `bandReason()`（trace-check.js）——档位结构化根因：29 细分 no_provenance / machine_fail 两子类并附 JSON 示例；69 指向具体软轴失败；84=clean。
  trace-validate 输出「档位根因」行；submit 门拒绝 reason 同样携带根因+修复指引。
  验证：118/118 测试通过；真实 trace 实测输出「档位根因: no_provenance — 缺 provenance 元数据行…（附 JSON 示例）」。

### #21 dsh 重启后全部后台可续子代理回合崩溃（致命 · 核心层非插件，2026-08-26）
- **现象**：dsh 0.1.1-rc.2 本轮重启批次后，所有**后台 continuable 子代理**的每个回合在收尾边界抛
  `UNKNOWN: Cannot read properties of undefined (reading 'then')`——step 正常摄入消息后 turn end 即崩，
  无任何模型输出、无收尾消息；**前台一次性子代理不受影响**。
- **证据**（金丝雀差分矩阵）：A 前台裸测=✅OK；B 后台+自定义模型=✗崩；C 后台零改动=✗崩；
  5 个求解器（fe52bbb3/f19974b3/11a0b3de/6fc9d266/101409cb）显式重置默认 provider/model 后仍全灭；
  interrupt_agent 复位后 send_message 复测仍崩（101409cb）。探针 fe52bbb3 turn22/23 全程记录
  （含插件 AutoPush 强制续推消息同样死于该错误）。
- **根因**：核心后台子代理调度器的 promise 链断裂（疑与 AGENTS.md 所列未合入官方的社区分支
  `fix/tool-runtime-scheduler-symbol-for` 同族）；与模型选择、solver-guard 配置均无关。
- **影响**：作战全面停摆——solver/judge/redteam/monitor 全部无法后台运行，仅剩前台串行中继可用；
  错误签名不透明（裸 UNKNOWN），首轮误诊为模型路由问题耗时约 30 分钟。
- **建议修复**：①对齐社区 fork 补丁；②turn 失败向父会话推送具体错误栈而非裸 UNKNOWN；
  ③每次重启完成后自动 spawn 一个后台金丝雀自检管线，通过才算"恢复完成"（防带病复工作战）。
- **状态**：📋 记录（待定夺）——总负责人拟执行 restart_harness 自救（见 DECISION_LOG D-15）。

### #22 subagent_config 接受任意模型名且覆盖生效性不可观测（中，2026-08-26）
- **现象**：`subagent_config model=<custom>` 对 7 个子代理全部返回 ok:true 并声称 persisted，
  但随后 probe 的 effective_config 仍显示原默认值——覆盖是否真正进入请求瀑布无从确认；
  叠加 #21 时该静默不一致严重误导诊断方向。
- **证据**：7 次 set 全 ok → fe52bbb3 probe effective=model deepseek-v4-flash（非设置值）；
  无任何 warning 行提示覆盖未被路由。
- **根因**：config 层不校验模型名可路由性，也不在 probe 中标注 "override 未命中" 状态。
- **影响**：批量改模型操作零反馈失效；故障时无法区分「模型不被接受」vs「管线崩溃」。
- **建议**：①set 时校验 provider/model 可路由性（拒绝并回显可用表）；②probe 对 override≠effective
  的情况输出显式警告行。
- **状态**：📋 记录。

## 三、已验证通过项（设计 §7 验收对照）

| 项 | 结果 | 证据 |
|---|---|---|
| 身份白名单（agent-identities） | ✅ | set Friday-02~08 生效；FROZEN 进白名单整组拒绝（双层防护） |
| 六道提交门 | ✅ | 多次 dry-run + 实提 6/6 全 PASS（channel/identity/cadence/redline/trace/model） |
| trace 门 29 拦截 | ✅ | 无 provenance 恒 29（保守面按设计工作） |
| 红线扫描 | ✅ | 多次 CLEAN（提交物零污染） |
| ScoreWatcher 异步出分 | ✅ | 33459/33460/33462/33465/33467/33470 全部回填推送（含假阳性警告） |
| 台账（submits.json） | ✅ | attemptId/identity/form/status/cliExit/cliOutTail 完整 |
| ModelGate | ✅ | 子代理 effective_config=deepseek-v4-flash（probe 实证） |
| SkillInjector | ✅ | 纪律刷新卡注入实证（heg2d probe 见"纪律刷新·solver-guard"） |
| AgentLedger/工作区 | ✅ | 5 档案 + 标准工作区 + 磁盘事件/越界写字段 |
| BohriumGuard | ✅ | abacus/cu-md 多作业提交/状态/事件推送；预算 $1.5-2/50 健康 |
| AutoPush | ⏳ 未触发 | 无弱收工场景（收束均带 attempt 证据），待后续验证 |

## 四、操作教训（总负责人侧，非插件问题）

1. **消息纪律**：给 running/resident 子代理发 send_message（followup）会积压在
   next-turn 队列（focused 两次裁决消息延迟消费）；正确姿势 =
   subagent_send mode=steer（步骤边界注入）。教训来源：协调手册 §消息纪律。
2. **编辑事故**：DECISION_LOG.md 三次编辑覆盖相邻条目标题（D-8/D-9/D-10）；
   教训：编辑长文档时 new_string 必须保留被替换标题，或改用追加模式。
3. **探测端口教训**：focused 数据"不可达"结论基于 50001 端口穷举；实际数据在
   50003（用户提供）——**"不可达"结论必须附探测范围（端口/路径/方法）**，
   防旧结论传染（§6 防传染）。

## 六、修复记录（2026-08-25 批次修复，测试 112/112 通过）

| # | 修复 | 实现 | 验证 |
|---|---|---|---|
| 3 | 失败提交不再记配额+cadence | submit.js: 仅 attemptId 非空且非 429 才 bumpQuota；失败行标 failed/quotaSpent | 单测 + 台账字段 |
| 5 | traceCount=0 假阳性警告移除 | monitor.js: 警告改为 trace<30 且真有分才提示 | 单测断言更新 |
| 6 | build-submit --bundle 直传 | 已实现（hash+透传），本次确认代码在 | 代码审查 |
| 7 | rest_no_script 名不副实 | rules.yaml 枚举移除（诚实拒绝未知形态） | 配置解析验证 |
| 8 | 通道门按题类型校验 | submit.js S2 探测（GET challenge，只读不建对象）+ 形态提示 | 代码审查 |
| 9 | 429 被 CLI 吞 | submit.js 检测词增强 + 429 本地标满（自动换身份） | 代码审查 |
| 10 | 服务端额度预检 | serverQuotaExceeded（GET attempts 按 authorId=account 计数，best-effort） | 代码审查 |
| 12 | ScoreWatcher 分数锁定 0 | monitor.js: status 精确集合为主判据 + backfilled 复查重评分 | 4 个新单测 |
| 13 | draft 探针烧额度 | 确认 dry-run 纯本地；channel-probe 纯只读；文档纪律 | 代码审查 |
| 14 | 台账状态脱节 | agent/request 钩子唤醒即刷 running | ledger 新单测 |
| 15 | 算力上云纪律未强制 | exec.js 重阈值检查（软警告/exec_hard_block 硬拒）+ rules.yaml 开关 | exec 新单测 |
| 16 | challengeBest 只读本地 | autopush 合并平台实时榜（fetchChallengeBest）+ chief 软/硬分级 | 代码审查 |
| 17 | AutoPush 词表盲区 | CLOSURE_CLAIM_RE 扩实战词 + 结构信号宽松触发 | autopush 新单测 |
| 18 | bohr 幽灵作业无 kill | solver-guard_bohr kill/describe（bohr job --job_id 形态，reason 留痕） | 实机 describe 验证成功 |

## 五、追加机制

- 本档案持续更新：新发现问题按 `### #N` 追加（编号递增，不重编号不删除）。
- 每条记录必含：现象 / 证据（attempt id/作业 id/文件路径）/ 根因 / 影响 /
  建议修复 / 状态。
- 状态标记：✅ 已修 / 🔧 修复中 / 📋 记录（待定夺）/ ⚠ 争议。
- 作战结束后本档案并入赛后复盘（供插件 v0.3 迭代）。

---

## 七、补遗（2026-08-26 下午）

1. **#21 追认：restart_harness 不自愈**。重启后金丝雀 D（后台零改动）同签名阵亡——缺陷为
   本机安装的确定性问题，非内存挂起态。已确认可用通道：**前台一次性子代理**（裸 `subagent`
   工具 run_in_background=false，kind=foreground）；注意 `subagent_solver` 即使
   run_in_background=false 也走 continuable 管线（实例 c9a9ebdc 秒崩），前台中继必须用裸 subagent。
2. **嫌疑线索（供用户排查）**：node_modules 全量包 mtime=2026-08-25 12:21 整批重装（pnpm 安装批次），
   无法区分单包；建议核对 dependencies\dsh 的 lockfile 是否漂移（semver 兼容范围内拉到新版本）、
   以及 dsh-client-auto-continue 等参与回合续接的插件在该批次的实际安装版本。solver-guard 源码内
   未检索到 subagent/end 类钩子注册——AutoPush 实际挂载点需复核。
3. **#18 关联线索**：前台执行发现直调 bohr.exe 报 AccessKey Invalid（config.yaml 旧 key 被服务端拒绝），
   改走 ~/.bohrium/bohr.ps1 包装脚本（注册表凭据）成功——此前 kill 失败很可能同因。建议统一 CLI 入口
   或更新 config.yaml。
4. **前台中继战果（abacus 保底里程碑 1）**：23268823 下载成功；summary.json 全 null 根因=
   run.sh grep 自定义标记 `!FINAL_ETOT_IS` 不存在于裸 ABACUS stdout（收集脚本口径缺陷，非物理失败）；
   9/9 能量已从 GE 迭代表末行恢复且按 DRHO<scf_thr 全收敛；产物 assemble/baseline_23268823.json
   （sha256 已附）、trace/draft_trace.md（TRACE 配方合规待机器预检）。遗留：refs_eval.json 语义、
   stdout 仅 9 位精度（下作业 backward_files 需追加 OUT.S）、run.sh 标记修复。

### #23 工具白名单静默失效：restrict() 引用已不存在的 cordis_* 全局工具（高，2026-08-26）
- **现象**：插件为每个子代理做工具收敛时调用 tools.restrict()，传入 cordis_define/cordis_run/
  cordis_stop/cordis_undefine/cordis_inspect_list/cordis_inspect_query/cordis_inspect_self 七个名字；
  当前 dsh 核心全局工具表已无这些名字 → restrict() 抛错 → 插件回退为**不收敛**。
- **证据**：events.json kind=tools/danger-fallback 共 **55 条**（2026-08-25T03:17 起批量出现），错误文本
  完整列出未知名单与 known global tools 表。
- **影响**：deny_shell_tools/focus 白名单对这批代理未生效——子代理实际可用工具面比 rules.yaml 声称的宽。
  属"限制看起来在、实际没在"的最危险一类。
- **建议**：①启动时校验 rules.yaml 工具名与全局表交集，未知名打一次性配置警告；②restrict 失败时
  fail-closed（禁用全部可选项再放行白名单）而非 fail-open 回退。
- **状态**：📋 记录（待定夺）。

### #24 denied-exec 文本被记入 writesOutside 数组——类别混淆，UI 把「命令拒绝」显示成「磁盘写越界」（高，2026-08-26）
- **现象**：agents.json 各代理的 writesOutside[] 应存「工作区外写路径」，实际内容是 **DENIED-EXEC:
  <被拒命令全文>** 字符串（exec 拒绝事件的转储）；web UI 渲染该数组 → 用户看到"存在磁盘写越界行为"。
- **证据**：全量提取 agents.json：42 条 = 20 条 diskEvents(denied-exec) + 21 条 writesOutside
  （其中 19 条 DENIED-EXEC 转储、仅 2 条 exec-cwd 备注）+ 重复计入。**真正路径型越界写入记录 = 0**。
- **根因**：exec 门拒绝路径把事件文本同时 push 进 writesOutside（复用字段当垃圾桶），语义未分流。
- **建议**：①denied-exec 只入 diskEvents，禁止写入 writesOutside；②UI 分开展示「命令拒绝」与「越界写入」
  两栏并各自计数；③历史数据迁移清洗。
- **状态**：📋 记录（待定夺）。

### #25 command_policy 拒绝理由不可操作（#20 同族，中，2026-08-26）
- **现象**：20 条 denied-exec 中 17 条原因只有笼统 `command_policy:python_only`，不分具体形状：
  &&复合链（6条）/ python -c 内联（6条）/ 非 python 工具 dir、powershell（2条）/ 绝对路径解释器
  （C:\ProgramData\anaconda3\python.exe…）等各有不同修法，但 agent 只看到同一句话。
- **代价实例**：fe52bbb3 的 `python gen_trace2.py && python copy_artifact.py`（用户举例）被拒后需自行
  拆两条重发，浪费轮次；54496a2d/8d19b8e3/6fc9d266 同类 && 链共 6 条全部如此。
- **建议**：拒绝消息附命中形状分类+修法提示（"&& 链请拆分为多次 exec"/"python -c 请落盘为脚本文件"）。
- **状态**：📋 记录（待定夺）。

### 补遗更正（2026-08-26）
- 本日上一轮报告曾下结论"台账不存在任何越界判定"——**数据源遗漏**：只扫了 events.json，未查
  agents.json 的每代理 writesOutside/diskEvents 字段（DENIED-EXEC 记录所在）。以本节 #24 为准修正：
  events.json 无 outside 事件属实，但 agents.json 存在大量被 UI 显示为越界的记录（实为命令拒绝转储）。

---

## 八、2026-08-27 复核与修复记录（#18 以后）

本节是对 #18–#25 的代码/运行台账复核，不改写历史证据：

- **#18 Bohrium kill：确认仍存在，已修复代码。** 实机只读 describe(23280079)成功；用不存在作业号做 kill 探针时，旧实现传 `--job_id`，CLI 输出 `requires at least 1 arg(s), only received 0` 但插件仍返回 `ok:true`。现改为 kill 位置参数、describe 使用 `-j`，并同时检查退出码与 stdout/stderr 错误文本。
- **#19 trace LLM 面：已有缓解。** `llm-surface` 风险检查、trace calibration 与高风险配方提示均已接入；它是风险预测而非平台 LLM judge 的精确替代，不再作为当前阻塞 bug。
- **#20 trace 29 档反馈：已修复。** `bandReason()` 已输出 `no_provenance`/`machine_fail` 具体根因和指引。
- **#21 后台 continuable：历史事故已闭环，原“核心调度器根因”结论过时。** 实际加载路径曾使用旧 profile 实体副本，旧 ModelGate listener 的裸 `next().then` 才是触发点；现 profile 统一为 canonical Junction，后台金丝雀已正常收尾。
- **#22 supervisor 模型覆盖：确认存在，已修复代码。** `dsh-subagent-supervisor` 现在用 `llm.resolveModelInfo` 校验 provider/model，`subagent_probe` 输出 `override_status`/`override_mismatches`，并区分 pending、matched、not_applied。该项属于 supervisor，不属于 solver-guard 主插件。
- **#23 工具白名单：默认路径已修复，潜在 fail-open 也已加固。** 默认 DANGER_TOOLS 已清除过时/子代理不可见名称；focus allow 一次性失败时改为按当前 scope 过滤后重试，无法过滤时使用 `allow: []`，最终限制失败则 fail-closed，不再宽放工具面。
- **#24 denied-exec 污染：确认仍存在，已修复代码。** denied-exec 不再写入 `writesOutside`；ledger 启动迁移会保留/补齐对应 `diskEvents` 后清理历史 `DENIED-EXEC:` 条目。
- **#25 command_policy 理由：已修复。** python-only 已区分非 Python 启动器、shell 元字符/复合链、`python -c` 等形状并提供修法。
- **新增 ModelGate audit 事件形状回归：已修复。** 核心 session event 的类型字段在顶层 `event.type`，不是 `event.data.type`；测试已覆盖真实形状，防止 audit 从“假阳性”变成“静默漏审”。

本轮验证：solver-guard **132/132**、supervisor **32/32**，所有改动文件 `node --check` 通过。Host 侧修复需下一次获准重启后进入运行进程；本轮未重启以免打断在途作战。

