//! # WebSocket Last-Look Client
//!
//! WebSocket-based implementation of last-look confirmation.

use crate::domain::entities::quote::Quote;
use crate::domain::services::last_look::{LastLookResult, LastLookService, LastLookStats};
use crate::domain::value_objects::VenueId;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
    /// Venues that require last-look.
    venues_requiring_last_look: RwLock<HashMap<String, bool>>,
    /// Stats per venue.
    stats: RwLock<HashMap<String, LastLookStats>>,
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
impl LastLookService for WebSocketLastLookClient {
    async fn request(&self, quote: &Quote, timeout: Duration) -> LastLookResult {
        // TODO: Implement actual WebSocket communication
        // For now, simulate a timeout since we don't have a real connection
        let _ = tokio::time::sleep(timeout).await;
        LastLookResult::timeout(quote.id(), timeout)
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

    #[tokio::test]
    async fn register_venue() {
        let client = WebSocketLastLookClient::new(WebSocketLastLookConfig::default());
        let venue = VenueId::new("test-venue");

        assert!(!client.requires_last_look(&venue));

        client.register_venue(&venue, true).await;
        assert!(client.requires_last_look(&venue));

        client.register_venue(&venue, false).await;
        assert!(!client.requires_last_look(&venue));
    }
}
