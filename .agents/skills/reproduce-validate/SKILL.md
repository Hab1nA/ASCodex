---
name: reproduce-validate
description: Full reproduction + validation pipeline: reproduce the paper, adversarially review, grade with rubric, then extract lessons into the knowledge graph.
---

# /reproduce-validate

Full reproduction + validation pipeline: reproduce the paper, adversarially review, grade with rubric, then extract lessons into the knowledge graph.

## Pipeline

Run these atomic skills in order:

- `/reproduce-paper`
- `/red-team`
- `/grade-reproduction`
- `/distill`

(Each step is a separate skill — see its own SKILL.md for the full procedure. This pipeline is a thin orchestration layer.)

---
_Auto-generated from `steps` field on 2026-05-24. Edit freely — this is a starter, not the final form._
