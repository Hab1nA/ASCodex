---
name: competition-coordinate
description: "总负责人多代理协调作战手册：六类角色分工、OODA 攻坚环、飞书群情报员 hook 驱动三级研判、身份配额中央注册、决策日志、跨代理信息纪律。触发词：'多代理'、'协调'、'总负责人'、'作战'、'派活'、'换人'、'飞书群监控'。"
version: 1.1.0
author: friday-team
tags: [bohrium-playground, coordination, multi-agent, orchestration]
---

# 多代理协调作战手册（总负责人视角）

> **ZCode 模式注记（2026-09-04）**：当前工作模式为"一题一会话"单会话解题（见 `ascodex-solve`），不存在总负责人与解题子代理，本手册的派活/换人/监控编排**不适用**，仅作历史多代理协作参考；身份配额纪律与决策日志条款仍可沿用。

## Codex 安全适配

Codex 的代理接口是 `collaboration.spawn_agent`、`collaboration.followup_task`、`collaboration.send_message` 和 `collaboration.wait_threads`；没有 DSH 的 `subagent_send`、`subagent_queue` 或 steer/cancel_first 参数。飞书 hook 仅在已连接的 Lark 工具可用时启用，否则标记为不可用。决策日志写入用户明确指定的 `bohrium-kb/round3_prep/DECISION_LOG.md`，不写入 `.claude/memory/`。

S4 Round-3 实战验证的作战编排（10 题并行、20+ 子代理、最终 761 分第 7 名）。总负责人**只决策不执行**。

## 六类角色

| 角色 | 数量 | 职责 |
|---|---|---|
| 总负责人 | 1 | 读战报→决策→派活；额度仲裁；换人/换角度裁决；汇报 |
| 解题代理 | 每题 1 | 读题→解题→按实验协议提交→回报 |
| 监控代理 | 1 持久 | 榜单/attempt/判词/额度看门狗，事件驱动主动报告 |
| 飞书群情报员 | 1 持久 | 官方交流群 hook 驱动监控（见下） |
| 判官信号分析师 | 按需 | 判词挖掘、档位分布、字段响应矩阵、换角度假设清单 |
| 红队/异质探针 | 按需 | clean-room 重读题面（禁读旧 REPORT），找集体盲区 |

## 失败分类法（worker 回报必须五选一，融合自平台 /orchestrate 技能）

解题代理每轮结束必须用以下分类回报，禁止含糊"还没好"：

| 分类 | 含义 | 总负责人动作 |
|---|---|---|
| `success` | 任务完成（分数提升/答案确定） | 落袋/继续下一目标 |
| `blocked` | 等外部输入/决策（如需用户裁定身份） | 立即处理阻塞 |
| `env_failure` | 工具/基础设施问题（429、评分器停摆、token 丢失） | 换通道/换身份重试 |
| `timeout` | 超时（时间窗口不足） | 缩减范围或放弃 |
| `inconclusive` | 不确定是否完成，需人工审查 | 总负责人裁决 |

## 教训沉淀协议（融合自平台 /reflect 技能）

每轮结束（或卡死复盘后），总负责人把新教训写入用户明确指定的项目日志（分类 + 去重 + 过期）：
- `pitfalls.yaml`：踩过的坑（wrong→right→impact），如 "trace 引论文 → 69→92.75 修复"；
- `patterns.yaml`：应持续遵循的多步流程（如"开题先译 §5 契约成自建 verifier"）；
- `decisions.yaml`：有取舍的架构决策（如"身份池冻结"）；
- `review-insights.yaml`：外部审阅的宝贵观察（如 jarvis 裸值 4 的洞察）。
规则：可操作、具体（含 attempt id/文件）、先查重再写、过期条目（+3 个月）清理。

## OODA 攻坚环

Observe（监控+情报员采集，总负责人不轮询）→ Orient（每题判官信号卡）→ Decide（收益×概率/耗时排序派活）→ Act（解题代理执行）→ 卡死检测环（同轴 3+ 变体不动 → 触发 unstuck-switch-angle 协议）。

## 飞书群情报员协议（hook 驱动）

1. **订阅**：群消息事件 hook（lark-event 长连接或增量拉取），只处理检查点后新消息。
2. **三级研判**：
   - A 级（立即上报）：评分器/通道故障修复、出题人格式/判分补充、规则/截止变更、数据资源更新。
   - B 级（攒批简报）：参赛者技术讨论（判官线索）、官方回复、多人反馈同类问题。
   - C 级（忽略）：闲聊、重复。
3. **上报格式**：`[级别] 时间 + 话题 + 原文关键句（逐字）+ 建议行动`。拿不准按 A 级。
4. **落盘**：FEISHU_INTEL.md（时间戳+原文）。
5. 情报员只报情报不决策；总负责人据 A 级情报派活（如"bundle 通道故障→暂缓依赖 bundle 的提交"）。

## 信息纪律（每条都是血泪教训）

1. **attempt id 归属核实**：任何 report 引用分数前必须 `GET /api/attempts/{id}` 核对 challengeId（三次 ID 混淆事故：26061/26377/26178）。
2. **身份配额中央注册**：提交前登记、禁止新注册、429 换池内身份、claimed_operator=1179613 校验（friday-t2 孤儿事故）。**身份类别由用户指定**：用户发开始解题命令时指定可用身份类别，代理只能用被指定的类别，禁止使用未被指定的其它身份（即使池内有余量）；换身份上报总负责人裁决，不自选。**绝对禁止新增/注册任何 Agent 身份**（池已冻结，违规代理将被中断）。
3. **知识库防传染**：REPORT 分"事实（attempt id 证据）"与"结论（证伪条件）"；接手代理先读证据再读结论，有权挑战旧结论。
   目录：活跃题 `work/`，完赛题 `archive/challenges/`，jarvis/ultron `archive/collab/`（只读）。选题必须同时扫这三处，不能只扫 `work/`。对照 `AGENTS.md` / `archive/README.md`。
4. **回报五要素**：attempt id + 身份 + harbor + trace + 判词。
5. **决策日志**：每条决策记（为何、预期、期限），供赛后复盘。

## 消息纪律（running/resident 代理 followup 纪律，10 solver 积压 11 条教训）

1. **running/resident 代理**（解题中/常驻代理）：使用 `collaboration.send_message` 发送非破坏性说明；需要改变任务时先等待工具调用边界，再用 `collaboration.followup_task`，不要假设存在 DSH 的 steer/cancel_first。
2. **followup 仅 cold resume**：对已停/已完成/卡死的代理，走 cold resume（恢复会话）再发消息，不往热会话塞 followup。
3. **代理状态定期核查**：总负责人使用 `collaboration.list_agents`/`wait_threads` 检查活动代理；Codex 没有持久 subagent_queue 或冷恢复账本。
4. **信息延迟意识**：发给 running 代理的普通消息不是即时生效的——关键裁决（换方向/止损/红线）用 steer/cancel_first，并确认收到。

## monitor 职责扩展：STATUS.md 固定状态文件

- monitor 代理维护 **STATUS.md**（固定位置，如 `bohrium-kb/round3_prep/STATUS.md`），**每 cycle 覆盖**（非追加）：
  ```
  # STATUS（<时间戳>）
  scoreboard: <总分/排名/各题最新分>
  in-flight: <进行中 attempt/job 及状态>
  since-last: <上次报告以来的事件>
  blocked: <等待裁决/外部输入的事项>
  next-checkpoint: <下次检查点时间>
  ```
- 目的（高分选手 endgame 机制）：人醒一眼看状态，不挖聊天记录；会话死亡后新会话从 STATUS.md 秒恢复上下文。
- 格式五要素固定，monitor 每次轮询后覆盖更新；MONITOR_REPORT.md 保留事件流详表，STATUS.md 只放当前快照。

## 时间与额度管理

- 提交窗口内评分有积压（高峰 15-45 分钟）：压哨提交要留足出分余量；评分回填可能赛后。
- 每身份每题 10 次上限：变体实验按"每题总预算"规划，单字段 A/B 优先于整包重写。
- 截止前 30 分钟停止新变体实验，只做"已就绪版本落袋"。

## 卡死处理流程（总负责人触发）

1. 解题代理报卡（或监控代理发现同轴 3+ 变体不动）。
2. 总负责人裁决：是否触发换角度（对照 unstuck-switch-angle 触发条件）。
3. 触发 → 并行：判官信号分析师挖掘 + clean-room 红队重读题面。
4. 换角度换人：原代理交资产（attempt 证据表+已证伪轴），新代理带差异化视角上任。
