//! # Application Errors
//!
//! Error types for the application layer.
//!
//! These errors represent failures that can occur during use case execution,
//! including validation failures, business rule violations, and infrastructure errors.

use crate::domain::errors::DomainError;
use std::fmt;
use thiserror::Error;

/// Application layer error.
#[derive(Debug, Error)]
pub enum ApplicationError {
    /// Client not found.
    #[error("client not found: {0}")]
    ClientNotFound(String),

    /// Client is not active.
    #[error("client not active: {0}")]
    ClientNotActive(String),

    /// Instrument not supported.
    #[error("instrument not supported: {0}")]
    InstrumentNotSupported(String),

    /// Compliance check failed.
    #[error("compliance check failed: {0}")]
    ComplianceFailed(String),

    /// Request validation failed.
    #[error("validation error: {0}")]
    ValidationError(String),

    /// Domain error.
    #[error("domain error: {0}")]
    DomainError(#[from] DomainError),

    /// Repository error.
    #[error("repository error: {0}")]
    RepositoryError(String),

    /// Event publishing error.
    #[error("event publishing error: {0}")]
    EventPublishError(String),

    /// Internal error.
    #[error("internal error: {0}")]
    Internal(String),

    /// RFQ not found.
    #[error("rfq not found: {0}")]
    RfqNotFound(String),

    /// Quote not found.
    #[error("quote not found: {0}")]
    QuoteNotFound(String),

    /// Quote expired.
    #[error("quote expired: {0}")]
    QuoteExpired(String),

    /// Invalid state for operation.
    #[error("invalid state: {0}")]
    InvalidState(String),

    /// Venue not available.
    #[error("venue not available: {0}")]
    VenueNotAvailable(String),

    /// Trade execution failed.
    #[error("execution failed: {0}")]
    ExecutionFailed(String),
}

impl ApplicationError {
    /// Creates a client not found error.
    #[must_use]
    pub fn client_not_found(client_id: impl Into<String>) -> Self {
        Self::ClientNotFound(client_id.into())
    }

    /// Creates a client not active error.
    #[must_use]
    pub fn client_not_active(client_id: impl Into<String>) -> Self {
        Self::ClientNotActive(client_id.into())
    }

    /// Creates an instrument not supported error.
    #[must_use]
    pub fn instrument_not_supported(instrument: impl fmt::Display) -> Self {
        Self::InstrumentNotSupported(instrument.to_string())
    }

    /// Creates a compliance failed error.
    #[must_use]
    pub fn compliance_failed(reason: impl Into<String>) -> Self {
        Self::ComplianceFailed(reason.into())
    }

    /// Creates a validation error.
    #[must_use]
    pub fn validation(message: impl Into<String>) -> Self {
        Self::ValidationError(message.into())
    }

    /// Creates a repository error.
    #[must_use]
    pub fn repository(message: impl Into<String>) -> Self {
        Self::RepositoryError(message.into())
    }

    /// Creates an event publish error.
    #[must_use]
    pub fn event_publish(message: impl Into<String>) -> Self {
        Self::EventPublishError(message.into())
    }

    /// Creates an internal error.
    #[must_use]
    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal(message.into())
    }
}

/// Result type for application operations.
pub type ApplicationResult<T> = Result<T, ApplicationError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn application_error_client_not_found() {
        let err = ApplicationError::client_not_found("client-123");
        assert!(err.to_string().contains("client-123"));
    }

    #[test]
    fn application_error_client_not_active() {
        let err = ApplicationError::client_not_active("client-456");
        assert!(err.to_string().contains("client-456"));
    }

    #[test]
    fn application_error_instrument_not_supported() {
        let err = ApplicationError::instrument_not_supported("BTC/XYZ");
        assert!(err.to_string().contains("BTC/XYZ"));
    }

    #[test]
    fn application_error_compliance_failed() {
        let err = ApplicationError::compliance_failed("KYC not verified");
        assert!(err.to_string().contains("KYC not verified"));
    }

    #[test]
    fn application_error_validation() {
        let err = ApplicationError::validation("quantity must be positive");
        assert!(err.to_string().contains("quantity must be positive"));
    }

    #[test]
    fn application_error_repository() {
        let err = ApplicationError::repository("database connection failed");
        assert!(err.to_string().contains("database connection failed"));
    }

    #[test]
    fn application_error_from_domain_error() {
        let domain_err = DomainError::InvalidQuantity("negative".to_string());
        let app_err: ApplicationError = domain_err.into();
        assert!(app_err.to_string().contains("negative"));
    }
}
