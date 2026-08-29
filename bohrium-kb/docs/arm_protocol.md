# ARM Protocol Reference — Agent Ready Manuscripts for Reproducible Science

> **Status**: Community Draft v1.0 (2026-04-14)
> **Authors**: Playground for Agentic Science contributors
> **Inspired by**: [ARM Hub](https://arm.bohrium.com) by Bohrium / 深势科技
> **Schema**: [`data/schemas/arm-manifest-v1.schema.json`](../data/schemas/arm-manifest-v1.schema.json)

---

## Table of Contents

1. [Motivation](#motivation)
2. [What Is an ARM Bundle?](#what-is-an-arm-bundle)
3. [Data Model: Three-Level Hierarchy](#data-model-three-level-hierarchy)
4. [Bundle Structure](#bundle-structure)
5. [The `arm_manifest.json` Contract](#the-arm_manifestjson-contract)
6. [Bundle Status Machine](#bundle-status-machine)
7. [Multi-Dimensional Scorecard](#multi-dimensional-scorecard)
8. [Maturity Levels](#maturity-levels)
9. [Handoff Protocol](#handoff-protocol)
10. [API Reference](#api-reference)
11. [Skeleton Bundles](#skeleton-bundles)
12. [Agent Workflow](#agent-workflow)
13. [Cross-Platform Interoperability](#cross-platform-interoperability)
14. [Design Decisions and Rationale](#design-decisions-and-rationale)
15. [Brainstorm Synthesis (5-Model Consensus)](#brainstorm-synthesis-5-model-consensus)
16. [Research Context: ARM Hub Analysis](#research-context-arm-hub-analysis)
17. [Open Questions and Future Work](#open-questions-and-future-work)
18. [References](#references)

---

## Motivation

Scientific reproducibility has a packaging problem. When a researcher (human or AI agent) successfully reproduces a paper's computational results, the artifacts — scripts, environments, data, figures — are scattered across loose files with no standard structure. The next agent who wants to build on that work must reverse-engineer the reproduction from scratch.

ARM (Agent Ready Manuscripts) solves this by treating reproductions as **versioned, self-contained software artifacts** with machine-readable contracts. An ARM bundle answers three questions any agent needs:

1. **What am I reproducing?** — Paper targets, figure labels, quantitative claims
2. **How do I run it?** — Environment, entrypoint, dependencies, expected outputs
3. **What happened last time?** — Status, stuck points, handoff suggestions, scorecard

This protocol document captures the complete ARM design — from research origins through implementation — so the community can treat it as a shared contract.

---

## What Is an ARM Bundle?

An ARM bundle is a **zip archive** containing everything needed to reproduce a paper's computational results. It is:

- **Self-contained**: All code, data references, and environment specs in one package
- **Machine-readable**: The `arm_manifest.json` is a typed contract, not free-text
- **Human-readable**: The `README.md` explains the reproduction for human reviewers
- **Versioned**: Bundles live inside a ReproductionSeries with semver version strings
- **Portable**: Docker-based environments ensure "runs on my machine" for everyone
- **Gradable**: A 6-dimension scorecard quantifies reproduction quality

---

## Data Model: Three-Level Hierarchy

### Before ARM

```
Challenge (paper) → Attempt (flat list, fork DAG via parent_attempt_id)
```

Each attempt was a loose collection: a method description, some figure uploads, maybe a script. No structure, no versioning within a single author's effort.

### After ARM

```
Challenge (paper to reproduce)
  └── ReproductionSeries (one author's coherent effort on one challenge)
        ├── owner, title, description
        ├── latest_version, latest_bundle_status
        └── Attempt v1.0 (first iteration)
            Attempt v1.1 (refinement)
            Attempt v2.0 (major revision)
```

**Key distinctions:**

| Concept | Mechanism | When to use |
|---------|-----------|-------------|
| **Version** (within series) | `attempt.version` = "v1.0", "v1.1" | Iterating on your own work |
| **Fork** (across series) | `attempt.parent_attempt_id` | Building on someone else's work |

This mirrors software development: versions are like commits on your branch; forks are like git forks across repositories.

### Database Schema

**`reproduction_series` table:**

| Column | Type | Description |
|--------|------|-------------|
| `id` | INTEGER PK | Auto-increment |
| `challenge_id` | VARCHAR(64) FK | Which paper |
| `owner_id` | VARCHAR(64) FK | Who is reproducing |
| `title` | VARCHAR(500) | Series title |
| `description` | TEXT | Series description |
| `latest_version` | VARCHAR(20) | Current version (e.g., "v1.0") |
| `latest_bundle_status` | VARCHAR(20) | Current bundle state |
| `created_at` | DATETIME | Creation timestamp |
| `updated_at` | DATETIME | Last update timestamp |

**New columns on `attempts` table:**

| Column | Type | Description |
|--------|------|-------------|
| `series_id` | INTEGER FK (nullable) | Link to ReproductionSeries |
| `version` | VARCHAR(20) | Semver within series |
| `bundle_status` | VARCHAR(20) | Current bundle state |
| `bundle_path` | VARCHAR(500) | Path to zip on disk |
| `manifest_json` | TEXT | Cached manifest JSON |
| `scorecard_json` | TEXT | Multi-dimensional scorecard JSON |

All new columns are **nullable** for backward compatibility. Existing attempts without bundles continue to work unchanged.

---

## Bundle Structure

Every ARM bundle zip must follow this directory layout:

```
ARM/
├── arm_manifest.json    ← REQUIRED: machine-readable contract
├── README.md            ← REQUIRED: human-readable description
├── Dockerfile           ← RECOMMENDED: reproducible environment
├── requirements.txt     ← RECOMMENDED: Python dependencies
├── src/                 ← RECOMMENDED: reproduction code
│   └── reproduce.py     ← Entrypoint script
├── data/                ← OPTIONAL: input data or download scripts
├── results/             ← OPTIONAL: output figures, tables, logs
│   └── reference/       ← Paper's original figures for comparison
├── traces/              ← OPTIONAL: agent execution traces
└── notebooks/           ← OPTIONAL: Jupyter notebooks
```

### Required Files

- **`arm_manifest.json`**: The machine-readable contract (see schema below). Must be valid JSON conforming to the ARM manifest v1 schema.
- **`README.md`**: Human-readable description including paper reference, reproduction method, how to run, and results summary.

### Recommended Files

- **`Dockerfile`**: Defines the reproducible execution environment. Discipline-specific base images are encouraged (see [Skeleton Bundles](#skeleton-bundles) for templates).
- **`requirements.txt`** (or `environment.yml`, `poetry.lock`): Pinned dependency versions.
- **`src/reproduce.py`**: The entrypoint script. Should be runnable via `python src/reproduce.py` and produce all expected outputs.

### Completeness Score

Bundle completeness is computed as:

```
completeness = (has_manifest + has_readme + has_dockerfile +
                has_requirements + has_src + has_results) / 6
```

A completeness score >= 0.6 transitions the bundle to `ready` status. Below 0.6 it remains `incomplete`.

---

## The `arm_manifest.json` Contract

The manifest is the core of an ARM bundle. It is a JSON file conforming to the schema at `data/schemas/arm-manifest-v1.schema.json`.

### Required Fields

```json
{
  "arm_version": "1.0",
  "paper": {
    "title": "Paper Title Here"
  },
  "entrypoint": "src/reproduce.py"
}
```

### Full Schema (all fields)

#### `arm_version` (string, required)

Always `"1.0"` for this version of the protocol.

#### `paper` (object, required)

Identifies the paper being reproduced.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `title` | string | yes | Paper title |
| `doi` | string | no | DOI identifier |
| `authors` | string | no | Author list |
| `year` | integer | no | Publication year |
| `journal` | string | no | Journal name |
| `challenge_id` | string | no | Playground challenge slug |
| `target_figures` | string[] | no | Figure labels to reproduce (e.g., `["Fig2a", "Fig3"]`) |
| `target_tables` | string[] | no | Table labels to reproduce |
| `target_claims` | string[] | no | Quantitative claims to verify (e.g., `"Accuracy > 92% on CIFAR-10"`) |

#### `entrypoint` (string, required)

Path to the main reproduction script, relative to bundle root. Convention: `src/reproduce.py`.

#### `entrypoint_args` (string[], optional)

Default CLI arguments for the entrypoint.

#### `environment` (object, optional)

Execution environment specification.

| Field | Type | Description |
|-------|------|-------------|
| `docker_image` | string | Base Docker image (e.g., `python:3.11-slim`) |
| `gpu_required` | boolean | Whether GPU is needed (default: false) |
| `estimated_runtime_minutes` | number | Expected runtime |
| `dependencies` | string | Path to requirements file (e.g., `requirements.txt`) |
| `lock_file` | string | Path to dependency lock file |
| `python_version` | string | Required Python version |
| `extra_packages` | string[] | System packages (apt) beyond base image |

#### `expected_outputs` (array, optional)

List of outputs the reproduction should produce.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | yes | Human label (e.g., "Figure 2a") |
| `path` | string | yes | Output file path relative to bundle root |
| `type` | enum | yes | `figure`, `table`, `metric`, `data`, `log`, `checkpoint` |
| `comparison_method` | enum | no | `visual_similarity`, `numeric_tolerance`, `json_diff`, `exact_match`, `custom` |
| `reference` | string | no | Path to reference output for comparison |
| `tolerance` | number | no | Acceptable deviation |
| `expected_value` | any | no | Expected value for metric-type outputs |

**Comparison methods explained:**

- **`visual_similarity`**: SSIM or perceptual hash comparison of figure images. `tolerance` is the minimum SSIM score (0-1, default 0.85).
- **`numeric_tolerance`**: Scalar comparison. `tolerance` is the maximum absolute or relative deviation.
- **`json_diff`**: Structural comparison of JSON outputs.
- **`exact_match`**: Byte-for-byte equality.
- **`custom`**: Defer to a user-provided comparison script.

#### `data_sources` (array, optional)

Input data for the reproduction.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | yes | Data source name |
| `path` | string | no | Local path within bundle (if embedded) |
| `uri` | string | no | External URI for download |
| `checksum` | string | no | SHA-256 hash for integrity |
| `size_bytes` | integer | no | File size |
| `license` | string | no | Data license |
| `dataset_id` | string | no | Playground dataset ID (if referencing platform data) |

#### `skills_used` (array, optional)

Agent skills used in the reproduction.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `skill_id` | string | yes | Platform skill slug |
| `version` | string | no | Skill version |
| `parameters` | object | no | Key parameters passed to the skill |

#### `agents_used` (array, optional)

AI agents that contributed.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `agent_id` | string | yes | Platform agent slug |
| `role` | string | no | Agent's role (e.g., "reviewer", "executor") |

#### `handoff` (object, optional)

Agent-to-agent handoff protocol for incomplete reproductions. See [Handoff Protocol](#handoff-protocol).

#### `scorecard` (object, optional)

Multi-dimensional quality scorecard. See [Multi-Dimensional Scorecard](#multi-dimensional-scorecard).

#### `provenance` (object, optional)

Origin and lineage tracking.

| Field | Type | Description |
|-------|------|-------------|
| `created_by` | enum | `human`, `agent`, `mixed` |
| `platform` | string | Source platform (e.g., `playground`, `arm-hub`) |
| `imported_from` | string | Original platform URL if imported |
| `attempt_id` | integer | Playground attempt ID |
| `series_id` | integer | Playground series ID |
| `parent_attempt_id` | integer | Fork parent attempt ID |
| `toolchain` | string | Tools/frameworks used (e.g., `cantera+python3.11`) |

---

## Bundle Status Machine

Every attempt's bundle progresses through a state machine:

```
    ┌─────────────────────────────────────┐
    │                                     │
    ▼                                     │
  draft ──► packaging ──► ready ──► verified
    │          │             │
    │          ▼             │
    │       incomplete       │
    │          │             │
    │          ▼             │
    └──────► failed ◄────────┘
```

### States

| State | Meaning | Transition |
|-------|---------|------------|
| `draft` | Series created, no bundle uploaded yet | → `packaging` (on upload) |
| `packaging` | Upload in progress, being validated | → `ready`, `incomplete`, or `failed` |
| `incomplete` | Bundle uploaded but completeness < 0.6 | → `packaging` (on re-upload) |
| `ready` | Bundle validated, completeness >= 0.6 | → `verified` (on independent verification) |
| `verified` | Outputs confirmed to match paper | Terminal success state |
| `failed` | Bundle invalid (corrupt zip, bad JSON) | → `packaging` (on re-upload) |

### The `incomplete` State — A Philosophical Innovation

ARM Hub uses a binary ready/failed model. We add `incomplete` because **structured incompleteness is more valuable than unstructured absence**. An incomplete bundle with a manifest and handoff notes helps the next agent more than no bundle at all. This aligns with our fork system's philosophy: "Stuck is a save point."

---

## Multi-Dimensional Scorecard

Each bundle is scored on 6 independent dimensions, each normalized to 0.0–1.0:

| Dimension | What It Measures | How It's Computed |
|-----------|-----------------|-------------------|
| **packaging** | Bundle structure completeness | `(manifest + readme + dockerfile + requirements + src + results) / 6` |
| **executability** | Can the bundle run? | 1.0 = Dockerfile builds; 0.5 = Dockerfile present; 0.0 = no Dockerfile |
| **output_coverage** | Fraction of targets reproduced | `Q1 / 10` (from RQS Figure Coverage) |
| **result_fidelity** | How closely outputs match paper | `(3*Q3 + 2*Q4) / 50` (from RQS Quantitative + Qualitative) |
| **environment_reproducibility** | Pinned versions, deterministic builds | `Q2 / 10` (from RQS Model Fidelity) |
| **trace_quality** | Execution trace completeness | 1.0 = typed trace with 5+ steps; 0.5 = minimal trace; 0.0 = none |

### Automatic vs. Manual Scoring

- **`packaging`**, **`executability`**, and **`trace_quality`** are computed **automatically** on bundle upload by the bundle validation pipeline.
- **`output_coverage`**, **`result_fidelity`**, and **`environment_reproducibility`** require **grading** (via the `/grade-reproduction` skill or human review) and are populated separately.

### Legacy RQS Integration

The scorecard also carries the legacy Reproduction Quality Score for backward compatibility:

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

---

## Maturity Levels

The scorecard maps to display-friendly maturity levels:

| Level | Name | Badge | Criteria |
|-------|------|-------|----------|
| 0 | **Seed** | `🌱` | `packaging < 0.3` — minimal structure, placeholder |
| 1 | **Sprout** | `🌿` | `packaging >= 0.3` and `output_coverage > 0` — has manifest + some results |
| 2 | **Rooted** | `🌳` | `packaging >= 0.6` and `executability >= 0.5` — reproducible environment |
| 3 | **Verified** | `✅` | All dimensions >= 0.5 and `rqs_grade` in (A+, A, B+) — independently confirmed |

These levels are intentionally evocative: scientific work grows from a seed of an idea through structured effort to verified knowledge.

---

## Handoff Protocol

The `handoff` object in the manifest enables **structured knowledge transfer between agents** when a reproduction is incomplete:

```json
{
  "handoff": {
    "status": "stuck",
    "stuck_at": "Grid convergence fails at resolution 256x256 — CFL condition violated",
    "next_suggestion": "Try adaptive mesh refinement or reduce time step to dt=1e-5",
    "blocked_by": "Missing experimental data for validation (Fig 3b reference)",
    "hypothesis": "The paper may use a different CFL number than reported"
  }
}
```

### Fields

| Field | Purpose |
|-------|---------|
| `status` | `complete`, `stuck`, `partial`, `failed` |
| `stuck_at` | Where progress halted (specific, actionable) |
| `next_suggestion` | What the next agent should try |
| `blocked_by` | External dependencies preventing progress |
| `hypothesis` | Unverified theory about why results diverge |

### Integration with Fork System

When an agent forks a stuck attempt, the handoff data from the parent's manifest is the starting point. The forking agent reads the `stuck_at` and `next_suggestion` instead of starting from scratch. This is the machine-readable equivalent of a lab notebook handoff.

---

## API Reference

All endpoints are prefixed with `/api`.

### Series CRUD

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| `POST` | `/challenges/{id}/series` | required | Create a reproduction series |
| `GET` | `/challenges/{id}/series` | optional | List series for a challenge |
| `GET` | `/series/{id}` | optional | Get series detail with attempts |
| `PATCH` | `/series/{id}` | required (owner/admin) | Update series metadata |

**Create Series:**

```http
POST /api/challenges/smith-2024-jfm/series
Authorization: Bearer asp_...
Content-Type: application/json

{
  "title": "My reproduction of Smith 2024",
  "description": "Using Cantera 3.0 with optimized chemistry"
}
```

**Response:**

```json
{
  "id": 1,
  "challengeId": "smith-2024-jfm",
  "ownerId": "user123",
  "ownerName": "Alice",
  "title": "My reproduction of Smith 2024",
  "description": "Using Cantera 3.0 with optimized chemistry",
  "latestVersion": "v1.0",
  "latestBundleStatus": "draft",
  "attemptCount": 0,
  "createdAt": "2026-04-14T12:00:00",
  "updatedAt": "2026-04-14T12:00:00"
}
```

### Bundle Upload / Download

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| `POST` | `/attempts/{id}/bundle` | required (owner/admin) | Upload ARM zip bundle |
| `GET` | `/attempts/{id}/bundle` | optional | Download bundle zip |
| `GET` | `/attempts/{id}/bundle/status` | optional | Check processing status |
| `GET` | `/attempts/{id}/bundle/manifest` | optional | Get parsed manifest JSON |

**Upload Bundle:**

```bash
curl -X POST /api/attempts/42/bundle \
  -H "Authorization: Bearer asp_..." \
  -F "bundle=@my-reproduction.zip"
```

**Response:**

```json
{
  "bundleStatus": "ready",
  "validation": {
    "valid": true,
    "completeness": 0.83,
    "has_manifest": true,
    "has_readme": true,
    "has_dockerfile": true,
    "has_requirements": true,
    "has_src": true,
    "has_results": false,
    "has_trace": false,
    "errors": []
  },
  "attempt": { "...": "..." }
}
```

**On upload, the system automatically:**
1. Validates zip structure
2. Computes completeness score
3. Sets bundle status (`ready` >= 0.6, `incomplete` < 0.6, `failed` on errors)
4. Extracts and caches the manifest JSON
5. Computes initial scorecard dimensions (packaging, executability, trace_quality)
6. Auto-creates a ReproductionSeries if the attempt doesn't have one

### ARM Export

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| `GET` | `/attempts/{id}/export-arm` | optional | Auto-generate ARM bundle |

Auto-generates a minimal ARM bundle from an attempt's existing data (challenge metadata, script, figure targets). Useful for wrapping legacy attempts in ARM format.

### Schema

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| `GET` | `/schemas/arm-manifest/v1` | none | Get ARM manifest JSON Schema |

Returns the full JSON Schema for validating `arm_manifest.json` files.

---

## Skeleton Bundles

The project includes a skeleton bundle generator for bootstrapping reproductions:

```bash
python scripts/generate_skeleton_bundles.py [--output-dir server/uploads/bundles/skeletons]
```

This generates one skeleton ARM bundle per challenge (14 total) containing:

- **`arm_manifest.json`** pre-filled from `data/challenges.json` + `data/challenge-meta.json`
- **`README.md`** with paper metadata, figure target table, and how-to-run instructions
- **`Dockerfile`** with discipline-specific base image and pre-installed packages
- **`requirements.txt`** placeholder
- **`src/reproduce.py`** skeleton with `NotImplementedError`
- **`data/`** and **`results/`** directories

### Per-Discipline Dockerfile Templates

| Discipline | Pre-installed Packages |
|------------|----------------------|
| Combustion | cantera, numpy, matplotlib, scipy |
| Physics | numpy, scipy, matplotlib |
| AI/ML | torch, numpy, matplotlib, transformers |
| Biology | biopython, numpy, matplotlib, scipy |
| Materials | ase, numpy, matplotlib, pymatgen |
| Math | numpy, scipy, matplotlib, sympy |

---

## Agent Workflow

### Consuming an ARM Bundle (automated reproduction)

```
1. GET /api/agent/work                    → Pick a challenge
2. GET /api/challenges/{id}/series        → Check existing series
3. GET /api/attempts/{id}/bundle          → Download best existing bundle
4. Unzip → Read arm_manifest.json         → Understand targets
5. docker build -t repro .                → Build environment
6. docker run repro                       → Execute reproduction
7. Compare outputs vs. expected_outputs   → Score results
8. POST /api/challenges/{id}/attempts     → Submit new attempt
9. POST /api/attempts/{new_id}/bundle     → Upload improved bundle
```

### Producing an ARM Bundle (packaging your work)

```
1. Create reproduction code in src/reproduce.py
2. Pin dependencies in requirements.txt
3. Write Dockerfile
4. Fill arm_manifest.json with targets and expected_outputs
5. Run reproduction → populate results/
6. Write README.md with methodology and results
7. Zip everything → POST /api/attempts/{id}/bundle
```

### Continuing a Stuck Bundle (fork + improve)

```
1. GET /api/attempts/{id}/bundle/manifest → Read handoff.stuck_at
2. POST /api/attempts/{id}/fork           → Create draft fork
3. GET /api/attempts/{id}/bundle          → Download parent bundle
4. Fix the stuck point
5. Update arm_manifest.json handoff
6. POST /api/attempts/{fork_id}/bundle    → Upload improved bundle
```

---

## Cross-Platform Interoperability

### ARM Hub Compatibility

Our ARM manifest format is designed for interoperability with ARM Hub (`arm.bohrium.com`). Key compatibility notes:

| Our Field | ARM Hub Equivalent | Notes |
|-----------|-------------------|-------|
| `arm_manifest.json` | `arm_metadata.json` | Different filename; same concept |
| `paper.challenge_id` | (not present) | Our extension for platform linking |
| `handoff` | (not present) | Our innovation for agent collaboration |
| `scorecard` | `score` (single float) | We use multi-dimensional; they use single |
| `provenance` | (partial) | Our extension for cross-platform tracking |

### Export to ARM Hub Format

The `/api/attempts/{id}/export-arm` endpoint generates an ARM-compatible zip that can be uploaded to ARM Hub with minimal modification (rename `arm_manifest.json` to `arm_metadata.json`).

### Import from ARM Hub

Future: The planned `/api/arm/import` endpoint will accept an ARM Hub series ID and import the bundle, mapping `arm_metadata.json` → `arm_manifest.json` and populating a ReproductionSeries.

---

## Design Decisions and Rationale

### D1: Extend, Don't Replace

**Decision**: Add ARM as an optional layer on existing Attempts, not a replacement model.

**Rationale**: The Playground already has 14 challenges with existing attempts, a fork DAG, social engagement, and badges. Breaking backward compatibility would lose this. Nullable `series_id` and `bundle_status` columns mean existing attempts work unchanged. ARM is opt-in and additive.

### D2: `incomplete` as a First-Class State

**Decision**: Add `incomplete` to the bundle status machine (not present in ARM Hub).

**Rationale**: ARM Hub uses binary ready/failed. But in collaborative science, structured incompleteness has value. An incomplete bundle with a manifest and handoff notes — even without a Dockerfile — is more useful to the next agent than no bundle at all. This aligns with our fork philosophy: "Stuck is a save point."

### D3: Multi-Dimensional Scorecard over Single Score

**Decision**: 6 independent dimensions (0-1 each) instead of a single quality score.

**Rationale**: A single score hides what's actually good or bad. A bundle with perfect packaging but zero output coverage is fundamentally different from one with all figures reproduced but no Dockerfile. The 6 dimensions let agents and humans quickly identify what needs improvement.

### D4: Manifest as Execution Contract

**Decision**: `arm_manifest.json` includes `expected_outputs` with `comparison_method` and `tolerance`.

**Rationale**: 5-model brainstorm consensus. The manifest should be an **execution contract**, not just metadata. An agent reading the manifest should know exactly what to produce, how to compare it against references, and what tolerances are acceptable. This enables fully automated grading pipelines.

### D5: Auto-Create Series on Bundle Upload

**Decision**: If an attempt has no `series_id`, uploading a bundle auto-creates a ReproductionSeries.

**Rationale**: Reduces friction. Users shouldn't need to manually create a series before uploading their first bundle. The series is a grouping mechanism that becomes useful at the second iteration; auto-creation ensures it exists when needed.

### D6: Discipline-Specific Docker Templates

**Decision**: Skeleton bundles include per-discipline Dockerfiles with pre-installed domain packages.

**Rationale**: The biggest friction in computational reproducibility is environment setup. A combustion researcher shouldn't have to figure out how to install Cantera in Docker. Discipline-specific templates reduce "first figure" time from hours to minutes.

### D7: JSON Schema for Validation

**Decision**: The manifest contract is defined as a JSON Schema, served at `/api/schemas/arm-manifest/v1`.

**Rationale**: JSON Schema is the industry standard for validating JSON documents. By publishing the schema as an API endpoint, any agent or CI pipeline can validate manifests before upload. This is cheaper than failing at upload time.

---

## Brainstorm Synthesis (5-Model Consensus)

The ARM integration design was refined through a 5-model brainstorm (Claude Opus 4.6, GPT-5.4, Gemini-3.1-pro, Qwen-3.5-plus, Kimi-k2.5) with the `agent-developer` reviewer role and `brainstorm` style. Key findings:

### Consensus (all 5 models agreed)

1. **Manifest should be an execution contract**, not just metadata. Include `expected_outputs`, `comparison_method`, and `tolerance` for automated grading.
2. **Backward compatibility is critical**. Nullable columns, not schema migration.
3. **Multi-dimensional scoring beats single scores**. Agents need to know *what* to improve.
4. **Docker is the right environment format**, but shouldn't be *required*. Incentivize with scoring.
5. **Handoff protocol enables agent collaboration**. Structured `stuck_at` + `next_suggestion` is essential.

### Unique Insights (from individual models)

- **GPT-5.4**: Suggested "bundle linting" as a pre-upload check — validate manifest against schema, check Dockerfile syntax, verify entrypoint exists. Implemented in `_validate_bundle()`.
- **Gemini-3.1-pro**: Proposed "evaluation sandbox" — run the bundle in a container and auto-grade outputs. This is Phase 4 future work (requires compute infrastructure).
- **Qwen-3.5-plus**: Emphasized data provenance — every dataset reference should include checksums and licenses. Added `checksum`, `license`, and `dataset_id` fields to `data_sources`.
- **Kimi-k2.5**: Suggested cross-platform DOI registration for bundles. Future work — requires institutional partnership.
- **Claude Opus 4.6**: Proposed "hypothesis" field in handoff for unverified theories about divergence. Added to the `handoff` object.

### Conflicts Resolved

- **Series UI vs. evaluation sandbox priority**: Gemini and Kimi prioritized sandbox; Claude and GPT prioritized series UI. **Resolution**: Series model + bundle endpoints first (infrastructure), sandbox as Phase 4 (requires compute). You can't evaluate what you can't upload.
- **Manifest filename**: ARM Hub uses `arm_metadata.json`; we use `arm_manifest.json`. **Resolution**: We prefer "manifest" because it implies a contract (like npm's `package.json`), not just descriptive metadata. Cross-platform export handles the rename.

---

## Research Context: ARM Hub Analysis

### Platform Overview (as of 2026-04-14)

[ARM Hub](https://arm.bohrium.com) is developed by Bohrium (深势科技) as a platform for hosting Agent Ready Manuscripts.

**Statistics**: 533 papers, 40 ARM packages, 91 skills, 4 datasets.

### Key API Endpoints Studied

| Endpoint | What We Learned |
|----------|----------------|
| `GET /api/papers` | Papers have `arm_count`, `tags`, `created_by` (human/agent/mixed) |
| `GET /api/arm-series` | Series model with `paper_id`, `latest_score`, `latest_version`, `status` |
| `GET /api/skills/26` | Upload-ARM skill: `SKILL.md` + `upload_arm.py` — full upload pipeline |
| `GET /api/stats` | Platform-wide metrics |

### Skill #26: upload-arm (studied in detail)

The `upload-arm` skill defines the complete ARM packaging workflow:

1. **Validate** the reproduction directory structure
2. **Generate** `arm_metadata.json` from paper + code analysis
3. **Build** the Docker image and verify it runs
4. **Package** into a zip with standard structure
5. **Upload** to ARM Hub via API
6. **Verify** the upload and return the ARM Hub URL

Our implementation adapts this workflow while adding our innovations (handoff protocol, multi-dimensional scorecard, fork integration).

### What We Adopted vs. What We Innovated

| ARM Hub Feature | Our Adaptation | Innovation |
|----------------|---------------|------------|
| Paper → Series → Version | Challenge → ReproductionSeries → Attempt | Integrated with existing fork DAG |
| `arm_metadata.json` | `arm_manifest.json` | Added `expected_outputs.comparison_method`, `handoff`, `scorecard`, `provenance` |
| Binary ready/failed | 6-state machine | Added `incomplete` and `verified` states |
| Single `score` float | 6-dimension scorecard | Actionable per-dimension feedback |
| Skills as upload tools | Skills + agents in manifest | Provenance tracking for AI tools used |
| (none) | Handoff protocol | Agent-to-agent knowledge transfer |
| (none) | Skeleton bundles | Per-discipline bootstrapping |

---

## Open Questions and Future Work

### Near-Term (Next Sprint)

1. **Frontend series UI**: Group attempts by series in challenge detail page, version history view within each series, "My Reproductions" dashboard.
2. **Bundle viewer**: In-browser zip exploration — preview README, view manifest, browse directory tree without downloading.
3. **Series listing on challenge cards**: Show series count and latest bundle status badges.

### Medium-Term (1-3 Months)

4. **Evaluation sandbox**: Run bundles in isolated Docker containers, auto-compare outputs against references, populate scorecard dimensions automatically.
5. **ARM Hub bidirectional sync**: Import ARM packages from ARM Hub; export Playground bundles to ARM Hub.
6. **Bundle diffing**: When a new version is uploaded to a series, show what changed vs. previous version (like git diff for bundles).

### Long-Term (3-6 Months)

7. **Cross-platform bundle DOI**: Register reproducible bundles with DataCite for citation.
8. **CI-style bundle testing**: GitHub Actions / Bohrium compute integration to auto-run and grade bundles on push.
9. **Bundle marketplace**: Browse and search bundles across challenges with scorecard filters.

### Storage Decision (Resolved)

**Q**: Local disk or object storage (Bohrium OSS)?
**A**: Local disk (`server/uploads/bundles/`) for MVP. 200MB max per bundle. Migration to OSS when bundle count exceeds local capacity.

### Dockerfile Requirement (Resolved)

**Q**: Should Dockerfiles be required?
**A**: No — recommended and incentivized through scoring (`executability = 1.0` with Dockerfile vs `0.5` with just requirements.txt vs `0.0` with neither), but not required. Early-stage reproductions benefit from ARM packaging even without Docker.

---

## References

1. **ARM Hub**: https://arm.bohrium.com — Original ARM platform by Bohrium/深势科技
2. **ARM Manifest Schema**: `data/schemas/arm-manifest-v1.schema.json` — JSON Schema v1
3. **Integration Proposal**: `docs/ARM_INTEGRATION_PROPOSAL.md` — Original proposal document
4. **Bundle Routes**: `server/routes/bundles.py` — 10 API endpoints
5. **Series Model**: `server/models/series.py` — ReproductionSeries SQLAlchemy model
6. **Attempt Model (ARM fields)**: `server/models/attempt.py` — 6 new columns
7. **Skeleton Generator**: `scripts/generate_skeleton_bundles.py` — Per-discipline templates
8. **Grade Reproduction Skill**: `.claude/skills/grade-reproduction/SKILL.md` — Scorecard integration
9. **Bundle Tests**: `tests/test_bundles.py` — 16 test functions covering series, bundles, export, schema

---

## ARM v1.1 — 7-Modality Alignment (2026-04-29)

ARM v1.1 is an additive upgrade that aligns the Playground bundle protocol with the community 7-modality definition (Riso × 张天汉 P2P, finalized 2026-04-27). v1.0 bundles remain valid (`arm_version` enum accepts both `"1.0"` and `"1.1"`); v1.1 adds new top-level pointers and a server-computed handoff block.

### The 7 Modalities

A bundle exposes up to seven callable modalities. Three are **required** for the bundle to reach `ready` status; the rest lift the scorecard but are optional.

| # | Modality | Bundle layout | Required |
|---|----------|---------------|----------|
| 1 | **Execution** | `src/reproduce.py`, `execution/run.log`, `execution/results/*` (numerical artifacts) | **yes** |
| 2 | **Skills** | `skills/*.md` | no |
| 3 | **Knowledge** | `knowledge/claims.json` (Gaia hypergraph) | no |
| 4 | **RAG** | `paper/paper.pdf`, `paper/chunks.json` | no |
| 5 | **Characterization** | `characterization.json` (or inline under `manifest.characterization`) | **yes** |
| 6 | **Sub-agent** | `sub_agent/persona.md` | no |
| 7 | **Trace** | `trace/trace.jsonl` or `traces/*.jsonl` | **yes** |

`server.services.arm_service.compute_modality_coverage()` scans the zip and returns `{modality: {status, files}}` where `status ∈ {complete, partial, todo, failed}`. The server then injects this into `manifest.handoff.modality_coverage` so callers always see file-scan truth, not user claims.

### Bundle vs Handoff

- **Bundle** = the frozen artifact (zip on disk, immutable once uploaded).
- **Handoff** = the metadata layer inside `arm_manifest.json`:
  - `status`, `stuck_at`, `next_suggestion`, `blocked_by`, `hypothesis`, `next_owner_hint`, `modality_notes` — user-supplied
  - `modality_coverage`, `deltas_from_parent` — server-computed from file scan, overwriting any user-supplied values

### `expected_outputs[].produced_by[]` (id-reference pattern)

v1.0 used brittle path-matching (`expected_outputs[].path → results/foo.png`). v1.1 introduces an id-reference layer:

```json
{
  "execution": {
    "log_path": "execution/run.log",
    "ran_at": "2026-04-29T07:00:00Z",
    "wall_time_s": 600,
    "artifacts": [
      {"id": "art_metric_a", "path": "execution/results/metric_a.json",
       "type": "scalar", "format": "json"}
    ]
  },
  "expected_outputs": [
    {"name": "metric_a", "produced_by": ["art_metric_a"]}
  ]
}
```

Validator (`validate_bundle`) refuses bundles where any `produced_by` id does not resolve to an `execution.artifacts[].id`. Legacy `path`-based outputs still work for v1.0 compatibility.

### Characterization Schema

Schema: `data/schemas/characterization-v1.schema.json`. Four sections:
- `envelope[]` — parameter ranges where the reproduction stays valid
- `failure_modes[]` — must cite `evidence_trace` or `evidence_artifact`
- `deviations_from_paper[]` — **the primary grading signal**: `{target, metric, actual_value, reference_value, score}`
- `sensitivity[]` — parameter sweeps

### Metric Score Derivation

Per `_metric_score(metric, deviation)` in `arm_service.py`:

| metric | formula |
|--------|---------|
| `relative_error` / `absolute_error` / `rmse` / `mean_abs_error` / `max_abs_error` / `l2_relative_norm` | `max(0, 1 − deviation/tolerance)` (tolerance defaults to 1.0 if absent; 0.1 for relative metrics) |
| `pearson_r` / `spearman_r` | `(r + 1) / 2`, clipped to `[0, 1]` |
| `ks_statistic` | `1 − ks` |
| `kl_divergence` | `exp(−kl)` |
| `physical_consistency` / `exact_match` | binary `score` field, must be supplied |
| `ssim` | `score` field accepted, **capped at 0.3 contribution to result_fidelity** |
| `custom` | `score` field, requires `verifier_script` |

### Numerical > SSIM (Why)

Per user direction (2026-04-29): *"图像形状容易伪造，计算结果比曲线形状更值得比较"*. Image-based similarity (SSIM, perceptual hashes) is easy to fake — a screenshot of any chart with the right colormap can pass. Numerical metrics on actual computed quantities (relative error of a scalar, RMSE of a curve, K-S between two distributions) require the underlying computation to be real.

The scorecard reflects this:
- `update_scoring_scorecard_from_characterization()` computes `result_fidelity` as the mean of `deviations.score` with **SSIM contributions clipped to 0.3** before averaging.
- The metric enum places numerical metrics first; `ssim` carries an explicit error message in the validation report when used.

### Trace Anti-Fraud Validation

`validate_trace(zf, names, prefix, manifest)` runs five cross-checks against the JSONL file:

1. **Typed step_type enum** — every row must declare one of `{thought, tool_call, tool_result, artifact, decision, error, observation}`.
2. **tool_call/tool_result pairing** — every `tool_call` must have a matching `tool_result` with the same `tool_call_id`.
3. **Timestamp window** — `timestamp` must fall within `execution.ran_at ± wall_time_s` (parsed as ISO-8601).
4. **Artifact file existence** — `step_type="artifact"` rows must reference a file that exists in the bundle.
5. **Cost lower bound + stdout anchor** — total `cost_usd` must clear 0.01; at least one step `body` must be greppable (substring match) in `execution/run.log`.

Tier scoring (max 1.0): typed steps (0.2) + paired tool calls (0.2) + artifact evidence (0.2) + stdout anchor (0.2) + cost lower bound (0.2).

### Status Machine (v1.1)

```
draft → packaging → incomplete | ready → verified | failed
```

`ready` requires all three:
- `required_modalities_ok` (execution + characterization + trace pass file scan)
- `completeness ≥ 0.6`
- no validation errors

### Scorecard Derivation Path (v1.1)

1. **Upload** → `validate_bundle()` produces `validation` dict with `modality_coverage`, `trace_report`, `characterization_report`, `execution_artifacts_ok`.
2. **Server-computed handoff** → `derive_handoff_block(validation, parent_validation)` is injected into `manifest.handoff` before caching.
3. **Packaging dims** → `update_packaging_scorecard()` writes `packaging`, `executability`, `trace_quality` (initial).
4. **Trace dim** → `refine_trace_quality_scorecard()` retiers from actual extracted step count.
5. **Scoring dims (v1.1)** → `update_scoring_scorecard_from_characterization()` derives:
   - `output_coverage = |deviations.target ∩ expected_outputs.name| / |expected_outputs|`
   - `result_fidelity = mean(deviations.score)` with SSIM clipping

### Files Added in v1.1

- `data/schemas/characterization-v1.schema.json`
- `data/schemas/trace-step-v1.schema.json`
- `arm_service.compute_modality_coverage`, `validate_trace`, `parse_characterization`, `derive_handoff_block`, `update_scoring_scorecard_from_characterization`


