//! # Venue Adapter Trait
//!
//! Port definition for venue integrations.
//!
//! This module defines the [`VenueAdapter`] trait that all venue integrations
//! must implement. It provides a uniform interface for requesting quotes,
//! executing trades, and checking venue health.
//!
//! # Examples
//!
//! ```ignore
//! use otc_rfq::infrastructure::venues::traits::{VenueAdapter, ExecutionResult};
//! use otc_rfq::infrastructure::venues::error::VenueResult;
//!
//! // Implement VenueAdapter for your venue
//! struct MyVenueAdapter { /* ... */ }
//!
//! #[async_trait::async_trait]
//! impl VenueAdapter for MyVenueAdapter {
//!     // ... implement required methods
//! }
//! ```

use crate::domain::entities::package_quote::PackageQuote;
use crate::domain::entities::quote::Quote;
use crate::domain::entities::rfq::Rfq;
use crate::domain::value_objects::strategy::Strategy;
use crate::domain::value_objects::timestamp::Timestamp;
use crate::domain::value_objects::{Price, Quantity, QuoteId, SettlementMethod, TradeId, VenueId};
use crate::infrastructure::venues::error::{VenueError, VenueResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Health status of a venue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VenueHealthStatus {
    /// Venue is healthy and operational.
    Healthy,
    /// Venue is degraded but operational.
    Degraded,
    /// Venue is unhealthy or unavailable.
    Unhealthy,
    /// Health status is unknown.
    Unknown,
}

impl fmt::Display for VenueHealthStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Healthy => write!(f, "HEALTHY"),
            Self::Degraded => write!(f, "DEGRADED"),
            Self::Unhealthy => write!(f, "UNHEALTHY"),
            Self::Unknown => write!(f, "UNKNOWN"),
        }
    }
}

/// Health information for a venue.
///
/// Contains the current health status and diagnostic information.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VenueHealth {
    /// The venue ID.
    venue_id: VenueId,
    /// Current health status.
    status: VenueHealthStatus,
    /// Latency in milliseconds (if available).
    latency_ms: Option<u64>,
    /// Optional message with additional details.
    message: Option<String>,
    /// When this health check was performed.
    checked_at: Timestamp,
}

impl VenueHealth {
    /// Creates a new healthy status.
    #[must_use]
    pub fn healthy(venue_id: VenueId) -> Self {
        Self {
            venue_id,
            status: VenueHealthStatus::Healthy,
            latency_ms: None,
            message: None,
            checked_at: Timestamp::now(),
        }
    }

    /// Creates a new healthy status with latency.
    #[must_use]
    pub fn healthy_with_latency(venue_id: VenueId, latency_ms: u64) -> Self {
        Self {
            venue_id,
            status: VenueHealthStatus::Healthy,
            latency_ms: Some(latency_ms),
            message: None,
            checked_at: Timestamp::now(),
        }
    }

    /// Creates a degraded status.
    #[must_use]
    pub fn degraded(venue_id: VenueId, message: impl Into<String>) -> Self {
        Self {
            venue_id,
            status: VenueHealthStatus::Degraded,
            latency_ms: None,
            message: Some(message.into()),
            checked_at: Timestamp::now(),
        }
    }

    /// Creates an unhealthy status.
    #[must_use]
    pub fn unhealthy(venue_id: VenueId, message: impl Into<String>) -> Self {
        Self {
            venue_id,
            status: VenueHealthStatus::Unhealthy,
            latency_ms: None,
            message: Some(message.into()),
            checked_at: Timestamp::now(),
        }
    }

    /// Creates an unknown status.
    #[must_use]
    pub fn unknown(venue_id: VenueId) -> Self {
        Self {
            venue_id,
            status: VenueHealthStatus::Unknown,
            latency_ms: None,
            message: None,
            checked_at: Timestamp::now(),
        }
    }

    /// Creates health from parts (for reconstruction).
    #[must_use]
    pub fn from_parts(
        venue_id: VenueId,
        status: VenueHealthStatus,
        latency_ms: Option<u64>,
        message: Option<String>,
        checked_at: Timestamp,
    ) -> Self {
        Self {
            venue_id,
            status,
            latency_ms,
            message,
            checked_at,
        }
    }

    /// Returns the venue ID.
    #[inline]
    #[must_use]
    pub fn venue_id(&self) -> &VenueId {
        &self.venue_id
    }

    /// Returns the health status.
    #[inline]
    #[must_use]
    pub fn status(&self) -> VenueHealthStatus {
        self.status
    }

    /// Returns the latency in milliseconds.
    #[inline]
    #[must_use]
    pub fn latency_ms(&self) -> Option<u64> {
        self.latency_ms
    }

    /// Returns the message.
    #[inline]
    #[must_use]
    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    /// Returns when this health check was performed.
    #[inline]
    #[must_use]
    pub fn checked_at(&self) -> Timestamp {
        self.checked_at
    }

    /// Returns true if the venue is healthy.
    #[inline]
    #[must_use]
    pub fn is_healthy(&self) -> bool {
        self.status == VenueHealthStatus::Healthy
    }

    /// Returns true if the venue is operational (healthy or degraded).
    #[inline]
    #[must_use]
    pub fn is_operational(&self) -> bool {
        matches!(
            self.status,
            VenueHealthStatus::Healthy | VenueHealthStatus::Degraded
        )
    }
}

impl fmt::Display for VenueHealth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "VenueHealth({}: {})", self.venue_id, self.status)?;
        if let Some(latency) = self.latency_ms {
            write!(f, " latency={}ms", latency)?;
        }
        Ok(())
    }
}

/// Result of a trade execution.
///
/// Contains details about the executed trade including the execution price,
/// quantity, and settlement information.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionResult {
    /// The trade ID assigned by the system.
    trade_id: TradeId,
    /// The quote that was executed.
    quote_id: QuoteId,
    /// The venue where the trade was executed.
    venue_id: VenueId,
    /// The execution price.
    execution_price: Price,
    /// The executed quantity.
    executed_quantity: Quantity,
    /// The settlement method.
    settlement_method: SettlementMethod,
    /// Venue-specific execution ID.
    venue_execution_id: Option<String>,
    /// Transaction hash for on-chain settlements.
    tx_hash: Option<String>,
    /// When the execution occurred.
    executed_at: Timestamp,
}

impl ExecutionResult {
    /// Creates a new execution result.
    #[must_use]
    pub fn new(
        quote_id: QuoteId,
        venue_id: VenueId,
        execution_price: Price,
        executed_quantity: Quantity,
        settlement_method: SettlementMethod,
    ) -> Self {
        Self {
            trade_id: TradeId::new_v4(),
            quote_id,
            venue_id,
            execution_price,
            executed_quantity,
            settlement_method,
            venue_execution_id: None,
            tx_hash: None,
            executed_at: Timestamp::now(),
        }
    }

    /// Creates an execution result from parts (for reconstruction).
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn from_parts(
        trade_id: TradeId,
        quote_id: QuoteId,
        venue_id: VenueId,
        execution_price: Price,
        executed_quantity: Quantity,
        settlement_method: SettlementMethod,
        venue_execution_id: Option<String>,
        tx_hash: Option<String>,
        executed_at: Timestamp,
    ) -> Self {
        Self {
            trade_id,
            quote_id,
            venue_id,
            execution_price,
            executed_quantity,
            settlement_method,
            venue_execution_id,
            tx_hash,
            executed_at,
        }
    }

    /// Sets the venue execution ID.
    #[must_use]
    pub fn with_venue_execution_id(mut self, id: impl Into<String>) -> Self {
        self.venue_execution_id = Some(id.into());
        self
    }

    /// Sets the transaction hash.
    #[must_use]
    pub fn with_tx_hash(mut self, hash: impl Into<String>) -> Self {
        self.tx_hash = Some(hash.into());
        self
    }

    /// Returns the trade ID.
    #[inline]
    #[must_use]
    pub fn trade_id(&self) -> TradeId {
        self.trade_id
    }

    /// Returns the quote ID.
    #[inline]
    #[must_use]
    pub fn quote_id(&self) -> QuoteId {
        self.quote_id
    }

    /// Returns the venue ID.
    #[inline]
    #[must_use]
    pub fn venue_id(&self) -> &VenueId {
        &self.venue_id
    }

    /// Returns the execution price.
    #[inline]
    #[must_use]
    pub fn execution_price(&self) -> Price {
        self.execution_price
    }

    /// Returns the executed quantity.
    #[inline]
    #[must_use]
    pub fn executed_quantity(&self) -> Quantity {
        self.executed_quantity
    }

    /// Returns the settlement method.
    #[inline]
    #[must_use]
    pub fn settlement_method(&self) -> SettlementMethod {
        self.settlement_method
    }

    /// Returns the venue execution ID.
    #[inline]
    #[must_use]
    pub fn venue_execution_id(&self) -> Option<&str> {
        self.venue_execution_id.as_deref()
    }

    /// Returns the transaction hash.
    #[inline]
    #[must_use]
    pub fn tx_hash(&self) -> Option<&str> {
        self.tx_hash.as_deref()
    }

    /// Returns when the execution occurred.
    #[inline]
    #[must_use]
    pub fn executed_at(&self) -> Timestamp {
        self.executed_at
    }

    /// Calculates the total notional value.
    #[must_use]
    pub fn notional_value(&self) -> Option<Price> {
        self.execution_price
            .safe_mul(self.executed_quantity.get())
            .ok()
    }
}

impl fmt::Display for ExecutionResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ExecutionResult({} @ {} from {})",
            self.executed_quantity, self.execution_price, self.venue_id
        )
    }
}

/// Trait defining the interface for venue adapters.
///
/// All venue integrations must implement this trait to provide a uniform
/// interface for the RFQ engine to interact with different liquidity sources.
///
/// # Async Methods
///
/// All methods are async to support non-blocking I/O operations when
/// communicating with external venues.
///
/// # Error Handling
///
/// Methods return `VenueResult<T>` which wraps `Result<T, VenueError>`.
/// Implementations should map venue-specific errors to appropriate
/// `VenueError` variants.
#[async_trait]
pub trait VenueAdapter: Send + Sync + fmt::Debug {
    /// Returns the venue ID.
    fn venue_id(&self) -> &VenueId;

    /// Returns the timeout in milliseconds for venue operations.
    fn timeout_ms(&self) -> u64;

    /// Requests a quote from the venue.
    ///
    /// # Arguments
    ///
    /// * `rfq` - The RFQ to request a quote for
    ///
    /// # Returns
    ///
    /// A quote from the venue, or an error if the quote cannot be obtained.
    ///
    /// # Errors
    ///
    /// - `VenueError::Timeout` - Request timed out
    /// - `VenueError::QuoteUnavailable` - Venue cannot provide a quote
    /// - `VenueError::InsufficientLiquidity` - Not enough liquidity
    /// - `VenueError::InvalidRequest` - Invalid RFQ parameters
    async fn request_quote(&self, rfq: &Rfq) -> VenueResult<Quote>;

    /// Executes a trade based on a quote.
    ///
    /// # Arguments
    ///
    /// * `quote` - The quote to execute
    ///
    /// # Returns
    ///
    /// The execution result, or an error if execution fails.
    ///
    /// # Errors
    ///
    /// - `VenueError::Timeout` - Request timed out
    /// - `VenueError::QuoteExpired` - Quote has expired
    /// - `VenueError::ExecutionFailed` - Trade execution failed
    /// - `VenueError::InsufficientLiquidity` - Liquidity no longer available
    async fn execute_trade(&self, quote: &Quote) -> VenueResult<ExecutionResult>;

    /// Performs a health check on the venue.
    ///
    /// # Returns
    ///
    /// The venue health status, or an error if the check fails.
    ///
    /// # Errors
    ///
    /// - `VenueError::Timeout` - Health check timed out
    /// - `VenueError::Connection` - Cannot connect to venue
    async fn health_check(&self) -> VenueResult<VenueHealth>;

    /// Returns true if the venue is currently available.
    ///
    /// Default implementation performs a health check and returns true
    /// if the venue is operational.
    async fn is_available(&self) -> bool {
        self.health_check()
            .await
            .map(|h| h.is_operational())
            .unwrap_or(false)
    }

    /// Returns true if the venue supports multi-leg quoting.
    ///
    /// Venues that support multi-leg quoting can provide package quotes
    /// for entire strategies with a single net price, potentially offering
    /// better pricing through internal hedging.
    ///
    /// Default implementation returns false. Override this method to
    /// indicate multi-leg support.
    fn supports_multi_leg(&self) -> bool {
        false
    }

    /// Requests a package quote for a multi-leg strategy.
    ///
    /// # Arguments
    ///
    /// * `strategy` - The multi-leg strategy to quote
    /// * `rfq` - The RFQ containing quantity and other parameters
    ///
    /// # Returns
    ///
    /// A package quote with net price and individual leg prices.
    ///
    /// # Errors
    ///
    /// - `VenueError::UnsupportedOperation` - Venue doesn't support multi-leg
    /// - `VenueError::Timeout` - Request timed out
    /// - `VenueError::QuoteUnavailable` - Venue cannot provide a quote
    /// - `VenueError::InsufficientLiquidity` - Not enough liquidity
    ///
    /// # Default Implementation
    ///
    /// Returns `VenueError::UnsupportedOperation` by default. Venues that
    /// support multi-leg quoting should override this method.
    async fn request_multi_leg_quote(
        &self,
        _strategy: &Strategy,
        _rfq: &Rfq,
    ) -> VenueResult<PackageQuote> {
        Err(VenueError::unsupported_operation("multi-leg quoting"))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::domain::value_objects::Blockchain;

    mod venue_health_status {
        use super::*;

        #[test]
        fn display() {
            assert_eq!(VenueHealthStatus::Healthy.to_string(), "HEALTHY");
            assert_eq!(VenueHealthStatus::Degraded.to_string(), "DEGRADED");
            assert_eq!(VenueHealthStatus::Unhealthy.to_string(), "UNHEALTHY");
            assert_eq!(VenueHealthStatus::Unknown.to_string(), "UNKNOWN");
        }
    }

    mod venue_health {
        use super::*;

        #[test]
        fn healthy() {
            let health = VenueHealth::healthy(VenueId::new("test"));
            assert!(health.is_healthy());
            assert!(health.is_operational());
            assert!(health.message().is_none());
        }

        #[test]
        fn healthy_with_latency() {
            let health = VenueHealth::healthy_with_latency(VenueId::new("test"), 50);
            assert!(health.is_healthy());
            assert_eq!(health.latency_ms(), Some(50));
        }

        #[test]
        fn degraded() {
            let health = VenueHealth::degraded(VenueId::new("test"), "High latency");
            assert!(!health.is_healthy());
            assert!(health.is_operational());
            assert_eq!(health.message(), Some("High latency"));
        }

        #[test]
        fn unhealthy() {
            let health = VenueHealth::unhealthy(VenueId::new("test"), "Connection failed");
            assert!(!health.is_healthy());
            assert!(!health.is_operational());
        }

        #[test]
        fn display() {
            let health = VenueHealth::healthy_with_latency(VenueId::new("binance"), 25);
            let display = health.to_string();
            assert!(display.contains("binance"));
            assert!(display.contains("HEALTHY"));
            assert!(display.contains("25ms"));
        }
    }

    mod execution_result {
        use super::*;

        fn test_execution() -> ExecutionResult {
            ExecutionResult::new(
                QuoteId::new_v4(),
                VenueId::new("test-venue"),
                Price::new(50000.0).expect("valid price"),
                Quantity::new(1.0).expect("valid quantity"),
                SettlementMethod::OnChain(Blockchain::Ethereum),
            )
        }

        #[test]
        fn new_creates_execution() {
            let exec = test_execution();
            assert_eq!(exec.venue_id(), &VenueId::new("test-venue"));
            assert!(exec.venue_execution_id().is_none());
            assert!(exec.tx_hash().is_none());
        }

        #[test]
        fn with_venue_execution_id() {
            let exec = test_execution().with_venue_execution_id("venue-123");
            assert_eq!(exec.venue_execution_id(), Some("venue-123"));
        }

        #[test]
        fn with_tx_hash() {
            let exec = test_execution().with_tx_hash("0xabc123");
            assert_eq!(exec.tx_hash(), Some("0xabc123"));
        }

        #[test]
        fn notional_value() {
            let exec = ExecutionResult::new(
                QuoteId::new_v4(),
                VenueId::new("test"),
                Price::new(100.0).expect("valid price"),
                Quantity::new(2.0).expect("valid quantity"),
                SettlementMethod::OffChain,
            );

            let notional = exec.notional_value().expect("should calculate");
            assert_eq!(notional, Price::new(200.0).expect("valid price"));
        }

        #[test]
        fn display() {
            let exec = test_execution();
            let display = exec.to_string();
            assert!(display.contains("test-venue"));
        }
    }
}
