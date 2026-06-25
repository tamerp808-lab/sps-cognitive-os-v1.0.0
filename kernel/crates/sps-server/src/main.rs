//! SPS HTTP server entry point.
//!
//! Usage: `sps-server [--db PATH] [--listen ADDR] [--web DIR]`

use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use sps_server::{run, ServerConfig};

#[derive(Parser, Debug)]
#[command(
    name = "sps-server",
    version,
    about = "SPS Cognitive Operating System — HTTP API server",
    long_about = "Exposes the SPS kernel via a REST + SSE API for the web UI."
)]
struct Cli {
    /// Path to the SQLite database file. Use `:memory:` for ephemeral.
    #[arg(long, default_value = "~/.sps/sps.db")]
    db: PathBuf,

    /// Listen address.
    #[arg(long, default_value = "127.0.0.1:7780")]
    listen: String,

    /// Static web UI directory (Next.js build output).
    #[arg(long)]
    web: Option<PathBuf>,

    /// Verbose logging.
    #[arg(short, long)]
    verbose: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            if cli.verbose {
                tracing_subscriber::EnvFilter::new("debug")
            } else {
                tracing_subscriber::EnvFilter::new("info")
            },
        )
        .try_init();

    let listen_addr: std::net::SocketAddr = cli
        .listen
        .parse()
        .map_err(|e| anyhow::anyhow!("invalid listen address: {}", e))?;

    let config = ServerConfig {
        db_path: cli.db,
        listen_addr,
        web_dir: cli.web,
    };

    tracing::info!("Starting SPS server — db: {}, listen: {}", config.db_path.display(), config.listen_addr);
    run(config).await
}
