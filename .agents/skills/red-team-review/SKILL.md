---
name: red-team-review
description: "Run an adversarial review on computational results: spawn the Red Team agent to hunt for unit mismatches, model bugs, numerical artifacts, and comparison methodology errors, then return ranked failure modes with the cheapest discriminating test for each. Trigger on: 'red team this', 'run red team', 'find bugs in my simulation', 'validate these results', 'what could be wrong', 'adversarial review', 'check for errors', 'falsify my results', 'sanity check my output', 'stress test this result', 'devil advocate my reproduction'. Also activates when results look too good or too bad, when a reproduction disagrees with the paper unexpectedly, or as a quality gate before finalizing a reproduction."
version: 1.1.0
author: friday-team
tags: [bohrium-playground, red-team, adversarial, clean-room, validation]
---

# /red-team-review — Adversarial Validation

## Codex/ZCode 安全适配

ZCode 单会话模式下没有可 spawn 的子代理：在本会话内以 clean-room 纪律执行复核（不复用既有实现、不读旧 REPORT/判词/分数），或建议用户另开一个会话承担红队角色。文件读取/实验用 shell 工具，结果只写入用户明确指定的工作区；任何平台写操作必须另行授权并先通过提交门（`.zcode/hooks/submit-gate.js` + `work/<slug>/.submit-authorized`）。

Run an adversarial review on a computational result. Spawns the Red Team agent to hunt for errors: unit mismatches, model/data bugs, numerical artifacts, comparison methodology issues. Returns ranked failure modes with cheapest discriminating tests.

## Trigger

User mentions: "red team", "validate", "check for errors", "falsify", "what could be wrong", "adversarial review".

## Workflow

1. **Gather context**: Collect the result to be reviewed — input parameters, method, output, comparison figure, independent reference used.
2. **执行 Red Team 复核**：ZCode 单会话模式下在本会话内按 `agents/codex-roles/bohrium-red-team.md` 的 clean-room 边界执行（从零重写、不读旧报告）；需要强隔离时请用户另开会话承担红队角色。
3. **Present findings**: Ranked failure modes, category audit table, cheapest discriminating test.

## Clean-room 独立参考协议（本地全对但判官丢时的决定性手段）

**触发**：本地自检全对（含自己写的独立参考）但判官仍丢分 = self-referent 盲区（09 C3 教训：数值在 solver/indep_ref 全过，判官仍丢 8 分，真缺口是字符串枚举串）。

1. **从题面从零重写**：新实现**只读题面文本**，不复用 solver/任何既有参考代码（09 indep_c3.py 不复用 indep_ref，暴露 σμν 下指标/相位/因子/归一化变体）。
2. **约定变体库**：把约定敏感点做成开关（下指标 g 收缩/整体相位/因子/归一化/σμν 定义），逐个验证，输出差异矩阵 + 约定修正候选清单（按似然排序）。
3. **物理点探针**：用独立参考对提交表达式在物理点集（含边界/退化/近零动量）100+ 点探针。
4. **字符串枚举字段逐分量判定**：独立参考全过仍丢 → 检查字符串枚举字段（mechanisms/classification/support——自检盲区最常在字符串），按题面逐分量机械判定（见 `judge-field-audit`）。
5. 输出：独立参考代码 + 约定变体探针表 + "数值 vs 字符串"定位结论（供 solver 修复）。

## Usage

Can be used standalone or as a quality gate in a pipeline:
- `/reproduce-paper` → `/red-team-review` → `/distill`

The most valuable output is the **cheapest discriminating test** — the single most informative experiment that would either confirm or refute the top failure mode.
