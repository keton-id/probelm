mod config;
mod models;
mod probe;

use std::collections::HashMap;
use std::sync::Arc;

use clap::{Parser, Subcommand};

/// Probe/test models on a 9Router gateway: ping, latency, caps.
#[derive(Parser)]
#[command(name = "mtest", version, about, subcommand_negates_reqs = true)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Discover models on the gateway.
    List(ListArgs),
    /// Probe selected models (ping/latency/caps).
    Probe(ProbeArgs),
}

#[derive(clap::Args)]
struct ListArgs {
    /// JSON config file (optional; uses discovery otherwise)
    #[arg(long, default_value = "config.json")]
    config: String,
    /// Filter by owned_by (e.g. midas, combo)
    #[arg(long)]
    owned_by: Option<String>,
    /// Filter by id prefix (e.g. midas)
    #[arg(long)]
    prefix: Option<String>,
    /// Output JSON instead of a table
    #[arg(long)]
    json: bool,
    /// Write model list to JSON file and exit (default: models.json)
    #[arg(long)]
    list_models: Option<Option<String>>,
}

#[derive(clap::Args)]
struct ProbeArgs {
    /// JSON config file
    #[arg(long, default_value = "config.json")]
    config: String,
    /// Only probe matching model ids (repeatable / comma-separated)
    #[arg(long = "model", value_delimiter = ',')]
    models: Vec<String>,
    /// Only probe models having this capability (e.g. vision, reasoning)
    #[arg(long = "cap", value_delimiter = ',')]
    caps: Vec<String>,
    /// Filter by id prefix (e.g. midas)
    #[arg(long)]
    prefix: Option<String>,
    /// Run only ping
    #[arg(long)]
    ping: bool,
    /// Run only latency
    #[arg(long)]
    latency: bool,
    /// Run only caps
    #[arg(long)]
    caps_only: bool,
    /// Output JSON instead of a table
    #[arg(long)]
    json: bool,
    /// Run models concurrently (default 1)
    #[arg(long, default_value_t = 1)]
    jobs: usize,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let code = match &cli.command {
        Some(Command::List(a)) => cmd_list(a).await,
        Some(Command::Probe(a)) => cmd_probe(a).await,
        None => {
            eprintln!("usage: mtest <list|probe> [options] — see --help");
            2
        }
    };
    std::process::exit(code);
}

async fn cmd_list(args: &ListArgs) -> i32 {
    let cfg = config::Config::load(&args.config).unwrap_or_else(|e| {
        eprintln!("ERROR: {e}"); std::process::exit(2);
    });
    let entries = match models::fetch_models(&cfg.base_url, &cfg.api_key, cfg.timeout_secs).await {
        Ok(v) => v,
        Err(e) => { eprintln!("ERROR: {e}"); return 2; }
    };

    // --list-models short-circuits to writing a JSON file
    if let Some(f) = &args.list_models {
        let path = f.clone().unwrap_or_else(|| "models.json".to_string());
        let json = models::export_json(&entries, &cfg.base_url);
        if let Err(e) = std::fs::write(&path, serde_json::to_string_pretty(&json).unwrap()) {
            eprintln!("ERROR: write {path}: {e}"); return 2;
        }
        println!("Wrote {} models to {path}", entries.len());
        return 0;
    }

    let filtered: Vec<_> = entries.iter()
        .filter(|m| args.owned_by.as_deref().map_or(true, |o| m.owned_by.as_deref() == Some(o)))
        .filter(|m| args.prefix.as_deref().map_or(true, |p| m.id.starts_with(&format!("{p}/"))))
        .collect();

    if args.json {
        let arr: Vec<_> = filtered.iter().map(|m| serde_json::json!({
            "id": m.id, "owned_by": m.owned_by, "capabilities": m.capabilities,
        })).collect();
        println!("{}", serde_json::to_string_pretty(&arr).unwrap());
    } else {
        println!("{:<42} {}", "MODEL", "OWNED_BY");
        println!("{}", "-".repeat(60));
        for m in &filtered {
            println!("{:<42} {}", m.id, m.owned_by.as_deref().unwrap_or("-"));
        }
    }
    0
}

async fn cmd_probe(args: &ProbeArgs) -> i32 {
    let cfg = Arc::new(config::Config::load(&args.config).unwrap_or_else(|e| {
        eprintln!("ERROR: {e}"); std::process::exit(2);
    }));

    // Resolve target models (explicit --model > config.models > all from discovery)
    let mut targets: Vec<String> = args.models.clone();
    if targets.is_empty() {
        targets = cfg.models.clone();
    }

    // Fetch caps + fall back to discovery for model list
    let entries = match models::fetch_models(&cfg.base_url, &cfg.api_key, cfg.timeout_secs).await {
        Ok(v) => v,
        Err(e) => { eprintln!("WARN: fetch models: {e}"); vec![] }
    };
    let caps_map: HashMap<String, models::Capabilities> = entries.iter()
        .map(|m| (m.id.clone(), m.capabilities.clone()))
        .collect();

    if targets.is_empty() {
        targets = entries.iter().map(|m| m.id.clone()).collect();
    }
    if let Some(p) = &args.prefix {
        targets = targets.into_iter().filter(|m| m.starts_with(&format!("{p}/"))).collect();
    }
    if !args.caps.is_empty() {
        targets = targets.into_iter().filter(|m| {
            caps_map.get(m).map(|c| args.caps.iter().any(|w| c.names().contains(w))).unwrap_or(false)
        }).collect();
    }
    if targets.is_empty() {
        eprintln!("ERROR: no models matched");
        return 2;
    }

    let do_ping = args.ping || !(args.latency || args.caps_only);
    let do_latency = args.latency || !(args.ping || args.caps_only);
    let do_caps = args.caps_only || !(args.ping || args.latency);

    let targets = Arc::new(targets);
    let cfg2 = Arc::clone(&cfg);
    let opts = probe::ProbeOpts {
        base_url: cfg.base_url.clone(),
        api_key: cfg.api_key.clone(),
        prompt: String::new(),
        max_tokens: cfg.max_tokens,
        temperature: cfg.temperature,
        timeout_secs: cfg.timeout_secs,
        do_ping,
        do_latency,
    };

    // concurrency
    let results: Vec<probe::ProbeResult> = if args.jobs <= 1 {
        let mut v = Vec::new();
        for m in targets.iter() {
            let prompt = cfg2.prompts.get(m).cloned().unwrap_or_else(|| cfg2.default_prompt.clone());
            let mut o = opts.clone();
            o.prompt = prompt;
            match probe::probe_one(m, &o).await {
                Ok(r) => v.push(r),
                Err(e) => { eprintln!("ERROR: {m}: {e}"); v.push(probe::ProbeResult { model: m.clone(), ..Default::default() }); }
            }
        }
        v
    } else {
        let semaphore = Arc::new(tokio::sync::Semaphore::new(args.jobs));
        let mut handles = Vec::new();
        for m in targets.iter() {
            let m = m.clone();
            let cfg2 = Arc::clone(&cfg2);
            let mut o = opts.clone();
            o.prompt = cfg2.prompts.get(&m).cloned().unwrap_or_else(|| cfg2.default_prompt.clone());
            let sem = Arc::clone(&semaphore);
            handles.push(tokio::spawn(async move {
                let _p = sem.acquire().await.unwrap();
                probe::probe_one(&m, &o).await.unwrap_or_else(|e| {
                    eprintln!("ERROR: {m}: {e}");
                    probe::ProbeResult { model: m, ..Default::default() }
                })
            }));
        }
        let mut v = Vec::new();
        for h in handles { v.push(h.await.unwrap()); }
        v
    };

    // caps into results
    let mut results = results;
    for r in results.iter_mut() {
        if do_caps {
            r.caps = caps_map.get(&r.model).cloned();
        }
    }

    if args.json {
        println!("{}", serde_json::to_string_pretty(&serde_json::json!({ "probes": results })).unwrap());
    } else {
        print_table(&results, do_ping, do_latency, do_caps);
    }

    // exit code: 1 if any ping failed
    if do_ping && results.iter().any(|r| r.ping.as_ref().map_or(true, |p| !p.ok)) {
        1
    } else {
        0
    }
}

fn print_table(results: &[probe::ProbeResult], _do_ping: bool, _do_latency: bool, do_caps: bool) {
    println!("{:<38} {:<5} {:>9} {:>9} {:>7}  {}", "MODEL", "PING", "TTFT(s)", "TOTAL(s)", "TOK/s", "CAPABILITIES");
    println!("{}", "-".repeat(90));
    for r in results {
        let ping = match &r.ping {
            Some(p) if p.ok => "OK".to_string(),
            Some(_) => "FAIL".to_string(),
            None => "-".to_string(),
        };
        let ttft = r.latency.as_ref().and_then(|l| l.ttft_secs).map(|v| format!("{v:.3}")).unwrap_or_else(|| "-".into());
        let total = r.latency.as_ref().and_then(|l| l.total_secs).map(|v| format!("{v:.3}")).unwrap_or_else(|| "-".into());
        let rate = r.latency.as_ref().and_then(|l| l.rate_per_sec).map(|v| format!("{v:.1}")).unwrap_or_else(|| "-".into());
        let caps = if do_caps {
            r.caps.as_ref().map(|c| c.compact()).unwrap_or_else(|| "unknown".into())
        } else { "-".into() };
        println!("{:<38} {:<5} {:>9} {:>9} {:>7}  {}", r.model, ping, ttft, total, rate, caps);
    }
}
