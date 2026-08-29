---
name: oracle-probe
description: "判官只读诊断：不改内容，从已授权的 worker/判分器侧读出分量级评分、运行日志、逐项校验结果与 scorecard 明细，用于差分判分定位；配额语义以当前服务契约核实。与提交路径分离。触发词：'判官明细'、'oracle probe'、'读判官日志'、'分量级评分'、'scorecard 明细'、'判分器侧'。"
metadata:
  version: 1.2.0
  author: friday-team
  tags: [bohrium-playground, scoring, oracle, diagnostics, read-only]
---

# 判官只读诊断（Oracle Probe）

**目的**：在不提交的前提下，尝试从已授权的只读接口读出**总分之外**的信号——分量级评分、运行日志、逐项校验结果。这是差分判分定位（`differential-scoring` Step 1）的第一步；是否消耗配额必须以当前服务证据核实。

**核心原则**：**诊断与提交分离。** 只读请求不得自动写入或提交；不要把“GET 不耗 quota”当作未经核实的普遍事实。先确认当前服务契约，再考虑任何定向突变。

## When to Use

- 拿到一个已评分 attempt，想知道"除了总分还有没有更细的信号"。
- 总分高而未满，想定位丢分分量（配合 `differential-scoring`）。
- 判词中性（"Strong submission"）但不到满分，想看判官逐项怎么判的。

## 只读端点清单（按信息量排序）

> 以下端点仅作协议示例。凭据只能来自当前进程环境变量，绝不写入日志；配额语义必须通过当前题目/服务端证据确认。

### 1. 自有/明确授权 Attempt 详情（scorecard 全貌）
```
GET https://play.bohrium.com/api/attempts/{id}
```
- `raw_score` / `effective_score` / `status` / `scorecard`（字段名以实时响应为准，不凭历史 schema 猜测）
- 判罚标记、判罚对象、原因与分数改写；平台当前报告的口径是有效分扣 1 分且保留原始分
- 成绩归属的 user/agent，以及榜单 scope（全站/赛季/题目）
- ARM bundle revision/hash、重传时间与 fresh rescore 状态
- trace 是否具有运行痕迹、是否进入待评队列；反作弊当前是加权判定，未知信号与权重不得臆测
- `scorecard.harbor_reward` / `trace_score` 等字段若真实存在，可作为证据记录，但不能据历史固定公式推算最终分
- `scorecard.scoringDetails`（判词原文）——**逐词挖掘，找指向具体分量的措辞**
- 留意 scorecard 里是否有**未读过的字段**（per-component 分数、逐项校验、子分数）。很多判分器把分量级分数藏在这里，只是没人翻。

### 2. 自有 Attempt 列表与公开聚合榜（横向对比档位）
```
GET https://play.bohrium.com/api/challenges/{id}/attempts
GET https://play.bohrium.com/api/challenges/{id}/attempts?outcome=stuck
```
- 看公开分数分布时同时记录 scope、season 与成绩归属；不得把全站榜、赛季榜或题目榜混成同一序列。
- 平台已关闭匿名读取他人提交的路径。不得尝试读取、枚举或推断他人的 outputs、trace、scorecard 私有明细；逐字段 diff 只限我方自有提交或数据所有者明确授权的 artifact。

### 3. Worker 侧（判分执行方，信息最细）
> Worker 是实际跑判分器/容器的服务，可能有**运行日志、逐项校验、分量级分数**。只允许读取当前身份自有或明确授权的 attempt，并使用当前协议明确的 HTTPS 主机；禁止根据历史示例访问任意 worker IP。

- `GET /api/attempts/{id}`（worker 侧）——状态 + 评分结果
- 找 worker 的**日志端点**（如 `GET /api/attempts/{id}/logs`、`GET /api/attempts/{id}/result`）——判分器 stdout/stderr、逐项 check 的 pass/fail。
- 找 worker 的**评分明细端点**（如 `GET /api/attempts/{id}/scorecard`、`GET /api/attempts/{id}/checks`）——分量级分数。

> **技巧**：用 CLI 源码（`@paper2arm/playground-cli` 的 `dist/index.js`）搜 `/api/` 路径，把所有只读端点列出来——判分器写了哪些端点，CLI 里通常都调过。

### 4. 题目/技能文档（判分口径的书面依据）
```
GET https://play.bohrium.com/api/challenges/{id}/content
GET https://play.bohrium.com/api/docs/{slug}?lang=zh
GET https://play.bohrium.com/api/skills
```
- 题面 §5/§6 评分契约（分量定义、口径、容差）。
- 平台技能（判分器实现细节、CI 约定、窗口定义）——**口径差常在这里暴露**。

## 诊断手法（配合 differential-scoring）

1. **翻 scorecard 隐藏字段**：把 `GET /api/attempts/{id}` 的完整 JSON dump 出来，逐字段扫，找 score/harbor_reward/trace_score/scoringDetails **之外**的字段（per-component、checks、subscores）。
2. **判词逐词**：scoringDetails 原文，把每个形容词/名词映射到分量（"derived"→推导、"complete"→完备性、"units"→量纲）。
3. **worker 日志**：判分器跑完通常会打"component X: 0.83 / component Y: 1.0"——这是**直接的分量级分数**，比盲试准得多。
4. **授权 artifact diff**：只在 artifact 属于我方或获得数据所有者明确授权时逐字段 diff。公开榜单显示满分不构成读取其 outputs 的授权。

## 新评分事实的归一化

probe 结果至少区分以下事实，缺失字段写 `unknown`，禁止用一个 `score` 覆盖：

```yaml
score_scope: all_time | season | challenge | unknown
season_id: <id-or-unknown>
raw_score: <number-or-unknown>
effective_score: <number-or-unknown>
penalty:
  applied: true | false | unknown
  delta: -1 | <server-value> | unknown
  subject: <self/authorized identity-or-unknown>
  reason: <server evidence-or-unknown>
credited_owner: {user: <...>, agent: <...>}
bundle: {revision: <...>, sha256: <...>, rescore_status: pending|completed|failed|unknown}
trace_admission: admitted | rejected_no_execution | pending | unknown
weighted_anticheat: {status: <...>, signals: <server evidence only>}
```

- ARM bundle 重传后，旧评分一律标为 `stale_for_current_bundle`，直到同一 revision/hash 的 fresh rescore 完成。
- 原始分可见不等于恢复计分；比较解题质量时用明确标注的 raw score，报告榜单结果时用对应 scope 的 effective score。
- 判罚依据、反作弊信号和他人身份属于诊断侧信息，不得复制进正式 trace、stdout、代码注释或提交 artifact。

## 输出：decomposition_ambiguity 段（判官分解歧义必填）

判官/红队的分数分解经常是**推断**且存在多个等价解（09：D70+N17 vs D62+N25，判官 H1 错指）。probe 后必须输出歧义段，供 `judge-field-audit` 设计判别性提交实验：

```
decomposition_ambiguity:
  等价解: [ {分解: "D70+N17", 置信: 中, 依据: 判词"Bell 8 丢"}, {分解: "D62+N25", 置信: 低, 依据: 档位反推} ]
  判别性 A/B 设计: [ {弄坏 C3 整块 → 掉 8/160 则解1成立，不动则解2}, ... ]
  悬空字符串字段清单: [ mechanisms 未核对, separability_class 未核对, ... ]
```

- 每个等价解附**置信 + 依据**（推断 vs 提交级实证分开标注）；
- 悬空字符串枚举字段永远列出（09 教训：枚举串是自检盲区高发区）；
- 判官分解的"预期增量"必须能落到判别性实验上，否则禁盲试。

## 纪律

- **只读不提交**：probe 阶段只用已授权的 GET，不 POST；不对 quota 结果做未经核实的断言。
- **权限边界**：只读也需要对象级授权；匿名端点失效时停止，不尝试绕过、换 host 或复用历史路径。
- **先零成本**：Step 1（翻 scorecard + 判词）拿到信号就停，不要急着花 quota。
- **记录**：每个 probe 的发现写进差分诊断卡（`differential-scoring` 回报格式）。
- **不泄露凭据**：凭据仅在当前进程环境中使用，不要把 token 写进交付物或日志。

## 与相关技能的关系

- `differential-scoring` = 本技能的"下游"（拿到信号后做分量假设 + 定向突变）。
- `platform-scorecard-analyze` Step 5.1 = 本技能的入口（判官明细 probe）。
- `platform-scorecard-analyze` = 判分器类型识别（A/B/C），决定 probe 重点。
