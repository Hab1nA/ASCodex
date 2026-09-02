use crate::agent::control::SpawnAgentOptions;
use crate::agent::control::SolverSpawnChallenge;
use crate::agent::next_thread_spawn_depth;
use crate::agent_communication::AgentCommunicationContext;
use crate::agent_communication::AgentCommunicationKind;
use crate::function_tool::FunctionCallError;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::context::boxed_tool_output;
use crate::tools::handlers::multi_agents_common::apply_spawn_agent_role;
use crate::tools::handlers::multi_agents_common::build_agent_spawn_config;
use crate::tools::handlers::multi_agents_common::thread_spawn_source;
use crate::tools::handlers::multi_agents_v2::communication_from_tool_message;
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::CoreToolRuntime;
use crate::tools::registry::ToolExecutor;
use codex_ascodex_coordination::RoundPlan;
use codex_ascodex_coordination::ROUND_PLAN_SCHEMA_VERSION;
use codex_protocol::AgentPath;
use codex_protocol::models::ResponseInputItem;
use codex_tools::JsonSchema;
use codex_tools::ResponsesApiTool;
use codex_tools::ToolName;
use codex_tools::ToolSpec;
use serde::Deserialize;
use serde_json::Value as JsonValue;
use serde_json::json;
use std::collections::BTreeMap;

const TOOL_NAME: &str = "solver_round_dispatch";
const MAX_PLAN_BYTES: u64 = 256 * 1024;
const SOLVER_ROLE: &str = "bohrium-solver";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SolverRoundDispatchArgs {
    plan_path: String,
}

struct SolverRoundDispatchOutput {
    body: String,
}

impl ToolOutput for SolverRoundDispatchOutput {
    fn log_output(&self) -> String {
        self.body.clone()
    }

    fn success_for_logging(&self) -> bool {
        true
    }

    fn to_response_item(&self, call_id: &str, payload: &ToolPayload) -> ResponseInputItem {
        crate::tools::context::FunctionToolOutput::from_text(self.body.clone(), Some(true))
            .to_response_item(call_id, payload)
    }

    fn code_mode_result(&self, _payload: &ToolPayload) -> JsonValue {
        serde_json::from_str(&self.body).unwrap_or_else(|_| json!({"message": self.body}))
    }
}

/// Round dispatch: one deterministic tool call fans the round plan out into one
/// bohrium-solver child per challenge. Each child is authorized against its own
/// per-challenge cycle binding and lease, receives its verified StageBrief and a
/// clean-room task message, and starts solving immediately — the batch runs fully
/// in parallel without the chief model issuing N separate spawn calls.
pub struct SolverRoundDispatchHandler;

impl ToolExecutor<ToolInvocation> for SolverRoundDispatchHandler {
    fn tool_name(&self) -> ToolName {
        ToolName::plain(TOOL_NAME)
    }

    fn spec(&self) -> ToolSpec {
        let properties = BTreeMap::from([(
            "plan_path".to_string(),
            JsonSchema::string(Some(
                "Absolute path to the ascodex-round-plan/v1 JSON file.".to_string(),
            )),
        )]);
        ToolSpec::Function(ResponsesApiTool {
            name: TOOL_NAME.to_string(),
            description: "Dispatch one bohrium-solver child per challenge in the round plan, all at once. The plan file pins the challenge set, per-challenge Chief leases, workspaces, and the task template; this tool validates it fail-closed and spawns every solver deterministically. Do not call spawn_agent for round dispatch; call this once with the plan.".to_string(),
            strict: true,
            defer_loading: None,
            parameters: JsonSchema::object(properties, Some(vec!["plan_path".to_string()]), Some(false.into())),
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
                    "solver_round_dispatch expects function arguments".to_string(),
                ));
            };
            let args: SolverRoundDispatchArgs = parse_arguments(&arguments)?;
            let plan = load_round_plan(&args.plan_path).map_err(FunctionCallError::RespondToModel)?;
            let solver_mode = std::env::var("ASCODEX_SOLVER_MODE")
                .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE"))
                .unwrap_or(false);
            if !solver_mode {
                return Err(FunctionCallError::RespondToModel(
                    "solver_round_dispatch requires ASCodex solver mode".to_string(),
                ));
            }
            let session = invocation.session.clone();
            let step_context = invocation.step_context.clone();
            let turn = &step_context.turn;
            let parent_thread_id = session.thread_id;
            let author = turn
                .session_source
                .get_agent_path()
                .unwrap_or_else(AgentPath::root);
            let child_depth = next_thread_spawn_depth(&turn.session_source);
            let mut receipts = Vec::new();
            let mut errors = Vec::new();
            for challenge in &plan.challenges {
                let task_name = plan.task_name_for(challenge);
                let result = (|| async {
                    let mut config = build_agent_spawn_config(
                        &session.get_base_instructions().await,
                        turn.as_ref(),
                    )
                    .map_err(|error| error.to_string())?;
                    apply_spawn_agent_role(&session, &mut config, Some(SOLVER_ROLE))
                        .await
                        .map_err(|error| error.to_string())?;
                    let spawn_source = thread_spawn_source(
                        parent_thread_id,
                        &turn.session_source,
                        child_depth,
                        Some(SOLVER_ROLE),
                        Some(task_name.clone()),
                    )
                    .map_err(|error| error.to_string())?;
                    let child_path = spawn_source
                        .get_agent_path()
                        .ok_or("round dispatch child is missing a canonical task name")?;
                    let communication = communication_from_tool_message(
                        author.clone(),
                        child_path.clone(),
                        plan.message_for(challenge),
                        &invocation.source,
                        /*trigger_turn*/ true,
                    );
                    let context =
                        AgentCommunicationContext::new(AgentCommunicationKind::Spawn, parent_thread_id);
                    session
                        .services
                        .agent_control
                        .spawn_agent_with_communication(
                            config,
                            communication,
                            context,
                            Some(spawn_source),
                            SpawnAgentOptions {
                                fork_mode: None,
                                parent_thread_id: Some(parent_thread_id),
                                parent_turn_id: Some(turn.sub_id.clone()),
                                root_turn_id: turn.turn_metadata_state.root_turn_id(),
                                environments: Some(step_context.environments.to_selections()),
                                solver_round_challenge: Some(SolverSpawnChallenge {
                                    campaign_id: plan.campaign_id.clone(),
                                    challenge_id: challenge.challenge_id.clone(),
                                    chief_lease_id: challenge.lease_id.clone(),
                                }),
                                ..SpawnAgentOptions::default()
                            },
                        )
                        .await
                        .map_err(|error| error.to_string())
                })()
                .await;
                match result {
                    Ok(spawned) => receipts.push(json!({
                        "challenge_id": challenge.challenge_id,
                        "task_name": task_name,
                        "thread_id": spawned.thread_id.to_string(),
                        "status": spawned.status,
                    })),
                    Err(error) => errors.push(json!({
                        "challenge_id": challenge.challenge_id,
                        "task_name": task_name,
                        "error": error.to_string(),
                    })),
                }
            }
            let output = SolverRoundDispatchOutput {
                body: json!({
                    "round_id": plan.round_id,
                    "campaign_id": plan.campaign_id,
                    "requested": plan.challenges.len(),
                    "dispatched": receipts.len(),
                    "failed": errors.len(),
                    "receipts": receipts,
                    "errors": errors,
                })
                .to_string(),
            };
            Ok(boxed_tool_output(output))
        })
    }
}

fn load_round_plan(plan_path: &str) -> Result<RoundPlan, String> {
    if plan_path.trim().is_empty() {
        return Err("plan_path is required".to_string());
    }
    if !std::path::Path::new(plan_path).is_absolute() {
        return Err("plan_path must be an absolute path".to_string());
    }
    let metadata = std::fs::metadata(plan_path)
        .map_err(|error| format!("cannot read round plan: {error}"))?;
    if !metadata.is_file() || metadata.len() > MAX_PLAN_BYTES {
        return Err("round plan must be a file of at most 256 KiB".to_string());
    }
    let bytes = std::fs::read(plan_path).map_err(|error| format!("cannot read round plan: {error}"))?;
    let plan: RoundPlan = serde_json::from_slice(&bytes)
        .map_err(|error| format!("invalid round plan: {error}"))?;
    if plan.schema_version != ROUND_PLAN_SCHEMA_VERSION {
        return Err(format!(
            "round plan schema must be {ROUND_PLAN_SCHEMA_VERSION}"
        ));
    }
    plan.validate().map_err(|error| error.to_string())?;
    Ok(plan)
}

impl CoreToolRuntime for SolverRoundDispatchHandler {}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_plan(dir: &std::path::Path, challenges: serde_json::Value) -> String {
        let plan = json!({
            "schema_version": ROUND_PLAN_SCHEMA_VERSION,
            "round_id": "round-1",
            "campaign_id": "camp-round-1",
            "task_message_template": "solve {challenge_id} inside {challenge_workspace}",
            "challenges": challenges,
        });
        let path = dir.join("plan.json");
        std::fs::write(&path, plan.to_string()).expect("write plan");
        path.to_string_lossy().to_string()
    }

    #[test]
    fn round_plan_loads_and_validates_from_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = write_plan(
            dir.path(),
            json!([
                {"challenge_id": "ch-01", "lease_id": "lease-1", "workspace_root": "C:/ws/ch-01"},
                {"challenge_id": "ch-02", "lease_id": "lease-2", "workspace_root": "C:/ws/ch-02"}
            ]),
        );
        let plan = load_round_plan(&path).expect("valid plan");
        assert_eq!(plan.challenges.len(), 2);
        assert_eq!(
            plan.message_for(&plan.challenges[0]),
            "solve ch-01 inside C:/ws/ch-01"
        );
    }

    #[test]
    fn round_plan_rejects_relative_missing_and_invalid_files() {
        assert!(load_round_plan("relative/plan.json").is_err());
        assert!(load_round_plan("C:/definitely-missing-plan.json").is_err());

        let dir = tempfile::tempdir().expect("tempdir");
        let duplicate = write_plan(
            dir.path(),
            json!([
                {"challenge_id": "ch-01", "lease_id": "lease-1", "workspace_root": "C:/ws/ch-01"},
                {"challenge_id": "ch-01", "lease_id": "lease-2", "workspace_root": "C:/ws/ch-02"}
            ]),
        );
        assert!(load_round_plan(&duplicate).is_err());

        let oversized = dir.path().join("big.json");
        std::fs::write(&oversized, [b' '; (MAX_PLAN_BYTES + 1) as usize]).expect("write big");
        assert!(load_round_plan(oversized.to_str().expect("utf-8 path")).is_err());
    }
}
