//! # Configuration
//!
//! Application configuration loading and management.
//!
//! This module provides configuration structures and loading mechanisms
//! for the OTC RFQ service, supporting both environment variables and
//! configuration files.
//!
//! # Configuration Sources
//!
//! Configuration is loaded in the following order (later sources override earlier):
//! 1. Default values
//! 2. Configuration file (if exists)
//! 3. Environment variables (prefixed with `OTC_RFQ_`)
//!
//! # Environment Variables
//!
//! | Variable | Description | Default |
//! |----------|-------------|---------|
//! | `OTC_RFQ_GRPC_HOST` | gRPC server host | `0.0.0.0` |
//! | `OTC_RFQ_GRPC_PORT` | gRPC server port | `50051` |
//! | `OTC_RFQ_REST_HOST` | REST server host | `0.0.0.0` |
//! | `OTC_RFQ_REST_PORT` | REST server port | `8080` |
//! | `OTC_RFQ_LOG_LEVEL` | Log level | `info` |
//! | `OTC_RFQ_LOG_FORMAT` | Log format (json/pretty) | `json` |
//!
//! # Examples
//!
//! ```ignore
//! use otc_rfq::config::AppConfig;
//!
//! let config = AppConfig::load()?;
//! println!("gRPC server: {}:{}", config.grpc.host, config.grpc.port);
//! ```

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::path::Path;
use thiserror::Error;

// ============================================================================
// Configuration Errors
// ============================================================================

/// Configuration loading errors.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// Failed to read configuration file.
    #[error("failed to read config file: {0}")]
    FileRead(#[from] std::io::Error),

    /// Failed to parse configuration.
    #[error("failed to parse config: {0}")]
    Parse(String),

    /// Invalid configuration value.
    #[error("invalid config value for {field}: {message}")]
    InvalidValue {
        /// Field name.
        field: String,
        /// Error message.
        message: String,
    },

    /// Environment variable error.
    #[error("environment variable error: {0}")]
    EnvVar(#[from] std::env::VarError),
}

// ============================================================================
// Server Configuration
// ============================================================================

/// gRPC server configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrpcConfig {
    /// Server host address.
    #[serde(default = "default_host")]
    pub host: String,

    /// Server port.
    #[serde(default = "default_grpc_port")]
    pub port: u16,

    /// Maximum concurrent connections.
    #[serde(default = "default_max_connections")]
    pub max_connections: usize,

    /// Request timeout in seconds.
    #[serde(default = "default_request_timeout")]
    pub request_timeout_secs: u64,

    /// Enable reflection service.
    #[serde(default = "default_true")]
    pub enable_reflection: bool,
}

impl Default for GrpcConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_grpc_port(),
            max_connections: default_max_connections(),
            request_timeout_secs: default_request_timeout(),
            enable_reflection: true,
        }
    }
}

impl GrpcConfig {
    /// Returns the socket address for the gRPC server.
    ///
    /// # Errors
    ///
    /// Returns an error if the address cannot be parsed.
    pub fn socket_addr(&self) -> Result<SocketAddr, ConfigError> {
        format!("{}:{}", self.host, self.port)
            .parse()
            .map_err(|e| ConfigError::InvalidValue {
                field: "grpc.host:port".to_string(),
                message: format!("{e}"),
            })
    }
}

/// REST/HTTP server configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestConfig {
    /// Server host address.
    #[serde(default = "default_host")]
    pub host: String,

    /// Server port.
    #[serde(default = "default_rest_port")]
    pub port: u16,

    /// Maximum concurrent connections.
    #[serde(default = "default_max_connections")]
    pub max_connections: usize,

    /// Request timeout in seconds.
    #[serde(default = "default_request_timeout")]
    pub request_timeout_secs: u64,

    /// Enable CORS.
    #[serde(default = "default_true")]
    pub enable_cors: bool,

    /// Allowed CORS origins (empty = allow all).
    #[serde(default)]
    pub cors_origins: Vec<String>,
}

impl Default for RestConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_rest_port(),
            max_connections: default_max_connections(),
            request_timeout_secs: default_request_timeout(),
            enable_cors: true,
            cors_origins: Vec::new(),
        }
    }
}

impl RestConfig {
    /// Returns the socket address for the REST server.
    ///
    /// # Errors
    ///
    /// Returns an error if the address cannot be parsed.
    pub fn socket_addr(&self) -> Result<SocketAddr, ConfigError> {
        format!("{}:{}", self.host, self.port)
            .parse()
            .map_err(|e| ConfigError::InvalidValue {
                field: "rest.host:port".to_string(),
                message: format!("{e}"),
            })
    }
}

// ============================================================================
// Logging Configuration
// ============================================================================

/// Log format options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum LogFormat {
    /// JSON format (structured logging).
    #[default]
    Json,
    /// Pretty format (human-readable).
    Pretty,
}

/// Logging configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogConfig {
    /// Log level (trace, debug, info, warn, error).
    #[serde(default = "default_log_level")]
    pub level: String,

    /// Log format.
    #[serde(default)]
    pub format: LogFormat,

    /// Include timestamps in logs.
    #[serde(default = "default_true")]
    pub include_timestamps: bool,

    /// Include target (module path) in logs.
    #[serde(default = "default_true")]
    pub include_target: bool,

    /// Include span information in logs.
    #[serde(default = "default_true")]
    pub include_spans: bool,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
            format: LogFormat::Json,
            include_timestamps: true,
            include_target: true,
            include_spans: true,
        }
    }
}

// ============================================================================
// Database Configuration
// ============================================================================

/// Database configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    /// Database URL.
    #[serde(default = "default_database_url")]
    pub url: String,

    /// Maximum connection pool size.
    #[serde(default = "default_pool_size")]
    pub max_connections: u32,

    /// Minimum connection pool size.
    #[serde(default = "default_min_connections")]
    pub min_connections: u32,

    /// Connection timeout in seconds.
    #[serde(default = "default_connection_timeout")]
    pub connect_timeout_secs: u64,

    /// Idle connection timeout in seconds.
    #[serde(default = "default_idle_timeout")]
    pub idle_timeout_secs: u64,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: default_database_url(),
            max_connections: default_pool_size(),
            min_connections: default_min_connections(),
            connect_timeout_secs: default_connection_timeout(),
            idle_timeout_secs: default_idle_timeout(),
        }
    }
}

// ============================================================================
// Venue Configuration
// ============================================================================

/// Venue adapter configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VenueConfig {
    /// Enable 0x Protocol adapter.
    #[serde(default)]
    pub enable_0x: bool,

    /// Enable 1inch adapter.
    #[serde(default)]
    pub enable_1inch: bool,

    /// Enable Uniswap adapter.
    #[serde(default)]
    pub enable_uniswap: bool,

    /// Enable Hashflow adapter.
    #[serde(default)]
    pub enable_hashflow: bool,

    /// Default quote timeout in milliseconds.
    #[serde(default = "default_quote_timeout")]
    pub quote_timeout_ms: u64,

    /// Maximum concurrent venue requests.
    #[serde(default = "default_max_concurrent_requests")]
    pub max_concurrent_requests: usize,
}

impl Default for VenueConfig {
    fn default() -> Self {
        Self {
            enable_0x: false,
            enable_1inch: false,
            enable_uniswap: false,
            enable_hashflow: false,
            quote_timeout_ms: default_quote_timeout(),
            max_concurrent_requests: default_max_concurrent_requests(),
        }
    }
}

// ============================================================================
// Application Configuration
// ============================================================================

/// Main application configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    /// gRPC server configuration.
    #[serde(default)]
    pub grpc: GrpcConfig,

    /// REST server configuration.
    #[serde(default)]
    pub rest: RestConfig,

    /// Logging configuration.
    #[serde(default)]
    pub log: LogConfig,

    /// Database configuration.
    #[serde(default)]
    pub database: DatabaseConfig,

    /// Venue configuration.
    #[serde(default)]
    pub venues: VenueConfig,

    /// Service name for tracing.
    #[serde(default = "default_service_name")]
    pub service_name: String,

    /// Environment (development, staging, production).
    #[serde(default = "default_environment")]
    pub environment: String,
}

impl AppConfig {
    /// Loads configuration from environment variables and optional config file.
    ///
    /// # Errors
    ///
    /// Returns an error if configuration loading fails.
    pub fn load() -> Result<Self, ConfigError> {
        let mut config = Self::default();

        // Try to load from config file if it exists
        let config_path =
            std::env::var("OTC_RFQ_CONFIG_FILE").unwrap_or_else(|_| "config.toml".to_string());

        if Path::new(&config_path).exists() {
            config = Self::from_file(&config_path)?;
        }

        // Override with environment variables
        config.apply_env_overrides();

        Ok(config)
    }

    /// Loads configuration from a TOML file.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or parsed.
    pub fn from_file(path: &str) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path)?;
        toml::from_str(&content).map_err(|e| ConfigError::Parse(e.to_string()))
    }

    /// Applies environment variable overrides to the configuration.
    fn apply_env_overrides(&mut self) {
        // gRPC configuration
        if let Ok(host) = std::env::var("OTC_RFQ_GRPC_HOST") {
            self.grpc.host = host;
        }
        if let Ok(port) = std::env::var("OTC_RFQ_GRPC_PORT")
            && let Ok(p) = port.parse()
        {
            self.grpc.port = p;
        }

        // REST configuration
        if let Ok(host) = std::env::var("OTC_RFQ_REST_HOST") {
            self.rest.host = host;
        }
        if let Ok(port) = std::env::var("OTC_RFQ_REST_PORT")
            && let Ok(p) = port.parse()
        {
            self.rest.port = p;
        }

        // Logging configuration
        if let Ok(level) = std::env::var("OTC_RFQ_LOG_LEVEL") {
            self.log.level = level;
        }
        if let Ok(format) = std::env::var("OTC_RFQ_LOG_FORMAT") {
            self.log.format = match format.to_lowercase().as_str() {
                "pretty" => LogFormat::Pretty,
                _ => LogFormat::Json,
            };
        }

        // Database configuration
        if let Ok(url) = std::env::var("OTC_RFQ_DATABASE_URL") {
            self.database.url = url;
        }

        // Service configuration
        if let Ok(name) = std::env::var("OTC_RFQ_SERVICE_NAME") {
            self.service_name = name;
        }
        if let Ok(env) = std::env::var("OTC_RFQ_ENVIRONMENT") {
            self.environment = env;
        }
    }

    /// Validates the configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if validation fails.
    pub fn validate(&self) -> Result<(), ConfigError> {
        // Validate gRPC address
        self.grpc.socket_addr()?;

        // Validate REST address
        self.rest.socket_addr()?;

        // Validate log level
        let valid_levels = ["trace", "debug", "info", "warn", "error"];
        if !valid_levels.contains(&self.log.level.to_lowercase().as_str()) {
            return Err(ConfigError::InvalidValue {
                field: "log.level".to_string(),
                message: format!(
                    "invalid log level '{}', must be one of: {:?}",
                    self.log.level, valid_levels
                ),
            });
        }

        Ok(())
    }
}

// ============================================================================
// Default Value Functions
// ============================================================================

fn default_host() -> String {
    "0.0.0.0".to_string()
}

fn default_grpc_port() -> u16 {
    50051
}

fn default_rest_port() -> u16 {
    8080
}

fn default_max_connections() -> usize {
    1000
}

fn default_request_timeout() -> u64 {
    30
}

fn default_true() -> bool {
    true
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_database_url() -> String {
    "postgres://localhost/otc_rfq".to_string()
}

fn default_pool_size() -> u32 {
    10
}

fn default_min_connections() -> u32 {
    1
}

fn default_connection_timeout() -> u64 {
    30
}

fn default_idle_timeout() -> u64 {
    600
}

fn default_quote_timeout() -> u64 {
    5000
}

fn default_max_concurrent_requests() -> usize {
    10
}

fn default_service_name() -> String {
    "otc-rfq".to_string()
}

fn default_environment() -> String {
    "development".to_string()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn app_config_default() {
        let config = AppConfig::default();
        assert_eq!(config.grpc.port, 50051);
        assert_eq!(config.rest.port, 8080);
        assert_eq!(config.log.level, "info");
    }

    #[test]
    fn grpc_config_socket_addr() {
        let config = GrpcConfig::default();
        let addr = config.socket_addr().unwrap();
        assert_eq!(addr.port(), 50051);
    }

    #[test]
    fn rest_config_socket_addr() {
        let config = RestConfig::default();
        let addr = config.socket_addr().unwrap();
        assert_eq!(addr.port(), 8080);
    }

    #[test]
    fn log_format_default() {
        let format = LogFormat::default();
        assert_eq!(format, LogFormat::Json);
    }

    #[test]
    fn app_config_validate_valid() {
        let config = AppConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn app_config_validate_invalid_log_level() {
        let mut config = AppConfig::default();
        config.log.level = "invalid".to_string();
        assert!(config.validate().is_err());
    }

    #[test]
    fn grpc_config_invalid_address() {
        let config = GrpcConfig {
            host: "invalid host with spaces".to_string(),
            ..Default::default()
        };
        assert!(config.socket_addr().is_err());
    }

    #[test]
    fn database_config_default() {
        let config = DatabaseConfig::default();
        assert_eq!(config.max_connections, 10);
        assert_eq!(config.min_connections, 1);
    }

    #[test]
    fn venue_config_default() {
        let config = VenueConfig::default();
        assert!(!config.enable_0x);
        assert_eq!(config.quote_timeout_ms, 5000);
    }
}
