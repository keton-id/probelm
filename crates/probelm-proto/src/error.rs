//! Typed errors for `probelm`.
//!
//! `probelm-core` previously returned every failure as a `String`. This module
//! gives callers enough structure to decide whether a failure is transient,
//! permanent, recoverable, or worth waking an agent.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Errors that can occur while probing a model or interacting with probelm.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum ProbeError {
    /// The model or gateway could not be reached at all (DNS, TCP, TLS).
    Unreachable { reason: String },
    /// The probe timed out before completing.
    Timeout { reason: String },
    /// The gateway refused the request (HTTP 401/403/429).
    Refused { reason: String },
    /// The response could not be parsed or violated expectations.
    Malformed { reason: String },
    /// The request was denied by local policy or configuration.
    PolicyDenied { reason: String },
    /// A catch-all for errors that do not yet have a dedicated variant.
    Other { reason: String },
}

impl ProbeError {
    /// True when a retry could plausibly succeed.
    pub fn is_transient(&self) -> bool {
        matches!(
            self,
            ProbeError::Unreachable { .. } | ProbeError::Timeout { .. }
        )
    }

    /// True when the request was rejected by the provider.
    pub fn is_provider_fault(&self) -> bool {
        matches!(
            self,
            ProbeError::Refused { .. } | ProbeError::Malformed { .. }
        )
    }
}

impl fmt::Display for ProbeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProbeError::Unreachable { reason } => write!(f, "unreachable: {reason}"),
            ProbeError::Timeout { reason } => write!(f, "timeout: {reason}"),
            ProbeError::Refused { reason } => write!(f, "refused: {reason}"),
            ProbeError::Malformed { reason } => write!(f, "malformed: {reason}"),
            ProbeError::PolicyDenied { reason } => write!(f, "policy denied: {reason}"),
            ProbeError::Other { reason } => write!(f, "{reason}"),
        }
    }
}

impl std::error::Error for ProbeError {}
