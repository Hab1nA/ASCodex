---
name: real-trace-capture
description: Capture a Bohrium Playground trace from genuine Codex execution records and stdout when a submission needs auditable, non-synthetic provenance.
---

# Real Trace Capture（真实 trace 捕获流程）

从 subagent 的真实执行记录中提取 trace.jsonl，**禁止脚本合成**。

## When to Use
- 任何需要提交到 Bohrium Playground 的 trace 生成
- 替代 gen_trace.py / create_trace.py 等合成方式

## 核心原则
**trace 必须来自真实 session 执行记录，不是脚本编造。**

## 捕获流程

### 方法 A：从 subagent session log 提取（推荐）
1. subagent 在真实 session 中执行 solve.py / reproduce.py
2. subagent 的每步 tool_call → tool_result 自然产生执行记录
3. 从 Codex 保存的原始 rollout/session execution record 中提取 tool_call/tool_result 对；final report 只能作为索引，不能作为原始执行证据
4. 按 trace.jsonl 格式转录，逐字段保留原始 timestamp/duration/stdout；无法从可信记录得到的字段必须标记缺失并阻断提交，禁止估算或补写

### 方法 B：手动执行 + 记录
1. 在 pwsh 中实际执行代码（`python solve.py`）
2. 记录每步的 command + stdout + stderr
3. 从这次实际执行的记录转录 trace.jsonl，每步包含真实输出；禁止回填虚构的时间、cost 或 token

## trace.jsonl 格式（每行一个 JSON，ASCGuard 门控 schema）

每行必须包含：`step_order`（从 1 连续递增，不可跳过/重复）、`step_id`（唯一）、
`step_type`、`timestamp`（RFC3339，非递减）、`duration_s`（非负）、`cost_usd`（非负）、
`tokens`（非负）。`tool_call` 行额外含 `tool_name` 与 `tool_args`；`tool_result`
必须紧随其 `tool_call`，含同 `tool_call_id` 与 `body`（真实 stdout）。

```json
{"step_order":1,"step_id":"s01","step_type":"thought","body":"读题面...（≥80字符）","timestamp":"2026-08-17T11:30:00Z","duration_s":1.2,"cost_usd":0.0,"tokens":0}
{"step_order":2,"step_id":"s02","step_type":"tool_call","tool_name":"pwsh","tool_args":{"command":"python solve.py"},"tool_call_id":"tc02","timestamp":"2026-08-17T11:30:05Z","duration_s":0.1,"cost_usd":0.0,"tokens":0}
{"step_order":3,"step_id":"s03","step_type":"tool_result","tool_call_id":"tc02","body":"[真实 stdout 完整输出]","timestamp":"2026-08-17T11:30:15Z","duration_s":10.0,"cost_usd":0.0,"tokens":0}
{"step_order":4,"step_id":"s04","step_type":"thought","body":"分析输出结果...（≥80字符）","timestamp":"2026-08-17T11:30:20Z","duration_s":1.1,"cost_usd":0.0,"tokens":0}
{"step_order":5,"step_id":"s05","step_type":"tool_call","tool_name":"pwsh","tool_args":{"command":"cat outputs/answer.json"},"tool_call_id":"tc05","timestamp":"2026-08-17T11:30:25Z","duration_s":0.1,"cost_usd":0.0,"tokens":0}
{"step_order":6,"step_id":"s06","step_type":"tool_result","tool_call_id":"tc05","body":"[真实文件内容]","timestamp":"2026-08-17T11:30:26Z","duration_s":1.0,"cost_usd":0.0,"tokens":0}
{"step_order":7,"step_id":"s07","step_type":"artifact","artifact_path":"outputs/answer.json","body":"sha256:abc123...","timestamp":"2026-08-17T11:30:30Z","duration_s":0.2,"cost_usd":0.0,"tokens":0}
{"step_order":8,"step_id":"s08","step_type":"decision","body":"提交答案","timestamp":"2026-08-17T11:30:35Z","duration_s":0.2,"cost_usd":0.01,"tokens":0}
```

## 铁律（违反 = trace_quality=0）

1. **tool_result.body 必须是真实 stdout** — 不能是编造的
2. **tool_call/tool_result 必须 1:1 配对** — tool_call_id 一致，tool_result 紧跟其 tool_call
3. **step_order 从 1 连续** — 不可跳过、重复、从非 1 开始
4. **无论文引用** — 作者名/年份/方程号/文献值全删
5. **首条 thought 不写结论** — 防 "answer appears pre-loaded"
6. **≥3 条 thought** — body ≥80 字符
7. **cost_usd 总和 ≥0.01** — 至少一步有非零 cost
8. **timestamp 非递减** — 按时间顺序
9. **step_id 唯一** — 不能重复
10. **tool_result.body 须锚定真实 stdout** — 至少一条 ≥16 字符的 tool_result body 必须能在 run.log 原文中找到

## 提交前校验清单

1. `grep -E "(Maliar|Paper \[|Table |Equation \(|et al\.)" trace.jsonl` → 应为空
2. 检查所有 step_type ∈ {thought, tool_call, tool_result, artifact, decision, error, observation}
3. 检查 tool_call 和 tool_result 的 tool_call_id 集合一致，且每条 tool_result 紧随其 tool_call
4. 检查 step_order 从 1 连续递增
5. 检查 ≥3 条 thought 且每条 body ≥80 字符
6. 检查首条 thought 是过程叙述不是结论
7. 检查 cost_usd 总和 ≥0.01
8. 检查 timestamp 非递减
9. 检查 step_id 唯一
10. 检查每条都有 duration_s/cost_usd/tokens，且至少一条 tool_result.body 出现在 run.log
11. **artifacts.json 的 path 必须相对提交 workspace 根**（如 `ch-e2e-1/analysis/results.json`），不是相对 challenge 目录；且 artifact 文件不能被列为 trace/run.log/manifests 本身
12. solver_guard_submit 的 workspace 必须是包含 challenge 目录和 contract 文件的根目录
