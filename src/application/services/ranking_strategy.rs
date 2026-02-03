//! # Ranking Strategy
//!
//! Strategies for ranking quotes.
//!
//! This module provides the [`RankingStrategy`] trait and implementations
//! for ranking quotes based on different criteria.

use crate::domain::entities::quote::Quote;
use crate::domain::value_objects::OrderSide;
use rust_decimal::prelude::ToPrimitive;
use serde::{Deserialize, Serialize};
use std::fmt;

/// A quote with its ranking information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RankedQuote {
    /// The quote being ranked.
    pub quote: Quote,
    /// The rank (1 = best).
    pub rank: usize,
    /// The score used for ranking (higher = better).
    pub score: f64,
}

impl RankedQuote {
    /// Creates a new ranked quote.
    #[must_use]
    pub fn new(quote: Quote, rank: usize, score: f64) -> Self {
        Self { quote, rank, score }
    }

    /// Returns true if this quote is the best (rank 1).
    #[must_use]
    pub fn is_best(&self) -> bool {
        self.rank == 1
    }
}

impl fmt::Display for RankedQuote {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "RankedQuote(#{} score={:.4} quote={})",
            self.rank, self.score, self.quote
        )
    }
}

/// Trait for ranking strategies.
///
/// Implementations define how quotes are scored and ranked based on
/// different criteria such as price, venue reputation, or composite scores.
pub trait RankingStrategy: Send + Sync + fmt::Debug {
    /// Ranks the given quotes for the specified order side.
    ///
    /// # Arguments
    ///
    /// * `quotes` - The quotes to rank
    /// * `side` - The order side (Buy or Sell)
    ///
    /// # Returns
    ///
    /// A vector of ranked quotes sorted by rank (best first).
    fn rank(&self, quotes: &[Quote], side: OrderSide) -> Vec<RankedQuote>;

    /// Returns the name of this ranking strategy.
    fn name(&self) -> &'static str;
}

/// Best price ranking strategy.
///
/// Ranks quotes by price:
/// - For Buy orders: lower price is better
/// - For Sell orders: higher price is better
#[derive(Debug, Clone, Default)]
pub struct BestPriceStrategy;

impl BestPriceStrategy {
    /// Creates a new best price strategy.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl RankingStrategy for BestPriceStrategy {
    fn rank(&self, quotes: &[Quote], side: OrderSide) -> Vec<RankedQuote> {
        if quotes.is_empty() {
            return Vec::new();
        }

        // Score quotes based on price
        let mut scored: Vec<(usize, f64)> = quotes
            .iter()
            .enumerate()
            .map(|(i, q)| {
                let price = q.price().get().to_f64().unwrap_or(0.0);
                let score = match side {
                    OrderSide::Buy => -price, // Lower price is better for buying
                    OrderSide::Sell => price, // Higher price is better for selling
                };
                (i, score)
            })
            .collect();

        // Sort by score (descending - higher score is better)
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Create ranked quotes
        scored
            .into_iter()
            .enumerate()
            .filter_map(|(rank, (idx, score))| {
                quotes
                    .get(idx)
                    .map(|q| RankedQuote::new(q.clone(), rank + 1, score))
            })
            .collect()
    }

    fn name(&self) -> &'static str {
        "BestPrice"
    }
}

/// Weighted score ranking strategy.
///
/// Ranks quotes using a weighted combination of factors:
/// - Price (configurable weight)
/// - Quantity available (configurable weight)
/// - Venue reliability (configurable weight)
#[derive(Debug, Clone)]
pub struct WeightedScoreStrategy {
    /// Weight for price factor (0.0 - 1.0).
    pub price_weight: f64,
    /// Weight for quantity factor (0.0 - 1.0).
    pub quantity_weight: f64,
}

impl Default for WeightedScoreStrategy {
    fn default() -> Self {
        Self {
            price_weight: 0.7,
            quantity_weight: 0.3,
        }
    }
}

impl WeightedScoreStrategy {
    /// Creates a new weighted score strategy with custom weights.
    #[must_use]
    pub fn new(price_weight: f64, quantity_weight: f64) -> Self {
        Self {
            price_weight,
            quantity_weight,
        }
    }
}

impl RankingStrategy for WeightedScoreStrategy {
    fn rank(&self, quotes: &[Quote], side: OrderSide) -> Vec<RankedQuote> {
        if quotes.is_empty() {
            return Vec::new();
        }

        // Find min/max for normalization
        let prices: Vec<f64> = quotes
            .iter()
            .map(|q| q.price().get().to_f64().unwrap_or(0.0))
            .collect();
        let quantities: Vec<f64> = quotes
            .iter()
            .map(|q| q.quantity().get().to_f64().unwrap_or(0.0))
            .collect();

        let min_price = prices.iter().cloned().fold(f64::INFINITY, f64::min);
        let max_price = prices.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let min_qty = quantities.iter().cloned().fold(f64::INFINITY, f64::min);
        let max_qty = quantities.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

        let price_range = (max_price - min_price).max(1.0);
        let qty_range = (max_qty - min_qty).max(1.0);

        // Score quotes
        let mut scored: Vec<(usize, f64)> = quotes
            .iter()
            .enumerate()
            .map(|(i, q)| {
                let price = q.price().get().to_f64().unwrap_or(0.0);
                let qty = q.quantity().get().to_f64().unwrap_or(0.0);

                // Normalize price (0-1, where 1 is best)
                let price_score = match side {
                    OrderSide::Buy => (max_price - price) / price_range,
                    OrderSide::Sell => (price - min_price) / price_range,
                };

                // Normalize quantity (0-1, where 1 is best)
                let qty_score = (qty - min_qty) / qty_range;

                let score = self.price_weight * price_score + self.quantity_weight * qty_score;
                (i, score)
            })
            .collect();

        // Sort by score (descending)
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Create ranked quotes
        scored
            .into_iter()
            .enumerate()
            .filter_map(|(rank, (idx, score))| {
                quotes
                    .get(idx)
                    .map(|q| RankedQuote::new(q.clone(), rank + 1, score))
            })
            .collect()
    }

    fn name(&self) -> &'static str {
        "WeightedScore"
    }
}

/// Lowest slippage ranking strategy.
///
/// Ranks quotes by estimated slippage, calculated as the deviation
/// from the best available price. Lower slippage = better score.
#[derive(Debug, Clone, Default)]
pub struct LowestSlippageStrategy;

impl LowestSlippageStrategy {
    /// Creates a new lowest slippage strategy.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Calculates slippage as percentage deviation from best price.
    fn calculate_slippage(price: f64, best_price: f64) -> f64 {
        if best_price == 0.0 {
            return 0.0;
        }
        ((price - best_price) / best_price).abs()
    }
}

impl RankingStrategy for LowestSlippageStrategy {
    fn rank(&self, quotes: &[Quote], side: OrderSide) -> Vec<RankedQuote> {
        if quotes.is_empty() {
            return Vec::new();
        }

        let prices: Vec<f64> = quotes
            .iter()
            .map(|q| q.price().get().to_f64().unwrap_or(0.0))
            .collect();

        // Find best price (lowest for buy, highest for sell)
        let best_price = match side {
            OrderSide::Buy => prices.iter().cloned().fold(f64::INFINITY, f64::min),
            OrderSide::Sell => prices.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
        };

        // Score by slippage (lower slippage = higher score)
        let mut scored: Vec<(usize, f64)> = quotes
            .iter()
            .enumerate()
            .map(|(i, q)| {
                let price = q.price().get().to_f64().unwrap_or(0.0);
                let slippage = Self::calculate_slippage(price, best_price);
                // Invert slippage so lower slippage = higher score
                let score = 1.0 - slippage.min(1.0);
                (i, score)
            })
            .collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        scored
            .into_iter()
            .enumerate()
            .filter_map(|(rank, (idx, score))| {
                quotes
                    .get(idx)
                    .map(|q| RankedQuote::new(q.clone(), rank + 1, score))
            })
            .collect()
    }

    fn name(&self) -> &'static str {
        "LowestSlippage"
    }
}

/// Configuration for cost calculation.
#[derive(Debug, Clone)]
pub struct CostConfig {
    /// Commission rate as a decimal (e.g., 0.001 = 0.1%).
    pub commission_rate: f64,
    /// Estimated gas cost in base currency.
    pub gas_cost: f64,
}

impl Default for CostConfig {
    fn default() -> Self {
        Self {
            commission_rate: 0.001, // 0.1% default commission
            gas_cost: 0.0,
        }
    }
}

impl CostConfig {
    /// Creates a new cost configuration.
    #[must_use]
    pub fn new(commission_rate: f64, gas_cost: f64) -> Self {
        Self {
            commission_rate,
            gas_cost,
        }
    }
}

/// Lowest cost ranking strategy.
///
/// Ranks quotes by total cost including:
/// - Base cost (price × quantity)
/// - Commission
/// - Gas costs
///
/// Lower total cost = better score.
#[derive(Debug, Clone, Default)]
pub struct LowestCostStrategy {
    /// Cost configuration.
    config: CostConfig,
}

impl LowestCostStrategy {
    /// Creates a new lowest cost strategy with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new lowest cost strategy with custom configuration.
    #[must_use]
    pub fn with_config(config: CostConfig) -> Self {
        Self { config }
    }

    /// Calculates total cost for a quote.
    fn calculate_total_cost(&self, price: f64, quantity: f64) -> f64 {
        let base_cost = price * quantity;
        let commission = base_cost * self.config.commission_rate;
        base_cost + commission + self.config.gas_cost
    }
}

impl RankingStrategy for LowestCostStrategy {
    fn rank(&self, quotes: &[Quote], side: OrderSide) -> Vec<RankedQuote> {
        if quotes.is_empty() {
            return Vec::new();
        }

        // Calculate costs for all quotes
        let costs: Vec<f64> = quotes
            .iter()
            .map(|q| {
                let price = q.price().get().to_f64().unwrap_or(0.0);
                let quantity = q.quantity().get().to_f64().unwrap_or(0.0);
                self.calculate_total_cost(price, quantity)
            })
            .collect();

        let min_cost = costs.iter().cloned().fold(f64::INFINITY, f64::min);
        let max_cost = costs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let cost_range = (max_cost - min_cost).max(1.0);

        // Score by cost (lower cost = higher score for buy, opposite for sell)
        let mut scored: Vec<(usize, f64)> = costs
            .iter()
            .enumerate()
            .map(|(i, &cost)| {
                let score = match side {
                    OrderSide::Buy => (max_cost - cost) / cost_range,
                    OrderSide::Sell => (cost - min_cost) / cost_range,
                };
                (i, score)
            })
            .collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        scored
            .into_iter()
            .enumerate()
            .filter_map(|(rank, (idx, score))| {
                quotes
                    .get(idx)
                    .map(|q| RankedQuote::new(q.clone(), rank + 1, score))
            })
            .collect()
    }

    fn name(&self) -> &'static str {
        "LowestCost"
    }
}

/// Composite ranking strategy.
///
/// Combines multiple ranking strategies with configurable weights.
/// Each strategy's scores are normalized and combined using weighted average.
#[derive(Debug)]
pub struct CompositeStrategy {
    /// Strategies with their weights.
    strategies: Vec<(Box<dyn RankingStrategy>, f64)>,
}

impl CompositeStrategy {
    /// Creates a new composite strategy with the given strategies and weights.
    ///
    /// # Arguments
    ///
    /// * `strategies` - Vector of (strategy, weight) pairs
    #[must_use]
    pub fn new(strategies: Vec<(Box<dyn RankingStrategy>, f64)>) -> Self {
        Self { strategies }
    }

    /// Creates a builder for constructing a composite strategy.
    #[must_use]
    pub fn builder() -> CompositeStrategyBuilder {
        CompositeStrategyBuilder::new()
    }
}

impl RankingStrategy for CompositeStrategy {
    fn rank(&self, quotes: &[Quote], side: OrderSide) -> Vec<RankedQuote> {
        if quotes.is_empty() || self.strategies.is_empty() {
            return Vec::new();
        }

        // Collect scores from each strategy
        let mut combined_scores: Vec<f64> = vec![0.0; quotes.len()];
        let mut total_weight = 0.0;

        for (strategy, weight) in &self.strategies {
            let ranked = strategy.rank(quotes, side);

            // Map scores back to original quote indices
            for rq in ranked {
                // Find the original index by matching quote
                if let Some(idx) = quotes.iter().position(|q| q.id() == rq.quote.id())
                    && let Some(score) = combined_scores.get_mut(idx)
                {
                    *score += rq.score * weight;
                }
            }
            total_weight += weight;
        }

        // Normalize by total weight
        if total_weight > 0.0 {
            for score in &mut combined_scores {
                *score /= total_weight;
            }
        }

        // Create scored pairs and sort
        let mut scored: Vec<(usize, f64)> = combined_scores.into_iter().enumerate().collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        scored
            .into_iter()
            .enumerate()
            .filter_map(|(rank, (idx, score))| {
                quotes
                    .get(idx)
                    .map(|q| RankedQuote::new(q.clone(), rank + 1, score))
            })
            .collect()
    }

    fn name(&self) -> &'static str {
        "Composite"
    }
}

/// Builder for constructing composite strategies.
#[derive(Debug, Default)]
pub struct CompositeStrategyBuilder {
    strategies: Vec<(Box<dyn RankingStrategy>, f64)>,
}

impl CompositeStrategyBuilder {
    /// Creates a new builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a strategy with the given weight.
    #[must_use]
    pub fn with_strategy(mut self, strategy: Box<dyn RankingStrategy>, weight: f64) -> Self {
        self.strategies.push((strategy, weight));
        self
    }

    /// Adds a best price strategy with the given weight.
    #[must_use]
    pub fn with_best_price(self, weight: f64) -> Self {
        self.with_strategy(Box::new(BestPriceStrategy::new()), weight)
    }

    /// Adds a lowest slippage strategy with the given weight.
    #[must_use]
    pub fn with_lowest_slippage(self, weight: f64) -> Self {
        self.with_strategy(Box::new(LowestSlippageStrategy::new()), weight)
    }

    /// Adds a lowest cost strategy with the given weight.
    #[must_use]
    pub fn with_lowest_cost(self, weight: f64) -> Self {
        self.with_strategy(Box::new(LowestCostStrategy::new()), weight)
    }

    /// Builds the composite strategy.
    #[must_use]
    pub fn build(self) -> CompositeStrategy {
        CompositeStrategy::new(self.strategies)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::domain::value_objects::timestamp::Timestamp;
    use crate::domain::value_objects::{Price, Quantity, RfqId, VenueId};

    fn create_quote(price: f64, quantity: f64, venue: &str) -> Quote {
        Quote::new(
            RfqId::new_v4(),
            VenueId::new(venue),
            Price::new(price).unwrap(),
            Quantity::new(quantity).unwrap(),
            Timestamp::now().add_secs(60),
        )
        .unwrap()
    }

    #[test]
    fn ranked_quote_new() {
        let quote = create_quote(100.0, 1.0, "venue-1");
        let ranked = RankedQuote::new(quote, 1, 0.95);
        assert_eq!(ranked.rank, 1);
        assert!((ranked.score - 0.95).abs() < f64::EPSILON);
        assert!(ranked.is_best());
    }

    #[test]
    fn ranked_quote_not_best() {
        let quote = create_quote(100.0, 1.0, "venue-1");
        let ranked = RankedQuote::new(quote, 2, 0.85);
        assert!(!ranked.is_best());
    }

    #[test]
    fn best_price_strategy_buy_side() {
        let strategy = BestPriceStrategy::new();
        let quotes = vec![
            create_quote(100.0, 1.0, "venue-1"),
            create_quote(95.0, 1.0, "venue-2"),
            create_quote(105.0, 1.0, "venue-3"),
        ];

        let ranked = strategy.rank(&quotes, OrderSide::Buy);

        assert_eq!(ranked.len(), 3);
        assert_eq!(ranked[0].rank, 1);
        assert!((ranked[0].quote.price().get().to_f64().unwrap() - 95.0).abs() < f64::EPSILON);
        assert_eq!(ranked[1].rank, 2);
        assert!((ranked[1].quote.price().get().to_f64().unwrap() - 100.0).abs() < f64::EPSILON);
        assert_eq!(ranked[2].rank, 3);
        assert!((ranked[2].quote.price().get().to_f64().unwrap() - 105.0).abs() < f64::EPSILON);
    }

    #[test]
    fn best_price_strategy_sell_side() {
        let strategy = BestPriceStrategy::new();
        let quotes = vec![
            create_quote(100.0, 1.0, "venue-1"),
            create_quote(95.0, 1.0, "venue-2"),
            create_quote(105.0, 1.0, "venue-3"),
        ];

        let ranked = strategy.rank(&quotes, OrderSide::Sell);

        assert_eq!(ranked.len(), 3);
        assert_eq!(ranked[0].rank, 1);
        assert!((ranked[0].quote.price().get().to_f64().unwrap() - 105.0).abs() < f64::EPSILON);
        assert_eq!(ranked[1].rank, 2);
        assert!((ranked[1].quote.price().get().to_f64().unwrap() - 100.0).abs() < f64::EPSILON);
        assert_eq!(ranked[2].rank, 3);
        assert!((ranked[2].quote.price().get().to_f64().unwrap() - 95.0).abs() < f64::EPSILON);
    }

    #[test]
    fn best_price_strategy_empty() {
        let strategy = BestPriceStrategy::new();
        let ranked = strategy.rank(&[], OrderSide::Buy);
        assert!(ranked.is_empty());
    }

    #[test]
    fn weighted_score_strategy_default() {
        let strategy = WeightedScoreStrategy::default();
        assert!((strategy.price_weight - 0.7).abs() < f64::EPSILON);
        assert!((strategy.quantity_weight - 0.3).abs() < f64::EPSILON);
    }

    #[test]
    fn weighted_score_strategy_custom() {
        let strategy = WeightedScoreStrategy::new(0.5, 0.5);
        assert!((strategy.price_weight - 0.5).abs() < f64::EPSILON);
        assert!((strategy.quantity_weight - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn weighted_score_strategy_buy_side() {
        let strategy = WeightedScoreStrategy::new(0.7, 0.3);
        let quotes = vec![
            create_quote(100.0, 10.0, "venue-1"),
            create_quote(95.0, 5.0, "venue-2"),
            create_quote(105.0, 15.0, "venue-3"),
        ];

        let ranked = strategy.rank(&quotes, OrderSide::Buy);

        assert_eq!(ranked.len(), 3);
        // Best should be venue-2 (lowest price) despite lower quantity
        assert_eq!(ranked[0].rank, 1);
    }

    #[test]
    fn ranking_strategy_name() {
        let best_price = BestPriceStrategy::new();
        assert_eq!(best_price.name(), "BestPrice");

        let weighted = WeightedScoreStrategy::default();
        assert_eq!(weighted.name(), "WeightedScore");

        let slippage = LowestSlippageStrategy::new();
        assert_eq!(slippage.name(), "LowestSlippage");

        let cost = LowestCostStrategy::new();
        assert_eq!(cost.name(), "LowestCost");

        let composite = CompositeStrategy::builder().build();
        assert_eq!(composite.name(), "Composite");
    }

    #[test]
    fn lowest_slippage_strategy_buy_side() {
        let strategy = LowestSlippageStrategy::new();
        let quotes = vec![
            create_quote(100.0, 1.0, "venue-1"),
            create_quote(95.0, 1.0, "venue-2"), // Best price (lowest)
            create_quote(105.0, 1.0, "venue-3"),
        ];

        let ranked = strategy.rank(&quotes, OrderSide::Buy);

        assert_eq!(ranked.len(), 3);
        // Best price (95) should have lowest slippage = rank 1
        assert_eq!(ranked[0].rank, 1);
        assert!((ranked[0].quote.price().get().to_f64().unwrap() - 95.0).abs() < f64::EPSILON);
        // Score should be 1.0 for best price (no slippage)
        assert!((ranked[0].score - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn lowest_slippage_strategy_sell_side() {
        let strategy = LowestSlippageStrategy::new();
        let quotes = vec![
            create_quote(100.0, 1.0, "venue-1"),
            create_quote(95.0, 1.0, "venue-2"),
            create_quote(105.0, 1.0, "venue-3"), // Best price (highest)
        ];

        let ranked = strategy.rank(&quotes, OrderSide::Sell);

        assert_eq!(ranked.len(), 3);
        // Best price (105) should have lowest slippage = rank 1
        assert_eq!(ranked[0].rank, 1);
        assert!((ranked[0].quote.price().get().to_f64().unwrap() - 105.0).abs() < f64::EPSILON);
    }

    #[test]
    fn lowest_slippage_strategy_empty() {
        let strategy = LowestSlippageStrategy::new();
        let ranked = strategy.rank(&[], OrderSide::Buy);
        assert!(ranked.is_empty());
    }

    #[test]
    fn lowest_cost_strategy_buy_side() {
        let strategy = LowestCostStrategy::new();
        let quotes = vec![
            create_quote(100.0, 10.0, "venue-1"), // Cost: 100*10 + 0.1% = 1001
            create_quote(95.0, 10.0, "venue-2"),  // Cost: 95*10 + 0.1% = 950.95 (lowest)
            create_quote(105.0, 10.0, "venue-3"), // Cost: 105*10 + 0.1% = 1051.05
        ];

        let ranked = strategy.rank(&quotes, OrderSide::Buy);

        assert_eq!(ranked.len(), 3);
        // Lowest cost should be rank 1
        assert_eq!(ranked[0].rank, 1);
        assert!((ranked[0].quote.price().get().to_f64().unwrap() - 95.0).abs() < f64::EPSILON);
    }

    #[test]
    fn lowest_cost_strategy_with_gas() {
        let config = CostConfig::new(0.001, 10.0); // 0.1% commission + $10 gas
        let strategy = LowestCostStrategy::with_config(config);
        let quotes = vec![
            create_quote(100.0, 1.0, "venue-1"), // Cost: 100 + 0.1 + 10 = 110.1
            create_quote(95.0, 1.0, "venue-2"),  // Cost: 95 + 0.095 + 10 = 105.095 (lowest)
        ];

        let ranked = strategy.rank(&quotes, OrderSide::Buy);

        assert_eq!(ranked.len(), 2);
        assert_eq!(ranked[0].rank, 1);
        assert!((ranked[0].quote.price().get().to_f64().unwrap() - 95.0).abs() < f64::EPSILON);
    }

    #[test]
    fn lowest_cost_strategy_empty() {
        let strategy = LowestCostStrategy::new();
        let ranked = strategy.rank(&[], OrderSide::Buy);
        assert!(ranked.is_empty());
    }

    #[test]
    fn cost_config_default() {
        let config = CostConfig::default();
        assert!((config.commission_rate - 0.001).abs() < f64::EPSILON);
        assert!((config.gas_cost - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn cost_config_custom() {
        let config = CostConfig::new(0.002, 5.0);
        assert!((config.commission_rate - 0.002).abs() < f64::EPSILON);
        assert!((config.gas_cost - 5.0).abs() < f64::EPSILON);
    }

    #[test]
    fn composite_strategy_single() {
        let strategy = CompositeStrategy::builder().with_best_price(1.0).build();

        let quotes = vec![
            create_quote(100.0, 1.0, "venue-1"),
            create_quote(95.0, 1.0, "venue-2"),
            create_quote(105.0, 1.0, "venue-3"),
        ];

        let ranked = strategy.rank(&quotes, OrderSide::Buy);

        assert_eq!(ranked.len(), 3);
        // Should behave like BestPriceStrategy
        assert_eq!(ranked[0].rank, 1);
        assert!((ranked[0].quote.price().get().to_f64().unwrap() - 95.0).abs() < f64::EPSILON);
    }

    #[test]
    fn composite_strategy_multiple() {
        let strategy = CompositeStrategy::builder()
            .with_best_price(0.5)
            .with_lowest_slippage(0.5)
            .build();

        let quotes = vec![
            create_quote(100.0, 1.0, "venue-1"),
            create_quote(95.0, 1.0, "venue-2"),
            create_quote(105.0, 1.0, "venue-3"),
        ];

        let ranked = strategy.rank(&quotes, OrderSide::Buy);

        assert_eq!(ranked.len(), 3);
        // Both strategies favor lowest price for buy side
        assert_eq!(ranked[0].rank, 1);
    }

    #[test]
    fn composite_strategy_empty_quotes() {
        let strategy = CompositeStrategy::builder().with_best_price(1.0).build();

        let ranked = strategy.rank(&[], OrderSide::Buy);
        assert!(ranked.is_empty());
    }

    #[test]
    fn composite_strategy_no_strategies() {
        let strategy = CompositeStrategy::builder().build();

        let quotes = vec![create_quote(100.0, 1.0, "venue-1")];
        let ranked = strategy.rank(&quotes, OrderSide::Buy);
        assert!(ranked.is_empty());
    }

    #[test]
    fn composite_strategy_builder() {
        let strategy = CompositeStrategy::builder()
            .with_best_price(0.4)
            .with_lowest_slippage(0.3)
            .with_lowest_cost(0.3)
            .build();

        assert_eq!(strategy.strategies.len(), 3);
    }
}
