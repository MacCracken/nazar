use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::State;
use axum::response::Json;
use axum::routing::get;
use clap::Parser;

use nazar_ai::AnomalyDetector;
use nazar_api::ProcReader;
use nazar_core::*;

#[derive(Parser)]
#[command(name = "nazar", about = "AI-native system monitor for AGNOS")]
struct Cli {
    /// AGNOS daimon API endpoint
    #[arg(long, default_value = "http://127.0.0.1:8090")]
    api_url: String,

    /// Run in headless mode (API only, no GUI)
    #[arg(long)]
    headless: bool,

    /// Port for nazar's own API
    #[arg(long, default_value = "8095")]
    port: u16,
}

fn main() {
    let cli = Cli::parse();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    tracing::info!("Nazar system monitor starting");
    tracing::info!("Connecting to daimon at {}", cli.api_url);

    let config = NazarConfig {
        api_url: cli.api_url.clone(),
        ..NazarConfig::default()
    };

    let state = new_shared_state(config);

    let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");

    // Start the metrics collector in a background task
    let collector_state = Arc::clone(&state);
    rt.spawn(collector_loop(collector_state));

    // Start the HTTP API in a background task
    let api_state = Arc::clone(&state);
    let port = cli.port;
    rt.spawn(run_http_api(api_state, port));

    if cli.headless {
        tracing::info!("Running in headless mode on port {}", cli.port);
        rt.block_on(async {
            tokio::signal::ctrl_c().await.ok();
            tracing::info!("Shutting down");
        });
    } else {
        tracing::info!("Starting GUI");
        nazar_ui::run_app(Arc::clone(&state));
    }
}

// ---------------------------------------------------------------------------
// Metrics collector
// ---------------------------------------------------------------------------

async fn collector_loop(state: SharedState) {
    let mut reader = ProcReader::new();
    let mut detector = AnomalyDetector::new();

    // Take an initial reading so the next one can compute CPU deltas
    let _warmup = reader.read_cpu();

    let poll_secs = {
        let s = state.read().unwrap();
        s.config.poll_interval_secs
    };

    let mut interval = tokio::time::interval(std::time::Duration::from_secs(poll_secs));
    tracing::info!("Collector started (poll every {poll_secs}s)");

    loop {
        interval.tick().await;

        let agents = AgentSummary {
            total: 0,
            running: 0,
            idle: 0,
            error: 0,
            cpu_usage: HashMap::new(),
            memory_usage: HashMap::new(),
        };

        let snapshot = reader.snapshot(agents, vec![]);

        // Feed the anomaly detector
        let alerts = detector.check(&snapshot);
        detector.record(snapshot.clone());

        // Update predictions
        let predictions = detector
            .predict_memory_exhaustion()
            .into_iter()
            .collect::<Vec<_>>();

        // Write to shared state
        {
            let mut s = state.write().unwrap();
            s.cpu_history.push(snapshot.cpu.total_percent);
            s.mem_history.push(snapshot.memory.used_percent());
            s.net_rx_history.push(snapshot.network.total_rx_bytes as f64);
            s.net_tx_history.push(snapshot.network.total_tx_bytes as f64);

            let max = s.config.max_history_points;
            for disk in &snapshot.disk {
                s.disk_history
                    .entry(disk.mount_point.clone())
                    .or_insert_with(|| TimeSeries::new(&disk.mount_point, "%", max))
                    .push(disk.used_percent());
            }

            if !alerts.is_empty() {
                tracing::warn!("{} alert(s) detected", alerts.len());
                for a in &alerts {
                    tracing::warn!("[{}] {}: {}", a.severity, a.component, a.message);
                }
                s.push_alerts(alerts);
            }

            s.predictions = predictions;
            s.latest = Some(snapshot);
        }
    }
}

// ---------------------------------------------------------------------------
// HTTP API (axum)
// ---------------------------------------------------------------------------

async fn run_http_api(state: SharedState, port: u16) {
    let app = axum::Router::new()
        .route("/health", get(api_health))
        .route("/v1/snapshot", get(api_snapshot))
        .route("/v1/alerts", get(api_alerts))
        .route("/v1/predict", get(api_predict))
        .with_state(state);

    let addr = format!("0.0.0.0:{port}");
    tracing::info!("HTTP API listening on {addr}");

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind HTTP API port");
    axum::serve(listener, app).await.ok();
}

async fn api_health(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = state.read().unwrap();
    Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "uptime_secs": (chrono::Utc::now() - s.started_at).num_seconds(),
        "has_snapshot": s.latest.is_some(),
        "samples": s.cpu_history.points.len(),
    }))
}

async fn api_snapshot(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = state.read().unwrap();
    match &s.latest {
        Some(snap) => Json(serde_json::to_value(snap).unwrap_or_default()),
        None => Json(serde_json::json!({"error": "No snapshot available yet"})),
    }
}

async fn api_alerts(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = state.read().unwrap();
    Json(serde_json::json!({
        "count": s.alerts.len(),
        "alerts": s.alerts,
    }))
}

async fn api_predict(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = state.read().unwrap();
    Json(serde_json::json!({
        "predictions": s.predictions,
    }))
}
