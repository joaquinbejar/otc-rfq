//! # Package Quote Ranking
//!
//! Strategies for ranking package quotes for multi-leg strategies.
//!
//! This module provides ranking strategies specifically designed for
//! package quotes, where the net price determines the ranking:
//! - For debit strategies (buyer pays): lower net price is better
//! - For credit strategies (buyer receives): higher net price (more credit) is better

use crate::domain::entities::package_quote::PackageQuote;
use crate::domain::value_objects::OrderSide;
use rust_decimal::prelude::ToPrimitive;
use serde::{Deserialize, Serialize};
use std::fmt;

/// A package quote with its ranking information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RankedPackageQuote {
    /// The package quote being ranked.
    pub quote: PackageQuote,
    /// The rank (1 = best).
    pub rank: usize,
    /// The score used for ranking (higher = better).
    pub score: f64,
}

impl RankedPackageQuote {
    /// Creates a new ranked package quote.
    #[must_use]
    pub fn new(quote: PackageQuote, rank: usize, score: f64) -> Self {
        Self { quote, rank, score }
    }

    /// Returns true if this quote is the best (rank 1).
    #[must_use]
    pub fn is_best(&self) -> bool {
        self.rank == 1
    }
}

impl fmt::Display for RankedPackageQuote {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "RankedPackageQuote(#{} score={:.4} net_price={} venue={})",
            self.rank,
            self.score,
            self.quote.net_price(),
            self.quote.venue_id()
        )
    }
}

/// Trait for package quote ranking strategies.
///
/// Implementations define how package quotes are scored and ranked based on
/// different criteria such as net price or composite scores.
pub trait PackageRankingStrategy: Send + Sync + fmt::Debug {
    /// Ranks the given package quotes for the specified order side.
    ///
    /// # Arguments
    ///
    /// * `quotes` - The package quotes to rank
    /// * `side` - The order side (Buy or Sell) - determines debit/credit interpretation
    ///
    /// # Returns
    ///
    /// A vector of ranked package quotes sorted by rank (best first).
    fn rank(&self, quotes: &[PackageQuote], side: OrderSide) -> Vec<RankedPackageQuote>;

    /// Returns the name of this ranking strategy.
    fn name(&self) -> &'static str;
}

/// Best net price ranking strategy for package quotes.
///
/// Ranks package quotes by net price:
/// - For Buy orders (debit strategies): lower net price (less cost) is better
/// - For Sell orders (credit strategies): higher net price (more credit) is better
///
/// # Net Price Convention
///
/// - Positive net price = debit (buyer pays)
/// - Negative net price = credit (buyer receives)
#[derive(Debug, Clone, Default)]
pub struct BestNetPriceStrategy;

impl BestNetPriceStrategy {
    /// Creates a new best net price strategy.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl PackageRankingStrategy for BestNetPriceStrategy {
    fn rank(&self, quotes: &[PackageQuote], _side: OrderSide) -> Vec<RankedPackageQuote> {
        if quotes.is_empty() {
            return Vec::new();
        }

        // Score quotes based on net price
        // Convention: positive net_price = debit (buyer pays), negative = credit (buyer receives)
        // For both sides, we want to minimize cost / maximize credit, so we negate net_price
        let mut scored: Vec<(usize, f64)> = quotes
            .iter()
            .enumerate()
            .filter_map(|(i, q)| {
                // Filter out quotes where net_price fails to convert to f64
                // (e.g., extreme Decimal values). This prevents silent 0.0 scores
                // that would incorrectly rank as best.
                q.net_price().to_f64().map(|net_price| {
                    // Negate so that lower net prices (less cost or more credit) get higher scores
                    let score = -net_price;
                    (i, score)
                })
            })
            .collect();

        // Sort by score descending (higher score = better)
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Create ranked quotes
        scored
            .into_iter()
            .enumerate()
            .filter_map(|(rank, (idx, score))| {
                quotes
                    .get(idx)
                    .map(|q| RankedPackageQuote::new(q.clone(), rank + 1, score))
            })
            .collect()
    }

    fn name(&self) -> &'static str {
        "BestNetPrice"
    }
}

/// Ranks package quotes considering both net price and venue reliability.
///
/// This strategy combines net price with venue-specific factors like
/// historical fill rates and response times.
#[derive(Debug, Clone)]
pub struct WeightedPackageStrategy {
    /// Weight for net price (0.0 to 1.0).
    price_weight: f64,
    /// Weight for venue reliability (0.0 to 1.0).
    reliability_weight: f64,
}

impl WeightedPackageStrategy {
    /// Creates a new weighted strategy with the specified weights.
    ///
    /// # Arguments
    ///
    /// * `price_weight` - Weight for net price factor
    /// * `reliability_weight` - Weight for venue reliability factor
    ///
    /// # Panics
    ///
    /// Panics if weights are negative or sum to zero.
    #[must_use]
    pub fn new(price_weight: f64, reliability_weight: f64) -> Self {
        assert!(price_weight >= 0.0, "price_weight must be non-negative");
        assert!(
            reliability_weight >= 0.0,
            "reliability_weight must be non-negative"
        );
        assert!(
            price_weight + reliability_weight > 0.0,
            "weights must sum to positive value"
        );

        Self {
            price_weight,
            reliability_weight,
        }
    }

    /// Creates a price-only strategy.
    #[must_use]
    pub fn price_only() -> Self {
        Self::new(1.0, 0.0)
    }

    /// Creates a balanced strategy with equal weights.
    #[must_use]
    pub fn balanced() -> Self {
        Self::new(0.5, 0.5)
    }
}

impl Default for WeightedPackageStrategy {
    fn default() -> Self {
        Self::price_only()
    }
}

impl PackageRankingStrategy for WeightedPackageStrategy {
    fn rank(&self, quotes: &[PackageQuote], _side: OrderSide) -> Vec<RankedPackageQuote> {
        if quotes.is_empty() {
            return Vec::new();
        }

        // For now, use price-based scoring only
        // Venue reliability would require additional context (venue metrics)
        let total_weight = self.price_weight + self.reliability_weight;

        let mut scored: Vec<(usize, f64)> = quotes
            .iter()
            .enumerate()
            .filter_map(|(i, q)| {
                // Filter out quotes where net_price fails to convert to f64
                // (e.g., extreme Decimal values). This prevents silent 0.0 scores
                // that would incorrectly rank as best.
                q.net_price().to_f64().map(|net_price| {
                    // Normalize price score - lower net_price is better for both sides
                    // (less cost for debit, more credit for credit strategies)
                    let price_score = -net_price;

                    // For now, assume all venues have equal reliability (1.0)
                    // In a full implementation, this would come from venue metrics
                    let reliability_score = 1.0;

                    let weighted_score = (self.price_weight * price_score
                        + self.reliability_weight * reliability_score)
                        / total_weight;

                    (i, weighted_score)
                })
            })
            .collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        scored
            .into_iter()
            .enumerate()
            .filter_map(|(rank, (idx, score))| {
                quotes
                    .get(idx)
                    .map(|q| RankedPackageQuote::new(q.clone(), rank + 1, score))
            })
            .collect()
    }

    fn name(&self) -> &'static str {
        "WeightedPackage"
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::domain::entities::package_quote::LegPrice;
    use crate::domain::value_objects::strategy::{Strategy, StrategyLeg, StrategyType};
    use crate::domain::value_objects::{
        AssetClass, Instrument, Price, Quantity, RfqId, Symbol, Timestamp, VenueId,
    };
    use rust_decimal::Decimal;

    fn test_instrument() -> Instrument {
        Instrument::builder(Symbol::new("BTC/USD").unwrap(), AssetClass::CryptoSpot).build()
    }

    fn test_strategy() -> Strategy {
        let inst = test_instrument();
        Strategy::new(
            StrategyType::Spread,
            vec![
                StrategyLeg::new(inst.clone(), OrderSide::Buy, 1).unwrap(),
                StrategyLeg::new(inst, OrderSide::Sell, 1).unwrap(),
            ],
            "BTC",
            None,
        )
        .unwrap()
    }

    fn create_package_quote(venue_id: &str, net_price: Decimal) -> PackageQuote {
        let inst = test_instrument();
        let leg_prices = vec![
            LegPrice::new(
                inst.clone(),
                OrderSide::Buy,
                Price::new(50000.0).unwrap(),
                Quantity::new(1.0).unwrap(),
            )
            .unwrap(),
            LegPrice::new(
                inst,
                OrderSide::Sell,
                Price::new(50100.0).unwrap(),
                Quantity::new(1.0).unwrap(),
            )
            .unwrap(),
        ];

        PackageQuote::new(
            RfqId::new_v4(),
            VenueId::new(venue_id),
            test_strategy(),
            net_price,
            leg_prices,
            Timestamp::now().add_secs(300),
        )
        .unwrap()
    }

    mod best_net_price_strategy {
        use super::*;

        #[test]
        fn ranks_buy_side_by_lowest_net_price() {
            let strategy = BestNetPriceStrategy::new();

            let quotes = vec![
                create_package_quote("venue-a", Decimal::new(100, 0)), // Net debit 100
                create_package_quote("venue-b", Decimal::new(50, 0)),  // Net debit 50 (best)
                create_package_quote("venue-c", Decimal::new(150, 0)), // Net debit 150
            ];

            let ranked = strategy.rank(&quotes, OrderSide::Buy);

            assert_eq!(ranked.len(), 3);
            let first = ranked.first().expect("should have quotes");
            assert_eq!(first.rank, 1);
            assert_eq!(first.quote.venue_id().as_str(), "venue-b"); // Lowest cost
            assert_eq!(ranked.get(1).unwrap().quote.venue_id().as_str(), "venue-a");
            assert_eq!(ranked.get(2).unwrap().quote.venue_id().as_str(), "venue-c");
        }

        #[test]
        fn ranks_sell_side_by_most_credit() {
            let strategy = BestNetPriceStrategy::new();

            let quotes = vec![
                create_package_quote("venue-a", Decimal::new(-100, 0)), // Net credit 100
                create_package_quote("venue-b", Decimal::new(-150, 0)), // Net credit 150 (best - most credit)
                create_package_quote("venue-c", Decimal::new(-50, 0)),  // Net credit 50
            ];

            let ranked = strategy.rank(&quotes, OrderSide::Sell);

            assert_eq!(ranked.len(), 3);
            let first = ranked.first().expect("should have quotes");
            assert_eq!(first.rank, 1);
            // For sell side, we want the most credit (most negative net_price)
            // -150 < -100 < -50, so venue-b (most credit) should rank first
            assert_eq!(first.quote.venue_id().as_str(), "venue-b");
            assert_eq!(ranked.get(1).unwrap().quote.venue_id().as_str(), "venue-a");
            assert_eq!(ranked.get(2).unwrap().quote.venue_id().as_str(), "venue-c");
        }

        #[test]
        fn handles_empty_quotes() {
            let strategy = BestNetPriceStrategy::new();
            let ranked = strategy.rank(&[], OrderSide::Buy);
            assert!(ranked.is_empty());
        }

        #[test]
        fn handles_single_quote() {
            let strategy = BestNetPriceStrategy::new();
            let quotes = vec![create_package_quote("venue-a", Decimal::new(100, 0))];

            let ranked = strategy.rank(&quotes, OrderSide::Buy);

            assert_eq!(ranked.len(), 1);
            let first = ranked.first().expect("should have one quote");
            assert_eq!(first.rank, 1);
            assert!(first.is_best());
        }

        #[test]
        fn name_returns_correct_value() {
            let strategy = BestNetPriceStrategy::new();
            assert_eq!(strategy.name(), "BestNetPrice");
        }
    }

    mod weighted_package_strategy {
        use super::*;

        #[test]
        fn price_only_behaves_like_best_net_price() {
            let strategy = WeightedPackageStrategy::price_only();

            let quotes = vec![
                create_package_quote("venue-a", Decimal::new(100, 0)),
                create_package_quote("venue-b", Decimal::new(50, 0)),
            ];

            let ranked = strategy.rank(&quotes, OrderSide::Buy);

            let first = ranked.first().expect("should have quotes");
            assert_eq!(first.quote.venue_id().as_str(), "venue-b");
        }

        #[test]
        fn balanced_creates_equal_weights() {
            let strategy = WeightedPackageStrategy::balanced();
            assert_eq!(strategy.price_weight, 0.5);
            assert_eq!(strategy.reliability_weight, 0.5);
        }

        #[test]
        fn name_returns_correct_value() {
            let strategy = WeightedPackageStrategy::default();
            assert_eq!(strategy.name(), "WeightedPackage");
        }

        #[test]
        #[should_panic(expected = "price_weight must be non-negative")]
        fn panics_on_negative_price_weight() {
            let _ = WeightedPackageStrategy::new(-1.0, 1.0);
        }

        #[test]
        #[should_panic(expected = "weights must sum to positive value")]
        fn panics_on_zero_total_weight() {
            let _ = WeightedPackageStrategy::new(0.0, 0.0);
        }
    }

    mod ranked_package_quote {
        use super::*;

        #[test]
        fn is_best_returns_true_for_rank_1() {
            let quote = create_package_quote("venue-a", Decimal::new(100, 0));
            let ranked = RankedPackageQuote::new(quote, 1, 100.0);
            assert!(ranked.is_best());
        }

        #[test]
        fn is_best_returns_false_for_other_ranks() {
            let quote = create_package_quote("venue-a", Decimal::new(100, 0));
            let ranked = RankedPackageQuote::new(quote, 2, 50.0);
            assert!(!ranked.is_best());
        }

        #[test]
        fn display_formats_correctly() {
            let quote = create_package_quote("venue-a", Decimal::new(100, 0));
            let ranked = RankedPackageQuote::new(quote, 1, 100.0);
            let display = ranked.to_string();
            assert!(display.contains("#1"));
            assert!(display.contains("venue-a"));
        }
    }
}
