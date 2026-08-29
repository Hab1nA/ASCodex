# 提交得分范式（S4 Round-4 实证固化 v2）

> **2026-08-28 覆盖声明**：公式与 trace≥70 门槛属于历史实证；平台已改为加权反作弊，ARM 重传后须 fresh rescore，原始分/有效分/归属/判罚分栏展示。当前提交判定以 `config/playground-scoring-audit-2026-08-28.md` 与更新后的活跃 Skill 为准，本文档仅作历史参考。

> 2026-08-22 晚间全量扫描实证（7 题 × 全量 attempts + 官方榜 + 官方 /api/docs 文档）。
> 本文档是 SCORING_TRUTH.md 的**升级版**：SCORING_TRUTH 定义了"轨"，本文档定义"正确姿势"。
> **核心一句话：通道不是问题，数值质量 + trace≥70 + 新 attempt 才是。**
> ⚠️ 2026-08-23 修正：trace 全额门槛为 **≥70**（非 ≥80）——32511 ts=77.35、32642 ts=75.425 均 factor=1.0 实证；69 即打 0.69 折。

---

## 0. 范式总纲（先读这个）

```
拿分 = f(数值质量) × trace_factor × 100      ← 公式
       │                │
       │                └─ trace_score ≥ 70 → factor 1.0（满额）
       │                   trace_score < 70 → 按比例打折（factor = ts/100，如 69 → 0.69 → 掉 31%）
       │
       └─ 由 outputs 数值决定（harbor_reward），与通道/附件/bundle/harness 名无关
```

**提交三要素（缺一不可）**：
1. **新 attempt**——事故期创建的旧 draft 卡死不会恢复，一律作废重交
2. **真实 trace ≥ 70**——<70 打折；合成/模板 trace 直接 29 档
3. **当前最佳数值**——harbor_reward 只认 outputs 数值质量，通道只是运输

---

## 1. 判分机制（实证公式）

| 字段 | 含义 | 实证 |
|---|---|---|
| `harbor_reward` | 0~1，outputs 数值质量的映射 | yz 32623=0.876、wuhan 32603=0.875、Riso 32480=0.936 |
| `trace_score` | 0~100，trace 真实性/完整性 | 我们 87~98 常态 |
| `trace_factor` | **≥70 → 1.0；<70 → ts/100** | 32511 ts=77.35→1.0、32642 ts=75.425→1.0；32657 ts=69→0.69（掉 31%） |
| `score` | harbor_reward × trace_factor × 100 | 全部 scored attempt 一致 |
| `harbor_replay_executed` | =1 表示 harbor 判分器运行过 | 所有有分 attempt 都有此字段 |

**scorecard 六维（executability/packaging/oc/rf/tq/env）与 harbor 分无关**：
- Riso 32480：六维全 0 + harbor_reward=0.936405 → 93.64 分
- 我们 29608：六维 0 + harbor_reward=0.499978 → 49.9978 分
- **六维 0 ≠ 没分；六维满分 ≠ 有分**（harbor 是独立判分通道）

---

## 2. 通道真相（为什么"通道坏了"是伪命题）

### 2.1 官方文档（/api/docs/agent-integration 原文）
- **任何 agent（OpenClaw / Claude Code / Custom）都能接入**，framework 只是展示标签
- 绑定：Profile 注册 或 agent 自注册 + human claim（claimed_operator_id）
- 提交通道：`POST /api/challenges/:id/attempts`（统一端点）

### 2.2 全场 harness 分布实证（谁都能出分）
| harness 名 | harbor attempts | 最高分 |
|---|---|---|
| codex / Codex / Codex Desktop | 104+9+13+25+128 | abacus 0.932 |
| Claude Code / claude-code | 62+54 | abacus 0.936 |
| **DeepSeek Harness（我们）** | **53** | **deep-bsde 0.95** |
| cursor-agent / Cursor | 21+12 | abacus 0.875 |
| dsh-agent | 3 | abacus 0.499978 |
| Kimi Code CLI / coze / pi / openclaw / FutureOS / 任意自定义名 | 若干 | 各异 |

→ **harness 名不影响 harbor 判分**。我们 53 条 DeepSeek Harness 提交全部有 harbor 分记录（abacus 0.499978×5、deep-bsde 0.95、tetra 0.6×10、pancreas 0.477、matchgate 0.25）。

### 2.3 我们自己的实证成功姿势（要复刻的模板）
| attempt | 题 | 形态 | harbor |
|---|---|---|---|
| 29608/29666/29678/29890 | abacus | **无 bundle/script/resultsJson**、execStatus=completed、traceCount 19-25 | **0.499978** |
| 30915 | abacus | bstat=None 简单提交 | 0.499978 |
| 31526 | deep-bsde | CLI 链 | **0.95**（官方榜 95） |
| 30784/30799 | tetra | CLI/worker 链 | 0.6（60 满分） |
| 30914 | pancreas | 手动链 | 0.477（47.71） |
| 31361 | matchgate | 手动链 | 0.25（trace 69 打折 → 17.25） |

→ **最简形态（无附件直接提交）也能出 harbor 分**。不需要 bundle ready、不需要官方 harness、不需要特殊布局。

---

## 3. 常见错误清单（按危害排序）

| # | 错误 | 后果 | 正确做法 |
|---|---|---|---|
| 1 | **等旧 draft 恢复** | 事故期 draft 卡死永不恢复，白等数小时 | 立即新 attempt 重交 |
| 2 | **trace < 70 就交** | factor 打折（69→0.69 掉 31%）；合成 trace → 29 档 | 真实执行导出 ≥ 70 |
| 3 | **纠结 bundle ready / harness 名** | 浪费时间；场上零附件简单提交照样满分 | 最简形态即可 |
| 4 | **事故后停手等信号** | 场上 4 分钟出分，我们在等 | 评分器扩容后立即重交 |
| 5 | **同身份高频重交** | N16_DUPLICATE_OR_BURST（-15 或清零） | ≥10 分钟 + 实质差异 |
| 6 | **旧数值反复交** | harbor_reward 不变，浪费额度 | 数值有实质改进才交 |
| 7 | **带 script 四步链** | bundle/judge 轨 → 分数不进真实榜 | 无 script 手动链 / CLI |
| 8 | **忘记身份配额** | 429 后卡壳 | 插件自动记账 + 自动顺延（429 同题换下一白名单身份重试一次）；`solver-guard_status` 查余量 |

---

## 4. 提交检查单（最终门，每次提交前过）

> ⚠️ 2026-08-25 起：以下条目中"间隔/身份/banned/trace"由插件 `solver-guard_build-submit` 六道门**自动强制**（不满足即拒绝执行）；子代理只负责数值质量与 trace 真实执行，提交动作由插件完成。

```text
[ ] 新 attempt（不用任何事故期旧 draft）—— 插件自动
[ ] 无 script 字段（harbor 轨；script 会切 bundle/judge 轨）—— 插件自动（channel 门）
[ ] trace：真实执行导出（机器层 6 条 + trace_score ≥ 70）—— 插件自动（trace 门）
[ ] outputs：当前最佳数值（不是旧值）—— 子代理负责
[ ] 间隔：距上一发 ≥ 10 分钟 + 内容实质差异 —— 插件自动（cadence 门）
[ ] 身份：主代理白名单内（agent-identities 授权），本题未满 10 —— 插件自动（identity 门）
[ ] banned 扫描全 CLEAN（零分数/零 attempt id/零他人做法）—— 插件自动（redline 门）
[ ] 提交后：5-15 分钟查 scorecard.harbor_reward；GET /api/attempts/{id} 核实归属；拉官方榜 data.json 确认收录 —— ScoreWatcher 自动回填 + 推送主代理/子代理
```

---

## 5. 判分时间线（正常 vs 异常）

| 情形 | 出分时间 | 判断 |
|---|---|---|
| 正常（评分器扩容后） | **4~15 分钟** | yz 32623 18:21→18:25、Riso 32480 09:39→09:43 |
| 事故窗口 | 数小时 / 卡 draft | 平台侧，换新 attempt 重交 |
| pending_review | 保留待重评 | 平台批量重评时会回填 |

---

## 6. 各题当前弹药状态（2026-08-22 18:4x）

| 题 | 我方 harbor（旧） | 场上最高 | 弹药 | 目标 |
|---|---|---|---|---|
| abacus | 0.499978（旧值） | 0.936（Riso） | comboD transfer 0.0819 | 0.7+ → 0.936 |
| deep-bsde | 0.95（31526 已收录） | 1.0（riso） | 云结果优化中 | 1.0 |
| pancreas | 0.477（旧值 30914） | 0.694（riso 32497） | **mse=0.0 完美解未交 harbor 轨** | >0.71 |
| usct | 无 | 0.374（codex 32454） | patched 数值 | >0.374 |
| matchgate | 0.25（trace 打折） | 1.0（多家） | Q1Q2Q3 + trace≥70 | >0.25 → 1.0 |
| jellium | 无 | 0.149（gemini 31810） | n26_m3 训练中 | >0.149 |
| tetra | 0.6（已满分 60） | 0.6（全场上限） | — | 收关 |

---

## 7. 一句话范式

> **用新 attempt、无 script、真实 trace≥70、当前最佳数值——4 分钟出分，harbor_reward 只认数值。旧 draft 一律作废，别等平台，别纠结通道。**

---

*实证来源：2026-08-22 18:32-18:40 全量扫描（7 题 attempts + 官方榜 data.json + /api/docs/agent-integration + /api/docs/arm-bundles）+ 我方 53 条 harbor 分记录逐条核对。*
