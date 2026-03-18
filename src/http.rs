//! Nazar HTTP API handlers (axum).

use std::fmt::Write;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};
use axum::routing::{get, post};
use tower_http::cors::{Any, CorsLayer};

use nazar_core::*;

/// Build the axum router with all API routes.
pub fn router(state: SharedState) -> axum::Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    axum::Router::new()
        .route("/health", get(api_health))
        .route("/v1/snapshot", get(api_snapshot))
        .route("/v1/alerts", get(api_alerts))
        .route("/v1/predict", get(api_predict))
        .route("/v1/processes", get(api_processes))
        .route("/v1/correlations", get(api_correlations))
        .route("/metrics", get(api_prometheus))
        .route("/v1/mcp/call", post(api_mcp_call))
        .layer(cors)
        .with_state(state)
}

async fn api_health(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = read_state(&state);
    Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "uptime_secs": (chrono::Utc::now() - s.started_at).num_seconds(),
        "has_snapshot": s.latest.is_some(),
        "samples": s.cpu_history.points.len(),
    }))
}

async fn api_snapshot(State(state): State<SharedState>) -> impl IntoResponse {
    let s = read_state(&state);
    match &s.latest {
        Some(snap) => match serde_json::to_value(snap) {
            Ok(val) => (StatusCode::OK, Json(val)),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Serialization failed: {e}")})),
            ),
        },
        None => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "No snapshot available yet"})),
        ),
    }
}

async fn api_alerts(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = read_state(&state);
    Json(serde_json::json!({
        "count": s.alerts.len(),
        "alerts": s.alerts,
    }))
}

async fn api_predict(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = read_state(&state);
    Json(serde_json::json!({
        "predictions": s.predictions,
    }))
}

async fn api_processes(State(state): State<SharedState>) -> impl IntoResponse {
    let s = read_state(&state);
    match &s.latest {
        Some(snap) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "count": snap.top_processes.len(),
                "processes": snap.top_processes,
            })),
        ),
        None => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "No snapshot available yet"})),
        ),
    }
}

async fn api_correlations(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = read_state(&state);
    Json(serde_json::json!({
        "count": s.correlations.len(),
        "correlations": s.correlations,
    }))
}

async fn api_prometheus(State(state): State<SharedState>) -> impl IntoResponse {
    let s = read_state(&state);
    let Some(snap) = &s.latest else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            [(axum::http::header::CONTENT_TYPE, "text/plain")],
            String::new(),
        );
    };

    /// Escape a string for use as a Prometheus label value.
    fn prom_escape(s: &str) -> String {
        s.replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
    }

    let mut out = String::with_capacity(4096);

    // CPU
    let _ = writeln!(
        out,
        "# HELP nazar_cpu_usage_percent Current CPU usage percentage"
    );
    let _ = writeln!(out, "# TYPE nazar_cpu_usage_percent gauge");
    let _ = writeln!(out, "nazar_cpu_usage_percent {:.2}", snap.cpu.total_percent);

    let _ = writeln!(out, "# HELP nazar_load_average Load average");
    let _ = writeln!(out, "# TYPE nazar_load_average gauge");
    let _ = writeln!(
        out,
        "nazar_load_average{{period=\"1m\"}} {:.2}",
        snap.cpu.load_average[0]
    );
    let _ = writeln!(
        out,
        "nazar_load_average{{period=\"5m\"}} {:.2}",
        snap.cpu.load_average[1]
    );
    let _ = writeln!(
        out,
        "nazar_load_average{{period=\"15m\"}} {:.2}",
        snap.cpu.load_average[2]
    );

    // Memory
    let _ = writeln!(out, "# HELP nazar_memory_used_bytes Memory used in bytes");
    let _ = writeln!(out, "# TYPE nazar_memory_used_bytes gauge");
    let _ = writeln!(out, "nazar_memory_used_bytes {}", snap.memory.used_bytes);

    let _ = writeln!(out, "# HELP nazar_memory_total_bytes Total memory in bytes");
    let _ = writeln!(out, "# TYPE nazar_memory_total_bytes gauge");
    let _ = writeln!(out, "nazar_memory_total_bytes {}", snap.memory.total_bytes);

    let _ = writeln!(out, "# HELP nazar_swap_used_bytes Swap used in bytes");
    let _ = writeln!(out, "# TYPE nazar_swap_used_bytes gauge");
    let _ = writeln!(out, "nazar_swap_used_bytes {}", snap.memory.swap_used_bytes);

    // Disk
    let _ = writeln!(out, "# HELP nazar_disk_used_bytes Disk space used in bytes");
    let _ = writeln!(out, "# TYPE nazar_disk_used_bytes gauge");
    for d in &snap.disk {
        let _ = writeln!(
            out,
            "nazar_disk_used_bytes{{mount=\"{}\",device=\"{}\"}} {}",
            prom_escape(&d.mount_point),
            prom_escape(&d.device),
            d.used_bytes
        );
    }

    let _ = writeln!(
        out,
        "# HELP nazar_disk_total_bytes Total disk space in bytes"
    );
    let _ = writeln!(out, "# TYPE nazar_disk_total_bytes gauge");
    for d in &snap.disk {
        let _ = writeln!(
            out,
            "nazar_disk_total_bytes{{mount=\"{}\",device=\"{}\"}} {}",
            prom_escape(&d.mount_point),
            prom_escape(&d.device),
            d.total_bytes
        );
    }

    let _ = writeln!(out, "# HELP nazar_disk_read_bytes Disk read bytes (delta)");
    let _ = writeln!(out, "# TYPE nazar_disk_read_bytes gauge");
    for d in &snap.disk {
        let _ = writeln!(
            out,
            "nazar_disk_read_bytes{{mount=\"{}\"}} {}",
            prom_escape(&d.mount_point),
            d.read_bytes
        );
    }

    let _ = writeln!(
        out,
        "# HELP nazar_disk_write_bytes Disk write bytes (delta)"
    );
    let _ = writeln!(out, "# TYPE nazar_disk_write_bytes gauge");
    for d in &snap.disk {
        let _ = writeln!(
            out,
            "nazar_disk_write_bytes{{mount=\"{}\"}} {}",
            prom_escape(&d.mount_point),
            d.write_bytes
        );
    }

    // Network
    let _ = writeln!(
        out,
        "# HELP nazar_network_rx_bytes Network bytes received (delta)"
    );
    let _ = writeln!(out, "# TYPE nazar_network_rx_bytes gauge");
    let _ = writeln!(
        out,
        "nazar_network_rx_bytes {}",
        snap.network.total_rx_bytes
    );

    let _ = writeln!(
        out,
        "# HELP nazar_network_tx_bytes Network bytes transmitted (delta)"
    );
    let _ = writeln!(out, "# TYPE nazar_network_tx_bytes gauge");
    let _ = writeln!(
        out,
        "nazar_network_tx_bytes {}",
        snap.network.total_tx_bytes
    );

    let _ = writeln!(
        out,
        "# HELP nazar_network_connections Active TCP connections"
    );
    let _ = writeln!(out, "# TYPE nazar_network_connections gauge");
    let _ = writeln!(
        out,
        "nazar_network_connections {}",
        snap.network.active_connections
    );

    // GPU
    if !snap.gpu.is_empty() {
        let _ = writeln!(
            out,
            "# HELP nazar_gpu_utilization_percent GPU utilization percentage"
        );
        let _ = writeln!(out, "# TYPE nazar_gpu_utilization_percent gauge");
        let _ = writeln!(
            out,
            "# HELP nazar_gpu_vram_used_bytes GPU VRAM used in bytes"
        );
        let _ = writeln!(out, "# TYPE nazar_gpu_vram_used_bytes gauge");
        let _ = writeln!(
            out,
            "# HELP nazar_gpu_vram_total_bytes GPU VRAM total in bytes"
        );
        let _ = writeln!(out, "# TYPE nazar_gpu_vram_total_bytes gauge");
        let _ = writeln!(out, "# HELP nazar_gpu_temperature_celsius GPU temperature");
        let _ = writeln!(out, "# TYPE nazar_gpu_temperature_celsius gauge");
        for g in &snap.gpu {
            let id = prom_escape(&g.id);
            let _ = writeln!(
                out,
                "nazar_gpu_utilization_percent{{id=\"{id}\"}} {:.1}",
                g.utilization_percent
            );
            let _ = writeln!(
                out,
                "nazar_gpu_vram_used_bytes{{id=\"{id}\"}} {}",
                g.vram_used_bytes
            );
            let _ = writeln!(
                out,
                "nazar_gpu_vram_total_bytes{{id=\"{id}\"}} {}",
                g.vram_total_bytes
            );
            if let Some(temp) = g.temp_celsius {
                let _ = writeln!(
                    out,
                    "nazar_gpu_temperature_celsius{{id=\"{id}\"}} {:.1}",
                    temp
                );
            }
        }
    }

    // Temperatures
    let _ = writeln!(
        out,
        "# HELP nazar_temperature_celsius Sensor temperature in celsius"
    );
    let _ = writeln!(out, "# TYPE nazar_temperature_celsius gauge");
    for t in &snap.temperatures {
        let _ = writeln!(
            out,
            "nazar_temperature_celsius{{label=\"{}\"}} {:.1}",
            prom_escape(&t.label),
            t.temp_celsius
        );
    }

    // Alerts count
    let _ = writeln!(out, "# HELP nazar_alerts_total Total active alerts");
    let _ = writeln!(out, "# TYPE nazar_alerts_total gauge");
    let _ = writeln!(out, "nazar_alerts_total {}", s.alerts.len());

    (
        StatusCode::OK,
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        out,
    )
}

/// MCP tool call callback — daimon dispatches tool calls here.
async fn api_mcp_call(
    State(state): State<SharedState>,
    Json(body): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    let name = body.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let arguments = body
        .get("arguments")
        .cloned()
        .unwrap_or(serde_json::json!({}));
    let result = nazar_mcp::execute_tool(name, &arguments, &state);
    Json(serde_json::json!({
        "content": [{"type": "text", "text": serde_json::to_string(&result.content).unwrap_or_default()}],
        "isError": result.is_error,
    }))
}
