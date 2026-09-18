use crate::core::config::FileConfig;
use std::io::{self, Write};
use std::path::Path;

pub fn prompt_input(prompt: &str, default: Option<&str>) -> String {
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

pub fn update_config_value(config_path: &str, field: &str, value: &str) -> Result<String, String> {
    let p = Path::new(config_path);
    let mut file_config = if p.exists() {
        let raw = std::fs::read_to_string(p).map_err(|e| format!("read {}: {e}", p.display()))?;
        serde_json::from_str::<FileConfig>(&raw).map_err(|e| format!("parse: {e}"))?
    } else {
        FileConfig::default()
    };

    let msg =
        match field {
            "url" | "base-url" => {
                let mut ep = file_config
                    .endpoint
                    .unwrap_or(crate::core::config::EndpointCfg {
                        base_url: None,
                        api_key: None,
                    });
                ep.base_url = Some(value.trim_end_matches('/').to_string());
                file_config.endpoint = Some(ep);
                format!("Updated base_url to '{value}'")
            }
            "key" | "api-key" => {
                let mut ep = file_config
                    .endpoint
                    .unwrap_or(crate::core::config::EndpointCfg {
                        base_url: None,
                        api_key: None,
                    });
                ep.api_key = Some(value.to_string());
                file_config.endpoint = Some(ep);
                "Updated api_key".to_string()
            }
            "add-model" => {
                let mut models = file_config.models.unwrap_or_default();
                if !models.contains(&value.to_string()) {
                    models.push(value.to_string());
                }
                file_config.models = Some(models);
                format!("Added model '{value}' to config")
            }
            "remove-model" => {
                let mut models = file_config.models.unwrap_or_default();
                models.retain(|m| m != value);
                file_config.models = Some(models);
                format!("Removed model '{value}' from config")
            }
            _ => return Err(format!(
                "Unknown configuration field '{field}'. Valid: url, key, add-model, remove-model"
            )),
        };

    let json = serde_json::to_string_pretty(&file_config).map_err(|e| format!("serialize: {e}"))?;
    std::fs::write(p, json).map_err(|e| format!("write {}: {e}", p.display()))?;
    Ok(msg)
}

pub fn run_single_update_wizard(config_path: &str) -> Result<(), String> {
    println!("--- probelm TTY Quick Config ---");
    let options = vec![
        "Add a model",
        "Remove a model",
        "Update Gateway Base URL",
        "Update Gateway API Key",
        "Exit",
    ];

    let choice = inquire::Select::new("Select field to update:", options)
        .prompt()
        .map_err(|e| format!("prompt error: {e}"))?;

    match choice {
        "Add a model" => {
            let model = prompt_input("Model ID to add", None);
            if !model.is_empty() {
                let res = update_config_value(config_path, "add-model", &model)?;
                println!("✓ {res}");
            }
        }
        "Remove a model" => {
            let model = prompt_input("Model ID to remove", None);
            if !model.is_empty() {
                let res = update_config_value(config_path, "remove-model", &model)?;
                println!("✓ {res}");
            }
        }
        "Update Gateway Base URL" => {
            let url = prompt_input("New Gateway Base URL", Some("http://localhost:20128"));
            if !url.is_empty() {
                let res = update_config_value(config_path, "url", &url)?;
                println!("✓ {res}");
            }
        }
        "Update Gateway API Key" => {
            let key = prompt_input("New Gateway API Key", None);
            if !key.is_empty() {
                let res = update_config_value(config_path, "key", &key)?;
                println!("✓ {res}");
            }
        }
        _ => {}
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_update_config_value() {
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join("test_probelm_tty_cfg.json");
        let path_str = path.to_string_lossy().to_string();

        let res = update_config_value(&path_str, "url", "http://localhost:9000");
        assert!(res.is_ok());

        let res = update_config_value(&path_str, "add-model", "test-model-1");
        assert!(res.is_ok());

        let res = update_config_value(&path_str, "remove-model", "test-model-1");
        assert!(res.is_ok());

        let _ = std::fs::remove_file(&path);
    }
}
