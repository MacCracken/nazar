//! Nazar MCP Server — exposes system monitoring as MCP tools
//!
//! 5 native tools that can be registered with daimon's MCP tool registry.

use serde::{Deserialize, Serialize};

/// MCP tool description (matches daimon's schema).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDescription {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
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
}
