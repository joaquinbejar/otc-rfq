//! # Compensating Trade Generator
//!
//! Generates compensating (reversal) trades for multi-leg execution rollback.
//!
//! When a multi-leg execution fails partway through, previously executed legs
//! must be reversed. This module provides the logic to generate the opposite
//! trades needed to unwind executed positions.
//!
//! # Example
//!
//! ```ignore
//! let generator = CompensatingTradeGenerator::new();
//! let compensating = generator.generate(&executed_leg_result);
//! // compensating.side is opposite of original
//! ```

use crate::domain::value_objects::{Instrument, OrderSide, Price, Quantity, Timestamp};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Result of executing a single leg, used as input for generating compensating trades.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LegExecutionResult {
    /// Index of the leg in the strategy (0-based).
    pub leg_index: usize,
    /// Instrument that was traded.
    pub instrument: Instrument,
    /// Side of the original trade (Buy or Sell).
    pub side: OrderSide,
    /// Price at which the leg was executed.
    pub price: Price,
    /// Quantity that was executed.
    pub quantity: Quantity,
    /// Venue-assigned execution ID.
    pub execution_id: String,
    /// When the leg was executed.
    pub executed_at: Timestamp,
}

impl LegExecutionResult {
    /// Creates a new leg execution result.
    #[must_use]
    pub fn new(
        leg_index: usize,
        instrument: Instrument,
        side: OrderSide,
        price: Price,
        quantity: Quantity,
        execution_id: String,
    ) -> Self {
        Self {
            leg_index,
            instrument,
            side,
            price,
            quantity,
            execution_id,
            executed_at: Timestamp::now(),
        }
    }

    /// Creates a new leg execution result with a specific timestamp.
    #[must_use]
    pub fn with_timestamp(
        leg_index: usize,
        instrument: Instrument,
        side: OrderSide,
        price: Price,
        quantity: Quantity,
        execution_id: String,
        executed_at: Timestamp,
    ) -> Self {
        Self {
            leg_index,
            instrument,
            side,
            price,
            quantity,
            execution_id,
            executed_at,
        }
    }
}

impl fmt::Display for LegExecutionResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Leg {} {} {} {} @ {} (exec_id: {})",
            self.leg_index,
            self.side,
            self.quantity,
            self.instrument,
            self.price,
            self.execution_id
        )
    }
}

/// A compensating trade to reverse an executed leg.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompensatingTrade {
    /// The original execution being reversed.
    pub original_leg_index: usize,
    /// Original execution ID being compensated.
    pub original_execution_id: String,
    /// Instrument to trade.
    pub instrument: Instrument,
    /// Side of the compensating trade (opposite of original).
    pub compensating_side: OrderSide,
    /// Price for the compensating trade.
    pub compensating_price: Price,
    /// Quantity to trade (same as original).
    pub quantity: Quantity,
    /// Reason for the compensating trade.
    pub reason: String,
    /// When the compensating trade was generated.
    pub generated_at: Timestamp,
}

impl CompensatingTrade {
    /// Creates a new compensating trade.
    #[must_use]
    pub fn new(
        original_leg_index: usize,
        original_execution_id: String,
        instrument: Instrument,
        compensating_side: OrderSide,
        compensating_price: Price,
        quantity: Quantity,
        reason: String,
    ) -> Self {
        Self {
            original_leg_index,
            original_execution_id,
            instrument,
            compensating_side,
            compensating_price,
            quantity,
            reason,
            generated_at: Timestamp::now(),
        }
    }
}

impl fmt::Display for CompensatingTrade {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Compensating leg {} {} {} {} @ {} (reversing {})",
            self.original_leg_index,
            self.compensating_side,
            self.quantity,
            self.instrument,
            self.compensating_price,
            self.original_execution_id
        )
    }
}

/// Generator for compensating trades.
///
/// Creates the opposite trades needed to unwind executed positions
/// during multi-leg execution rollback.
#[derive(Debug, Clone, Default)]
pub struct CompensatingTradeGenerator {
    /// Default reason for compensating trades.
    default_reason: String,
}

impl CompensatingTradeGenerator {
    /// Creates a new compensating trade generator.
    #[must_use]
    pub fn new() -> Self {
        Self {
            default_reason: "multi-leg execution rollback".to_string(),
        }
    }

    /// Creates a generator with a custom default reason.
    #[must_use]
    pub fn with_reason(reason: impl Into<String>) -> Self {
        Self {
            default_reason: reason.into(),
        }
    }

    /// Generates a compensating trade for an executed leg.
    ///
    /// The compensating trade has the opposite side of the original:
    /// - Buy becomes Sell
    /// - Sell becomes Buy
    ///
    /// The price and quantity remain the same to ensure exact reversal.
    #[must_use]
    pub fn generate(&self, executed: &LegExecutionResult) -> CompensatingTrade {
        self.generate_with_reason(executed, &self.default_reason)
    }

    /// Generates a compensating trade with a custom reason.
    #[must_use]
    pub fn generate_with_reason(
        &self,
        executed: &LegExecutionResult,
        reason: &str,
    ) -> CompensatingTrade {
        let compensating_side = match executed.side {
            OrderSide::Buy => OrderSide::Sell,
            OrderSide::Sell => OrderSide::Buy,
        };

        CompensatingTrade::new(
            executed.leg_index,
            executed.execution_id.clone(),
            executed.instrument.clone(),
            compensating_side,
            executed.price,
            executed.quantity,
            reason.to_string(),
        )
    }

    /// Generates compensating trades for multiple executed legs.
    ///
    /// Trades are generated in reverse order (LIFO) to properly unwind
    /// the execution sequence.
    #[must_use]
    pub fn generate_all(&self, executed_legs: &[LegExecutionResult]) -> Vec<CompensatingTrade> {
        executed_legs
            .iter()
            .rev()
            .map(|leg| self.generate(leg))
            .collect()
    }

    /// Generates compensating trades with a custom reason for all legs.
    #[must_use]
    pub fn generate_all_with_reason(
        &self,
        executed_legs: &[LegExecutionResult],
        reason: &str,
    ) -> Vec<CompensatingTrade> {
        executed_legs
            .iter()
            .rev()
            .map(|leg| self.generate_with_reason(leg, reason))
            .collect()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn create_test_instrument() -> Instrument {
        use crate::domain::value_objects::{AssetClass, Symbol};
        let symbol = Symbol::new("BTC/USD").unwrap();
        Instrument::builder(symbol, AssetClass::CryptoSpot).build()
    }

    fn create_test_leg_result(leg_index: usize, side: OrderSide) -> LegExecutionResult {
        LegExecutionResult::new(
            leg_index,
            create_test_instrument(),
            side,
            Price::new(50000.0).unwrap(),
            Quantity::new(1.0).unwrap(),
            format!("exec-{}", leg_index),
        )
    }

    #[test]
    fn test_generate_compensating_trade_buy_to_sell() {
        let generator = CompensatingTradeGenerator::new();
        let executed = create_test_leg_result(0, OrderSide::Buy);

        let compensating = generator.generate(&executed);

        assert_eq!(compensating.compensating_side, OrderSide::Sell);
        assert_eq!(compensating.original_leg_index, 0);
        assert_eq!(compensating.instrument, executed.instrument);
        assert_eq!(compensating.compensating_price, executed.price);
        assert_eq!(compensating.quantity, executed.quantity);
    }

    #[test]
    fn test_generate_compensating_trade_sell_to_buy() {
        let generator = CompensatingTradeGenerator::new();
        let executed = create_test_leg_result(1, OrderSide::Sell);

        let compensating = generator.generate(&executed);

        assert_eq!(compensating.compensating_side, OrderSide::Buy);
        assert_eq!(compensating.original_leg_index, 1);
    }

    #[test]
    fn test_generate_with_custom_reason() {
        let generator = CompensatingTradeGenerator::new();
        let executed = create_test_leg_result(0, OrderSide::Buy);
        let reason = "leg 2 failed";

        let compensating = generator.generate_with_reason(&executed, reason);

        assert_eq!(compensating.reason, reason);
    }

    #[test]
    fn test_generate_all_reverses_order() {
        let generator = CompensatingTradeGenerator::new();
        let executed_legs = vec![
            create_test_leg_result(0, OrderSide::Buy),
            create_test_leg_result(1, OrderSide::Sell),
            create_test_leg_result(2, OrderSide::Buy),
        ];

        let compensating = generator.generate_all(&executed_legs);

        assert_eq!(compensating.len(), 3);
        assert_eq!(compensating[0].original_leg_index, 2);
        assert_eq!(compensating[1].original_leg_index, 1);
        assert_eq!(compensating[2].original_leg_index, 0);
    }

    #[test]
    fn test_generate_all_empty_input() {
        let generator = CompensatingTradeGenerator::new();
        let executed_legs: Vec<LegExecutionResult> = vec![];

        let compensating = generator.generate_all(&executed_legs);

        assert!(compensating.is_empty());
    }

    #[test]
    fn test_leg_execution_result_display() {
        let leg = create_test_leg_result(0, OrderSide::Buy);
        let display = format!("{}", leg);

        assert!(display.contains("Leg 0"));
        assert!(display.contains("BUY"));
        assert!(display.contains("BTC/USD"));
    }

    #[test]
    fn test_compensating_trade_display() {
        let generator = CompensatingTradeGenerator::new();
        let executed = create_test_leg_result(0, OrderSide::Buy);
        let compensating = generator.generate(&executed);

        let display = format!("{}", compensating);

        assert!(display.contains("Compensating leg 0"));
        assert!(display.contains("SELL"));
        assert!(display.contains("reversing exec-0"));
    }
}
