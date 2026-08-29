---
name: grade-reproduction
description: "Grade a completed paper reproduction using the Reproduction Quality Score (RQS, 0-110 pts): evaluate figure coverage, model fidelity, quantitative and qualitative agreement, report completeness, and divergence documentation, then assign a letter grade (A+ to F) with improvement recommendations. Trigger on: 'grade my reproduction', 'rate this reproduction', 'compute RQS', 'quality score', 'evaluate my results', 'how good is this reproduction', 'score my work', 'letter grade for reproduction', 'quality assessment', 'is my reproduction good enough'. Also activates when a user finishes reproducing a paper and wants an objective evaluation, or when comparing reproduction quality across multiple papers."
---

# /grade-reproduction — Post-Reproduction Quality Grading (RQS)

Grade a completed reproduction's quality using the Reproduction Quality Score (RQS, 0–110 pts). Evaluates 6 dimensions, assigns a letter grade (A+ to F), and produces actionable improvement recommendations.

Distilled from 78 paper reproductions in the ASURF project (2024-2026). Works for any computational discipline.

## Trigger

User mentions: "grade reproduction", "rate my reproduction", "RQS", "quality score", "evaluate results", "how good is this reproduction", "score reproduction", "quality assessment", "letter grade".

## Workflow

### Step 1 — Collect Evidence

Gather all reproduction artifacts:
- Progress report (`<author><year>_progress_report.md`)
- Reproduction figures vs. paper figures (side-by-side)
- Numerical data files and digitized paper data
- Trace logs (`trace.md`)
- Scripts and configuration files
- `meta.yaml` status (if available)
- WTS / `EVALUATION.md` (if available — provides D0 feasibility for Q1 denominator)

### Step 2 — Score Six Quality Dimensions

Score each 0–10 with justification. Deduct points with explicit reasons.

#### Q1 — Figure Coverage (weight: 2×)

What fraction of reproducible figures (D0 = PASS or PARTIAL) were actually reproduced?

| Score | Meaning |
|-------|---------|
| 10 | 100% of reproducible figures completed |
| 8 | 80–99% completed |
| 6 | 60–79% completed |
| 4 | 40–59% completed |
| 2 | 20–39% completed |
| 0 | < 20% completed |

Figures skipped with valid justification (not computational, missing data) do not count against coverage.

#### Q2 — Model / Mechanism Fidelity (weight: 2×)

Does the reproduction use the same model as the paper?

| Score | Meaning |
|-------|---------|
| 10 | Exact model match, validated against known references |
| 8 | Same model, minor format differences (e.g., CHEMKIN → YAML conversion) |
| 6 | Same model family, different version or parameterization |
| 4 | Different but related model (e.g., surrogate mechanism, different pretrained weights) |
| 2 | Substantially different model |
| 0 | Wrong model entirely |

#### Q3 — Quantitative Agreement (weight: 3×)

How close are the numbers? This is the highest-weighted dimension.

| Score | Meaning |
|-------|---------|
| 10 | RMS error < 2%, all points within 5% |
| 9 | RMS error < 5%, all points within 10% |
| 8 | RMS error < 5%, most points within 10% |
| 7 | RMS error 5–10%, correct magnitudes |
| 6 | RMS error 10–15%, correct order of magnitude |
| 5 | RMS error 15–20% |
| 3 | RMS error 20–50%, trends correct but magnitudes off |
| 1 | RMS error > 50%, major discrepancies |
| 0 | Results contradict paper |

**Required metrics** (choose appropriate ones):
- **Curves/profiles**: RMS error, max error, mean relative error, R², points within N% tolerance
- **Scalars**: absolute error + relative error
- **Correlations**: R², slope, intercept
- **Multiple conditions**: mean and worst-case error across all conditions

If paper data is not digitized, state that explicitly and score based on visual comparison (cap at 7 without quantitative metrics).

#### Q4 — Qualitative Agreement (weight: 2×)

Do the shapes, trends, and orderings match?

| Score | Meaning |
|-------|---------|
| 10 | All physical features, trends, curve shapes, and orderings match |
| 8 | Most features match; minor visual differences |
| 6 | Key trends correct; some features missing or shifted |
| 4 | Partial trend agreement; notable feature mismatches |
| 2 | Few features match |
| 0 | Contradicts paper's qualitative findings |

**Check**: peak locations, curve ordering under parameter sweeps, monotonicity, asymptotic behavior, phase transitions, scaling regimes.

#### Q5 — Report Completeness (weight: 1×)

Does the progress report follow the template and contain all required elements?

Checklist (1 point each):
1. Progress report exists and is current
2. Setup table with full provenance (tool, version, mechanism, conditions, grid, tolerances, run command, wall time)
3. Side-by-side figure comparison for every reproduced figure
4. Quantitative divergence metrics table
5. Uncertainty budget (digitization, numerical, model version, domain/BCs)
6. Sources-of-divergence analysis
7. Key takeaways as transferable general rules
8. Reproduction summary table with per-figure status
9. Self-contained (reader doesn't need trace.md or paper)
10. PDF report generated

#### Q6 — Divergence Documentation (weight: 1×)

Are discrepancies explained, classified, and quantified?

| Score | Meaning |
|-------|---------|
| 10 | Every discrepancy classified (expected/unexplained), quantified, and explained |
| 8 | Most discrepancies documented with quantitative metrics |
| 6 | Major discrepancies documented; minor ones missing |
| 4 | Some discrepancies noted but not classified or quantified |
| 2 | Discrepancies acknowledged without analysis |
| 0 | No divergence documentation |

**Discrepancy classifications:**
- **Expected**: tool differences, digitization error, known model limitations
- **Unexplained**: no clear cause identified — flag for investigation
- **Paper error**: reproduction reveals likely error in original paper

### Step 3 — Compute RQS

```
RQS = 2×Q1 + 2×Q2 + 3×Q3 + 2×Q4 + 1×Q5 + 1×Q6
```

**Maximum: 110.** Assign grade:

| RQS Range | Grade | Meaning |
|-----------|-------|---------|
| 100–110 | A+ | Exceptional — publication-ready |
| 85–99 | A | Solid — minor improvements possible |
| 70–84 | B+ | Adequate — correct trends, moderate quantitative gaps |
| 55–69 | B | Partial — significant gaps but core claims validated |
| 40–54 | C | Weak — major issues, claims not convincingly supported |
| < 40 | F | Fail — critical deficiencies, unreliable results |

### Step 4 — Write the RQS Report

Output as `evaluation.md` in the paper directory:

```markdown
# Reproduction Quality Assessment — <paper-id>

| | |
|---|---|
| **Paper** | <full citation> |
| **Status** | <COMPLETE / PARTIAL (N/M figures)> |
| **Scored** | <YYYY-MM-DD> |

---

## Q1 — Figure Coverage: <score>/10 (weight 2×) → <weighted>
<justification with figure-by-figure status>

## Q2 — Model Fidelity: <score>/10 (weight 2×) → <weighted>
<justification with mechanism/model comparison>

## Q3 — Quantitative Agreement: <score>/10 (weight 3×) → <weighted>
<metrics table: figure, metric, paper value, reproduction value, deviation>

## Q4 — Qualitative Agreement: <score>/10 (weight 2×) → <weighted>
<list of physical features checked>

## Q5 — Report Completeness: <score>/10 (weight 1×) → <weighted>
<checklist with ✅/❌>

## Q6 — Divergence Documentation: <score>/10 (weight 1×) → <weighted>
<assessment of discrepancy handling>

---

## Score

| Dimension | Raw | Weight | Weighted |
|-----------|-----|--------|----------|
| Q1 — Figure Coverage | /10 | 2× | |
| Q2 — Model Fidelity | /10 | 2× | |
| Q3 — Quantitative Agreement | /10 | 3× | |
| Q4 — Qualitative Agreement | /10 | 2× | |
| Q5 — Report Completeness | /10 | 1× | |
| Q6 — Divergence Documentation | /10 | 1× | |
| **TOTAL** | | | **<total> / 110** |

**GRADE: <letter>**

---

## Improvement Recommendations

<Numbered list. For each recommendation:
- What to do
- Which dimension it improves (Q1–Q6)
- Expected score impact (+N points)
- Estimated effort (hours)>

---

## Comparison to Reference Reproductions

| Paper | RQS | Grade |
|-------|-----|-------|
| <this paper> | <score> | <grade> |
| <reference 1> | <score> | <grade> |
| <reference 2> | <score> | <grade> |
```

## Principles

- **Numbers, not adjectives.** "Good agreement" is not a score. "RMS error 2.3%, 21/23 points within 5%" is.
- **Justify every deduction.** A score of 7/10 must explain where the 3 points went.
- **Grade the reproduction, not the paper.** A brilliant paper can have a poor reproduction; a simple paper can have a perfect one.
- **Be calibrated.** A+ means publication-ready. Most honest reproductions land at B+ to A. Reserve A+ for < 2% error and complete coverage.
- **Don't inflate.** Self-scored evaluations tend to be 10–15 points too generous. If in doubt, round down and add improvement recommendations.
- **Flag critical deficiencies.** If Q3 < 4 (quantitative agreement fundamentally wrong) or Q2 < 4 (wrong model), flag the evaluation regardless of total RQS.

## Common Pitfalls

- **Scoring qualitative-only as quantitative.** If you didn't digitize the paper's data, Q3 is capped at 7. Visual "looks close" is not quantitative agreement.
- **100% coverage ≠ perfect reproduction.** Coverage (Q1) counts figures attempted, not quality. A paper with 6/6 figures but 30% RMS error is still a low-quality reproduction.
- **Ignoring the uncertainty budget.** A 5% deviation against a 10% measurement uncertainty is excellent agreement. A 5% deviation against a 0.1% numerical tolerance is a problem.
- **Confusing mechanism version with mechanism identity.** GRI-Mech 3.0 in CHEMKIN vs Cantera YAML is a minor format difference (Q2 = 8), not a different mechanism (Q2 = 4).

## ARM Bundle Scorecard Integration

When grading an ARM bundle (attempt with `bundleStatus != null`), produce both the traditional RQS **and** a multi-dimensional scorecard. The scorecard maps RQS dimensions to a normalized 0.0–1.0 scale for machine consumption.

### Scorecard Schema

Output the scorecard as JSON (stored in `attempt.scorecard_json`):

```json
{
  "packaging": 0.95,
  "executability": 0.80,
  "output_coverage": 0.60,
  "result_fidelity": 0.72,
  "environment_reproducibility": 0.90,
  "trace_quality": 0.85,
  "rqs_total": 78,
  "rqs_grade": "B+"
}
```

### Dimension Mapping (RQS → Scorecard)

| Scorecard Dimension | Source | How to Compute |
|---------------------|--------|----------------|
| `packaging` | Bundle validation | `(has_manifest + has_readme + has_dockerfile + has_requirements + has_src + has_results) / 6` |
| `executability` | Dockerfile + entrypoint | 1.0 if Dockerfile builds successfully; 0.5 if Dockerfile present but untested; 0.0 if missing |
| `output_coverage` | Q1 (Figure Coverage) | `Q1 / 10` |
| `result_fidelity` | Q3 (Quantitative) + Q4 (Qualitative) | `(3*Q3 + 2*Q4) / 50` (weighted average of the two highest-weight dimensions) |
| `environment_reproducibility` | Q2 (Model Fidelity) | `Q2 / 10` |
| `trace_quality` | Trace completeness | 1.0 if typed trace with 5+ steps; 0.5 if trace exists but minimal; 0.0 if no trace |
| `rqs_total` | Full RQS formula | `2*Q1 + 2*Q2 + 3*Q3 + 2*Q4 + 1*Q5 + 1*Q6` |
| `rqs_grade` | Grade bands | A+ / A / B+ / B / C / F |

### Workflow Extension for ARM Bundles

After computing Q1–Q6 and the RQS, add **Step 4b**:

#### Step 4b — Generate Scorecard JSON

1. Read the bundle's existing scorecard (from `attempt.scorecard_json`) if it has packaging/executability scores from upload validation
2. Merge in the grading-derived dimensions (output_coverage, result_fidelity, environment_reproducibility, trace_quality)
3. Add rqs_total and rqs_grade
4. Write the merged scorecard to `scorecard.json` in the evaluation directory and update the attempt's `scorecard_json` field via API

```bash
# Update attempt scorecard via API
curl -X PATCH /api/attempts/{id} \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"scorecard_json": "{...}"}'
```

### Maturity Levels (from Scorecard)

The scorecard maps to ARM maturity levels for display:

| Level | Name | Criteria |
|-------|------|----------|
| 0 | Seed | `packaging < 0.3` — minimal structure |
| 1 | Sprout | `packaging >= 0.3` and `output_coverage > 0` — has manifest + some results |
| 2 | Rooted | `packaging >= 0.6` and `executability >= 0.5` — reproducible environment |
| 3 | Verified | All dimensions >= 0.5 and `rqs_grade` in (A+, A, B+) — independently graded |
