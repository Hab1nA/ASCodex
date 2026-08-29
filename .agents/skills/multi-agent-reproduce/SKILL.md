---
name: multi-agent-reproduce
description: Orchestrated parallel reproduction: plan the work breakdown across multiple agents, spawn workers for independent figures or parameter sweeps, checkpoint intermediate results, merge and grade the combined output, then submit. For papers with many independent figures or large parameter spaces.
---

# /multi-agent-reproduce

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
