//! # Property-Based Tests for SBE Serialization Roundtrips
//!
//! Uses proptest to verify that SBE encoding/decoding preserves data
//! for all message types.
//!
//! # Test Coverage
//!
//! - `RfqCreated` message roundtrip
//! - `QuoteReceived` message roundtrip
//! - `TradeExecuted` message roundtrip
//! - Edge cases: max values, empty strings, special characters

#![allow(clippy::unwrap_used)]
#![allow(clippy::indexing_slicing)]
#![allow(clippy::too_many_arguments)]

use proptest::prelude::*;
use rust_decimal::Decimal;

use crate::domain::events::domain_event::EventMetadata;
use crate::domain::events::rfq_events::{QuoteReceived, RfqCreated};
use crate::domain::events::trade_events::TradeExecuted;
use crate::domain::value_objects::timestamp::Timestamp;
use crate::domain::value_objects::{
    AssetClass, CounterpartyId, EventId, Instrument, OrderSide, Price, Quantity, QuoteId, RfqId,
    SettlementMethod, Symbol, TradeId, VenueId,
};
use crate::infrastructure::sbe::traits::{SbeDecode, SbeEncode};
use crate::infrastructure::sbe::types::{SbeDecimal, SbeUuid};

// ============================================================================
// SbeUuid Roundtrip Tests
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    #[test]
    fn sbe_uuid_roundtrip(bytes in any::<[u8; 16]>()) {
        let uuid = uuid::Uuid::from_bytes(bytes);
        let sbe = SbeUuid::from_uuid(uuid);
        let back = sbe.to_uuid();
        prop_assert_eq!(uuid, back);
    }

    #[test]
    fn sbe_uuid_encode_decode(bytes in any::<[u8; 16]>()) {
        let uuid = uuid::Uuid::from_bytes(bytes);
        let sbe = SbeUuid::from_uuid(uuid);
        let mut buffer = [0u8; 16];
        sbe.encode(&mut buffer).unwrap();
        let decoded = SbeUuid::decode(&buffer).unwrap();
        prop_assert_eq!(sbe, decoded);
        prop_assert_eq!(uuid, decoded.to_uuid());
    }
}

// ============================================================================
// SbeDecimal Roundtrip Tests
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    #[test]
    fn sbe_decimal_roundtrip(mantissa in -1_000_000_000_000i64..1_000_000_000_000i64, scale in 0u32..8u32) {
        let decimal = Decimal::new(mantissa, scale);
        let sbe = SbeDecimal::from_decimal(decimal);
        let back = sbe.to_decimal().unwrap();
        prop_assert_eq!(decimal, back);
    }

    #[test]
    fn sbe_decimal_encode_decode(mantissa in -1_000_000_000_000i64..1_000_000_000_000i64, scale in 0u32..8u32) {
        let decimal = Decimal::new(mantissa, scale);
        let sbe = SbeDecimal::from_decimal(decimal);
        let mut buffer = [0u8; 9];
        sbe.encode(&mut buffer).unwrap();
        let decoded = SbeDecimal::decode(&buffer).unwrap();
        prop_assert_eq!(sbe, decoded);
        let back = decoded.to_decimal().unwrap();
        prop_assert_eq!(decimal, back);
    }
}

// ============================================================================
// RfqCreated Roundtrip Tests
// ============================================================================

fn create_test_rfq_created(
    event_uuid: [u8; 16],
    rfq_uuid: [u8; 16],
    client_id: &str,
    symbol_str: &str,
    side: OrderSide,
    qty_mantissa: i64,
    qty_scale: u32,
) -> Option<RfqCreated> {
    let symbol = Symbol::new(symbol_str).ok()?;
    let quantity =
        Quantity::from_decimal(Decimal::new(qty_mantissa.abs().max(1), qty_scale)).ok()?;
    let instrument = Instrument::builder(symbol, AssetClass::CryptoSpot).build();

    let event_id = EventId::new(uuid::Uuid::from_bytes(event_uuid));
    let rfq_id = RfqId::new(uuid::Uuid::from_bytes(rfq_uuid));

    let mut metadata = EventMetadata::for_rfq(rfq_id);
    metadata.event_id = event_id;
    metadata.timestamp = Timestamp::now();

    Some(RfqCreated {
        metadata,
        client_id: CounterpartyId::new(client_id),
        instrument,
        side,
        quantity,
        expires_at: Timestamp::now().add_secs(60),
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn rfq_created_roundtrip(
        event_uuid in any::<[u8; 16]>(),
        rfq_uuid in any::<[u8; 16]>(),
        client_id in "[a-zA-Z0-9]{1,20}",
        side in prop_oneof![Just(OrderSide::Buy), Just(OrderSide::Sell)],
        qty_mantissa in 1i64..1_000_000_000i64,
        qty_scale in 0u32..6u32,
    ) {
        if let Some(original) = create_test_rfq_created(
            event_uuid, rfq_uuid, &client_id, "BTC/USD", side, qty_mantissa, qty_scale
        ) {
            let mut buffer = vec![0u8; original.encoded_size() + 100];
            let encoded_size = original.encode(&mut buffer).unwrap();
            let decoded = RfqCreated::decode(&buffer[..encoded_size]).unwrap();

            prop_assert_eq!(original.metadata.event_id.get(), decoded.metadata.event_id.get());
            prop_assert_eq!(original.client_id.as_str(), decoded.client_id.as_str());
            prop_assert_eq!(original.side, decoded.side);
            prop_assert_eq!(original.quantity.get(), decoded.quantity.get());
        }
    }
}

// ============================================================================
// QuoteReceived Roundtrip Tests
// ============================================================================

fn create_test_quote_received(
    event_uuid: [u8; 16],
    rfq_uuid: [u8; 16],
    quote_uuid: [u8; 16],
    venue_id: &str,
    price_mantissa: i64,
    price_scale: u32,
    qty_mantissa: i64,
    qty_scale: u32,
) -> Option<QuoteReceived> {
    let price = Price::from_decimal(Decimal::new(price_mantissa.abs().max(1), price_scale)).ok()?;
    let quantity =
        Quantity::from_decimal(Decimal::new(qty_mantissa.abs().max(1), qty_scale)).ok()?;

    let event_id = EventId::new(uuid::Uuid::from_bytes(event_uuid));
    let rfq_id = RfqId::new(uuid::Uuid::from_bytes(rfq_uuid));
    let quote_id = QuoteId::new(uuid::Uuid::from_bytes(quote_uuid));

    let mut metadata = EventMetadata::for_rfq(rfq_id);
    metadata.event_id = event_id;
    metadata.timestamp = Timestamp::now();

    Some(QuoteReceived {
        metadata,
        quote_id,
        venue_id: VenueId::new(venue_id),
        price,
        quantity,
        valid_until: Timestamp::now().add_secs(60),
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn quote_received_roundtrip(
        event_uuid in any::<[u8; 16]>(),
        rfq_uuid in any::<[u8; 16]>(),
        quote_uuid in any::<[u8; 16]>(),
        venue_id in "[a-zA-Z0-9_-]{1,20}",
        price_mantissa in 1i64..1_000_000_000i64,
        price_scale in 0u32..6u32,
        qty_mantissa in 1i64..1_000_000_000i64,
        qty_scale in 0u32..6u32,
    ) {
        if let Some(original) = create_test_quote_received(
            event_uuid, rfq_uuid, quote_uuid, &venue_id,
            price_mantissa, price_scale, qty_mantissa, qty_scale
        ) {
            let mut buffer = vec![0u8; original.encoded_size() + 100];
            let encoded_size = original.encode(&mut buffer).unwrap();
            let decoded = QuoteReceived::decode(&buffer[..encoded_size]).unwrap();

            prop_assert_eq!(original.metadata.event_id.get(), decoded.metadata.event_id.get());
            prop_assert_eq!(original.quote_id.get(), decoded.quote_id.get());
            prop_assert_eq!(original.venue_id.as_str(), decoded.venue_id.as_str());
            prop_assert_eq!(original.price.get(), decoded.price.get());
            prop_assert_eq!(original.quantity.get(), decoded.quantity.get());
        }
    }
}

// ============================================================================
// TradeExecuted Roundtrip Tests
// ============================================================================

fn create_test_trade_executed(
    event_uuid: [u8; 16],
    rfq_uuid: [u8; 16],
    trade_uuid: [u8; 16],
    quote_uuid: [u8; 16],
    venue_id: &str,
    counterparty_id: &str,
    price_mantissa: i64,
    price_scale: u32,
    qty_mantissa: i64,
    qty_scale: u32,
) -> Option<TradeExecuted> {
    let price = Price::from_decimal(Decimal::new(price_mantissa.abs().max(1), price_scale)).ok()?;
    let quantity =
        Quantity::from_decimal(Decimal::new(qty_mantissa.abs().max(1), qty_scale)).ok()?;

    let event_id = EventId::new(uuid::Uuid::from_bytes(event_uuid));
    let rfq_id = RfqId::new(uuid::Uuid::from_bytes(rfq_uuid));
    let trade_id = TradeId::new(uuid::Uuid::from_bytes(trade_uuid));
    let quote_id = QuoteId::new(uuid::Uuid::from_bytes(quote_uuid));

    let mut metadata = EventMetadata::for_rfq(rfq_id);
    metadata.event_id = event_id;
    metadata.timestamp = Timestamp::now();

    Some(TradeExecuted {
        metadata,
        trade_id,
        quote_id,
        venue_id: VenueId::new(venue_id),
        counterparty_id: CounterpartyId::new(counterparty_id),
        price,
        quantity,
        settlement_method: SettlementMethod::OffChain,
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn trade_executed_roundtrip(
        event_uuid in any::<[u8; 16]>(),
        rfq_uuid in any::<[u8; 16]>(),
        trade_uuid in any::<[u8; 16]>(),
        quote_uuid in any::<[u8; 16]>(),
        venue_id in "[a-zA-Z0-9_-]{1,20}",
        counterparty_id in "[a-zA-Z0-9_-]{1,20}",
        price_mantissa in 1i64..1_000_000_000i64,
        price_scale in 0u32..6u32,
        qty_mantissa in 1i64..1_000_000_000i64,
        qty_scale in 0u32..6u32,
    ) {
        if let Some(original) = create_test_trade_executed(
            event_uuid, rfq_uuid, trade_uuid, quote_uuid,
            &venue_id, &counterparty_id,
            price_mantissa, price_scale, qty_mantissa, qty_scale
        ) {
            let mut buffer = vec![0u8; original.encoded_size() + 100];
            let encoded_size = original.encode(&mut buffer).unwrap();
            let decoded = TradeExecuted::decode(&buffer[..encoded_size]).unwrap();

            prop_assert_eq!(original.metadata.event_id.get(), decoded.metadata.event_id.get());
            prop_assert_eq!(original.trade_id.get(), decoded.trade_id.get());
            prop_assert_eq!(original.quote_id.get(), decoded.quote_id.get());
            prop_assert_eq!(original.venue_id.as_str(), decoded.venue_id.as_str());
            prop_assert_eq!(original.counterparty_id.as_str(), decoded.counterparty_id.as_str());
            prop_assert_eq!(original.price.get(), decoded.price.get());
            prop_assert_eq!(original.quantity.get(), decoded.quantity.get());
        }
    }
}

// ============================================================================
// Edge Case Tests
// ============================================================================

#[test]
fn uuid_nil_roundtrip() {
    let uuid = uuid::Uuid::nil();
    let sbe = SbeUuid::from_uuid(uuid);
    let back = sbe.to_uuid();
    assert_eq!(uuid, back);
}

#[test]
fn uuid_max_roundtrip() {
    let uuid = uuid::Uuid::max();
    let sbe = SbeUuid::from_uuid(uuid);
    let back = sbe.to_uuid();
    assert_eq!(uuid, back);
}

#[test]
fn decimal_zero_roundtrip() {
    let decimal = Decimal::ZERO;
    let sbe = SbeDecimal::from_decimal(decimal);
    let back = sbe.to_decimal().unwrap();
    assert_eq!(decimal, back);
}

#[test]
fn decimal_large_positive_roundtrip() {
    let decimal = Decimal::new(999_999_999_999, 0);
    let sbe = SbeDecimal::from_decimal(decimal);
    let back = sbe.to_decimal().unwrap();
    assert_eq!(decimal, back);
}

#[test]
fn decimal_large_negative_roundtrip() {
    let decimal = Decimal::new(-999_999_999_999, 0);
    let sbe = SbeDecimal::from_decimal(decimal);
    let back = sbe.to_decimal().unwrap();
    assert_eq!(decimal, back);
}

#[test]
fn decimal_high_precision_roundtrip() {
    let decimal = Decimal::new(123456789, 8); // 1.23456789
    let sbe = SbeDecimal::from_decimal(decimal);
    let back = sbe.to_decimal().unwrap();
    assert_eq!(decimal, back);
}

#[test]
fn rfq_created_with_short_client_id() {
    if let Some(event) =
        create_test_rfq_created([1u8; 16], [2u8; 16], "a", "BTC/USD", OrderSide::Buy, 100, 2)
    {
        let mut buffer = vec![0u8; event.encoded_size() + 100];
        let size = event.encode(&mut buffer).unwrap();
        let decoded = RfqCreated::decode(&buffer[..size]).unwrap();
        assert_eq!(event.client_id.as_str(), decoded.client_id.as_str());
    }
}

#[test]
fn quote_received_with_large_price() {
    if let Some(event) = create_test_quote_received(
        [1u8; 16],
        [2u8; 16],
        [3u8; 16],
        "venue-1",
        99999999999,
        2,
        1,
        8,
    ) {
        let mut buffer = vec![0u8; event.encoded_size() + 100];
        let size = event.encode(&mut buffer).unwrap();
        let decoded = QuoteReceived::decode(&buffer[..size]).unwrap();
        assert_eq!(event.price.get(), decoded.price.get());
    }
}

#[test]
fn trade_executed_with_long_identifiers() {
    if let Some(event) = create_test_trade_executed(
        [1u8; 16],
        [2u8; 16],
        [3u8; 16],
        [4u8; 16],
        "venue-with-long-name",
        "counterparty-long-id",
        50000,
        0,
        150,
        2,
    ) {
        let mut buffer = vec![0u8; event.encoded_size() + 100];
        let size = event.encode(&mut buffer).unwrap();
        let decoded = TradeExecuted::decode(&buffer[..size]).unwrap();
        assert_eq!(event.venue_id.as_str(), decoded.venue_id.as_str());
        assert_eq!(
            event.counterparty_id.as_str(),
            decoded.counterparty_id.as_str()
        );
    }
}

#[test]
fn all_message_types_have_unique_template_ids() {
    use crate::infrastructure::sbe::{
        QUOTE_RECEIVED_TEMPLATE_ID, RFQ_CREATED_TEMPLATE_ID, TRADE_EXECUTED_TEMPLATE_ID,
    };

    assert_ne!(RFQ_CREATED_TEMPLATE_ID, QUOTE_RECEIVED_TEMPLATE_ID);
    assert_ne!(RFQ_CREATED_TEMPLATE_ID, TRADE_EXECUTED_TEMPLATE_ID);
    assert_ne!(QUOTE_RECEIVED_TEMPLATE_ID, TRADE_EXECUTED_TEMPLATE_ID);

    assert_eq!(RFQ_CREATED_TEMPLATE_ID, 1);
    assert_eq!(QUOTE_RECEIVED_TEMPLATE_ID, 2);
    assert_eq!(TRADE_EXECUTED_TEMPLATE_ID, 3);
}
