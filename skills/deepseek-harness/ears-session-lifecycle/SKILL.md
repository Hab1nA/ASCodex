---
name: ears-session-lifecycle
description: Complete EARS session lifecycle for a multi-session reproduction task: sync with master and plan work, execute the reproduction, checkpoint progress, reflect on lessons learned, and wrap up the branch. Ensures no knowledge is lost between sessions — the checkpoint captures task state (what to do next) while reflect captures reusable knowledge (what to remember).
---

# /ears-lifecycle

Complete EARS session lifecycle for a multi-session reproduction task: sync with master and plan work, execute the reproduction, checkpoint progress, reflect on lessons learned, and wrap up the branch. Ensures no knowledge is lost between sessions — the checkpoint captures task state (what to do next) while reflect captures reusable knowledge (what to remember).

## Pipeline

Run these atomic skills in order:

- `/sync-and-plan`
- `/reproduce-paper`
- `/checkpoint`
- `/reflect`
- `/wrap-up`

(Each step is a separate skill — see its own SKILL.md for the full procedure. This pipeline is a thin orchestration layer.)

---
_Auto-generated from `steps` field on 2026-05-24. Edit freely — this is a starter, not the final form._
