---
name: judge-field-audit
description: "判官字段审计与提交级裁决：交付物字段分数值类 vs 字符串枚举类（同权重受检），枚举字段逐 label 码表核对；判官分解存在多等价解或判官口径分歧时，用破坏性提交级差分（故意弄坏单字段看 harbor 变不变）裁决。差分必须覆盖全部字段类型。触发词：'字段审计'、'枚举字段'、'多等价解'、'decomposition ambiguity'、'判官口径分歧'、'破坏性差分'、'弄坏字段'。"
version: 1.0.0
author: friday-team
tags: [bohrium-playground, scoring, field-audit, destructive-diff, strings]
---

# 判官字段审计与提交级裁决（Judge Field Audit）

本地自检全对 ≠ 判官过。本技能回答两个问题：**哪些字段在计分（含字符串枚举类）**，以及**判官按哪个口径/哪组等价解判**。09 满分链路（92→100）的核心机制。

## When to Use

- 判官/红队分解存在**多等价解**（如 D70+N17 vs D62+N25），无法确定哪个为真
- 交付物含**字符串/枚举/词表字段**（mechanisms / separability_class / outer_maximization / classification / support）
- "改 X 必 +N"承诺无法本地机器校验（只能靠提交读 harbor）
- 判官口径分歧：题面字面读 vs 物理实现（01 R_cell vs R_RIR）

## 阶段 A — 本地审计（零成本，先做）

1. **字段分类**：交付物每个 checkpoint 证书字段分成**数值类**（表达式/数值）vs **字符串枚举类**（枚举词表/分类标签）。**两类同权重受检**——C3 mechanisms 枚举串错 = 丢 8 分，数值全对。
2. **枚举逐 label 码表核对**：把题面/契约里的合法词表逐 label 列出，逐字段核对（09 C3 mechanisms: `nonzero` / `first_bilinear_vanishes` / `second_bilinear_vanishes` / `lorentz_contraction_vanishes`；C2 `separability_class`；C6 `outer_maximization`）。
3. **题面逐字判机制**：按**每个分量独立判**（如 J1/J2 是否消失 → 选哪个 label），拒绝"看起来合理"的整体描述。09 教训：4 个零分量（J2=0）应为 `second_bilinear_vanishes` 却全标 `lorentz_contraction_vanishes`。
4. 输出审计清单：每 checkpoint「数值 PASS + 字符串枚举一致性」双状态。

## 阶段 B — 提交级裁决（≤2 发，破坏性差分）

**破坏性差分**：故意弄坏**单个字段**的受检内容，提交读 harbor：

- 分数**掉了** → 该字段受检且在拿分（弄坏才掉）→ 说明当前版本正确，缺口在别处；
- 分数**不动** → 该字段未受检/从未拿分（09 29181：弄坏 C3 entanglement 不动 = C3 数值非计分点，丢分在枚举串）。

**纪律**：
- 差分必须覆盖**全部字段类型**（数值 + 字符串）——弄坏数值不动 ≠ 不检查（09 实证：真丢分项是枚举串，差分测错维度）。
- 每发提交前写一行：「此发确立什么、每个结果导向什么」；**禁止 retry 循环**（失败 = 停下列线诊断，不为看同样错误再付一次 quota）。
- harbor 粗档化时小差分不可分辨 → 换"大幅弄坏"（09 弄坏 C3 整块 weight 8 预期掉 0.08）或结合判断。
- 修复后提交验证 1 发，确认增量。

## 输出（decomposition_ambiguity 段）

```
题: <slug>
判官分解等价解: <解1 (conf) / 解2 (conf) ...>
判别性 A/B 设计: <每个解能被什么提交实验区分>
悬空字符串字段清单: <枚举类字段 + 核对状态>
提交级裁决: <attempt_id, 弄坏字段, harbor 增量, 结论(受检/未受检/口径锁定)>
```

## 坑

- **差分测错维度**：弄坏数值不动 ≠ 不检查——先核对字符串枚举字段（丢分高发区，09 教训）。
- **本地自检全对 ≠ 判官过**（self-referent 盲区）：本地自检含独立参考仍全对，判官仍丢 → 用 clean-room 独立参考（见 `red-team-review`）曝光约定差异。
- **判官分解是推断**：judge/红队的 N/D 分解（如"H1 Bell-8 丢"）是推断不是事实——提交级差分优先（09 29180 best-of-16 直接证伪 H1）。

## 与相关技能的关系

- `differential-scoring` = 定位**哪个分量**掉分（本技能是分量内的**字段级**审计与裁决）。
- `oracle-probe` = 阶段 A 之前的只读诊断（判词/scorecard 明细，零 quota）。
- `red-team-review` = clean-room 独立参考（从题面从零重写，曝光字符串约定差异）。
- `platform-scorecard-analyze` = 判分器类型识别与 A/B 框架（本技能是其 Step 5 的字段级深化）。

## 实证

- 09：29180（best-of-16 证伪 H1）→ 29181（C3 破坏性差分，弄坏数值不动）→ 29183（修复 mechanisms 枚举串 → **100 满分**）。
- 01：29176（R_cell vs R_RIR 口径 A/B，harbor 0.6 < 0.77 → 判官按实现口径）。
- 03：29179（Cu27Pd28_ico 近简并 tuple A/B，DROP 3.125 → gold=baseline）。
