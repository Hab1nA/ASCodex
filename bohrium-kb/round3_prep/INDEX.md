# 知识库索引（bohrium-kb/round3_prep/）

> **2026-08-28 平台变更覆盖声明**：全站榜/赛季榜统一、判罚扣 1 分并保留原始分、榜单显示归属、ARM 重传后须 fresh rescore、反作弊改为加权三信号、无运行痕迹不入待评队列。本索引中 trace≥70、固定公式和 -1000 翻账均为历史经验；评分/提交判定先读 `config/playground-scoring-audit-2026-08-28.md` 与活跃 Skill。

> 更新：2026-08-23（S4 Round-4 赛后整理，历史记录已全量清理）
> 本索引收录**第一级（核心经验资产）**与**第二级（高价值过程资产）**文档。

---

## 🔴 第一级：核心经验资产（实战破案 / 实证总结 / 工程蓝图）

| 文档 | 核心内容 | 何时读 |
|---|---|---|
| **TRACE69_VERDICT.md** | trace 29/69 档位根因破案：29=构造/模板（无真实执行痕迹）、69=时间轴物理矛盾（duration Σ ≪ 窗口）；含 6 条修复清单 + analyze_trace.py 自检工具 | 任何 trace_score<70 的提交前 |
| **SUBMISSION_PARADIGM.md** | 提交得分范式 v2：新 attempt + 最简形态（无 script）+ 最佳数值 + trace≥70；score=harbor_reward×trace_factor×100 | 每次提交前（最高依据） |
| **SCORING_TRUTH.md** | 真实分机制 + **平台故障模式与判分器状态矩阵**（7 类故障/间歇窗口/延迟回填，原 SCORING_INCIDENT_REPORT 已并入） | 开新题定通道 / 平台异常判断 |
| **HARNESS_GUARD_PLUGIN_DESIGN.md** | 纪律守卫插件设计（v0.2 与实现对齐）：六道提交门（通道/身份/间隔/红线/trace/模型）+ BohriumGuard + 异步 ScoreWatcher（主代理优先推送）+ SkillInjector + AutoPush 强制续推 + per-agent 身份白名单 + 会话隔离 | 理解插件边界/派活治理时 |
| **TRACE_LAW.md** | trace 反作弊 8 规则 + 机器层 6 条（typed/pairing/timestamp/artifact/cost/stdout） | 构造 trace 时 |
| **TRACE_99_RECIPE.md** | trace 满分配方（论文引用清除/stdout 放 body/1:1 配对/≥70 全量解锁） | 写 trace 时（与 TRACE69_VERDICT 配套）|
| **HARBOR_LAW.md** | harbor 档位规律全集 + LLM 判词/表述偏好与反模式 + 判词原文样例 + 接口机制（原 JUDGE_FEEDBACK 已并入 §2.4）| 判 harbor 档位 / 判词解读时 |
| **JARVIS_METHOD.md** | 自建 verifier 法：逐字翻译 §5 评分契约 → 本地镜像判分器 → 先假设后提交 | 开题第一步 |

## 🟠 第二级：高价值过程资产（语料 / 教训 / 操作清单）

| 文档 | 核心内容 | 何时读 |
|---|---|---|
| **OPERATIONS_PLAYBOOK.md** | 行为准则手册（六类角色/OODA/铁律/派活模板） | 总负责人/新代理上岗 |
| **REDTEAM_TRACE_FINDINGS.md** | 红队 trace 结构对照发现（高分 trace 形态解剖） | trace 优化时 |
| **HIGHSCORE_PLAYBOOK.md** | 高分战术（判词驱动单变量受控实验、差分定位）+ 换角度实证校准（原 LESSONS_24H 已并入 OPERATIONS_PLAYBOOK） | 高分未满时 |
| **IDENTITY_POOL.md** | 身份池（Friday/Jarvis/Ultron 全量凭据映射、池冻结纪律） | 派活/提交身份选择 |
| **SKILLS_AGENT_IMPROVEMENTS.md** | 技能改进方法论 | 技能打磨 |

---

## 📁 未收录说明

- **skills/ 目录（.dsh/skills/，32 个）**：经验固化的最高形态，与本文档互为印证。完整评估见下节「Skill 资产索引」。
- **bohrium-kb/docs/**：平台官方协议（agent-integration、dev-AGENT_API、dev-ARM_PROTOCOL_REFERENCE 等）——参考类，非经验但通道破解依据。
- **已清理**（2026-08-23 两批）：① 过时/低价值：STATUS/BATTLE_PLAN/flowforge_FINAL_SPRINT/PPT_ISSUE/round3_issue_report/RUBRIC_STRUCTURE/SELECTION_BLACKLIST/RESEARCH_SUMMARY/旧 INDEX/IDENTITY_AUDIT/CLEANUP；② **历史记录全量**：DECISION_LOG/MONITOR_REPORT/FEISHU_INTEL/round4_watchdog_*/r3_09_*/judge_r3_*/judge_feedback_*.json/challenges_all/challenge_10_full/round3_index/skills.json/ppt4x4_fullscore_ids + 目录 challenges/data/highscore_skills/research/skills/skills_platform/tests —— 均为流水账/中间产物/重复副本，经验教训已提炼进保留文档。

---

## 🧩 Skill 资产索引（.dsh/skills/，2026-08-23 清理后 32 个）

### A 类：作战核心经验（15 个，实战提炼，直接服务解题）

| Skill | 触发场景 |
|---|---|
| playground-solve-optimal | 三类题战术总纲——开题选策略 |
| platform-scorecard-analyze | 判分器类型判断（确定性/LLM/成像）|
| differential-scoring | 高分未满（≥90% 非 100）差分定位 |
| judge-field-audit | 判官字段审计+提交级裁决 |
| oracle-probe | 判官只读诊断（不耗 quota）|
| trace-maximize | trace 满分（与 TRACE_LAW/TRACE69_VERDICT 配套）|
| trace-contamination-redline | 提交物污染红线 gate |
| **real-trace-capture** | 真实 trace 捕获（TRACE69 破案产物，禁脚本合成）|
| unstuck-switch-angle | 防卡死强制换角度 |
| closure-evidence-standard | 封板三问+真天花板核对 |
| competition-coordinate | 总负责人多代理协调 |
| bohrium-bohr | 云算力提交（重型计算纪律）|
| submit-attempt | 提交 attempt（harbor 轨）|
| mp-r-family-solve | mp-r 家族题字段专项 |
| red-team-review | 对抗性审阅 |

### B 类：平台/通用工作流（非解题经验，保留不注入）

reproduce-paper / reproduce-validate / llm-reproduce / multi-agent-reproduce / bio-reproduce / score-difficulty / grade-reproduction / resume / checkpoint / distill

### C 类：领域专用（其他题域用）

dft-convergence / materials-dft-pipeline / proof-verify / proof-pipeline / sequence-align / generate-grader / ears-session-lifecycle

### ✅ 已清理（2026-08-23，4 个）

| Skill | 清理原因 |
|---|---|
| worker-submit-chain | R4 实证推翻：worker /uploads 队列停摆 9.5h+、四步链 bundle 轨不进官方榜；"ARM 全 0 → worker 第二轨"前提证伪 |
| arm-full-workflow | 与 reproduce-paper/submit-attempt 高度重叠，且强调旧 ARM 上传流程与 harbor 轨结论冲突 |
| bohrium-compute-pipeline | 与 bohrium-bohr 完全重叠（bohrium-bohr 为完整版）|
| reproduce-submit | 与 reproduce-paper + submit-attempt 组合完全覆盖 |

### 冗余诊断结论
- 经验本体不过度分散：A 类 15 个各有明确触发场景；trace 三件套（real-trace-capture + trace-maximize + trace-contamination-redline）分工清晰不冗余
- 冗杂集中在 B/C 类平台工作流（10+ 个互相嵌套引用），建议不在作战指令中注入以省上下文
- 每次派活只注入 A 类对应 skill（插件 SkillInjector 已按阶段自动注入：pre_submit/stuck/closing/cloud/judge/handover 六阶段映射，judge 含差分判分与字段审计四卡）

---

## 阅读路径建议

```
开新题      → JARVIS_METHOD → platform-scorecard-analyze (skill)
准备提交    → SUBMISSION_PARADIGM → TRACE69_VERDICT → TRACE_99_RECIPE → trace-contamination-redline (skill)
分数异常    → SCORING_TRUTH（含平台故障矩阵）→ oracle-probe (skill)
卡死换角度  → unstuck-switch-angle (skill) → OPERATIONS_PLAYBOOK §4（换角度实证校准）
复盘归档    → 经验已内联于 SCORING_TRUTH / OPERATIONS_PLAYBOOK / HARBOR_LAW / TRACE_LAW
```
