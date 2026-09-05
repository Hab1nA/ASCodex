# TRACE_LAW — trace_score 决定因素与高分配方（权威复盘）

> **2026-08-28 覆盖声明**：平台反作弊已从旧"8 规则"改为加权计分并新增三个信号；无真实运行痕迹的 trace 不进入待评队列；旧 trace_score≥70 / 固定乘数公式不再作为现行平台契约。以下为历史复盘，仅供真实性硬门参考，不得据此断言新分数公式。现行口径以 `config/playground-scoring-audit-2026-08-28.md` 为准。

---

## 0. 一句话总纲（先记牢）

**trace_score 不是"过程写得越长越高的主观分"，而是"真实性 + 独立性 + 合法性"三轴
的确定性门控分。** 它的最终作用是把一个 0–100 的 `trace_factor` 系数乘到 `harbor_reward`
（答案分）上：

```
最终 score = harbor_reward(答案, 0–1) × trace_factor(trace_score 决定) × 100 + reasoning_bonus(+5)
```

- `trace_decision = blocked` → `trace_factor = 0.0` → **总分直接归零**（答案再好也没用）。
- `trace_decision = review` → `trace_factor ≈ 0.3–0.7` → 答案分被打折。
- `trace_decision = accept`（trace_score ≳ 70）→ `trace_factor = 1.0` → **答案分全额兑现**。
  （实证修正：门槛为 **≥70** 非 ≥80；69 即打 0.69 折。）

**结论：harbor_reward 是"能考多少"，trace_score 是"能拿到多少"。trace 低分 = 亲手把满分答案砍成 0。**

---

## 1. trace_score 决定因素排序（从最致命到可忽略）

### 1.1 【最致命】论文引用惩罚（"leaning on the paper"）

铁证级因果链（多题交叉证实）：**完整 trace 但大量引用论文方程 → 69 档（review）；
删除全部论文引用后纯独立推导 → 90+ 档（accept）**。

**机制**：harbor-lbg 把"推导过程中引用论文的中间结论"视为**抄袭/非独立推导**，触发
`N14_METHOD_SUBSTITUTION_OR_FALLBACK` 或直接判 review。判官要的是"**从题面 §3.1 输入
第一性原理独立推导**"，不是"论文说 X，所以我用 X"。

**规避铁律**：trace 的 body 里**严禁出现**：
- 论文方程编号（"Eq(5)"、"Eq4.11"、arXiv 号）；
- 作者名 / 年份（"Maliar et al. 2010" 类）；
- "benchmark value"、"published常数"、"paper says" 等文献值引用。

只写"我做了什么、stdout 是什么、结论是什么"。所有数值都要以**脚本真实 stdout** 的
形式出现（那是"我的执行结果"），而不是"文献给的"。

### 1.2 【决定性细节】stdout 必须放在 tool_result 的 `body` 字段

harbor 判分器**只读 `body`**。把真实 stdout 放到 `tool_output` 或任何别的字段 = 等于
没有执行证据。

- 铁律：`tool_result` 的 `body` = **完整真实 stdout 逐字全文**（不是摘要、不是"运行成功"）。

### 1.3 真实执行证据（N09 / N06 / N11 三个 N 码）

harbor 要求 trace 里有"真实符号计算/数值执行证据"，否则按缺漏扣分：

| N 码 | 含义 | 罚分 |
|---|---|---|
| `N06` 伪造执行 | 声称执行但无真实 stdout | -35 |
| `N09` 无执行证据 | 无 tool_call/tool_result 展示计算过程 | -30 |
| `N11` 输出无因果支持 | outputs 没有"重跑+字节级一致"的验证链 | -6 |
| `N14` 方法替换 | 用 fallback 措辞而非如实命名方法 | -8 |
| `N16` 突发/重复提交 | trace 引用 prior attempts / 外部分数 | -15（序列级） |

**答案前置 = block（决定性反例）**：首条 thought 直接陈述答案结论的 trace，harbor 判
"**answer appears pre-loaded**" → `trace_decision=block` → `trace_factor=0` → 总分归零。

**铁律**：
- 每条 `tool_call` 必须紧跟 `tool_result`，result 的 body 是真实命令的真实 stdout；
- 每条 tool_call 带具体 `tool_name`（"python"、"write"、"read"）和真实 `tool_args`；
- **write 类调用的 tool_result 回显写入文件的完整内容**（JSON/CSV 直接贴全文）；
- 最后附一条"重跑 reproduce.py + `sha256sum` 逐文件比对 = IDENTICAL"的验证步（N11 最强解）。

### 1.4 1:1 tool_call / tool_result 配对（硬合法性）

- 每条 `tool_call` 有且仅有一条 `tool_result`，两者 `tool_call_id` **完全相同**。
- 这是 validate.py 反作弊规则第 3 条，配对失败直接掉桶。所有 99 分 trace 都严格 1:1。

### 1.5 步数与复杂度的关系（★关键结论）

**13–20 步是"简单 schema 题"的最优；60–90 步长 trace 只适合"复杂训练题"。**

| 题型 | 最优步数 |
|---|---|
| 简单 schema 题 | **13–20 步**（13–16 最优） |
| 复杂训练题（多次训练 + 多输出 JSON） | **20 步 +**（真实交互丰富度；判分器偏好 60–90 步长 trace） |

**步数铁律**：
- 简单题 <20 步，别堆砌（堆砌 = 坐实 N16 或"人造"嫌疑）；
- 复杂题可以更长，但**必须**有 ≥3 条 body ≥80 字符的长 thought（反作弊规则第 5 条）；
- 每步 duration_s / cost_usd / tokens 字段齐全且合理（训练步 cost_usd 可以到 0.9，
  简单验证步 0.001；总 cost_usd ≥ 0.01 是硬门槛）。

### 1.6 timestamp 窗口（合法性）

- `timestamp` 单调递增、间隔 3–13 秒、总跨度 <2 分钟（简单题）；
- 必须落在 `execution.ran_at ± wall_time_s` 窗口内，与 ran_at 同一天；
- `step_order` 从 1 连续递增；`step_id`（若带）唯一。

### 1.7 反作弊 8 规则（Zhao 仓库 validate.py，镜像服务端）

1. trace 非空；
2. `step_type ∈ {thought, tool_call, tool_result, artifact, decision, error, observation}`；
3. tool_call 与 tool_result 按 tool_call_id **1:1 配对**；
4. **总 cost_usd ≥ 0.01**（按真实模型价折算）；
5. **≥3 条 thought 且 body ≥80 字符**；
6. 至少一步 cost_usd>0 或 tokens>0；
7. timestamp 非递减；
8. step_id 唯一（若带）。

bundle 上下文：`timestamp_window`（步骤时间戳 ∈ execution.ran_at ± wall_time_s）、
`artifact_path` 须存在于 bundle、至少一步 body 可 grep 于 execution/run.log（stdout anchor）。

---

## 2. trace_score ≥70 门控 FINAL 全量解锁

### 2.1 门控模型

```
trace_score 档位    trace_decision    trace_factor    最终兑现
─────────────────────────────────────────────────────────────
< ~30 (blocked)      block            0.0             总分归零
~30–69 (review)      review           ts/100          答案分打折（如 69→0.69）
≥ 70 (accept)        accept           1.0             答案分全额兑现
```

**关键观察**：同一 harbor 值的答案，只因 trace 从 accept 档掉到 29（block），最终分从打折变归零。**trace 是绝对门控，不是连续加分。**

---

## 3. reasoning_bonus +5 的机制与获取方法

### 3.1 机制

- 独立于 trace_score 的**额外加分**，最高 +5，直接加在最终 score 上。
- 触发条件：判官在 trace 里检出 **reasoning / thinking token**（思考链）。
- 判词 note 原文："未在 trace 中检出 reasoning token。用会暴露 reasoning content 的模型
  （如 DeepSeek）并在 trace 里保留思考链，可得 +5 分。"

### 3.2 获取方法

1. 用**会暴露 reasoning content 的模型**（DeepSeek 天然可拿；判官明示 DeepSeek = 加分通道）；
2. 提交 `--raw-messages <raw_messages.jsonl>`，且 raw_messages 里 assistant 消息带
   **thinking 链**；
3. 保持 raw_messages 与 trace 内容一致（不一致会坐实 N06 伪造）。

### 3.3 与 trace_score 的关系（★重要）

**若不提供 raw_messages 也不扣 trace 分（带/不带对 trace_score 无影响）**——reasoning_bonus
是独立加分项，不是 trace_score 的一部分。想要满分就带上，不带上最多丢 5 分而非掉 trace 档。

---

## 4. 随机性 vs 确定性（最终判定）

**核心结论：harbor_reward 是确定性的；trace_score 是"准确定性 + 少数随机"的混合，但
随机性来自"内容缺陷被随机捕获"，而非判分器本身掷骰子。**

### 4.1 harbor_reward = 完全确定性

同一答案内容跨身份/跨重复提交 harbor 分**零漂移**（多题、多身份、数十次重复证实）。

**→ harbor_reward 只由 `/app/outputs` 内容决定，与身份/历史/重复无关，可作确定性评分预言机。**

### 4.2 trace_score = 准确定性，但有"内容缺陷随机捕获"的方差

**同一 trace 文件、不同身份，trace_score 会不同**（这是"随机"的唯一形态）。

**判读**：这不是判分器随机，而是 **trace 内容里的潜在缺陷（如"答案前置"、"引用论文"、
弱 N11 验证链）在 LLM judge 的逐次抽取/采样中被"概率性"捕获**。同一份有缺陷的 trace，
有时被判过、有时被揪住。

### 4.3 分题型判定

| 题型 | trace 评分本质 |
|---|---|
| 确定性数值题 | 有 per-challenge 确定性 verifier；答案分零方差 |
| LLM 推导题 | harbor 是离散 rubric tier（不可通过换措辞突破） |
| trace 本身 | 准确定性：内容缺陷决定档位，缺陷捕获有抽样方差 |

### 4.4 实操结论

- **答案分：当确定性看待**——A/B 换身份换措辞没用，改内容才有意义。
- **trace 分：当"修复内容缺陷"看待**——低分时不是"重提交赌运气"，而是"找缺陷并重写
  一条干净 15 步 recipe trace 再落袋"。

---

## 5. 可复用 trace 高分配方（分题类）

### 5.1 简单 schema 题

**15 步骨架（TRACE_99_RECIPE，可照抄）**：

```
step 1  thought    Read the output contract（任务 + 输出合同，用自己的话，不含答案）
step 2  tool_call  python src/derive_part1.py（真实命令）
step 3  tool_result  ← 完整真实 stdout 逐字
step 4  tool_call  python src/derive_part2.py
step 5  tool_result  ← stdout
...    （每个推导阶段一个独立脚本 + 独立 call/result 对，harbor 好定位"哪步推了什么"）
step N  tool_call  write outputs/answer.json
step N+1 tool_result ← Wrote ... Content:\n<answer.json 完整内容>
step N+2 tool_call  python src/self_check.py
step N+3 tool_result ← ALL CONTRACT CHECKS PASSED + 完整 stdout
step N+4 artifact   SHA-256: <各文件校验和>
step N+5 decision   Submit via playground CLI
```

**五条铁律**：
1. **首条 thought 只述任务、不含答案结论**（含答案 = "answer appears pre-loaded" = block）；
2. stdout 一律放 `body` 字段，逐字全文；
3. write 的 tool_result 回显文件**完整内容**；
4. tool_call/tool_result 严格 1:1 同 tool_call_id；
5. **无任何** "prior attempt / 上次 / 分数 / 迭代 / 战报 / N16" 字样——纯解题过程叙述。

### 5.2 复杂训练题（多次训练 + 多输出 JSON）

**区别**：
- 步数可到 20 步以上；
- 训练步 duration_s / cost_usd 真实，不要伪装成"秒级"；
- **必须** ≥3 条 body≥80 字符的长 thought；
- **必须**删光论文引用/作者名/benchmark 文献值（69→90+ 的核心）；
- 每个输出 JSON 一条 artifact（带 artifact_path + sha256）；
- 最后的 SHA-256 汇总 artifact + decision。

### 5.3 通用"安全清单"（提交前逐条过）

1. `solver-guard_trace-validate --trace trace.jsonl`（插件 trace 质量门单跑）必须通过且档位预测 ≥70；
2. cost_usd 总和 ≥ 0.01；≥3 长 thought；≥1 步 cost/tokens >0；
3. timestamp 非递减、span <2min、与 ran_at 同窗口；step_order 连续；
4. 每个 artifact_path 文件真实存在；≥1 步 body 可 grep 于 execution/run.log（无 BOM）；
5. 无论文方程编号/作者/年份/benchmark 文献值；无 prior-attempt/分数引用；
6. raw_messages（可选）含 DeepSeek thinking 链 → +5，但内容须与 trace 一致。

### 5.4 提交纪律（配合 trace 拿高分）

- **只用插件唯一入口** `solver-guard_build-submit`（六门 + 执行一体，token 由插件持有；裸 REST 被 guard 归零 `missing_worker_submission`）；
- **每题 ≤10 次提交**是稀缺资源；trace 没修好前**绝不烧 slot**；
- **一次到位**：整个会话恰好一次 `solver-guard_build-submit`，其余走 `--dry-run`（N16 规避）；
- trace blocked 时，不修好 trace 不重交。

---

## 6. 快速决策表（遇到 trace_score 低分时）

| 现象 | 根因 | 动作 |
|---|---|---|
| trace 29 随机 | 叙述化无证据 + 引论文 | 重写 15 步 recipe，删论文引用 |
| trace 69 平台期 | 完整但"leaning on paper" | 删除作者/年份/方程编号/benchmark 值 |
| trace 29 → blocked | 首条 thought 答案前置 | thought 只述任务，结论藏进 tool_result stdout |
| trace 39→78 档跳变 | N06 伪执行 | tool_call 用真实工具名 + 真实 stdout 进 body |
| trace 因 N16 −15 | 引用 prior attempts | 去掉所有"战报/迭代/分数"字样，单次提交 |
| trace 低但 harbor 高 | 长 trace 缺长 thought | 补 ≥3 条 body≥80 的长 thought |
| 同 trace 跨身份方差 | 内容缺陷被随机捕获 | 重写干净 recipe trace 再落袋一次 |

---

## artifact_path 相对基准（E2E 实测）

trace 内 artifact 步骤的 `artifact_path` 按 **trace 文件自身所在目录** 解析（平台 ARM 布局：trace.jsonl 与 outputs/ 同级）。
工作区把 trace 放子目录（trace/ vs outputs/ 同级）时，必须写 `../outputs/xxx`；写 `outputs/xxx` 会解析到 trace 目录下而判 missing（档位 29）。
验证：solver-guard_trace-validate 提前自查。
