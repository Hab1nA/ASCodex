//! Chief-first AutoPush: decide whether a worker's weak closure report must be force-pushed
//! back to work instead of being accepted.
//!
//! The DeepSeek Harness design (`HARNESS_GUARD_PLUGIN_DESIGN.md` §3.9) turns "close too
//! easily" into a software boundary: a worker that ends with weak evidence (no attempt,
//! someone on the field is higher, or the closure evidence is incomplete) is pushed back by
//! the guard after a Chief decision window. The Chief always owns the decision: any reply
//! inside the window lets the closure through; only a timeout falls back to the guard's push.
//!
//! This module is deliberately pure and offline: it only decides. Persisting the decision and
//! waking the Chief reuses the existing `chief_wake_requests` channel and the resident
//! supervisor, so no second background loop is introduced here.

use crate::{ClosureEvidence, WorkerReport};
use serde::{Deserialize, Serialize};

/// AutoPush outcome for a worker end report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutoPushDecision {
    /// The closure is strong enough: accept it without pushing.
    AcceptClosure,
    /// The closure is weak: force the worker to continue (or, on a repeat, switch to
    /// clean-room red-team mode). `round` is the 1-based push number.
    Push {
        round: u32,
        red_team: bool,
        reason: String,
    },
    /// The closure is weak but the per-worker push interval has not elapsed; wait.
    WaitForInterval { next_push_at_ms: i64 },
}

/// Inputs for the weak-closure decision. All fields are facts the caller has already verified
/// (e.g. from the ledger), not values the worker reports about itself.
#[derive(Debug, Clone)]
pub struct AutoPushInput<'a> {
    /// Closure evidence the worker submitted (if any). `None` is already weak.
    pub closure: Option<&'a ClosureEvidence>,
    /// The worker's end report. `None` (no report at all) is already weak.
    pub report: Option<&'a WorkerReport>,
    /// Whether a higher score is currently on the field for this challenge.
    pub peer_higher: bool,
    /// Historical best reward for this challenge/identity, if known.
    pub historical_best: Option<f64>,
    /// Number of pushes already issued for this worker (0 before the first).
    pub push_round: u32,
    /// Maximum pushes before switching to red-team mode (default 2).
    pub max_pushes: u32,
    /// Minimum interval between two pushes for the same worker (ms).
    pub min_push_interval_ms: i64,
    /// When the last push (or the previous end proposal) was issued, if any.
    pub last_push_at_ms: Option<i64>,
    pub now_ms: i64,
}

const DEFAULT_MAX_PUSHES: u32 = 2;
const DEFAULT_MIN_PUSH_INTERVAL_MS: i64 = 30 * 60 * 1000;

impl Default for AutoPushInput<'_> {
    fn default() -> Self {
        Self {
            closure: None,
            report: None,
            peer_higher: false,
            historical_best: None,
            push_round: 0,
            max_pushes: DEFAULT_MAX_PUSHES,
            min_push_interval_ms: DEFAULT_MIN_PUSH_INTERVAL_MS,
            last_push_at_ms: None,
            now_ms: 0,
        }
    }
}

/// Strong closure requires: a successful worker report that names an attempt, complete closure
/// evidence, no higher score on the field, and no higher historical best that would be
/// overridden by a budget stop.
fn is_strong_closure(input: &AutoPushInput<'_>) -> bool {
    let Some(report) = input.report else {
        return false;
    };
    if report.attempt_id.as_deref().is_none_or(str::is_empty) {
        return false;
    }
    let Some(closure) = input.closure else {
        return false;
    };
    if closure.validate().is_err() {
        return false;
    }
    if input.peer_higher {
        return false;
    }
    if let (Some(current), Some(best)) = (closure.current_reward, input.historical_best) {
        if best > current {
            // A budget stop cannot override a higher historical record.
            return false;
        }
    }
    true
}

/// Decide what the guard must do with a worker end report.
pub fn evaluate_auto_push(input: &AutoPushInput<'_>) -> AutoPushDecision {
    if is_strong_closure(input) {
        return AutoPushDecision::AcceptClosure;
    }
    let reason = weak_reason(input);
    if input.push_round >= input.max_pushes {
        // Repeat weak closure: attack the conclusion with a clean-room red team.
        return AutoPushDecision::Push {
            round: input.push_round + 1,
            red_team: true,
            reason,
        };
    }
    if let Some(last) = input.last_push_at_ms {
        let next = last.saturating_add(input.min_push_interval_ms);
        if input.now_ms < next {
            return AutoPushDecision::WaitForInterval {
                next_push_at_ms: next,
            };
        }
    }
    AutoPushDecision::Push {
        round: input.push_round + 1,
        red_team: false,
        reason,
    }
}

/// Whether the guard may fall back to a push because the Chief decision window elapsed
/// without a reply. The Chief always wins inside the window.
pub fn should_force_push(
    now_ms: i64,
    end_proposed_at_ms: i64,
    chief_window_ms: i64,
    chief_responded: bool,
) -> bool {
    if chief_responded {
        return false;
    }
    if chief_window_ms <= 0 {
        return false;
    }
    now_ms.saturating_sub(end_proposed_at_ms) >= chief_window_ms
}

fn weak_reason(input: &AutoPushInput<'_>) -> String {
    if input.report.is_none() {
        return "worker ended with no end report".to_string();
    }
    let report = input.report.expect("checked");
    if report.attempt_id.as_deref().is_none_or(str::is_empty) {
        return "worker ended without a verifiable attempt".to_string();
    }
    if input.closure.map(|c| c.validate().is_err()).unwrap_or(true) {
        return "closure evidence is incomplete (peer/falsifiers/ceiling/dual-track)".to_string();
    }
    if input.peer_higher {
        return "a higher score is currently on the field".to_string();
    }
    if let (Some(current), Some(best)) = (
        input.closure.and_then(|c| c.current_reward),
        input.historical_best,
    ) {
        if best > current {
            return "historical best is higher than the current result".to_string();
        }
    }
    "weak closure with no stronger rationale".to_string()
}

/// Typed force-push request written for the Chief-first channel. It reuses the existing wake
/// flow: the resident supervisor consumes these the same way it consumes reconciliation
/// wakes, so the Chief sees the push before any worker is resumed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AutoPushRequest {
    pub schema_version: String,
    pub push_id: String,
    pub campaign_id: String,
    pub challenge_id: String,
    pub agent_id: String,
    pub round: u32,
    pub red_team: bool,
    pub reason: String,
    pub issued_at_ms: i64,
    pub platform_write_attempted: bool,
}

pub const AUTO_PUSH_SCHEMA_VERSION: &str = "ascodex-auto-push-request/v1";

impl AutoPushRequest {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != AUTO_PUSH_SCHEMA_VERSION
            || self.push_id.trim().is_empty()
            || self.campaign_id.trim().is_empty()
            || self.challenge_id.trim().is_empty()
            || self.agent_id.trim().is_empty()
            || self.round == 0
            || self.reason.trim().is_empty()
            || self.issued_at_ms <= 0
            || self.platform_write_attempted
        {
            return Err(
                "auto push request is missing required fields or violates the read-only contract"
                    .to_string(),
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ReportStatus, Role, SCHEMA_VERSION, WorkerReport};

    fn report(attempt: Option<&str>) -> WorkerReport {
        WorkerReport {
            schema_version: SCHEMA_VERSION.to_string(),
            role: Role::Solver,
            status: ReportStatus::Success,
            challenge_id: "challenge-a".to_string(),
            identity: Some("id-a".to_string()),
            attempt_id: attempt.map(str::to_string),
            harbor_reward: Some(0.9),
            trace_score: Some(85.0),
            judge_summary: None,
            evidence: Vec::new(),
        }
    }

    fn closure(top_peer_checked: bool, current: Option<f64>) -> ClosureEvidence {
        ClosureEvidence {
            top_peer_checked,
            independent_falsifiers: 2,
            historical_ceiling_checked: true,
            dual_track_verified: true,
            budget_stop_requested: false,
            top_peer_reward: Some(0.9),
            current_reward: current,
            historical_best_reward: Some(0.8),
            evidence: vec![
                crate::EvidenceRef {
                    kind: "closure_falsifier".to_string(),
                    path: "evidence/falsifier-1.json".to_string(),
                    sha256: Some("e".repeat(64)),
                },
                crate::EvidenceRef {
                    kind: "closure_falsifier".to_string(),
                    path: "evidence/falsifier-2.json".to_string(),
                    sha256: Some("f".repeat(64)),
                },
                crate::EvidenceRef {
                    kind: "closure_peer".to_string(),
                    path: "evidence/peer.json".to_string(),
                    sha256: Some("a".repeat(64)),
                },
                crate::EvidenceRef {
                    kind: "closure_historical".to_string(),
                    path: "evidence/historical.json".to_string(),
                    sha256: Some("b".repeat(64)),
                },
                crate::EvidenceRef {
                    kind: "closure_harbor".to_string(),
                    path: "evidence/harbor.json".to_string(),
                    sha256: Some("c".repeat(64)),
                },
                crate::EvidenceRef {
                    kind: "closure_trace".to_string(),
                    path: "evidence/trace.json".to_string(),
                    sha256: Some("d".repeat(64)),
                },
            ],
        }
    }

    #[test]
    fn strong_closure_is_accepted() {
        let c = closure(true, Some(0.9));
        assert!(
            c.validate().is_ok(),
            "test closure must be valid: {:?}",
            c.validate()
        );
        let input = AutoPushInput {
            closure: Some(&c),
            report: Some(&report(Some("att-1"))),
            peer_higher: false,
            historical_best: Some(0.8),
            push_round: 0,
            ..AutoPushInput::default()
        };
        assert_eq!(evaluate_auto_push(&input), AutoPushDecision::AcceptClosure);
    }

    #[test]
    fn no_attempt_is_pushed_and_repeats_escalate_to_red_team() {
        let input = AutoPushInput {
            closure: Some(&closure(true, Some(0.9))),
            report: Some(&report(None)),
            peer_higher: false,
            push_round: 0,
            ..AutoPushInput::default()
        };
        assert!(matches!(
            evaluate_auto_push(&input),
            AutoPushDecision::Push {
                red_team: false,
                ..
            }
        ));

        // Second weak closure (round == max_pushes) escalates to clean-room red team.
        let input = AutoPushInput {
            closure: Some(&closure(true, Some(0.9))),
            report: Some(&report(None)),
            peer_higher: false,
            push_round: 2,
            ..AutoPushInput::default()
        };
        assert!(matches!(
            evaluate_auto_push(&input),
            AutoPushDecision::Push { red_team: true, .. }
        ));
    }

    #[test]
    fn peer_higher_and_historical_best_block_closure() {
        // Peer higher than current result is weak.
        let input = AutoPushInput {
            closure: Some(&closure(true, Some(0.9))),
            report: Some(&report(Some("att-1"))),
            peer_higher: true,
            push_round: 0,
            ..AutoPushInput::default()
        };
        assert!(matches!(
            evaluate_auto_push(&input),
            AutoPushDecision::Push { .. }
        ));

        // Historical best above current is weak even with a budget stop request.
        let mut c = closure(true, Some(0.9));
        c.budget_stop_requested = true;
        let input = AutoPushInput {
            closure: Some(&c),
            report: Some(&report(Some("att-1"))),
            peer_higher: false,
            historical_best: Some(0.99),
            push_round: 0,
            ..AutoPushInput::default()
        };
        assert!(matches!(
            evaluate_auto_push(&input),
            AutoPushDecision::Push { .. }
        ));
    }

    #[test]
    fn interval_gate_waits_between_pushes() {
        let input = AutoPushInput {
            closure: Some(&closure(true, Some(0.9))),
            report: Some(&report(None)),
            peer_higher: false,
            push_round: 1,
            last_push_at_ms: Some(1_000),
            min_push_interval_ms: 60_000,
            now_ms: 10_000,
            ..AutoPushInput::default()
        };
        assert!(matches!(
            evaluate_auto_push(&input),
            AutoPushDecision::WaitForInterval {
                next_push_at_ms: 61_000
            }
        ));
    }

    #[test]
    fn chief_window_force_push_respects_reply() {
        // Chief replied inside the window: never force.
        assert!(!should_force_push(2_000, 1_000, 90_000, true));
        // Window elapsed without a reply: force.
        assert!(should_force_push(100_000, 1_000, 90_000, false));
        // Non-positive window never forces.
        assert!(!should_force_push(100_000, 1_000, 0, false));
    }

    #[test]
    fn auto_push_request_validate_fails_closed() {
        let request = AutoPushRequest {
            schema_version: AUTO_PUSH_SCHEMA_VERSION.to_string(),
            push_id: "push-1".to_string(),
            campaign_id: "campaign-a".to_string(),
            challenge_id: "challenge-a".to_string(),
            agent_id: "agent-a".to_string(),
            round: 1,
            red_team: false,
            reason: "weak closure".to_string(),
            issued_at_ms: 100,
            platform_write_attempted: false,
        };
        request.validate().expect("valid push request");

        let mut tampered = request.clone();
        tampered.platform_write_attempted = true;
        assert!(tampered.validate().is_err());
        let mut tampered = request;
        tampered.round = 0;
        assert!(tampered.validate().is_err());
    }

    /// Real E2E scenario: a solver ends with no attempt (lazy/drifted end report).
    /// The guard must push it back, respect the cooldown interval, escalate to a
    /// clean-room red team after max_pushes, and the Chief always wins inside its
    /// decision window.
    #[test]
    fn e2e_lazy_solver_is_pushed_then_red_teamed_and_chief_wins_window() {
        let c = closure(true, Some(0.9));
        let input = AutoPushInput {
            closure: Some(&c),
            report: Some(&report(None)), // solver ended without a verifiable attempt
            peer_higher: false,
            push_round: 0,
            ..AutoPushInput::default()
        };
        // 1st weak end -> Push (round 1, no red team)
        match evaluate_auto_push(&input) {
            AutoPushDecision::Push { round, red_team, .. } => {
                assert_eq!(round, 1);
                assert!(!red_team);
            }
            other => panic!("expected Push round 1, got {other:?}"),
        }

        // Cooldown not yet elapsed -> WaitForInterval with a concrete next time.
        let input = AutoPushInput {
            closure: Some(&c),
            report: Some(&report(None)),
            peer_higher: false,
            push_round: 1,
            last_push_at_ms: Some(10_000),
            min_push_interval_ms: 60_000,
            now_ms: 30_000,
            ..AutoPushInput::default()
        };
        match evaluate_auto_push(&input) {
            AutoPushDecision::WaitForInterval { next_push_at_ms } => {
                assert_eq!(next_push_at_ms, 70_000);
            }
            other => panic!("expected WaitForInterval, got {other:?}"),
        }

        // Interval elapsed, still weak -> Push round 2.
        let input = AutoPushInput {
            closure: Some(&c),
            report: Some(&report(None)),
            peer_higher: false,
            push_round: 1,
            last_push_at_ms: Some(10_000),
            min_push_interval_ms: 60_000,
            now_ms: 80_000,
            max_pushes: 2,
            ..AutoPushInput::default()
        };
        match evaluate_auto_push(&input) {
            AutoPushDecision::Push { round, red_team, .. } => {
                assert_eq!(round, 2);
                assert!(!red_team);
            }
            other => panic!("expected Push round 2, got {other:?}"),
        }

        // Exhausted max_pushes -> escalate to clean-room red team.
        let input = AutoPushInput {
            closure: Some(&c),
            report: Some(&report(None)),
            peer_higher: false,
            push_round: 2,
            last_push_at_ms: Some(140_000),
            min_push_interval_ms: 60_000,
            now_ms: 210_000,
            max_pushes: 2,
            ..AutoPushInput::default()
        };
        match evaluate_auto_push(&input) {
            AutoPushDecision::Push { round, red_team, .. } => {
                assert_eq!(round, 3);
                assert!(red_team, "repeated weak ends must escalate to red team");
            }
            other => panic!("expected red-team push, got {other:?}"),
        }

        // Chief window: no reply yet -> force push once the window elapses.
        assert!(!should_force_push(50_000, 10_000, 90_000, false));
        assert!(should_force_push(110_000, 10_000, 90_000, false));
        // Chief replied -> never force inside or outside the window.
        assert!(!should_force_push(200_000, 10_000, 90_000, true));
    }
}
