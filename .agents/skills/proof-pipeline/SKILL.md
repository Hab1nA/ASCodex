---
name: proof-pipeline
description: Mathematics paper pipeline: reproduce the numerical results, formally verify the theorem in Lean 4, extract patterns into knowledge graph.
---

# /prove-and-verify

Mathematics paper pipeline: reproduce the numerical results, formally verify the theorem in Lean 4, extract patterns into knowledge graph.

## Pipeline

Run these atomic skills in order:

- `/reproduce-paper`
- `/proof-verify`
- `/distill`

(Each step is a separate skill — see its own SKILL.md for the full procedure. This pipeline is a thin orchestration layer.)

---
_Auto-generated from `steps` field on 2026-05-24. Edit freely — this is a starter, not the final form._
