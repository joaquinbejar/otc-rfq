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
}

/// Result type for SBE API operations.
pub type SbeApiResult<T> = Result<T, SbeApiError>;
