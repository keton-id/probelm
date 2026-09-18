use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::models::Capabilities;

/// Authoritative specification for a known model.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelSpec {
    pub context_window: Option<u64>,
    pub max_output: Option<u64>,
    pub reasoning: Option<bool>,
    pub vision: Option<bool>,
    pub tools: Option<bool>,
    pub search: Option<bool>,
    pub pdf: Option<bool>,
    pub audio: Option<bool>,
    pub video: Option<bool>,
}

/// Normalize model ID by stripping vendor/gateway prefixes.
pub fn normalize_model_name(id: &str) -> String {
    let clean = id.trim().to_lowercase();
    // Common prefixes from 9router / aggregators
    for prefix in &[
        "midas/",
        "cx/",
        "combo/",
        "openai/",
        "anthropic/",
        "deepseek/",
        "z-ai/",
        "zai-org/",
        "zai/",
        "google/",
        "meta-llama/",
        "qwen/",
        "moonshotai/",
        "minimax/",
        "hpc-ai/",
        "azure_ai/",
        "bedrock/",
    ] {
        if let Some(stripped) = clean.strip_prefix(prefix) {
            return stripped.to_string();
        }
    }
    clean
}

/// Curated Source of Truth for flagship and standard models.
static BUILTIN_SPECS: &[(&str, u64, u64, bool, bool, bool)] = &[
    // (model_suffix, contextWindow, maxOutput, reasoning, vision, tools)
    ("glm-5.2", 1_048_576, 131_072, true, false, true),
    ("glm-5.3", 1_000_000, 131_072, true, true, true),
    ("glm-5.3-flash", 1_000_000, 131_072, true, true, true),
    ("glm-4.7", 200_000, 128_000, true, false, true),
    ("deepseek-v4-pro", 1_000_000, 384_000, true, false, true),
    ("deepseek-v4-flash", 1_000_000, 128_000, true, false, true),
    ("deepseek-v3.2", 163_840, 64_000, true, false, true),
    ("gpt-5.6-sol", 400_000, 128_000, true, true, true),
    ("gpt-5.6-terra", 400_000, 128_000, true, true, true),
    ("gpt-5.6-luna", 400_000, 128_000, true, true, true),
    ("gpt-5.5", 400_000, 128_000, true, true, true),
    ("gpt-5.4", 400_000, 128_000, true, true, true),
    ("gpt-5.4-mini", 400_000, 128_000, true, true, true),
    ("gpt-5.3-codex", 400_000, 128_000, true, false, true),
    ("gpt-5.3-codex-spark", 400_000, 128_000, true, false, true),
    ("sonnet-4.5", 200_000, 64_000, false, true, true),
    ("sonnet-4", 200_000, 64_000, false, true, true),
    ("haiku-4.5", 200_000, 64_000, false, false, true),
    ("claude-opus-4.7", 1_000_000, 128_000, true, true, true),
    ("qwen3.6-35b-a3b", 1_000_000, 65_536, true, true, true),
    ("qwen3-coder", 1_000_000, 64_000, true, false, true),
    ("kimi-k2.7-code", 262_144, 65_536, true, true, true),
    ("kimi-k2.6", 262_144, 262_144, true, true, true),
    ("mimo-pro", 262_144, 131_072, false, true, true),
    ("mimo", 262_144, 131_072, false, true, true),
    ("gemma-4-26b-a4b-it", 128_000, 64_000, false, true, true),
];

fn get_cache_path() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|h| {
        Path::new(&h)
            .join(".config")
            .join("probelm")
            .join("specs-cache.json")
    })
}

/// Load cached specifications from disk if present.
fn load_cached_specs() -> HashMap<String, ModelSpec> {
    if let Some(path) = get_cache_path() {
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(map) = serde_json::from_str::<HashMap<String, ModelSpec>>(&content) {
                    return map;
                }
            }
        }
    }
    HashMap::new()
}

/// Look up authoritative specification for a given model ID.
pub fn lookup_spec(model_id: &str) -> Option<ModelSpec> {
    let norm = normalize_model_name(model_id);

    // 1. Check cached remote database
    let cache = load_cached_specs();
    if let Some(spec) = cache.get(&norm) {
        return Some(spec.clone());
    }

    // 2. Check curated built-in table
    for &(name, ctx, out, reasoning, vision, tools) in BUILTIN_SPECS {
        if norm == name || norm.contains(name) {
            return Some(ModelSpec {
                context_window: Some(ctx),
                max_output: Some(out),
                reasoning: Some(reasoning),
                vision: Some(vision),
                tools: Some(tools),
                search: None,
                pdf: None,
                audio: None,
                video: None,
            });
        }
    }

    None
}

/// Enrich gateway capabilities with authoritative specs if gateway reported lower/default values.
pub fn enrich_capabilities(model_id: &str, mut caps: Capabilities) -> Capabilities {
    if let Some(spec) = lookup_spec(model_id) {
        if let Some(ctx) = spec.context_window {
            // If gateway reported 0 or fallback 200k for models that are actually 1M+, use authoritative
            if caps.contextWindow.unwrap_or(0) < ctx {
                caps.contextWindow = Some(ctx);
            }
        }
        if let Some(out) = spec.max_output {
            if caps.maxOutput.unwrap_or(0) < out {
                caps.maxOutput = Some(out);
            }
        }
        if let Some(r) = spec.reasoning {
            caps.reasoning = caps.reasoning || r;
        }
        if let Some(v) = spec.vision {
            caps.vision = caps.vision || v;
        }
        if let Some(t) = spec.tools {
            caps.tools = caps.tools || t;
        }
    }
    caps
}

/// Download authoritative model specification database from LiteLLM community source.
pub async fn sync_remote_specs() -> Result<usize, String> {
    let url = "https://raw.githubusercontent.com/BerriAI/litellm/main/model_prices_and_context_window.json";
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("Client error: {e}"))?;

    let resp = client
        .get(url)
        .header("User-Agent", "probelm/0.1.0")
        .send()
        .await
        .map_err(|e| format!("GET {url}: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("HTTP {} from {}", resp.status(), url));
    }

    let raw_json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse JSON from LiteLLM: {e}"))?;

    let mut spec_map: HashMap<String, ModelSpec> = HashMap::new();

    if let Some(obj) = raw_json.as_object() {
        for (key, val) in obj {
            if key == "sample_spec" {
                continue;
            }
            let norm = normalize_model_name(key);
            let ctx = val.get("max_input_tokens").and_then(|v| v.as_u64());
            let out = val.get("max_output_tokens").and_then(|v| v.as_u64());
            let vision = val.get("supports_vision").and_then(|v| v.as_bool());
            let reasoning = val.get("supports_reasoning").and_then(|v| v.as_bool());
            let tools = val
                .get("supports_function_calling")
                .and_then(|v| v.as_bool());

            if ctx.is_some() || out.is_some() {
                spec_map.insert(
                    norm,
                    ModelSpec {
                        context_window: ctx,
                        max_output: out,
                        reasoning,
                        vision,
                        tools,
                        search: None,
                        pdf: None,
                        audio: None,
                        video: None,
                    },
                );
            }
        }
    }

    let count = spec_map.len();
    if let Some(cache_path) = get_cache_path() {
        if let Some(parent) = cache_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let json_str =
            serde_json::to_string_pretty(&spec_map).map_err(|e| format!("Serialize error: {e}"))?;
        std::fs::write(&cache_path, json_str)
            .map_err(|e| format!("Write {}: {e}", cache_path.display()))?;
    }

    Ok(count)
}
