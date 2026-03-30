//! # SBE Error Types
//!
//! Error types for SBE encoding and decoding operations.

use thiserror::Error;

/// Errors that can occur during SBE encoding/decoding.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SbeError {
    /// Buffer is too small for the message.
    #[error("buffer too small: need {needed} bytes, have {available}")]
    BufferTooSmall {
        /// Bytes needed.
        needed: usize,
        /// Bytes available.
        available: usize,
    },

    /// Invalid message header.
    #[error("invalid message header: {0}")]
    InvalidHeader(String),

    /// Unknown template ID.
    #[error("unknown template ID: {0}")]
    UnknownTemplateId(u16),

    /// Invalid field value.
    #[error("invalid field value: {0}")]
    InvalidFieldValue(String),

    /// Invalid string encoding.
    #[error("invalid string encoding: {0}")]
    InvalidString(String),

    /// Invalid enum value.
    #[error("invalid enum value: {0}")]
    InvalidEnumValue(u8),

    /// Invalid UUID bytes.
    #[error("invalid UUID bytes")]
    InvalidUuid,

    /// Invalid decimal encoding.
    #[error("invalid decimal encoding: {0}")]
    InvalidDecimal(String),

    /// Invalid timestamp value.
    #[error("invalid timestamp: {0}")]
    InvalidTimestamp(String),

    /// Arithmetic overflow during offset calculation.
    #[error("arithmetic overflow during offset calculation")]
    Overflow,
}

/// Result type for SBE operations.
pub type SbeResult<T> = Result<T, SbeError>;
