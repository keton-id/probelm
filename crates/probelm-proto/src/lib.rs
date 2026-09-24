//! Pure data types and helpers for `probelm`.
//!
//! This crate intentionally contains no async code, no HTTP client, and no
//! filesystem access. It can be consumed by the async `probelm-core`, the TUI,
//! and by external harnesses such as `arsy-code` without pulling in a runtime.

use serde::{Deserialize, Serialize};

pub mod error;
pub mod state;

pub use error::ProbeError;
pub use state::{diff_snapshots, ProbeSnapshot, StateFile, WatchEvent, STATE_SCHEMA_VERSION};

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

/// Capabilities reported by a model endpoint.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[allow(non_snake_case)]
pub struct Capabilities {
    #[serde(default)]
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
        list.join(" ")
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

/// Result of probing one model. Fields omitted when a test is skipped.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ProbeResult {
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caps: Option<Capabilities>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ping: Option<PingOutcome>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency: Option<LatencyOutcome>,
    /// Optional per-probe error details. Omitted when the probe succeeds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state_error: Option<ProbeError>,
}

/// Outcome of a non-streaming ping probe.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct PingOutcome {
    pub ok: bool,
    pub http_code: u16,
}

/// Outcome of a streaming latency/throughput probe.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct LatencyOutcome {
    pub ttft_secs: Option<f64>,
    pub total_secs: Option<f64>,
    pub tokens: u64,
    pub rate_per_sec: Option<f64>,
}

/// Options for a single probe. This is a pure data struct and intentionally
/// has no defaults beyond those required by `Default`.
#[derive(Debug, Clone, Default)]
pub struct ProbeOpts {
    pub base_url: String,
    pub api_key: String,
    pub prompt: String,
    pub max_tokens: u32,
    pub temperature: f64,
    pub timeout_secs: u64,
    pub do_ping: bool,
    pub do_latency: bool,
    pub reasoning_effort: Option<String>,
}

impl ProbeResult {
    /// Convenience helper to determine the overall state from a probe result.
    pub fn state(&self) -> ProbeState {
        if self.state_error.is_some() || self.ping.as_ref().is_some_and(|p| !p.ok) {
            return ProbeState::Unreachable;
        }
        match (&self.ping, &self.latency) {
            (Some(p), _) if !p.ok => ProbeState::Unreachable,
            (Some(p), Some(l)) if p.ok && l.tokens == 0 && l.rate_per_sec.is_none() => {
                ProbeState::Degraded
            }
            (Some(p), _) if p.ok => ProbeState::Healthy,
            _ => ProbeState::Unknown,
        }
    }
}

/// High-level probe state of a model, derived from a [`ProbeResult`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ProbeState {
    #[default]
    Unknown,
    Healthy,
    Degraded,
    Unreachable,
}

/// Normalize a model name to a stable identifier used by `ModelSpec` lookups.
///
/// Currently lower-cases and collapses whitespace and common separators.
pub fn normalize_model_name(name: &str) -> String {
    name.to_lowercase()
        .replace([' ', '\t', '\n', '\r'], "-")
        .replace(['_', '/', ':'], "-")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_model_names() {
        assert_eq!(normalize_model_name("Foo Bar"), "foo-bar");
        assert_eq!(normalize_model_name("a_b/c:d"), "a-b-c-d");
        assert_eq!(normalize_model_name("Foo\tBar\n"), "foo-bar-");
    }

    #[test]
    fn healthy_state_when_ping_ok() {
        let result = ProbeResult {
            ping: Some(PingOutcome {
                ok: true,
                http_code: 200,
            }),
            ..Default::default()
        };
        assert_eq!(result.state(), ProbeState::Healthy);
    }

    #[test]
    fn unreachable_state_when_ping_fails() {
        let result = ProbeResult {
            ping: Some(PingOutcome {
                ok: false,
                http_code: 503,
            }),
            ..Default::default()
        };
        assert_eq!(result.state(), ProbeState::Unreachable);
    }
}
