//! Long-running periodic probing backing `probelm watch`.
//!
//! Produces per-cycle snapshots, diffs transitions, and atomically writes
//! `state.json` so external harnesses can consume model health without
//! holding the process's stdout open.

use crate::probe::{probe_one, ProbeOpts};
use probelm_proto::{ProbeResult, ProbeSnapshot, StateFile};
use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Tunables for the watch loop. Plain data; the CLI builds it from config + argv.
#[derive(Debug, Clone)]
pub struct WatchConfig {
    pub base_url: String,
    pub api_key: String,
    /// Exact model IDs to probe each cycle.
    pub models: Vec<String>,
    pub prompt: String,
    pub max_tokens: u32,
    pub temperature: f64,
    pub timeout_secs: u64,
    pub interval_secs: u64,
    pub reasoning_effort: Option<String>,
}

/// Probe every configured model once and produce snapshots for this cycle.
pub async fn probe_snapshots(cfg: &WatchConfig) -> Vec<ProbeSnapshot> {
    let now = unix_now();
    let mut out = Vec::with_capacity(cfg.models.len());
    for model in &cfg.models {
        let opts = ProbeOpts {
            base_url: cfg.base_url.clone(),
            api_key: cfg.api_key.clone(),
            prompt: cfg.prompt.clone(),
            max_tokens: cfg.max_tokens,
            temperature: cfg.temperature,
            timeout_secs: cfg.timeout_secs,
            do_ping: true,
            do_latency: true,
            reasoning_effort: cfg.reasoning_effort.clone(),
        };
        let res = probe_one(model, &opts).await;
        out.push(snapshot_from_result(model.clone(), res, now));
    }
    out
}

/// Convert a single probe result into a durable snapshot.
pub fn snapshot_from_result(
    model: String,
    res: ProbeResult,
    updated_at_unix: u64,
) -> ProbeSnapshot {
    ProbeSnapshot {
        model,
        state: res.state(),
        ping: res.ping,
        latency: res.latency,
        error: res.state_error,
        updated_at_unix,
    }
}

/// Build the full state file for a cycle.
pub fn build_state_file(cfg: &WatchConfig, snapshots: Vec<ProbeSnapshot>) -> StateFile {
    StateFile {
        schema_version: probelm_proto::STATE_SCHEMA_VERSION,
        generated_at_unix: unix_now(),
        interval_secs: cfg.interval_secs,
        models: snapshots,
    }
}

/// Atomically replace `path` with the serialized state file.
///
/// Writes to `<path>.tmp`, fsyncs, then renames over the target so a reader
/// never observes a partially written file.
pub fn write_state_file_atomic(path: &Path, state: &StateFile) -> std::io::Result<()> {
    let json = serde_json::to_string(state).map_err(std::io::Error::other)?;
    let mut tmp = OsString::from(path.as_os_str());
    tmp.push(".tmp");
    let tmp_path = PathBuf::from(tmp);
    let mut f = std::fs::File::create(&tmp_path)?;
    f.write_all(json.as_bytes())?;
    f.write_all(b"\n")?;
    f.sync_all()?;
    drop(f);
    std::fs::rename(&tmp_path, path)
}

/// Read and parse a previously written state file; `None` when missing/corrupt.
pub fn read_state_file(path: &Path) -> Option<StateFile> {
    let raw = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_state_file_with_schema_version() {
        let cfg = WatchConfig {
            base_url: "http://x".into(),
            api_key: "k".into(),
            models: vec![],
            prompt: "p".into(),
            max_tokens: 4,
            temperature: 0.0,
            timeout_secs: 5,
            interval_secs: 30,
            reasoning_effort: None,
        };
        let file = build_state_file(&cfg, vec![]);
        assert_eq!(file.schema_version, probelm_proto::STATE_SCHEMA_VERSION);
        assert_eq!(file.interval_secs, 30);
        assert!(file.generated_at_unix > 0);
    }

    #[test]
    fn atomic_write_then_read_round_trips() {
        let dir = std::env::temp_dir().join(format!("probelm-watch-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("state.json");
        let cfg = WatchConfig {
            base_url: "http://x".into(),
            api_key: "k".into(),
            models: vec!["m".into()],
            prompt: "p".into(),
            max_tokens: 4,
            temperature: 0.0,
            timeout_secs: 5,
            interval_secs: 30,
            reasoning_effort: None,
        };
        let res = ProbeResult {
            model: "m".into(),
            ..Default::default()
        };
        let file = build_state_file(&cfg, vec![snapshot_from_result("m".into(), res, 42)]);
        write_state_file_atomic(&path, &file).unwrap();
        let back = read_state_file(&path).unwrap();
        assert_eq!(back, file);
        assert_eq!(back.models[0].model, "m");
        assert_eq!(back.models[0].updated_at_unix, 42);
        // No leftover temp file
        assert!(!path.with_extension("json.tmp").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
