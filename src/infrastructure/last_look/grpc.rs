//! # gRPC Last-Look Client
//!
//! gRPC-based implementation of last-look confirmation.

use crate::domain::entities::quote::Quote;
use crate::domain::services::last_look::{LastLookResult, LastLookService, LastLookStats};
use crate::domain::value_objects::VenueId;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::time::Duration;
use tokio::sync::RwLock;

/// Configuration for gRPC last-look client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrpcLastLookConfig {
    /// gRPC endpoint URL.
    pub endpoint: String,
    /// Connection timeout.
    pub connect_timeout: Duration,
    /// Request timeout (deadline).
    pub request_timeout: Duration,
    /// Whether to use TLS.
    pub use_tls: bool,
}

impl Default for GrpcLastLookConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://localhost:50051".to_string(),
            connect_timeout: Duration::from_secs(5),
            request_timeout: Duration::from_millis(200),
            use_tls: false,
        }
    }
}

/// gRPC-based last-look client.
///
/// Sends last-look requests via gRPC unary calls with deadline.
pub struct GrpcLastLookClient {
    /// Configuration.
    config: GrpcLastLookConfig,
    /// Venues that require last-look.
    venues_requiring_last_look: RwLock<HashMap<String, bool>>,
    /// Stats per venue.
    stats: RwLock<HashMap<String, LastLookStats>>,
}

impl fmt::Debug for GrpcLastLookClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GrpcLastLookClient")
            .field("endpoint", &self.config.endpoint)
            .finish()
    }
}

impl GrpcLastLookClient {
    /// Creates a new gRPC last-look client.
    #[must_use]
    pub fn new(config: GrpcLastLookConfig) -> Self {
        Self {
            config,
            venues_requiring_last_look: RwLock::new(HashMap::new()),
            stats: RwLock::new(HashMap::new()),
        }
    }

    /// Registers a venue as requiring last-look.
    pub async fn register_venue(&self, venue_id: &VenueId, requires: bool) {
        let mut guard = self.venues_requiring_last_look.write().await;
        guard.insert(venue_id.to_string(), requires);
    }
}

#[async_trait]
impl LastLookService for GrpcLastLookClient {
    async fn request(&self, quote: &Quote, timeout: Duration) -> LastLookResult {
        // TODO: Implement actual gRPC communication using tonic
        // For now, simulate a timeout since we don't have a real connection
        let effective_timeout = timeout.min(self.config.request_timeout);
        let _ = tokio::time::sleep(effective_timeout).await;
        LastLookResult::timeout(quote.id(), effective_timeout)
    }

    fn requires_last_look(&self, venue_id: &VenueId) -> bool {
        // Use try_read for sync method - returns false if lock is contended
        self.venues_requiring_last_look
            .try_read()
            .ok()
            .and_then(|guard| guard.get(&venue_id.to_string()).copied())
            .unwrap_or(false)
    }

    async fn get_stats(&self, venue_id: &VenueId) -> Option<LastLookStats> {
        let guard = self.stats.read().await;
        guard.get(&venue_id.to_string()).cloned()
    }

    async fn record_result(&self, venue_id: &VenueId, result: &LastLookResult) {
        let mut guard = self.stats.write().await;
        let stats = guard
            .entry(venue_id.to_string())
            .or_insert_with(LastLookStats::new);
        match result {
            LastLookResult::Confirmed { .. } => stats.record_confirmation(),
            LastLookResult::Rejected { .. } => stats.record_rejection(),
            LastLookResult::Timeout { .. } => stats.record_timeout(),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn grpc_config_default() {
        let config = GrpcLastLookConfig::default();
        assert!(config.endpoint.contains("localhost"));
        assert_eq!(config.request_timeout, Duration::from_millis(200));
        assert!(!config.use_tls);
    }

    #[test]
    fn grpc_client_debug() {
        let client = GrpcLastLookClient::new(GrpcLastLookConfig::default());
        let debug = format!("{:?}", client);
        assert!(debug.contains("GrpcLastLookClient"));
    }

    #[tokio::test]
    async fn register_venue() {
        let client = GrpcLastLookClient::new(GrpcLastLookConfig::default());
        let venue = VenueId::new("test-venue");

        assert!(!client.requires_last_look(&venue));

        client.register_venue(&venue, true).await;
        assert!(client.requires_last_look(&venue));
    }
}
