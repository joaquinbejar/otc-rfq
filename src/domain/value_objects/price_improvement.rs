//! # Price Improvement Calculation
//!
//! Value objects for calculating price improvement of OTC/RFQ quotes
//! against CLOB reference prices.
//!
//! Price improvement is expressed in basis points (bps), where:
//! - Positive values indicate the quote is better than the reference
//! - Negative values indicate the quote is worse than the reference
//!
//! # Examples
//!
//! ```
//! use otc_rfq::domain::value_objects::price_improvement::{PriceImprovement, ImprovementSource};
//! use otc_rfq::domain::value_objects::{Price, OrderSide};
//! use rust_decimal::Decimal;
//!
//! let quote = Price::new(49950.0).unwrap();
//! let reference = Price::new(50000.0).unwrap();
//!
//! // For a buy order, lower price = positive improvement
//! let improvement = PriceImprovement::calculate(
//!     quote,
//!     reference,
//!     ImprovementSource::ClobMid,
//!     OrderSide::Buy,
//! ).unwrap();
//!
//! assert!(improvement.is_positive());
//! assert_eq!(improvement.improvement_bps(), Decimal::new(10, 0)); // +10 bps
//! ```

use crate::domain::value_objects::enums::OrderSide;
use crate::domain::value_objects::price::Price;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Basis points multiplier (1 bp = 0.01% = 0.0001).
/// Equivalent to `Decimal::new(10_000, 0)` but using `from_parts` for const context.
const BPS_MULTIPLIER: Decimal = Decimal::from_parts(10_000, 0, 0, false, 0);

/// Source of the reference price used for improvement calculation.
///
/// Different sources have different characteristics:
/// - **ClobMid**: Mid-price from the central limit order book (most common)
/// - **ClobBestBid**: Best bid price (for sell-side comparison)
/// - **ClobBestAsk**: Best ask price (for buy-side comparison)
/// - **LastTrade**: Price of the most recent trade
///
/// # Examples
///
/// ```
/// use otc_rfq::domain::value_objects::price_improvement::ImprovementSource;
///
/// let source = ImprovementSource::ClobMid;
/// assert_eq!(source.to_string(), "CLOB_MID");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ImprovementSource {
    /// Central Limit Order Book mid-price.
    #[default]
    ClobMid,
    /// Best bid price from the order book.
    ClobBestBid,
    /// Best ask price from the order book.
    ClobBestAsk,
    /// Price of the most recent trade.
    LastTrade,
}

impl ImprovementSource {
    /// Returns all available improvement sources.
    #[must_use]
    pub const fn all() -> [Self; 4] {
        [
            Self::ClobMid,
            Self::ClobBestBid,
            Self::ClobBestAsk,
            Self::LastTrade,
        ]
    }
}

impl fmt::Display for ImprovementSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ClobMid => write!(f, "CLOB_MID"),
            Self::ClobBestBid => write!(f, "CLOB_BEST_BID"),
            Self::ClobBestAsk => write!(f, "CLOB_BEST_ASK"),
            Self::LastTrade => write!(f, "LAST_TRADE"),
        }
    }
}

/// Price improvement of a quote compared to a reference price.
///
/// Improvement is calculated as the percentage difference in basis points:
/// - **Buy side**: `((reference - quote) / reference) × 10,000`
/// - **Sell side**: `((quote - reference) / reference) × 10,000`
///
/// Positive values indicate the quote is better than the reference.
/// Negative values indicate the quote is worse than the reference.
///
/// # Examples
///
/// ```
/// use otc_rfq::domain::value_objects::price_improvement::{PriceImprovement, ImprovementSource};
/// use otc_rfq::domain::value_objects::{Price, OrderSide};
///
/// // Buy at 49,950 vs reference 50,000 = +10 bps improvement
/// let improvement = PriceImprovement::calculate(
///     Price::new(49950.0).unwrap(),
///     Price::new(50000.0).unwrap(),
///     ImprovementSource::ClobMid,
///     OrderSide::Buy,
/// ).unwrap();
///
/// assert!(improvement.is_positive());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PriceImprovement {
    /// The quoted price being evaluated.
    quote_price: Price,
    /// The reference price for comparison.
    reference_price: Price,
    /// Source of the reference price.
    source: ImprovementSource,
    /// Order side used for calculation.
    side: OrderSide,
    /// Improvement in basis points (positive = better, negative = worse).
    improvement_bps: Decimal,
}

impl PriceImprovement {
    /// Calculates price improvement for a quote vs a reference price.
    ///
    /// # Arguments
    ///
    /// * `quote_price` - The OTC/RFQ quote price
    /// * `reference_price` - The CLOB reference price
    /// * `source` - Source of the reference price
    /// * `side` - Order side (Buy or Sell)
    ///
    /// # Returns
    ///
    /// `Some(PriceImprovement)` if calculation succeeds, `None` if:
    /// - Reference price is zero (division by zero protection)
    /// - Arithmetic overflow occurs during calculation (extremely unlikely with real prices)
    ///
    /// # Formula
    ///
    /// - Buy side: `improvement = ((reference - quote) / reference) × 10,000`
    /// - Sell side: `improvement = ((quote - reference) / reference) × 10,000`
    ///
    /// # Examples
    ///
    /// ```
    /// use otc_rfq::domain::value_objects::price_improvement::{PriceImprovement, ImprovementSource};
    /// use otc_rfq::domain::value_objects::{Price, OrderSide};
    ///
    /// // Buy: lower quote = positive improvement
    /// let buy_improvement = PriceImprovement::calculate(
    ///     Price::new(99.0).unwrap(),
    ///     Price::new(100.0).unwrap(),
    ///     ImprovementSource::ClobMid,
    ///     OrderSide::Buy,
    /// ).unwrap();
    /// assert!(buy_improvement.is_positive());
    ///
    /// // Sell: higher quote = positive improvement
    /// let sell_improvement = PriceImprovement::calculate(
    ///     Price::new(101.0).unwrap(),
    ///     Price::new(100.0).unwrap(),
    ///     ImprovementSource::ClobMid,
    ///     OrderSide::Sell,
    /// ).unwrap();
    /// assert!(sell_improvement.is_positive());
    /// ```
    #[must_use]
    pub fn calculate(
        quote_price: Price,
        reference_price: Price,
        source: ImprovementSource,
        side: OrderSide,
    ) -> Option<Self> {
        let ref_value = reference_price.get();

        // Cannot calculate improvement against zero reference
        if ref_value.is_zero() {
            return None;
        }

        let quote_value = quote_price.get();

        // Calculate improvement based on side
        let improvement_bps = match side {
            // Buy: lower quote is better → (reference - quote) / reference
            OrderSide::Buy => {
                let diff = ref_value.checked_sub(quote_value)?;
                let ratio = diff.checked_div(ref_value)?;
                ratio.checked_mul(BPS_MULTIPLIER)?
            }
            // Sell: higher quote is better → (quote - reference) / reference
            OrderSide::Sell => {
                let diff = quote_value.checked_sub(ref_value)?;
                let ratio = diff.checked_div(ref_value)?;
                ratio.checked_mul(BPS_MULTIPLIER)?
            }
        };

        Some(Self {
            quote_price,
            reference_price,
            source,
            side,
            improvement_bps,
        })
    }

    /// Returns the quote price.
    #[inline]
    #[must_use]
    pub const fn quote_price(&self) -> Price {
        self.quote_price
    }

    /// Returns the reference price.
    #[inline]
    #[must_use]
    pub const fn reference_price(&self) -> Price {
        self.reference_price
    }

    /// Returns the source of the reference price.
    #[inline]
    #[must_use]
    pub const fn source(&self) -> ImprovementSource {
        self.source
    }

    /// Returns the order side used for calculation.
    #[inline]
    #[must_use]
    pub const fn side(&self) -> OrderSide {
        self.side
    }

    /// Returns the improvement in basis points.
    ///
    /// Positive values indicate the quote is better than the reference.
    /// Negative values indicate the quote is worse than the reference.
    #[inline]
    #[must_use]
    pub const fn improvement_bps(&self) -> Decimal {
        self.improvement_bps
    }

    /// Returns true if the improvement is positive (quote is better than reference).
    #[inline]
    #[must_use]
    pub fn is_positive(&self) -> bool {
        self.improvement_bps.is_sign_positive() && !self.improvement_bps.is_zero()
    }

    /// Returns true if the improvement is negative (quote is worse than reference).
    #[inline]
    #[must_use]
    pub fn is_negative(&self) -> bool {
        self.improvement_bps.is_sign_negative()
    }

    /// Returns true if there is no improvement (quote equals reference).
    #[inline]
    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.improvement_bps.is_zero()
    }

    /// Returns the improvement as a percentage (bps / 100).
    ///
    /// # Examples
    ///
    /// ```
    /// use otc_rfq::domain::value_objects::price_improvement::{PriceImprovement, ImprovementSource};
    /// use otc_rfq::domain::value_objects::{Price, OrderSide};
    /// use rust_decimal::Decimal;
    ///
    /// let improvement = PriceImprovement::calculate(
    ///     Price::new(99.0).unwrap(),
    ///     Price::new(100.0).unwrap(),
    ///     ImprovementSource::ClobMid,
    ///     OrderSide::Buy,
    /// ).unwrap();
    ///
    /// // 100 bps = 1%
    /// assert_eq!(improvement.as_percentage(), Decimal::ONE);
    /// ```
    #[must_use]
    pub fn as_percentage(&self) -> Decimal {
        self.improvement_bps / Decimal::ONE_HUNDRED
    }

    /// Returns the absolute improvement in basis points.
    #[must_use]
    pub fn abs_improvement_bps(&self) -> Decimal {
        self.improvement_bps.abs()
    }
}

impl fmt::Display for PriceImprovement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sign = if self.is_positive() { "+" } else { "" };
        write!(
            f,
            "PriceImprovement({}{} bps vs {})",
            sign, self.improvement_bps, self.source
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

    mod improvement_source {
        use super::*;

        #[test]
        fn display() {
            assert_eq!(ImprovementSource::ClobMid.to_string(), "CLOB_MID");
            assert_eq!(ImprovementSource::ClobBestBid.to_string(), "CLOB_BEST_BID");
            assert_eq!(ImprovementSource::ClobBestAsk.to_string(), "CLOB_BEST_ASK");
            assert_eq!(ImprovementSource::LastTrade.to_string(), "LAST_TRADE");
        }

        #[test]
        fn default_is_clob_mid() {
            assert_eq!(ImprovementSource::default(), ImprovementSource::ClobMid);
        }

        #[test]
        fn all_sources() {
            let all = ImprovementSource::all();
            assert_eq!(all.len(), 4);
            assert!(all.contains(&ImprovementSource::ClobMid));
            assert!(all.contains(&ImprovementSource::ClobBestBid));
            assert!(all.contains(&ImprovementSource::ClobBestAsk));
            assert!(all.contains(&ImprovementSource::LastTrade));
        }

        #[test]
        fn serde_roundtrip() {
            for source in ImprovementSource::all() {
                let json = serde_json::to_string(&source).unwrap();
                let deserialized: ImprovementSource = serde_json::from_str(&json).unwrap();
                assert_eq!(source, deserialized);
            }
        }
    }

    mod price_improvement_calculation {
        use super::*;

        #[test]
        fn buy_side_positive_improvement() {
            // Quote 99, Reference 100 → +100 bps improvement (1% better)
            let improvement = PriceImprovement::calculate(
                price(99.0),
                price(100.0),
                ImprovementSource::ClobMid,
                OrderSide::Buy,
            )
            .unwrap();

            assert!(improvement.is_positive());
            assert!(!improvement.is_negative());
            assert!(!improvement.is_zero());
            assert_eq!(improvement.improvement_bps(), Decimal::ONE_HUNDRED);
            assert_eq!(improvement.as_percentage(), Decimal::ONE);
        }

        #[test]
        fn buy_side_negative_improvement() {
            // Quote 101, Reference 100 → -100 bps improvement (1% worse)
            let improvement = PriceImprovement::calculate(
                price(101.0),
                price(100.0),
                ImprovementSource::ClobMid,
                OrderSide::Buy,
            )
            .unwrap();

            assert!(!improvement.is_positive());
            assert!(improvement.is_negative());
            assert!(!improvement.is_zero());
            assert_eq!(improvement.improvement_bps(), Decimal::new(-100, 0));
        }

        #[test]
        fn sell_side_positive_improvement() {
            // Quote 101, Reference 100 → +100 bps improvement (1% better)
            let improvement = PriceImprovement::calculate(
                price(101.0),
                price(100.0),
                ImprovementSource::ClobMid,
                OrderSide::Sell,
            )
            .unwrap();

            assert!(improvement.is_positive());
            assert!(!improvement.is_negative());
            assert_eq!(improvement.improvement_bps(), Decimal::ONE_HUNDRED);
        }

        #[test]
        fn sell_side_negative_improvement() {
            // Quote 99, Reference 100 → -100 bps improvement (1% worse)
            let improvement = PriceImprovement::calculate(
                price(99.0),
                price(100.0),
                ImprovementSource::ClobMid,
                OrderSide::Sell,
            )
            .unwrap();

            assert!(!improvement.is_positive());
            assert!(improvement.is_negative());
            assert_eq!(improvement.improvement_bps(), Decimal::new(-100, 0));
        }

        #[test]
        fn zero_improvement_when_equal() {
            let improvement = PriceImprovement::calculate(
                price(100.0),
                price(100.0),
                ImprovementSource::ClobMid,
                OrderSide::Buy,
            )
            .unwrap();

            assert!(!improvement.is_positive());
            assert!(!improvement.is_negative());
            assert!(improvement.is_zero());
            assert_eq!(improvement.improvement_bps(), Decimal::ZERO);
        }

        #[test]
        fn returns_none_for_zero_reference() {
            let result = PriceImprovement::calculate(
                price(100.0),
                Price::from_decimal(Decimal::ZERO).unwrap(),
                ImprovementSource::ClobMid,
                OrderSide::Buy,
            );

            assert!(result.is_none());
        }

        #[test]
        fn handles_small_differences() {
            // Quote 99.99, Reference 100.00 → +1 bp
            let improvement = PriceImprovement::calculate(
                price(99.99),
                price(100.0),
                ImprovementSource::ClobMid,
                OrderSide::Buy,
            )
            .unwrap();

            assert!(improvement.is_positive());
            // 0.01 / 100 * 10000 = 1 bp
            assert_eq!(improvement.improvement_bps(), Decimal::ONE);
        }

        #[test]
        fn all_sources_work() {
            for source in ImprovementSource::all() {
                let improvement =
                    PriceImprovement::calculate(price(99.0), price(100.0), source, OrderSide::Buy)
                        .unwrap();

                assert_eq!(improvement.source(), source);
                assert!(improvement.is_positive());
            }
        }

        #[test]
        fn accessors() {
            let improvement = PriceImprovement::calculate(
                price(99.0),
                price(100.0),
                ImprovementSource::ClobBestAsk,
                OrderSide::Buy,
            )
            .unwrap();

            assert_eq!(improvement.quote_price().get(), Decimal::new(99, 0));
            assert_eq!(improvement.reference_price().get(), Decimal::new(100, 0));
            assert_eq!(improvement.source(), ImprovementSource::ClobBestAsk);
            assert_eq!(improvement.side(), OrderSide::Buy);
            assert_eq!(improvement.abs_improvement_bps(), Decimal::ONE_HUNDRED);
        }
    }

    mod display_and_serde {
        use super::*;

        #[test]
        fn display_positive() {
            let improvement = PriceImprovement::calculate(
                price(99.0),
                price(100.0),
                ImprovementSource::ClobMid,
                OrderSide::Buy,
            )
            .unwrap();

            let display = improvement.to_string();
            assert!(display.contains("+100"));
            assert!(display.contains("CLOB_MID"));
        }

        #[test]
        fn display_negative() {
            let improvement = PriceImprovement::calculate(
                price(101.0),
                price(100.0),
                ImprovementSource::ClobMid,
                OrderSide::Buy,
            )
            .unwrap();

            let display = improvement.to_string();
            assert!(display.contains("-100"));
        }

        #[test]
        fn display_zero() {
            let improvement = PriceImprovement::calculate(
                price(100.0),
                price(100.0),
                ImprovementSource::ClobMid,
                OrderSide::Buy,
            )
            .unwrap();

            let display = improvement.to_string();
            assert!(display.contains("0"));
        }

        #[test]
        fn serde_roundtrip() {
            let improvement = PriceImprovement::calculate(
                price(99.5),
                price(100.0),
                ImprovementSource::LastTrade,
                OrderSide::Sell,
            )
            .unwrap();

            let json = serde_json::to_string(&improvement).unwrap();
            let deserialized: PriceImprovement = serde_json::from_str(&json).unwrap();

            assert_eq!(improvement, deserialized);
        }
    }

    mod realistic_scenarios {
        use super::*;

        #[test]
        fn btc_buy_with_price_improvement() {
            // CLOB mid: 50,000 USD
            // OTC quote: 49,950 USD (50 USD better)
            // Improvement: 50/50000 * 10000 = 10 bps
            let improvement = PriceImprovement::calculate(
                price(49950.0),
                price(50000.0),
                ImprovementSource::ClobMid,
                OrderSide::Buy,
            )
            .unwrap();

            assert!(improvement.is_positive());
            assert_eq!(improvement.improvement_bps(), Decimal::new(10, 0));
        }

        #[test]
        fn eth_sell_worse_than_clob() {
            // CLOB mid: 3,000 USD
            // OTC quote: 2,985 USD (15 USD worse)
            // Improvement: -15/3000 * 10000 = -50 bps
            let improvement = PriceImprovement::calculate(
                price(2985.0),
                price(3000.0),
                ImprovementSource::ClobMid,
                OrderSide::Sell,
            )
            .unwrap();

            assert!(improvement.is_negative());
            assert_eq!(improvement.improvement_bps(), Decimal::new(-50, 0));
        }

        #[test]
        fn compare_vs_best_bid_for_sell() {
            // Best bid: 100.00
            // OTC quote: 100.05 (5 bps better than best bid)
            let improvement = PriceImprovement::calculate(
                price(100.05),
                price(100.0),
                ImprovementSource::ClobBestBid,
                OrderSide::Sell,
            )
            .unwrap();

            assert!(improvement.is_positive());
            assert_eq!(improvement.improvement_bps(), Decimal::new(5, 0));
        }

        #[test]
        fn compare_vs_best_ask_for_buy() {
            // Best ask: 100.00
            // OTC quote: 99.95 (5 bps better than best ask)
            let improvement = PriceImprovement::calculate(
                price(99.95),
                price(100.0),
                ImprovementSource::ClobBestAsk,
                OrderSide::Buy,
            )
            .unwrap();

            assert!(improvement.is_positive());
            assert_eq!(improvement.improvement_bps(), Decimal::new(5, 0));
        }
    }
}
