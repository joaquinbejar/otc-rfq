//! # Off-Book Execution Events
//!
//! Domain events for off-book block trade execution.
//!
//! These events capture significant state changes during off-book execution
//! for audit, compliance, and integration purposes.

use crate::domain::entities::block_trade::BlockTradeId;
use crate::domain::events::domain_event::{DomainEvent, EventMetadata, EventType};
use crate::domain::value_objects::timestamp::Timestamp;
use crate::domain::value_objects::{CounterpartyId, EventId, RfqId};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// Step in the off-book execution flow where failure occurred.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[repr(u8)]
pub enum ExecutionStep {
    /// Risk check step.
    RiskCheck = 0,
    /// Collateral locking step.
    CollateralLock = 1,
    /// On-chain settlement step.
    Settlement = 2,
    /// Position update step.
    PositionUpdate = 3,
    /// Report scheduling step.
    ReportScheduling = 4,
}

impl fmt::Display for ExecutionStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RiskCheck => write!(f, "RISK_CHECK"),
            Self::CollateralLock => write!(f, "COLLATERAL_LOCK"),
            Self::Settlement => write!(f, "SETTLEMENT"),
            Self::PositionUpdate => write!(f, "POSITION_UPDATE"),
            Self::ReportScheduling => write!(f, "REPORT_SCHEDULING"),
        }
    }
}

/// Trade hash for on-chain verification.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TradeHash(String);

impl TradeHash {
    /// Creates a new trade hash.
    #[must_use]
    pub fn new(hash: impl Into<String>) -> Self {
        Self(hash.into())
    }

    /// Returns the hash as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TradeHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Event emitted when off-book execution starts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OffBookExecutionStarted {
    /// Event metadata.
    pub metadata: EventMetadata,
    /// Block trade ID.
    pub block_trade_id: BlockTradeId,
    /// Buyer counterparty ID.
    pub buyer_id: CounterpartyId,
    /// Seller counterparty ID.
    pub seller_id: CounterpartyId,
}

impl OffBookExecutionStarted {
    /// Creates a new off-book execution started event.
    #[must_use]
    pub fn new(
        block_trade_id: BlockTradeId,
        buyer_id: CounterpartyId,
        seller_id: CounterpartyId,
    ) -> Self {
        Self {
            metadata: EventMetadata::new(None),
            block_trade_id,
            buyer_id,
            seller_id,
        }
    }
}

impl DomainEvent for OffBookExecutionStarted {
    fn event_id(&self) -> EventId {
        self.metadata.event_id
    }

    fn rfq_id(&self) -> Option<RfqId> {
        self.metadata.rfq_id
    }

    fn timestamp(&self) -> Timestamp {
        self.metadata.timestamp
    }

    fn event_type(&self) -> EventType {
        EventType::Trade
    }

    fn event_name(&self) -> &'static str {
        "OffBookExecutionStarted"
    }
}

/// Event emitted when collateral is locked for both parties.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollateralLocked {
    /// Event metadata.
    pub metadata: EventMetadata,
    /// Block trade ID.
    pub block_trade_id: BlockTradeId,
    /// Lock identifier.
    pub lock_id: Uuid,
    /// Amount locked for buyer.
    pub buyer_amount: Decimal,
    /// Amount locked for seller.
    pub seller_amount: Decimal,
}

impl CollateralLocked {
    /// Creates a new collateral locked event.
    #[must_use]
    pub fn new(
        block_trade_id: BlockTradeId,
        lock_id: Uuid,
        buyer_amount: Decimal,
        seller_amount: Decimal,
    ) -> Self {
        Self {
            metadata: EventMetadata::new(None),
            block_trade_id,
            lock_id,
            buyer_amount,
            seller_amount,
        }
    }
}

impl DomainEvent for CollateralLocked {
    fn event_id(&self) -> EventId {
        self.metadata.event_id
    }

    fn rfq_id(&self) -> Option<RfqId> {
        self.metadata.rfq_id
    }

    fn timestamp(&self) -> Timestamp {
        self.metadata.timestamp
    }

    fn event_type(&self) -> EventType {
        EventType::Settlement
    }

    fn event_name(&self) -> &'static str {
        "CollateralLocked"
    }
}

/// Event emitted when off-book settlement completes successfully.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OffBookSettled {
    /// Event metadata.
    pub metadata: EventMetadata,
    /// Block trade ID.
    pub block_trade_id: BlockTradeId,
    /// On-chain trade hash.
    pub trade_hash: TradeHash,
    /// When settlement completed.
    pub settlement_timestamp: Timestamp,
}

impl OffBookSettled {
    /// Creates a new off-book settled event.
    #[must_use]
    pub fn new(
        block_trade_id: BlockTradeId,
        trade_hash: TradeHash,
        settlement_timestamp: Timestamp,
    ) -> Self {
        Self {
            metadata: EventMetadata::new(None),
            block_trade_id,
            trade_hash,
            settlement_timestamp,
        }
    }
}

impl DomainEvent for OffBookSettled {
    fn event_id(&self) -> EventId {
        self.metadata.event_id
    }

    fn rfq_id(&self) -> Option<RfqId> {
        self.metadata.rfq_id
    }

    fn timestamp(&self) -> Timestamp {
        self.metadata.timestamp
    }

    fn event_type(&self) -> EventType {
        EventType::Settlement
    }

    fn event_name(&self) -> &'static str {
        "OffBookSettled"
    }
}

/// Event emitted when off-book execution fails.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OffBookFailed {
    /// Event metadata.
    pub metadata: EventMetadata,
    /// Block trade ID.
    pub block_trade_id: BlockTradeId,
    /// Failure reason.
    pub reason: String,
    /// Step where failure occurred.
    pub step_failed: ExecutionStep,
}

impl OffBookFailed {
    /// Creates a new off-book failed event.
    #[must_use]
    pub fn new(block_trade_id: BlockTradeId, reason: String, step_failed: ExecutionStep) -> Self {
        Self {
            metadata: EventMetadata::new(None),
            block_trade_id,
            reason,
            step_failed,
        }
    }
}

impl DomainEvent for OffBookFailed {
    fn event_id(&self) -> EventId {
        self.metadata.event_id
    }

    fn rfq_id(&self) -> Option<RfqId> {
        self.metadata.rfq_id
    }

    fn timestamp(&self) -> Timestamp {
        self.metadata.timestamp
    }

    fn event_type(&self) -> EventType {
        EventType::Trade
    }

    fn event_name(&self) -> &'static str {
        "OffBookFailed"
    }
}

/// Event emitted when collateral is released (on failure/rollback).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollateralReleased {
    /// Event metadata.
    pub metadata: EventMetadata,
    /// Block trade ID.
    pub block_trade_id: BlockTradeId,
    /// Lock identifier that was released.
    pub lock_id: Uuid,
    /// Reason for release.
    pub reason: String,
}

impl CollateralReleased {
    /// Creates a new collateral released event.
    #[must_use]
    pub fn new(block_trade_id: BlockTradeId, lock_id: Uuid, reason: String) -> Self {
        Self {
            metadata: EventMetadata::new(None),
            block_trade_id,
            lock_id,
            reason,
        }
    }
}

impl DomainEvent for CollateralReleased {
    fn event_id(&self) -> EventId {
        self.metadata.event_id
    }

    fn rfq_id(&self) -> Option<RfqId> {
        self.metadata.rfq_id
    }

    fn timestamp(&self) -> Timestamp {
        self.metadata.timestamp
    }

    fn event_type(&self) -> EventType {
        EventType::Settlement
    }

    fn event_name(&self) -> &'static str {
        "CollateralReleased"
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn test_block_trade_id() -> BlockTradeId {
        BlockTradeId::new_v4()
    }

    #[test]
    fn execution_step_display() {
        assert_eq!(ExecutionStep::RiskCheck.to_string(), "RISK_CHECK");
        assert_eq!(ExecutionStep::CollateralLock.to_string(), "COLLATERAL_LOCK");
        assert_eq!(ExecutionStep::Settlement.to_string(), "SETTLEMENT");
        assert_eq!(ExecutionStep::PositionUpdate.to_string(), "POSITION_UPDATE");
        assert_eq!(
            ExecutionStep::ReportScheduling.to_string(),
            "REPORT_SCHEDULING"
        );
    }

    #[test]
    fn trade_hash_creation() {
        let hash = TradeHash::new("0x1234abcd");
        assert_eq!(hash.as_str(), "0x1234abcd");
        assert_eq!(hash.to_string(), "0x1234abcd");
    }

    #[test]
    fn off_book_execution_started_event() {
        let event = OffBookExecutionStarted::new(
            test_block_trade_id(),
            CounterpartyId::new("buyer-1"),
            CounterpartyId::new("seller-1"),
        );

        assert_eq!(event.event_name(), "OffBookExecutionStarted");
        assert_eq!(event.event_type(), EventType::Trade);
        assert!(event.rfq_id().is_none());
    }

    #[test]
    fn collateral_locked_event() {
        let event = CollateralLocked::new(
            test_block_trade_id(),
            Uuid::new_v4(),
            Decimal::new(10000, 2),
            Decimal::new(10000, 2),
        );

        assert_eq!(event.event_name(), "CollateralLocked");
        assert_eq!(event.event_type(), EventType::Settlement);
    }

    #[test]
    fn off_book_settled_event() {
        let event = OffBookSettled::new(
            test_block_trade_id(),
            TradeHash::new("0xabc123"),
            Timestamp::now(),
        );

        assert_eq!(event.event_name(), "OffBookSettled");
        assert_eq!(event.event_type(), EventType::Settlement);
    }

    #[test]
    fn off_book_failed_event() {
        let event = OffBookFailed::new(
            test_block_trade_id(),
            "Risk check failed".to_string(),
            ExecutionStep::RiskCheck,
        );

        assert_eq!(event.event_name(), "OffBookFailed");
        assert_eq!(event.event_type(), EventType::Trade);
        assert_eq!(event.step_failed, ExecutionStep::RiskCheck);
    }

    #[test]
    fn collateral_released_event() {
        let event = CollateralReleased::new(
            test_block_trade_id(),
            Uuid::new_v4(),
            "Settlement failed".to_string(),
        );

        assert_eq!(event.event_name(), "CollateralReleased");
        assert_eq!(event.event_type(), EventType::Settlement);
    }
}
