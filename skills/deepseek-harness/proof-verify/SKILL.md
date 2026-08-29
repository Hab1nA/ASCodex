---
name: proof-verify
description: "Translate mathematical proofs from papers into machine-checkable Lean 4 code: comprehend proof structure, audit Mathlib coverage, build a skeleton with sorry placeholders, fill in tactic proofs bottom-up, and report verification gaps. Trigger on: 'verify this proof', 'formalize the theorem', 'check in Lean 4', 'proof assistant', 'is this proof correct', 'machine-checkable proof', 'formalize with Mathlib', 'Lean verification', 'how many sorrys remain', 'translate proof to Lean', 'formal verification of theorem'. Also activates when a user suspects a published proof has gaps, wants a reproducibility certificate for a mathematical result, or needs to identify unstated hypotheses in an informal proof."
---

# /proof-verify — Formal Proof Verification

Translate mathematical proofs from papers into machine-checkable form using Lean 4. Covers analysis, algebra, topology, and combinatorics. Identifies gaps in informal proofs, highlights unverifiable steps, and produces a verified artifact that can be added to Mathlib or used as a reproducibility certificate.

## Trigger

User mentions: "verify proof", "formalize proof", "Lean 4", "proof assistant", "check theorem", "formalize", "proof verification", "machine-checkable", "Mathlib".

## Workflow

### Step 1 — Proof Comprehension

Before touching Lean:

1. **Read the full proof** in the paper. Understand the logical structure, not just the symbols.
2. **Identify the proof strategy**: direct, contradiction, induction, construction, diagonal argument, etc.
3. **Map dependencies**: which lemmas, theorems, and definitions are cited? Draw a dependency DAG:

```
Main Theorem
├── Lemma 3.1 (proved in paper)
│   ├── Definition 2.2
│   └── Proposition 2.5 (cited, external)
├── Lemma 3.2 (proved in paper)
│   └── Theorem A (cited, Rudin 1976)
└── Standard results (Bolzano-Weierstrass, etc.)
```

4. **Flag informal gaps**: phrases like "it is easy to see", "by a standard argument", "a straightforward computation shows" — these are where formalization will be hardest.

### Step 2 — Mathlib Audit

Check what already exists in Lean 4 / Mathlib:

1. **Search Mathlib** for the main definitions and theorems used
2. **Catalog availability**:

| Component | Mathlib Status | Action |
|-----------|---------------|--------|
| Metric spaces | `Mathlib.Topology.MetricSpace.Basic` | Use directly |
| Compactness | `Mathlib.Topology.Compactness.IsCompact` | Use directly |
| Custom norm space | Not in Mathlib | Must formalize |
| Paper's Lemma 3.1 | Not in Mathlib | Must prove |

3. **Estimate effort**: each missing component adds formalization work. Prioritize the main theorem path.

### Step 3 — Skeleton Translation

Build the Lean 4 structure top-down:

```lean
import Mathlib

/-- Main theorem from [Author, Year], Theorem X.Y -/
theorem main_result
    (h1 : condition_1)
    (h2 : condition_2) :
    conclusion := by
  -- Step 1: Apply Lemma 3.1
  have step1 := lemma_3_1 h1
  -- Step 2: Apply Lemma 3.2
  have step2 := lemma_3_2 h2 step1
  -- Step 3: Combine
  sorry  -- TODO: complete this step
```

Use `sorry` as a placeholder for unfinished proofs. The goal is to get the full structure compiling before filling in details.

### Step 4 — Fill in Proofs

Work bottom-up through the dependency DAG:

1. **Start with leaves** (definitions, simple lemmas)
2. **Progress upward** to the main theorem
3. **For each step**:
   - Try `exact?`, `apply?`, `simp?`, `omega`, `linarith`, `norm_num` first
   - If tactics fail, break into smaller sub-goals
   - If a step requires domain-specific reasoning, use `calc` blocks for readability
4. **Track `sorry` count**: zero sorrys = fully verified

### Step 5 — Gap Analysis

For any step that cannot be formalized:

| Step | Paper Says | Formalization Status | Diagnosis |
|------|-----------|---------------------|-----------|
| Lemma 3.1 | "by compactness" | Verified | Clean |
| Lemma 3.2, eq. (7) | "straightforward" | `sorry` | Requires 30-line argument the paper omits |
| Thm 4.1, step 3 | "it follows that" | `sorry` | Appears to need an unstated hypothesis |

Classify each gap:
- **Routine**: formalizable but tedious (the paper is correct, Lean just needs more detail)
- **Non-trivial**: requires mathematical insight not present in the paper
- **Potential error**: the step may be incorrect or require additional hypotheses

### Step 6 — Report

```markdown
## Verification Summary

**Paper**: [Author et al., Year, Journal]
**Theorem**: [Main result]
**Lean 4 / Mathlib version**: [version]

### Status: [Fully Verified / Partially Verified / Gap Found]

### Sorry Count: N/M statements

### Verified Components
- [List of fully verified lemmas/theorems]

### Unverified Gaps
- [Each gap with classification and explanation]

### Discoveries
- [Any errors, unstated assumptions, or simplifications found during formalization]
```

## Principles

- **Formalize the structure first, details second.** A skeleton with sorrys is more valuable than a single fully-proved leaf lemma.
- **Trust but verify.** Assume the paper is correct until Lean disagrees.
- **Minimize sorry, don't eliminate at all costs.** A sorry with a clear explanation is better than an incorrect proof.
- **Use Mathlib idioms.** Don't reinvent what Mathlib provides. Follow Mathlib naming and style conventions.
- **Document the mapping.** Every Lean statement should reference the corresponding paper statement (theorem number, page, equation).

## Common Pitfalls

- **Definitional mismatches.** The paper's definition of "compact" may differ subtly from Mathlib's. Check equivalences.
- **Universe issues.** Lean's universe polymorphism can cause unexpected type errors. Use `Type*` unless you need specific universes.
- **Classical reasoning.** Some proofs require `Classical.choice` or `Decidable` instances. Import `Mathlib.Tactic` early.
- **Notation overload.** Paper notation (∑, ∫, ‖·‖) must map to specific Lean notations. Ambiguity causes errors.
- **Typeclass inference loops.** Complex algebraic hierarchies can cause Lean to hang. Use `@explicit_function` to bypass.
