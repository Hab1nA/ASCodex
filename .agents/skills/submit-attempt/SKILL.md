---
name: submit-attempt
description: "Submit reproduction results to the Playground for Agentic Science platform. Builds an ARM (Agent Ready Manuscript) bundle from local artifacts — figures, script, Dockerfile, trace, tolerances — uploads it, and triggers scoring. Trigger on: 'submit attempt', 'submit my results', 'upload reproduction', 'submit to playground', 'post my figures'. Also activates at the end of /reproduce-paper Phase 4 when figures exist in output/ or figures/."
metadata:
  version: 1.2.0
  author: friday-team
  tags: [bohrium-playground, submission, arm-bundle, scoring]
---

## Codex 安全适配

本技能在 Codex 中只负责发现产物、生成 manifest 和本地 dry-run。不得直接执行文中的 curl、POST、score 或 legacy worker 链；当前 Codex 没有 `solver-guard_build-submit`。真实提交必须由用户明确授权，并先运行 `bohrium-kb/tools/submit_gate_audit.py`。提交后只读核对必须绑定当前 bundle revision/hash，并区分 replay、`resultsJson`、scorecard、raw/effective score、判罚、credited owner、fresh rescore 与榜单 scope。凭据只能来自当前进程的 `PLAYGROUND_TOKEN`，禁止从文件回退或打印。

# Submit Attempt (ARM Bundle)

Package local reproduction artifacts into an ARM bundle and submit to the Playground platform.

The default flow: **discover artifacts → build manifest → package ARM zip → create attempt → upload bundle → trigger scoring**.

## Prerequisites

1. **Auth token**: Read `PLAYGROUND_TOKEN` from the current process only. Never fall back to `~/.playground/token`, DSH credential files, or a hardcoded value.

2. **API base URL**: Require an explicitly configured `PLAYGROUND_API` for any future authorized write. Do not infer or trust historical endpoints.

3. **Challenge ID**: Must know which challenge this reproduction targets. If unknown, stop and request a separately authorized, read-only lookup.

4. **Authorized identity**: Record the intended user/agent identity and quota reservation before any future write. After scoring, the server-reported credited owner must match; do not infer ownership from the token label alone.

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
3. Neither → stop. Do not synthesize a trace from conversation context, summaries, reports, or model memory. Re-run the relevant computation through the `real-trace-capture` workflow and capture genuine tool calls, results, timestamps, stdout, and artifact provenance; if that evidence cannot be recovered, the attempt is not submission-ready.

Trace step types: `thought`, `tool_call`, `tool_result`, `artifact`, `decision`, `error`, `observation`

## Step 4 — Prepare ARM Bundle (dry-run only in Codex)

Every bundle build must produce and locally record an immutable content hash plus a monotonically distinct revision identifier. A rebuilt or re-uploaded bundle is a new scoring subject even when it belongs to the same attempt.

Do not execute the historical command examples below in Codex. Use only `submit_gate_audit.py`; the examples are retained for provenance and contain direct network-write operations.

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

**Historical Harness flow (not executable from Codex):**
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

### Legacy mode (historical reference only)

```bash
python3 submit.py --challenge-id ... --figures ... --method ... --outcome ... --legacy --score
```

Uploads figures as loose multipart form data, like the pre-ARM pipeline.

## Step 4.5 — 提交前强制内检（dry-run 看包，缺包内文件静默致命）

1. **dry-run 构建**：`submit.py --dry-run`（或手动 zip）构建 bundle，**看包内成员清单**。
2. **契约文件名逐字核对**：题面/契约点名的每个输出文件（如 `wavefunction.ts`、`characterization.json`、`results.csv`）必须**逐字**在包内——文件名客户端不校验，错名上传成功但 0 分（07 all-zero 疑似 artifact 命名/加载问题；09 满分链路严格执行）。
3. **包内完整性**：`arm_manifest.json` + 契约点名文件 + trace/raw_messages 齐全；manifest 中 expected_outputs 与包内文件一一对应。
4. **真实运行 admission**：trace 至少包含一段可核验运行闭环（call/result/stdout/artifact provenance）。完全没有运行痕迹的 trace 当前不会进入待评队列，必须在 dry-run 阶段 fail closed。
5. **revision 账本**：记录 attempt（若已存在）、bundle revision、bundle sha256、构建时间、文件清单 hash；诊断报告不装入 bundle。

## Step 4.6 — 两轨判别：ARM bundle vs worker /uploads

- 本技能 = **ARM 轨**（bundle 走平台 `POST /api/attempts/{id}/bundle`）。
- `worker-submit-chain` 不在当前 Codex 能力集中；ARM/worker 轨切换只能作为历史假设，必须以当前题面与 live 只读证据重新确认。
- scorecard 分量是路径信号晴雨表：executability/packaging/trace_quality 的差异反映走的是哪条轨、哪段没接上。

## Step 5 — Score and Report

Codex 不触发评分 POST。只有在用户明确授权并完成六门审计后，才可由单独的执行层处理。ARM bundle 重传后，旧评分立即标记 `stale_for_current_bundle`；不得沿用上一个包的结论，必须等待当前 revision/hash 的 fresh rescore。

只读核验至少包括：

- replay、`resultsJson`、scorecard 与 trace admission；
- raw score、effective score、判罚标记及可查询依据（平台当前报告有效分扣 1 且原始分仍可见）；
- credited user/agent；
- bundle revision/hash 与 rescore 状态必须对应；
- 榜单 scope/season 及该 scope 的有效分。全站榜和赛季总榜口径虽已统一，仍不得把不同 scope 的观测混成一个布尔 `leaderboard_present`；
- 加权反作弊状态。新增三个信号的名称、权重和阈值没有实时证据时保持 unknown。

匿名读取他人提交的路径已关闭。核验只限自有或明确授权的 attempt；不要绕过对象权限，也不要把判罚依据、反作弊信号或榜单情报写入提交 trace/artifact。

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
| Raw score      | 90.0 / 100           |
| Effective score | 89.0 / 100         |
| Penalty        | applied (-1), basis recorded locally |
| Credited owner | user / agent         |
| Bundle revision | rev + sha256        |
| Rescore status | completed fresh      |
| Leaderboard scope | season / all-time / challenge |
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
  "bundle_revision": "<server-or-local-revision>",
  "bundle_sha256": "<sha256>",
  "rescore_status": "completed",
  "completeness": 0.83,
  "raw_score": 90.0,
  "effective_score": 89.0,
  "penalty": {"applied": true, "delta": -1, "basis_available": true},
  "credited_owner": {"user": "<user>", "agent": "<agent>"},
  "leaderboard": {"scope": "season", "season_id": "<id>", "present": true},
  "trace_admission": "admitted",
  "weighted_anticheat": {"status": "<server-value>", "signals": "<server-evidence-or-unknown>"},
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
| Bundle re-uploaded | Mark prior score stale; wait for fresh rescore tied to the new revision/hash. |
| No execution trace | Do not upload; capture a genuine run and rebuild the bundle. |
| Owner mismatch | Do not credit or close the attempt; reconcile the authorized identity and server attribution. |
| Penalty details unavailable | Preserve raw/effective/flag as observed and mark basis unknown; do not guess. |
