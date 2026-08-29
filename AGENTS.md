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

## Codex 适配边界

Codex 具备 shell、浏览器、Bohrium 通用 API 技能和协作代理，但没有 DSH 的 solver-guard、ScoreWatcher、AutoPush、SkillInjector 或持久 supervisor。任何“六门强制拦截、后台监控、自动提交”都只能在实现并验证对应代码后声明已启用；角色文档本身不等于运行时权限。
