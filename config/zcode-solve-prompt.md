# ZCode 开题 Prompt 模板（ascodex-solve）

## 使用前提

1. 会话在本仓库（ASCLocal-Codex）内打开——根目录或 `work/<slug>/` 均可，项目技能与仓库钩子都会被自动发现。
2. `PLAYGROUND_TOKEN` / `BOHRIUM_TOKEN` 已在启动 zcode 的**进程环境变量**里（不要写进 prompt，不要写文件）。
3. 要允许真实提交时，**由你手工**在会话外创建 `work/<slug>/.submit-authorized`，内容为本次允许提交次数（如 `3`）；不存在时该会话只能 dry-run 与只读审计。每次提交扣 1 次，扣尽自动失效。

## 一行版（日常）

```
开始解题 challenge=<题目ID或URL> workspace=work/<slug> identity=<授权身份> auth=dry-run|submit:N
```

示例：

```
开始解题 challenge=ch-mp-r-split-01 workspace=work/ch-mp-r-split-01 identity=pool-A-3 auth=dry-run
```

## 完整版（首题 / 复杂题）

```
开始解题 challenge=<ID或URL> workspace=work/<slug> identity=<授权身份> auth=dry-run|submit:N
已知情报：<判分器类型 / 历史尝试 / 官方群补充说明；没有则写"无">
预算：<时间 / 算力上限；没有则写"默认">
特别注意：<题目特殊约束>
按 ascodex-solve 技能流程执行：先开场六步再动手。
```

## 说明

- 触发词"开始解题"会被仓库 `UserPromptSubmit` 钩子自动注入纪律前言（现行契约指针、开场六步、提交门、红线），prompt 里不必重复纪律。
- `auth` 与授权文件是两道一致的闸：prompt 里写 `auth=dry-run` 时不要创建授权文件；要提交时先创建 `.submit-authorized` 再告知会话。
- 多个会话并行解题时，各题 workspace 互不重叠、各用各的授权文件；`work/` 下同时存在多个授权文件会被提交门拒绝（恰一原则），提交前清理已用完的。
