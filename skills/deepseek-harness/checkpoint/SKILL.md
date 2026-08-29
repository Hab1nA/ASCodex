---
name: checkpoint
description: "Save current task progress to structured checkpoint file. Use when: session is ending, context window approaching limit, significant progress made, or before risky operation. Captures phase, parameters, experiments, decisions, blockers, and resume instructions."
version: 3.0.0
author: system
tags: [session-management, state-persistence, long-running-tasks]
---

# Checkpoint Skill

Save current task progress to a structured YAML checkpoint file that survives context loss.

## When to Use

- **Session ending**: Before closing session or switching tasks
- **Context window full**: Approaching token limit
- **Significant progress**: Completed a major phase or experiment
- **Before risky operations**: Before destructive changes or complex merges
- **Manual checkpoint**: User explicitly requests `/checkpoint`

## Step 1 — Gather current state

Review conversation history and files to extract:

**Phase tracking**:
- What phase are we in? (e.g., parameter_extraction, mesh_generation, convergence_testing, debugging, implementation)
- What phases are completed?
- What phases are next?

**Critical state**:
- **Parameters**: Extracted values with sources (paper page/table or "estimated from X") and confidence levels
- **Experiments**: What's been tried? Status (pending/in_progress/completed/failed/diverged), results paths, failure reasons
- **Decisions**: Key choices made, rationale, alternatives considered
- **Custom state**: Task-specific data (meshes, models, datasets, API endpoints, etc.)

**Blockers**:
- Open questions blocking progress
- Decisions needed
- Assumptions made that need verification

**Artifacts**: Critical files (configs, results, scripts) with roles

**Resume instructions**: Specific commands to run next session, conditions to verify

## Step 2 — Determine checkpoint filename

```bash
BRANCH=$(git branch --show-current)
TIMESTAMP=$(date -u +"%Y%m%d-%H%M%S")
CHECKPOINT_FILE=".claude/checkpoints/${BRANCH}-${TIMESTAMP}.yaml"
mkdir -p .claude/checkpoints
```

## Step 3 — Write checkpoint YAML

**Quality bar**:
- Every parameter must have `source` (paper page/table or "estimated from X")
- Every failed experiment must have `reason` (not just "didn't work")
- Every decision must have `rationale`
- Resume instructions must be actionable (specific commands or file checks, not vague "continue work")

**Example checkpoint**:

```yaml
checkpoint_version: '3.0.0'
created_at: 2026-04-14T18:00:00Z
branch: vk/5d01-ears

phase:
  current: convergence_testing
  completed: [parameter_extraction, mesh_generation]
  next: [refine_mesh_a, run_full_simulation]

state:
  parameters:
    - name: inlet_velocity
      value: 2.5
      unit: m/s
      source: "Table 2, page 4"
      confidence: high
    - name: turbulence_intensity
      value: 0.05
      unit: dimensionless
      source: "Estimated from Re=50000, not stated in paper"
      confidence: medium

  experiments:
    - name: convergence_sweep_mesh_a
      status: in_progress
      progress: 0.6
      results_path: results/mesh_a_sweep_partial.json
      next_step: "Continue from timestep 500"
    - name: mesh_b_test
      status: failed
      failed_at: t=0.003s
      reason: "Negative temperature at cell 45231, bad aspect ratio near inlet"

  decisions:
    - question: "Which wall function to use?"
      decision: "standard wall function"
      rationale: "Paper mentions 'standard k-epsilon' without specifying wall treatment"
      alternatives_considered: ["enhanced wall treatment", "scalable wall function"]
      confidence: medium

blockers:
  - question: "Inlet profile: uniform or 1/7th power law?"
    impact: medium
    assumption: "Using uniform for now"

artifacts:
  - path: system/controlDict
    role: config
  - path: results/mesh_a_residuals.csv
    role: data

resume_instructions: |
  1. Check if mesh_a convergence sweep finished:
     ls -lh results/mesh_a_sweep_final.json
  2. If finished: analyze results with analyze_convergence.py
  3. If not finished: resume from timestep 500:
     ./run_sweep.sh --resume 500
  4. Resolve blocker: check OpenFOAM docs for inlet profile
  5. Do NOT retry mesh_b without fixing geometry
```

## Step 4 — Validate and commit

```bash
# Check YAML syntax
python3 -c "import yaml; yaml.safe_load(open('$CHECKPOINT_FILE'))"

# Commit
git add .claude/checkpoints/
PHASE=$(python3 -c "import yaml; print(yaml.safe_load(open('$CHECKPOINT_FILE'))['phase']['current'])")
git commit -m "checkpoint: $PHASE"
```

## Step 5 — Report summary

Display brief summary to user:
- Phase (current / completed / next)
- Parameters captured (count, confidence breakdown)
- Experiments (count by status)
- Decisions made
- Blockers open
- Resume instructions (count of steps)

## Step 6 — Cleanup old checkpoints

Keep only last 5 checkpoints per branch:

```bash
BRANCH=$(git branch --show-current)
CHECKPOINTS=$(ls -t .claude/checkpoints/${BRANCH}-*.yaml 2>/dev/null)
COUNT=$(echo "$CHECKPOINTS" | wc -l)

if [ "$COUNT" -gt 5 ]; then
  mkdir -p .claude/checkpoints/archive
  echo "$CHECKPOINTS" | tail -n +6 | while read old; do
    mv "$old" .claude/checkpoints/archive/
  done
fi
```

## Related Skills

- `/resume` — Load and restore from checkpoint
- `/reflect` — Extract lessons learned (memory vs state: different purposes)
- `/wrap-up` — Merge branch (should checkpoint before merging)
