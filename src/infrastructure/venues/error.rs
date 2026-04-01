//! # Venue Errors
//!
//! Error types for venue operations.
//!
//! This module provides error types for venue adapter operations including
//! quote requests, trade execution, and health checks.
//!
//! # Examples
//!
//! ```
//! use otc_rfq::infrastructure::venues::error::VenueError;
//!
//! let error = VenueError::timeout("Request timed out after 5000ms");
//! assert!(error.is_retryable());
//!
//! let error = VenueError::authentication("Invalid API key");
//! assert!(!error.is_retryable());
//! ```

use crate::domain::value_objects::VenueId;
use thiserror::Error;

/// Error type for venue adapter operations.
///
/// Represents errors that can occur when interacting with liquidity venues,
/// including network issues, authentication failures, and business logic errors.
#[derive(Debug, Clone, Error)]
pub enum VenueError {
    /// Request timed out.
    #[error("venue timeout: {message}")]
    Timeout {
        /// Error message.
        message: String,
        /// Timeout duration in milliseconds.
        timeout_ms: Option<u64>,
    },

    /// Network or connection error.
    #[error("venue connection error: {message}")]
    Connection {
        /// Error message.
        message: String,
    },

    /// Authentication or authorization failure.
    #[error("venue authentication error: {message}")]
    Authentication {
        /// Error message.
        message: String,
    },

    /// Rate limit exceeded.
    #[error("venue rate limit exceeded: {message}")]
    RateLimited {
        /// Error message.
        message: String,
        /// Retry after duration in milliseconds.
        retry_after_ms: Option<u64>,
    },

    /// Invalid request parameters.
    #[error("venue invalid request: {message}")]
    InvalidRequest {
        /// Error message.
        message: String,
    },

    /// Quote not available or rejected.
    #[error("venue quote unavailable: {message}")]
    QuoteUnavailable {
        /// Error message.
        message: String,
    },

    /// Insufficient liquidity.
    #[error("venue insufficient liquidity: {message}")]
    InsufficientLiquidity {
        /// Error message.
        message: String,
    },

    /// Trade execution failed.
    #[error("venue execution failed: {message}")]
    ExecutionFailed {
        /// Error message.
        message: String,
        /// Venue-specific error code.
        error_code: Option<String>,
    },

    /// Quote has expired.
    #[error("venue quote expired: {message}")]
    QuoteExpired {
        /// Error message.
        message: String,
    },

    /// Venue is unavailable or unhealthy.
    #[error("venue unavailable: {venue_id} - {message}")]
    VenueUnavailable {
        /// The venue ID.
        venue_id: VenueId,
        /// Error message.
        message: String,
    },

    /// Protocol or format error.
    #[error("venue protocol error: {message}")]
    ProtocolError {
        /// Error message.
        message: String,
    },

    /// Internal venue error.
    #[error("venue internal error: {message}")]
    InternalError {
        /// Error message.
        message: String,
    },

    /// Unknown or unclassified error.
    #[error("venue unknown error: {message}")]
    Unknown {
        /// Error message.
        message: String,
    },

    /// Operation not supported by this venue.
    #[error("venue unsupported operation: {operation}")]
    UnsupportedOperation {
        /// The operation that is not supported.
        operation: String,
    },
}

impl VenueError {
    /// Creates a timeout error.
    #[must_use]
    pub fn timeout(message: impl Into<String>) -> Self {
        Self::Timeout {
            message: message.into(),
            timeout_ms: None,
        }
    }

    /// Creates a timeout error with duration.
    #[must_use]
    pub fn timeout_with_duration(message: impl Into<String>, timeout_ms: u64) -> Self {
        Self::Timeout {
            message: message.into(),
            timeout_ms: Some(timeout_ms),
        }
    }

    /// Creates a connection error.
    #[must_use]
    pub fn connection(message: impl Into<String>) -> Self {
        Self::Connection {
            message: message.into(),
        }
    }

    /// Creates an authentication error.
    #[must_use]
    pub fn authentication(message: impl Into<String>) -> Self {
        Self::Authentication {
            message: message.into(),
        }
    }

    /// Creates a rate limited error.
    #[must_use]
    pub fn rate_limited(message: impl Into<String>) -> Self {
        Self::RateLimited {
            message: message.into(),
            retry_after_ms: None,
        }
    }

    /// Creates a rate limited error with retry duration.
    #[must_use]
    pub fn rate_limited_with_retry(message: impl Into<String>, retry_after_ms: u64) -> Self {
        Self::RateLimited {
            message: message.into(),
            retry_after_ms: Some(retry_after_ms),
        }
    }

    /// Creates an invalid request error.
    #[must_use]
    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self::InvalidRequest {
            message: message.into(),
        }
    }

    /// Creates a quote unavailable error.
    #[must_use]
    pub fn quote_unavailable(message: impl Into<String>) -> Self {
        Self::QuoteUnavailable {
            message: message.into(),
        }
    }

    /// Creates an insufficient liquidity error.
    #[must_use]
    pub fn insufficient_liquidity(message: impl Into<String>) -> Self {
        Self::InsufficientLiquidity {
            message: message.into(),
        }
    }

    /// Creates an execution failed error.
    #[must_use]
    pub fn execution_failed(message: impl Into<String>) -> Self {
        Self::ExecutionFailed {
            message: message.into(),
            error_code: None,
        }
    }

    /// Creates an execution failed error with error code.
    #[must_use]
    pub fn execution_failed_with_code(
        message: impl Into<String>,
        error_code: impl Into<String>,
    ) -> Self {
        Self::ExecutionFailed {
            message: message.into(),
            error_code: Some(error_code.into()),
        }
    }

    /// Creates a quote expired error.
    #[must_use]
    pub fn quote_expired(message: impl Into<String>) -> Self {
        Self::QuoteExpired {
            message: message.into(),
        }
    }

    /// Creates a venue unavailable error.
    #[must_use]
    pub fn venue_unavailable(venue_id: VenueId, message: impl Into<String>) -> Self {
        Self::VenueUnavailable {
            venue_id,
            message: message.into(),
        }
    }

    /// Creates a protocol error.
    #[must_use]
    pub fn protocol_error(message: impl Into<String>) -> Self {
        Self::ProtocolError {
            message: message.into(),
        }
    }

    /// Creates an internal error.
    #[must_use]
    pub fn internal_error(message: impl Into<String>) -> Self {
        Self::InternalError {
            message: message.into(),
        }
    }

    /// Creates an unknown error.
    #[must_use]
    pub fn unknown(message: impl Into<String>) -> Self {
        Self::Unknown {
            message: message.into(),
        }
    }

    /// Creates an unsupported operation error.
    #[must_use]
    pub fn unsupported_operation(operation: impl Into<String>) -> Self {
        Self::UnsupportedOperation {
            operation: operation.into(),
        }
    }

    /// Returns true if this error is retryable.
    ///
    /// Retryable errors are transient and may succeed on retry.
    #[must_use]
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::Timeout { .. }
                | Self::Connection { .. }
                | Self::RateLimited { .. }
                | Self::VenueUnavailable { .. }
        )
    }

    /// Returns true if this error is a client error (bad request).
    #[must_use]
    pub fn is_client_error(&self) -> bool {
        matches!(
            self,
            Self::InvalidRequest { .. } | Self::Authentication { .. } | Self::QuoteExpired { .. }
        )
    }

    /// Returns true if this error is a venue/server error.
    #[must_use]
    pub fn is_venue_error(&self) -> bool {
        matches!(
            self,
            Self::InternalError { .. } | Self::ProtocolError { .. } | Self::VenueUnavailable { .. }
        )
    }

    /// Returns the retry delay in milliseconds, if applicable.
    #[must_use]
    pub fn retry_after_ms(&self) -> Option<u64> {
        match self {
            Self::RateLimited { retry_after_ms, .. } => *retry_after_ms,
            _ => None,
        }
    }

    /// Returns the error code, if any.
    #[must_use]
    pub fn error_code(&self) -> Option<&str> {
        match self {
            Self::ExecutionFailed { error_code, .. } => error_code.as_deref(),
            _ => None,
        }
    }
}

/// Result type for venue operations.
pub type VenueResult<T> = Result<T, VenueError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeout_is_retryable() {
        let error = VenueError::timeout("test");
        assert!(error.is_retryable());
        assert!(!error.is_client_error());
    }

    #[test]
    fn connection_is_retryable() {
        let error = VenueError::connection("test");
        assert!(error.is_retryable());
    }

    #[test]
    fn rate_limited_is_retryable() {
        let error = VenueError::rate_limited_with_retry("test", 1000);
        assert!(error.is_retryable());
        assert_eq!(error.retry_after_ms(), Some(1000));
    }

    #[test]
    fn authentication_is_not_retryable() {
        let error = VenueError::authentication("test");
        assert!(!error.is_retryable());
        assert!(error.is_client_error());
    }

    #[test]
    fn invalid_request_is_client_error() {
        let error = VenueError::invalid_request("test");
        assert!(error.is_client_error());
        assert!(!error.is_retryable());
    }

    #[test]
    fn execution_failed_with_code() {
        let error = VenueError::execution_failed_with_code("test", "ERR_001");
        assert_eq!(error.error_code(), Some("ERR_001"));
    }

    #[test]
    fn venue_unavailable_is_venue_error() {
        let error = VenueError::venue_unavailable(VenueId::new("test"), "down");
        assert!(error.is_venue_error());
        assert!(error.is_retryable());
    }

    #[test]
    fn display_format() {
        let error = VenueError::timeout("request timed out");
        let display = error.to_string();
        assert!(display.contains("timeout"));
        assert!(display.contains("request timed out"));
    }
}
