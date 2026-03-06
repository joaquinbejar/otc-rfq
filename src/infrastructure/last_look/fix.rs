//! # FIX Last-Look Client
//!
//! FIX protocol-based implementation of last-look confirmation.
//!
//! Uses QuoteStatusRequest (MsgType=a) for last-look requests.

use crate::domain::entities::quote::Quote;
use crate::domain::services::last_look::{LastLookResult, LastLookService, LastLookStats};
use crate::domain::value_objects::VenueId;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::time::Duration;
use tokio::sync::RwLock;

/// Configuration for FIX last-look client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixLastLookConfig {
    /// FIX session host.
    pub host: String,
    /// FIX session port.
    pub port: u16,
    /// Sender CompID.
    pub sender_comp_id: String,
    /// Target CompID.
    pub target_comp_id: String,
    /// Heartbeat interval.
    pub heartbeat_interval: Duration,
}

impl Default for FixLastLookConfig {
    fn default() -> Self {
        Self {
            host: "localhost".to_string(),
            port: 9878,
            sender_comp_id: "CLIENT".to_string(),
            target_comp_id: "SERVER".to_string(),
            heartbeat_interval: Duration::from_secs(30),
        }
    }
}

/// FIX protocol-based last-look client.
///
/// Sends QuoteStatusRequest (MsgType=a) messages for last-look confirmation.
pub struct FixLastLookClient {
    /// Configuration.
    config: FixLastLookConfig,
    /// Venues that require last-look.
    venues_requiring_last_look: RwLock<HashMap<String, bool>>,
    /// Stats per venue.
    stats: RwLock<HashMap<String, LastLookStats>>,
}

impl fmt::Debug for FixLastLookClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FixLastLookClient")
            .field("host", &self.config.host)
            .field("port", &self.config.port)
            .field("sender_comp_id", &self.config.sender_comp_id)
            .finish()
    }
}

impl FixLastLookClient {
    /// Creates a new FIX last-look client.
    #[must_use]
    pub fn new(config: FixLastLookConfig) -> Self {
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

    /// Returns the FIX session identifier.
    #[must_use]
    pub fn session_id(&self) -> String {
        format!(
            "{}->{}@{}:{}",
            self.config.sender_comp_id,
            self.config.target_comp_id,
            self.config.host,
            self.config.port
        )
    }
}

#[async_trait]
impl LastLookService for FixLastLookClient {
    async fn request(&self, quote: &Quote, timeout: Duration) -> LastLookResult {
        // TODO: Implement actual FIX communication
        // Would send QuoteStatusRequest (35=a) and await QuoteStatusReport (35=AI)
        // For now, simulate a timeout since we don't have a real FIX session
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
    fn fix_config_default() {
        let config = FixLastLookConfig::default();
        assert_eq!(config.host, "localhost");
        assert_eq!(config.port, 9878);
        assert_eq!(config.sender_comp_id, "CLIENT");
    }

    #[test]
    fn fix_client_debug() {
        let client = FixLastLookClient::new(FixLastLookConfig::default());
        let debug = format!("{:?}", client);
        assert!(debug.contains("FixLastLookClient"));
    }

    #[test]
    fn fix_session_id() {
        let client = FixLastLookClient::new(FixLastLookConfig::default());
        let session_id = client.session_id();
        assert!(session_id.contains("CLIENT"));
        assert!(session_id.contains("SERVER"));
    }

    #[tokio::test]
    async fn register_venue() {
        let client = FixLastLookClient::new(FixLastLookConfig::default());
        let venue = VenueId::new("test-venue");

        assert!(!client.requires_last_look(&venue));

        client.register_venue(&venue, true).await;
        assert!(client.requires_last_look(&venue));
    }
}
