//! Durable state snapshots and transition events for `probelm watch`.
//!
//! Pure types shared between the `probelm` CLI (writer) and external
//! harnesses such as `arsy-code` (reader). This module performs no I/O:
//! atomic file writing lives in `probelm-core::watch`.

use crate::{LatencyOutcome, PingOutcome, ProbeError, ProbeState};
use serde::{Deserialize, Serialize};

/// Schema version of the `state.json` file. Bump on breaking layout changes.
pub const STATE_SCHEMA_VERSION: u32 = 1;

/// One model's latest probe snapshot as recorded in `state.json`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ProbeSnapshot {
    pub model: String,
    pub state: ProbeState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ping: Option<PingOutcome>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency: Option<LatencyOutcome>,
    /// Present when the last cycle failed for this model.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ProbeError>,
    /// Unix epoch seconds when this snapshot was taken.
    pub updated_at_unix: u64,
}

/// Full view written atomically to `state.json` every watch cycle.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct StateFile {
    pub schema_version: u32,
    /// Unix epoch seconds when this file was generated.
    pub generated_at_unix: u64,
    pub interval_secs: u64,
    pub models: Vec<ProbeSnapshot>,
}

/// One NDJSON line emitted on stdout when a model's state changes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum WatchEvent {
    /// A model's state changed since the previous cycle.
    Transition {
        model: String,
        from: ProbeState,
        to: ProbeState,
        snapshot: ProbeSnapshot,
    },
}

/// Diff the previous snapshots against the new ones.
///
/// Emits one [`WatchEvent::Transition`] per model whose [`ProbeState`]
/// changed. Models absent from `prev` are treated as `Unknown`, so the
/// first cycle reports every model's initial state as a transition.
pub fn diff_snapshots(prev: &[ProbeSnapshot], cur: &[ProbeSnapshot]) -> Vec<WatchEvent> {
    let mut events = Vec::new();
    for snap in cur {
        let from = prev
            .iter()
            .find(|p| p.model == snap.model)
            .map(|p| p.state)
            .unwrap_or(ProbeState::Unknown);
        if from != snap.state {
            events.push(WatchEvent::Transition {
                model: snap.model.clone(),
                from,
                to: snap.state,
                snapshot: snap.clone(),
            });
        }
    }
    events
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PingOutcome, ProbeResult};

    fn snapshot(model: &str, state: ProbeState) -> ProbeSnapshot {
        ProbeSnapshot {
            model: model.to_string(),
            state,
            ping: Some(PingOutcome {
                ok: true,
                http_code: 200,
            }),
            latency: None,
            error: None,
            updated_at_unix: 1,
        }
    }

    #[test]
    fn first_cycle_emits_every_model_transition() {
        let cur = vec![
            snapshot("a", ProbeState::Healthy),
            snapshot("b", ProbeState::Unreachable),
        ];
        let events = diff_snapshots(&[], &cur);
        assert_eq!(events.len(), 2);
        match &events[0] {
            WatchEvent::Transition {
                model, from, to, ..
            } => {
                assert_eq!(model, "a");
                assert_eq!(*from, ProbeState::Unknown);
                assert_eq!(*to, ProbeState::Healthy);
            }
        }
    }

    #[test]
    fn no_transition_when_state_unchanged() {
        let prev = vec![snapshot("a", ProbeState::Healthy)];
        let cur = vec![snapshot("a", ProbeState::Healthy)];
        assert!(diff_snapshots(&prev, &cur).is_empty());
    }

    #[test]
    fn emits_transition_on_state_change() {
        let prev = vec![snapshot("a", ProbeState::Healthy)];
        let mut degraded = snapshot("a", ProbeState::Degraded);
        degraded.updated_at_unix = 2;
        let events = diff_snapshots(&prev, &[degraded]);
        assert_eq!(events.len(), 1);
        match &events[0] {
            WatchEvent::Transition {
                model,
                from,
                to,
                snapshot,
            } => {
                assert_eq!(model, "a");
                assert_eq!(*from, ProbeState::Healthy);
                assert_eq!(*to, ProbeState::Degraded);
                assert_eq!(snapshot.updated_at_unix, 2);
            }
        }
    }

    #[test]
    fn state_file_round_trips() {
        let file = StateFile {
            schema_version: STATE_SCHEMA_VERSION,
            generated_at_unix: 2,
            interval_secs: 30,
            models: vec![snapshot("a", ProbeState::Healthy)],
        };
        let json = serde_json::to_string(&file).unwrap();
        let back: StateFile = serde_json::from_str(&json).unwrap();
        assert_eq!(back, file);
    }

    #[test]
    fn probe_result_error_maps_to_unreachable() {
        let result = ProbeResult {
            state_error: Some(crate::ProbeError::Timeout {
                reason: "slow".into(),
            }),
            ..Default::default()
        };
        assert_eq!(result.state(), ProbeState::Unreachable);
    }
}
