---
name: distill
description: "Extract recurring patterns from trace files and distill them into reusable knowledge graph entries: scan traces for errors, surprises, and parameter insights, tag pattern strength, and write structured concept files. Trigger on: 'distill traces', 'extract patterns', 'what have we learned', 'summarize what we know', 'knowledge distillation', 'turn traces into knowledge', 'lessons learned', 'write a concept file', 'promote pattern to docs', 'review our traces', 'harvest insights'. Also activates after completing a batch of reproductions when the user wants to consolidate experience, or when asking what general lessons emerged across multiple papers."
---

# /distill — Pattern Extraction from Traces

Extract patterns from trace.md files into reusable knowledge files. Turns scattered experience into distilled, actionable knowledge.

## Trigger

User mentions: "distill", "extract patterns", "what have we learned", "summarize traces", "knowledge distillation". Also use after completing a batch of reproductions to extract emergent knowledge.

## Workflow

### Step 1 — Gather traces

Search for trace.md files across the project:
```
find . -name 'trace.md' -not -path './.git/*' | sort
```

Read each trace file. Look for:
- Errors and their root causes
- Surprising results or unexpected behaviors
- Parameter choices that worked (or didn't)
- Workflow decisions and their outcomes

### Step 2 — Identify patterns

A pattern is a recurring observation that appears in 2+ traces. Tag each pattern:

```
[N=<count>, <weak|moderate|strong>]
```

- `weak`: 2 occurrences, may be coincidence
- `moderate`: 3-4 occurrences with consistent mechanism
- `strong`: 5+ occurrences, well-understood mechanism

**Quantitative patterns with specific thresholds beat qualitative summaries.** Example:
- Good: "Grid resolution of 15+ points per characteristic length scale is needed for 1% accuracy [N=7, strong]"
- Bad: "Use fine grids [N=7, strong]"

### Step 3 — Write concept file

Create `knowledge/<concept>.md` with:

```markdown
# <Concept Name>

## Established Patterns [N >= 3]
- Pattern 1 [N=5, strong] — Source: trace1, trace2, ...
- Pattern 2 [N=3, moderate] — Source: ...

## Emerging Patterns [N = 2]
- Pattern 3 [N=2, weak] — Source: ...

## Open Questions
- Question that traces surface but don't answer

## Action Items
- [ ] Concrete steps to validate or apply these patterns
```

### Step 4 — Check for promotion

If any pattern reaches `[N >= 5, strong]`, it is a candidate for promotion to CLAUDE.md or other project-level documentation. Flag these to the user — only the project owner decides what goes into the foreground.

## Anti-Patterns

- Don't distill from a single trace. Wait for patterns to emerge from 2+ traces.
- Don't write platitudes ("be careful with parameters"). Be specific and quantitative.
- Don't duplicate existing knowledge. Check concept files before writing.
- Don't promote prematurely. A pattern at N=2 is weak — it may be coincidence.
