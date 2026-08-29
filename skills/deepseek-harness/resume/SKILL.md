---
name: resume
description: "Resume task from checkpoint file. Use when: starting new session after interruption, recovering from crash, or forking from another branch's checkpoint. Loads state, verifies artifacts, displays resume instructions."
version: 3.0.0
author: system
tags: [session-management, state-persistence, long-running-tasks]
---

# Resume Skill

Load and restore task state from a structured checkpoint file.

## When to Use

- **Session start**: After context loss or session restart
- **Crash recovery**: Picking up after unexpected interruption
- **Task handoff**: Another person/agent continuing work
- **Branch fork**: Starting from another branch's checkpoint
- **Manual resume**: User explicitly requests `/resume [checkpoint-file]`

## Step 1 — Find checkpoint

**If no argument provided**: Find latest checkpoint on current branch:

```bash
BRANCH=$(git branch --show-current)
LATEST=$(ls -t .claude/checkpoints/${BRANCH}-*.yaml 2>/dev/null | head -1)

if [ -z "$LATEST" ]; then
  echo "No checkpoint found for branch: $BRANCH"
  echo "Available checkpoints:"
  ls -t .claude/checkpoints/*.yaml 2>/dev/null | head -10
  exit 1
fi

echo "Found checkpoint: $LATEST"
CHECKPOINT_FILE="$LATEST"
```

**If argument provided**: Use that checkpoint file or branch name:

```bash
ARG="$1"
if [ -f "$ARG" ]; then
  CHECKPOINT_FILE="$ARG"
elif ls -t .claude/checkpoints/${ARG}-*.yaml 2>/dev/null | head -1 > /dev/null; then
  CHECKPOINT_FILE=$(ls -t .claude/checkpoints/${ARG}-*.yaml | head -1)
else
  echo "Checkpoint not found: $ARG"
  exit 1
fi
```

## Step 2 — Load and display checkpoint

```python
import yaml
import os

checkpoint_file = os.environ.get('CHECKPOINT_FILE', '.claude/checkpoints/latest.yaml')

with open(checkpoint_file, 'r') as f:
    checkpoint = yaml.safe_load(f)

print("=" * 60)
print(f"CHECKPOINT RESUME: {checkpoint_file}")
print(f"Created: {checkpoint['created_at']}")
print(f"Branch: {checkpoint['branch']}")
print("=" * 60)
print()

# Phase
phase = checkpoint.get('phase', {})
print("PHASE")
print(f"  Current: {phase.get('current', '?')}")
print(f"  Completed: {', '.join(phase.get('completed', []))}")
print(f"  Next: {', '.join(phase.get('next', []))}")
print()

# Parameters
params = checkpoint.get('state', {}).get('parameters', [])
if params:
    print(f"PARAMETERS ({len(params)} extracted)")
    for p in params:
        unit = p.get('unit', '')
        print(f"  {p['name']} = {p['value']} {unit} [{p.get('confidence','?')}] -- {p.get('source','?')}")
    print()

# Experiments
experiments = checkpoint.get('state', {}).get('experiments', [])
if experiments:
    print(f"EXPERIMENTS ({len(experiments)} total)")
    for exp in experiments:
        progress = f" {int(exp['progress']*100)}%" if 'progress' in exp else ""
        reason = f" -- {exp['reason']}" if 'reason' in exp else ""
        print(f"  {exp['name']} [{exp['status']}]{progress}{reason}")
    print()

# Decisions
decisions = checkpoint.get('state', {}).get('decisions', [])
if decisions:
    print(f"DECISIONS ({len(decisions)} made)")
    for dec in decisions:
        print(f"  {dec.get('question','?')} -> {dec.get('decision','?')} [{dec.get('confidence','?')}]")
    print()

# Blockers
blockers = checkpoint.get('blockers', [])
if blockers:
    print(f"BLOCKERS ({len(blockers)} open)")
    for b in blockers:
        assumption = f" (assuming: {b['assumption']})" if 'assumption' in b else ""
        print(f"  [{b.get('impact','?')}] {b['question']}{assumption}")
    print()

# Artifacts — verify they exist
artifacts = checkpoint.get('artifacts', [])
if artifacts:
    print(f"ARTIFACTS ({len(artifacts)} tracked)")
    missing = 0
    for a in artifacts:
        path = a['path'].replace('~', os.path.expanduser('~'))
        exists = os.path.exists(path)
        status = "OK" if exists else "MISSING"
        if not exists:
            missing += 1
        print(f"  [{status}] {a['path']} ({a.get('role','?')})")
    if missing > 0:
        print(f"\n  WARNING: {missing} artifact(s) missing!")
    print()

# Resume instructions
print("=" * 60)
print("RESUME INSTRUCTIONS")
print("=" * 60)
print()
print(checkpoint.get('resume_instructions', '(no instructions)'))
print()
print("=" * 60)
```

## Step 3 — Update checkpoint chain (optional)

Track which session resumed from which checkpoint:

```python
from datetime import datetime
import yaml, os

checkpoint_file = os.environ.get('CHECKPOINT_FILE')
with open(checkpoint_file, 'r') as f:
    checkpoint = yaml.safe_load(f)

checkpoint['resumed_at'] = datetime.utcnow().isoformat() + 'Z'
checkpoint['resume_count'] = checkpoint.get('resume_count', 0) + 1

with open(checkpoint_file, 'w') as f:
    yaml.dump(checkpoint, f, default_flow_style=False, allow_unicode=True, sort_keys=False)

print(f"Resume #{checkpoint['resume_count']} recorded")
```

Then commit:

```bash
git add "$CHECKPOINT_FILE"
git commit -m "checkpoint: resumed from $(basename $CHECKPOINT_FILE)" 2>/dev/null || true
```

## Arguments

- **No argument**: Find and resume from latest checkpoint on current branch
- **[checkpoint-file]**: Resume from specific checkpoint file
- **[branch-name]**: Find and resume from latest checkpoint on specified branch

## Related Skills

- `/checkpoint` — Save current progress to checkpoint
- `/reflect` — Extract lessons learned (different scope: memory vs state)
- `/sync-and-plan` — Pull updates and plan next work
