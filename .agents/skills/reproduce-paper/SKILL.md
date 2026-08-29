---
name: reproduce-paper
description: "Orchestrate end-to-end reproduction of a published paper's computational results through a 5-phase workflow: preparation, parameter extraction, independent reference validation, target computation, and comparison with knowledge extraction. Trigger on: 'reproduce this paper', 'replicate the results', 'validate published figures', 'can we reproduce this', 'reproduce figure 3', 'check this paper claims', 'rerun the simulations from the paper', 'replicate the experiment', 'start a new reproduction', 'paper reproduction workflow'. Also activates when a user shares a paper DOI or PDF and asks to verify its computational claims, or when beginning any new challenge from the platform's challenge list."
---

# /reproduce-paper — Paper Reproduction Orchestrator

Systematically reproduce the computational results of a published paper using a principled 5-phase workflow. Works across any computational discipline: physics, chemistry, biology, materials science, mathematics, AI/ML.

## Trigger

User mentions: reproducing a paper, validating published results, replicating figures, checking a paper's claims, "reproduce", "replicate", "validate paper".

## Workflow

### Phase 0 — Preparation

1. **Read the paper fully** before writing any code. Understand the claims, methods, assumptions, and parameter values.
2. **Parse the paper** (if PDF available): extract text, figures, tables, equations using MinerU or manual extraction.
3. **Check prior work**: search for existing reproductions of this paper or related papers. Don't reinvent what exists.
4. **Identify figures to reproduce**: rank by (a) scientific importance, (b) computational feasibility, (c) data availability.

### Phase 1 — Parameter Extraction

Extract all computational parameters from the paper with confidence tags:

| Parameter | Value | Source | Confidence |
|-----------|-------|--------|------------|
| ... | ... | Table 1, p.3 | HIGH — explicit |
| ... | ... | Section 2.1 | MEDIUM — inferred |
| ... | ... | Not stated | LOW — assumed |

Flag any parameter that is not explicitly stated. These are the most likely sources of discrepancy.

### Phase 2 — Independent Reference

**Before comparing to the paper**, validate your setup against an independent reference:
- Analytic solution (if available)
- A simpler, well-validated tool
- A known benchmark case
- A limiting case where the answer is known

If your independent reference disagrees with the paper by more than expected, investigate before proceeding. The bug may be in your setup, the paper, or the reference.

### Phase 3 — Target Computation

1. **Build from cheap to expensive.** Start with the simplest relevant case and verify before increasing complexity. A typical ladder:
   - Analytic/limiting case
   - Simplified numerical case (coarse grid, simple model)
   - Full-fidelity case (fine grid, full model)

2. **Estimate cost before committing.** Run a short pilot, measure wall-clock per step, extrapolate.

3. **Minimize output.** Track scalar diagnostics frequently; save field data sparsely.

### Phase 4 — Comparison & Reporting

1. **Side-by-side figure comparison**: paper original (left) vs reproduction (right). Quantify divergence.
2. **Classify discrepancies**:
   - **Expected**: different method, approximation, or parameter choice (document why)
   - **Unexplained**: same method but different result (investigate)
3. **Write progress report** with: figure comparisons, divergence metrics, uncertainty budget, provenance (tool version, input files, run command, wall time).

### Phase 5 — Knowledge Extraction

After completing the reproduction:
1. What surprised you? What was harder than expected?
2. What pitfalls would you warn the next person about?
3. What general lessons apply beyond this specific paper?

Log insights to `trace.md`. If a pattern appears 3+ times across papers, distill it into a reusable knowledge file.

## Principles

- **Read before code.** Understand the paper's physics/math before touching a keyboard.
- **Validate before compare.** Independent reference first, paper comparison second.
- **Quantify everything.** "Looks similar" is not a result.
- **Log everything.** Every decision, every parameter choice, every discrepancy.
- **Never copy parameters without understanding why.** Reason from first principles.
