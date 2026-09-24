//! `probelm describe`: machine-readable integration manifest.
//!
//! Harnesses (e.g. arsy-code) read this to discover how to drive `probelm`
//! without hard-coding assumptions: subprocess invocation, the `state.json`
//! contract, the NDJSON transition stream, and the MCP surface.

use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct DescribeManifest {
    /// Public command name.
    pub command: String,
    pub version: String,
    /// Probe states a model can be in (serde snake_case).
    pub probe_states: [&'static str; 4],
    pub watch: WatchContract,
    pub mcp: McpContract,
}

#[derive(Debug, Serialize)]
pub struct WatchContract {
    /// argv to spawn for the long-running probe loop.
    pub spawn: Vec<String>,
    /// Durable state file contract.
    pub state_file: StateFileContract,
    /// stdout contract (one NDJSON line per state transition).
    pub stdout: StdoutContract,
}

#[derive(Debug, Serialize)]
pub struct StateFileContract {
    pub default_path: String,
    pub schema: &'static str,
    pub schema_version: u32,
    /// Written every cycle via write-to-temp + rename (atomic replace).
    pub write_mode: &'static str,
}

#[derive(Debug, Serialize)]
pub struct StdoutContract {
    pub format: &'static str,
    /// Event kinds that may appear, one per line.
    pub events: [&'static str; 1],
    /// Emitted only when a model's state changes.
    pub transition_trigger: &'static str,
}

#[derive(Debug, Serialize)]
pub struct McpContract {
    pub spawn: Vec<String>,
    pub tools: [&'static str; 3],
    /// Resource URIs; `probelm://state` mirrors the watch `state.json`.
    pub resources: [&'static str; 1],
}

/// Emit the manifest as pretty JSON to stdout.
pub fn cmd_describe() -> i32 {
    let manifest = DescribeManifest {
        command: "probelm".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        probe_states: ["unknown", "healthy", "degraded", "unreachable"],
        watch: WatchContract {
            spawn: vec![
                "probelm".to_string(),
                "watch".to_string(),
                "--config".to_string(),
                "config.json".to_string(),
                "--interval".to_string(),
                "30".to_string(),
                "--state".to_string(),
                "state.json".to_string(),
            ],
            state_file: StateFileContract {
                default_path: "state.json".to_string(),
                schema: "probelm_proto::state::StateFile",
                schema_version: probelm_proto::STATE_SCHEMA_VERSION,
                write_mode: "atomic (temp file + rename)",
            },
            stdout: StdoutContract {
                format: "ndjson",
                events: ["transition"],
                transition_trigger: "a model's ProbeState changed since the previous cycle",
            },
        },
        mcp: McpContract {
            spawn: vec![
                "probelm".to_string(),
                "mcp".to_string(),
                "serve".to_string(),
            ],
            tools: ["list_models", "probe_models", "sync_specs"],
            resources: [crate::mcp::STATE_RESOURCE_URI],
        },
    };
    match serde_json::to_string_pretty(&manifest) {
        Ok(json) => {
            println!("{json}");
            0
        }
        Err(e) => {
            eprintln!("ERROR: serialize manifest: {e}");
            2
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_is_valid_json_with_expected_contract() {
        let json = serde_json::to_value(DescribeManifest {
            command: "probelm".into(),
            version: "test".into(),
            probe_states: ["unknown", "healthy", "degraded", "unreachable"],
            watch: WatchContract {
                spawn: vec!["probelm".into(), "watch".into()],
                state_file: StateFileContract {
                    default_path: "state.json".into(),
                    schema: "x",
                    schema_version: 1,
                    write_mode: "atomic",
                },
                stdout: StdoutContract {
                    format: "ndjson",
                    events: ["transition"],
                    transition_trigger: "state change",
                },
            },
            mcp: McpContract {
                spawn: vec!["probelm".into(), "mcp".into(), "serve".into()],
                tools: ["list_models", "probe_models", "sync_specs"],
                resources: ["probelm://state"],
            },
        })
        .unwrap();
        assert_eq!(json["watch"]["state_file"]["schema_version"], 1);
        assert_eq!(json["watch"]["stdout"]["format"], "ndjson");
        assert_eq!(json["mcp"]["resources"][0], "probelm://state");
        assert_eq!(json["probe_states"].as_array().unwrap().len(), 4);
    }
}
