//! Offline, typed reducer for platform reconciliation.
//!
//! The monitor/client is intentionally outside this crate.  It may fetch a page of platform
//! facts and hand the page to this reducer, but the reducer never performs network I/O and never
//! treats a missing response as a failed or successful attempt.  The cursor and response hash
//! make a replayed page observable across process restarts; an item that cannot be verified is
//! retained as `unknown_needs_reconcile` rather than being counted as in-flight.

use crate::{CoordinationError, EvidenceAvailability, PlatformObservation};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const RECONCILIATION_SCHEMA_VERSION: &str = "ascodex-platform-reconciliation/v1";

/// A monotonic position in one read-only platform feed.  The feed owner is responsible for
/// mapping an opaque API cursor to this position before invoking the reducer.  Positions are
/// deliberately scoped by `stream_id`; a cursor from another challenge or endpoint is rejected.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReconciliationCursor {
    pub stream_id: String,
    pub position: u64,
}

impl ReconciliationCursor {
    pub fn validate(&self, expected_stream: &str) -> Result<(), CoordinationError> {
        if expected_stream.trim().is_empty()
            || self.stream_id != expected_stream
            || self.stream_id.trim().is_empty()
        {
            return Err(invalid("reconciliation cursor is not bound to the feed"));
        }
        Ok(())
    }
}

/// A page item from a read-only client.  `UnknownNeedsReconcile` is used when the client got a
/// response (and therefore has a response hash) but could not prove the complete evidence set.
/// It is not an attempt failure and must not advance any quota/cadence accounting.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlatformReconcileItem {
    pub cursor: ReconciliationCursor,
    pub challenge_id: String,
    pub attempt_id: String,
    pub route: String,
    pub observed_at_ms: i64,
    pub response_sha256: String,
    /// Score/ownership/bundle facts are kept separate from the legacy observation so an API
    /// response can expose the raw score and a later penalty without rewriting history.
    #[serde(default)]
    pub facts: ReconciliationFacts,
    pub state: PlatformReconcileItemState,
}

/// Platform facts that are useful for reconciliation but are not sufficient by themselves to
/// prove an official score.  `raw_score` remains visible after a penalty; `effective_score` is
/// the value used for crediting and is never used to refund quota/cadence.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ReconciliationFacts {
    pub raw_score: Option<f64>,
    pub effective_score: Option<f64>,
    /// Penalties are intentionally bounded to the current platform contract (-1), never the
    /// historical -1000 sentinel.  An applied penalty subtracts exactly one point from the raw
    /// score to produce the credited effective score; it does not hide or replace the raw score.
    pub penalty: Option<f64>,
    pub penalty_applied: bool,
    pub penalty_basis: Option<PenaltyBasis>,
    pub credited_owner: Option<String>,
    pub bundle_revision: Option<String>,
    pub rescore_status: Option<BundleRescoreStatus>,
    /// Missing execution trace is not a pending score.  Such a page must be represented as an
    /// explicit unknown item and revisited after a fresh read.
    pub trace_evidence: Option<EvidenceAvailability>,
    pub score_evidence: Option<EvidenceAvailability>,
    pub penalty_evidence: Option<EvidenceAvailability>,
    pub credited_owner_evidence: Option<EvidenceAvailability>,
    pub bundle_evidence: Option<EvidenceAvailability>,
    pub leaderboard_scope: Option<LeaderboardScope>,
    pub anti_cheat: Option<AntiCheatEvidence>,
    pub anonymous_other_submission_access: Option<AnonymousSubmissionAccess>,
    pub challenge_page: Option<ChallengePageEvidence>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LeaderboardScope {
    UnifiedOverallAndSeason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AntiCheatMode {
    WeightedThreeSignals,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AntiCheatSignal {
    pub name: String,
    pub weight: f64,
    pub availability: EvidenceAvailability,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AntiCheatEvidence {
    pub mode: AntiCheatMode,
    pub signals: Vec<AntiCheatSignal>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnonymousSubmissionAccess {
    Closed,
    Open,
}

/// Read-only challenge-page evidence.  The three independent regions must be reported
/// separately, and a missing attachment is represented explicitly as `Unavailable` rather than
/// by omitting this structure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChallengePageEvidence {
    pub challenge_section: EvidenceAvailability,
    pub my_submissions_section: EvidenceAvailability,
    pub leaderboard_section: EvidenceAvailability,
    pub share_route: Option<String>,
    pub share_route_status: EvidenceAvailability,
    pub attachment_status: EvidenceAvailability,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PenaltyBasis {
    pub object: String,
    pub reason: String,
    pub rewritten_score: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BundleRescoreStatus {
    NotApplicable,
    Pending,
    Completed,
    Failed,
}

impl ReconciliationFacts {
    fn validate(&self, observation: bool) -> Result<(), CoordinationError> {
        for (name, value) in [
            ("raw_score", self.raw_score),
            ("effective_score", self.effective_score),
            ("penalty", self.penalty),
        ] {
            if let Some(value) = value {
                if !value.is_finite() {
                    return Err(invalid(format!("{name} must be finite")));
                }
            }
        }
        if let Some(raw) = self.raw_score
            && !(0.0..=100.0).contains(&raw)
        {
            return Err(invalid("raw_score must be within 0..=100"));
        }
        if let Some(effective) = self.effective_score
            && !(-1.0..=100.0).contains(&effective)
        {
            return Err(invalid("effective_score must be within -1..=100"));
        }
        if let Some(penalty) = self.penalty
            && !(-1.0..=0.0).contains(&penalty)
        {
            return Err(invalid("penalty must be within -1..=0"));
        }
        if self.penalty_applied {
            let basis = self.penalty_basis.as_ref().ok_or_else(|| {
                invalid("an applied penalty must include its object, reason, and score rewrite")
            })?;
            let raw = self
                .raw_score
                .ok_or_else(|| invalid("an applied penalty requires the preserved raw score"))?;
            if self.penalty != Some(-1.0)
                || self.effective_score != Some(raw - 1.0)
                || basis.object.trim().is_empty()
                || basis.reason.trim().is_empty()
                || basis.rewritten_score != raw - 1.0
            {
                return Err(invalid(
                    "an applied penalty must subtract exactly one point from the raw score and preserve a complete basis",
                ));
            }
        } else if self.penalty.is_some_and(|penalty| penalty < 0.0) {
            return Err(invalid("negative penalty must be marked as applied"));
        }
        if !self.penalty_applied && self.penalty_basis.is_some() {
            return Err(invalid("penalty basis requires an applied penalty"));
        }
        if self
            .credited_owner
            .as_deref()
            .is_some_and(|owner| owner.trim().is_empty())
        {
            return Err(invalid("credited owner cannot be empty"));
        }
        if self
            .bundle_revision
            .as_deref()
            .is_some_and(|revision| revision.trim().is_empty())
        {
            return Err(invalid("bundle revision cannot be empty"));
        }
        if self.bundle_revision.is_some() && self.rescore_status.is_none() {
            return Err(invalid(
                "bundle revision requires an explicit rescore status",
            ));
        }
        if observation {
            require_present_if(
                self.raw_score.is_some() || self.effective_score.is_some(),
                self.score_evidence,
                "score",
            )?;
        }
        if observation {
            require_present_if(
                self.penalty.is_some() || self.penalty_applied || self.penalty_basis.is_some(),
                self.penalty_evidence,
                "penalty",
            )?;
        }
        if observation {
            require_present_if(
                self.credited_owner.is_some(),
                self.credited_owner_evidence,
                "credited owner",
            )?;
        }
        if observation {
            require_present_if(
                self.bundle_revision.is_some() || self.rescore_status.is_some(),
                self.bundle_evidence,
                "bundle",
            )?;
        }
        if self.credited_owner.is_some()
            && self.leaderboard_scope != Some(LeaderboardScope::UnifiedOverallAndSeason)
        {
            return Err(invalid(
                "credited owner requires the unified overall/season leaderboard scope",
            ));
        }
        if let Some(anti_cheat) = &self.anti_cheat {
            if anti_cheat.mode != AntiCheatMode::WeightedThreeSignals
                || anti_cheat.signals.len() != 3
            {
                return Err(invalid(
                    "anti-cheat evidence must use exactly three weighted signals",
                ));
            }
            let mut names = std::collections::BTreeSet::new();
            for signal in &anti_cheat.signals {
                if signal.name.trim().is_empty()
                    || !signal.weight.is_finite()
                    || signal.weight < 0.0
                    || !names.insert(signal.name.as_str())
                {
                    return Err(invalid(
                        "anti-cheat signals require unique names and finite non-negative weights",
                    ));
                }
            }
            if anti_cheat.signals.iter().all(|signal| signal.weight == 0.0) {
                return Err(invalid("anti-cheat signal weights cannot all be zero"));
            }
        }
        if self.anonymous_other_submission_access == Some(AnonymousSubmissionAccess::Open) {
            return Err(invalid(
                "anonymous access to other submissions must remain closed",
            ));
        }
        if let Some(page) = &self.challenge_page {
            let route_present = page.share_route_status == EvidenceAvailability::Present;
            if route_present
                != page
                    .share_route
                    .as_deref()
                    .is_some_and(|route| !route.trim().is_empty())
            {
                return Err(invalid(
                    "share route and its evidence availability must agree",
                ));
            }
        }
        if observation {
            if matches!(
                self.rescore_status,
                Some(BundleRescoreStatus::Pending | BundleRescoreStatus::Failed)
            ) {
                return Err(invalid(
                    "a bundle awaiting rescore cannot be a confirmed observation",
                ));
            }
            if matches!(
                self.trace_evidence,
                Some(EvidenceAvailability::Pending | EvidenceAvailability::Unavailable)
            ) {
                return Err(invalid(
                    "an observation without execution trace must remain unknown",
                ));
            }
        }
        Ok(())
    }

    fn merge_into(&self, target: &mut ReconciliationFacts) {
        if self.raw_score.is_some() {
            target.raw_score = self.raw_score;
        }
        if self.effective_score.is_some() {
            target.effective_score = self.effective_score;
        }
        if self.penalty.is_some() {
            target.penalty = self.penalty;
        }
        if self.penalty_applied {
            target.penalty_applied = true;
        }
        if self.penalty_basis.is_some() {
            target.penalty_basis = self.penalty_basis.clone();
        }
        if self.credited_owner.is_some() {
            target.credited_owner = self.credited_owner.clone();
        }
        if self.bundle_revision.is_some() {
            target.bundle_revision = self.bundle_revision.clone();
        }
        if self.rescore_status.is_some() {
            target.rescore_status = self.rescore_status;
        }
        if self.trace_evidence.is_some() {
            target.trace_evidence = self.trace_evidence;
        }
        if self.score_evidence.is_some() {
            target.score_evidence = self.score_evidence;
        }
        if self.penalty_evidence.is_some() {
            target.penalty_evidence = self.penalty_evidence;
        }
        if self.credited_owner_evidence.is_some() {
            target.credited_owner_evidence = self.credited_owner_evidence;
        }
        if self.bundle_evidence.is_some() {
            target.bundle_evidence = self.bundle_evidence;
        }
        if self.leaderboard_scope.is_some() {
            target.leaderboard_scope = self.leaderboard_scope;
        }
        if self.anti_cheat.is_some() {
            target.anti_cheat = self.anti_cheat.clone();
        }
        if self.anonymous_other_submission_access.is_some() {
            target.anonymous_other_submission_access = self.anonymous_other_submission_access;
        }
        if self.challenge_page.is_some() {
            target.challenge_page = self.challenge_page.clone();
        }
    }
}

fn require_present_if(
    has_value: bool,
    availability: Option<EvidenceAvailability>,
    name: &str,
) -> Result<(), CoordinationError> {
    if has_value && availability != Some(EvidenceAvailability::Present) {
        return Err(invalid(format!("{name} value requires present evidence")));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
#[serde(deny_unknown_fields)]
pub enum PlatformReconcileItemState {
    Observation { observation: PlatformObservation },
    UnknownNeedsReconcile { reason: String },
}

impl PlatformReconcileItem {
    pub fn observation(
        cursor: ReconciliationCursor,
        observation: PlatformObservation,
        facts: ReconciliationFacts,
    ) -> Self {
        Self {
            cursor,
            challenge_id: observation.challenge_id.clone(),
            attempt_id: observation.attempt_id.clone(),
            route: observation.route.clone(),
            observed_at_ms: observation.observed_at_ms,
            response_sha256: observation.response_sha256.clone(),
            facts,
            state: PlatformReconcileItemState::Observation { observation },
        }
    }

    pub fn unknown(
        cursor: ReconciliationCursor,
        challenge_id: impl Into<String>,
        attempt_id: impl Into<String>,
        route: impl Into<String>,
        observed_at_ms: i64,
        response_sha256: impl Into<String>,
        reason: impl Into<String>,
        facts: ReconciliationFacts,
    ) -> Self {
        Self {
            cursor,
            challenge_id: challenge_id.into(),
            attempt_id: attempt_id.into(),
            route: route.into(),
            observed_at_ms,
            response_sha256: response_sha256.into(),
            facts,
            state: PlatformReconcileItemState::UnknownNeedsReconcile {
                reason: reason.into(),
            },
        }
    }

    fn validate(
        &self,
        expected_stream: &str,
        expected_challenge: &str,
        now_ms: i64,
    ) -> Result<(), CoordinationError> {
        self.cursor.validate(expected_stream)?;
        if self.challenge_id.trim().is_empty()
            || self.challenge_id != expected_challenge
            || self.attempt_id.trim().is_empty()
            || self.route.trim().is_empty()
            || self.observed_at_ms < 0
            || self.observed_at_ms > now_ms
            || !is_sha256(&self.response_sha256)
        {
            return Err(invalid(
                "reconciliation item is not bound, non-future, or hash-addressed",
            ));
        }
        self.facts.validate(matches!(
            &self.state,
            PlatformReconcileItemState::Observation { .. }
        ))?;
        match &self.state {
            PlatformReconcileItemState::Observation { observation } => {
                if observation.attempt_id != self.attempt_id
                    || observation.challenge_id != self.challenge_id
                    || observation.route != self.route
                    || observation.observed_at_ms != self.observed_at_ms
                    || observation.response_sha256 != self.response_sha256
                {
                    return Err(invalid(
                        "reconciliation observation metadata disagrees with its envelope",
                    ));
                }
                observation.validate(expected_challenge, now_ms)?;
            }
            PlatformReconcileItemState::UnknownNeedsReconcile { reason } => {
                if reason.trim().is_empty() {
                    return Err(invalid("unknown reconciliation item requires a reason"));
                }
            }
        }
        Ok(())
    }

    fn dedup_key(&self) -> String {
        format!("{}:{}", self.attempt_id, self.response_sha256)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReconciledAttemptState {
    Confirmed,
    UnknownNeedsReconcile,
}

/// Latest fact for an attempt.  A later unknown item does not erase `last_confirmed_observation`;
/// this preserves the last-known fact while making the current state explicitly degraded.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReconciledAttempt {
    pub challenge_id: String,
    pub attempt_id: String,
    pub route: String,
    pub state: ReconciledAttemptState,
    pub last_cursor: ReconciliationCursor,
    pub last_response_sha256: String,
    pub last_observed_at_ms: i64,
    pub last_confirmed_observation: Option<PlatformObservation>,
    pub unknown_reason: Option<String>,
    pub facts: ReconciliationFacts,
}

/// Durable, serializable reducer state.  A ledger may persist this value beside its event log;
/// no network client or mutable global is required to replay it after restart.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlatformReconciliationSnapshot {
    pub schema_version: String,
    pub stream_id: String,
    pub challenge_id: String,
    pub cursor: Option<ReconciliationCursor>,
    pub attempts: BTreeMap<String, ReconciledAttempt>,
    /// `attempt_id:response_sha256 -> immutable item`.  Keeping the complete item makes exact
    /// replays idempotent while rejecting a caller that reuses a response hash/cursor with
    /// different facts.
    pub seen_items: BTreeMap<String, PlatformReconcileItem>,
}

impl PlatformReconciliationSnapshot {
    pub fn new(
        stream_id: impl Into<String>,
        challenge_id: impl Into<String>,
    ) -> Result<Self, CoordinationError> {
        let stream_id = stream_id.into();
        let challenge_id = challenge_id.into();
        if stream_id.trim().is_empty() || challenge_id.trim().is_empty() {
            return Err(invalid("reconciliation stream and challenge are required"));
        }
        Ok(Self {
            schema_version: RECONCILIATION_SCHEMA_VERSION.to_string(),
            stream_id,
            challenge_id,
            cursor: None,
            attempts: BTreeMap::new(),
            seen_items: BTreeMap::new(),
        })
    }

    pub fn validate(&self) -> Result<(), CoordinationError> {
        if self.schema_version != RECONCILIATION_SCHEMA_VERSION
            || self.stream_id.trim().is_empty()
            || self.challenge_id.trim().is_empty()
        {
            return Err(invalid("invalid reconciliation snapshot header"));
        }
        if let Some(cursor) = &self.cursor {
            cursor.validate(&self.stream_id)?;
        }
        for (attempt_id, fact) in &self.attempts {
            if attempt_id != &fact.attempt_id
                || fact.challenge_id != self.challenge_id
                || fact.attempt_id.trim().is_empty()
                || !is_sha256(&fact.last_response_sha256)
            {
                return Err(invalid(
                    "reconciliation snapshot contains an unbound attempt fact",
                ));
            }
            fact.last_cursor.validate(&self.stream_id)?;
            fact.facts
                .validate(fact.state == ReconciledAttemptState::Confirmed)?;
            if let Some(cursor) = &self.cursor {
                if fact.last_cursor.position > cursor.position {
                    return Err(invalid("attempt cursor is ahead of snapshot cursor"));
                }
            }
            match fact.state {
                ReconciledAttemptState::Confirmed => {
                    if fact.last_confirmed_observation.is_none() || fact.unknown_reason.is_some() {
                        return Err(invalid("confirmed attempt fact has inconsistent evidence"));
                    }
                }
                ReconciledAttemptState::UnknownNeedsReconcile => {
                    if fact.unknown_reason.as_deref().is_none_or(str::is_empty) {
                        return Err(invalid("unknown attempt fact has no reason"));
                    }
                }
            }
        }
        for (key, item) in &self.seen_items {
            if key != &item.dedup_key() {
                return Err(invalid("reconciliation dedup key is malformed"));
            }
            item.validate(&self.stream_id, &self.challenge_id, i64::MAX)?;
            if let Some(latest) = &self.cursor {
                if item.cursor.position > latest.position {
                    return Err(invalid("dedup cursor is ahead of snapshot cursor"));
                }
            }
        }
        Ok(())
    }

    /// Apply one item.  `Duplicate` is a successful no-op; `Stale` is also a no-op and tells the
    /// caller to advance its remote page without mutating local truth.  Cursor collisions and
    /// malformed/tampered duplicates fail closed.
    pub fn apply(
        &mut self,
        item: PlatformReconcileItem,
        now_ms: i64,
    ) -> Result<ReconciliationApplyResult, CoordinationError> {
        self.validate()?;
        item.validate(&self.stream_id, &self.challenge_id, now_ms)?;
        let key = item.dedup_key();
        if let Some(previous_item) = self.seen_items.get(&key) {
            if previous_item == &item {
                return Ok(ReconciliationApplyResult::Duplicate);
            }
            return Err(invalid(
                "same attempt/response was replayed with conflicting cursor or facts",
            ));
        }
        if let Some(current) = &self.cursor {
            if item.cursor.position < current.position {
                return Ok(ReconciliationApplyResult::Stale);
            }
            if item.cursor.position == current.position {
                return Err(invalid(
                    "different reconciliation items share one cursor position",
                ));
            }
        }

        // Work on a clone so a cross-item invariant failure cannot partially mutate the caller's
        // durable snapshot.
        let mut next = self.clone();
        let (state, observation, unknown_reason) = match item.state.clone() {
            PlatformReconcileItemState::Observation { observation } => {
                (ReconciledAttemptState::Confirmed, Some(observation), None)
            }
            PlatformReconcileItemState::UnknownNeedsReconcile { reason } => (
                ReconciledAttemptState::UnknownNeedsReconcile,
                None,
                Some(reason),
            ),
        };
        let fact = next
            .attempts
            .entry(item.attempt_id.clone())
            .or_insert_with(|| ReconciledAttempt {
                challenge_id: item.challenge_id.clone(),
                attempt_id: item.attempt_id.clone(),
                route: item.route.clone(),
                state,
                last_cursor: item.cursor.clone(),
                last_response_sha256: item.response_sha256.clone(),
                last_observed_at_ms: item.observed_at_ms,
                last_confirmed_observation: None,
                unknown_reason: None,
                facts: item.facts.clone(),
            });
        if fact.challenge_id != item.challenge_id || fact.route != item.route {
            return Err(invalid(
                "attempt identity is bound to conflicting challenge or route",
            ));
        }
        if item.observed_at_ms < fact.last_observed_at_ms {
            return Err(invalid("new cursor carries an older attempt observation"));
        }
        fact.state = state;
        fact.last_cursor = item.cursor.clone();
        fact.last_response_sha256 = item.response_sha256.clone();
        fact.last_observed_at_ms = item.observed_at_ms;
        fact.unknown_reason = unknown_reason;
        item.facts.merge_into(&mut fact.facts);
        if let Some(observation) = observation {
            fact.last_confirmed_observation = Some(observation);
        }
        next.cursor = Some(item.cursor.clone());
        next.seen_items.insert(key, item);
        next.validate()?;
        *self = next;
        Ok(ReconciliationApplyResult::Applied)
    }

    pub fn unknown_needs_reconcile(&self) -> impl Iterator<Item = &ReconciledAttempt> {
        self.attempts
            .values()
            .filter(|fact| fact.state == ReconciledAttemptState::UnknownNeedsReconcile)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReconciliationApplyResult {
    Applied,
    Duplicate,
    Stale,
}

/// Local submission vocabulary used before a read-only platform response has produced a typed
/// item.  Only rows with a real attempt id can be counted as platform in-flight.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalSubmissionState {
    Submitted,
    PendingParse,
    Scored,
    Backfilled,
    Stuck,
    Failed,
    UnknownNeedsReconcile,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalSubmissionRecord {
    pub submission_id: String,
    pub attempt_id: Option<String>,
    pub state: LocalSubmissionState,
    pub created_at_ms: i64,
    pub quota_reserved: bool,
    pub reconciled_from: Option<LocalSubmissionState>,
    pub reconciled_at_ms: Option<i64>,
    pub reconcile_reason: Option<String>,
}

impl LocalSubmissionRecord {
    pub fn counts_as_in_flight(&self) -> bool {
        matches!(
            self.state,
            LocalSubmissionState::Submitted | LocalSubmissionState::PendingParse
        ) && self
            .attempt_id
            .as_deref()
            .is_some_and(|id| !id.trim().is_empty())
    }

    pub fn needs_reconcile(&self) -> bool {
        self.state == LocalSubmissionState::UnknownNeedsReconcile
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalReconciliationDelta {
    pub moved_submission_ids: Vec<String>,
    pub released_reservation_ids: Vec<String>,
}

/// Move old, id-less transport/parse records out of the in-flight set.  This function does not
/// change used quota and does not infer whether the platform accepted anything; callers receive
/// explicit reservation-release intents to apply in their own atomic ledger transaction.
pub fn reconcile_ambiguous_local_submissions(
    records: &mut BTreeMap<String, LocalSubmissionRecord>,
    now_ms: i64,
    grace_ms: i64,
) -> Result<LocalReconciliationDelta, CoordinationError> {
    if now_ms < 0 || grace_ms < 0 {
        return Err(invalid(
            "reconciliation time and grace must be non-negative",
        ));
    }
    for (key, record) in records.iter() {
        if key != &record.submission_id
            || record.submission_id.trim().is_empty()
            || record.created_at_ms < 0
            || record.created_at_ms > now_ms
            || record
                .attempt_id
                .as_deref()
                .is_some_and(|id| id.trim().is_empty())
        {
            return Err(invalid(
                "local submission record is malformed or future-dated",
            ));
        }
    }
    let mut next = records.clone();
    let mut moved_submission_ids = Vec::new();
    let mut released_reservation_ids = Vec::new();
    for record in next.values_mut() {
        if record.state == LocalSubmissionState::UnknownNeedsReconcile && record.quota_reserved {
            record.quota_reserved = false;
            released_reservation_ids.push(record.submission_id.clone());
            continue;
        }
        let ambiguous = matches!(
            record.state,
            LocalSubmissionState::Submitted | LocalSubmissionState::PendingParse
        ) && record.attempt_id.is_none();
        if !ambiguous || now_ms - record.created_at_ms < grace_ms {
            continue;
        }
        let previous = record.state;
        record.state = LocalSubmissionState::UnknownNeedsReconcile;
        record.reconciled_from = Some(previous);
        record.reconciled_at_ms = Some(now_ms);
        record.reconcile_reason = Some(
            "local transport produced no platform attempt id; platform outcome remains unknown"
                .to_string(),
        );
        moved_submission_ids.push(record.submission_id.clone());
        if record.quota_reserved {
            record.quota_reserved = false;
            released_reservation_ids.push(record.submission_id.clone());
        }
    }
    *records = next;
    Ok(LocalReconciliationDelta {
        moved_submission_ids,
        released_reservation_ids,
    })
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value.bytes().all(|byte| byte.is_ascii_hexdigit())
        && value == value.to_ascii_lowercase()
}

fn invalid(message: impl Into<String>) -> CoordinationError {
    CoordinationError::InvalidReport(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observation(cursor: u64, attempt: &str, hash_char: char) -> PlatformReconcileItem {
        let hash = hash_char.to_string().repeat(64);
        let observation = PlatformObservation {
            attempt_id: attempt.to_string(),
            challenge_id: "challenge-a".to_string(),
            route: "/api/attempts/a".to_string(),
            observed_at_ms: cursor as i64,
            response_sha256: hash.clone(),
            replay_status: EvidenceAvailability::Present,
            results_status: EvidenceAvailability::Redacted,
            scorecard_status: EvidenceAvailability::Present,
            leaderboard_status: EvidenceAvailability::Present,
            harbor_reward: Some(0.5),
            trace_score: Some(80.0),
        };
        PlatformReconcileItem {
            cursor: ReconciliationCursor {
                stream_id: "challenge-a/attempts".to_string(),
                position: cursor,
            },
            challenge_id: "challenge-a".to_string(),
            attempt_id: attempt.to_string(),
            route: "/api/attempts/a".to_string(),
            observed_at_ms: cursor as i64,
            response_sha256: hash,
            facts: ReconciliationFacts::default(),
            state: PlatformReconcileItemState::Observation { observation },
        }
    }

    #[test]
    fn cursor_and_duplicate_are_idempotent() {
        let mut snapshot =
            PlatformReconciliationSnapshot::new("challenge-a/attempts", "challenge-a").unwrap();
        let item = observation(1, "a1", 'a');
        assert_eq!(
            snapshot.apply(item.clone(), 10).unwrap(),
            ReconciliationApplyResult::Applied
        );
        assert_eq!(
            snapshot.apply(item, 10).unwrap(),
            ReconciliationApplyResult::Duplicate
        );
        assert_eq!(snapshot.cursor.unwrap().position, 1);
        assert_eq!(snapshot.attempts.len(), 1);
    }

    #[test]
    fn stale_page_does_not_roll_back_latest_fact() {
        let mut snapshot =
            PlatformReconciliationSnapshot::new("challenge-a/attempts", "challenge-a").unwrap();
        snapshot.apply(observation(2, "a2", 'b'), 10).unwrap();
        assert_eq!(
            snapshot.apply(observation(1, "a1", 'a'), 10).unwrap(),
            ReconciliationApplyResult::Stale
        );
        assert_eq!(snapshot.cursor.unwrap().position, 2);
        assert!(!snapshot.attempts.contains_key("a1"));
    }

    #[test]
    fn unknown_is_explicit_and_last_confirmed_fact_is_preserved() {
        let mut snapshot =
            PlatformReconciliationSnapshot::new("challenge-a/attempts", "challenge-a").unwrap();
        snapshot.apply(observation(1, "a1", 'a'), 10).unwrap();
        let unknown = PlatformReconcileItem {
            cursor: ReconciliationCursor {
                stream_id: "challenge-a/attempts".to_string(),
                position: 2,
            },
            challenge_id: "challenge-a".to_string(),
            attempt_id: "a1".to_string(),
            route: "/api/attempts/a".to_string(),
            observed_at_ms: 2,
            response_sha256: "b".repeat(64),
            facts: ReconciliationFacts::default(),
            state: PlatformReconcileItemState::UnknownNeedsReconcile {
                reason: "platform response incomplete".to_string(),
            },
        };
        snapshot.apply(unknown, 10).unwrap();
        let fact = snapshot.attempts.get("a1").unwrap();
        assert_eq!(fact.state, ReconciledAttemptState::UnknownNeedsReconcile);
        assert!(fact.last_confirmed_observation.is_some());
        assert_eq!(snapshot.unknown_needs_reconcile().count(), 1);
    }

    #[test]
    fn conflicting_cursor_and_metadata_fail_closed() {
        let mut snapshot =
            PlatformReconciliationSnapshot::new("challenge-a/attempts", "challenge-a").unwrap();
        snapshot.apply(observation(1, "a1", 'a'), 10).unwrap();
        let mut conflicting = observation(2, "a1", 'b');
        if let PlatformReconcileItemState::Observation { observation } = &mut conflicting.state {
            observation.route = "/different".to_string();
        }
        assert!(snapshot.apply(conflicting, 10).is_err());
        let same_position = observation(1, "a2", 'c');
        assert!(snapshot.apply(same_position, 10).is_err());
    }

    #[test]
    fn malformed_snapshot_is_rejected_before_mutation() {
        let mut snapshot =
            PlatformReconciliationSnapshot::new("challenge-a/attempts", "challenge-a").unwrap();
        snapshot.schema_version = crate::SCHEMA_VERSION.to_string();
        assert!(snapshot.apply(observation(1, "a1", 'a'), 10).is_err());
    }

    #[test]
    fn applied_penalty_preserves_raw_score_and_subtracts_one_point() {
        let mut snapshot =
            PlatformReconciliationSnapshot::new("challenge-a/attempts", "challenge-a").unwrap();
        let mut item = observation(1, "a1", 'a');
        item.facts = ReconciliationFacts {
            raw_score: Some(88.0),
            effective_score: Some(87.0),
            penalty: Some(-1.0),
            penalty_applied: true,
            penalty_basis: Some(PenaltyBasis {
                object: "trace".to_string(),
                reason: "weighted anti-cheat signals".to_string(),
                rewritten_score: 87.0,
            }),
            credited_owner: Some("owner-1".to_string()),
            leaderboard_scope: Some(LeaderboardScope::UnifiedOverallAndSeason),
            score_evidence: Some(EvidenceAvailability::Present),
            penalty_evidence: Some(EvidenceAvailability::Present),
            credited_owner_evidence: Some(EvidenceAvailability::Present),
            ..ReconciliationFacts::default()
        };
        snapshot.apply(item, 10).unwrap();
        let facts = &snapshot.attempts["a1"].facts;
        assert_eq!(facts.raw_score, Some(88.0));
        assert_eq!(facts.effective_score, Some(87.0));
        assert_eq!(facts.credited_owner.as_deref(), Some("owner-1"));

        let mut invalid = observation(2, "a2", 'b');
        invalid.facts.penalty = Some(-1000.0);
        invalid.facts.penalty_applied = true;
        invalid.facts.penalty_evidence = Some(EvidenceAvailability::Present);
        assert!(snapshot.apply(invalid, 10).is_err());
    }

    #[test]
    fn bundle_revision_pending_rescore_stays_unknown_and_keeps_prior_score() {
        let mut snapshot =
            PlatformReconciliationSnapshot::new("challenge-a/attempts", "challenge-a").unwrap();
        let mut scored = observation(1, "a1", 'a');
        scored.facts = ReconciliationFacts {
            raw_score: Some(75.0),
            effective_score: Some(75.0),
            score_evidence: Some(EvidenceAvailability::Present),
            bundle_revision: Some("bundle-v1".to_string()),
            rescore_status: Some(BundleRescoreStatus::Completed),
            bundle_evidence: Some(EvidenceAvailability::Present),
            trace_evidence: Some(EvidenceAvailability::Present),
            ..ReconciliationFacts::default()
        };
        snapshot.apply(scored, 10).unwrap();
        let mut pending = PlatformReconcileItem {
            cursor: ReconciliationCursor {
                stream_id: "challenge-a/attempts".to_string(),
                position: 2,
            },
            challenge_id: "challenge-a".to_string(),
            attempt_id: "a1".to_string(),
            route: "/api/attempts/a".to_string(),
            observed_at_ms: 2,
            response_sha256: "b".repeat(64),
            facts: ReconciliationFacts {
                bundle_revision: Some("bundle-v2".to_string()),
                rescore_status: Some(BundleRescoreStatus::Pending),
                bundle_evidence: Some(EvidenceAvailability::Present),
                trace_evidence: Some(EvidenceAvailability::Unavailable),
                ..ReconciliationFacts::default()
            },
            state: PlatformReconcileItemState::UnknownNeedsReconcile {
                reason: "bundle revision requires fresh scoring and trace evidence".to_string(),
            },
        };
        snapshot.apply(pending.clone(), 10).unwrap();
        let fact = &snapshot.attempts["a1"];
        assert_eq!(fact.state, ReconciledAttemptState::UnknownNeedsReconcile);
        assert_eq!(fact.facts.raw_score, Some(75.0));
        assert_eq!(fact.facts.bundle_revision.as_deref(), Some("bundle-v2"));
        assert_eq!(
            fact.facts.rescore_status,
            Some(BundleRescoreStatus::Pending)
        );

        if let PlatformReconcileItemState::UnknownNeedsReconcile { .. } = pending.state {
            pending.state = PlatformReconcileItemState::Observation {
                observation: match observation(2, "a1", 'b').state {
                    PlatformReconcileItemState::Observation { observation } => observation,
                    _ => unreachable!(),
                },
            };
        }
        assert!(
            PlatformReconciliationSnapshot::new("challenge-a/attempts", "challenge-a")
                .unwrap()
                .apply(pending, 10)
                .is_err()
        );
    }

    #[test]
    fn python_converter_manifest_deserializes_to_typed_items() {
        let hash = "a".repeat(64);
        let manifest = format!(
            r#"[
            {{
                "cursor": {{"stream_id": "challenge-a/attempts", "position": 1}},
                "challenge_id": "challenge-a",
                "attempt_id": "attempt-a",
                "route": "/api/attempts/attempt-a",
                "observed_at_ms": 10,
                "response_sha256": "{hash}",
                "facts": {{
                    "raw_score": 91.0,
                    "effective_score": 91.0,
                    "credited_owner": "agent-1",
                    "bundle_revision": "bundle-v1",
                    "rescore_status": "completed",
                    "trace_evidence": "present",
                    "score_evidence": "present",
                    "credited_owner_evidence": "present",
                    "bundle_evidence": "present",
                    "leaderboard_scope": "unified_overall_and_season",
                    "anti_cheat": {{
                        "mode": "weighted_three_signals",
                        "signals": [
                            {{"name": "execution_admission", "weight": 0.4, "availability": "present"}},
                            {{"name": "tool_event_pairing", "weight": 0.3, "availability": "present"}},
                            {{"name": "artifact_provenance", "weight": 0.3, "availability": "present"}}
                        ]
                    }},
                    "anonymous_other_submission_access": "closed",
                "challenge_page": {{
                    "challenge_section": "present",
                    "my_submissions_section": "present",
                    "leaderboard_section": "present",
                    "share_route": "/challenge/challenge-a",
                    "share_route_status": "present",
                    "attachment_status": "present"
                }}}},
                "state": {{
                    "kind": "observation",
                    "observation": {{
                        "attempt_id": "attempt-a",
                        "challenge_id": "challenge-a",
                        "route": "/api/attempts/attempt-a",
                        "observed_at_ms": 10,
                        "response_sha256": "{hash}",
                        "replay_status": "present",
                        "results_status": "present",
                        "scorecard_status": "present",
                        "leaderboard_status": "present",
                        "harbor_reward": 0.91,
                        "trace_score": 88.0
                    }}
                }}
            }},
            {{
                "cursor": {{"stream_id": "challenge-a/attempts", "position": 2}},
                "challenge_id": "challenge-a",
                "attempt_id": "attempt-b",
                "route": "/api/attempts/attempt-b",
                "observed_at_ms": 20,
                "response_sha256": "{hash}",
                "facts": {{}},
                "state": {{
                    "kind": "unknown_needs_reconcile",
                    "reason": "replay/results/scorecard/leaderboard evidence incomplete"
                }}
            }}
        ]"#
        );
        let items: Vec<PlatformReconcileItem> = serde_json::from_str(&manifest).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].attempt_id, "attempt-a");
        assert_eq!(
            items[0].facts.leaderboard_scope,
            Some(LeaderboardScope::UnifiedOverallAndSeason)
        );
        assert_eq!(
            items[0].facts.rescore_status,
            Some(BundleRescoreStatus::Completed)
        );
        let challenge_page = items[0]
            .facts
            .challenge_page
            .as_ref()
            .expect("python manifest should deserialize challenge page evidence");
        assert_eq!(
            challenge_page.challenge_section,
            EvidenceAvailability::Present
        );
        assert_eq!(
            challenge_page.share_route.as_deref(),
            Some("/challenge/challenge-a")
        );
        assert_eq!(
            challenge_page.share_route_status,
            EvidenceAvailability::Present
        );
        assert_eq!(
            items[1].state,
            PlatformReconcileItemState::UnknownNeedsReconcile {
                reason: "replay/results/scorecard/leaderboard evidence incomplete".to_string(),
            }
        );

        let mut snapshot =
            PlatformReconciliationSnapshot::new("challenge-a/attempts", "challenge-a").unwrap();
        snapshot.apply(items[0].clone(), 20).unwrap();
        let fact = &snapshot.attempts["attempt-a"];
        assert_eq!(fact.state, ReconciledAttemptState::Confirmed);
        assert!(fact.last_confirmed_observation.is_some());
    }

    #[test]
    fn idless_old_local_row_is_not_in_flight_and_releases_only_reservation() {
        let mut records = BTreeMap::from([
            (
                "old".to_string(),
                LocalSubmissionRecord {
                    submission_id: "old".to_string(),
                    attempt_id: None,
                    state: LocalSubmissionState::PendingParse,
                    created_at_ms: 0,
                    quota_reserved: true,
                    reconciled_from: None,
                    reconciled_at_ms: None,
                    reconcile_reason: None,
                },
            ),
            (
                "real".to_string(),
                LocalSubmissionRecord {
                    submission_id: "real".to_string(),
                    attempt_id: Some("a1".to_string()),
                    state: LocalSubmissionState::PendingParse,
                    created_at_ms: 0,
                    quota_reserved: true,
                    reconciled_from: None,
                    reconciled_at_ms: None,
                    reconcile_reason: None,
                },
            ),
        ]);
        let delta = reconcile_ambiguous_local_submissions(&mut records, 10, 5).unwrap();
        assert_eq!(delta.moved_submission_ids, vec!["old"]);
        assert_eq!(delta.released_reservation_ids, vec!["old"]);
        assert!(records["old"].needs_reconcile());
        assert!(!records["old"].counts_as_in_flight());
        assert!(records["real"].counts_as_in_flight());
        assert!(
            reconcile_ambiguous_local_submissions(&mut records, 10, 5)
                .unwrap()
                .moved_submission_ids
                .is_empty()
        );
    }
}
