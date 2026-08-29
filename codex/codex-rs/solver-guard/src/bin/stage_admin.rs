use codex_ascodex_coordination::{ActorContext, ResearchCycleRecord};
use codex_solver_guard::{CoordinationEventRecord, Ledger};
use serde_json::json;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const USAGE: &str = "usage: ascodex-stage-admin <issue|supersede|revoke> --ledger <absolute-path> --chief-context <absolute-json> [command options] --event-id <id> --idempotency-key <key>";

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("ascodex-stage-admin: {error}");
        std::process::exit(2);
    }
}

async fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let command = args.next().ok_or(USAGE)?;
    let options = parse_options(args)?;
    let ledger_path = absolute_path(required(&options, "--ledger")?)?;
    let chief_path = absolute_path(required(&options, "--chief-context")?)?;
    let chief: ActorContext =
        serde_json::from_slice(&fs::read(chief_path).map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
    let now_ms = trusted_now_ms()?;
    let ledger = Ledger::connect_file(&ledger_path)
        .await
        .map_err(|error| error.to_string())?;
    match command.as_str() {
        "issue" | "supersede" => {
            let cycle_path = absolute_path(required(&options, "--cycle")?)?;
            let workspace = absolute_path(required(&options, "--workspace")?)?;
            let capability_map = required(&options, "--capability-map")?;
            let cycle: ResearchCycleRecord =
                serde_json::from_slice(&fs::read(cycle_path).map_err(|error| error.to_string())?)
                    .map_err(|error| error.to_string())?;
            let predecessor = if command == "supersede" {
                Some(required(&options, "--predecessor-cycle-id")?)
            } else {
                None
            };
            let payload = if let Some(predecessor) = predecessor {
                json!({ "predecessor_cycle_id": predecessor, "cycle": cycle }).to_string()
            } else {
                serde_json::to_string(&cycle).map_err(|error| error.to_string())?
            };
            let event = CoordinationEventRecord {
                event_id: required(&options, "--event-id")?,
                idempotency_key: required(&options, "--idempotency-key")?,
                aggregate_type: "campaign",
                aggregate_id: &cycle.campaign_id,
                expected_version: cycle.expected_state_version,
                event_type: if predecessor.is_some() {
                    "research_cycle_superseded"
                } else {
                    "research_cycle_issued"
                },
                payload_json: &payload,
                occurred_at_ms: now_ms,
            };
            let issuance = if let Some(predecessor) = predecessor {
                ledger
                    .supersede_research_cycle_audited(
                        &chief,
                        predecessor,
                        &cycle,
                        &workspace,
                        capability_map,
                        now_ms,
                        &event,
                    )
                    .await
            } else {
                ledger
                    .issue_research_cycle_audited(
                        &chief,
                        &cycle,
                        &workspace,
                        capability_map,
                        now_ms,
                        &event,
                    )
                    .await
            }
            .map_err(|error| error.to_string())?;
            println!(
                "{}",
                json!({
                    "status": if predecessor.is_some() { "superseded" } else { "issued" },
                    "cycle_id": issuance.cycle_id,
                    "brief_ids": issuance.brief_ids,
                    "cycle_event_version": issuance.cycle_event_version,
                    "cycle_sha256": issuance.cycle_sha256,
                })
            );
        }
        "revoke" => {
            let cycle_id = required(&options, "--cycle-id")?;
            let expected_version = parse_version(required(&options, "--expected-version")?)?;
            let payload = json!({ "cycle_id": cycle_id }).to_string();
            let event = CoordinationEventRecord {
                event_id: required(&options, "--event-id")?,
                idempotency_key: required(&options, "--idempotency-key")?,
                aggregate_type: "campaign",
                aggregate_id: &chief.campaign_id,
                expected_version,
                event_type: "research_cycle_revoked",
                payload_json: &payload,
                occurred_at_ms: now_ms,
            };
            let version = ledger
                .revoke_research_cycle_audited(&chief, cycle_id, now_ms, &event)
                .await
                .map_err(|error| error.to_string())?;
            println!(
                "{}",
                json!({ "status": "revoked", "cycle_id": cycle_id, "version": version })
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
