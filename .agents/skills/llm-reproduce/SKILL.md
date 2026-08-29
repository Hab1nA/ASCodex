---
name: llm-reproduce
description: AI/ML paper pipeline: establish baseline evals, reproduce scaling laws or capability claims, validate against artifacts, extract patterns. Catches prompt-sensitivity and contamination issues.
---

# /llm-reproduce

AI/ML paper pipeline: establish baseline evals, reproduce scaling laws or capability claims, validate against artifacts, extract patterns. Catches prompt-sensitivity and contamination issues.

## Pipeline

Run these atomic skills in order:

- `/benchmark-llm`
- `/reproduce-paper`
- `/red-team`
- `/distill`

(Each step is a separate skill — see its own SKILL.md for the full procedure. This pipeline is a thin orchestration layer.)

---
_Auto-generated from `steps` field on 2026-05-24. Edit freely — this is a starter, not the final form._
