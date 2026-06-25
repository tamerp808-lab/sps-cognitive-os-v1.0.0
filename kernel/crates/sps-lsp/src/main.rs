//! SPS LSP server entry point — runs on stdio.

use std::sync::Arc;
use sps_code_intel::index::CodebaseIndex;

#[tokio::main]
async fn main() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new("warn"))
        .try_init();
    let index = Arc::new(CodebaseIndex::new());
    tracing::info!("Starting SPS Language Server on stdio");
    sps_lsp::server::run_stdio(index).await;
}
