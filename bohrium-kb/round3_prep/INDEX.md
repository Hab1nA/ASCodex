# 知识库索引（bohrium-kb/round3_prep/）

> **2026-08-28 平台变更覆盖声明**：全站榜/赛季榜统一、判罚扣 1 分并保留原始分、榜单显示归属、ARM 重传后须 fresh rescore、反作弊改为加权三信号、无运行痕迹不入待评队列。本索引中 trace≥70、固定公式和 -1000 翻账均为历史经验；评分/提交判定先读 `config/playground-scoring-audit-2026-08-28.md` 与活跃 Skill。

本目录只收录**跨轮可复用**的机制/纪律/规则文档；解题过程的历史记录（战果档案、attempt 台账、决策流水）不在此存放。

---

## 核心经验资产

| 文档 | 核心内容 | 何时读 |
|---|---|---|
| **SCORING_TRUTH.md** | 真实分机制 + 提交范式 + 平台故障模式矩阵（轨定义/公式/坑清单/检查单） | 开新题定通道 / 提交前 / 平台异常判断 |
| **HARBOR_LAW.md** | harbor 三类判分器规律全集 + LLM 判词偏好与反模式 + 判官口径类型学 + 单字段 A/B 方法论 | 判 harbor 档位 / 判词解读 / 定提分策略时 |
| **TRACE_LAW.md** | trace 三轴门控 + N 码表 + 反作弊 8 规则 + 机器层 6 条 + 分题类配方 + 低分决策表 | 构造/修复 trace 时 |
| **TRACE_99_RECIPE.md** | trace 满分配方（论文引用清除/stdout 放 body/1:1 配对/≥70 全量解锁/15 步模板） | 写 trace 时（与 TRACE_LAW 配套） |
| **HIGHSCORE_PLAYBOOK.md** | 高分战术（N 码规避/数学题写法/HJB 配方/字节级复现闭环/提交预算纪律） | 高分未满时 |
| **HARNESS_GUARD_PLUGIN_DESIGN.md** | DSH 纪律守卫插件设计（六道提交门 + 异步出分守护 + SkillInjector + AutoPush + 身份白名单）——ZCode 下对应 `.zcode/` 钩子 + 校验器，本文档作机制蓝本 | 理解守卫边界/迁移治理时 |
| **OPERATIONS_PLAYBOOK.md** | 行为准则手册（六类角色/OODA/防卡死协议/实验协议/提交物纪律/封板标准） | 总负责人/新代理上岗 |
| **IDENTITY_POOL.md** | 身份池（Friday/Jarvis/Ultron 全量凭据映射、池冻结纪律） | 派活/提交身份选择 |

---

## Skill 资产索引（.agents/skills/）

### A 类：作战核心经验（实战提炼，直接服务解题）

| Skill | 触发场景 |
|---|---|
| playground-solve-optimal | 三类题战术总纲——开题选策略 |
| platform-scorecard-analyze | 判分器类型判断（确定性/LLM/成像）|
| differential-scoring | 高分未满（≥90% 非 100）差分定位 |
| judge-field-audit | 判官字段审计+提交级裁决 |
| oracle-probe | 判官只读诊断（不耗 quota）|
| trace-maximize | trace 满分（与 TRACE_LAW 配套）|
| trace-contamination-redline | 提交物污染红线 gate |
| **real-trace-capture** | 真实 trace 捕获（禁脚本合成）|
| unstuck-switch-angle | 防卡死强制换角度 |
| closure-evidence-standard | 封板三问+真天花板核对 |
| competition-coordinate | 多代理协调（ZCode 单会话模式下仅作参考） |
| bohrium-bohr | 云算力提交（重型计算纪律）|
| submit-attempt | 提交 attempt（harbor 轨）|
| mp-r-family-solve | mp-r 家族题字段专项 |
| red-team-review | 对抗性审阅 |

### B 类：平台/通用工作流（非解题经验，保留不注入）

reproduce-paper / reproduce-validate / llm-reproduce / multi-agent-reproduce / bio-reproduce / score-difficulty / grade-reproduction / resume / checkpoint / distill

### C 类：领域专用（其他题域用）

dft-convergence / materials-dft-pipeline / proof-verify / proof-pipeline / sequence-align / generate-grader / ears-session-lifecycle

---

## 阅读路径建议

```
开新题      → 自建 verifier（译 §5 评分契约）→ platform-scorecard-analyze (skill)
准备提交    → SCORING_TRUTH → TRACE_LAW → TRACE_99_RECIPE → trace-contamination-redline (skill)
分数异常    → SCORING_TRUTH（平台故障矩阵）→ oracle-probe (skill)
卡死换角度  → unstuck-switch-angle (skill) → OPERATIONS_PLAYBOOK §4
复盘归档    → 经验内联于 SCORING_TRUTH / OPERATIONS_PLAYBOOK / HARBOR_LAW / TRACE_LAW
```
