use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

/// Runtime config resolved from env overrides + JSON config file (env wins).
#[derive(Debug, Clone)]
pub struct Config {
    pub base_url: String,
    pub api_key: String,
    pub models: Vec<String>,
    pub default_prompt: String,
    pub prompts: HashMap<String, String>,
    pub max_tokens: u32,
    pub temperature: f64,
    pub timeout_secs: u64,
}

#[derive(Debug, Deserialize, Default)]
struct FileConfig {
    endpoint: Option<EndpointCfg>,
    models: Option<Vec<String>>,
    defaultPrompt: Option<String>,
    prompts: Option<HashMap<String, String>>,
    maxTokens: Option<u32>,
    temperature: Option<f64>,
    timeoutSeconds: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct EndpointCfg {
    baseUrl: Option<String>,
    apiKey: Option<String>,
}

impl Config {
    /// Load from a JSON file (optional) merged with env overrides.
    /// Returns defaults when the file does not exist.
    pub fn load(path: &str) -> Result<Config, String> {
        let file = if Path::new(path).exists() {
            let raw = std::fs::read_to_string(path).map_err(|e| format!("read {path}: {e}"))?;
            serde_json::from_str::<FileConfig>(&raw).map_err(|e| format!("parse {path}: {e}"))?
        } else {
            FileConfig::default()
        };

        let env_url = std::env::var("ROUTER_URL").ok();
        let env_key = std::env::var("ROUTER_KEY").ok();

        let base_url = env_url
            .or(file.endpoint.as_ref().and_then(|e| e.baseUrl.clone()))
            .unwrap_or_else(|| "http://localhost:20128".to_string())
            .trim_end_matches('/')
            .to_string();

        let api_key = env_key
            .or(file.endpoint.as_ref().and_then(|e| e.apiKey.clone()))
            .unwrap_or_default();

        if api_key.is_empty() {
            return Err("no api key — set endpoint.apiKey or ROUTER_KEY env".to_string());
        }

        Ok(Config {
            base_url,
            api_key,
            models: file.models.unwrap_or_default(),
            default_prompt: file
                .defaultPrompt
                .unwrap_or_else(|| "Reply with exactly: OK".to_string()),
            prompts: file.prompts.unwrap_or_default(),
            max_tokens: file.maxTokens.unwrap_or(64),
            temperature: file.temperature.unwrap_or(0.0),
            timeout_secs: file.timeoutSeconds.unwrap_or(120),
        })
    }
}
