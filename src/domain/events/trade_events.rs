//! # Trade Events
//!
//! Domain events for trade and settlement lifecycle.
//!
//! This module provides events that track trade execution and settlement.
//!
//! # Event Flow
//!
//! ```text
//! TradeExecuted -> SettlementInitiated -> SettlementConfirmed | SettlementFailed
//! ```

use crate::domain::errors::{DomainError, DomainResult};
use crate::domain::events::domain_event::{DomainEvent, EventMetadata, EventType};
use crate::domain::value_objects::timestamp::Timestamp;
use crate::domain::value_objects::{
    Blockchain, CounterpartyId, EventId, Instrument, OrderSide, Price, Quantity, QuoteId, RfqId,
    SettlementMethod, TradeId, VenueId,
};
use serde::{Deserialize, Serialize};

/// Event emitted when a trade is executed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TradeExecuted {
    /// Event metadata.
    pub metadata: EventMetadata,
    /// The trade ID.
    pub trade_id: TradeId,
    /// The quote that was executed.
    pub quote_id: QuoteId,
    /// The venue where the trade was executed.
    pub venue_id: VenueId,
    /// The counterparty (client).
    pub counterparty_id: CounterpartyId,
    /// The execution price.
    pub price: Price,
    /// The executed quantity.
    pub quantity: Quantity,
    /// The settlement method.
    pub settlement_method: SettlementMethod,
    /// Taker fee (positive = fee, negative = rebate).
    pub taker_fee: Option<rust_decimal::Decimal>,
    /// Maker fee (positive = fee, negative = rebate).
    pub maker_fee: Option<rust_decimal::Decimal>,
    /// Net fee (taker + maker).
    pub net_fee: Option<rust_decimal::Decimal>,
}

impl TradeExecuted {
    /// Creates a builder for [`TradeExecuted`] events.
    pub fn builder() -> TradeExecutedBuilder {
        TradeExecutedBuilder::default()
    }

    /// Creates a new TradeExecuted event.
    ///
    /// # Deprecated
    ///
    /// Use [`TradeExecuted::builder()`] instead. This method is kept for SBE deserialization compatibility.
    #[must_use]
    #[deprecated(
        since = "0.1.0",
        note = "please use `TradeExecuted::builder()` instead"
    )]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        rfq_id: RfqId,
        trade_id: TradeId,
        quote_id: QuoteId,
        venue_id: VenueId,
        counterparty_id: CounterpartyId,
        price: Price,
        quantity: Quantity,
        settlement_method: SettlementMethod,
    ) -> Self {
        Self {
            metadata: EventMetadata::for_rfq(rfq_id),
            trade_id,
            quote_id,
            venue_id,
            counterparty_id,
            price,
            quantity,
            settlement_method,
            taker_fee: None,
            maker_fee: None,
            net_fee: None,
        }
    }
}

/// Builder for constructing a [`TradeExecuted`] event.
///
/// # Example
///
/// ```
/// use otc_rfq::domain::events::trade_events::TradeExecuted;
/// use otc_rfq::domain::value_objects::*;
///
/// // All required fields must be set before calling `.build()`,
/// let event = TradeExecuted::builder()
///     .rfq_id(RfqId::new_v4())
///     .trade_id(TradeId::new_v4())
///     .quote_id(QuoteId::new_v4())
///     .venue_id(VenueId::new("venue-1"))
///     .counterparty_id(CounterpartyId::new("client-1"))
///     .price(Price::new(50000.0).unwrap())
///     .quantity(Quantity::new(1.0).unwrap())
///     .settlement_method(SettlementMethod::OffChain)
///     .taker_fee(rust_decimal::Decimal::new(50, 1))
///     .build();
/// ```
#[derive(Debug, Clone, Default)]
#[must_use = "builders do nothing unless .build() is called"]
pub struct TradeExecutedBuilder {
    rfq_id: Option<RfqId>,
    trade_id: Option<TradeId>,
    quote_id: Option<QuoteId>,
    venue_id: Option<VenueId>,
    counterparty_id: Option<CounterpartyId>,
    price: Option<Price>,
    quantity: Option<Quantity>,
    settlement_method: Option<SettlementMethod>,
    taker_fee: Option<rust_decimal::Decimal>,
    maker_fee: Option<rust_decimal::Decimal>,
    net_fee: Option<rust_decimal::Decimal>,
}

impl TradeExecutedBuilder {
    #[inline]
    fn missing_required_field(field: &str) -> DomainError {
        DomainError::ValidationError(format!("{field} is required"))
    }

    #[inline]
    fn build_from_fields(
        required: (
            RfqId,
            TradeId,
            QuoteId,
            VenueId,
            CounterpartyId,
            Price,
            Quantity,
            SettlementMethod,
        ),
        fees: (
            Option<rust_decimal::Decimal>,
            Option<rust_decimal::Decimal>,
            Option<rust_decimal::Decimal>,
        ),
    ) -> TradeExecuted {
        let (
            rfq_id,
            trade_id,
            quote_id,
            venue_id,
            counterparty_id,
            price,
            quantity,
            settlement_method,
        ) = required;
        let (taker_fee, maker_fee, net_fee) = fees;

        TradeExecuted {
            metadata: EventMetadata::for_rfq(rfq_id),
            trade_id,
            quote_id,
            venue_id,
            counterparty_id,
            price,
            quantity,
            settlement_method,
            taker_fee,
            maker_fee,
            net_fee,
        }
    }

    /// Sets the RFQ ID.
    pub fn rfq_id(mut self, value: RfqId) -> Self {
        self.rfq_id = Some(value);
        self
    }

    /// Sets the trade ID.
    pub fn trade_id(mut self, value: TradeId) -> Self {
        self.trade_id = Some(value);
        self
    }

    /// Sets the quote ID.
    pub fn quote_id(mut self, value: QuoteId) -> Self {
        self.quote_id = Some(value);
        self
    }

    /// Sets the venue ID.
    pub fn venue_id(mut self, value: VenueId) -> Self {
        self.venue_id = Some(value);
        self
    }

    /// Sets the counterparty ID.
    pub fn counterparty_id(mut self, value: CounterpartyId) -> Self {
        self.counterparty_id = Some(value);
        self
    }

    /// Sets the execution price.
    pub fn price(mut self, value: Price) -> Self {
        self.price = Some(value);
        self
    }

    /// Sets the executed quantity.
    pub fn quantity(mut self, value: Quantity) -> Self {
        self.quantity = Some(value);
        self
    }

    /// Sets the settlement method.
    pub fn settlement_method(mut self, value: SettlementMethod) -> Self {
        self.settlement_method = Some(value);
        self
    }

    /// Sets the taker fee.
    pub fn taker_fee(mut self, value: rust_decimal::Decimal) -> Self {
        self.taker_fee = Some(value);
        self
    }

    /// Sets the maker fee.
    pub fn maker_fee(mut self, value: rust_decimal::Decimal) -> Self {
        self.maker_fee = Some(value);
        self
    }

    /// Sets the net fee.
    pub fn net_fee(mut self, value: rust_decimal::Decimal) -> Self {
        self.net_fee = Some(value);
        self
    }

    /// Builds the `TradeExecuted` event.
    ///
    /// # Panics
    ///
    /// Panics if any required field is missing.
    ///
    /// # Note
    ///
    /// For safer construction that returns a `Result` instead of panicking,
    /// use [`try_build`](Self::try_build).
    #[must_use]
    #[allow(clippy::expect_used)]
    pub fn build(self) -> TradeExecuted {
        let Self {
            rfq_id,
            trade_id,
            quote_id,
            venue_id,
            counterparty_id,
            price,
            quantity,
            settlement_method,
            taker_fee,
            maker_fee,
            net_fee,
        } = self;

        let required_fields = (
            rfq_id.expect("rfq_id is required"),
            trade_id.expect("trade_id is required"),
            quote_id.expect("quote_id is required"),
            venue_id.expect("venue_id is required"),
            counterparty_id.expect("counterparty_id is required"),
            price.expect("price is required"),
            quantity.expect("quantity is required"),
            settlement_method.expect("settlement_method is required"),
        );

        Self::build_from_fields(required_fields, (taker_fee, maker_fee, net_fee))
    }

    /// Attempts to build the `TradeExecuted` event.
    ///
    /// Returns an error if any required field is missing.
    ///
    /// # Errors
    ///
    /// Returns `DomainError::ValidationError` if any required field is missing:
    /// - `rfq_id`
    /// - `trade_id`
    /// - `quote_id`
    /// - `venue_id`
    /// - `counterparty_id`
    /// - `price`
    /// - `quantity`
    /// - `settlement_method`
    ///
    /// # Examples
    ///
    /// ```
    /// use otc_rfq::domain::events::trade_events::TradeExecuted;
    /// use otc_rfq::domain::value_objects::*;
    ///
    /// let result = TradeExecuted::builder()
    ///     .rfq_id(RfqId::new_v4())
    ///     .trade_id(TradeId::new_v4())
    ///     .quote_id(QuoteId::new_v4())
    ///     .venue_id(VenueId::new("venue-1"))
    ///     .counterparty_id(CounterpartyId::new("client-1"))
    ///     .price(Price::new(50000.0).unwrap())
    ///     .quantity(Quantity::new(1.0).unwrap())
    ///     .settlement_method(SettlementMethod::OffChain)
    ///     .try_build();
    ///
    /// assert!(result.is_ok());
    /// ```
    pub fn try_build(self) -> DomainResult<TradeExecuted> {
        let Self {
            rfq_id,
            trade_id,
            quote_id,
            venue_id,
            counterparty_id,
            price,
            quantity,
            settlement_method,
            taker_fee,
            maker_fee,
            net_fee,
        } = self;

        let required_fields = (
            rfq_id.ok_or_else(|| Self::missing_required_field("rfq_id"))?,
            trade_id.ok_or_else(|| Self::missing_required_field("trade_id"))?,
            quote_id.ok_or_else(|| Self::missing_required_field("quote_id"))?,
            venue_id.ok_or_else(|| Self::missing_required_field("venue_id"))?,
            counterparty_id.ok_or_else(|| Self::missing_required_field("counterparty_id"))?,
            price.ok_or_else(|| Self::missing_required_field("price"))?,
            quantity.ok_or_else(|| Self::missing_required_field("quantity"))?,
            settlement_method.ok_or_else(|| Self::missing_required_field("settlement_method"))?,
        );

        Ok(Self::build_from_fields(
            required_fields,
            (taker_fee, maker_fee, net_fee),
        ))
    }
}

impl DomainEvent for TradeExecuted {
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
        "TradeExecuted"
    }
}

/// Event emitted when settlement is initiated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettlementInitiated {
    /// Event metadata.
    pub metadata: EventMetadata,
    /// The trade being settled.
    pub trade_id: TradeId,
    /// The settlement method.
    pub settlement_method: SettlementMethod,
    /// Transaction hash for on-chain settlement (if applicable).
    pub tx_hash: Option<String>,
}

impl SettlementInitiated {
    /// Creates a new SettlementInitiated event.
    #[must_use]
    pub fn new(
        rfq_id: RfqId,
        trade_id: TradeId,
        settlement_method: SettlementMethod,
        tx_hash: Option<String>,
    ) -> Self {
        Self {
            metadata: EventMetadata::for_rfq(rfq_id),
            trade_id,
            settlement_method,
            tx_hash,
        }
    }

    /// Creates a new SettlementInitiated event for on-chain settlement.
    #[must_use]
    pub fn on_chain(
        rfq_id: RfqId,
        trade_id: TradeId,
        blockchain: Blockchain,
        tx_hash: String,
    ) -> Self {
        Self::new(
            rfq_id,
            trade_id,
            SettlementMethod::OnChain(blockchain),
            Some(tx_hash),
        )
    }

    /// Creates a new SettlementInitiated event for off-chain settlement.
    #[must_use]
    pub fn off_chain(rfq_id: RfqId, trade_id: TradeId) -> Self {
        Self::new(rfq_id, trade_id, SettlementMethod::OffChain, None)
    }
}

impl DomainEvent for SettlementInitiated {
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
        "SettlementInitiated"
    }
}

/// Event emitted when settlement is confirmed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettlementConfirmed {
    /// Event metadata.
    pub metadata: EventMetadata,
    /// The trade that was settled.
    pub trade_id: TradeId,
    /// Transaction hash for on-chain settlement (if applicable).
    pub tx_hash: Option<String>,
    /// Block number for on-chain settlement (if applicable).
    pub block_number: Option<u64>,
}

impl SettlementConfirmed {
    /// Creates a new SettlementConfirmed event.
    #[must_use]
    pub fn new(
        rfq_id: RfqId,
        trade_id: TradeId,
        tx_hash: Option<String>,
        block_number: Option<u64>,
    ) -> Self {
        Self {
            metadata: EventMetadata::for_rfq(rfq_id),
            trade_id,
            tx_hash,
            block_number,
        }
    }

    /// Creates a new SettlementConfirmed event for on-chain settlement.
    #[must_use]
    pub fn on_chain(rfq_id: RfqId, trade_id: TradeId, tx_hash: String, block_number: u64) -> Self {
        Self::new(rfq_id, trade_id, Some(tx_hash), Some(block_number))
    }

    /// Creates a new SettlementConfirmed event for off-chain settlement.
    #[must_use]
    pub fn off_chain(rfq_id: RfqId, trade_id: TradeId) -> Self {
        Self::new(rfq_id, trade_id, None, None)
    }
}

impl DomainEvent for SettlementConfirmed {
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
        "SettlementConfirmed"
    }
}

/// Event emitted when settlement fails.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettlementFailed {
    /// Event metadata.
    pub metadata: EventMetadata,
    /// The trade that failed to settle.
    pub trade_id: TradeId,
    /// Reason for failure.
    pub reason: String,
    /// Transaction hash if the failure was on-chain.
    pub tx_hash: Option<String>,
}

impl SettlementFailed {
    /// Creates a new SettlementFailed event.
    #[must_use]
    pub fn new(
        rfq_id: RfqId,
        trade_id: TradeId,
        reason: impl Into<String>,
        tx_hash: Option<String>,
    ) -> Self {
        Self {
            metadata: EventMetadata::for_rfq(rfq_id),
            trade_id,
            reason: reason.into(),
            tx_hash,
        }
    }
}

impl DomainEvent for SettlementFailed {
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
        "SettlementFailed"
    }
}

/// Event emitted when positions are updated after trade execution.
///
/// This event notifies the Position Manager to:
/// 1. Update requester and MM positions
/// 2. Trigger Greeks recalculation for the instrument
/// 3. Compute margin impact for both counterparties
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PositionUpdated {
    /// Event metadata.
    pub metadata: EventMetadata,
    /// The trade that triggered the position update.
    pub trade_id: TradeId,
    /// Requester position update.
    pub requester_id: CounterpartyId,
    /// Requester's side of the trade.
    pub requester_side: OrderSide,
    /// Market maker position update (opposite side).
    pub mm_id: VenueId,
    /// Market maker's side of the trade.
    pub mm_side: OrderSide,
    /// Instrument for Greeks recalculation.
    pub instrument: Instrument,
    /// Asset class for efficient Greeks calculation.
    pub asset_class: crate::domain::value_objects::enums::AssetClass,
    /// Trade details for position calculation.
    pub quantity: Quantity,
    /// Execution price.
    pub price: Price,
}

impl PositionUpdated {
    /// Creates a new `PositionUpdated` event.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        rfq_id: RfqId,
        trade_id: TradeId,
        requester_id: CounterpartyId,
        requester_side: OrderSide,
        mm_id: VenueId,
        mm_side: OrderSide,
        instrument: Instrument,
        quantity: Quantity,
        price: Price,
    ) -> Self {
        let asset_class = instrument.asset_class();
        Self {
            metadata: EventMetadata::for_rfq(rfq_id),
            trade_id,
            requester_id,
            requester_side,
            mm_id,
            mm_side,
            instrument,
            asset_class,
            quantity,
            price,
        }
    }
}

impl DomainEvent for PositionUpdated {
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
        "PositionUpdated"
    }
}

/// Enum containing all trade and settlement events.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TradeEvent {
    /// Trade was executed.
    Executed(TradeExecuted),
    /// Position was updated.
    PositionUpdated(PositionUpdated),
    /// Settlement was initiated.
    SettlementInitiated(SettlementInitiated),
    /// Settlement was confirmed.
    SettlementConfirmed(SettlementConfirmed),
    /// Settlement failed.
    SettlementFailed(SettlementFailed),
}

impl DomainEvent for TradeEvent {
    fn event_id(&self) -> EventId {
        match self {
            Self::Executed(e) => e.event_id(),
            Self::PositionUpdated(e) => e.event_id(),
            Self::SettlementInitiated(e) => e.event_id(),
            Self::SettlementConfirmed(e) => e.event_id(),
            Self::SettlementFailed(e) => e.event_id(),
        }
    }

    fn rfq_id(&self) -> Option<RfqId> {
        match self {
            Self::Executed(e) => e.rfq_id(),
            Self::PositionUpdated(e) => e.rfq_id(),
            Self::SettlementInitiated(e) => e.rfq_id(),
            Self::SettlementConfirmed(e) => e.rfq_id(),
            Self::SettlementFailed(e) => e.rfq_id(),
        }
    }

    fn timestamp(&self) -> Timestamp {
        match self {
            Self::Executed(e) => e.timestamp(),
            Self::PositionUpdated(e) => e.timestamp(),
            Self::SettlementInitiated(e) => e.timestamp(),
            Self::SettlementConfirmed(e) => e.timestamp(),
            Self::SettlementFailed(e) => e.timestamp(),
        }
    }

    fn event_type(&self) -> EventType {
        match self {
            Self::Executed(e) => e.event_type(),
            Self::PositionUpdated(e) => e.event_type(),
            Self::SettlementInitiated(e) => e.event_type(),
            Self::SettlementConfirmed(e) => e.event_type(),
            Self::SettlementFailed(e) => e.event_type(),
        }
    }

    fn event_name(&self) -> &'static str {
        match self {
            Self::Executed(e) => e.event_name(),
            Self::PositionUpdated(e) => e.event_name(),
            Self::SettlementInitiated(e) => e.event_name(),
            Self::SettlementConfirmed(e) => e.event_name(),
            Self::SettlementFailed(e) => e.event_name(),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn test_rfq_id() -> RfqId {
        RfqId::new_v4()
    }

    fn test_trade_id() -> TradeId {
        TradeId::new_v4()
    }

    fn test_quote_id() -> QuoteId {
        QuoteId::new_v4()
    }

    fn test_venue_id() -> VenueId {
        VenueId::new("test-venue")
    }

    fn test_counterparty_id() -> CounterpartyId {
        CounterpartyId::new("test-client")
    }

    mod trade_executed {
        use super::*;
        use crate::domain::errors::DomainError;

        #[test]
        fn creates_event() {
            let rfq_id = test_rfq_id();
            let trade_id = test_trade_id();
            let event = TradeExecuted::builder()
                .rfq_id(rfq_id)
                .trade_id(trade_id)
                .quote_id(test_quote_id())
                .venue_id(test_venue_id())
                .counterparty_id(test_counterparty_id())
                .price(Price::new(50000.0).unwrap())
                .quantity(Quantity::new(1.0).unwrap())
                .settlement_method(SettlementMethod::OnChain(Blockchain::Ethereum))
                .build();

            assert_eq!(event.rfq_id(), Some(rfq_id));
            assert_eq!(event.trade_id, trade_id);
            assert_eq!(event.event_name(), "TradeExecuted");
            assert_eq!(event.event_type(), EventType::Trade);
        }

        #[test]
        fn serde_roundtrip() {
            let event = TradeExecuted::builder()
                .rfq_id(test_rfq_id())
                .trade_id(test_trade_id())
                .quote_id(test_quote_id())
                .venue_id(test_venue_id())
                .counterparty_id(test_counterparty_id())
                .price(Price::new(50000.0).unwrap())
                .quantity(Quantity::new(1.0).unwrap())
                .settlement_method(SettlementMethod::OffChain)
                .build();

            let json = serde_json::to_string(&event).unwrap();
            let deserialized: TradeExecuted = serde_json::from_str(&json).unwrap();
            assert_eq!(event.trade_id, deserialized.trade_id);
        }

        #[test]
        fn builder_creates_event_without_fees() {
            let event = TradeExecuted::builder()
                .rfq_id(test_rfq_id())
                .trade_id(test_trade_id())
                .quote_id(test_quote_id())
                .venue_id(test_venue_id())
                .counterparty_id(test_counterparty_id())
                .price(Price::new(50000.0).unwrap())
                .quantity(Quantity::new(1.0).unwrap())
                .settlement_method(SettlementMethod::OffChain)
                .build();

            assert!(event.taker_fee.is_none());
            assert!(event.maker_fee.is_none());
            assert!(event.net_fee.is_none());
        }

        #[test]
        fn builder_creates_event_with_fees() {
            let event = TradeExecuted::builder()
                .rfq_id(test_rfq_id())
                .trade_id(test_trade_id())
                .quote_id(test_quote_id())
                .venue_id(test_venue_id())
                .counterparty_id(test_counterparty_id())
                .price(Price::new(50000.0).unwrap())
                .quantity(Quantity::new(1.0).unwrap())
                .settlement_method(SettlementMethod::OffChain)
                .taker_fee(rust_decimal::Decimal::new(50, 1))
                .maker_fee(rust_decimal::Decimal::new(-20, 1))
                .net_fee(rust_decimal::Decimal::new(30, 1))
                .build();

            assert_eq!(event.taker_fee, Some(rust_decimal::Decimal::new(50, 1)));
            assert_eq!(event.maker_fee, Some(rust_decimal::Decimal::new(-20, 1)));
            assert_eq!(event.net_fee, Some(rust_decimal::Decimal::new(30, 1)));
        }

        #[test]
        #[should_panic(expected = "price is required")]
        fn builder_missing_required_field_panics() {
            let _event = TradeExecuted::builder()
                .rfq_id(test_rfq_id())
                .trade_id(test_trade_id())
                .quote_id(test_quote_id())
                .venue_id(test_venue_id())
                .counterparty_id(test_counterparty_id())
                // .price(Price::new(50000.0).unwrap()) // Missing required field
                .quantity(Quantity::new(1.0).unwrap())
                .settlement_method(SettlementMethod::OffChain)
                .build();
        }

        #[test]
        fn try_build_success_all_fields() {
            let result = TradeExecuted::builder()
                .rfq_id(test_rfq_id())
                .trade_id(test_trade_id())
                .quote_id(test_quote_id())
                .venue_id(test_venue_id())
                .counterparty_id(test_counterparty_id())
                .price(Price::new(50000.0).unwrap())
                .quantity(Quantity::new(1.0).unwrap())
                .settlement_method(SettlementMethod::OffChain)
                .taker_fee(rust_decimal::Decimal::new(50, 1))
                .maker_fee(rust_decimal::Decimal::new(-20, 1))
                .net_fee(rust_decimal::Decimal::new(30, 1))
                .try_build();

            assert!(result.is_ok());
            let event = result.unwrap();
            assert_eq!(event.taker_fee, Some(rust_decimal::Decimal::new(50, 1)));
            assert_eq!(event.maker_fee, Some(rust_decimal::Decimal::new(-20, 1)));
            assert_eq!(event.net_fee, Some(rust_decimal::Decimal::new(30, 1)));
        }

        #[test]
        fn try_build_success_required_only() {
            let result = TradeExecuted::builder()
                .rfq_id(test_rfq_id())
                .trade_id(test_trade_id())
                .quote_id(test_quote_id())
                .venue_id(test_venue_id())
                .counterparty_id(test_counterparty_id())
                .price(Price::new(50000.0).unwrap())
                .quantity(Quantity::new(1.0).unwrap())
                .settlement_method(SettlementMethod::OffChain)
                .try_build();

            assert!(result.is_ok());
            let event = result.unwrap();
            assert!(event.taker_fee.is_none());
            assert!(event.maker_fee.is_none());
            assert!(event.net_fee.is_none());
        }

        #[test]
        fn try_build_missing_rfq_id() {
            let result = TradeExecuted::builder()
                // .rfq_id(test_rfq_id()) // Missing
                .trade_id(test_trade_id())
                .quote_id(test_quote_id())
                .venue_id(test_venue_id())
                .counterparty_id(test_counterparty_id())
                .price(Price::new(50000.0).unwrap())
                .quantity(Quantity::new(1.0).unwrap())
                .settlement_method(SettlementMethod::OffChain)
                .try_build();

            assert!(result.is_err());
            assert!(
                matches!(result, Err(DomainError::ValidationError(ref msg)) if msg.contains("rfq_id is required"))
            );
        }

        #[test]
        fn try_build_missing_trade_id() {
            let result = TradeExecuted::builder()
                .rfq_id(test_rfq_id())
                // .trade_id(test_trade_id()) // Missing
                .quote_id(test_quote_id())
                .venue_id(test_venue_id())
                .counterparty_id(test_counterparty_id())
                .price(Price::new(50000.0).unwrap())
                .quantity(Quantity::new(1.0).unwrap())
                .settlement_method(SettlementMethod::OffChain)
                .try_build();

            assert!(result.is_err());
            assert!(
                matches!(result, Err(DomainError::ValidationError(ref msg)) if msg.contains("trade_id is required"))
            );
        }

        #[test]
        fn try_build_missing_quote_id() {
            let result = TradeExecuted::builder()
                .rfq_id(test_rfq_id())
                .trade_id(test_trade_id())
                // .quote_id(test_quote_id()) // Missing
                .venue_id(test_venue_id())
                .counterparty_id(test_counterparty_id())
                .price(Price::new(50000.0).unwrap())
                .quantity(Quantity::new(1.0).unwrap())
                .settlement_method(SettlementMethod::OffChain)
                .try_build();

            assert!(result.is_err());
            assert!(
                matches!(result, Err(DomainError::ValidationError(ref msg)) if msg.contains("quote_id is required"))
            );
        }

        #[test]
        fn try_build_missing_venue_id() {
            let result = TradeExecuted::builder()
                .rfq_id(test_rfq_id())
                .trade_id(test_trade_id())
                .quote_id(test_quote_id())
                // .venue_id(test_venue_id()) // Missing
                .counterparty_id(test_counterparty_id())
                .price(Price::new(50000.0).unwrap())
                .quantity(Quantity::new(1.0).unwrap())
                .settlement_method(SettlementMethod::OffChain)
                .try_build();

            assert!(result.is_err());
            assert!(
                matches!(result, Err(DomainError::ValidationError(ref msg)) if msg.contains("venue_id is required"))
            );
        }

        #[test]
        fn try_build_missing_counterparty_id() {
            let result = TradeExecuted::builder()
                .rfq_id(test_rfq_id())
                .trade_id(test_trade_id())
                .quote_id(test_quote_id())
                .venue_id(test_venue_id())
                // .counterparty_id(test_counterparty_id()) // Missing
                .price(Price::new(50000.0).unwrap())
                .quantity(Quantity::new(1.0).unwrap())
                .settlement_method(SettlementMethod::OffChain)
                .try_build();

            assert!(result.is_err());
            assert!(
                matches!(result, Err(DomainError::ValidationError(ref msg)) if msg.contains("counterparty_id is required"))
            );
        }

        #[test]
        fn try_build_missing_price() {
            let result = TradeExecuted::builder()
                .rfq_id(test_rfq_id())
                .trade_id(test_trade_id())
                .quote_id(test_quote_id())
                .venue_id(test_venue_id())
                .counterparty_id(test_counterparty_id())
                // .price(Price::new(50000.0).unwrap()) // Missing
                .quantity(Quantity::new(1.0).unwrap())
                .settlement_method(SettlementMethod::OffChain)
                .try_build();

            assert!(result.is_err());
            assert!(
                matches!(result, Err(DomainError::ValidationError(ref msg)) if msg.contains("price is required"))
            );
        }

        #[test]
        fn try_build_missing_quantity() {
            let result = TradeExecuted::builder()
                .rfq_id(test_rfq_id())
                .trade_id(test_trade_id())
                .quote_id(test_quote_id())
                .venue_id(test_venue_id())
                .counterparty_id(test_counterparty_id())
                .price(Price::new(50000.0).unwrap())
                // .quantity(Quantity::new(1.0).unwrap()) // Missing
                .settlement_method(SettlementMethod::OffChain)
                .try_build();

            assert!(result.is_err());
            assert!(
                matches!(result, Err(DomainError::ValidationError(ref msg)) if msg.contains("quantity is required"))
            );
        }

        #[test]
        fn try_build_missing_settlement_method() {
            let result = TradeExecuted::builder()
                .rfq_id(test_rfq_id())
                .trade_id(test_trade_id())
                .quote_id(test_quote_id())
                .venue_id(test_venue_id())
                .counterparty_id(test_counterparty_id())
                .price(Price::new(50000.0).unwrap())
                .quantity(Quantity::new(1.0).unwrap())
                // .settlement_method(SettlementMethod::OffChain) // Missing
                .try_build();

            assert!(result.is_err());
            assert!(
                matches!(result, Err(DomainError::ValidationError(ref msg)) if msg.contains("settlement_method is required"))
            );
        }
    }

    mod settlement_initiated {
        use super::*;

        #[test]
        fn on_chain() {
            let event = SettlementInitiated::on_chain(
                test_rfq_id(),
                test_trade_id(),
                Blockchain::Ethereum,
                "0xabc123".to_string(),
            );

            assert_eq!(event.tx_hash, Some("0xabc123".to_string()));
            assert_eq!(
                event.settlement_method,
                SettlementMethod::OnChain(Blockchain::Ethereum)
            );
            assert_eq!(event.event_name(), "SettlementInitiated");
            assert_eq!(event.event_type(), EventType::Settlement);
        }

        #[test]
        fn off_chain() {
            let event = SettlementInitiated::off_chain(test_rfq_id(), test_trade_id());

            assert!(event.tx_hash.is_none());
            assert_eq!(event.settlement_method, SettlementMethod::OffChain);
        }
    }

    mod settlement_confirmed {
        use super::*;

        #[test]
        fn on_chain() {
            let event = SettlementConfirmed::on_chain(
                test_rfq_id(),
                test_trade_id(),
                "0xdef456".to_string(),
                12345678,
            );

            assert_eq!(event.tx_hash, Some("0xdef456".to_string()));
            assert_eq!(event.block_number, Some(12345678));
            assert_eq!(event.event_name(), "SettlementConfirmed");
        }

        #[test]
        fn off_chain() {
            let event = SettlementConfirmed::off_chain(test_rfq_id(), test_trade_id());

            assert!(event.tx_hash.is_none());
            assert!(event.block_number.is_none());
        }
    }

    mod settlement_failed {
        use super::*;

        #[test]
        fn creates_event() {
            let event = SettlementFailed::new(
                test_rfq_id(),
                test_trade_id(),
                "Insufficient funds",
                Some("0xfailed".to_string()),
            );

            assert_eq!(event.reason, "Insufficient funds");
            assert_eq!(event.tx_hash, Some("0xfailed".to_string()));
            assert_eq!(event.event_name(), "SettlementFailed");
        }
    }

    mod trade_event_enum {
        use super::*;
        use crate::domain::value_objects::enums::AssetClass;
        use crate::domain::value_objects::symbol::Symbol;

        #[test]
        fn serde_roundtrip() {
            let event = TradeEvent::SettlementConfirmed(SettlementConfirmed::on_chain(
                test_rfq_id(),
                test_trade_id(),
                "0xabc".to_string(),
                100,
            ));

            let json = serde_json::to_string(&event).unwrap();
            let deserialized: TradeEvent = serde_json::from_str(&json).unwrap();
            assert_eq!(event.event_name(), deserialized.event_name());
        }

        #[test]
        fn serde_roundtrip_position_updated() {
            let event = TradeEvent::PositionUpdated(PositionUpdated::new(
                test_rfq_id(),
                test_trade_id(),
                test_counterparty_id(),
                OrderSide::Buy,
                test_venue_id(),
                OrderSide::Sell,
                Instrument::new(
                    Symbol::new("BTC/USD").unwrap(),
                    AssetClass::CryptoSpot,
                    SettlementMethod::OffChain,
                ),
                Quantity::new(1.0).unwrap(),
                Price::new(50000.0).unwrap(),
            ));

            let json = serde_json::to_string(&event).unwrap();
            let deserialized: TradeEvent = serde_json::from_str(&json).unwrap();
            assert_eq!(event.event_name(), deserialized.event_name());
            assert_eq!(event.event_type(), EventType::Trade);
        }

        #[test]
        fn domain_event_trait() {
            let event = TradeEvent::SettlementFailed(SettlementFailed::new(
                test_rfq_id(),
                test_trade_id(),
                "Error",
                None,
            ));

            assert_eq!(event.event_name(), "SettlementFailed");
            assert_eq!(event.event_type(), EventType::Settlement);
        }
    }

    mod position_updated {
        use super::*;
        use crate::domain::value_objects::enums::AssetClass;
        use crate::domain::value_objects::symbol::Symbol;

        #[test]
        fn creates_event() {
            let rfq_id = test_rfq_id();
            let trade_id = test_trade_id();
            let requester_id = test_counterparty_id();
            let mm_id = test_venue_id();
            let instrument = Instrument::new(
                Symbol::new("BTC/USD").unwrap(),
                AssetClass::CryptoSpot,
                SettlementMethod::OffChain,
            );

            let event = PositionUpdated::new(
                rfq_id,
                trade_id,
                requester_id.clone(),
                OrderSide::Buy,
                mm_id.clone(),
                OrderSide::Sell,
                instrument.clone(),
                Quantity::new(1.0).unwrap(),
                Price::new(50000.0).unwrap(),
            );

            assert_eq!(event.rfq_id(), Some(rfq_id));
            assert_eq!(event.trade_id, trade_id);
            assert_eq!(event.requester_id, requester_id);
            assert_eq!(event.requester_side, OrderSide::Buy);
            assert_eq!(event.mm_id, mm_id);
            assert_eq!(event.mm_side, OrderSide::Sell);
            assert_eq!(event.instrument, instrument);
            assert_eq!(event.event_name(), "PositionUpdated");
            assert_eq!(event.event_type(), EventType::Trade);
        }

        #[test]
        fn serde_roundtrip() {
            let event = PositionUpdated::new(
                test_rfq_id(),
                test_trade_id(),
                test_counterparty_id(),
                OrderSide::Buy,
                test_venue_id(),
                OrderSide::Sell,
                Instrument::new(
                    Symbol::new("ETH/USD").unwrap(),
                    AssetClass::CryptoSpot,
                    SettlementMethod::OffChain,
                ),
                Quantity::new(10.0).unwrap(),
                Price::new(3000.0).unwrap(),
            );

            let json = serde_json::to_string(&event).unwrap();
            let deserialized: PositionUpdated = serde_json::from_str(&json).unwrap();
            assert_eq!(event.trade_id, deserialized.trade_id);
            assert_eq!(event.requester_id, deserialized.requester_id);
            assert_eq!(event.mm_id, deserialized.mm_id);
        }

        #[test]
        fn opposite_sides() {
            let event = PositionUpdated::new(
                test_rfq_id(),
                test_trade_id(),
                test_counterparty_id(),
                OrderSide::Sell,
                test_venue_id(),
                OrderSide::Buy,
                Instrument::new(
                    Symbol::new("BTC/USD").unwrap(),
                    AssetClass::CryptoSpot,
                    SettlementMethod::OffChain,
                ),
                Quantity::new(1.0).unwrap(),
                Price::new(50000.0).unwrap(),
            );

            assert_eq!(event.requester_side, OrderSide::Sell);
            assert_eq!(event.mm_side, OrderSide::Buy);
        }
    }
}
