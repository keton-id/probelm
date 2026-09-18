pub mod oauth;
pub mod openai;

pub use oauth::{scan_local_oauth_providers, DetectedOAuthSession, SessionStatus};
pub use openai::OpenAiAdapter;

use serde::{Deserialize, Serialize};

/// Supported provider/connector kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AdapterKind {
    OpenAiKey,
    ClaudeCodeOAuth,
    GitHubCopilotOAuth,
    Custom,
}

/// Authentication and identity details for a provider adapter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthDetails {
    pub kind: AdapterKind,
    pub identifier: String,
    pub active: bool,
    pub description: String,
}

/// Common trait representing an LLM connector or gateway adapter.
pub trait ProviderAdapter: Send + Sync {
    fn id(&self) -> &str;
    fn display_name(&self) -> &str;
    fn adapter_kind(&self) -> AdapterKind;
    fn auth_details(&self) -> AuthDetails;
}
