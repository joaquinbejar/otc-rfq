//! # Domain Errors
//!
//! Typed domain error definitions.
//!
//! This module provides the [`DomainError`] enum for representing
//! domain-level errors with numeric error codes.
//!
//! # Error Code Ranges
//!
//! - **1000-1999**: Validation errors
//! - **2000-2999**: State errors
//! - **3000-3999**: Compliance errors
//! - **4000-4999**: Arithmetic errors
//!
//! # Examples
//!
//! ```
//! use otc_rfq::domain::errors::DomainError;
//!
//! let error = DomainError::InvalidPrice("price must be positive".to_string());
//! assert_eq!(error.code(), 1001);
//! ```

use crate::domain::value_objects::arithmetic::ArithmeticError;
use crate::domain::value_objects::rfq_state::RfqState;
use thiserror::Error;

/// Domain-level error with numeric error codes.
///
/// Provides typed errors for domain operations with consistent
/// error codes for logging and API responses.
///
/// # Error Code Ranges
///
/// | Range | Category |
/// |-------|----------|
/// | 1000-1999 | Validation errors |
/// | 2000-2999 | State errors |
/// | 3000-3999 | Compliance errors |
/// | 4000-4999 | Arithmetic errors |
///
/// # Examples
///
/// ```
/// use otc_rfq::domain::errors::DomainError;
///
/// let error = DomainError::InvalidQuantity("quantity must be positive".to_string());
/// assert!(error.code() >= 1000 && error.code() < 2000);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DomainError {
    // ========================================================================
    // Validation Errors (1000-1999)
    // ========================================================================
    /// Invalid price value.
    #[error("invalid price: {0}")]
    InvalidPrice(String),

    /// Invalid quantity value.
    #[error("invalid quantity: {0}")]
    InvalidQuantity(String),

    /// Invalid state value.
    #[error("invalid state: {0}")]
    InvalidState(String),

    /// Invalid symbol format.
    #[error("invalid symbol: {0}")]
    InvalidSymbol(String),

    /// Invalid identifier.
    #[error("invalid identifier: {0}")]
    InvalidId(String),

    /// Invalid timestamp.
    #[error("invalid timestamp: {0}")]
    InvalidTimestamp(String),

    /// Generic validation error.
    #[error("validation error: {0}")]
    ValidationError(String),

    // ========================================================================
    // State Errors (2000-2999)
    // ========================================================================
    /// Invalid state transition attempted.
    #[error("invalid state transition from {from} to {to}")]
    InvalidStateTransition {
        /// The current state.
        from: RfqState,
        /// The attempted target state.
        to: RfqState,
    },

    /// Quote has expired.
    #[error("quote expired: {0}")]
    QuoteExpired(String),

    /// Quote not found.
    #[error("quote not found: {0}")]
    QuoteNotFound(String),

    /// RFQ not found.
    #[error("rfq not found: {0}")]
    RfqNotFound(String),

    /// Trade not found.
    #[error("trade not found: {0}")]
    TradeNotFound(String),

    /// Entity already exists.
    #[error("entity already exists: {0}")]
    AlreadyExists(String),

    /// Operation not allowed in current state.
    #[error("operation not allowed: {0}")]
    OperationNotAllowed(String),

    // ========================================================================
    // Compliance Errors (3000-3999)
    // ========================================================================
    /// Compliance check blocked the operation.
    #[error("compliance blocked: {0}")]
    ComplianceBlocked(String),

    /// KYC verification failed.
    #[error("kyc verification failed: {0}")]
    KycFailed(String),

    /// Counterparty not authorized.
    #[error("counterparty not authorized: {0}")]
    CounterpartyNotAuthorized(String),

    /// Trading limit exceeded.
    #[error("trading limit exceeded: {0}")]
    TradingLimitExceeded(String),

    /// Instrument not allowed.
    #[error("instrument not allowed: {0}")]
    InstrumentNotAllowed(String),

    // ========================================================================
    // Arithmetic Errors (4000-4999)
    // ========================================================================
    /// Arithmetic overflow.
    #[error("arithmetic overflow")]
    Overflow,

    /// Arithmetic underflow.
    #[error("arithmetic underflow")]
    Underflow,

    /// Division by zero.
    #[error("division by zero")]
    DivisionByZero,

    /// Invalid arithmetic value.
    #[error("invalid arithmetic value: {0}")]
    InvalidArithmeticValue(String),
}

impl DomainError {
    /// Returns the numeric error code.
    ///
    /// # Error Code Ranges
    ///
    /// - 1000-1999: Validation errors
    /// - 2000-2999: State errors
    /// - 3000-3999: Compliance errors
    /// - 4000-4999: Arithmetic errors
    ///
    /// # Examples
    ///
    /// ```
    /// use otc_rfq::domain::errors::DomainError;
    ///
    /// assert_eq!(DomainError::InvalidPrice("test".to_string()).code(), 1001);
    /// assert_eq!(DomainError::Overflow.code(), 4001);
    /// ```
    #[must_use]
    pub const fn code(&self) -> u16 {
        match self {
            // Validation errors (1000-1999)
            Self::InvalidPrice(_) => 1001,
            Self::InvalidQuantity(_) => 1002,
            Self::InvalidState(_) => 1003,
            Self::InvalidSymbol(_) => 1004,
            Self::InvalidId(_) => 1005,
            Self::InvalidTimestamp(_) => 1006,
            Self::ValidationError(_) => 1099,

            // State errors (2000-2999)
            Self::InvalidStateTransition { .. } => 2001,
            Self::QuoteExpired(_) => 2002,
            Self::QuoteNotFound(_) => 2003,
            Self::RfqNotFound(_) => 2004,
            Self::TradeNotFound(_) => 2005,
            Self::AlreadyExists(_) => 2006,
            Self::OperationNotAllowed(_) => 2099,

            // Compliance errors (3000-3999)
            Self::ComplianceBlocked(_) => 3001,
            Self::KycFailed(_) => 3002,
            Self::CounterpartyNotAuthorized(_) => 3003,
            Self::TradingLimitExceeded(_) => 3004,
            Self::InstrumentNotAllowed(_) => 3005,

            // Arithmetic errors (4000-4999)
            Self::Overflow => 4001,
            Self::Underflow => 4002,
            Self::DivisionByZero => 4003,
            Self::InvalidArithmeticValue(_) => 4004,
        }
    }

    /// Returns the error category name.
    ///
    /// # Examples
    ///
    /// ```
    /// use otc_rfq::domain::errors::DomainError;
    ///
    /// assert_eq!(DomainError::InvalidPrice("test".to_string()).category(), "validation");
    /// assert_eq!(DomainError::QuoteExpired("test".to_string()).category(), "state");
    /// ```
    #[must_use]
    pub const fn category(&self) -> &'static str {
        match self.code() {
            1000..=1999 => "validation",
            2000..=2999 => "state",
            3000..=3999 => "compliance",
            4000..=4999 => "arithmetic",
            _ => "unknown",
        }
    }

    /// Returns true if this is a validation error.
    #[inline]
    #[must_use]
    pub const fn is_validation_error(&self) -> bool {
        matches!(self.code(), 1000..=1999)
    }

    /// Returns true if this is a state error.
    #[inline]
    #[must_use]
    pub const fn is_state_error(&self) -> bool {
        matches!(self.code(), 2000..=2999)
    }

    /// Returns true if this is a compliance error.
    #[inline]
    #[must_use]
    pub const fn is_compliance_error(&self) -> bool {
        matches!(self.code(), 3000..=3999)
    }

    /// Returns true if this is an arithmetic error.
    #[inline]
    #[must_use]
    pub const fn is_arithmetic_error(&self) -> bool {
        matches!(self.code(), 4000..=4999)
    }
}

impl From<ArithmeticError> for DomainError {
    fn from(err: ArithmeticError) -> Self {
        match err {
            ArithmeticError::Overflow => Self::Overflow,
            ArithmeticError::Underflow => Self::Underflow,
            ArithmeticError::DivisionByZero => Self::DivisionByZero,
            ArithmeticError::InvalidValue(msg) => Self::InvalidArithmeticValue(msg.to_string()),
        }
    }
}

/// Result type for domain operations.
pub type DomainResult<T> = Result<T, DomainError>;

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    mod error_codes {
        use super::*;

        #[test]
        fn validation_errors_in_range() {
            let errors = [
                DomainError::InvalidPrice("test".to_string()),
                DomainError::InvalidQuantity("test".to_string()),
                DomainError::InvalidState("test".to_string()),
                DomainError::InvalidSymbol("test".to_string()),
                DomainError::InvalidId("test".to_string()),
                DomainError::InvalidTimestamp("test".to_string()),
                DomainError::ValidationError("test".to_string()),
            ];

            for error in errors {
                let code = error.code();
                assert!(
                    (1000..2000).contains(&code),
                    "Expected validation error code 1000-1999, got {}",
                    code
                );
                assert!(error.is_validation_error());
                assert_eq!(error.category(), "validation");
            }
        }

        #[test]
        fn state_errors_in_range() {
            let errors = [
                DomainError::InvalidStateTransition {
                    from: RfqState::Created,
                    to: RfqState::Executed,
                },
                DomainError::QuoteExpired("test".to_string()),
                DomainError::QuoteNotFound("test".to_string()),
                DomainError::RfqNotFound("test".to_string()),
                DomainError::TradeNotFound("test".to_string()),
                DomainError::AlreadyExists("test".to_string()),
                DomainError::OperationNotAllowed("test".to_string()),
            ];

            for error in errors {
                let code = error.code();
                assert!(
                    (2000..3000).contains(&code),
                    "Expected state error code 2000-2999, got {}",
                    code
                );
                assert!(error.is_state_error());
                assert_eq!(error.category(), "state");
            }
        }

        #[test]
        fn compliance_errors_in_range() {
            let errors = [
                DomainError::ComplianceBlocked("test".to_string()),
                DomainError::KycFailed("test".to_string()),
                DomainError::CounterpartyNotAuthorized("test".to_string()),
                DomainError::TradingLimitExceeded("test".to_string()),
                DomainError::InstrumentNotAllowed("test".to_string()),
            ];

            for error in errors {
                let code = error.code();
                assert!(
                    (3000..4000).contains(&code),
                    "Expected compliance error code 3000-3999, got {}",
                    code
                );
                assert!(error.is_compliance_error());
                assert_eq!(error.category(), "compliance");
            }
        }

        #[test]
        fn arithmetic_errors_in_range() {
            let errors = [
                DomainError::Overflow,
                DomainError::Underflow,
                DomainError::DivisionByZero,
                DomainError::InvalidArithmeticValue("test".to_string()),
            ];

            for error in errors {
                let code = error.code();
                assert!(
                    (4000..5000).contains(&code),
                    "Expected arithmetic error code 4000-4999, got {}",
                    code
                );
                assert!(error.is_arithmetic_error());
                assert_eq!(error.category(), "arithmetic");
            }
        }
    }

    mod display {
        use super::*;

        #[test]
        fn validation_error_display() {
            let error = DomainError::InvalidPrice("must be positive".to_string());
            assert_eq!(error.to_string(), "invalid price: must be positive");
        }

        #[test]
        fn state_transition_error_display() {
            let error = DomainError::InvalidStateTransition {
                from: RfqState::Created,
                to: RfqState::Executed,
            };
            assert_eq!(
                error.to_string(),
                "invalid state transition from CREATED to EXECUTED"
            );
        }

        #[test]
        fn compliance_error_display() {
            let error = DomainError::ComplianceBlocked("sanctioned entity".to_string());
            assert_eq!(error.to_string(), "compliance blocked: sanctioned entity");
        }

        #[test]
        fn arithmetic_error_display() {
            assert_eq!(DomainError::Overflow.to_string(), "arithmetic overflow");
            assert_eq!(DomainError::Underflow.to_string(), "arithmetic underflow");
            assert_eq!(DomainError::DivisionByZero.to_string(), "division by zero");
        }
    }

    mod from_arithmetic_error {
        use super::*;

        #[test]
        fn overflow_converts() {
            let domain_err: DomainError = ArithmeticError::Overflow.into();
            assert_eq!(domain_err, DomainError::Overflow);
        }

        #[test]
        fn underflow_converts() {
            let domain_err: DomainError = ArithmeticError::Underflow.into();
            assert_eq!(domain_err, DomainError::Underflow);
        }

        #[test]
        fn division_by_zero_converts() {
            let domain_err: DomainError = ArithmeticError::DivisionByZero.into();
            assert_eq!(domain_err, DomainError::DivisionByZero);
        }

        #[test]
        fn invalid_value_converts() {
            let domain_err: DomainError = ArithmeticError::InvalidValue("negative").into();
            assert_eq!(
                domain_err,
                DomainError::InvalidArithmeticValue("negative".to_string())
            );
        }
    }

    mod specific_codes {
        use super::*;

        #[test]
        fn specific_error_codes() {
            assert_eq!(DomainError::InvalidPrice("".to_string()).code(), 1001);
            assert_eq!(DomainError::InvalidQuantity("".to_string()).code(), 1002);
            assert_eq!(DomainError::InvalidState("".to_string()).code(), 1003);
            assert_eq!(
                DomainError::InvalidStateTransition {
                    from: RfqState::Created,
                    to: RfqState::Executed
                }
                .code(),
                2001
            );
            assert_eq!(DomainError::QuoteExpired("".to_string()).code(), 2002);
            assert_eq!(DomainError::ComplianceBlocked("".to_string()).code(), 3001);
            assert_eq!(DomainError::KycFailed("".to_string()).code(), 3002);
            assert_eq!(DomainError::Overflow.code(), 4001);
            assert_eq!(DomainError::Underflow.code(), 4002);
            assert_eq!(DomainError::DivisionByZero.code(), 4003);
        }
    }
}
