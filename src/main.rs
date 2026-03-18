mod collector;
mod http;

use std::sync::Arc;

use clap::Parser;

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

    // Start the metrics collector
    let collector_state = Arc::clone(&state);
    rt.spawn(collector::collector_loop(collector_state));

    // Start the HTTP API
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
        // GUI window closed — shut down cleanly
        tracing::info!("GUI closed, shutting down");
        rt.shutdown_timeout(std::time::Duration::from_secs(2));
    }
}

async fn run_http_api(state: SharedState, port: u16) {
    let app = http::router(state);
    let addr = format!("0.0.0.0:{port}");
    tracing::info!("HTTP API listening on {addr}");

    let listener = match tokio::net::TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => {
            tracing::error!("Failed to bind HTTP API on {addr}: {e}");
            return;
        }
    };
    axum::serve(listener, app).await.ok();
}
