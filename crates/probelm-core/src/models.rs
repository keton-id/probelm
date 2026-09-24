pub use probelm_proto::{normalize_model_name, Capabilities, ModelEntry};

use serde::Deserialize;
use serde_json::Value;

use crate::specs::enrich_capabilities;

#[derive(Deserialize)]
struct ModelsResponse {
    data: Vec<ModelEntry>,
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
        m.capabilities = enrich_capabilities(&m.id, m.capabilities.clone());
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
    let now = rfc3339_now();
    serde_json::json!({
        "baseUrl": base_url,
        "exportedAt": now,
        "modelCount": entries.len(),
        "providers": providers,
        "models": models,
    })
}

fn rfc3339_now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
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
