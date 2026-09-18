use crate::adap::{AdapterKind, AuthDetails, ProviderAdapter};
use crate::core::models::{fetch_models, ModelEntry};
use crate::core::probe::{probe_one, ProbeOpts, ProbeResult};

/// Adapter for standard OpenAI-compatible gateways (e.g. 9Router, vLLM, LiteLLM, or direct OpenAI).
#[derive(Debug, Clone)]
pub struct OpenAiAdapter {
    pub base_url: String,
    pub api_key: String,
}

impl OpenAiAdapter {
    pub fn new(base_url: String, api_key: String) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
        }
    }

    pub async fn fetch_models(&self, timeout_secs: u64) -> Result<Vec<ModelEntry>, String> {
        fetch_models(&self.base_url, &self.api_key, timeout_secs).await
    }

    pub async fn probe(&self, model: &str, opts: &ProbeOpts) -> Result<ProbeResult, String> {
        let mut probe_opts = opts.clone();
        probe_opts.base_url = self.base_url.clone();
        probe_opts.api_key = self.api_key.clone();
        probe_one(model, &probe_opts).await
    }
}

impl ProviderAdapter for OpenAiAdapter {
    fn id(&self) -> &str {
        "openai-compatible"
    }

    fn display_name(&self) -> &str {
        "OpenAI Compatible Gateway"
    }

    fn adapter_kind(&self) -> AdapterKind {
        AdapterKind::OpenAiKey
    }

    fn auth_details(&self) -> AuthDetails {
        let masked = if self.api_key.len() <= 8 {
            "***".to_string()
        } else {
            format!(
                "{}...{}",
                &self.api_key[..4],
                &self.api_key[self.api_key.len() - 4..]
            )
        };
        AuthDetails {
            kind: AdapterKind::OpenAiKey,
            identifier: masked,
            active: !self.api_key.is_empty(),
            description: format!("Gateway at {}", self.base_url),
        }
    }
}
