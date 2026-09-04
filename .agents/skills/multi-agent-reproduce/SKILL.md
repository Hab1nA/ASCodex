---
name: multi-agent-reproduce
description: Orchestrated parallel reproduction: plan the work breakdown across multiple agents, spawn workers for independent figures or parameter sweeps, checkpoint intermediate results, merge and grade the combined output, then submit. For papers with many independent figures or large parameter spaces.
---

# /multi-agent-reproduce

> **ZCode 模式注记（2026-09-04）**：本技能引用的 `/orchestrate` 与 worker spawn 编排在当前单会话模式（见 `ascodex-solve`）中不可用；需要并行时由用户各开会话按题执行，本技能仅作编排思路参考。

Orchestrated parallel reproduction: plan the work breakdown across multiple agents, spawn workers for independent figures or parameter sweeps, checkpoint intermediate results, merge and grade the combined output, then submit. For papers with many independent figures or large parameter spaces.

## Pipeline

Run these atomic skills in order:

- `/orchestrate`
- `/reproduce-paper`
- `/checkpoint`
- `/grade-reproduction`
- `/submit-attempt`

(Each step is a separate skill — see its own SKILL.md for the full procedure. This pipeline is a thin orchestration layer.)

---
_Auto-generated from `steps` field on 2026-05-24. Edit freely — this is a starter, not the final form._
