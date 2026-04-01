//! # Multi-Leg Execution Events
//!
//! Domain events emitted during multi-leg strategy execution.
//!
//! These events provide an audit trail for the all-or-none execution
//! of multi-leg strategies, including rollback scenarios.

use crate::domain::value_objects::{
    Instrument, OrderSide, PackageQuoteId, Price, Quantity, Timestamp,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Unique identifier for a multi-leg execution.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MultiLegExecutionId(String);

impl MultiLegExecutionId {
    /// Creates a new multi-leg execution ID.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Creates a new random multi-leg execution ID.
    #[must_use]
    pub fn generate() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    /// Returns the ID as a string slice.
    #[must_use]
    #[inline]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for MultiLegExecutionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for MultiLegExecutionId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for MultiLegExecutionId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

/// Event emitted when multi-leg execution starts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiLegExecutionStarted {
    /// Unique execution ID.
    pub execution_id: MultiLegExecutionId,
    /// The package quote being executed.
    pub package_quote_id: PackageQuoteId,
    /// Total number of legs to execute.
    pub total_legs: usize,
    /// Net price for the entire package.
    pub net_price: Decimal,
    /// When execution started.
    pub started_at: Timestamp,
}

impl MultiLegExecutionStarted {
    /// Creates a new multi-leg execution started event.
    #[must_use]
    pub fn new(
        execution_id: MultiLegExecutionId,
        package_quote_id: PackageQuoteId,
        total_legs: usize,
        net_price: Decimal,
    ) -> Self {
        Self {
            execution_id,
            package_quote_id,
            total_legs,
            net_price,
            started_at: Timestamp::now(),
        }
    }
}

/// Event emitted when a single leg is successfully executed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LegExecuted {
    /// The parent execution ID.
    pub execution_id: MultiLegExecutionId,
    /// Index of the executed leg (0-based).
    pub leg_index: usize,
    /// Instrument for this leg.
    pub instrument: Instrument,
    /// Side of the leg (Buy or Sell).
    pub side: OrderSide,
    /// Execution price.
    pub price: Price,
    /// Executed quantity.
    pub quantity: Quantity,
    /// Venue-assigned execution ID.
    pub venue_execution_id: String,
    /// When the leg was executed.
    pub executed_at: Timestamp,
}

impl LegExecuted {
    /// Creates a new leg executed event.
    #[must_use]
    pub fn new(
        execution_id: MultiLegExecutionId,
        leg_index: usize,
        instrument: Instrument,
        side: OrderSide,
        price: Price,
        quantity: Quantity,
        venue_execution_id: String,
    ) -> Self {
        Self {
            execution_id,
            leg_index,
            instrument,
            side,
            price,
            quantity,
            venue_execution_id,
            executed_at: Timestamp::now(),
        }
    }
}

/// Event emitted when a leg execution fails.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LegFailed {
    /// The parent execution ID.
    pub execution_id: MultiLegExecutionId,
    /// Index of the failed leg (0-based).
    pub leg_index: usize,
    /// Instrument for this leg.
    pub instrument: Instrument,
    /// Reason for failure.
    pub reason: String,
    /// Number of legs successfully executed before failure.
    pub legs_executed_before_failure: usize,
    /// When the failure occurred.
    pub failed_at: Timestamp,
}

impl LegFailed {
    /// Creates a new leg failed event.
    #[must_use]
    pub fn new(
        execution_id: MultiLegExecutionId,
        leg_index: usize,
        instrument: Instrument,
        reason: String,
        legs_executed_before_failure: usize,
    ) -> Self {
        Self {
            execution_id,
            leg_index,
            instrument,
            reason,
            legs_executed_before_failure,
            failed_at: Timestamp::now(),
        }
    }
}

/// Event emitted when rollback starts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiLegRollbackStarted {
    /// The parent execution ID.
    pub execution_id: MultiLegExecutionId,
    /// Reason for rollback.
    pub reason: String,
    /// Number of legs to roll back.
    pub legs_to_rollback: usize,
    /// When rollback started.
    pub started_at: Timestamp,
}

impl MultiLegRollbackStarted {
    /// Creates a new rollback started event.
    #[must_use]
    pub fn new(execution_id: MultiLegExecutionId, reason: String, legs_to_rollback: usize) -> Self {
        Self {
            execution_id,
            reason,
            legs_to_rollback,
            started_at: Timestamp::now(),
        }
    }
}

/// Event emitted when rollback completes successfully.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiLegRollbackCompleted {
    /// The parent execution ID.
    pub execution_id: MultiLegExecutionId,
    /// Number of legs rolled back.
    pub legs_rolled_back: usize,
    /// When rollback completed.
    pub completed_at: Timestamp,
}

impl MultiLegRollbackCompleted {
    /// Creates a new rollback completed event.
    #[must_use]
    pub fn new(execution_id: MultiLegExecutionId, legs_rolled_back: usize) -> Self {
        Self {
            execution_id,
            legs_rolled_back,
            completed_at: Timestamp::now(),
        }
    }
}

/// Event emitted when multi-leg execution completes successfully.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiLegExecutionCompleted {
    /// The execution ID.
    pub execution_id: MultiLegExecutionId,
    /// The package quote that was executed.
    pub package_quote_id: PackageQuoteId,
    /// Total legs executed.
    pub total_legs: usize,
    /// Net price achieved.
    pub net_price: Decimal,
    /// Total execution time in milliseconds.
    pub execution_time_ms: u64,
    /// When execution completed.
    pub completed_at: Timestamp,
}

impl MultiLegExecutionCompleted {
    /// Creates a new execution completed event.
    #[must_use]
    pub fn new(
        execution_id: MultiLegExecutionId,
        package_quote_id: PackageQuoteId,
        total_legs: usize,
        net_price: Decimal,
        execution_time_ms: u64,
    ) -> Self {
        Self {
            execution_id,
            package_quote_id,
            total_legs,
            net_price,
            execution_time_ms,
            completed_at: Timestamp::now(),
        }
    }
}

/// Event emitted when execution ends in partial failure (rollback also failed).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiLegPartialFailure {
    /// The execution ID.
    pub execution_id: MultiLegExecutionId,
    /// Number of legs that were executed.
    pub legs_executed: usize,
    /// Number of legs successfully rolled back.
    pub legs_rolled_back: usize,
    /// Number of legs that failed to roll back.
    pub legs_failed_rollback: usize,
    /// Original failure reason.
    pub original_failure: String,
    /// Rollback failure reason.
    pub rollback_failure: String,
    /// When the partial failure occurred.
    pub failed_at: Timestamp,
}

impl MultiLegPartialFailure {
    /// Creates a new partial failure event.
    #[must_use]
    pub fn new(
        execution_id: MultiLegExecutionId,
        legs_executed: usize,
        legs_rolled_back: usize,
        legs_failed_rollback: usize,
        original_failure: String,
        rollback_failure: String,
    ) -> Self {
        Self {
            execution_id,
            legs_executed,
            legs_rolled_back,
            legs_failed_rollback,
            original_failure,
            rollback_failure,
            failed_at: Timestamp::now(),
        }
    }
}

/// Enum encompassing all multi-leg execution events.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MultiLegExecutionEvent {
    /// Execution started.
    Started(MultiLegExecutionStarted),
    /// A leg was executed.
    LegExecuted(LegExecuted),
    /// A leg failed.
    LegFailed(LegFailed),
    /// Rollback started.
    RollbackStarted(MultiLegRollbackStarted),
    /// Rollback completed.
    RollbackCompleted(MultiLegRollbackCompleted),
    /// Execution completed successfully.
    Completed(MultiLegExecutionCompleted),
    /// Partial failure (rollback also failed).
    PartialFailure(MultiLegPartialFailure),
}

impl From<MultiLegExecutionStarted> for MultiLegExecutionEvent {
    fn from(event: MultiLegExecutionStarted) -> Self {
        Self::Started(event)
    }
}

impl From<LegExecuted> for MultiLegExecutionEvent {
    fn from(event: LegExecuted) -> Self {
        Self::LegExecuted(event)
    }
}

impl From<LegFailed> for MultiLegExecutionEvent {
    fn from(event: LegFailed) -> Self {
        Self::LegFailed(event)
    }
}

impl From<MultiLegRollbackStarted> for MultiLegExecutionEvent {
    fn from(event: MultiLegRollbackStarted) -> Self {
        Self::RollbackStarted(event)
    }
}

impl From<MultiLegRollbackCompleted> for MultiLegExecutionEvent {
    fn from(event: MultiLegRollbackCompleted) -> Self {
        Self::RollbackCompleted(event)
    }
}

impl From<MultiLegExecutionCompleted> for MultiLegExecutionEvent {
    fn from(event: MultiLegExecutionCompleted) -> Self {
        Self::Completed(event)
    }
}

impl From<MultiLegPartialFailure> for MultiLegExecutionEvent {
    fn from(event: MultiLegPartialFailure) -> Self {
        Self::PartialFailure(event)
    }
}
