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

    /// Run as MCP server (JSON-RPC over stdin/stdout)
    #[arg(long)]
    mcp: bool,

    /// Bind address for nazar's HTTP API (use 0.0.0.0 for external access)
    #[arg(long, default_value = "127.0.0.1")]
    bind: String,

    /// Port for nazar's own API
    #[arg(long, default_value = "8095")]
    port: u16,

    /// Path to SQLite database for metric persistence
    #[arg(long)]
    db_path: Option<String>,
}

fn main() {
    let cli = Cli::parse();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    tracing::info!("Nazar system monitor starting");

    // Load persisted config, then apply CLI overrides
    let mut config = NazarConfig::load();
    config.api_url = cli.api_url.clone();

    if let Some(path) = NazarConfig::config_path() {
        tracing::info!("Config: {}", path.display());
    }
    tracing::info!("Connecting to daimon at {}", config.api_url);

    let state = new_shared_state(config);

    // Open SQLite store
    let db_path = cli.db_path.clone().or_else(|| {
        std::env::var_os("HOME").map(|h| {
            std::path::PathBuf::from(h)
                .join(".local/share/nazar/metrics.db")
                .to_string_lossy()
                .to_string()
        })
    });

    let store = db_path.and_then(|p| {
        match nazar_store::MetricStore::open(std::path::Path::new(&p)) {
            Ok(s) => {
                tracing::info!("Database: {p}");
                // Prune old data on startup
                if let Ok(pruned) = s.prune_older_than(30)
                    && pruned > 0
                {
                    tracing::info!("Pruned {pruned} rows older than 30 days");
                }
                Some(s)
            }
            Err(e) => {
                tracing::warn!("Failed to open database at {p}: {e}");
                None
            }
        }
    });

    let store = store.map(|s| Arc::new(std::sync::Mutex::new(s)));

    let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");

    // Start the metrics collector (loads historical snapshots internally)
    let collector_state = Arc::clone(&state);
    let collector_store = store.clone();
    let collector_port = cli.port;
    let collector_handle = rt.spawn(collector::collector_loop(
        collector_state,
        collector_store,
        collector_port,
    ));

    if cli.mcp {
        tracing::info!("Running in MCP mode (stdio)");
        let mcp_state = Arc::clone(&state);
        rt.block_on(async {
            tokio::select! {
                _ = collector_handle => {
                    tracing::error!("Collector exited unexpectedly");
                }
                _ = nazar_mcp::transport::run_mcp_stdio(mcp_state) => {
                    tracing::info!("MCP transport closed");
                }
            }
        });
        return;
    }

    // Start the HTTP API
    let api_state = Arc::clone(&state);
    let bind = cli.bind.clone();
    let port = cli.port;
    let api_handle = rt.spawn(run_http_api(api_state, bind, port));

    if cli.headless {
        tracing::info!("Running in headless mode on port {}", cli.port);
        rt.block_on(async {
            tokio::select! {
                result = collector_handle => {
                    match result {
                        Ok(()) => tracing::error!("Collector exited unexpectedly"),
                        Err(e) => tracing::error!("Collector panicked: {e}"),
                    }
                }
                result = api_handle => {
                    match result {
                        Ok(()) => tracing::error!("HTTP API exited unexpectedly"),
                        Err(e) => tracing::error!("HTTP API panicked: {e}"),
                    }
                }
                _ = tokio::signal::ctrl_c() => {
                    tracing::info!("Shutting down");
                }
            }
        });
    } else {
        tracing::info!("Starting GUI");
        nazar_ui::run_app(Arc::clone(&state));
        tracing::info!("GUI closed, shutting down");
        rt.shutdown_timeout(std::time::Duration::from_secs(2));
    }
}

async fn run_http_api(state: SharedState, bind: String, port: u16) {
    let app = http::router(state);
    let addr = format!("{bind}:{port}");
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
