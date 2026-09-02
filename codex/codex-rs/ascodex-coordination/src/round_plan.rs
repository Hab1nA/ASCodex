//! Round-level dispatch contract: one competition round, up to ten challenges,
//! one solver child per challenge, all dispatched in a single atomic batch.
//!
//! A round plan is the round-scoped analogue of a typed ChallengeContract: the
//! operator (or the chief's first turn) pins the challenge set, the per-challenge
//! Chief lease, and the child task template before any solver is spawned. The
//! `solver_round_dispatch` tool validates this file before spawning, so the
//! model can never improvise the challenge set at dispatch time.

use crate::CoordinationError;
use serde::{Deserialize, Serialize};

/// A competition round carries at most ten challenges.
pub const MAX_ROUND_CHALLENGES: usize = 10;

pub const ROUND_PLAN_SCHEMA_VERSION: &str = "ascodex-round-plan/v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoundPlanChallenge {
    pub challenge_id: String,
    /// The per-challenge Chief lease that authorized this challenge's research
    /// cycle (and therefore this child's dispatch).
    pub lease_id: String,
    /// Absolute challenge workspace root rendered into the child task message.
    pub workspace_root: String,
    /// Optional per-challenge task name; defaults to `solver_{challenge_id}`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_name: Option<String>,
    /// Optional per-challenge task message; overrides the plan template.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoundPlan {
    pub schema_version: String,
    pub round_id: String,
    pub campaign_id: String,
    /// Task message template rendered per challenge. `{challenge_id}` and
    /// `{challenge_workspace}` placeholders are replaced with the challenge
    /// identity and its workspace root.
    pub task_message_template: String,
    pub challenges: Vec<RoundPlanChallenge>,
}

impl RoundPlan {
    pub fn validate(&self) -> Result<(), CoordinationError> {
        if self.schema_version != ROUND_PLAN_SCHEMA_VERSION
            || self.round_id.trim().is_empty()
            || self.campaign_id.trim().is_empty()
        {
            return Err(invalid("round plan identifiers are required"));
        }
        if self.challenges.is_empty() || self.challenges.len() > MAX_ROUND_CHALLENGES {
            return Err(invalid(&format!(
                "a round dispatches 1..={MAX_ROUND_CHALLENGES} challenges"
            )));
        }
        if !self.task_message_template.contains("{challenge_id}")
            || !self.task_message_template.contains("{challenge_workspace}")
        {
            return Err(invalid(
                "task message template must bind {challenge_id} and {challenge_workspace}",
            ));
        }
        let mut challenge_ids = std::collections::BTreeSet::new();
        let mut lease_ids = std::collections::BTreeSet::new();
        let mut workspace_roots = std::collections::BTreeSet::new();
        let mut task_names = std::collections::BTreeSet::new();
        for challenge in &self.challenges {
            if !valid_identifier(&challenge.challenge_id) {
                return Err(invalid("challenge ids must be non-empty path-safe identifiers"));
            }
            if challenge.lease_id.trim().is_empty() || challenge.lease_id.contains('/') {
                return Err(invalid("challenge lease ids must be non-empty"));
            }
            if !std::path::Path::new(&challenge.workspace_root).is_absolute() {
                return Err(invalid("challenge workspace roots must be absolute"));
            }
            if !challenge_ids.insert(challenge.challenge_id.as_str()) {
                return Err(invalid("challenge ids must be unique within a round"));
            }
            if !lease_ids.insert(challenge.lease_id.as_str()) {
                return Err(invalid("chief lease ids must be unique within a round"));
            }
            if !workspace_roots.insert(challenge.workspace_root.as_str()) {
                return Err(invalid("challenge workspace roots must be unique within a round"));
            }
            let task_name = self.task_name_for(challenge);
            if !valid_task_name(&task_name) {
                return Err(invalid(
                    "task names must be lowercase letters, digits, and underscores only",
                ));
            }
            if !task_names.insert(task_name) {
                return Err(invalid("task names must be unique within a round"));
            }
            if let Some(message) = &challenge.message
                && message.trim().is_empty()
            {
                return Err(invalid("per-challenge messages must be non-empty"));
            }
        }
        Ok(())
    }

    /// The effective task name for one challenge. Agent paths only accept
    /// lowercase letters, digits, and underscores, so every non-conforming
    /// character (e.g. the hyphen in `ch-01`) is mapped to `_`.
    pub fn task_name_for(&self, challenge: &RoundPlanChallenge) -> String {
        challenge.task_name.clone().unwrap_or_else(|| {
            let sanitized: String = challenge
                .challenge_id
                .bytes()
                .map(|byte| {
                    if byte.is_ascii_lowercase() || byte.is_ascii_digit() {
                        byte as char
                    } else {
                        '_'
                    }
                })
                .collect();
            format!("solver_{sanitized}")
        })
    }

    /// The effective task message for one challenge: the per-challenge message
    /// when present, otherwise the rendered template.
    pub fn message_for(&self, challenge: &RoundPlanChallenge) -> String {
        if let Some(message) = &challenge.message {
            return message.clone();
        }
        self.task_message_template
            .replace("{challenge_id}", &challenge.challenge_id)
            .replace("{challenge_workspace}", &challenge.workspace_root)
    }
}

fn valid_identifier(value: &str) -> bool {
    !value.trim().is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        && !value.starts_with('.')
        && !value.starts_with('-')
}

fn valid_task_name(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

fn invalid(message: &str) -> CoordinationError {
    CoordinationError::InvalidDecision(format!("round plan is invalid: {message}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn challenge(id: &str, lease: &str) -> RoundPlanChallenge {
        RoundPlanChallenge {
            challenge_id: id.to_string(),
            lease_id: lease.to_string(),
            workspace_root: format!("C:/ws/{id}"),
            task_name: None,
            message: None,
        }
    }

    fn plan(challenges: Vec<RoundPlanChallenge>) -> RoundPlan {
        RoundPlan {
            schema_version: ROUND_PLAN_SCHEMA_VERSION.to_string(),
            round_id: "round-1".to_string(),
            campaign_id: "camp-round-1".to_string(),
            task_message_template: "solve {challenge_id} inside {challenge_workspace}"
                .to_string(),
            challenges,
        }
    }

    #[test]
    fn accepts_valid_round_plan_and_renders_messages() {
        let valid = plan(vec![
            challenge("ch-01", "lease-1"),
            challenge("ch-02", "lease-2"),
        ]);
        valid.validate().expect("valid round plan");
        assert_eq!(
            valid.message_for(&valid.challenges[0]),
            "solve ch-01 inside C:/ws/ch-01"
        );
        assert_eq!(valid.task_name_for(&valid.challenges[1]), "solver_ch_02");
    }

    #[test]
    fn rejects_out_of_range_duplicate_and_unsafe_plans() {
        let empty = plan(Vec::new());
        assert!(empty.validate().is_err());

        let eleven = plan((0..11).map(|i| challenge(&format!("ch-{i:02}"), &format!("lease-{i}"))).collect());
        assert!(eleven.validate().is_err());

        let duplicate = plan(vec![challenge("ch-01", "lease-1"), challenge("ch-01", "lease-2")]);
        assert!(duplicate.validate().is_err());

        let duplicate_lease = plan(vec![challenge("ch-01", "lease-1"), challenge("ch-02", "lease-1")]);
        assert!(duplicate_lease.validate().is_err());

        let traversal = plan(vec![challenge("../escape", "lease-1")]);
        assert!(traversal.validate().is_err());

        let relative_root = RoundPlanChallenge {
            challenge_id: "ch-01".to_string(),
            lease_id: "lease-1".to_string(),
            workspace_root: "ws/ch-01".to_string(),
            task_name: None,
            message: None,
        };
        assert!(plan(vec![relative_root]).validate().is_err());

        let mut template = plan(vec![challenge("ch-01", "lease-1")]);
        template.task_message_template = "no placeholders".to_string();
        assert!(template.validate().is_err());
    }

    #[test]
    fn per_challenge_message_overrides_template() {
        let mut overridden = challenge("ch-01", "lease-1");
        overridden.message = Some("custom task".to_string());
        let valid = plan(vec![overridden]);
        valid.validate().expect("valid round plan");
        assert_eq!(valid.message_for(&valid.challenges[0]), "custom task");
    }
}
