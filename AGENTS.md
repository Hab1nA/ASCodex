# ASCLocal-Codex 工作区规范

本目录是从 `ASCLocal` 与 DeepSeek Harness 提取的、面向 Codex 的 Bohrium Playground 迁移镜像。经验、规则和源码保留来源语义；凭据、会话、日志、运行时依赖不进入本目录。

## 工作区布局

- `bohrium-kb/docs/`：平台/API/ARM 协议文档。
- `bohrium-kb/round3_prep/`：作战手册、评分真相、Trace 与提交纪律。
- `bohrium-kb/tools/`：源工作区工具脚本；涉及 POST/删除/提交的脚本只能在明确授权、先 dry-run 后使用。
- `work/_template/`：题目工作区与 ARM/Trace 模板。
- `skills/deepseek-harness/`：从 Harness 导出的 32 个技能文本；正文中的 DSH 专用工具名需映射到 Codex 工具后才可执行。
- `agents/codex-roles/`：Codex 协作角色定义。
- `agents/harness-presets/source/`：原始 Cordis preset，仅作审计/追溯，不是 Codex 可直接加载的 preset。
- `config/`：脱敏后的配置参考与迁移清单。

## 安全与证据纪律

1. 凭据只从进程环境变量读取；禁止提交、复制或打印 token、AccessKey、密码和 `*.credentials*`。
2. 不清理或改写 DeepSeek Harness 的 `sessions/`、沙箱临时目录、undo 快照或运行态；本镜像不包含它们。
3. 不把 `play.bohrium.com` 暴露到公网；写操作默认 dry-run，提交前必须核对题目、身份额度、challengeId、通道、Trace、红线和模型六道门。
4. 只接受可追溯证据：attempt 必须进一步核实 replay、`resultsJson`、`scorecard`、`harbor_reward` 与官方榜状态，`submitted`/`queued` 不是成功。
5. Trace 必须来自真实执行；保持 tool call/result 一一对应、stdout body、时间戳与 artifact provenance 自洽，禁止 prior score/attempt/团队信息污染。
6. 重型计算优先 Bohrium 云端；本地仅做短时 smoke test 与结果分析。
7. 选题同时检查 `work/`、历史归档和协作记录；新增题目只写入 `work/`，完成后再归档。

## ZCode 单会话解题模式（2026-09-04 起）

本仓库已把 ASCodex 的解题能力内化为 ZCode 形态，工作模式为**一题一会话**：用户开多个独立 ZCode 会话，每会话解一道题；无总负责人、无解题子代理（多代理派发组件已废止）。

- 入口技能：`.agents/skills/ascodex-solve/SKILL.md`（开场六步 → 四阶段解题 → 证据校验 → 提交 → 回报五要素）。触发词"开始解题"等会由 `.zcode/hooks/solve-prompt-inject.js` 自动注入纪律前言；prompt 模板见 `config/zcode-solve-prompt.md`。
- **提交门（硬拦截，PreToolUse 钩子 `.zcode/hooks/submit-gate.js`）**：针对 `play.bohrium.com` 的写命令与 `submit_bundle.py` 上传，仅当对应题目的 `work/<slug>/.submit-authorized` 存在时放行。该文件**只能由用户在会话外手工创建**（首个非空行 = 允许提交次数，每次原子扣 1，扣尽失效）；模型侧任何触及该文件的操作一律被拒。`--dry-run` 与只读审计不受限。已知边界：本门是启发式而非安全边界（文档型工具如 urllib 拼接、动态构造路径理论上可绕过），补偿控制是提交后 `bohrium-kb/tools/submit_gate_audit.py` 审计 + 只读核验。同理，`redline_scan.py`/`trace_check.py` 等校验器的修改属运维操作，解题会话不得改动。
- 证据纪律：trace 用 `scripts/ascodex_trace_builder.py` 从真实 `execution/run.log` 确定性转录（禁凭空合成）；提交前 `trace_check.py` + `redline_scan.py` 必须全绿；平台回传的编号/得分/评审信息不进 run.log 与提交物，平台情报只写 `work/<slug>/diagnostics/`（不进 bundle）。

## Codex 适配边界（历史）

Codex 具备 shell、浏览器、Bohrium 通用 API 技能和协作代理，但没有 DSH 的 solver-guard、ScoreWatcher、AutoPush、SkillInjector 或持久 supervisor。ZCode 下的等价强制已实现并入库（`.zcode/` 钩子 + `work/_template/` 校验器）；本节保留作为 DSH→Codex→ZCode 迁移史的边界记录。
