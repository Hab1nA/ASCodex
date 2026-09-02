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
use codex_solver_guard::trace_builder::{TraceBuildRequest, build_trace_from_runlog};
use codex_tools::JsonSchema;
use codex_tools::ResponsesApiTool;
use codex_tools::ToolName;
use codex_tools::ToolSpec;
use serde::Deserialize;
use serde_json::Value as JsonValue;
use serde_json::json;
use std::collections::BTreeMap;
use std::path::Path;

const TOOL_NAME: &str = "solver_guard_build_trace";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BuildTraceArgs {
    /// Absolute path to the challenge workspace (evidence/ lives inside it).
    workspace: String,
    /// Path to the execution stdout capture, workspace-relative or absolute.
    run_log_path: String,
    /// The real command that produced the run log.
    entrypoint: String,
    /// Bundle-local artifact path the artifact step points at.
    artifact_path: String,
    /// Short human description of the computation.
    problem: String,
}

struct BuildTraceOutput {
    body: String,
    success: bool,
}

impl ToolOutput for BuildTraceOutput {
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

pub struct BuildTraceHandler;

impl ToolExecutor<ToolInvocation> for BuildTraceHandler {
    fn tool_name(&self) -> ToolName {
        ToolName::plain(TOOL_NAME)
    }

    fn spec(&self) -> ToolSpec {
        let properties = BTreeMap::from([
            ("workspace".to_string(), JsonSchema::string(None)),
            ("run_log_path".to_string(), JsonSchema::string(None)),
            ("entrypoint".to_string(), JsonSchema::string(None)),
            ("artifact_path".to_string(), JsonSchema::string(None)),
            ("problem".to_string(), JsonSchema::string(None)),
        ]);
        ToolSpec::Function(ResponsesApiTool {
            name: TOOL_NAME.to_string(),
            description: "Deterministically build the submission trace.jsonl from a real run.log. Run your computation first with its stdout captured to evidence/run.log, then call this once; it writes evidence/trace.jsonl that passes admission on the first attempt. Do NOT hand-write trace.jsonl.".to_string(),
            // agnes-2.5-flash hangs forever on strict:true function schemas
            // (probed 2026-09-02: strict=true never completes, strict=false
            // returns instantly). deny_unknown_fields + required-path checks
            // keep argument validation fail-closed.
            strict: false,
            defer_loading: None,
            parameters: JsonSchema::object(
                properties,
                Some(vec![
                    "workspace".to_string(),
                    "run_log_path".to_string(),
                    "entrypoint".to_string(),
                    "artifact_path".to_string(),
                    "problem".to_string(),
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
                    "solver_guard_build_trace expects function arguments".to_string(),
                ));
            };
            let args: BuildTraceArgs = match parse_arguments(arguments) {
                Ok(args) => args,
                Err(err) => {
                    return Ok(boxed_tool_output(BuildTraceOutput {
                        body: json!({
                            "status": "blocked",
                            "reason": format!("invalid arguments: {err}"),
                        })
                        .to_string(),
                        success: false,
                    }));
                }
            };
            let request = TraceBuildRequest {
                workspace: Path::new(&args.workspace),
                run_log_path: Path::new(&args.run_log_path),
                entrypoint: &args.entrypoint,
                artifact_path: &args.artifact_path,
                problem: &args.problem,
            };
            match build_trace_from_runlog(&request) {
                Ok(built) => Ok(boxed_tool_output(BuildTraceOutput {
                    body: json!({
                        "status": "ok",
                        "trace_path": built.trace_path.display().to_string(),
                        "step_count": built.step_count,
                        "anchor_body": built.anchor_body,
                        "trace_sha256": built.trace_sha256,
                    })
                    .to_string(),
                    success: true,
                })),
                Err(reason) => Ok(boxed_tool_output(BuildTraceOutput {
                    body: json!({
                        "status": "blocked",
                        "reason": reason,
                    })
                    .to_string(),
                    success: false,
                })),
            }
        })
    }
}

impl CoreToolRuntime for BuildTraceHandler {}
