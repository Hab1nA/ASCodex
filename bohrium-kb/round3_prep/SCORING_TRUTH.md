# 真实分提交方式（S4 Round-4 实证固化）

> **2026-08-28 平台口径变更覆盖声明**：全站榜/赛季榜已统一；判罚从 -1000 改为扣 1 分且保留原始分；榜单显示归属；ARM 重传后必须 fresh rescore；反作弊改为加权三信号。本文档以下内容保留为历史实证记录，但固定公式、-1000 翻账和单一 leaderboard 判断不再作为现行契约。现行优先以 `config/playground-scoring-audit-2026-08-28.md` 与活跃 Skill 为准。

> 本文档由 S4 Round-4 实战实证总结（2026-08-22），attempt id 证据链完整。
> **核心一句话：真实排行榜只认 harbor 轨（CLI/手动链）分数；四步链 bundle/judge 轨出分但不收录。**

## 0. 真实分判定标准

- **唯一权威**：`http://nwjs1473070.bohrium.tech:50001/competition-leaderboard/data.json`
  （官方匿名实时榜，按 operator 汇总，77 参赛者 × 10 题矩阵；快照每分钟刷新）
- Playground API 上的 score 可能包含"旧评分逻辑被 agent 触发"的异常高分——**以排行榜为准**。
- 判定方法：提交后拉 data.json，看自己 operator 名下该题分数是否更新。

## 1. 有效提交方式（harbor 轨——唯一进真实榜）

### 方式 A：playground CLI（推荐；作战中由插件唯一入口执行）

> ⚠️ **作战执行约定（2026-08-25）**：实际提交一律走插件 `solver-guard_build-submit`（六道门禁 + 执行一体，token 由插件持有注入，身份按主代理白名单选择）；下面等价 CLI 命令仅保留作离线核对/平台侧理解。

```powershell
$tok = ([regex]::Match((Get-Content "$env:USERPROFILE\.dsh\<凭据文件>" -Raw), 'api_token\s*=\s*(\S+)')).Groups[1].Value
$env:PLAYGROUND_TOKEN = $tok
playground submit --challenge-id <ID> --outputs <outputs> --trace <trace> --model "DeepSeek V4 Flash" --harness "DeepSeek Harness"
```
- CLI 自动打包 ARM bundle（submission.json/logs/results + native_trace/trace.jsonl）→ worker /uploads 挂载 → harbor 判分。
- **注意**：CLI 直接提交若 trace 未挂载（traceCount=0）→ 判词恒 29/0（需确认 trace 被识别；R3/R4 中 CLI trace 挂载稳定性因题/版本而异，T2 29383 CLI 成功、T4 CLI 失败）。

### 方式 B：REST 手动链（无 script 字段的 draft 路径）
```
POST /api/challenges/{id}/attempts   （multipart：data 含 method/model/harness/type=agent/status=draft/outcome + trace=json.dumps(step_list) 列表序列化字符串；不附 script 文件）
→ POST /api/attempts/{id}/bundle     （files={"bundle": (bundle.zip, bytes, "application/zip")}，响应 bundleAvailable=true）
→ POST /api/attempts/{id}/submit     （header-only → 200）
→ POST /api/attempts/{id}/score      （409 "already grading" = 正常）
```
- **关键：draft 不附 script 字段**（附了 script 会切到 bundle/judge 轨 → 分数不收录）。
- 分数 = `harbor_reward × trace_factor × 100`（source=harbor_worker）。
- trace_factor：**trace_score ≥70 → 1.0 全额；<70 按比例（factor = ts/100，如 69→0.69、29→0.29）**。
  （⚠️ 2026-08-23 修正：此前按 R3 经验写"≥80"，R4 实证 32511 ts=77.35 / 32642 ts=75.425 均 factor=1.0 全额，门槛实为 **70**；69 与 70 一分之差即打 0.69 折——32657/32732/32654 等 6 样本实证。）

### 有效实证（attempt id）
| 题 | attempt | 身份 | harbor | trace | 真实分 |
|---|---|---|---|---|---|
| open-EFT multi | 31046 | Friday-03 | 1.0 | 98.125 | **100** |
| cnvkit | 29618/29533/29600 | Friday-02 | 1.0 | 84.7~95.45 | **100** |
| tetra | 30799 | Friday-02 | 0.6 | 95.45 | **60**（全场上限） |
| pancreas | 30914 | Friday-03 | 0.4771 | 87.75 | **47.71**（场上最高 73.75） |
| deep-bsde | （Jarvis 30037） | — | 0.84 | — | **84** |

## 2. 无效提交方式（bundle/judge 轨——出分但不收录）

### 四步链（带 script 字段）
```
POST attempts（multipart：data 含 trace=json.dumps(step_list) + files={"script": (src/reproduce.py, 文件对象)} ← 就是这步切轨）
→ POST /bundle → POST submit（请求体必须带 trace 否则 400）→ POST /score
```
- 判分：Tier-2 LLM bundle 判官 / ARM fallback——`六维 mean×100 + reasoning_bonus(5)` 或 `0.5×trace_score×avg(exec,pack,tq)` 等。
- **ARM 轨公式实证（usct 5 点拟合，误差 ≤0.14%，来源 JUDGE_FEEDBACK_USCT_R4 已并入）**：
  `score ≈ 0.5 × trace_score × (exec + pack + tq) / 3`——30695: 0.5×79.35×1.0=39.68≈39.62 ✓；
  30800: 0.5×33.4×0.5567=9.29 ✓；升级前 29667: 0.5×31.4×1.0=15.71 ✓。
  → **ARM 轨天花板 ≈ 0.5×100×1.0 = 50 分**（无 grader 题即使 trace 满分也到不了 60+）。
- **pack 六要素规则**（ARM 轨次杠杆，白拿 1-2 分）：`pack = completeness/6`——
  arm_manifest + **根级 README.md** + Dockerfile + requirements.txt + src/reproduce.py +
  results（execution/results/* 或 results/*）。缺 README=5/6，缺 README+results=4/6。
- **全部被真实排行榜过滤**（无论分数高低、新旧提交）。

### 无效实证（attempt id）
| 题 | attempt | 出分 | 榜上 |
|---|---|---|---|
| tetra | 30928/30948/30986/30990/30999/31002 | 98.57×6 | 60（仅 harbor） |
| deep-bsde | 31131/31268/31356 | 94.29/96.43/96.43 | 0（Jarvis harbor 84 才是真实） |
| jellium | 31211 等 | 15.71 | 0（全场 0） |
| usct | 30800 等 | 9.29 | 0（全场 0，含盘古 39.62） |
| pancreas | 31403/29642 | 100 | 47.71（judge 轨无效） |
| Lean | 31373 | 23.57 | 0（GLM 46.43 亦 0） |

## 3. 判分公式速查

| 轨 | 触发 | 公式 | 进真实榜 |
|---|---|---|---|
| harbor 轨 | CLI / REST 手动链（无 script） | harbor_reward × trace_factor × 100 | ✅ |
| bundle 轨（Tier-2 LLM judge） | 四步链（带 script） | 六维 mean×100 + bonus(5)（或 0.5×ts×avg 等题而异） | ❌ 过滤 |
| 平台阻塞 | per-challenge grader 未注册（grader_name=null） | 无（唯一路径=平台恢复注册） | ❌ |

## 4. 配套纪律（违反即翻账/否决/无效）

1. **N16 burst 惩罚**：同身份短窗高频提交触发 N16_DUPLICATE_OR_BURST_SUBMISSION（-15 且整体否决为 0；Friday-01 -1000 翻账同源）。**间隔 ≥10 分钟 + 每次实质差异**（完全相同内容 10 分钟内多次提交 = 评委明示的作弊红线；用户确认不必过慢提交）。
2. **trace 机器层 6 条**（/api/protocol 公开）：typed_step_type / tool_call_pairing（1:1）/ timestamp_window（∈ execution.ran_at ± wall_time_s）/ artifact_existence（artifact_path 指向 bundle 真实文件且 mtime ∈ run window）/ cost_floor（≥0.01）/ stdout_anchor（≥1 步 body 子串 ∈ execution/run.log）。
3. **trace 真实性**：合成/模板化 trace（统一 45s 步进、假 id、模板语言）一律 29 档（blocked）。**必须真实执行导出**（真实 stdout 进 body、真实时间戳/成本、1:1 配对、≥1 body 是 run.log 子串）。
4. **污染红线**：提交物零分数/零 attempt id/零判官结论/零他人做法；提交前 banned 扫描全 CLEAN。
5. **身份配额**：每身份每题 10 次；429/满额顺延池内下一身份（Friday-01~08）；禁新增。
6. **模型声明**：`--model "DeepSeek V4 Flash" --harness "DeepSeek Harness"`。
7. **判分慢**：部分题评分器评估 >30 分钟（jellium 曾修复中）——勿误判为失败。

## 5. 常见坑清单（实证）

| 坑 | 后果 | 修复 |
|---|---|---|
| CLI 提交 trace 未挂载 | traceCount=0 → 判词 29/0 | 确认 trace 参数/格式（json.dumps 列表） |
| run.log 是 CLI 固定文案 | stdout_anchor 必挂 → trace 29 | run.log = 真实执行日志落盘 |
| 模板时间戳照抄 | timestamp_window 必挂 | 时间戳 ∈ ran_at ± wall_time，真实会话时间 |
| 损坏 characterization.json | 整包否决（13.57 事故） | 提交前 JSON 语法校验 |
| skills/ 无 manifest.json | 400 unsupported schema | schema_version="playground-skill-evidence/v1" 或删掉 skills/ |
| native_trace 多个 | 400 | bundle 内恰好一个 native_trace/trace.jsonl |
| zip 时间戳早于 1980 | 挂载失败 | 时间戳 ≥1980 |
| deviations.target 与 expected_outputs.name 错位 | oc/rf 双零（他队实证） | target 名与 expected_outputs 命名空间对齐 |
| 同内容 10 分钟内重交 | N16/作弊判定 | 实质差异 + ≥10 分钟 |

## 6. 平台阻塞清单（非提交物可解）

| 题 | 阻塞 | 唯一路径 |
|---|---|---|
| Lean（paired-block） | per-challenge grader 未注册（grader_name=null、"No per-challenge grader registered"） | 平台恢复注册后 CLI/harbor 轨零改动重交全绿证明 |
| usct | harbor 数值轨未接通（全场 0） | 平台接通数值 verifier |
| 无 grader 题（usct/Lean 类） | **result_fidelity 结构性不可达**：grader_name=null 时 RF/cov 非计分轴（新判分器对新 attempt 不输出 cov/fid 键，69/69 无 RF>0 实证）——**不要为 characterization deviations 花 quota** | 唯一路径 = 平台注册 hidden verifier |
| jellium | 能量评估路径未给分（全场 0） | 判官能量路径恢复 |

## 6b. 平台故障模式与判分器状态矩阵（S4 R4 实证，2026-08-22）

> 来源：SCORING_INCIDENT_REPORT_R4（已并入本节后删除原文档）。提分困难 ≠ 提交物问题——先对照此表定位平台侧状态。

**7 类故障模式（按影响排序）**：
1. **基础设施事故 + 批量重评风暴**：attempt 置 gradable=false/pending=true（"preserved for re-scoring"横幅）；随后批量重评可推翻已落袋分（deep-bsde 31526 ts 97→69、95→84 回滚实证）。→ 落袋分数不耐久，重评批后需复查。
2. **worker /uploads 队列停摆**：卡 draft 数小时+（updatedAt 静止），事故期创建的旧 draft **永不恢复**——一律新 attempt 重交。
3. **间歇性 harbor 窗口**（usct 特有）：07:15→14:56-15:31→17:33-18:02 窗口模式，每次只处理单条/小批，约 2-3h 周期（可预测下一次）。→ 窗口内排队提交可能被处理。
4. **逐题判分器状态不一致**：同账号同形态在不同题行为不同（deep-bsde CLI ✅ harbor / abacus CLI 恒 0 / jellium 能量轨未恢复）。→ **每题开打前先探测**（1 发低成本探针看 harbor_replay_executed）。
5. **harness 准入差异**：部分题（abacus）只有官方 harness 枚举名（Claude Code/codex/dsh-agent）触发 harbor；"DeepSeek Harness" 无 replay。→ 非 harness 名问题，是题级判分器注册差异。
6. **trace 判分档位化**：29=构造/模板、69=时间轴伪造（duration Σ ≪ 窗口）、84+=真实执行导出——只有真实执行记录能到 84+。
7. **judge/bundle 轨分数被官方榜过滤**：四步链（带 script）96.43-100 全部不收录。

**延迟回填认知**：harbor 回填 30-50 分钟是机制非故障（32732：18:43 提交→19:15 回填；33052 同）。**提交后 5-15 分钟查无 harbor ≠ 失败**，等 30-50 分钟再判生死。

**判官字段 redacted 阻碍复刻**：他人零附件高分 attempt 的传递字段被 answerRedacted=true 隐藏，无法逆向"官方 harness 发布路径"——不要浪费时间逆向，直接走自己已验证的 CLI 形态。

## 7. 提交前检查单（最终门）

> ⚠️ 2026-08-25 起：间隔/身份/banned/trace/通道由插件 `solver-guard_build-submit` 六道门自动强制；以下为理解性清单。

1. 通道确认：harbor 轨（CLI 或 REST 手动链**无 script**）—— 插件自动（channel 门）
2. 间隔确认：距上一发 ≥10 分钟（查平台 createdAt；同内容重交禁止）—— 插件自动（cadence 门）
3. trace：真实执行导出 + 机器层 6 条自查 + **≥70 分**（全额门槛，69 即打折）—— 插件自动（trace 门）
4. banned 扫描 CLEAN —— 插件自动（redline 门）
5. 提交后：GET /api/attempts/{id} 核实归属 + 拉排行榜 data.json 确认收录 —— ScoreWatcher 自动回填并推送主代理/子代理
6. 配额：插件自动记账（quota 台账），`solver-guard_status` 查询余量；身份按主代理白名单（agent-identities）选择

---
*实证来源：S4 Round-4 Friday 团队全量 attempt 记录 + 官方排行榜 data.json（11:27-11:29Z 快照）+ 判官/红队机制分析。*
