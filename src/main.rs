mod config;
mod models;
mod probe;

use std::collections::HashMap;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
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
    /// Initialize configuration for 9Router (interactive setup wizard).
    Init(InitArgs),
    /// Discover models on the gateway.
    List(ListArgs),
    /// Probe selected models (ping/latency/caps).
    Probe(ProbeArgs),
}

#[derive(clap::Args, Debug)]
struct InitArgs {
    /// 9Router gateway base URL [default: http://localhost:20128]
    #[arg(long)]
    url: Option<String>,
    /// 9Router API key (auto-detected if available)
    #[arg(long)]
    key: Option<String>,
    /// Filter models by prefix (e.g. midas)
    #[arg(long)]
    prefix: Option<String>,
    /// Specific model ids to configure (comma-separated)
    #[arg(long = "model", value_delimiter = ',')]
    models: Option<Vec<String>>,
    /// Output file path (default: ./config.json)
    #[arg(short, long)]
    out: Option<String>,
    /// Save globally to ~/.config/probelm/config.json
    #[arg(short, long)]
    global: bool,
    /// Non-interactive mode (use defaults and auto-detected values)
    #[arg(short = 'y', long = "yes")]
    yes: bool,
    /// Overwrite existing config file without confirmation
    #[arg(short = 'f', long = "force")]
    force: bool,
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
    /// Probe all models discovered on the gateway (ignores config list)
    #[arg(short = 'a', long = "all")]
    all: bool,
    /// Only probe matching model ids (repeatable / comma-separated)
    #[arg(long = "model", value_delimiter = ',')]
    models: Vec<String>,
    /// Filter by owned_by (e.g. midas, cx, combo)
    #[arg(long)]
    owned_by: Option<String>,
    /// Filter by id prefix (e.g. midas, cx)
    #[arg(long)]
    prefix: Option<String>,
    /// Only probe models having this capability (e.g. vision, reasoning)
    #[arg(long = "cap", value_delimiter = ',')]
    caps: Vec<String>,
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
        Some(Command::Init(a)) => cmd_init(a).await,
        Some(Command::List(a)) => cmd_list(a).await,
        Some(Command::Probe(a)) => cmd_probe(a).await,
        None => {
            eprintln!("usage: mtest <init|list|probe> [options] — see --help");
            2
        }
    };
    std::process::exit(code);
}

fn prompt_line(prompt: &str, default: Option<&str>) -> String {
    if let Some(def) = default {
        print!("{prompt} [{def}]: ");
    } else {
        print!("{prompt}: ");
    }
    let _ = io::stdout().flush();
    let mut line = String::new();
    if io::stdin().read_line(&mut line).is_ok() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            default.unwrap_or("").to_string()
        } else {
            trimmed.to_string()
        }
    } else {
        default.unwrap_or("").to_string()
    }
}

fn mask_key(k: &str) -> String {
    if k.len() <= 8 {
        "***".to_string()
    } else {
        format!("{}...{}", &k[..7], &k[k.len() - 4..])
    }
}

async fn cmd_init(args: &InitArgs) -> i32 {
    let interactive = !args.yes;

    if interactive {
        println!("==================================================");
        println!("       probelm / mtest Configuration Wizard       ");
        println!("==================================================");
    }

    // 1. Determine Base URL
    let default_url = args
        .url
        .clone()
        .or_else(|| std::env::var("ROUTER_URL").ok())
        .unwrap_or_else(|| "http://localhost:20128".to_string());

    let base_url = if interactive && args.url.is_none() {
        prompt_line("? 9Router Gateway URL", Some(&default_url))
    } else {
        default_url
    };
    let base_url = base_url.trim_end_matches('/').to_string();

    // 2. Determine API Key
    let detected_key = config::detect_local_9router_key();
    let api_key = if let Some(k) = &args.key {
        k.clone()
    } else if interactive {
        if let Some(ref dk) = detected_key {
            let masked = mask_key(dk);
            let prompt_text = format!("? 9Router API Key (detected: {masked})");
            let input = prompt_line(&prompt_text, Some(dk));
            input
        } else {
            let input = prompt_line("? 9Router API Key (e.g. sk-...)", None);
            if input.is_empty() {
                eprintln!("ERROR: API Key cannot be empty.");
                return 2;
            }
            input
        }
    } else {
        match detected_key {
            Some(k) => k,
            None => {
                eprintln!("ERROR: No API key provided or detected. Use --key <key>.");
                return 2;
            }
        }
    };

    // 3. Connect and discover models
    if interactive {
        print!("==> Connecting to {} and fetching models... ", base_url);
        let _ = io::stdout().flush();
    }

    let fetched_models = models::fetch_models(&base_url, &api_key, 10).await;
    let selected_models: Vec<String> = match fetched_models {
        Ok(entries) => {
            if interactive {
                println!("OK (found {} models)", entries.len());

                // Group prefixes for user insight
                let mut prefix_counts: HashMap<String, usize> = HashMap::new();
                for m in &entries {
                    if let Some(idx) = m.id.find('/') {
                        *prefix_counts.entry(m.id[..idx].to_string()).or_insert(0) += 1;
                    } else if let Some(ref o) = m.owned_by {
                        *prefix_counts.entry(o.clone()).or_insert(0) += 1;
                    }
                }
                let mut sorted_prefixes: Vec<_> = prefix_counts.into_iter().collect();
                sorted_prefixes.sort_by(|a, b| b.1.cmp(&a.1));
                let summary = sorted_prefixes
                    .iter()
                    .take(4)
                    .map(|(p, c)| format!("{p} ({c})"))
                    .collect::<Vec<_>>()
                    .join(", ");
                println!("    Available groups: {summary}");

                let default_prefix = args.prefix.as_deref().unwrap_or("midas");
                let filter_prompt = format!("? Filter models to include ('{default_prefix}', 'all', or comma-separated)");
                let filter_input = prompt_line(&filter_prompt, Some(default_prefix));

                let matched: Vec<String> = if filter_input == "all" {
                    entries.iter().map(|m| m.id.clone()).collect()
                } else if filter_input.contains(',') {
                    filter_input
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect()
                } else {
                    let p = filter_input.trim();
                    entries
                        .iter()
                        .filter(|m| m.id.starts_with(&format!("{p}/")) || m.id == p)
                        .map(|m| m.id.clone())
                        .collect()
                };

                if matched.is_empty() {
                    println!("! No models matched '{}', adding all {} models.", filter_input, entries.len());
                    entries.iter().map(|m| m.id.clone()).collect()
                } else {
                    println!("    Selected {} models for configuration.", matched.len());
                    matched
                }
            } else if let Some(models_arg) = &args.models {
                models_arg.clone()
            } else if let Some(prefix_arg) = &args.prefix {
                entries
                    .iter()
                    .filter(|m| m.id.starts_with(&format!("{prefix_arg}/")) || m.id == *prefix_arg)
                    .map(|m| m.id.clone())
                    .collect()
            } else {
                // Default: filter by midas if available, else all
                let midas_models: Vec<String> = entries
                    .iter()
                    .filter(|m| m.id.starts_with("midas/"))
                    .map(|m| m.id.clone())
                    .collect();
                if midas_models.is_empty() {
                    entries.iter().map(|m| m.id.clone()).collect()
                } else {
                    midas_models
                }
            }
        }
        Err(e) => {
            if interactive {
                println!("WARN: Could not fetch models ({e}). Using default template.");
            }
            args.models.clone().unwrap_or_else(|| {
                vec![
                    "midas/glm-5.2".to_string(),
                    "midas/deepseek-v4-pro".to_string(),
                ]
            })
        }
    };

    // 4. Determine save path
    let target_path: PathBuf = if let Some(out) = &args.out {
        PathBuf::from(out)
    } else if args.global {
        let home = std::env::var_os("HOME").expect("HOME environment variable required for --global");
        Path::new(&home).join(".config").join("probelm").join("config.json")
    } else if interactive {
        println!();
        println!("Save target options:");
        println!("  [1] Local project file (./config.json)");
        println!("  [2] Global user config (~/.config/probelm/config.json)");
        let choice = prompt_line("? Choose save location [1/2]", Some("1"));
        if choice == "2" {
            let home = std::env::var_os("HOME").expect("HOME directory not found");
            Path::new(&home).join(".config").join("probelm").join("config.json")
        } else {
            PathBuf::from("config.json")
        }
    } else {
        PathBuf::from("config.json")
    };

    // Check overwrite
    if target_path.exists() && !args.force && interactive {
        let confirm = prompt_line(
            &format!("! File '{}' already exists. Overwrite? (y/N)", target_path.display()),
            Some("n"),
        );
        if confirm.to_lowercase() != "y" && confirm.to_lowercase() != "yes" {
            println!("Aborted.");
            return 0;
        }
    }

    // Build configuration structure
    let file_config = config::FileConfig {
        endpoint: Some(config::EndpointCfg {
            base_url: Some(base_url.clone()),
            api_key: Some(api_key.clone()),
        }),
        models: Some(selected_models.clone()),
        default_prompt: Some("Reply with exactly: OK".to_string()),
        prompts: None,
        max_tokens: Some(64),
        temperature: Some(0.0),
        timeout_seconds: Some(120),
    };

    // Write file
    if let Some(parent) = target_path.parent() {
        if !parent.as_os_str().is_empty() {
            let _ = std::fs::create_dir_all(parent);
        }
    }

    let json_str = match serde_json::to_string_pretty(&file_config) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("ERROR: Failed to serialize config: {e}");
            return 2;
        }
    };

    if let Err(e) = std::fs::write(&target_path, json_str) {
        eprintln!("ERROR: Failed to write to '{}': {e}", target_path.display());
        return 2;
    }

    println!();
    println!("✓ Configuration successfully written to: {}", target_path.display());
    println!("  - Gateway URL : {}", base_url);
    println!("  - API Key     : {}", mask_key(&api_key));
    println!("  - Models count: {}", selected_models.len());
    println!();
    println!("Quickstart commands:");
    println!("  mtest list                 # Discover models on the gateway");
    println!("  mtest probe                # Probe configured models");
    println!("  mtest probe --model <name> # Probe specific model");

    0
}

async fn cmd_list(args: &ListArgs) -> i32 {
    let cfg = config::Config::load(&args.config).unwrap_or_else(|e| {
        eprintln!("ERROR: {e}");
        std::process::exit(2);
    });
    let entries = match models::fetch_models(&cfg.base_url, &cfg.api_key, cfg.timeout_secs).await {
        Ok(v) => v,
        Err(e) => {
            eprintln!("ERROR: {e}");
            return 2;
        }
    };

    // --list-models short-circuits to writing a JSON file
    if let Some(f) = &args.list_models {
        let path = f.clone().unwrap_or_else(|| "models.json".to_string());
        let json = models::export_json(&entries, &cfg.base_url);
        if let Err(e) = std::fs::write(&path, serde_json::to_string_pretty(&json).unwrap()) {
            eprintln!("ERROR: write {path}: {e}");
            return 2;
        }
        println!("Wrote {} models to {path}", entries.len());
        return 0;
    }

    let filtered: Vec<_> = entries
        .iter()
        .filter(|m| args.owned_by.as_deref().map_or(true, |o| m.owned_by.as_deref() == Some(o)))
        .filter(|m| args.prefix.as_deref().map_or(true, |p| m.id.starts_with(&format!("{p}/"))))
        .collect();

    if args.json {
        let arr: Vec<_> = filtered
            .iter()
            .map(|m| {
                serde_json::json!({
                    "id": m.id, "owned_by": m.owned_by, "capabilities": m.capabilities,
                })
            })
            .collect();
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
        eprintln!("ERROR: {e}");
        std::process::exit(2);
    }));

    // Fetch caps + discover models from gateway
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

    // Resolve target models:
    // 1. Explicit --model list
    // 2. If --all, --prefix, or --owned-by passed: start from all discovered gateway models
    // 3. If config.models has items: use config.models
    // 4. Otherwise: use all discovered gateway models
    let mut targets: Vec<String> = if !args.models.is_empty() {
        args.models.clone()
    } else if args.all || args.prefix.is_some() || args.owned_by.is_some() || cfg.models.is_empty() {
        entries.iter().map(|m| m.id.clone()).collect()
    } else {
        cfg.models.clone()
    };

    if let Some(p) = &args.prefix {
        targets = targets
            .into_iter()
            .filter(|m| m.starts_with(&format!("{p}/")) || m.starts_with(p))
            .collect();
    }
    if let Some(o) = &args.owned_by {
        let owned_ids: Vec<String> = entries
            .iter()
            .filter(|m| m.owned_by.as_deref() == Some(o))
            .map(|m| m.id.clone())
            .collect();
        targets = targets.into_iter().filter(|m| owned_ids.contains(m)).collect();
    }
    if !args.caps.is_empty() {
        targets = targets
            .into_iter()
            .filter(|m| {
                caps_map
                    .get(m)
                    .map(|c| args.caps.iter().any(|w| c.names().contains(w)))
                    .unwrap_or(false)
            })
            .collect();
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

    // Concurrency
    let results: Vec<probe::ProbeResult> = if args.jobs <= 1 {
        let mut v = Vec::new();
        for m in targets.iter() {
            let prompt = cfg2.prompts.get(m).cloned().unwrap_or_else(|| cfg2.default_prompt.clone());
            let mut o = opts.clone();
            o.prompt = prompt;
            match probe::probe_one(m, &o).await {
                Ok(r) => v.push(r),
                Err(e) => {
                    eprintln!("ERROR: {m}: {e}");
                    v.push(probe::ProbeResult {
                        model: m.clone(),
                        ..Default::default()
                    });
                }
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
                    probe::ProbeResult {
                        model: m,
                        ..Default::default()
                    }
                })
            }));
        }
        let mut v = Vec::new();
        for h in handles {
            v.push(h.await.unwrap());
        }
        v
    };

    // Caps into results
    let mut results = results;
    for r in results.iter_mut() {
        if do_caps {
            r.caps = caps_map.get(&r.model).cloned();
        }
    }

    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({ "probes": results })).unwrap()
        );
    } else {
        print_table(&results, do_ping, do_latency, do_caps);
    }

    // Exit code: 1 if any ping failed
    if do_ping && results.iter().any(|r| r.ping.as_ref().map_or(true, |p| !p.ok)) {
        1
    } else {
        0
    }
}

fn visual_width(s: &str) -> usize {
    let mut w = 0;
    for c in s.chars() {
        if ('\u{1F300}'..='\u{1FAFF}').contains(&c)
            || ('\u{2600}'..='\u{27BF}').contains(&c)
            || c == '👁'
            || c == '🛠'
        {
            w += 2;
        } else {
            w += 1;
        }
    }
    w
}

fn pad_visual(s: &str, target_width: usize) -> String {
    let w = visual_width(s);
    if w >= target_width {
        s.to_string()
    } else {
        format!("{}{}", s, " ".repeat(target_width - w))
    }
}

fn print_table(results: &[probe::ProbeResult], _do_ping: bool, _do_latency: bool, do_caps: bool) {
    if do_caps {
        println!(
            "{:<36} {:<5} {:>8} {:>8} {:>7}  {:<12} {:>6} {:>6}",
            "MODEL", "PING", "TTFT(s)", "TOTAL(s)", "TOK/s", "CAPS", "CTX", "OUT"
        );
        println!("{}", "-".repeat(95));
    } else {
        println!(
            "{:<36} {:<5} {:>8} {:>8} {:>7}",
            "MODEL", "PING", "TTFT(s)", "TOTAL(s)", "TOK/s"
        );
        println!("{}", "-".repeat(70));
    }

    for r in results {
        let ping = match &r.ping {
            Some(p) if p.ok => "OK".to_string(),
            Some(_) => "FAIL".to_string(),
            None => "-".to_string(),
        };
        let ttft = r
            .latency
            .as_ref()
            .and_then(|l| l.ttft_secs)
            .map(|v| format!("{v:.3}"))
            .unwrap_or_else(|| "-".into());
        let total = r
            .latency
            .as_ref()
            .and_then(|l| l.total_secs)
            .map(|v| format!("{v:.3}"))
            .unwrap_or_else(|| "-".into());
        let rate = r
            .latency
            .as_ref()
            .and_then(|l| l.rate_per_sec)
            .map(|v| format!("{v:.1}"))
            .unwrap_or_else(|| "-".into());

        if do_caps {
            let (icons, ctx, out) = if let Some(c) = &r.caps {
                (
                    c.icons(),
                    models::Capabilities::format_tokens(c.contextWindow),
                    models::Capabilities::format_tokens(c.maxOutput),
                )
            } else {
                ("-".to_string(), "-".to_string(), "-".to_string())
            };
            let padded_icons = pad_visual(&icons, 12);
            println!(
                "{:<36} {:<5} {:>8} {:>8} {:>7}  {} {:>6} {:>6}",
                r.model, ping, ttft, total, rate, padded_icons, ctx, out
            );
        } else {
            println!(
                "{:<36} {:<5} {:>8} {:>8} {:>7}",
                r.model, ping, ttft, total, rate
            );
        }
    }

    if do_caps {
        println!("{}", "-".repeat(95));
        println!("Legend: 🧠 Reasoning  👁 Vision  🛠 Tools  📄 PDF  🔍 Search  🎙 Audio  🎬 Video  🎨 Image");
    }
}
