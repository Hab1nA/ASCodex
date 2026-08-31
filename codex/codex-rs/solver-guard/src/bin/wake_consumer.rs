use codex_solver_guard::{CoordinationEventRecord, Ledger};
use serde_json::json;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const USAGE: &str = "usage: ascodex-wake-consumer consume --wakes-dir <absolute-path> --ledger <absolute-path> --campaign-id <id> --now-ms <ms>";

/// Bounded, auditable Chief wake consumption: read every `chief-*.json` file in the wakes
/// directory, parse it, and record + ack it in the ledger (idempotent). A malformed or unbound
/// file fails the run immediately (fail-stop) so a bad wake cannot be skipped silently. This is
/// a single bounded pass, not a resident supervisor.
#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("ascodex-wake-consumer: {error}");
        std::process::exit(2);
    }
}

async fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let command = args.next().ok_or(USAGE)?;
    if command != "consume" {
        return Err(USAGE.to_string());
    }
    let options = parse_options(args)?;
    let wakes_dir = absolute_path(required(&options, "--wakes-dir")?)?;
    let ledger_path = absolute_path(required(&options, "--ledger")?)?;
    let campaign_id = required(&options, "--campaign-id")?.to_string();
    let now_ms = options
        .get("--now-ms")
        .map(String::as_str)
        .map(|value| {
            value
                .parse::<i64>()
                .map_err(|_| "invalid --now-ms".to_string())
        })
        .transpose()?
        .unwrap_or(trusted_now_ms()?);
    let ledger = Ledger::connect_file(&ledger_path)
        .await
        .map_err(|error| error.to_string())?;

    let entries = fs::read_dir(&wakes_dir)
        .map_err(|error| format!("cannot read wakes directory: {error}"))?;
    let mut consumed = 0usize;
    let mut files = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| format!("cannot read wakes entry: {error}"))?;
        if entry.file_name().to_string_lossy().starts_with("chief-")
            && entry
                .path()
                .extension()
                .map(|ext| ext == "json")
                .unwrap_or(false)
        {
            files.push(entry.path());
        }
    }
    files.sort();
    for path in files {
        let wake_json = fs::read_to_string(&path)
            .map_err(|error| format!("cannot read wake file {}: {error}", path.display()))?;
        let wake: serde_json::Value = serde_json::from_str(&wake_json)
            .map_err(|error| format!("wake file {} is invalid JSON: {error}", path.display()))?;
        let wake_id = wake
            .get("wake_id")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| format!("wake file {} has no wake_id", path.display()))?;
        let event_id = format!("wake-consumer-{wake_id}-record");
        let idempotency_key = format!("wake-consumer-{wake_id}-record-key");
        let ack_event_id = format!("wake-consumer-{wake_id}-ack");
        let ack_key = format!("wake-consumer-{wake_id}-ack-key");
        let record_payload = json!({ "wake_id": wake_id }).to_string();
        let events = vec![
            CoordinationEventRecord {
                event_id: &event_id,
                idempotency_key: &idempotency_key,
                aggregate_type: "chief_wake",
                aggregate_id: wake_id,
                expected_version: 0,
                event_type: "chief_wake_requested",
                payload_json: &record_payload,
                occurred_at_ms: now_ms,
            },
            CoordinationEventRecord {
                event_id: &ack_event_id,
                idempotency_key: &ack_key,
                aggregate_type: "chief_wake",
                aggregate_id: wake_id,
                expected_version: 1,
                event_type: "chief_wake_acked",
                payload_json: &record_payload,
                occurred_at_ms: now_ms,
            },
        ];
        ledger
            .consume_chief_wake_file(&wake_json, &campaign_id, now_ms, &events)
            .await
            .map_err(|error| format!("wake consumer failed on {}: {error}", path.display()))?;
        consumed += 1;
    }
    println!(
        "{}",
        json!({ "status": "consumed", "files": consumed, "campaign_id": campaign_id })
    );
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

fn trusted_now_ms() -> Result<i64, String> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| error.to_string())
        .and_then(|duration| {
            i64::try_from(duration.as_millis()).map_err(|_| "system time overflow".to_string())
        })
}
