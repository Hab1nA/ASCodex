---
name: materials-dft-pipeline
description: Materials science DFT reproduction pipeline: use the materials skill set for domain knowledge, converge DFT parameters, reproduce target calculations, profile GPU utilization, adversarially validate, and extract lessons into knowledge graph.
---

# /materials-dft-full

Materials science DFT reproduction pipeline: use the materials skill set for domain knowledge, converge DFT parameters, reproduce target calculations, profile GPU utilization, adversarially validate, and extract lessons into knowledge graph.

## Pipeline

Run these atomic skills in order:

- `/materials-skill-set`
- `/dft-convergence`
- `/reproduce-paper`
- `/gpu-utilization-profiling`
- `/red-team`
- `/distill`

(Each step is a separate skill — see its own SKILL.md for the full procedure. This pipeline is a thin orchestration layer.)

---
_Auto-generated from `steps` field on 2026-05-24. Edit freely — this is a starter, not the final form._
