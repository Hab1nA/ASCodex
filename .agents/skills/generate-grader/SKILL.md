---
name: generate-grader
description: "Read a hackathon topic markdown file, extract scoring criteria and reference values, and generate a programmatic grader function for `server/services/hackathon_scoring.py`. Use this skill whenever: a new hackathon challenge is added and needs a grader, the user says 'generate grader', 'add scoring for this topic', 'create grader from topic', 'new hackathon challenge', or 'programmatic scoring'. Also activates when the user points to a topic markdown and asks for automated scoring."
---

# /generate-grader — Hackathon Topic → Programmatic Grader

> **ZCode 注记（2026-09-04）**：本技能面向**源项目的评分服务源码**（`server/services/hackathon_scoring.py`），该路径在当前仓库不存在，平台判分器也不由我们维护——解题会话**不要**用本技能去"改判分器"；自建 verifier 走 `ascodex-solve` 开场六步。仅当用户明确要求做离线评分器工程时，把输出目标改为用户指定目录。

Read a hackathon topic markdown, extract its scoring criteria and reference values, and generate a complete grader function that plugs into the existing scoring engine.

## Context

The Playground hackathon scoring engine lives in `server/services/hackathon_scoring.py`. Each challenge has a pure grader function:

```python
def grade_xxx(metrics: dict[str, Any]) -> tuple[float, dict]:
    """Grade <challenge> reproduction.

    Expected metrics::
        { ... }
    """
    # scoring logic
    return _clamp(score), details
```

Functions are registered in the `HACKATHON_GRADERS` dict:

```python
HACKATHON_GRADERS = {
    'challenge-id': grade_xxx,
    ...
}
```

The **topic markdown files** (in `/home/developer/hackathon-1/topics/` or user-specified path) are the **source of truth** for scoring criteria. They contain:

- Reference values (tables with expected numerical outputs)
- Pass/Good/Excellent thresholds
- Error tolerance specifications (relative error, absolute error, R², etc.)
- Scoring weight breakdowns
- Expected metric names and formats

## Trigger

User mentions: "generate grader", "add scoring", "new hackathon challenge", "create grader from topic", "programmatic scoring for topic", or points to a topic `.md` file and asks for automated evaluation.

## Workflow

### Phase 1 — Read & Understand the Topic

1. **Read the topic markdown** file end-to-end
2. **Extract these elements** (create a structured summary):

| Element | Where to Look | Example |
|---------|---------------|---------|
| Challenge name | Title / `## 论文信息` table | "Huggett 1993 JEDC" |
| Challenge ID | Derive from `<first-author>-<year>-<journal>-<keyword>` | `huggett-1993-jedc` |
| Reference values | Tables in `## 目标` or `## 参考数据` | 8 cases of (γ, a_min) → (q, r) |
| Scoring criteria | `## 验证标准` section | "q relative error < 1%, r absolute error < 2pp" |
| Pass thresholds | Often in `## Milestone` sections | "R² ≥ 0.90 = Pass, ≥ 0.95 = Excellent" |
| Expected metrics format | From `## 快速开始` code blocks or milestone outputs | `{"r2_si": float, "r2_ge": float}` |
| Scoring dimensions | Multiple criteria often split 60/40 or by component | "q scoring: 60%, r scoring: 40% per case" |

3. **Classify the scoring type**:

| Type | Description | Examples |
|------|-------------|---------|
| **scalar-comparison** | Compare submitted scalars against reference values | Huggett (q, r), Aiyagari (r, saving_rate) |
| **goodness-of-fit** | R², RMSE, or similar statistical measures | RustBCA (R² for Si/Ge), Wind Wake (R²) |
| **threshold-based** | Binary pass/fail on specific criteria | DeePMD (training completed, LAMMPS ran) |
| **ratio-comparison** | Ratio between two submitted values must meet threshold | Hi-C CTCF (intersection/mnase peak ratio) |
| **composite** | Mix of the above | DeePMD (threshold + scalar + bonus) |
| **image-based** | Requires visual/multimodal comparison | Future topics (see Phase 1b) |

### Phase 1b — Image-Based Challenges (Special Handling)

If the topic's scoring criteria involve **visual comparison** (e.g., "profile plot must show X trend", "figure must match paper Fig N"):

1. **Check if quantitative proxies exist.** Many "image" challenges actually have underlying numerical criteria:
   - "CTCF peak > 2× MNase peak" → extract peak values, compare numerically
   - "Profile trends must match" → extract curve data, compute correlation

2. **If purely visual with no quantitative proxy:**
   - Generate a grader that accepts **extracted numerical features** (peak heights, curve slopes, trend directions) rather than raw images
   - Document in the grader docstring what preprocessing the submitter must do
   - Add a `# TODO: multimodal evaluation` comment noting that a future LLM-based visual grader could handle this

3. **Never generate a grader that requires runtime LLM calls.** All graders must be pure functions with deterministic output.

### Phase 2 — Design the Grader

Before writing code, produce a **grader design document** (print to stdout, don't save to file):

```
## Grader Design: <challenge-id>

### Metrics Schema
Expected input:
{
    "metric_name_1": <type>,
    "metric_name_2": <type>,
    ...
}

### Reference Values
<Table of reference values extracted from topic>

### Scoring Logic
- Component 1 (weight: X%): <description>
  - Threshold 1: <condition> → <points>
  - Threshold 2: <condition> → <points>
- Component 2 (weight: Y%): <description>
  ...

### Score Distribution
- 0: <what this means>
- 60: <what this means>
- 85: <what this means>
- 100: <what this means>

### Edge Cases
- Missing metrics: <handling>
- Out-of-range values: <handling>
- Partial submission: <handling>
```

**Ask the user to confirm the design before proceeding to Phase 3.** If running non-interactively (e.g., in a pipeline), proceed with the design.

### Phase 3 — Generate the Grader Code

Generate the following artifacts:

#### 3a. Reference Constants

If the topic has reference values, add a module-level dict:

```python
# Source: /path/to/topic.md
XXX_REFERENCE = {
    (param1, param2): {'metric_a': value, 'metric_b': value},
    ...
}
```

**Critical**: Copy values EXACTLY from the topic markdown. Double-check every number.

#### 3b. Grader Function

Follow the established pattern:

```python
def grade_xxx(metrics: dict[str, Any]) -> tuple[float, dict]:
    """Grade <Challenge Name> reproduction.

    Expected metrics::

        {
            "metric_1": <float>,  # description
            "metric_2": <float>,  # description
            ...
        }

    Scoring per topic spec:
    - <brief scoring summary>
    """
    details: dict[str, Any] = {}

    # 1. Parse and validate inputs
    # 2. Score each component
    # 3. Combine scores with weights

    return _clamp(total_score), details
```

**Code style rules:**
- Use existing helpers: `_to_float()`, `_to_int()`, `_to_bool()`, `_clamp()`
- Always populate `details` dict with submitted values, reference values, and per-component scores
- Handle missing metrics gracefully (return 0 + error message, don't crash)
- Support multiple input formats (nested dicts, flat keys) like the Huggett/Aiyagari graders
- Add parser helpers if needed (e.g., `_parse_xxx_values()`)
- Comment the scoring thresholds with their source (e.g., `# Topic spec: R² ≥ 0.90 = Pass`)

#### 3c. Registry Entry

Add to `HACKATHON_GRADERS`:

```python
HACKATHON_GRADERS = {
    ...
    'new-challenge-id': grade_xxx,
}
```

#### 3d. Tests

Generate test cases in `tests/test_hackathon_scoring.py`:

```python
class TestXxxGrader:
    """Tests for grade_xxx()."""

    def test_perfect_reproduction(self):
        """Exact reference values → score ~100."""
        metrics = { ... }  # exact reference values
        score, details = grade_xxx(metrics)
        assert score >= 99.0

    def test_within_tolerance(self):
        """Values within tolerance → score > 60."""
        metrics = { ... }  # slightly off values
        score, details = grade_xxx(metrics)
        assert score > 60

    def test_partial_submission(self):
        """Only some metrics provided → partial score."""
        metrics = { ... }
        score, details = grade_xxx(metrics)
        assert 0 < score < 100

    def test_empty_metrics(self):
        """No relevant metrics → 0."""
        score, details = grade_xxx({})
        assert score == 0.0
        assert 'error' in details

    def test_grader_registered(self):
        """Challenge ID is in HACKATHON_GRADERS."""
        assert 'new-challenge-id' in HACKATHON_GRADERS
```

#### 3e. Challenge Registration (Optional)

If the challenge doesn't exist in `data/challenges.json` and `data/challenge-meta.json`, generate entries:

**challenges.json entry:**
```json
{
    "id": "new-challenge-id",
    "title": "...",
    "paper": "...",
    "discipline": "...",
    "origin": "hackathon",
    "track": "...",
    "estimatedMinutes": ...
}
```

**challenge-meta.json entry:**
```json
{
    "id": "new-challenge-id",
    "conditions": { ... },
    "physicsRequired": "...",
    "cost": "...",
    "pitfalls": ["..."],
    "gettingStarted": "..."
}
```

### Phase 4 — Apply Changes

1. **Edit** `server/services/hackathon_scoring.py`:
   - Add reference constants (after existing ones)
   - Add parser helpers (in Helpers section)
   - Add grader function (before HACKATHON_GRADERS)
   - Update HACKATHON_GRADERS dict

2. **Edit** `tests/test_hackathon_scoring.py`:
   - Add import if needed
   - Add test class
   - Update `test_hackathon_ids` if it exists

3. **Edit** `data/challenges.json` and `data/challenge-meta.json` if needed

4. **Run tests**: `cd <project_root> && python -m pytest tests/test_hackathon_scoring.py -v`

5. **Run full suite**: `python -m pytest tests/ -q` to verify no regressions

### Phase 5 — Report

Print a summary:

```
## Grader Generated: <challenge-id>

### Files Modified
- server/services/hackathon_scoring.py — added grade_xxx() + XXX_REFERENCE
- tests/test_hackathon_scoring.py — added TestXxxGrader (N tests)
- data/challenges.json — added challenge entry (if applicable)

### Scoring Summary
- Type: <scoring-type>
- Components: N
- Reference values: N cases
- Score range: 0–100

### Test Results
- All N tests passing ✓
```

## Existing Graders (Reference)

| Challenge ID | Grader | Type | Key Metrics |
|-------------|--------|------|-------------|
| `zhang-2018-prl-deepmd` | `grade_deepmd` | composite | energy_rmse, force_rmse, lammps_completed, training_completed |
| `wang-2024-pof-dg` | `grade_wind_wake` | goodness-of-fit | r_squared, sections_fitted |
| `hofsass-2014-rustbca` | `grade_rustbca` | goodness-of-fit | r2_si, r2_ge |
| `huggett-1993-jedc` | `grade_huggett` | scalar-comparison | q_values{}, r_values{} (8 cases) |
| `aiyagari-1994-qje` | `grade_aiyagari` | scalar-comparison | r_values{}, saving_rate_values{} (8 cases) |
| `akgol-2021-nm-hic` | `grade_hic_ctcf` | ratio-comparison | ctcf_intersection_peak, ctcf_mnase_peak |

## Scoring Primitives (Reusable Patterns)

These patterns appear across multiple graders. Reuse them:

### Linear Decay
```python
# < threshold_a → full points
# threshold_a to threshold_b → linear decay from full to partial
# > threshold_b → 0
if error < threshold_a:
    pts = max_pts
elif error < threshold_b:
    pts = max_pts * (1.0 - (error - threshold_a) / (threshold_b - threshold_a))
else:
    pts = 0.0
```

### Multi-Case Scoring
```python
# N reference cases, each worth 100/N points
n_cases = len(REFERENCE)
pts_per_case = 100.0 / n_cases
total = 0.0
for key, ref in REFERENCE.items():
    case_score = 0.0
    # Score component A (weight_a% of case points)
    # Score component B (weight_b% of case points)
    total += case_score
```

### Relative Error Check
```python
rel_err = abs(submitted - reference) / abs(reference) if reference != 0 else abs(submitted - reference)
```

### Absolute Error Check (for near-zero values)
```python
abs_err = abs(submitted - reference)
abs_err_pp = abs_err * 100  # convert to percentage points
```

### Bonus Points
```python
# Optional metric adds bonus (capped at 100 total)
if optional_metric is not None and optional_metric > threshold:
    score = min(100.0, score + bonus_pts)
```

## Principles

- **Topic markdown is the source of truth.** Every threshold, reference value, and scoring weight must trace back to the topic file. Add source comments.
- **Pure functions only.** No DB access, no network calls, no LLM calls. Graders must be deterministic.
- **Graceful degradation.** Missing metrics → partial score (not crash). Unknown fields → ignored.
- **Details are for debugging.** The `details` dict should contain enough info for a human to understand exactly how the score was computed.
- **Test with exact reference values.** The "perfect reproduction" test uses the topic's own reference values — if that doesn't score ~100, the grader has a bug.
- **Copy numbers carefully.** The #1 bug source is transcription errors in reference values. Always double-check against the topic markdown.

## Common Pitfalls

- **Unit mismatches**: Topic says "r in %" but grader expects decimal. Always check.
- **Key format variations**: Users submit `"(1.5, -2)"` or `"q_1.5_-2"`. Support both formats.
- **Division by zero**: Reference values can be 0 or negative. Guard relative error calculations.
- **Score > 100**: Multiple bonus components can push past 100. Always use `_clamp()`.
- **Forgetting to register**: Adding the function but not updating `HACKATHON_GRADERS` means it's dead code.
