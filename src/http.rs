//! Nazar HTTP API handlers (axum).

use axum::extract::State;
use axum::response::Json;
use axum::routing::get;

use nazar_core::*;

/// Build the axum router with all API routes.
pub fn router(state: SharedState) -> axum::Router {
    axum::Router::new()
        .route("/health", get(api_health))
        .route("/v1/snapshot", get(api_snapshot))
        .route("/v1/alerts", get(api_alerts))
        .route("/v1/predict", get(api_predict))
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

async fn api_snapshot(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = read_state(&state);
    match &s.latest {
        Some(snap) => Json(serde_json::to_value(snap).unwrap_or_default()),
        None => Json(serde_json::json!({"error": "No snapshot available yet"})),
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
