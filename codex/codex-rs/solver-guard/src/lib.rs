//! ASCodex's fail-closed policy and attempt ledger boundary.

use chrono::DateTime;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sha2::{Digest, Sha256};
use sqlx::{
    Row, Sqlite, SqlitePool, Transaction, sqlite::SqliteConnectOptions, sqlite::SqlitePoolOptions,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use thiserror::Error;

use codex_ascodex_coordination::{
    Action, ActorContext, PlatformObservation, PlatformReconcileItem, PlatformReconcileItemState,
    PlatformReconciliationSnapshot, ReconciliationApplyResult, RecoveryCanaryTrace,
    ResearchCycleRecord, Role, StageBrief,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Gate {
    Channel,
    Identity,
    Cadence,
    Redline,
    Trace,
    Model,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RpcDecision {
    Allow,
    Block,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineageFailure {
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineageRequest<'a> {
    pub parent_present: bool,
    pub depth: i32,
    pub role: Option<&'a str>,
    pub ephemeral: bool,
}

/// Apply the solver profile's parent/child invariants before any child resources are reserved.
/// The default profile deliberately keeps the policy small and explicit: solver children must
/// have a parent, stay within two levels, carry an approved role, and be durable.
pub fn lineage_preflight(
    request: &LineageRequest<'_>,
    solver_mode: bool,
) -> Result<(), LineageFailure> {
    if !solver_mode {
        return Ok(());
    }
    if !request.parent_present {
        return Err(LineageFailure {
            reason: "solver child is missing a parent lineage".to_string(),
        });
    }
    if request.depth < 1 || request.depth > 2 {
        return Err(LineageFailure {
            reason: format!(
                "solver child depth {} is outside the allowed range 1..=2",
                request.depth
            ),
        });
    }
    if request.ephemeral {
        return Err(LineageFailure {
            reason: "ephemeral solver children are not allowed".to_string(),
        });
    }
    let Some(role) = request.role else {
        return Err(LineageFailure {
            reason: "solver child must declare an approved role".to_string(),
        });
    };
    if !codex_ascodex_coordination::is_approved_solver_role(role) {
        return Err(LineageFailure {
            reason: format!("solver child role `{role}` is not approved"),
        });
    }
    Ok(())
}

/// Solver-mode workers are dispatched directly by the Chief/root thread. Keeping this stricter
/// rule separate preserves the generic lineage validator while preventing ordinary workers from
/// becoming a second dispatcher in ASCodex.
pub fn solver_spawn_depth_preflight(depth: i32, solver_mode: bool) -> Result<(), LineageFailure> {
    if solver_mode && depth != 1 {
        return Err(LineageFailure {
            reason: format!(
                "solver child depth {} is outside the direct-Chief range (expected 1)",
                depth
            ),
        });
    }
    Ok(())
}

pub fn rpc_preflight(method: &str, solver_mode: bool) -> RpcDecision {
    if !solver_mode {
        return RpcDecision::Allow;
    }

    const HIGH_RISK_METHODS: &[&str] = &[
        "command/exec",
        "process/spawn",
        "fs/writeFile",
        "fs/createDirectory",
        "fs/remove",
        "fs/copy",
        "mcpServer/",
        "config/value/write",
        "config/batchWrite",
        "config/mcpServer/reload",
        "environment/add",
        "plugin/install",
        "plugin/uninstall",
        "thread/settings/update",
        "item/tool/call",
        "thread/fork",
    ];

    if HIGH_RISK_METHODS
        .iter()
        .any(|prefix| method == *prefix || method.starts_with(prefix))
    {
        RpcDecision::Block
    } else {
        RpcDecision::Allow
    }
}

pub fn tool_preflight(tool_name: &str, solver_mode: bool) -> RpcDecision {
    if !solver_mode {
        return RpcDecision::Allow;
    }
    const SUBMISSION_TOOLS: &[&str] = &[
        "solver-guard_build-submit",
        "solver-guard_bohr",
        "playground_submit",
        "submit_attempt",
    ];
    if SUBMISSION_TOOLS
        .iter()
        .any(|name| tool_name == *name || tool_name.ends_with(name))
    {
        RpcDecision::Block
    } else {
        RpcDecision::Allow
    }
}

/// Apply a second, argument-aware check to shell-like tools. Local computation remains usable,
/// while obvious platform submission commands must go through the broker. Network egress for
/// arbitrary scripts is enforced by the execution sandbox in a later integration stage.
pub fn tool_preflight_with_input(
    tool_name: &str,
    input: &serde_json::Value,
    solver_mode: bool,
) -> RpcDecision {
    if matches!(tool_preflight(tool_name, solver_mode), RpcDecision::Block) {
        return RpcDecision::Block;
    }
    if !solver_mode {
        return RpcDecision::Allow;
    }
    let lower_name = tool_name.to_ascii_lowercase();
    if lower_name.starts_with("mcp__") || lower_name.starts_with("dynamic__") {
        return RpcDecision::Block;
    }
    let is_shell = matches!(
        lower_name.as_str(),
        "exec_command" | "shell_command" | "unified_exec"
    );
    if !is_shell {
        return RpcDecision::Allow;
    }
    let text = input.to_string().to_ascii_lowercase();
    const SUBMISSION_MARKERS: &[&str] = &[
        "play.bohrium.com",
        "playground-cli",
        "playground_submit",
        "submit-attempt",
        "solver-guard_build-submit",
        "ascodex-lease-admin",
        "ascodex-stage-admin",
        "ascodex-observation-admin",
        "bohr job submit",
        "curl ",
        "invoke-webrequest",
        "http://",
        "https://",
    ];
    if SUBMISSION_MARKERS
        .iter()
        .any(|marker| text.contains(marker))
    {
        RpcDecision::Block
    } else {
        RpcDecision::Allow
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GateFailure {
    pub gate: Gate,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Admission {
    pub allowed: bool,
    pub failures: Vec<GateFailure>,
}

impl Admission {
    fn denied(gate: Gate, reason: impl Into<String>) -> Self {
        Self {
            allowed: false,
            failures: vec![GateFailure {
                gate,
                reason: reason.into(),
            }],
        }
    }

    fn allow() -> Self {
        Self {
            allowed: true,
            failures: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub channel: ChannelPolicy,
    pub identity: IdentityPolicy,
    pub cadence: CadencePolicy,
    pub redline: RedlinePolicy,
    pub trace: TracePolicy,
    pub model: ModelPolicy,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ChannelPolicy {
    pub harbor_only: bool,
    pub workspace_root: PathBuf,
    /// Optional canonical root for the trusted CLI executable. When omitted, the assigned
    /// workspace root is used; an explicit root is required for a CLI installed elsewhere.
    #[serde(default)]
    pub trusted_cli_root: Option<PathBuf>,
    pub trusted_cli_sha256: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IdentityPolicy {
    pub name: String,
    pub challenge_id: String,
    pub owner: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CadencePolicy {
    pub min_interval_seconds: i64,
    pub max_estimated_cost_usd: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RedlinePolicy {
    pub clean: bool,
    /// Additional case-insensitive substrings supplied by the campaign policy.
    /// The built-in terms below remain enabled and cannot be removed by callers.
    #[serde(default)]
    pub banned_terms: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TracePolicy {
    pub real_execution: bool,
    pub paired_tool_events: bool,
    pub artifact_provenance: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ModelPolicy {
    pub provider: String,
    pub model: String,
}

#[derive(Debug, Clone)]
pub struct AdmissionRequest<'a> {
    pub channel: &'a str,
    pub identity: &'a str,
    pub challenge_id: &'a str,
    pub owner: &'a str,
    pub cli_path: &'a Path,
    pub workspace: &'a Path,
    pub estimated_cost_usd: f64,
    pub trace: TraceEvidence,
    pub provider: &'a str,
    pub model: &'a str,
    pub content_sha256: &'a str,
    pub now_ms: i64,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct TraceEvidence {
    pub real_execution: bool,
    pub paired_tool_events: bool,
    pub artifact_provenance: bool,
}

/// The execution block emitted by an ARM bundle. This is deliberately separate from the
/// trace booleans: a caller cannot turn a synthetic trace into a real execution by setting a
/// flag, and the block is checked against files and timestamps owned by the workspace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionRecord {
    pub run_id: String,
    pub session_id: String,
    pub agent_id: String,
    pub ran_at_ms: i64,
    pub wall_time_ms: i64,
    pub log_path: PathBuf,
    pub cwd: PathBuf,
    pub entrypoint: String,
    pub status: String,
    pub exit_code: i64,
    pub run_log_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RedlineFinding {
    pub path: PathBuf,
    pub term: String,
}

#[derive(Debug, Error)]
pub enum RedlineValidationError {
    #[error("redline evidence path is outside the assigned workspace: {0}")]
    OutsideWorkspace(String),
    #[error("redline evidence cannot be read: {0}")]
    Io(#[from] std::io::Error),
    #[error("redline scan failed: {0}")]
    Invalid(String),
}

#[derive(Debug, Error)]
pub enum TraceValidationError {
    #[error("trace evidence path is outside the assigned workspace: {0}")]
    OutsideWorkspace(String),
    #[error("trace evidence cannot be read: {0}")]
    Io(#[from] std::io::Error),
    #[error("trace evidence is invalid: {0}")]
    Invalid(String),
}

/// Validate an ARM execution block and bind it to the supplied trace and run log. The function
/// accepts the field spellings used by the Playground bundles (`execution_id`/`run_id`,
/// `ran_at`/`ran_at_ms`, `wall_time_s`/`wall_time_ms`) but requires one unambiguous value for
/// every security-relevant field.
pub fn validate_execution_record(
    workspace: &Path,
    execution_manifest_path: &Path,
    trace_path: &Path,
    run_log_path: &Path,
) -> Result<ExecutionRecord, TraceValidationError> {
    let workspace = workspace.canonicalize()?;
    let manifest_path = canonical_evidence_path(&workspace, execution_manifest_path)?;
    let trace_path = canonical_evidence_path(&workspace, trace_path)?;
    let run_log_path = canonical_evidence_path(&workspace, run_log_path)?;
    let manifest: JsonValue = serde_json::from_str(&std::fs::read_to_string(&manifest_path)?)
        .map_err(|err| TraceValidationError::Invalid(format!("execution manifest: {err}")))?;
    let execution = manifest.get("execution").ok_or_else(|| {
        TraceValidationError::Invalid("execution manifest must contain an execution object".into())
    })?;
    let object = execution
        .as_object()
        .ok_or_else(|| TraceValidationError::Invalid("execution must be a JSON object".into()))?;
    let required = |names: &[&str]| -> Result<String, TraceValidationError> {
        names
            .iter()
            .find_map(|name| object.get(*name).and_then(JsonValue::as_str))
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string)
            .ok_or_else(|| {
                TraceValidationError::Invalid(format!(
                    "execution field {} is required",
                    names.join(" or ")
                ))
            })
    };
    let run_id = required(&["run_id", "execution_id"])?;
    let session_id = required(&["session_id"])?;
    let agent_id = required(&["agent_id"])?;
    let entrypoint = required(&["entrypoint", "command"])?;
    let status = required(&["status"])?;
    let log_raw = required(&["log_path", "run_log_path"])?;
    let cwd_raw = required(&["cwd", "working_directory"])?;
    let ran_at_ms = parse_execution_timestamp(object)?;
    let wall_time_ms = parse_execution_wall_time(object)?;
    let exit_code = object
        .get("exit_code")
        .and_then(JsonValue::as_i64)
        .ok_or_else(|| TraceValidationError::Invalid("execution exit_code is required".into()))?;
    if status != "ok" && status != "success" && status != "completed" {
        return Err(TraceValidationError::Invalid(format!(
            "execution status `{status}` is not successful"
        )));
    }
    if exit_code != 0 {
        return Err(TraceValidationError::Invalid(
            "execution exit_code must be zero".into(),
        ));
    }
    let log_path = canonical_evidence_path(&workspace, Path::new(&log_raw))?;
    let cwd = canonical_directory_path(&workspace, Path::new(&cwd_raw))?;
    if !cwd.is_dir() {
        return Err(TraceValidationError::Invalid(
            "execution cwd must be a directory".into(),
        ));
    }
    if log_path != run_log_path {
        return Err(TraceValidationError::Invalid(
            "execution log_path does not match the supplied run log".into(),
        ));
    }
    let run_log_sha256 = sha256_file(&run_log_path)?;
    if let Some(declared) = object
        .get("run_log_sha256")
        .or_else(|| object.get("stdout_sha256"))
        .and_then(JsonValue::as_str)
    {
        if !declared.eq_ignore_ascii_case(&run_log_sha256) {
            return Err(TraceValidationError::Invalid(
                "execution run log hash does not match".into(),
            ));
        }
    }
    let steps = parse_trace_steps(&std::fs::read_to_string(&trace_path)?)?;
    let start = ran_at_ms.saturating_sub(5 * 60 * 1000);
    let end = ran_at_ms
        .saturating_add(wall_time_ms)
        .saturating_add(5 * 60 * 1000);
    for (index, step) in steps.iter().enumerate() {
        let timestamp = parse_timestamp(step.get("timestamp"), index)?;
        if timestamp < start || timestamp > end {
            return Err(TraceValidationError::Invalid(format!(
                "trace timestamp at step {} is outside the execution window",
                index + 1
            )));
        }
    }
    let modified_ms = file_modified_ms(&run_log_path)?;
    if modified_ms < start || modified_ms > end {
        return Err(TraceValidationError::Invalid(
            "run log modification time is outside the execution window".into(),
        ));
    }
    Ok(ExecutionRecord {
        run_id,
        session_id,
        agent_id,
        ran_at_ms,
        wall_time_ms,
        log_path,
        cwd,
        entrypoint,
        status,
        exit_code,
        run_log_sha256,
    })
}

fn parse_execution_timestamp(
    object: &serde_json::Map<String, JsonValue>,
) -> Result<i64, TraceValidationError> {
    if let Some(value) = object.get("ran_at_ms").and_then(JsonValue::as_i64) {
        return Ok(value);
    }
    let value = object.get("ran_at").ok_or_else(|| {
        TraceValidationError::Invalid("execution ran_at or ran_at_ms is required".into())
    })?;
    parse_timestamp(Some(value), 0)
}

fn parse_execution_wall_time(
    object: &serde_json::Map<String, JsonValue>,
) -> Result<i64, TraceValidationError> {
    if let Some(value) = object.get("wall_time_ms").and_then(JsonValue::as_i64) {
        if value > 0 {
            return Ok(value);
        }
    }
    let seconds = object
        .get("wall_time_s")
        .and_then(JsonValue::as_f64)
        .filter(|value| value.is_finite() && *value > 0.0)
        .ok_or_else(|| {
            TraceValidationError::Invalid(
                "execution wall_time_s or wall_time_ms is required".into(),
            )
        })?;
    if seconds > 24.0 * 60.0 * 60.0 {
        return Err(TraceValidationError::Invalid(
            "execution wall time exceeds 24 hours".into(),
        ));
    }
    Ok((seconds * 1000.0).round() as i64)
}

fn file_modified_ms(path: &Path) -> Result<i64, TraceValidationError> {
    let modified = std::fs::metadata(path)?.modified()?;
    let elapsed = modified
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|err| {
            TraceValidationError::Invalid(format!("invalid file modification time: {err}"))
        })?;
    i64::try_from(elapsed.as_millis())
        .map_err(|_| TraceValidationError::Invalid("file modification time is too large".into()))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArtifactManifest {
    artifacts: Vec<ArtifactManifestEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArtifactManifestEntry {
    path: String,
    sha256: String,
}

/// Validate trace provenance from files owned by the assigned workspace. This intentionally
/// derives all three admission signals instead of trusting caller-supplied booleans.
pub fn validate_trace_evidence(
    workspace: &Path,
    trace_path: &Path,
    run_log_path: &Path,
    artifact_manifest_path: &Path,
) -> Result<TraceEvidence, TraceValidationError> {
    let workspace = workspace.canonicalize()?;
    let trace_path = canonical_evidence_path(&workspace, trace_path)?;
    let run_log_path = canonical_evidence_path(&workspace, run_log_path)?;
    let artifact_manifest_path = canonical_evidence_path(&workspace, artifact_manifest_path)?;

    let trace_text = std::fs::read_to_string(&trace_path)?;
    if trace_text.len() > 8 * 1024 * 1024 {
        return Err(TraceValidationError::Invalid(
            "trace exceeds the 8 MiB admission limit".to_string(),
        ));
    }
    let run_log = std::fs::read_to_string(&run_log_path)?;
    if run_log.trim().is_empty() {
        return Err(TraceValidationError::Invalid(
            "execution run.log is empty".to_string(),
        ));
    }
    let steps = parse_trace_steps(&trace_text)?;
    validate_trace_steps(&steps, &run_log)?;

    let manifest: ArtifactManifest =
        serde_json::from_str(&std::fs::read_to_string(&artifact_manifest_path)?)
            .map_err(|err| TraceValidationError::Invalid(format!("artifact manifest: {err}")))?;
    let artifacts = validate_artifact_manifest(&workspace, &manifest)?;
    validate_trace_artifact_refs(&steps, &trace_path, &artifacts)?;

    Ok(TraceEvidence {
        real_execution: true,
        paired_tool_events: true,
        artifact_provenance: true,
    })
}

const BUILTIN_REDLINE_TERMS: &[&str] = &[
    "attempt id",
    "attempt_id",
    "harbor_reward",
    "trace_score",
    "scoringdetails",
    "leaderboard",
    "judge verdict",
    "red team",
    "red-team",
    "opponent",
    "prior score",
    "high score",
];

/// Scan the trace, execution log, and manifest-listed text artifacts for information that
/// could only have come from platform feedback or another solver. The caller-provided boolean
/// is deliberately not part of this API: admission must be derived from files owned by the
/// workspace. Binary artifacts are ignored; textual source/comment files are scanned.
pub fn validate_redline_evidence(
    workspace: &Path,
    trace_path: &Path,
    run_log_path: &Path,
    artifact_manifest_path: &Path,
    policy: &RedlinePolicy,
) -> Result<Vec<RedlineFinding>, RedlineValidationError> {
    let workspace = workspace.canonicalize().map_err(|err| {
        RedlineValidationError::Invalid(format!("workspace cannot be resolved: {err}"))
    })?;
    let trace_path = canonical_redline_path(&workspace, trace_path)?;
    let run_log_path = canonical_redline_path(&workspace, run_log_path)?;
    let manifest_path = canonical_redline_path(&workspace, artifact_manifest_path)?;
    let manifest: ArtifactManifest =
        serde_json::from_str(&std::fs::read_to_string(&manifest_path)?)
            .map_err(|err| RedlineValidationError::Invalid(format!("artifact manifest: {err}")))?;
    let artifacts = validate_artifact_manifest(&workspace, &manifest)
        .map_err(|err| RedlineValidationError::Invalid(err.to_string()))?;

    let mut paths = BTreeSet::from([trace_path, run_log_path]);
    paths.extend(artifacts.keys().cloned());
    let mut terms = BUILTIN_REDLINE_TERMS
        .iter()
        .map(|term| (*term).to_string())
        .collect::<Vec<_>>();
    terms.extend(
        policy
            .banned_terms
            .iter()
            .filter(|term| !term.trim().is_empty())
            .map(|term| term.to_ascii_lowercase()),
    );
    terms.sort();
    terms.dedup();

    let mut findings = Vec::new();
    for path in paths {
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::InvalidData => {
                // Non-UTF8 outputs (for example npz files) are binary artifacts, not transcript
                // surfaces. Their integrity is covered by the manifest hash check above.
                continue;
            }
            Err(error) => return Err(RedlineValidationError::Io(error)),
        };
        let lower = text.to_ascii_lowercase();
        for term in &terms {
            if lower.contains(term) {
                findings.push(RedlineFinding {
                    path: path.clone(),
                    term: term.clone(),
                });
            }
        }
        if contains_attempt_number(&lower) {
            findings.push(RedlineFinding {
                path: path.clone(),
                term: "attempt number".to_string(),
            });
        }
    }
    Ok(findings)
}

fn canonical_redline_path(
    workspace: &Path,
    supplied: &Path,
) -> Result<PathBuf, RedlineValidationError> {
    let candidate = if supplied.is_absolute() {
        supplied.to_path_buf()
    } else {
        workspace.join(supplied)
    };
    let canonical = candidate.canonicalize()?;
    if !canonical.starts_with(workspace) || !canonical.is_file() {
        return Err(RedlineValidationError::OutsideWorkspace(
            supplied.display().to_string(),
        ));
    }
    Ok(canonical)
}

fn contains_attempt_number(text: &str) -> bool {
    let bytes = text.as_bytes();
    let needle = b"attempt";
    let mut offset = 0;
    while let Some(relative) = text[offset..].find("attempt") {
        let start = offset + relative + needle.len();
        let suffix = &bytes[start..];
        let mut index = 0;
        while index < suffix.len() && suffix[index].is_ascii_whitespace() {
            index += 1;
        }
        if index < suffix.len() && (suffix[index] == b'_' || suffix[index] == b'-') {
            index += 1;
            while index < suffix.len() && suffix[index].is_ascii_whitespace() {
                index += 1;
            }
        }
        let digit_start = index;
        while index < suffix.len() && suffix[index].is_ascii_digit() {
            index += 1;
        }
        if index > digit_start {
            return true;
        }
        offset = start;
        if offset >= text.len() {
            break;
        }
    }
    false
}

fn canonical_evidence_path(
    workspace: &Path,
    supplied: &Path,
) -> Result<PathBuf, TraceValidationError> {
    let candidate = if supplied.is_absolute() {
        supplied.to_path_buf()
    } else {
        workspace.join(supplied)
    };
    let canonical = candidate.canonicalize()?;
    if !canonical.starts_with(workspace) || !canonical.is_file() {
        return Err(TraceValidationError::OutsideWorkspace(
            supplied.display().to_string(),
        ));
    }
    Ok(canonical)
}

fn canonical_directory_path(
    workspace: &Path,
    supplied: &Path,
) -> Result<PathBuf, TraceValidationError> {
    let candidate = if supplied.is_absolute() {
        supplied.to_path_buf()
    } else {
        workspace.join(supplied)
    };
    let canonical = candidate.canonicalize()?;
    if !canonical.starts_with(workspace) || !canonical.is_dir() {
        return Err(TraceValidationError::OutsideWorkspace(
            supplied.display().to_string(),
        ));
    }
    Ok(canonical)
}

fn parse_trace_steps(text: &str) -> Result<Vec<JsonValue>, TraceValidationError> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(TraceValidationError::Invalid("trace is empty".to_string()));
    }
    let steps = if trimmed.starts_with('[') {
        serde_json::from_str::<Vec<JsonValue>>(trimmed)
            .map_err(|err| TraceValidationError::Invalid(format!("trace JSON array: {err}")))?
    } else {
        trimmed
            .lines()
            .enumerate()
            .map(|(index, line)| {
                serde_json::from_str::<JsonValue>(line).map_err(|err| {
                    TraceValidationError::Invalid(format!("trace JSONL line {}: {err}", index + 1))
                })
            })
            .collect::<Result<Vec<_>, _>>()?
    };
    if steps.is_empty() || steps.len() > 10_000 || steps.iter().any(|step| !step.is_object()) {
        return Err(TraceValidationError::Invalid(
            "trace must contain 1..=10000 JSON objects".to_string(),
        ));
    }
    Ok(steps)
}

fn validate_trace_steps(steps: &[JsonValue], run_log: &str) -> Result<(), TraceValidationError> {
    const ALLOWED: &[&str] = &[
        "thought",
        "tool_call",
        "tool_result",
        "artifact",
        "decision",
        "error",
        "observation",
    ];
    let mut calls = BTreeMap::<String, usize>::new();
    let mut results = BTreeMap::<String, usize>::new();
    let mut step_ids = BTreeSet::<String>::new();
    let mut previous_timestamp = None;
    let mut stdout_anchored = false;
    let mut thought_count = 0usize;
    let mut long_thought_count = 0usize;
    let mut total_cost = 0.0f64;

    for (index, step) in steps.iter().enumerate() {
        let order = step.get("step_order").and_then(JsonValue::as_u64);
        if order != Some((index + 1) as u64) {
            return Err(TraceValidationError::Invalid(format!(
                "step_order must be contiguous at step {}",
                index + 1
            )));
        }
        let step_type = required_string(step, "step_type", index)?;
        if !ALLOWED.contains(&step_type) {
            return Err(TraceValidationError::Invalid(format!(
                "unsupported step_type `{step_type}` at step {}",
                index + 1
            )));
        }
        let step_id = required_string(step, "step_id", index)?;
        if !step_ids.insert(step_id.to_string()) {
            return Err(TraceValidationError::Invalid(
                "step_id must be non-empty and unique".to_string(),
            ));
        }
        for field in ["duration_s", "cost_usd"] {
            let value = step.get(field).and_then(JsonValue::as_f64).ok_or_else(|| {
                TraceValidationError::Invalid(format!("{field} is required at step {}", index + 1))
            })?;
            if !value.is_finite() || value < 0.0 {
                return Err(TraceValidationError::Invalid(format!(
                    "{field} must be finite and non-negative"
                )));
            }
            if field == "cost_usd" {
                total_cost += value;
            }
        }
        let tokens = step
            .get("tokens")
            .and_then(JsonValue::as_i64)
            .ok_or_else(|| {
                TraceValidationError::Invalid(format!("tokens is required at step {}", index + 1))
            })?;
        if tokens < 0 {
            return Err(TraceValidationError::Invalid(
                "tokens must be non-negative".to_string(),
            ));
        }
        let timestamp = parse_timestamp(step.get("timestamp"), index)?;
        if previous_timestamp.is_some_and(|previous| timestamp < previous) {
            return Err(TraceValidationError::Invalid(
                "trace timestamps must be monotonic".to_string(),
            ));
        }
        previous_timestamp = Some(timestamp);

        match step_type {
            "thought" => {
                let body = required_string(step, "body", index)?;
                thought_count += 1;
                if body.chars().count() >= 80 {
                    long_thought_count += 1;
                }
            }
            "tool_call" => {
                let id = required_string(step, "tool_call_id", index)?;
                required_string(step, "tool_name", index)?;
                if step.get("tool_args").is_none_or(JsonValue::is_null) {
                    return Err(TraceValidationError::Invalid(format!(
                        "tool_args is required at step {}",
                        index + 1
                    )));
                }
                *calls.entry(id.to_string()).or_default() += 1;
            }
            "tool_result" => {
                if index == 0
                    || steps[index - 1]
                        .get("step_type")
                        .and_then(JsonValue::as_str)
                        != Some("tool_call")
                {
                    return Err(TraceValidationError::Invalid(
                        "each tool_result must immediately follow its tool_call".to_string(),
                    ));
                }
                let id = required_string(step, "tool_call_id", index)?;
                let body = required_string(step, "body", index)?;
                if body.trim().chars().count() >= 16 && run_log.contains(body) {
                    stdout_anchored = true;
                }
                *results.entry(id.to_string()).or_default() += 1;
            }
            _ => {}
        }
    }
    if calls.is_empty()
        || calls != results
        || calls.values().any(|count| *count != 1)
        || results.values().any(|count| *count != 1)
    {
        return Err(TraceValidationError::Invalid(
            "tool_call/tool_result pairs must be non-empty and exactly 1:1".to_string(),
        ));
    }
    if thought_count < 3 || long_thought_count < 3 {
        return Err(TraceValidationError::Invalid(
            "trace requires at least three thought steps with bodies of 80+ characters".to_string(),
        ));
    }
    if total_cost < 0.01 {
        return Err(TraceValidationError::Invalid(
            "trace total cost_usd must be at least 0.01".to_string(),
        ));
    }
    if !stdout_anchored {
        return Err(TraceValidationError::Invalid(
            "no tool_result body is anchored in execution run.log".to_string(),
        ));
    }
    Ok(())
}

fn required_string<'a>(
    step: &'a JsonValue,
    field: &str,
    index: usize,
) -> Result<&'a str, TraceValidationError> {
    step.get(field)
        .and_then(JsonValue::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            TraceValidationError::Invalid(format!("{field} is required at step {}", index + 1))
        })
}

fn parse_timestamp(value: Option<&JsonValue>, index: usize) -> Result<i64, TraceValidationError> {
    if let Some(number) = value.and_then(JsonValue::as_i64) {
        return Ok(number);
    }
    let text = value.and_then(JsonValue::as_str).ok_or_else(|| {
        TraceValidationError::Invalid(format!("timestamp is required at step {}", index + 1))
    })?;
    DateTime::parse_from_rfc3339(text)
        .map(|timestamp| timestamp.timestamp_millis())
        .map_err(|err| {
            TraceValidationError::Invalid(format!("invalid timestamp at step {}: {err}", index + 1))
        })
}

fn validate_artifact_manifest(
    workspace: &Path,
    manifest: &ArtifactManifest,
) -> Result<BTreeMap<PathBuf, String>, TraceValidationError> {
    if manifest.artifacts.is_empty() {
        return Err(TraceValidationError::Invalid(
            "artifact manifest must contain at least one artifact".to_string(),
        ));
    }
    let mut artifacts = BTreeMap::new();
    for artifact in &manifest.artifacts {
        if artifact.path.trim().is_empty()
            || artifact.sha256.len() != 64
            || !artifact.sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(TraceValidationError::Invalid(
                "artifact path and 64-character SHA-256 are required".to_string(),
            ));
        }
        let path = canonical_evidence_path(workspace, Path::new(&artifact.path))?;
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| {
                matches!(
                    name.to_ascii_lowercase().as_str(),
                    "trace.jsonl" | "run.log" | "artifacts.json" | "raw_messages.jsonl"
                )
            })
        {
            return Err(TraceValidationError::Invalid(
                "evidence files cannot be submitted as business artifacts".to_string(),
            ));
        }
        let actual = sha256_file(&path)?;
        if !actual.eq_ignore_ascii_case(&artifact.sha256) {
            return Err(TraceValidationError::Invalid(format!(
                "artifact hash mismatch for {}",
                artifact.path
            )));
        }
        if artifacts.insert(path, actual).is_some() {
            return Err(TraceValidationError::Invalid(
                "artifact manifest contains a duplicate path".to_string(),
            ));
        }
    }
    Ok(artifacts)
}

fn validate_trace_artifact_refs(
    steps: &[JsonValue],
    trace_path: &Path,
    artifacts: &BTreeMap<PathBuf, String>,
) -> Result<(), TraceValidationError> {
    let trace_dir = trace_path.parent().ok_or_else(|| {
        TraceValidationError::Invalid("trace path has no parent directory".to_string())
    })?;
    let mut referenced = 0usize;
    for step in steps
        .iter()
        .filter(|step| step.get("step_type").and_then(JsonValue::as_str) == Some("artifact"))
    {
        let raw = step
            .get("artifact_path")
            .and_then(JsonValue::as_str)
            .filter(|path| !path.trim().is_empty())
            .ok_or_else(|| {
                TraceValidationError::Invalid(
                    "artifact trace step requires artifact_path".to_string(),
                )
            })?;
        let candidate = trace_dir.join(raw).canonicalize()?;
        if !artifacts.contains_key(&candidate) {
            return Err(TraceValidationError::Invalid(format!(
                "trace artifact is absent from the verified manifest: {raw}"
            )));
        }
        referenced += 1;
    }
    if referenced == 0 {
        return Err(TraceValidationError::Invalid(
            "trace must reference at least one verified artifact".to_string(),
        ));
    }
    Ok(())
}

impl Policy {
    pub fn from_yaml(yaml: &str) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(yaml)
    }

    pub fn admit(&self, request: &AdmissionRequest<'_>) -> Admission {
        if !self.channel.harbor_only || request.channel != "harbor" {
            return Admission::denied(
                Gate::Channel,
                "submission channel is not the verified Harbor route",
            );
        }
        if request.identity != self.identity.name
            || request.challenge_id != self.identity.challenge_id
            || request.owner != self.identity.owner
        {
            return Admission::denied(
                Gate::Identity,
                "identity, challenge, or owner binding does not match policy",
            );
        }
        if !request.estimated_cost_usd.is_finite()
            || !self.cadence.max_estimated_cost_usd.is_finite()
            || self.cadence.max_estimated_cost_usd < 0.0
            || request.estimated_cost_usd < 0.0
            || request.estimated_cost_usd > self.cadence.max_estimated_cost_usd
        {
            return Admission::denied(
                Gate::Cadence,
                "estimated cost is missing or exceeds the reservation limit",
            );
        }
        let trusted_cli_root = self
            .channel
            .trusted_cli_root
            .as_deref()
            .unwrap_or(&self.channel.workspace_root);
        if !is_within_workspace(request.workspace, &self.channel.workspace_root)
            || !request.cli_path.is_absolute()
            || !is_within_workspace(request.cli_path, trusted_cli_root)
        {
            return Admission::denied(
                Gate::Channel,
                "submission or trusted CLI path is outside the assigned workspace",
            );
        }
        if !self.redline.clean {
            return Admission::denied(Gate::Redline, "redline scan is not clean");
        }
        if !request.trace.real_execution
            || !request.trace.paired_tool_events
            || !request.trace.artifact_provenance
            || !self.trace.real_execution
            || !self.trace.paired_tool_events
            || !self.trace.artifact_provenance
        {
            return Admission::denied(
                Gate::Trace,
                "trace lacks real execution, paired events, or artifact provenance",
            );
        }
        if request.provider != self.model.provider || request.model != self.model.model {
            return Admission::denied(Gate::Model, "effective provider/model is not approved");
        }
        let actual_hash = match sha256_file(request.cli_path) {
            Ok(hash) => hash,
            Err(_) => {
                return Admission::denied(
                    Gate::Channel,
                    "trusted CLI cannot be read for hash verification",
                );
            }
        };
        if !self
            .channel
            .trusted_cli_sha256
            .eq_ignore_ascii_case(&actual_hash)
        {
            return Admission::denied(Gate::Channel, "trusted CLI hash does not match policy");
        }
        Admission::allow()
    }
}

#[derive(Debug, Error)]
pub enum LedgerError {
    #[error("ledger database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("ledger is degraded: {0}")]
    Degraded(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoordinationEventRecord<'a> {
    pub event_id: &'a str,
    pub idempotency_key: &'a str,
    pub aggregate_type: &'a str,
    pub aggregate_id: &'a str,
    pub expected_version: u64,
    pub event_type: &'a str,
    pub payload_json: &'a str,
    pub occurred_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ActorLeaseMetadata {
    pub lease_id: String,
    pub agent_id: String,
    pub session_id: String,
    pub thread_id: String,
    pub campaign_id: String,
    pub challenge_id: String,
    pub role: String,
    pub registered_at_ms: i64,
    pub revoked_at_ms: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageBriefLedgerTarget<'a> {
    pub cycle_id: &'a str,
    pub campaign_id: &'a str,
    pub challenge_id: &'a str,
    pub role: Role,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersistedStageBrief {
    pub cycle_id: String,
    pub campaign_id: String,
    pub challenge_id: String,
    pub role: Role,
    pub stage_brief: StageBrief,
    pub workspace_root: PathBuf,
    pub capability_map_path: String,
    pub cycle_event_version: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CycleIssuance {
    pub cycle_id: String,
    pub brief_ids: Vec<String>,
    pub cycle_event_version: u64,
    pub cycle_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PersistedPlatformObservation {
    pub observation_id: String,
    pub campaign_id: String,
    pub challenge_id: String,
    pub attempt_id: String,
    pub monitor_lease_id: String,
    pub observation: PlatformObservation,
    pub observation_sha256: String,
    pub event_version: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PersistedPlatformReconciliation {
    pub reconciliation_id: String,
    pub campaign_id: String,
    pub challenge_id: String,
    pub stream_id: String,
    pub snapshot: PlatformReconciliationSnapshot,
    pub snapshot_sha256: String,
    pub updated_at_ms: i64,
    pub event_version: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PlatformReconciliationApplyOutcome {
    pub persisted: PersistedPlatformReconciliation,
    pub result: ReconciliationApplyResult,
}

#[derive(Debug, Clone)]
struct PersistedReconciliationSnapshot {
    campaign_id: String,
    challenge_id: String,
    stream_id: String,
    snapshot: PlatformReconciliationSnapshot,
    snapshot_sha256: String,
    updated_at_ms: i64,
    event_version: Option<u64>,
}

/// A genuine, isolated two-turn runtime probe recorded before any formal research thread is
/// rehydrated. A passed canary authorizes only rehydration, never submission or campaign state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PersistedRecoveryCanary {
    pub recovery_id: String,
    pub runtime_instance_id: String,
    pub recovery_attempt: u64,
    pub trace: RecoveryCanaryTrace,
    pub trace_sha256: String,
    pub recorded_at_ms: i64,
}

/// Persistent binding between a live Core thread and one Chief-issued research cycle.
///
/// A binding is the authoritative cycle/role selector for Core spawn and resume admission;
/// process environment values are never used to create or select one. Revoked rows are retained
/// for audit and to prevent a historical binding from being silently reused.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ThreadCycleBinding {
    pub binding_id: String,
    pub thread_id: String,
    pub parent_thread_id: Option<String>,
    pub agent_id: String,
    pub session_id: String,
    pub campaign_id: String,
    pub challenge_id: String,
    pub cycle_id: String,
    pub cycle_event_version: u64,
    pub chief_lease_id: String,
    pub role: Role,
    pub issued_at_ms: i64,
    pub revoked_at_ms: Option<i64>,
}

#[derive(Debug, Error)]
pub enum BrokerError {
    #[error("submission admission blocked")]
    Admission(Admission),
    #[error(transparent)]
    Ledger(#[from] LedgerError),
}

#[derive(Debug, Clone)]
pub struct Ledger {
    pool: SqlitePool,
}

impl Ledger {
    pub async fn close(self) {
        self.pool.close().await;
    }

    pub async fn connect(database_url: &str) -> Result<Self, LedgerError> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect(database_url)
            .await?;
        Self::initialize(pool).await
    }

    pub async fn connect_file(path: &Path) -> Result<Self, LedgerError> {
        if !path.is_absolute() {
            return Err(LedgerError::Degraded(
                "ledger path must be absolute".to_string(),
            ));
        }
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;
        Self::initialize(pool).await
    }

    /// Opens an existing administrator-provisioned ledger without creating a new empty database.
    /// Worker admission uses this path so a typo or missing mount fails closed.
    pub async fn open_file(path: &Path) -> Result<Self, LedgerError> {
        if !path.is_absolute() || !path.is_file() {
            return Err(LedgerError::Degraded(
                "existing ledger path must be an absolute regular file".to_string(),
            ));
        }
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(false);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;
        Self::initialize(pool).await
    }

    async fn initialize(pool: SqlitePool) -> Result<Self, LedgerError> {
        sqlx::query("PRAGMA busy_timeout = 5000")
            .execute(&pool)
            .await?;
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS reservations (id TEXT PRIMARY KEY, challenge_id TEXT NOT NULL, owner TEXT NOT NULL, estimated_cost REAL NOT NULL, state TEXT NOT NULL)",
        )
        .execute(&pool)
        .await?;
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS attempts (id TEXT PRIMARY KEY, challenge_id TEXT NOT NULL, owner TEXT NOT NULL, content_sha256 TEXT NOT NULL, started_at_ms INTEGER NOT NULL, state TEXT NOT NULL, result_json TEXT)",
        )
        .execute(&pool)
        .await?;
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS attempts_cadence_idx ON attempts (challenge_id, owner, started_at_ms)",
        )
        .execute(&pool)
        .await?;
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS coordination_events (event_id TEXT PRIMARY KEY, idempotency_key TEXT NOT NULL UNIQUE, aggregate_type TEXT NOT NULL, aggregate_id TEXT NOT NULL, state_version INTEGER NOT NULL, event_type TEXT NOT NULL, payload_json TEXT NOT NULL, occurred_at_ms INTEGER NOT NULL, UNIQUE (aggregate_type, aggregate_id, state_version))",
        )
        .execute(&pool)
        .await?;
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS actor_leases (lease_id TEXT PRIMARY KEY, agent_id TEXT NOT NULL, session_id TEXT NOT NULL, thread_id TEXT NOT NULL, campaign_id TEXT NOT NULL, challenge_id TEXT NOT NULL, role TEXT NOT NULL, context_json TEXT NOT NULL, registered_at_ms INTEGER NOT NULL, revoked_at_ms INTEGER, UNIQUE (agent_id, session_id, thread_id, campaign_id, challenge_id))",
        )
        .execute(&pool)
        .await?;
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS research_cycle_issuances (cycle_id TEXT PRIMARY KEY, campaign_id TEXT NOT NULL, challenge_id TEXT NOT NULL, chief_lease_id TEXT NOT NULL, cycle_json TEXT NOT NULL, cycle_sha256 TEXT NOT NULL, cycle_event_version INTEGER NOT NULL, issued_at_ms INTEGER NOT NULL, revoked_at_ms INTEGER)",
        )
        .execute(&pool)
        .await?;
        sqlx::query(
            "CREATE UNIQUE INDEX IF NOT EXISTS active_research_cycle_idx ON research_cycle_issuances (campaign_id, challenge_id) WHERE revoked_at_ms IS NULL",
        )
        .execute(&pool)
        .await?;
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS stage_brief_issuances (brief_id TEXT PRIMARY KEY, cycle_id TEXT NOT NULL, campaign_id TEXT NOT NULL, challenge_id TEXT NOT NULL, role TEXT NOT NULL, brief_json TEXT NOT NULL, brief_sha256 TEXT NOT NULL, workspace_root TEXT NOT NULL, capability_map_path TEXT NOT NULL, issued_at_ms INTEGER NOT NULL, expires_at_ms INTEGER NOT NULL, revoked_at_ms INTEGER, UNIQUE (cycle_id, role))",
        )
        .execute(&pool)
        .await?;
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS thread_cycle_bindings (binding_id TEXT PRIMARY KEY, thread_id TEXT NOT NULL, parent_thread_id TEXT, agent_id TEXT NOT NULL, session_id TEXT NOT NULL, campaign_id TEXT NOT NULL, challenge_id TEXT NOT NULL, cycle_id TEXT NOT NULL, cycle_event_version INTEGER NOT NULL, chief_lease_id TEXT NOT NULL, role TEXT NOT NULL, issued_at_ms INTEGER NOT NULL, revoked_at_ms INTEGER)",
        )
        .execute(&pool)
        .await?;
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS platform_observations (observation_id TEXT PRIMARY KEY, event_id TEXT NOT NULL UNIQUE, campaign_id TEXT NOT NULL, challenge_id TEXT NOT NULL, attempt_id TEXT NOT NULL, monitor_lease_id TEXT NOT NULL, observation_json TEXT NOT NULL, observation_sha256 TEXT NOT NULL, response_sha256 TEXT NOT NULL, observed_at_ms INTEGER NOT NULL, event_version INTEGER NOT NULL, UNIQUE (attempt_id, response_sha256))",
        )
        .execute(&pool)
        .await?;
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS recovery_canaries (recovery_id TEXT PRIMARY KEY, event_id TEXT NOT NULL UNIQUE, runtime_instance_id TEXT NOT NULL, recovery_attempt INTEGER NOT NULL, trace_json TEXT NOT NULL, trace_sha256 TEXT NOT NULL, recorded_at_ms INTEGER NOT NULL, UNIQUE (runtime_instance_id, recovery_attempt))",
        )
        .execute(&pool)
        .await?;
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS platform_observation_attempt_idx ON platform_observations (challenge_id, attempt_id, observed_at_ms)",
        )
        .execute(&pool)
        .await?;
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS reconciliation_snapshots (campaign_id TEXT NOT NULL, challenge_id TEXT NOT NULL, stream_id TEXT NOT NULL, snapshot_json TEXT NOT NULL, snapshot_sha256 TEXT NOT NULL, cursor_position INTEGER, updated_at_ms INTEGER NOT NULL, event_version INTEGER, PRIMARY KEY (stream_id, challenge_id))",
        )
        .execute(&pool)
        .await?;
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS reconciliation_items (item_id TEXT PRIMARY KEY, event_id TEXT NOT NULL UNIQUE, campaign_id TEXT NOT NULL, challenge_id TEXT NOT NULL, attempt_id TEXT NOT NULL, stream_id TEXT NOT NULL, item_json TEXT NOT NULL, item_sha256 TEXT NOT NULL, response_sha256 TEXT NOT NULL, cursor_position INTEGER NOT NULL, observed_at_ms INTEGER NOT NULL, item_state TEXT NOT NULL, event_version INTEGER NOT NULL, UNIQUE (attempt_id, response_sha256))",
        )
        .execute(&pool)
        .await?;
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS reconciliation_item_attempt_idx ON reconciliation_items (challenge_id, attempt_id, cursor_position)",
        )
        .execute(&pool)
        .await?;
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS reconciliation_penalties (penalty_id TEXT PRIMARY KEY, campaign_id TEXT NOT NULL, challenge_id TEXT NOT NULL, attempt_id TEXT NOT NULL, stream_id TEXT NOT NULL, response_sha256 TEXT NOT NULL, penalty_object TEXT NOT NULL, penalty_reason TEXT NOT NULL, raw_score REAL, effective_score REAL, rewritten_score REAL NOT NULL, recorded_at_ms INTEGER NOT NULL, event_version INTEGER NOT NULL, UNIQUE (stream_id, attempt_id, response_sha256))",
        )
        .execute(&pool)
        .await?;
        sqlx::query(
            "CREATE UNIQUE INDEX IF NOT EXISTS thread_cycle_active_idx ON thread_cycle_bindings (thread_id) WHERE revoked_at_ms IS NULL",
        )
        .execute(&pool)
        .await?;
        Ok(Self { pool })
    }

    /// Persist a completed process canary. This API is intentionally unavailable as a model
    /// tool or app-server RPC: the runtime runner must derive the evidence from actual Core
    /// lifecycle events and terminal model messages. Replays are accepted only byte-for-byte.
    pub async fn record_recovery_canary(
        &self,
        trace: &RecoveryCanaryTrace,
        now_ms: i64,
        event: &CoordinationEventRecord<'_>,
    ) -> Result<PersistedRecoveryCanary, LedgerError> {
        if !trace.rehydration_allowed(now_ms) {
            return Err(LedgerError::Degraded(
                "recovery canary has not completed the isolated two-turn probe".into(),
            ));
        }
        let trace_json = serde_json::to_string(trace).map_err(|error| {
            LedgerError::Degraded(format!("cannot serialize recovery canary: {error}"))
        })?;
        let trace_sha256 = sha256_bytes(trace_json.as_bytes());
        let recovery_attempt = i64::try_from(trace.recovery_attempt)
            .map_err(|_| LedgerError::Degraded("recovery attempt overflow".into()))?;
        if event.aggregate_type != "recovery"
            || event.aggregate_id != trace.recovery_id
            || event.event_type != "recovery_canary_passed"
            || event.payload_json != trace_json
            || event.occurred_at_ms != now_ms
        {
            return Err(LedgerError::Degraded(
                "recovery canary event is not bound to the typed trace".into(),
            ));
        }
        let mut transaction = self.pool.begin().await?;
        let appended = append_event_in_transaction(&mut transaction, event).await?;
        if appended.replayed {
            let row = sqlx::query(
                "SELECT recovery_id, event_id, runtime_instance_id, recovery_attempt, trace_json, trace_sha256, recorded_at_ms FROM recovery_canaries WHERE recovery_id = ?",
            )
            .bind(&trace.recovery_id)
            .fetch_optional(&mut *transaction)
            .await?
            .ok_or_else(|| LedgerError::Degraded("replayed recovery canary is missing".into()))?;
            let existing = persisted_recovery_canary_from_row(&row, now_ms).await?;
            if existing.trace != *trace
                || existing.recorded_at_ms != now_ms
                || row.try_get::<String, _>("event_id")? != event.event_id
            {
                return Err(LedgerError::Degraded(
                    "replayed canary conflicts with persisted evidence".into(),
                ));
            }
            transaction.commit().await?;
            return Ok(existing);
        }
        sqlx::query(
            "INSERT INTO recovery_canaries (recovery_id, event_id, runtime_instance_id, recovery_attempt, trace_json, trace_sha256, recorded_at_ms) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&trace.recovery_id)
        .bind(event.event_id)
        .bind(&trace.runtime_instance_id)
        .bind(recovery_attempt)
        .bind(&trace_json)
        .bind(&trace_sha256)
        .bind(now_ms)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(PersistedRecoveryCanary {
            recovery_id: trace.recovery_id.clone(),
            runtime_instance_id: trace.runtime_instance_id.clone(),
            recovery_attempt: trace.recovery_attempt,
            trace: trace.clone(),
            trace_sha256,
            recorded_at_ms: now_ms,
        })
    }

    /// Internal row loader used by replay and runtime lookup. Hashes and lifecycle evidence are
    /// checked before a row can be returned to Core.
    async fn load_recovery_canary_row(
        &self,
        recovery_id: &str,
        runtime_instance_id: &str,
        now_ms: i64,
    ) -> Result<PersistedRecoveryCanary, LedgerError> {
        let row = sqlx::query(
            "SELECT recovery_id, event_id, runtime_instance_id, recovery_attempt, trace_json, trace_sha256, recorded_at_ms FROM recovery_canaries WHERE recovery_id = ? AND runtime_instance_id = ?",
        )
        .bind(recovery_id)
        .bind(runtime_instance_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| LedgerError::Degraded("recovery canary is not registered".into()))?;
        persisted_recovery_canary_from_row(&row, now_ms).await
    }

    /// Load and re-hash one passed canary for this exact process instance. A canary from a prior
    /// process cannot be replayed to authorize the current runtime.
    pub async fn load_recovery_canary(
        &self,
        recovery_id: &str,
        runtime_instance_id: &str,
        now_ms: i64,
    ) -> Result<PersistedRecoveryCanary, LedgerError> {
        if recovery_id.trim().is_empty() || runtime_instance_id.trim().is_empty() {
            return Err(LedgerError::Degraded(
                "recovery canary lookup requires recovery and runtime instance ids".into(),
            ));
        }
        self.load_recovery_canary_row(recovery_id, runtime_instance_id, now_ms)
            .await
    }

    /// Atomically persists one complete read-only platform observation and its chief-first
    /// campaign event. This method performs no network or platform write; callers must supply a
    /// response-derived observation through a registered Monitor lease.
    pub async fn record_platform_observation_audited(
        &self,
        monitor: &ActorContext,
        observation: &PlatformObservation,
        now_ms: i64,
        event: &CoordinationEventRecord<'_>,
    ) -> Result<PersistedPlatformObservation, LedgerError> {
        if monitor.role != Role::Monitor
            || monitor.campaign_id.trim().is_empty()
            || monitor.challenge_id.trim().is_empty()
        {
            return Err(LedgerError::Degraded(
                "platform observation requires a bound Monitor context".into(),
            ));
        }
        monitor
            .validate(Action::MonitorReadOnly, now_ms)
            .map_err(|error| {
                LedgerError::Degraded(format!("monitor lease validation failed: {error}"))
            })?;
        observation
            .validate(&monitor.challenge_id, now_ms)
            .map_err(|error| {
                LedgerError::Degraded(format!("invalid platform observation: {error}"))
            })?;
        let observation_json = serde_json::to_string(observation).map_err(|error| {
            LedgerError::Degraded(format!("cannot serialize platform observation: {error}"))
        })?;
        let observation_sha256 = sha256_bytes(observation_json.as_bytes());
        let observation_id = format!(
            "observation:{}:{}",
            observation.attempt_id, observation.response_sha256
        );
        if event.aggregate_type != "campaign"
            || event.aggregate_id != monitor.campaign_id
            || event.event_type != "platform_observation_recorded"
            || event.payload_json != observation_json
            || event.occurred_at_ms != now_ms
        {
            return Err(LedgerError::Degraded(
                "platform observation event is not bound to the typed observation".into(),
            ));
        }

        let mut transaction = self.pool.begin().await?;
        let lease_row = sqlx::query(
            "SELECT role, context_json, revoked_at_ms FROM actor_leases WHERE lease_id = ?",
        )
        .bind(&monitor.lease.lease_id)
        .fetch_optional(&mut *transaction)
        .await?
        .ok_or_else(|| LedgerError::Degraded("monitor lease is not registered".into()))?;
        if lease_row
            .try_get::<Option<i64>, _>("revoked_at_ms")?
            .is_some()
            || lease_row.try_get::<String, _>("role")? != role_name(Role::Monitor)
        {
            return Err(LedgerError::Degraded(
                "monitor lease is revoked or belongs to another role".into(),
            ));
        }
        let stored_context: ActorContext = serde_json::from_str(
            &lease_row.try_get::<String, _>("context_json")?,
        )
        .map_err(|error| {
            LedgerError::Degraded(format!("stored monitor context is invalid: {error}"))
        })?;
        if stored_context != *monitor {
            return Err(LedgerError::Degraded(
                "monitor context does not match the registered lease".into(),
            ));
        }

        let appended = append_event_in_transaction(&mut transaction, event).await?;
        if appended.replayed {
            let persisted = load_platform_observation_row(
                &mut transaction,
                event.event_id,
                &observation_id,
                &observation_json,
                &observation_sha256,
                &monitor.lease.lease_id,
                appended.version,
            )
            .await?;
            transaction.commit().await?;
            return Ok(persisted);
        }
        sqlx::query(
            "INSERT INTO platform_observations (observation_id, event_id, campaign_id, challenge_id, attempt_id, monitor_lease_id, observation_json, observation_sha256, response_sha256, observed_at_ms, event_version) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&observation_id)
        .bind(event.event_id)
        .bind(&monitor.campaign_id)
        .bind(&monitor.challenge_id)
        .bind(&observation.attempt_id)
        .bind(&monitor.lease.lease_id)
        .bind(&observation_json)
        .bind(&observation_sha256)
        .bind(&observation.response_sha256)
        .bind(observation.observed_at_ms)
        .bind(i64::try_from(appended.version).map_err(|_| {
            LedgerError::Degraded("platform observation event version overflow".into())
        })?)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(PersistedPlatformObservation {
            observation_id,
            campaign_id: monitor.campaign_id.clone(),
            challenge_id: monitor.challenge_id.clone(),
            attempt_id: observation.attempt_id.clone(),
            monitor_lease_id: monitor.lease.lease_id.clone(),
            observation: observation.clone(),
            observation_sha256,
            event_version: appended.version,
        })
    }

    /// Loads and re-hashes the latest persisted observation for one bound attempt.
    pub async fn load_latest_platform_observation(
        &self,
        challenge_id: &str,
        attempt_id: &str,
    ) -> Result<PersistedPlatformObservation, LedgerError> {
        if challenge_id.trim().is_empty() || attempt_id.trim().is_empty() {
            return Err(LedgerError::Degraded(
                "platform observation lookup requires challenge and attempt ids".into(),
            ));
        }
        let row = sqlx::query(
            "SELECT observation_id, campaign_id, challenge_id, attempt_id, monitor_lease_id, observation_json, observation_sha256, event_version FROM platform_observations WHERE challenge_id = ? AND attempt_id = ? ORDER BY observed_at_ms DESC, event_version DESC LIMIT 1",
        )
        .bind(challenge_id)
        .bind(attempt_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| LedgerError::Degraded("platform observation is not registered".into()))?;
        persisted_platform_observation_from_row(&row)
    }

    /// Apply one typed platform reconciliation item through the reducer and persist the result
    /// snapshot, the immutable item row, and any penalty audit row in a single event-versioned
    /// transaction. Duplicate/stale replay is a no-op; conflicts fail closed.
    pub async fn apply_platform_reconciliation_audited(
        &self,
        monitor: &ActorContext,
        item: &PlatformReconcileItem,
        now_ms: i64,
        event: &CoordinationEventRecord<'_>,
    ) -> Result<PlatformReconciliationApplyOutcome, LedgerError> {
        if monitor.role != Role::Monitor
            || monitor.campaign_id.trim().is_empty()
            || monitor.challenge_id.trim().is_empty()
        {
            return Err(LedgerError::Degraded(
                "platform reconciliation requires a bound Monitor context".into(),
            ));
        }
        if item.challenge_id != monitor.challenge_id {
            return Err(LedgerError::Degraded(
                "platform reconciliation item is outside the monitor's challenge binding".into(),
            ));
        }
        monitor
            .validate(Action::MonitorReadOnly, now_ms)
            .map_err(|error| {
                LedgerError::Degraded(format!("monitor lease validation failed: {error}"))
            })?;
        let item_json = serde_json::to_string(item).map_err(|error| {
            LedgerError::Degraded(format!("cannot serialize reconciliation item: {error}"))
        })?;
        let item_sha256 = sha256_bytes(item_json.as_bytes());
        let item_id = format!("reconcile:{}:{}", item.attempt_id, item.response_sha256);
        let stream_id = item.cursor.stream_id.clone();
        if stream_id.trim().is_empty()
            || item.challenge_id.trim().is_empty()
            || item.attempt_id.trim().is_empty()
        {
            return Err(LedgerError::Degraded(
                "reconciliation item is missing stream, challenge, or attempt identity".into(),
            ));
        }
        if event.aggregate_type != "campaign"
            || event.aggregate_id != monitor.campaign_id
            || event.event_type != "platform_reconciliation_recorded"
            || event.payload_json != item_json
            || event.occurred_at_ms != now_ms
        {
            return Err(LedgerError::Degraded(
                "reconciliation event is not bound to the typed item".into(),
            ));
        }

        let persisted_snapshot =
            load_reconciliation_snapshot(&self.pool, &stream_id, &item.challenge_id).await?;
        if let Some(persisted) = &persisted_snapshot
            && persisted.campaign_id != monitor.campaign_id
        {
            return Err(LedgerError::Degraded(
                "reconciliation snapshot belongs to a different campaign".into(),
            ));
        }
        let mut snapshot = match &persisted_snapshot {
            Some(persisted) => persisted.snapshot.clone(),
            None => PlatformReconciliationSnapshot::new(&stream_id, &item.challenge_id).map_err(
                |error| {
                    LedgerError::Degraded(format!("cannot create reconciliation snapshot: {error}"))
                },
            )?,
        };
        let result = snapshot.apply(item.clone(), now_ms).map_err(|error| {
            LedgerError::Degraded(format!("reconciliation apply failed: {error}"))
        })?;

        match result {
            ReconciliationApplyResult::Applied => {
                let snapshot_json = serde_json::to_string(&snapshot).map_err(|error| {
                    LedgerError::Degraded(format!(
                        "cannot serialize reconciliation snapshot: {error}"
                    ))
                })?;
                let snapshot_sha256 = sha256_bytes(snapshot_json.as_bytes());
                let mut transaction = self.pool.begin().await?;
                let appended = append_event_in_transaction(&mut transaction, event).await?;
                if appended.replayed {
                    return Err(LedgerError::Degraded(
                        "reconciliation event idempotency is already bound to another item".into(),
                    ));
                }
                let event_version = i64::try_from(appended.version).map_err(|_| {
                    LedgerError::Degraded("reconciliation event version overflow".into())
                })?;
                sqlx::query(
                    "INSERT INTO reconciliation_items (item_id, event_id, campaign_id, challenge_id, attempt_id, stream_id, item_json, item_sha256, response_sha256, cursor_position, observed_at_ms, item_state, event_version) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                )
                .bind(&item_id)
                .bind(event.event_id)
                .bind(&monitor.campaign_id)
                .bind(&item.challenge_id)
                .bind(&item.attempt_id)
                .bind(&stream_id)
                .bind(&item_json)
                .bind(&item_sha256)
                .bind(&item.response_sha256)
                .bind(i64::try_from(item.cursor.position).map_err(|_| {
                    LedgerError::Degraded("reconciliation cursor position overflow".into())
                })?)
                .bind(item.observed_at_ms)
                .bind(reconciliation_item_state_name(item))
                .bind(event_version)
                .execute(&mut *transaction)
                .await?;
                sqlx::query(
                    "INSERT INTO reconciliation_snapshots (campaign_id, challenge_id, stream_id, snapshot_json, snapshot_sha256, cursor_position, updated_at_ms, event_version) VALUES (?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT (stream_id, challenge_id) DO UPDATE SET snapshot_json = excluded.snapshot_json, snapshot_sha256 = excluded.snapshot_sha256, cursor_position = excluded.cursor_position, updated_at_ms = excluded.updated_at_ms, event_version = excluded.event_version",
                )
                .bind(&monitor.campaign_id)
                .bind(&item.challenge_id)
                .bind(&stream_id)
                .bind(&snapshot_json)
                .bind(&snapshot_sha256)
                .bind(snapshot.cursor.as_ref().map(|cursor| cursor.position).map(i64::try_from).transpose().map_err(|_| {
                    LedgerError::Degraded("reconciliation snapshot cursor overflow".into())
                })?)
                .bind(now_ms)
                .bind(event_version)
                .execute(&mut *transaction)
                .await?;
                if item.facts.penalty_applied {
                    let basis = item.facts.penalty_basis.as_ref().ok_or_else(|| {
                        LedgerError::Degraded(
                            "applied reconciliation penalty is missing its basis".into(),
                        )
                    })?;
                    sqlx::query(
                        "INSERT INTO reconciliation_penalties (penalty_id, campaign_id, challenge_id, attempt_id, stream_id, response_sha256, penalty_object, penalty_reason, raw_score, effective_score, rewritten_score, recorded_at_ms, event_version) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                    )
                    .bind(&format!(
                        "reconcile-penalty:{}:{}:{}",
                        stream_id, item.attempt_id, item.response_sha256
                    ))
                    .bind(&monitor.campaign_id)
                    .bind(&item.challenge_id)
                    .bind(&item.attempt_id)
                    .bind(&stream_id)
                    .bind(&item.response_sha256)
                    .bind(&basis.object)
                    .bind(&basis.reason)
                    .bind(item.facts.raw_score)
                    .bind(item.facts.effective_score)
                    .bind(basis.rewritten_score)
                    .bind(now_ms)
                    .bind(event_version)
                    .execute(&mut *transaction)
                    .await?;
                }
                transaction.commit().await?;
                Ok(PlatformReconciliationApplyOutcome {
                    persisted: PersistedPlatformReconciliation {
                        reconciliation_id: item_id,
                        campaign_id: monitor.campaign_id.clone(),
                        challenge_id: item.challenge_id.clone(),
                        stream_id,
                        snapshot,
                        snapshot_sha256,
                        updated_at_ms: now_ms,
                        event_version: Some(appended.version),
                    },
                    result,
                })
            }
            ReconciliationApplyResult::Duplicate => {
                let row = sqlx::query(
                    "SELECT event_id, campaign_id, challenge_id, attempt_id, stream_id, item_json, item_sha256, response_sha256, cursor_position, observed_at_ms, item_state, event_version FROM reconciliation_items WHERE event_id = ?",
                )
                .bind(event.event_id)
                .fetch_optional(&self.pool)
                .await?
                .ok_or_else(|| {
                    LedgerError::Degraded("replayed reconciliation item is missing".into())
                })?;
                let stored_item_json: String = row.try_get("item_json")?;
                let stored_item_sha256: String = row.try_get("item_sha256")?;
                let stored_cursor: i64 = row.try_get("cursor_position")?;
                if row.try_get::<String, _>("event_id")? != event.event_id
                    || row.try_get::<String, _>("campaign_id")? != monitor.campaign_id
                    || row.try_get::<String, _>("challenge_id")? != item.challenge_id
                    || row.try_get::<String, _>("attempt_id")? != item.attempt_id
                    || row.try_get::<String, _>("stream_id")? != stream_id
                    || stored_item_json != item_json
                    || stored_item_sha256 != item_sha256
                    || row.try_get::<String, _>("response_sha256")? != item.response_sha256
                    || u64::try_from(stored_cursor).map_err(|_| {
                        LedgerError::Degraded("stored reconciliation cursor is invalid".into())
                    })? != item.cursor.position
                    || row.try_get::<i64, _>("observed_at_ms")? != item.observed_at_ms
                    || row.try_get::<String, _>("item_state")?
                        != reconciliation_item_state_name(item)
                {
                    return Err(LedgerError::Degraded(
                        "replayed reconciliation item does not match persisted evidence".into(),
                    ));
                }
                let persisted = persisted_snapshot.ok_or_else(|| {
                    LedgerError::Degraded(
                        "duplicate reconciliation replay is missing its snapshot row".into(),
                    )
                })?;
                Ok(PlatformReconciliationApplyOutcome {
                    persisted: PersistedPlatformReconciliation {
                        reconciliation_id: item_id,
                        campaign_id: monitor.campaign_id.clone(),
                        challenge_id: item.challenge_id.clone(),
                        stream_id,
                        snapshot: persisted.snapshot,
                        snapshot_sha256: persisted.snapshot_sha256,
                        updated_at_ms: persisted.updated_at_ms,
                        event_version: persisted.event_version,
                    },
                    result,
                })
            }
            ReconciliationApplyResult::Stale => {
                let persisted = persisted_snapshot.ok_or_else(|| {
                    LedgerError::Degraded("stale reconciliation item has no snapshot row".into())
                })?;
                Ok(PlatformReconciliationApplyOutcome {
                    persisted: PersistedPlatformReconciliation {
                        reconciliation_id: item_id,
                        campaign_id: monitor.campaign_id.clone(),
                        challenge_id: item.challenge_id.clone(),
                        stream_id,
                        snapshot: persisted.snapshot,
                        snapshot_sha256: persisted.snapshot_sha256,
                        updated_at_ms: persisted.updated_at_ms,
                        event_version: persisted.event_version,
                    },
                    result,
                })
            }
        }
    }

    /// Load the latest persisted reconciliation snapshot for a stream/challenge pair, verifying
    /// the serialized JSON hash before returning it.
    pub async fn load_latest_platform_reconciliation(
        &self,
        stream_id: &str,
        challenge_id: &str,
    ) -> Result<PersistedPlatformReconciliation, LedgerError> {
        if stream_id.trim().is_empty() || challenge_id.trim().is_empty() {
            return Err(LedgerError::Degraded(
                "reconciliation lookup requires stream and challenge ids".into(),
            ));
        }
        let persisted = load_reconciliation_snapshot(&self.pool, stream_id, challenge_id)
            .await?
            .ok_or_else(|| {
                LedgerError::Degraded("reconciliation snapshot is not registered".into())
            })?;
        Ok(PersistedPlatformReconciliation {
            reconciliation_id: format!("reconcile-stream:{stream_id}:{challenge_id}"),
            campaign_id: persisted.campaign_id,
            challenge_id: persisted.challenge_id,
            stream_id: persisted.stream_id,
            snapshot: persisted.snapshot,
            snapshot_sha256: persisted.snapshot_sha256,
            updated_at_ms: persisted.updated_at_ms,
            event_version: persisted.event_version,
        })
    }

    /// Bind the Chief/root thread to one active research cycle. This is an administrative
    /// operation: the Chief lease and cycle issuance are both resolved from the ledger, and the
    /// resulting row is the durable selector used by worker admission.
    pub async fn bind_root_thread_to_cycle(
        &self,
        chief: &ActorContext,
        cycle_id: &str,
        cycle_event_version: u64,
        binding_id: &str,
        now_ms: i64,
    ) -> Result<ThreadCycleBinding, LedgerError> {
        if chief.role != Role::Chief
            || chief.thread_id.trim().is_empty()
            || chief.agent_id.trim().is_empty()
            || chief.session_id.trim().is_empty()
            || cycle_id.trim().is_empty()
            || binding_id.trim().is_empty()
            || cycle_event_version == 0
        {
            return Err(LedgerError::Degraded(
                "root cycle binding requires a complete Chief identity and cycle".into(),
            ));
        }
        let resolved = self
            .resolve_chief_context(
                &chief.lease.lease_id,
                &chief.agent_id,
                &chief.session_id,
                &chief.thread_id,
                &chief.campaign_id,
                &chief.challenge_id,
                now_ms,
            )
            .await?;
        if resolved != *chief {
            return Err(LedgerError::Degraded(
                "root cycle binding Chief context is not registered".into(),
            ));
        }
        let cycle = self
            .load_active_cycle_for_binding(
                cycle_id,
                &chief.campaign_id,
                &chief.challenge_id,
                &chief.lease.lease_id,
                cycle_event_version,
                now_ms,
            )
            .await?;
        let binding = ThreadCycleBinding {
            binding_id: binding_id.to_string(),
            thread_id: chief.thread_id.clone(),
            parent_thread_id: None,
            agent_id: chief.agent_id.clone(),
            session_id: chief.session_id.clone(),
            campaign_id: chief.campaign_id.clone(),
            challenge_id: chief.challenge_id.clone(),
            cycle_id: cycle.cycle_id,
            cycle_event_version,
            chief_lease_id: chief.lease.lease_id.clone(),
            role: Role::Chief,
            issued_at_ms: now_ms,
            revoked_at_ms: None,
        };
        self.insert_thread_cycle_binding(&binding).await?;
        Ok(binding)
    }

    /// Bind a freshly-created direct child to its parent's active cycle. The parent must already
    /// be durably bound and the requested role must have a Chief-issued StageBrief in that cycle.
    pub async fn bind_child_thread_to_cycle(
        &self,
        parent_thread_id: &str,
        child_thread_id: &str,
        child_agent_id: &str,
        child_session_id: &str,
        role: Role,
        binding_id: &str,
        now_ms: i64,
    ) -> Result<ThreadCycleBinding, LedgerError> {
        if parent_thread_id.trim().is_empty()
            || child_thread_id.trim().is_empty()
            || child_agent_id.trim().is_empty()
            || child_session_id.trim().is_empty()
            || binding_id.trim().is_empty()
            || role == Role::Chief
        {
            return Err(LedgerError::Degraded(
                "child cycle binding requires complete identities and a worker role".into(),
            ));
        }
        if child_agent_id != child_thread_id {
            return Err(LedgerError::Degraded(
                "child cycle binding agent identity must equal its thread identity".into(),
            ));
        }
        let parent_row = sqlx::query(
            "SELECT thread_id, agent_id, session_id, campaign_id, challenge_id, cycle_id, cycle_event_version, chief_lease_id, issued_at_ms, revoked_at_ms FROM thread_cycle_bindings WHERE thread_id = ? AND role = ? AND revoked_at_ms IS NULL",
        )
        .bind(parent_thread_id)
        .bind(role_name(Role::Chief))
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| LedgerError::Degraded("parent thread has no active Chief cycle binding".into()))?;
        let parent_cycle_id: String = parent_row.try_get("cycle_id")?;
        let parent_campaign_id: String = parent_row.try_get("campaign_id")?;
        let parent_challenge_id: String = parent_row.try_get("challenge_id")?;
        let parent_version = u64::try_from(parent_row.try_get::<i64, _>("cycle_event_version")?)
            .map_err(|_| LedgerError::Degraded("stored cycle event version is invalid".into()))?;
        let parent_lease_id: String = parent_row.try_get("chief_lease_id")?;
        if parent_row.try_get::<String, _>("session_id")? != child_session_id {
            return Err(LedgerError::Degraded(
                "child cycle binding session does not match the Chief parent session".into(),
            ));
        }
        let parent = self
            .load_active_cycle_for_binding(
                &parent_cycle_id,
                &parent_campaign_id,
                &parent_challenge_id,
                &parent_lease_id,
                parent_version,
                now_ms,
            )
            .await?;
        let _brief = self
            .load_stage_brief_issuance(
                &StageBriefLedgerTarget {
                    cycle_id: &parent.cycle_id,
                    campaign_id: &parent_campaign_id,
                    challenge_id: &parent_challenge_id,
                    role,
                },
                now_ms,
            )
            .await?;
        let binding = ThreadCycleBinding {
            binding_id: binding_id.to_string(),
            thread_id: child_thread_id.to_string(),
            parent_thread_id: Some(parent_thread_id.to_string()),
            agent_id: child_agent_id.to_string(),
            session_id: child_session_id.to_string(),
            campaign_id: parent_campaign_id,
            challenge_id: parent_challenge_id,
            cycle_id: parent.cycle_id,
            cycle_event_version: parent_version,
            chief_lease_id: parent_lease_id,
            role,
            issued_at_ms: now_ms,
            revoked_at_ms: None,
        };
        self.insert_thread_cycle_binding(&binding).await?;
        Ok(binding)
    }

    /// Resolve a live thread's durable cycle binding. Parent, session and role are checked against
    /// the stored row; active cycle and role brief checks make revoke/supersede immediately deny.
    pub async fn resolve_thread_cycle_binding(
        &self,
        thread_id: &str,
        agent_id: &str,
        session_id: &str,
        parent_thread_id: Option<&str>,
        role: Role,
        now_ms: i64,
    ) -> Result<ThreadCycleBinding, LedgerError> {
        if [thread_id, agent_id, session_id]
            .iter()
            .any(|value| value.trim().is_empty())
        {
            return Err(LedgerError::Degraded(
                "thread cycle lookup requires live identity".into(),
            ));
        }
        let row = sqlx::query(
            "SELECT binding_id, thread_id, parent_thread_id, agent_id, session_id, campaign_id, challenge_id, cycle_id, cycle_event_version, chief_lease_id, role, issued_at_ms, revoked_at_ms FROM thread_cycle_bindings WHERE thread_id = ? AND revoked_at_ms IS NULL",
        )
        .bind(thread_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| LedgerError::Degraded("thread has no active cycle binding".into()))?;
        let stored_parent: Option<String> = row.try_get("parent_thread_id")?;
        if row.try_get::<String, _>("agent_id")? != agent_id
            || row.try_get::<String, _>("session_id")? != session_id
            || parse_role_name(&row.try_get::<String, _>("role")?)? != role
            || stored_parent.as_deref() != parent_thread_id
        {
            return Err(LedgerError::Degraded(
                "thread cycle binding does not match live identity, parent, or role".into(),
            ));
        }
        let cycle_id: String = row.try_get("cycle_id")?;
        let campaign_id: String = row.try_get("campaign_id")?;
        let challenge_id: String = row.try_get("challenge_id")?;
        let cycle_event_version = u64::try_from(row.try_get::<i64, _>("cycle_event_version")?)
            .map_err(|_| LedgerError::Degraded("stored cycle event version is invalid".into()))?;
        let chief_lease_id: String = row.try_get("chief_lease_id")?;
        let _cycle = self
            .load_active_cycle_for_binding(
                &cycle_id,
                &campaign_id,
                &challenge_id,
                &chief_lease_id,
                cycle_event_version,
                now_ms,
            )
            .await?;
        if role != Role::Chief {
            self.load_stage_brief_issuance(
                &StageBriefLedgerTarget {
                    cycle_id: &cycle_id,
                    campaign_id: &campaign_id,
                    challenge_id: &challenge_id,
                    role,
                },
                now_ms,
            )
            .await?;
        }
        Ok(ThreadCycleBinding {
            binding_id: row.try_get("binding_id")?,
            thread_id: row.try_get("thread_id")?,
            parent_thread_id: stored_parent,
            agent_id: row.try_get("agent_id")?,
            session_id: row.try_get("session_id")?,
            campaign_id,
            challenge_id,
            cycle_id,
            cycle_event_version,
            chief_lease_id,
            role,
            issued_at_ms: row.try_get("issued_at_ms")?,
            revoked_at_ms: row.try_get("revoked_at_ms")?,
        })
    }

    /// Resolve a binding using Core's canonical thread/session identity. The AgentControl root
    /// registry uses the thread id as its live agent id, while administrator-issued Chief context
    /// may use a separate operator identity; this entry point deliberately checks both stored
    /// thread and session plus the role without accepting a caller-selected cycle.
    pub async fn resolve_thread_cycle_binding_for_live_thread(
        &self,
        thread_id: &str,
        session_id: &str,
        role: Role,
        now_ms: i64,
    ) -> Result<ThreadCycleBinding, LedgerError> {
        if thread_id.trim().is_empty() || session_id.trim().is_empty() {
            return Err(LedgerError::Degraded(
                "live thread cycle lookup requires thread and session identity".into(),
            ));
        }
        let row = sqlx::query(
            "SELECT agent_id, session_id, parent_thread_id FROM thread_cycle_bindings WHERE thread_id = ? AND role = ? AND revoked_at_ms IS NULL",
        )
        .bind(thread_id)
        .bind(role_name(role))
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| LedgerError::Degraded("thread has no active cycle binding".into()))?;
        if row.try_get::<String, _>("session_id")? != session_id {
            return Err(LedgerError::Degraded(
                "thread cycle binding session does not match the live thread".into(),
            ));
        }
        let parent_thread_id: Option<String> = row.try_get("parent_thread_id")?;
        let agent_id: String = row.try_get("agent_id")?;
        self.resolve_thread_cycle_binding(
            thread_id,
            &agent_id,
            session_id,
            parent_thread_id.as_deref(),
            role,
            now_ms,
        )
        .await
    }

    /// Revoke a thread binding while retaining the row as audit history.
    pub async fn revoke_thread_cycle_binding(
        &self,
        thread_id: &str,
        revoked_at_ms: i64,
    ) -> Result<(), LedgerError> {
        let result = sqlx::query(
            "UPDATE thread_cycle_bindings SET revoked_at_ms = ? WHERE thread_id = ? AND revoked_at_ms IS NULL",
        )
        .bind(revoked_at_ms)
        .bind(thread_id)
        .execute(&self.pool)
        .await?;
        if result.rows_affected() != 1 {
            return Err(LedgerError::Degraded(
                "thread cycle binding is missing or already revoked".into(),
            ));
        }
        Ok(())
    }

    async fn insert_thread_cycle_binding(
        &self,
        binding: &ThreadCycleBinding,
    ) -> Result<(), LedgerError> {
        let result = sqlx::query(
            "INSERT INTO thread_cycle_bindings (binding_id, thread_id, parent_thread_id, agent_id, session_id, campaign_id, challenge_id, cycle_id, cycle_event_version, chief_lease_id, role, issued_at_ms, revoked_at_ms) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)",
        )
        .bind(&binding.binding_id)
        .bind(&binding.thread_id)
        .bind(&binding.parent_thread_id)
        .bind(&binding.agent_id)
        .bind(&binding.session_id)
        .bind(&binding.campaign_id)
        .bind(&binding.challenge_id)
        .bind(&binding.cycle_id)
        .bind(i64::try_from(binding.cycle_event_version).map_err(|_| LedgerError::Degraded("cycle event version overflow".into()))?)
        .bind(&binding.chief_lease_id)
        .bind(role_name(binding.role))
        .bind(binding.issued_at_ms)
        .execute(&self.pool)
        .await?;
        if result.rows_affected() != 1 {
            return Err(LedgerError::Degraded(
                "thread cycle binding was not inserted".into(),
            ));
        }
        Ok(())
    }

    async fn load_active_cycle_for_binding(
        &self,
        cycle_id: &str,
        campaign_id: &str,
        challenge_id: &str,
        chief_lease_id: &str,
        cycle_event_version: u64,
        now_ms: i64,
    ) -> Result<ResearchCycleRecord, LedgerError> {
        let row = sqlx::query(
            "SELECT cycle_json, cycle_sha256, cycle_event_version, chief_lease_id, revoked_at_ms FROM research_cycle_issuances WHERE cycle_id = ? AND campaign_id = ? AND challenge_id = ? AND revoked_at_ms IS NULL",
        )
        .bind(cycle_id)
        .bind(campaign_id)
        .bind(challenge_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| LedgerError::Degraded("active research cycle is not registered".into()))?;
        if row.try_get::<String, _>("chief_lease_id")? != chief_lease_id
            || u64::try_from(row.try_get::<i64, _>("cycle_event_version")?).map_err(|_| {
                LedgerError::Degraded("stored cycle event version is invalid".into())
            })? != cycle_event_version
        {
            return Err(LedgerError::Degraded(
                "thread cycle binding points to a stale or different Chief cycle".into(),
            ));
        }
        let cycle_json: String = row.try_get("cycle_json")?;
        if sha256_bytes(cycle_json.as_bytes()) != row.try_get::<String, _>("cycle_sha256")? {
            return Err(LedgerError::Degraded(
                "active research cycle hash does not match".into(),
            ));
        }
        let cycle: ResearchCycleRecord = serde_json::from_str(&cycle_json).map_err(|error| {
            LedgerError::Degraded(format!("stored research cycle is invalid JSON: {error}"))
        })?;
        cycle.validate(now_ms).map_err(|error| {
            LedgerError::Degraded(format!("active research cycle is invalid: {error}"))
        })?;
        Ok(cycle)
    }

    /// Persist an administrator-issued actor context. The context must already authorize
    /// submission requests; solver sessions have no runtime path to call this method.
    pub async fn provision_actor_context(
        &self,
        context: &ActorContext,
        now_ms: i64,
    ) -> Result<(), LedgerError> {
        validate_persisted_actor_context(context, now_ms)?;
        let context_json = serde_json::to_string(context).map_err(|err| {
            LedgerError::Degraded(format!("cannot serialize actor context: {err}"))
        })?;
        sqlx::query(
            "INSERT INTO actor_leases (lease_id, agent_id, session_id, thread_id, campaign_id, challenge_id, role, context_json, registered_at_ms, revoked_at_ms) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)",
        )
        .bind(&context.lease.lease_id)
        .bind(&context.agent_id)
        .bind(&context.session_id)
        .bind(&context.thread_id)
        .bind(&context.campaign_id)
        .bind(&context.challenge_id)
        .bind(role_name(context.role))
        .bind(context_json)
        .bind(now_ms)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Administrative variant that commits the lease and its audit event atomically.
    pub async fn provision_actor_context_audited(
        &self,
        context: &ActorContext,
        now_ms: i64,
        event: &CoordinationEventRecord<'_>,
    ) -> Result<u64, LedgerError> {
        validate_persisted_actor_context(context, now_ms)?;
        if event.aggregate_type != "actor_lease"
            || event.aggregate_id != context.lease.lease_id
            || event.event_type != "actor_lease_provisioned"
            || event.occurred_at_ms != now_ms
        {
            return Err(LedgerError::Degraded(
                "lease provision event is not bound to the actor context".into(),
            ));
        }
        let context_json = serde_json::to_string(context).map_err(|err| {
            LedgerError::Degraded(format!("cannot serialize actor context: {err}"))
        })?;
        let mut transaction = self.pool.begin().await?;
        let appended = append_event_in_transaction(&mut transaction, event).await?;
        if appended.replayed {
            let row = sqlx::query(
                "SELECT context_json, registered_at_ms, revoked_at_ms FROM actor_leases WHERE lease_id = ?",
            )
            .bind(&context.lease.lease_id)
            .fetch_optional(&mut *transaction)
            .await?
            .ok_or_else(|| LedgerError::Degraded("replayed provision is missing its lease".into()))?;
            if row.try_get::<String, _>("context_json")? != context_json
                || row.try_get::<i64, _>("registered_at_ms")? != now_ms
                || row.try_get::<Option<i64>, _>("revoked_at_ms")?.is_some()
            {
                return Err(LedgerError::Degraded(
                    "replayed provision does not match the active lease".into(),
                ));
            }
            transaction.commit().await?;
            return Ok(appended.version);
        }
        sqlx::query(
            "INSERT INTO actor_leases (lease_id, agent_id, session_id, thread_id, campaign_id, challenge_id, role, context_json, registered_at_ms, revoked_at_ms) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)",
        )
        .bind(&context.lease.lease_id)
        .bind(&context.agent_id)
        .bind(&context.session_id)
        .bind(&context.thread_id)
        .bind(&context.campaign_id)
        .bind(&context.challenge_id)
        .bind(role_name(context.role))
        .bind(context_json)
        .bind(now_ms)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(appended.version)
    }

    /// Resolve an actor exclusively from the persistent registry and bind it to live Core
    /// identities. Caller-supplied role and lease contents are never accepted here.
    pub async fn resolve_actor_context(
        &self,
        lease_id: &str,
        live_agent_id: &str,
        live_session_id: &str,
        live_thread_id: &str,
        campaign_id: &str,
        challenge_id: &str,
        identity_class: &str,
        action: Action,
        now_ms: i64,
    ) -> Result<ActorContext, LedgerError> {
        if [
            lease_id,
            live_agent_id,
            live_session_id,
            live_thread_id,
            campaign_id,
            challenge_id,
            identity_class,
        ]
        .iter()
        .any(|value| value.trim().is_empty())
        {
            return Err(LedgerError::Degraded(
                "actor lookup requires complete live identity and campaign binding".into(),
            ));
        }
        let row = sqlx::query(
            "SELECT agent_id, session_id, thread_id, campaign_id, challenge_id, role, context_json, revoked_at_ms FROM actor_leases WHERE lease_id = ?",
        )
        .bind(lease_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| LedgerError::Degraded("actor lease is not registered".into()))?;
        let revoked_at_ms: Option<i64> = row.try_get("revoked_at_ms")?;
        if revoked_at_ms.is_some() {
            return Err(LedgerError::Degraded("actor lease has been revoked".into()));
        }
        let stored = [
            row.try_get::<String, _>("agent_id")?,
            row.try_get::<String, _>("session_id")?,
            row.try_get::<String, _>("thread_id")?,
            row.try_get::<String, _>("campaign_id")?,
            row.try_get::<String, _>("challenge_id")?,
        ];
        let presented = [
            live_agent_id,
            live_session_id,
            live_thread_id,
            campaign_id,
            challenge_id,
        ];
        if stored
            .iter()
            .map(String::as_str)
            .zip(presented)
            .any(|(stored, presented)| stored != presented)
            || row.try_get::<String, _>("role")? != "solver"
        {
            return Err(LedgerError::Degraded(
                "actor lease does not match the live invocation".into(),
            ));
        }
        let context_json: String = row.try_get("context_json")?;
        let context: ActorContext = serde_json::from_str(&context_json).map_err(|err| {
            LedgerError::Degraded(format!("stored actor context is invalid: {err}"))
        })?;
        if context.lease.lease_id != lease_id
            || context.agent_id != live_agent_id
            || context.session_id != live_session_id
            || context.thread_id != live_thread_id
            || context.campaign_id != campaign_id
            || context.challenge_id != challenge_id
            || context.role != Role::Solver
        {
            return Err(LedgerError::Degraded(
                "stored actor context does not match the registry or live invocation".into(),
            ));
        }
        context.validate(action, now_ms).map_err(|err| {
            LedgerError::Degraded(format!("actor lease validation failed: {err}"))
        })?;
        if !context
            .lease
            .authorized_identity_classes
            .contains(identity_class)
        {
            return Err(LedgerError::Degraded(
                "identity class is not authorized by the actor lease".into(),
            ));
        }
        Ok(context)
    }

    /// Resolves the administrator-provisioned chief context needed to issue a research cycle.
    /// This is intentionally separate from solver submission resolution: a chief can decide but
    /// never receives a submission capability from this API.
    pub async fn resolve_chief_context(
        &self,
        lease_id: &str,
        live_agent_id: &str,
        live_session_id: &str,
        live_thread_id: &str,
        campaign_id: &str,
        challenge_id: &str,
        now_ms: i64,
    ) -> Result<ActorContext, LedgerError> {
        self.resolve_chief_context_for_action(
            lease_id,
            live_agent_id,
            live_session_id,
            live_thread_id,
            campaign_id,
            challenge_id,
            Action::Decide,
            now_ms,
        )
        .await
    }

    /// Resolves the administrator-provisioned Chief authority required for one direct child
    /// dispatch. Keeping this entry point action-specific prevents a valid `Decide` lease from
    /// being reused as `SpawnChild` authority by Core.
    pub async fn resolve_chief_spawn_context(
        &self,
        lease_id: &str,
        live_agent_id: &str,
        live_session_id: &str,
        live_thread_id: &str,
        campaign_id: &str,
        challenge_id: &str,
        cycle_id: &str,
        cycle_event_version: u64,
        now_ms: i64,
    ) -> Result<ActorContext, LedgerError> {
        if cycle_id.trim().is_empty() || cycle_event_version == 0 {
            return Err(LedgerError::Degraded(
                "chief spawn lookup requires a cycle id and nonzero event version".into(),
            ));
        }
        let context = self
            .resolve_chief_context_for_action(
                lease_id,
                live_agent_id,
                live_session_id,
                live_thread_id,
                campaign_id,
                challenge_id,
                Action::SpawnChild,
                now_ms,
            )
            .await?;
        let cycle_row = sqlx::query(
            "SELECT chief_lease_id, cycle_json, cycle_sha256, cycle_event_version, revoked_at_ms FROM research_cycle_issuances WHERE cycle_id = ? AND campaign_id = ? AND challenge_id = ?",
        )
        .bind(cycle_id)
        .bind(campaign_id)
        .bind(challenge_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| LedgerError::Degraded("active research cycle is not registered".into()))?;
        let stored_version = u64::try_from(cycle_row.try_get::<i64, _>("cycle_event_version")?)
            .map_err(|_| LedgerError::Degraded("stored cycle event version is invalid".into()))?;
        if cycle_row
            .try_get::<Option<i64>, _>("revoked_at_ms")?
            .is_some()
            || cycle_row.try_get::<String, _>("chief_lease_id")? != lease_id
            || stored_version != cycle_event_version
        {
            return Err(LedgerError::Degraded(
                "research cycle is revoked, stale, or not issued by the live Chief lease".into(),
            ));
        }
        let cycle_json: String = cycle_row.try_get("cycle_json")?;
        if sha256_bytes(cycle_json.as_bytes()) != cycle_row.try_get::<String, _>("cycle_sha256")? {
            return Err(LedgerError::Degraded(
                "active research cycle hash does not match".into(),
            ));
        }
        let cycle: ResearchCycleRecord = serde_json::from_str(&cycle_json).map_err(|error| {
            LedgerError::Degraded(format!("stored research cycle is invalid JSON: {error}"))
        })?;
        if cycle.cycle_id != cycle_id
            || cycle.campaign_id != campaign_id
            || cycle.challenge_id != challenge_id
        {
            return Err(LedgerError::Degraded(
                "stored research cycle does not match the Chief spawn binding".into(),
            ));
        }
        cycle.validate(now_ms).map_err(|error| {
            LedgerError::Degraded(format!("active research cycle is invalid: {error}"))
        })?;
        Ok(context)
    }

    async fn resolve_chief_context_for_action(
        &self,
        lease_id: &str,
        live_agent_id: &str,
        live_session_id: &str,
        live_thread_id: &str,
        campaign_id: &str,
        challenge_id: &str,
        action: Action,
        now_ms: i64,
    ) -> Result<ActorContext, LedgerError> {
        if [
            lease_id,
            live_agent_id,
            live_session_id,
            live_thread_id,
            campaign_id,
            challenge_id,
        ]
        .iter()
        .any(|value| value.trim().is_empty())
        {
            return Err(LedgerError::Degraded(
                "chief lookup requires complete live identity and campaign binding".into(),
            ));
        }
        let row = sqlx::query(
            "SELECT agent_id, session_id, thread_id, campaign_id, challenge_id, role, context_json, revoked_at_ms FROM actor_leases WHERE lease_id = ?",
        )
        .bind(lease_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| LedgerError::Degraded("chief lease is not registered".into()))?;
        if row.try_get::<Option<i64>, _>("revoked_at_ms")?.is_some()
            || row.try_get::<String, _>("role")? != role_name(Role::Chief)
        {
            return Err(LedgerError::Degraded(
                "chief lease is revoked or belongs to another role".into(),
            ));
        }
        let context_json: String = row.try_get("context_json")?;
        let context: ActorContext = serde_json::from_str(&context_json).map_err(|error| {
            LedgerError::Degraded(format!("stored chief context is invalid: {error}"))
        })?;
        if context.role != Role::Chief
            || context.lease.lease_id != lease_id
            || context.agent_id != live_agent_id
            || context.session_id != live_session_id
            || context.thread_id != live_thread_id
            || context.campaign_id != campaign_id
            || context.challenge_id != challenge_id
        {
            return Err(LedgerError::Degraded(
                "chief lease does not match the live invocation".into(),
            ));
        }
        context.validate(action, now_ms).map_err(|error| {
            LedgerError::Degraded(format!("chief lease validation failed: {error}"))
        })?;
        Ok(context)
    }

    /// Atomically records a chief-approved research cycle and all routed StageBriefs.
    /// The coordination event is written to the campaign aggregate in the same transaction, so a
    /// stale decision cannot create an orphaned brief that a worker may later load.
    pub async fn issue_research_cycle_audited(
        &self,
        chief: &ActorContext,
        cycle: &ResearchCycleRecord,
        workspace_root: &Path,
        capability_map_path: &str,
        now_ms: i64,
        event: &CoordinationEventRecord<'_>,
    ) -> Result<CycleIssuance, LedgerError> {
        self.issue_research_cycle_internal(
            chief,
            cycle,
            workspace_root,
            capability_map_path,
            None,
            now_ms,
            event,
        )
        .await
    }

    /// Atomically invalidates one active predecessor and installs its Chief-approved successor.
    pub async fn supersede_research_cycle_audited(
        &self,
        chief: &ActorContext,
        predecessor_cycle_id: &str,
        cycle: &ResearchCycleRecord,
        workspace_root: &Path,
        capability_map_path: &str,
        now_ms: i64,
        event: &CoordinationEventRecord<'_>,
    ) -> Result<CycleIssuance, LedgerError> {
        if predecessor_cycle_id.trim().is_empty() || predecessor_cycle_id == cycle.cycle_id {
            return Err(LedgerError::Degraded(
                "supersede requires a distinct predecessor cycle id".into(),
            ));
        }
        self.issue_research_cycle_internal(
            chief,
            cycle,
            workspace_root,
            capability_map_path,
            Some(predecessor_cycle_id),
            now_ms,
            event,
        )
        .await
    }

    async fn issue_research_cycle_internal(
        &self,
        chief: &ActorContext,
        cycle: &ResearchCycleRecord,
        workspace_root: &Path,
        capability_map_path: &str,
        predecessor_cycle_id: Option<&str>,
        now_ms: i64,
        event: &CoordinationEventRecord<'_>,
    ) -> Result<CycleIssuance, LedgerError> {
        let resolved_chief = self
            .resolve_chief_context(
                &chief.lease.lease_id,
                &chief.agent_id,
                &chief.session_id,
                &chief.thread_id,
                &chief.campaign_id,
                &chief.challenge_id,
                now_ms,
            )
            .await?;
        if resolved_chief != *chief {
            return Err(LedgerError::Degraded(
                "chief context does not match the registered lease".into(),
            ));
        }
        cycle
            .validate(now_ms)
            .map_err(|error| LedgerError::Degraded(format!("invalid research cycle: {error}")))?;
        if cycle.campaign_id != chief.campaign_id || cycle.challenge_id != chief.challenge_id {
            return Err(LedgerError::Degraded(
                "research cycle does not match the chief lease".into(),
            ));
        }
        let workspace_root = std::fs::canonicalize(workspace_root).map_err(|error| {
            LedgerError::Degraded(format!(
                "cannot canonicalize stage brief workspace root: {error}"
            ))
        })?;
        if !workspace_root.is_dir() {
            return Err(LedgerError::Degraded(
                "stage brief workspace root is not a directory".into(),
            ));
        }
        for brief in &cycle.stage_briefs {
            validate_stage_brief_files(brief, &workspace_root, capability_map_path)?;
            let challenge_root =
                std::fs::canonicalize(&brief.challenge_workspace_root).map_err(|error| {
                    LedgerError::Degraded(format!(
                        "cannot canonicalize stage brief challenge workspace root: {error}"
                    ))
                })?;
            if !challenge_root.is_dir() || workspace_root.starts_with(&challenge_root) {
                return Err(LedgerError::Degraded(
                    "stage brief challenge workspace must be a directory and cannot contain the policy root".into(),
                ));
            }
        }
        let cycle_json = serde_json::to_string(cycle).map_err(|error| {
            LedgerError::Degraded(format!("cannot serialize research cycle: {error}"))
        })?;
        let expected_event_type = if predecessor_cycle_id.is_some() {
            "research_cycle_superseded"
        } else {
            "research_cycle_issued"
        };
        let expected_event_payload = if let Some(predecessor_cycle_id) = predecessor_cycle_id {
            serde_json::json!({
                "predecessor_cycle_id": predecessor_cycle_id,
                "cycle": cycle,
            })
            .to_string()
        } else {
            cycle_json.clone()
        };
        if event.aggregate_type != "campaign"
            || event.aggregate_id != cycle.campaign_id
            || event.expected_version != cycle.expected_state_version
            || event.event_type != expected_event_type
            || event.payload_json != expected_event_payload
            || event.occurred_at_ms != now_ms
        {
            return Err(LedgerError::Degraded(
                "research cycle event is not bound to the chief-approved cycle".into(),
            ));
        }
        let cycle_sha256 = sha256_bytes(cycle_json.as_bytes());
        let brief_ids = cycle
            .stage_briefs
            .iter()
            .map(|brief| brief.brief_id.clone())
            .collect::<Vec<_>>();
        let mut transaction = self.pool.begin().await?;
        let appended = append_event_in_transaction(&mut transaction, event).await?;
        if appended.replayed {
            let row = sqlx::query(
                "SELECT cycle_json, cycle_sha256, cycle_event_version, chief_lease_id, revoked_at_ms FROM research_cycle_issuances WHERE cycle_id = ?",
            )
            .bind(&cycle.cycle_id)
            .fetch_optional(&mut *transaction)
            .await?
            .ok_or_else(|| LedgerError::Degraded("replayed cycle issuance is missing".into()))?;
            if row.try_get::<String, _>("cycle_json")? != cycle_json
                || row.try_get::<String, _>("cycle_sha256")? != cycle_sha256
                || row.try_get::<i64, _>("cycle_event_version")?
                    != i64::try_from(appended.version)
                        .map_err(|_| LedgerError::Degraded("cycle version overflow".into()))?
                || row.try_get::<String, _>("chief_lease_id")? != chief.lease.lease_id
                || row.try_get::<Option<i64>, _>("revoked_at_ms")?.is_some()
            {
                return Err(LedgerError::Degraded(
                    "replayed cycle issuance does not match stored state".into(),
                ));
            }
            let rows = sqlx::query(
                "SELECT brief_id, brief_json, brief_sha256, workspace_root, capability_map_path, revoked_at_ms FROM stage_brief_issuances WHERE cycle_id = ? ORDER BY brief_id",
            )
            .bind(&cycle.cycle_id)
            .fetch_all(&mut *transaction)
            .await?;
            if rows.len() != cycle.stage_briefs.len() {
                return Err(LedgerError::Degraded(
                    "replayed cycle issuance has a different brief set".into(),
                ));
            }
            for brief in &cycle.stage_briefs {
                let brief_json = serde_json::to_string(brief).map_err(|error| {
                    LedgerError::Degraded(format!("cannot serialize stage brief: {error}"))
                })?;
                let matching = rows.iter().find(|row| {
                    row.try_get::<String, _>("brief_id").ok().as_deref()
                        == Some(brief.brief_id.as_str())
                });
                let Some(matching) = matching else {
                    return Err(LedgerError::Degraded(
                        "replayed cycle issuance is missing a stage brief".into(),
                    ));
                };
                if matching.try_get::<String, _>("brief_json")? != brief_json
                    || matching.try_get::<String, _>("brief_sha256")?
                        != sha256_bytes(brief_json.as_bytes())
                    || matching.try_get::<String, _>("workspace_root")?
                        != workspace_root.to_string_lossy()
                    || matching.try_get::<String, _>("capability_map_path")? != capability_map_path
                    || matching
                        .try_get::<Option<i64>, _>("revoked_at_ms")?
                        .is_some()
                {
                    return Err(LedgerError::Degraded(
                        "replayed stage brief issuance does not match stored state".into(),
                    ));
                }
            }
            let root = sqlx::query(
                "SELECT parent_thread_id, agent_id, session_id, campaign_id, challenge_id, cycle_id, cycle_event_version, chief_lease_id, role, revoked_at_ms FROM thread_cycle_bindings WHERE thread_id = ? AND role = ? AND revoked_at_ms IS NULL",
            )
            .bind(&chief.thread_id)
            .bind(role_name(Role::Chief))
            .fetch_optional(&mut *transaction)
            .await?
            .ok_or_else(|| {
                LedgerError::Degraded("replayed cycle issuance is missing its Chief root binding".into())
            })?;
            if root
                .try_get::<Option<String>, _>("parent_thread_id")?
                .is_some()
                || root.try_get::<String, _>("agent_id")? != chief.agent_id
                || root.try_get::<String, _>("session_id")? != chief.session_id
                || root.try_get::<String, _>("campaign_id")? != cycle.campaign_id
                || root.try_get::<String, _>("challenge_id")? != cycle.challenge_id
                || root.try_get::<String, _>("cycle_id")? != cycle.cycle_id
                || root.try_get::<i64, _>("cycle_event_version")?
                    != i64::try_from(appended.version)
                        .map_err(|_| LedgerError::Degraded("cycle version overflow".into()))?
                || root.try_get::<String, _>("chief_lease_id")? != chief.lease.lease_id
                || root.try_get::<Option<i64>, _>("revoked_at_ms")?.is_some()
            {
                return Err(LedgerError::Degraded(
                    "replayed cycle issuance does not match its Chief root binding".into(),
                ));
            }
            if let Some(predecessor) = predecessor_cycle_id {
                let predecessor_row = sqlx::query(
                    "SELECT revoked_at_ms FROM research_cycle_issuances WHERE cycle_id = ? AND campaign_id = ? AND challenge_id = ?",
                )
                .bind(predecessor)
                .bind(&cycle.campaign_id)
                .bind(&cycle.challenge_id)
                .fetch_optional(&mut *transaction)
                .await?
                .ok_or_else(|| {
                    LedgerError::Degraded("replayed supersede predecessor is missing".into())
                })?;
                if predecessor_row.try_get::<Option<i64>, _>("revoked_at_ms")? != Some(now_ms) {
                    return Err(LedgerError::Degraded(
                        "replayed supersede predecessor is not revoked".into(),
                    ));
                }
            }
            transaction.commit().await?;
            return Ok(CycleIssuance {
                cycle_id: cycle.cycle_id.clone(),
                brief_ids,
                cycle_event_version: appended.version,
                cycle_sha256,
            });
        }
        let active_cycles = sqlx::query(
            "SELECT cycle_id FROM research_cycle_issuances WHERE campaign_id = ? AND challenge_id = ? AND revoked_at_ms IS NULL AND cycle_id != ?",
        )
        .bind(&cycle.campaign_id)
        .bind(&cycle.challenge_id)
        .bind(&cycle.cycle_id)
        .fetch_all(&mut *transaction)
        .await?;
        match predecessor_cycle_id {
            None if !active_cycles.is_empty() => {
                return Err(LedgerError::Degraded(
                    "an active research cycle already exists; use the supersede control path"
                        .into(),
                ));
            }
            Some(predecessor) => {
                if active_cycles.len() != 1
                    || active_cycles[0].try_get::<String, _>("cycle_id")? != predecessor
                {
                    return Err(LedgerError::Degraded(
                        "supersede predecessor is not the single active cycle".into(),
                    ));
                }
                let predecessor_row = sqlx::query(
                    "SELECT cycle_json, cycle_sha256, revoked_at_ms FROM research_cycle_issuances WHERE cycle_id = ? AND campaign_id = ? AND challenge_id = ?",
                )
                .bind(predecessor)
                .bind(&cycle.campaign_id)
                .bind(&cycle.challenge_id)
                .fetch_optional(&mut *transaction)
                .await?
                .ok_or_else(|| {
                    LedgerError::Degraded("supersede predecessor record is missing".into())
                })?;
                if predecessor_row
                    .try_get::<Option<i64>, _>("revoked_at_ms")?
                    .is_some()
                {
                    return Err(LedgerError::Degraded(
                        "supersede predecessor is already revoked".into(),
                    ));
                }
                let predecessor_json: String = predecessor_row.try_get("cycle_json")?;
                if sha256_bytes(predecessor_json.as_bytes())
                    != predecessor_row.try_get::<String, _>("cycle_sha256")?
                {
                    return Err(LedgerError::Degraded(
                        "supersede predecessor hash does not match".into(),
                    ));
                }
                let predecessor_record: ResearchCycleRecord =
                    serde_json::from_str(&predecessor_json).map_err(|error| {
                        LedgerError::Degraded(format!(
                            "supersede predecessor is invalid JSON: {error}"
                        ))
                    })?;
                ResearchCycleRecord::validate_successor(&predecessor_record, cycle, now_ms)
                    .map_err(|error| {
                        LedgerError::Degraded(format!("invalid research cycle successor: {error}"))
                    })?;
                let revoked = sqlx::query(
                    "UPDATE research_cycle_issuances SET revoked_at_ms = ? WHERE cycle_id = ? AND revoked_at_ms IS NULL",
                )
                .bind(now_ms)
                .bind(predecessor)
                .execute(&mut *transaction)
                .await?;
                if revoked.rows_affected() != 1 {
                    return Err(LedgerError::Degraded(
                        "active predecessor disappeared during supersede".into(),
                    ));
                }
                sqlx::query(
                    "UPDATE stage_brief_issuances SET revoked_at_ms = ? WHERE cycle_id = ? AND revoked_at_ms IS NULL",
                )
                .bind(now_ms)
                .bind(predecessor)
                .execute(&mut *transaction)
                .await?;
                sqlx::query(
                    "UPDATE thread_cycle_bindings SET revoked_at_ms = ? WHERE cycle_id = ? AND revoked_at_ms IS NULL",
                )
                .bind(now_ms)
                .bind(predecessor)
                .execute(&mut *transaction)
                .await?;
            }
            _ => {}
        }
        sqlx::query(
            "INSERT INTO research_cycle_issuances (cycle_id, campaign_id, challenge_id, chief_lease_id, cycle_json, cycle_sha256, cycle_event_version, issued_at_ms, revoked_at_ms) VALUES (?, ?, ?, ?, ?, ?, ?, ?, NULL)",
        )
        .bind(&cycle.cycle_id)
        .bind(&cycle.campaign_id)
        .bind(&cycle.challenge_id)
        .bind(&chief.lease.lease_id)
        .bind(&cycle_json)
        .bind(&cycle_sha256)
        .bind(i64::try_from(appended.version).map_err(|_| LedgerError::Degraded("cycle version overflow".into()))?)
        .bind(now_ms)
        .execute(&mut *transaction)
        .await?;
        let root_binding_id = format!("chief:{}:{}", chief.thread_id, cycle.cycle_id);
        sqlx::query(
            "INSERT INTO thread_cycle_bindings (binding_id, thread_id, parent_thread_id, agent_id, session_id, campaign_id, challenge_id, cycle_id, cycle_event_version, chief_lease_id, role, issued_at_ms, revoked_at_ms) VALUES (?, ?, NULL, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)",
        )
        .bind(root_binding_id)
        .bind(&chief.thread_id)
        .bind(&chief.agent_id)
        .bind(&chief.session_id)
        .bind(&chief.campaign_id)
        .bind(&chief.challenge_id)
        .bind(&cycle.cycle_id)
        .bind(i64::try_from(appended.version).map_err(|_| LedgerError::Degraded("cycle version overflow".into()))?)
        .bind(&chief.lease.lease_id)
        .bind(role_name(Role::Chief))
        .bind(now_ms)
        .execute(&mut *transaction)
        .await?;
        for brief in &cycle.stage_briefs {
            let brief_json = serde_json::to_string(brief).map_err(|error| {
                LedgerError::Degraded(format!("cannot serialize stage brief: {error}"))
            })?;
            let brief_sha256 = sha256_bytes(brief_json.as_bytes());
            sqlx::query(
                "INSERT INTO stage_brief_issuances (brief_id, cycle_id, campaign_id, challenge_id, role, brief_json, brief_sha256, workspace_root, capability_map_path, issued_at_ms, expires_at_ms, revoked_at_ms) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)",
            )
            .bind(&brief.brief_id)
            .bind(&cycle.cycle_id)
            .bind(&cycle.campaign_id)
            .bind(&cycle.challenge_id)
            .bind(role_name(brief.target_role))
            .bind(brief_json)
            .bind(brief_sha256)
            .bind(workspace_root.to_string_lossy().as_ref())
            .bind(capability_map_path)
            .bind(now_ms)
            .bind(brief.expires_at_ms)
            .execute(&mut *transaction)
            .await?;
        }
        transaction.commit().await?;
        Ok(CycleIssuance {
            cycle_id: cycle.cycle_id.clone(),
            brief_ids,
            cycle_event_version: appended.version,
            cycle_sha256,
        })
    }

    /// Loads the immutable, chief-approved StageBrief selected by a worker dispatch.
    pub async fn load_stage_brief_issuance(
        &self,
        target: &StageBriefLedgerTarget<'_>,
        now_ms: i64,
    ) -> Result<PersistedStageBrief, LedgerError> {
        if [target.cycle_id, target.campaign_id, target.challenge_id]
            .iter()
            .any(|value| value.trim().is_empty())
        {
            return Err(LedgerError::Degraded(
                "stage brief lookup requires complete cycle and campaign binding".into(),
            ));
        }
        let row = sqlx::query(
            "SELECT brief.brief_json, brief.brief_sha256, brief.workspace_root, brief.capability_map_path, cycle.cycle_json, cycle.cycle_sha256, cycle.cycle_event_version, brief.revoked_at_ms FROM stage_brief_issuances AS brief INNER JOIN research_cycle_issuances AS cycle ON cycle.cycle_id = brief.cycle_id WHERE brief.cycle_id = ? AND brief.campaign_id = ? AND brief.challenge_id = ? AND brief.role = ? AND cycle.campaign_id = brief.campaign_id AND cycle.challenge_id = brief.challenge_id AND cycle.revoked_at_ms IS NULL",
        )
        .bind(target.cycle_id)
        .bind(target.campaign_id)
        .bind(target.challenge_id)
        .bind(role_name(target.role))
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| LedgerError::Degraded("stage brief issuance is not registered".into()))?;
        if row.try_get::<Option<i64>, _>("revoked_at_ms")?.is_some() {
            return Err(LedgerError::Degraded(
                "stage brief issuance is revoked".into(),
            ));
        }
        let brief_json: String = row.try_get("brief_json")?;
        if sha256_bytes(brief_json.as_bytes()) != row.try_get::<String, _>("brief_sha256")? {
            return Err(LedgerError::Degraded(
                "stage brief issuance hash mismatch".into(),
            ));
        }
        let stage_brief: StageBrief = serde_json::from_str(&brief_json).map_err(|error| {
            LedgerError::Degraded(format!("stored stage brief is invalid JSON: {error}"))
        })?;
        let cycle_json: String = row.try_get("cycle_json")?;
        if sha256_bytes(cycle_json.as_bytes()) != row.try_get::<String, _>("cycle_sha256")? {
            return Err(LedgerError::Degraded(
                "research cycle issuance hash mismatch".into(),
            ));
        }
        let cycle: ResearchCycleRecord = serde_json::from_str(&cycle_json).map_err(|error| {
            LedgerError::Degraded(format!("stored research cycle is invalid JSON: {error}"))
        })?;
        if cycle.cycle_id != target.cycle_id
            || cycle.campaign_id != target.campaign_id
            || cycle.challenge_id != target.challenge_id
            || !cycle.stage_briefs.contains(&stage_brief)
        {
            return Err(LedgerError::Degraded(
                "stored research cycle does not bind the requested stage brief".into(),
            ));
        }
        cycle.validate(now_ms).map_err(|error| {
            LedgerError::Degraded(format!("stored research cycle is invalid: {error}"))
        })?;
        if stage_brief.campaign_id != target.campaign_id
            || stage_brief.challenge_id != target.challenge_id
            || stage_brief.target_role != target.role
        {
            return Err(LedgerError::Degraded(
                "stored stage brief does not match its lookup binding".into(),
            ));
        }
        stage_brief.validate(now_ms).map_err(|error| {
            LedgerError::Degraded(format!("stored stage brief is invalid: {error}"))
        })?;
        Ok(PersistedStageBrief {
            cycle_id: target.cycle_id.to_string(),
            campaign_id: target.campaign_id.to_string(),
            challenge_id: target.challenge_id.to_string(),
            role: target.role,
            stage_brief,
            workspace_root: PathBuf::from(row.try_get::<String, _>("workspace_root")?),
            capability_map_path: row.try_get("capability_map_path")?,
            cycle_event_version: u64::try_from(row.try_get::<i64, _>("cycle_event_version")?)
                .map_err(|_| LedgerError::Degraded("stored cycle version is negative".into()))?,
        })
    }

    /// Revokes a Chief-issued cycle and every routed brief atomically. This is a control-plane
    /// operation; worker contexts and model-visible tools cannot invoke it.
    pub async fn revoke_research_cycle_audited(
        &self,
        chief: &ActorContext,
        cycle_id: &str,
        now_ms: i64,
        event: &CoordinationEventRecord<'_>,
    ) -> Result<u64, LedgerError> {
        if cycle_id.trim().is_empty() {
            return Err(LedgerError::Degraded("cycle id is required".into()));
        }
        let resolved = self
            .resolve_chief_context(
                &chief.lease.lease_id,
                &chief.agent_id,
                &chief.session_id,
                &chief.thread_id,
                &chief.campaign_id,
                &chief.challenge_id,
                now_ms,
            )
            .await?;
        if resolved != *chief {
            return Err(LedgerError::Degraded(
                "chief context does not match the registered lease".into(),
            ));
        }
        if event.aggregate_type != "campaign"
            || event.aggregate_id != chief.campaign_id
            || event.event_type != "research_cycle_revoked"
            || event.payload_json != serde_json::json!({ "cycle_id": cycle_id }).to_string()
            || event.occurred_at_ms != now_ms
        {
            return Err(LedgerError::Degraded(
                "cycle revocation event is not bound to the chief campaign".into(),
            ));
        }
        let mut transaction = self.pool.begin().await?;
        let cycle = sqlx::query(
            "SELECT campaign_id, challenge_id, revoked_at_ms FROM research_cycle_issuances WHERE cycle_id = ?",
        )
        .bind(cycle_id)
        .fetch_optional(&mut *transaction)
        .await?
        .ok_or_else(|| LedgerError::Degraded("research cycle is not registered".into()))?;
        if cycle.try_get::<String, _>("campaign_id")? != chief.campaign_id
            || cycle.try_get::<String, _>("challenge_id")? != chief.challenge_id
        {
            return Err(LedgerError::Degraded(
                "research cycle is not owned by the live chief campaign".into(),
            ));
        }
        let appended = append_event_in_transaction(&mut transaction, event).await?;
        if !appended.replayed {
            if cycle.try_get::<Option<i64>, _>("revoked_at_ms")?.is_some() {
                return Err(LedgerError::Degraded(
                    "research cycle is already revoked".into(),
                ));
            }
            let result = sqlx::query(
                "UPDATE research_cycle_issuances SET revoked_at_ms = ? WHERE cycle_id = ? AND revoked_at_ms IS NULL",
            )
            .bind(now_ms)
            .bind(cycle_id)
            .execute(&mut *transaction)
            .await?;
            if result.rows_affected() != 1 {
                return Err(LedgerError::Degraded(
                    "research cycle disappeared during revocation".into(),
                ));
            }
            sqlx::query(
                "UPDATE stage_brief_issuances SET revoked_at_ms = ? WHERE cycle_id = ? AND revoked_at_ms IS NULL",
            )
            .bind(now_ms)
            .bind(cycle_id)
            .execute(&mut *transaction)
            .await?;
            sqlx::query(
                "UPDATE thread_cycle_bindings SET revoked_at_ms = ? WHERE cycle_id = ? AND revoked_at_ms IS NULL",
            )
            .bind(now_ms)
            .bind(cycle_id)
            .execute(&mut *transaction)
            .await?;
        } else {
            let revoked = sqlx::query(
                "SELECT revoked_at_ms FROM research_cycle_issuances WHERE cycle_id = ?",
            )
            .bind(cycle_id)
            .fetch_one(&mut *transaction)
            .await?;
            if revoked.try_get::<Option<i64>, _>("revoked_at_ms")? != Some(now_ms) {
                return Err(LedgerError::Degraded(
                    "replayed cycle revocation does not match stored state".into(),
                ));
            }
        }
        transaction.commit().await?;
        Ok(appended.version)
    }

    pub async fn revoke_actor_lease(
        &self,
        lease_id: &str,
        revoked_at_ms: i64,
    ) -> Result<(), LedgerError> {
        if lease_id.trim().is_empty() || revoked_at_ms < 0 {
            return Err(LedgerError::Degraded(
                "lease id and revocation time are required".into(),
            ));
        }
        let result = sqlx::query(
            "UPDATE actor_leases SET revoked_at_ms = ? WHERE lease_id = ? AND revoked_at_ms IS NULL",
        )
        .bind(revoked_at_ms)
        .bind(lease_id)
        .execute(&self.pool)
        .await?;
        if result.rows_affected() != 1 {
            return Err(LedgerError::Degraded(
                "actor lease is missing or already revoked".into(),
            ));
        }
        Ok(())
    }

    pub async fn revoke_actor_lease_audited(
        &self,
        lease_id: &str,
        revoked_at_ms: i64,
        event: &CoordinationEventRecord<'_>,
    ) -> Result<u64, LedgerError> {
        if lease_id.trim().is_empty()
            || revoked_at_ms < 0
            || event.aggregate_type != "actor_lease"
            || event.aggregate_id != lease_id
            || event.event_type != "actor_lease_revoked"
            || event.occurred_at_ms != revoked_at_ms
        {
            return Err(LedgerError::Degraded(
                "lease revocation event is not bound to the lease".into(),
            ));
        }
        let mut transaction = self.pool.begin().await?;
        let appended = append_event_in_transaction(&mut transaction, event).await?;
        if appended.replayed {
            let row = sqlx::query("SELECT revoked_at_ms FROM actor_leases WHERE lease_id = ?")
                .bind(lease_id)
                .fetch_optional(&mut *transaction)
                .await?
                .ok_or_else(|| {
                    LedgerError::Degraded("replayed revocation is missing its lease".into())
                })?;
            if row.try_get::<Option<i64>, _>("revoked_at_ms")? != Some(revoked_at_ms) {
                return Err(LedgerError::Degraded(
                    "replayed revocation does not match the lease state".into(),
                ));
            }
            transaction.commit().await?;
            return Ok(appended.version);
        }
        let result = sqlx::query(
            "UPDATE actor_leases SET revoked_at_ms = ? WHERE lease_id = ? AND revoked_at_ms IS NULL",
        )
        .bind(revoked_at_ms)
        .bind(lease_id)
        .execute(&mut *transaction)
        .await?;
        if result.rows_affected() != 1 {
            return Err(LedgerError::Degraded(
                "actor lease is missing or already revoked".into(),
            ));
        }
        transaction.commit().await?;
        Ok(appended.version)
    }

    pub async fn inspect_actor_lease(
        &self,
        lease_id: &str,
    ) -> Result<ActorLeaseMetadata, LedgerError> {
        if lease_id.trim().is_empty() {
            return Err(LedgerError::Degraded("lease id is required".into()));
        }
        let row = sqlx::query(
            "SELECT lease_id, agent_id, session_id, thread_id, campaign_id, challenge_id, role, registered_at_ms, revoked_at_ms FROM actor_leases WHERE lease_id = ?",
        )
        .bind(lease_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| LedgerError::Degraded("actor lease is not registered".into()))?;
        Ok(ActorLeaseMetadata {
            lease_id: row.try_get("lease_id")?,
            agent_id: row.try_get("agent_id")?,
            session_id: row.try_get("session_id")?,
            thread_id: row.try_get("thread_id")?,
            campaign_id: row.try_get("campaign_id")?,
            challenge_id: row.try_get("challenge_id")?,
            role: row.try_get("role")?,
            registered_at_ms: row.try_get("registered_at_ms")?,
            revoked_at_ms: row.try_get("revoked_at_ms")?,
        })
    }

    /// Append one event with optimistic version checking. Replaying the same idempotency key is
    /// a no-op and returns its original version; concurrent or stale writers fail closed.
    pub async fn append_coordination_event(
        &self,
        record: &CoordinationEventRecord<'_>,
    ) -> Result<u64, LedgerError> {
        let mut transaction = self.pool.begin().await?;
        let version = append_event_in_transaction(&mut transaction, record)
            .await?
            .version;
        transaction.commit().await?;
        Ok(version)
    }

    pub async fn reserve(
        &self,
        id: &str,
        challenge_id: &str,
        owner: &str,
        estimated_cost: f64,
        budget: f64,
    ) -> Result<(), LedgerError> {
        self.reserve_with_cadence(id, challenge_id, owner, estimated_cost, budget, "", 0, 0)
            .await
    }

    pub async fn reserve_with_cadence(
        &self,
        id: &str,
        challenge_id: &str,
        owner: &str,
        estimated_cost: f64,
        budget: f64,
        content_sha256: &str,
        now_ms: i64,
        min_interval_seconds: i64,
    ) -> Result<(), LedgerError> {
        if estimated_cost.is_sign_negative() {
            return Err(LedgerError::Degraded("negative estimated cost".into()));
        }
        if min_interval_seconds.is_negative() {
            return Err(LedgerError::Degraded("negative cadence interval".into()));
        }
        let mut transaction = self.pool.begin().await?;
        let row = sqlx::query(
            "SELECT COALESCE(SUM(estimated_cost), 0.0) AS reserved FROM reservations WHERE challenge_id = ? AND state = 'reserved'",
        )
        .bind(challenge_id)
        .fetch_one(&mut *transaction)
        .await?;
        let reserved: f64 = row.try_get("reserved")?;
        if reserved + estimated_cost > budget {
            return Err(LedgerError::Degraded(
                "reservation would exceed challenge budget".into(),
            ));
        }
        if min_interval_seconds > 0 {
            let latest = sqlx::query(
                "SELECT MAX(started_at_ms) AS latest FROM attempts WHERE challenge_id = ? AND owner = ? AND state IN ('reserved', 'committed')",
            )
            .bind(challenge_id)
            .bind(owner)
            .fetch_one(&mut *transaction)
            .await?;
            let latest_ms: Option<i64> = latest.try_get("latest")?;
            if let Some(latest_ms) = latest_ms {
                let elapsed = now_ms.saturating_sub(latest_ms);
                if elapsed < min_interval_seconds.saturating_mul(1000) {
                    return Err(LedgerError::Degraded(
                        "submission cadence interval is not satisfied".into(),
                    ));
                }
            }
        }
        sqlx::query(
            "INSERT INTO reservations (id, challenge_id, owner, estimated_cost, state) VALUES (?, ?, ?, ?, 'reserved')",
        )
        .bind(id)
        .bind(challenge_id)
        .bind(owner)
        .bind(estimated_cost)
        .execute(&mut *transaction)
        .await?;
        sqlx::query(
            "INSERT INTO attempts (id, challenge_id, owner, content_sha256, started_at_ms, state, result_json) VALUES (?, ?, ?, ?, ?, 'reserved', NULL)",
        )
        .bind(id)
        .bind(challenge_id)
        .bind(owner)
        .bind(content_sha256)
        .bind(now_ms)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(())
    }

    pub async fn release(&self, id: &str) -> Result<(), LedgerError> {
        let mut transaction = self.pool.begin().await?;
        let result = sqlx::query(
            "UPDATE reservations SET state = 'released' WHERE id = ? AND state = 'reserved'",
        )
        .bind(id)
        .execute(&mut *transaction)
        .await?;
        if result.rows_affected() == 0 {
            return Err(LedgerError::Degraded(
                "reservation is missing or already finalized".into(),
            ));
        }
        let attempt = sqlx::query(
            "UPDATE attempts SET state = 'released' WHERE id = ? AND state = 'reserved'",
        )
        .bind(id)
        .execute(&mut *transaction)
        .await?;
        if attempt.rows_affected() == 0 {
            return Err(LedgerError::Degraded(
                "attempt record is missing or already finalized".into(),
            ));
        }
        transaction.commit().await?;
        Ok(())
    }

    pub async fn commit(&self, id: &str, result_json: Option<&str>) -> Result<(), LedgerError> {
        let mut transaction = self.pool.begin().await?;
        let result = sqlx::query(
            "UPDATE reservations SET state = 'committed' WHERE id = ? AND state = 'reserved'",
        )
        .bind(id)
        .execute(&mut *transaction)
        .await?;
        if result.rows_affected() == 0 {
            return Err(LedgerError::Degraded(
                "reservation is missing or already finalized".into(),
            ));
        }
        let attempt = sqlx::query(
            "UPDATE attempts SET state = 'committed', result_json = ? WHERE id = ? AND state = 'reserved'",
        )
        .bind(result_json)
        .bind(id)
        .execute(&mut *transaction)
        .await?;
        if attempt.rows_affected() == 0 {
            return Err(LedgerError::Degraded(
                "attempt record is missing or already finalized".into(),
            ));
        }
        transaction.commit().await?;
        Ok(())
    }
}

struct AppendedEvent {
    version: u64,
    replayed: bool,
}

async fn append_event_in_transaction(
    transaction: &mut Transaction<'_, Sqlite>,
    record: &CoordinationEventRecord<'_>,
) -> Result<AppendedEvent, LedgerError> {
    if record.event_id.trim().is_empty()
        || record.idempotency_key.trim().is_empty()
        || record.aggregate_type.trim().is_empty()
        || record.aggregate_id.trim().is_empty()
        || record.event_type.trim().is_empty()
        || record.payload_json.trim().is_empty()
        || record.occurred_at_ms < 0
    {
        return Err(LedgerError::Degraded(
            "coordination event identifiers, payload, and timestamp are required".into(),
        ));
    }
    if let Some(row) = sqlx::query(
        "SELECT event_id, aggregate_type, aggregate_id, state_version, event_type, payload_json, occurred_at_ms FROM coordination_events WHERE idempotency_key = ?",
    )
    .bind(record.idempotency_key)
    .fetch_optional(&mut **transaction)
    .await?
    {
        let version: i64 = row.try_get("state_version")?;
        let same = row.try_get::<String, _>("event_id")? == record.event_id
            && row.try_get::<String, _>("aggregate_type")? == record.aggregate_type
            && row.try_get::<String, _>("aggregate_id")? == record.aggregate_id
            && row.try_get::<String, _>("event_type")? == record.event_type
            && row.try_get::<String, _>("payload_json")? == record.payload_json
            && row.try_get::<i64, _>("occurred_at_ms")? == record.occurred_at_ms
            && version == i64::try_from(record.expected_version).unwrap_or(-1) + 1;
        if !same {
            return Err(LedgerError::Degraded(
                "idempotency key is already bound to a different event".into(),
            ));
        }
        return Ok(AppendedEvent {
            version: u64::try_from(version)
                .map_err(|_| LedgerError::Degraded("stored event version is negative".into()))?,
            replayed: true,
        });
    }
    let row = sqlx::query(
        "SELECT COALESCE(MAX(state_version), 0) AS current_version FROM coordination_events WHERE aggregate_type = ? AND aggregate_id = ?",
    )
    .bind(record.aggregate_type)
    .bind(record.aggregate_id)
    .fetch_one(&mut **transaction)
    .await?;
    let current: i64 = row.try_get("current_version")?;
    let expected = i64::try_from(record.expected_version)
        .map_err(|_| LedgerError::Degraded("expected event version is too large".into()))?;
    if current != expected {
        return Err(LedgerError::Degraded(format!(
            "coordination state version conflict: expected {}, found {}",
            record.expected_version, current
        )));
    }
    let next = current
        .checked_add(1)
        .ok_or_else(|| LedgerError::Degraded("coordination state version overflow".into()))?;
    sqlx::query(
        "INSERT INTO coordination_events (event_id, idempotency_key, aggregate_type, aggregate_id, state_version, event_type, payload_json, occurred_at_ms) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(record.event_id)
    .bind(record.idempotency_key)
    .bind(record.aggregate_type)
    .bind(record.aggregate_id)
    .bind(next)
    .bind(record.event_type)
    .bind(record.payload_json)
    .bind(record.occurred_at_ms)
    .execute(&mut **transaction)
    .await?;
    Ok(AppendedEvent {
        version: u64::try_from(next)
            .map_err(|_| LedgerError::Degraded("stored event version is negative".into()))?,
        replayed: false,
    })
}

async fn load_platform_observation_row(
    transaction: &mut Transaction<'_, Sqlite>,
    event_id: &str,
    observation_id: &str,
    observation_json: &str,
    observation_sha256: &str,
    monitor_lease_id: &str,
    event_version: u64,
) -> Result<PersistedPlatformObservation, LedgerError> {
    let row = sqlx::query(
        "SELECT observation_id, campaign_id, challenge_id, attempt_id, monitor_lease_id, observation_json, observation_sha256, event_version FROM platform_observations WHERE event_id = ?",
    )
    .bind(event_id)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or_else(|| LedgerError::Degraded("replayed platform observation is missing".into()))?;
    if row.try_get::<String, _>("observation_id")? != observation_id
        || row.try_get::<String, _>("observation_json")? != observation_json
        || row.try_get::<String, _>("observation_sha256")? != observation_sha256
        || row.try_get::<String, _>("monitor_lease_id")? != monitor_lease_id
        || u64::try_from(row.try_get::<i64, _>("event_version")?).map_err(|_| {
            LedgerError::Degraded("stored observation event version is invalid".into())
        })? != event_version
    {
        return Err(LedgerError::Degraded(
            "replayed platform observation does not match stored state".into(),
        ));
    }
    persisted_platform_observation_from_row(&row)
}

fn persisted_platform_observation_from_row(
    row: &sqlx::sqlite::SqliteRow,
) -> Result<PersistedPlatformObservation, LedgerError> {
    let observation_json: String = row.try_get("observation_json")?;
    let observation_sha256: String = row.try_get("observation_sha256")?;
    if sha256_bytes(observation_json.as_bytes()) != observation_sha256 {
        return Err(LedgerError::Degraded(
            "platform observation hash does not match".into(),
        ));
    }
    let observation: PlatformObservation =
        serde_json::from_str(&observation_json).map_err(|error| {
            LedgerError::Degraded(format!(
                "stored platform observation is invalid JSON: {error}"
            ))
        })?;
    Ok(PersistedPlatformObservation {
        observation_id: row.try_get("observation_id")?,
        campaign_id: row.try_get("campaign_id")?,
        challenge_id: row.try_get("challenge_id")?,
        attempt_id: row.try_get("attempt_id")?,
        monitor_lease_id: row.try_get("monitor_lease_id")?,
        observation,
        observation_sha256,
        event_version: u64::try_from(row.try_get::<i64, _>("event_version")?).map_err(|_| {
            LedgerError::Degraded("stored observation event version is invalid".into())
        })?,
    })
}

async fn persisted_recovery_canary_from_row(
    row: &sqlx::sqlite::SqliteRow,
    now_ms: i64,
) -> Result<PersistedRecoveryCanary, LedgerError> {
    let trace_json: String = row.try_get("trace_json")?;
    let trace_sha256: String = row.try_get("trace_sha256")?;
    if sha256_bytes(trace_json.as_bytes()) != trace_sha256 {
        return Err(LedgerError::Degraded(
            "recovery canary hash does not match".into(),
        ));
    }
    let trace: RecoveryCanaryTrace = serde_json::from_str(&trace_json).map_err(|error| {
        LedgerError::Degraded(format!("stored recovery canary is invalid JSON: {error}"))
    })?;
    let recovery_id: String = row.try_get("recovery_id")?;
    let runtime_instance_id: String = row.try_get("runtime_instance_id")?;
    let stored_attempt = u64::try_from(row.try_get::<i64, _>("recovery_attempt")?)
        .map_err(|_| LedgerError::Degraded("stored recovery attempt is invalid".into()))?;
    if trace.recovery_id != recovery_id
        || trace.runtime_instance_id != runtime_instance_id
        || trace.recovery_attempt != stored_attempt
        || !trace.rehydration_allowed(now_ms)
    {
        return Err(LedgerError::Degraded(
            "stored recovery canary is stale, incomplete, or misbound".into(),
        ));
    }
    Ok(PersistedRecoveryCanary {
        recovery_id,
        runtime_instance_id,
        recovery_attempt: stored_attempt,
        trace,
        trace_sha256,
        recorded_at_ms: row.try_get("recorded_at_ms")?,
    })
}

async fn load_reconciliation_snapshot(
    pool: &SqlitePool,
    stream_id: &str,
    challenge_id: &str,
) -> Result<Option<PersistedReconciliationSnapshot>, LedgerError> {
    let row = sqlx::query(
        "SELECT campaign_id, challenge_id, stream_id, snapshot_json, snapshot_sha256, updated_at_ms, event_version FROM reconciliation_snapshots WHERE stream_id = ? AND challenge_id = ?",
    )
    .bind(stream_id)
    .bind(challenge_id)
    .fetch_optional(pool)
    .await?;
    let Some(row) = row else {
        return Ok(None);
    };
    let snapshot_json: String = row.try_get("snapshot_json")?;
    let snapshot_sha256: String = row.try_get("snapshot_sha256")?;
    if sha256_bytes(snapshot_json.as_bytes()) != snapshot_sha256 {
        return Err(LedgerError::Degraded(
            "reconciliation snapshot hash does not match".into(),
        ));
    }
    let snapshot: PlatformReconciliationSnapshot =
        serde_json::from_str(&snapshot_json).map_err(|error| {
            LedgerError::Degraded(format!(
                "stored reconciliation snapshot is invalid JSON: {error}"
            ))
        })?;
    let event_version = row
        .try_get::<Option<i64>, _>("event_version")?
        .map(|version| {
            u64::try_from(version).map_err(|_| {
                LedgerError::Degraded("stored reconciliation event version is invalid".into())
            })
        })
        .transpose()?;
    Ok(Some(PersistedReconciliationSnapshot {
        campaign_id: row.try_get("campaign_id")?,
        challenge_id: row.try_get("challenge_id")?,
        stream_id: row.try_get("stream_id")?,
        snapshot,
        snapshot_sha256,
        updated_at_ms: row.try_get("updated_at_ms")?,
        event_version,
    }))
}

fn reconciliation_item_state_name(item: &PlatformReconcileItem) -> &'static str {
    match item.state {
        PlatformReconcileItemState::Observation { .. } => "observation",
        PlatformReconcileItemState::UnknownNeedsReconcile { .. } => "unknown_needs_reconcile",
    }
}

#[derive(Debug, Clone)]
pub struct SubmissionBroker {
    policy: Policy,
    ledger: Ledger,
}

impl SubmissionBroker {
    pub fn new(policy: Policy, ledger: Ledger) -> Self {
        Self { policy, ledger }
    }

    pub async fn prepare(
        &self,
        request: &AdmissionRequest<'_>,
        reservation_id: &str,
        budget: f64,
    ) -> Result<(), BrokerError> {
        let admission = self.policy.admit(request);
        if !admission.allowed {
            return Err(BrokerError::Admission(admission));
        }
        self.ledger
            .reserve_with_cadence(
                reservation_id,
                request.challenge_id,
                request.owner,
                request.estimated_cost_usd,
                budget,
                request.content_sha256,
                request.now_ms,
                self.policy.cadence.min_interval_seconds,
            )
            .await
            .map_err(BrokerError::from)
    }

    pub async fn commit(
        &self,
        reservation_id: &str,
        result_json: Option<&str>,
    ) -> Result<(), BrokerError> {
        self.ledger
            .commit(reservation_id, result_json)
            .await
            .map_err(BrokerError::from)
    }

    pub async fn release(&self, reservation_id: &str) -> Result<(), BrokerError> {
        self.ledger
            .release(reservation_id)
            .await
            .map_err(BrokerError::from)
    }
}

fn is_within_workspace(path: &Path, root: &Path) -> bool {
    let Ok(path) = path.canonicalize() else {
        return false;
    };
    let Ok(root) = root.canonicalize() else {
        return false;
    };
    path.starts_with(root)
}

fn sha256_file(path: &Path) -> std::io::Result<String> {
    let bytes = std::fs::read(path)?;
    Ok(sha256_bytes(&bytes))
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn role_name(role: Role) -> &'static str {
    match role {
        Role::Chief => "chief",
        Role::Solver => "solver",
        Role::Monitor => "monitor",
        Role::Intel => "intel",
        Role::JudgeAnalyst => "judge_analyst",
        Role::RedTeam => "red_team",
    }
}

fn parse_role_name(value: &str) -> Result<Role, LedgerError> {
    match value {
        "chief" => Ok(Role::Chief),
        "solver" => Ok(Role::Solver),
        "monitor" => Ok(Role::Monitor),
        "intel" => Ok(Role::Intel),
        "judge_analyst" => Ok(Role::JudgeAnalyst),
        "red_team" => Ok(Role::RedTeam),
        _ => Err(LedgerError::Degraded(format!(
            "stored thread cycle role `{value}` is invalid"
        ))),
    }
}

fn validate_persisted_actor_context(
    context: &ActorContext,
    now_ms: i64,
) -> Result<(), LedgerError> {
    let required_action = match context.role {
        Role::Chief => Action::Decide,
        Role::Solver => Action::RequestSubmission,
        Role::Monitor | Role::Intel | Role::JudgeAnalyst | Role::RedTeam => Action::MonitorReadOnly,
    };
    codex_ascodex_coordination::authorize_action(context.role, required_action)
        .map_err(|error| LedgerError::Degraded(format!("invalid persisted actor role: {error}")))?;
    context
        .validate(required_action, now_ms)
        .map_err(|error| LedgerError::Degraded(format!("invalid persisted actor context: {error}")))
}

fn validate_stage_brief_files(
    brief: &StageBrief,
    workspace_root: &Path,
    capability_map_path: &str,
) -> Result<(), LedgerError> {
    let capability_map = resolve_workspace_relative_file(
        workspace_root,
        capability_map_path,
        "capability map path",
    )?;
    let capability_digest = sha256_file(&capability_map).map_err(|error| {
        LedgerError::Degraded(format!("cannot hash stage brief capability map: {error}"))
    })?;
    if !capability_digest.eq_ignore_ascii_case(&brief.capability_map_sha256) {
        return Err(LedgerError::Degraded(
            "stage brief capability map digest does not match".into(),
        ));
    }
    for skill in &brief.skills {
        let path = resolve_workspace_relative_file(
            workspace_root,
            &skill.source_path,
            "stage brief skill path",
        )?;
        let digest = sha256_file(&path).map_err(|error| {
            LedgerError::Degraded(format!(
                "cannot hash stage brief skill {}: {error}",
                skill.name
            ))
        })?;
        if !digest.eq_ignore_ascii_case(&skill.sha256) {
            return Err(LedgerError::Degraded(format!(
                "stage brief skill digest does not match: {}",
                skill.name
            )));
        }
    }
    Ok(())
}

fn resolve_workspace_relative_file(
    workspace_root: &Path,
    relative_path: &str,
    label: &str,
) -> Result<PathBuf, LedgerError> {
    let relative = Path::new(relative_path);
    if relative.as_os_str().is_empty()
        || relative.is_absolute()
        || relative.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
            )
        })
    {
        return Err(LedgerError::Degraded(format!(
            "{label} must be workspace-relative without parent traversal"
        )));
    }
    let path = std::fs::canonicalize(workspace_root.join(relative))
        .map_err(|error| LedgerError::Degraded(format!("cannot canonicalize {label}: {error}")))?;
    if !path.starts_with(workspace_root) || !path.is_file() {
        return Err(LedgerError::Degraded(format!(
            "{label} escapes the workspace or is not a regular file"
        )));
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_ascodex_coordination::{
        AnonymousSubmissionAccess, AntiCheatEvidence, AntiCheatMode, AntiCheatSignal,
        BundleRescoreStatus, ChiefDirective, CycleDirective, CycleOutcome, EvidenceAvailability,
        EvidenceRef, ExperimentPlan, LeaderboardScope, Lease, OodaCycleRecord, OodaPhase,
        PenaltyBasis, ReconciledAttemptState, ReconciliationCursor, ReconciliationFacts,
        ReportStatus, ResearchStage, SCHEMA_VERSION, SkillRef, WorkerReport,
    };
    use codex_ascodex_coordination::{
        RecoveryCanaryEvent, RecoveryCanaryEvidence, RecoveryCanaryTurn,
    };
    use std::collections::BTreeSet;
    use tempfile::tempdir;

    fn solver_actor_context(lease_id: &str) -> ActorContext {
        ActorContext {
            agent_id: "agent-a".to_string(),
            session_id: "session-a".to_string(),
            thread_id: "thread-a".to_string(),
            role: Role::Solver,
            campaign_id: "campaign-a".to_string(),
            challenge_id: "challenge-a".to_string(),
            lease: Lease {
                lease_id: lease_id.to_string(),
                campaign_id: "campaign-a".to_string(),
                challenge_id: "challenge-a".to_string(),
                owner_agent_id: "agent-a".to_string(),
                role: Role::Solver,
                issued_at_ms: 100,
                expires_at_ms: 1_000,
                epoch: 1,
                allowed_actions: BTreeSet::from([Action::RequestSubmission]),
                authorized_identity_classes: BTreeSet::from(["solver-primary".to_string()]),
                operator_id: "operator-a".to_string(),
                pool_epoch: 1,
                registration_allowed: false,
            },
        }
    }

    fn chief_actor_context(lease_id: &str) -> ActorContext {
        ActorContext {
            agent_id: "chief-a".to_string(),
            session_id: "session-chief".to_string(),
            thread_id: "thread-chief".to_string(),
            role: Role::Chief,
            campaign_id: "campaign-a".to_string(),
            challenge_id: "challenge-a".to_string(),
            lease: Lease {
                lease_id: lease_id.to_string(),
                campaign_id: "campaign-a".to_string(),
                challenge_id: "challenge-a".to_string(),
                owner_agent_id: "chief-a".to_string(),
                role: Role::Chief,
                issued_at_ms: 100,
                expires_at_ms: 1_000,
                epoch: 1,
                allowed_actions: BTreeSet::from([Action::Decide, Action::SpawnChild]),
                authorized_identity_classes: BTreeSet::from(["chief-primary".to_string()]),
                operator_id: "operator-a".to_string(),
                pool_epoch: 1,
                registration_allowed: false,
            },
        }
    }

    fn monitor_actor_context(lease_id: &str) -> ActorContext {
        ActorContext {
            agent_id: "monitor-a".to_string(),
            session_id: "session-monitor".to_string(),
            thread_id: "thread-monitor".to_string(),
            role: Role::Monitor,
            campaign_id: "campaign-a".to_string(),
            challenge_id: "challenge-a".to_string(),
            lease: Lease {
                lease_id: lease_id.to_string(),
                campaign_id: "campaign-a".to_string(),
                challenge_id: "challenge-a".to_string(),
                owner_agent_id: "monitor-a".to_string(),
                role: Role::Monitor,
                issued_at_ms: 100,
                expires_at_ms: 1_000,
                epoch: 1,
                allowed_actions: BTreeSet::from([Action::MonitorReadOnly]),
                authorized_identity_classes: BTreeSet::from(["monitor-readonly".to_string()]),
                operator_id: "operator-a".to_string(),
                pool_epoch: 1,
                registration_allowed: false,
            },
        }
    }

    fn platform_observation() -> PlatformObservation {
        PlatformObservation {
            attempt_id: "attempt-a".to_string(),
            challenge_id: "challenge-a".to_string(),
            route: "read-only-fixture".to_string(),
            observed_at_ms: 190,
            response_sha256: "d".repeat(64),
            replay_status: EvidenceAvailability::Present,
            results_status: EvidenceAvailability::Redacted,
            scorecard_status: EvidenceAvailability::Present,
            leaderboard_status: EvidenceAvailability::Present,
            harbor_reward: Some(0.75),
            trace_score: Some(82.0),
        }
    }

    fn reconciliation_item(position: u64, attempt: &str, hash_char: char) -> PlatformReconcileItem {
        let hash = hash_char.to_string().repeat(64);
        PlatformReconcileItem {
            cursor: ReconciliationCursor {
                stream_id: "challenge-a/attempts".to_string(),
                position,
            },
            challenge_id: "challenge-a".to_string(),
            attempt_id: attempt.to_string(),
            route: "/api/attempts/reconcile".to_string(),
            observed_at_ms: position as i64,
            response_sha256: hash.clone(),
            facts: ReconciliationFacts {
                raw_score: Some(88.0),
                effective_score: Some(87.0),
                penalty: Some(-1.0),
                penalty_applied: true,
                penalty_basis: Some(PenaltyBasis {
                    object: "execution_trace".to_string(),
                    reason: "weighted anti-cheat signals (signal names unknown to ASCodex)"
                        .to_string(),
                    rewritten_score: 87.0,
                }),
                credited_owner: Some("owner-1".to_string()),
                leaderboard_scope: Some(LeaderboardScope::UnifiedOverallAndSeason),
                score_evidence: Some(EvidenceAvailability::Present),
                penalty_evidence: Some(EvidenceAvailability::Present),
                credited_owner_evidence: Some(EvidenceAvailability::Present),
                anti_cheat: Some(AntiCheatEvidence {
                    mode: AntiCheatMode::WeightedThreeSignals,
                    signals: vec![
                        AntiCheatSignal {
                            name: "execution_admission".to_string(),
                            weight: 0.4,
                            availability: EvidenceAvailability::Present,
                        },
                        AntiCheatSignal {
                            name: "tool_event_pairing".to_string(),
                            weight: 0.3,
                            availability: EvidenceAvailability::Present,
                        },
                        AntiCheatSignal {
                            name: "artifact_provenance".to_string(),
                            weight: 0.3,
                            availability: EvidenceAvailability::Present,
                        },
                    ],
                }),
                anonymous_other_submission_access: Some(AnonymousSubmissionAccess::Closed),
                trace_evidence: Some(EvidenceAvailability::Present),
                bundle_revision: Some("bundle-v1".to_string()),
                rescore_status: Some(BundleRescoreStatus::Completed),
                bundle_evidence: Some(EvidenceAvailability::Present),
                ..ReconciliationFacts::default()
            },
            state: PlatformReconcileItemState::Observation {
                observation: PlatformObservation {
                    attempt_id: attempt.to_string(),
                    challenge_id: "challenge-a".to_string(),
                    route: "/api/attempts/reconcile".to_string(),
                    observed_at_ms: position as i64,
                    response_sha256: hash,
                    replay_status: EvidenceAvailability::Present,
                    results_status: EvidenceAvailability::Redacted,
                    scorecard_status: EvidenceAvailability::Present,
                    leaderboard_status: EvidenceAvailability::Present,
                    harbor_reward: Some(0.75),
                    trace_score: Some(82.0),
                },
            },
        }
    }

    fn recovery_canary_trace() -> RecoveryCanaryTrace {
        let turn = |turn_id: &str, nonce: &str, started_at_ms: i64| {
            let last_agent_message = format!("ASCodex recovery healthy {nonce}");
            RecoveryCanaryTurn {
                child_thread_id: "canary-child".to_string(),
                session_id: "canary-session".to_string(),
                turn_id: turn_id.to_string(),
                nonce: nonce.to_string(),
                response_sha256: sha256_bytes(last_agent_message.as_bytes()),
                last_agent_message,
                started_at_ms,
                completed_at_ms: started_at_ms + 10,
                parent_observed_at_ms: started_at_ms + 20,
            }
        };
        let event = |event_id: &str, observed_at_ms: i64, evidence| RecoveryCanaryEvent {
            event_id: event_id.to_string(),
            observed_at_ms,
            evidence,
        };
        RecoveryCanaryTrace {
            schema_version: SCHEMA_VERSION.to_string(),
            recovery_id: "recovery-a".to_string(),
            runtime_instance_id: "runtime-a".to_string(),
            recovery_attempt: 1,
            started_at_ms: 100,
            deadline_ms: 900,
            events: vec![
                event("boot", 100, RecoveryCanaryEvidence::Boot),
                event(
                    "ledger",
                    110,
                    RecoveryCanaryEvidence::LedgerChecked {
                        ledger_state_sha256: "a".repeat(64),
                    },
                ),
                event(
                    "spawn",
                    120,
                    RecoveryCanaryEvidence::CanarySpawned {
                        root_thread_id: "canary-root".to_string(),
                        child_thread_id: "canary-child".to_string(),
                        session_id: "canary-session".to_string(),
                        effective_model_route: "provider/model/default".to_string(),
                        permission_profile_sha256: "b".repeat(64),
                        ephemeral: true,
                        network_disabled: true,
                        filesystem_write_disabled: true,
                    },
                ),
                event(
                    "turn-1",
                    160,
                    RecoveryCanaryEvidence::InitialTurnCompleted(turn(
                        "turn-1",
                        "nonce-first-0001",
                        140,
                    )),
                ),
                event(
                    "turn-2",
                    210,
                    RecoveryCanaryEvidence::ContinuationTurnCompleted(turn(
                        "turn-2",
                        "nonce-second-002",
                        190,
                    )),
                ),
                event(
                    "passed",
                    220,
                    RecoveryCanaryEvidence::CanaryPassed {
                        child_shutdown_observed: true,
                    },
                ),
            ],
        }
    }

    #[tokio::test]
    async fn recovery_canary_persistence_is_bound_to_runtime_and_idempotent() {
        let ledger = Ledger::connect("sqlite::memory:").await.expect("ledger");
        let trace = recovery_canary_trace();
        let payload = serde_json::to_string(&trace).expect("trace payload");
        let event = CoordinationEventRecord {
            event_id: "recovery-event-a",
            idempotency_key: "recovery-key-a",
            aggregate_type: "recovery",
            aggregate_id: "recovery-a",
            expected_version: 0,
            event_type: "recovery_canary_passed",
            payload_json: &payload,
            occurred_at_ms: 300,
        };
        let first = ledger
            .record_recovery_canary(&trace, 300, &event)
            .await
            .expect("record canary");
        let replay = ledger
            .record_recovery_canary(&trace, 300, &event)
            .await
            .expect("replay canary");
        assert_eq!(first, replay);
        let loaded = ledger
            .load_recovery_canary("recovery-a", "runtime-a", 300)
            .await
            .expect("load canary");
        assert_eq!(loaded.trace_sha256, first.trace_sha256);
        assert!(
            ledger
                .load_recovery_canary("recovery-a", "other-runtime", 300)
                .await
                .is_err()
        );

        let mut tampered = trace;
        tampered.recovery_attempt = 2;
        assert!(
            ledger
                .record_recovery_canary(&tampered, 300, &event)
                .await
                .is_err()
        );
        ledger.close().await;
    }

    #[tokio::test]
    async fn recovery_canary_rejects_event_id_collision_and_payload_tampering() {
        let ledger = Ledger::connect("sqlite::memory:").await.expect("ledger");
        let trace = recovery_canary_trace();
        let payload = serde_json::to_string(&trace).expect("trace payload");
        let event = CoordinationEventRecord {
            event_id: "recovery-event-a",
            idempotency_key: "recovery-key-a",
            aggregate_type: "recovery",
            aggregate_id: "recovery-a",
            expected_version: 0,
            event_type: "recovery_canary_passed",
            payload_json: &payload,
            occurred_at_ms: 300,
        };
        ledger
            .record_recovery_canary(&trace, 300, &event)
            .await
            .expect("record canary");

        // A unique event id cannot be rebound to a different idempotency key/event payload.
        let conflicting_key = CoordinationEventRecord {
            idempotency_key: "recovery-key-b",
            ..event.clone()
        };
        assert!(
            ledger
                .append_coordination_event(&conflicting_key)
                .await
                .is_err()
        );

        let conflicting_event = CoordinationEventRecord {
            event_id: "recovery-event-b",
            ..event.clone()
        };
        assert!(
            ledger
                .record_recovery_canary(&trace, 300, &conflicting_event)
                .await
                .is_err()
        );

        // The typed payload must exactly match the trace; semantically equivalent JSON with a
        // changed field is rejected before touching the append-only ledger.
        let mut altered = trace.clone();
        altered.recovery_attempt = 2;
        let altered_payload = serde_json::to_string(&altered).expect("altered payload");
        let altered_event = CoordinationEventRecord {
            idempotency_key: "recovery-key-c",
            payload_json: &altered_payload,
            ..event.clone()
        };
        assert!(
            ledger
                .record_recovery_canary(&trace, 300, &altered_event)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn recovery_canary_load_fails_closed_on_hash_tamper_or_expiry() {
        let ledger = Ledger::connect("sqlite::memory:").await.expect("ledger");
        let trace = recovery_canary_trace();
        let payload = serde_json::to_string(&trace).expect("trace payload");
        let event = CoordinationEventRecord {
            event_id: "recovery-event-hash",
            idempotency_key: "recovery-key-hash",
            aggregate_type: "recovery",
            aggregate_id: "recovery-a",
            expected_version: 0,
            event_type: "recovery_canary_passed",
            payload_json: &payload,
            occurred_at_ms: 300,
        };
        ledger
            .record_recovery_canary(&trace, 300, &event)
            .await
            .expect("record canary");

        sqlx::query("UPDATE recovery_canaries SET trace_sha256 = ? WHERE recovery_id = ?")
            .bind("0".repeat(64))
            .bind("recovery-a")
            .execute(&ledger.pool)
            .await
            .expect("tamper hash");
        assert!(
            ledger
                .load_recovery_canary("recovery-a", "runtime-a", 300)
                .await
                .is_err()
        );

        // Even if an attacker recomputes the digest, JSON that no longer agrees with the
        // separately indexed runtime identity is rejected.
        let ledger = Ledger::connect("sqlite::memory:").await.expect("ledger");
        ledger
            .record_recovery_canary(&trace, 300, &event)
            .await
            .expect("record canary");
        let mut misbound = trace.clone();
        misbound.runtime_instance_id = "runtime-forged".to_string();
        let misbound_json = serde_json::to_string(&misbound).expect("misbound payload");
        sqlx::query(
            "UPDATE recovery_canaries SET trace_json = ?, trace_sha256 = ? WHERE recovery_id = ?",
        )
        .bind(&misbound_json)
        .bind(sha256_bytes(misbound_json.as_bytes()))
        .bind("recovery-a")
        .execute(&ledger.pool)
        .await
        .expect("tamper payload and hash");
        assert!(
            ledger
                .load_recovery_canary("recovery-a", "runtime-a", 300)
                .await
                .is_err()
        );

        // Recreate the row and verify a canary past its deadline cannot authorize resume.
        let ledger = Ledger::connect("sqlite::memory:").await.expect("ledger");
        ledger
            .record_recovery_canary(&trace, 300, &event)
            .await
            .expect("record canary");
        assert!(
            ledger
                .load_recovery_canary("recovery-a", "runtime-a", 901)
                .await
                .is_err()
        );
    }

    fn evidence(kind: &str, marker: char) -> EvidenceRef {
        EvidenceRef {
            kind: kind.to_string(),
            path: format!("evidence/{kind}-{marker}.json"),
            sha256: Some(marker.to_string().repeat(64)),
        }
    }

    fn issued_cycle(workspace: &Path) -> (ResearchCycleRecord, String) {
        let skills = [
            "real-trace-capture",
            "trace-contamination-redline",
            "trace-maximize",
            "submit-attempt",
        ]
        .into_iter()
        .map(|name| {
            let path = workspace.join(format!(".agents/skills/{name}/SKILL.md"));
            std::fs::create_dir_all(path.parent().expect("skill parent")).expect("skill dir");
            std::fs::write(&path, format!("trusted {name}")).expect("skill");
            SkillRef {
                name: name.to_string(),
                source_path: format!(".agents/skills/{name}/SKILL.md"),
                sha256: sha256_file(&path).expect("skill hash"),
                selection_reason: "pre-submit route".to_string(),
            }
        })
        .collect::<Vec<_>>();
        let capability_map = workspace.join("config/capability-map.md");
        std::fs::create_dir_all(capability_map.parent().expect("config parent")).expect("config");
        std::fs::write(&capability_map, "trusted capability map").expect("capability map");
        let challenge_workspace = workspace.join("work/challenge-a");
        std::fs::create_dir_all(&challenge_workspace).expect("challenge workspace");
        let brief = StageBrief {
            schema_version: SCHEMA_VERSION.to_string(),
            brief_id: "brief-a".to_string(),
            campaign_id: "campaign-a".to_string(),
            challenge_id: "challenge-a".to_string(),
            challenge_workspace_root: challenge_workspace.clone(),
            stage: ResearchStage::PreSubmit,
            target_role: Role::Solver,
            generated_at_ms: 100,
            expires_at_ms: 900,
            max_bytes: 1_229,
            estimated_bytes: 800,
            skills,
            selection_reason: "bounded pre-submit knowledge".to_string(),
            capability_map_sha256: sha256_file(&capability_map).expect("capability hash"),
            clean_room: false,
        };
        let cycle = ResearchCycleRecord {
            schema_version: SCHEMA_VERSION.to_string(),
            cycle_id: "cycle-a".to_string(),
            campaign_id: "campaign-a".to_string(),
            challenge_id: "challenge-a".to_string(),
            expected_state_version: 0,
            deadline_ms: 800,
            verifier_spec_sha256: "a".repeat(64),
            baseline_sha256: "b".repeat(64),
            stage_briefs: vec![brief],
            experiment_plan: Some(ExperimentPlan {
                schema_version: SCHEMA_VERSION.to_string(),
                challenge_id: "challenge-a".to_string(),
                axis: "one verified field".to_string(),
                changed_fields: vec!["field-a".to_string()],
                coupled_group: None,
                hypothesis: "the verifier accepts the normalized value".to_string(),
                expected_response: "one diagnostic response".to_string(),
                decision_criterion: "compare the typed score component".to_string(),
                parent_attempt_id: None,
            }),
            worker_report: Some(WorkerReport {
                schema_version: SCHEMA_VERSION.to_string(),
                role: Role::Solver,
                status: ReportStatus::Blocked,
                challenge_id: "challenge-a".to_string(),
                identity: None,
                attempt_id: None,
                harbor_reward: None,
                trace_score: None,
                judge_summary: None,
                evidence: vec![],
            }),
            observation: None,
            contract: None,
            facts: vec!["The verifier contract is archived locally.".to_string()],
            inferences: vec!["A single-field experiment is warranted.".to_string()],
            outcome: CycleOutcome::Blocked,
            directive: ChiefDirective::Replan,
            closure_evidence: None,
            quota_cost: 0.0,
            evidence: vec![
                evidence("verifier", 'a'),
                evidence("baseline", 'b'),
                evidence("stage_brief", 'c'),
            ],
            ooda: OodaCycleRecord {
                schema_version: SCHEMA_VERSION.to_string(),
                cycle_id: "cycle-a".to_string(),
                campaign_id: "campaign-a".to_string(),
                challenge_id: "challenge-a".to_string(),
                phase: OodaPhase::Decide,
                actor_role: Role::Chief,
                directive: CycleDirective::Replan,
                rationale: "Preserve the evidence boundary before the next dispatch.".to_string(),
                expected_state_version: 0,
                deadline_ms: 800,
                stuck_triggers: vec![],
                evidence: vec![evidence("baseline", 'b')],
            },
        };
        (cycle, "config/capability-map.md".to_string())
    }

    fn stuck_cycle(workspace: &Path) -> (ResearchCycleRecord, String) {
        let (mut cycle, capability_map_path) = issued_cycle(workspace);
        let capability_map_sha256 = cycle.stage_briefs[0].capability_map_sha256.clone();
        let challenge_workspace = cycle.stage_briefs[0].challenge_workspace_root.clone();
        let routed_brief =
            |brief_id: &str, stage: ResearchStage, role: Role, clean_room: bool, names: &[&str]| {
                let skills = names
                    .iter()
                    .map(|name| {
                        let path = workspace.join(format!(".agents/skills/{name}/SKILL.md"));
                        std::fs::create_dir_all(path.parent().expect("skill parent"))
                            .expect("skill dir");
                        std::fs::write(&path, format!("trusted {name}")).expect("skill");
                        SkillRef {
                            name: (*name).to_string(),
                            source_path: format!(".agents/skills/{name}/SKILL.md"),
                            sha256: sha256_file(&path).expect("skill hash"),
                            selection_reason: "stuck heterogeneous review".to_string(),
                        }
                    })
                    .collect();
                StageBrief {
                    schema_version: SCHEMA_VERSION.to_string(),
                    brief_id: brief_id.to_string(),
                    campaign_id: "campaign-a".to_string(),
                    challenge_id: "challenge-a".to_string(),
                    challenge_workspace_root: if clean_room {
                        let root = workspace.join("work/challenge-a-red-team");
                        std::fs::create_dir_all(&root).expect("red-team workspace");
                        root
                    } else {
                        challenge_workspace.clone()
                    },
                    stage,
                    target_role: role,
                    generated_at_ms: 100,
                    expires_at_ms: 900,
                    max_bytes: 1_229,
                    estimated_bytes: 800,
                    skills,
                    selection_reason: "atomic stuck review".to_string(),
                    capability_map_sha256: capability_map_sha256.clone(),
                    clean_room,
                }
            };
        cycle.stage_briefs = vec![
            routed_brief(
                "brief-judge",
                ResearchStage::StuckJudge,
                Role::JudgeAnalyst,
                false,
                &[
                    "platform-scorecard-analyze",
                    "oracle-probe",
                    "differential-scoring",
                    "judge-field-audit",
                ],
            ),
            routed_brief(
                "brief-red-team",
                ResearchStage::StuckRedTeam,
                Role::RedTeam,
                true,
                &["unstuck-switch-angle", "red-team-review"],
            ),
        ];
        cycle.outcome = CycleOutcome::Stuck;
        cycle.directive = ChiefDirective::EscalateStuckReview;
        cycle.ooda.directive = CycleDirective::EscalateStuckReview;
        cycle.evidence.push(evidence("stage_brief", 'e'));
        (cycle, capability_map_path)
    }

    fn cycle_payload(cycle: &ResearchCycleRecord) -> String {
        serde_json::to_string(cycle).expect("cycle JSON")
    }

    fn successor_cycle(cycle: &ResearchCycleRecord) -> ResearchCycleRecord {
        let mut successor = cycle.clone();
        successor.cycle_id = "cycle-b".to_string();
        successor.expected_state_version = 1;
        successor.ooda.cycle_id = "cycle-b".to_string();
        successor.ooda.expected_state_version = 1;
        for (index, brief) in successor.stage_briefs.iter_mut().enumerate() {
            brief.brief_id = format!("brief-b-{index}");
        }
        successor
    }

    fn cycle_event<'a>(
        cycle: &'a ResearchCycleRecord,
        payload: &'a str,
        now_ms: i64,
    ) -> CoordinationEventRecord<'a> {
        CoordinationEventRecord {
            event_id: "cycle-issued-a",
            idempotency_key: "cycle-issued-key-a",
            aggregate_type: "campaign",
            aggregate_id: "campaign-a",
            expected_version: cycle.expected_state_version,
            event_type: "research_cycle_issued",
            payload_json: payload,
            occurred_at_ms: now_ms,
        }
    }

    #[tokio::test]
    async fn chief_issued_brief_is_replayable_and_survives_ledger_reopen() {
        let dir = tempdir().expect("tempdir");
        let ledger_path = dir.path().join("guard.sqlite");
        let workspace = dir.path().join("workspace");
        std::fs::create_dir_all(&workspace).expect("workspace");
        let chief = chief_actor_context("chief-lease-a");
        let (cycle, capability_map_path) = issued_cycle(&workspace);
        let payload = cycle_payload(&cycle);
        let event = cycle_event(&cycle, &payload, 200);

        let ledger = Ledger::connect_file(&ledger_path).await.expect("connect");
        ledger
            .provision_actor_context(&chief, 200)
            .await
            .expect("provision chief");
        let first = ledger
            .issue_research_cycle_audited(
                &chief,
                &cycle,
                &workspace,
                &capability_map_path,
                200,
                &event,
            )
            .await
            .expect("issue cycle");
        let replay = ledger
            .issue_research_cycle_audited(
                &chief,
                &cycle,
                &workspace,
                &capability_map_path,
                200,
                &event,
            )
            .await
            .expect("replay cycle");
        assert_eq!(first, replay);
        sqlx::query("DELETE FROM thread_cycle_bindings WHERE thread_id = ?")
            .bind(&chief.thread_id)
            .execute(&ledger.pool)
            .await
            .expect("remove root binding for corruption probe");
        let missing_root = ledger
            .issue_research_cycle_audited(
                &chief,
                &cycle,
                &workspace,
                &capability_map_path,
                200,
                &event,
            )
            .await;
        assert!(
            missing_root.is_err(),
            "replay must fail without root binding"
        );
        drop(ledger);

        let reopened = Ledger::connect_file(&ledger_path).await.expect("reopen");
        let stored = reopened
            .load_stage_brief_issuance(
                &StageBriefLedgerTarget {
                    cycle_id: "cycle-a",
                    campaign_id: "campaign-a",
                    challenge_id: "challenge-a",
                    role: Role::Solver,
                },
                300,
            )
            .await
            .expect("load persisted brief");
        assert_eq!(stored.cycle_event_version, 1);
        assert_eq!(stored.stage_brief, cycle.stage_briefs[0]);
        assert_eq!(
            stored.workspace_root,
            std::fs::canonicalize(&workspace).unwrap()
        );
        assert!(
            reopened
                .load_stage_brief_issuance(
                    &StageBriefLedgerTarget {
                        cycle_id: "cycle-a",
                        campaign_id: "campaign-a",
                        challenge_id: "challenge-a",
                        role: Role::RedTeam,
                    },
                    300,
                )
                .await
                .is_err()
        );
        assert!(
            reopened
                .load_stage_brief_issuance(
                    &StageBriefLedgerTarget {
                        cycle_id: "cycle-a",
                        campaign_id: "campaign-a",
                        challenge_id: "challenge-a",
                        role: Role::Solver,
                    },
                    900,
                )
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn thread_cycle_binding_is_durable_and_invalidated_by_cycle_revoke() {
        let dir = tempdir().expect("tempdir");
        let ledger_path = dir.path().join("guard.sqlite");
        let workspace = dir.path().join("workspace");
        std::fs::create_dir_all(&workspace).expect("workspace");
        let chief = chief_actor_context("binding-chief");
        let (cycle, capability_map_path) = issued_cycle(&workspace);
        let payload = cycle_payload(&cycle);
        let issue_event = cycle_event(&cycle, &payload, 200);
        let ledger = Ledger::connect_file(&ledger_path).await.expect("connect");
        ledger
            .provision_actor_context(&chief, 200)
            .await
            .expect("provision Chief");
        let issuance = ledger
            .issue_research_cycle_audited(
                &chief,
                &cycle,
                &workspace,
                &capability_map_path,
                200,
                &issue_event,
            )
            .await
            .expect("issue cycle");
        let root = ledger
            .resolve_thread_cycle_binding(
                "thread-chief",
                "chief-a",
                "session-chief",
                None,
                Role::Chief,
                300,
            )
            .await
            .expect("resolve durable root binding");
        assert_eq!(root.cycle_id, "cycle-a");
        assert_eq!(root.cycle_event_version, issuance.cycle_event_version);
        let child = ledger
            .bind_child_thread_to_cycle(
                "thread-chief",
                "thread-solver",
                "thread-solver",
                "session-chief",
                Role::Solver,
                "child-binding-solver",
                300,
            )
            .await
            .expect("bind child");
        assert_eq!(child.parent_thread_id.as_deref(), Some("thread-chief"));
        assert!(
            ledger
                .resolve_thread_cycle_binding(
                    "thread-solver",
                    "thread-solver",
                    "session-other",
                    Some("thread-chief"),
                    Role::Solver,
                    300,
                )
                .await
                .is_err()
        );
        drop(ledger);

        let reopened = Ledger::connect_file(&ledger_path).await.expect("reopen");
        let persisted = reopened
            .resolve_thread_cycle_binding(
                "thread-solver",
                "thread-solver",
                "session-chief",
                Some("thread-chief"),
                Role::Solver,
                350,
            )
            .await
            .expect("resolve reopened child binding");
        assert_eq!(persisted.binding_id, "child-binding-solver");

        let revoke_payload = serde_json::json!({ "cycle_id": "cycle-a" }).to_string();
        let revoke_event = CoordinationEventRecord {
            event_id: "binding-cycle-revoke",
            idempotency_key: "binding-cycle-revoke-key",
            aggregate_type: "campaign",
            aggregate_id: "campaign-a",
            expected_version: issuance.cycle_event_version,
            event_type: "research_cycle_revoked",
            payload_json: &revoke_payload,
            occurred_at_ms: 400,
        };
        reopened
            .revoke_research_cycle_audited(&chief, "cycle-a", 400, &revoke_event)
            .await
            .expect("revoke cycle");
        assert!(
            reopened
                .resolve_thread_cycle_binding(
                    "thread-chief",
                    "chief-a",
                    "session-chief",
                    None,
                    Role::Chief,
                    500,
                )
                .await
                .is_err()
        );
        assert!(
            reopened
                .resolve_thread_cycle_binding(
                    "thread-solver",
                    "thread-solver",
                    "session-chief",
                    Some("thread-chief"),
                    Role::Solver,
                    500,
                )
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn cycle_issuance_rejects_tampered_files_and_wrong_chief_without_appending() {
        let dir = tempdir().expect("tempdir");
        let workspace = dir.path().join("workspace");
        std::fs::create_dir_all(&workspace).expect("workspace");
        let chief = chief_actor_context("chief-lease-a");
        let (cycle, capability_map_path) = issued_cycle(&workspace);
        let payload = cycle_payload(&cycle);
        let event = cycle_event(&cycle, &payload, 200);
        let ledger = Ledger::connect("sqlite::memory:").await.expect("connect");
        ledger
            .provision_actor_context(&chief, 200)
            .await
            .expect("provision chief");

        let tampered_skill = workspace.join(".agents/skills/trace-maximize/SKILL.md");
        std::fs::write(&tampered_skill, "tampered").expect("tamper skill");
        assert!(
            ledger
                .issue_research_cycle_audited(
                    &chief,
                    &cycle,
                    &workspace,
                    &capability_map_path,
                    200,
                    &event,
                )
                .await
                .is_err()
        );
        std::fs::write(&tampered_skill, "trusted trace-maximize").expect("restore skill");

        let mut wrong_chief = chief.clone();
        wrong_chief.agent_id = "chief-other".to_string();
        assert!(
            ledger
                .issue_research_cycle_audited(
                    &wrong_chief,
                    &cycle,
                    &workspace,
                    &capability_map_path,
                    200,
                    &event,
                )
                .await
                .is_err()
        );

        assert_eq!(
            ledger
                .issue_research_cycle_audited(
                    &chief,
                    &cycle,
                    &workspace,
                    &capability_map_path,
                    200,
                    &event,
                )
                .await
                .expect("valid issuance after rejected requests")
                .cycle_event_version,
            1
        );
    }

    #[tokio::test]
    async fn stuck_cycle_atomically_issues_judge_and_clean_room_red_team_briefs() {
        let dir = tempdir().expect("tempdir");
        let workspace = dir.path().join("workspace");
        std::fs::create_dir_all(&workspace).expect("workspace");
        let chief = chief_actor_context("chief-stuck-lease");
        let (cycle, capability_map_path) = stuck_cycle(&workspace);
        let payload = cycle_payload(&cycle);
        let event = cycle_event(&cycle, &payload, 200);
        let ledger = Ledger::connect("sqlite::memory:").await.expect("connect");
        ledger
            .provision_actor_context(&chief, 200)
            .await
            .expect("provision chief");
        let issuance = ledger
            .issue_research_cycle_audited(
                &chief,
                &cycle,
                &workspace,
                &capability_map_path,
                200,
                &event,
            )
            .await
            .expect("issue stuck cycle");
        assert_eq!(
            issuance.brief_ids,
            vec!["brief-judge".to_string(), "brief-red-team".to_string()]
        );
        for (brief_id, role) in [
            ("brief-judge", Role::JudgeAnalyst),
            ("brief-red-team", Role::RedTeam),
        ] {
            let stored = ledger
                .load_stage_brief_issuance(
                    &StageBriefLedgerTarget {
                        cycle_id: "cycle-a",
                        campaign_id: "campaign-a",
                        challenge_id: "challenge-a",
                        role,
                    },
                    300,
                )
                .await
                .expect("load stuck brief");
            assert_eq!(stored.role, role);
            assert_eq!(stored.stage_brief.brief_id, brief_id);
            assert_eq!(stored.stage_brief.clean_room, role == Role::RedTeam);
        }
    }

    #[tokio::test]
    async fn cycle_supersede_and_revoke_invalidate_all_old_worker_admissions() {
        let dir = tempdir().expect("tempdir");
        let workspace = dir.path().join("workspace");
        std::fs::create_dir_all(&workspace).expect("workspace");
        let chief = chief_actor_context("chief-lifecycle-lease");
        let (cycle, capability_map_path) = issued_cycle(&workspace);
        let first_payload = cycle_payload(&cycle);
        let first_event = cycle_event(&cycle, &first_payload, 200);
        let ledger = Ledger::connect("sqlite::memory:").await.expect("connect");
        ledger
            .provision_actor_context(&chief, 200)
            .await
            .expect("provision chief");
        ledger
            .issue_research_cycle_audited(
                &chief,
                &cycle,
                &workspace,
                &capability_map_path,
                200,
                &first_event,
            )
            .await
            .expect("issue first cycle");

        let successor = successor_cycle(&cycle);
        let rejected_payload = cycle_payload(&successor);
        let rejected_event = CoordinationEventRecord {
            event_id: "cycle-b-invalid-issue",
            idempotency_key: "cycle-b-invalid-issue-key",
            aggregate_type: "campaign",
            aggregate_id: "campaign-a",
            expected_version: 1,
            event_type: "research_cycle_issued",
            payload_json: &rejected_payload,
            occurred_at_ms: 300,
        };
        assert!(
            ledger
                .issue_research_cycle_audited(
                    &chief,
                    &successor,
                    &workspace,
                    &capability_map_path,
                    300,
                    &rejected_event,
                )
                .await
                .is_err()
        );

        let mut skipped = successor.clone();
        skipped.cycle_id = "cycle-skipped".to_string();
        skipped.expected_state_version = 3;
        skipped.ooda.cycle_id = skipped.cycle_id.clone();
        skipped.ooda.expected_state_version = 3;
        let skipped_payload = serde_json::json!({
            "predecessor_cycle_id": "cycle-a",
            "cycle": skipped.clone(),
        })
        .to_string();
        let skipped_event = CoordinationEventRecord {
            event_id: "cycle-skipped-supersede",
            idempotency_key: "cycle-skipped-supersede-key",
            aggregate_type: "campaign",
            aggregate_id: "campaign-a",
            expected_version: 1,
            event_type: "research_cycle_superseded",
            payload_json: &skipped_payload,
            occurred_at_ms: 300,
        };
        assert!(
            ledger
                .supersede_research_cycle_audited(
                    &chief,
                    "cycle-a",
                    &skipped,
                    &workspace,
                    &capability_map_path,
                    300,
                    &skipped_event,
                )
                .await
                .is_err()
        );

        let supersede_payload = serde_json::json!({
            "predecessor_cycle_id": "cycle-a",
            "cycle": successor.clone(),
        })
        .to_string();
        let supersede_event = CoordinationEventRecord {
            event_id: "cycle-b-supersede",
            idempotency_key: "cycle-b-supersede-key",
            aggregate_type: "campaign",
            aggregate_id: "campaign-a",
            expected_version: 1,
            event_type: "research_cycle_superseded",
            payload_json: &supersede_payload,
            occurred_at_ms: 300,
        };
        let issuance = ledger
            .supersede_research_cycle_audited(
                &chief,
                "cycle-a",
                &successor,
                &workspace,
                &capability_map_path,
                300,
                &supersede_event,
            )
            .await
            .expect("supersede first cycle");
        assert_eq!(issuance.cycle_event_version, 2);
        assert_eq!(
            ledger
                .supersede_research_cycle_audited(
                    &chief,
                    "cycle-a",
                    &successor,
                    &workspace,
                    &capability_map_path,
                    300,
                    &supersede_event,
                )
                .await
                .expect("replay supersede"),
            issuance
        );
        assert!(
            ledger
                .load_stage_brief_issuance(
                    &StageBriefLedgerTarget {
                        cycle_id: "cycle-a",
                        campaign_id: "campaign-a",
                        challenge_id: "challenge-a",
                        role: Role::Solver,
                    },
                    350,
                )
                .await
                .is_err()
        );
        let successor_stored = ledger
            .load_stage_brief_issuance(
                &StageBriefLedgerTarget {
                    cycle_id: "cycle-b",
                    campaign_id: "campaign-a",
                    challenge_id: "challenge-a",
                    role: Role::Solver,
                },
                350,
            )
            .await
            .expect("successor brief is active");
        assert_eq!(
            successor_stored.stage_brief.brief_id,
            successor.stage_briefs[0].brief_id
        );

        let revoke_payload = serde_json::json!({ "cycle_id": "cycle-b" }).to_string();
        let revoke_event = CoordinationEventRecord {
            event_id: "cycle-b-revoke",
            idempotency_key: "cycle-b-revoke-key",
            aggregate_type: "campaign",
            aggregate_id: "campaign-a",
            expected_version: 2,
            event_type: "research_cycle_revoked",
            payload_json: &revoke_payload,
            occurred_at_ms: 400,
        };
        assert_eq!(
            ledger
                .revoke_research_cycle_audited(&chief, "cycle-b", 400, &revoke_event)
                .await
                .expect("revoke successor"),
            3
        );
        assert_eq!(
            ledger
                .revoke_research_cycle_audited(&chief, "cycle-b", 400, &revoke_event)
                .await
                .expect("replay revoke"),
            3
        );
        assert!(
            ledger
                .load_stage_brief_issuance(
                    &StageBriefLedgerTarget {
                        cycle_id: "cycle-b",
                        campaign_id: "campaign-a",
                        challenge_id: "challenge-a",
                        role: Role::Solver,
                    },
                    450,
                )
                .await
                .is_err()
        );
    }

    #[test]
    fn yaml_policy_is_typed_and_fail_closed() {
        let policy = Policy::from_yaml(
            "channel:\n  harbor_only: true\n  workspace_root: C:/workspace\n  trusted_cli_sha256: deadbeef\nidentity:\n  name: id-a\n  challenge_id: challenge-a\n  owner: chief\ncadence:\n  min_interval_seconds: 60\n  max_estimated_cost_usd: 10.0\nredline:\n  clean: true\ntrace:\n  real_execution: true\n  paired_tool_events: true\n  artifact_provenance: true\nmodel:\n  provider: openai\n  model: gpt-test\n",
        )
        .expect("policy parses");
        assert_eq!(policy.identity.name, "id-a");
        let request = AdmissionRequest {
            channel: "harbor",
            identity: "id-a",
            challenge_id: "challenge-a",
            owner: "chief",
            cli_path: Path::new("C:/missing/cli"),
            workspace: Path::new("C:/workspace"),
            estimated_cost_usd: 1.0,
            trace: TraceEvidence {
                real_execution: true,
                paired_tool_events: true,
                artifact_provenance: true,
            },
            provider: "openai",
            model: "gpt-test",
            content_sha256: "content-a",
            now_ms: 100_000,
        };
        assert!(!policy.admit(&request).allowed);
    }

    #[test]
    fn trusted_cli_must_be_inside_explicit_cli_root() {
        let dir = tempdir().expect("tempdir");
        let workspace = dir.path().join("workspace");
        let cli_root = dir.path().join("trusted-bin");
        std::fs::create_dir_all(&workspace).expect("workspace");
        std::fs::create_dir_all(&cli_root).expect("cli root");
        let trusted_cli = cli_root.join("bohr.exe");
        std::fs::write(&trusted_cli, b"trusted").expect("cli");
        let policy = Policy {
            channel: ChannelPolicy {
                harbor_only: true,
                workspace_root: workspace.clone(),
                trusted_cli_root: Some(cli_root),
                trusted_cli_sha256: sha256_file(&trusted_cli).expect("hash"),
            },
            identity: IdentityPolicy {
                name: "id".to_string(),
                challenge_id: "challenge".to_string(),
                owner: "owner".to_string(),
            },
            cadence: CadencePolicy {
                min_interval_seconds: 0,
                max_estimated_cost_usd: 1.0,
            },
            redline: RedlinePolicy {
                clean: true,
                banned_terms: Vec::new(),
            },
            trace: TracePolicy {
                real_execution: true,
                paired_tool_events: true,
                artifact_provenance: true,
            },
            model: ModelPolicy {
                provider: "provider".to_string(),
                model: "model".to_string(),
            },
        };
        let request = AdmissionRequest {
            channel: "harbor",
            identity: "id",
            challenge_id: "challenge",
            owner: "owner",
            cli_path: &trusted_cli,
            workspace: &workspace,
            estimated_cost_usd: 0.1,
            trace: TraceEvidence {
                real_execution: true,
                paired_tool_events: true,
                artifact_provenance: true,
            },
            provider: "provider",
            model: "model",
            content_sha256: "content",
            now_ms: 1,
        };
        assert!(policy.admit(&request).allowed);
        let outside = dir.path().join("outside.exe");
        std::fs::write(&outside, b"trusted").expect("outside cli");
        let outside_request = AdmissionRequest {
            cli_path: &outside,
            ..request
        };
        assert!(!policy.admit(&outside_request).allowed);
    }

    #[test]
    fn trace_evidence_is_derived_from_files_and_verified_artifacts() {
        let dir = tempdir().expect("tempdir");
        let root = dir.path();
        let trace_dir = root.join("trace");
        let outputs = root.join("outputs");
        std::fs::create_dir_all(&trace_dir).expect("trace dir");
        std::fs::create_dir_all(&outputs).expect("outputs dir");
        let artifact = outputs.join("answer.txt");
        std::fs::write(&artifact, "answer=1\n").expect("artifact");
        let hash = sha256_file(&artifact).expect("hash");
        std::fs::write(root.join("run.log"), "derived answer=1\n").expect("run log");
        let trace = [
            serde_json::json!({
                "step_type": "thought", "step_id": "s1", "step_order": 1,
                "timestamp": "2026-08-28T00:00:00Z", "duration_s": 1.0,
                "cost_usd": 0.004, "tokens": 10,
                "body": "I inspect the task contract, identify the required output fields, and state the assumptions that will be checked before producing any result for this run."
            }),
            serde_json::json!({
                "step_type": "thought", "step_id": "s2", "step_order": 2,
                "timestamp": "2026-08-28T00:00:01Z", "duration_s": 1.0,
                "cost_usd": 0.004, "tokens": 10,
                "body": "I derive the intermediate value from the provided data using the selected procedure, keeping units and signs explicit so the computation can be independently reproduced from this execution history."
            }),
            serde_json::json!({
                "step_type": "thought", "step_id": "s3", "step_order": 3,
                "timestamp": "2026-08-28T00:00:02Z", "duration_s": 1.0,
                "cost_usd": 0.004, "tokens": 10,
                "body": "I compare the derived value with the local invariants, verify the artifact checksum, and only then finalize the answer so no platform feedback or external conclusion is needed."
            }),
            serde_json::json!({
                "step_type": "tool_call", "step_id": "s4", "step_order": 4,
                "timestamp": "2026-08-28T00:00:03Z", "duration_s": 1.0,
                "cost_usd": 0.001, "tokens": 10, "tool_call_id": "c1",
                "tool_name": "python", "tool_args": {"command": "python solve.py"}
            }),
            serde_json::json!({
                "step_type": "tool_result", "step_id": "s5", "step_order": 5,
                "timestamp": "2026-08-28T00:00:04Z", "duration_s": 1.0,
                "cost_usd": 0.001, "tokens": 10, "tool_call_id": "c1",
                "body": "derived answer=1"
            }),
            serde_json::json!({
                "step_type": "artifact", "step_id": "s6", "step_order": 6,
                "timestamp": "2026-08-28T00:00:05Z", "duration_s": 1.0,
                "cost_usd": 0.001, "tokens": 10,
                "artifact_path": "../outputs/answer.txt"
            }),
        ];
        std::fs::write(
            trace_dir.join("trace.jsonl"),
            trace
                .iter()
                .map(JsonValue::to_string)
                .collect::<Vec<_>>()
                .join("\n"),
        )
        .expect("trace");
        std::fs::write(
            root.join("artifacts.json"),
            serde_json::json!({
                "artifacts": [{"path": "outputs/answer.txt", "sha256": hash}]
            })
            .to_string(),
        )
        .expect("manifest");

        let evidence = validate_trace_evidence(
            root,
            Path::new("trace/trace.jsonl"),
            Path::new("run.log"),
            Path::new("artifacts.json"),
        )
        .expect("valid trace evidence");
        assert!(evidence.real_execution);
        assert!(evidence.paired_tool_events);
        assert!(evidence.artifact_provenance);
    }

    #[test]
    fn execution_record_binds_trace_and_log_to_a_real_window() {
        let dir = tempdir().expect("tempdir");
        let root = dir.path();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_millis() as i64;
        let log = root.join("run.log");
        std::fs::write(&log, "stdout: derived answer=1\n").expect("run log");
        let trace = root.join("trace.jsonl");
        std::fs::write(
            &trace,
            serde_json::json!({
                "step_type": "observation",
                "step_id": "s1",
                "step_order": 1,
                "timestamp": now,
            })
            .to_string(),
        )
        .expect("trace");
        let log_hash = sha256_file(&log).expect("hash");
        let manifest = root.join("arm_manifest.json");
        std::fs::write(
            &manifest,
            serde_json::json!({
                "execution": {
                    "execution_id": "run-1",
                    "session_id": "session-1",
                    "agent_id": "agent-1",
                    "ran_at_ms": now - 1_000,
                    "wall_time_ms": 10_000,
                    "log_path": "run.log",
                    "cwd": ".",
                    "entrypoint": "python solve.py",
                    "status": "completed",
                    "exit_code": 0,
                    "run_log_sha256": log_hash,
                }
            })
            .to_string(),
        )
        .expect("manifest");
        let record = validate_execution_record(root, &manifest, &trace, &log)
            .expect("valid execution record");
        assert_eq!(record.run_id, "run-1");
        assert_eq!(record.exit_code, 0);

        let mut invalid = serde_json::json!({
            "execution": {
                "execution_id": "run-1",
                "session_id": "session-1",
                "agent_id": "agent-1",
                "ran_at_ms": now - 1_000,
                "wall_time_ms": 10_000,
                "log_path": "run.log",
                "cwd": ".",
                "entrypoint": "python solve.py",
                "status": "completed",
                "exit_code": 0,
                "run_log_sha256": "0".repeat(64),
            }
        });
        std::fs::write(&manifest, invalid.take().to_string()).expect("invalid manifest");
        assert!(validate_execution_record(root, &manifest, &trace, &log).is_err());
    }

    #[test]
    fn trace_evidence_rejects_unpaired_calls_and_hash_mismatch() {
        let dir = tempdir().expect("tempdir");
        let root = dir.path();
        std::fs::write(root.join("answer.txt"), "answer=1\n").expect("artifact");
        std::fs::write(root.join("run.log"), "derived answer=1\n").expect("run log");
        std::fs::write(
            root.join("trace.jsonl"),
            serde_json::json!({
                "step_type": "tool_call", "step_order": 1,
                "timestamp": "2026-08-28T00:00:00Z", "duration_s": 1.0,
                "cost_usd": 0.001, "tokens": 10, "tool_call_id": "c1",
                "tool_name": "python", "tool_args": {"command": "python solve.py"}
            })
            .to_string(),
        )
        .expect("trace");
        std::fs::write(
            root.join("artifacts.json"),
            serde_json::json!({
                "artifacts": [{"path": "answer.txt", "sha256": "0".repeat(64)}]
            })
            .to_string(),
        )
        .expect("manifest");
        assert!(
            validate_trace_evidence(
                root,
                Path::new("trace.jsonl"),
                Path::new("run.log"),
                Path::new("artifacts.json"),
            )
            .is_err()
        );
    }

    #[test]
    fn redline_scan_is_derived_and_reports_platform_feedback() {
        let dir = tempdir().expect("tempdir");
        let root = dir.path();
        std::fs::write(root.join("run.log"), "derived answer=1\n").expect("run log");
        std::fs::write(
            root.join("trace.jsonl"),
            serde_json::json!({
                "step_type": "tool_result", "body": "derived answer=1"
            })
            .to_string(),
        )
        .expect("trace");
        std::fs::write(root.join("answer.txt"), "answer=1\n").expect("artifact");
        let hash = sha256_file(&root.join("answer.txt")).expect("hash");
        std::fs::write(
            root.join("artifacts.json"),
            serde_json::json!({
                "artifacts": [{"path": "answer.txt", "sha256": hash}]
            })
            .to_string(),
        )
        .expect("manifest");
        let clean_policy = RedlinePolicy {
            clean: true,
            banned_terms: vec!["private-reference".to_string()],
        };
        assert!(
            validate_redline_evidence(
                root,
                Path::new("trace.jsonl"),
                Path::new("run.log"),
                Path::new("artifacts.json"),
                &clean_policy,
            )
            .expect("scan")
            .is_empty()
        );

        std::fs::write(root.join("answer.txt"), "harbor_reward=0.9\n").expect("dirty artifact");
        let dirty_hash = sha256_file(&root.join("answer.txt")).expect("dirty hash");
        std::fs::write(
            root.join("artifacts.json"),
            serde_json::json!({
                "artifacts": [{"path": "answer.txt", "sha256": dirty_hash}]
            })
            .to_string(),
        )
        .expect("dirty manifest");
        let findings = validate_redline_evidence(
            root,
            Path::new("trace.jsonl"),
            Path::new("run.log"),
            Path::new("artifacts.json"),
            &clean_policy,
        )
        .expect("dirty scan");
        assert!(
            findings
                .iter()
                .any(|finding| finding.term == "harbor_reward")
        );
    }

    #[test]
    fn redline_scan_detects_attempt_numbers_but_ignores_binary_outputs() {
        let dir = tempdir().expect("tempdir");
        let root = dir.path();
        std::fs::write(root.join("run.log"), "attempt 29180\n").expect("run log");
        std::fs::write(root.join("trace.jsonl"), "{}\n").expect("trace");
        std::fs::write(root.join("answer.bin"), [0, 159, 0, 1]).expect("binary");
        let hash = sha256_file(&root.join("answer.bin")).expect("hash");
        std::fs::write(
            root.join("artifacts.json"),
            serde_json::json!({"artifacts": [{"path": "answer.bin", "sha256": hash}]}).to_string(),
        )
        .expect("manifest");
        let findings = validate_redline_evidence(
            root,
            Path::new("trace.jsonl"),
            Path::new("run.log"),
            Path::new("artifacts.json"),
            &RedlinePolicy {
                clean: true,
                banned_terms: Vec::new(),
            },
        )
        .expect("scan");
        assert!(
            findings
                .iter()
                .any(|finding| finding.term == "attempt number")
        );
        assert!(
            !findings
                .iter()
                .any(|finding| finding.path.ends_with("answer.bin"))
        );
    }

    #[test]
    fn rpc_preflight_blocks_solver_bypass_surface_only_in_solver_mode() {
        assert_eq!(rpc_preflight("command/exec", false), RpcDecision::Allow);
        assert_eq!(rpc_preflight("command/exec", true), RpcDecision::Block);
        assert_eq!(rpc_preflight("fs/readFile", true), RpcDecision::Allow);
        // Resume is admitted by the app-server/Core recovery-canary path; treating it as a
        // blanket RPC bypass would also prevent the guarded recovery workflow itself.
        assert_eq!(rpc_preflight("thread/resume", true), RpcDecision::Allow);
        assert_eq!(
            rpc_preflight("mcpServer/tool/call", true),
            RpcDecision::Block
        );
    }

    #[test]
    fn tool_preflight_requires_the_guard_broker_for_submission_tools() {
        assert_eq!(
            tool_preflight("solver-guard_build-submit", true),
            RpcDecision::Block
        );
        assert_eq!(tool_preflight("exec_command", true), RpcDecision::Allow);
    }

    #[test]
    fn argument_preflight_blocks_submission_markers_and_external_tools() {
        assert_eq!(
            tool_preflight_with_input(
                "exec_command",
                &serde_json::json!({"command": "python solve.py"}),
                true,
            ),
            RpcDecision::Allow
        );
        assert_eq!(
            tool_preflight_with_input(
                "exec_command",
                &serde_json::json!({"command": "curl https://play.bohrium.com"}),
                true,
            ),
            RpcDecision::Block
        );
        assert_eq!(
            tool_preflight_with_input("mcp__bohrium__query", &serde_json::json!({}), true,),
            RpcDecision::Block
        );
        for admin in [
            "ascodex-lease-admin",
            "ascodex-stage-admin",
            "ascodex-observation-admin",
        ] {
            assert_eq!(
                tool_preflight_with_input(
                    "exec_command",
                    &serde_json::json!({"command": format!("{admin} inspect")}),
                    true,
                ),
                RpcDecision::Block
            );
        }
    }

    #[test]
    fn lineage_preflight_is_disabled_outside_solver_mode() {
        let request = LineageRequest {
            parent_present: false,
            depth: 99,
            role: None,
            ephemeral: true,
        };
        assert_eq!(lineage_preflight(&request, false), Ok(()));
    }

    #[test]
    fn lineage_preflight_rejects_invalid_solver_children() {
        let missing_parent = LineageRequest {
            parent_present: false,
            depth: 1,
            role: Some("bohrium-solver"),
            ephemeral: false,
        };
        assert!(lineage_preflight(&missing_parent, true).is_err());
        let deep = LineageRequest {
            parent_present: true,
            depth: 3,
            role: Some("bohrium-solver"),
            ephemeral: false,
        };
        assert!(lineage_preflight(&deep, true).is_err());
        let ephemeral = LineageRequest {
            parent_present: true,
            depth: 1,
            role: Some("bohrium-solver"),
            ephemeral: true,
        };
        assert!(lineage_preflight(&ephemeral, true).is_err());
    }

    #[test]
    fn lineage_preflight_accepts_approved_durable_child() {
        let request = LineageRequest {
            parent_present: true,
            depth: 2,
            role: Some("bohrium-red-team"),
            ephemeral: false,
        };
        assert_eq!(lineage_preflight(&request, true), Ok(()));
    }

    #[test]
    fn solver_spawn_depth_only_allows_direct_children() {
        assert_eq!(solver_spawn_depth_preflight(1, true), Ok(()));
        assert!(solver_spawn_depth_preflight(2, true).is_err());
        assert!(solver_spawn_depth_preflight(0, true).is_err());
        assert_eq!(solver_spawn_depth_preflight(2, false), Ok(()));
    }

    #[tokio::test]
    async fn reservation_counts_pending_cost() {
        let _dir = tempdir().expect("tempdir");
        let database_url = "sqlite::memory:";
        let ledger = Ledger::connect(&database_url).await.expect("connect");
        ledger
            .reserve("a", "challenge", "chief", 6.0, 10.0)
            .await
            .expect("first");
        assert!(
            ledger
                .reserve("b", "challenge", "chief", 5.0, 10.0)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn cadence_and_attempt_lifecycle_are_persisted() {
        let ledger = Ledger::connect("sqlite::memory:").await.expect("connect");
        ledger
            .reserve_with_cadence("a", "challenge", "chief", 1.0, 10.0, "hash-a", 100_000, 60)
            .await
            .expect("first reservation");
        assert!(
            ledger
                .reserve_with_cadence("b", "challenge", "chief", 1.0, 10.0, "hash-b", 110_000, 60)
                .await
                .is_err()
        );
        ledger
            .commit("a", Some("{\"attempt\":1}"))
            .await
            .expect("commit");
        assert!(
            ledger
                .reserve_with_cadence("c", "challenge", "chief", 1.0, 10.0, "hash-c", 161_000, 60)
                .await
                .is_ok()
        );
        ledger.release("c").await.expect("release");
    }

    #[tokio::test]
    async fn broker_rejects_before_reserving_when_admission_fails() {
        let ledger = Ledger::connect("sqlite::memory:").await.expect("connect");
        let policy = Policy::from_yaml(
            "channel:\n  harbor_only: true\n  workspace_root: C:/workspace\n  trusted_cli_sha256: deadbeef\nidentity:\n  name: id-a\n  challenge_id: challenge-a\n  owner: chief\ncadence:\n  min_interval_seconds: 60\n  max_estimated_cost_usd: 10.0\nredline:\n  clean: true\ntrace:\n  real_execution: true\n  paired_tool_events: true\n  artifact_provenance: true\nmodel:\n  provider: openai\n  model: gpt-test\n",
        )
        .expect("policy parses");
        let broker = SubmissionBroker::new(policy, ledger.clone());
        let request = AdmissionRequest {
            channel: "wrong-channel",
            identity: "id-a",
            challenge_id: "challenge-a",
            owner: "chief",
            cli_path: Path::new("C:/missing/cli"),
            workspace: Path::new("C:/workspace"),
            estimated_cost_usd: 1.0,
            trace: TraceEvidence {
                real_execution: true,
                paired_tool_events: true,
                artifact_provenance: true,
            },
            provider: "openai",
            model: "gpt-test",
            content_sha256: "content-a",
            now_ms: 100_000,
        };
        assert!(matches!(
            broker.prepare(&request, "r1", 10.0).await,
            Err(BrokerError::Admission(_))
        ));
        assert!(
            ledger
                .reserve("r2", "challenge-a", "chief", 10.0, 10.0)
                .await
                .is_ok()
        );
    }

    #[tokio::test]
    async fn coordination_events_are_versioned_and_idempotent() {
        let ledger = Ledger::connect("sqlite::memory:").await.expect("connect");
        let first = CoordinationEventRecord {
            event_id: "e1",
            idempotency_key: "k1",
            aggregate_type: "campaign",
            aggregate_id: "c1",
            expected_version: 0,
            event_type: "plan_approved",
            payload_json: "{}",
            occurred_at_ms: 1,
        };
        assert_eq!(
            ledger
                .append_coordination_event(&first)
                .await
                .expect("append"),
            1
        );
        assert_eq!(
            ledger
                .append_coordination_event(&first)
                .await
                .expect("idempotent replay"),
            1
        );

        let stale = CoordinationEventRecord {
            event_id: "e2",
            idempotency_key: "k2",
            aggregate_type: "campaign",
            aggregate_id: "c1",
            expected_version: 0,
            event_type: "execution_started",
            payload_json: "{}",
            occurred_at_ms: 2,
        };
        assert!(ledger.append_coordination_event(&stale).await.is_err());

        let second = CoordinationEventRecord {
            event_id: "e2",
            idempotency_key: "k2",
            aggregate_type: "campaign",
            aggregate_id: "c1",
            expected_version: 1,
            event_type: "execution_started",
            payload_json: "{}",
            occurred_at_ms: 2,
        };
        assert_eq!(
            ledger
                .append_coordination_event(&second)
                .await
                .expect("append"),
            2
        );
    }

    #[tokio::test]
    async fn actor_registry_persists_and_resolves_only_the_bound_actor() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("guard.sqlite");
        let context = solver_actor_context("lease-a");
        let ledger = Ledger::connect_file(&path).await.expect("connect");
        ledger
            .provision_actor_context(&context, 200)
            .await
            .expect("provision");
        drop(ledger);

        let reopened = Ledger::connect_file(&path).await.expect("reopen");
        let resolved = reopened
            .resolve_actor_context(
                "lease-a",
                "agent-a",
                "session-a",
                "thread-a",
                "campaign-a",
                "challenge-a",
                "solver-primary",
                Action::RequestSubmission,
                300,
            )
            .await
            .expect("resolve");
        assert_eq!(resolved, context);

        for invalid in [
            (
                "agent-x",
                "session-a",
                "thread-a",
                "campaign-a",
                "challenge-a",
                "solver-primary",
                300,
            ),
            (
                "agent-a",
                "session-x",
                "thread-a",
                "campaign-a",
                "challenge-a",
                "solver-primary",
                300,
            ),
            (
                "agent-a",
                "session-a",
                "thread-x",
                "campaign-a",
                "challenge-a",
                "solver-primary",
                300,
            ),
            (
                "agent-a",
                "session-a",
                "thread-a",
                "campaign-x",
                "challenge-a",
                "solver-primary",
                300,
            ),
            (
                "agent-a",
                "session-a",
                "thread-a",
                "campaign-a",
                "challenge-x",
                "solver-primary",
                300,
            ),
            (
                "agent-a",
                "session-a",
                "thread-a",
                "campaign-a",
                "challenge-a",
                "solver-secondary",
                300,
            ),
            (
                "agent-a",
                "session-a",
                "thread-a",
                "campaign-a",
                "challenge-a",
                "solver-primary",
                1_000,
            ),
        ] {
            assert!(
                reopened
                    .resolve_actor_context(
                        "lease-a",
                        invalid.0,
                        invalid.1,
                        invalid.2,
                        invalid.3,
                        invalid.4,
                        invalid.5,
                        Action::RequestSubmission,
                        invalid.6,
                    )
                    .await
                    .is_err()
            );
        }

        reopened
            .revoke_actor_lease("lease-a", 400)
            .await
            .expect("revoke");
        assert!(
            reopened
                .resolve_actor_context(
                    "lease-a",
                    "agent-a",
                    "session-a",
                    "thread-a",
                    "campaign-a",
                    "challenge-a",
                    "solver-primary",
                    Action::RequestSubmission,
                    500,
                )
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn actor_registry_rejects_invalid_or_duplicate_provisioning() {
        let ledger = Ledger::connect("sqlite::memory:").await.expect("connect");

        let mut wrong_role = solver_actor_context("lease-chief");
        wrong_role.role = Role::Chief;
        wrong_role.lease.role = Role::Chief;
        assert!(
            ledger
                .provision_actor_context(&wrong_role, 200)
                .await
                .is_err()
        );

        let mut missing_action = solver_actor_context("lease-no-action");
        missing_action.lease.allowed_actions.clear();
        assert!(
            ledger
                .provision_actor_context(&missing_action, 200)
                .await
                .is_err()
        );

        let context = solver_actor_context("lease-a");
        ledger
            .provision_actor_context(&context, 200)
            .await
            .expect("provision");
        assert!(ledger.provision_actor_context(&context, 200).await.is_err());

        let same_actor = solver_actor_context("lease-b");
        assert!(
            ledger
                .provision_actor_context(&same_actor, 200)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn monitor_observation_is_persisted_idempotently_and_hash_checked() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("observation.sqlite");
        let ledger = Ledger::connect_file(&path).await.expect("connect");
        let monitor = monitor_actor_context("monitor-lease");
        ledger
            .provision_actor_context(&monitor, 200)
            .await
            .expect("provision monitor");
        let observation = platform_observation();
        let payload = serde_json::to_string(&observation).expect("observation payload");
        let event = CoordinationEventRecord {
            event_id: "observation-event-a",
            idempotency_key: "observation-key-a",
            aggregate_type: "campaign",
            aggregate_id: "campaign-a",
            expected_version: 0,
            event_type: "platform_observation_recorded",
            payload_json: &payload,
            occurred_at_ms: 200,
        };
        let persisted = ledger
            .record_platform_observation_audited(&monitor, &observation, 200, &event)
            .await
            .expect("record observation");
        assert_eq!(persisted.event_version, 1);
        assert_eq!(persisted.observation, observation);
        assert_eq!(
            ledger
                .record_platform_observation_audited(&monitor, &observation, 200, &event)
                .await
                .expect("replay observation"),
            persisted
        );
        assert_eq!(
            ledger
                .load_latest_platform_observation("challenge-a", "attempt-a")
                .await
                .expect("load observation"),
            persisted
        );

        let mut other = observation.clone();
        other.route = "different-route".to_string();
        let other_payload = serde_json::to_string(&other).expect("other payload");
        let other_event = CoordinationEventRecord {
            event_id: "observation-event-b",
            idempotency_key: "observation-key-b",
            aggregate_type: "campaign",
            aggregate_id: "campaign-a",
            expected_version: 1,
            event_type: "platform_observation_recorded",
            payload_json: &other_payload,
            occurred_at_ms: 200,
        };
        assert!(
            ledger
                .record_platform_observation_audited(&monitor, &other, 200, &other_event)
                .await
                .is_err()
        );
        sqlx::query(
            "UPDATE platform_observations SET observation_json = ? WHERE observation_id = ?",
        )
        .bind("{}")
        .bind(&persisted.observation_id)
        .execute(&ledger.pool)
        .await
        .expect("tamper observation fixture");
        assert!(
            ledger
                .load_latest_platform_observation("challenge-a", "attempt-a")
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn non_monitor_context_cannot_record_platform_observation() {
        let ledger = Ledger::connect("sqlite::memory:").await.expect("connect");
        let solver = solver_actor_context("solver-observation");
        let observation = platform_observation();
        let payload = serde_json::to_string(&observation).expect("observation payload");
        let event = CoordinationEventRecord {
            event_id: "observation-event-solver",
            idempotency_key: "observation-key-solver",
            aggregate_type: "campaign",
            aggregate_id: "campaign-a",
            expected_version: 0,
            event_type: "platform_observation_recorded",
            payload_json: &payload,
            occurred_at_ms: 200,
        };
        assert!(
            ledger
                .record_platform_observation_audited(&solver, &observation, 200, &event)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn platform_reconciliation_apply_is_persisted_idempotently_and_hash_checked() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("reconcile.sqlite");
        let ledger = Ledger::connect_file(&path).await.expect("connect");
        let monitor = monitor_actor_context("monitor-reconcile");
        ledger
            .provision_actor_context(&monitor, 200)
            .await
            .expect("provision monitor");

        let item = reconciliation_item(1, "attempt-a", 'd');
        let payload = serde_json::to_string(&item).expect("item payload");
        let event = CoordinationEventRecord {
            event_id: "reconcile-event-a",
            idempotency_key: "reconcile-key-a",
            aggregate_type: "campaign",
            aggregate_id: "campaign-a",
            expected_version: 0,
            event_type: "platform_reconciliation_recorded",
            payload_json: &payload,
            occurred_at_ms: 200,
        };
        let applied = ledger
            .apply_platform_reconciliation_audited(&monitor, &item, 200, &event)
            .await
            .expect("apply reconciliation");
        assert_eq!(applied.result, ReconciliationApplyResult::Applied);
        assert_eq!(
            applied
                .persisted
                .snapshot
                .cursor
                .as_ref()
                .map(|cursor| cursor.position),
            Some(1)
        );
        assert!(
            applied
                .persisted
                .snapshot
                .attempts
                .contains_key("attempt-a")
        );

        let mut other_campaign = monitor_actor_context("monitor-reconcile-other-campaign");
        other_campaign.agent_id = "monitor-other".to_string();
        other_campaign.session_id = "session-monitor-other".to_string();
        other_campaign.thread_id = "thread-monitor-other".to_string();
        other_campaign.campaign_id = "campaign-b".to_string();
        other_campaign.lease.owner_agent_id = "monitor-other".to_string();
        other_campaign.lease.campaign_id = "campaign-b".to_string();
        ledger
            .provision_actor_context(&other_campaign, 200)
            .await
            .expect("provision other campaign monitor");
        let other_event = CoordinationEventRecord {
            event_id: "reconcile-event-other-campaign",
            idempotency_key: "reconcile-key-other-campaign",
            aggregate_type: "campaign",
            aggregate_id: "campaign-b",
            expected_version: 0,
            event_type: "platform_reconciliation_recorded",
            payload_json: &payload,
            occurred_at_ms: 200,
        };
        assert!(
            ledger
                .apply_platform_reconciliation_audited(&other_campaign, &item, 200, &other_event)
                .await
                .is_err()
        );

        let replayed = ledger
            .apply_platform_reconciliation_audited(&monitor, &item, 200, &event)
            .await
            .expect("replay reconciliation");
        assert_eq!(replayed.result, ReconciliationApplyResult::Duplicate);
        assert_eq!(
            replayed.persisted.snapshot_sha256,
            applied.persisted.snapshot_sha256
        );

        let persisted = ledger
            .load_latest_platform_reconciliation("challenge-a/attempts", "challenge-a")
            .await
            .expect("load reconciliation");
        assert_eq!(persisted.snapshot_sha256, applied.persisted.snapshot_sha256);

        let penalty_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM reconciliation_penalties WHERE challenge_id = 'challenge-a' AND attempt_id = 'attempt-a'",
        )
        .fetch_one(&ledger.pool)
        .await
        .expect("count penalties");
        assert_eq!(penalty_count, 1);

        sqlx::query("UPDATE reconciliation_snapshots SET snapshot_json = '{}' WHERE stream_id = 'challenge-a/attempts' AND challenge_id = 'challenge-a'")
            .execute(&ledger.pool)
            .await
            .expect("tamper snapshot");
        assert!(
            ledger
                .load_latest_platform_reconciliation("challenge-a/attempts", "challenge-a")
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn reconciliation_conflicts_and_non_monitor_calls_fail_closed() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("reconcile-conflict.sqlite");
        let ledger = Ledger::connect_file(&path).await.expect("connect");
        let monitor = monitor_actor_context("monitor-reconcile-conflict");
        ledger
            .provision_actor_context(&monitor, 200)
            .await
            .expect("provision monitor");

        let item = reconciliation_item(1, "attempt-a", 'd');
        let payload = serde_json::to_string(&item).expect("item payload");
        let mut foreign_challenge = item.clone();
        foreign_challenge.challenge_id = "challenge-b".to_string();
        let foreign_payload = serde_json::to_string(&foreign_challenge).expect("foreign payload");
        let foreign_event = CoordinationEventRecord {
            event_id: "reconcile-event-foreign",
            idempotency_key: "reconcile-key-foreign",
            aggregate_type: "campaign",
            aggregate_id: "campaign-a",
            expected_version: 0,
            event_type: "platform_reconciliation_recorded",
            payload_json: &foreign_payload,
            occurred_at_ms: 200,
        };
        assert!(
            ledger
                .apply_platform_reconciliation_audited(
                    &monitor,
                    &foreign_challenge,
                    200,
                    &foreign_event
                )
                .await
                .is_err()
        );
        let event = CoordinationEventRecord {
            event_id: "reconcile-event-conflict-a",
            idempotency_key: "reconcile-key-conflict-a",
            aggregate_type: "campaign",
            aggregate_id: "campaign-a",
            expected_version: 0,
            event_type: "platform_reconciliation_recorded",
            payload_json: &payload,
            occurred_at_ms: 200,
        };
        ledger
            .apply_platform_reconciliation_audited(&monitor, &item, 200, &event)
            .await
            .expect("apply first");

        let conflicting = reconciliation_item(1, "attempt-b", 'e');
        let conflicting_payload = serde_json::to_string(&conflicting).expect("conflict payload");
        let conflicting_event = CoordinationEventRecord {
            event_id: "reconcile-event-conflict-b",
            idempotency_key: "reconcile-key-conflict-b",
            aggregate_type: "campaign",
            aggregate_id: "campaign-a",
            expected_version: 1,
            event_type: "platform_reconciliation_recorded",
            payload_json: &conflicting_payload,
            occurred_at_ms: 200,
        };
        assert!(
            ledger
                .apply_platform_reconciliation_audited(
                    &monitor,
                    &conflicting,
                    200,
                    &conflicting_event
                )
                .await
                .is_err()
        );

        let solver = solver_actor_context("solver-reconcile");
        let solver_event = CoordinationEventRecord {
            event_id: "reconcile-event-solver",
            idempotency_key: "reconcile-key-solver",
            aggregate_type: "campaign",
            aggregate_id: "campaign-a",
            expected_version: 2,
            event_type: "platform_reconciliation_recorded",
            payload_json: &payload,
            occurred_at_ms: 200,
        };
        assert!(
            ledger
                .apply_platform_reconciliation_audited(&solver, &item, 200, &solver_event)
                .await
                .is_err()
        );

        let persisted = ledger
            .load_latest_platform_reconciliation("challenge-a/attempts", "challenge-a")
            .await
            .expect("load after conflict");
        assert_eq!(
            persisted
                .snapshot
                .cursor
                .as_ref()
                .map(|cursor| cursor.position),
            Some(1)
        );
        assert!(persisted.snapshot.attempts.contains_key("attempt-a"));
    }

    #[tokio::test]
    async fn pending_rescore_and_missing_trace_stays_unknown_and_keeps_last_confirmed() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("reconcile-pending.sqlite");
        let ledger = Ledger::connect_file(&path).await.expect("connect");
        let monitor = monitor_actor_context("monitor-reconcile-pending");
        ledger
            .provision_actor_context(&monitor, 200)
            .await
            .expect("provision monitor");

        let confirmed = reconciliation_item(1, "attempt-a", 'd');
        let confirmed_payload = serde_json::to_string(&confirmed).expect("confirmed payload");
        let confirmed_event = CoordinationEventRecord {
            event_id: "reconcile-event-pending-a",
            idempotency_key: "reconcile-key-pending-a",
            aggregate_type: "campaign",
            aggregate_id: "campaign-a",
            expected_version: 0,
            event_type: "platform_reconciliation_recorded",
            payload_json: &confirmed_payload,
            occurred_at_ms: 200,
        };
        ledger
            .apply_platform_reconciliation_audited(&monitor, &confirmed, 200, &confirmed_event)
            .await
            .expect("apply confirmed");

        let mut unknown = reconciliation_item(2, "attempt-a", 'e');
        unknown.facts.bundle_revision = Some("bundle-v2".into());
        unknown.facts.rescore_status = Some(BundleRescoreStatus::Pending);
        unknown.facts.bundle_evidence = Some(EvidenceAvailability::Present);
        unknown.facts.trace_evidence = Some(EvidenceAvailability::Unavailable);
        unknown.facts.score_evidence = Some(EvidenceAvailability::Unavailable);
        unknown.facts.penalty_evidence = None;
        unknown.facts.credited_owner = None;
        unknown.facts.credited_owner_evidence = None;
        unknown.facts.leaderboard_scope = None;
        unknown.facts.anti_cheat = None;
        unknown.facts.anonymous_other_submission_access = None;
        unknown.state = PlatformReconcileItemState::UnknownNeedsReconcile {
            reason: "bundle revision awaits fresh rescore and execution trace is missing".into(),
        };
        let unknown_payload = serde_json::to_string(&unknown).expect("unknown payload");
        let unknown_event = CoordinationEventRecord {
            event_id: "reconcile-event-pending-b",
            idempotency_key: "reconcile-key-pending-b",
            aggregate_type: "campaign",
            aggregate_id: "campaign-a",
            expected_version: 1,
            event_type: "platform_reconciliation_recorded",
            payload_json: &unknown_payload,
            occurred_at_ms: 200,
        };
        let outcome = ledger
            .apply_platform_reconciliation_audited(&monitor, &unknown, 200, &unknown_event)
            .await
            .expect("apply unknown");
        assert_eq!(outcome.result, ReconciliationApplyResult::Applied);
        let fact = &outcome.persisted.snapshot.attempts["attempt-a"];
        assert_eq!(fact.state, ReconciledAttemptState::UnknownNeedsReconcile);
        assert!(fact.last_confirmed_observation.is_some());
        assert_eq!(fact.facts.bundle_revision.as_deref(), Some("bundle-v2"));
        assert_eq!(
            fact.facts.rescore_status,
            Some(BundleRescoreStatus::Pending)
        );
        assert_eq!(
            outcome
                .persisted
                .snapshot
                .cursor
                .as_ref()
                .map(|cursor| cursor.position),
            Some(2)
        );
    }

    #[tokio::test]
    async fn stale_reconciliation_is_a_noop_and_does_not_roll_back() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("reconcile-stale.sqlite");
        let ledger = Ledger::connect_file(&path).await.expect("connect");
        let monitor = monitor_actor_context("monitor-reconcile-stale");
        ledger
            .provision_actor_context(&monitor, 200)
            .await
            .expect("provision monitor");

        let second = reconciliation_item(2, "attempt-b", 'e');
        let second_payload = serde_json::to_string(&second).expect("second payload");
        let second_event = CoordinationEventRecord {
            event_id: "reconcile-event-stale-a",
            idempotency_key: "reconcile-key-stale-a",
            aggregate_type: "campaign",
            aggregate_id: "campaign-a",
            expected_version: 0,
            event_type: "platform_reconciliation_recorded",
            payload_json: &second_payload,
            occurred_at_ms: 200,
        };
        ledger
            .apply_platform_reconciliation_audited(&monitor, &second, 200, &second_event)
            .await
            .expect("apply second");

        let stale = reconciliation_item(1, "attempt-a", 'd');
        let stale_payload = serde_json::to_string(&stale).expect("stale payload");
        let stale_event = CoordinationEventRecord {
            event_id: "reconcile-event-stale-b",
            idempotency_key: "reconcile-key-stale-b",
            aggregate_type: "campaign",
            aggregate_id: "campaign-a",
            expected_version: 1,
            event_type: "platform_reconciliation_recorded",
            payload_json: &stale_payload,
            occurred_at_ms: 200,
        };
        let outcome = ledger
            .apply_platform_reconciliation_audited(&monitor, &stale, 200, &stale_event)
            .await
            .expect("apply stale");
        assert_eq!(outcome.result, ReconciliationApplyResult::Stale);
        let persisted = ledger
            .load_latest_platform_reconciliation("challenge-a/attempts", "challenge-a")
            .await
            .expect("load after stale");
        assert_eq!(
            persisted
                .snapshot
                .cursor
                .as_ref()
                .map(|cursor| cursor.position),
            Some(2)
        );
        assert!(!persisted.snapshot.attempts.contains_key("attempt-a"));
    }

    #[tokio::test]
    async fn chief_spawn_resolution_requires_spawn_child_action_and_live_binding() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("guard.sqlite");
        let workspace = dir.path().join("workspace");
        std::fs::create_dir_all(&workspace).expect("workspace");
        let ledger = Ledger::connect_file(&path).await.expect("connect");
        let chief = chief_actor_context("chief-spawn-lease");
        ledger
            .provision_actor_context(&chief, 200)
            .await
            .expect("provision chief");
        let (cycle, capability_map_path) = issued_cycle(&workspace);
        let payload = cycle_payload(&cycle);
        let event = cycle_event(&cycle, &payload, 200);
        let issuance = ledger
            .issue_research_cycle_audited(
                &chief,
                &cycle,
                &workspace,
                &capability_map_path,
                200,
                &event,
            )
            .await
            .expect("issue active cycle");

        let resolved = ledger
            .resolve_chief_spawn_context(
                "chief-spawn-lease",
                "chief-a",
                "session-chief",
                "thread-chief",
                "campaign-a",
                "challenge-a",
                "cycle-a",
                issuance.cycle_event_version,
                300,
            )
            .await
            .expect("spawn child should be authorized");
        assert_eq!(resolved, chief);
        for (lease_id, cycle_id, cycle_event_version) in [
            (
                "missing-chief-lease",
                "cycle-a",
                issuance.cycle_event_version,
            ),
            (
                "chief-spawn-lease",
                "missing-cycle",
                issuance.cycle_event_version,
            ),
            (
                "chief-spawn-lease",
                "cycle-a",
                issuance.cycle_event_version + 1,
            ),
        ] {
            assert!(
                ledger
                    .resolve_chief_spawn_context(
                        lease_id,
                        "chief-a",
                        "session-chief",
                        "thread-chief",
                        "campaign-a",
                        "challenge-a",
                        cycle_id,
                        cycle_event_version,
                        300,
                    )
                    .await
                    .is_err()
            );
        }

        let mut decide_only = chief.clone();
        decide_only
            .lease
            .allowed_actions
            .remove(&Action::SpawnChild);
        let decide_only_path = dir.path().join("decide-only.sqlite");
        let decide_only_ledger = Ledger::connect_file(&decide_only_path)
            .await
            .expect("connect decide-only ledger");
        decide_only_ledger
            .provision_actor_context(&decide_only, 200)
            .await
            .expect("provision decide-only chief");
        assert!(
            decide_only_ledger
                .resolve_chief_spawn_context(
                    "chief-spawn-lease",
                    "chief-a",
                    "session-chief",
                    "thread-chief",
                    "campaign-a",
                    "challenge-a",
                    "cycle-a",
                    issuance.cycle_event_version,
                    300,
                )
                .await
                .is_err()
        );

        assert!(
            ledger
                .resolve_chief_spawn_context(
                    "chief-spawn-lease",
                    "chief-a",
                    "session-chief",
                    "thread-other",
                    "campaign-a",
                    "challenge-a",
                    "cycle-a",
                    issuance.cycle_event_version,
                    300,
                )
                .await
                .is_err()
        );
        assert!(
            ledger
                .resolve_chief_spawn_context(
                    "chief-spawn-lease",
                    "chief-a",
                    "session-chief",
                    "thread-chief",
                    "campaign-a",
                    "challenge-a",
                    "cycle-a",
                    issuance.cycle_event_version,
                    1_000,
                )
                .await
                .is_err()
        );
        ledger
            .revoke_actor_lease("chief-spawn-lease", 400)
            .await
            .expect("revoke chief lease");
        assert!(
            ledger
                .resolve_chief_spawn_context(
                    "chief-spawn-lease",
                    "chief-a",
                    "session-chief",
                    "thread-chief",
                    "campaign-a",
                    "challenge-a",
                    "cycle-a",
                    issuance.cycle_event_version,
                    500,
                )
                .await
                .is_err()
        );

        let wrong_role_path = dir.path().join("wrong-role.sqlite");
        let wrong_role_ledger = Ledger::connect_file(&wrong_role_path)
            .await
            .expect("connect wrong-role ledger");
        wrong_role_ledger
            .provision_actor_context(&solver_actor_context("solver-not-chief"), 200)
            .await
            .expect("provision solver");
        assert!(
            wrong_role_ledger
                .resolve_chief_spawn_context(
                    "solver-not-chief",
                    "agent-a",
                    "session-a",
                    "thread-a",
                    "campaign-a",
                    "challenge-a",
                    "cycle-a",
                    issuance.cycle_event_version,
                    300,
                )
                .await
                .is_err()
        );

        let revoked_cycle_path = dir.path().join("revoked-cycle.sqlite");
        let revoked_workspace = dir.path().join("revoked-workspace");
        std::fs::create_dir_all(&revoked_workspace).expect("revoked workspace");
        let revoked_cycle_ledger = Ledger::connect_file(&revoked_cycle_path)
            .await
            .expect("connect revoked-cycle ledger");
        revoked_cycle_ledger
            .provision_actor_context(&chief, 200)
            .await
            .expect("provision cycle-revocation Chief");
        let (revoked_cycle, revoked_capability_map_path) = issued_cycle(&revoked_workspace);
        let revoked_payload = cycle_payload(&revoked_cycle);
        let revoked_issue_event = cycle_event(&revoked_cycle, &revoked_payload, 200);
        let revoked_issuance = revoked_cycle_ledger
            .issue_research_cycle_audited(
                &chief,
                &revoked_cycle,
                &revoked_workspace,
                &revoked_capability_map_path,
                200,
                &revoked_issue_event,
            )
            .await
            .expect("issue cycle to revoke");
        let revoke_payload = serde_json::json!({ "cycle_id": "cycle-a" }).to_string();
        let revoke_event = CoordinationEventRecord {
            event_id: "cycle-revoked-spawn-test",
            idempotency_key: "cycle-revoked-spawn-key",
            aggregate_type: "campaign",
            aggregate_id: "campaign-a",
            expected_version: revoked_issuance.cycle_event_version,
            event_type: "research_cycle_revoked",
            payload_json: &revoke_payload,
            occurred_at_ms: 400,
        };
        revoked_cycle_ledger
            .revoke_research_cycle_audited(&chief, "cycle-a", 400, &revoke_event)
            .await
            .expect("revoke cycle");
        assert!(
            revoked_cycle_ledger
                .resolve_chief_spawn_context(
                    "chief-spawn-lease",
                    "chief-a",
                    "session-chief",
                    "thread-chief",
                    "campaign-a",
                    "challenge-a",
                    "cycle-a",
                    revoked_issuance.cycle_event_version,
                    500,
                )
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn audited_actor_lease_changes_commit_with_events() {
        let ledger = Ledger::connect("sqlite::memory:").await.expect("connect");
        let context = solver_actor_context("lease-audited");
        let provision = CoordinationEventRecord {
            event_id: "lease-provision-1",
            idempotency_key: "lease-provision-key-1",
            aggregate_type: "actor_lease",
            aggregate_id: "lease-audited",
            expected_version: 0,
            event_type: "actor_lease_provisioned",
            payload_json: "{\"lease_id\":\"lease-audited\"}",
            occurred_at_ms: 200,
        };
        assert_eq!(
            ledger
                .provision_actor_context_audited(&context, 200, &provision)
                .await
                .expect("audited provision"),
            1
        );
        assert_eq!(
            ledger
                .provision_actor_context_audited(&context, 200, &provision)
                .await
                .expect("idempotent provision replay"),
            1
        );
        let metadata = ledger
            .inspect_actor_lease("lease-audited")
            .await
            .expect("inspect");
        assert_eq!(metadata.agent_id, "agent-a");

        let revoke = CoordinationEventRecord {
            event_id: "lease-revoke-1",
            idempotency_key: "lease-revoke-key-1",
            aggregate_type: "actor_lease",
            aggregate_id: "lease-audited",
            expected_version: 1,
            event_type: "actor_lease_revoked",
            payload_json: "{\"lease_id\":\"lease-audited\"}",
            occurred_at_ms: 400,
        };
        assert_eq!(
            ledger
                .revoke_actor_lease_audited("lease-audited", 400, &revoke)
                .await
                .expect("audited revoke"),
            2
        );
        assert_eq!(
            ledger
                .revoke_actor_lease_audited("lease-audited", 400, &revoke)
                .await
                .expect("idempotent revoke replay"),
            2
        );
        assert_eq!(
            ledger
                .inspect_actor_lease("lease-audited")
                .await
                .expect("inspect revoked")
                .revoked_at_ms,
            Some(400)
        );
    }

    #[tokio::test]
    async fn failed_audit_rolls_back_lease_and_conflicting_idempotency_fails() {
        let ledger = Ledger::connect("sqlite::memory:").await.expect("connect");
        let context = solver_actor_context("lease-rollback");
        let bad_event = CoordinationEventRecord {
            event_id: "wrong-event",
            idempotency_key: "wrong-key",
            aggregate_type: "campaign",
            aggregate_id: "lease-rollback",
            expected_version: 0,
            event_type: "actor_lease_provisioned",
            payload_json: "{}",
            occurred_at_ms: 200,
        };
        assert!(
            ledger
                .provision_actor_context_audited(&context, 200, &bad_event)
                .await
                .is_err()
        );
        assert!(ledger.inspect_actor_lease("lease-rollback").await.is_err());

        let first = CoordinationEventRecord {
            event_id: "event-a",
            idempotency_key: "shared-key",
            aggregate_type: "campaign",
            aggregate_id: "campaign-a",
            expected_version: 0,
            event_type: "first",
            payload_json: "{}",
            occurred_at_ms: 1,
        };
        ledger
            .append_coordination_event(&first)
            .await
            .expect("first event");
        let conflicting = CoordinationEventRecord {
            event_id: "event-b",
            event_type: "different",
            ..first
        };
        assert!(
            ledger
                .append_coordination_event(&conflicting)
                .await
                .is_err()
        );
    }
}
