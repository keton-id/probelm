use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One entry from `GET /v1/models`.
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct ModelEntry {
    pub id: String,
    pub object: Option<String>,
    pub owned_by: Option<String>,
    #[serde(default)]
    pub capabilities: Capabilities,
}

#[derive(Debug, Clone, Default, Deserialize, JsonSchema, Serialize)]
#[allow(non_snake_case)]
pub struct Capabilities {
    pub vision: bool,
    #[serde(default)]
    pub pdf: bool,
    #[serde(default)]
    pub search: bool,
    #[serde(default)]
    pub tools: bool,
    #[serde(default)]
    pub audioInput: bool,
    #[serde(default)]
    pub audioOutput: bool,
    #[serde(default)]
    pub videoInput: bool,
    #[serde(default)]
    pub imageOutput: bool,
    #[serde(default)]
    pub reasoning: bool,
    pub thinkingFormat: Option<String>,
    pub contextWindow: Option<u64>,
    pub maxOutput: Option<u64>,
}

impl Capabilities {
    /// Format token counts to human-readable k/m notation (e.g. 200k, 1m).
    pub fn format_tokens(n: Option<u64>) -> String {
        match n {
            None => "-".to_string(),
            Some(0) => "-".to_string(),
            Some(v) => {
                if v >= 1_000_000 {
                    let m = v as f64 / 1_000_000.0;
                    if m.fract().abs() < 0.05 {
                        format!("{:.0}m", m)
                    } else {
                        format!("{:.1}m", m)
                    }
                } else if v >= 1_000 {
                    let k = (v as f64 / 1_000.0).round() as u64;
                    format!("{k}k")
                } else {
                    format!("{v}")
                }
            }
        }
    }

    /// Extract icons representing active capabilities.
    pub fn icons(&self) -> String {
        let mut list = Vec::new();
        if self.reasoning {
            list.push("🧠");
        }
        if self.vision {
            list.push("👁");
        }
        if self.tools {
            list.push("🛠");
        }
        if self.pdf {
            list.push("📄");
        }
        if self.search {
            list.push("🔍");
        }
        if self.audioInput || self.audioOutput {
            list.push("🎙");
        }
        if self.videoInput {
            list.push("🎬");
        }
        if self.imageOutput {
            list.push("🎨");
        }
        if list.is_empty() {
            "-".to_string()
        } else {
            list.join(" ")
        }
    }

    /// Compact single-line representation used in the table.
    #[allow(dead_code)]
    pub fn compact(&self) -> String {
        let icons = self.icons();
        let ctx = Self::format_tokens(self.contextWindow);
        let out = Self::format_tokens(self.maxOutput);
        format!("{icons} ctx:{ctx} out:{out}")
    }

    /// Capability names present (lowercase), for `--cap` filtering.
    pub fn names(&self) -> Vec<String> {
        let mut v = Vec::new();
        for (name, on) in [
            ("vision", self.vision),
            ("pdf", self.pdf),
            ("search", self.search),
            ("tools", self.tools),
            ("audio", self.audioInput || self.audioOutput),
            ("video", self.videoInput),
            ("image", self.imageOutput),
            ("reasoning", self.reasoning),
        ] {
            if on {
                v.push(name.to_string());
            }
        }
        v
    }
}

/// Raw response from `GET /v1/models`.
#[derive(Deserialize)]
pub struct ModelsResponse {
    pub data: Vec<ModelEntry>,
}

/// Fetch all models from the gateway.
pub async fn fetch_models(
    base_url: &str,
    api_key: &str,
    timeout: u64,
) -> Result<Vec<ModelEntry>, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(timeout))
        .build()
        .map_err(|e| format!("http client: {e}"))?;

    let url = format!("{base_url}/v1/models");
    let resp = client
        .get(&url)
        .header("Authorization", format!("Bearer {api_key}"))
        .send()
        .await
        .map_err(|e| format!("GET {url}: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("GET {url}: HTTP {}", resp.status()));
    }

    let text = resp.text().await.map_err(|e| format!("read body: {e}"))?;
    let parsed: ModelsResponse =
        serde_json::from_str(&text).map_err(|e| format!("parse models: {e}"))?;
    let mut data = parsed.data;
    for m in &mut data {
        m.capabilities = crate::specs::enrich_capabilities(&m.id, m.capabilities.clone());
    }
    Ok(data)
}

/// Pretty JSON for the `--list-models` export (mirrors the bash tool's format).
pub fn export_json(entries: &[ModelEntry], base_url: &str) -> Value {
    let mut providers: serde_json::Map<String, Value> = serde_json::Map::new();
    for m in entries {
        let owner = m.owned_by.clone().unwrap_or_else(|| "unknown".into());
        providers
            .entry(owner)
            .or_insert_with(|| Value::Array(Vec::new()))
            .as_array_mut()
            .unwrap()
            .push(Value::String(m.id.clone()));
    }
    let models: Vec<String> = entries.iter().map(|m| m.id.clone()).collect();
    let now = chrono_now();
    serde_json::json!({
        "baseUrl": base_url,
        "exportedAt": now,
        "modelCount": entries.len(),
        "providers": providers,
        "models": models,
    })
}

fn chrono_now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // naive civil date from epoch (no chrono dependency)
    // days-from-epoch via Howard Hinnant's algorithm
    let z = (secs / 86400) as i64;
    let s = secs % 86400;
    let (hh, m) = (s / 3600, s % 3600);
    let (mm, ss) = (m / 60, m % 60);
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mth = if mp < 10 { mp + 3 } else { mp - 9 };
    let yr = if mth <= 2 { y + 1 } else { y };
    format!("{yr:04}-{mth:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
}
