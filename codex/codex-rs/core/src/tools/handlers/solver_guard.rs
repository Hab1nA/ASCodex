use crate::function_tool::FunctionCallError;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::context::boxed_tool_output;
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::CoreToolRuntime;
use crate::tools::registry::ToolExecutor;
use codex_protocol::models::ResponseInputItem;
use codex_tools::JsonSchema;
use codex_tools::ResponsesApiTool;
use codex_tools::ToolName;
use codex_tools::ToolSpec;
use serde::Deserialize;
use serde_json::Value as JsonValue;
use serde_json::json;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

const TOOL_NAME: &str = "solver_guard_submit";
static DRY_RUN_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SolverGuardSubmitArgs {
    channel: String,
    identity: String,
    identity_class: String,
    campaign_id: String,
    lease_id: String,
    challenge_id: String,
    owner: String,
    cli_path: String,
    workspace: String,
    estimated_cost_usd: f64,
    trace_path: String,
    run_log_path: String,
    artifact_manifest_path: String,
    execution_manifest_path: String,
    provider: String,
    model: String,
    content_sha256: String,
    #[serde(default = "default_true")]
    dry_run: bool,
}

fn default_true() -> bool {
    true
}

struct SolverGuardOutput {
    body: String,
    success: bool,
}

impl ToolOutput for SolverGuardOutput {
    fn log_output(&self) -> String {
        self.body.clone()
    }

    fn success_for_logging(&self) -> bool {
        self.success
    }

    fn to_response_item(&self, call_id: &str, payload: &ToolPayload) -> ResponseInputItem {
        FunctionToolOutput::from_text(self.body.clone(), Some(self.success))
            .to_response_item(call_id, payload)
    }

    fn code_mode_result(&self, _payload: &ToolPayload) -> JsonValue {
        serde_json::from_str(&self.body).unwrap_or_else(|_| json!({"message": self.body}))
    }
}

pub struct SolverGuardSubmitHandler;

impl ToolExecutor<ToolInvocation> for SolverGuardSubmitHandler {
    fn tool_name(&self) -> ToolName {
        ToolName::plain(TOOL_NAME)
    }

    fn spec(&self) -> ToolSpec {
        let properties = BTreeMap::from([
            ("channel".to_string(), JsonSchema::string(None)),
            ("identity".to_string(), JsonSchema::string(None)),
            ("identity_class".to_string(), JsonSchema::string(None)),
            ("campaign_id".to_string(), JsonSchema::string(None)),
            ("lease_id".to_string(), JsonSchema::string(None)),
            ("challenge_id".to_string(), JsonSchema::string(None)),
            ("owner".to_string(), JsonSchema::string(None)),
            ("cli_path".to_string(), JsonSchema::string(None)),
            ("workspace".to_string(), JsonSchema::string(None)),
            ("estimated_cost_usd".to_string(), JsonSchema::number(None)),
            ("trace_path".to_string(), JsonSchema::string(None)),
            ("run_log_path".to_string(), JsonSchema::string(None)),
            (
                "artifact_manifest_path".to_string(),
                JsonSchema::string(None),
            ),
            (
                "execution_manifest_path".to_string(),
                JsonSchema::string(None),
            ),
            ("provider".to_string(), JsonSchema::string(None)),
            ("model".to_string(), JsonSchema::string(None)),
            ("content_sha256".to_string(), JsonSchema::string(None)),
            (
                "dry_run".to_string(),
                JsonSchema::boolean(Some(
                    "Must remain true until a verified network executor is installed.".to_string(),
                )),
            ),
        ]);
        ToolSpec::Function(ResponsesApiTool {
            name: TOOL_NAME.to_string(),
            description: "Run ASCodex's six-gate submission preflight. This native tool is the only permitted submission entry; it is dry-run only until the verified executor is enabled.".to_string(),
            strict: true,
            defer_loading: None,
            parameters: JsonSchema::object(
                properties,
                Some(vec![
                    "channel".to_string(),
                    "identity".to_string(),
                    "identity_class".to_string(),
                    "campaign_id".to_string(),
                    "lease_id".to_string(),
                    "challenge_id".to_string(),
                    "owner".to_string(),
                    "cli_path".to_string(),
                    "workspace".to_string(),
                    "estimated_cost_usd".to_string(),
                    "trace_path".to_string(),
                    "run_log_path".to_string(),
                    "artifact_manifest_path".to_string(),
                    "execution_manifest_path".to_string(),
                    "provider".to_string(),
                    "model".to_string(),
                    "content_sha256".to_string(),
                ]),
                Some(false.into()),
            ),
            output_schema: Some(json!({
                "type": "object",
                "additionalProperties": true,
            })),
        })
    }

    fn handle<'a>(&'a self, invocation: ToolInvocation) -> codex_tools::ToolExecutorFuture<'a>
    where
        ToolInvocation: 'a,
    {
        Box::pin(async move {
            let ToolPayload::Function { arguments } = &invocation.payload else {
                return Err(FunctionCallError::RespondToModel(
                    "solver_guard_submit expects function arguments".to_string(),
                ));
            };
            let args: SolverGuardSubmitArgs = parse_arguments(&arguments)?;
            let response = self.preflight(args, &invocation).await;
            let success = response.get("allowed").and_then(JsonValue::as_bool) == Some(true);
            Ok(boxed_tool_output(SolverGuardOutput {
                body: serde_json::to_string(&response).map_err(|err| {
                    FunctionCallError::Fatal(format!("failed to serialize Guard result: {err}"))
                })?,
                success,
            }))
        })
    }
}

impl SolverGuardSubmitHandler {
    async fn preflight(
        &self,
        args: SolverGuardSubmitArgs,
        invocation: &ToolInvocation,
    ) -> JsonValue {
        if !args.dry_run {
            return json!({
                "allowed": false,
                "status": "executor_unavailable",
                "reason": "ASCodex network submit executor is not enabled; use dry_run=true",
            });
        }
        let runtime_session_id = invocation.session.session_id().to_string();
        let runtime_thread_id = invocation.session.thread_id.to_string();
        // Codex currently identifies an agent by its thread id. Keep this compatibility mapping
        // inside Core so model-supplied arguments can never choose a runtime identity.
        let runtime_agent_id = runtime_thread_id.as_str();
        let now_ms = match invocation
            .session
            .services
            .time_provider
            .current_time(invocation.session.thread_id)
            .await
        {
            Ok(current_time) => current_time.timestamp_millis(),
            Err(err) => {
                return json!({
                    "allowed": false,
                    "status": "blocked",
                    "dry_run": true,
                    "reason": format!("cannot read trusted Core time: {err:#}"),
                });
            }
        };
        let policy_path = match std::env::var("ASCODEX_SOLVER_POLICY_FILE") {
            Ok(path) if !path.trim().is_empty() => path,
            _ => {
                return json!({
                    "allowed": false,
                    "status": "blocked",
                    "reason": "ASCODEX_SOLVER_POLICY_FILE is not configured",
                });
            }
        };
        let yaml = match std::fs::read_to_string(&policy_path) {
            Ok(yaml) => yaml,
            Err(err) => {
                return json!({
                    "allowed": false,
                    "status": "blocked",
                    "reason": format!("cannot read Guard policy: {err}"),
                });
            }
        };
        let policy = match codex_solver_guard::Policy::from_yaml(&yaml) {
            Ok(policy) => policy,
            Err(err) => {
                return json!({
                    "allowed": false,
                    "status": "blocked",
                    "reason": format!("invalid Guard policy: {err}"),
                });
            }
        };
        let workspace = Path::new(&args.workspace);
        let trace = match codex_solver_guard::validate_trace_evidence(
            workspace,
            Path::new(&args.trace_path),
            Path::new(&args.run_log_path),
            Path::new(&args.artifact_manifest_path),
        ) {
            Ok(trace) => trace,
            Err(err) => {
                return json!({
                    "allowed": false,
                    "status": "blocked",
                    "dry_run": true,
                    "reason": format!("trace evidence validation failed: {err}"),
                });
            }
        };
        let execution = match codex_solver_guard::validate_execution_record(
            workspace,
            Path::new(&args.execution_manifest_path),
            Path::new(&args.trace_path),
            Path::new(&args.run_log_path),
        ) {
            Ok(record) => record,
            Err(err) => {
                return json!({
                    "allowed": false,
                    "status": "blocked",
                    "dry_run": true,
                    "reason": format!("execution record validation failed: {err}"),
                });
            }
        };
        if execution.session_id != runtime_session_id || execution.agent_id != runtime_agent_id {
            return json!({
                "allowed": false,
                "status": "blocked",
                "dry_run": true,
                "reason": "execution record session/agent does not match the live invocation",
            });
        }
        let redline_findings = match codex_solver_guard::validate_redline_evidence(
            workspace,
            Path::new(&args.trace_path),
            Path::new(&args.run_log_path),
            Path::new(&args.artifact_manifest_path),
            &policy.redline,
        ) {
            Ok(findings) => findings,
            Err(err) => {
                return json!({
                    "allowed": false,
                    "status": "blocked",
                    "dry_run": true,
                    "reason": format!("redline evidence validation failed: {err}"),
                });
            }
        };
        if !redline_findings.is_empty() {
            return json!({
                "allowed": false,
                "status": "blocked",
                "dry_run": true,
                "reason": "submission artifacts contain platform-feedback or external-solver references",
                "redline_findings": redline_findings,
            });
        }
        let request = codex_solver_guard::AdmissionRequest {
            channel: &args.channel,
            identity: &args.identity,
            challenge_id: &args.challenge_id,
            owner: &args.owner,
            cli_path: Path::new(&args.cli_path),
            workspace,
            estimated_cost_usd: args.estimated_cost_usd,
            trace,
            provider: &args.provider,
            model: &args.model,
            content_sha256: &args.content_sha256,
            now_ms,
        };
        let admission = policy.admit(&request);
        if !admission.allowed {
            return json!({
                "allowed": false,
                "status": "blocked",
                "dry_run": true,
                "failures": admission.failures,
            });
        }

        let ledger_path = match std::env::var("ASCODEX_SOLVER_LEDGER_FILE") {
            Ok(path) if !path.trim().is_empty() => path,
            _ => {
                return json!({
                    "allowed": false,
                    "status": "blocked",
                    "dry_run": true,
                    "reason": "ASCODEX_SOLVER_LEDGER_FILE is not configured",
                });
            }
        };
        let ledger_path = Path::new(&ledger_path);
        if !ledger_path.is_absolute() {
            return json!({
                "allowed": false,
                "status": "blocked",
                "dry_run": true,
                "reason": "ASCODEX_SOLVER_LEDGER_FILE must be absolute",
            });
        }
        let ledger = match codex_solver_guard::Ledger::connect_file(ledger_path).await {
            Ok(ledger) => ledger,
            Err(err) => {
                return json!({
                    "allowed": false,
                    "status": "ledger_unavailable",
                    "dry_run": true,
                    "reason": format!("cannot open Guard ledger: {err}"),
                });
            }
        };
        if let Err(err) = ledger
            .resolve_actor_context(
                &args.lease_id,
                runtime_agent_id,
                &runtime_session_id,
                &runtime_thread_id,
                &args.campaign_id,
                &args.challenge_id,
                &args.identity_class,
                codex_ascodex_coordination::Action::RequestSubmission,
                now_ms,
            )
            .await
        {
            return json!({
                "allowed": false,
                "status": "blocked",
                "dry_run": true,
                "reason": format!("actor lease validation failed: {err}"),
            });
        }
        let budget = policy.cadence.max_estimated_cost_usd;
        let broker = codex_solver_guard::SubmissionBroker::new(policy, ledger);
        let content_tag: String = request.content_sha256.chars().take(16).collect();
        let reservation_id = format!(
            "dry-run-{}-{}-{}",
            std::process::id(),
            request.now_ms,
            format!(
                "{}-{}",
                content_tag,
                DRY_RUN_SEQUENCE.fetch_add(1, Ordering::Relaxed)
            )
        );
        if let Err(err) = broker.prepare(&request, &reservation_id, budget).await {
            return json!({
                "allowed": false,
                "status": "blocked",
                "dry_run": true,
                "reason": format!("Guard ledger reservation failed: {err}"),
            });
        }
        if let Err(err) = broker.release(&reservation_id).await {
            return json!({
                "allowed": false,
                "status": "ledger_degraded",
                "dry_run": true,
                "reason": format!("Guard ledger release failed: {err}"),
            });
        }
        json!({
            "allowed": true,
            "status": "gated_ok",
            "dry_run": true,
            "reservation": "reserved_then_released",
            "failures": [],
        })
    }
}

impl CoreToolRuntime for SolverGuardSubmitHandler {}
