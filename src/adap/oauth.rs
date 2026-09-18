use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Status of a local OAuth / session credential.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionStatus {
    Available,
    NotConfigured,
    Expired,
}

/// Metadata about a detected local developer session or OAuth token.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedOAuthSession {
    pub provider_id: String,
    pub display_name: String,
    pub status: SessionStatus,
    pub account: Option<String>,
    pub token_preview: Option<String>,
    pub source_path: Option<PathBuf>,
}

/// Helper to scan known local developer environments for OAuth / session credentials.
pub fn scan_local_oauth_providers() -> Vec<DetectedOAuthSession> {
    let mut sessions = Vec::new();
    let home = match std::env::var_os("HOME") {
        Some(h) => PathBuf::from(h),
        None => return sessions,
    };

    // 1. Claude Code session check
    let claude_config = home.join(".claude.json");
    let claude_dir = home.join(".claude");
    if claude_config.exists() || claude_dir.exists() {
        let mut account = None;
        let mut preview = None;
        let mut available = false;

        if claude_config.exists() {
            if let Ok(content) = std::fs::read_to_string(&claude_config) {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(email) = v.get("oauthAccount").and_then(|o| o.get("emailAddress")).and_then(|e| e.as_str()) {
                        account = Some(email.to_string());
                        available = true;
                    }
                    if let Some(token) = v.get("oauthToken").and_then(|t| t.as_str()) {
                        preview = Some(mask_token(token));
                        available = true;
                    }
                }
            }
        }

        sessions.push(DetectedOAuthSession {
            provider_id: "claude-code".to_string(),
            display_name: "Claude Code OAuth Session".to_string(),
            status: if available { SessionStatus::Available } else { SessionStatus::NotConfigured },
            account,
            token_preview: preview,
            source_path: if claude_config.exists() { Some(claude_config) } else { Some(claude_dir) },
        });
    }

    // 2. GitHub Copilot session check
    let copilot_config = home.join(".config").join("github-copilot").join("hosts.json");
    let gh_hosts = home.join(".config").join("gh").join("hosts.yml");
    if copilot_config.exists() || gh_hosts.exists() {
        let mut account = None;
        let mut preview = None;
        let mut available = false;

        if copilot_config.exists() {
            if let Ok(content) = std::fs::read_to_string(&copilot_config) {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(obj) = v.as_object() {
                        for (host, val) in obj {
                            if let Some(user) = val.get("user").and_then(|u| u.as_str()) {
                                account = Some(format!("{user}@{host}"));
                                available = true;
                            }
                            if let Some(tok) = val.get("oauth_token").and_then(|t| t.as_str()) {
                                preview = Some(mask_token(tok));
                                available = true;
                            }
                        }
                    }
                }
            }
        }

        sessions.push(DetectedOAuthSession {
            provider_id: "github-copilot".to_string(),
            display_name: "GitHub Copilot / Codex OAuth".to_string(),
            status: if available { SessionStatus::Available } else { SessionStatus::NotConfigured },
            account,
            token_preview: preview,
            source_path: if copilot_config.exists() { Some(copilot_config) } else { Some(gh_hosts) },
        });
    }

    sessions
}

fn mask_token(t: &str) -> String {
    if t.len() <= 8 {
        "***".to_string()
    } else {
        format!("{}...{}", &t[..4], &t[t.len() - 4..])
    }
}
