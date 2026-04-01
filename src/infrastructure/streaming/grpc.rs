//! # gRPC Streaming Client
//!
//! gRPC-based implementation for receiving streaming quotes from MMs.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::Duration;

/// Configuration for gRPC streaming client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrpcStreamingConfig {
    /// gRPC endpoint URL.
    pub endpoint: String,
    /// Connection timeout.
    pub connect_timeout: Duration,
    /// Whether to use TLS.
    pub use_tls: bool,
    /// Keep-alive interval.
    pub keep_alive_interval: Duration,
}

impl Default for GrpcStreamingConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://localhost:50051".to_string(),
            connect_timeout: Duration::from_secs(5),
            use_tls: false,
            keep_alive_interval: Duration::from_secs(30),
        }
    }
}

/// gRPC-based streaming quote client.
///
/// Receives streaming quotes from MMs via bidirectional gRPC streaming
/// and forwards them to the composite service for aggregation.
pub struct GrpcStreamingClient {
    config: GrpcStreamingConfig,
}

impl fmt::Debug for GrpcStreamingClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GrpcStreamingClient")
            .field("endpoint", &self.config.endpoint)
            .finish()
    }
}

impl GrpcStreamingClient {
    /// Creates a new gRPC streaming client.
    #[must_use]
    pub fn new(config: GrpcStreamingConfig) -> Self {
        Self { config }
    }

    /// Returns the configuration.
    #[must_use]
    pub fn config(&self) -> &GrpcStreamingConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grpc_config_default() {
        let config = GrpcStreamingConfig::default();
        assert!(config.endpoint.contains("localhost"));
        assert!(!config.use_tls);
    }

    #[test]
    fn grpc_client_creation() {
        let client = GrpcStreamingClient::new(GrpcStreamingConfig::default());
        let debug = format!("{:?}", client);
        assert!(debug.contains("GrpcStreamingClient"));
    }
}
