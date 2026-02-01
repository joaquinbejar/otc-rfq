//! # OTC RFQ Engine
//!
//! Main entry point for the OTC RFQ service.

use tracing::info;

mod config;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .json()
        .init();

    info!("Starting OTC RFQ Engine v{}", env!("CARGO_PKG_VERSION"));

    // TODO: Implement in M4 #51
    // - Load configuration
    // - Initialize dependencies
    // - Start gRPC server
    // - Start REST/WebSocket server
    // - Handle graceful shutdown

    info!("OTC RFQ Engine started successfully");

    // Keep the server running
    tokio::signal::ctrl_c().await?;
    info!("Shutting down OTC RFQ Engine");

    Ok(())
}
