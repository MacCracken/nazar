//! MCP stdio transport — JSON-RPC 2.0 over stdin/stdout.

use nazar_core::SharedState;
use serde_json::json;
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::{execute_tool, tool_definitions};

/// Run the MCP server over stdin/stdout. Reads JSON-RPC requests line by line
/// from stdin, dispatches to tool handlers, writes responses to stdout.
/// Tracing goes to stderr so stdout stays clean for JSON-RPC.
pub async fn run_mcp_stdio(state: SharedState) {
    let stdin = tokio::io::stdin();
    let reader = BufReader::new(stdin);
    let mut lines = reader.lines();

    while let Ok(Some(raw_line)) = lines.next_line().await {
        let line = raw_line.trim().to_string();
        if line.is_empty() {
            continue;
        }

        let request: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                let err = json_rpc_error(None, -32700, &format!("Parse error: {e}"));
                println!("{}", serde_json::to_string(&err).unwrap_or_default());
                continue;
            }
        };

        let method = request.get("method").and_then(|v| v.as_str()).unwrap_or("");
        let id = request.get("id").cloned();

        // Notifications (no id) don't get a response
        if method == "notifications/initialized" || method == "notifications/cancelled" {
            continue;
        }

        let response = match method {
            "initialize" => json_rpc_response(id, handle_initialize()),
            "tools/list" => json_rpc_response(id, handle_tools_list()),
            "tools/call" => {
                let params = request.get("params").cloned().unwrap_or(json!({}));
                json_rpc_response(id, handle_tools_call(&params, &state))
            }
            _ => json_rpc_error(id, -32601, &format!("Method not found: {method}")),
        };

        println!("{}", serde_json::to_string(&response).unwrap_or_default());
    }
}

fn handle_initialize() -> serde_json::Value {
    json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {
            "tools": {}
        },
        "serverInfo": {
            "name": "nazar",
            "version": env!("CARGO_PKG_VERSION")
        }
    })
}

fn handle_tools_list() -> serde_json::Value {
    let tools: Vec<serde_json::Value> = tool_definitions()
        .into_iter()
        .map(|t| json!({
            "name": t.name,
            "description": t.description,
            "inputSchema": t.input_schema,
        }))
        .collect();
    json!({ "tools": tools })
}

fn handle_tools_call(params: &serde_json::Value, state: &SharedState) -> serde_json::Value {
    let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let arguments = params.get("arguments").cloned().unwrap_or(json!({}));
    let result = execute_tool(name, &arguments, state);

    json!({
        "content": [{
            "type": "text",
            "text": serde_json::to_string_pretty(&result.content).unwrap_or_default()
        }],
        "isError": result.is_error
    })
}

fn json_rpc_response(id: Option<serde_json::Value>, result: serde_json::Value) -> serde_json::Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    })
}

fn json_rpc_error(id: Option<serde_json::Value>, code: i32, message: &str) -> serde_json::Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use nazar_core::{NazarConfig, new_shared_state};

    #[test]
    fn initialize_response() {
        let resp = handle_initialize();
        assert_eq!(resp["protocolVersion"], "2024-11-05");
        assert_eq!(resp["serverInfo"]["name"], "nazar");
    }

    #[test]
    fn tools_list_returns_five() {
        let resp = handle_tools_list();
        let tools = resp["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 5);
    }

    #[test]
    fn tools_call_unknown() {
        let state = new_shared_state(NazarConfig::default());
        let resp = handle_tools_call(&json!({"name": "unknown"}), &state);
        assert!(resp["isError"].as_bool().unwrap());
    }

    #[test]
    fn tools_call_config_get() {
        let state = new_shared_state(NazarConfig::default());
        let resp = handle_tools_call(&json!({
            "name": "nazar_config",
            "arguments": {"action": "get"}
        }), &state);
        assert!(!resp["isError"].as_bool().unwrap());
    }

    #[test]
    fn json_rpc_error_format() {
        let err = json_rpc_error(Some(json!(1)), -32601, "Not found");
        assert_eq!(err["jsonrpc"], "2.0");
        assert_eq!(err["error"]["code"], -32601);
    }
}
