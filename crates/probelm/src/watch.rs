//! `probelm watch`: long-running periodic probing with durable state output.
//!
//! Contract (the integration channel consumed by harnesses such as arsy-code):
//!
//! - Every cycle, all target models are probed and the full view is written
//!   atomically to `state.json` (`StateFile` schema, see `probelm-proto`).
//! - On stdout, one NDJSON line per state *transition*:
//!   `{"event":"transition","model":...,"from":...,"to":...,"snapshot":{...}}`.
//! - No output is emitted for cycles with no transition; late readers rely on
//!   `state.json`, live readers tail stdout.
//! - On SIGINT/SIGTERM a final state file is written before exit.

use crate::core::config::Config;
use crate::core::watch::{probe_snapshots, write_state_file_atomic, WatchConfig};
use crate::models;
use probelm_proto::{diff_snapshots, StateFile, WatchEvent};
use std::collections::HashMap;
use std::io::{self, Write};
use std::path::PathBuf;

use clap::{Args, ValueHint};

#[derive(Args, Debug, Clone)]
pub struct WatchArgs {
    /// JSON config file.
    #[arg(short, long, default_value = "config.json")]
    pub config: String,
    /// Seconds between probe cycles.
    #[arg(short = 'i', long, default_value_t = 30)]
    pub interval: u64,
    /// Path of the durable state file written every cycle.
    #[arg(long, default_value = "state.json", value_hint = ValueHint::FilePath)]
    pub state: PathBuf,
    /// Model IDs to watch (positional or via --model, e.g. midas/glm-5.2).
    #[arg(value_name = "MODEL", value_delimiter = ',')]
    pub positional_models: Vec<String>,
    /// Explicit model IDs to watch (comma-separated or repeatable).
    #[arg(short = 'm', long = "model", value_delimiter = ',')]
    pub models: Vec<String>,
    /// Watch ALL models discovered on the gateway (ignores config list).
    #[arg(short = 'a', long = "all")]
    pub all: bool,
    /// Filter by owned_by (e.g. midas, cx, combo).
    #[arg(long)]
    pub owned_by: Option<String>,
    /// Filter by id prefix (e.g. midas, cx).
    #[arg(long)]
    pub prefix: Option<String>,
    /// Only watch models having this capability (e.g. vision, reasoning).
    #[arg(long = "cap", value_delimiter = ',')]
    pub caps: Vec<String>,
    /// Custom probe prompt string.
    #[arg(short = 'p', long = "prompt")]
    pub prompt: Option<String>,
    /// Number of concurrent probe workers (default 1).
    #[arg(short = 'j', long = "jobs", default_value_t = 1)]
    pub jobs: usize,
    /// Reasoning effort level sent to the API (none, low, medium, high).
    #[arg(long = "reasoning-effort", value_name = "EFFORT")]
    pub reasoning_effort: Option<String>,
}

/// Command entrypoint. Returns the process exit code.
pub async fn cmd_watch(args: &WatchArgs) -> i32 {
    let cfg = match Config::load(&args.config) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("ERROR: {e}");
            return 2;
        }
    };

    // Discovery + caps (same resolution as `probelm test`).
    let entries = match models::fetch_models(&cfg.base_url, &cfg.api_key, cfg.timeout_secs).await {
        Ok(v) => v,
        Err(e) => {
            eprintln!("WARN: fetch models: {e}");
            vec![]
        }
    };
    let caps_map: HashMap<String, models::Capabilities> = entries
        .iter()
        .map(|m| (m.id.clone(), m.capabilities.clone()))
        .collect();

    let targets = resolve_targets(args, &cfg, &entries, &caps_map);
    if targets.is_empty() {
        eprintln!("ERROR: no models matched the specified criteria");
        return 2;
    }

    let watch_cfg = WatchConfig {
        base_url: cfg.base_url.clone(),
        api_key: cfg.api_key.clone(),
        models: targets.clone(),
        prompt: args
            .prompt
            .clone()
            .unwrap_or_else(|| cfg.default_prompt.clone()),
        max_tokens: cfg.max_tokens,
        temperature: cfg.temperature,
        timeout_secs: cfg.timeout_secs,
        interval_secs: args.interval.max(1),
        reasoning_effort: args
            .reasoning_effort
            .clone()
            .or_else(|| cfg.reasoning_effort.clone()),
    };

    let stdout = io::stdout();
    let mut prev_snapshots: Vec<probelm_proto::ProbeSnapshot> = Vec::new();
    eprintln!(
        "watch: {} model(s), interval {}s, state file {}",
        targets.len(),
        args.interval,
        args.state.display()
    );

    loop {
        let snapshots = probe_snapshots(&watch_cfg).await;
        let state = StateFile {
            schema_version: probelm_proto::STATE_SCHEMA_VERSION,
            generated_at_unix: unix_now(),
            interval_secs: watch_cfg.interval_secs,
            models: snapshots.clone(),
        };

        if let Err(e) = write_state_file_atomic(&args.state, &state) {
            eprintln!("ERROR: write state file: {e}");
        }

        for event in diff_snapshots(&prev_snapshots, &snapshots) {
            emit_event(&event, &stdout);
        }
        prev_snapshots = snapshots;

        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                eprintln!("watch: interrupted; writing final state");
                break;
            }
            _ = tokio::time::sleep(std::time::Duration::from_secs(watch_cfg.interval_secs)) => {}
        }
    }

    // Final write on shutdown so readers see the last cycle even if it was a
    // no-transition cycle immediately before the signal.
    let snapshots = probe_snapshots(&watch_cfg).await;
    let state = StateFile {
        schema_version: probelm_proto::STATE_SCHEMA_VERSION,
        generated_at_unix: unix_now(),
        interval_secs: watch_cfg.interval_secs,
        models: snapshots.clone(),
    };
    let _ = write_state_file_atomic(&args.state, &state);
    for event in diff_snapshots(&prev_snapshots, &snapshots) {
        emit_event(&event, &stdout);
    }
    0
}

fn emit_event(event: &WatchEvent, out: &io::Stdout) {
    if let Ok(line) = serde_json::to_string(event) {
        let mut handle = out.lock();
        let _ = writeln!(handle, "{line}");
        let _ = handle.flush();
    }
}

fn resolve_targets(
    args: &WatchArgs,
    cfg: &Config,
    entries: &[models::ModelEntry],
    caps_map: &HashMap<String, models::Capabilities>,
) -> Vec<String> {
    let all_discovered_ids: Vec<String> = entries.iter().map(|m| m.id.clone()).collect();

    let raw_inputs: Vec<String> = if !args.positional_models.is_empty() {
        let mut combined = args.positional_models.clone();
        combined.extend(args.models.clone());
        combined
    } else if !args.models.is_empty() {
        args.models.clone()
    } else if args.all || args.prefix.is_some() || args.owned_by.is_some() || cfg.models.is_empty()
    {
        all_discovered_ids.clone()
    } else {
        cfg.models.clone()
    };

    let mut targets: Vec<String> = Vec::new();
    for input in &raw_inputs {
        if input.contains('*') {
            let matched: Vec<String> = all_discovered_ids
                .iter()
                .filter(|id| crate::matches_pattern(input, id))
                .cloned()
                .collect();
            if matched.is_empty() {
                eprintln!("WARN: No models matched pattern '{input}'");
            }
            targets.extend(matched);
        } else if all_discovered_ids.contains(input) {
            targets.push(input.clone());
        } else {
            let matched: Vec<String> = all_discovered_ids
                .iter()
                .filter(|id| id.starts_with(&format!("{input}/")) || id.as_str() == input.as_str())
                .cloned()
                .collect();
            if !matched.is_empty() {
                targets.extend(matched);
            } else {
                targets.push(input.clone());
            }
        }
    }

    let mut seen = std::collections::HashSet::new();
    targets.retain(|item| seen.insert(item.clone()));

    if let Some(p) = &args.prefix {
        let prefixes: Vec<&str> = p
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();
        targets.retain(|m| {
            prefixes.iter().any(|prefix| {
                m.starts_with(&format!("{prefix}/"))
                    || m.starts_with(prefix)
                    || crate::matches_pattern(prefix, m)
            })
        });
    }
    if let Some(o) = &args.owned_by {
        let owned_ids: Vec<String> = entries
            .iter()
            .filter(|m| m.owned_by.as_deref() == Some(o))
            .map(|m| m.id.clone())
            .collect();
        targets.retain(|m| owned_ids.contains(m));
    }
    if !args.caps.is_empty() {
        targets.retain(|m| {
            caps_map
                .get(m)
                .map(|c| args.caps.iter().any(|w| c.names().contains(w)))
                .unwrap_or(false)
        });
    }
    // Keep deterministic ordering.
    targets.sort();
    targets
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> Config {
        Config {
            base_url: "http://localhost:20128".into(),
            api_key: "k".into(),
            models: vec![],
            default_prompt: "Reply with exactly: OK".into(),
            prompts: HashMap::new(),
            max_tokens: 64,
            temperature: 0.0,
            timeout_secs: 120,
            reasoning_effort: None,
            config_path: None,
        }
    }

    fn args(models: Vec<String>, all: bool) -> WatchArgs {
        WatchArgs {
            config: "config.json".into(),
            interval: 30,
            state: PathBuf::from("state.json"),
            positional_models: models,
            models: vec![],
            all,
            owned_by: None,
            prefix: None,
            caps: vec![],
            prompt: None,
            jobs: 1,
            reasoning_effort: None,
        }
    }

    #[test]
    fn resolves_config_models_when_no_filters() {
        let mut c = cfg();
        c.models = vec!["a".into(), "b".into()];
        let targets = resolve_targets(&args(vec![], false), &c, &[], &HashMap::new());
        assert_eq!(targets, vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn resolves_all_discovered_when_all_flag() {
        let entries = vec![models::ModelEntry {
            id: "x/1".into(),
            object: None,
            owned_by: Some("x".into()),
            capabilities: Default::default(),
        }];
        let targets = resolve_targets(&args(vec![], true), &cfg(), &entries, &HashMap::new());
        assert_eq!(targets, vec!["x/1".to_string()]);
    }
}
