//! Nazar MCP Server — exposes system monitoring as MCP tools
//!
//! 5 native tools with real backend handlers that read from shared MonitorState.

use nazar_core::*;
use serde::{Deserialize, Serialize};

/// MCP tool description (matches daimon's schema).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDescription {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

/// Result from executing an MCP tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub content: serde_json::Value,
    pub is_error: bool,
}

impl ToolResult {
    fn ok(value: serde_json::Value) -> Self {
        Self { content: value, is_error: false }
    }

    fn err(message: &str) -> Self {
        Self {
            content: serde_json::json!({ "error": message }),
            is_error: true,
        }
    }
}

/// Get the 5 Nazar MCP tool definitions.
pub fn tool_definitions() -> Vec<ToolDescription> {
    vec![
        ToolDescription {
            name: "nazar_dashboard".to_string(),
            description: "Get a system monitoring dashboard snapshot (CPU, memory, disk, agents)".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        ToolDescription {
            name: "nazar_alerts".to_string(),
            description: "Get current system anomaly alerts and warnings".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "severity": {"type": "string", "description": "Filter by severity: info, warning, critical"}
                },
                "required": []
            }),
        },
        ToolDescription {
            name: "nazar_predict".to_string(),
            description: "Predict resource exhaustion based on current trends".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "metric": {"type": "string", "description": "Metric to predict: memory, disk, cpu"}
                },
                "required": []
            }),
        },
        ToolDescription {
            name: "nazar_history".to_string(),
            description: "Get historical metrics for a specific resource".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "metric": {"type": "string", "description": "Metric: cpu, memory, disk, network"},
                    "points": {"type": "integer", "description": "Number of data points (default: 60)"}
                },
                "required": ["metric"]
            }),
        },
        ToolDescription {
            name: "nazar_config".to_string(),
            description: "Get or update Nazar monitor configuration".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "action": {"type": "string", "description": "Action: get or set"},
                    "key": {"type": "string", "description": "Config key"},
                    "value": {"type": "string", "description": "Config value (for set)"}
                },
                "required": ["action"]
            }),
        },
    ]
}

/// Execute an MCP tool by name against shared state.
pub fn execute_tool(
    name: &str,
    params: &serde_json::Value,
    state: &SharedState,
) -> ToolResult {
    match name {
        "nazar_dashboard" => handle_dashboard(state),
        "nazar_alerts" => handle_alerts(params, state),
        "nazar_predict" => handle_predict(params, state),
        "nazar_history" => handle_history(params, state),
        "nazar_config" => handle_config(params, state),
        _ => ToolResult::err(&format!("Unknown tool: {name}")),
    }
}

fn handle_dashboard(state: &SharedState) -> ToolResult {
    let s = read_state(state);
    match &s.latest {
        Some(snap) => ToolResult::ok(serde_json::json!({
            "timestamp": snap.timestamp.to_rfc3339(),
            "cpu": {
                "total_percent": snap.cpu.total_percent,
                "load_average": snap.cpu.load_average,
                "cores": snap.cpu.cores.len(),
            },
            "memory": {
                "used_percent": snap.memory.used_percent(),
                "used_gb": snap.memory.used_bytes as f64 / 1e9,
                "total_gb": snap.memory.total_bytes as f64 / 1e9,
            },
            "disk": snap.disk.iter().map(|d| serde_json::json!({
                "mount": d.mount_point,
                "used_percent": d.used_percent(),
                "used_gb": d.used_bytes as f64 / 1e9,
                "total_gb": d.total_bytes as f64 / 1e9,
            })).collect::<Vec<_>>(),
            "network": {
                "rx_mb": snap.network.total_rx_bytes as f64 / 1e6,
                "tx_mb": snap.network.total_tx_bytes as f64 / 1e6,
                "connections": snap.network.active_connections,
            },
            "agents": {
                "total": snap.agents.total,
                "running": snap.agents.running,
                "error": snap.agents.error,
            },
            "alerts_count": s.alerts.len(),
        })),
        None => ToolResult::err("No snapshot available yet"),
    }
}

fn handle_alerts(params: &serde_json::Value, state: &SharedState) -> ToolResult {
    let s = read_state(state);
    let severity_filter = params.get("severity").and_then(|v| v.as_str());

    let alerts: Vec<_> = s
        .alerts
        .iter()
        .filter(|a| {
            severity_filter.is_none_or(|f| {
                matches!(
                    (f, &a.severity),
                    ("info", AlertSeverity::Info)
                        | ("warning", AlertSeverity::Warning)
                        | ("critical", AlertSeverity::Critical)
                )
            })
        })
        .map(|a| {
            serde_json::json!({
                "severity": a.severity.to_string(),
                "component": a.component,
                "message": a.message,
                "timestamp": a.timestamp.to_rfc3339(),
            })
        })
        .collect();

    ToolResult::ok(serde_json::json!({
        "count": alerts.len(),
        "alerts": alerts,
    }))
}

fn handle_predict(_params: &serde_json::Value, state: &SharedState) -> ToolResult {
    let s = read_state(state);
    if s.predictions.is_empty() {
        return ToolResult::ok(serde_json::json!({
            "message": "Not enough data for predictions (need 10+ samples)",
            "predictions": [],
        }));
    }

    let preds: Vec<_> = s
        .predictions
        .iter()
        .map(|p| {
            let poll_secs = s.config.poll_interval_secs;
            serde_json::json!({
                "metric": p.metric,
                "current_percent": p.current_value,
                "target_percent": p.predicted_value,
                "minutes_until": (p.intervals_until * poll_secs) / 60,
                "trend": format!("{:?}", p.trend),
            })
        })
        .collect();

    ToolResult::ok(serde_json::json!({
        "predictions": preds,
    }))
}

fn handle_history(params: &serde_json::Value, state: &SharedState) -> ToolResult {
    let s = read_state(state);
    let metric = match params.get("metric").and_then(|v| v.as_str()) {
        Some(m) => m,
        None => return ToolResult::err("'metric' parameter is required"),
    };
    let n = params
        .get("points")
        .and_then(|v| v.as_u64())
        .unwrap_or(60) as usize;

    let series = match metric {
        "cpu" => Some(&s.cpu_history),
        "memory" => Some(&s.mem_history),
        "network_rx" => Some(&s.net_rx_history),
        "network_tx" => Some(&s.net_tx_history),
        _ => None,
    };

    match series {
        Some(ts) => {
            let points: Vec<_> = ts
                .points
                .iter()
                .rev()
                .take(n)
                .rev()
                .map(|p| {
                    serde_json::json!({
                        "timestamp": p.timestamp.to_rfc3339(),
                        "value": p.value,
                    })
                })
                .collect();
            ToolResult::ok(serde_json::json!({
                "metric": metric,
                "unit": ts.unit,
                "count": points.len(),
                "points": points,
            }))
        }
        None => {
            // Check disk series
            if let Some(mount) = metric.strip_prefix("disk:")
                && let Some(ts) = s.disk_history.get(mount)
            {
                    let points: Vec<_> = ts
                        .points
                        .iter()
                        .rev()
                        .take(n)
                        .rev()
                        .map(|p| {
                            serde_json::json!({
                                "timestamp": p.timestamp.to_rfc3339(),
                                "value": p.value,
                            })
                        })
                        .collect();
                    return ToolResult::ok(serde_json::json!({
                        "metric": metric,
                        "unit": ts.unit,
                        "count": points.len(),
                        "points": points,
                    }));
            }
            ToolResult::err(&format!(
                "Unknown metric '{metric}'. Available: cpu, memory, network_rx, network_tx, disk:<mount>"
            ))
        }
    }
}

fn handle_config(params: &serde_json::Value, state: &SharedState) -> ToolResult {
    let action = match params.get("action").and_then(|v| v.as_str()) {
        Some(a) => a,
        None => return ToolResult::err("'action' parameter is required"),
    };

    match action {
        "get" => {
            let s = read_state(state);
            ToolResult::ok(serde_json::json!({
                "api_url": s.config.api_url,
                "poll_interval_secs": s.config.poll_interval_secs,
                "max_history_points": s.config.max_history_points,
                "show_anomalies": s.config.show_anomalies,
                "show_agents": s.config.show_agents,
                "ui_refresh_ms": s.config.ui_refresh_ms,
                "cpu_threshold": s.config.cpu_threshold,
                "memory_threshold": s.config.memory_threshold,
                "disk_threshold": s.config.disk_threshold,
            }))
        }
        "set" => {
            let key = params.get("key").and_then(|v| v.as_str());
            let value = params.get("value").and_then(|v| v.as_str());
            match (key, value) {
                (Some(k), Some(v)) => {
                    let mut s = write_state(state);
                    match k {
                        "poll_interval_secs" => {
                            if let Ok(n) = v.parse::<u64>() {
                                s.config.poll_interval_secs = n;
                            } else {
                                return ToolResult::err("Invalid value for poll_interval_secs");
                            }
                        }
                        "show_anomalies" => {
                            s.config.show_anomalies = v == "true";
                        }
                        "show_agents" => {
                            s.config.show_agents = v == "true";
                        }
                        "ui_refresh_ms" => {
                            if let Ok(n) = v.parse::<u64>() {
                                s.config.ui_refresh_ms = n;
                            } else {
                                return ToolResult::err("Invalid value for ui_refresh_ms");
                            }
                        }
                        "cpu_threshold" | "memory_threshold" | "disk_threshold" => {
                            if let Ok(n) = v.parse::<f64>() {
                                match k {
                                    "cpu_threshold" => s.config.cpu_threshold = n,
                                    "memory_threshold" => s.config.memory_threshold = n,
                                    "disk_threshold" => s.config.disk_threshold = n,
                                    _ => unreachable!(),
                                }
                            } else {
                                return ToolResult::err(&format!("Invalid value for {k}"));
                            }
                        }
                        _ => return ToolResult::err(&format!("Unknown config key: {k}")),
                    }
                    ToolResult::ok(serde_json::json!({ "updated": k, "value": v }))
                }
                _ => ToolResult::err("'key' and 'value' are required for set action"),
            }
        }
        _ => ToolResult::err("Action must be 'get' or 'set'"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_definitions_count() {
        assert_eq!(tool_definitions().len(), 5);
    }

    #[test]
    fn tool_names() {
        let tools = tool_definitions();
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"nazar_dashboard"));
        assert!(names.contains(&"nazar_alerts"));
        assert!(names.contains(&"nazar_predict"));
        assert!(names.contains(&"nazar_history"));
        assert!(names.contains(&"nazar_config"));
    }

    #[test]
    fn tool_schemas_valid_json() {
        for tool in tool_definitions() {
            assert!(tool.input_schema.is_object());
        }
    }

    #[test]
    fn dashboard_no_snapshot() {
        let state = new_shared_state(NazarConfig::default());
        let result = execute_tool("nazar_dashboard", &serde_json::json!({}), &state);
        assert!(result.is_error);
    }

    #[test]
    fn alerts_empty() {
        let state = new_shared_state(NazarConfig::default());
        let result = execute_tool("nazar_alerts", &serde_json::json!({}), &state);
        assert!(!result.is_error);
        assert_eq!(result.content["count"], 0);
    }

    #[test]
    fn predict_not_enough_data() {
        let state = new_shared_state(NazarConfig::default());
        let result = execute_tool("nazar_predict", &serde_json::json!({}), &state);
        assert!(!result.is_error);
    }

    #[test]
    fn history_requires_metric() {
        let state = new_shared_state(NazarConfig::default());
        let result = execute_tool("nazar_history", &serde_json::json!({}), &state);
        assert!(result.is_error);
    }

    #[test]
    fn history_cpu() {
        let state = new_shared_state(NazarConfig::default());
        {
            let mut s = state.write().unwrap();
            s.cpu_history.push(42.0);
            s.cpu_history.push(55.0);
        }
        let result = execute_tool(
            "nazar_history",
            &serde_json::json!({"metric": "cpu", "points": 10}),
            &state,
        );
        assert!(!result.is_error);
        assert_eq!(result.content["count"], 2);
    }

    #[test]
    fn config_get() {
        let state = new_shared_state(NazarConfig::default());
        let result = execute_tool(
            "nazar_config",
            &serde_json::json!({"action": "get"}),
            &state,
        );
        assert!(!result.is_error);
        assert_eq!(result.content["poll_interval_secs"], 5);
    }

    #[test]
    fn config_set() {
        let state = new_shared_state(NazarConfig::default());
        let result = execute_tool(
            "nazar_config",
            &serde_json::json!({"action": "set", "key": "poll_interval_secs", "value": "10"}),
            &state,
        );
        assert!(!result.is_error);
        let s = state.read().unwrap();
        assert_eq!(s.config.poll_interval_secs, 10);
    }

    #[test]
    fn unknown_tool() {
        let state = new_shared_state(NazarConfig::default());
        let result = execute_tool("nazar_foo", &serde_json::json!({}), &state);
        assert!(result.is_error);
    }
}
