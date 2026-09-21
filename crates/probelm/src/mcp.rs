use std::{
    collections::{HashMap, HashSet},
    io::IsTerminal,
    sync::Arc,
};

use inquire::Select;
use kurir::{Harness, RegistrationOptions, Scope, ServerSpec};
use rmcp::{
    handler::server::wrapper::Parameters,
    model::{Implementation, ServerCapabilities, ServerConfig},
    schemars::JsonSchema,
    service::RequestContext,
    tool, tool_router,
    transport::stdio,
    ErrorData as McpError, Json, RoleServer, ServerHandler, ServiceExt,
};
use serde::{Deserialize, Serialize};
use tokio::sync::Semaphore;

use crate::{
    config::Config,
    models::{self, Capabilities},
    probe::{self, ProbeOpts, ProbeResult},
};

#[derive(Clone)]
pub struct ProbelmServer {
    tool_router: rmcp::handler::server::router::tool::ToolRouter<Self>,
    config: Config,
}

impl ProbelmServer {
    pub fn new(config: Config) -> Self {
        Self {
            tool_router: Self::tool_router(),
            config,
        }
    }

    pub async fn serve_stdio(self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let service = self.serve(stdio()).await?;
        service.waiting().await?;
        Ok(())
    }
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct ListModelsRequest {
    #[serde(default)]
    pub prefix: Option<String>,
    #[serde(default)]
    pub owned_by: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ModelSummary {
    pub id: String,
    pub owned_by: Option<String>,
    pub capabilities: Capabilities,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ListModelsResponse {
    pub models: Vec<ModelSummary>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct ProbeModelsRequest {
    #[serde(default)]
    pub models: Vec<String>,
    #[serde(default)]
    pub all: bool,
    #[serde(default)]
    pub prompt: Option<String>,
    #[serde(default)]
    pub jobs: Option<usize>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ProbeFailure {
    pub model: String,
    pub error: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ProbeModelsResponse {
    pub results: Vec<ProbeResult>,
    pub failures: Vec<ProbeFailure>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SyncSpecsResponse {
    pub updated_models: usize,
}

#[tool_router(router = tool_router)]
impl ProbelmServer {
    #[tool(
        name = "list_models",
        description = "List models available through the configured OpenAI-compatible gateway."
    )]
    pub async fn list_models(
        &self,
        Parameters(request): Parameters<ListModelsRequest>,
    ) -> Result<Json<ListModelsResponse>, String> {
        let entries = models::fetch_models(
            &self.config.base_url,
            &self.config.api_key,
            self.config.timeout_secs,
        )
        .await?;
        let models = entries
            .into_iter()
            .filter(|model| {
                request.prefix.as_deref().is_none_or(|prefix| {
                    model.id.starts_with(prefix) || model.id.starts_with(&format!("{prefix}/"))
                })
            })
            .filter(|model| {
                request
                    .owned_by
                    .as_deref()
                    .is_none_or(|owner| model.owned_by.as_deref() == Some(owner))
            })
            .map(|model| ModelSummary {
                id: model.id,
                owned_by: model.owned_by,
                capabilities: model.capabilities,
            })
            .collect();
        Ok(Json(ListModelsResponse { models }))
    }

    #[tool(
        name = "probe_models",
        description = "Probe selected gateway models for availability, latency, throughput, and capabilities."
    )]
    pub async fn probe_models(
        &self,
        Parameters(request): Parameters<ProbeModelsRequest>,
    ) -> Result<Json<ProbeModelsResponse>, String> {
        let entries = models::fetch_models(
            &self.config.base_url,
            &self.config.api_key,
            self.config.timeout_secs,
        )
        .await?;
        let capabilities: HashMap<String, Capabilities> = entries
            .iter()
            .map(|model| (model.id.clone(), model.capabilities.clone()))
            .collect();
        let discovered: Vec<String> = entries.into_iter().map(|model| model.id).collect();
        let targets = resolve_targets(&request, &discovered, &self.config.models);
        if targets.is_empty() {
            return Err("no models matched the request".into());
        }

        let jobs = request.jobs.unwrap_or(1).clamp(1, 32);
        let semaphore = Arc::new(Semaphore::new(jobs));
        let mut handles = Vec::with_capacity(targets.len());
        for model in targets {
            let prompt = request
                .prompt
                .clone()
                .or_else(|| self.config.prompts.get(&model).cloned())
                .unwrap_or_else(|| self.config.default_prompt.clone());
            let opts = ProbeOpts {
                base_url: self.config.base_url.clone(),
                api_key: self.config.api_key.clone(),
                prompt,
                max_tokens: self.config.max_tokens,
                temperature: self.config.temperature,
                timeout_secs: self.config.timeout_secs,
                do_ping: true,
                do_latency: true,
                reasoning_effort: self.config.reasoning_effort.clone(),
            };
            let permit = Arc::clone(&semaphore);
            handles.push(tokio::spawn(async move {
                let _permit = permit
                    .acquire_owned()
                    .await
                    .map_err(|error| error.to_string())?;
                let result = probe::probe_one(&model, &opts).await;
                Ok::<_, String>((model, result))
            }));
        }

        let mut response = ProbeModelsResponse {
            results: Vec::new(),
            failures: Vec::new(),
        };
        for handle in handles {
            let (model, result) = handle.await.map_err(|error| error.to_string())??;
            match result {
                Ok(mut result) => {
                    result.caps = capabilities.get(&model).cloned();
                    response.results.push(result);
                }
                Err(error) => response.failures.push(ProbeFailure { model, error }),
            }
        }
        Ok(Json(response))
    }

    #[tool(
        name = "sync_specs",
        description = "Refresh the local model capability database from the LiteLLM source."
    )]
    pub async fn sync_specs(&self) -> Result<Json<SyncSpecsResponse>, String> {
        let updated_models = crate::specs::sync_remote_specs().await?;
        Ok(Json(SyncSpecsResponse { updated_models }))
    }
}

#[rmcp::tool_handler(router = self.tool_router)]
impl ServerHandler for ProbelmServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("probelm", env!("CARGO_PKG_VERSION")))
    }

    async fn list_resources(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<rmcp::model::ListResourcesResult, McpError> {
        Ok(Default::default())
    }
}

fn resolve_targets(
    request: &ProbeModelsRequest,
    discovered: &[String],
    configured: &[String],
) -> Vec<String> {
    let raw = if request.models.is_empty() {
        if request.all || configured.is_empty() {
            discovered.to_vec()
        } else {
            configured.to_vec()
        }
    } else {
        request.models.clone()
    };
    let mut targets = Vec::new();
    for input in raw {
        let matches: Vec<String> = if input.contains('*') {
            discovered
                .iter()
                .filter(|id| crate::matches_pattern(&input, id))
                .cloned()
                .collect()
        } else {
            discovered
                .iter()
                .filter(|id| *id == &input || id.starts_with(&format!("{input}/")))
                .cloned()
                .collect()
        };
        if matches.is_empty() {
            targets.push(input);
        } else {
            targets.extend(matches);
        }
    }
    let mut seen = HashSet::new();
    targets.retain(|model| seen.insert(model.clone()));
    targets
}

pub fn install_harness(
    client: Option<&str>,
    name: &str,
    project: bool,
    force: bool,
    dry_run: bool,
) -> Result<kurir::RegistrationResult, String> {
    let harness = match client {
        Some(value) => value
            .parse::<Harness>()
            .map_err(|error| error.to_string())?,
        None => {
            if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
                return Err("interactive MCP installation requires a TTY; pass --client for non-interactive use".into());
            }
            Select::new(
                "Install probelm MCP for which harness?",
                Harness::ALL.to_vec(),
            )
            .prompt()
            .map_err(|error| format!("MCP installation cancelled: {error}"))?
        }
    };
    let options = RegistrationOptions {
        scope: if project { Scope::Project } else { Scope::User },
        cwd: std::env::current_dir().map_err(|error| format!("current directory: {error}"))?,
        force,
        dry_run,
        ..RegistrationOptions::default()
    };
    let spec = ServerSpec::stdio(name, "probelm", vec!["mcp".into(), "serve".into()]);
    kurir::register(harness, &spec, &options).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_configured_models_by_default() {
        let request = ProbeModelsRequest::default();
        let discovered = vec!["midas/glm".into(), "cx/gpt".into()];
        let configured = vec!["cx/gpt".into()];

        assert_eq!(
            resolve_targets(&request, &discovered, &configured),
            configured
        );
    }

    #[test]
    fn expands_wildcards_and_removes_duplicates() {
        let request = ProbeModelsRequest {
            models: vec!["cx/*".into(), "cx/gpt".into()],
            ..Default::default()
        };
        let discovered = vec!["cx/gpt".into(), "cx/other".into()];

        assert_eq!(
            resolve_targets(&request, &discovered, &[]),
            vec!["cx/gpt", "cx/other"]
        );
    }
}
