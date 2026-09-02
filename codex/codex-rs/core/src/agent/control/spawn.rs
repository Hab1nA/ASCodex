use super::residency::is_v2_resident_session_source;
use super::*;
use crate::agent::role::apply_role_to_config;
use crate::codex_thread::CodexThread;
use crate::config::PermissionProfileSnapshot;
use crate::context::ASCodexStageBrief;
use crate::context::ContextualUserFragment;
use crate::context::CurrentTimeReminder;
use crate::context::DeveloperInstructions;
use crate::context::ManagedDeveloperInstructions;
use crate::context::MultiAgentModeInstructions;
use crate::context::MultiAgentRoleInstructions;
use crate::context::world_state::PersistentModeState;
use crate::session::multi_agents::resolve_usage_hints;
use crate::tools::handlers::multi_agents_common::build_agent_resume_config;
use codex_context_fragments::set_annotated_content;
use codex_context_fragments::to_annotated_content;
use codex_extension_api::ExtensionDataInit;
use codex_protocol::intersect_effective_permission_profiles;
use codex_protocol::models::PermissionProfile;
use codex_protocol::models::SandboxEnforcement;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::protocol::EnvironmentConfigState;
use codex_utils_path_uri::PathUri;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

const AGENT_NAMES: &str = include_str!("../agent_names.txt");

struct SpawnAgentThreadInheritance {
    environments: Option<TurnEnvironmentSnapshot>,
    exec_policy: Option<Arc<crate::exec_policy::ExecPolicyManager>>,
}

/// Initial input delivered after a spawned agent acquires execution capacity.
///
/// V2 communication spawns keep the communication and its context paired so centralized
/// submission and lifecycle logging cannot receive one without the other. Other spawn sources
/// provide user input directly, making an uncontextualized inter-agent communication
/// unrepresentable.
#[allow(clippy::large_enum_variant)]
enum SpawnInitialInput {
    UserInput(Vec<UserInput>),
    InterAgentCommunication(InterAgentCommunication, AgentCommunicationContext),
}

fn default_agent_nickname_list() -> Vec<&'static str> {
    AGENT_NAMES
        .lines()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .collect()
}

pub(super) fn agent_nickname_candidates(config: &Config, role_name: Option<&str>) -> Vec<String> {
    let role_name = role_name.unwrap_or(DEFAULT_ROLE_NAME);
    if let Some(candidates) =
        resolve_role_config(config, role_name).and_then(|role| role.nickname_candidates.clone())
    {
        return candidates;
    }

    default_agent_nickname_list()
        .into_iter()
        .map(ToOwned::to_owned)
        .collect()
}

fn keep_forked_rollout_item(item: &RolloutItem, preserve_reference_context_item: bool) -> bool {
    match item {
        RolloutItem::ResponseItem(envelope) => match &envelope.item {
            ResponseItem::Message { role, phase, .. } => match role.as_str() {
                "system" | "developer" | "user" => true,
                "assistant" => *phase == Some(MessagePhase::FinalAnswer),
                _ => false,
            },
            ResponseItem::FunctionCallOutput { call_id: None, .. } => true,
            ResponseItem::AdditionalTools { .. }
            | ResponseItem::AgentMessage { .. }
            | ResponseItem::Reasoning { .. }
            | ResponseItem::LocalShellCall { .. }
            | ResponseItem::FunctionCall { .. }
            | ResponseItem::ToolSearchCall { .. }
            | ResponseItem::FunctionCallOutput {
                call_id: Some(_), ..
            }
            | ResponseItem::CustomToolCall { .. }
            | ResponseItem::CustomToolCallOutput { .. }
            | ResponseItem::ToolSearchOutput { .. }
            | ResponseItem::WebSearchCall { .. }
            | ResponseItem::ImageGenerationCall { .. }
            | ResponseItem::Compaction { .. }
            | ResponseItem::CompactionTrigger { .. }
            | ResponseItem::ContextCompaction { .. }
            | ResponseItem::Other => false,
        },
        RolloutItem::RealtimeItem(_)
        | RolloutItem::InterAgentCommunication(_)
        | RolloutItem::InterAgentCommunicationMetadata { .. }
        | RolloutItem::SecurityRiskScore(_) => false,
        // Full-history forks preserve the cached prompt prefix and can keep diffing
        // from the parent's durable baseline. Truncated forks drop part of that prompt,
        // so they must rebuild context on their first child turn.
        RolloutItem::TurnContext(_) | RolloutItem::WorldState(_) => preserve_reference_context_item,
        RolloutItem::Compacted(_) | RolloutItem::EventMsg(_) | RolloutItem::SessionMeta(_) => true,
    }
}

fn retain_forked_developer_message(item: &mut ResponseItem, usage_hint_texts: &[String]) -> bool {
    if !matches!(item, ResponseItem::Message { role, .. } if role == "developer") {
        return true;
    }

    let Some(mut content) = to_annotated_content(item) else {
        return false;
    };
    content.retain(|content_item| {
        let ContentItem::InputText { text } = content_item.content() else {
            return true;
        };

        !(ASCodexStageBrief::matches_text(text)
            || MultiAgentRoleInstructions::matches_text(text)
            || MultiAgentModeInstructions::matches_text(text)
            || CurrentTimeReminder::matches_text(text)
            || usage_hint_texts
                .iter()
                .any(|usage_hint_text| usage_hint_text == text))
    });
    !content.is_empty() && set_annotated_content(item, content).is_some()
}

fn retain_without_ascodex_stage_brief(item: &mut ResponseItem) -> bool {
    if !matches!(item, ResponseItem::Message { role, .. } if role == "developer") {
        return true;
    }
    let Some(mut content) = to_annotated_content(item) else {
        return false;
    };
    content.retain(|content_item| {
        let ContentItem::InputText { text } = content_item.content() else {
            return true;
        };
        !ASCodexStageBrief::matches_text(text)
    });
    !content.is_empty() && set_annotated_content(item, content).is_some()
}

fn remove_ascodex_stage_briefs(items: &mut Vec<RolloutItem>) {
    items.retain_mut(|item| match item {
        RolloutItem::ResponseItem(envelope) => {
            retain_without_ascodex_stage_brief(&mut envelope.item)
        }
        _ => true,
    });
}

async fn load_agent_model_context(
    state: &ThreadManagerState,
    thread_id: ThreadId,
    history_mode: ThreadHistoryMode,
) -> CodexResult<Option<Vec<RolloutItem>>> {
    match history_mode {
        ThreadHistoryMode::Legacy => Ok(state
            .read_stored_thread(ReadThreadParams {
                thread_id,
                include_archived: true,
                include_history: true,
            })
            .await?
            .history
            .map(|history| history.items)),
        ThreadHistoryMode::Paginated => Ok(Some(
            state
                .load_latest_model_context(LoadThreadHistoryParams {
                    thread_id,
                    include_archived: true,
                })
                .await?
                .items,
        )),
    }
}

impl AgentControl {
    /// Restore persisted V2 agent identities without reopening their runtimes.
    pub(crate) async fn restore_v2_agent_metadata(
        &self,
        config: &Config,
        root_thread_id: ThreadId,
    ) {
        self.state.register_root_thread(root_thread_id);

        let Ok(state) = self.upgrade() else {
            return;
        };
        let Some(agent_graph_store) = state.agent_graph_store() else {
            return;
        };
        let descendant_ids = match agent_graph_store
            .list_thread_spawn_descendants(
                root_thread_id,
                Some(codex_agent_graph_store::ThreadSpawnEdgeStatus::Open),
            )
            .await
        {
            Ok(descendant_ids) => descendant_ids,
            Err(err) => {
                warn!("failed to restore persisted V2 agent metadata for {root_thread_id}: {err}");
                return;
            }
        };

        for thread_id in descendant_ids {
            if self.state.agent_metadata_for_thread(thread_id).is_some() {
                continue;
            }
            let restore_result = async {
                let stored_thread = state
                    .read_stored_thread(ReadThreadParams {
                        thread_id,
                        include_archived: true,
                        include_history: false,
                    })
                    .await?;
                let stored_agent_path = stored_thread
                    .agent_path
                    .as_deref()
                    .map(AgentPath::try_from)
                    .transpose()
                    .map_err(|err| {
                        CodexErr::InvalidRequest(format!("invalid stored agent path: {err}"))
                    })?;
                let mut reservation = self.state.reserve_spawn_slot(/*max_threads*/ None)?;
                let mut metadata = self.prepare_agent_metadata(
                    &mut reservation,
                    config,
                    stored_agent_path.or_else(|| stored_thread.source.get_agent_path()),
                    stored_thread
                        .agent_role
                        .or_else(|| stored_thread.source.get_agent_role()),
                    stored_thread
                        .agent_nickname
                        .or_else(|| stored_thread.source.get_nickname()),
                )?;
                metadata.agent_id = Some(thread_id);
                reservation.commit(metadata);
                Ok::<(), CodexErr>(())
            }
            .await;
            if let Err(err) = restore_result {
                warn!("failed to restore V2 agent metadata for {thread_id}: {err}");
            }
        }
    }

    /// Spawn a new agent thread and submit the initial prompt.
    #[cfg(test)]
    pub(crate) async fn spawn_agent(
        &self,
        config: Config,
        initial_input: Vec<UserInput>,
        session_source: Option<SessionSource>,
    ) -> CodexResult<ThreadId> {
        let spawned_agent = Box::pin(self.spawn_agent_internal(
            config,
            SpawnInitialInput::UserInput(initial_input),
            session_source,
            SpawnAgentOptions::default(),
        ))
        .await?;
        Ok(spawned_agent.thread_id)
    }

    /// Spawn an agent thread with some metadata.
    pub(crate) async fn spawn_agent_with_metadata(
        &self,
        config: Config,
        initial_input: Vec<UserInput>,
        session_source: Option<SessionSource>,
        options: SpawnAgentOptions, // TODO(jif) drop with new fork.
    ) -> CodexResult<LiveAgent> {
        Box::pin(self.spawn_agent_internal(
            config,
            SpawnInitialInput::UserInput(initial_input),
            session_source,
            options,
        ))
        .await
    }

    pub(crate) async fn spawn_agent_with_communication(
        &self,
        config: Config,
        communication: InterAgentCommunication,
        context: AgentCommunicationContext,
        session_source: Option<SessionSource>,
        options: SpawnAgentOptions,
    ) -> CodexResult<LiveAgent> {
        Box::pin(self.spawn_agent_internal(
            config,
            SpawnInitialInput::InterAgentCommunication(communication, context),
            session_source,
            options,
        ))
        .await
    }

    fn validate_loaded_v2_child(
        &self,
        thread: &CodexThread,
        parent_thread_id: ThreadId,
    ) -> CodexResult<()> {
        if thread.is_running()
            && thread.multi_agent_version() == Some(MultiAgentVersion::V2)
            && thread.session_source.parent_thread_id() == Some(parent_thread_id)
            && Arc::ptr_eq(&self.state, &thread.session.services.agent_control.state)
        {
            return Ok(());
        }
        Err(CodexErr::InvalidRequest(format!(
            "multi-agent v2 child {} is not owned by its loaded parent",
            thread.session.thread_id
        )))
    }

    /// A provided parent enables owner-validated reloads; `None` preserves sender-driven reloads.
    pub(crate) async fn ensure_v2_agent_loaded(
        &self,
        mut config: Config,
        thread_id: ThreadId,
        parent: Option<Arc<CodexThread>>,
    ) -> CodexResult<()> {
        let state = self.upgrade()?;
        let solver_mode = std::env::var("ASCODEX_SOLVER_MODE")
            .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE"))
            .unwrap_or(false);
        let parent = if let Some(parent) = parent {
            let parent_thread_id = parent.session.thread_id;
            let turn = parent.session.new_default_turn().await;
            config = build_agent_resume_config(&turn).map_err(|_| {
                CodexErr::InvalidRequest(format!(
                    "cannot resume multi-agent v2 child {thread_id} with the current parent settings"
                ))
            })?;
            let registered_parent = state.get_thread(parent_thread_id).await.ok();
            if !registered_parent
                .as_ref()
                .is_some_and(|registered| Arc::ptr_eq(registered, &parent))
                || !parent.is_running()
                || parent.multi_agent_version() != Some(MultiAgentVersion::V2)
                || !Arc::ptr_eq(&self.state, &parent.session.services.agent_control.state)
            {
                return Err(CodexErr::InvalidRequest(format!(
                    "cannot resume multi-agent v2 child {thread_id}: parent ownership is unavailable; resume the parent first"
                )));
            }
            Some((parent, turn.environments.clone()))
        } else {
            None
        };
        let owner_thread_id = parent.as_ref().map(|(parent, _)| parent.session.thread_id);
        if owner_thread_id.is_none() && !solver_mode && state.get_thread(thread_id).await.is_ok() {
            self.touch_loaded_v2_residency(&state, thread_id).await;
            return Ok(());
        }
        if self.state.agent_metadata_for_thread(thread_id).is_none() {
            return Err(CodexErr::ThreadNotFound(thread_id));
        }
        if solver_mode {
            ensure_recovery_canary_for_resume(
                &required_ascodex_env("ASCODEX_SOLVER_LEDGER_FILE")?,
                &thread_id.to_string(),
                now_unix_timestamp_ms(),
            )
            .await?;
        }
        let mut environment_selections = self.state.evicted_environments(thread_id);

        let stored_thread = state
            .read_stored_thread(ReadThreadParams {
                thread_id,
                include_archived: true,
                include_history: false,
            })
            .await?;
        let stored_model = stored_thread.model.clone();
        let stored_model_provider = stored_thread.model_provider.clone();
        let stored_reasoning_effort = stored_thread.reasoning_effort.clone();
        let stored_source = stored_thread.source.clone();
        let stored_parent_thread_id = stored_thread.parent_thread_id;
        let mut history = load_agent_model_context(&state, thread_id, stored_thread.history_mode)
            .await?
            .ok_or(CodexErr::ThreadNotFound(thread_id))?;
        let stored_solver_role = if solver_mode {
            let role_name = solver_resume_role(&stored_source)?;
            remove_ascodex_stage_briefs(&mut history);
            Some(role_name.to_string())
        } else {
            None
        };
        let initial_history = InitialHistory::Resumed(ResumedHistory {
            conversation_id: thread_id,
            history: Arc::new(history),
            rollout_path: stored_thread.rollout_path,
        });
        if initial_history.get_multi_agent_version() != Some(MultiAgentVersion::V2) {
            return Err(CodexErr::ThreadNotFound(thread_id));
        }
        let (session_source, _) = initial_history
            .get_resumed_session_sources()
            .unwrap_or((stored_source, None));
        let stage_brief = if solver_mode {
            let role_name = solver_resume_role(&session_source)?;
            if stored_solver_role.as_deref() != Some(role_name) {
                return Err(CodexErr::InvalidRequest(
                    "ASCodex solver resume blocked: persisted session sources disagree on child role"
                    .to_string(),
                ));
            }
            let role = codex_ascodex_coordination::role_from_solver_role_name(role_name)
                .ok_or_else(|| {
                    CodexErr::InvalidRequest(format!(
                        "ASCodex solver resume blocked: unknown role `{role_name}`"
                    ))
                })?;
            let parent_thread_id = session_source.parent_thread_id();
            let binding = resolve_thread_cycle_binding_for_resume(
                &required_ascodex_env("ASCODEX_SOLVER_LEDGER_FILE")?,
                &thread_id.to_string(),
                &self.session_id.to_string(),
                parent_thread_id.map(|id| id.to_string()),
                role,
                now_unix_timestamp_ms(),
            )
            .await?;
            validate_contract_for_spawn(&binding, role, now_unix_timestamp_ms())?;
            Some(load_stage_brief_for_spawn(Some(role_name), &binding).await?)
        } else {
            None
        };
        if let Some(brief) = stage_brief.as_ref() {
            apply_stage_brief_workspace_acl(&mut config, brief)?;
        }
        if let Some(parent_thread_id) = owner_thread_id {
            if session_source.parent_thread_id() != Some(parent_thread_id)
                || initial_history
                    .get_resumed_parent_thread_id()
                    .is_some_and(|recorded_parent| recorded_parent != parent_thread_id)
                || stored_parent_thread_id
                    .is_some_and(|recorded_parent| recorded_parent != parent_thread_id)
            {
                return Err(CodexErr::InvalidRequest(format!(
                    "cannot resume multi-agent v2 child {thread_id}: recorded parent ownership is inconsistent"
                )));
            }
            if let Ok(thread) = state.get_thread(thread_id).await {
                self.validate_loaded_v2_child(&thread, parent_thread_id)?;
                self.touch_loaded_v2_residency(&state, thread_id).await;
                return Ok(());
            }
        }
        config.model_reasoning_effort = stored_reasoning_effort;
        if let Some(role_name) = session_source.get_agent_role() {
            let runtime_approval_policy = config.permissions.approval_policy.value();
            let runtime_approvals_reviewer = config.approvals_reviewer;
            let runtime_cwd = config.cwd.clone();
            let runtime_permission_profile = match config.permissions.active_permission_profile() {
                Some(active_permission_profile) => {
                    PermissionProfileSnapshot::active_with_profile_workspace_roots(
                        config.permissions.permission_profile().clone(),
                        active_permission_profile,
                        config.permissions.profile_workspace_roots().to_vec(),
                    )
                }
                None => PermissionProfileSnapshot::legacy(
                    config.permissions.permission_profile().clone(),
                ),
            };

            apply_role_to_config(&mut config, Some(&role_name))
                .await
                .map_err(CodexErr::InvalidRequest)?;
            config
                .permissions
                .approval_policy
                .set(runtime_approval_policy)
                .map_err(|err| {
                    CodexErr::InvalidRequest(format!("approval_policy is invalid: {err}"))
                })?;
            config.approvals_reviewer = runtime_approvals_reviewer;
            config.cwd = runtime_cwd;
            config
                .permissions
                .set_permission_profile_from_session_snapshot(runtime_permission_profile)
                .map_err(|err| {
                    CodexErr::InvalidRequest(format!("permission_profile is invalid: {err}"))
                })?;
            if solver_mode {
                enforce_solver_role_permissions_by_name(&mut config, Some(&role_name))?;
                if let Some(brief) = stage_brief.as_ref() {
                    // Role application restores the persisted profile; re-apply the
                    // signed ACL after that restoration so it remains authoritative.
                    apply_stage_brief_workspace_acl(&mut config, brief)?;
                }
            }
        }
        if let Some(model) = stored_model {
            config.model = Some(model);
        }
        if config.model_provider_id != stored_model_provider {
            config.model_provider = config
                .model_providers
                .get(&stored_model_provider)
                .cloned()
                .ok_or_else(|| {
                    CodexErr::InvalidRequest(format!(
                        "Model provider `{stored_model_provider}` not found"
                    ))
                })?;
            config.model_provider_id = stored_model_provider;
        }
        let parent_thread_id = owner_thread_id
            .or_else(|| initial_history.get_resumed_parent_thread_id())
            .or(stored_parent_thread_id);
        let (inherited_environments, inherited_exec_policy, client_mcp_extensions) = if let Some(
            (parent, parent_environments),
        ) =
            parent.as_ref()
        {
            let parent_config = parent.session.get_config().await;
            if !crate::exec_policy::child_uses_parent_exec_policy(&parent_config, &config) {
                return Err(CodexErr::InvalidRequest(format!(
                    "cannot resume multi-agent v2 child {thread_id}: parent execution policy has changed; retry through the parent"
                )));
            }
            if let Some(selections) = environment_selections.as_mut() {
                for selection in selections {
                    let environment_id = &selection.environment_id;
                    let invalid_environment = |reason: &str| {
                        CodexErr::InvalidRequest(format!(
                            "cannot resume multi-agent v2 child {thread_id}: cached environment {environment_id} {reason}"
                        ))
                    };
                    // Matching the attachment also keeps startup on the captured owner executor.
                    let owner_environment = parent_environments
                        .turn_environments()
                        .find(|environment| {
                            let parent_selection = &environment.selection;
                            parent_selection.environment_id == selection.environment_id
                                && parent_selection.cwd == selection.cwd
                                && parent_selection.workspace_roots == selection.workspace_roots
                        })
                        .ok_or_else(|| {
                            invalid_environment("no longer matches a ready parent environment")
                        })?;
                    let owner_config = owner_environment.config();
                    let child_config = match &selection.config {
                        EnvironmentConfigState::FromThread => {
                            // Pin current owner authority instead of re-inferring child settings.
                            selection.config = EnvironmentConfigState::Ready(owner_config.clone());
                            continue;
                        }
                        EnvironmentConfigState::Ready(config) => config,
                        EnvironmentConfigState::Pending | EnvironmentConfigState::Failed(_) => {
                            return Err(invalid_environment("configuration is not ready"));
                        }
                    };
                    let mut bounded_config = child_config.clone();
                    bounded_config.permission_profile = owner_config.permission_profile.clone();
                    if bounded_config != *owner_config {
                        return Err(invalid_environment(
                            "configuration differs from the current parent",
                        ));
                    }
                    if child_config.permission_profile == owner_config.permission_profile {
                        continue;
                    }
                    if owner_environment.environment.is_remote() {
                        return Err(invalid_environment(
                            "permissions changed on a remote executor",
                        ));
                    }
                    let cwd = selection.cwd.to_abs_path().map_err(|_| {
                        invalid_environment("working directory is not a local absolute path")
                    })?;
                    let roots = owner_environment
                        .workspace_roots()
                        .iter()
                        .map(PathUri::to_abs_path)
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(|_| {
                            invalid_environment("workspace roots are not local absolute paths")
                        })?;
                    let authority = owner_environment
                        .permission_profile()
                        .clone()
                        .materialize_project_roots_with_workspace_roots(&roots);
                    let requested = child_config
                        .permission_profile
                        .permission_profile()
                        .clone()
                        .materialize_project_roots_with_workspace_roots(&roots);
                    let permissions =
                        intersect_effective_permission_profiles(&authority, &requested, &cwd)
                            .map_err(|err| {
                                invalid_environment(&format!(
                                    "permissions cannot be intersected safely: {err}"
                                ))
                            })?;
                    bounded_config.permission_profile =
                        PermissionProfileSnapshot::legacy(permissions);
                    selection.config = EnvironmentConfigState::Ready(bounded_config);
                }
            }
            (
                Some(parent_environments.clone()),
                Some(Arc::clone(&parent.session.services.exec_policy)),
                Some(parent.client_mcp_extensions()),
            )
        } else {
            (
                self.inherited_environments_for_source(&state, Some(&session_source))
                    .await,
                self.inherited_exec_policy_for_source(&state, Some(&session_source), &config)
                    .await,
                None,
            )
        };
        // Reserving a slot can evict an idle nested parent. Keep its authority captured above.
        let residency_slot = self
            .reserve_v2_residency_slot(&state, &config, Some(thread_id))
            .await?;

        match state
            .resume_thread_with_history_with_source(ResumeThreadWithHistoryOptions {
                config,
                initial_history,
                agent_control: self.clone(),
                session_source,
                parent_thread_id,
                environment_selections,
                inherited_environments,
                inherited_exec_policy,
                client_mcp_extensions,
            })
            .await
        {
            Ok(reloaded_thread) => {
                if let Some(parent_thread_id) = owner_thread_id {
                    self.validate_loaded_v2_child(&reloaded_thread.thread, parent_thread_id)?;
                }
                self.state.clear_evicted_environments(thread_id);
                residency_slot.commit(reloaded_thread.thread_id);
                state.notify_thread_created(reloaded_thread.thread_id);
                if let Some(stage_brief) = stage_brief {
                    reloaded_thread
                        .thread
                        .inject_fragment_without_turn_and_flush(ASCodexStageBrief::new(
                            stage_brief.rendered,
                        ))
                        .await?;
                }
                Ok(())
            }
            Err(err) => {
                if let Ok(thread) = state.get_thread(thread_id).await {
                    if let Some(parent_thread_id) = owner_thread_id {
                        self.validate_loaded_v2_child(&thread, parent_thread_id)?;
                    }
                    self.state.clear_evicted_environments(thread_id);
                    drop(residency_slot);
                    self.touch_loaded_v2_residency(&state, thread_id).await;
                    return Ok(());
                }
                Err(err)
            }
        }
    }

    async fn spawn_agent_internal(
        &self,
        mut config: Config,
        initial_input: SpawnInitialInput,
        session_source: Option<SessionSource>,
        options: SpawnAgentOptions,
    ) -> CodexResult<LiveAgent> {
        let state = self.upgrade()?;
        let solver_mode = std::env::var("ASCODEX_SOLVER_MODE")
            .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE"))
            .unwrap_or(false);
        if options.solver_round_challenge.is_some() && !solver_mode {
            return Err(CodexErr::InvalidRequest(
                "ASCodex round dispatch requires solver mode for the challenge override".into(),
            ));
        }
        let round_challenge = options.solver_round_challenge.clone();
        let stage_brief = if let Some(SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
            depth,
            agent_role,
            parent_thread_id,
            ..
        })) = session_source.as_ref()
        {
            let lineage_request = codex_solver_guard::LineageRequest {
                parent_present: state.get_thread(*parent_thread_id).await.is_ok(),
                depth: *depth,
                role: agent_role.as_deref(),
                ephemeral: config.ephemeral,
            };
            if let Err(failure) =
                codex_solver_guard::lineage_preflight(&lineage_request, solver_mode)
            {
                return Err(CodexErr::InvalidRequest(format!(
                    "ASCodex solver lineage blocked: {}",
                    failure.reason
                )));
            }
            if let Err(failure) =
                codex_solver_guard::solver_spawn_depth_preflight(*depth, solver_mode)
            {
                return Err(CodexErr::InvalidRequest(format!(
                    "ASCodex solver dispatch blocked: {}",
                    failure.reason
                )));
            }
            if solver_mode {
                let cycle_binding = if let Some(challenge) = round_challenge.as_ref() {
                    self.authorize_solver_spawn_from_chief_for_challenge(
                        &state,
                        *parent_thread_id,
                        challenge,
                    )
                    .await?
                } else {
                    self.authorize_solver_spawn_from_chief(&state, *parent_thread_id)
                        .await?
                };
                let contract_role_name = agent_role.as_deref().ok_or_else(|| {
                    CodexErr::InvalidRequest(
                        "ASCodex contract gate blocked: missing child role".into(),
                    )
                })?;
                let contract_role =
                    codex_ascodex_coordination::role_from_solver_role_name(contract_role_name)
                        .ok_or_else(|| {
                            CodexErr::InvalidRequest("ASCodex child role is invalid".into())
                        })?;
                validate_contract_for_spawn(
                    &cycle_binding,
                    contract_role,
                    now_unix_timestamp_ms(),
                )?;
                enforce_solver_role_permissions_by_name(&mut config, agent_role.as_deref())?;
                let brief =
                    load_stage_brief_for_spawn(agent_role.as_deref(), &cycle_binding).await?;
                apply_stage_brief_workspace_acl(&mut config, &brief)?;
                Some(brief)
            } else {
                None
            }
        } else {
            None
        };
        let multi_agent_version = state
            .effective_multi_agent_version_for_spawn(
                &InitialHistory::New,
                session_source.as_ref(),
                options.parent_thread_id,
                /*forked_from_thread_id*/ None,
                &config,
            )
            .await;
        if let Some(session_source) = session_source.as_ref() {
            self.ensure_execution_capacity(multi_agent_version, session_source)?;
        }
        let agent_max_threads = config.effective_agent_max_threads(multi_agent_version);
        let spawn_uses_v2_residency = multi_agent_version == MultiAgentVersion::V2
            && session_source
                .as_ref()
                .is_some_and(is_v2_resident_session_source);
        let residency_slot = if spawn_uses_v2_residency {
            Some(
                self.reserve_v2_residency_slot(&state, &config, /*protected_thread_id*/ None)
                    .await?,
            )
        } else {
            None
        };
        let reservation_max_threads = if spawn_uses_v2_residency {
            None
        } else {
            agent_max_threads
        };
        let mut reservation = self.state.reserve_spawn_slot(reservation_max_threads)?;
        let inheritance = SpawnAgentThreadInheritance {
            environments: self
                .inherited_environments_for_source(&state, session_source.as_ref())
                .await,
            exec_policy: self
                .inherited_exec_policy_for_source(&state, session_source.as_ref(), &config)
                .await,
        };
        let (session_source, mut agent_metadata) = match session_source {
            Some(SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                parent_thread_id,
                depth,
                agent_path,
                agent_role,
                ..
            })) => {
                let (session_source, agent_metadata) = self.prepare_thread_spawn(
                    &mut reservation,
                    &config,
                    parent_thread_id,
                    depth,
                    agent_path,
                    agent_role,
                    /*preferred_agent_nickname*/ None,
                )?;
                (Some(session_source), agent_metadata)
            }
            other => (other, AgentMetadata::default()),
        };
        let notification_source = session_source.clone();

        // The same `AgentControl` is sent to spawn the thread.
        let new_thread = match (session_source, options.fork_mode.as_ref(), inheritance) {
            (Some(session_source), Some(_), inheritance) => {
                Box::pin(self.spawn_forked_thread(
                    &state,
                    config,
                    session_source,
                    &options,
                    inheritance,
                    multi_agent_version,
                ))
                .await?
            }
            (Some(session_source), None, inheritance) => {
                let history_mode = if let Some(parent_thread_id) = options.parent_thread_id
                    && let Ok(parent_thread) = state.get_thread(parent_thread_id).await
                {
                    matches!(
                        parent_thread.config_snapshot().await.history_mode,
                        ThreadHistoryMode::Paginated
                    )
                    .then_some(ThreadHistoryMode::Paginated)
                } else {
                    None
                };
                Box::pin(state.spawn_new_thread_with_source(
                    config.clone(),
                    self.clone(),
                    session_source,
                    history_mode,
                    options.parent_thread_id,
                    /*forked_from_thread_id*/ None,
                    /*thread_source*/ Some(ThreadSource::Subagent),
                    /*metrics_service_name*/ None,
                    inheritance.environments,
                    inheritance.exec_policy,
                    options.environments.clone(),
                ))
                .await?
            }
            (None, _, _) => Box::pin(state.spawn_new_thread(config.clone(), self.clone())).await?,
        };
        if solver_mode {
            if let Some(SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                parent_thread_id,
                agent_role: Some(role_name),
                ..
            })) = notification_source.as_ref()
            {
                let ledger_file = required_ascodex_env("ASCODEX_SOLVER_LEDGER_FILE")?;
                let role = codex_ascodex_coordination::role_from_solver_role_name(role_name)
                    .ok_or_else(|| {
                        CodexErr::InvalidRequest("ASCodex child role is invalid".into())
                    })?;
                let ledger =
                    codex_solver_guard::Ledger::open_file(std::path::Path::new(&ledger_file))
                        .await
                        .map_err(|error| {
                            CodexErr::InvalidRequest(format!(
                                "ASCodex cycle binding blocked: {error}"
                            ))
                        })?;
                let parent = parent_thread_id.to_string();
                let child = new_thread.thread_id.to_string();
                let child_session_id = self.session_id.to_string();
                let now_ms = now_unix_timestamp_ms();
                let result = if let Some(challenge) = round_challenge.as_ref() {
                    ledger
                        .bind_child_thread_to_cycle_for_challenge(
                            &parent,
                            &child,
                            &child,
                            &child_session_id,
                            role,
                            &format!("child:{child}"),
                            &challenge.campaign_id,
                            &challenge.challenge_id,
                            now_ms,
                        )
                        .await
                } else {
                    ledger
                        .bind_child_thread_to_cycle(
                            &parent,
                            &child,
                            &child,
                            &child_session_id,
                            role,
                            &format!("child:{child}"),
                            now_ms,
                        )
                        .await
                };
                ledger.close().await;
                if let Err(error) = result {
                    let _ = self.shutdown_live_agent(new_thread.thread_id).await;
                    return Err(CodexErr::InvalidRequest(format!(
                        "ASCodex cycle binding blocked: {error}"
                    )));
                }
            }
        }
        agent_metadata.agent_id = Some(new_thread.thread_id);
        reservation.commit(agent_metadata.clone());
        if let Some(residency_slot) = residency_slot {
            residency_slot.commit(new_thread.thread_id);
        }

        if let Some(SessionSource::SubAgent(
            subagent_source @ SubAgentSource::ThreadSpawn {
                parent_thread_id, ..
            },
        )) = notification_source.as_ref()
        {
            let client_metadata = match state.get_thread(*parent_thread_id).await {
                Ok(parent_thread) => parent_thread.session.app_server_client_metadata().await,
                Err(error) => {
                    tracing::warn!(
                        error = %error,
                        parent_thread_id = %parent_thread_id,
                        "skipping subagent thread analytics: failed to load parent thread metadata"
                    );
                    crate::session::session::AppServerClientMetadata {
                        client_name: None,
                        client_version: None,
                    }
                }
            };
            let thread_config = new_thread.thread.config_snapshot().await;
            let parent_thread_id = thread_config.parent_thread_id;
            emit_subagent_session_started(
                &new_thread.thread.session.services.analytics_events_client,
                client_metadata,
                new_thread.thread.session.session_id(),
                new_thread.thread_id,
                parent_thread_id,
                thread_config,
                subagent_source.clone(),
            );
        }

        // Notify a new thread has been created. This notification will be processed by clients
        // to subscribe or drain this newly created thread.
        // TODO(jif) add helper for drain
        state.notify_thread_created(new_thread.thread_id);

        self.persist_thread_spawn_edge_for_source(
            new_thread.thread.as_ref(),
            new_thread.thread_id,
            notification_source.as_ref(),
        )
        .await;

        let start_options = TurnStartOptions {
            parent_turn_id: options.parent_turn_id,
            root_turn_id: options.root_turn_id,
            cyber_access_program: options.cyber_access_program,
            ..Default::default()
        };
        if let Some(stage_brief) = stage_brief {
            new_thread
                .thread
                .inject_fragment_without_turn_and_flush(ASCodexStageBrief::new(
                    stage_brief.rendered,
                ))
                .await?;
        }
        match initial_input {
            SpawnInitialInput::UserInput(input) => {
                self.send_input(new_thread.thread_id, input, start_options)
                    .await?;
            }
            SpawnInitialInput::InterAgentCommunication(communication, context) => {
                self.send_inter_agent_communication_after_capacity_check(
                    new_thread.thread_id,
                    &state,
                    communication,
                    context,
                    start_options,
                )
                .await?;
            }
        }
        if multi_agent_version != MultiAgentVersion::V2 {
            let child_reference = agent_metadata
                .agent_path
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|| new_thread.thread_id.to_string());
            self.maybe_start_completion_watcher(
                new_thread.thread_id,
                notification_source,
                child_reference,
                agent_metadata.agent_path.clone(),
            );
        }

        Ok(LiveAgent {
            thread_id: new_thread.thread_id,
            metadata: agent_metadata,
            status: self.get_status(new_thread.thread_id).await,
        })
    }

    async fn spawn_forked_thread(
        &self,
        state: &Arc<ThreadManagerState>,
        config: Config,
        session_source: SessionSource,
        options: &SpawnAgentOptions,
        inheritance: SpawnAgentThreadInheritance,
        multi_agent_version: MultiAgentVersion,
    ) -> CodexResult<crate::thread_manager::NewThread> {
        let SpawnAgentThreadInheritance {
            environments: inherited_environments,
            exec_policy: inherited_exec_policy,
        } = inheritance;
        if options.fork_parent_spawn_call_id.is_none() {
            return Err(CodexErr::Fatal(
                "spawn_agent fork requires a parent spawn call id".to_string(),
            ));
        }
        let Some(fork_mode) = options.fork_mode.as_ref() else {
            return Err(CodexErr::Fatal(
                "spawn_agent fork requires a fork mode".to_string(),
            ));
        };
        let SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
            parent_thread_id, ..
        }) = &session_source
        else {
            return Err(CodexErr::Fatal(
                "spawn_agent fork requires a thread-spawn session source".to_string(),
            ));
        };

        let parent_thread_id = *parent_thread_id;
        let parent_thread = state.get_thread(parent_thread_id).await?;
        let (subagent_developer_instructions, parent_developer_instructions) = match (
            multi_agent_version,
            config
                .multi_agent_v2
                .subagent_developer_instructions
                .as_ref(),
        ) {
            (MultiAgentVersion::V2, override_instructions)
                if override_instructions.is_some() || session_source.get_agent_role().is_some() =>
            {
                let parent_developer_instructions = match parent_thread
                    .session
                    .new_default_turn()
                    .await
                    .developer_instructions
                    .clone()
                {
                    Some(instructions) if !instructions.is_empty() => Some(instructions),
                    Some(_) | None => None,
                };
                (
                    Some(config.developer_instructions.clone().unwrap_or_default()),
                    parent_developer_instructions,
                )
            }
            (MultiAgentVersion::Disabled | MultiAgentVersion::V1, _)
            | (MultiAgentVersion::V2, _) => (None, None),
        };
        let parent_history_mode = parent_thread.config_snapshot().await.history_mode;
        // `record_conversation_items` only queues persistence writes asynchronously.
        // Flush before snapshotting store history for a fork.
        parent_thread.ensure_rollout_materialized().await;
        parent_thread.flush_rollout().await?;

        let destination_history_mode = matches!(parent_history_mode, ThreadHistoryMode::Paginated)
            .then_some(ThreadHistoryMode::Paginated);
        let mut forked_rollout_items =
            load_agent_model_context(state, parent_thread_id, parent_history_mode)
                .await?
                .ok_or_else(|| {
                    CodexErr::Fatal(format!(
                        "parent thread history unavailable for fork: {parent_thread_id}"
                    ))
                })?;

        let selected_capability_roots = forked_rollout_items
            .iter()
            .find_map(|item| {
                let RolloutItem::SessionMeta(meta_line) = item else {
                    return None;
                };
                Some(meta_line.meta.selected_capability_roots.clone())
            })
            .unwrap_or_default();
        if let SpawnAgentForkMode::LastNTurns(last_n_turns) = fork_mode {
            forked_rollout_items =
                truncate_rollout_to_last_n_fork_turns(forked_rollout_items, *last_n_turns);
        }
        let multi_agent_v2_usage_hint_texts_to_filter: Vec<String> =
            if multi_agent_version == MultiAgentVersion::V2 {
                let parent_config = parent_thread.session.get_config().await;
                let parent_usage_hints =
                    resolve_usage_hints(&parent_config.multi_agent_v2, /*catalog*/ None);
                [parent_usage_hints.root, parent_usage_hints.subagent]
                    .into_iter()
                    .flatten()
                    .map(|instructions| instructions.render())
                    .collect()
            } else {
                Vec::new()
            };
        let mut preserve_reference_context_item =
            matches!(fork_mode, SpawnAgentForkMode::FullHistory);
        if preserve_reference_context_item {
            for item in forked_rollout_items.iter().rev() {
                let RolloutItem::Compacted(compacted) = item else {
                    continue;
                };
                // Legacy checkpoints force the child to rebuild context regardless of the
                // live parent's reference baseline; an older superseded checkpoint does not.
                if compacted.replacement_history.is_none() {
                    preserve_reference_context_item = false;
                }
                break;
            }
        }
        let mut replaced_parent_developer_instructions = false;
        // Scrub inherited hints and replace only the parent's developer-instruction fragment.
        // Compaction stores response items separately, so sanitize both top-level messages and
        // compacted replacement histories with the same policy.
        let retain_forked_item = |response_item: &mut ResponseItem, replaced: &mut bool| {
            if matches!(response_item, ResponseItem::AgentMessage { .. }) {
                return false;
            }
            if !retain_forked_developer_message(
                response_item,
                &multi_agent_v2_usage_hint_texts_to_filter,
            ) {
                return false;
            }

            if matches!(response_item, ResponseItem::Message { role, .. } if role == "developer") {
                let Some(mut content) = to_annotated_content(response_item) else {
                    return false;
                };
                content.retain_mut(|content_item| {
                    let ContentItem::InputText { text } = content_item.content_mut() else {
                        return true;
                    };
                    if ManagedDeveloperInstructions::matches_text(text)
                        || PersistentModeState::matches_text(text)
                    {
                        // If the child will rebuild its initial context, drop the inherited
                        // instructions; startup will add the current requirements and effort
                        // instructions once.
                        return preserve_reference_context_item;
                    }
                    let (
                        Some(parent_developer_instructions),
                        Some(subagent_developer_instructions),
                    ) = (
                        parent_developer_instructions.as_ref(),
                        subagent_developer_instructions.as_ref(),
                    )
                    else {
                        return true;
                    };
                    // TODO(anp) track better message fragment provenance in rollouts.
                    if !text.contains(parent_developer_instructions) {
                        return true;
                    }

                    *replaced = true;
                    let replacement = if preserve_reference_context_item {
                        subagent_developer_instructions.as_str()
                    } else {
                        ""
                    };
                    *text = text.replace(parent_developer_instructions, replacement);
                    !text.is_empty()
                });
                return !content.is_empty()
                    && set_annotated_content(response_item, content).is_some();
            }

            true
        };
        forked_rollout_items.retain_mut(|item| {
            if !keep_forked_rollout_item(item, preserve_reference_context_item)
                || destination_history_mode == Some(ThreadHistoryMode::Paginated)
                    && matches!(
                        &*item,
                        RolloutItem::EventMsg(
                            EventMsg::ItemCompleted(_)
                                | EventMsg::TokenCount(_)
                                | EventMsg::ThreadGoalUpdated(_)
                                | EventMsg::ThreadSettingsApplied(_),
                        )
                    )
            {
                return false;
            }

            match item {
                RolloutItem::ResponseItem(response_item) => {
                    retain_forked_item(response_item, &mut replaced_parent_developer_instructions)
                }
                RolloutItem::Compacted(compacted) => {
                    if let Some(replacement_history) = compacted.replacement_history.as_mut() {
                        // Matches before this checkpoint cannot survive its replacement history.
                        replaced_parent_developer_instructions = false;
                        replacement_history.retain_mut(|response_item| {
                            retain_forked_item(
                                response_item,
                                &mut replaced_parent_developer_instructions,
                            )
                        });
                    }
                    true
                }
                RolloutItem::WorldState(world_state) => {
                    if multi_agent_version == MultiAgentVersion::V2 {
                        world_state.state.remove("multi_agent_usage_hint");
                    }
                    true
                }
                RolloutItem::RealtimeItem(_) => false,
                RolloutItem::EventMsg(_)
                | RolloutItem::SessionMeta(_)
                | RolloutItem::TurnContext(_)
                | RolloutItem::InterAgentCommunication(_)
                | RolloutItem::InterAgentCommunicationMetadata { .. } => true,
                RolloutItem::SecurityRiskScore(_) => false,
            }
        });
        // Full forks reuse the parent's reference context instead of rebuilding it. If that
        // context omitted the parent's developer fragment, append the child's override so its
        // instructions still reach the model exactly once.
        if let Some(subagent_developer_instructions) = subagent_developer_instructions.as_ref()
            && preserve_reference_context_item
            && !replaced_parent_developer_instructions
            && !subagent_developer_instructions.is_empty()
            && parent_thread
                .session
                .reference_context_item()
                .await
                .is_some()
        {
            let developer_message = ContextualUserFragment::into(DeveloperInstructions::new(
                subagent_developer_instructions,
            ));
            forked_rollout_items.push(RolloutItem::ResponseItem(developer_message.into()));
        }
        if preserve_reference_context_item
            && multi_agent_version == MultiAgentVersion::V2
            && let Some(subagent_usage_hint) = options
                .multi_agent_v2_usage_hints
                .as_ref()
                .map(|hints| hints.subagent.clone())
                .unwrap_or_else(|| {
                    resolve_usage_hints(&config.multi_agent_v2, /*catalog*/ None).subagent
                })
        {
            let subagent_usage_hint_message = ContextualUserFragment::into(subagent_usage_hint);
            forked_rollout_items.push(RolloutItem::ResponseItem(
                subagent_usage_hint_message.into(),
            ));
        }
        let mut thread_extension_init = ExtensionDataInit::new();
        thread_extension_init.insert(selected_capability_roots);

        state
            .fork_thread_with_source(
                config.clone(),
                InitialHistory::Forked(forked_rollout_items),
                destination_history_mode,
                self.clone(),
                session_source,
                /*thread_source*/ Some(ThreadSource::Subagent),
                /*parent_thread_id*/ Some(parent_thread_id),
                /*forked_from_thread_id*/ Some(parent_thread_id),
                inherited_environments,
                inherited_exec_policy,
                options.environments.clone(),
                thread_extension_init,
            )
            .await
    }

    /// Resume an existing agent thread from a recorded rollout file.
    pub(crate) async fn resume_agent_from_rollout(
        &self,
        config: Config,
        thread_id: ThreadId,
        session_source: SessionSource,
    ) -> CodexResult<ThreadId> {
        let root_depth = thread_spawn_depth(&session_source).unwrap_or(0);
        let (resumed_thread_id, resumed_multi_agent_version) = Box::pin(
            self.resume_single_agent_from_rollout(config.clone(), thread_id, session_source),
        )
        .await?;
        let state = self.upgrade()?;
        if config.multi_agent_version_from_features() == MultiAgentVersion::V2
            || resumed_multi_agent_version == MultiAgentVersion::V2
        {
            return Ok(resumed_thread_id);
        }
        let Some(agent_graph_store) = state.agent_graph_store() else {
            return Ok(resumed_thread_id);
        };

        let mut resume_queue = VecDeque::from([(thread_id, root_depth)]);
        while let Some((parent_thread_id, parent_depth)) = resume_queue.pop_front() {
            let child_ids = match agent_graph_store
                .list_thread_spawn_children(
                    parent_thread_id,
                    Some(codex_agent_graph_store::ThreadSpawnEdgeStatus::Open),
                )
                .await
            {
                Ok(child_ids) => child_ids,
                Err(err) => {
                    warn!(
                        "failed to load persisted thread-spawn children for {parent_thread_id}: {err}"
                    );
                    continue;
                }
            };

            for child_thread_id in child_ids {
                let child_depth = parent_depth + 1;
                let child_resumed = if state.get_thread(child_thread_id).await.is_ok() {
                    true
                } else {
                    let child_session_source =
                        SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                            parent_thread_id,
                            depth: child_depth,
                            agent_path: None,
                            agent_nickname: None,
                            agent_role: None,
                        });
                    match Box::pin(self.resume_single_agent_from_rollout(
                        config.clone(),
                        child_thread_id,
                        child_session_source,
                    ))
                    .await
                    {
                        Ok((_, _)) => true,
                        Err(err) => {
                            warn!("failed to resume descendant thread {child_thread_id}: {err}");
                            false
                        }
                    }
                };
                if child_resumed {
                    resume_queue.push_back((child_thread_id, child_depth));
                }
            }
        }

        Ok(resumed_thread_id)
    }

    async fn resume_single_agent_from_rollout(
        &self,
        mut config: Config,
        thread_id: ThreadId,
        session_source: SessionSource,
    ) -> CodexResult<(ThreadId, MultiAgentVersion)> {
        let state = self.upgrade()?;
        let stored_thread = state
            .read_stored_thread(ReadThreadParams {
                thread_id,
                include_archived: true,
                include_history: false,
            })
            .await?;
        let solver_mode = std::env::var("ASCODEX_SOLVER_MODE")
            .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE"))
            .unwrap_or(false);
        if solver_mode {
            ensure_recovery_canary_for_resume(
                &required_ascodex_env("ASCODEX_SOLVER_LEDGER_FILE")?,
                &thread_id.to_string(),
                now_unix_timestamp_ms(),
            )
            .await?;
        }
        let resumed_agent_path = stored_thread
            .agent_path
            .as_deref()
            .map(AgentPath::try_from)
            .transpose()
            .map_err(|err| CodexErr::InvalidRequest(format!("invalid stored agent path: {err}")))?;
        let resumed_agent_nickname = stored_thread.agent_nickname.clone();
        let mut resumed_agent_role = stored_thread.agent_role.clone();
        let mut history = load_agent_model_context(&state, thread_id, stored_thread.history_mode)
            .await?
            .ok_or(CodexErr::ThreadNotFound(thread_id))?;
        let stage_brief = if solver_mode {
            let role_name = solver_resume_role(&stored_thread.source)?.to_string();
            enforce_solver_role_permissions_by_name(&mut config, Some(&role_name))?;
            resumed_agent_role = Some(role_name.clone());
            let role = codex_ascodex_coordination::role_from_solver_role_name(&role_name)
                .ok_or_else(|| {
                    CodexErr::InvalidRequest(format!(
                        "ASCodex solver resume blocked: unknown role `{role_name}`"
                    ))
                })?;
            let binding = resolve_thread_cycle_binding_for_resume(
                &required_ascodex_env("ASCODEX_SOLVER_LEDGER_FILE")?,
                &thread_id.to_string(),
                &self.session_id.to_string(),
                stored_thread.parent_thread_id.map(|id| id.to_string()),
                role,
                now_unix_timestamp_ms(),
            )
            .await?;
            validate_contract_for_spawn(&binding, role, now_unix_timestamp_ms())?;
            Some(load_stage_brief_for_spawn(Some(&role_name), &binding).await?)
        } else {
            None
        };
        if let Some(brief) = stage_brief.as_ref() {
            apply_stage_brief_workspace_acl(&mut config, brief)?;
        }
        let session_source = if solver_mode {
            stored_thread.source.clone()
        } else {
            session_source
        };
        if stage_brief.is_some() {
            remove_ascodex_stage_briefs(&mut history);
        }
        let initial_history = InitialHistory::Resumed(ResumedHistory {
            conversation_id: thread_id,
            history: Arc::new(history),
            rollout_path: stored_thread.rollout_path,
        });
        let parent_thread_id = stored_thread.parent_thread_id;
        let multi_agent_version = state
            .effective_multi_agent_version_for_spawn(
                &initial_history,
                Some(&session_source),
                parent_thread_id,
                /*forked_from_thread_id*/ None,
                &config,
            )
            .await;
        let agent_max_threads = config.effective_agent_max_threads(multi_agent_version);
        let mut reservation = self.state.reserve_spawn_slot(agent_max_threads)?;
        let (session_source, agent_metadata) = match session_source {
            SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                parent_thread_id,
                depth,
                agent_path,
                agent_role: _,
                agent_nickname: _,
            }) => self.prepare_thread_spawn(
                &mut reservation,
                &config,
                parent_thread_id,
                depth,
                agent_path.or(resumed_agent_path),
                resumed_agent_role,
                resumed_agent_nickname,
            )?,
            other => (other, AgentMetadata::default()),
        };
        let notification_source = session_source.clone();
        let inherited_environments = self
            .inherited_environments_for_source(&state, Some(&session_source))
            .await;
        let inherited_exec_policy = self
            .inherited_exec_policy_for_source(&state, Some(&session_source), &config)
            .await;

        let resumed_thread = state
            .resume_thread_with_history_with_source(ResumeThreadWithHistoryOptions {
                config: config.clone(),
                initial_history,
                agent_control: self.clone(),
                session_source,
                parent_thread_id,
                environment_selections: None,
                inherited_environments,
                inherited_exec_policy,
                client_mcp_extensions: None,
            })
            .await?;
        if let Some(stage_brief) = stage_brief {
            resumed_thread
                .thread
                .inject_fragment_without_turn_and_flush(ASCodexStageBrief::new(
                    stage_brief.rendered,
                ))
                .await?;
        }
        let mut agent_metadata = agent_metadata;
        agent_metadata.agent_id = Some(resumed_thread.thread_id);
        reservation.commit(agent_metadata.clone());
        // Resumed threads are re-registered in-memory and need the same listener
        // attachment path as freshly spawned threads.
        state.notify_thread_created(resumed_thread.thread_id);
        if multi_agent_version != MultiAgentVersion::V2 {
            let child_reference = agent_metadata
                .agent_path
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|| resumed_thread.thread_id.to_string());
            self.maybe_start_completion_watcher(
                resumed_thread.thread_id,
                Some(notification_source.clone()),
                child_reference,
                agent_metadata.agent_path.clone(),
            );
        }
        self.persist_thread_spawn_edge_for_source(
            resumed_thread.thread.as_ref(),
            resumed_thread.thread_id,
            Some(&notification_source),
        )
        .await;

        Ok((resumed_thread.thread_id, multi_agent_version))
    }

    /// Single admission point for generic cold resumes.  V2 children already have an
    /// owner-validated path, but V1 persisted `ThreadSpawn` children can otherwise reach
    /// `ThreadManagerState::spawn_thread` with caller-owned config.  We therefore re-run the
    /// canary, role, cycle binding, StageBrief, and workspace ACL gates here before startup.
    pub(crate) async fn admit_resumed_v1_solver_thread(
        &self,
        config: &mut Config,
        initial_history: &mut InitialHistory,
    ) -> CodexResult<()> {
        let solver_mode = std::env::var("ASCODEX_SOLVER_MODE")
            .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE"))
            .unwrap_or(false);
        if !solver_mode {
            return Ok(());
        }
        let ledger_file = required_ascodex_env("ASCODEX_SOLVER_LEDGER_FILE")?;
        self.admit_resumed_v1_solver_thread_with_gate(
            solver_mode,
            &ledger_file,
            config,
            initial_history,
        )
        .await
    }

    pub(super) async fn admit_resumed_v1_solver_thread_with_gate(
        &self,
        solver_mode: bool,
        ledger_file: &str,
        config: &mut Config,
        initial_history: &mut InitialHistory,
    ) -> CodexResult<()> {
        if !solver_mode {
            return Ok(());
        }
        let InitialHistory::Resumed(_) = initial_history else {
            return Ok(());
        };
        if initial_history.get_multi_agent_version() == Some(MultiAgentVersion::V2) {
            return Ok(());
        }
        let (session_source, _) =
            initial_history
                .get_resumed_session_sources()
                .ok_or_else(|| {
                    CodexErr::InvalidRequest(
                        "ASCodex solver resume requires a persisted session source".into(),
                    )
                })?;
        if !session_source.is_non_root_agent() {
            return Ok(());
        }
        let SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
            parent_thread_id, ..
        }) = &session_source
        else {
            return Err(CodexErr::InvalidRequest(
                "ASCodex solver resume only accepts persisted ThreadSpawn child sources".into(),
            ));
        };
        let thread_id = match initial_history {
            InitialHistory::Resumed(resumed) => resumed.conversation_id,
            _ => return Ok(()),
        };
        ensure_recovery_canary_for_resume(
            ledger_file,
            &thread_id.to_string(),
            now_unix_timestamp_ms(),
        )
        .await?;
        let role_name = solver_resume_role(&session_source)?.to_string();
        if let InitialHistory::Resumed(resumed) = initial_history {
            remove_ascodex_stage_briefs(Arc::make_mut(&mut resumed.history));
        }
        enforce_solver_role_permissions_by_name(config, Some(&role_name))?;
        let role = codex_ascodex_coordination::role_from_solver_role_name(&role_name).ok_or_else(
            || {
                CodexErr::InvalidRequest(format!(
                    "ASCodex solver resume blocked: unknown role `{role_name}`"
                ))
            },
        )?;
        let binding = resolve_thread_cycle_binding_for_resume(
            ledger_file,
            &thread_id.to_string(),
            &self.session_id.to_string(),
            Some(parent_thread_id.to_string()),
            role,
            now_unix_timestamp_ms(),
        )
        .await?;
        validate_contract_for_spawn(&binding, role, now_unix_timestamp_ms())?;
        let brief = load_stage_brief_for_spawn(Some(&role_name), &binding).await?;
        apply_stage_brief_workspace_acl(config, &brief)?;
        Ok(())
    }

    async fn authorize_solver_spawn_from_chief(
        &self,
        state: &Arc<ThreadManagerState>,
        parent_thread_id: ThreadId,
    ) -> CodexResult<codex_solver_guard::ThreadCycleBinding> {
        let parent_thread = state.get_thread(parent_thread_id).await.map_err(|error| {
            CodexErr::InvalidRequest(format!(
                "ASCodex Chief spawn lease blocked: live parent thread is unavailable: {error}"
            ))
        })?;
        if parent_thread.session.session_id() != self.session_id
            || !Arc::ptr_eq(
                &self.state,
                &parent_thread.session.services.agent_control.state,
            )
        {
            return Err(CodexErr::InvalidRequest(
                "ASCodex Chief spawn lease blocked: parent does not belong to this live agent-control session"
                    .to_string(),
            ));
        }
        let now_ms = parent_thread
            .session
            .services
            .time_provider
            .current_time(parent_thread_id)
            .await
            .map_err(|error| {
                CodexErr::InvalidRequest(format!(
                    "ASCodex Chief spawn lease blocked: cannot read trusted Core time: {error:#}"
                ))
            })?
            .timestamp_millis();
        let ledger_file = required_ascodex_env("ASCODEX_SOLVER_LEDGER_FILE")?;
        let chief_lease_id = required_ascodex_env("ASCODEX_CHIEF_LEASE_ID")?;
        let live_thread_id = parent_thread_id.to_string();
        let live_session_id = parent_thread.session.session_id().to_string();
        let binding = resolve_parent_cycle_binding(
            std::path::Path::new(&ledger_file),
            &live_thread_id,
            &live_session_id,
            now_ms,
        )
        .await?;
        for (name, actual) in [
            ("ASCODEX_CAMPAIGN_ID", binding.campaign_id.as_str()),
            ("ASCODEX_CHALLENGE_ID", binding.challenge_id.as_str()),
            ("ASCODEX_CYCLE_ID", binding.cycle_id.as_str()),
        ] {
            if let Some(expected) = std::env::var(name)
                .ok()
                .filter(|value| !value.trim().is_empty())
                && expected != actual
            {
                return Err(CodexErr::InvalidRequest(format!(
                    "ASCodex Chief spawn lease blocked: {name} disagrees with durable thread cycle binding"
                )));
            }
        }
        if let Some(version) = std::env::var("ASCODEX_CYCLE_EVENT_VERSION")
            .ok()
            .filter(|value| !value.trim().is_empty())
        {
            if version.parse::<u64>().ok() != Some(binding.cycle_event_version) {
                return Err(CodexErr::InvalidRequest(
                    "ASCodex Chief spawn lease blocked: cycle event version disagrees with durable thread cycle binding".into(),
                ));
            }
        }
        let _ = verify_chief_spawn_lease(
            std::path::Path::new(&ledger_file),
            &chief_lease_id,
            &binding.agent_id,
            &binding.session_id,
            &live_thread_id,
            &binding.campaign_id,
            &binding.challenge_id,
            &binding.cycle_id,
            binding.cycle_event_version,
            now_ms,
        )
        .await?;
        if chief_lease_id != binding.chief_lease_id {
            return Err(CodexErr::InvalidRequest(
                "ASCodex Chief spawn lease blocked: lease does not match durable cycle binding"
                    .into(),
            ));
        }
        Ok(binding)
    }

    /// Round-dispatch variant of `authorize_solver_spawn_from_chief`: a round Chief holds one
    /// active cycle binding per challenge on the same thread, so the child's challenge selects
    /// the parent binding and the per-challenge lease explicitly instead of the process-wide
    /// single-challenge environment. The lease/parent/session checks are identical.
    async fn authorize_solver_spawn_from_chief_for_challenge(
        &self,
        state: &Arc<ThreadManagerState>,
        parent_thread_id: ThreadId,
        challenge: &super::SolverSpawnChallenge,
    ) -> CodexResult<codex_solver_guard::ThreadCycleBinding> {
        let parent_thread = state.get_thread(parent_thread_id).await.map_err(|error| {
            CodexErr::InvalidRequest(format!(
                "ASCodex round dispatch blocked: live parent thread is unavailable: {error}"
            ))
        })?;
        if parent_thread.session.session_id() != self.session_id
            || !Arc::ptr_eq(
                &self.state,
                &parent_thread.session.services.agent_control.state,
            )
        {
            return Err(CodexErr::InvalidRequest(
                "ASCodex round dispatch blocked: parent does not belong to this live agent-control session"
                    .to_string(),
            ));
        }
        let now_ms = parent_thread
            .session
            .services
            .time_provider
            .current_time(parent_thread_id)
            .await
            .map_err(|error| {
                CodexErr::InvalidRequest(format!(
                    "ASCodex round dispatch blocked: cannot read trusted Core time: {error:#}"
                ))
            })?
            .timestamp_millis();
        let ledger_file = required_ascodex_env("ASCODEX_SOLVER_LEDGER_FILE")?;
        let live_thread_id = parent_thread_id.to_string();
        let live_session_id = parent_thread.session.session_id().to_string();
        if let Some(env_lease) = std::env::var("ASCODEX_CHIEF_LEASE_ID")
            .ok()
            .filter(|value| !value.trim().is_empty())
            && env_lease != challenge.chief_lease_id
        {
            return Err(CodexErr::InvalidRequest(
                "ASCodex round dispatch blocked: ASCODEX_CHIEF_LEASE_ID disagrees with the round plan lease"
                    .into(),
            ));
        }
        let ledger = codex_solver_guard::Ledger::open_file(std::path::Path::new(&ledger_file))
            .await
            .map_err(|error| {
                CodexErr::InvalidRequest(format!(
                    "ASCodex round dispatch blocked: cannot open existing Guard ledger: {error}"
                ))
            })?;
        let resolved = ledger
            .resolve_thread_cycle_binding_for_challenge(
                &live_thread_id,
                &live_session_id,
                codex_ascodex_coordination::Role::Chief,
                &challenge.campaign_id,
                &challenge.challenge_id,
                now_ms,
            )
            .await;
        ledger.close().await;
        let binding = resolved.map_err(|error| {
            CodexErr::InvalidRequest(format!("ASCodex round dispatch blocked: {error}"))
        })?;
        if challenge.chief_lease_id != binding.chief_lease_id {
            return Err(CodexErr::InvalidRequest(
                "ASCodex round dispatch blocked: round plan lease does not match durable cycle binding"
                    .into(),
            ));
        }
        verify_chief_spawn_lease(
            std::path::Path::new(&ledger_file),
            &challenge.chief_lease_id,
            &binding.agent_id,
            &binding.session_id,
            &live_thread_id,
            &binding.campaign_id,
            &binding.challenge_id,
            &binding.cycle_id,
            binding.cycle_event_version,
            now_ms,
        )
        .await?;
        Ok(binding)
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn verify_chief_spawn_lease(
    ledger_file: &std::path::Path,
    chief_lease_id: &str,
    live_agent_id: &str,
    live_session_id: &str,
    live_thread_id: &str,
    campaign_id: &str,
    challenge_id: &str,
    cycle_id: &str,
    cycle_event_version: u64,
    now_ms: i64,
) -> CodexResult<()> {
    let ledger = codex_solver_guard::Ledger::open_file(ledger_file)
        .await
        .map_err(|error| {
            CodexErr::InvalidRequest(format!(
                "ASCodex Chief spawn lease blocked: cannot open existing Guard ledger: {error}"
            ))
        })?;
    let result = ledger
        .resolve_chief_spawn_context(
            chief_lease_id,
            live_agent_id,
            live_session_id,
            live_thread_id,
            campaign_id,
            challenge_id,
            cycle_id,
            cycle_event_version,
            now_ms,
        )
        .await;
    ledger.close().await;
    result.map(|_| ()).map_err(|error| {
        CodexErr::InvalidRequest(format!("ASCodex Chief spawn lease blocked: {error}"))
    })
}

pub(super) fn enforce_solver_role_permissions_by_name(
    config: &mut Config,
    role_name: Option<&str>,
) -> CodexResult<()> {
    let role_name = role_name.ok_or_else(|| {
        CodexErr::InvalidRequest(
            "ASCodex solver profile requires an explicit child role for permission admission"
                .to_string(),
        )
    })?;
    let role =
        codex_ascodex_coordination::role_from_solver_role_name(role_name).ok_or_else(|| {
            CodexErr::InvalidRequest(format!(
                "ASCodex solver profile does not recognize child role `{role_name}`"
            ))
        })?;
    if role.is_read_only() {
        let narrowed = config
            .permissions
            .permission_profile()
            .intersect_with_read_only()
            .ok_or_else(|| {
                CodexErr::InvalidRequest(
                    "ASCodex read-only child requires a managed parent permission profile"
                        .to_string(),
                )
            })?;
        config
            .permissions
            .set_permission_profile(narrowed)
            .map_err(|error| {
                CodexErr::InvalidRequest(format!(
                    "ASCodex read-only child permission narrowing failed: {error}"
                ))
            })?;
    } else if !matches!(role, codex_ascodex_coordination::Role::Solver) {
        return Err(CodexErr::InvalidRequest(
            "ASCodex solver profile child role is not executable".to_string(),
        ));
    }
    Ok(())
}

/// Apply the signed StageBrief's explicit role ACL to Core's managed permission
/// profile. This must run before environment inheritance or thread startup: a
/// parent workspace root is an untrusted hint and must never widen a child.
pub(super) fn apply_stage_brief_workspace_acl(
    config: &mut Config,
    brief: &codex_ascodex_runtime::VerifiedStageBrief,
) -> CodexResult<()> {
    let parent_roots = config
        .effective_workspace_roots()
        .iter()
        .map(|root| root.to_path_buf())
        .collect::<Vec<_>>();
    let acl = codex_ascodex_runtime::role_workspace_acl(brief, &parent_roots).map_err(|error| {
        CodexErr::InvalidRequest(format!("ASCodex workspace ACL blocked: {error}"))
    })?;

    // The Windows unelevated restricted-token backend cannot enforce split
    // read or write ACLs at all (it refuses to run unsandboxed), and
    // executor_windows_sandbox_level forces RestrictedToken for any Windows
    // cwd. Attempting a split ACL there makes every StageBrief-gated spawn
    // fail at process creation. On Windows workstations keep the parent's
    // filesystem profile — the discipline boundary (workspace ownership,
    // redline, six gates, egress preflight) is enforced by the Guard gates
    // instead of the OS sandbox. Network restriction, workspace roots, and
    // cwd below still apply. Real deployments on Linux/macOS or an elevated
    // Windows backend still get the split ACL.
    let windows_skip_split_acl = cfg!(windows);
    if windows_skip_split_acl {
        config
            .permissions
            .set_permission_profile(
                PermissionProfile::from_runtime_permissions_with_enforcement(
                    SandboxEnforcement::Managed,
                    &config.permissions.file_system_sandbox_policy(),
                    codex_protocol::permissions::NetworkSandboxPolicy::Restricted,
                ),
            )
            .map_err(|error| {
                CodexErr::InvalidRequest(format!(
                    "ASCodex Windows network narrowing failed: {error}"
                ))
            })?;
    } else {
        let mut entries = Vec::with_capacity(acl.readable_roots.len() + acl.writable_roots.len());
        for root in &acl.readable_roots {
            let absolute = codex_utils_absolute_path::AbsolutePathBuf::from_absolute_path(root)
                .map_err(|_| {
                    CodexErr::InvalidRequest(
                        "ASCodex ACL contains a non-absolute readable root".into(),
                    )
                })?;
            entries.push(FileSystemSandboxEntry::new(
                FileSystemPath::from(absolute),
                FileSystemAccessMode::Read,
            ));
        }
        for root in &acl.writable_roots {
            let absolute = codex_utils_absolute_path::AbsolutePathBuf::from_absolute_path(root)
                .map_err(|_| {
                    CodexErr::InvalidRequest(
                        "ASCodex ACL contains a non-absolute writable root".into(),
                    )
                })?;
            entries.push(FileSystemSandboxEntry::new(
                FileSystemPath::from(absolute),
                FileSystemAccessMode::Write,
            ));
        }
        if entries.is_empty() {
            return Err(CodexErr::InvalidRequest(
                "ASCodex workspace ACL produced no filesystem entries".into(),
            ));
        }
        let policy = FileSystemSandboxPolicy::restricted(entries);
        // Solver profile always resolves the OS-level network sandbox to Restricted, regardless
        // of the egress allowlist: the allowlist only narrows the in-process egress preflight
        // while the sandbox layer denies the network outright as the outer boundary. The parent's
        // network mode is never inherited, so a previously-enabled parent cannot leak egress.
        let profile = PermissionProfile::from_runtime_permissions_with_enforcement(
            SandboxEnforcement::Managed,
            &policy,
            codex_protocol::permissions::NetworkSandboxPolicy::Restricted,
        );
        config
            .permissions
            .set_permission_profile(profile)
            .map_err(|error| {
                CodexErr::InvalidRequest(format!(
                    "ASCodex workspace ACL permission narrowing failed: {error}"
                ))
            })?;
    }
    let mut roots = acl.readable_roots;
    roots.extend(acl.writable_roots);
    let roots = roots
        .into_iter()
        .map(|root| {
            codex_utils_absolute_path::AbsolutePathBuf::from_absolute_path(root).map_err(|_| {
                CodexErr::InvalidRequest("ASCodex ACL contains an invalid workspace root".into())
            })
        })
        .collect::<CodexResult<Vec<_>>>()?;
    config.permissions.set_workspace_roots(roots);
    config.cwd = codex_utils_absolute_path::AbsolutePathBuf::from_absolute_path(&acl.cwd)
        .map_err(|_| CodexErr::InvalidRequest("ASCodex ACL cwd is not absolute".into()))?;
    Ok(())
}

async fn load_stage_brief_for_spawn(
    agent_role: Option<&str>,
    binding: &codex_solver_guard::ThreadCycleBinding,
) -> CodexResult<codex_ascodex_runtime::VerifiedStageBrief> {
    let role_name = agent_role.ok_or_else(|| {
        CodexErr::InvalidRequest(
            "ASCodex solver profile requires an explicit child role".to_string(),
        )
    })?;
    let role =
        codex_ascodex_coordination::role_from_solver_role_name(role_name).ok_or_else(|| {
            CodexErr::InvalidRequest(format!(
                "ASCodex solver profile does not recognize child role `{role_name}`"
            ))
        })?;
    let ledger_file = std::env::var("ASCODEX_SOLVER_LEDGER_FILE").map_err(|_| {
        CodexErr::InvalidRequest(
            "ASCodex solver profile requires ASCODEX_SOLVER_LEDGER_FILE".to_string(),
        )
    })?;
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| {
            CodexErr::InvalidRequest("ASCodex system clock precedes Unix epoch".to_string())
        })?
        .as_millis()
        .try_into()
        .map_err(|_| {
            CodexErr::InvalidRequest("ASCodex system clock is out of range".to_string())
        })?;
    codex_ascodex_runtime::load_and_render_issued_brief(
        std::path::Path::new(&ledger_file),
        codex_ascodex_runtime::IssuedStageBriefTarget {
            cycle_id: &binding.cycle_id,
            campaign_id: &binding.campaign_id,
            challenge_id: &binding.challenge_id,
            role,
        },
        now_ms,
    )
    .await
    .map_err(|error| CodexErr::InvalidRequest(format!("ASCodex stage brief blocked: {error}")))
}

/// Resolve the typed ChallengeContract pair for one challenge. Single-challenge mode keeps
/// the process-wide `ASCODEX_CONTRACT_FILE`/`ASCODEX_CONTRACT_INPUT_FILE` pair; round mode
/// sets `ASCODEX_CONTRACT_DIR` and each challenge resolves to
/// `<dir>/<challenge_id>.json` + `<dir>/<challenge_id>.fingerprint-input.json`.
/// Both modes fail closed when nothing resolvable is configured.
pub(crate) fn resolve_ascodex_contract_paths(
    challenge_id: &str,
) -> Result<(std::path::PathBuf, std::path::PathBuf), String> {
    let from_env = |name: &str| {
        std::env::var(name)
            .ok()
            .filter(|value| !value.trim().is_empty())
    };
    let contract_file = from_env("ASCODEX_CONTRACT_FILE");
    let fingerprint_input_file = from_env("ASCODEX_CONTRACT_INPUT_FILE");
    match (contract_file, fingerprint_input_file) {
        (Some(contract), Some(input)) => Ok((std::path::PathBuf::from(contract), std::path::PathBuf::from(input))),
        (None, None) => {
            let dir = from_env("ASCODEX_CONTRACT_DIR").ok_or_else(|| {
                "either ASCODEX_CONTRACT_FILE/INPUT_FILE or ASCODEX_CONTRACT_DIR must be configured"
                    .to_string()
            })?;
            if !std::path::Path::new(&dir).is_absolute() {
                return Err("ASCODEX_CONTRACT_DIR must be an absolute path".to_string());
            }
            let dir = std::path::PathBuf::from(dir);
            Ok((
                dir.join(format!("{challenge_id}.json")),
                dir.join(format!("{challenge_id}.fingerprint-input.json")),
            ))
        }
        (Some(_), None) => Err("ASCODEX_CONTRACT_INPUT_FILE is required when ASCODEX_CONTRACT_FILE is set".to_string()),
        (None, Some(_)) => Err("ASCODEX_CONTRACT_FILE is required when ASCODEX_CONTRACT_INPUT_FILE is set".to_string()),
    }
}

fn validate_contract_for_spawn(
    binding: &codex_solver_guard::ThreadCycleBinding,
    role: codex_ascodex_coordination::Role,
    now_ms: i64,
) -> CodexResult<()> {
    let (contract_file, fingerprint_input_file) =
        resolve_ascodex_contract_paths(&binding.challenge_id).map_err(|error| {
            CodexErr::InvalidRequest(format!("ASCodex contract gate blocked: {error}"))
        })?;
    codex_solver_guard::validate_contract_files(
        &contract_file,
        &fingerprint_input_file,
        &binding.challenge_id,
        Some(role),
        now_ms,
    )
    .map_err(|error| CodexErr::InvalidRequest(format!("ASCodex contract gate blocked: {error}")))
}

async fn resolve_parent_cycle_binding(
    ledger_file: &std::path::Path,
    parent_thread_id: &str,
    session_id: &str,
    now_ms: i64,
) -> CodexResult<codex_solver_guard::ThreadCycleBinding> {
    let ledger = codex_solver_guard::Ledger::open_file(ledger_file)
        .await
        .map_err(|error| {
            CodexErr::InvalidRequest(format!(
                "ASCodex cycle binding blocked: cannot open existing Guard ledger: {error}"
            ))
        })?;
    let result = ledger
        .resolve_thread_cycle_binding_for_live_thread(
            parent_thread_id,
            session_id,
            codex_ascodex_coordination::Role::Chief,
            now_ms,
        )
        .await;
    ledger.close().await;
    result.map_err(|error| {
        CodexErr::InvalidRequest(format!("ASCodex cycle binding blocked: {error}"))
    })
}

async fn resolve_thread_cycle_binding_for_resume(
    ledger_file: &str,
    thread_id: &str,
    session_id: &str,
    parent_thread_id: Option<String>,
    role: codex_ascodex_coordination::Role,
    now_ms: i64,
) -> CodexResult<codex_solver_guard::ThreadCycleBinding> {
    let ledger = codex_solver_guard::Ledger::open_file(std::path::Path::new(ledger_file))
        .await
        .map_err(|error| {
            CodexErr::InvalidRequest(format!(
                "ASCodex cycle binding blocked: cannot open existing Guard ledger: {error}"
            ))
        })?;
    let result = ledger
        .resolve_thread_cycle_binding(
            thread_id,
            &thread_id,
            session_id,
            parent_thread_id.as_deref(),
            role,
            now_ms,
        )
        .await;
    ledger.close().await;
    result.map_err(|error| {
        CodexErr::InvalidRequest(format!("ASCodex cycle binding blocked: {error}"))
    })
}

/// Recovery is a separate admission layer from cycle/brief validation. A valid persisted
/// binding proves *which* work may resume; this check proves that the current runtime instance
/// has first passed the isolated two-turn canary. Missing or stale runtime identifiers fail
/// closed, and no prompt/history text can substitute for the ledger record.
async fn ensure_recovery_canary_for_resume(
    ledger_file: &str,
    thread_id: &str,
    now_ms: i64,
) -> CodexResult<()> {
    let ledger = codex_solver_guard::Ledger::open_file(std::path::Path::new(ledger_file))
        .await
        .map_err(|error| {
            CodexErr::InvalidRequest(format!(
                "ASCodex recovery blocked: cannot open existing Guard ledger: {error}"
            ))
        })?;
    let recovery_id = required_ascodex_env("ASCODEX_RECOVERY_ID")?;
    let runtime_instance_id = required_ascodex_env("ASCODEX_RUNTIME_INSTANCE_ID")?;
    let result = ledger
        .load_recovery_canary(&recovery_id, &runtime_instance_id, now_ms)
        .await;
    ledger.close().await;
    let persisted = result.map_err(|error| {
        CodexErr::InvalidRequest(format!(
            "ASCodex recovery blocked: isolated canary is not valid for this runtime: {error}"
        ))
    })?;
    if persisted
        .trace
        .events
        .iter()
        .any(|event| event.evidence.phase() == codex_ascodex_coordination::RecoveryPhase::Failed)
    {
        return Err(CodexErr::InvalidRequest(
            "ASCodex recovery blocked: canary recorded a terminal failure".to_string(),
        ));
    }
    // The canary record is intentionally a prefix ending at CanaryPassed. Core owns the next
    // transition (rehydrate); requiring this exact prefix prevents a forged post-rehydrate
    // success record from being used as a preflight proof.
    if !persisted.trace.rehydration_allowed(now_ms) || thread_id.trim().is_empty() {
        return Err(CodexErr::InvalidRequest(
            "ASCodex recovery blocked: canary has not reached the rehydration boundary".into(),
        ));
    }
    Ok(())
}

fn required_ascodex_env(name: &str) -> CodexResult<String> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| CodexErr::InvalidRequest(format!("ASCodex solver profile requires {name}")))
}

pub(super) fn solver_resume_role(session_source: &SessionSource) -> CodexResult<&str> {
    let SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
        depth,
        agent_role: Some(role),
        ..
    }) = session_source
    else {
        return Err(CodexErr::InvalidRequest(
            "ASCodex solver resume requires a persisted ThreadSpawn source with an explicit role"
                .to_string(),
        ));
    };
    codex_solver_guard::solver_spawn_depth_preflight(*depth, true).map_err(|failure| {
        CodexErr::InvalidRequest(format!("ASCodex solver resume blocked: {}", failure.reason))
    })?;
    Ok(role.as_str())
}
