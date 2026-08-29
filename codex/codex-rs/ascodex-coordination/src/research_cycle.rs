use crate::{
    ChallengeContract, ClosureEvidence, CoordinationError, EvidenceRef, ExperimentPlan,
    OodaCycleRecord, OodaPhase, PlatformObservation, Role, SCHEMA_VERSION, StageBrief,
    WorkerReport, validate_evidence_refs,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CycleOutcome {
    Progress,
    Completed,
    Blocked,
    EnvFailure,
    Timeout,
    Inconclusive,
    FailedRetryable,
    FailedTerminal,
    Stuck,
    ClosureCandidate,
    ClosureReview,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChiefDirective {
    Continue,
    Replan,
    EscalateStuckReview,
    BeginClosureReview,
    ApproveClosure,
    Abort,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResearchCycleRecord {
    pub schema_version: String,
    pub cycle_id: String,
    pub campaign_id: String,
    pub challenge_id: String,
    pub expected_state_version: u64,
    pub deadline_ms: i64,
    pub verifier_spec_sha256: String,
    pub baseline_sha256: String,
    pub stage_briefs: Vec<StageBrief>,
    pub experiment_plan: Option<ExperimentPlan>,
    pub worker_report: Option<WorkerReport>,
    pub observation: Option<PlatformObservation>,
    pub contract: Option<ChallengeContract>,
    pub facts: Vec<String>,
    pub inferences: Vec<String>,
    pub outcome: CycleOutcome,
    pub directive: ChiefDirective,
    pub closure_evidence: Option<ClosureEvidence>,
    pub quota_cost: f64,
    pub evidence: Vec<EvidenceRef>,
    pub ooda: OodaCycleRecord,
}

impl ResearchCycleRecord {
    pub fn validate(&self, now_ms: i64) -> Result<(), CoordinationError> {
        if self.schema_version != SCHEMA_VERSION
            || self.cycle_id.trim().is_empty()
            || self.campaign_id.trim().is_empty()
            || self.challenge_id.trim().is_empty()
            || self.deadline_ms <= now_ms
            || self.facts.is_empty()
            || self.inferences.is_empty()
            || self.facts.iter().any(|v| v.trim().is_empty())
            || self.inferences.iter().any(|v| v.trim().is_empty())
        {
            return Err(invalid(
                "cycle identifiers, future deadline, facts, and inferences are required",
            ));
        }
        validate_digest(&self.verifier_spec_sha256, "verifier/spec")?;
        validate_digest(&self.baseline_sha256, "baseline")?;
        if !self.quota_cost.is_finite() || self.quota_cost.is_sign_negative() {
            return Err(invalid("quota cost must be finite and non-negative"));
        }
        if self.stage_briefs.is_empty() {
            return Err(invalid("cycle must contain at least one stage brief"));
        }
        let mut brief_ids = std::collections::BTreeSet::new();
        let mut brief_roles = std::collections::BTreeSet::new();
        for brief in &self.stage_briefs {
            if brief.campaign_id != self.campaign_id
                || brief.challenge_id != self.challenge_id
                || !brief_ids.insert(brief.brief_id.as_str())
                || !brief_roles.insert(brief.target_role)
            {
                return Err(invalid(
                    "stage briefs must have unique ids and roles bound to this campaign and challenge",
                ));
            }
            brief.validate(now_ms)?;
        }
        if self.ooda.campaign_id != self.campaign_id
            || self.ooda.challenge_id != self.challenge_id
            || self.ooda.expected_state_version != self.expected_state_version
            || self.ooda.actor_role != Role::Chief
            || !matches!(self.ooda.phase, OodaPhase::Decide | OodaPhase::Review)
        {
            return Err(invalid(
                "OODA record is not a chief decision for this cycle",
            ));
        }
        self.ooda.validate(now_ms)?;
        let expected_ooda = match self.directive {
            ChiefDirective::Continue => crate::CycleDirective::Continue,
            ChiefDirective::Replan => crate::CycleDirective::Replan,
            ChiefDirective::EscalateStuckReview => crate::CycleDirective::EscalateStuckReview,
            ChiefDirective::BeginClosureReview => crate::CycleDirective::ClosureReview,
            ChiefDirective::ApproveClosure => crate::CycleDirective::ApproveClosure,
            ChiefDirective::Abort => crate::CycleDirective::Abort,
        };
        if self.ooda.directive != expected_ooda {
            return Err(invalid("chief directive and OODA directive disagree"));
        }
        validate_evidence_refs(&self.evidence)?;

        if let Some(plan) = &self.experiment_plan {
            plan.validate()?;
            if plan.challenge_id != self.challenge_id {
                return Err(invalid("experiment plan challenge does not match cycle"));
            }
        }
        if let Some(report) = &self.worker_report {
            report.validate()?;
            if report.challenge_id != self.challenge_id {
                return Err(invalid("worker report challenge does not match cycle"));
            }
        }
        if let Some(observation) = &self.observation {
            observation.validate(&self.challenge_id, now_ms)?;
        }
        if let Some(contract) = &self.contract {
            contract.validate(&self.challenge_id, now_ms)?;
        }
        let has_kind = |kind: &str| self.evidence.iter().any(|e| e.kind == kind);
        for required in ["verifier", "baseline", "stage_brief"] {
            if !has_kind(required) {
                return Err(invalid(
                    "cycle is missing a typed verifier, baseline, or brief reference",
                ));
            }
        }
        if self
            .evidence
            .iter()
            .filter(|reference| reference.kind == "stage_brief")
            .count()
            != self.stage_briefs.len()
        {
            return Err(invalid(
                "each routed stage brief requires one typed evidence reference",
            ));
        }

        match self.outcome {
            CycleOutcome::Progress => {
                if self.experiment_plan.is_none() || self.worker_report.is_none() {
                    return Err(invalid(
                        "progress requires an experiment plan and worker report",
                    ));
                }
                if self.directive != ChiefDirective::Continue
                    && self.directive != ChiefDirective::Replan
                    && self.directive != ChiefDirective::BeginClosureReview
                {
                    return Err(invalid(
                        "progress permits only continue, replan, or closure review",
                    ));
                }
            }
            CycleOutcome::Completed => {
                let report = self
                    .worker_report
                    .as_ref()
                    .ok_or_else(|| invalid("completed work requires a successful worker report"))?;
                let observation = self.observation.as_ref().ok_or_else(|| {
                    invalid("completed work requires a complete platform observation")
                })?;
                let contract = self
                    .contract
                    .as_ref()
                    .ok_or_else(|| invalid("completed work requires a bound challenge contract"))?;
                contract.formal_admission(&self.challenge_id, now_ms)?;
                if self.experiment_plan.is_none()
                    || report.status != crate::ReportStatus::Success
                    || report.attempt_id.as_deref() != Some(observation.attempt_id.as_str())
                {
                    return Err(invalid(
                        "completed work must bind its experiment, successful report, and observed attempt",
                    ));
                }
                if !has_kind("platform_observation") {
                    return Err(invalid(
                        "completed work requires a hashed platform observation reference",
                    ));
                }
                if !has_kind("contract") {
                    return Err(invalid(
                        "completed work requires a hashed contract reference",
                    ));
                }
                if !matches!(
                    self.directive,
                    ChiefDirective::Continue
                        | ChiefDirective::Replan
                        | ChiefDirective::BeginClosureReview
                ) {
                    return Err(invalid(
                        "completed work permits only continue, replan, or closure review",
                    ));
                }
            }
            CycleOutcome::Blocked
            | CycleOutcome::EnvFailure
            | CycleOutcome::Timeout
            | CycleOutcome::Inconclusive
            | CycleOutcome::FailedRetryable => {
                if matches!(
                    self.directive,
                    ChiefDirective::Continue
                        | ChiefDirective::BeginClosureReview
                        | ChiefDirective::ApproveClosure
                ) {
                    return Err(invalid("non-success outcomes cannot continue or close"));
                }
            }
            CycleOutcome::FailedTerminal => {
                if !matches!(
                    self.directive,
                    ChiefDirective::Replan | ChiefDirective::Abort
                ) {
                    return Err(invalid("terminal failure permits only replan or abort"));
                }
            }
            CycleOutcome::Stuck => {
                if !matches!(
                    self.directive,
                    ChiefDirective::EscalateStuckReview | ChiefDirective::Abort
                ) {
                    return Err(invalid(
                        "stuck requires an atomic judge plus clean-room red-team review",
                    ));
                }
                if self.directive == ChiefDirective::EscalateStuckReview {
                    let routes = self
                        .stage_briefs
                        .iter()
                        .map(|brief| (brief.stage, brief.target_role, brief.clean_room))
                        .collect::<std::collections::BTreeSet<_>>();
                    let expected = std::collections::BTreeSet::from([
                        (crate::ResearchStage::StuckJudge, Role::JudgeAnalyst, false),
                        (crate::ResearchStage::StuckRedTeam, Role::RedTeam, true),
                    ]);
                    if routes != expected || self.stage_briefs.len() != 2 {
                        return Err(invalid(
                            "stuck escalation requires exactly one judge brief and one clean-room red-team brief",
                        ));
                    }
                }
            }
            CycleOutcome::ClosureCandidate => {
                if self.directive != ChiefDirective::BeginClosureReview
                    || self.ooda.phase != OodaPhase::Review
                {
                    return Err(invalid(
                        "closure candidate must begin a separate review phase",
                    ));
                }
                let closure = self
                    .closure_evidence
                    .as_ref()
                    .ok_or_else(|| invalid("closure requires structured evidence"))?;
                self.contract
                    .as_ref()
                    .ok_or_else(|| invalid("closure requires a bound challenge contract"))?
                    .formal_admission(&self.challenge_id, now_ms)?;
                if !has_kind("contract") {
                    return Err(invalid("closure requires a hashed contract reference"));
                }
                closure.validate()?;
                if !has_kind("closure_falsifier")
                    || self
                        .evidence
                        .iter()
                        .filter(|e| e.kind == "closure_falsifier")
                        .count()
                        < 2
                {
                    return Err(invalid("closure requires two typed falsifier references"));
                }
            }
            CycleOutcome::ClosureReview => {
                if self.directive != ChiefDirective::ApproveClosure
                    || self.ooda.phase != OodaPhase::Review
                {
                    return Err(invalid(
                        "closure review can only approve or replan/abort in a new cycle",
                    ));
                }
                let closure = self
                    .closure_evidence
                    .as_ref()
                    .ok_or_else(|| invalid("approval requires structured closure evidence"))?;
                self.contract
                    .as_ref()
                    .ok_or_else(|| invalid("approval requires a bound challenge contract"))?
                    .formal_admission(&self.challenge_id, now_ms)?;
                if !has_kind("contract") {
                    return Err(invalid("approval requires a hashed contract reference"));
                }
                closure.validate()?;
                if self
                    .evidence
                    .iter()
                    .filter(|e| e.kind == "closure_falsifier")
                    .count()
                    < 2
                {
                    return Err(invalid("approval requires two typed falsifier references"));
                }
            }
        }
        if self.directive == ChiefDirective::EscalateStuckReview
            && self.outcome != CycleOutcome::Stuck
        {
            return Err(invalid(
                "stuck escalation is only valid for a stuck outcome",
            ));
        }
        if self.directive == ChiefDirective::ApproveClosure
            && self.outcome != CycleOutcome::ClosureReview
        {
            return Err(invalid(
                "approval cannot occur in the same record as a closure candidate",
            ));
        }
        Ok(())
    }

    /// Validate that `current` is a legal successor of an already persisted cycle.
    ///
    /// A well-formed record on its own is not enough to advance a campaign: callers must
    /// provide the immediately preceding record, preserve the campaign/challenge binding and
    /// advance the optimistic version exactly once.  This is the reducer boundary used by a
    /// future Core/app-server coordination service; prompts and role labels cannot skip it.
    pub fn validate_successor(
        previous: &ResearchCycleRecord,
        current: &ResearchCycleRecord,
        now_ms: i64,
    ) -> Result<(), CoordinationError> {
        // The previous record may be outside its original validity window by the time a
        // successor is reconciled. Its integrity was checked when it was issued; revalidate the
        // new record against the current clock and only inspect the previous terminal metadata
        // here.
        current.validate(now_ms)?;
        if previous.campaign_id != current.campaign_id
            || previous.challenge_id != current.challenge_id
        {
            return Err(invalid("successor changes campaign or challenge"));
        }
        if previous.cycle_id == current.cycle_id {
            return Err(invalid("successor must have a distinct cycle id"));
        }
        if current.expected_state_version != previous.expected_state_version + 1 {
            return Err(invalid("successor must advance state version exactly once"));
        }
        if previous.outcome == CycleOutcome::ClosureReview
            && previous.directive == ChiefDirective::ApproveClosure
        {
            return Err(invalid("a closed cycle cannot have a successor"));
        }
        if previous.directive == ChiefDirective::Abort {
            return Err(invalid("an aborted cycle cannot have a successor"));
        }
        match (previous.outcome, previous.directive, current.directive) {
            (
                CycleOutcome::Stuck,
                ChiefDirective::EscalateStuckReview,
                ChiefDirective::Continue,
            )
            | (CycleOutcome::Stuck, ChiefDirective::EscalateStuckReview, ChiefDirective::Replan)
            | (CycleOutcome::Stuck, ChiefDirective::EscalateStuckReview, ChiefDirective::Abort)
            | (
                CycleOutcome::ClosureCandidate,
                ChiefDirective::BeginClosureReview,
                ChiefDirective::ApproveClosure,
            )
            | (_, ChiefDirective::BeginClosureReview, ChiefDirective::ApproveClosure) => {}
            (_, ChiefDirective::ApproveClosure, _) => {
                return Err(invalid(
                    "closure approval must immediately follow closure review",
                ));
            }
            _ => {}
        }
        Ok(())
    }
}

fn validate_digest(value: &str, label: &str) -> Result<(), CoordinationError> {
    if value.len() != 64 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(invalid(&format!(
            "{label} must be a 64-character hexadecimal digest"
        )));
    }
    Ok(())
}

fn invalid(message: &str) -> CoordinationError {
    CoordinationError::InvalidDecision(format!("research cycle is invalid: {message}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CycleDirective, ReportStatus, SkillRef};

    fn brief() -> StageBrief {
        StageBrief {
            schema_version: SCHEMA_VERSION.into(),
            brief_id: "brief-1".into(),
            campaign_id: "campaign-1".into(),
            challenge_id: "challenge-1".into(),
            challenge_workspace_root: std::env::temp_dir().join("ascodex-challenge-1"),
            stage: crate::ResearchStage::PreSubmit,
            target_role: Role::Solver,
            generated_at_ms: 100,
            expires_at_ms: 500,
            max_bytes: 1229,
            estimated_bytes: 900,
            skills: [
                "real-trace-capture",
                "trace-contamination-redline",
                "trace-maximize",
                "submit-attempt",
            ]
            .into_iter()
            .map(|name| SkillRef {
                name: name.into(),
                source_path: format!(".agents/skills/{name}/SKILL.md"),
                sha256: "a".repeat(64),
                selection_reason: "route".into(),
            })
            .collect(),
            selection_reason: "route".into(),
            capability_map_sha256: "b".repeat(64),
            clean_room: false,
        }
    }

    fn evidence(kind: &str, n: char) -> EvidenceRef {
        EvidenceRef {
            kind: kind.into(),
            path: format!("evidence/{kind}-{n}.json"),
            sha256: n.to_string().repeat(64).into(),
        }
    }

    fn base() -> ResearchCycleRecord {
        let plan = ExperimentPlan {
            schema_version: SCHEMA_VERSION.into(),
            challenge_id: "challenge-1".into(),
            axis: "axis".into(),
            changed_fields: vec!["field".into()],
            coupled_group: None,
            hypothesis: "hypothesis".into(),
            expected_response: "response".into(),
            decision_criterion: "criterion".into(),
            parent_attempt_id: None,
        };
        let report = WorkerReport {
            schema_version: SCHEMA_VERSION.into(),
            role: Role::Solver,
            status: ReportStatus::Blocked,
            challenge_id: "challenge-1".into(),
            identity: None,
            attempt_id: None,
            harbor_reward: None,
            trace_score: None,
            judge_summary: None,
            evidence: vec![],
        };
        let ooda = OodaCycleRecord {
            schema_version: SCHEMA_VERSION.into(),
            cycle_id: "cycle-1".into(),
            campaign_id: "campaign-1".into(),
            challenge_id: "challenge-1".into(),
            phase: OodaPhase::Decide,
            actor_role: Role::Chief,
            directive: CycleDirective::Replan,
            rationale: "needs another plan".into(),
            expected_state_version: 1,
            deadline_ms: 400,
            stuck_triggers: vec![],
            evidence: vec![evidence("score", 'd')],
        };
        ResearchCycleRecord {
            schema_version: SCHEMA_VERSION.into(),
            cycle_id: "cycle-1".into(),
            campaign_id: "campaign-1".into(),
            challenge_id: "challenge-1".into(),
            expected_state_version: 1,
            deadline_ms: 400,
            verifier_spec_sha256: "c".repeat(64),
            baseline_sha256: "d".repeat(64),
            stage_briefs: vec![brief()],
            experiment_plan: Some(plan),
            worker_report: Some(report),
            observation: None,
            contract: None,
            facts: vec!["fact".into()],
            inferences: vec!["inference".into()],
            outcome: CycleOutcome::Blocked,
            directive: ChiefDirective::Replan,
            closure_evidence: None,
            quota_cost: 0.0,
            evidence: vec![
                evidence("verifier", 'a'),
                evidence("baseline", 'b'),
                evidence("stage_brief", 'c'),
            ],
            ooda,
        }
    }

    #[test]
    fn blocked_cycle_can_only_replan_or_abort() {
        assert!(base().validate(150).is_ok());
        let mut bad = base();
        bad.directive = ChiefDirective::Continue;
        assert!(bad.validate(150).is_err());
    }

    #[test]
    fn mismatched_nested_challenge_and_non_chief_ooda_are_rejected() {
        let mut bad = base();
        bad.experiment_plan.as_mut().unwrap().challenge_id = "other".into();
        assert!(bad.validate(150).is_err());
        let mut bad = base();
        bad.ooda.actor_role = Role::Monitor;
        assert!(bad.validate(150).is_err());
    }

    #[test]
    fn completed_cycle_requires_official_platform_evidence() {
        let mut cycle = base();
        cycle.outcome = CycleOutcome::Completed;
        cycle.directive = ChiefDirective::Continue;
        cycle.ooda.directive = CycleDirective::Continue;
        assert!(cycle.validate(150).is_err());
    }

    #[test]
    fn closure_requires_two_typed_falsifiers_and_separate_approval_record() {
        let mut candidate = base();
        candidate.outcome = CycleOutcome::ClosureCandidate;
        candidate.directive = ChiefDirective::BeginClosureReview;
        candidate.ooda.directive = CycleDirective::ClosureReview;
        candidate.ooda.phase = OodaPhase::Review;
        candidate.contract = Some(ChallengeContract {
            schema_version: SCHEMA_VERSION.into(),
            challenge_id: "challenge-1".into(),
            contract_version: "v1".into(),
            fingerprint: "0123456789abcdef".into(),
            required_submission: "json".into(),
            status: crate::ContractStatus::Known,
            adapter_id: Some("adapter-v1".into()),
            round_start_ms: Some(100),
            round_end_ms: Some(500),
        });
        candidate.closure_evidence = Some(ClosureEvidence {
            top_peer_checked: true,
            independent_falsifiers: 2,
            historical_ceiling_checked: true,
            dual_track_verified: true,
            budget_stop_requested: false,
            top_peer_reward: Some(0.8),
            current_reward: Some(0.8),
            historical_best_reward: Some(0.8),
            evidence: vec![
                evidence("closure_peer", '3'),
                evidence("closure_historical", '4'),
                evidence("closure_harbor", '5'),
                evidence("closure_trace", '6'),
                evidence("closure_falsifier", '1'),
                evidence("closure_falsifier", '2'),
            ],
        });
        candidate.evidence.extend([
            evidence("contract", '0'),
            evidence("closure_falsifier", '1'),
            evidence("closure_falsifier", '2'),
        ]);
        assert!(candidate.validate(150).is_ok());
        let mut approved = candidate.clone();
        approved.outcome = CycleOutcome::ClosureReview;
        approved.directive = ChiefDirective::ApproveClosure;
        approved.ooda.directive = CycleDirective::ApproveClosure;
        assert!(approved.validate(150).is_ok());
    }

    #[test]
    fn stuck_escalation_requires_atomic_judge_and_clean_room_red_team_briefs() {
        let mut cycle = base();
        cycle.outcome = CycleOutcome::Stuck;
        cycle.directive = ChiefDirective::EscalateStuckReview;
        cycle.ooda.directive = CycleDirective::EscalateStuckReview;
        cycle.stage_briefs = vec![StageBrief {
            stage: crate::ResearchStage::StuckJudge,
            target_role: Role::JudgeAnalyst,
            skills: [
                "platform-scorecard-analyze",
                "oracle-probe",
                "differential-scoring",
                "judge-field-audit",
            ]
            .into_iter()
            .map(|name| SkillRef {
                name: name.into(),
                source_path: format!(".agents/skills/{name}/SKILL.md"),
                sha256: "a".repeat(64),
                selection_reason: "stuck judge route".into(),
            })
            .collect(),
            clean_room: false,
            ..brief()
        }];
        assert!(cycle.validate(150).is_err());

        cycle.stage_briefs.push(StageBrief {
            brief_id: "brief-red-team".into(),
            stage: crate::ResearchStage::StuckRedTeam,
            target_role: Role::RedTeam,
            skills: ["unstuck-switch-angle", "red-team-review"]
                .into_iter()
                .map(|name| SkillRef {
                    name: name.into(),
                    source_path: format!(".agents/skills/{name}/SKILL.md"),
                    sha256: "a".repeat(64),
                    selection_reason: "clean-room red-team route".into(),
                })
                .collect(),
            clean_room: true,
            ..brief()
        });
        cycle.evidence.push(evidence("stage_brief", 'e'));
        let validation = cycle.validate(150);
        assert!(validation.is_ok(), "{validation:?}");

        cycle.stage_briefs[1].clean_room = false;
        assert!(cycle.validate(150).is_err());
    }

    #[test]
    fn successor_requires_monotonic_version_and_blocks_terminal_cycles() {
        let previous = base();
        let mut current = previous.clone();
        current.cycle_id = "cycle-2".into();
        current.expected_state_version = previous.expected_state_version + 1;
        current.ooda.cycle_id = current.cycle_id.clone();
        current.ooda.expected_state_version = current.expected_state_version;
        current.deadline_ms = 600;
        assert!(ResearchCycleRecord::validate_successor(&previous, &current, 150).is_ok());

        current.expected_state_version += 1;
        assert!(ResearchCycleRecord::validate_successor(&previous, &current, 150).is_err());

        let mut closed = previous.clone();
        closed.outcome = CycleOutcome::ClosureReview;
        closed.directive = ChiefDirective::ApproveClosure;
        closed.ooda.directive = crate::CycleDirective::ApproveClosure;
        assert!(ResearchCycleRecord::validate_successor(&closed, &current, 150).is_err());
    }
}
