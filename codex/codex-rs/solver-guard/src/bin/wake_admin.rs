use codex_solver_guard::{CoordinationEventRecord, Ledger};
use serde_json::json;
use std::env;
use std::path::{Path, PathBuf};

const USAGE: &str =
    "usage: ascodex-wake-admin <inspect|ack|list-waiting> --ledger <absolute-path> [options]";

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("ascodex-wake-admin: {error}");
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
        "inspect" => {
            let wake_id = required(&options, "--wake-id")?;
            let wake = ledger
                .load_chief_wake(wake_id)
                .await
                .map_err(|error| error.to_string())?;
            println!(
                "{}",
                serde_json::to_string(&wake).map_err(|error| error.to_string())?
            );
        }
        "ack" => {
            let wake_id = required(&options, "--wake-id")?;
            let now_ms = trusted_now_ms()?;
            let payload = json!({ "wake_id": wake_id }).to_string();
            let event = CoordinationEventRecord {
                event_id: required(&options, "--event-id")?,
                idempotency_key: required(&options, "--idempotency-key")?,
                aggregate_type: "chief_wake",
                aggregate_id: wake_id,
                expected_version: parse_version(required(&options, "--expected-version")?)?,
                event_type: "chief_wake_acked",
                payload_json: &payload,
                occurred_at_ms: now_ms,
            };
            let version = ledger
                .ack_chief_wake_audited(wake_id, now_ms, &event)
                .await
                .map_err(|error| error.to_string())?;
            println!(
                "{}",
                json!({ "status": "acked", "wake_id": wake_id, "version": version })
            );
        }
        "list-waiting" => {
            let wakes = ledger
                .list_waiting_chief_wakes()
                .await
                .map_err(|error| error.to_string())?;
            println!(
                "{}",
                serde_json::to_string(&wakes).map_err(|error| error.to_string())?
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
