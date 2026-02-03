//! # OTC RFQ Engine
//!
//! Main entry point for the OTC RFQ service.
//!
//! This binary starts the OTC RFQ engine with:
//! - gRPC server for RFQ service
//! - REST/HTTP server for API endpoints
//! - WebSocket support for real-time streaming
//! - Graceful shutdown handling
//!
//! # Configuration
//!
//! Configuration is loaded from:
//! 1. Default values
//! 2. Configuration file (config.toml or `OTC_RFQ_CONFIG_FILE`)
//! 3. Environment variables (prefixed with `OTC_RFQ_`)
//!
//! # Usage
//!
//! ```bash
//! # Run with defaults
//! cargo run --bin otc-rfq
//!
//! # Run with custom ports
//! OTC_RFQ_GRPC_PORT=50052 OTC_RFQ_REST_PORT=8081 cargo run --bin otc-rfq
//!
//! # Run with pretty logging
//! OTC_RFQ_LOG_FORMAT=pretty cargo run --bin otc-rfq
//! ```

use anyhow::Context;
use std::sync::Arc;
use tokio::signal;
use tokio::sync::watch;
use tracing::{error, info, warn};

mod config;

use config::{AppConfig, LogFormat};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load configuration
    let config = AppConfig::load().context("Failed to load configuration")?;
    config.validate().context("Invalid configuration")?;

    // Initialize tracing based on configuration
    init_tracing(&config);

    info!(
        version = env!("CARGO_PKG_VERSION"),
        environment = %config.environment,
        service = %config.service_name,
        "Starting OTC RFQ Engine"
    );

    // Create shutdown signal channel
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    // Initialize repositories (using in-memory implementations for now)
    let rfq_repository = create_rfq_repository();
    let venue_repository = create_venue_repository();
    let trade_repository = create_trade_repository();

    // Start servers
    let grpc_handle = start_grpc_server(&config, Arc::clone(&rfq_repository), shutdown_rx.clone());
    let rest_handle = start_rest_server(
        &config,
        Arc::clone(&rfq_repository),
        Arc::clone(&venue_repository),
        Arc::clone(&trade_repository),
        shutdown_rx.clone(),
    );

    info!(
        grpc_addr = %format!("{}:{}", config.grpc.host, config.grpc.port),
        rest_addr = %format!("{}:{}", config.rest.host, config.rest.port),
        "OTC RFQ Engine started successfully"
    );

    // Wait for shutdown signal
    wait_for_shutdown().await;

    info!("Shutdown signal received, initiating graceful shutdown...");

    // Signal all tasks to shutdown
    let _ = shutdown_tx.send(true);

    // Wait for servers to finish with timeout
    let shutdown_timeout = tokio::time::Duration::from_secs(30);
    let shutdown_result = tokio::time::timeout(shutdown_timeout, async {
        let _ = tokio::join!(grpc_handle, rest_handle);
    })
    .await;

    match shutdown_result {
        Ok(()) => info!("Graceful shutdown completed"),
        Err(_) => warn!("Shutdown timeout exceeded, forcing exit"),
    }

    info!("OTC RFQ Engine stopped");
    Ok(())
}

/// Initializes the tracing subscriber based on configuration.
fn init_tracing(config: &AppConfig) {
    use tracing_subscriber::EnvFilter;

    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&config.log.level));

    match config.log.format {
        LogFormat::Json => {
            tracing_subscriber::fmt()
                .with_env_filter(filter)
                .json()
                .with_target(config.log.include_target)
                .init();
        }
        LogFormat::Pretty => {
            tracing_subscriber::fmt()
                .with_env_filter(filter)
                .pretty()
                .with_target(config.log.include_target)
                .init();
        }
    }
}

/// Creates an in-memory RFQ repository.
fn create_rfq_repository() -> Arc<dyn otc_rfq::application::use_cases::create_rfq::RfqRepository> {
    Arc::new(InMemoryRfqRepository::new())
}

/// Creates an in-memory venue repository.
fn create_venue_repository() -> Arc<dyn otc_rfq::api::rest::handlers::VenueRepository> {
    Arc::new(InMemoryVenueRepository::new())
}

/// Creates an in-memory trade repository.
fn create_trade_repository() -> Arc<dyn otc_rfq::api::rest::handlers::TradeRepository> {
    Arc::new(InMemoryTradeRepository::new())
}

/// Starts the gRPC server.
fn start_grpc_server(
    config: &AppConfig,
    rfq_repository: Arc<dyn otc_rfq::application::use_cases::create_rfq::RfqRepository>,
    mut shutdown_rx: watch::Receiver<bool>,
) -> tokio::task::JoinHandle<()> {
    let addr = match config.grpc.socket_addr() {
        Ok(a) => a,
        Err(e) => {
            error!(error = %e, "Invalid gRPC address");
            return tokio::spawn(async {});
        }
    };

    tokio::spawn(async move {
        use otc_rfq::api::grpc::RfqServiceImpl;
        use otc_rfq::api::grpc::proto::rfq_service_server::RfqServiceServer;
        use tonic::transport::Server;

        let service = RfqServiceImpl::new(rfq_repository);

        info!(addr = %addr, "Starting gRPC server");

        let server = Server::builder()
            .add_service(RfqServiceServer::new(service))
            .serve_with_shutdown(addr, async move {
                let _ = shutdown_rx.changed().await;
            });

        if let Err(e) = server.await {
            error!(error = %e, "gRPC server error");
        }

        info!("gRPC server stopped");
    })
}

/// Starts the REST/HTTP server.
fn start_rest_server(
    config: &AppConfig,
    rfq_repository: Arc<dyn otc_rfq::application::use_cases::create_rfq::RfqRepository>,
    venue_repository: Arc<dyn otc_rfq::api::rest::handlers::VenueRepository>,
    trade_repository: Arc<dyn otc_rfq::api::rest::handlers::TradeRepository>,
    mut shutdown_rx: watch::Receiver<bool>,
) -> tokio::task::JoinHandle<()> {
    let addr = match config.rest.socket_addr() {
        Ok(a) => a,
        Err(e) => {
            error!(error = %e, "Invalid REST address");
            return tokio::spawn(async {});
        }
    };

    tokio::spawn(async move {
        use otc_rfq::api::rest::handlers::AppState;
        use otc_rfq::api::rest::routes::create_router;

        let state = Arc::new(AppState {
            rfq_repository,
            venue_repository,
            trade_repository,
        });

        let router = create_router(state);

        info!(addr = %addr, "Starting REST server");

        let listener = match tokio::net::TcpListener::bind(addr).await {
            Ok(l) => l,
            Err(e) => {
                error!(error = %e, "Failed to bind REST server");
                return;
            }
        };

        let server = axum::serve(listener, router).with_graceful_shutdown(async move {
            let _ = shutdown_rx.changed().await;
        });

        if let Err(e) = server.await {
            error!(error = %e, "REST server error");
        }

        info!("REST server stopped");
    })
}

/// Waits for shutdown signals (SIGTERM, SIGINT).
async fn wait_for_shutdown() {
    let ctrl_c = async {
        if let Err(e) = signal::ctrl_c().await {
            error!(error = %e, "Failed to install Ctrl+C handler");
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match signal::unix::signal(signal::unix::SignalKind::terminate()) {
            Ok(mut sig) => {
                sig.recv().await;
            }
            Err(e) => {
                error!(error = %e, "Failed to install SIGTERM handler");
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => info!("Received Ctrl+C"),
        () = terminate => info!("Received SIGTERM"),
    }
}

// ============================================================================
// In-Memory Repository Implementations
// ============================================================================

use otc_rfq::api::rest::handlers::{TradeFilter, TradeRepository, VenueRepository};
use otc_rfq::application::use_cases::create_rfq::RfqRepository;
use otc_rfq::domain::entities::rfq::Rfq;
use otc_rfq::domain::entities::trade::Trade;
use otc_rfq::domain::entities::venue::Venue;
use otc_rfq::domain::value_objects::{RfqId, TradeId, VenueId};
use std::collections::HashMap;
use tokio::sync::RwLock;

/// In-memory RFQ repository for development/testing.
#[derive(Debug)]
struct InMemoryRfqRepository {
    rfqs: RwLock<HashMap<RfqId, Rfq>>,
}

impl InMemoryRfqRepository {
    fn new() -> Self {
        Self {
            rfqs: RwLock::new(HashMap::new()),
        }
    }
}

#[async_trait::async_trait]
impl RfqRepository for InMemoryRfqRepository {
    async fn save(&self, rfq: &Rfq) -> Result<(), String> {
        let mut rfqs = self.rfqs.write().await;
        rfqs.insert(rfq.id(), rfq.clone());
        Ok(())
    }

    async fn find_by_id(&self, id: RfqId) -> Result<Option<Rfq>, String> {
        let rfqs = self.rfqs.read().await;
        Ok(rfqs.get(&id).cloned())
    }
}

/// In-memory venue repository for development/testing.
#[derive(Debug)]
struct InMemoryVenueRepository {
    venues: RwLock<HashMap<VenueId, Venue>>,
}

impl InMemoryVenueRepository {
    fn new() -> Self {
        Self {
            venues: RwLock::new(HashMap::new()),
        }
    }
}

#[async_trait::async_trait]
impl VenueRepository for InMemoryVenueRepository {
    async fn find_all(&self) -> Result<Vec<Venue>, String> {
        let venues = self.venues.read().await;
        Ok(venues.values().cloned().collect())
    }

    async fn find_by_id(&self, id: &VenueId) -> Result<Option<Venue>, String> {
        let venues = self.venues.read().await;
        Ok(venues.get(id).cloned())
    }

    async fn save(&self, venue: &Venue) -> Result<(), String> {
        let mut venues = self.venues.write().await;
        venues.insert(venue.id().clone(), venue.clone());
        Ok(())
    }
}

/// In-memory trade repository for development/testing.
#[derive(Debug)]
struct InMemoryTradeRepository {
    trades: RwLock<HashMap<TradeId, Trade>>,
}

impl InMemoryTradeRepository {
    fn new() -> Self {
        Self {
            trades: RwLock::new(HashMap::new()),
        }
    }
}

#[async_trait::async_trait]
impl TradeRepository for InMemoryTradeRepository {
    async fn find_all(&self, _filter: &TradeFilter) -> Result<Vec<Trade>, String> {
        let trades = self.trades.read().await;
        Ok(trades.values().cloned().collect())
    }

    async fn find_by_id(&self, id: TradeId) -> Result<Option<Trade>, String> {
        let trades = self.trades.read().await;
        Ok(trades.get(&id).cloned())
    }
}
