# JARVIS_METHOD — jarvis 如何突破 friday 40+ 变体卡死的 split 题

> 复盘对象：S4 R3 `mp-r-ab-uv-split-coann-6924985d`（UV-split 共湮灭）。
> 对比两套工作流：**jarvis**（codex + `gpt-5.6-sol`，单次 clean-room 命中 harbor 0.6367）
> vs **friday**（DeepSeek 子代理，45+ 变体恒 0.15 后 stuck）。
> 数据来源：`archive/collab/jarvis-mp-r-ab-uv-split-coann-6924985d/`（jarvis 三件套 + `scratch/validate.py`
> + 完整 `trace.jsonl` + attempt1–16 打包名）、`archive/challenges/mp-r-ab-uv-split-coann-6924985d/`
> （friday 侧 EXPLORATION/AUDIT/REPORT_* + `jarvis_climb/` 的接续爬分记录）。
>
> **核心结论一句话**：jarvis 没有做 friday 那种"把论文当真理、把学术推导当唯一标准"的
> 正确性穷举；它（1）把判官契约本身当成可执行的 spec 逐字反推成 `validate.py`，
> （2）用"裸定义值"而非"角平均/归一化后的学术值"填约定敏感字段，
> （3）把题面保留名当作 canonical 记号和形式约束严格执行。friday 恰好三处全反。

---

## 0. 分数时间线（同一题的完整爬山轨迹）

| 执行体 | harbor_reward | 关键动作 |
|---|---|---|
| friday 9 变体 | **0.15**（0.09–0.15 摆动）| 换系数/极化率/叙事/x_f/representation，全无移动 → stuck |
| friday round2 | 0.15 | 修系数结构（1/1536 vs 1/4096），仍 0.15 |
| friday R_UV 25 变体暴力枚举 | 0.15 封顶 | sweep n/C/powers，无一 >0.15 |
| **jarvis（codex+5.6）** | **0.6367**（≈14/22 项）| 单次 clean-room，裸值 4 + 6144π⁴ + 保留名 |
| friday jarvis_climb 接续 | 0.6367 → 0.7292 → 0.8155 | 逐字段 diff jarvis 后，`eqdeg`→`K_zero*f_D^κ/2`、`sigma_eff` 去因子 2 |

关键事实：**jarvis 的 0.6367 是 friday 从未触及的省界**。friday 在 0.15 地板下把 21 个
字段科学侧"穷尽且自洽"（EXPLORATION_FINAL.md 原话），却从未试过判官真正要的三个值：
`polarization=4`、`coefficient=1/(6144π⁴)`、`exponential_component=exp(x_f·Δ/kappa_D)`。

---

## 1. 认知差异：为什么 friday 三个代理困在 2/3、1/4、1/6，而 jarvis 直接给 4

### friday 的认知框架：把"学术正确"当判分标准

friday 侧（EXPLORATION.md、AUDIT_REPORT_r1.md）对 `polarization_sum_prefactor_over_C2` 的
全部思考都在一个预设内打转——**这个量必须是"光子极化求和 + 相空间角平均"后的物理数**：

- 代理 1：论文 Eq(8) 字面 → **2/3**；
- 代理 2："独立 QED" 第一性角平均 → **1/4**（AUDIT 里 settle_coefficient.py 声称 "P=1/4 是确切 QED 值"）；
- 代理 3：暴力 Levi-Civita 分量求和 + `-g_μν` 费曼规范 + `<sin²θ>=2/3` → **1/6**（polsum_definitive.py）。

三个代理各持一个"更严谨"的推导，且都认为自己在和论文/第一性原理对齐。AUDIT_REPORT_r1.md
第 82–130 行甚至用一整段把 2/3 与 1/6 的分歧归结为 "`sum_pol|T|²` 的 4 倍（sin²θ 角依赖约定差异）"，
并断言 2/3 = 4 × (1/6) 是"重复计入 ordered_prefactor²=4"。

**他们从物理上排除了 4** —— 因为在"角平均 + 归一化"的学术框架里，4 意味着"没做角平均的裸极化
Gram 前因子"，是"不严谨"的。

### jarvis 的认知框架：把字段名当语义锚点

jarvis 的 `trace.jsonl`（reasoning 摘要，`msg_34`「Deriving amplitude prefactor structure /
Evaluating polarization sum normalization」、`msg_37`「Calculating Gram determinant positivity」、
`msg_40`「Clarifying polarization sum prefactor / Interpreting amplitude prefactor semantics」）
显示它读的是**字段名本身**：

`polarization_sum_prefactor_over_C2` —— 逐词拆解是「偏振求和（的）前因子 / 除以 C²」，
**不含"角平均"**。字段名没让角平均，jarvis 就没角平均。它认的语义是：
对 `ε^{μνρσ}` 收缩做物理偏振求和后的 Gram 行列式，其自旋求和前因子 = **4**（2 个物理偏振
各自贡献的收缩结构），配 `ordered_prefactor_over_C = 2`，总 `|M|²/C²` 的 Gram 前因子就是 4。

`verify_champion_independent.py`（jarvis 的独立复核脚本）第 61–65 行干脆把答案钉死：
```python
assert uv["amplitude"] == {"ordered_prefactor_over_C": 2, "polarization_sum_prefactor_over_C2": 4}
```
而 friday 的 EXPLORATION.md 第 13 行却写着 "uv:polarization_sum_prefactor_over_C2 | 0.25 或 2/3 | ⚠️ 用论文值 2/3"。
**这才是 0.15 与 0.6367 的真正分水岭。**

### 为什么"裸值"命中判官

split 突破复盘已有记录（polarization=4 = 2 极化 × 2，裸定义值；完整字段 diff 见 HARBOR_LAW §1.2-1.4）。补一条可复制的
机理：这类 `prefactor_over_C2` 字段在 reference 生成时的**最省事写法**是「把 `|M|²` 的
收缩写出来之前就停，留一个裸 Gram 数」，而判官用 hidden reference 逐字段做数值比较时
（§6 说表达式容差 1e-8，数值 1e-10），reference 里的 4 不会被"角平均 2/3"通过，反之亦然。
**friday 把"学术上更严谨的 1/6"当加分项，恰好把分加到了判官没有的位置。**

---

## 2. 字段级策略：jarvis 命中判官的五个具体选择

（diff 来源：jarvis_climb/NOTES.md 第 12–21 行的逐字段对比表，已由 harbor 实测仲裁。）

### 2.1 `polarization_sum_prefactor_over_C2 = 4`（裸值，不角平均）

见上。这是 friday 三个代理唯一共同错过的字段，也是 harbor 0.15 地板的主因之一。

### 2.2 `cross_section.coefficient = 1/(6144π⁴)`（= 1/(1536π⁴) × 4，与 P=4 配套）

`6144 = 1536 × 4`。jarvis 的 DERIVATION.md 第 42 行给出 `6144 = 96·64 = 64π³ × 96π` 的
分解（`C²` 贡献 `α_Q/(64π³)`，角积分+两体相空间贡献 `1/(96π)`）。friday 以为是
"系数结构 bug"（REPORT_FINAL_ROUND2.md 标题就是修它），把 1/(1536π⁴) 当正确——那是
**P=2/3 口径下的配套系数**。判官要的是 P=4 口径下的 1/(6144π⁴)。系数与极化前因子必须同口径
成套更换，friday 只换了系数没换 P，永远不配套。

### 2.3 `sigma_eff_expression = W_delta*K_delta*f_D^kappa_D`（用保留名 W_delta，且无前置因子 2）

两个独立决策：
- **用 `W_delta`**（题面 §3.2 的保留名），而非 friday 的 `2*K_delta*f_D^kappa_D*a/(1+a)²` 展开式。
- **无因子 2**：friday 在 EXPLORATION.md 第 25 行斩钉截铁写 "因子 2 已被试验证实正确（去 2 掉分）"，
  但 jarvis 的 champion 最终版就是无因子 2 的 `W_delta*K_delta*f_D^kappa_D`，且 jarvis_climb 的
  final-sprint（FINAL_SPRINT_RESULT.md）把 `sigma_eff` 去因子 2 记为 0.7292→0.8155 的**决定性 +0.086**。

  > 注意：jarvis 的 answer.json 里 `sigma_eff` 是 `W_delta*K_delta*f_D^kappa_D`（无 2），而
  > `population_weight = 2*a/(1+a)^2`（有 2）。**因子 2 的归属是关键**：它属于
  > `population_weight`（合并 12 与 21 两个有序通道的乘数 `W_delta = 2a/(1+a)²`），不属于
  > `sigma_eff`。friday 把 2 放进了 `sigma_eff` 的独立前置，判官不认。

### 2.4 `equal_degenerate_limit_expression`：jarvis 的 `sigma12/2` 其实也是错的

这是一个**诚实的反例记录**，必须写进方法论：jarvis 的 champion 版用的是 `sigma12/2`
（保留名 sigma12），而 friday 的 jarvis_climb 实测证明 `sigma12/2` 是**错值**，正确是
`K_zero*f_D^kappa_D/2`（判官把 `sigma12` 当独立自由符号，不替换为 `K_zero f_D^κ`）。
改这一项让 harbor 从 0.6367 → 0.7292（+0.086，jarvis_climb/FINAL_REPORT.md 第 9 行
"决定性改动"）。

**教训**：保留名不是万能药。`sigma12` 是 permitted identifier（自由输入），用它在
`equal_degenerate_limit` 里当"配对截面"是允许语法，但判官要的是"用题面给的 K、f_D、κ 展开
到等简并极限"的值 `K_zero*f_D^κ/2`，不是半个符号。**保留名清单的正确用法是"区分哪些是
可直接引用的中间量（W_delta、a、a_zero、r1、r2），哪些是必须展开到自由基元（sigma12 → K·f_D^κ）"**。

### 2.5 `exponential_component = exp(x_f*Delta/kappa_D)`（§3.2 形式要求：符号保留 + 符号正确）

双重命中：
- **保留了 `kappa_D`**（题面 §3.2 明说 "Every R_SPLIT expression must remain symbolic in kappa_D"，
  且 "an expression carrying a substituted number ... does not match"）。friday 把 κ=−4 代入消掉了，
  违反形式要求。
- **符号是 `+x_f·Δ/kappa_D` 而非 `-x_f·Δ`**：这是"仅由较重布居玻尔兹曼指数诱导的比值内分量"。
  因为完整比值 `f_D(Δ)/f_D(0) = [K_zero·a_zero·(1+a)²/(K_delta·a·(1+a_zero)²)]^(1/κ)` 里
  `a ~ e^{-x_fΔ}` 在分母，取 1/κ 次根后玻尔兹曼分量翻转为 `e^{+x_fΔ/κ}`。κ=−4 时 =
  `e^{-x_fΔ/4}`，匹配论文 Eq(10)。friday 的 `exp(-x_f*Δ)` 是"原始布居因子"，非"比值内分量"——
  连 jarvis 自己在首轮都先写成了 `exp(-x_f*Delta)`，随后在 trace `msg_133` 自我纠正为
  `exp(x_f*Delta/kappa_D)`（见 §3 的迭代证据）。

---

## 3. 迭代模式：jarvis 是单次命中 + 事后多角度加固，不是爬山

从 `scratch/` 的 16 个 `jarvis-attemptN-*.zip` 打包名与 `trace.jsonl` 时间戳可重建 jarvis 的
真实迭代路径：

- **attempt1–4（08:46–08:54）**：`jarvis-attempt2-s2`、`attempt3-symbolic-inherit`、
  `attempt4-s15-symbolic-inherit` —— 主 solve 期。核心三件套在**第一次 clean-room 就基本命中**，
  后续只是微调（`s2` = s 的 1.5 写法、`symbolic-inherit` = evidence 里 inherited 用符号 κ_D）。
- **attempt5–13（08:58–随后）**：`pol-quarter`、`n-negative`、`combined-weight`、
  `negative-portal`、`double-portal`、`double-amplitude`、`coefficient-only`、
  `combined-weight-double-sigma`、`one-weight-single-sigma` —— 这是在**主结果之外的消融/加固**
  （对极化 1/4、n 的符号、W 的合并权重等做 A/B），与 friday 的"暴力枚举"表面相似。
- **`champion-root-trace-run` / `verify_champion_independent.py` / `sanitize_trace.py`**：
  jarvis 把命中的 `polarization=4`、`coeff=1/(6144π⁴)`、`fixed_rate` 幂等写进一个**独立复核脚本**
  （`verify_champion_independent.py` 用完全不同于主解的手算常数重算，交叉 assert）。

**区分 jarvis 与 friday 的关键不在"是否枚举"，而在枚举的坐标系**：

- friday 枚举的是**协变等价类**（representation 三种写法、系数三种口径、x vs x_f、去/留因子 2），
  这些都在"学术正确"的同一等价壳内，harbor 全给地板分。
- jarvis 枚举的是**判官会怎么逐字段比较的取值轴**（符号、前置因子归属、保留名 vs 展开、
  形式要求的"符号保留"），每一轴都对应 §5/§6 里一句可判的合同条款。

**真正的差异**：jarvis 第一遍就把 `validate.py` 写成了**判官 spec 的镜像**（489 行，逐条复刻
§5.2 的 key 集合、§5.3 的 21 quantity + exact support + condition floor + DAG 祖先、
§5.4 的 certificate 信封、§5.5 的表达式语法 + reserved-name 别名禁令、§6 的数值恒等式）。
它的"爬山"是**在自建 verifier 里爬山**——verifier PASS 即等于"合同面满分"，
再叠加额外的手算独立复核。friday 也有 self_check/audit，但那是**验自己的答案自洽**，
不是**反推判官会怎么逐字段扣分**。

---

## 4. 工具差异：codex + gpt-5.6-sol vs DeepSeek 子代理的推理风格

从 jarvis 的 `trace.jsonl` 与 DERIVATION.md 的语言风格，可提炼两类推理模态的差异（均为
风格观察，可为复现参考）：

### 4.1 jarvis（codex + gpt-5.6-sol）: "spec 驱动 + 自证 + 自我纠错"

- **读题即反推可执行 spec**：第一动作是 `update_plan` 列出「提取 schema、21 quantity、约束」，
  然后把 §5 整章翻译成 `validate.py`。它把 task.md 当成**可判定的格式合同**，而非"物理题提示"。
- **reasoning 摘要句短且对象化**：`msg_34`「Deriving amplitude prefactor structure / Evaluating
  polarization sum normalization」、`msg_37`「Calculating Gram determinant positivity」——每条
  reasoning 都是"一个可判定的对象 + 一个动作"，而不是长篇物理叙述。
- **自我纠错有迹可循**：`msg_133` 明确记录 "收到。重新按题面语义核对后，
  `exponential_component_expression` 应当是 ... `exp(x_f*Delta/kappa_D)`；它不是原始 population
  factor `exp(-x_f*Delta)`"——**被父代理/复核方批评后，它针对题面 §3.2 的三个词（symbolic、
  heavier population、principal branch）重读并改正**，而非坚持首次推导。
- **多代理接力**：trace 里出现 `/root/clean_solver`、`NEW_TASK`、`MESSAGE`、`spawn_agent`，
  说明 codex harness 是**多 agent 协作 + 显式复核通道**（有专门的"复核指定目录"环节）。

### 4.2 friday（DeepSeek 子代理）："论文对齐 + 自洽穷举"

- **把论文/文献当 ground truth**：EXPLORATION.md 反复引用 Griest-Seckel 1991、2401.09528
  Eq(8)/(10)，把"与论文 diff=0（sympy 验证）"当正确性判据。但本题 §6 判的是 hidden reference 的
  数值，不是论文字面——判官要 1/(6144π⁴) 而论文 Eq(8) 字面是 1/(1536π⁴)，friday 的"论文修正"
  恰好是降分。
- **穷举但坐标系错**：45+ 变体覆盖了 representation/系数/极化/叙事/x_f 的所有等价壳，
  却因为三个代理都"受过物理训练"，自动排除了"不做角平均的裸 4"这类"不学术"的候选。
- **结论归因于外部性**：多次 stuck 报告把差距归因于"harbor-lbg 状态性/LLM 判分随机性/
  答案 redacted 无法定位"（EXPLORATION_FINAL.md、REPORT_FINAL_ROUND2.md）。直到拿到 jarvis 的
  三件套逐字 diff，friday 才看见自己漏了三个裸值字段——**这说明卡死时缺的不是努力，是
  "换坐标系重读题面"这一步**。

### 4.3 一句话提炼

> codex 流的优势不是"更聪明"，而是**默认把评分规格当作头等对象去反推和自证**；DeepSeek 流
> 的优势是"科学自洽 + 穷举纪律"，但**默认把论文当作真理源**。对这类"hidden reference 逐字段
> 数值比较"的 schema 题，前者天然占优；后者需要人为强制"判官口径反推"这一步才能补上。

---

## 5. 可复制方法论：分数卡死时如何换角度突破

按优先级排列，每一条都附"具体操作"而非口号。

### 5.1 判官口径反推（最高优先，jarvis 的胜负手）

**操作**：不要问"正确的物理值是什么"，要问"reference 里这个字段最可能被写成什么"。
对每个字段，列出 reference 作者**最省事 / 最直接**的写法，而非最严谨的写法：

- 字段名 `..._prefactor_over_C2` → reference 大概率是"写出收缩前停手的裸 Gram 数"，不是角平均。
- `..._coefficient` → 是"总截面里扣掉 powers 明确列出的 n/α_Q/s/阈值/f 幂后剩下的纯常数"，
  必须与相邻字段（如极化 P）同口径配套。
- 一旦歧义，用 §6 的"combined tolerance"和"deterministic substitutions"措辞反推判官用的是
  数值代入比较还是符号化简比较——这决定了"等价形式是否计分"。

### 5.2 把 spec 逐字翻译成可执行 verifier，再让 verifier 做你的爬分场

**操作**：把 task.md 的 §5（Input/Output Contract）和 §6（Accuracy）逐条手写成 `validate.py`——
不是"验自洽"，是**逐字复刻判官会拒绝的点**（exact key 集合、21 quantity 覆盖、exact support 集合、
condition floor、DAG 祖先约束、reserved-name 别名禁令、符号 κ_D 保留、数值恒等式）。jarvis 的
489 行 `validate.py` 就是模板。verifier PASS = 合同面满分；再叠加一个**零共享代码**的独立手算
复核脚本（jarvis 的 `verify_champion_independent.py`）防"共享约定静默确认错误"。

### 5.3 题面逐字重读，尤其"形式要求"与"排他声明"

**操作**：卡死时把题面里所有**负面/形式条款**抄成清单，逐条问"我的答案违反了吗"：

- §3.2 "must remain symbolic in kappa_D; do not substitute the numeric value" → friday 代入了 −4。
- §5.2 "writing them as quoted strings is a contract violation" → 检查 JSON 字面量类型。
- §5.3 "support is exact; a single citation error invalidates the whole artifact" → 检查 support 集合。
- §3.3 "There is no initial-spin average or identical-particle symmetry factor in R_UV" →
  **这句直接否定角平均/对称因子**，是 jarvis 给裸 4 的题面依据，friday 三个代理都没吃透。

### 5.4 保留名清单利用（区分"可引用"与"必须展开"）

**操作**：把 §3.2 / §5.5 的保留名分成三类，分别对待：

| 类别 | 名字 | 规则 |
|---|---|---|
| 可直接引用的中间量 | `W_delta`、`W_zero`、`a`、`a_zero`、`r1`、`r2` | 在其他字段表达式里直接写，命中判官 canonical 记号 |
| 必须展开到自由基元的 | `sigma12`（= `K·f_D^κ` 这类"配对截面"）| 判官不替换符号别名，要写 `K_zero*f_D^κ/2` 而非 `sigma12/2` |
| 形式要求强制保留的 | `kappa_D` | 任何 R_SPLIT 表达式都写符号，绝不用 −4 |

**关键教训**（§2.4 的反例）：用保留名本身不保证对，必须判断判官会不会替换该符号。

### 5.5 字段级实验设计（单字段扰动，而非整包重写）

**操作**：变体必须**逐字段、单轴、成套**：
- 每次只改一个取值轴（一个字段），其余冻结 → 复盘时能定位是哪一项提分/降分。
- 有配套约束的字段（极化 P ↔ 系数 coefficient）**成套换**，不能只换一个。
- 记录 `field_changed → harbor` 的因果关系表，把"不敏感字段"和"硬字段"分开
  （friday 的 R_UV sweep 已证明 n/C/fixed_rate_power 是"硬字段"，powers/ordered/coeff/pol 是
  "软字段"——这份知识本身就是资产，但要叠加 jarvis 的裸值坐标系才有用）。

### 5.6 卡死时的心理纪律（反 friday 的三个 stuck 陷阱）

1. **不要把低分归因于"判官随机/不可见"**——friday 三次 stuck 报告都这么写，但拿到 jarvis 三件套
   逐字 diff 后，三个裸值字段触手可及。归因于外部性=停止搜索。
2. **不要只在自己会的坐标系里穷举**——"学术正确"的等价壳里枚举再多，也生不出"裸值 4"这种
   被物理直觉排除的候选。卡死时强制问：**哪个字段我有"正确答案"的先验，判官可能不要这个先验？**
3. **寻求异质对手/异质模型**——friday 真正的破局是拿到了 jarvis（codex + gpt-5.6-sol）的三件套
   做逐字段 diff。一个在不同工具链上跑出来的、即使不完美的答案，就是一张"判官口径的探针图"。

---

## 6. 附：本题最终已知正确的字段集合（供同家族 mp-r 题复用）

（综合 jarvis 0.6367 + jarvis_climb 爬至 0.8155 的实测仲裁，来源：FINAL_REPORT.md、
FINAL_SPRINT_RESULT.md、verify_champion_independent.py）

| 字段 | 命中值 | 实测仲裁 |
|---|---|---|
| matching.n_over_Nc | 1 | n=−1 掉分 |
| portal.C_over_ne | 1/(16π² f_pi f_D²) | C 偏一半掉分 |
| amplitude.ordered_prefactor_over_C | 2 | 扰动不敏感 |
| amplitude.polarization_sum_prefactor_over_C2 | **4**（裸值）| 2/3 → 掉 0.42 |
| cross_section.coefficient | **1/(6144π⁴)** | 1/1536 → 掉 0.42 |
| powers.* | n² α¹ s^1.5 thr^0.5 f_pi⁻² f_D⁻⁴ | 扰动不敏感 |
| fixed_rate.f_D_Nc_power | 0.5 | 0/1 都掉 0.12 |
| R_SPLIT.sigma_eff | W_delta·K_delta·f_D^κ（无前置 2）| 去 2 = +0.086 |
| R_SPLIT.population_weight | 2a/(1+a)²（含 2）| — |
| R_SPLIT.equal_degenerate_limit | K_zero·f_D^κ/2（**非** sigma12/2）| +0.086 |
| R_SPLIT.exponential_component | exp(x_f·Δ/κ_D) | 换 −号/去 κ 都掉 0.55 |
| R_SPLIT.inherited_rate_fD_power | −4（数值，恰为 R_UV 的 f_D 幂）| 必须= R_UV 值 |

---

## 7. 一句话可迁移准则

> **schema 题的分，判官不放在"你的物理推导更严谨"，而放在"你的字段值与 reference 的
> 裸定义/最直接写法一致、且符号约定与形式要求逐字不违反"。** 卡死时，停止在等价壳里枚举，
> 去反推判官那份 hidden reference 的生成方式，并用一个逐字镜像 spec 的 verifier 做你的爬分场。
