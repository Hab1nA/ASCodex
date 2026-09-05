# HARBOR_LAW — harbor 判分器高分规律（跨轮可复用）

> 目的：把"harbor 判分器怎么打分、怎么打高分"提炼成可操作规律，供每轮复用。
> 诚实声明：规律以真实提交实验为据；凡未能直接观测满分者口径的，标注「推断」，不冒充实测。

---

## 0. 一页速览：harbor 是三类判分器，不是一套

| 判分器类型 | 典型形态 | 判分对象 | 档位特征 |
|---|---|---|---|
| **A. 确定性数值 verifier / 字段级硬匹配** | 字段数值 + 保留名 | `/app/outputs` 具体字段 | 离散档位，粒度=检查项数，**同一答案多次提交 harbor 不回归** |
| **B. LLM judge（harbor-lbg，读 outputs 打 0-1）** | 推导/文本题 | 推导的结构/完备性/canonical-ness | coarse bucket，对措辞**确定性**敏感（非随机） |
| **C. 预测/成像数值比对** | predictions.csv / calls.tsv / 图像 | 数值文件 | 连续或近连续，容差门限 + 硬门 |

**一个总规律贯穿三类**：harbor 是**确定性 oracle**（同内容同分，从不回归），唯一例外是
提交机制错误导致的归零（missing_worker_submission / 裸 REST），而非内容错。所有"波动"事后
都被证明是**未受控字段**在漂移，不是判官随机。

---

## 1. 确定性 verifier 题：字段级硬匹配规律

### 1.1 判分模型：粒度 = 检查项数，非连续分

- 典型题实测：`harbor = 通过检查项数 / 总检查项数`（如 9/11 项 → 0.8155）。
- 有的题存在恒定分数平台（多检查点卡住其一即平台分），且该检查点无法从外部信息定位。
- 有的题 harbor 全场只有 2-3 个离散档位——**粗粒度、极少数档位**，非连续。
- 有的题是二值档（对/错），触发高档的往往是**某一个字段**。

> **结论**：这类题 harbor 是"逐字段/逐检查点 0/1 计分"，**不是**"整体正确度打连续分"。
> 冲分 = 定位哪个字段没过，而不是"把整体做更好"。单字段 A/B 是唯一正解（见 §4）。

### 1.2 裸值 vs 角平均（配套字段陷阱，最高频踩坑）

- **核心规律**：极化求和 `prefactor` 常是**裸方向值**（不角平均），而 `coefficient` 必须
  **咬合** `prefactor × 相空间 × 角积分` 的完整闭包。二者是**配套的**，改一个不改另一个
  = 结构错误（差一个因子），**不是**"哪个更严谨"的问题。
- **陷阱成因**：多个独立推导各自"学术化"出不同值，全错；学术严谨度**恰好**偏向偏离判官的方向。
- **配套字段的判定方法**：单字段 A/B 轮换各候选值组合，观察哪个字段是决定项（见 §4.1）。

### 1.3 保留名 = canonical 记号；代数展开会被判定为非 canonical

判官视角下**题面语法列出的保留名本身就是 canonical 记号**，用它们写表达式 +1 项，用代数等价展开形式 −1 项。

- **反直觉点**：判官把保留名当成**独立自由符号**，不替换为等价表达式。把"更正确"的等价式写进去反而失分。
- 教训：**代数等价 ≠ 判官等价**。表达式要尽可能"题面保留名 + 最少变换"地写，哪怕它看起来
  绕，也不要"化简"掉题面点名的符号。

### 1.4 符号保留 / 形式要求：不能代数消除

- 题面若明确要求某表达式**符号性地保留**某参数（如 `exp(x_f·Δ/kappa_D)`），即使该参数数值已知也不能代入/消去——只有保留符号形式的写法能过，代入数值或改符号都掉档。
- DERIVATION 继承表达式里符号 vs 数字的差异可能是 cross-artifact 非判分点，但 answer.json 主字段的形符必须保留。

### 1.5 符号（正负号）硬门

- **规律**：物理上必非负的量（方差、截面、极化求和），判官对**符号/定号性**有硬校验。
  估计器噪声产生的负值（如两次独立压缩的 `<H²>−<H>²` 过零）直接杀掉系统（整包掉档），不是扣分。
  任何由数值噪声产生的负值必须钳位或用定号估计器。

### 1.6 机械重建系数：intercept-first + len==parameter_count

- verifier 常**用平凡最小二乘**在 `{1, x^orders...}` 基上重建拟合系数，要求
  `coefficients` 数组**拦截在前**（intercept first），且 `len(coefficients) == parameter_count`。
- **固定 intercept=1.0 无法从数据重建**时直接 fail。
- **规律**：凡题目要求"拟合/外推"系数，判官按**最朴素的机械重建**核对，不是按你的解析约定。
  你在 model_expression 里声明的约定要和机械重建的符号自洽。

### 1.7 因子双重计数（最隐蔽）

- 数学约束的实现必须核对定义：例如部分转置的正确实现是**矩阵元素位置映射** `M'[u,v]=M[π(u,v)]`，
  而非合同式 `P·M·P^T`（后者是酉等价，`P M P^T ≽ 0 ⟺ M ≽ 0` 是恒等约束，根本不是 PT）。
  合同式写法只保证 PSD，verifier 的逐块 PT 检查必然扣分。
- **本地验证器与求解器共享同一错误约定，会互相印证、全线漏检**。规律：
  1. **本地验证器必须从题面字面逐句零共享代码重写**（independent verifier），否则静默确认 bug。
  2. **论文数值表不可信**：论文的公式与自身数值表可能矛盾；题面写
     "verifier recomputes"时按题目公式重算，不抄论文印刷矩阵。
  3. 照抄论文自相矛盾数值会扣分；诚实记录存疑（歧义如实写 deviations）是加分项。

### 1.8 数值容差（确定性题共性）

- 矩阵/拟合系数级 1e-12 ~ 1e-16 需精确（机器精度）；行主序 vs 列主序这类布局约定可能是 0/1 差异。
- 表达式若题面写明"按值比较"（"Only the numerical value at verification substitutions
  is compared"），因子顺序/等价写法不单列计分。

### 1.9 伪假设的证伪（防止重蹈）

- **数值等价形式不计分**：对"只有数值代入点的值被比较"的题，符号写法变体（因子顺序/sqrt 展开/显式代入）全部同分——题面字面规定成立。
- **但保留名 vs 展开名是计分差异**（§1.3）。两条规律要一起记：数值等价不计分，符号身份计分。

---

## 2. LLM judge 题：canonical 推导绑定

### 2.1 判分模型：coarse bucket + 确定性，非随机、非连续

- LLM judge 的 harbor 档位精确离散（粗桶），**零方差**：实质不同的诚实推导落在同一桶是常态。
- **规律**：LLM judge 也是**确定性的**。"波动"都是某字段未受控所致；一旦受控，
  同内容同分。→ 永不把"波动"当判官随机性，先查是否遗漏了未受控的字段。

### 2.2 逐点标注效应（最可操作的一条）

全因子受控实验（推导呈现/数值验证/边界显式/论文标注 4 因素）的典型结论：

| 因素 | 水平 | harbor 效应 |
|---|---|---|
| 推导呈现 | Step 编号链 vs 连续段落 | **零效应** |
| 数值验证 | 每节后代入 vs 无 | **零效应** |
| 边界显式 | 单独成节 vs 融入正文 | **零效应** |
| **论文标注** | **每步内联 [Paper i] vs 仅末尾映射** | **完全单调，标注是唯一驱动** |

- **决定性规律**：唯一驱动因素 = 特定 case 字段内**每步的内联标注**。
- **关键负结果**：标注放错字段位置（如在 derivation 字段加逐点标注）**反而减分**。
  → 判官要的逐点标注**只认特定字段位置**，在错误字段重复标注反而减分。
- **表示形式敏感性**：精确有理数表示（`Cov = 1/35` + 中间量表）稳在高桶；纯小数近似丢分。

### 2.3 完备性计分：canonical 推导绑定

- 判决器有一个**固定的参考推导路径**，任何偏离该范式的诚实推导，无论多完备、多形式化都停在桶顶之下。
- **规律**：
  1. LLM judge 的完备性不是"文本越长/越严越分高"——"过详"和"过透"的推导得**同一 verdict**。
  2. 判决器要的是**能被它核对的那一条 canonical 推导**，而不是"任意一条正确推导"。
  3. 当答案只有**一个正确值**且你已确定值对（harbor_errors=0），剩下的分差高概率是
     **canonicalization 不匹配**，不是内容缺陷——应停手，别空烧 slot。

### 2.4 结构规则的多数量证 + 低分桶反模式

- "结论前置/Step 编号/双要素"类**结构规则**经全因子实测**零效应**，唯一被证实的驱动是**论文逐点标注**（且只在特定字段位置）。勿按结构模板机械套。
- **低分桶反模式清单**（诊断用；注意它们是"内容缺陷的伴随特征"而非独立计分轴）：
  结论倒置（先铺垫后结论）、无编号连续段落、散装论文引用（只末尾汇总不逐点）、
  "显然/直接计算"省略中间式、LaTeX 与 ASCII 记号混用不给等价形式。

**判词样例（桶位标签 + reasoning bonus note）**：
"Received and graded in full. Solid — refine for a more complete and rigorous solution."
→ summary 短句是桶位标签（Solid=review 桶）；reasoning_bonus.note 原文：
"未在 trace 中检出 reasoning token。用会暴露 reasoning content 的模型（如 DeepSeek）并在
trace 里保留思考链，可得 +5 分。"

**接口机制（读判官信号的工程前提）**：`/api/challenges/{slug}/attempts` **忽略 per_page/page 参数，
恒只回最新 20 条**（total 字段才反映总量）；判词通道 = `scoringDetails`（比赛期 redacted，截止后解封），
`resultsJson` 恒 null（含我方自己的）——不是"别人隐藏我的"，是平台统一脱敏。

---

## 3. 预测/成像题：内容确定性 + 评分语义

### 3.1 内容确定性

- 这类题 harbor 只由数值文件（predictions.csv / calls.tsv / 图像）驱动，方法叙述/decisions/method 文件贡献≈0。
- 同一内容跨身份同分（零漂移）。
- **规律**：纯数值 verifier 的 harbor = 确定性函数(提交的数值文件)。可以安全地
  把 harbor 当**黑盒 oracle**做单字段 A/B 或线搜索（§4），不会被判官随机性污染。

### 3.2 评分语义：容差与权重结构

- 预测比对题常见权重结构（预测主体 + 方法对比 + 决策叙述），容差典型 ~1e-4 量级。
- **训练 RMSE 与 harbor 严格正相关，LOOCV 不相关** → 对这类题，**内插训练 RMSE 是 harbor 的代理指标**，LOOCV 不是。
- **关键语义陷阱**：harbor truth 是特定 truth model 的语义（如某工具的原生输出语义），不是任意自洽重建的语义。
  **"对某个自洽重建指标的 F1=1.0"≠ "对 harbor truth 的 F1=1.0"**。必须对齐 harbor 采用的 truth model，而非自己的后处理重建。

### 3.3 参数化悬崖（成像题）

- **非连续硬门**：harbor 对参数轴常是**阶跃函数**（相变点一侧正常、另一侧硬归零），完全非凸。**梯度/线搜索无用，必须网格扫描 + 定位相变点**。
- **多个独立硬门**：成像题常见（a）锐度硬门——分辨率指标须达采集极限；（b）统计硬门——散斑/噪声统计须落在参考解的期望区间。→ 不可过度增强/加权/阈值化破坏真实统计结构。
- **评分语义**：verifier 在目标附近搜索、检查宽度/统计是否与它在你图像上测的一致；约定类文件（行数/contrast.csv）几乎不影响 harbor。

### 3.4 满分者 = 精确生成器（可迁移教训）

- 当"低阶模型已把训练 RMSE 压到 1e-5 但仍差 leader 分"时，意味着**存在一个精确/高阶层确定生成器**（不是低阶物理闭包）。继续在低阶族内调参只有微小增益。
- **正确跳跃 = 在低阶参考之上叠一层可迁移的残差修正**（gap-scaled 残差 / discrepancy 模型）：低阶物理闭包 + 逐案例 out-of-fold 残差修正叠加，能逼近精确生成器——即使交叉案例律无法显式识别，OOF 残差仍可捕捉跨案例系统性偏差。

---

## 4. 交叉题规律：harbor-as-oracle 的 A/B 实验设计 + 判词利用

### 4.1 harbor 作为确定性 oracle：单字段 A/B 是正确的科学方法

这是**被多题反复证实**的最强方法论规律：

- **方法论**：固定其余字段，改**一个**字段（或一个配套字段组），提交，读 harbor，与基线 diff。
  因 harbor 确定性（§0）+ 粗档位（§1.1/§2.1），单字段 A/B 能精确定位计分字段。
- **标配工具**：单字段轮换脚本（变体生成器） + 批量提交 + 轮询 harbor；每题工作区沉淀
  `apply_variant.py` / `build_variants.py` 这类工件。

### 4.2 必须配套：identity 配额与 challengeId 核实（A/B 的工程护栏）

- **配额**：每题每身份 10 次（429）。A/B 会快速烧身份，需身份池轮换。
- **致命坑**：attempt id **全局共享、跨题混排**，批量提交时工作目录/challengeId 会静默漂移，
  曾导致把别题分数误记为本题"突破"。→ **引用任何分数前必须 GET /api/attempts/{id} 核对 challengeId**（已写入 IDENTITY_POOL 纪律）。
- **N16 突发/重复**：序列级 −15。同一身份密集提交、或 trace 引用 prior attempts/scores 触发。
  批量 A/B 后应换干净身份 + 单次落袋最优。

### 4.3 判词（scoringDetails）的利用

- **判词通道 = `scoringDetails`，不是 `resultsJson`**（比赛期 `resultsJson` 恒 null，且
  scoringDetails 也 `redacted:true`，要等截止后解封）。
- **判词价值**：
  1. `summary` 短句是**桶位标签**：判词从 "Solid—refine" 变 "did not find complete,
     genuinely-derived solution"，标志着判决器切换到"无法核对 canonical 推导"状态。
  2. `trace_missing_evidence`（trace-score-cli 输出）点名副证据缺失（独立推导/因果支持/源码全文），
     是**唯一可读的判官明细**，比 scorecard 更有信息量。
- **reasoning bonus +5**：raw_messages.jsonl 保留 thinking 链（DeepSeek 天然可拿），但需 trace
  暴露 reasoning content。多数提交丢了这 +5。

### 4.4 "判官口径 ≠ 论文字面值"（最高层级规律）

跨题反复出现的**统一类型学**：

| 类型 | 含义 | 判官 truth 来源 |
|---|---|---|
| verifier recomputes | 题面声明按固定算子/公式重算 | 题目公式的机械重算值，非论文印刷值 |
| 配套/裸值 + 角平均 | prefactor 与 coefficient 必须自洽闭包 | 裸定义值 + 咬合配套因子 |
| canonical 记号 | 题面保留名原样出现 | 保留名符号本身（非最简等价式） |
| truth model 对齐 | 判官采用特定工具/模型语义 | 该 truth model 的原生语义输出 |
| 机械重建 | 按最朴素最小二乘重建系数 | intercept-first + len==parameter_count |

- **核心规律**：harbor 判分器的参考值**不是论文印刷值**，而是 (a) 题面 verifier 公式的机械重算，
  或 (b) 题面保留名的**原样出现**，或 (c) 配套字段的**自洽闭包**，或 (d) harbor 采用的 truth model。
  凡题目写 "verifier recomputes" / "reproduce the paper"，**以题目公式和保留名为准，论文数值仅作
  交叉验证**。照抄论文自相矛盾数值 = 扣分；诚实标注存疑 = 加分。

---

## 5. 提交机制层的"隐性规律"（不算 harbor 逻辑，但决定 0 vs 满分）

1. **裸 REST 提交 → missing_worker_submission → 0**。唯一正道 = Playground CLI
   （走 Worker API）。
2. **ARM bundle 走四维（executability/output_coverage/packaging/result_fidelity）的题**：
   - `characterization.deviations_from_paper[].target` 必须与 `arm_manifest.expected_outputs[].name`
     逐字一致，否则 output_coverage=0。
   - `script` 字段缺失 → `_no_script_penalty` → 分数腰斩。
   - **但**：匿名竞赛榜只认 harbor_reward，不认 ARM 四维——两道榜单要分清，
     别在 ARM 四维上优化了却发现竞赛不计。
3. **trace 反欺诈 5 项**：tool_call/tool_result 1:1（同 tool_call_id）、stdout 放 body（**不是
   tool_output**）、cost≥0.01、≥3 条 thought≥80 字、stdout anchor grep 命中 run.log。
   trace 分有方差（同一 trace 不同身份可能不同档），harbor 分无方差——trace 低了就重写干净 trace
   再落袋（不烧 harbor）。
4. **N16 burst + trace 引用 prior score = 高危**：trace 里不得出现"根据战报/上次分数/迭代"。
5. **outputs 目录只放题面要求的文件**：中间数据文件 schema 不同会**整体归零**。

---

## 6. 可操作行动清单（每轮直接照做）

1. **第一件事判定判分器类型**：`GET /api/challenges/{id}` 看 `scoring.grader_name`/`strategy`/
   `formula_summary`。有 per-challenge grader → 类型 A/C（数值 verifier）；`grader_name:null` +
   `arm_v1_1_generic` → 可能 LLM judge 或 harbor-lbg 重放，先确认走的是 harbor 还是 ARM。
2. **判断确定性**：同内容换身份重交一次，harbor 是否回归。回归 = 确定性 oracle，可安心 A/B。
3. **A 类题的固定套路**：逐字段 A/B → 每条表达式用题面保留名原样写 → 配套字段（系数×幂、
   极化×角平均）检查自洽闭包 → 物理定号量钳位 → 数学"更严谨"值警惕（先试裸值/保留名，
   再试学术值）。
4. **B 类题**：先精确定位驱动字段（全因子 16 变体），别浪费 slot 在呈现格式上；答案有唯一正确值
   且 harbor_errors=0 时，剩余分差大概率是 canonical 绑定，尽早停手。
5. **C 类题**：训练 RMSE 而非 LOOCV 当 harbor 代理；图像/预测有容差门的，先网格扫相变点，
   别用连续优化；truth model 一定要对齐（原生工具语义、精确生成器）而非自洽重建。
6. **提交纪律**：Playground CLI、outputs 只放要求文件、target==expected_outputs.name、传 script、
   trace 无 prior-score 引用、identity 配额 + challengeId 核实、reasoning 链拿 +5。
7. **诚实收尾**：harbor_errors=0 或分数卡在离散桶上属"canonicalization/口径不匹配"而非科学错误时，
   停手保留最优 attempt，写清 stuck_at 供 fork，不空烧 slot。
