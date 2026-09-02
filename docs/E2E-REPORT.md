# ASCodex E2E 实测报告（真实 LLM，agnes-2.5-flash）

## 验证结论

**核心架构有效**：chief→solver 子代理派发、StageBrief 注入、六道门控、ledger 周期签发/绑定、
Windows 沙箱降级、真实执行 trace 构建，全链路在真实 LLM 会话下跑通。

## E2E 暴露并修复的问题（提交 2c42a6d）

1. **agnes round-2 请求兼容**（core/client.rs + codex-api/responses.rs）：
   - `function_call` 回放缺 `id` → agnes 400；补确定性 id
   - `function_call_output.output` 内容数组 → agnes 400；归一为 string
   - `reasoning.summary` 非空 → agnes 400；清空
   - `agent_message` 类型 → agnes 400；转普通 message 并透传 encrypted 载荷
     （agent message 的 payload 存在 encrypted_content，必须包含否则子代理收到空任务）
2. **submit 工具作用域**（tools/spec_plan.rs + solver_guard.rs）：仅 solver 角色子代理注册；
   schema 瘦身（身份字段可选默认 policy/env）；授权改为 thread_cycle_binding（spawn 时自动绑定）
3. **Windows 沙箱**（agent/control/spawn.rs）：无提权下 StageBrief split ACL 无法 enforce
   （restricted-token 拒绝分读写），降级保留父 profile + 强制 network Restricted；
   写纪律由 Guard 门承担（workspace 归属/redline/六门）
4. **trace 校验 CRLF 归一化**（solver-guard/lib.rs）：PowerShell stdout 为 CRLF，trace body 为 LF，
   锚定检查归一化后匹配
5. **real-trace-capture skill 对齐门 schema**：step_order 连续/duration_s/tool_name/tool_args/
   manifest path 相对 workspace 根（之前 skill 示例缺 step_order 导致门拒）
6. **solver agent 恢复 shell**：trace 需真实执行；内容级拦截（tool_preflight_with_input）
   已足够阻止提交命令，不必禁 shell
7. **config**：danger-full-access + [windows] sandbox=unelevated + reasoning low/none
   （high/medium 档 agnes 超长推理不落工具调用）
8. **contract 门**：contract/fingerprint-input 须在提交 workspace 根（solver-ws），
   子代理提交 workspace 用根目录

## 验证证据
- solver-guard 85 测试全过；离线 trace+manifest 校验 ALL PASSED
- 子代理真实执行（写 analysis/results.json、跑 verifier exit 0）、构建 5 个证据文件、
  两次调用 solver_guard_submit 被门拦（contract 路径 → trace schema → CRLF）逐步收敛
- chief spawn 子代理成功（subAgentActivity started/completed）
- core 测试 1 个栈溢出为预存问题（干净树复现），与改动无关

## 剩余限制（非软件缺陷）
- agnes 模型多步稳定性：子代理偶发误解任务（把 chief 指令当自己任务）、提前收工、
  超长推理不落工具。已在 go 文件 message 中显式"你的唯一任务"规避
- 真实 Bohrium 提交（executor 启用）与评分器高分验证仍需平台写授权
