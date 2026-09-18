pub mod adap;
pub mod core;
pub mod mcp;
pub mod tty;
pub mod tui;

// Backward-compatible re-exports
pub use crate::core::config;
pub use crate::core::models;
pub use crate::core::probe;
pub use crate::core::specs;
use std::collections::HashMap;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use clap::{Parser, Subcommand};

// ANSI Color constants
const COLOR_RESET: &str = "\x1b[0m";
const COLOR_BOLD: &str = "\x1b[1m";
const COLOR_DIM: &str = "\x1b[2m";
const COLOR_GREEN: &str = "\x1b[32m";
const COLOR_RED: &str = "\x1b[31m";
const COLOR_CYAN: &str = "\x1b[36m";
const COLOR_YELLOW: &str = "\x1b[33m";

/// Probe and benchmark models on an OpenAI-compatible gateway: ping, latency, throughput, caps.
#[derive(Parser)]
#[command(name = "probelm", version, about, subcommand_negates_reqs = true)]
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
    /// Test & benchmark selected models (ping/latency/throughput/caps).
    #[command(alias = "probe", alias = "check", alias = "bench", alias = "run")]
    Test(TestArgs),
    /// Sync authoritative model specifications database from LiteLLM / Community source.
    #[command(alias = "sync")]
    SyncSpecs,
    /// Serve the MCP API or install it into an agent harness.
    Mcp(McpArgs),
    /// Interactive full-screen TUI dashboard to explore, benchmark, and manage models.
    #[command(alias = "manage", alias = "dashboard")]
    Tui(TuiArgs),
    /// Single-update TTY wizard or quick configuration adjustment.
    Config(ConfigArgs),
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
    #[arg(short, long, default_value = "config.json")]
    config: String,
    /// Filter by owned_by (e.g. midas, combo, cx)
    #[arg(long)]
    owned_by: Option<String>,
    /// Filter by id prefix (e.g. midas, cx)
    #[arg(long)]
    prefix: Option<String>,
    /// Output JSON instead of a table
    #[arg(long)]
    json: bool,
    /// Write model list to JSON file and exit (default: models.json)
    #[arg(long)]
    list_models: Option<Option<String>>,
}

#[derive(clap::Args, Debug, Clone)]
struct TestArgs {
    /// Model IDs to test (positional or via --model, e.g. midas/glm-5.2)
    #[arg(value_name = "MODEL", value_delimiter = ',')]
    positional_models: Vec<String>,
    /// JSON config file
    #[arg(short, long, default_value = "config.json")]
    config: String,
    /// Test ALL models discovered on the gateway (ignores config list)
    #[arg(short = 'a', long = "all")]
    all: bool,
    /// Explicit model IDs to test (comma-separated or repeatable)
    #[arg(short = 'm', long = "model", value_delimiter = ',')]
    models: Vec<String>,
    /// Filter by owned_by (e.g. midas, cx, combo)
    #[arg(long)]
    owned_by: Option<String>,
    /// Filter by id prefix (e.g. midas, cx)
    #[arg(long)]
    prefix: Option<String>,
    /// Only test models having this capability (e.g. vision, reasoning)
    #[arg(long = "cap", value_delimiter = ',')]
    caps: Vec<String>,
    /// Custom test prompt string
    #[arg(short = 'p', long = "prompt")]
    prompt: Option<String>,
    /// Sort results by one or more keys (comma-separated):
    /// 'ctx' (context window), 'speed' (tok/s), 'ttft' (latency), 'out' (max output), 'name', 'ping'
    /// Append :asc or :desc to override direction (e.g. --sort ctx,speed or -s ctx:desc,ttft:asc)
    #[arg(short = 's', long = "sort", value_delimiter = ',')]
    sort: Vec<String>,
    /// Run only ping availability check
    #[arg(long)]
    ping: bool,
    /// Run only latency & throughput test
    #[arg(long)]
    latency: bool,
    /// Run only capability inspection
    #[arg(long)]
    caps_only: bool,
    /// Output JSON instead of a table
    #[arg(long)]
    json: bool,
    /// Output Markdown table format (ready for PR/issue)
    #[arg(long = "md", alias = "markdown")]
    markdown: bool,
    /// Number of concurrent test workers (default 1)
    #[arg(short = 'j', long = "jobs", default_value_t = 1)]
    jobs: usize,
}

#[derive(clap::Args, Debug)]
struct McpArgs {
    #[command(subcommand)]
    command: McpCommand,
}

#[derive(Subcommand, Debug)]
enum McpCommand {
    /// Serve MCP over stdio for a harness to launch.
    Serve {
        /// JSON config file.
        #[arg(short, long, default_value = "config.json")]
        config: String,
    },
    /// Register the stdio server with an agent harness.
    Install {
        /// Harness id; omit to choose interactively in a TTY.
        #[arg(long)]
        client: Option<String>,
        /// Name exposed to the harness.
        #[arg(long, default_value = "probelm")]
        name: String,
        /// Register in the project instead of the user configuration.
        #[arg(long)]
        project: bool,
        /// Replace an existing conflicting entry.
        #[arg(long)]
        force: bool,
        /// Preview the registration without writing.
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(clap::Args, Debug, Clone)]
struct TuiArgs {
    /// JSON config file
    #[arg(short, long, default_value = "config.json")]
    config: String,
}

#[derive(clap::Args, Debug, Clone)]
struct ConfigArgs {
    /// JSON config file
    #[arg(short, long, default_value = "config.json")]
    config: String,
    /// Update Gateway base URL
    #[arg(long)]
    set_url: Option<String>,
    /// Update Gateway API key
    #[arg(long)]
    set_key: Option<String>,
    /// Add model ID to configuration
    #[arg(long)]
    add_model: Option<String>,
    /// Remove model ID from configuration
    #[arg(long)]
    remove_model: Option<String>,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let code = match &cli.command {
        Some(Command::Init(a)) => cmd_init(a).await,
        Some(Command::List(a)) => cmd_list(a).await,
        Some(Command::Test(a)) => cmd_test(a).await,
        Some(Command::SyncSpecs) => cmd_sync_specs().await,
        Some(Command::Mcp(args)) => cmd_mcp(args).await,
        Some(Command::Tui(a)) => cmd_tui(a).await,
        Some(Command::Config(a)) => cmd_config(a),
        None => {
            eprintln!(
                "usage: probelm <init|test|list|sync-specs|mcp|tui|config> [options] — see --help"
            );
            eprintln!("aliases for 'test': probe, check, bench, run");
            eprintln!("aliases for 'tui': manage, dashboard");
            2
        }
    };
    std::process::exit(code);
}

async fn cmd_tui(args: &TuiArgs) -> i32 {
    let cfg = match config::Config::load(&args.config) {
        Ok(c) => c,
        Err(_) => {
            let base_url = std::env::var("ROUTER_URL")
                .unwrap_or_else(|_| "http://localhost:20128".to_string())
                .trim_end_matches('/')
                .to_string();
            let api_key = std::env::var("ROUTER_KEY")
                .ok()
                .or_else(config::detect_local_9router_key)
                .unwrap_or_default();
            config::Config {
                base_url,
                api_key,
                models: Vec::new(),
                default_prompt: "Reply with exactly: OK".to_string(),
                prompts: HashMap::new(),
                max_tokens: 64,
                temperature: 0.0,
                timeout_secs: 15,
                config_path: None,
            }
        }
    };

    let discovered = models::fetch_models(&cfg.base_url, &cfg.api_key, 5)
        .await
        .unwrap_or_default();
    let app = tui::app::App::new(cfg, args.config.clone(), discovered);
    if let Err(e) = tui::run_tui(app).await {
        eprintln!("TUI error: {e}");
        return 1;
    }
    0
}

fn cmd_config(args: &ConfigArgs) -> i32 {
    if let Some(ref url) = args.set_url {
        match tty::update_config_value(&args.config, "url", url) {
            Ok(msg) => println!("✓ {msg}"),
            Err(e) => {
                eprintln!("ERROR: {e}");
                return 1;
            }
        }
        return 0;
    }

    if let Some(ref key) = args.set_key {
        match tty::update_config_value(&args.config, "key", key) {
            Ok(msg) => println!("✓ {msg}"),
            Err(e) => {
                eprintln!("ERROR: {e}");
                return 1;
            }
        }
        return 0;
    }

    if let Some(ref m) = args.add_model {
        match tty::update_config_value(&args.config, "add-model", m) {
            Ok(msg) => println!("✓ {msg}"),
            Err(e) => {
                eprintln!("ERROR: {e}");
                return 1;
            }
        }
        return 0;
    }

    if let Some(ref m) = args.remove_model {
        match tty::update_config_value(&args.config, "remove-model", m) {
            Ok(msg) => println!("✓ {msg}"),
            Err(e) => {
                eprintln!("ERROR: {e}");
                return 1;
            }
        }
        return 0;
    }

    if let Err(e) = tty::run_single_update_wizard(&args.config) {
        eprintln!("ERROR: {e}");
        return 1;
    }
    0
}

async fn cmd_mcp(args: &McpArgs) -> i32 {
    match &args.command {
        McpCommand::Serve { config } => {
            let config = match config::Config::load(config) {
                Ok(config) => config,
                Err(error) => {
                    eprintln!("ERROR: {error}");
                    return 2;
                }
            };
            match mcp::ProbelmServer::new(config).serve_stdio().await {
                Ok(()) => 0,
                Err(error) => {
                    eprintln!("ERROR: MCP server: {error}");
                    2
                }
            }
        }
        McpCommand::Install {
            client,
            name,
            project,
            force,
            dry_run,
        } => match mcp::install_harness(client.as_deref(), name, *project, *force, *dry_run) {
            Ok(result) => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "harness": result.harness,
                        "name": result.name,
                        "target": result.target,
                        "changed": result.changed,
                        "action": result.action,
                    }))
                    .expect("MCP registration result is serializable")
                );
                0
            }
            Err(error) => {
                eprintln!("ERROR: MCP install: {error}");
                2
            }
        },
    }
}

async fn cmd_sync_specs() -> i32 {
    println!("==> Syncing model specifications from LiteLLM authoritative database...");
    match specs::sync_remote_specs().await {
        Ok(count) => {
            println!("✓ Successfully synced {count} model specifications to ~/.config/probelm/specs-cache.json");
            0
        }
        Err(e) => {
            eprintln!("ERROR: Failed to sync specs: {e}");
            2
        }
    }
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
            prompt_line(&prompt_text, Some(dk))
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
                sorted_prefixes.sort_by_key(|a| std::cmp::Reverse(a.1));
                let summary = sorted_prefixes
                    .iter()
                    .take(4)
                    .map(|(p, c)| format!("{p} ({c})"))
                    .collect::<Vec<_>>()
                    .join(", ");
                println!("    Available groups: {summary}");

                let default_prefix = args.prefix.as_deref().unwrap_or("midas");
                let filter_prompt = format!(
                    "? Filter models to include ('{default_prefix}', 'all', or comma-separated)"
                );
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
                    println!(
                        "! No models matched '{}', adding all {} models.",
                        filter_input,
                        entries.len()
                    );
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
        let home =
            std::env::var_os("HOME").expect("HOME environment variable required for --global");
        Path::new(&home)
            .join(".config")
            .join("probelm")
            .join("config.json")
    } else if interactive {
        println!();
        println!("Save target options:");
        println!("  [1] Local project file (./config.json)");
        println!("  [2] Global user config (~/.config/probelm/config.json)");
        let choice = prompt_line("? Choose save location [1/2]", Some("1"));
        if choice == "2" {
            let home = std::env::var_os("HOME").expect("HOME directory not found");
            Path::new(&home)
                .join(".config")
                .join("probelm")
                .join("config.json")
        } else {
            PathBuf::from("config.json")
        }
    } else {
        PathBuf::from("config.json")
    };

    // Check overwrite
    if target_path.exists() && !args.force && interactive {
        let confirm = prompt_line(
            &format!(
                "! File '{}' already exists. Overwrite? (y/N)",
                target_path.display()
            ),
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
    println!(
        "✓ Configuration successfully written to: {}",
        target_path.display()
    );
    println!("  - Gateway URL : {}", base_url);
    println!("  - API Key     : {}", mask_key(&api_key));
    println!("  - Models count: {}", selected_models.len());
    println!();
    println!("Quickstart commands:");
    println!("  mtest list                 # Discover models on the gateway");
    println!("  mtest test                 # Test configured models");
    println!("  mtest test <model_name>    # Test specific model directly");

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
        .filter(|m| {
            args.owned_by
                .as_deref()
                .is_none_or(|o| m.owned_by.as_deref() == Some(o))
        })
        .filter(|m| {
            args.prefix.as_deref().is_none_or(|p| {
                p.split(',').any(|prefix| {
                    let prefix = prefix.trim();
                    m.id.starts_with(&format!("{prefix}/"))
                        || m.id == prefix
                        || matches_pattern(prefix, &m.id)
                })
            })
        })
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
        println!("{:<42} OWNED_BY", "MODEL");
        println!("{}", "-".repeat(60));
        for m in &filtered {
            println!("{:<42} {}", m.id, m.owned_by.as_deref().unwrap_or("-"));
        }
    }
    0
}

async fn cmd_test(args: &TestArgs) -> i32 {
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
    // 1. Positional or explicit --model list
    // 2. If --all, --prefix, or --owned-by passed: start from all discovered gateway models
    // 3. If config.models has items: use config.models
    // 4. Otherwise: use all discovered gateway models
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
                .filter(|id| matches_pattern(input, id))
                .cloned()
                .collect();
            if !matched.is_empty() {
                targets.extend(matched);
            } else {
                eprintln!("WARN: No models matched pattern '{input}'");
            }
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
                    || matches_pattern(prefix, m)
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
    if targets.is_empty() {
        eprintln!("ERROR: no models matched the specified criteria");
        return 2;
    }

    let do_ping = args.ping || !(args.latency || args.caps_only);
    let do_latency = args.latency || !(args.ping || args.caps_only);
    let do_caps = args.caps_only || !(args.ping || args.latency);

    let targets = Arc::new(targets);
    let cfg2 = Arc::clone(&cfg);
    let custom_prompt = args.prompt.clone();
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
            let prompt = custom_prompt
                .clone()
                .or_else(|| cfg2.prompts.get(m).cloned())
                .unwrap_or_else(|| cfg2.default_prompt.clone());
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
            let prompt = custom_prompt
                .clone()
                .or_else(|| cfg2.prompts.get(&m).cloned())
                .unwrap_or_else(|| cfg2.default_prompt.clone());
            o.prompt = prompt;
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

    // Multi-key sorting if requested
    if !args.sort.is_empty() {
        results.sort_by(|a, b| {
            for key in &args.sort {
                let cmp = compare_probe_result(a, b, key);
                if cmp != std::cmp::Ordering::Equal {
                    return cmp;
                }
            }
            std::cmp::Ordering::Equal
        });
    }

    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({ "probes": results })).unwrap()
        );
    } else if args.markdown {
        print_markdown_table(&results, do_caps);
    } else {
        print_table(&results, do_ping, do_latency, do_caps);
    }

    // Exit code: 1 if any ping failed
    if do_ping
        && results
            .iter()
            .any(|r| r.ping.as_ref().is_none_or(|p| !p.ok))
    {
        1
    } else {
        0
    }
}
pub fn matches_pattern(pattern: &str, text: &str) -> bool {
    let pattern = pattern.trim();
    if pattern == text {
        return true;
    }
    if !pattern.contains('*') {
        return text == pattern || text.starts_with(&format!("{pattern}/"));
    }
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        return text == parts[0];
    }
    let mut remainder = text;
    for (i, &part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        if i == 0 {
            if !remainder.starts_with(part) {
                return false;
            }
            remainder = &remainder[part.len()..];
        } else if i == parts.len() - 1 {
            if !remainder.ends_with(part) {
                return false;
            }
        } else {
            if let Some(idx) = remainder.find(part) {
                remainder = &remainder[idx + part.len()..];
            } else {
                return false;
            }
        }
    }
    true
}

fn compare_probe_result(
    a: &probe::ProbeResult,
    b: &probe::ProbeResult,
    key: &str,
) -> std::cmp::Ordering {
    let key_lower = key.to_lowercase();
    let (field, is_desc) = if let Some(stripped) = key_lower.strip_suffix(":asc") {
        (stripped, false)
    } else if let Some(stripped) = key_lower.strip_suffix(":desc") {
        (stripped, true)
    } else {
        match key_lower.as_str() {
            "ctx" | "context" | "context_window" | "out" | "max_out" | "output" | "speed"
            | "rate" | "toks" | "tps" => (key_lower.as_str(), true),
            _ => (key_lower.as_str(), false),
        }
    };

    let ord = match field {
        "ctx" | "context" | "context_window" => {
            let ctx_a = a.caps.as_ref().and_then(|c| c.contextWindow).unwrap_or(0);
            let ctx_b = b.caps.as_ref().and_then(|c| c.contextWindow).unwrap_or(0);
            ctx_a.cmp(&ctx_b)
        }
        "out" | "max_out" | "output" => {
            let out_a = a.caps.as_ref().and_then(|c| c.maxOutput).unwrap_or(0);
            let out_b = b.caps.as_ref().and_then(|c| c.maxOutput).unwrap_or(0);
            out_a.cmp(&out_b)
        }
        "speed" | "rate" | "toks" | "tps" => {
            let rate_a = a
                .latency
                .as_ref()
                .and_then(|l| l.rate_per_sec)
                .unwrap_or(0.0);
            let rate_b = b
                .latency
                .as_ref()
                .and_then(|l| l.rate_per_sec)
                .unwrap_or(0.0);
            rate_a
                .partial_cmp(&rate_b)
                .unwrap_or(std::cmp::Ordering::Equal)
        }
        "ttft" | "latency" | "first" => {
            let ttft_a = a
                .latency
                .as_ref()
                .and_then(|l| l.ttft_secs)
                .unwrap_or(f64::MAX);
            let ttft_b = b
                .latency
                .as_ref()
                .and_then(|l| l.ttft_secs)
                .unwrap_or(f64::MAX);
            ttft_a
                .partial_cmp(&ttft_b)
                .unwrap_or(std::cmp::Ordering::Equal)
        }
        "total" | "time" => {
            let tot_a = a
                .latency
                .as_ref()
                .and_then(|l| l.total_secs)
                .unwrap_or(f64::MAX);
            let tot_b = b
                .latency
                .as_ref()
                .and_then(|l| l.total_secs)
                .unwrap_or(f64::MAX);
            tot_a
                .partial_cmp(&tot_b)
                .unwrap_or(std::cmp::Ordering::Equal)
        }
        "ping" | "status" | "ok" => {
            let ok_a = a.ping.as_ref().is_some_and(|p| p.ok);
            let ok_b = b.ping.as_ref().is_some_and(|p| p.ok);
            ok_a.cmp(&ok_b)
        }
        "name" | "model" | "id" => a.model.cmp(&b.model),
        _ => std::cmp::Ordering::Equal,
    };

    if is_desc {
        ord.reverse()
    } else {
        ord
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
            "{}{:<36} {:<5} {:>8} {:>8} {:>7}  {:<12} {:>6} {:>6}{}",
            COLOR_BOLD,
            "MODEL",
            "PING",
            "TTFT(s)",
            "TOTAL(s)",
            "TOK/s",
            "CAPS",
            "CTX",
            "OUT",
            COLOR_RESET
        );
        println!("{}{}{}", COLOR_DIM, "-".repeat(95), COLOR_RESET);
    } else {
        println!(
            "{}{:<36} {:<5} {:>8} {:>8} {:>7}{}",
            COLOR_BOLD, "MODEL", "PING", "TTFT(s)", "TOTAL(s)", "TOK/s", COLOR_RESET
        );
        println!("{}{}{}", COLOR_DIM, "-".repeat(70), COLOR_RESET);
    }

    for r in results {
        let ping_display = match &r.ping {
            Some(p) if p.ok => format!("{COLOR_GREEN}OK  {COLOR_RESET}"),
            Some(_) => format!("{COLOR_RED}FAIL{COLOR_RESET}"),
            None => format!("{COLOR_DIM}-   {COLOR_RESET}"),
        };

        let ttft = r
            .latency
            .as_ref()
            .and_then(|l| l.ttft_secs)
            .map(|v| {
                if v < 0.5 {
                    format!("{COLOR_CYAN}{v:6.3}{COLOR_RESET}")
                } else if v > 2.5 {
                    format!("{COLOR_YELLOW}{v:6.3}{COLOR_RESET}")
                } else {
                    format!("{v:6.3}")
                }
            })
            .unwrap_or_else(|| format!("{COLOR_DIM}     -{COLOR_RESET}"));

        let total = r
            .latency
            .as_ref()
            .and_then(|l| l.total_secs)
            .map(|v| format!("{v:6.3}"))
            .unwrap_or_else(|| format!("{COLOR_DIM}     -{COLOR_RESET}"));

        let rate = r
            .latency
            .as_ref()
            .and_then(|l| l.rate_per_sec)
            .map(|v| {
                if v >= 20.0 {
                    format!("{COLOR_GREEN}{v:5.1}{COLOR_RESET}")
                } else {
                    format!("{v:5.1}")
                }
            })
            .unwrap_or_else(|| format!("{COLOR_DIM}    -{COLOR_RESET}"));

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
                "{:<36} {}   {}   {}   {}  {} {:>6} {:>6}",
                r.model, ping_display, ttft, total, rate, padded_icons, ctx, out
            );
        } else {
            println!(
                "{:<36} {}   {}   {}   {}",
                r.model, ping_display, ttft, total, rate
            );
        }
    }

    if do_caps {
        println!("{}{}{}", COLOR_DIM, "-".repeat(95), COLOR_RESET);
        println!(
            "{}Legend:{} 🧠 Reasoning  👁 Vision  🛠 Tools  📄 PDF  🔍 Search  🎙 Audio  🎬 Video  🎨 Image",
            COLOR_DIM, COLOR_RESET
        );
    }
}

fn print_markdown_table(results: &[probe::ProbeResult], do_caps: bool) {
    if do_caps {
        println!("| Model | Ping | TTFT (s) | Total (s) | Tok/s | Caps | Context | Max Out |");
        println!("| :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: |");
        for r in results {
            let ping = match &r.ping {
                Some(p) if p.ok => "OK",
                Some(_) => "FAIL",
                None => "-",
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
            let (icons, ctx, out) = if let Some(c) = &r.caps {
                (
                    c.icons(),
                    models::Capabilities::format_tokens(c.contextWindow),
                    models::Capabilities::format_tokens(c.maxOutput),
                )
            } else {
                ("-".to_string(), "-".to_string(), "-".to_string())
            };
            println!(
                "| `{}` | {} | {} | {} | {} | {} | {} | {} |",
                r.model, ping, ttft, total, rate, icons, ctx, out
            );
        }
    } else {
        println!("| Model | Ping | TTFT (s) | Total (s) | Tok/s |");
        println!("| :--- | :---: | :---: | :---: | :---: |");
        for r in results {
            let ping = match &r.ping {
                Some(p) if p.ok => "OK",
                Some(_) => "FAIL",
                None => "-",
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
            println!(
                "| `{}` | {} | {} | {} | {} |",
                r.model, ping, ttft, total, rate
            );
        }
    }
}
