use codex_ascodex_coordination::ActorContext;
use codex_solver_guard::{CoordinationEventRecord, Ledger};
use serde_json::json;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const USAGE: &str =
    "usage: ascodex-lease-admin <provision|revoke|inspect> --ledger <absolute-path> [options]";

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("ascodex-lease-admin: {error}");
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
        "provision" => {
            let context_path = absolute_path(required(&options, "--context")?)?;
            let bytes = fs::read(&context_path).map_err(|error| error.to_string())?;
            let context: ActorContext =
                serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
            let now_ms = trusted_now_ms()?;
            let event_id = required(&options, "--event-id")?;
            let idempotency_key = required(&options, "--idempotency-key")?;
            let expected_version = parse_version(required(&options, "--expected-version")?)?;
            let payload = json!({
                "lease_id": context.lease.lease_id,
                "campaign_id": context.campaign_id,
                "challenge_id": context.challenge_id,
                "role": role_name(context.role)
            })
            .to_string();
            let event = CoordinationEventRecord {
                event_id,
                idempotency_key,
                aggregate_type: "actor_lease",
                aggregate_id: &context.lease.lease_id,
                expected_version,
                event_type: "actor_lease_provisioned",
                payload_json: &payload,
                occurred_at_ms: now_ms,
            };
            let version = ledger
                .provision_actor_context_audited(&context, now_ms, &event)
                .await
                .map_err(|error| error.to_string())?;
            println!(
                "{}",
                json!({ "status": "provisioned", "lease_id": context.lease.lease_id, "version": version })
            );
        }
        "revoke" => {
            let lease_id = required(&options, "--lease-id")?;
            let now_ms = trusted_now_ms()?;
            let event_id = required(&options, "--event-id")?;
            let idempotency_key = required(&options, "--idempotency-key")?;
            let expected_version = parse_version(required(&options, "--expected-version")?)?;
            let payload = json!({ "lease_id": lease_id }).to_string();
            let event = CoordinationEventRecord {
                event_id,
                idempotency_key,
                aggregate_type: "actor_lease",
                aggregate_id: lease_id,
                expected_version,
                event_type: "actor_lease_revoked",
                payload_json: &payload,
                occurred_at_ms: now_ms,
            };
            let version = ledger
                .revoke_actor_lease_audited(lease_id, now_ms, &event)
                .await
                .map_err(|error| error.to_string())?;
            println!(
                "{}",
                json!({ "status": "revoked", "lease_id": lease_id, "version": version })
            );
        }
        "inspect" => {
            let lease_id = required(&options, "--lease-id")?;
            let metadata = ledger
                .inspect_actor_lease(lease_id)
                .await
                .map_err(|error| error.to_string())?;
            println!(
                "{}",
                serde_json::to_string(&metadata).map_err(|error| error.to_string())?
            );
        }
        _ => return Err(USAGE.to_string()),
    }
    Ok(())
}

fn role_name(role: codex_ascodex_coordination::Role) -> &'static str {
    match role {
        codex_ascodex_coordination::Role::Chief => "chief",
        codex_ascodex_coordination::Role::Solver => "solver",
        codex_ascodex_coordination::Role::Monitor => "monitor",
        codex_ascodex_coordination::Role::Intel => "intel",
        codex_ascodex_coordination::Role::JudgeAnalyst => "judge_analyst",
        codex_ascodex_coordination::Role::RedTeam => "red_team",
    }
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
