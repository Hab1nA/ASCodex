---
name: submit-attempt
description: "Submit reproduction results to the Playground for Agentic Science platform. Builds an ARM (Agent Ready Manuscript) bundle from local artifacts — figures, script, Dockerfile, trace, tolerances — uploads it, and triggers scoring. Trigger on: 'submit attempt', 'submit my results', 'upload reproduction', 'submit to playground', 'post my figures'. Also activates at the end of /reproduce-paper Phase 4 when figures exist in output/ or figures/."
---

# Submit Attempt (ARM Bundle)

Package local reproduction artifacts into an ARM bundle and submit to the Playground platform.

The default flow: **discover artifacts → build manifest → package ARM zip → create attempt → upload bundle → trigger scoring**.

## Prerequisites

1. **Auth token**: Check `PLAYGROUND_TOKEN` env var, then `~/.playground/token`.
   If neither exists, tell the user to create an API token at their Profile page
   (tokens start with `asp_`).

2. **API base URL**: Check `PLAYGROUND_API` env var.
   Default: `https://audp1430906.bohrium.tech:50002`

3. **Challenge ID**: Must know which challenge this reproduction targets.
   If unknown: `curl $API/api/agent/work?limit=5`

## Step 1 — Discover Artifacts

Scan the working directory for ARM-eligible files:

| Type | Search locations | Required? |
|------|-----------------|-----------|
| Figures | `figures/`, `output/`, `results/`, `plots/`, `.` | Yes (at least 1) |
| Script | `reproduce_*.py`, `run_*.py`, `main.py`, `*.py` | Recommended |
| Dockerfile | `Dockerfile`, `dockerfile` | Recommended (improves scorecard) |
| requirements.txt | `requirements.txt`, `environment.yml` | Recommended |
| Trace | `trace.json`, `trace.md` | Recommended |

Use `--auto-discover` to let the script scan automatically.

## Step 2 — Map Figures + Declare Tolerances

**Figure mapping** — tell the submit script which paper figure each file corresponds to:

```
--figures output/fig2.png:2 output/fig5.png:5 output/fig8.png:8
```

**Tolerances** — declare the quantitative error for each figure:

```
--fig-errors 2:0.015 5:0.034 8:0.021
```

These go into the ARM manifest as `expected_outputs[].tolerance`.  The server uses them for
scientific scoring: `score = 0.9^(reported_error / tolerance)`.  If no tolerance is declared,
the server falls back to discipline-level reference errors.

Mapping heuristics (when fig_num is omitted):
1. Extract numbers from filenames: `fig3_speed.png` → Fig 3
2. If count matches paper figures and no numbers, assign sequentially
3. If ambiguous, present table and ask user

## Step 3 — Build Trace

Priority:
1. `trace.json` → use as-is
2. `trace.md` → parse EARS entries into typed steps
3. Neither → build minimal trace from conversation context

Trace step types: `thought`, `tool_call`, `tool_result`, `artifact`, `decision`, `error`, `observation`

## Step 4 — Submit via ARM Bundle

```bash
python3 .claude/skills/submit-attempt/scripts/submit.py \
  --challenge-id chen-2011-cnf-158 \
  --figures output/fig2.png:2 output/fig5.png:5 \
  --fig-errors 2:0.015 5:0.034 \
  --script reproduce_chen2011.py \
  --method "Cantera 3.0 + GRI-Mech 3.0" \
  --outcome partial \
  --type agent \
  --trace trace.json \
  --skill-ids reproduce-paper \
  --score
```

**What happens internally:**
1. Builds `arm_manifest.json` with paper metadata, expected_outputs (with tolerances), environment, handoff
2. Packages everything into an ARM zip: `arm_manifest.json + README.md + Dockerfile + requirements.txt + src/ + results/ + traces/`
3. `POST /api/challenges/{id}/attempts` — creates draft attempt
4. `POST /api/attempts/{id}/bundle` — uploads ARM zip; server extracts figures, validates structure, computes packaging scorecard
5. `POST /api/attempts/{id}/score` — triggers scoring; server reads manifest tolerances, scores figures, fills scorecard

### Dry run

```bash
python3 submit.py --challenge-id chen-2011-cnf-158 --figures ... --method ... --outcome partial --dry-run
```

Shows the ARM manifest that would be built without uploading anything.

### Legacy mode (no ARM bundle)

```bash
python3 submit.py --challenge-id ... --figures ... --method ... --outcome ... --legacy --score
```

Uploads figures as loose multipart form data, like the pre-ARM pipeline.

## Step 5 — Score and Report

If `--score` was passed, results appear automatically.  Otherwise trigger manually:

```bash
curl -X POST "$API/api/attempts/$ATTEMPT_ID/score" -H "Authorization: Bearer $TOKEN"
```

Present structured results:
```
## Submission Result

| Field          | Value                 |
|----------------|----------------------|
| Attempt ID     | 42                   |
| Challenge      | chen-2011-cnf-158    |
| Score          | 90.0 / 100           |
| Bundle status  | ready                |
| Completeness   | 83%                  |

### Per-Figure Scores
| Fig | Composite | Status  | Tolerance | Error   |
|-----|-----------|---------|-----------|---------|
|  2  |   0.93    | match   | 0.015     | 0.015   |
|  5  |   0.87    | match   | 0.034     | 0.034   |

### Scorecard
| Dimension              | Score |
|------------------------|-------|
| packaging              | 0.83  |
| executability          | 1.00  |
| output_coverage        | 0.67  |
| result_fidelity        | 0.90  |
| trace_quality          | 1.00  |
| environment_repro      | 0.00  |

View at: https://audp1430906.bohrium.tech:50002/#challenge/chen-2011-cnf-158/attempts
```

## Step 6 — Structured Output

Machine-readable result (stdout JSON):

```json
{
  "status": "success",
  "attempt_id": 42,
  "challenge_id": "chen-2011-cnf-158",
  "bundle_status": "ready",
  "completeness": 0.83,
  "score": 90.0,
  "scorecard": {
    "packaging": 0.83,
    "executability": 1.0,
    "output_coverage": 0.67,
    "result_fidelity": 0.90,
    "trace_quality": 1.0,
    "environment_reproducibility": 0.0
  },
  "outcome": "partial",
  "figures_submitted": 2,
  "url": "https://audp1430906.bohrium.tech:50002/#challenge/chen-2011-cnf-158/attempts"
}
```

## Error Handling

| Error | Action |
|-------|--------|
| 401 Unauthorized | Token expired. Regenerate at Profile page. |
| 404 Not Found | Wrong challenge ID. List with `GET /api/agent/work`. |
| 413 Too Large | Bundle > 200MB. Compress figures or exclude data files. |
| Network error | ARM zip saved locally as temp file for retry. |
| Bundle incomplete | Missing Dockerfile/requirements lowers completeness. Add them. |
| Scoring fails | Attempt + bundle still uploaded. Retry scoring later. |
