use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

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
    #[allow(dead_code)]
    pub config_path: Option<PathBuf>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FileConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<EndpointCfg>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub models: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompts: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_seconds: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EndpointCfg {
    pub base_url: Option<String>,
    pub api_key: Option<String>,
}

impl Config {
    /// Resolve path looking in given path, and fallback to ~/.config/probelm/config.json if default.
    pub fn resolve_path(path: &str) -> Option<PathBuf> {
        let p = Path::new(path);
        if p.exists() {
            return Some(p.to_path_buf());
        }
        if path == "config.json" {
            if let Some(home) = std::env::var_os("HOME") {
                let global = Path::new(&home)
                    .join(".config")
                    .join("probelm")
                    .join("config.json");
                if global.exists() {
                    return Some(global);
                }
            }
        }
        None
    }

    /// Load from a JSON file (optional) merged with env overrides.
    /// Returns defaults when the file does not exist.
    pub fn load(path: &str) -> Result<Config, String> {
        let resolved = Self::resolve_path(path);
        let file = if let Some(p) = &resolved {
            let raw =
                std::fs::read_to_string(p).map_err(|e| format!("read {}: {e}", p.display()))?;
            serde_json::from_str::<FileConfig>(&raw)
                .map_err(|e| format!("parse {}: {e}", p.display()))?
        } else if Path::new(path).exists() {
            let raw = std::fs::read_to_string(path).map_err(|e| format!("read {path}: {e}"))?;
            serde_json::from_str::<FileConfig>(&raw).map_err(|e| format!("parse {path}: {e}"))?
        } else {
            FileConfig::default()
        };

        let env_url = std::env::var("ROUTER_URL").ok();
        let env_key = std::env::var("ROUTER_KEY").ok();

        let base_url = env_url
            .or(file.endpoint.as_ref().and_then(|e| e.base_url.clone()))
            .unwrap_or_else(|| "http://localhost:20128".to_string())
            .trim_end_matches('/')
            .to_string();

        let api_key = env_key
            .or(file.endpoint.as_ref().and_then(|e| e.api_key.clone()))
            .unwrap_or_default();

        if api_key.is_empty() {
            return Err(
                "No API key configured.\n  Run 'probelm init' to generate config.json, or export ROUTER_KEY=<key>.".to_string()
            );
        }

        Ok(Config {
            base_url,
            api_key,
            models: file.models.unwrap_or_default(),
            default_prompt: file
                .default_prompt
                .unwrap_or_else(|| "Reply with exactly: OK".to_string()),
            prompts: file.prompts.unwrap_or_default(),
            max_tokens: file.max_tokens.unwrap_or(64),
            temperature: file.temperature.unwrap_or(0.0),
            timeout_secs: file.timeout_seconds.unwrap_or(120),
            config_path: resolved,
        })
    }
}

/// Helper to detect local 9router API key from sqlite database or env
pub fn detect_local_9router_key() -> Option<String> {
    if let Ok(k) = std::env::var("ROUTER_KEY") {
        if !k.is_empty() {
            return Some(k);
        }
    }

    if let Some(home) = std::env::var_os("HOME") {
        let db_path = Path::new(&home)
            .join(".9router")
            .join("db")
            .join("data.sqlite");
        if db_path.exists() {
            // Try sqlite3 command
            if let Ok(output) = std::process::Command::new("sqlite3")
                .arg(&db_path)
                .arg("SELECT key FROM apiKeys WHERE isActive=1 LIMIT 1;")
                .output()
            {
                if output.status.success() {
                    let key = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    if !key.is_empty() {
                        return Some(key);
                    }
                }
            }

            // Fallback: search key pattern in file if sqlite3 command is not available
            if let Ok(bytes) = std::fs::read(&db_path) {
                let content = String::from_utf8_lossy(&bytes);
                // 9router keys typically start with "sk-" followed by hex
                if let Some(idx) = content.find("sk-") {
                    let slice = &content[idx..];
                    let end = slice
                        .find(|c: char| c.is_whitespace() || c == '\0' || c == '"' || c == '\'')
                        .unwrap_or(slice.len());
                    let candidate = &slice[..end];
                    if candidate.len() >= 20 {
                        return Some(candidate.to_string());
                    }
                }
            }
        }
    }

    None
}
