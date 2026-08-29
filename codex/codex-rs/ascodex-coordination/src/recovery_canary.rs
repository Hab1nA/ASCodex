use crate::{CoordinationError, SCHEMA_VERSION};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

/// Process recovery is deliberately separate from campaign recovery. Formal research threads may
/// only be rehydrated after an isolated child has completed both an initial and a continuation
/// turn in this runtime instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryPhase {
    Boot,
    LedgerChecked,
    CanarySpawned,
    InitialTurnCompleted,
    ContinuationTurnCompleted,
    CanaryPassed,
    Rehydrated,
    Active,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RecoveryCanaryEvidence {
    Boot,
    LedgerChecked {
        /// Digest of a canonical ledger projection, not the SQLite file bytes.
        ledger_state_sha256: String,
    },
    CanarySpawned {
        root_thread_id: String,
        child_thread_id: String,
        session_id: String,
        effective_model_route: String,
        permission_profile_sha256: String,
        ephemeral: bool,
        network_disabled: bool,
        filesystem_write_disabled: bool,
    },
    InitialTurnCompleted(RecoveryCanaryTurn),
    ContinuationTurnCompleted(RecoveryCanaryTurn),
    CanaryPassed {
        child_shutdown_observed: bool,
    },
    Rehydrated {
        /// Must still match the pre-canary canonical projection. The canary is not allowed to
        /// mutate campaign state.
        ledger_state_sha256: String,
        rehydrated_thread_ids: Vec<String>,
    },
    Active,
    Failed {
        after_phase: RecoveryPhase,
        reason: String,
    },
}

impl RecoveryCanaryEvidence {
    pub fn phase(&self) -> RecoveryPhase {
        match self {
            Self::Boot => RecoveryPhase::Boot,
            Self::LedgerChecked { .. } => RecoveryPhase::LedgerChecked,
            Self::CanarySpawned { .. } => RecoveryPhase::CanarySpawned,
            Self::InitialTurnCompleted(_) => RecoveryPhase::InitialTurnCompleted,
            Self::ContinuationTurnCompleted(_) => RecoveryPhase::ContinuationTurnCompleted,
            Self::CanaryPassed { .. } => RecoveryPhase::CanaryPassed,
            Self::Rehydrated { .. } => RecoveryPhase::Rehydrated,
            Self::Active => RecoveryPhase::Active,
            Self::Failed { .. } => RecoveryPhase::Failed,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryCanaryTurn {
    pub child_thread_id: String,
    pub session_id: String,
    pub turn_id: String,
    /// A per-turn random marker. The runner asks the child to return it and verifies the actual
    /// terminal model message rather than trusting a lifecycle status alone.
    pub nonce: String,
    pub last_agent_message: String,
    pub response_sha256: String,
    pub started_at_ms: i64,
    pub completed_at_ms: i64,
    pub parent_observed_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryCanaryEvent {
    pub event_id: String,
    pub observed_at_ms: i64,
    pub evidence: RecoveryCanaryEvidence,
}

/// Append-only evidence for one process recovery attempt.
///
/// Valid prefixes may be persisted while a canary is running. Only a complete trace ending in
/// `Active` authorizes formal work; `Failed` is terminal and never authorizes rehydration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryCanaryTrace {
    pub schema_version: String,
    pub recovery_id: String,
    pub runtime_instance_id: String,
    pub recovery_attempt: u64,
    pub started_at_ms: i64,
    pub deadline_ms: i64,
    pub events: Vec<RecoveryCanaryEvent>,
}

impl RecoveryCanaryTrace {
    pub fn validate(&self, now_ms: i64) -> Result<(), CoordinationError> {
        if self.schema_version != SCHEMA_VERSION
            || self.recovery_id.trim().is_empty()
            || self.runtime_instance_id.trim().is_empty()
            || self.recovery_attempt == 0
            || self.started_at_ms < 0
            || self.deadline_ms <= self.started_at_ms
            || self.events.is_empty()
        {
            return Err(invalid(
                "schema, recovery/runtime ids, positive attempt, deadline, and events are required",
            ));
        }

        let successful_order = [
            RecoveryPhase::Boot,
            RecoveryPhase::LedgerChecked,
            RecoveryPhase::CanarySpawned,
            RecoveryPhase::InitialTurnCompleted,
            RecoveryPhase::ContinuationTurnCompleted,
            RecoveryPhase::CanaryPassed,
            RecoveryPhase::Rehydrated,
            RecoveryPhase::Active,
        ];
        let mut event_ids = BTreeSet::new();
        let mut previous_observed_at = self.started_at_ms;
        let mut successful_phases = Vec::new();
        let mut failed = false;

        for event in &self.events {
            if event.event_id.trim().is_empty() || !event_ids.insert(event.event_id.as_str()) {
                return Err(invalid("event ids must be non-empty and unique"));
            }
            if event.observed_at_ms < previous_observed_at
                || event.observed_at_ms > self.deadline_ms
                || event.observed_at_ms > now_ms
            {
                return Err(invalid(
                    "event timestamps must be monotonic, observed, and inside the recovery deadline",
                ));
            }
            previous_observed_at = event.observed_at_ms;

            match &event.evidence {
                RecoveryCanaryEvidence::Failed {
                    after_phase,
                    reason,
                } => {
                    if failed
                        || reason.trim().is_empty()
                        || matches!(after_phase, RecoveryPhase::Active | RecoveryPhase::Failed)
                        || successful_phases.last().copied() != Some(*after_phase)
                    {
                        return Err(invalid(
                            "failure must be terminal, explained, and attached to the last successful phase",
                        ));
                    }
                    failed = true;
                }
                evidence => {
                    if failed {
                        return Err(invalid("failure is terminal"));
                    }
                    let phase = evidence.phase();
                    let expected = successful_order.get(successful_phases.len()).copied();
                    if expected != Some(phase) {
                        return Err(invalid("recovery phases cannot be skipped or reordered"));
                    }
                    successful_phases.push(phase);
                }
            }
        }

        if now_ms > self.deadline_ms
            && !failed
            && successful_phases.last().copied() != Some(RecoveryPhase::Active)
        {
            return Err(invalid("expired recovery must record a terminal failure"));
        }

        self.validate_evidence()?;
        Ok(())
    }

    pub fn activation_allowed(&self, now_ms: i64) -> bool {
        self.validate(now_ms).is_ok()
            && matches!(
                self.events.last().map(|event| event.evidence.phase()),
                Some(RecoveryPhase::Active)
            )
    }

    /// Authorizes only the transition from the isolated process canary into formal thread
    /// rehydration. It does not authorize the recovered agent to become active; the caller must
    /// append and persist matching `Rehydrated` and `Active` evidence afterwards.
    pub fn rehydration_allowed(&self, now_ms: i64) -> bool {
        self.validate(now_ms).is_ok()
            && matches!(
                self.events.last().map(|event| event.evidence.phase()),
                Some(RecoveryPhase::CanaryPassed)
            )
    }

    fn validate_evidence(&self) -> Result<(), CoordinationError> {
        let mut ledger_digest = None;
        let mut spawned = None;
        let mut first_turn = None;

        for event in &self.events {
            match &event.evidence {
                RecoveryCanaryEvidence::LedgerChecked {
                    ledger_state_sha256,
                } => {
                    validate_digest(ledger_state_sha256, "ledger state")?;
                    ledger_digest = Some(ledger_state_sha256.as_str());
                }
                RecoveryCanaryEvidence::CanarySpawned {
                    root_thread_id,
                    child_thread_id,
                    session_id,
                    effective_model_route,
                    permission_profile_sha256,
                    ephemeral,
                    network_disabled,
                    filesystem_write_disabled,
                } => {
                    if [
                        root_thread_id.as_str(),
                        child_thread_id.as_str(),
                        session_id.as_str(),
                        effective_model_route.as_str(),
                    ]
                    .iter()
                    .any(|value| value.trim().is_empty())
                        || root_thread_id == child_thread_id
                        || !ephemeral
                        || !network_disabled
                        || !filesystem_write_disabled
                    {
                        return Err(invalid(
                            "canary must use a distinct isolated ephemeral child and record its effective model route",
                        ));
                    }
                    validate_digest(permission_profile_sha256, "permission profile")?;
                    spawned = Some((child_thread_id.as_str(), session_id.as_str()));
                }
                RecoveryCanaryEvidence::InitialTurnCompleted(turn) => {
                    let (child_thread_id, session_id) = spawned
                        .ok_or_else(|| invalid("initial turn has no matching spawned child"))?;
                    validate_turn(turn, event.observed_at_ms, child_thread_id, session_id)?;
                    first_turn = Some(turn);
                }
                RecoveryCanaryEvidence::ContinuationTurnCompleted(turn) => {
                    let (child_thread_id, session_id) = spawned.ok_or_else(|| {
                        invalid("continuation turn has no matching spawned child")
                    })?;
                    validate_turn(turn, event.observed_at_ms, child_thread_id, session_id)?;
                    let first = first_turn
                        .ok_or_else(|| invalid("continuation requires a completed initial turn"))?;
                    if turn.turn_id == first.turn_id
                        || turn.nonce == first.nonce
                        || turn.started_at_ms < first.parent_observed_at_ms
                    {
                        return Err(invalid(
                            "continuation must be a distinct turn started after the parent observed the initial completion",
                        ));
                    }
                }
                RecoveryCanaryEvidence::CanaryPassed {
                    child_shutdown_observed,
                } => {
                    if !child_shutdown_observed {
                        return Err(invalid(
                            "canary pass requires observing clean child shutdown",
                        ));
                    }
                }
                RecoveryCanaryEvidence::Rehydrated {
                    ledger_state_sha256,
                    rehydrated_thread_ids,
                } => {
                    validate_digest(ledger_state_sha256, "rehydrated ledger state")?;
                    if ledger_digest != Some(ledger_state_sha256.as_str()) {
                        return Err(invalid(
                            "canonical ledger state changed while the isolated canary ran",
                        ));
                    }
                    let unique = rehydrated_thread_ids.iter().collect::<BTreeSet<_>>();
                    if unique.len() != rehydrated_thread_ids.len()
                        || rehydrated_thread_ids
                            .iter()
                            .any(|thread_id| thread_id.trim().is_empty())
                        || spawned.is_some_and(|(child_thread_id, _)| {
                            rehydrated_thread_ids
                                .iter()
                                .any(|thread_id| thread_id == child_thread_id)
                        })
                    {
                        return Err(invalid(
                            "rehydrated thread ids must be unique and cannot include the disposable canary",
                        ));
                    }
                }
                RecoveryCanaryEvidence::Boot
                | RecoveryCanaryEvidence::Active
                | RecoveryCanaryEvidence::Failed { .. } => {}
            }
        }
        Ok(())
    }
}

fn validate_turn(
    turn: &RecoveryCanaryTurn,
    observed_at_ms: i64,
    child_thread_id: &str,
    session_id: &str,
) -> Result<(), CoordinationError> {
    if turn.child_thread_id != child_thread_id
        || turn.session_id != session_id
        || turn.turn_id.trim().is_empty()
        || turn.nonce.len() < 16
        || turn.nonce.chars().any(char::is_whitespace)
        || turn.last_agent_message.trim().is_empty()
        || !turn.last_agent_message.contains(&turn.nonce)
        || turn.started_at_ms < 0
        || turn.completed_at_ms < turn.started_at_ms
        || turn.parent_observed_at_ms < turn.completed_at_ms
        || observed_at_ms != turn.parent_observed_at_ms
    {
        return Err(invalid(
            "turn evidence must bind the spawned child/session, ordered lifecycle times, and an echoed nonce",
        ));
    }
    validate_digest(&turn.response_sha256, "canary response").and_then(|()| {
        let actual = format!("{:x}", Sha256::digest(turn.last_agent_message.as_bytes()));
        if turn.response_sha256 == actual {
            Ok(())
        } else {
            Err(invalid(
                "canary response digest does not match the terminal model message",
            ))
        }
    })
}

fn validate_digest(value: &str, label: &str) -> Result<(), CoordinationError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(invalid(&format!(
            "{label} must be a 64-character hexadecimal digest"
        )));
    }
    Ok(())
}

fn invalid(message: &str) -> CoordinationError {
    CoordinationError::InvalidDecision(format!("recovery canary is invalid: {message}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(
        event_id: &str,
        observed_at_ms: i64,
        evidence: RecoveryCanaryEvidence,
    ) -> RecoveryCanaryEvent {
        RecoveryCanaryEvent {
            event_id: event_id.into(),
            observed_at_ms,
            evidence,
        }
    }

    fn turn(turn_id: &str, nonce: &str, started: i64) -> RecoveryCanaryTurn {
        let last_agent_message = format!("healthy {nonce}");
        RecoveryCanaryTurn {
            child_thread_id: "canary-child".into(),
            session_id: "canary-session".into(),
            turn_id: turn_id.into(),
            nonce: nonce.into(),
            response_sha256: format!("{:x}", Sha256::digest(last_agent_message.as_bytes())),
            last_agent_message,
            started_at_ms: started,
            completed_at_ms: started + 10,
            parent_observed_at_ms: started + 20,
        }
    }

    fn complete_trace() -> RecoveryCanaryTrace {
        let ledger_digest = "a".repeat(64);
        RecoveryCanaryTrace {
            schema_version: SCHEMA_VERSION.into(),
            recovery_id: "recovery-1".into(),
            runtime_instance_id: "runtime-2".into(),
            recovery_attempt: 1,
            started_at_ms: 100,
            deadline_ms: 1_000,
            events: vec![
                event("boot", 100, RecoveryCanaryEvidence::Boot),
                event(
                    "ledger",
                    110,
                    RecoveryCanaryEvidence::LedgerChecked {
                        ledger_state_sha256: ledger_digest.clone(),
                    },
                ),
                event(
                    "spawn",
                    120,
                    RecoveryCanaryEvidence::CanarySpawned {
                        root_thread_id: "canary-root".into(),
                        child_thread_id: "canary-child".into(),
                        session_id: "canary-session".into(),
                        effective_model_route: "provider/model/default".into(),
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
                event(
                    "rehydrated",
                    230,
                    RecoveryCanaryEvidence::Rehydrated {
                        ledger_state_sha256: ledger_digest,
                        rehydrated_thread_ids: vec!["chief-thread".into()],
                    },
                ),
                event("active", 240, RecoveryCanaryEvidence::Active),
            ],
        }
    }

    #[test]
    fn two_turn_isolated_canary_authorizes_activation() {
        let trace = complete_trace();
        assert!(trace.validate(300).is_ok());
        assert!(trace.activation_allowed(300));
    }

    #[test]
    fn valid_prefix_does_not_authorize_formal_work() {
        let mut trace = complete_trace();
        trace.events.truncate(5);
        assert!(trace.validate(300).is_ok());
        assert!(!trace.activation_allowed(300));
    }

    #[test]
    fn only_canary_passed_prefix_authorizes_rehydration() {
        let mut trace = complete_trace();
        trace.events.truncate(6);
        assert!(trace.rehydration_allowed(300));
        assert!(!trace.activation_allowed(300));

        trace.events.truncate(5);
        assert!(!trace.rehydration_allowed(300));
    }

    #[test]
    fn phase_skip_and_expired_unfinished_trace_fail_closed() {
        let mut skipped = complete_trace();
        skipped.events.remove(4);
        assert!(skipped.validate(300).is_err());

        let mut expired = complete_trace();
        expired.events.truncate(4);
        assert!(expired.validate(1_001).is_err());
        assert!(!expired.activation_allowed(1_001));
    }

    #[test]
    fn explicit_failure_is_terminal_and_never_activates() {
        let mut trace = complete_trace();
        trace.events.truncate(4);
        trace.events.push(event(
            "failed",
            170,
            RecoveryCanaryEvidence::Failed {
                after_phase: RecoveryPhase::InitialTurnCompleted,
                reason: "continuation turn errored".into(),
            },
        ));
        assert!(trace.validate(1_001).is_ok());
        assert!(!trace.activation_allowed(1_001));

        trace
            .events
            .push(event("illegal-active", 180, RecoveryCanaryEvidence::Active));
        assert!(trace.validate(300).is_err());
    }

    #[test]
    fn continuation_must_reuse_child_and_echo_a_fresh_nonce() {
        let mut wrong_child = complete_trace();
        let RecoveryCanaryEvidence::ContinuationTurnCompleted(turn) =
            &mut wrong_child.events[4].evidence
        else {
            panic!("expected continuation evidence");
        };
        turn.child_thread_id = "replacement-child".into();
        assert!(wrong_child.validate(300).is_err());

        let mut replayed = complete_trace();
        let first_nonce = match &replayed.events[3].evidence {
            RecoveryCanaryEvidence::InitialTurnCompleted(turn) => turn.nonce.clone(),
            _ => panic!("expected initial evidence"),
        };
        let RecoveryCanaryEvidence::ContinuationTurnCompleted(turn) =
            &mut replayed.events[4].evidence
        else {
            panic!("expected continuation evidence");
        };
        turn.nonce = first_nonce.clone();
        turn.last_agent_message = first_nonce;
        assert!(replayed.validate(300).is_err());
    }

    #[test]
    fn canary_cannot_mutate_ledger_or_join_rehydrated_threads() {
        let mut drifted = complete_trace();
        let RecoveryCanaryEvidence::Rehydrated {
            ledger_state_sha256,
            ..
        } = &mut drifted.events[6].evidence
        else {
            panic!("expected rehydration evidence");
        };
        *ledger_state_sha256 = "d".repeat(64);
        assert!(drifted.validate(300).is_err());

        let mut retained = complete_trace();
        let RecoveryCanaryEvidence::Rehydrated {
            rehydrated_thread_ids,
            ..
        } = &mut retained.events[6].evidence
        else {
            panic!("expected rehydration evidence");
        };
        rehydrated_thread_ids.push("canary-child".into());
        assert!(retained.validate(300).is_err());
    }

    #[test]
    fn terminal_message_digest_is_bound_and_canonical() {
        let mut tampered = complete_trace();
        let RecoveryCanaryEvidence::InitialTurnCompleted(turn) = &mut tampered.events[3].evidence
        else {
            panic!("expected initial evidence");
        };
        turn.last_agent_message.push_str(" altered");
        assert!(tampered.validate(300).is_err());

        let mut uppercase = complete_trace();
        let RecoveryCanaryEvidence::InitialTurnCompleted(turn) = &mut uppercase.events[3].evidence
        else {
            panic!("expected initial evidence");
        };
        turn.response_sha256 = turn.response_sha256.to_ascii_uppercase();
        assert!(uppercase.validate(300).is_err());
    }
}
