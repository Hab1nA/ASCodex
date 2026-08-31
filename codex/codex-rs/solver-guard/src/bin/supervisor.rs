use codex_solver_guard::{CoordinationEventRecord, Ledger, collect_wake_files, next_backoff_ms};
use serde_json::json;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

const USAGE: &str = "usage: ascodex-supervisor supervise --wakes-dir <absolute-path> --ledger <absolute-path> --campaign-id <id> --interval-ms <ms> [--base-backoff-ms <ms>] [--max-backoff-ms <ms>] [--max-cycles <n>]";

/// Resident supervisor: periodically polls the wakes directory, consumes any new Chief wake
/// files into the ledger (idempotent), and backs off exponentially on failure so a broken wake
/// or ledger does not spin the loop. `--max-cycles` bounds the loop for tests/CI; without it
/// the supervisor runs until interrupted.
#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("ascodex-supervisor: {error}");
        std::process::exit(2);
    }
}

async fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let command = args.next().ok_or(USAGE)?;
    if command != "supervise" {
        return Err(USAGE.to_string());
    }
    let options = parse_options(args)?;
    let wakes_dir = absolute_path(required(&options, "--wakes-dir")?)?;
    let ledger_path = absolute_path(required(&options, "--ledger")?)?;
    let campaign_id = required(&options, "--campaign-id")?.to_string();
    let interval_ms = parse_u64(required(&options, "--interval-ms")?)?;
    let base_backoff_ms = options
        .get("--base-backoff-ms")
        .map(|value| parse_u64(value))
        .transpose()?
        .unwrap_or(1_000);
    let max_backoff_ms = options
        .get("--max-backoff-ms")
        .map(|value| parse_u64(value))
        .transpose()?
        .unwrap_or(60_000);
    let max_cycles = options
        .get("--max-cycles")
        .map(|value| {
            value
                .parse::<u64>()
                .map_err(|_| "invalid --max-cycles".to_string())
        })
        .transpose()?
        .unwrap_or(u64::MAX);
    if interval_ms == 0 {
        return Err("interval must be positive".to_string());
    }

    let ledger = Ledger::connect_file(&ledger_path)
        .await
        .map_err(|error| error.to_string())?;
    let mut cycle = 0u64;
    let mut consecutive_failures = 0u32;
    loop {
        cycle += 1;
        if cycle > max_cycles {
            break;
        }
        let cycle_result = consume_once(&ledger, &wakes_dir, &campaign_id).await;
        match cycle_result {
            Ok(consumed) => {
                consecutive_failures = 0;
                println!(
                    "{}",
                    json!({ "cycle": cycle, "status": "consumed", "files": consumed })
                );
            }
            Err(error) => {
                consecutive_failures = consecutive_failures.saturating_add(1);
                let backoff =
                    next_backoff_ms(consecutive_failures - 1, base_backoff_ms, max_backoff_ms)
                        .unwrap_or(max_backoff_ms);
                eprintln!(
                    "ascodex-supervisor: cycle {cycle} failed: {error}; backing off {backoff}ms"
                );
                tokio::time::sleep(Duration::from_millis(backoff)).await;
                continue;
            }
        }
        tokio::time::sleep(Duration::from_millis(interval_ms)).await;
    }
    println!("{}", json!({ "status": "stopped", "cycles": cycle - 1 }));
    Ok(())
}

/// One poll cycle: collect wake files and consume each into the ledger. Any failure aborts the
/// cycle (fail-stop) so a broken wake is not skipped silently; the supervisor backs off.
async fn consume_once(
    ledger: &Ledger,
    wakes_dir: &Path,
    campaign_id: &str,
) -> Result<usize, String> {
    let files = collect_wake_files(wakes_dir)?;
    let mut consumed = 0usize;
    for path in files {
        let wake_json = fs::read_to_string(&path)
            .map_err(|error| format!("cannot read wake file {}: {error}", path.display()))?;
        let wake: serde_json::Value = serde_json::from_str(&wake_json)
            .map_err(|error| format!("wake file {} is invalid JSON: {error}", path.display()))?;
        let wake_id = wake
            .get("wake_id")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| format!("wake file {} has no wake_id", path.display()))?;
        let now_ms = trusted_now_ms()?;
        let event_id = format!("wake-supervisor-{wake_id}-record");
        let idempotency_key = format!("wake-supervisor-{wake_id}-record-key");
        let ack_event_id = format!("wake-supervisor-{wake_id}-ack");
        let ack_key = format!("wake-supervisor-{wake_id}-ack-key");
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
            .consume_chief_wake_file(&wake_json, campaign_id, now_ms, &events)
            .await
            .map_err(|error| format!("wake consumer failed on {}: {error}", path.display()))?;
        consumed += 1;
    }
    Ok(consumed)
}

fn parse_u64(value: &str) -> Result<u64, String> {
    value
        .parse()
        .map_err(|_| format!("invalid number: {value}"))
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
