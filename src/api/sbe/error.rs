//! # SBE API Errors
//!
//! Error types for SBE API operations.

use thiserror::Error;

/// Errors that can occur during SBE API operations.
#[derive(Debug, Error)]
pub enum SbeApiError {
    /// Missing required field in request.
    #[error("missing required field: {0}")]
    MissingField(&'static str),

    /// Invalid field value.
    #[error("invalid {field}: {message}")]
    InvalidValue {
        /// Field name.
        field: &'static str,
        /// Error message.
        message: String,
    },

    /// Invalid UUID format.
    #[error("invalid UUID: {0}")]
    InvalidUuid(String),

    /// Invalid decimal format.
    #[error("invalid decimal: {0}")]
    InvalidDecimal(String),

    /// Invalid enum value.
    #[error("invalid enum value for {enum_name}: {value}")]
    InvalidEnum {
        /// Enum name.
        enum_name: &'static str,
        /// Invalid value.
        value: u8,
    },

    /// SBE encoding error.
    #[error("SBE encoding error: {0}")]
    SbeEncoding(#[from] crate::infrastructure::sbe::SbeError),

    /// Domain error during conversion.
    #[error("domain error: {0}")]
    Domain(String),

    /// IO error during transport.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// Frame size exceeds maximum.
    #[error("frame too large: {size} bytes exceeds max {max} bytes")]
    FrameTooLarge {
        /// Actual frame size.
        size: usize,
        /// Maximum allowed size.
        max: usize,
    },

    /// Connection closed by peer.
    #[error("connection closed")]
    ConnectionClosed,

    /// Operation timeout.
    #[error("timeout")]
    Timeout,

    /// Template not yet implemented.
    #[error("template {0} not implemented")]
    NotImplemented(u16),
}

/// Result type for SBE API operations.
pub type SbeApiResult<T> = Result<T, SbeApiError>;
