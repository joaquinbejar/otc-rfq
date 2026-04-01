//! # WebSocket Last-Look Client
//!
//! WebSocket-based implementation of last-look confirmation.

use crate::domain::entities::quote::Quote;
use crate::domain::services::last_look::{LastLookResult, LastLookService, LastLookStats};
use crate::domain::value_objects::VenueId;
use async_trait::async_trait;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::Duration;
use tokio::sync::RwLock;

/// Configuration for WebSocket last-look client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketLastLookConfig {
    /// WebSocket endpoint URL.
    pub endpoint: String,
    /// Connection timeout.
    pub connect_timeout: Duration,
    /// Whether to use TLS.
    pub use_tls: bool,
}

impl Default for WebSocketLastLookConfig {
    fn default() -> Self {
        Self {
            endpoint: "ws://localhost:8080/last-look".to_string(),
            connect_timeout: Duration::from_secs(5),
            use_tls: false,
        }
    }
}

/// WebSocket-based last-look client.
///
/// Sends last-look requests via WebSocket and awaits responses with timeout.
pub struct WebSocketLastLookClient {
    /// Configuration.
    config: WebSocketLastLookConfig,
    /// Venues that require last-look (lock-free for sync access).
    venues_requiring_last_look: DashMap<String, bool>,
    /// Stats per venue.
    stats: RwLock<LastLookStats>,
}

impl fmt::Debug for WebSocketLastLookClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WebSocketLastLookClient")
            .field("endpoint", &self.config.endpoint)
            .finish()
    }
}

impl WebSocketLastLookClient {
    /// Creates a new WebSocket last-look client.
    #[must_use]
    pub fn new(config: WebSocketLastLookConfig) -> Self {
        Self {
            config,
            venues_requiring_last_look: DashMap::new(),
            stats: RwLock::new(LastLookStats::new()),
        }
    }

    /// Registers a venue as requiring last-look.
    pub fn register_venue(&self, venue_id: &VenueId, requires: bool) {
        self.venues_requiring_last_look
            .insert(venue_id.to_string(), requires);
    }
}

#[async_trait]
impl LastLookService for WebSocketLastLookClient {
    async fn request(&self, quote: &Quote, timeout: Duration) -> LastLookResult {
        // TODO: Implement actual WebSocket communication
        // For now, simulate a timeout since we don't have a real connection
        let _ = tokio::time::sleep(timeout).await;
        LastLookResult::timeout(quote.id(), timeout)
    }

    fn requires_last_look(&self, venue_id: &VenueId) -> bool {
        // DashMap provides lock-free reads, no contention issues
        self.venues_requiring_last_look
            .get(&venue_id.to_string())
            .map(|v| *v)
            .unwrap_or(false)
    }

    async fn get_stats(&self, _venue_id: &VenueId) -> Option<LastLookStats> {
        let guard = self.stats.read().await;
        Some(guard.clone())
    }

    async fn record_result(&self, _venue_id: &VenueId, result: &LastLookResult) {
        let mut guard = self.stats.write().await;
        match result {
            LastLookResult::Confirmed { .. } => guard.record_confirmation(),
            LastLookResult::Rejected { .. } => guard.record_rejection(),
            LastLookResult::Timeout { .. } => guard.record_timeout(),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn websocket_config_default() {
        let config = WebSocketLastLookConfig::default();
        assert!(config.endpoint.contains("localhost"));
        assert!(!config.use_tls);
    }

    #[test]
    fn websocket_client_debug() {
        let client = WebSocketLastLookClient::new(WebSocketLastLookConfig::default());
        let debug = format!("{:?}", client);
        assert!(debug.contains("WebSocketLastLookClient"));
    }

    #[test]
    fn register_venue() {
        let client = WebSocketLastLookClient::new(WebSocketLastLookConfig::default());
        let venue = VenueId::new("test-venue");

        assert!(!client.requires_last_look(&venue));

        client.register_venue(&venue, true);
        assert!(client.requires_last_look(&venue));

        client.register_venue(&venue, false);
        assert!(!client.requires_last_look(&venue));
    }
}
