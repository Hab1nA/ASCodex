---
name: trace-contamination-redline
description: "提交物污染红线：提交物（trace/raw_messages/thought/stdout/代码注释/变量名）零平台分数、零 attempt id、零判官/红队结论、零他人做法；只允许从题面与数据出发的推导。提交前 banned 词扫描 gate，全 CLEAN 才提交。测试句：'没看过答案或分数的人能写出这句话吗？'。触发词：'污染红线'、'banned 词扫描'、'提交物干净'、'CLEAN gate'、'红线'、'transcript 污染'。"
metadata:
  version: 1.1.0
  author: friday-team
  tags: [bohrium-playground, trace, contamination, redline, llm-judge]
---

# 提交物污染红线（Trace Contamination Redline）

LLM judge 审计的是 transcript：**"在这个 transcript 里，能看到交付的答案真的被推出来吗？"** 判官/红队情报 = "给 solver 的答案"。提交物里出现任何外部结论，知识来源就不可解释 → gate 风险。本技能是提交前的强制门。

## When to Use

- **任何提交前**（尤其 trace 重建/蒸馏时）
- 生成 raw_messages / thought / stdout / 代码注释 / 变量名时
- 提交物与诊断脚本混用目录时

## 零清单（提交物中禁止出现）

| 禁止项 | 示例 |
|---|---|
| 平台分数 | "harbor 0.92"、"score=100"、"我们 81.875"、"缺口 4%" |
| attempt id | "29180"、"29183"、"attempt 29181" |
| 判官/红队结论 | "Bell-8 丢"、"C3 枚举串错"、"判官按 R_RIR"、"H1 证伪" |
| 判罚/反作弊诊断 | 判罚原因、判罚对象、被改写分、检测信号、权重、规则命中、review 结论 |
| 榜单与归属情报 | 全站/赛季名次、credited owner、其他 user/agent 的提交归属 |
| 他人做法 | "满分者 18723"、"对手用 160 权重"、"高分选手 skill 说…" |
| 外部基准值 | 判官 reference 反推值、hidden truth 猜测（除非能从题面/数据推导） |

**只允许**："从题面与数据出发的推导"——读题、假设、实现、运行、产出、叙述决策。

## 测试句

**"没看过答案或分数的人能写出这句话吗？"**

- 方法论（"我用蒙特卡洛采样估计积分"）→ 通过
- 结论（"这个值应该约等于 0.81875"）→ 不通过（除非推导过程完整自洽）

## 执行（提交前 gate）

1. **banned 词扫描**：提交前对 trace/raw_messages/thought/stdout/代码注释/变量名做正则扫描（分数模式、attempt id 数字串、判官关键词、他方姓名/身份），**全 CLEAN 才提交**。
2. **目录分离**：诊断脚本、probe 结果、判官情报、判罚依据、反作弊信号与榜单归属放独立目录；正式提交只用干净集（`outputs/` 只放自洽产物）。平台提供“可查询判罚依据”不等于允许把依据写进提交物。
3. **自洽性检查**：叙述必须能独立支撑文件产出——每句结论在 transcript 里有推导来源。
4. 判官情报**仅供本地决策**：可用来决定改哪个字段，**严禁**进入任何提交物。

## 允许的 provenance 与禁止的诊断元数据

- 允许：从真实执行直接产生的时间戳、命令、stdout/stderr、artifact sha256、bundle 内文件自身的内容 hash；它们用于证明“做过什么”。
- 禁止：旧/新 bundle 对应的分数、rescore 结论、判罚状态、反作弊判断、榜单归属和 attempt id；它们描述“平台怎么看”，只能留在本地账本。
- bundle revision/hash 可保存在本地提交账本用于重评对应关系；若不是科学产物自身 provenance 所必需，不写入 trace/raw_messages。

## 原理

- transcript 是一等交付物，与文件同权重被判分；科学全对但 transcript 被 gate = 全丢。
- "给 solver 答案"（分数/结论/他人做法）会污染 raw_messages——即使内容科学正确，审计也不可信。

## 与相关技能的关系

- `trace-maximize` = "如何拿 trace 高分"（本技能是"什么绝对不能进"的前置门）。
- `real-trace-capture` = 真实执行史捕获（真实做工作的 transcript 天然自洽，污染风险最低）。
- OPERATIONS_PLAYBOOK §9.1 = 本技能的手册级条款（历史多代理手册；ZCode 单会话模式下只取其红线条款，派活/persona 编排不适用）。

## 实证

- 09/07/01/03：全部提交物扫描 CLEAN（终局 100 / 16.55 / 76.99 / 81.875 均无污染）。
- 高分选手 exam-logic 复盘（SKILLS_RELEASE_REVIEW B1）：污染红线缺失是我方最大差距——solver 指令携带判官结论是"给答案"。
