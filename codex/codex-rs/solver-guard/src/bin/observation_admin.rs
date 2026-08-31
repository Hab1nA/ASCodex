use codex_ascodex_coordination::{ActorContext, PlatformObservation, PlatformReconcileItem};
use codex_solver_guard::{CoordinationEventRecord, Ledger};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const USAGE: &str = "usage: ascodex-observation-admin <record|record-confirmation|inspect|reconcile|reconcile-batch|inspect-reconcile> --ledger <absolute-path> [options]";

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("ascodex-observation-admin: {error}");
        std::process::exit(2);
    }
}

async fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let command = args.next().ok_or(USAGE)?;
    let options = parse_options(args)?;
    let ledger_path = absolute_path(required(&options, "--ledger")?)?;
    let ledger = Ledger::connect_file(&ledger_path)
        .await
        .map_err(|error| error.to_string())?;
    match command.as_str() {
        "record" => {
            let monitor_path = absolute_path(required(&options, "--monitor-context")?)?;
            let observation_path = absolute_path(required(&options, "--observation")?)?;
            let monitor: ActorContext =
                serde_json::from_slice(&fs::read(monitor_path).map_err(|error| error.to_string())?)
                    .map_err(|error| error.to_string())?;
            let observation: PlatformObservation = serde_json::from_slice(
                &fs::read(observation_path).map_err(|error| error.to_string())?,
            )
            .map_err(|error| error.to_string())?;
            let payload = serde_json::to_string(&observation).map_err(|error| error.to_string())?;
            let now_ms = trusted_now_ms()?;
            let event = CoordinationEventRecord {
                event_id: required(&options, "--event-id")?,
                idempotency_key: required(&options, "--idempotency-key")?,
                aggregate_type: "campaign",
                aggregate_id: &monitor.campaign_id,
                expected_version: parse_version(required(&options, "--expected-version")?)?,
                event_type: "platform_observation_recorded",
                payload_json: &payload,
                occurred_at_ms: now_ms,
            };
            let persisted = ledger
                .record_platform_observation_audited(&monitor, &observation, now_ms, &event)
                .await
                .map_err(|error| error.to_string())?;
            println!(
                "{}",
                json!({
                    "status": "recorded",
                    "observation_id": persisted.observation_id,
                    "challenge_id": persisted.challenge_id,
                    "attempt_id": persisted.attempt_id,
                    "observation_sha256": persisted.observation_sha256,
                    "event_version": persisted.event_version,
                })
            );
        }
        "record-confirmation" => {
            let monitor_path = absolute_path(required(&options, "--monitor-context")?)?;
            let confirmation_path = absolute_path(required(&options, "--confirmation")?)?;
            let monitor: ActorContext =
                serde_json::from_slice(&fs::read(monitor_path).map_err(|error| error.to_string())?)
                    .map_err(|error| error.to_string())?;
            let confirmation: codex_solver_guard::LeaderboardConfirmation = serde_json::from_slice(
                &fs::read(confirmation_path).map_err(|error| error.to_string())?,
            )
            .map_err(|error| error.to_string())?;
            let payload =
                serde_json::to_string(&confirmation).map_err(|error| error.to_string())?;
            let now_ms = trusted_now_ms()?;
            let event = CoordinationEventRecord {
                event_id: required(&options, "--event-id")?,
                idempotency_key: required(&options, "--idempotency-key")?,
                aggregate_type: "leaderboard_confirmation",
                aggregate_id: &confirmation.confirmation_id,
                expected_version: parse_version(required(&options, "--expected-version")?)?,
                event_type: "leaderboard_confirmation_recorded",
                payload_json: &payload,
                occurred_at_ms: now_ms,
            };
            let version = ledger
                .record_leaderboard_confirmation_audited(&monitor, &confirmation, now_ms, &event)
                .await
                .map_err(|error| error.to_string())?;
            println!(
                "{}",
                json!({
                    "status": "recorded",
                    "confirmation_id": confirmation.confirmation_id,
                    "attempt_id": confirmation.attempt_id,
                    "owner": confirmation.owner,
                    "event_version": version,
                })
            );
        }
        "inspect" => {
            let challenge_id = required(&options, "--challenge-id")?;
            let attempt_id = required(&options, "--attempt-id")?;
            let persisted = ledger
                .load_latest_platform_observation(challenge_id, attempt_id)
                .await
                .map_err(|error| error.to_string())?;
            println!(
                "{}",
                json!({
                    "observation_id": persisted.observation_id,
                    "campaign_id": persisted.campaign_id,
                    "challenge_id": persisted.challenge_id,
                    "attempt_id": persisted.attempt_id,
                    "observation": persisted.observation,
                    "observation_sha256": persisted.observation_sha256,
                    "event_version": persisted.event_version,
                })
            );
        }
        "reconcile" => {
            let monitor_path = absolute_path(required(&options, "--monitor-context")?)?;
            let item_path = absolute_path(required(&options, "--item")?)?;
            let monitor: ActorContext =
                serde_json::from_slice(&fs::read(monitor_path).map_err(|error| error.to_string())?)
                    .map_err(|error| error.to_string())?;
            let item: PlatformReconcileItem =
                serde_json::from_slice(&fs::read(item_path).map_err(|error| error.to_string())?)
                    .map_err(|error| error.to_string())?;
            let payload = serde_json::to_string(&item).map_err(|error| error.to_string())?;
            let now_ms = trusted_now_ms()?;
            let event = CoordinationEventRecord {
                event_id: required(&options, "--event-id")?,
                idempotency_key: required(&options, "--idempotency-key")?,
                aggregate_type: "campaign",
                aggregate_id: &monitor.campaign_id,
                expected_version: parse_version(required(&options, "--expected-version")?)?,
                event_type: "platform_reconciliation_recorded",
                payload_json: &payload,
                occurred_at_ms: now_ms,
            };
            let outcome = ledger
                .apply_platform_reconciliation_audited(&monitor, &item, now_ms, &event)
                .await
                .map_err(|error| error.to_string())?;
            println!(
                "{}",
                json!({
                    "status": match outcome.result {
                        codex_ascodex_coordination::ReconciliationApplyResult::Applied => "applied",
                        codex_ascodex_coordination::ReconciliationApplyResult::Duplicate => "duplicate",
                        codex_ascodex_coordination::ReconciliationApplyResult::Stale => "stale",
                    },
                    "reconciliation_id": outcome.persisted.reconciliation_id,
                    "campaign_id": outcome.persisted.campaign_id,
                    "challenge_id": outcome.persisted.challenge_id,
                    "stream_id": outcome.persisted.stream_id,
                    "cursor_position": outcome
                        .persisted
                        .snapshot
                        .cursor
                        .as_ref()
                        .map(|cursor| cursor.position),
                    "snapshot_sha256": outcome.persisted.snapshot_sha256,
                    "event_version": outcome.persisted.event_version,
                })
            );
        }
        "reconcile-batch" => {
            let monitor_path = absolute_path(required(&options, "--monitor-context")?)?;
            let manifest_path = absolute_path(required(&options, "--manifest")?)?;
            let starting_version = parse_version(required(&options, "--starting-event-version")?)?;
            let monitor: ActorContext =
                serde_json::from_slice(&fs::read(monitor_path).map_err(|error| error.to_string())?)
                    .map_err(|error| error.to_string())?;
            let items: Vec<PlatformReconcileItem> = serde_json::from_slice(
                &fs::read(manifest_path).map_err(|error| error.to_string())?,
            )
            .map_err(|error| error.to_string())?;
            let now_ms = trusted_now_ms()?;
            if items.is_empty() {
                return Err("reconciliation manifest is empty".to_string());
            }
            let mut expected_version = starting_version;
            let mut summaries = Vec::with_capacity(items.len());
            for (index, item) in items.iter().enumerate() {
                let payload = serde_json::to_string(item).map_err(|error| error.to_string())?;
                let digest = format!("{:x}", Sha256::digest(payload.as_bytes()));
                let event_id = format!(
                    "reconcile-batch:{}:{}:{}:{}:{}",
                    monitor.campaign_id,
                    item.cursor.stream_id,
                    item.cursor.position,
                    item.attempt_id,
                    digest
                );
                let event = CoordinationEventRecord {
                    event_id: event_id.as_str(),
                    idempotency_key: event_id.as_str(),
                    aggregate_type: "campaign",
                    aggregate_id: &monitor.campaign_id,
                    expected_version,
                    event_type: "platform_reconciliation_recorded",
                    payload_json: &payload,
                    occurred_at_ms: now_ms,
                };
                let outcome = ledger
                    .apply_platform_reconciliation_audited(&monitor, item, now_ms, &event)
                    .await
                    .map_err(|error| error.to_string())?;
                let status = match outcome.result {
                    codex_ascodex_coordination::ReconciliationApplyResult::Applied => {
                        expected_version = outcome
                            .persisted
                            .event_version
                            .unwrap_or(expected_version)
                            .checked_add(1)
                            .ok_or_else(|| "event version overflow".to_string())?;
                        "applied"
                    }
                    codex_ascodex_coordination::ReconciliationApplyResult::Duplicate => "duplicate",
                    codex_ascodex_coordination::ReconciliationApplyResult::Stale => "stale",
                };
                summaries.push(json!({
                    "index": index,
                    "status": status,
                    "event_id": event_id,
                    "attempt_id": item.attempt_id,
                    "cursor_position": item.cursor.position,
                    "event_version": outcome.persisted.event_version,
                    "snapshot_sha256": outcome.persisted.snapshot_sha256,
                }));
            }
            println!(
                "{}",
                json!({
                    "status": "batch-complete",
                    "campaign_id": monitor.campaign_id,
                    "challenge_id": monitor.challenge_id,
                    "items": summaries,
                    "next_expected_version": expected_version,
                })
            );
        }
        "inspect-reconcile" => {
            let stream_id = required(&options, "--stream-id")?;
            let challenge_id = required(&options, "--challenge-id")?;
            let persisted = ledger
                .load_latest_platform_reconciliation(stream_id, challenge_id)
                .await
                .map_err(|error| error.to_string())?;
            println!(
                "{}",
                json!({
                    "reconciliation_id": persisted.reconciliation_id,
                    "campaign_id": persisted.campaign_id,
                    "challenge_id": persisted.challenge_id,
                    "stream_id": persisted.stream_id,
                    "snapshot": persisted.snapshot,
                    "snapshot_sha256": persisted.snapshot_sha256,
                    "event_version": persisted.event_version,
                })
            );
        }
        _ => return Err(USAGE.to_string()),
    }
    Ok(())
}

fn parse_options(
    args: impl Iterator<Item = String>,
) -> Result<std::collections::BTreeMap<String, String>, String> {
    let args = args.collect::<Vec<_>>();
    if args.len() % 2 != 0 {
        return Err(USAGE.to_string());
    }
    let mut options = std::collections::BTreeMap::new();
    for pair in args.chunks_exact(2) {
        if !pair[0].starts_with("--") || options.insert(pair[0].clone(), pair[1].clone()).is_some()
        {
            return Err("options must be unique --name value pairs".to_string());
        }
    }
    Ok(options)
}

fn required<'a>(
    options: &'a std::collections::BTreeMap<String, String>,
    name: &str,
) -> Result<&'a str, String> {
    options
        .get(name)
        .map(String::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("missing required option {name}"))
}

fn absolute_path(value: &str) -> Result<PathBuf, String> {
    let path = Path::new(value);
    if !path.is_absolute() {
        return Err("administrative file paths must be absolute".to_string());
    }
    Ok(path.to_path_buf())
}

fn parse_version(value: &str) -> Result<u64, String> {
    value
        .parse()
        .map_err(|_| "invalid expected version".to_string())
}

fn trusted_now_ms() -> Result<i64, String> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| error.to_string())
        .and_then(|duration| {
            i64::try_from(duration.as_millis()).map_err(|_| "system time overflow".to_string())
        })
}
