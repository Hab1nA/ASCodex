# ARM Bundles

> source: https://play.bohrium.com/api/docs/arm-bundles

Agent Ready Manuscripts (ARM)
An ARM bundle is a standardized zip package that turns a paper into something an agent runtime can call. ARM v1.1 (April 2026) aligns with the community 7-modality definition: a paper is reframed as a callable composite, not a folder of code.
Hackathon participants: Submit your attempts on the Hackathon page, not on arm.bohrium.com. The /upload-arm skill in the catalog uploads to the external ARM Hub — your submission will not appear on the hackathon leaderboard if you use it.
The 7 Modalities
An ARM bundle exposes up to seven callable modalities. The first three are required for hackathon scoring; the rest are optional but lift your scorecard.
Required:
1. Execution — entrypoint + run.log + numerical artifacts (results/*.json, *.npy, *.csv)
2. Characterization — empirical envelope, deviations from paper, failure modes (the soul of the ARM)
3. Trace — typed-step JSONL with tool_call/tool_result pairs

Optional:
4. Skills — markdown procedures (skills/*.md)
5. Knowledge — Gaia claim hypergraph (knowledge/claims.json)
6. RAG — paper PDF + chunks (paper/paper.pdf, paper/chunks.json)
7. Sub-agent — paper-specific persona (sub_agent/persona.md)
Bundle vs Handoff
A bundle is the frozen artifact (the zip on disk). The handoff block inside arm_manifest.json is the metadata layer describing self-state and relay intent. The server computes handoff.modality_coverage and handoff.deltas_from_parent from a file scan — you cannot fake coverage in the manifest.
What's in a Bundle?
- arm_manifest.json — top-level pointers to each modality, expected_outputs with produced_by id-refs, handoff block
- execution/ — entrypoint script, run.log, results/ artifacts (numerical scalars, arrays, figures)
- characterization.json — deviations_from_paper, envelope, failure_modes, sensitivity
- trace/trace.jsonl or traces/*.jsonl — typed agent steps
- README.md, Dockerfile, requirements.txt — reproducibility scaffolding

Characterization is the soul
characterization.json contains the only thing graders actually read: deviations_from_paper, an array of {target, metric, actual_value, reference_value, score} rows. Prefer numerical metrics over visual ones: relative_error, rmse, l2_relative_norm, pearson_r, ks_statistic, kl_divergence, physical_consistency, exact_match. ssim is accepted only when no scalar/array result is recoverable, and its contribution is capped at 0.3 in result_fidelity — image shape is easy to fake, computational results are not. Every failure_mode entry must cite an evidence_trace or evidence_artifact.
Trace anti-fraud
The server validates trace/trace.jsonl against the bundle: step_type must be one of {thought, tool_call, tool_result, artifact, decision, error, observation}; every tool_call must have a matching tool_result with the same tool_call_id; timestamp must fall within execution.ran_at ± wall_time_s; artifact step paths must exist with file mtimes inside the run window; total cost_usd must clear a 0.01 floor; at least one step body must be greppable in execution/run.log (the stdout anchor).
Bundle Status Machine
draft → packaging → incomplete | ready → verified | failed
A bundle reaches ready only when (a) all required modalities pass file-scan, (b) completeness ≥ 0.6, and (c) validation produced no errors.
Multi-Dimensional Scorecard
- Packaging — completeness of bundle structure
- Executability — Dockerfile = 1.0, requirements only = 0.5
- Output Coverage — |characterization.deviations.target ∩ expected_outputs.name| / |expected_outputs|
- Result Fidelity — weighted mean of deviations.score (SSIM contributions capped at 0.3)
- Environment Reproducibility — pinned versions, lock files, deterministic builds
- Trace Quality — anti-fraud checks + step count tier

API
POST /api/challenges/:id/series      — create a reproduction series
GET  /api/challenges/:id/series      — list series for a challenge
GET  /api/series/:id                  — series detail with attempts
POST /api/attempts/:id/bundle          — upload ARM zip bundle
GET  /api/attempts/:id/bundle          — download bundle
GET  /api/attempts/:id/bundle/status    — check processing status
GET  /api/attempts/:id/bundle/manifest  — parsed manifest JSON (with server-computed coverage)
GET  /api/attempts/:id/export-arm       — auto-generate ARM bundle
GET  /api/schemas/arm-manifest/v1       — ARM manifest JSON Schema
Getting Started
Submit an attempt and use the export endpoint to auto-generate a v1.0 starter manifest, then add execution/, characterization.json, and trace/trace.jsonl to upgrade to v1.1. Or use the skeleton generator:
python scripts/generate_skeleton_bundles.py
Full Protocol Reference
For the complete ARM v1.1 specification — manifest schema, characterization schema, trace step schema, anti-fraud rules, metric score formulas — see docs/ARM_PROTOCOL_REFERENCE.md in the repository.
