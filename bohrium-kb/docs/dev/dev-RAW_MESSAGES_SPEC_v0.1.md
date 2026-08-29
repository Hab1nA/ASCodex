# raw_messages.jsonl — 平台侧 delta 补充（对 v1.0）

> **Canonical spec**: 廖若雪 v1.0，已发布于 [github.com/liaoruoxue/paper2arm_info/blob/main/specs/trace-format-spec.md](https://github.com/liaoruoxue/paper2arm_info/blob/main/specs/trace-format-spec.md) — `session_start / message / session_end` 三层结构、step_id Math-Shepherd 锚点、失败轨迹强制、reward 自动算。
>
> 本 doc：天汉侧对 v1.0 的 4 项 delta 提案，跟若雪在 P2P `oc_5dd53a368d2f5e8d80f89f181d4aa388` 对齐中（2026-05-09）。下面是 standalone draft，思路保留供讨论，但**最终标准走 v1.0**。

## v1.0 之上想补的 4 项

1. **content 字段支持 list[block]**：纯 string 吃不下多模态题（光学 figure、X 射线 detector image），借 Anthropic content blocks（`text` / `image` / `tool_use` / `tool_result` / `thinking`）保留多模态原信息
2. **optional `provider_raw` 字段**：存 SDK 原 message JSON。extended thinking / structured output 这些 SDK-specific reasoning chain 不存原 JSON 后面 PRM 训练捞不回来。**optional**——裸 wrapping 选手不强制
3. **平台侧 Layer 1 enforcement**：缺 `traces/raw_messages.jsonl` 跟缺 figure 同级 block，server/services/scoring_service.py 加一道 schema 校验
4. **session_end 加 optional `step_rewards`**：list of `{step_id, reward, judge}`，Math-Shepherd 自动标注完回写进来——给 PRM 训练一个标准化的 supervision 入口

下面是初稿全文（有重叠部分以 v1.0 为准）。

## 设计原则

1. **Provider 中立**：Anthropic Messages API、OpenAI Chat Completions、Vertex AI Gemini 都能映射上；不锁死单一 SDK。
2. **细颗粒度**：每行一条 **message**（不是一个 turn、不是一整段 trajectory）。SFT/DPO/RL 都从同一文件出发，按 `(attempt_id, turn_idx)` 截窗。
3. **保留原始**：`provider_raw` 字段存 SDK 返回的原始 JSON，下游训练管线想看 reasoning trace / structured output / function call schema 都能从这里取。
4. **失败轨迹同等公民**：bundle 里 raw_messages.jsonl 在 score=0 时也必须存在；只有"完全没启动"的 attempt 才允许缺。
5. **Reward 解耦**：reward signal 不写在 message 里，写在 sidecar `trajectory_meta.json`。原因：reward 会随评分管线迭代，message 不该跟着翻 schema。
6. **不引入新依赖**：JSON-only，UTF-8，每行 ≤ 1 MB。Parquet/Arrow 是下游派生，不是源格式。

## 文件 layout（每个 ARM bundle 内）

```
trace/
├── raw_messages.jsonl        ← 一行一条 message，按时间序
├── trajectory_meta.json       ← attempt 级别的 reward / outcome / runtime info
└── trace.jsonl                ← 现有的 summary trace（保留，向后兼容）
```

`raw_messages.jsonl` 是新加的；`trace.jsonl`（现有 step_type 枚举）保持不动，作为人类可读的精炼版本。两者是**不同抽象层级**——raw 是 SDK 原话，summary 是后期自述。

## 每行 schema（1 message）

```json
{
  "attempt_id": 6313,
  "msg_idx": 0,
  "turn_idx": 0,
  "role": "user",
  "content": "...",
  "tool_calls": null,
  "tool_call_id": null,
  "name": null,
  "model_id": null,
  "provider": null,
  "provider_raw": null,
  "tokens_in": null,
  "tokens_out": null,
  "cost_usd": null,
  "timestamp": "2026-04-29T17:00:00Z",
  "parent_msg_idx": null,
  "meta": {}
}
```

### 字段定义

| field | type | required | 说明 |
|---|---|---|---|
| `attempt_id` | int | ✅ | 关联到 Playground `Attempt.id` |
| `msg_idx` | int | ✅ | 该 attempt 内 message 顺序，从 0 开始，单调递增 |
| `turn_idx` | int | ✅ | 该 attempt 内 turn 顺序（一个 turn 含 user→assistant，或 assistant→tool→assistant 闭环）。同一 turn 内多条 message 共享 turn_idx |
| `role` | enum | ✅ | `system` / `user` / `assistant` / `tool` |
| `content` | str \| list[block] | ✅ | 文本或 Anthropic 风格 content blocks（`{type:"text"\|"image"\|"thinking"\|"tool_use"\|"tool_result", ...}`）|
| `tool_calls` | list[{id, name, args}] \| null | ⚠️ assistant only | OpenAI 风格 function call list；与 content blocks 内的 `tool_use` 二选一冗余存（向下兼容两种 SDK） |
| `tool_call_id` | str \| null | ⚠️ tool only | 关联回 assistant 的 tool_use.id |
| `name` | str \| null | ⚠️ tool only | tool 名称（`bash`/`read`/`edit`/...） |
| `model_id` | str \| null | optional | 该 message 由哪个模型产出（如 `claude-opus-4-7`、`gpt-5-4-1106`）。只对 assistant role 有意义 |
| `provider` | str \| null | optional | `anthropic` / `openai` / `vertex` / `bohrclaw` / etc |
| `provider_raw` | dict \| null | optional | SDK 返回原始 message 的 deep copy。**强烈推荐填**——下游训练才能用上 reasoning content / structured output |
| `tokens_in` | int \| null | optional | input tokens（assistant 那次 generate 用的） |
| `tokens_out` | int \| null | optional | output tokens |
| `cost_usd` | float \| null | optional | 金额（毛估即可） |
| `timestamp` | str | ✅ | ISO 8601 UTC |
| `parent_msg_idx` | int \| null | optional | 显式 DAG 链接；分支重试时父消息 |
| `meta` | dict | optional | 自由 metadata：context truncated to N tokens、cache hit 标记、retry 次数等 |

### content blocks 推荐用 Anthropic 格式（多模态友好）

```jsonc
"content": [
  {"type": "thinking", "thinking": "..."},                    // assistant 思考链（如有）
  {"type": "text", "text": "..."},
  {"type": "tool_use", "id": "tu_01abc", "name": "bash", "input": {"command": "..."}},
  {"type": "tool_result", "tool_use_id": "tu_01abc", "content": "...", "is_error": false},
  {"type": "image", "source": {"type": "base64", "media_type": "image/png", "data": "..."}}
]
```

字符串简写允许（纯文本时）：`"content": "hello"` 等价于 `[{type:"text",text:"hello"}]`。

## sidecar `trajectory_meta.json`（attempt 级，1 文件 / attempt）

```json
{
  "attempt_id": 6313,
  "challenge_id": "s2-mcstas-neutron-psi-dmc",
  "agent_runtime": "claude_code",
  "agent_runtime_version": "1.0.62",
  "agent_provider": "anthropic",
  "outcome_reward": 100.0,
  "outcome_normalized": 1.0,
  "outcome_reason": "s2_topic_md LLM judge",
  "n_messages": 47,
  "n_turns": 12,
  "wall_seconds": 1840,
  "total_tokens_in": 320541,
  "total_tokens_out": 48720,
  "total_cost_usd": 4.92,
  "step_rewards": [
    {"msg_idx": 12, "reward": 0.8, "judge": "milestone:M1_source_detector"},
    {"msg_idx": 24, "reward": 1.0, "judge": "milestone:M2_guide"}
  ],
  "termination": "success",
  "schema_version": "0.1"
}
```

### 字段定义

| field | type | required | 说明 |
|---|---|---|---|
| `attempt_id` | int | ✅ | 同上 |
| `challenge_id` | str | ✅ | Challenge.id |
| `agent_runtime` | str | ✅ | `claude_code` / `cursor` / `windsurf` / `openhands` / `bohrclaw` / etc — 让训练管线能 stratify 不同 runtime |
| `agent_runtime_version` | str | optional | 版本号 |
| `agent_provider` | str | optional | `anthropic` / `openai` / `mixed` |
| `outcome_reward` | float | ✅ | 最终 LLM judge 给的分（0-100） |
| `outcome_normalized` | float | ✅ | 归一化到 [0, 1]，下游 RL 直接用 |
| `outcome_reason` | str | optional | 哪个 grader 给的（`s2_topic_md` / `arm_v1.1` / `programmatic:zhang-2018-prl-deepmd`）|
| `n_messages` / `n_turns` | int | ✅ | 计数（用于 corpus stratification） |
| `wall_seconds` | int | optional | end_to_end wall time |
| `total_tokens_in/out` | int | optional | 对全 trajectory 求和 |
| `total_cost_usd` | float | optional | |
| `step_rewards` | list | optional | **PRM 训练留的口子** — 每个 milestone 命中可记一个 step reward。下一阶段把 milestone 拆分到 turn 级 |
| `termination` | enum | ✅ | `success` / `partial` / `failed` / `blocked` / `timeout` |
| `schema_version` | str | ✅ | `"0.1"` for this draft |

## 怎么用：3 个下游消费者

### A. SFT trainer（HuggingFace TRL）
```python
import json, datasets
def to_chatml(path):
    with open(path) as f:
        msgs = [json.loads(l) for l in f]
    return {"messages": [
        {"role": m["role"], "content": m["content"]} for m in msgs
    ]}
ds = datasets.Dataset.from_list([to_chatml(p) for p in glob("**/raw_messages.jsonl")])
# 直接喂 trl.SFTTrainer
```

### B. DPO 配对（preference learning）
```python
# 同 challenge_id 不同 attempt → 按 outcome_reward 排序
# 高分 = chosen, 低分 = rejected
for ch_id, group in groupby_challenge():
    sorted_attempts = sorted(group, key=lambda a: a.outcome_reward, reverse=True)
    for chosen, rejected in pair_top_with_bottom(sorted_attempts):
        yield {"prompt": challenge_prompt, "chosen": chosen.messages, "rejected": rejected.messages}
```

### C. PRM training（process reward model）
```python
# step_rewards 给的是 sparse milestone reward → 训 PRM 把它扩散到每个 message
# trajectory_meta.json 是 supervision signal
```

## 与现有 trace.jsonl 的关系

```
现有 trace.jsonl (summary, ~10 步, 后期自述)         → 保留，人类可读，trace UI 渲染
新增 raw_messages.jsonl (raw, 全程 SDK 原话)         → 训练管线源数据
新增 trajectory_meta.json (attempt 级 reward)        → 训练管线 supervision
```

EARS（prompt-level 改进）继续输出 trace.md/trace.jsonl；trace-recorder（weight-level 改进）输出 raw_messages.jsonl + trajectory_meta.json。两层互补。

## 验证（spec 是否成立）

写一个 reference encoder 测试：
- Anthropic Messages API response → raw_messages.jsonl ✓
- OpenAI Chat Completions → raw_messages.jsonl ✓
- Cursor 内置 trace export → raw_messages.jsonl ✓ (TBD by selectors)

跑通 3 种 runtime → format 即可定稿。

## 强制规则（下一场 hackathon 起）

- 所有 attempt submit 时，bundle 里**必须**含 `trace/raw_messages.jsonl` + `trace/trajectory_meta.json`
- score=0 的 attempt 也必须含（失败轨迹同等公民）
- 缺这两个文件的 attempt 走 Layer 1 block，跟"缺 figure"同级
- 平台提供 `trace-recorder` 工具用于自动产出（claude_code / cursor 各一个 wrapper），不让选手手写

## 待若雪决策的开放问题

1. **content blocks 是 Anthropic 格式还是 ChatML 字符串？** — 我倾向 Anthropic blocks（多模态原生支持，downstream 解析也容易），但若 majority 选手用 OpenAI SDK，需要双向转换工具
2. **provider_raw 强制填还是 optional？** — 我标了 optional 但推荐；强制会让单纯 wrapping 的选手提交难度上升
3. **step_rewards 的 judge 字段命名空间** — 现在我用 `milestone:M1_xxx`，需要跟 scoring_service 现有字段对齐
4. **schema_version 演进** — 0.1 → 0.2 怎么 migrate？建议 monotonic + 在 trajectory_meta 里强制带版本号
5. **是否要存 system prompt？** — assistant 在没 system 时不重要，但 agent 框架（claude code）的 system prompt 信息量很大。我倾向必存，可能涉及隐私/安全审查

## 想要的反馈

- 这个 schema 能不能直接 ingest 进你已经在做的 paper2arm_info repo 的训练管线？
- DPO 那个 SKU（同 paper 不同 attempt 配对）你之前没显式提，我打算单独立作 deliverable，需要你的 outcome_reward 这一项支持，标准定下来后我们就解锁了
- step_rewards 这个口子你需要现在做满还是先留 placeholder 后续 backfill？
