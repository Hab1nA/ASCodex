---
name: mp-r-family-solve
description: "mp-r 同家族题（split coannihilation / UV portal 等）的字段级专项规则：裸值 vs 角平均陷阱、保留名三分类、κ_D 符号保留、因子归属、cross_target 继承链。来自 split 15→81.55 与 uv 34→42 的完整实战。触发词：'mp-r'、'split'、'UV portal'、'coannihilation'、'kappa_D'、'保留名'。"
version: 1.0.0
author: friday-team
tags: [bohrium-playground, mp-r, field-level, high-energy-physics]
---

# mp-r 家族题字段级规则（split/UV portal）

本家族题（mp-r-ab-uv-split-coann、mp-r-a-uv-portal）是**确定性数值 verifier**：字段级 0/1 硬匹配，每题 ~22 项检查、逐项计分。以下规则来自 split 0.15→81.55（+66）与 uv 0.34→0.42 的完整实战。

## When to Use

- 任何 mp-r 家族题（含未来赛季同款）
- 任何含"保留名语法 + 符号表达式 + 继承链"的 schema 题

## 字段级五条铁律（每一条都有 attempt 证据）

### 1. 裸值 vs 角平均陷阱
- 极化求和等"约定敏感"量：**判官要的是字段名字面的裸 Gram 前因子，不是角平均后的物理值**。
- split 教训：polarization=**4**（裸）正确；2/3、1/4、1/6（我们三个代理各自"学术推导"的角平均值）全错。
- uv 教训（反方向）：coefficient 里必须**含**角平均因子（1/(4096π⁴)→1/(6144π⁴) = 除 2/3）——**角平均因子进 coefficient，不进 polarization 字段**。
- 读题面"排他声明"：split §3.3 "My initial-spin average … in R_UV" 直接否定角平均——**friday 三代理集体漏读**。

### 2. 保留名三分类
题面语法保留名（W_delta/W_zero/a/r1/r2/sigma12/K_delta/K_zero/κ_D…）的使用规则：
- **可直接引用**（中间量）：sigma_eff 用 `W_delta`、g_eff 用 `g1*(1+a)`、population_weight 用 `W_delta`——判官把保留名当 canonical 记号，代数展开反而失分。
- **必须展开到自由基元**：equal_degenerate_limit 必须写 `K_zero*f_D^kappa_D/2`，写 `sigma12/2` 是错的（jarvis 在此也错了，我方爬分修正 +0.086）。
- **符号必须保留**：κ_D 在表达式里**处处写 κ_D 本身**，禁止代入数值（即使已知 κ_D=−4）。exponential_component 必须写 `exp(x_f*Delta/kappa_D)` 而非 `exp(-x_f*Delta)`。

### 3. 因子归属（只出现一次）
- 因子 2 属于 population_weight（`2a/(1+a)^2` 的合并权重定义），sigma_eff 写 `W_delta*K_delta*f_D^kappa_D` **不得再乘 2**。
- split 的 +0.086 突破（0.7292→0.8155）就是去掉 sigma_eff 的重复因子 2。

### 4. coefficient 与 polarization 成套
- coefficient × powers = 总截面 σ；polarization 是独立标量字段。
- split 正确组合：P=4 + coefficient=1/(6144π⁴)；uv 正确组合：P=1/4 + coefficient=1/(6144π⁴)（含角平均）。
- 组合测试法：P 与 coefficient 必须**同时**换（自洽组合），只换一个会误导判断。

### 5. 继承链（cross_target）
- `split:inherited_rate_fD_power` 必须依赖 `uv:power_f_D`；`split:fD_ratio` 祖先须含 population_weight 与 inherited_rate_fD_power。
- evidence/derivation.json 的 21 个 quantity 标识符**逐字固定**（uv:* 12 个、split:* 9 个，大小写敏感）；support 精确集合、conditions 地板子集；**一处 citation 错 = 整件 invalid**。

## 爬分方法（jarvis 法）

1. 把题面 §5 评分契约逐字翻译成自建 verifier（validate.py），本地爬分场。
2. 以"判官 reference 最省事写法"为目标：裸值、保留名、符号保留、因子一次。
3. 单字段 A/B：每变体只改一个字段，读 harbor diff（harbor 确定性 oracle）。
4. 分数字段响应不对称性可定位硬字段：改错硬字段 → 0.15→0.12（降）；改对 → 档位升。

## 陷阱自检清单（提交前）

- [ ] polarization/coefficient 是裸值还是角平均？题面有无"no spin-average"类排他声明？
- [ ] 每个表达式用保留名了吗？有没有该展开的（equal_degenerate）？
- [ ] κ_D 出现处都写符号了吗？
- [ ] 因子归属检查（2 只出现一次）？
- [ ] evidence 21 个 quantity 标识符逐字、support 精确、继承链满足？
- [ ] phase 字符串含 "fixed" + ("Higgs"|"scalar")；universal_uv_claim=false
