//! # Logging Middleware
//!
//! Request/response logging with structured fields.
//!
//! This module provides middleware for logging HTTP requests and responses
//! with structured fields including method, path, status, duration, and client ID.
//!
//! # Features
//!
//! - Structured logging with tracing crate
//! - Request ID generation and propagation
//! - Configurable log levels
//! - Sensitive data redaction
//! - Duration tracking
//!
//! # Usage
//!
//! ```ignore
//! use otc_rfq::api::middleware::logging::{LoggingConfig, logging_middleware};
//!
//! let config = LoggingConfig::default();
//! let app = Router::new()
//!     .route("/api", get(handler))
//!     .layer(logging_layer(config));
//! ```

use axum::{
    extract::Request,
    http::{HeaderMap, HeaderValue},
    middleware::Next,
    response::Response,
};
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, error, info, instrument, warn, Level, Span};
use uuid::Uuid;

// ============================================================================
// Configuration
// ============================================================================

/// Logging configuration.
#[derive(Debug, Clone)]
pub struct LoggingConfig {
    /// Log level for successful requests.
    pub success_level: Level,
    /// Log level for client errors (4xx).
    pub client_error_level: Level,
    /// Log level for server errors (5xx).
    pub server_error_level: Level,
    /// Whether to log request headers.
    pub log_headers: bool,
    /// Whether to log request body (be careful with sensitive data).
    pub log_body: bool,
    /// Headers to redact from logs.
    pub redacted_headers: Vec<String>,
    /// Whether to include latency in logs.
    pub include_latency: bool,
    /// Whether to generate request IDs.
    pub generate_request_id: bool,
    /// Header name for request ID.
    pub request_id_header: String,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            success_level: Level::INFO,
            client_error_level: Level::WARN,
            server_error_level: Level::ERROR,
            log_headers: false,
            log_body: false,
            redacted_headers: vec![
                "authorization".to_string(),
                "cookie".to_string(),
                "x-api-key".to_string(),
                "x-auth-token".to_string(),
            ],
            include_latency: true,
            generate_request_id: true,
            request_id_header: "X-Request-ID".to_string(),
        }
    }
}

impl LoggingConfig {
    /// Creates a new logging config.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Enables header logging.
    #[must_use]
    pub fn with_headers(mut self) -> Self {
        self.log_headers = true;
        self
    }

    /// Adds headers to redact.
    #[must_use]
    pub fn with_redacted_headers(mut self, headers: Vec<String>) -> Self {
        self.redacted_headers.extend(headers);
        self
    }

    /// Disables request ID generation.
    #[must_use]
    pub fn without_request_id(mut self) -> Self {
        self.generate_request_id = false;
        self
    }

    /// Sets a custom request ID header name.
    #[must_use]
    pub fn with_request_id_header(mut self, header: impl Into<String>) -> Self {
        self.request_id_header = header.into();
        self
    }
}

// ============================================================================
// Request ID
// ============================================================================

/// A unique request identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RequestId(pub String);

impl RequestId {
    /// Generates a new random request ID.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    /// Creates a request ID from an existing string.
    #[must_use]
    pub fn from_string(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Returns the request ID as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for RequestId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for RequestId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ============================================================================
// Log Entry
// ============================================================================

/// A structured log entry for a request.
#[derive(Debug, Clone)]
pub struct LogEntry {
    /// Request ID.
    pub request_id: String,
    /// HTTP method.
    pub method: String,
    /// Request path.
    pub path: String,
    /// Query string (if any).
    pub query: Option<String>,
    /// Response status code.
    pub status: u16,
    /// Request duration in milliseconds.
    pub duration_ms: u64,
    /// Client ID (from auth or IP).
    pub client_id: Option<String>,
    /// User agent.
    pub user_agent: Option<String>,
    /// Remote address.
    pub remote_addr: Option<String>,
}

// ============================================================================
// Sensitive Data Redaction
// ============================================================================

/// Redacts sensitive values from headers.
#[must_use]
pub fn redact_headers(headers: &HeaderMap, redacted_names: &[String]) -> Vec<(String, String)> {
    headers
        .iter()
        .map(|(name, value)| {
            let name_str = name.as_str().to_lowercase();
            let value_str = if redacted_names.iter().any(|r| r.to_lowercase() == name_str) {
                "[REDACTED]".to_string()
            } else {
                value.to_str().unwrap_or("[invalid utf8]").to_string()
            };
            (name.as_str().to_string(), value_str)
        })
        .collect()
}

/// Redacts sensitive patterns from a string.
///
/// Replaces common sensitive field values with `[REDACTED]`.
#[must_use]
pub fn redact_sensitive(input: &str) -> String {
    let mut result = input.to_string();

    // Simple pattern matching for common sensitive fields
    let sensitive_keys = ["password", "token", "secret", "api_key"];

    for key in sensitive_keys {
        // Match pattern: "key":"value" or "key": "value"
        let pattern = format!(r#""{}""#, key);
        if let Some(start) = result.find(&pattern) {
            // Find the colon after the key
            if let Some(colon_offset) = result[start..].find(':') {
                let colon_pos = start + colon_offset;
                // Find the opening quote of the value
                if let Some(quote_offset) = result[colon_pos..].find('"') {
                    let value_start = colon_pos + quote_offset + 1;
                    // Find the closing quote
                    if let Some(end_offset) = result[value_start..].find('"') {
                        let value_end = value_start + end_offset;
                        result.replace_range(value_start..value_end, "[REDACTED]");
                    }
                }
            }
        }
    }

    result
}

// ============================================================================
// Logging State
// ============================================================================

/// Shared state for logging middleware.
#[derive(Debug, Clone)]
pub struct LoggingState {
    /// Configuration.
    pub config: LoggingConfig,
}

impl LoggingState {
    /// Creates a new logging state.
    #[must_use]
    pub fn new(config: LoggingConfig) -> Self {
        Self { config }
    }
}

impl Default for LoggingState {
    fn default() -> Self {
        Self::new(LoggingConfig::default())
    }
}

// ============================================================================
// Middleware
// ============================================================================

/// Logging middleware function.
///
/// Logs request and response information with structured fields.
#[instrument(skip_all, fields(request_id))]
pub async fn logging_middleware(
    state: Arc<LoggingState>,
    mut request: Request,
    next: Next,
) -> Response {
    let start = Instant::now();

    // Generate or extract request ID
    let request_id = if state.config.generate_request_id {
        request
            .headers()
            .get(&state.config.request_id_header)
            .and_then(|v| v.to_str().ok())
            .map(RequestId::from_string)
            .unwrap_or_else(RequestId::new)
    } else {
        RequestId::from_string("none")
    };

    // Record request ID in span
    Span::current().record("request_id", request_id.as_str());

    // Store request ID in extensions for downstream handlers
    request.extensions_mut().insert(request_id.clone());

    // Extract request info
    let method = request.method().to_string();
    let path = request.uri().path().to_string();
    let query = request.uri().query().map(|s| s.to_string());
    let user_agent = request
        .headers()
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    // Extract client ID from auth claims if available
    let client_id = request
        .extensions()
        .get::<crate::api::middleware::auth::Claims>()
        .and_then(|c| c.client_id.clone().or_else(|| Some(c.sub.clone())));

    // Log headers if configured
    if state.config.log_headers {
        let headers = redact_headers(request.headers(), &state.config.redacted_headers);
        debug!(
            request_id = %request_id,
            method = %method,
            path = %path,
            headers = ?headers,
            "Request headers"
        );
    }

    // Log request start
    debug!(
        request_id = %request_id,
        method = %method,
        path = %path,
        query = ?query,
        client_id = ?client_id,
        "Request started"
    );

    // Execute the request
    let mut response = next.run(request).await;

    // Calculate duration
    let duration = start.elapsed();
    let duration_ms = duration.as_millis() as u64;

    // Add request ID to response headers
    if state.config.generate_request_id {
        if let Ok(header_value) = HeaderValue::from_str(request_id.as_str()) {
            if let Ok(header_name) = axum::http::header::HeaderName::from_bytes(
                state.config.request_id_header.as_bytes(),
            ) {
                response.headers_mut().insert(header_name, header_value);
            }
        }
    }

    // Get status code
    let status = response.status();
    let status_code = status.as_u16();

    // Create log entry
    let entry = LogEntry {
        request_id: request_id.to_string(),
        method: method.clone(),
        path: path.clone(),
        query,
        status: status_code,
        duration_ms,
        client_id,
        user_agent,
        remote_addr: None,
    };

    // Log based on status code
    if status.is_success() || status.is_redirection() {
        info!(
            request_id = %entry.request_id,
            method = %entry.method,
            path = %entry.path,
            status = entry.status,
            duration_ms = entry.duration_ms,
            client_id = ?entry.client_id,
            "Request completed"
        );
    } else if status.is_client_error() {
        warn!(
            request_id = %entry.request_id,
            method = %entry.method,
            path = %entry.path,
            status = entry.status,
            duration_ms = entry.duration_ms,
            client_id = ?entry.client_id,
            "Client error"
        );
    } else {
        error!(
            request_id = %entry.request_id,
            method = %entry.method,
            path = %entry.path,
            status = entry.status,
            duration_ms = entry.duration_ms,
            client_id = ?entry.client_id,
            "Server error"
        );
    }

    response
}

/// Creates a logging state for use with middleware.
#[must_use]
pub fn create_logging_state() -> Arc<LoggingState> {
    Arc::new(LoggingState::default())
}

/// Creates a logging state with custom config.
#[must_use]
pub fn create_logging_state_with_config(config: LoggingConfig) -> Arc<LoggingState> {
    Arc::new(LoggingState::new(config))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn logging_config_default() {
        let config = LoggingConfig::default();
        assert_eq!(config.success_level, Level::INFO);
        assert!(!config.log_headers);
        assert!(!config.log_body);
        assert!(config.generate_request_id);
        assert_eq!(config.request_id_header, "X-Request-ID");
    }

    #[test]
    fn logging_config_with_headers() {
        let config = LoggingConfig::default().with_headers();
        assert!(config.log_headers);
    }

    #[test]
    fn logging_config_with_redacted_headers() {
        let config =
            LoggingConfig::default().with_redacted_headers(vec!["x-custom-secret".to_string()]);
        assert!(config
            .redacted_headers
            .contains(&"x-custom-secret".to_string()));
    }

    #[test]
    fn logging_config_without_request_id() {
        let config = LoggingConfig::default().without_request_id();
        assert!(!config.generate_request_id);
    }

    #[test]
    fn logging_config_with_request_id_header() {
        let config = LoggingConfig::default().with_request_id_header("X-Correlation-ID");
        assert_eq!(config.request_id_header, "X-Correlation-ID");
    }

    #[test]
    fn request_id_new() {
        let id1 = RequestId::new();
        let id2 = RequestId::new();
        assert_ne!(id1, id2);
        assert!(!id1.as_str().is_empty());
    }

    #[test]
    fn request_id_from_string() {
        let id = RequestId::from_string("test-id-123");
        assert_eq!(id.as_str(), "test-id-123");
    }

    #[test]
    fn request_id_display() {
        let id = RequestId::from_string("display-test");
        assert_eq!(format!("{}", id), "display-test");
    }

    #[test]
    fn redact_headers_redacts_authorization() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "authorization",
            HeaderValue::from_static("Bearer secret123"),
        );
        headers.insert("content-type", HeaderValue::from_static("application/json"));

        let redacted = vec!["authorization".to_string()];
        let result = redact_headers(&headers, &redacted);

        let auth_header = result.iter().find(|(k, _)| k == "authorization");
        let content_header = result.iter().find(|(k, _)| k == "content-type");

        assert_eq!(auth_header.unwrap().1, "[REDACTED]");
        assert_eq!(content_header.unwrap().1, "application/json");
    }

    #[test]
    fn redact_headers_case_insensitive() {
        let mut headers = HeaderMap::new();
        headers.insert("Authorization", HeaderValue::from_static("Bearer token"));

        let redacted = vec!["authorization".to_string()];
        let result = redact_headers(&headers, &redacted);

        let auth_header = result
            .iter()
            .find(|(k, _)| k.to_lowercase() == "authorization");
        assert_eq!(auth_header.unwrap().1, "[REDACTED]");
    }

    #[test]
    fn redact_sensitive_password() {
        let input = r#"{"username":"user","password":"secret123"}"#;
        let result = redact_sensitive(input);
        assert!(result.contains("[REDACTED]"));
        assert!(!result.contains("secret123"));
    }

    #[test]
    fn redact_sensitive_token() {
        let input = r#"{"token":"abc123xyz"}"#;
        let result = redact_sensitive(input);
        assert!(result.contains("[REDACTED]"));
        assert!(!result.contains("abc123xyz"));
    }

    #[test]
    fn redact_sensitive_preserves_other_data() {
        let input = r#"{"username":"testuser","email":"test@example.com"}"#;
        let result = redact_sensitive(input);
        assert_eq!(result, input);
    }

    #[test]
    fn logging_state_default() {
        let state = LoggingState::default();
        assert!(state.config.generate_request_id);
    }

    #[test]
    fn logging_state_new() {
        let config = LoggingConfig::default().with_headers();
        let state = LoggingState::new(config);
        assert!(state.config.log_headers);
    }

    #[test]
    fn log_entry_fields() {
        let entry = LogEntry {
            request_id: "req-123".to_string(),
            method: "GET".to_string(),
            path: "/api/v1/rfqs".to_string(),
            query: Some("limit=10".to_string()),
            status: 200,
            duration_ms: 42,
            client_id: Some("client-456".to_string()),
            user_agent: Some("test-agent".to_string()),
            remote_addr: Some("127.0.0.1".to_string()),
        };

        assert_eq!(entry.request_id, "req-123");
        assert_eq!(entry.method, "GET");
        assert_eq!(entry.path, "/api/v1/rfqs");
        assert_eq!(entry.status, 200);
        assert_eq!(entry.duration_ms, 42);
    }
}
