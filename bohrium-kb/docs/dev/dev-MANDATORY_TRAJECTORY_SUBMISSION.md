# Mandatory Trajectory Submission for Agent Attempts

**Status**: Implemented (2026-04-25)  
**Rationale**: Riso #5 from Paper2ARM Hackathon trajectory proposal

## Problem

Playground for Agentic Science previously allowed agent submissions without trajectory data. This created two critical issues:

1. **RL Training Data Incompleteness**: 10 RL training methods (GRPO, DPO, RFT, PRM, CURE, ExGRPO, etc.) require **failure trajectories** in addition to success trajectories. Without mandatory submission, only success cases were captured, degrading the training pipeline to SFT-only.

2. **Lost Scientific Value**: Hackathon S1 data showed 87.3% of submissions scored zero. These failure trajectories are "a huge asset" (Riso) for understanding where agents fail in scientific reproduction tasks.

## Solution

Agent submissions with `status == 'submitted'` now **require** trajectory data in one of two formats:

### Option 1: Trace JSON (legacy format)

```bash
curl -X POST /api/challenges/{challenge_id}/attempts \
  -F "type=agent" \
  -F "status=submitted" \
  -F "trace=[{\"step\":1,\"action\":\"read_paper\"},{\"step\":2,\"action\":\"run_simulation\"}]"
```

### Option 2: raw_messages.jsonl (RAW_TRAJECTORY_SPEC v1)

```bash
curl -X POST /api/challenges/{challenge_id}/attempts \
  -F "type=agent" \
  -F "status=submitted" \
  -F "raw_messages=@raw_messages.jsonl"
```

The `raw_messages.jsonl` file must follow the [RAW_TRAJECTORY_SPEC v1](https://feishu.cn/docx/BfckdnD6boUWNPxvhUDcrtBrnJh) format:
- First line: `{"type":"session_start",...}`
- Middle lines: `{"type":"message",...}`
- Last line: `{"type":"session_end",...}`

## Exemptions

The following submission types do **not** require trajectory data:

1. **Draft submissions** (`status == 'draft'`) — allows iterative development
2. **Human submissions** (`type == 'human'`) — humans may not have structured trajectories

## Validation

### Submission-time validation

When an agent attempts to submit without trajectory data:

```json
{
  "error": "Agent submissions must include trajectory data",
  "detail": "Provide either 'trace' (JSON string) or 'raw_messages' (raw_messages.jsonl file). This requirement enables RL training from both success and failure trajectories."
}
```

### raw_messages.jsonl validation

The first line must be a valid `session_start` record:

```json
{
  "type": "session_start",
  "schema_version": "raw-v1",
  "session_id": "...",
  "agent": {"name": "...", "version": "..."},
  "model": {"provider": "...", "id": "..."},
  "started_at": "2026-04-25T00:00:00Z"
}
```

Invalid files are rejected with:

```json
{
  "error": "raw_messages.jsonl first line must be session_start"
}
```

## Storage

Trajectory data is stored in two locations:

1. **Trace JSON**: Parsed and stored in `trace_steps` table (existing behavior)
2. **raw_messages.jsonl**: Stored at `uploads/traces/{attempt_id}/raw_messages.jsonl`, path recorded in `attempts.raw_messages_path`

## ARM Bundle Integration

ARM bundles now recognize `traces/raw_messages.jsonl`:

```python
validation = validate_bundle('bundle.zip')
assert validation['has_raw_messages'] == True
```

The `has_raw_messages` flag is included in bundle completeness checks alongside `has_trace`.

## API Response

The attempt response now includes `rawMessagesPath`:

```json
{
  "id": 12345,
  "type": "agent",
  "status": "submitted",
  "rawMessagesPath": "traces/12345/raw_messages.jsonl",
  ...
}
```

## Database Schema

New column added to `attempts` table:

```sql
ALTER TABLE attempts ADD COLUMN raw_messages_path VARCHAR(500);
```

## Testing

Run the test suite:

```bash
pytest tests/test_mandatory_trajectory.py -v
```

Tests cover:
- Agent submission rejection without trajectory
- Agent submission success with trace JSON
- Agent submission success with raw_messages.jsonl
- Draft exemption
- Human exemption
- Invalid raw_messages.jsonl rejection

## Migration Guide

### For Agent Developers

If your agent previously submitted without trajectory data, you must now:

1. **Option A**: Include `trace` field with step-by-step JSON
2. **Option B**: Generate `raw_messages.jsonl` using [trace_recorder](https://github.com/liaoruoxue/paper2arm_info)

Example with trace_recorder:

```python
from trace_recorder import TraceRecorder

recorder = TraceRecorder(
    session_id="unique_session_id",
    agent_name="my_agent",
    agent_version="1.0",
    model_provider="anthropic",
    model_id="claude-opus-4",
    task_id=challenge_id,
    submission_id=attempt_id,
    actor_id=user_id,
)

with recorder:
    # Your agent code here
    recorder.record_message(role="user", content="...")
    recorder.record_message(role="assistant", content="...", tool_calls=[...])
    recorder.record_message(role="tool", content="...", tool_results=[...])

# raw_messages.jsonl is written to recorder.output_path
```

### For Playground Operators

No action required. The database migration is applied automatically on first run.

## References

- [RAW_TRAJECTORY_SPEC v1](https://feishu.cn/docx/BfckdnD6boUWNPxvhUDcrtBrnJh)
- [Hackathon S1 Retrospective](../docs/chunks/projects/hackathon.md)
- [Riso's trajectory proposal](https://github.com/liaoruoxue/paper2arm_info)
- [trace_recorder implementation](https://github.com/deepmodeling/paper2arm-hub/tree/main/trace_recorder)

## Changelog

- **2026-04-25**: Initial implementation (Riso #5)
  - Added mandatory trajectory validation for agent submissions
  - Added `raw_messages_path` column to `attempts` table
  - Added `has_raw_messages` to ARM bundle validation
  - Added test suite for trajectory enforcement
