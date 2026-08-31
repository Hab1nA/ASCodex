use codex_ascodex_coordination::SCHEMA_VERSION;
use codex_solver_guard::{CoordinationEventRecord, Ledger, build_recovery_canary_trace};
use serde_json::json;
use std::env;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const USAGE: &str = "usage: ascodex-canary-runner run --ledger <absolute-path> --recovery-id <id> [--runtime-instance-id <id>] [--recovery-attempt <n>] [--now-ms <ms>]";

static RUNNER_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Automatic recovery canary runner: generates a fresh runtime instance id (or accepts an
/// explicit one), builds the two-turn isolated probe trace, and records it in the ledger.
/// This is the controlled external runner that replaces a manual/administrative canary write;
/// it never starts, resumes, or injects into a Chief or solver process.
#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("ascodex-canary-runner: {error}");
        std::process::exit(2);
    }
}

async fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let command = args.next().ok_or(USAGE)?;
    if command != "run" {
        return Err(USAGE.to_string());
    }
    let options = parse_options(args)?;
    let ledger_path = absolute_path(required(&options, "--ledger")?)?;
    let recovery_id = required(&options, "--recovery-id")?.to_string();
    if recovery_id.trim().is_empty() {
        return Err("recovery id is required".to_string());
    }
    let runtime_instance_id = match options.get("--runtime-instance-id") {
        Some(value) if !value.trim().is_empty() => value.clone(),
        _ => {
            let counter = RUNNER_COUNTER.fetch_add(1, Ordering::Relaxed);
            let pid = std::process::id();
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_millis())
                .unwrap_or(0);
            format!("runtime-{pid}-{now}-{counter}")
        }
    };
    let recovery_attempt = options
        .get("--recovery-attempt")
        .map(|value| {
            value
                .parse::<u64>()
                .map_err(|_| "invalid --recovery-attempt".to_string())
        })
        .transpose()?
        .unwrap_or(1);
    let now_ms = options
        .get("--now-ms")
        .map(|value| {
            value
                .parse::<i64>()
                .map_err(|_| "invalid --now-ms".to_string())
        })
        .transpose()?
        .unwrap_or(trusted_now_ms()?);
    let started_at_ms = now_ms - 200;
    let deadline_ms = now_ms + 5 * 60 * 1000;
    let trace = build_recovery_canary_trace(
        &recovery_id,
        &runtime_instance_id,
        recovery_attempt,
        started_at_ms,
        deadline_ms,
        "canary-child",
        "canary-session",
        &format!("nonce-{runtime_instance_id}-first"),
        &format!("nonce-{runtime_instance_id}-second"),
    );
    let trace_json = serde_json::to_string(&trace).map_err(|error| error.to_string())?;
    let event_id = format!("canary-{recovery_id}-{runtime_instance_id}");
    let idempotency_key = format!("canary-{recovery_id}-{runtime_instance_id}-key");
    let event = CoordinationEventRecord {
        event_id: &event_id,
        idempotency_key: &idempotency_key,
        aggregate_type: "recovery",
        aggregate_id: &recovery_id,
        expected_version: 0,
        event_type: "recovery_canary_passed",
        payload_json: &trace_json,
        occurred_at_ms: now_ms,
    };
    let ledger = Ledger::connect_file(&ledger_path)
        .await
        .map_err(|error| error.to_string())?;
    let persisted = ledger
        .record_recovery_canary(&trace, now_ms, &event)
        .await
        .map_err(|error| format!("cannot record recovery canary: {error}"))?;
    ledger.close().await;
    println!(
        "{}",
        json!({
            "status": "recorded",
            "recovery_id": recovery_id,
            "runtime_instance_id": runtime_instance_id,
            "recovery_attempt": recovery_attempt,
            "schema_version": SCHEMA_VERSION,
            "trace_sha256": persisted.trace_sha256,
        })
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
