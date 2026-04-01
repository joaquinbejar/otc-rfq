//! # Block Trade Events
//!
//! Domain events for bilateral block trade lifecycle.
//!
//! These events capture significant state changes in block trades
//! for audit, compliance, and integration purposes.

use crate::domain::entities::block_trade::{BlockTradeId, BlockTradeState, BlockTradeValidation};
use crate::domain::events::domain_event::{DomainEvent, EventMetadata, EventType};
use crate::domain::services::ReportingTier;
use crate::domain::value_objects::timestamp::Timestamp;
use crate::domain::value_objects::{CounterpartyId, EventId, Instrument, Price, Quantity, RfqId};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Event emitted when a block trade is submitted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockTradeSubmitted {
    /// Event metadata.
    pub metadata: EventMetadata,
    /// Block trade ID.
    pub block_trade_id: BlockTradeId,
    /// Buyer counterparty ID.
    pub buyer_id: CounterpartyId,
    /// Seller counterparty ID.
    pub seller_id: CounterpartyId,
    /// Instrument being traded.
    pub instrument: Instrument,
    /// Agreed price.
    pub price: Price,
    /// Agreed quantity.
    pub quantity: Quantity,
    /// When the parties agreed on terms.
    pub agreed_at: Timestamp,
}

impl BlockTradeSubmitted {
    /// Creates a new block trade submitted event.
    #[must_use]
    pub fn new(
        block_trade_id: BlockTradeId,
        buyer_id: CounterpartyId,
        seller_id: CounterpartyId,
        instrument: Instrument,
        price: Price,
        quantity: Quantity,
        agreed_at: Timestamp,
    ) -> Self {
        Self {
            metadata: EventMetadata::new(None),
            block_trade_id,
            buyer_id,
            seller_id,
            instrument,
            price,
            quantity,
            agreed_at,
        }
    }
}

impl DomainEvent for BlockTradeSubmitted {
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
        "BlockTradeSubmitted"
    }
}

/// Event emitted when block trade validation completes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockTradeValidated {
    /// Event metadata.
    pub metadata: EventMetadata,
    /// Block trade ID.
    pub block_trade_id: BlockTradeId,
    /// Whether validation passed.
    pub passed: bool,
    /// Validation details.
    pub validation: BlockTradeValidation,
    /// Reporting tier (if validation passed).
    pub reporting_tier: Option<ReportingTier>,
}

impl BlockTradeValidated {
    /// Creates a new block trade validated event.
    #[must_use]
    pub fn new(
        block_trade_id: BlockTradeId,
        validation: BlockTradeValidation,
        reporting_tier: Option<ReportingTier>,
    ) -> Self {
        let passed = validation.is_valid();
        Self {
            metadata: EventMetadata::new(None),
            block_trade_id,
            passed,
            validation,
            reporting_tier,
        }
    }
}

impl DomainEvent for BlockTradeValidated {
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
        "BlockTradeValidated"
    }
}

/// Event emitted when a counterparty confirms a block trade.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockTradeConfirmed {
    /// Event metadata.
    pub metadata: EventMetadata,
    /// Block trade ID.
    pub block_trade_id: BlockTradeId,
    /// Counterparty that confirmed.
    pub counterparty_id: CounterpartyId,
    /// Whether this is the buyer or seller.
    pub role: BlockTradeRole,
    /// Whether both parties have now confirmed.
    pub fully_confirmed: bool,
}

/// Role in a block trade.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BlockTradeRole {
    /// The buyer.
    Buyer,
    /// The seller.
    Seller,
}

impl fmt::Display for BlockTradeRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Buyer => write!(f, "BUYER"),
            Self::Seller => write!(f, "SELLER"),
        }
    }
}

impl BlockTradeConfirmed {
    /// Creates a new block trade confirmed event.
    #[must_use]
    pub fn new(
        block_trade_id: BlockTradeId,
        counterparty_id: CounterpartyId,
        role: BlockTradeRole,
        fully_confirmed: bool,
    ) -> Self {
        Self {
            metadata: EventMetadata::new(None),
            block_trade_id,
            counterparty_id,
            role,
            fully_confirmed,
        }
    }
}

impl DomainEvent for BlockTradeConfirmed {
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
        "BlockTradeConfirmed"
    }
}

/// Event emitted when a block trade is approved (both parties confirmed).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockTradeApproved {
    /// Event metadata.
    pub metadata: EventMetadata,
    /// Block trade ID.
    pub block_trade_id: BlockTradeId,
    /// Reporting tier.
    pub reporting_tier: ReportingTier,
}

impl BlockTradeApproved {
    /// Creates a new block trade approved event.
    #[must_use]
    pub fn new(block_trade_id: BlockTradeId, reporting_tier: ReportingTier) -> Self {
        Self {
            metadata: EventMetadata::new(None),
            block_trade_id,
            reporting_tier,
        }
    }
}

impl DomainEvent for BlockTradeApproved {
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
        "BlockTradeApproved"
    }
}

/// Event emitted when a block trade is rejected.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockTradeRejected {
    /// Event metadata.
    pub metadata: EventMetadata,
    /// Block trade ID.
    pub block_trade_id: BlockTradeId,
    /// Rejection reason.
    pub reason: String,
    /// State when rejected.
    pub rejected_from_state: BlockTradeState,
}

impl BlockTradeRejected {
    /// Creates a new block trade rejected event.
    #[must_use]
    pub fn new(
        block_trade_id: BlockTradeId,
        reason: String,
        rejected_from_state: BlockTradeState,
    ) -> Self {
        Self {
            metadata: EventMetadata::new(None),
            block_trade_id,
            reason,
            rejected_from_state,
        }
    }
}

impl DomainEvent for BlockTradeRejected {
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
        "BlockTradeRejected"
    }
}

/// Event emitted when a block trade is executed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockTradeExecuted {
    /// Event metadata.
    pub metadata: EventMetadata,
    /// Block trade ID.
    pub block_trade_id: BlockTradeId,
    /// Final execution price.
    pub price: Price,
    /// Final execution quantity.
    pub quantity: Quantity,
    /// Reporting tier.
    pub reporting_tier: ReportingTier,
}

impl BlockTradeExecuted {
    /// Creates a new block trade executed event.
    #[must_use]
    pub fn new(
        block_trade_id: BlockTradeId,
        price: Price,
        quantity: Quantity,
        reporting_tier: ReportingTier,
    ) -> Self {
        Self {
            metadata: EventMetadata::new(None),
            block_trade_id,
            price,
            quantity,
            reporting_tier,
        }
    }
}

impl DomainEvent for BlockTradeExecuted {
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
        "BlockTradeExecuted"
    }
}

/// Event emitted when a block trade execution fails.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockTradeFailed {
    /// Event metadata.
    pub metadata: EventMetadata,
    /// Block trade ID.
    pub block_trade_id: BlockTradeId,
    /// Failure reason.
    pub reason: String,
}

impl BlockTradeFailed {
    /// Creates a new block trade failed event.
    #[must_use]
    pub fn new(block_trade_id: BlockTradeId, reason: String) -> Self {
        Self {
            metadata: EventMetadata::new(None),
            block_trade_id,
            reason,
        }
    }
}

impl DomainEvent for BlockTradeFailed {
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
        "BlockTradeFailed"
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::domain::value_objects::{AssetClass, Symbol};

    fn create_test_instrument() -> Instrument {
        let symbol = Symbol::new("BTC/USD").unwrap();
        Instrument::builder(symbol, AssetClass::CryptoSpot).build()
    }

    #[test]
    fn block_trade_submitted_event() {
        let event = BlockTradeSubmitted::new(
            BlockTradeId::new_v4(),
            CounterpartyId::new("buyer-1"),
            CounterpartyId::new("seller-1"),
            create_test_instrument(),
            Price::new(50000.0).unwrap(),
            Quantity::new(30.0).unwrap(),
            Timestamp::now(),
        );

        assert_eq!(event.event_name(), "BlockTradeSubmitted");
        assert_eq!(event.event_type(), EventType::Trade);
    }

    #[test]
    fn block_trade_validated_event() {
        let validation = BlockTradeValidation::passed();
        let event = BlockTradeValidated::new(
            BlockTradeId::new_v4(),
            validation,
            Some(ReportingTier::Standard),
        );

        assert!(event.passed);
        assert_eq!(event.event_name(), "BlockTradeValidated");
    }

    #[test]
    fn block_trade_confirmed_event() {
        let event = BlockTradeConfirmed::new(
            BlockTradeId::new_v4(),
            CounterpartyId::new("buyer-1"),
            BlockTradeRole::Buyer,
            false,
        );

        assert_eq!(event.role, BlockTradeRole::Buyer);
        assert!(!event.fully_confirmed);
    }

    #[test]
    fn block_trade_approved_event() {
        let event = BlockTradeApproved::new(BlockTradeId::new_v4(), ReportingTier::Large);

        assert_eq!(event.reporting_tier, ReportingTier::Large);
        assert_eq!(event.event_name(), "BlockTradeApproved");
    }

    #[test]
    fn block_trade_rejected_event() {
        let event = BlockTradeRejected::new(
            BlockTradeId::new_v4(),
            "Validation failed".to_string(),
            BlockTradeState::Validating,
        );

        assert_eq!(event.reason, "Validation failed");
        assert_eq!(event.rejected_from_state, BlockTradeState::Validating);
    }

    #[test]
    fn block_trade_executed_event() {
        let event = BlockTradeExecuted::new(
            BlockTradeId::new_v4(),
            Price::new(50000.0).unwrap(),
            Quantity::new(30.0).unwrap(),
            ReportingTier::Standard,
        );

        assert_eq!(event.event_name(), "BlockTradeExecuted");
    }

    #[test]
    fn block_trade_failed_event() {
        let event = BlockTradeFailed::new(BlockTradeId::new_v4(), "Settlement failed".to_string());

        assert_eq!(event.reason, "Settlement failed");
        assert_eq!(event.event_name(), "BlockTradeFailed");
    }

    #[test]
    fn block_trade_role_display() {
        assert_eq!(BlockTradeRole::Buyer.to_string(), "BUYER");
        assert_eq!(BlockTradeRole::Seller.to_string(), "SELLER");
    }
}
