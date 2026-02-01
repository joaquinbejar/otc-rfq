//! # Domain Errors
//!
//! Typed error types for domain operations.
//!
//! Error codes are organized by category:
//! - 1000-1999: Validation errors
//! - 2000-2999: State errors
//! - 3000-3999: Compliance errors
//! - 4000-4999: Arithmetic errors
//!
//! # Examples
//!
//! ```
//! use otc_rfq::domain::errors::{DomainError, DomainResult};
//!
//! fn validate_price(price: f64) -> DomainResult<f64> {
//!     if price <= 0.0 {
//!         return Err(DomainError::InvalidPrice("price must be positive".to_string()));
//!     }
//!     Ok(price)
//! }
//! ```

pub mod arithmetic_error;
pub mod domain_error;

pub use arithmetic_error::ArithmeticError;
pub use domain_error::{DomainError, DomainResult};
