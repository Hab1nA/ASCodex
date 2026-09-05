# 真实分机制与提交范式

> **2026-08-28 平台口径变更覆盖声明**：全站榜/赛季榜已统一；判罚从 -1000 改为扣 1 分且保留原始分；榜单显示归属；ARM 重传后必须 fresh rescore；反作弊改为加权三信号。本文档的固定公式与单一 leaderboard 判断不替代现行契约。现行优先以 `config/playground-scoring-audit-2026-08-28.md` 与活跃 Skill 为准。

> **核心一句话：真实排行榜只认 harbor 轨分数；bundle/judge 轨出分但不收录。通道不是问题，数值质量 + trace≥70 + 新 attempt 才是。**

## 0. 真实分判定标准

- **唯一权威**：官方匿名实时榜 `data.json`（按 operator 汇总的参赛者×题目矩阵，快照定期刷新）。
- Playground API 上的 score 可能包含"旧评分逻辑被 agent 触发"的异常高分——**以排行榜为准**。
- 判定方法：提交后拉 data.json，看自己 operator 名下该题分数是否更新。

## 1. 有效提交方式（harbor 轨——唯一进真实榜）

### 方式 A：playground CLI（推荐；作战中由插件唯一入口执行）

> ⚠️ **作战执行约定**：实际提交一律走插件 `solver-guard_build-submit`（六道门禁 + 执行一体，token 由插件持有注入，身份按主代理白名单选择）；下面等价 CLI 命令仅保留作离线核对/平台侧理解。

```powershell
$tok = ([regex]::Match((Get-Content "$env:USERPROFILE\.dsh\<凭据文件>" -Raw), 'api_token\s*=\s*(\S+)')).Groups[1].Value
$env:PLAYGROUND_TOKEN = $tok
playground submit --challenge-id <ID> --outputs <outputs> --trace <trace> --model "DeepSeek V4 Flash" --harness "DeepSeek Harness"
```
- CLI 自动打包 ARM bundle（submission.json/logs/results + native_trace/trace.jsonl）→ worker /uploads 挂载 → harbor 判分。
- **注意**：CLI 直接提交若 trace 未挂载（traceCount=0）→ 判词恒 29/0（需确认 trace 被识别；trace 挂载稳定性因题/版本而异）。

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

## 2. 无效提交方式（bundle/judge 轨——出分但不收录）

### 四步链（带 script 字段）
```
POST attempts（multipart：data 含 trace=json.dumps(step_list) + files={"script": (src/reproduce.py, 文件对象)} ← 就是这步切轨）
→ POST /bundle → POST submit（请求体必须带 trace 否则 400）→ POST /score
```
- 判分：Tier-2 LLM bundle 判官 / ARM fallback——`六维 mean×100 + reasoning_bonus(5)` 或 ARM 轨公式等。
- **ARM 轨公式**：`score ≈ 0.5 × trace_score × (exec + pack + tq) / 3`
  → **ARM 轨天花板 ≈ 0.5×100×1.0 = 50 分**（无 grader 题即使 trace 满分也到不了 60+）。
- **pack 六要素规则**（ARM 轨次杠杆，白拿 1-2 分）：`pack = completeness/6`——
  arm_manifest + **根级 README.md** + Dockerfile + requirements.txt + src/reproduce.py +
  results（execution/results/* 或 results/*）。缺 README=5/6，缺 README+results=4/6。
- **全部被真实排行榜过滤**（无论分数高低、新旧提交）。

## 3. 判分公式速查

| 轨 | 触发 | 公式 | 进真实榜 |
|---|---|---|---|
| harbor 轨 | CLI / REST 手动链（无 script） | harbor_reward × trace_factor × 100 | ✅ |
| bundle 轨（Tier-2 LLM judge） | 四步链（带 script） | 六维 mean×100 + bonus(5)（或 0.5×ts×avg 等题而异） | ❌ 过滤 |
| 平台阻塞 | per-challenge grader 未注册（grader_name=null） | 无（唯一路径=平台恢复注册） | ❌ |

## 4. 提交三要素与判分字段

**提交三要素（缺一不可）**：
1. **新 attempt**——事故期创建的旧 draft 卡死不会恢复，一律作废重交
2. **真实 trace ≥ 70**——<70 打折；合成/模板 trace 直接 29 档
3. **当前最佳数值**——harbor_reward 只认 outputs 数值质量，通道只是运输

| 字段 | 含义 |
|---|---|
| `harbor_reward` | 0~1，outputs 数值质量的映射 |
| `trace_score` | 0~100，trace 真实性/完整性 |
| `trace_factor` | **≥70 → 1.0；<70 → ts/100**（69 → 0.69，掉 31%） |
| `score` | harbor_reward × trace_factor × 100 |
| `harbor_replay_executed` | =1 表示 harbor 判分器运行过 |

**scorecard 六维（executability/packaging/oc/rf/tq/env）与 harbor 分无关**：六维 0 ≠ 没分；六维满分 ≠ 有分（harbor 是独立判分通道）。

**harness 名不影响 harbor 判分**：官方文档明示任何 agent（OpenClaw / Claude Code / Custom）都能接入，framework 只是展示标签；全场各 harness 名（codex/Claude Code/cursor-agent/自定义名）都有 harbor 出分记录。**最简形态（无附件直接提交）也能出 harbor 分**——不需要 bundle ready、不需要官方 harness、不需要特殊布局。

## 5. 配套纪律（违反即翻账/否决/无效）

1. **N16 burst 惩罚**：同身份短窗高频提交触发 N16_DUPLICATE_OR_BURST_SUBMISSION（-15 且整体否决为 0；旧版曾 -1000 翻账）。**间隔 ≥10 分钟 + 每次实质差异**（完全相同内容 10 分钟内多次提交 = 评委明示的作弊红线）。
2. **trace 机器层 6 条**（/api/protocol 公开）：typed_step_type / tool_call_pairing（1:1）/ timestamp_window（∈ execution.ran_at ± wall_time_s）/ artifact_existence（artifact_path 指向 bundle 真实文件且 mtime ∈ run window）/ cost_floor（≥0.01）/ stdout_anchor（≥1 步 body 子串 ∈ execution/run.log）。
3. **trace 真实性**：合成/模板化 trace（统一 45s 步进、假 id、模板语言）一律 29 档（blocked）。**必须真实执行导出**（真实 stdout 进 body、真实时间戳/成本、1:1 配对、≥1 body 是 run.log 子串）。
4. **污染红线**：提交物零分数/零 attempt id/零判官结论/零他人做法；提交前 banned 扫描全 CLEAN。
5. **身份配额**：每身份每题 10 次；429/满额顺延池内下一身份；禁新增。
6. **模型声明**：`--model "DeepSeek V4 Flash" --harness "DeepSeek Harness"`。
7. **判分慢**：部分题评分器评估 >30 分钟——勿误判为失败。

## 6. 常见坑清单

| 坑 | 后果 | 修复 |
|---|---|---|
| CLI 提交 trace 未挂载 | traceCount=0 → 判词 29/0 | 确认 trace 参数/格式（json.dumps 列表） |
| run.log 是 CLI 固定文案 | stdout_anchor 必挂 → trace 29 | run.log = 真实执行日志落盘 |
| 模板时间戳照抄 | timestamp_window 必挂 | 时间戳 ∈ ran_at ± wall_time，真实会话时间 |
| 损坏 characterization.json | 整包否决 | 提交前 JSON 语法校验 |
| skills/ 无 manifest.json | 400 unsupported schema | schema_version="playground-skill-evidence/v1" 或删掉 skills/ |
| native_trace 多个 | 400 | bundle 内恰好一个 native_trace/trace.jsonl |
| zip 时间戳早于 1980 | 挂载失败 | 时间戳 ≥1980 |
| deviations.target 与 expected_outputs.name 错位 | oc/rf 双零 | target 名与 expected_outputs 命名空间对齐 |
| 同内容 10 分钟内重交 | N16/作弊判定 | 实质差异 + ≥10 分钟 |
| 等旧 draft 恢复 | 事故期 draft 卡死永不恢复 | 立即新 attempt 重交 |
| 带 script 四步链 | bundle/judge 轨 → 不进真实榜 | 无 script 手动链 / CLI |
| trace <70 就交 | factor 打折（69→0.69） | 真实执行导出 ≥70 |

## 7. 平台故障模式与判分器状态矩阵

> 提分困难 ≠ 提交物问题——先对照此表定位平台侧状态。

**7 类故障模式（按影响排序）**：
1. **基础设施事故 + 批量重评风暴**：attempt 置 gradable=false/pending=true（"preserved for re-scoring"横幅）；随后批量重评可推翻已落袋分（实测曾从 97/95 回滚到 69/84）。→ 落袋分数不耐久，重评批后需复查。
2. **worker /uploads 队列停摆**：卡 draft 数小时+（updatedAt 静止），事故期创建的旧 draft **永不恢复**——一律新 attempt 重交。
3. **间歇性 harbor 窗口**（个别题特有）：按周期只处理单条/小批，窗口可预测。→ 窗口内排队提交可能被处理。
4. **逐题判分器状态不一致**：同账号同形态在不同题行为不同。→ **每题开打前先探测**（1 发低成本探针看 harbor_replay_executed）。
5. **harness 准入差异**：个别题只有官方 harness 枚举名触发 harbor——非 harness 名问题，是题级判分器注册差异。
6. **trace 判分档位化**：29=构造/模板、69=时间轴伪造（duration Σ ≪ 窗口）、84+=真实执行导出——只有真实执行记录能到 84+。
7. **judge/bundle 轨分数被官方榜过滤**：四步链（带 script）96-100 也全部不收录。

**平台阻塞类（非提交物可解）**：per-challenge grader 未注册（grader_name=null、"No per-challenge grader registered"）的题，result_fidelity 结构性不可达——**不要为 characterization deviations 花 quota**，唯一路径 = 平台注册 hidden verifier。

**延迟回填认知**：harbor 回填 30-50 分钟是机制非故障。**提交后 5-15 分钟查无 harbor ≠ 失败**，等 30-50 分钟再判生死。

**判官字段 redacted 阻碍复刻**：他人零附件高分 attempt 的传递字段被 answerRedacted=true 隐藏，无法逆向"官方 harness 发布路径"——不要浪费时间逆向，直接走自己已验证的 CLI 形态。

**判分时间线**：正常 4~15 分钟出分；事故窗口数小时/卡 draft（平台侧，换新 attempt 重交）；pending_review 保留待重评（平台批量重评时会回填）。

## 8. 提交前检查单（最终门）

> ⚠️ 间隔/身份/banned/trace/通道由插件 `solver-guard_build-submit` 六道门自动强制；以下为理解性清单。

```text
[ ] 新 attempt（不用任何事故期旧 draft）
[ ] 无 script 字段（harbor 轨；script 会切 bundle/judge 轨）—— 插件自动（channel 门）
[ ] trace：真实执行导出（机器层 6 条 + trace_score ≥ 70）—— 插件自动（trace 门）
[ ] outputs：当前最佳数值（不是旧值）
[ ] 间隔：距上一发 ≥ 10 分钟 + 内容实质差异 —— 插件自动（cadence 门）
[ ] 身份：白名单内，本题未满 10 —— 插件自动（identity 门）
[ ] banned 扫描全 CLEAN（零分数/零 attempt id/零他人做法）—— 插件自动（redline 门）
[ ] 提交后：GET /api/attempts/{id} 核实归属 + 拉排行榜 data.json 确认收录 —— ScoreWatcher 自动回填并推送
```

## 9. 一句话范式

> **用新 attempt、无 script、真实 trace≥70、当前最佳数值——harbor_reward 只认数值。旧 draft 一律作废，别等平台，别纠结通道。**
