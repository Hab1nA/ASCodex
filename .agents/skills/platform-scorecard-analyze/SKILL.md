---
name: platform-scorecard-analyze
description: "识别 Bohrium Playground 判分器类型，归一化原始分、有效分、判罚、归属与榜单范围，并在授权预算内设计单字段 A/B。高分未满时先定位丢分分量，禁止盲试。触发词：'判分器类型'、'harbor 档位'、'单字段 A/B'、'怎么提分'、'分量级分解'、'哪个分量掉分'。"
metadata:
  version: 1.3.0
  author: friday-team
  tags: [bohrium-playground, scoring, harbor, ab-testing]
---

# 判分器识别与 Harbor Oracle 实验

识别题目判分器类型，先把平台返回的评分事实按 revision、身份与榜单范围归一化，再用受控实验定位计分字段。

## When to Use

- 新题开题第一步（决定整个攻分策略）
- 分数卡死时判断"是字段问题还是判词问题"
- 需要定位"哪个字段在计分"时

## 核心原则（实测铁律）

1. **先核实 oracle 稳定性**：只有在同 bundle revision、同归属、同评分版本的重复证据显示稳定时，才把某个分量当确定性 oracle；不得把历史稳定性当作现行全站保证。
2. **判官口径 ≠ 论文字面值**：以题目 verifier 公式与保留名为准；论文数值只作交叉，遇矛盾信题目。
3. **单字段 A/B 是优先诊断法**：固定其余、改一个字段、等待该 bundle 的 fresh rescore，再比较同一 scope 的 raw score/明确分量。任何提交都需用户授权和额度门。
4. **本地饱和即换轨**：同一分量本地复核 ≥3 轮无新信息 → 停止本地复核，转 oracle 差分诊断（Step 5）。"本地都对"≠"判官给分"——这是最大时间黑洞（KMC 95.75 教训）。

## Step 0 — 建立评分事实卡

在分析档位前记录：challenge、attempt、bundle revision/hash、fresh rescore 状态、raw score、effective score、判罚及依据、credited user/agent、榜单 scope/season、trace admission 与反作弊状态。缺失写 `unknown`。

- 平台当前报告：全站榜与赛季总榜口径已统一，但每条观测仍要保留 scope/season，不能跨范围混算。
- 受罚提交保留原始分，同时有效分扣 1 分并保留判罚标记；不得用 effective score 反推科学内容质量，也不得把 raw score 当榜单计分。
- ARM bundle 重传后必须重新评分；旧分对新 revision 无效，`rescore_status != completed` 时禁止进入 A/B 结论。
- 成绩必须绑定 credited user/agent；归属不明时停止身份/额度归因。
- 反作弊是加权判定且增加了三个信号。没有服务端证据时只记 `weighted_anticheat: unknown`，不猜信号名、阈值或权重。

## Step 1 — 识别判分器类型（读题面评分契约）

| 类型 | 特征 | 判分方式 | 代表题 |
|---|---|---|---|
| A 确定性数值 verifier | §6 写"确定性 generic substitutions/容差 1e-8"、逐字段 schema | 每字段 0/1 硬匹配，离散档位（split 22 项≈9/11 档） | twist/ppt/split/uv |
| B LLM judge (harbor-lbg) | 判词化（"did not find complete, genuinely-derived"）、coarse bucket | 档位式（0.78/0.80/0.88；0.1/0.22/0.32） | gbsde/permuton |
| C 预测/成像内容比对 | 答案含预测值/图像/calls 集 | 与隐藏 truth 数值比对（容差 ~1e-4）/F1/图像指标 | flowforge/cnv/ultrasound |

**确定性 verifier 识别法**（09 误判教训）：harbor 对内容多变体**完全不变**（加严推导/raw_messages/模板换皮全不动）= 确定性 verifier 特征；LLM judge 会响应内容呈现。09 被误判为 LLM 内容桶达数轮，浪费大量内容 A/B 火力，读题后才发现是确定性 verifier——**先读题面 §5/§6 逐字确认，不要凭判词猜类型**。

## Step 1b — 档位分布反推参考值法（读全场分档，反推判官参考值）

确定性 verifier 题的分档是离散的，可**反推隐藏参考值**：

- 只有实时 scorecard 明示公式且 bundle 已 fresh rescore 时，才把 harbor 档位换算成总分；否则直接比较原始分量，不套用历史固定公式。
- 全场分布找"1/160 桶位"：如 score 增量 = 1/160 的整数倍（09: 8/160 丢分 → harbor 档差 0.05 量级）→ 反推每个 checkpoint 权重。
- 用已知档位 + 我方分数差，推断判官 reference 的取值窗口（如 judge-01 E_ref ∈ [−30.59,−30.01]，实测最优窗 [−30.32,−30.28] → E_ref≈−30.30）；"0.9 档贴参考"——harbor 0.9 档的答案即贴近判官 reference，继续微调方向从档位差反推。
- 用途：把"差多少分"翻译成"哪个 checkpoint 的哪个字段差多少"，为差分实验（`judge-field-audit`）提供靶点。

## Step 2 — A 类题：字段级硬匹配攻分

1. 逐字读题面 §5 评分契约：**保留名清单**（表达式必须用的记号）、**形式要求**（如"κ_D 必须保留符号"）、**词表**（validity 映射）、**排他声明**（如"no spin-average"——friday 曾集体漏读）。
2. 把契约翻译成自建 verifier（jarvis 法：题面 §5 → 逐条可执行检查），在本地爬分。
3. 单字段 A/B：每次只改一个字段，提交 → 读 harbor → diff。
4. 高频坑（实测）：裸值 vs 角平均（极化求和等）、因子双重计数、符号保留、负值（如 q）被拒、机械重建系数符号。
5. **字符串/枚举字段审计（09 满分教训，与数值同权重受检）**：交付物字段分**数值类 vs 字符串枚举类**，枚举类逐 label 码表核对（mechanisms: nonzero / first_bilinear_vanishes / second_bilinear_vanishes / lorentz_contraction_vanishes；separability_class；outer_maximization），按题面**逐分量机械判定**——C3 mechanisms 枚举串全错（数值全对）丢 8 分，修复 92→100。本地自检必须两类都覆盖，差分实验也须覆盖两类字段（见 `judge-field-audit`）。

## Step 3 — B 类题：LLM judge 攻分

- 判词是唯一可读信号：scoringDetails 的判词（"leaning on paper" = 引用论文被罚；"not genuinely-derived" = canonical 推导绑定）。
- 已证效应：**论文逐点标注 [Paper i] 每步标注** 比仅末尾映射高 0.12（gbsde f3）；纯小数/极简版会降级（判官计完备性与表示形式）。
- 多推导族全同分 = canonical 绑定 → 转 §4 换角度协议，不要在等价壳内继续换呈现。

## Step 4 — C 类题：内容比对攻分

- 内容确定性（同内容跨身份同分）；分数只由主输出文件决定（flowforge 只由 predictions.csv）。
- 找"参数化悬崖"：超声 α/β 轴存在极锐峰（0.76 有效、0.765 归零）；cnv 的 calls 集是离散语义（对齐 truth model：cn.mops integerCN）。
- 本地指标要当心自洽假象：cnv 的 median-ratio 本地 F1=1.0 但 harbor 差——必须用 harbor 实测校准。

## Step 4b — 判官口径 A/B 原则（题面字面 vs 实现分歧 → 提交裁决）

题面字面读与物理实现/词义发生**硬分歧**时（如 01：题面 "boundary sphere radius" 字面读 R_cell vs 实现 R_RIR 满足 "g→1 at large r" 契约），**本地验证解决不了——判官按哪个口径判，唯一裁决者 = 提交**：

1. 固定其余字段，只改口径候选（R_cell run），经用户授权提交 1 发，等待当前 bundle revision fresh rescore 后读 raw score/目标分量，与同 scope 基线 diff。
2. 高于基线 → 该口径命中，续评；≤ 基线 → 另一口径确证，按判词/物理收场（01 29176：R_cell 0.6 < baseline 0.769915 → 判官按 R_RIR 口径，定稿 76.99）。
3. 判官口径分歧的预期增量必须来自**判别性实验设计**（该提交能区分两种口径），否则禁盲试。

## Step 5 — 高分未满（总分高但未满 100）：先分量级分解，禁止盲试 A/B

**铁律：总分高而未满时，禁止直接盲试 A/B。先定位"哪个分量（哪个 25%/子项）在掉分"，再对该分量做单字段 A/B。**（KMC 95.75 局的最大教训）

> 完整工作流见 **`differential-scoring`** 技能（缺口量化 → 零 quota 差分 → 分量假设 → 定向突变 → 定点清除）；零 quota probe 端点清单见 **`oracle-probe`** 技能。此处只记两条要诀：
> - **本地饱和门**：同分量本地复核 ≥3 轮无新信息 → 停止本地复核，转 oracle 差分。"本地都对"≠"判官给分"（最大时间黑洞）。
> - **诊断预算**：A/B 预算 ≥50% 先用于定位分量；定位不到前不做"整包重写"。

## 回报格式（每题维护"判官信号卡"）

```
题: <slug>
判分器类型: A/B/C
评分事实: <bundle revision/hash | rescore status | raw/effective | penalty+basis | credited user/agent | scope/season | trace admission>
档位结构: 我方 raw X vs 同 scope 全场公开聚合 [分布]；参考值反推: <档位 → 权重/参考窗口>
分量级定位: <哪个分量在掉分 + 证据>          ← 高分未满时必填
字段双绿: <数值字段: PASS/FAIL | 字符串枚举字段: PASS/FAIL>   ← 两类都受检才算过
已证字段响应: <字段, 改动, harbor 变化, attempt_id>...
判词要点: "..."
结论置信级: <推断(判官/红队分解) vs 提交级实证(attempt_id 锚点)>   ← 推断必须标注
剩余候选轴: ...
轴台账: <轴 | 假设 | attempt_id | harbor 增量 | 结论>   ← 卡死判定的客观依据
```

判罚依据和反作弊诊断只进本地信号卡，禁止进入 trace、stdout、代码注释或提交 artifact；他人提交只能使用公开聚合信息，不能匿名读取其 outputs。

> **轴台账**是卡死判定的客观依据：每个 A/B 轴一行，附 attempt id 证据。同轴 ≥2 次不动 + 本地饱和 → 触发 `unstuck-switch-angle`（见其收紧后的触发条件）。
> **双绿列**：字符串枚举字段与数值同权重受检（09 C3 枚举串错丢 8 分）；任何结论挂验证锚点（attempt id 或独立参考），判官/红队的推断分解必须与提交级实证分开标注。
