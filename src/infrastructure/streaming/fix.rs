//! # FIX Streaming Client
//!
//! FIX protocol-based implementation for receiving streaming quotes (MassQuote).

use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::Duration;

/// Configuration for FIX streaming client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixStreamingConfig {
    /// FIX session sender comp ID.
    pub sender_comp_id: String,
    /// FIX session target comp ID.
    pub target_comp_id: String,
    /// FIX version (e.g., "FIX.4.4").
    pub fix_version: String,
    /// Heartbeat interval.
    pub heartbeat_interval: Duration,
}

impl Default for FixStreamingConfig {
    fn default() -> Self {
        Self {
            sender_comp_id: "OTC_PLATFORM".to_string(),
            target_comp_id: "MARKET_MAKER".to_string(),
            fix_version: "FIX.4.4".to_string(),
            heartbeat_interval: Duration::from_secs(30),
        }
    }
}

/// FIX protocol-based streaming quote client.
///
/// Receives MassQuote (i) messages from MMs via FIX protocol
/// and forwards them to the composite service for aggregation.
pub struct FixStreamingClient {
    config: FixStreamingConfig,
}

impl fmt::Debug for FixStreamingClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FixStreamingClient")
            .field("sender", &self.config.sender_comp_id)
            .field("target", &self.config.target_comp_id)
            .finish()
    }
}

impl FixStreamingClient {
    /// Creates a new FIX streaming client.
    #[must_use]
    pub fn new(config: FixStreamingConfig) -> Self {
        Self { config }
    }

    /// Returns the configuration.
    #[must_use]
    pub fn config(&self) -> &FixStreamingConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fix_config_default() {
        let config = FixStreamingConfig::default();
        assert_eq!(config.fix_version, "FIX.4.4");
    }

    #[test]
    fn fix_client_creation() {
        let client = FixStreamingClient::new(FixStreamingConfig::default());
        let debug = format!("{:?}", client);
        assert!(debug.contains("FixStreamingClient"));
    }
}
