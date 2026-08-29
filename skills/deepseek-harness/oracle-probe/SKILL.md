---
name: oracle-probe
description: "判官只读诊断：不花 quota、不改内容，从 worker/判分器侧读出分量级评分、运行日志、逐项校验结果与 scorecard 明细，用于差分判分定位。与'提交'路径分离，避免诊断误耗 quota。触发词：'判官明细'、'oracle probe'、'读判官日志'、'分量级评分'、'scorecard 明细'、'判分器侧'。"
version: 1.1.0
author: friday-team
tags: [bohrium-playground, scoring, oracle, diagnostics, read-only]
---

# 判官只读诊断（Oracle Probe）

**目的**：在不提交、不花 quota 的前提下，从判分器/worker 侧读出**总分之外**的信号——分量级评分、运行日志、逐项校验结果。这是差分判分定位（`differential-scoring` Step 1）的零成本第一步。

**核心原则**：**诊断与提交分离。** 只读端点不消耗 attempt quota；提交端点才消耗。先穷尽只读诊断，再花 quota 做定向突变。

## When to Use

- 拿到一个已评分 attempt，想知道"除了总分还有没有更细的信号"。
- 总分高而未满，想定位丢分分量（配合 `differential-scoring`）。
- 判词中性（"Strong submission"）但不到满分，想看判官逐项怎么判的。

## 只读端点清单（按信息量排序）

> 以下端点均用 `Authorization: Bearer <asp_* / identity JWT>`。**只读（GET）不耗 quota**；只有 POST 提交/评分才耗。

### 1. Attempt 详情（scorecard 全貌）
```
GET https://play.bohrium.com/api/attempts/{id}
```
- `score` / `status` / `scorecard`
- `scorecard.harbor_reward`（harbor 分）
- `scorecard.trace_score`（trace 分）+ `trace_factor`（≥80→1.0）
- `scorecard.scoringDetails`（判词原文）——**逐词挖掘，找指向具体分量的措辞**
- 留意 scorecard 里是否有**未读过的字段**（per-component 分数、逐项校验、子分数）。很多判分器把分量级分数藏在这里，只是没人翻。

### 2. Attempt 列表（横向对比档位）
```
GET https://play.bohrium.com/api/challenges/{id}/attempts
GET https://play.bohrium.com/api/challenges/{id}/attempts?outcome=stuck
```
- 看全场分数分布（档位对称性）：满分档 / 我方档 / 低分档各占多少。
- 找**已评分的 100 分 attempt**（`outcome=success` 且 score=100），尝试取其 outputs 做 diff（若未 redacted）。

### 3. Worker 侧（判分执行方，信息最细）
> Worker 是实际跑判分器/容器的服务，往往有**运行日志、逐项校验、分量级分数**。端点从 CLI 源码或 attempt 详情里的 worker 字段找（例：`http://47.92.88.121:443/api/...`）。

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
4. **100 分 diff**：取满分 attempt 的 outputs，与我方逐字段 diff，口径差（CI/窗口/舍入/保留名）往往就在 diff 里。

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

- **只读不提交**：probe 阶段只用 GET，不 POST，不耗 quota。
- **先零成本**：Step 1（翻 scorecard + 判词）拿到信号就停，不要急着花 quota。
- **记录**：每个 probe 的发现写进差分诊断卡（`differential-scoring` 回报格式）。
- **不泄露凭据**：probe 用 identity JWT / asp_* token，不要把 token 写进交付物或日志。

## 与相关技能的关系

- `differential-scoring` = 本技能的"下游"（拿到信号后做分量假设 + 定向突变）。
- `platform-scorecard-analyze` Step 5.1 = 本技能的入口（判官明细 probe）。
- `platform-scorecard-analyze` = 判分器类型识别（A/B/C），决定 probe 重点。
