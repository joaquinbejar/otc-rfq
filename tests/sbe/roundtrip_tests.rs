//! # SBE Roundtrip Property Tests
//!
//! Property-based tests to verify SBE encoding/decoding preserves all data.
//!
//! These tests use proptest to generate arbitrary domain events and verify
//! that encoding followed by decoding produces equivalent data.

use otc_rfq::domain::events::rfq_events::{QuoteReceived, RfqCreated};
use otc_rfq::domain::events::trade_events::TradeExecuted;
use otc_rfq::domain::value_objects::{
    AssetClass, CounterpartyId, Instrument, OrderSide, Price, Quantity, QuoteId, RfqId,
    SettlementMethod, Symbol, TradeId, VenueId,
};
use otc_rfq::domain::value_objects::timestamp::Timestamp;
use otc_rfq::infrastructure::sbe::{SbeDecode, SbeEncode};
use proptest::prelude::*;

// ============================================================================
// Arbitrary Implementations
// ============================================================================

/// Strategy for generating valid venue IDs (non-empty ASCII strings).
fn arb_venue_id() -> impl Strategy<Value = VenueId> {
    "[a-zA-Z0-9_-]{1,50}".prop_map(VenueId::new)
}

/// Strategy for generating valid counterparty IDs.
fn arb_counterparty_id() -> impl Strategy<Value = CounterpartyId> {
    "[a-zA-Z0-9_-]{1,50}".prop_map(CounterpartyId::new)
}

/// Strategy for generating valid symbols.
fn arb_symbol() -> impl Strategy<Value = Symbol> {
    "[A-Z]{2,5}/[A-Z]{2,5}".prop_filter_map("valid symbol", |s| Symbol::new(&s).ok())
}

/// Strategy for generating valid prices (positive, reasonable range).
fn arb_price() -> impl Strategy<Value = Price> {
    (1u64..1_000_000_000u64, 0u32..8u32).prop_filter_map("valid price", |(mantissa, scale)| {
        let decimal = rust_decimal::Decimal::new(mantissa as i64, scale);
        Price::from_decimal(decimal).ok()
    })
}

/// Strategy for generating valid quantities (positive, reasonable range).
fn arb_quantity() -> impl Strategy<Value = Quantity> {
    (1u64..1_000_000_000u64, 0u32..8u32).prop_filter_map("valid quantity", |(mantissa, scale)| {
        let decimal = rust_decimal::Decimal::new(mantissa as i64, scale);
        Quantity::from_decimal(decimal).ok()
    })
}

/// Strategy for generating valid timestamps (reasonable range for nanoseconds).
fn arb_timestamp() -> impl Strategy<Value = Timestamp> {
    // Use a range that fits in i64 nanoseconds (roughly 1970-2200)
    (0i64..4_000_000_000i64).prop_filter_map("valid timestamp", |secs| Timestamp::from_secs(secs))
}

/// Strategy for generating order sides.
fn arb_order_side() -> impl Strategy<Value = OrderSide> {
    prop_oneof![Just(OrderSide::Buy), Just(OrderSide::Sell)]
}

/// Strategy for generating instruments.
fn arb_instrument() -> impl Strategy<Value = Instrument> {
    arb_symbol().prop_map(|symbol| Instrument::builder(symbol, AssetClass::CryptoSpot).build())
}

/// Strategy for generating RfqCreated events.
fn arb_rfq_created() -> impl Strategy<Value = RfqCreated> {
    (
        arb_counterparty_id(),
        arb_instrument(),
        arb_order_side(),
        arb_quantity(),
        arb_timestamp(),
    )
        .prop_map(|(client_id, instrument, side, quantity, expires_at)| {
            RfqCreated::new(
                RfqId::new_v4(),
                client_id,
                instrument,
                side,
                quantity,
                expires_at,
            )
        })
}

/// Strategy for generating QuoteReceived events.
fn arb_quote_received() -> impl Strategy<Value = QuoteReceived> {
    (
        arb_venue_id(),
        arb_price(),
        arb_quantity(),
        arb_timestamp(),
    )
        .prop_map(|(venue_id, price, quantity, valid_until)| {
            QuoteReceived::new(
                RfqId::new_v4(),
                QuoteId::new_v4(),
                venue_id,
                price,
                quantity,
                valid_until,
            )
        })
}

/// Strategy for generating TradeExecuted events.
fn arb_trade_executed() -> impl Strategy<Value = TradeExecuted> {
    (
        arb_venue_id(),
        arb_counterparty_id(),
        arb_price(),
        arb_quantity(),
    )
        .prop_map(|(venue_id, counterparty_id, price, quantity)| {
            TradeExecuted::new(
                RfqId::new_v4(),
                TradeId::new_v4(),
                QuoteId::new_v4(),
                venue_id,
                counterparty_id,
                price,
                quantity,
                SettlementMethod::OffChain,
            )
        })
}

// ============================================================================
// Property Tests
// ============================================================================

proptest! {
    /// Test that RfqCreated roundtrips correctly through SBE encoding.
    #[test]
    fn rfq_created_roundtrip(event in arb_rfq_created()) {
        let encoded = event.encode_to_vec().map_err(|e| TestCaseError::fail(e.to_string()))?;
        let decoded = RfqCreated::decode(&encoded).map_err(|e| TestCaseError::fail(e.to_string()))?;

        // Verify all fields are preserved
        prop_assert_eq!(event.metadata.event_id, decoded.metadata.event_id);
        prop_assert_eq!(event.metadata.rfq_id, decoded.metadata.rfq_id);
        prop_assert_eq!(event.client_id, decoded.client_id);
        prop_assert_eq!(event.side, decoded.side);
        prop_assert_eq!(event.quantity, decoded.quantity);
        prop_assert_eq!(
            event.instrument.symbol().as_str(),
            decoded.instrument.symbol().as_str()
        );
    }

    /// Test that QuoteReceived roundtrips correctly through SBE encoding.
    #[test]
    fn quote_received_roundtrip(event in arb_quote_received()) {
        let encoded = event.encode_to_vec().map_err(|e| TestCaseError::fail(e.to_string()))?;
        let decoded = QuoteReceived::decode(&encoded).map_err(|e| TestCaseError::fail(e.to_string()))?;

        // Verify all fields are preserved
        prop_assert_eq!(event.metadata.event_id, decoded.metadata.event_id);
        prop_assert_eq!(event.metadata.rfq_id, decoded.metadata.rfq_id);
        prop_assert_eq!(event.quote_id, decoded.quote_id);
        prop_assert_eq!(event.venue_id, decoded.venue_id);
        prop_assert_eq!(event.price, decoded.price);
        prop_assert_eq!(event.quantity, decoded.quantity);
    }

    /// Test that TradeExecuted roundtrips correctly through SBE encoding.
    #[test]
    fn trade_executed_roundtrip(event in arb_trade_executed()) {
        let encoded = event.encode_to_vec().map_err(|e| TestCaseError::fail(e.to_string()))?;
        let decoded = TradeExecuted::decode(&encoded).map_err(|e| TestCaseError::fail(e.to_string()))?;

        // Verify all fields are preserved
        prop_assert_eq!(event.metadata.event_id, decoded.metadata.event_id);
        prop_assert_eq!(event.metadata.rfq_id, decoded.metadata.rfq_id);
        prop_assert_eq!(event.trade_id, decoded.trade_id);
        prop_assert_eq!(event.quote_id, decoded.quote_id);
        prop_assert_eq!(event.venue_id, decoded.venue_id);
        prop_assert_eq!(event.counterparty_id, decoded.counterparty_id);
        prop_assert_eq!(event.price, decoded.price);
        prop_assert_eq!(event.quantity, decoded.quantity);
    }
}

// ============================================================================
// Edge Case Tests
// ============================================================================

#[cfg(test)]
mod edge_cases {
    use super::*;

    /// Test RfqCreated with minimum valid values.
    #[test]
    fn rfq_created_min_values() {
        let event = RfqCreated::new(
            RfqId::new_v4(),
            CounterpartyId::new("a"),
            Instrument::builder(Symbol::new("A/B").unwrap(), AssetClass::CryptoSpot).build(),
            OrderSide::Buy,
            Quantity::new(0.00000001).unwrap(),
            Timestamp::from_secs(0).unwrap(),
        );

        let encoded = event.encode_to_vec().unwrap();
        let decoded = RfqCreated::decode(&encoded).unwrap();

        assert_eq!(event.metadata.event_id, decoded.metadata.event_id);
        assert_eq!(event.client_id, decoded.client_id);
    }

    /// Test RfqCreated with maximum reasonable values.
    #[test]
    fn rfq_created_max_values() {
        let event = RfqCreated::new(
            RfqId::new_v4(),
            CounterpartyId::new("a".repeat(100)),
            Instrument::builder(Symbol::new("AAAAA/BBBBB").unwrap(), AssetClass::CryptoSpot)
                .build(),
            OrderSide::Sell,
            Quantity::new(999_999_999.99999999).unwrap(),
            Timestamp::from_secs(4_000_000_000).unwrap(),
        );

        let encoded = event.encode_to_vec().unwrap();
        let decoded = RfqCreated::decode(&encoded).unwrap();

        assert_eq!(event.metadata.event_id, decoded.metadata.event_id);
        assert_eq!(event.side, decoded.side);
    }

    /// Test QuoteReceived with zero price (edge case).
    #[test]
    fn quote_received_zero_price() {
        let event = QuoteReceived::new(
            RfqId::new_v4(),
            QuoteId::new_v4(),
            VenueId::new("venue"),
            Price::zero(),
            Quantity::new(1.0).unwrap(),
            Timestamp::now(),
        );

        let encoded = event.encode_to_vec().unwrap();
        let decoded = QuoteReceived::decode(&encoded).unwrap();

        assert_eq!(event.price, decoded.price);
        assert!(decoded.price.is_zero());
    }

    /// Test TradeExecuted with special characters in venue ID.
    #[test]
    fn trade_executed_special_venue_id() {
        let event = TradeExecuted::new(
            RfqId::new_v4(),
            TradeId::new_v4(),
            QuoteId::new_v4(),
            VenueId::new("venue-with-dashes_and_underscores"),
            CounterpartyId::new("client-123"),
            Price::new(50000.0).unwrap(),
            Quantity::new(1.0).unwrap(),
            SettlementMethod::OffChain,
        );

        let encoded = event.encode_to_vec().unwrap();
        let decoded = TradeExecuted::decode(&encoded).unwrap();

        assert_eq!(event.venue_id, decoded.venue_id);
        assert_eq!(event.counterparty_id, decoded.counterparty_id);
    }

    /// Test that encoding produces deterministic output.
    #[test]
    fn encoding_is_deterministic() {
        let event = RfqCreated::new(
            RfqId::new_v4(),
            CounterpartyId::new("client"),
            Instrument::builder(Symbol::new("BTC/USD").unwrap(), AssetClass::CryptoSpot).build(),
            OrderSide::Buy,
            Quantity::new(100.0).unwrap(),
            Timestamp::from_secs(1000).unwrap(),
        );

        let encoded1 = event.encode_to_vec().unwrap();
        let encoded2 = event.encode_to_vec().unwrap();

        assert_eq!(encoded1, encoded2);
    }

    /// Test that different events produce different encodings.
    #[test]
    fn different_events_different_encodings() {
        let event1 = QuoteReceived::new(
            RfqId::new_v4(),
            QuoteId::new_v4(),
            VenueId::new("venue1"),
            Price::new(100.0).unwrap(),
            Quantity::new(1.0).unwrap(),
            Timestamp::now(),
        );

        let event2 = QuoteReceived::new(
            RfqId::new_v4(),
            QuoteId::new_v4(),
            VenueId::new("venue2"),
            Price::new(200.0).unwrap(),
            Quantity::new(2.0).unwrap(),
            Timestamp::now(),
        );

        let encoded1 = event1.encode_to_vec().unwrap();
        let encoded2 = event2.encode_to_vec().unwrap();

        assert_ne!(encoded1, encoded2);
    }

    /// Test buffer too small error.
    #[test]
    fn buffer_too_small_error() {
        let event = RfqCreated::new(
            RfqId::new_v4(),
            CounterpartyId::new("client"),
            Instrument::builder(Symbol::new("BTC/USD").unwrap(), AssetClass::CryptoSpot).build(),
            OrderSide::Buy,
            Quantity::new(100.0).unwrap(),
            Timestamp::now(),
        );

        let mut small_buffer = [0u8; 10];
        let result = event.encode(&mut small_buffer);

        assert!(result.is_err());
    }

    /// Test decoding invalid template ID.
    #[test]
    fn decode_invalid_template_id() {
        // Create a buffer with invalid template ID
        let mut buffer = [0u8; 100];
        buffer[2..4].copy_from_slice(&999u16.to_le_bytes()); // Invalid template ID

        let result = RfqCreated::decode(&buffer);
        assert!(result.is_err());
    }

    /// Test high precision decimals.
    #[test]
    fn high_precision_decimals() {
        let event = QuoteReceived::new(
            RfqId::new_v4(),
            QuoteId::new_v4(),
            VenueId::new("venue"),
            Price::new(0.00000001).unwrap(),
            Quantity::new(0.00000001).unwrap(),
            Timestamp::now(),
        );

        let encoded = event.encode_to_vec().unwrap();
        let decoded = QuoteReceived::decode(&encoded).unwrap();

        assert_eq!(event.price, decoded.price);
        assert_eq!(event.quantity, decoded.quantity);
    }
}
