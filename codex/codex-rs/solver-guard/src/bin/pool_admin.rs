use codex_solver_guard::{CoordinationEventRecord, IdentityPoolEntry, Ledger};
use serde_json::json;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const USAGE: &str = "usage: ascodex-pool-admin <provision|freeze|thaw|remove|inspect|list> --ledger <absolute-path> [options]";

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("ascodex-pool-admin: {error}");
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
            let entry_path = absolute_path(required(&options, "--entry")?)?;
            let bytes = fs::read(&entry_path).map_err(|error| error.to_string())?;
            let entry: IdentityPoolEntry =
                serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
            let now_ms = trusted_now_ms()?;
            let key = entry_key(&entry);
            let payload = json!({ "identity_pool_entry": key }).to_string();
            let event = CoordinationEventRecord {
                event_id: required(&options, "--event-id")?,
                idempotency_key: required(&options, "--idempotency-key")?,
                aggregate_type: "identity_pool",
                aggregate_id: &key,
                expected_version: parse_version(required(&options, "--expected-version")?)?,
                event_type: "identity_pool_provisioned",
                payload_json: &payload,
                occurred_at_ms: now_ms,
            };
            let version = ledger
                .provision_identity_pool_entry_audited(&entry, now_ms, &event)
                .await
                .map_err(|error| error.to_string())?;
            println!(
                "{}",
                json!({ "status": "provisioned", "name": entry.name, "identity_class": entry.identity_class, "version": version })
            );
        }
        "freeze" | "thaw" => {
            let (name, challenge_id, owner, identity_class) = entry_selectors(&options)?;
            let now_ms = trusted_now_ms()?;
            let frozen = command == "freeze";
            let event_type = if frozen {
                "identity_pool_frozen"
            } else {
                "identity_pool_thawed"
            };
            let key = entry_key_parts(&name, &challenge_id, &owner, &identity_class);
            let payload = json!({ "identity_pool_entry": key }).to_string();
            let event = CoordinationEventRecord {
                event_id: required(&options, "--event-id")?,
                idempotency_key: required(&options, "--idempotency-key")?,
                aggregate_type: "identity_pool",
                aggregate_id: &key,
                expected_version: parse_version(required(&options, "--expected-version")?)?,
                event_type,
                payload_json: &payload,
                occurred_at_ms: now_ms,
            };
            let version = ledger
                .set_identity_pool_frozen_audited(
                    &name,
                    &challenge_id,
                    &owner,
                    &identity_class,
                    frozen,
                    now_ms,
                    &event,
                )
                .await
                .map_err(|error| error.to_string())?;
            println!(
                "{}",
                json!({ "status": if frozen { "frozen" } else { "thawed" }, "name": name, "identity_class": identity_class, "version": version })
            );
        }
        "remove" => {
            let (name, challenge_id, owner, identity_class) = entry_selectors(&options)?;
            let now_ms = trusted_now_ms()?;
            let key = entry_key_parts(&name, &challenge_id, &owner, &identity_class);
            let payload = json!({ "identity_pool_entry": key }).to_string();
            let event = CoordinationEventRecord {
                event_id: required(&options, "--event-id")?,
                idempotency_key: required(&options, "--idempotency-key")?,
                aggregate_type: "identity_pool",
                aggregate_id: &key,
                expected_version: parse_version(required(&options, "--expected-version")?)?,
                event_type: "identity_pool_removed",
                payload_json: &payload,
                occurred_at_ms: now_ms,
            };
            let version = ledger
                .remove_identity_pool_entry_audited(
                    &name,
                    &challenge_id,
                    &owner,
                    &identity_class,
                    now_ms,
                    &event,
                )
                .await
                .map_err(|error| error.to_string())?;
            println!(
                "{}",
                json!({ "status": "removed", "name": name, "identity_class": identity_class, "version": version })
            );
        }
        "inspect" => {
            let (name, challenge_id, owner, identity_class) = entry_selectors(&options)?;
            let entry = ledger
                .get_identity_pool_entry(&name, &challenge_id, &owner, &identity_class)
                .await
                .map_err(|error| error.to_string())?;
            println!(
                "{}",
                serde_json::to_string(&entry).map_err(|error| error.to_string())?
            );
        }
        "list" => {
            let entries = ledger
                .list_active_identity_pool_entries()
                .await
                .map_err(|error| error.to_string())?;
            println!(
                "{}",
                serde_json::to_string(&entries).map_err(|error| error.to_string())?
            );
        }
        _ => return Err(USAGE.to_string()),
    }
    Ok(())
}

fn entry_key(entry: &IdentityPoolEntry) -> String {
    entry_key_parts(
        &entry.name,
        &entry.challenge_id,
        &entry.owner,
        &entry.identity_class,
    )
}

fn entry_key_parts(name: &str, challenge_id: &str, owner: &str, identity_class: &str) -> String {
    format!("{name}\u{1f}{challenge_id}\u{1f}{owner}\u{1f}{identity_class}")
}

fn entry_selectors(
    options: &std::collections::BTreeMap<String, String>,
) -> Result<(String, String, String, String), String> {
    Ok((
        required(options, "--name")?.to_string(),
        required(options, "--challenge-id")?.to_string(),
        required(options, "--owner")?.to_string(),
        required(options, "--identity-class")?.to_string(),
    ))
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
