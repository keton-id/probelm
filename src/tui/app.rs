use crate::adap::oauth::{scan_local_oauth_providers, DetectedOAuthSession};
use crate::core::config::{Config, FileConfig};
use crate::core::models::ModelEntry;
use crate::core::probe::{probe_one, ProbeOpts, ProbeResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Models,
    Adapters,
    Help,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    AddingModel,
}

#[derive(Debug, Clone)]
pub struct ModelItem {
    pub id: String,
    pub in_config: bool,
    pub last_probe: Option<ProbeResult>,
}

pub struct App {
    pub config: Config,
    pub config_path: String,
    pub active_tab: Tab,
    pub input_mode: InputMode,
    pub input_buffer: String,
    pub models: Vec<ModelItem>,
    pub selected_index: usize,
    pub detected_sessions: Vec<DetectedOAuthSession>,
    pub status_message: String,
    pub is_probing: bool,
    pub should_quit: bool,
}

impl App {
    pub fn new(config: Config, config_path: String, discovered_models: Vec<ModelEntry>) -> Self {
        let mut model_map = std::collections::HashMap::new();

        for m in &config.models {
            model_map.insert(
                m.clone(),
                ModelItem {
                    id: m.clone(),
                    in_config: true,
                    last_probe: None,
                },
            );
        }

        for entry in discovered_models {
            model_map
                .entry(entry.id.clone())
                .or_insert_with(|| ModelItem {
                    id: entry.id,
                    in_config: false,
                    last_probe: None,
                });
        }

        let mut models: Vec<ModelItem> = model_map.into_values().collect();
        models.sort_by(|a, b| {
            // Configured models first, then alphabetical
            b.in_config.cmp(&a.in_config).then_with(|| a.id.cmp(&b.id))
        });

        let detected_sessions = scan_local_oauth_providers();

        Self {
            config,
            config_path,
            active_tab: Tab::Models,
            input_mode: InputMode::Normal,
            input_buffer: String::new(),
            models,
            selected_index: 0,
            detected_sessions,
            status_message: "Ready. Press '?' for help, 'a' to add model, 'r' to probe."
                .to_string(),
            is_probing: false,
            should_quit: false,
        }
    }

    pub fn selected_model(&self) -> Option<&ModelItem> {
        self.models.get(self.selected_index)
    }

    pub fn select_next(&mut self) {
        if !self.models.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.models.len();
        }
    }

    pub fn select_prev(&mut self) {
        if !self.models.is_empty() {
            if self.selected_index == 0 {
                self.selected_index = self.models.len() - 1;
            } else {
                self.selected_index -= 1;
            }
        }
    }

    pub fn toggle_selected_model(&mut self) {
        if let Some(item) = self.models.get_mut(self.selected_index) {
            item.in_config = !item.in_config;
            let id = item.id.clone();
            let is_in = item.in_config;
            self.sync_models_to_config();
            if is_in {
                self.status_message = format!("✓ Added '{id}' to config");
            } else {
                self.status_message = format!("- Removed '{id}' from config");
            }
        }
    }

    pub fn submit_add_model(&mut self) {
        let trimmed = self.input_buffer.trim().to_string();
        if !trimmed.is_empty() {
            if let Some(existing) = self.models.iter_mut().find(|m| m.id == trimmed) {
                existing.in_config = true;
            } else {
                self.models.insert(
                    0,
                    ModelItem {
                        id: trimmed.clone(),
                        in_config: true,
                        last_probe: None,
                    },
                );
                self.selected_index = 0;
            }
            self.sync_models_to_config();
            self.status_message = format!("✓ Inserted '{trimmed}' into config");
        }
        self.input_buffer.clear();
        self.input_mode = InputMode::Normal;
    }

    pub fn delete_selected_model(&mut self) {
        if let Some(item) = self.models.get_mut(self.selected_index) {
            if item.in_config {
                item.in_config = false;
                let id = item.id.clone();
                self.sync_models_to_config();
                self.status_message = format!("- Removed '{id}' from config");
            }
        }
    }

    pub async fn run_probe_selected(&mut self) {
        let Some(item) = self.models.get(self.selected_index) else {
            return;
        };
        let model_id = item.id.clone();
        self.status_message = format!("⏳ Probing '{model_id}'...");
        self.is_probing = true;

        let opts = ProbeOpts {
            base_url: self.config.base_url.clone(),
            api_key: self.config.api_key.clone(),
            prompt: self.config.default_prompt.clone(),
            max_tokens: self.config.max_tokens,
            temperature: self.config.temperature,
            timeout_secs: 15,
            do_ping: true,
            do_latency: true,
        };

        match probe_one(&model_id, &opts).await {
            Ok(res) => {
                let ping_status = res
                    .ping
                    .as_ref()
                    .map(|p| if p.ok { "200 OK" } else { "FAILED" })
                    .unwrap_or("N/A");
                let rate = res
                    .latency
                    .as_ref()
                    .and_then(|l| l.rate_per_sec)
                    .map(|r| format!("{:.1} tok/s", r))
                    .unwrap_or_else(|| "-".to_string());
                self.status_message =
                    format!("✓ Probed '{model_id}': ping={ping_status}, speed={rate}");
                if let Some(m) = self.models.iter_mut().find(|m| m.id == model_id) {
                    m.last_probe = Some(res);
                }
            }
            Err(e) => {
                self.status_message = format!("❌ Probe failed for '{model_id}': {e}");
            }
        }

        self.is_probing = false;
    }

    fn sync_models_to_config(&mut self) {
        let configured: Vec<String> = self
            .models
            .iter()
            .filter(|m| m.in_config)
            .map(|m| m.id.clone())
            .collect();
        self.config.models = configured.clone();

        // Persist to file if file path is set
        let path = std::path::Path::new(&self.config_path);
        let mut file_config = if path.exists() {
            if let Ok(raw) = std::fs::read_to_string(path) {
                serde_json::from_str::<FileConfig>(&raw).unwrap_or_default()
            } else {
                FileConfig::default()
            }
        } else {
            FileConfig::default()
        };

        file_config.models = Some(configured);
        if let Ok(json) = serde_json::to_string_pretty(&file_config) {
            let _ = std::fs::write(path, json);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_model_navigation_and_toggle() {
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join("test_probelm_tui_cfg.json");
        let path_str = path.to_string_lossy().to_string();

        let cfg = Config {
            base_url: "http://localhost:20128".to_string(),
            api_key: "sk-test".to_string(),
            models: vec!["gpt-4o".to_string()],
            default_prompt: "OK".to_string(),
            prompts: std::collections::HashMap::new(),
            max_tokens: 64,
            temperature: 0.0,
            timeout_secs: 10,
            config_path: None,
        };
        let discovered = vec![ModelEntry {
            id: "claude-3-5".to_string(),
            object: None,
            owned_by: None,
            capabilities: Default::default(),
        }];

        let mut app = App::new(cfg, path_str, discovered);
        assert_eq!(app.models.len(), 2);
        assert!(app.models[0].in_config);

        app.select_next();
        assert_eq!(app.selected_index, 1);
        app.select_prev();
        assert_eq!(app.selected_index, 0);

        // Add custom model
        app.input_buffer = "deepseek-v3".to_string();
        app.submit_add_model();
        assert_eq!(app.models[0].id, "deepseek-v3");
        assert!(app.models[0].in_config);

        let _ = std::fs::remove_file(&path);
    }
}
