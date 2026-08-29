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
3. 从 subagent 的 final report 中提取 tool_call/tool_result 对
4. 按 trace.jsonl 格式组装，保持真实 timestamp/duration/cost

### 方法 B：手动执行 + 记录
1. 在 pwsh 中实际执行代码（`python solve.py`）
2. 记录每步的 command + stdout + stderr
3. 手动组装 trace.jsonl，每步包含真实输出

## trace.jsonl 格式（每行一个 JSON）

```json
{"step_id":"s01","step_type":"thought","body":"读题面...","timestamp":"2026-08-17T11:30:00Z","cost_usd":0.0,"tokens":0}
{"step_id":"s02","step_type":"tool_call","name":"pwsh","arguments":{"command":"python solve.py"},"tool_call_id":"tc02","timestamp":"2026-08-17T11:30:05Z","cost_usd":0.0,"tokens":0}
{"step_id":"s03","step_type":"tool_result","tool_call_id":"tc02","body":"[真实 stdout 完整输出]","timestamp":"2026-08-17T11:30:15Z"}
{"step_id":"s04","step_type":"thought","body":"分析输出结果...","timestamp":"2026-08-17T11:30:20Z","cost_usd":0.0,"tokens":0}
{"step_id":"s05","step_type":"tool_call","name":"pwsh","arguments":{"command":"cat outputs/answer.json"},"tool_call_id":"tc05","timestamp":"2026-08-17T11:30:25Z","cost_usd":0.0,"tokens":0}
{"step_id":"s06","step_type":"tool_result","tool_call_id":"tc05","body":"[真实文件内容]","timestamp":"2026-08-17T11:30:26Z"}
{"step_id":"s07","step_type":"artifact","body":"sha256:abc123...","timestamp":"2026-08-17T11:30:30Z","cost_usd":0.0,"tokens":0}
{"step_id":"s08","step_type":"decision","body":"提交答案","timestamp":"2026-08-17T11:30:35Z","cost_usd":0.01,"tokens":0}
```

## 铁律（违反 = trace_quality=0）

1. **tool_result.body 必须是真实 stdout** — 不能是编造的
2. **tool_call/tool_result 必须 1:1 配对** — tool_call_id 一致
3. **无论文引用** — 作者名/年份/方程号/文献值全删
4. **首条 thought 不写结论** — 防 "answer appears pre-loaded"
5. **≥3 条 thought** — body ≥80 字符
6. **cost_usd ≥0.01** — 至少一步有非零 cost
7. **timestamp 非递减** — 按时间顺序
8. **step_id 唯一** — 不能重复

## 提交前校验清单

1. `grep -E "(Maliar|Paper \[|Table |Equation \(|et al\.)" trace.jsonl` → 应为空
2. 检查所有 step_type ∈ {thought, tool_call, tool_result, artifact, decision, error, observation}
3. 检查 tool_call 和 tool_result 的 tool_call_id 集合一致
4. 检查 ≥3 条 thought 且每条 body ≥80 字符
5. 检查首条 thought 是过程叙述不是结论
6. 检查 cost_usd 总和 ≥0.01
7. 检查 timestamp 非递减
8. 检查 step_id 唯一
