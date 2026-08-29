---
name: trace-maximize
description: "构建可审计的 Bohrium Playground 真实执行 trace：保留真实运行痕迹、tool call/result 配对、stdout、时间戳与 artifact provenance，并隔离判罚/判官情报。用于 trace 生成、修复与提交前真实性检查；不假定平台固定公式或反作弊权重。触发词：'trace 提分'、'trace 高分'、'trace 配方'、'trace_score'、'写 trace'。"
metadata:
  version: 1.2.0
  author: friday-team
  tags: [bohrium-playground, trace, scoring, anti-fraud]
---

# 真实执行 Trace 配方

trace 是执行证据，不是事后撰写的解题故事。平台当前报告：完全没有运行痕迹的轨迹不进入待评队列；反作弊已改为加权计分并新增三个检测信号。信号名、权重、阈值和总分组合公式若无实时服务证据，一律标为未知，不沿用历史固定公式。

## When to Use

- 任何提交前的 trace 生成
- trace 被拒、未进入待评或评分异常时的诊断
- 复杂训练题的 trace 设计

## ASCodex 本地 admission invariants

这些是 ASCodex 自己的提交前硬门，不宣称等同于平台加权模型：

1. 至少有一段可验证的真实运行：命令/工具调用、对应结果、真实 stdout/stderr 或结构化输出，以及由该运行产生的 artifact。
2. `tool_call` / `tool_result` 按 `tool_call_id` 一一配对；结果正文放在当前 trace schema 要求的字段中，schema 不明时先核对当前契约。
3. step id 唯一、时间戳非递减且落在真实执行窗口；duration、tokens、cost 不编造。
4. artifact 记录路径、hash、生成它的调用以及 execution manifest/log hash，provenance 可反向追到真实运行。
5. thought/decision 与当时可见证据一致，不把最终答案伪装成预先已知，也不补写不存在的失败尝试。
6. 运行痕迹缺失、配对破损、provenance 不闭合时直接 `not_submission_ready`，不要上传后等待平台替你发现。

## 模板（按题型）

### 简单 schema 题（15 步骨架）
thought(读契约,≥80字) → 4-6 组 [tool_call 执行 + tool_result 完整 stdout] → write answer.json(回显全文) → self-check(真实输出) → artifact(sha256) → decision。

### 复杂训练题（20+ 步）
每条真实命令一个 tool_call/tool_result 对（训练/验证/扫描），thought 穿插 ≥3 条长推理，真实 duration（train 2109s 可写 2109.0），timestamp 与真实时间一致。**用真实会话记录转录，不编造**。

## 平台信号处理

- 保存服务端返回的 `trace_admission`、加权反作弊状态与信号明细原文；未知三个新增信号时写 `platform_weighted_anticheat_unknown`，不得把本地检查项冒充平台信号。
- 若 trace 完全没有运行痕迹且未进入待评队列，修复真实执行证据后生成新 bundle revision；不能只改叙述或重复上传同一伪 trace。
- 判罚标记、判罚原因、原始/有效分只进本地诊断账本，严禁写回 trace。
- 历史的 `trace_score >= 70`、固定 `trace_factor`、reasoning bonus、N 码与“8 条规则”只能作为旧记录考古，不能作为当前阈值或总分公式。

## 真实执行史变体（trace 墙时的第二路线）

- 蒸馏构造（gen_trace_distilled.py）在旧题曾可行，但构造 trace 不等于真实执行；**同 labels 多次低于 70 = trace-labels 耦合**，应优先捕获真实执行史。
- **换路线**：零情报 solver（只读题面+数据）在干净上下文完整走一遍实际工作（读题→假设→实现→跑→产出→叙述），raw_messages 即提交物——"提交实际做工作的 transcript"（高分选手 exam-logic）。真实 stdout/thought/失败尝试都是素材，不编造。
- **判别**：trace admission 或加权反作弊异常时，先核对真实运行、schema、配对和 provenance，再根据服务端可见依据决定是否需要重新执行；不要围绕未知权重盲调标签。
- 铁律（`trace-contamination-redline`）：无论哪条路线，判官/红队情报严禁进入 trace/thought/stdout/注释/变量名。

## 判分 wall-time 平衡（性能预算）

- 判分 wall-time 是硬约束：盲目提升 restarts/采样精度会因判分出子槽超时掉 N（09 29145：Bell 优化器 restarts 17min 判分 → 90.27 < 92 基线 29142/44 ≈6min）。
- 提交前估算判分时长：复杂度/restarts 与基准时隙对比；超时风险变体（>1.5× 基准）不提交或先降档。

## 提交前安全清单

1. 按 `trace-contamination-redline` 扫描平台分数、attempt、判罚/反作弊结论、他人做法等污染信息
2. 至少一段真实运行闭环；所有 tool result 含真实结果并与 call 配对
3. step id、时间戳、duration 与真实日志一致
4. artifact hash、execution manifest 和生成调用可相互核对
5. 本地运行 `bohrium-kb/tools/submit_gate_audit.py` 并保留诊断侧 JSON 报告（不装入提交 bundle）
6. 任何平台加权信号/权重未知项均显式为 unknown，而非猜测通过
