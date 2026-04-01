//! # Spread Metrics
//!
//! Value objects for calculating spread metrics for OTC quotes.
//!
//! This module provides three types of spread calculations:
//! - **`SpreadMetrics`**: Pre-trade bid-ask spread in basis points
//! - **`EffectiveSpread`**: Post-trade execution cost vs mid-price
//! - **`RealizedSpread`**: Effective spread adjusted for price movement
//!
//! # Examples
//!
//! ```
//! use otc_rfq::domain::value_objects::spread_metrics::SpreadMetrics;
//! use otc_rfq::domain::value_objects::Price;
//! use rust_decimal::Decimal;
//!
//! let bid = Price::new(99.0).unwrap();
//! let ask = Price::new(101.0).unwrap();
//!
//! let spread = SpreadMetrics::calculate(bid, ask).unwrap();
//! assert_eq!(spread.spread_bps(), Decimal::new(200, 0)); // 200 bps = 2%
//! ```

use crate::domain::value_objects::enums::OrderSide;
use crate::domain::value_objects::price::Price;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Basis points multiplier (1 bp = 0.01% = 0.0001).
/// Equivalent to `Decimal::new(10_000, 0)` but using `from_parts` for const context.
const BPS_MULTIPLIER: Decimal = Decimal::from_parts(10_000, 0, 0, false, 0);

/// Two as a Decimal constant for mid-price and effective spread calculations.
const TWO: Decimal = Decimal::from_parts(2, 0, 0, false, 0);

/// Spread metrics calculated from bid and ask prices.
///
/// Represents the bid-ask spread in basis points, commonly used to measure
/// market liquidity and trading costs before execution.
///
/// # Formula
///
/// ```text
/// mid_price = (bid + ask) / 2
/// spread_bps = (ask - bid) / mid × 10,000
/// half_spread_bps = spread_bps / 2
/// ```
///
/// # Examples
///
/// ```
/// use otc_rfq::domain::value_objects::spread_metrics::SpreadMetrics;
/// use otc_rfq::domain::value_objects::Price;
/// use rust_decimal::Decimal;
///
/// let spread = SpreadMetrics::calculate(
///     Price::new(99.0).unwrap(),   // bid
///     Price::new(101.0).unwrap(),  // ask
/// ).unwrap();
///
/// assert_eq!(spread.spread_bps(), Decimal::new(200, 0)); // 2% = 200 bps
/// assert_eq!(spread.half_spread_bps(), Decimal::new(100, 0)); // 1% = 100 bps
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpreadMetrics {
    /// Bid price (best buy price).
    bid_price: Price,
    /// Ask price (best sell price).
    ask_price: Price,
    /// Mid price: (bid + ask) / 2.
    mid_price: Price,
    /// Spread in basis points: (ask - bid) / mid × 10,000.
    spread_bps: Decimal,
    /// Half spread in basis points: spread_bps / 2.
    half_spread_bps: Decimal,
}

impl SpreadMetrics {
    /// Calculates spread metrics from bid and ask prices.
    ///
    /// # Arguments
    ///
    /// * `bid` - Best bid price (highest buy price)
    /// * `ask` - Best ask price (lowest sell price)
    ///
    /// # Returns
    ///
    /// `Some(SpreadMetrics)` if calculation succeeds, `None` if:
    /// - Mid price is zero (division by zero protection)
    /// - Bid is greater than ask (crossed market)
    /// - Arithmetic overflow occurs (extremely unlikely)
    ///
    /// # Examples
    ///
    /// ```
    /// use otc_rfq::domain::value_objects::spread_metrics::SpreadMetrics;
    /// use otc_rfq::domain::value_objects::Price;
    ///
    /// // Normal spread
    /// let spread = SpreadMetrics::calculate(
    ///     Price::new(99.0).unwrap(),
    ///     Price::new(101.0).unwrap(),
    /// ).unwrap();
    /// assert!(!spread.is_crossed());
    ///
    /// // Crossed market returns None
    /// let crossed = SpreadMetrics::calculate(
    ///     Price::new(101.0).unwrap(),
    ///     Price::new(99.0).unwrap(),
    /// );
    /// assert!(crossed.is_none());
    /// ```
    #[must_use]
    pub fn calculate(bid: Price, ask: Price) -> Option<Self> {
        let bid_value = bid.get();
        let ask_value = ask.get();

        // Crossed market check (bid > ask)
        if bid_value > ask_value {
            return None;
        }

        // Calculate mid price: (bid + ask) / 2
        let sum = bid_value.checked_add(ask_value)?;
        let mid_value = sum.checked_div(TWO)?;

        // Cannot calculate spread with zero mid price
        if mid_value.is_zero() {
            return None;
        }

        // Calculate spread: (ask - bid) / mid × 10,000
        let spread = ask_value.checked_sub(bid_value)?;
        let ratio = spread.checked_div(mid_value)?;
        let spread_bps = ratio.checked_mul(BPS_MULTIPLIER)?;
        let half_spread_bps = spread_bps.checked_div(TWO)?;

        let mid_price = Price::from_decimal(mid_value).ok()?;

        Some(Self {
            bid_price: bid,
            ask_price: ask,
            mid_price,
            spread_bps,
            half_spread_bps,
        })
    }

    /// Returns the bid price.
    #[inline]
    #[must_use]
    pub const fn bid_price(&self) -> Price {
        self.bid_price
    }

    /// Returns the ask price.
    #[inline]
    #[must_use]
    pub const fn ask_price(&self) -> Price {
        self.ask_price
    }

    /// Returns the mid price.
    #[inline]
    #[must_use]
    pub const fn mid_price(&self) -> Price {
        self.mid_price
    }

    /// Returns the spread in basis points.
    #[inline]
    #[must_use]
    pub const fn spread_bps(&self) -> Decimal {
        self.spread_bps
    }

    /// Returns the half spread in basis points.
    ///
    /// Half spread represents the cost of a single side of the trade.
    #[inline]
    #[must_use]
    pub const fn half_spread_bps(&self) -> Decimal {
        self.half_spread_bps
    }

    /// Returns true if the market is crossed (bid > ask).
    ///
    /// Note: `calculate()` returns `None` for crossed markets,
    /// so this will always return `false` for valid `SpreadMetrics`.
    #[inline]
    #[must_use]
    pub fn is_crossed(&self) -> bool {
        self.bid_price.get() > self.ask_price.get()
    }

    /// Returns true if the market is locked (bid == ask).
    #[inline]
    #[must_use]
    pub fn is_locked(&self) -> bool {
        self.spread_bps.is_zero()
    }

    /// Returns the spread as a percentage (bps / 100).
    #[must_use]
    pub fn spread_percentage(&self) -> Decimal {
        self.spread_bps / Decimal::ONE_HUNDRED
    }
}

impl fmt::Display for SpreadMetrics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "SpreadMetrics({} bps, mid={})",
            self.spread_bps,
            self.mid_price.get()
        )
    }
}

/// Effective spread calculated post-trade.
///
/// Measures the actual execution cost compared to the mid-price at the time
/// of execution. This is a more accurate measure of trading costs than the
/// quoted spread because it reflects the actual price received.
///
/// # Formula
///
/// ```text
/// effective_spread_bps = 2 × |execution_price - mid_price| / mid_price × 10,000
/// ```
///
/// The factor of 2 makes the effective spread comparable to the quoted spread
/// (which measures the full round-trip cost).
///
/// # Examples
///
/// ```
/// use otc_rfq::domain::value_objects::spread_metrics::EffectiveSpread;
/// use otc_rfq::domain::value_objects::{Price, OrderSide};
/// use rust_decimal::Decimal;
///
/// // Buy at 100.5 when mid is 100.0 → 100 bps effective spread
/// let effective = EffectiveSpread::calculate(
///     Price::new(100.5).unwrap(),  // execution price
///     Price::new(100.0).unwrap(),  // mid price
///     OrderSide::Buy,
/// ).unwrap();
///
/// assert_eq!(effective.effective_spread_bps(), Decimal::new(100, 0));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectiveSpread {
    /// Price at which the trade was executed.
    execution_price: Price,
    /// Mid price at the time of execution.
    mid_price: Price,
    /// Effective spread in basis points.
    effective_spread_bps: Decimal,
    /// Side of the trade.
    side: OrderSide,
}

impl EffectiveSpread {
    /// Calculates effective spread from execution price and mid price.
    ///
    /// # Arguments
    ///
    /// * `execution_price` - The actual execution price
    /// * `mid_price` - The mid price at the time of execution
    /// * `side` - The side of the trade (Buy or Sell)
    ///
    /// # Returns
    ///
    /// `Some(EffectiveSpread)` if calculation succeeds, `None` if:
    /// - Mid price is zero (division by zero protection)
    /// - Arithmetic overflow occurs (extremely unlikely)
    ///
    /// # Examples
    ///
    /// ```
    /// use otc_rfq::domain::value_objects::spread_metrics::EffectiveSpread;
    /// use otc_rfq::domain::value_objects::{Price, OrderSide};
    ///
    /// // Execute at mid → 0 effective spread
    /// let at_mid = EffectiveSpread::calculate(
    ///     Price::new(100.0).unwrap(),
    ///     Price::new(100.0).unwrap(),
    ///     OrderSide::Buy,
    /// ).unwrap();
    /// assert!(at_mid.effective_spread_bps().is_zero());
    /// ```
    #[must_use]
    pub fn calculate(execution_price: Price, mid_price: Price, side: OrderSide) -> Option<Self> {
        let exec_value = execution_price.get();
        let mid_value = mid_price.get();

        // Cannot calculate with zero mid price
        if mid_value.is_zero() {
            return None;
        }

        // Calculate |execution - mid|
        let diff = if exec_value >= mid_value {
            exec_value.checked_sub(mid_value)?
        } else {
            mid_value.checked_sub(exec_value)?
        };

        // effective_spread = 2 × |exec - mid| / mid × 10,000
        let doubled = diff.checked_mul(TWO)?;
        let ratio = doubled.checked_div(mid_value)?;
        let effective_spread_bps = ratio.checked_mul(BPS_MULTIPLIER)?;

        Some(Self {
            execution_price,
            mid_price,
            effective_spread_bps,
            side,
        })
    }

    /// Returns the execution price.
    #[inline]
    #[must_use]
    pub const fn execution_price(&self) -> Price {
        self.execution_price
    }

    /// Returns the mid price at execution.
    #[inline]
    #[must_use]
    pub const fn mid_price(&self) -> Price {
        self.mid_price
    }

    /// Returns the effective spread in basis points.
    #[inline]
    #[must_use]
    pub const fn effective_spread_bps(&self) -> Decimal {
        self.effective_spread_bps
    }

    /// Returns the trade side.
    #[inline]
    #[must_use]
    pub const fn side(&self) -> OrderSide {
        self.side
    }

    /// Returns true if execution was better than the quoted half spread.
    ///
    /// # Arguments
    ///
    /// * `quoted_half_spread_bps` - The half spread from the quoted market
    ///
    /// # Examples
    ///
    /// ```
    /// use otc_rfq::domain::value_objects::spread_metrics::EffectiveSpread;
    /// use otc_rfq::domain::value_objects::{Price, OrderSide};
    /// use rust_decimal::Decimal;
    ///
    /// let effective = EffectiveSpread::calculate(
    ///     Price::new(100.25).unwrap(),  // 50 bps effective (half of 100)
    ///     Price::new(100.0).unwrap(),
    ///     OrderSide::Buy,
    /// ).unwrap();
    ///
    /// // Better than 100 bps quoted half spread
    /// assert!(effective.is_better_than_quoted(Decimal::new(100, 0)));
    /// // Worse than 25 bps quoted half spread
    /// assert!(!effective.is_better_than_quoted(Decimal::new(25, 0)));
    /// ```
    #[must_use]
    pub fn is_better_than_quoted(&self, quoted_half_spread_bps: Decimal) -> bool {
        // Effective spread / 2 gives the one-sided cost
        let effective_half = self.effective_spread_bps / TWO;
        effective_half < quoted_half_spread_bps
    }

    /// Returns the effective spread as a percentage (bps / 100).
    #[must_use]
    pub fn effective_spread_percentage(&self) -> Decimal {
        self.effective_spread_bps / Decimal::ONE_HUNDRED
    }
}

impl fmt::Display for EffectiveSpread {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "EffectiveSpread({} bps, {} @ {})",
            self.effective_spread_bps,
            self.side,
            self.execution_price.get()
        )
    }
}

/// Realized spread with price movement adjustment.
///
/// Measures the true cost of a trade after accounting for adverse selection
/// (price movement against the trader after execution). This is calculated
/// by comparing the mid price before and after the trade.
///
/// # Formula
///
/// ```text
/// price_movement_bps = (mid_after - mid_before) / mid_before × 10,000
///
/// For Buy:  realized_spread_bps = effective_spread_bps - price_movement_bps
/// For Sell: realized_spread_bps = effective_spread_bps + price_movement_bps
/// ```
///
/// # Interpretation (Trader Perspective)
///
/// - **Higher realized spread**: Price moved against the trader after execution
///   (e.g., buy then price drops, sell then price rises)
/// - **Lower realized spread**: Price moved favorably after execution
/// - **Negative realized spread**: Exceptional case where favorable movement
///   exceeded the original execution cost
///
/// # Examples
///
/// ```
/// use otc_rfq::domain::value_objects::spread_metrics::{EffectiveSpread, RealizedSpread};
/// use otc_rfq::domain::value_objects::{Price, OrderSide};
/// use rust_decimal::Decimal;
///
/// // Buy at 100.5 when mid is 100.0
/// let effective = EffectiveSpread::calculate(
///     Price::new(100.5).unwrap(),
///     Price::new(100.0).unwrap(),
///     OrderSide::Buy,
/// ).unwrap();
///
/// // Price drops to 99.5 after 5 minutes (adverse selection)
/// let realized = RealizedSpread::calculate(
///     &effective,
///     Price::new(100.0).unwrap(),  // mid before
///     Price::new(99.5).unwrap(),   // mid after
/// ).unwrap();
///
/// // Realized spread is higher due to adverse selection
/// assert!(realized.realized_spread_bps() > effective.effective_spread_bps());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealizedSpread {
    /// Effective spread in basis points.
    effective_spread_bps: Decimal,
    /// Price movement in basis points (positive = price increased).
    price_movement_bps: Decimal,
    /// Realized spread in basis points.
    realized_spread_bps: Decimal,
    /// Side of the original trade.
    side: OrderSide,
}

impl RealizedSpread {
    /// Calculates realized spread from effective spread and price movement.
    ///
    /// # Arguments
    ///
    /// * `effective` - The effective spread from the original trade
    /// * `mid_before` - Mid price at the time of execution
    /// * `mid_after` - Mid price after the measurement window (e.g., 5 minutes)
    ///
    /// # Returns
    ///
    /// `Some(RealizedSpread)` if calculation succeeds, `None` if:
    /// - Mid before is zero (division by zero protection)
    /// - Arithmetic overflow occurs (extremely unlikely)
    ///
    /// # Examples
    ///
    /// ```
    /// use otc_rfq::domain::value_objects::spread_metrics::{EffectiveSpread, RealizedSpread};
    /// use otc_rfq::domain::value_objects::{Price, OrderSide};
    ///
    /// let effective = EffectiveSpread::calculate(
    ///     Price::new(100.5).unwrap(),
    ///     Price::new(100.0).unwrap(),
    ///     OrderSide::Buy,
    /// ).unwrap();
    ///
    /// // No price movement → realized equals effective
    /// let realized = RealizedSpread::calculate(
    ///     &effective,
    ///     Price::new(100.0).unwrap(),
    ///     Price::new(100.0).unwrap(),
    /// ).unwrap();
    ///
    /// assert_eq!(realized.realized_spread_bps(), effective.effective_spread_bps());
    /// ```
    #[must_use]
    pub fn calculate(
        effective: &EffectiveSpread,
        mid_before: Price,
        mid_after: Price,
    ) -> Option<Self> {
        let before_value = mid_before.get();
        let after_value = mid_after.get();

        // Cannot calculate with zero mid price
        if before_value.is_zero() {
            return None;
        }

        // Calculate price movement: (after - before) / before × 10,000
        let diff = after_value.checked_sub(before_value)?;
        let ratio = diff.checked_div(before_value)?;
        let price_movement_bps = ratio.checked_mul(BPS_MULTIPLIER)?;

        // Calculate realized spread based on side
        // Buy: realized = effective - movement (price drop hurts buyer)
        // Sell: realized = effective + movement (price drop helps seller)
        let realized_spread_bps = match effective.side() {
            OrderSide::Buy => effective
                .effective_spread_bps
                .checked_sub(price_movement_bps)?,
            OrderSide::Sell => effective
                .effective_spread_bps
                .checked_add(price_movement_bps)?,
        };

        Some(Self {
            effective_spread_bps: effective.effective_spread_bps,
            price_movement_bps,
            realized_spread_bps,
            side: effective.side(),
        })
    }

    /// Returns the effective spread in basis points.
    #[inline]
    #[must_use]
    pub const fn effective_spread_bps(&self) -> Decimal {
        self.effective_spread_bps
    }

    /// Returns the price movement in basis points.
    ///
    /// Positive values indicate price increased, negative values indicate decrease.
    #[inline]
    #[must_use]
    pub const fn price_movement_bps(&self) -> Decimal {
        self.price_movement_bps
    }

    /// Returns the realized spread in basis points.
    #[inline]
    #[must_use]
    pub const fn realized_spread_bps(&self) -> Decimal {
        self.realized_spread_bps
    }

    /// Returns the trade side.
    #[inline]
    #[must_use]
    pub const fn side(&self) -> OrderSide {
        self.side
    }

    /// Returns true if price moved against the trader after execution.
    ///
    /// This occurs when:
    /// - Buy side: price dropped after purchase
    /// - Sell side: price rose after sale
    ///
    /// From a market microstructure perspective, this indicates the trader
    /// may have been on the wrong side of information flow.
    #[must_use]
    pub fn has_adverse_selection(&self) -> bool {
        self.realized_spread_bps > self.effective_spread_bps
    }

    /// Returns the adverse selection component in basis points.
    ///
    /// Positive values indicate adverse selection, negative values indicate
    /// favorable price movement.
    #[must_use]
    pub fn adverse_selection_bps(&self) -> Decimal {
        self.realized_spread_bps - self.effective_spread_bps
    }
}

impl fmt::Display for RealizedSpread {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let adverse = if self.has_adverse_selection() {
            " [adverse]"
        } else {
            ""
        };
        write!(
            f,
            "RealizedSpread({} bps, movement={} bps{})",
            self.realized_spread_bps, self.price_movement_bps, adverse
        )
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::useless_vec
)]
mod tests {
    use super::*;

    fn price(value: f64) -> Price {
        Price::new(value).unwrap()
    }

    mod spread_metrics_tests {
        use super::*;

        #[test]
        fn basic_spread_calculation() {
            // bid=99, ask=101, mid=100 → spread = 2/100 × 10000 = 200 bps
            let spread = SpreadMetrics::calculate(price(99.0), price(101.0)).unwrap();

            assert_eq!(spread.bid_price().get(), Decimal::new(99, 0));
            assert_eq!(spread.ask_price().get(), Decimal::new(101, 0));
            assert_eq!(spread.mid_price().get(), Decimal::new(100, 0));
            assert_eq!(spread.spread_bps(), Decimal::new(200, 0));
            assert_eq!(spread.half_spread_bps(), Decimal::new(100, 0));
        }

        #[test]
        fn zero_spread_when_locked() {
            // bid=100, ask=100 → 0 bps
            let spread = SpreadMetrics::calculate(price(100.0), price(100.0)).unwrap();

            assert!(spread.is_locked());
            assert_eq!(spread.spread_bps(), Decimal::ZERO);
            assert_eq!(spread.half_spread_bps(), Decimal::ZERO);
        }

        #[test]
        fn returns_none_for_crossed() {
            // bid > ask is invalid
            let result = SpreadMetrics::calculate(price(101.0), price(99.0));
            assert!(result.is_none());
        }

        #[test]
        fn returns_none_for_zero_prices() {
            let zero = Price::from_decimal(Decimal::ZERO).unwrap();
            let result = SpreadMetrics::calculate(zero, zero);
            assert!(result.is_none());
        }

        #[test]
        fn small_spread() {
            // bid=99.99, ask=100.01 → spread = 0.02/100 × 10000 = 2 bps
            let spread = SpreadMetrics::calculate(price(99.99), price(100.01)).unwrap();

            assert_eq!(spread.spread_bps(), Decimal::new(2, 0));
            assert_eq!(spread.half_spread_bps(), Decimal::ONE);
        }

        #[test]
        fn spread_percentage() {
            let spread = SpreadMetrics::calculate(price(99.0), price(101.0)).unwrap();
            // 200 bps = 2%
            assert_eq!(spread.spread_percentage(), Decimal::TWO);
        }

        #[test]
        fn is_not_crossed_for_valid_spread() {
            let spread = SpreadMetrics::calculate(price(99.0), price(101.0)).unwrap();
            assert!(!spread.is_crossed());
        }

        #[test]
        fn display_format() {
            let spread = SpreadMetrics::calculate(price(99.0), price(101.0)).unwrap();
            let display = spread.to_string();
            assert!(display.contains("200"));
            assert!(display.contains("100"));
        }

        #[test]
        fn serde_roundtrip() {
            let spread = SpreadMetrics::calculate(price(99.0), price(101.0)).unwrap();
            let json = serde_json::to_string(&spread).unwrap();
            let deserialized: SpreadMetrics = serde_json::from_str(&json).unwrap();
            assert_eq!(spread, deserialized);
        }
    }

    mod effective_spread_tests {
        use super::*;

        #[test]
        fn buy_at_mid_zero_effective() {
            // Execute at mid → 0 effective spread
            let effective =
                EffectiveSpread::calculate(price(100.0), price(100.0), OrderSide::Buy).unwrap();

            assert_eq!(effective.effective_spread_bps(), Decimal::ZERO);
        }

        #[test]
        fn buy_above_mid_positive() {
            // Buy at 100.5 when mid is 100 → 2 × 0.5/100 × 10000 = 100 bps
            let effective =
                EffectiveSpread::calculate(price(100.5), price(100.0), OrderSide::Buy).unwrap();

            assert_eq!(effective.effective_spread_bps(), Decimal::new(100, 0));
        }

        #[test]
        fn sell_below_mid_positive() {
            // Sell at 99.5 when mid is 100 → 2 × 0.5/100 × 10000 = 100 bps
            let effective =
                EffectiveSpread::calculate(price(99.5), price(100.0), OrderSide::Sell).unwrap();

            assert_eq!(effective.effective_spread_bps(), Decimal::new(100, 0));
        }

        #[test]
        fn returns_none_for_zero_mid() {
            let zero = Price::from_decimal(Decimal::ZERO).unwrap();
            let result = EffectiveSpread::calculate(price(100.0), zero, OrderSide::Buy);
            assert!(result.is_none());
        }

        #[test]
        fn better_than_quoted_comparison() {
            // 50 bps effective (half of 100)
            let effective =
                EffectiveSpread::calculate(price(100.25), price(100.0), OrderSide::Buy).unwrap();

            // Better than 100 bps quoted half spread
            assert!(effective.is_better_than_quoted(Decimal::new(100, 0)));
            // Worse than 10 bps quoted half spread
            assert!(!effective.is_better_than_quoted(Decimal::new(10, 0)));
        }

        #[test]
        fn effective_spread_percentage() {
            let effective =
                EffectiveSpread::calculate(price(100.5), price(100.0), OrderSide::Buy).unwrap();

            // 100 bps = 1%
            assert_eq!(effective.effective_spread_percentage(), Decimal::ONE);
        }

        #[test]
        fn display_format() {
            let effective =
                EffectiveSpread::calculate(price(100.5), price(100.0), OrderSide::Buy).unwrap();

            let display = effective.to_string();
            assert!(display.contains("100"));
            assert!(display.contains("BUY"));
        }

        #[test]
        fn serde_roundtrip() {
            let effective =
                EffectiveSpread::calculate(price(100.5), price(100.0), OrderSide::Buy).unwrap();

            let json = serde_json::to_string(&effective).unwrap();
            let deserialized: EffectiveSpread = serde_json::from_str(&json).unwrap();
            assert_eq!(effective, deserialized);
        }
    }

    mod realized_spread_tests {
        use super::*;

        fn make_effective(exec: f64, mid: f64, side: OrderSide) -> EffectiveSpread {
            EffectiveSpread::calculate(price(exec), price(mid), side).unwrap()
        }

        #[test]
        fn no_movement_equals_effective() {
            let effective = make_effective(100.5, 100.0, OrderSide::Buy);
            let realized =
                RealizedSpread::calculate(&effective, price(100.0), price(100.0)).unwrap();

            assert_eq!(
                realized.realized_spread_bps(),
                effective.effective_spread_bps()
            );
            assert_eq!(realized.price_movement_bps(), Decimal::ZERO);
            assert!(!realized.has_adverse_selection());
        }

        #[test]
        fn adverse_selection_buy() {
            // Buy at 100.5, mid drops to 99.5 → adverse selection
            let effective = make_effective(100.5, 100.0, OrderSide::Buy);
            let realized =
                RealizedSpread::calculate(&effective, price(100.0), price(99.5)).unwrap();

            // Price movement: (99.5 - 100) / 100 × 10000 = -50 bps
            assert_eq!(realized.price_movement_bps(), Decimal::new(-50, 0));
            // Realized = 100 - (-50) = 150 bps
            assert_eq!(realized.realized_spread_bps(), Decimal::new(150, 0));
            assert!(realized.has_adverse_selection());
            assert_eq!(realized.adverse_selection_bps(), Decimal::new(50, 0));
        }

        #[test]
        fn favorable_movement_buy() {
            // Buy at 100.5, mid rises to 100.5 → favorable
            let effective = make_effective(100.5, 100.0, OrderSide::Buy);
            let realized =
                RealizedSpread::calculate(&effective, price(100.0), price(100.5)).unwrap();

            // Price movement: (100.5 - 100) / 100 × 10000 = 50 bps
            assert_eq!(realized.price_movement_bps(), Decimal::new(50, 0));
            // Realized = 100 - 50 = 50 bps
            assert_eq!(realized.realized_spread_bps(), Decimal::new(50, 0));
            assert!(!realized.has_adverse_selection());
        }

        #[test]
        fn adverse_selection_sell() {
            // Sell at 99.5, mid rises to 100.5 → adverse selection
            let effective = make_effective(99.5, 100.0, OrderSide::Sell);
            let realized =
                RealizedSpread::calculate(&effective, price(100.0), price(100.5)).unwrap();

            // Price movement: (100.5 - 100) / 100 × 10000 = 50 bps
            assert_eq!(realized.price_movement_bps(), Decimal::new(50, 0));
            // Realized = 100 + 50 = 150 bps (for sell, price rise is adverse)
            assert_eq!(realized.realized_spread_bps(), Decimal::new(150, 0));
            assert!(realized.has_adverse_selection());
        }

        #[test]
        fn favorable_movement_sell() {
            // Sell at 99.5, mid drops to 99.5 → favorable
            let effective = make_effective(99.5, 100.0, OrderSide::Sell);
            let realized =
                RealizedSpread::calculate(&effective, price(100.0), price(99.5)).unwrap();

            // Price movement: (99.5 - 100) / 100 × 10000 = -50 bps
            assert_eq!(realized.price_movement_bps(), Decimal::new(-50, 0));
            // Realized = 100 + (-50) = 50 bps
            assert_eq!(realized.realized_spread_bps(), Decimal::new(50, 0));
            assert!(!realized.has_adverse_selection());
        }

        #[test]
        fn returns_none_for_zero_mid_before() {
            let effective = make_effective(100.5, 100.0, OrderSide::Buy);
            let zero = Price::from_decimal(Decimal::ZERO).unwrap();
            let result = RealizedSpread::calculate(&effective, zero, price(100.0));
            assert!(result.is_none());
        }

        #[test]
        fn display_format() {
            let effective = make_effective(100.5, 100.0, OrderSide::Buy);
            let realized =
                RealizedSpread::calculate(&effective, price(100.0), price(99.5)).unwrap();

            let display = realized.to_string();
            assert!(display.contains("150"));
            assert!(display.contains("-50"));
            assert!(display.contains("adverse"));
        }

        #[test]
        fn serde_roundtrip() {
            let effective = make_effective(100.5, 100.0, OrderSide::Buy);
            let realized =
                RealizedSpread::calculate(&effective, price(100.0), price(99.5)).unwrap();

            let json = serde_json::to_string(&realized).unwrap();
            let deserialized: RealizedSpread = serde_json::from_str(&json).unwrap();
            assert_eq!(realized, deserialized);
        }
    }

    mod realistic_scenarios {
        use super::*;

        #[test]
        fn btc_market_spread() {
            // BTC market: bid=49,950, ask=50,050 → 20 bps spread
            // spread = 100 / 50000 × 10000 = 20 bps
            let spread = SpreadMetrics::calculate(price(49950.0), price(50050.0)).unwrap();

            assert_eq!(spread.mid_price().get(), Decimal::new(50000, 0));
            assert_eq!(spread.spread_bps(), Decimal::new(20, 0));
        }

        #[test]
        fn tight_fx_spread() {
            // EUR/USD: bid=1.0850, ask=1.0852 → ~1.8 bps
            let spread = SpreadMetrics::calculate(price(1.0850), price(1.0852)).unwrap();

            // (0.0002 / 1.0851) × 10000 ≈ 1.84 bps
            assert!(spread.spread_bps() > Decimal::ONE);
            assert!(spread.spread_bps() < Decimal::TWO);
        }

        #[test]
        fn full_trade_lifecycle() {
            // 1. Pre-trade: Check quoted spread
            let quoted = SpreadMetrics::calculate(price(99.0), price(101.0)).unwrap();
            assert_eq!(quoted.spread_bps(), Decimal::new(200, 0));

            // 2. Execute buy at 100.5
            let effective =
                EffectiveSpread::calculate(price(100.5), quoted.mid_price(), OrderSide::Buy)
                    .unwrap();
            assert_eq!(effective.effective_spread_bps(), Decimal::new(100, 0));

            // 3. After 5 minutes, price dropped to 99.0 (adverse selection)
            let realized =
                RealizedSpread::calculate(&effective, quoted.mid_price(), price(99.0)).unwrap();

            // Price movement: -100 bps, realized = 100 - (-100) = 200 bps
            assert_eq!(realized.price_movement_bps(), Decimal::new(-100, 0));
            assert_eq!(realized.realized_spread_bps(), Decimal::new(200, 0));
            assert!(realized.has_adverse_selection());
        }
    }
}
