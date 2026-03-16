use clap::Parser;

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

    let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");

    if cli.headless {
        tracing::info!("Running in headless mode on port {}", cli.port);
        rt.block_on(async {
            let _client = nazar_api::ApiClient::new(&cli.api_url);
            tracing::info!("Nazar headless mode ready (API client connected)");
            // TODO: start metrics collection loop
            tokio::signal::ctrl_c().await.ok();
        });
    } else {
        tracing::info!("Starting GUI");
        nazar_ui::run_app(&cli.api_url);
    }
}
