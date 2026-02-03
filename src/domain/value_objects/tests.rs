//! # Property-Based Tests for Domain Value Objects
//!
//! This module contains property-based tests using proptest for comprehensive
//! testing of domain value objects, particularly arithmetic operations.
//!
//! # Test Categories
//!
//! - **Arithmetic Roundtrips**: Verify that arithmetic operations are consistent
//! - **Serialization Roundtrips**: Verify serde serialization/deserialization
//! - **Conversion Roundtrips**: Verify type conversions are lossless
//! - **Edge Cases**: Test boundary conditions and special values

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use proptest::prelude::*;

use super::arithmetic::{ArithmeticError, CheckedArithmetic};
use super::price::Price;
use super::quantity::Quantity;
use super::timestamp::Timestamp;
use rust_decimal::Decimal;

// ============================================================================
// Strategy Definitions
// ============================================================================

/// Strategy for generating valid non-negative f64 values for Price/Quantity.
fn valid_positive_f64() -> impl Strategy<Value = f64> {
    (0.0f64..1_000_000_000.0f64).prop_filter("must be finite", |v| v.is_finite())
}

/// Strategy for generating valid Decimal values.
fn valid_decimal() -> impl Strategy<Value = Decimal> {
    (-1_000_000_000i64..1_000_000_000i64).prop_map(|v| Decimal::new(v, 2))
}

/// Strategy for generating valid non-negative Decimal values.
#[allow(dead_code)]
fn valid_positive_decimal() -> impl Strategy<Value = Decimal> {
    (0i64..1_000_000_000i64).prop_map(|v| Decimal::new(v, 2))
}

/// Strategy for generating valid timestamps (within reasonable range).
fn valid_timestamp_millis() -> impl Strategy<Value = i64> {
    // Range from year 2000 to year 2100
    946684800000i64..4102444800000i64
}

// ============================================================================
// Price Property Tests
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    /// Price addition is commutative: a + b == b + a
    #[test]
    fn price_addition_commutative(
        a in valid_positive_f64(),
        b in valid_positive_f64()
    ) {
        if let (Ok(price_a), Ok(price_b)) = (Price::new(a), Price::new(b))
            && let (Ok(ab), Ok(ba)) = (price_a.safe_add(price_b), price_b.safe_add(price_a)) {
                prop_assert_eq!(ab, ba);
            }
    }

    /// Price addition with zero is identity: a + 0 == a
    #[test]
    fn price_addition_identity(a in valid_positive_f64()) {
        if let Ok(price_a) = Price::new(a) {
            let result = price_a.safe_add(Price::ZERO);
            prop_assert_eq!(result, Ok(price_a));
        }
    }

    /// Price subtraction inverse: (a + b) - b == a
    #[test]
    fn price_subtraction_inverse(
        a in valid_positive_f64(),
        b in valid_positive_f64()
    ) {
        if let (Ok(price_a), Ok(price_b)) = (Price::new(a), Price::new(b))
            && let Ok(sum) = price_a.safe_add(price_b)
                && let Ok(result) = sum.safe_sub(price_b) {
                    prop_assert_eq!(result, price_a);
                }
    }

    /// Price multiplication by 1 is identity: a * 1 == a
    #[test]
    fn price_multiplication_identity(a in valid_positive_f64()) {
        if let Ok(price_a) = Price::new(a) {
            let result = price_a.safe_mul(Decimal::ONE);
            prop_assert_eq!(result, Ok(price_a));
        }
    }

    /// Price multiplication by 0 gives zero: a * 0 == 0
    #[test]
    fn price_multiplication_zero(a in valid_positive_f64()) {
        if let Ok(price_a) = Price::new(a) {
            let result = price_a.safe_mul(Decimal::ZERO);
            prop_assert_eq!(result, Ok(Price::ZERO));
        }
    }

    /// Price division by 1 is identity: a / 1 == a
    #[test]
    fn price_division_identity(a in valid_positive_f64()) {
        if let Ok(price_a) = Price::new(a) {
            let result = price_a.safe_div(Decimal::ONE);
            prop_assert_eq!(result, Ok(price_a));
        }
    }

    /// Price serde roundtrip preserves value
    #[test]
    fn price_serde_roundtrip(a in valid_positive_f64()) {
        if let Ok(price) = Price::new(a) {
            let json = serde_json::to_string(&price).expect("serialize");
            let deserialized: Price = serde_json::from_str(&json).expect("deserialize");
            prop_assert_eq!(price, deserialized);
        }
    }

    /// Price comparison is consistent with Decimal comparison
    #[test]
    fn price_comparison_consistent(
        a in valid_positive_f64(),
        b in valid_positive_f64()
    ) {
        if let (Ok(price_a), Ok(price_b)) = (Price::new(a), Price::new(b)) {
            let price_cmp = price_a.cmp(&price_b);
            let decimal_cmp = price_a.get().cmp(&price_b.get());
            prop_assert_eq!(price_cmp, decimal_cmp);
        }
    }
}

// ============================================================================
// Quantity Property Tests
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    /// Quantity addition is commutative: a + b == b + a
    #[test]
    fn quantity_addition_commutative(
        a in valid_positive_f64(),
        b in valid_positive_f64()
    ) {
        if let (Ok(qty_a), Ok(qty_b)) = (Quantity::new(a), Quantity::new(b))
            && let (Ok(ab), Ok(ba)) = (qty_a.safe_add(qty_b), qty_b.safe_add(qty_a)) {
                prop_assert_eq!(ab, ba);
            }
    }

    /// Quantity addition with zero is identity: a + 0 == a
    #[test]
    fn quantity_addition_identity(a in valid_positive_f64()) {
        if let Ok(qty_a) = Quantity::new(a) {
            let result = qty_a.safe_add(Quantity::ZERO);
            prop_assert_eq!(result, Ok(qty_a));
        }
    }

    /// Quantity subtraction inverse: (a + b) - b == a
    #[test]
    fn quantity_subtraction_inverse(
        a in valid_positive_f64(),
        b in valid_positive_f64()
    ) {
        if let (Ok(qty_a), Ok(qty_b)) = (Quantity::new(a), Quantity::new(b))
            && let Ok(sum) = qty_a.safe_add(qty_b)
                && let Ok(result) = sum.safe_sub(qty_b) {
                    prop_assert_eq!(result, qty_a);
                }
    }

    /// Quantity fill ratio is always in [0, 1]
    #[test]
    fn quantity_fill_ratio_bounded(
        total in valid_positive_f64(),
        filled in valid_positive_f64()
    ) {
        if let (Ok(qty_total), Ok(qty_filled)) = (Quantity::new(total), Quantity::new(filled)) {
            let ratio = qty_total.fill_ratio(qty_filled);
            prop_assert!(ratio >= Decimal::ZERO);
            prop_assert!(ratio <= Decimal::ONE);
        }
    }

    /// Quantity remaining_after is always <= total
    #[test]
    fn quantity_remaining_bounded(
        total in valid_positive_f64(),
        filled in valid_positive_f64()
    ) {
        if let (Ok(qty_total), Ok(qty_filled)) = (Quantity::new(total), Quantity::new(filled)) {
            let remaining = qty_total.remaining_after(qty_filled);
            prop_assert!(remaining <= qty_total);
        }
    }

    /// Quantity serde roundtrip preserves value
    #[test]
    fn quantity_serde_roundtrip(a in valid_positive_f64()) {
        if let Ok(qty) = Quantity::new(a) {
            let json = serde_json::to_string(&qty).expect("serialize");
            let deserialized: Quantity = serde_json::from_str(&json).expect("deserialize");
            prop_assert_eq!(qty, deserialized);
        }
    }
}

// ============================================================================
// Decimal Checked Arithmetic Property Tests
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    /// Decimal addition is commutative
    #[test]
    fn decimal_addition_commutative(
        a in valid_decimal(),
        b in valid_decimal()
    ) {
        if let (Ok(ab), Ok(ba)) = (a.safe_add(b), b.safe_add(a)) {
            prop_assert_eq!(ab, ba);
        }
    }

    /// Decimal multiplication is commutative
    #[test]
    fn decimal_multiplication_commutative(
        a in valid_decimal(),
        b in valid_decimal()
    ) {
        if let (Ok(ab), Ok(ba)) = (a.safe_mul(b), b.safe_mul(a)) {
            prop_assert_eq!(ab, ba);
        }
    }

    /// Decimal addition with zero is identity
    #[test]
    fn decimal_addition_identity(a in valid_decimal()) {
        let result = a.safe_add(Decimal::ZERO);
        prop_assert_eq!(result, Ok(a));
    }

    /// Decimal multiplication by one is identity
    #[test]
    fn decimal_multiplication_identity(a in valid_decimal()) {
        let result = a.safe_mul(Decimal::ONE);
        prop_assert_eq!(result, Ok(a));
    }

    /// Decimal multiplication by zero gives zero
    #[test]
    fn decimal_multiplication_zero(a in valid_decimal()) {
        let result = a.safe_mul(Decimal::ZERO);
        prop_assert_eq!(result, Ok(Decimal::ZERO));
    }

    /// Decimal subtraction inverse: (a + b) - b == a
    #[test]
    fn decimal_subtraction_inverse(
        a in valid_decimal(),
        b in valid_decimal()
    ) {
        if let Ok(sum) = a.safe_add(b)
            && let Ok(result) = sum.safe_sub(b) {
                prop_assert_eq!(result, a);
            }
    }

    /// Decimal division by self gives one (for non-zero)
    #[test]
    fn decimal_division_self(a in valid_decimal().prop_filter("non-zero", |d| !d.is_zero())) {
        let result = a.safe_div(a);
        prop_assert_eq!(result, Ok(Decimal::ONE));
    }

    /// Decimal division by zero always fails
    #[test]
    fn decimal_division_by_zero(a in valid_decimal()) {
        let result = a.safe_div(Decimal::ZERO);
        prop_assert_eq!(result, Err(ArithmeticError::DivisionByZero));
    }
}

// ============================================================================
// Timestamp Property Tests
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    /// Timestamp from_millis roundtrip
    #[test]
    fn timestamp_millis_roundtrip(millis in valid_timestamp_millis()) {
        if let Some(ts) = Timestamp::from_millis(millis) {
            prop_assert_eq!(ts.timestamp_millis(), millis);
        }
    }

    /// Timestamp from_secs roundtrip
    #[test]
    fn timestamp_secs_roundtrip(secs in 946684800i64..4102444800i64) {
        if let Some(ts) = Timestamp::from_secs(secs) {
            prop_assert_eq!(ts.timestamp_secs(), secs);
        }
    }

    /// Timestamp addition and subtraction are inverses
    #[test]
    fn timestamp_add_sub_inverse(
        millis in valid_timestamp_millis(),
        delta in 0i64..86400000i64  // Up to 1 day in millis
    ) {
        if let Some(ts) = Timestamp::from_millis(millis) {
            let later = ts.add_millis(delta);
            let back = later.add_millis(-delta);
            prop_assert_eq!(ts.timestamp_millis(), back.timestamp_millis());
        }
    }

    /// Timestamp ordering is consistent
    #[test]
    fn timestamp_ordering_consistent(
        a in valid_timestamp_millis(),
        b in valid_timestamp_millis()
    ) {
        if let (Some(ts_a), Some(ts_b)) = (Timestamp::from_millis(a), Timestamp::from_millis(b)) {
            let ts_cmp = ts_a.cmp(&ts_b);
            let millis_cmp = a.cmp(&b);
            prop_assert_eq!(ts_cmp, millis_cmp);
        }
    }

    /// Timestamp is_before and is_after are consistent
    #[test]
    fn timestamp_before_after_consistent(
        a in valid_timestamp_millis(),
        b in valid_timestamp_millis()
    ) {
        if let (Some(ts_a), Some(ts_b)) = (Timestamp::from_millis(a), Timestamp::from_millis(b)) {
            if a < b {
                prop_assert!(ts_a.is_before(&ts_b));
                prop_assert!(ts_b.is_after(&ts_a));
            } else if a > b {
                prop_assert!(ts_b.is_before(&ts_a));
                prop_assert!(ts_a.is_after(&ts_b));
            } else {
                prop_assert!(!ts_a.is_before(&ts_b));
                prop_assert!(!ts_a.is_after(&ts_b));
            }
        }
    }

    /// Timestamp serde roundtrip
    #[test]
    fn timestamp_serde_roundtrip(millis in valid_timestamp_millis()) {
        if let Some(ts) = Timestamp::from_millis(millis) {
            let json = serde_json::to_string(&ts).expect("serialize");
            let deserialized: Timestamp = serde_json::from_str(&json).expect("deserialize");
            // Allow small differences due to serialization format
            let diff = (ts.timestamp_millis() - deserialized.timestamp_millis()).abs();
            prop_assert!(diff < 1000, "Timestamps differ by {} ms", diff);
        }
    }
}

// ============================================================================
// Integer Checked Arithmetic Property Tests
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    /// u64 addition is commutative
    #[test]
    fn u64_addition_commutative(
        a in 0u64..u64::MAX / 2,
        b in 0u64..u64::MAX / 2
    ) {
        if let (Ok(ab), Ok(ba)) = (a.safe_add(b), b.safe_add(a)) {
            prop_assert_eq!(ab, ba);
        }
    }

    /// u64 multiplication is commutative
    #[test]
    fn u64_multiplication_commutative(
        a in 0u64..u32::MAX as u64,
        b in 0u64..u32::MAX as u64
    ) {
        if let (Ok(ab), Ok(ba)) = (a.safe_mul(b), b.safe_mul(a)) {
            prop_assert_eq!(ab, ba);
        }
    }

    /// u64 addition with zero is identity
    #[test]
    fn u64_addition_identity(a in any::<u64>()) {
        let result = a.safe_add(0);
        prop_assert_eq!(result, Ok(a));
    }

    /// u64 multiplication by one is identity
    #[test]
    fn u64_multiplication_identity(a in any::<u64>()) {
        let result = a.safe_mul(1);
        prop_assert_eq!(result, Ok(a));
    }

    /// u64 subtraction inverse: (a + b) - b == a
    #[test]
    fn u64_subtraction_inverse(
        a in 0u64..u64::MAX / 2,
        b in 0u64..u64::MAX / 2
    ) {
        if let Ok(sum) = a.safe_add(b)
            && let Ok(result) = sum.safe_sub(b) {
                prop_assert_eq!(result, a);
            }
    }

    /// i64 addition is commutative
    #[test]
    fn i64_addition_commutative(
        a in i64::MIN / 2..i64::MAX / 2,
        b in i64::MIN / 2..i64::MAX / 2
    ) {
        if let (Ok(ab), Ok(ba)) = (a.safe_add(b), b.safe_add(a)) {
            prop_assert_eq!(ab, ba);
        }
    }

    /// i64 subtraction inverse: (a + b) - b == a
    #[test]
    fn i64_subtraction_inverse(
        a in i64::MIN / 2..i64::MAX / 2,
        b in i64::MIN / 2..i64::MAX / 2
    ) {
        if let Ok(sum) = a.safe_add(b)
            && let Ok(result) = sum.safe_sub(b) {
                prop_assert_eq!(result, a);
            }
    }
}

// ============================================================================
// Edge Case Tests (Non-Property-Based)
// ============================================================================

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod edge_cases {
    use super::*;
    use crate::domain::value_objects::ids::{CounterpartyId, QuoteId, RfqId, TradeId, VenueId};
    use crate::domain::value_objects::rfq_state::RfqState;
    use crate::domain::value_objects::symbol::Symbol;

    #[test]
    fn price_very_small_values() {
        let small = Price::new(0.000001).unwrap();
        assert!(small.is_positive());
        assert!(!small.is_zero());
    }

    #[test]
    fn price_very_large_values() {
        let large = Price::new(999_999_999.99).unwrap();
        assert!(large.is_positive());
    }

    #[test]
    fn quantity_very_small_values() {
        let small = Quantity::new(0.000001).unwrap();
        assert!(small.is_positive());
        assert!(!small.is_zero());
    }

    #[test]
    fn quantity_very_large_values() {
        let large = Quantity::new(999_999_999.99).unwrap();
        assert!(large.is_positive());
    }

    #[test]
    fn rfq_id_uniqueness() {
        let ids: Vec<RfqId> = (0..1000).map(|_| RfqId::new_v4()).collect();
        let unique: std::collections::HashSet<_> = ids.iter().collect();
        assert_eq!(ids.len(), unique.len());
    }

    #[test]
    fn quote_id_uniqueness() {
        let ids: Vec<QuoteId> = (0..1000).map(|_| QuoteId::new_v4()).collect();
        let unique: std::collections::HashSet<_> = ids.iter().collect();
        assert_eq!(ids.len(), unique.len());
    }

    #[test]
    fn trade_id_uniqueness() {
        let ids: Vec<TradeId> = (0..1000).map(|_| TradeId::new_v4()).collect();
        let unique: std::collections::HashSet<_> = ids.iter().collect();
        assert_eq!(ids.len(), unique.len());
    }

    #[test]
    fn venue_id_case_sensitivity() {
        let id1 = VenueId::new("0x-api");
        let id2 = VenueId::new("0X-API");
        // VenueId preserves case
        assert_ne!(id1, id2);
    }

    #[test]
    fn counterparty_id_whitespace_handling() {
        let id1 = CounterpartyId::new("DESK_001");
        let id2 = CounterpartyId::new("DESK_001 ");
        // CounterpartyId preserves whitespace
        assert_ne!(id1, id2);
    }

    #[test]
    fn symbol_unicode_rejection() {
        let result = Symbol::new("BTC/USD€");
        assert!(result.is_err());
    }

    #[test]
    fn symbol_numeric_assets() {
        let result = Symbol::new("1INCH/USDC");
        assert!(result.is_ok());
    }

    #[test]
    fn rfq_state_all_states_have_unique_u8() {
        let states = [
            RfqState::Created,
            RfqState::QuoteRequesting,
            RfqState::QuotesReceived,
            RfqState::ClientSelecting,
            RfqState::Executing,
            RfqState::Executed,
            RfqState::Failed,
            RfqState::Cancelled,
            RfqState::Expired,
        ];

        let u8_values: Vec<u8> = states.iter().map(|s| s.as_u8()).collect();
        let unique: std::collections::HashSet<_> = u8_values.iter().collect();
        assert_eq!(u8_values.len(), unique.len());
    }

    #[test]
    fn timestamp_epoch() {
        let epoch = Timestamp::from_secs(0).unwrap();
        assert_eq!(epoch.timestamp_secs(), 0);
        assert!(epoch.is_expired());
    }

    #[test]
    fn timestamp_far_future() {
        // Year 2100
        let future = Timestamp::from_secs(4102444800).unwrap();
        assert!(!future.is_expired());
    }

    #[test]
    fn price_subtraction_to_zero() {
        let a = Price::new(100.0).unwrap();
        let b = Price::new(100.0).unwrap();
        let result = a.safe_sub(b).unwrap();
        assert!(result.is_zero());
    }

    #[test]
    fn quantity_subtraction_to_zero() {
        let a = Quantity::new(100.0).unwrap();
        let b = Quantity::new(100.0).unwrap();
        let result = a.safe_sub(b).unwrap();
        assert!(result.is_zero());
    }

    #[test]
    fn decimal_precision_preserved() {
        let price = Price::new(123.456789).unwrap();
        let json = serde_json::to_string(&price).unwrap();
        let deserialized: Price = serde_json::from_str(&json).unwrap();
        assert_eq!(price, deserialized);
    }
}
