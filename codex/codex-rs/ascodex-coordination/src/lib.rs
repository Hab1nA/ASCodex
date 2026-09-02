//! Typed coordination contracts for ASCodex's research-agent loop.
//!
//! This crate deliberately contains policy and validation only. It does not spawn processes,
//! submit attempts, or contact Bohrium. The host (Codex Core/app-server) remains responsible for
//! execution; these types make the hand-off between the chief and workers explicit and auditable.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

mod auto_push;
mod contract;
mod reconciliation;
mod recovery_canary;
mod research_cycle;
mod round_plan;
mod stage_brief;
mod workspace_acl;

pub use auto_push::*;
pub use contract::*;
pub use reconciliation::*;
pub use recovery_canary::*;
pub use research_cycle::*;
pub use round_plan::*;
pub use stage_brief::*;
pub use workspace_acl::*;

pub const SCHEMA_VERSION: &str = "ascodex-coordination/v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Chief,
    Solver,
    Monitor,
    Intel,
    JudgeAnalyst,
    RedTeam,
}

impl Role {
    pub fn is_read_only(self) -> bool {
        matches!(
            self,
            Self::Monitor | Self::Intel | Self::JudgeAnalyst | Self::RedTeam
        )
    }

    pub fn approved_solver_role_name(self) -> Option<&'static str> {
        Some(match self {
            Self::Solver => "bohrium-solver",
            Self::Intel => "bohrium-intel",
            Self::JudgeAnalyst => "bohrium-judge-analyst",
            Self::RedTeam => "bohrium-red-team",
            Self::Monitor => "bohrium-monitor",
            Self::Chief => return None,
        })
    }
}

/// Resolves the only role names that the solver profile permits on child threads.
/// Keeping the mapping with the coordination contract prevents Core from accepting a
/// StageBrief for a name that has not passed the lineage allow-list.
pub fn role_from_solver_role_name(role: &str) -> Option<Role> {
    match role {
        "bohrium-solver" => Some(Role::Solver),
        "bohrium-intel" => Some(Role::Intel),
        "bohrium-judge-analyst" => Some(Role::JudgeAnalyst),
        "bohrium-red-team" => Some(Role::RedTeam),
        "bohrium-monitor" => Some(Role::Monitor),
        _ => None,
    }
}

pub fn is_approved_solver_role(role: &str) -> bool {
    [
        "bohrium-solver",
        "bohrium-intel",
        "bohrium-judge-analyst",
        "bohrium-red-team",
        "bohrium-monitor",
    ]
    .contains(&role)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    ReadEvidence,
    Decide,
    SpawnChild,
    RunLocalSmoke,
    WriteWorkspace,
    RequestSubmission,
    SubmitAttempt,
    MonitorReadOnly,
    TransitionAgent,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CoordinationError {
    #[error("role {role:?} is not authorized for action {action:?}")]
    Unauthorized { role: Role, action: Action },
    #[error("experiment plan is invalid: {0}")]
    InvalidExperiment(String),
    #[error("worker report is invalid: {0}")]
    InvalidReport(String),
    #[error("decision record is invalid: {0}")]
    InvalidDecision(String),
}

/// Check the host-side capability boundary. The chief can decide and spawn, but never executes
/// solver work or submits. Read-only observers cannot mutate a workspace or request a write.
pub fn authorize_action(role: Role, action: Action) -> Result<(), CoordinationError> {
    let allowed = match role {
        Role::Chief => matches!(
            action,
            Action::ReadEvidence | Action::Decide | Action::SpawnChild | Action::TransitionAgent
        ),
        Role::Solver => matches!(
            action,
            Action::ReadEvidence
                | Action::RunLocalSmoke
                | Action::WriteWorkspace
                | Action::RequestSubmission
                | Action::TransitionAgent
        ),
        Role::Monitor | Role::Intel | Role::JudgeAnalyst | Role::RedTeam => {
            matches!(action, Action::ReadEvidence | Action::MonitorReadOnly)
        }
    };
    if allowed {
        Ok(())
    } else {
        Err(CoordinationError::Unauthorized { role, action })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceRef {
    pub kind: String,
    pub path: String,
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExperimentPlan {
    pub schema_version: String,
    pub challenge_id: String,
    pub axis: String,
    pub changed_fields: Vec<String>,
    pub coupled_group: Option<String>,
    pub hypothesis: String,
    pub expected_response: String,
    pub decision_criterion: String,
    pub parent_attempt_id: Option<String>,
}

impl ExperimentPlan {
    pub fn validate(&self) -> Result<(), CoordinationError> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(CoordinationError::InvalidExperiment(
                "unsupported schema version".to_string(),
            ));
        }
        for (name, value) in [
            ("challenge_id", &self.challenge_id),
            ("axis", &self.axis),
            ("hypothesis", &self.hypothesis),
            ("expected_response", &self.expected_response),
            ("decision_criterion", &self.decision_criterion),
        ] {
            if value.trim().is_empty() {
                return Err(CoordinationError::InvalidExperiment(format!(
                    "{name} is required"
                )));
            }
        }
        if self.changed_fields.is_empty()
            || self
                .changed_fields
                .iter()
                .any(|field| field.trim().is_empty())
        {
            return Err(CoordinationError::InvalidExperiment(
                "changed_fields must contain at least one non-empty field".to_string(),
            ));
        }
        let unique = self.changed_fields.iter().collect::<BTreeSet<_>>();
        if unique.len() != self.changed_fields.len() {
            return Err(CoordinationError::InvalidExperiment(
                "changed_fields must not contain duplicates".to_string(),
            ));
        }
        if self.changed_fields.len() > 1 && self.coupled_group.as_deref().is_none_or(str::is_empty)
        {
            return Err(CoordinationError::InvalidExperiment(
                "multiple changed fields require an explicit coupled_group".to_string(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportStatus {
    Success,
    Blocked,
    EnvFailure,
    Timeout,
    Inconclusive,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkerReport {
    pub schema_version: String,
    pub role: Role,
    pub status: ReportStatus,
    pub challenge_id: String,
    pub identity: Option<String>,
    pub attempt_id: Option<String>,
    pub harbor_reward: Option<f64>,
    pub trace_score: Option<f64>,
    pub judge_summary: Option<String>,
    pub evidence: Vec<EvidenceRef>,
}

impl WorkerReport {
    pub fn validate(&self) -> Result<(), CoordinationError> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(CoordinationError::InvalidReport(
                "unsupported schema version".to_string(),
            ));
        }
        if self.challenge_id.trim().is_empty() {
            return Err(CoordinationError::InvalidReport(
                "challenge_id is required".to_string(),
            ));
        }
        if matches!(self.status, ReportStatus::Success) {
            if self.identity.as_deref().is_none_or(str::is_empty)
                || self.attempt_id.as_deref().is_none_or(str::is_empty)
                || self.harbor_reward.is_none()
                || self.trace_score.is_none()
                || self.judge_summary.as_deref().is_none_or(str::is_empty)
            {
                return Err(CoordinationError::InvalidReport(
                    "success requires attempt_id, identity, harbor, trace, and judge evidence"
                        .to_string(),
                ));
            }
            if self.evidence.is_empty() {
                return Err(CoordinationError::InvalidReport(
                    "success requires at least one evidence reference".to_string(),
                ));
            }
            validate_evidence_refs(&self.evidence)?;
            let kinds = self
                .evidence
                .iter()
                .map(|evidence| evidence.kind.as_str())
                .collect::<BTreeSet<_>>();
            if !["attempt", "trace", "artifact", "score"]
                .iter()
                .all(|kind| kinds.contains(kind))
            {
                return Err(CoordinationError::InvalidReport(
                    "success requires attempt, trace, artifact, and score evidence".to_string(),
                ));
            }
        }
        if let Some(score) = self.harbor_reward {
            if !score.is_finite() || !(0.0..=1.0).contains(&score) {
                return Err(CoordinationError::InvalidReport(
                    "harbor_reward must be finite and within 0..=1".to_string(),
                ));
            }
        }
        if let Some(score) = self.trace_score {
            if !score.is_finite() || !(0.0..=100.0).contains(&score) {
                return Err(CoordinationError::InvalidReport(
                    "trace_score must be finite and within 0..=100".to_string(),
                ));
            }
        }
        Ok(())
    }
}

fn validate_evidence_refs(evidence: &[EvidenceRef]) -> Result<(), CoordinationError> {
    let mut unique = BTreeSet::new();
    for reference in evidence {
        if !matches!(
            reference.kind.as_str(),
            "attempt"
                | "trace"
                | "artifact"
                | "score"
                | "verifier"
                | "baseline"
                | "stage_brief"
                | "worker_report"
                | "platform_observation"
                | "contract"
                | "closure_falsifier"
                | "closure_peer"
                | "closure_historical"
                | "closure_harbor"
                | "closure_trace"
                | "quota_snapshot"
        ) || reference.path.trim().is_empty()
            || reference.path.contains("..")
            || std::path::Path::new(&reference.path).is_absolute()
            || !unique.insert((&reference.kind, &reference.path))
        {
            return Err(CoordinationError::InvalidReport(
                "evidence kind/path must be unique, relative, and recognized".to_string(),
            ));
        }
        if let Some(sha256) = &reference.sha256 {
            if sha256.len() != 64 || !sha256.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                return Err(CoordinationError::InvalidReport(
                    "evidence sha256 must be a 64-character hexadecimal digest".to_string(),
                ));
            }
        } else {
            return Err(CoordinationError::InvalidReport(
                "evidence sha256 is required".to_string(),
            ));
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DecisionRecord {
    pub schema_version: String,
    pub decision_id: String,
    pub challenge_id: String,
    pub rationale: String,
    pub expected_outcome: String,
    pub deadline_ms: i64,
    pub evidence: Vec<EvidenceRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentRecord {
    pub agent_id: String,
    pub role: Role,
    pub state: AgentState,
    pub parent_agent_id: Option<String>,
    pub workspace: String,
    pub lease_id: Option<String>,
}

/// Runtime identity presented by Core when an agent asks the coordinator to mutate state.
/// Prompt text, role labels, and thread messages are not authority; the lease must bind the
/// caller to a concrete agent/session/thread and to the active campaign and challenge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lease {
    pub lease_id: String,
    pub campaign_id: String,
    pub challenge_id: String,
    pub owner_agent_id: String,
    pub role: Role,
    pub issued_at_ms: i64,
    pub expires_at_ms: i64,
    pub epoch: u64,
    pub allowed_actions: BTreeSet<Action>,
    pub authorized_identity_classes: BTreeSet<String>,
    pub operator_id: String,
    pub pool_epoch: u64,
    pub registration_allowed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActorContext {
    pub agent_id: String,
    pub session_id: String,
    pub thread_id: String,
    pub role: Role,
    pub campaign_id: String,
    pub challenge_id: String,
    pub lease: Lease,
}

impl ActorContext {
    pub fn validate(&self, action: Action, now_ms: i64) -> Result<(), CoordinationError> {
        authorize_action(self.role, action)?;
        let non_empty = [
            ("agent_id", self.agent_id.as_str()),
            ("session_id", self.session_id.as_str()),
            ("thread_id", self.thread_id.as_str()),
            ("campaign_id", self.campaign_id.as_str()),
            ("challenge_id", self.challenge_id.as_str()),
            ("lease_id", self.lease.lease_id.as_str()),
            ("operator_id", self.lease.operator_id.as_str()),
        ];
        if non_empty.iter().any(|(_, value)| value.trim().is_empty()) {
            return Err(CoordinationError::InvalidDecision(
                "actor context contains an empty identity field".to_string(),
            ));
        }
        if self.lease.owner_agent_id != self.agent_id
            || self.lease.role != self.role
            || self.lease.campaign_id != self.campaign_id
            || self.lease.challenge_id != self.challenge_id
        {
            return Err(CoordinationError::InvalidDecision(
                "actor context does not match its lease".to_string(),
            ));
        }
        if self.lease.issued_at_ms < 0
            || self.lease.expires_at_ms <= self.lease.issued_at_ms
            || now_ms < self.lease.issued_at_ms
            || now_ms >= self.lease.expires_at_ms
        {
            return Err(CoordinationError::InvalidDecision(
                "actor lease is outside its validity window".to_string(),
            ));
        }
        if !self.lease.allowed_actions.contains(&action) {
            return Err(CoordinationError::Unauthorized {
                role: self.role,
                action,
            });
        }
        if self.lease.registration_allowed {
            return Err(CoordinationError::InvalidDecision(
                "runtime leases cannot register new agents".to_string(),
            ));
        }
        if self.lease.authorized_identity_classes.is_empty() || self.lease.pool_epoch == 0 {
            return Err(CoordinationError::InvalidDecision(
                "lease identity pool is not frozen".to_string(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoordinatorEventRecord {
    pub event_id: String,
    pub idempotency_key: String,
    pub version: u64,
    pub actor: Role,
    pub aggregate: String,
    pub event: String,
    /// Registration payload required to replay agent membership without consulting live state.
    #[serde(default)]
    pub agent: Option<AgentRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoordinatorSnapshot {
    pub schema_version: String,
    pub campaign_id: String,
    pub challenge_id: String,
    pub campaign_state: CampaignState,
    pub state_version: u64,
    pub agents: std::collections::BTreeMap<String, AgentRecord>,
    pub events: Vec<CoordinatorEventRecord>,
}

#[derive(Debug, Clone)]
pub struct Coordinator {
    snapshot: CoordinatorSnapshot,
    applied_idempotency_keys: BTreeSet<String>,
}

impl Coordinator {
    pub fn new(campaign_id: impl Into<String>, challenge_id: impl Into<String>) -> Self {
        Self {
            snapshot: CoordinatorSnapshot {
                schema_version: SCHEMA_VERSION.to_string(),
                campaign_id: campaign_id.into(),
                challenge_id: challenge_id.into(),
                campaign_state: CampaignState::New,
                state_version: 0,
                agents: std::collections::BTreeMap::new(),
                events: Vec::new(),
            },
            applied_idempotency_keys: BTreeSet::new(),
        }
    }

    pub fn from_snapshot(snapshot: CoordinatorSnapshot) -> Result<Self, CoordinationError> {
        if snapshot.schema_version != SCHEMA_VERSION
            || snapshot.campaign_id.trim().is_empty()
            || snapshot.challenge_id.trim().is_empty()
        {
            return Err(CoordinationError::InvalidDecision(
                "coordinator snapshot has invalid identifiers or schema".to_string(),
            ));
        }
        let mut applied = BTreeSet::new();
        let mut event_ids = BTreeSet::new();
        let mut replay =
            Coordinator::new(snapshot.campaign_id.clone(), snapshot.challenge_id.clone());
        for (offset, event) in snapshot.events.iter().enumerate() {
            if event.version != offset as u64 + 1
                || event.event_id.trim().is_empty()
                || event.idempotency_key.trim().is_empty()
                || !applied.insert(event.idempotency_key.clone())
                || !event_ids.insert(event.event_id.clone())
            {
                return Err(CoordinationError::InvalidDecision(
                    "coordinator snapshot contains duplicate, empty, or non-contiguous events"
                        .to_string(),
                ));
            }
            replay.replay_event(event)?;
        }
        if snapshot.events.len() as u64 != snapshot.state_version {
            return Err(CoordinationError::InvalidDecision(
                "coordinator snapshot event count does not match state version".to_string(),
            ));
        }
        if replay.snapshot.campaign_state != snapshot.campaign_state
            || replay.snapshot.agents != snapshot.agents
        {
            return Err(CoordinationError::InvalidDecision(
                "coordinator snapshot state does not match its event log".to_string(),
            ));
        }
        Ok(Self {
            snapshot,
            applied_idempotency_keys: applied,
        })
    }

    pub fn snapshot(&self) -> &CoordinatorSnapshot {
        &self.snapshot
    }

    #[cfg(test)]
    pub(crate) fn register_agent(
        &mut self,
        actor: Role,
        agent: AgentRecord,
        expected_version: u64,
        event_id: &str,
        idempotency_key: &str,
    ) -> Result<u64, CoordinationError> {
        if actor != Role::Chief {
            return Err(CoordinationError::Unauthorized {
                role: actor,
                action: Action::SpawnChild,
            });
        }
        let event = format!("agent:{}:registered", agent.agent_id);
        if self.applied_idempotency_keys.contains(idempotency_key) {
            return self.replay_idempotent(
                idempotency_key,
                event_id,
                Role::Chief,
                &event,
                Some(&agent),
            );
        }
        self.validate_agent_registration(&agent)?;
        self.commit_event(
            actor,
            expected_version,
            event_id,
            idempotency_key,
            event,
            Some(agent.clone()),
            |snapshot| {
                snapshot.agents.insert(agent.agent_id.clone(), agent);
            },
        )
    }

    pub(crate) fn transition_campaign(
        &mut self,
        actor: Role,
        event: CampaignEvent,
        expected_version: u64,
        event_id: &str,
        idempotency_key: &str,
    ) -> Result<u64, CoordinationError> {
        let permitted = match event {
            CampaignEvent::EvidenceReady | CampaignEvent::TransportAccepted => {
                matches!(actor, Role::Solver)
            }
            CampaignEvent::ScoreObserved => matches!(actor, Role::Monitor | Role::JudgeAnalyst),
            CampaignEvent::ScoreVerified => matches!(actor, Role::JudgeAnalyst),
            _ => actor == Role::Chief,
        };
        if !permitted {
            return Err(CoordinationError::Unauthorized {
                role: actor,
                action: Action::Decide,
            });
        }
        let event_name = format!("campaign:{event:?}");
        if self.applied_idempotency_keys.contains(idempotency_key) {
            return self.replay_idempotent(idempotency_key, event_id, actor, &event_name, None);
        }
        let next = self.snapshot.campaign_state.transition(event)?;
        self.commit_event(
            actor,
            expected_version,
            event_id,
            idempotency_key,
            event_name,
            None,
            |snapshot| snapshot.campaign_state = next,
        )
    }

    /// Context-bound campaign transition used by Core/app-server. The legacy role-only method
    /// remains available for snapshot migration tests, but production dispatch should use this
    /// entry so a sibling session cannot impersonate the same role.
    pub fn transition_campaign_with_context(
        &mut self,
        context: &ActorContext,
        event: CampaignEvent,
        expected_version: u64,
        event_id: &str,
        idempotency_key: &str,
        now_ms: i64,
    ) -> Result<u64, CoordinationError> {
        context.validate(Action::Decide, now_ms)?;
        if context.campaign_id != self.snapshot.campaign_id
            || context.challenge_id != self.snapshot.challenge_id
        {
            return Err(CoordinationError::InvalidDecision(
                "actor context is bound to a different campaign or challenge".to_string(),
            ));
        }
        self.transition_campaign(
            context.role,
            event,
            expected_version,
            event_id,
            idempotency_key,
        )
    }

    pub(crate) fn transition_agent(
        &mut self,
        actor: Role,
        agent_id: &str,
        event: AgentEvent,
        expected_version: u64,
        event_id: &str,
        idempotency_key: &str,
    ) -> Result<u64, CoordinationError> {
        let event_name = format!("agent:{agent_id}:{event:?}");
        if self.applied_idempotency_keys.contains(idempotency_key) {
            return self.replay_idempotent(idempotency_key, event_id, actor, &event_name, None);
        }
        let agent = self.snapshot.agents.get(agent_id).ok_or_else(|| {
            CoordinationError::InvalidDecision("agent is not registered".to_string())
        })?;
        let self_action = matches!(
            event,
            AgentEvent::WaitInput
                | AgentEvent::WaitScore
                | AgentEvent::RequestHandoff
                | AgentEvent::ProposeEnd
                | AgentEvent::Crash
        );
        if self_action && actor != agent.role {
            return Err(CoordinationError::Unauthorized {
                role: actor,
                action: Action::Decide,
            });
        }
        if !self_action && actor != Role::Chief {
            return Err(CoordinationError::Unauthorized {
                role: actor,
                action: Action::Decide,
            });
        }
        let next = agent.state.transition(event)?;
        self.commit_event(
            actor,
            expected_version,
            event_id,
            idempotency_key,
            event_name,
            None,
            |snapshot| {
                if let Some(agent) = snapshot.agents.get_mut(agent_id) {
                    agent.state = next;
                }
            },
        )
    }

    /// Context-bound agent transition. Non-chief actors may only transition themselves; the
    /// chief may operate registered children through an explicitly leased chief session.
    pub fn transition_agent_with_context(
        &mut self,
        context: &ActorContext,
        agent_id: &str,
        event: AgentEvent,
        expected_version: u64,
        event_id: &str,
        idempotency_key: &str,
        now_ms: i64,
    ) -> Result<u64, CoordinationError> {
        context.validate(Action::TransitionAgent, now_ms)?;
        if context.campaign_id != self.snapshot.campaign_id
            || context.challenge_id != self.snapshot.challenge_id
        {
            return Err(CoordinationError::InvalidDecision(
                "actor context is bound to a different campaign or challenge".to_string(),
            ));
        }
        let target = self.snapshot.agents.get(agent_id).ok_or_else(|| {
            CoordinationError::InvalidDecision("agent is not registered".to_string())
        })?;
        if context.role != Role::Chief && context.agent_id != agent_id {
            return Err(CoordinationError::Unauthorized {
                role: context.role,
                action: Action::TransitionAgent,
            });
        }
        if context.role == Role::Chief && target.role == Role::Chief && context.agent_id != agent_id
        {
            return Err(CoordinationError::Unauthorized {
                role: context.role,
                action: Action::TransitionAgent,
            });
        }
        self.transition_agent(
            context.role,
            agent_id,
            event,
            expected_version,
            event_id,
            idempotency_key,
        )
    }

    fn commit_event(
        &mut self,
        actor: Role,
        expected_version: u64,
        event_id: &str,
        idempotency_key: &str,
        event: String,
        agent: Option<AgentRecord>,
        apply: impl FnOnce(&mut CoordinatorSnapshot),
    ) -> Result<u64, CoordinationError> {
        if event_id.trim().is_empty() || idempotency_key.trim().is_empty() {
            return Err(CoordinationError::InvalidDecision(
                "event id and idempotency key are required".to_string(),
            ));
        }
        if self.applied_idempotency_keys.contains(idempotency_key) {
            return Ok(self.snapshot.state_version);
        }
        if self
            .snapshot
            .events
            .iter()
            .any(|record| record.event_id == event_id)
        {
            return Err(CoordinationError::InvalidDecision(
                "event id was reused for a different operation".to_string(),
            ));
        }
        if expected_version != self.snapshot.state_version {
            return Err(CoordinationError::InvalidDecision(format!(
                "coordinator version conflict: expected {}, found {}",
                expected_version, self.snapshot.state_version
            )));
        }
        apply(&mut self.snapshot);
        self.snapshot.state_version += 1;
        self.snapshot.events.push(CoordinatorEventRecord {
            event_id: event_id.to_string(),
            idempotency_key: idempotency_key.to_string(),
            version: self.snapshot.state_version,
            actor,
            aggregate: self.snapshot.campaign_id.clone(),
            event,
            agent,
        });
        self.applied_idempotency_keys
            .insert(idempotency_key.to_string());
        Ok(self.snapshot.state_version)
    }

    fn replay_idempotent(
        &self,
        idempotency_key: &str,
        event_id: &str,
        actor: Role,
        event: &str,
        agent: Option<&AgentRecord>,
    ) -> Result<u64, CoordinationError> {
        let record = self
            .snapshot
            .events
            .iter()
            .find(|record| record.idempotency_key == idempotency_key)
            .expect("idempotency key is indexed only after an event is recorded");
        if record.event_id == event_id
            && record.actor == actor
            && record.event == event
            && record.agent.as_ref() == agent
        {
            Ok(self.snapshot.state_version)
        } else {
            Err(CoordinationError::InvalidDecision(
                "idempotency key was reused with a different event".to_string(),
            ))
        }
    }

    fn replay_event(&mut self, record: &CoordinatorEventRecord) -> Result<(), CoordinationError> {
        if let Some(agent) = &record.agent {
            let expected = format!("agent:{}:registered", agent.agent_id);
            if record.event != expected || record.actor != Role::Chief {
                return Err(CoordinationError::InvalidDecision(
                    "invalid agent registration event".to_string(),
                ));
            }
            self.validate_agent_registration(agent)?;
            self.snapshot
                .agents
                .insert(agent.agent_id.clone(), agent.clone());
        } else if let Some(label) = record.event.strip_prefix("campaign:") {
            let event = campaign_event_from_debug(label).ok_or_else(|| {
                CoordinationError::InvalidDecision("unknown campaign event in snapshot".to_string())
            })?;
            let permitted = match event {
                CampaignEvent::EvidenceReady | CampaignEvent::TransportAccepted => {
                    record.actor == Role::Solver
                }
                CampaignEvent::ScoreObserved => {
                    matches!(record.actor, Role::Monitor | Role::JudgeAnalyst)
                }
                CampaignEvent::ScoreVerified => record.actor == Role::JudgeAnalyst,
                _ => record.actor == Role::Chief,
            };
            if !permitted {
                return Err(CoordinationError::InvalidDecision(
                    "unauthorized campaign event in snapshot".to_string(),
                ));
            }
            self.snapshot.campaign_state = self.snapshot.campaign_state.transition(event)?;
        } else if let Some(rest) = record.event.strip_prefix("agent:") {
            let (agent_id, label) = rest.split_once(':').ok_or_else(|| {
                CoordinationError::InvalidDecision("malformed agent event in snapshot".to_string())
            })?;
            let event = agent_event_from_debug(label).ok_or_else(|| {
                CoordinationError::InvalidDecision("unknown agent event in snapshot".to_string())
            })?;
            let agent = self.snapshot.agents.get(agent_id).ok_or_else(|| {
                CoordinationError::InvalidDecision(
                    "agent event references unknown agent".to_string(),
                )
            })?;
            let self_action = matches!(
                event,
                AgentEvent::WaitInput
                    | AgentEvent::WaitScore
                    | AgentEvent::RequestHandoff
                    | AgentEvent::ProposeEnd
                    | AgentEvent::Crash
            );
            if (self_action && record.actor != agent.role)
                || (!self_action && record.actor != Role::Chief)
            {
                return Err(CoordinationError::InvalidDecision(
                    "unauthorized agent event in snapshot".to_string(),
                ));
            }
            let next = agent.state.transition(event)?;
            self.snapshot
                .agents
                .get_mut(agent_id)
                .expect("agent was checked above")
                .state = next;
        } else {
            return Err(CoordinationError::InvalidDecision(
                "unknown event aggregate in snapshot".to_string(),
            ));
        }
        self.snapshot.state_version += 1;
        self.snapshot.events.push(record.clone());
        self.applied_idempotency_keys
            .insert(record.idempotency_key.clone());
        Ok(())
    }

    fn validate_agent_registration(&self, agent: &AgentRecord) -> Result<(), CoordinationError> {
        let invalid_chief = agent.role == Role::Chief
            && (agent.parent_agent_id.is_some()
                || agent.lease_id.is_some()
                || self
                    .snapshot
                    .agents
                    .values()
                    .any(|candidate| candidate.role == Role::Chief));
        let invalid_child =
            agent.role != Role::Chief
                && (agent.parent_agent_id.is_none()
                    || agent.lease_id.as_deref().is_none_or(str::is_empty)
                    || !self
                        .snapshot
                        .agents
                        .get(agent.parent_agent_id.as_deref().unwrap_or_default())
                        .is_some_and(|parent| parent.role == Role::Chief)
                    || self.snapshot.agents.values().any(|candidate| {
                        candidate.lease_id.as_deref() == agent.lease_id.as_deref()
                    }));
        if agent.agent_id.trim().is_empty()
            || agent.workspace.trim().is_empty()
            || agent.state != AgentState::Registered
            || self.snapshot.agents.contains_key(&agent.agent_id)
            || invalid_chief
            || invalid_child
        {
            return Err(CoordinationError::InvalidDecision(
                "agent registration has invalid identity, state, lineage, workspace, or lease"
                    .to_string(),
            ));
        }
        Ok(())
    }
}

fn campaign_event_from_debug(label: &str) -> Option<CampaignEvent> {
    Some(match label {
        "IntakeAccepted" => CampaignEvent::IntakeAccepted,
        "VerifierFrozen" => CampaignEvent::VerifierFrozen,
        "PlanApproved" => CampaignEvent::PlanApproved,
        "ExecutionStarted" => CampaignEvent::ExecutionStarted,
        "EvidenceReady" => CampaignEvent::EvidenceReady,
        "PreflightPassed" => CampaignEvent::PreflightPassed,
        "TransportAccepted" => CampaignEvent::TransportAccepted,
        "SubmissionDispatched" => CampaignEvent::SubmissionDispatched,
        "ScoreObserved" => CampaignEvent::ScoreObserved,
        "ScoreVerified" => CampaignEvent::ScoreVerified,
        "ReplanRequired" => CampaignEvent::ReplanRequired,
        "ClosureReviewStarted" => CampaignEvent::ClosureReviewStarted,
        "ClosureApproved" => CampaignEvent::ClosureApproved,
        "Abort" => CampaignEvent::Abort,
        _ => return None,
    })
}

fn agent_event_from_debug(label: &str) -> Option<AgentEvent> {
    Some(match label {
        "Brief" => AgentEvent::Brief,
        "Activate" => AgentEvent::Activate,
        "WaitInput" => AgentEvent::WaitInput,
        "WaitScore" => AgentEvent::WaitScore,
        "RequestHandoff" => AgentEvent::RequestHandoff,
        "ProposeEnd" => AgentEvent::ProposeEnd,
        "AcceptEnd" => AgentEvent::AcceptEnd,
        "Suspend" => AgentEvent::Suspend,
        "Crash" => AgentEvent::Crash,
        "Recover" => AgentEvent::Recover,
        "Abort" => AgentEvent::Abort,
        _ => return None,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CampaignState {
    New,
    Intake,
    Oriented,
    Planned,
    Executing,
    EvidenceReady,
    Preflight,
    Submitting,
    AwaitingScore,
    Observed,
    ScoreVerified,
    ClosureReview,
    Replan,
    Closed,
    Aborted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CampaignEvent {
    IntakeAccepted,
    VerifierFrozen,
    PlanApproved,
    ExecutionStarted,
    EvidenceReady,
    PreflightPassed,
    TransportAccepted,
    SubmissionDispatched,
    ScoreObserved,
    ScoreVerified,
    ReplanRequired,
    ClosureReviewStarted,
    ClosureApproved,
    Abort,
}

impl CampaignState {
    /// Advance the durable campaign state. Every transition is explicit so a replay cannot turn a
    /// transport acknowledgement or queued attempt into an official score.
    pub fn transition(self, event: CampaignEvent) -> Result<Self, CoordinationError> {
        use CampaignEvent as E;
        use CampaignState as S;
        let next = match (self, event) {
            (S::New, E::IntakeAccepted) => S::Intake,
            (S::Intake, E::VerifierFrozen) => S::Oriented,
            (S::Oriented, E::PlanApproved) => S::Planned,
            (S::Planned, E::ExecutionStarted) => S::Executing,
            (S::Executing, E::EvidenceReady) => S::EvidenceReady,
            (S::EvidenceReady, E::PreflightPassed) => S::Preflight,
            (S::Preflight, E::TransportAccepted) => S::Submitting,
            (S::Submitting, E::SubmissionDispatched) => S::AwaitingScore,
            (S::AwaitingScore, E::ScoreObserved) => S::Observed,
            (S::Observed, E::ScoreVerified) => S::ScoreVerified,
            (S::Observed, E::ReplanRequired) | (S::ScoreVerified, E::ReplanRequired) => S::Replan,
            (S::Replan, E::PlanApproved) => S::Planned,
            (S::ScoreVerified, E::ClosureReviewStarted) => S::ClosureReview,
            (S::ClosureReview, E::ClosureApproved) => S::Closed,
            (
                S::New
                | S::Intake
                | S::Oriented
                | S::Planned
                | S::Executing
                | S::EvidenceReady
                | S::Preflight
                | S::Submitting
                | S::AwaitingScore
                | S::Observed
                | S::ScoreVerified
                | S::ClosureReview
                | S::Replan,
                E::Abort,
            ) => S::Aborted,
            _ => {
                return Err(CoordinationError::InvalidDecision(format!(
                    "illegal campaign transition: {self:?} + {event:?}"
                )));
            }
        };
        Ok(next)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentState {
    Registered,
    Briefed,
    Active,
    WaitingInput,
    WaitingScore,
    HandoffPending,
    EndProposed,
    EndAccepted,
    Suspended,
    Crashed,
    Recovering,
    Aborted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentEvent {
    Brief,
    Activate,
    WaitInput,
    WaitScore,
    RequestHandoff,
    ProposeEnd,
    AcceptEnd,
    Suspend,
    Crash,
    Recover,
    Abort,
}

impl AgentState {
    pub fn transition(self, event: AgentEvent) -> Result<Self, CoordinationError> {
        use AgentEvent::*;
        use AgentState::*;
        let next = match (self, event) {
            (Registered, Brief) => Briefed,
            (Briefed, Activate) | (Recovering, Activate) => Active,
            (Active, WaitInput) => WaitingInput,
            (Active, WaitScore) => WaitingScore,
            (Active, RequestHandoff) => HandoffPending,
            (HandoffPending, ProposeEnd) => EndProposed,
            (EndProposed, AcceptEnd) => EndAccepted,
            (Active | WaitingInput | WaitingScore, Suspend) => Suspended,
            (Active | WaitingInput | WaitingScore, Crash) => Crashed,
            (Crashed | Suspended, Recover) => Recovering,
            (
                Registered | Briefed | Active | WaitingInput | WaitingScore | HandoffPending
                | EndProposed | Suspended | Crashed | Recovering,
                Abort,
            ) => Aborted,
            _ => {
                return Err(CoordinationError::InvalidDecision(format!(
                    "illegal agent transition: {self:?} + {event:?}"
                )));
            }
        };
        Ok(next)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttemptState {
    Candidate,
    Preflight,
    Reserved,
    TransportSent,
    PendingParse,
    ScoreObserved,
    PendingReview,
    Backfilled,
    LeaderboardConfirmed,
    OfficialConfirmed,
    Stuck,
    FailedRetryable,
    FailedTerminal,
    Released,
    Committed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttemptEvent {
    PreflightStarted,
    ReservationHeld,
    TransportAccepted,
    ParsePending,
    ScoreReceived,
    ReviewPending,
    BackfillReceived,
    LeaderboardSeen,
    OfficialConfirm,
    Commit,
    Release,
    RetryableFailure,
    TerminalFailure,
    Stuck,
}

impl AttemptState {
    /// Apply only legal attempt transitions. Transport acceptance, score observation, and
    /// official confirmation remain distinct so a queued request cannot be mistaken for success.
    pub fn transition(self, event: AttemptEvent) -> Result<Self, CoordinationError> {
        use AttemptEvent as E;
        use AttemptState as S;
        let next = match (self, event) {
            (S::Candidate, E::PreflightStarted) => S::Preflight,
            (S::Preflight, E::ReservationHeld) => S::Reserved,
            (S::Reserved, E::TransportAccepted) => S::TransportSent,
            (S::TransportSent, E::ParsePending) => S::PendingParse,
            (S::PendingParse, E::ScoreReceived) => S::ScoreObserved,
            (S::ScoreObserved, E::ReviewPending) => S::PendingReview,
            (S::PendingReview, E::BackfillReceived) => S::Backfilled,
            (S::ScoreObserved | S::Backfilled, E::LeaderboardSeen) => S::LeaderboardConfirmed,
            (S::LeaderboardConfirmed, E::OfficialConfirm) => S::OfficialConfirmed,
            (S::OfficialConfirmed, E::Commit) => S::Committed,
            (S::Candidate | S::Preflight | S::Reserved, E::Release) => S::Released,
            (
                S::Candidate | S::Preflight | S::Reserved | S::TransportSent | S::PendingParse,
                E::RetryableFailure,
            ) => S::FailedRetryable,
            (
                S::Candidate | S::Preflight | S::Reserved | S::TransportSent | S::PendingParse,
                E::TerminalFailure,
            ) => S::FailedTerminal,
            (
                S::Candidate | S::Preflight | S::Reserved | S::PendingParse | S::PendingReview,
                E::Stuck,
            ) => S::Stuck,
            _ => {
                return Err(CoordinationError::InvalidReport(format!(
                    "illegal attempt transition: {self:?} + {event:?}"
                )));
            }
        };
        Ok(next)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OfficialEvidence {
    pub challenge_id: String,
    pub replay_executed: bool,
    pub results_populated: bool,
    pub scorecard_populated: bool,
    pub harbor_reward: f64,
    pub leaderboard_seen: bool,
    /// Explicit platform outcome for fields that may be null/redacted during delayed scoring.
    /// `None` preserves the legacy requirement that the corresponding boolean is true.
    #[serde(default)]
    pub results_json_status: Option<EvidenceAvailability>,
    #[serde(default)]
    pub scorecard_status: Option<EvidenceAvailability>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceAvailability {
    Present,
    Redacted,
    Pending,
    Unavailable,
    NotApplicable,
}

/// Immutable observation captured by a read-only platform monitor. A score is not considered
/// official until the attempt response, replay/result path, scorecard path, and leaderboard
/// publication are all represented independently in this record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlatformObservation {
    pub attempt_id: String,
    pub challenge_id: String,
    pub route: String,
    pub observed_at_ms: i64,
    pub response_sha256: String,
    pub replay_status: EvidenceAvailability,
    pub results_status: EvidenceAvailability,
    pub scorecard_status: EvidenceAvailability,
    pub leaderboard_status: EvidenceAvailability,
    pub harbor_reward: Option<f64>,
    pub trace_score: Option<f64>,
}

impl PlatformObservation {
    pub fn validate(
        &self,
        expected_challenge_id: &str,
        now_ms: i64,
    ) -> Result<(), CoordinationError> {
        if self.attempt_id.trim().is_empty()
            || self.challenge_id != expected_challenge_id
            || self.route.trim().is_empty()
            || self.observed_at_ms < 0
            || self.observed_at_ms > now_ms
            || self.response_sha256.len() != 64
            || !self
                .response_sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
            || self.response_sha256 != self.response_sha256.to_ascii_lowercase()
            || self.replay_status != EvidenceAvailability::Present
            || !matches!(
                self.results_status,
                EvidenceAvailability::Present | EvidenceAvailability::Redacted
            )
            || !matches!(
                self.scorecard_status,
                EvidenceAvailability::Present | EvidenceAvailability::Redacted
            )
            || self.leaderboard_status != EvidenceAvailability::Present
        {
            return Err(CoordinationError::InvalidReport(
                "platform observation lacks a complete, bound, and non-future evidence set"
                    .to_string(),
            ));
        }
        let reward = self.harbor_reward.ok_or_else(|| {
            CoordinationError::InvalidReport("platform observation requires harbor_reward".into())
        })?;
        if !reward.is_finite() || !(0.0..=1.0).contains(&reward) {
            return Err(CoordinationError::InvalidReport(
                "platform harbor_reward must be finite and within 0..=1".to_string(),
            ));
        }
        if let Some(trace_score) = self.trace_score
            && (!trace_score.is_finite() || !(0.0..=100.0).contains(&trace_score))
        {
            return Err(CoordinationError::InvalidReport(
                "platform trace_score must be finite and within 0..=100".to_string(),
            ));
        }
        Ok(())
    }
}

pub fn confirm_attempt_observation(
    state: AttemptState,
    observation: &PlatformObservation,
    expected_challenge_id: &str,
    now_ms: i64,
) -> Result<AttemptState, CoordinationError> {
    if state != AttemptState::ScoreObserved {
        return Err(CoordinationError::InvalidReport(
            "only a score-observed attempt may be officially confirmed".to_string(),
        ));
    }
    observation.validate(expected_challenge_id, now_ms)?;
    Ok(AttemptState::LeaderboardConfirmed)
}

impl OfficialEvidence {
    pub fn validate(&self, expected_challenge_id: &str) -> Result<(), CoordinationError> {
        let results_ok = match self.results_json_status {
            Some(EvidenceAvailability::Present | EvidenceAvailability::Redacted) => true,
            Some(
                EvidenceAvailability::Pending
                | EvidenceAvailability::Unavailable
                | EvidenceAvailability::NotApplicable,
            ) => false,
            None => self.results_populated,
        };
        let scorecard_ok = match self.scorecard_status {
            Some(EvidenceAvailability::Present | EvidenceAvailability::Redacted) => true,
            Some(
                EvidenceAvailability::Pending
                | EvidenceAvailability::Unavailable
                | EvidenceAvailability::NotApplicable,
            ) => false,
            None => self.scorecard_populated,
        };
        if self.challenge_id != expected_challenge_id
            || !self.replay_executed
            || !results_ok
            || !scorecard_ok
            || !self.leaderboard_seen
            || !self.harbor_reward.is_finite()
            || !(0.0..=1.0).contains(&self.harbor_reward)
        {
            return Err(CoordinationError::InvalidReport(
                "official confirmation requires challenge, replay, resolved results/scorecard status, reward, and leaderboard evidence".to_string(),
            ));
        }
        Ok(())
    }
}

#[deprecated(note = "use confirm_attempt_observation with typed platform evidence")]
pub fn confirm_attempt(
    state: AttemptState,
    _evidence: &OfficialEvidence,
    _expected_challenge_id: &str,
) -> Result<AttemptState, CoordinationError> {
    if state != AttemptState::ScoreObserved {
        return Err(CoordinationError::InvalidReport(
            "legacy boolean confirmation is disabled; use typed platform observation".to_string(),
        ));
    }
    Err(CoordinationError::InvalidReport(
        "legacy boolean confirmation is disabled; use typed platform observation".to_string(),
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageType {
    Command,
    Report,
    Event,
    Query,
    Response,
    Control,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Topic {
    Assignment,
    Experiment,
    Score,
    Stuck,
    Handoff,
    Shutdown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Party {
    pub session_id: String,
    pub thread_id: String,
    pub role: Role,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageEvidence {
    pub artifact_id: String,
    pub sha256: String,
    pub path: String,
    pub provenance: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MessageEnvelope {
    pub schema_version: String,
    pub message_id: String,
    pub message_type: MessageType,
    pub topic: Topic,
    pub campaign_id: String,
    pub challenge_id: Option<String>,
    pub sender: Party,
    pub recipient: Party,
    pub parent_message_id: Option<String>,
    pub correlation_id: String,
    pub causation_id: Option<String>,
    pub stream_seq: u64,
    pub issued_at_ms: i64,
    pub expires_at_ms: i64,
    pub priority: u8,
    pub requires_ack: bool,
    pub trigger_turn: bool,
    pub lease_id: Option<String>,
    pub authority: String,
    pub expected_state_version: u64,
    pub idempotency_key: String,
    pub payload: String,
    pub evidence: Vec<MessageEvidence>,
}

impl MessageEnvelope {
    pub fn validate(&self, now_ms: i64) -> Result<(), CoordinationError> {
        if self.schema_version != SCHEMA_VERSION
            || self.message_id.trim().is_empty()
            || self.campaign_id.trim().is_empty()
            || self.correlation_id.trim().is_empty()
            || self.idempotency_key.trim().is_empty()
            || self.authority.trim().is_empty()
            || self.payload.trim().is_empty()
            || self.sender.session_id.trim().is_empty()
            || self.sender.thread_id.trim().is_empty()
            || self.recipient.session_id.trim().is_empty()
            || self.recipient.thread_id.trim().is_empty()
        {
            return Err(CoordinationError::InvalidDecision(
                "message envelope identifiers and payload are required".to_string(),
            ));
        }
        if self.challenge_id.as_deref().is_some_and(str::is_empty) {
            return Err(CoordinationError::InvalidDecision(
                "challenge_id cannot be empty when present".to_string(),
            ));
        }
        if self.expires_at_ms <= self.issued_at_ms || now_ms > self.expires_at_ms {
            return Err(CoordinationError::InvalidDecision(
                "message is expired or has an invalid lifetime".to_string(),
            ));
        }
        if self.priority > 3 {
            return Err(CoordinationError::InvalidDecision(
                "message priority must be within 0..=3".to_string(),
            ));
        }
        if self.trigger_turn
            && !matches!(
                self.message_type,
                MessageType::Command | MessageType::Control
            )
        {
            return Err(CoordinationError::InvalidDecision(
                "only command/control messages may trigger a turn".to_string(),
            ));
        }
        if self.trigger_turn && !matches!(self.authority.as_str(), "user" | "chief") {
            return Err(CoordinationError::InvalidDecision(
                "triggering a turn requires user or chief authority".to_string(),
            ));
        }
        for evidence in &self.evidence {
            if evidence.artifact_id.trim().is_empty()
                || evidence.provenance.trim().is_empty()
                || evidence.path.trim().is_empty()
                || evidence.path.contains("..")
                || std::path::Path::new(&evidence.path).is_absolute()
                || evidence.sha256.len() != 64
                || evidence.sha256 != evidence.sha256.to_ascii_lowercase()
                || !evidence.sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
            {
                return Err(CoordinationError::InvalidDecision(
                    "message evidence must be hashed, relative, and provenance-bound".to_string(),
                ));
            }
        }
        Ok(())
    }

    /// Runtime-bound validation for messages that may mutate state or wake a thread. A model
    /// supplied role/authority string is never sufficient; Core must present the live lease.
    pub fn validate_for_context(
        &self,
        context: &ActorContext,
        now_ms: i64,
    ) -> Result<(), CoordinationError> {
        self.validate(now_ms)?;
        let action = match context.role {
            Role::Chief => Action::Decide,
            Role::Solver => Action::WriteWorkspace,
            Role::Monitor | Role::Intel | Role::JudgeAnalyst | Role::RedTeam => {
                Action::MonitorReadOnly
            }
        };
        context.validate(action, now_ms)?;
        let expected_challenge = self.challenge_id.as_deref().unwrap_or("");
        if self.sender.session_id != context.session_id
            || self.sender.thread_id != context.thread_id
            || self.sender.role != context.role
            || self.campaign_id != context.campaign_id
            || expected_challenge != context.challenge_id
            || self.lease_id.as_deref() != Some(context.lease.lease_id.as_str())
            || (self.trigger_turn && (context.role != Role::Chief || self.authority != "chief"))
        {
            return Err(CoordinationError::Unauthorized {
                role: context.role,
                action: Action::Decide,
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReceiptStatus {
    Accepted,
    Delivered,
    Acked,
    Applied,
    Rejected,
    Expired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReceiptEvent {
    Deliver,
    Ack,
    Apply,
    Reject,
    Expire,
}

impl ReceiptStatus {
    pub fn transition(self, event: ReceiptEvent) -> Result<Self, CoordinationError> {
        use ReceiptEvent as E;
        use ReceiptStatus as S;
        let next = match (self, event) {
            (S::Accepted, E::Deliver) => S::Delivered,
            (S::Delivered, E::Ack) => S::Acked,
            (S::Acked, E::Apply) => S::Applied,
            (S::Accepted | S::Delivered, E::Reject) => S::Rejected,
            (S::Accepted | S::Delivered, E::Expire) => S::Expired,
            _ => {
                return Err(CoordinationError::InvalidDecision(format!(
                    "illegal receipt transition: {self:?} + {event:?}"
                )));
            }
        };
        Ok(next)
    }
}

impl DecisionRecord {
    pub fn validate(&self, now_ms: i64) -> Result<(), CoordinationError> {
        if self.schema_version != SCHEMA_VERSION
            || self.decision_id.trim().is_empty()
            || self.challenge_id.trim().is_empty()
            || self.rationale.trim().is_empty()
            || self.expected_outcome.trim().is_empty()
        {
            return Err(CoordinationError::InvalidDecision(
                "schema, identifiers, rationale, and expected_outcome are required".to_string(),
            ));
        }
        if self.deadline_ms <= now_ms {
            return Err(CoordinationError::InvalidDecision(
                "deadline must be in the future".to_string(),
            ));
        }
        if self.evidence.is_empty() {
            return Err(CoordinationError::InvalidDecision(
                "every decision must cite at least one evidence reference".to_string(),
            ));
        }
        Ok(())
    }
}

/// The durable hand-off for one chief-led OODA cycle.  Prompts may suggest a phase or action,
/// but only this validated record is allowed to advance the coordination state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OodaPhase {
    Observe,
    Orient,
    Decide,
    Act,
    Review,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CycleDirective {
    Continue,
    Replan,
    DispatchJudgeAnalyst,
    DispatchRedTeam,
    EscalateStuckReview,
    ClosureReview,
    ApproveClosure,
    Abort,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OodaCycleRecord {
    pub schema_version: String,
    pub cycle_id: String,
    pub campaign_id: String,
    pub challenge_id: String,
    pub phase: OodaPhase,
    pub actor_role: Role,
    pub directive: CycleDirective,
    pub rationale: String,
    pub expected_state_version: u64,
    pub deadline_ms: i64,
    pub stuck_triggers: Vec<StuckTrigger>,
    pub evidence: Vec<EvidenceRef>,
}

impl OodaCycleRecord {
    pub fn validate(&self, now_ms: i64) -> Result<(), CoordinationError> {
        if self.schema_version != SCHEMA_VERSION
            || self.cycle_id.trim().is_empty()
            || self.campaign_id.trim().is_empty()
            || self.challenge_id.trim().is_empty()
            || self.rationale.trim().is_empty()
            || self.deadline_ms <= now_ms
        {
            return Err(CoordinationError::InvalidDecision(
                "OODA cycle requires valid identifiers, rationale, and a future deadline"
                    .to_string(),
            ));
        }
        if self.evidence.is_empty() {
            return Err(CoordinationError::InvalidDecision(
                "OODA cycle must cite evidence".to_string(),
            ));
        }
        validate_evidence_refs(&self.evidence)?;
        if self.actor_role != Role::Chief
            || !matches!(self.phase, OodaPhase::Decide | OodaPhase::Review)
        {
            return Err(CoordinationError::Unauthorized {
                role: self.actor_role,
                action: Action::Decide,
            });
        }
        if !self.stuck_triggers.is_empty()
            && !matches!(
                self.directive,
                CycleDirective::Replan
                    | CycleDirective::EscalateStuckReview
                    | CycleDirective::Abort
            )
        {
            return Err(CoordinationError::InvalidDecision(
                "stuck evidence requires replan, abort, or an atomic heterogeneous review"
                    .to_string(),
            ));
        }
        if matches!(
            self.directive,
            CycleDirective::ClosureReview | CycleDirective::ApproveClosure
        ) && self.phase != OodaPhase::Review
        {
            return Err(CoordinationError::InvalidDecision(
                "closure review is only valid in the review phase".to_string(),
            ));
        }
        if matches!(self.directive, CycleDirective::Abort)
            && !matches!(self.actor_role, Role::Chief)
        {
            return Err(CoordinationError::Unauthorized {
                role: self.actor_role,
                action: Action::Decide,
            });
        }
        Ok(())
    }
}

/// Evidence required before a chief may approve the closure review.  The booleans are claims
/// only after validation; the caller must still provide hashed evidence references.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClosureEvidence {
    pub top_peer_checked: bool,
    pub independent_falsifiers: u8,
    pub historical_ceiling_checked: bool,
    pub dual_track_verified: bool,
    pub budget_stop_requested: bool,
    pub top_peer_reward: Option<f64>,
    pub current_reward: Option<f64>,
    pub historical_best_reward: Option<f64>,
    pub evidence: Vec<EvidenceRef>,
}

impl ClosureEvidence {
    pub fn validate(&self) -> Result<(), CoordinationError> {
        if !self.top_peer_checked
            || self.independent_falsifiers < 2
            || !self.historical_ceiling_checked
            || !self.dual_track_verified
        {
            return Err(CoordinationError::InvalidDecision(
                "closure requires peer, two independent falsifiers, historical ceiling, and dual-track checks".to_string(),
            ));
        }
        if self.evidence.is_empty() {
            return Err(CoordinationError::InvalidDecision(
                "closure requires hashed evidence references".to_string(),
            ));
        }
        validate_evidence_refs(&self.evidence)?;
        let required_rewards = [
            ("top_peer_reward", self.top_peer_reward),
            ("current_reward", self.current_reward),
            ("historical_best_reward", self.historical_best_reward),
        ];
        for (name, value) in required_rewards {
            let value = value.ok_or_else(|| {
                CoordinationError::InvalidDecision(format!(
                    "{name} is required for a ceiling decision"
                ))
            })?;
            if !value.is_finite() || !(0.0..=1.0).contains(&value) {
                return Err(CoordinationError::InvalidDecision(format!(
                    "{name} must be finite and within 0..=1"
                )));
            }
        }
        let current = self.current_reward.expect("validated above");
        if self.top_peer_reward.expect("validated above") > current
            || self.historical_best_reward.expect("validated above") > current
        {
            return Err(CoordinationError::InvalidDecision(
                "closure cannot override a higher peer or historical result".to_string(),
            ));
        }
        let kinds = self
            .evidence
            .iter()
            .map(|reference| reference.kind.as_str())
            .collect::<BTreeSet<_>>();
        for required in [
            "closure_peer",
            "closure_historical",
            "closure_harbor",
            "closure_trace",
        ] {
            if !kinds.contains(required) {
                return Err(CoordinationError::InvalidDecision(
                    "closure requires typed peer, historical, Harbor, and Trace evidence"
                        .to_string(),
                ));
            }
        }
        if self
            .evidence
            .iter()
            .filter(|reference| reference.kind == "closure_falsifier")
            .count()
            < 2
        {
            return Err(CoordinationError::InvalidDecision(
                "closure requires two distinct falsifier evidence records".to_string(),
            ));
        }
        if self.budget_stop_requested && !kinds.contains("quota_snapshot") {
            return Err(CoordinationError::InvalidDecision(
                "budget stop requires a typed quota snapshot".to_string(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExperimentOutcome {
    pub challenge_id: String,
    pub axis: String,
    pub attempt_id: Option<String>,
    pub harbor_reward: Option<f64>,
    pub judge_phrase: Option<String>,
    pub completed_at_ms: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StuckTrigger {
    SameAxisNoProgress,
    RepeatedJudgePhrase,
    GapWithoutProgress,
}

#[derive(Debug, Clone, Copy)]
pub struct StuckPolicy {
    pub same_axis_attempts: usize,
    pub repeated_phrase_count: usize,
    pub required_gap_bands: i32,
    pub no_progress_window_ms: i64,
}

impl Default for StuckPolicy {
    fn default() -> Self {
        Self {
            same_axis_attempts: 3,
            repeated_phrase_count: 2,
            required_gap_bands: 2,
            no_progress_window_ms: 2 * 60 * 60 * 1000,
        }
    }
}

/// Detect the evidence-based handoff conditions from OPERATIONS_PLAYBOOK §4. A trigger asks the
/// chief to dispatch judge-analysis and clean-room red-team work; it is not permission to submit.
pub fn detect_stuck(
    outcomes: &[ExperimentOutcome],
    own_band: i32,
    field_highest_band: i32,
    now_ms: i64,
    policy: StuckPolicy,
) -> Vec<StuckTrigger> {
    let mut triggers = Vec::new();
    // Outcomes from multiple challenges can share an axis or judge phrase. Scope all
    // heuristics to the most recently observed challenge so one campaign cannot trigger a
    // replan because of another campaign's history.
    let active_challenge = outcomes
        .iter()
        .max_by_key(|outcome| outcome.completed_at_ms)
        .map(|outcome| outcome.challenge_id.as_str());
    let scoped = outcomes.iter().filter(|outcome| {
        active_challenge.is_none_or(|challenge| outcome.challenge_id == challenge)
    });
    let mut scoped_outcomes = scoped.collect::<Vec<_>>();
    scoped_outcomes.sort_by_key(|outcome| outcome.completed_at_ms);
    if policy.same_axis_attempts > 0 {
        let mut axis_groups = std::collections::BTreeMap::<&str, Vec<&ExperimentOutcome>>::new();
        for outcome in &scoped_outcomes {
            axis_groups
                .entry(outcome.axis.as_str())
                .or_default()
                .push(outcome);
        }
        if axis_groups.values().any(|items| {
            let scored = items
                .iter()
                .filter_map(|item| {
                    item.harbor_reward
                        .map(|reward| (item.completed_at_ms, reward))
                })
                .rev()
                .take(policy.same_axis_attempts)
                .collect::<Vec<_>>();
            scored.len() == policy.same_axis_attempts
                && scored.windows(2).all(|pair| pair[0].1 == pair[1].1)
        }) {
            triggers.push(StuckTrigger::SameAxisNoProgress);
        }
    }
    if policy.repeated_phrase_count > 0 {
        let mut phrases = scoped_outcomes
            .iter()
            .rev()
            .filter_map(|outcome| outcome.judge_phrase.as_deref())
            .map(|phrase| phrase.trim().to_ascii_lowercase());
        if let Some(first) = phrases.next() {
            let count = std::iter::once(first.clone())
                .chain(phrases)
                .take_while(|phrase| phrase == &first)
                .count();
            if count >= policy.repeated_phrase_count {
                triggers.push(StuckTrigger::RepeatedJudgePhrase);
            }
        }
    }
    if field_highest_band - own_band >= policy.required_gap_bands {
        let mut best = None;
        let mut last_progress = None;
        for outcome in &scoped_outcomes {
            if let Some(reward) = outcome.harbor_reward {
                if best.is_none_or(|current| reward > current) {
                    best = Some(reward);
                    last_progress = Some(outcome.completed_at_ms);
                }
            }
        }
        let last_progress = last_progress.unwrap_or(now_ms);
        if now_ms.saturating_sub(last_progress) >= policy.no_progress_window_ms {
            triggers.push(StuckTrigger::GapWithoutProgress);
        }
    }
    triggers
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan(fields: Vec<&str>, coupled_group: Option<&str>) -> ExperimentPlan {
        ExperimentPlan {
            schema_version: SCHEMA_VERSION.to_string(),
            challenge_id: "c1".to_string(),
            axis: "bare_value".to_string(),
            changed_fields: fields.into_iter().map(str::to_string).collect(),
            coupled_group: coupled_group.map(str::to_string),
            hypothesis: "the reference uses the direct field value".to_string(),
            expected_response: "harbor changes if the field is scored".to_string(),
            decision_criterion: "retain only a reproducible improvement".to_string(),
            parent_attempt_id: None,
        }
    }

    fn outcome(axis: &str, reward: f64, phrase: Option<&str>, at: i64) -> ExperimentOutcome {
        ExperimentOutcome {
            challenge_id: "c1".to_string(),
            axis: axis.to_string(),
            attempt_id: Some(format!("a{at}")),
            harbor_reward: Some(reward),
            judge_phrase: phrase.map(str::to_string),
            completed_at_ms: at,
        }
    }

    fn actor(agent_id: &str, role: Role) -> ActorContext {
        ActorContext {
            agent_id: agent_id.to_string(),
            session_id: format!("session-{agent_id}"),
            thread_id: format!("thread-{agent_id}"),
            role,
            campaign_id: "campaign-1".to_string(),
            challenge_id: "challenge-1".to_string(),
            lease: Lease {
                lease_id: format!("lease-{agent_id}"),
                campaign_id: "campaign-1".to_string(),
                challenge_id: "challenge-1".to_string(),
                owner_agent_id: agent_id.to_string(),
                role,
                issued_at_ms: 0,
                expires_at_ms: 1_000,
                epoch: 1,
                allowed_actions: BTreeSet::from([Action::Decide, Action::TransitionAgent]),
                authorized_identity_classes: BTreeSet::from(["solver".to_string()]),
                operator_id: "1179613".to_string(),
                pool_epoch: 1,
                registration_allowed: false,
            },
        }
    }

    #[test]
    fn chief_and_observers_cannot_execute_or_submit() {
        assert!(authorize_action(Role::Chief, Action::Decide).is_ok());
        assert!(authorize_action(Role::Chief, Action::SubmitAttempt).is_err());
        assert!(authorize_action(Role::RedTeam, Action::WriteWorkspace).is_err());
        assert!(authorize_action(Role::Solver, Action::RequestSubmission).is_ok());
    }

    #[test]
    fn experiments_require_a_hypothesis_and_single_axis_or_explicit_coupling() {
        assert!(plan(vec!["coefficient"], None).validate().is_ok());
        assert!(plan(vec!["a", "b"], None).validate().is_err());
        assert!(
            plan(vec!["a", "b"], Some("paired_prefactor"))
                .validate()
                .is_ok()
        );
    }

    #[test]
    fn success_report_requires_the_five_evidence_elements() {
        let report = WorkerReport {
            schema_version: SCHEMA_VERSION.to_string(),
            role: Role::Solver,
            status: ReportStatus::Success,
            challenge_id: "c1".to_string(),
            identity: Some("id1".to_string()),
            attempt_id: Some("a1".to_string()),
            harbor_reward: Some(0.9),
            trace_score: Some(88.0),
            judge_summary: Some("accepted".to_string()),
            evidence: vec![
                EvidenceRef {
                    kind: "attempt".to_string(),
                    path: "artifacts/attempt.json".to_string(),
                    sha256: Some("a".repeat(64)),
                },
                EvidenceRef {
                    kind: "trace".to_string(),
                    path: "trace/trace.jsonl".to_string(),
                    sha256: Some("b".repeat(64)),
                },
                EvidenceRef {
                    kind: "artifact".to_string(),
                    path: "outputs/answer.json".to_string(),
                    sha256: Some("c".repeat(64)),
                },
                EvidenceRef {
                    kind: "score".to_string(),
                    path: "score/observation.json".to_string(),
                    sha256: Some("d".repeat(64)),
                },
            ],
        };
        assert!(report.validate().is_ok());
        let mut incomplete = report.clone();
        incomplete.trace_score = None;
        assert!(incomplete.validate().is_err());
    }

    #[test]
    fn attempt_and_receipt_transitions_are_ordered_and_fail_closed() {
        let state = AttemptState::Candidate
            .transition(AttemptEvent::PreflightStarted)
            .and_then(|state| state.transition(AttemptEvent::ReservationHeld))
            .and_then(|state| state.transition(AttemptEvent::TransportAccepted))
            .and_then(|state| state.transition(AttemptEvent::ParsePending))
            .and_then(|state| state.transition(AttemptEvent::ScoreReceived))
            .expect("valid attempt lifecycle");
        assert_eq!(state, AttemptState::ScoreObserved);
        assert!(state.transition(AttemptEvent::Commit).is_err());

        let receipt = ReceiptStatus::Accepted
            .transition(ReceiptEvent::Deliver)
            .and_then(|status| status.transition(ReceiptEvent::Ack))
            .and_then(|status| status.transition(ReceiptEvent::Apply))
            .expect("valid receipt lifecycle");
        assert_eq!(receipt, ReceiptStatus::Applied);
        assert!(receipt.transition(ReceiptEvent::Expire).is_err());
    }

    #[test]
    fn platform_observation_separates_api_score_from_leaderboard_confirmation() {
        let state = AttemptState::ScoreObserved;
        let observation = PlatformObservation {
            attempt_id: "attempt-1".to_string(),
            challenge_id: "challenge-1".to_string(),
            route: "/api/challenges/c1/attempts/attempt-1".to_string(),
            observed_at_ms: 100,
            response_sha256: "a".repeat(64),
            replay_status: EvidenceAvailability::Present,
            results_status: EvidenceAvailability::Redacted,
            scorecard_status: EvidenceAvailability::Present,
            leaderboard_status: EvidenceAvailability::Present,
            harbor_reward: Some(0.75),
            trace_score: Some(82.0),
        };
        assert_eq!(
            confirm_attempt_observation(state, &observation, "challenge-1", 200)
                .expect("complete platform observation"),
            AttemptState::LeaderboardConfirmed
        );
        assert_eq!(
            AttemptState::LeaderboardConfirmed
                .transition(AttemptEvent::OfficialConfirm)
                .expect("explicit official confirmation"),
            AttemptState::OfficialConfirmed
        );

        let mut pending = observation.clone();
        pending.leaderboard_status = EvidenceAvailability::Pending;
        assert!(confirm_attempt_observation(state, &pending, "challenge-1", 200).is_err());
        assert!(confirm_attempt_observation(state, &observation, "other", 200).is_err());
    }

    #[test]
    fn success_report_rejects_incomplete_or_unhashed_evidence() {
        let report = WorkerReport {
            schema_version: SCHEMA_VERSION.to_string(),
            role: Role::Solver,
            status: ReportStatus::Success,
            challenge_id: "c1".to_string(),
            identity: Some("id1".to_string()),
            attempt_id: Some("a1".to_string()),
            harbor_reward: Some(0.9),
            trace_score: Some(88.0),
            judge_summary: Some("accepted".to_string()),
            evidence: vec![EvidenceRef {
                kind: "attempt".to_string(),
                path: "artifacts/attempt.json".to_string(),
                sha256: Some("a".repeat(64)),
            }],
        };
        assert!(report.validate().is_err());
        let mut malformed = report;
        malformed.evidence = vec![
            EvidenceRef {
                kind: "attempt".to_string(),
                path: "../attempt.json".to_string(),
                sha256: Some("a".repeat(64)),
            },
            EvidenceRef {
                kind: "trace".to_string(),
                path: "trace.jsonl".to_string(),
                sha256: Some("b".repeat(64)),
            },
            EvidenceRef {
                kind: "artifact".to_string(),
                path: "answer.json".to_string(),
                sha256: Some("c".repeat(64)),
            },
            EvidenceRef {
                kind: "score".to_string(),
                path: "score.json".to_string(),
                sha256: None,
            },
        ];
        assert!(malformed.validate().is_err());
    }

    #[test]
    fn message_envelope_enforces_turn_authority_and_expiry() {
        let mut message = MessageEnvelope {
            schema_version: SCHEMA_VERSION.to_string(),
            message_id: "m1".to_string(),
            message_type: MessageType::Report,
            topic: Topic::Score,
            campaign_id: "campaign-1".to_string(),
            challenge_id: Some("challenge-1".to_string()),
            sender: Party {
                session_id: "s1".to_string(),
                thread_id: "t1".to_string(),
                role: Role::Monitor,
            },
            recipient: Party {
                session_id: "s1".to_string(),
                thread_id: "t0".to_string(),
                role: Role::Chief,
            },
            parent_message_id: None,
            correlation_id: "c1".to_string(),
            causation_id: None,
            stream_seq: 1,
            issued_at_ms: 100,
            expires_at_ms: 200,
            priority: 1,
            requires_ack: true,
            trigger_turn: false,
            lease_id: None,
            authority: "monitor".to_string(),
            expected_state_version: 1,
            idempotency_key: "m-key".to_string(),
            payload: "{}".to_string(),
            evidence: Vec::new(),
        };
        assert!(message.validate(150).is_ok());
        message.trigger_turn = true;
        assert!(message.validate(150).is_err());
        message.message_type = MessageType::Command;
        assert!(message.validate(150).is_err());
        message.authority = "chief".to_string();
        assert!(message.validate(150).is_ok());
        assert!(message.validate(201).is_err());
        message.challenge_id = Some(String::new());
        assert!(message.validate(150).is_err());
    }

    #[test]
    fn official_confirmation_allows_explicit_redacted_platform_fields() {
        let mut evidence = OfficialEvidence {
            challenge_id: "c1".to_string(),
            replay_executed: true,
            results_populated: false,
            scorecard_populated: false,
            harbor_reward: 0.91,
            leaderboard_seen: true,
            results_json_status: Some(EvidenceAvailability::Redacted),
            scorecard_status: Some(EvidenceAvailability::Redacted),
        };
        assert!(evidence.validate("c1").is_ok());
        evidence.results_json_status = Some(EvidenceAvailability::Pending);
        assert!(evidence.validate("c1").is_err());
        evidence.results_json_status = None;
        assert!(evidence.validate("c1").is_err());
    }

    #[test]
    fn campaign_does_not_equate_transport_with_submission_dispatch() {
        let state = CampaignState::Preflight
            .transition(CampaignEvent::TransportAccepted)
            .expect("transport accepted");
        assert_eq!(state, CampaignState::Submitting);
        assert!(state.transition(CampaignEvent::TransportAccepted).is_err());
        assert_eq!(
            state
                .transition(CampaignEvent::SubmissionDispatched)
                .expect("submission dispatched"),
            CampaignState::AwaitingScore
        );
    }

    #[test]
    fn campaign_closure_review_is_not_approval() {
        let review = CampaignState::ScoreVerified
            .transition(CampaignEvent::ClosureReviewStarted)
            .expect("enter closure review");
        assert_eq!(review, CampaignState::ClosureReview);
        assert!(
            review
                .transition(CampaignEvent::ClosureReviewStarted)
                .is_err()
        );
        assert_eq!(
            review
                .transition(CampaignEvent::ClosureApproved)
                .expect("approve closure"),
            CampaignState::Closed
        );
    }

    #[test]
    fn coordinator_enforces_chief_first_loop_and_idempotent_events() {
        let mut coordinator = Coordinator::new("campaign-1", "challenge-1");
        let chief = AgentRecord {
            agent_id: "chief-1".to_string(),
            role: Role::Chief,
            state: AgentState::Registered,
            parent_agent_id: None,
            workspace: "work/challenge-1/chief-1".to_string(),
            lease_id: None,
        };
        assert_eq!(
            coordinator
                .register_agent(Role::Chief, chief, 0, "chief-event", "chief-key")
                .expect("chief registers root"),
            1
        );
        let solver = AgentRecord {
            agent_id: "solver-1".to_string(),
            role: Role::Solver,
            state: AgentState::Registered,
            parent_agent_id: Some("chief-1".to_string()),
            workspace: "work/challenge-1/solver-1".to_string(),
            lease_id: Some("lease-1".to_string()),
        };
        assert!(
            coordinator
                .register_agent(Role::Solver, solver.clone(), 1, "e0", "k0")
                .is_err()
        );
        assert_eq!(
            coordinator
                .register_agent(Role::Chief, solver.clone(), 1, "e0", "k0")
                .expect("chief registers solver"),
            2
        );
        assert_eq!(
            coordinator
                .register_agent(Role::Chief, solver.clone(), 0, "e0", "k0",)
                .expect("duplicate event is idempotent"),
            2
        );
        assert!(
            coordinator
                .transition_campaign(Role::Solver, CampaignEvent::IntakeAccepted, 2, "e1", "k1")
                .is_err()
        );
        assert_eq!(
            coordinator
                .transition_campaign(Role::Chief, CampaignEvent::IntakeAccepted, 2, "e1", "k1")
                .expect("chief advances campaign"),
            3
        );
        assert_eq!(
            coordinator
                .transition_campaign(Role::Chief, CampaignEvent::IntakeAccepted, 0, "e1", "k1")
                .expect("same event replay"),
            3
        );
        assert!(
            coordinator
                .transition_agent(
                    Role::Monitor,
                    "solver-1",
                    AgentEvent::WaitInput,
                    3,
                    "e2",
                    "k2"
                )
                .is_err()
        );
    }

    #[test]
    fn coordinator_rejects_idempotency_key_conflicts_and_invalid_lineage() {
        let mut coordinator = Coordinator::new("campaign-1", "challenge-1");
        let chief = AgentRecord {
            agent_id: "chief-1".to_string(),
            role: Role::Chief,
            state: AgentState::Registered,
            parent_agent_id: None,
            workspace: "work/challenge-1/chief-1".to_string(),
            lease_id: None,
        };
        coordinator
            .register_agent(Role::Chief, chief, 0, "chief-event", "chief-key")
            .expect("register chief");
        let invalid = AgentRecord {
            agent_id: "solver-1".to_string(),
            role: Role::Solver,
            state: AgentState::Registered,
            parent_agent_id: Some("missing".to_string()),
            workspace: "work/challenge-1/solver-1".to_string(),
            lease_id: Some("lease-1".to_string()),
        };
        assert!(
            coordinator
                .register_agent(Role::Chief, invalid, 1, "solver-event", "solver-key")
                .is_err()
        );
        let solver = AgentRecord {
            agent_id: "solver-1".to_string(),
            role: Role::Solver,
            state: AgentState::Registered,
            parent_agent_id: Some("chief-1".to_string()),
            workspace: "work/challenge-1/solver-1".to_string(),
            lease_id: Some("lease-1".to_string()),
        };
        coordinator
            .register_agent(Role::Chief, solver.clone(), 1, "solver-event", "solver-key")
            .expect("register solver");
        let conflicting = AgentRecord {
            agent_id: "solver-2".to_string(),
            workspace: "work/challenge-1/solver-2".to_string(),
            lease_id: Some("lease-2".to_string()),
            ..solver.clone()
        };
        assert!(
            coordinator
                .register_agent(Role::Chief, conflicting, 0, "different", "solver-key")
                .is_err()
        );
        let duplicate_lease = AgentRecord {
            agent_id: "solver-3".to_string(),
            workspace: "work/challenge-1/solver-3".to_string(),
            ..solver
        };
        assert!(
            coordinator
                .register_agent(
                    Role::Chief,
                    duplicate_lease,
                    2,
                    "solver-3-event",
                    "solver-3-key"
                )
                .is_err()
        );
    }

    #[test]
    fn coordinator_snapshot_round_trip_rejects_inconsistent_versions() {
        let mut coordinator = Coordinator::new("campaign-1", "challenge-1");
        coordinator
            .transition_campaign(Role::Chief, CampaignEvent::IntakeAccepted, 0, "e1", "k1")
            .expect("advance");
        let snapshot = coordinator.snapshot().clone();
        assert!(Coordinator::from_snapshot(snapshot.clone()).is_ok());
        let mut invalid = snapshot;
        invalid.state_version = 3;
        assert!(Coordinator::from_snapshot(invalid).is_err());
    }

    #[test]
    fn context_bound_transitions_reject_expired_and_sibling_leases() {
        let mut coordinator = Coordinator::new("campaign-1", "challenge-1");
        let chief = AgentRecord {
            agent_id: "chief-1".to_string(),
            role: Role::Chief,
            state: AgentState::Registered,
            parent_agent_id: None,
            workspace: "work/challenge-1/chief-1".to_string(),
            lease_id: None,
        };
        coordinator
            .register_agent(Role::Chief, chief, 0, "chief-event", "chief-key")
            .expect("register chief");
        let solver = AgentRecord {
            agent_id: "solver-1".to_string(),
            role: Role::Solver,
            state: AgentState::Registered,
            parent_agent_id: Some("chief-1".to_string()),
            workspace: "work/challenge-1/solver-1".to_string(),
            lease_id: Some("lease-solver-1".to_string()),
        };
        coordinator
            .register_agent(Role::Chief, solver, 1, "solver-event", "solver-key")
            .expect("register solver");
        let chief_context = actor("chief-1", Role::Chief);
        coordinator
            .transition_agent_with_context(
                &chief_context,
                "solver-1",
                AgentEvent::Brief,
                2,
                "brief-event",
                "brief-key",
                10,
            )
            .expect("chief briefs child");
        coordinator
            .transition_agent_with_context(
                &chief_context,
                "solver-1",
                AgentEvent::Activate,
                3,
                "activate-event",
                "activate-key",
                10,
            )
            .expect("chief activates child");

        let solver_context = actor("solver-1", Role::Solver);
        assert!(
            coordinator
                .transition_agent_with_context(
                    &solver_context,
                    "solver-2",
                    AgentEvent::WaitInput,
                    4,
                    "bad-event",
                    "bad-key",
                    10,
                )
                .is_err()
        );
        assert!(
            coordinator
                .transition_agent_with_context(
                    &solver_context,
                    "solver-1",
                    AgentEvent::WaitInput,
                    4,
                    "wait-event",
                    "wait-key",
                    1_000,
                )
                .is_err()
        );
        assert!(
            coordinator
                .transition_agent_with_context(
                    &solver_context,
                    "solver-1",
                    AgentEvent::WaitInput,
                    4,
                    "wait-event",
                    "wait-key",
                    500,
                )
                .is_ok()
        );
    }

    #[test]
    fn stuck_detection_requires_evidence_not_a_single_bad_result() {
        let outcomes = vec![
            outcome("bare_value", 0.2, Some("missing derivation"), 1_000),
            outcome("bare_value", 0.2, Some("missing derivation"), 2_000),
            outcome("bare_value", 0.2, Some("missing derivation"), 3_000),
        ];
        let triggers = detect_stuck(&outcomes, 1, 3, 3_000, StuckPolicy::default());
        assert!(triggers.contains(&StuckTrigger::SameAxisNoProgress));
        assert!(triggers.contains(&StuckTrigger::RepeatedJudgePhrase));
        assert!(!triggers.contains(&StuckTrigger::GapWithoutProgress));
    }

    #[test]
    fn stuck_detection_does_not_mix_challenge_histories() {
        let mut outcomes = vec![outcome("bare_value", 0.4, Some("same"), 100)];
        outcomes.push(ExperimentOutcome {
            challenge_id: "other-challenge".to_string(),
            axis: "bare_value".to_string(),
            attempt_id: Some("other-1".to_string()),
            harbor_reward: Some(0.4),
            judge_phrase: Some("same".to_string()),
            completed_at_ms: 101,
        });
        outcomes.push(ExperimentOutcome {
            challenge_id: "other-challenge".to_string(),
            axis: "bare_value".to_string(),
            attempt_id: Some("other-2".to_string()),
            harbor_reward: Some(0.4),
            judge_phrase: Some("same".to_string()),
            completed_at_ms: 102,
        });
        outcomes.push(outcome("bare_value", 0.5, Some("new"), 103));
        let triggers = detect_stuck(&outcomes, 1, 3, 100, StuckPolicy::default());
        assert!(!triggers.contains(&StuckTrigger::SameAxisNoProgress));
        assert!(!triggers.contains(&StuckTrigger::RepeatedJudgePhrase));
    }

    #[test]
    fn gap_trigger_waits_for_the_configured_window() {
        let outcomes = vec![outcome("axis", 0.2, None, 1_000)];
        let policy = StuckPolicy {
            no_progress_window_ms: 100,
            ..StuckPolicy::default()
        };
        let triggers = detect_stuck(&outcomes, 1, 3, 1_101, policy);
        assert!(triggers.contains(&StuckTrigger::GapWithoutProgress));
    }

    #[test]
    fn unscored_attempts_do_not_fake_same_axis_stuck_or_progress() {
        let outcomes = (1..=3)
            .map(|index| ExperimentOutcome {
                challenge_id: "challenge-1".to_string(),
                axis: "axis".to_string(),
                attempt_id: None,
                harbor_reward: None,
                judge_phrase: None,
                completed_at_ms: index,
            })
            .collect::<Vec<_>>();
        let triggers = detect_stuck(&outcomes, 1, 1, 200, StuckPolicy::default());
        assert!(!triggers.contains(&StuckTrigger::SameAxisNoProgress));
    }

    #[test]
    fn repeated_low_scores_do_not_reset_the_no_progress_window() {
        let outcomes = vec![
            outcome("axis", 0.5, None, 1_000),
            outcome("axis", 0.4, None, 1_050),
            outcome("axis", 0.4, None, 1_090),
        ];
        let policy = StuckPolicy {
            no_progress_window_ms: 100,
            ..StuckPolicy::default()
        };
        let triggers = detect_stuck(&outcomes, 1, 3, 1_101, policy);
        assert!(triggers.contains(&StuckTrigger::GapWithoutProgress));
    }

    #[test]
    fn schema_is_serializable_for_audit_records() {
        let json = serde_json::to_string(&plan(vec!["coefficient"], None)).expect("serialize");
        assert!(json.contains("ascodex-coordination/v1"));
    }

    #[test]
    fn ooda_cycle_rejects_stuck_continue_and_wrong_phase_owner() {
        let evidence = vec![EvidenceRef {
            kind: "score".to_string(),
            path: "observations/score.json".to_string(),
            sha256: Some("a".repeat(64)),
        }];
        let mut cycle = OodaCycleRecord {
            schema_version: SCHEMA_VERSION.to_string(),
            cycle_id: "cycle-1".to_string(),
            campaign_id: "campaign-1".to_string(),
            challenge_id: "challenge-1".to_string(),
            phase: OodaPhase::Decide,
            actor_role: Role::Chief,
            directive: CycleDirective::Continue,
            rationale: "baseline is still being tested".to_string(),
            expected_state_version: 3,
            deadline_ms: 2_000,
            stuck_triggers: vec![StuckTrigger::SameAxisNoProgress],
            evidence,
        };
        assert!(cycle.validate(1_000).is_err());
        cycle.directive = CycleDirective::Replan;
        assert!(cycle.validate(1_000).is_ok());
        cycle.actor_role = Role::Solver;
        assert!(cycle.validate(1_000).is_err());
    }

    #[test]
    fn closure_evidence_requires_two_falsifiers_and_blocks_lower_historical_stop() {
        let base = ClosureEvidence {
            top_peer_checked: true,
            independent_falsifiers: 2,
            historical_ceiling_checked: true,
            dual_track_verified: true,
            budget_stop_requested: true,
            top_peer_reward: Some(0.8),
            current_reward: Some(0.7),
            historical_best_reward: Some(0.8),
            evidence: vec![
                EvidenceRef {
                    kind: "closure_peer".to_string(),
                    path: "observations/peer.json".to_string(),
                    sha256: Some("b".repeat(64)),
                },
                EvidenceRef {
                    kind: "closure_historical".to_string(),
                    path: "observations/historical.json".to_string(),
                    sha256: Some("c".repeat(64)),
                },
                EvidenceRef {
                    kind: "closure_harbor".to_string(),
                    path: "observations/harbor.json".to_string(),
                    sha256: Some("d".repeat(64)),
                },
                EvidenceRef {
                    kind: "closure_trace".to_string(),
                    path: "observations/trace.json".to_string(),
                    sha256: Some("e".repeat(64)),
                },
                EvidenceRef {
                    kind: "quota_snapshot".to_string(),
                    path: "observations/quota.json".to_string(),
                    sha256: Some("f".repeat(64)),
                },
                EvidenceRef {
                    kind: "closure_falsifier".to_string(),
                    path: "observations/falsifier-a.json".to_string(),
                    sha256: Some("1".repeat(64)),
                },
                EvidenceRef {
                    kind: "closure_falsifier".to_string(),
                    path: "observations/falsifier-b.json".to_string(),
                    sha256: Some("2".repeat(64)),
                },
            ],
        };
        assert!(base.validate().is_err());
        let mut accepted = base;
        accepted.current_reward = Some(0.8);
        assert!(accepted.validate().is_ok());
        accepted.independent_falsifiers = 1;
        assert!(accepted.validate().is_err());
    }
}
