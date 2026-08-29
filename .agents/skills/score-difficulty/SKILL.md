---
name: score-difficulty
description: "Score a paper's reproduction difficulty before starting using the Weighted Total Score (WTS, 12-60 pts): assess information completeness, physics complexity, model size, computational cost, post-processing effort, and code modification needs, then produce a time estimate, risk analysis, and phased plan. Trigger on: 'score difficulty', 'how hard is this paper', 'feasibility check', 'compute WTS', 'estimate effort', 'triage this paper', 'can we reproduce this in time', 'difficulty assessment', 'scope the reproduction', 'is this paper doable', 'pre-reproduction evaluation'. Also activates when a user is deciding which paper to tackle next, comparing multiple candidate papers, or needs to justify time allocation for a reproduction project."
---

# /score-difficulty — Pre-Reproduction Difficulty Scoring (WTS)

Score a paper's reproduction difficulty **before** you start. Produces a Weighted Total Score (WTS, 12–60 pts) across 6 dimensions, a time estimate, risk analysis, and a phased reproduction plan.

Distilled from 78 paper reproductions in the ASURF project (2024-2026). Works for any computational discipline.

## Trigger

User mentions: "score difficulty", "how hard is this paper", "difficulty assessment", "WTS", "feasibility check", "scoping", "estimate effort", "can we reproduce this", "triage paper".

## Workflow

### Step 1 — Read the Paper

Read the full paper. For each figure and table, classify reproducibility:

```markdown
## D0 — Solver Feasibility

| Figure | Verdict | Reason |
|--------|---------|--------|
| Fig 1  | PASS    | Computable with available tools |
| Fig 2  | PASS    | Standard simulation output |
| Fig 3  | FAIL    | Requires proprietary experimental data |
| Fig 4  | PARTIAL | Needs external dataset (available online) |
```

**Verdicts:**
- **PASS** — reproducible with available tools and data
- **FAIL** — not reproducible (experimental-only, proprietary data, hardware-specific)
- **PARTIAL** — reproducible with extra effort (missing data obtainable, tool extension needed)

Only PASS and PARTIAL figures count toward reproduction scope.

### Step 2 — Score Six Dimensions

Score each dimension 1–5 with a written justification. No score without justification.

#### D1 — Information Completeness (weight: 2×)

How much is stated vs. how much you must infer?

| Score | Meaning |
|-------|---------|
| 1 | Everything specified: model, parameters, software, version, grid, tolerances |
| 2 | Most stated; 1–2 items need reasonable inference |
| 3 | Core parameters stated; numerics/grid/tolerances missing |
| 4 | Significant gaps; must infer from related papers or supplementary material |
| 5 | Critical parameters missing; reproduction requires guesswork or author contact |

**What to check:** mechanism/model name and version, initial/boundary conditions with units, grid/mesh resolution, solver tolerances, time step or CFL, domain size, software name and version. Each missing item adds to the score.

#### D2 — Physics / Model Complexity (weight: 3×)

How many interacting phenomena must be captured?

| Score | Meaning |
|-------|---------|
| 1 | Single phenomenon, steady state, well-understood |
| 2 | 2 coupled phenomena or mild unsteadiness |
| 3 | 3+ phenomena with nontrivial interactions |
| 4 | Multi-scale or chaotic dynamics; sensitive to initial conditions |
| 5 | Turbulence, phase change, multi-physics coupling, or frontier methods |

#### D3 — Model / Mechanism Size (weight: 1×)

Cost per evaluation. Discipline-specific examples:

| Score | Combustion | Materials/DFT | Biology | AI/ML | Math |
|-------|-----------|---------------|---------|-------|------|
| 1 | ≤10 species | Single element cell | <100 sequences | <1B params, single GPU | Closed-form |
| 2 | 10–30 species | Small unit cell | 100–1K sequences | 1–7B, single GPU | Standard PDE |
| 3 | 30–60 species | Medium supercell | 1K–10K sequences | 7–30B, multi-GPU | Iterative solver |
| 4 | 60–100 species | Large slab + adsorbates | 10K–100K seqs, phylogeny | 30–70B, multi-node | High-dim optimization |
| 5 | >100 species or DNN surrogate | AIMD / large-scale screening | WGS / population dynamics | >70B or RL training | Open conjecture formalization |

#### D4 — Computational Cost (weight: 2×)

Total wall time across all cases.

| Score | Meaning |
|-------|---------|
| 1 | Minutes (< 1 hr total) |
| 2 | Hours (1–10 hr) |
| 3 | Overnight (10–100 hr) |
| 4 | Days (100–1000 hr) |
| 5 | Weeks+ (> 1000 hr or requires HPC allocation) |

Count: (cases per figure) × (figures) × (wall time per case). List the calculation.

#### D5 — Post-Processing Complexity (weight: 1×)

How hard is it to get from raw output to the paper's plots/tables?

| Score | Meaning |
|-------|---------|
| 1 | Direct output (temperature profile, loss curve) |
| 2 | Simple derived quantity (flame speed from slope, accuracy from logits) |
| 3 | Multi-step pipeline (extract → smooth → fit → compare) |
| 4 | Statistical analysis, non-trivial regression, or custom visualization |
| 5 | Requires external tools (CEMA, topological analysis, formal verification) |

#### D6 — Code / Infrastructure Modification (weight: 3×)

How much new code must you write beyond running existing tools?

| Score | Meaning |
|-------|---------|
| 1 | Run existing tool with config change only |
| 2 | Write analysis scripts (< 200 lines) |
| 3 | Modify tool settings + write moderate code (200–500 lines) |
| 4 | Implement new physics module, boundary condition, or training loop |
| 5 | Build new solver component or integrate multiple tools end-to-end |

### Step 3 — Compute WTS

```
WTS = 2×D1 + 3×D2 + 1×D3 + 2×D4 + 1×D5 + 3×D6
```

**Range: 12–60.** Interpret as:

| WTS Range | Category | Estimated Time |
|-----------|----------|---------------|
| 12–18 | Straightforward | < 1 week |
| 19–28 | Moderate | 1–2 weeks |
| 29–38 | Challenging | 3–4 weeks |
| 39–48 | Very Challenging | 1–2 months |
| 49–60 | Research-Grade | 2+ months |

### Step 4 — Write the WTS Report

Output as `EVALUATION.md` in the paper directory:

```markdown
# Paper Reproduction Difficulty Evaluation

**Paper:** <full citation>
**Date scored:** <YYYY-MM-DD>

## D0 — Solver Feasibility
<figure table from Step 1>

## D1 — Information Completeness: <score>/5
<justification>

## D2 — Physics / Model Complexity: <score>/5
<justification>

## D3 — Model / Mechanism Size: <score>/5
<justification>

## D4 — Computational Cost: <score>/5
<case count calculation>

## D5 — Post-Processing Complexity: <score>/5
<justification>

## D6 — Code Modification Required: <score>/5
<justification with line-count estimates>

## Weighted Total Score
WTS = 2×<D1> + 3×<D2> + 1×<D3> + 2×<D4> + 1×<D5> + 3×<D6> = **<total>**

## Category: <name> (<range>)
Estimated time: <estimate>

## Risks and Mitigation
<numbered list of top 3–5 risks, each with impact, probability, and mitigation>

## Reproduction Plan
<phased plan: what to reproduce first, what to defer, what to skip>
```

## Principles

- **Score conservatively.** Underestimating difficulty wastes more time than overestimating. If between two scores, pick the higher one.
- **Justify every score.** A bare number is useless. The justification is the value — it tells the reproducer what to watch for.
- **D0 is a gate, not a score.** FAIL figures are excluded from scope. Don't penalize the paper for having experimental figures.
- **Cost estimation is multiplicative.** 5 figures × 20 cases × 10 min/case = 1000 min ≈ 17 hr → D4 = 2. Show the math.
- **D6 dominates.** A paper that needs new solver code (D6 = 4–5) is hard regardless of other dimensions. Weight 3× reflects this.

## Common Pitfalls

- **Confusing D2 (physics) with D3 (mechanism size).** A small mechanism with complex flame-turbulence interaction is D2 = 5, D3 = 1. A large mechanism with simple steady flame is D2 = 1, D3 = 4.
- **Ignoring supplementary material.** Many papers bury critical parameters in supplements. Check before scoring D1 high.
- **Underestimating D4 by counting only one case.** Parameter sweeps multiply cost. A "simple" paper with 8 pressures × 10 equivalence ratios × 5 temperatures = 400 cases.
- **Not checking mechanism availability.** A paper using a published mechanism that has no YAML file may need D6 = 2 just for format conversion.
