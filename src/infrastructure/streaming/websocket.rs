//! # WebSocket Streaming Client
//!
//! WebSocket-based implementation for receiving streaming quotes from MMs.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::Duration;

/// Configuration for WebSocket streaming client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketStreamingConfig {
    /// WebSocket endpoint URL.
    pub endpoint: String,
    /// Connection timeout.
    pub connect_timeout: Duration,
    /// Whether to use TLS.
    pub use_tls: bool,
    /// Ping interval for keepalive.
    pub ping_interval: Duration,
}

impl Default for WebSocketStreamingConfig {
    fn default() -> Self {
        Self {
            endpoint: "ws://localhost:8080/streaming".to_string(),
            connect_timeout: Duration::from_secs(5),
            use_tls: false,
            ping_interval: Duration::from_secs(30),
        }
    }
}

/// WebSocket-based streaming quote client.
///
/// Receives streaming quotes from MMs via WebSocket and forwards them
/// to the composite service for aggregation.
pub struct WebSocketStreamingClient {
    config: WebSocketStreamingConfig,
}

impl fmt::Debug for WebSocketStreamingClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WebSocketStreamingClient")
            .field("endpoint", &self.config.endpoint)
            .finish()
    }
}

impl WebSocketStreamingClient {
    /// Creates a new WebSocket streaming client.
    #[must_use]
    pub fn new(config: WebSocketStreamingConfig) -> Self {
        Self { config }
    }

    /// Returns the configuration.
    #[must_use]
    pub fn config(&self) -> &WebSocketStreamingConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn websocket_config_default() {
        let config = WebSocketStreamingConfig::default();
        assert!(config.endpoint.contains("localhost"));
        assert!(!config.use_tls);
    }

    #[test]
    fn websocket_client_creation() {
        let client = WebSocketStreamingClient::new(WebSocketStreamingConfig::default());
        let debug = format!("{:?}", client);
        assert!(debug.contains("WebSocketStreamingClient"));
    }
}
