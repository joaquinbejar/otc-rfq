//! # Ranking Strategy
//!
//! Strategies for ranking quotes.
//!
//! This module provides the [`RankingStrategy`] trait and implementations
//! for ranking quotes based on different criteria.

use crate::domain::entities::quote::Quote;
use crate::domain::entities::quote_normalizer::NormalizedQuote;
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

/// A normalized quote with its ranking information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RankedNormalizedQuote {
    /// The normalized quote being ranked.
    pub normalized_quote: NormalizedQuote,
    /// The rank (1 = best).
    pub rank: usize,
    /// The score used for ranking (higher = better).
    pub score: f64,
}

impl RankedNormalizedQuote {
    /// Creates a new ranked normalized quote.
    #[must_use]
    pub fn new(normalized_quote: NormalizedQuote, rank: usize, score: f64) -> Self {
        Self {
            normalized_quote,
            rank,
            score,
        }
    }

    /// Returns true if this quote is the best (rank 1).
    #[must_use]
    pub fn is_best(&self) -> bool {
        self.rank == 1
    }
}

impl fmt::Display for RankedNormalizedQuote {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "RankedNormalizedQuote(#{} score={:.4} quote={})",
            self.rank, self.score, self.normalized_quote
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

    /// Ranks normalized quotes for the specified order side.
    ///
    /// This method enables ranking quotes that have been normalized for
    /// fair multi-venue comparison (FX conversion, fee inclusion, etc.).
    ///
    /// # Arguments
    ///
    /// * `quotes` - The normalized quotes to rank
    /// * `side` - The order side (Buy or Sell)
    ///
    /// # Returns
    ///
    /// A vector of ranked normalized quotes sorted by rank (best first).
    ///
    /// # Default Implementation
    ///
    /// The default implementation scores each quote using `score_normalized`
    /// and ranks them by score (higher is better).
    fn rank_normalized(
        &self,
        quotes: &[NormalizedQuote],
        side: OrderSide,
    ) -> Vec<RankedNormalizedQuote> {
        if quotes.is_empty() {
            return Vec::new();
        }

        // Score each quote
        let mut scored: Vec<(usize, f64)> = quotes
            .iter()
            .enumerate()
            .map(|(i, q)| {
                let score = self.score_normalized(q, side);
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
                    .map(|q| RankedNormalizedQuote::new(q.clone(), rank + 1, score))
            })
            .collect()
    }

    /// Scores a normalized quote for ranking.
    ///
    /// # Arguments
    ///
    /// * `quote` - The normalized quote to score
    /// * `side` - The order side (Buy or Sell)
    ///
    /// # Returns
    ///
    /// A score where higher values indicate better quotes.
    fn score_normalized(&self, quote: &NormalizedQuote, side: OrderSide) -> f64;

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

    fn score_normalized(&self, quote: &NormalizedQuote, side: OrderSide) -> f64 {
        let price = quote.all_in_price().get().to_f64().unwrap_or(0.0);
        match side {
            OrderSide::Buy => -price, // Lower price is better for buying
            OrderSide::Sell => price, // Higher price is better for selling
        }
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

    fn score_normalized(&self, quote: &NormalizedQuote, _side: OrderSide) -> f64 {
        // For normalized quotes, use simple weighted score
        // Price and quantity are already normalized
        let price = quote.all_in_price().get().to_f64().unwrap_or(0.0);
        let qty = quote.normalized_quantity().get().to_f64().unwrap_or(0.0);
        self.price_weight * price + self.quantity_weight * qty
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

    fn score_normalized(&self, quote: &NormalizedQuote, _side: OrderSide) -> f64 {
        // For normalized quotes, assume minimal slippage since they're already normalized
        // Use all_in_price for scoring
        let price = quote.all_in_price().get().to_f64().unwrap_or(0.0);
        1.0 / (1.0 + price)
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

    fn score_normalized(&self, quote: &NormalizedQuote, side: OrderSide) -> f64 {
        // For normalized quotes, all_in_price already includes fees
        let price = quote.all_in_price().get().to_f64().unwrap_or(0.0);
        let quantity = quote.normalized_quantity().get().to_f64().unwrap_or(0.0);
        let total_cost = price * quantity;
        match side {
            OrderSide::Buy => -total_cost, // Lower cost is better
            OrderSide::Sell => total_cost, // Higher revenue is better
        }
    }

    fn name(&self) -> &'static str {
        "LowestCost"
    }
}

/// Configurable weights for multi-factor ranking.
///
/// Weights should sum to approximately 1.0 for proper normalization,
/// but this is not enforced.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RankingWeights {
    /// Weight for price factor (default 0.70).
    pub price: f64,
    /// Weight for quantity factor (default 0.15).
    pub quantity: f64,
    /// Weight for reliability factor (default 0.10).
    pub reliability: f64,
    /// Weight for freshness factor (default 0.05).
    pub freshness: f64,
}

impl Default for RankingWeights {
    fn default() -> Self {
        Self {
            price: 0.70,
            quantity: 0.15,
            reliability: 0.10,
            freshness: 0.05,
        }
    }
}

impl RankingWeights {
    /// Creates new ranking weights.
    #[must_use]
    pub fn new(price: f64, quantity: f64, reliability: f64, freshness: f64) -> Self {
        Self {
            price,
            quantity,
            reliability,
            freshness,
        }
    }

    /// Returns the sum of all weights.
    #[must_use]
    pub fn sum(&self) -> f64 {
        self.price + self.quantity + self.reliability + self.freshness
    }
}

/// Weighted multi-factor ranking strategy.
///
/// Ranks quotes using a weighted combination of four factors:
/// - **Price** (default 70%): Best price scores highest
/// - **Quantity** (default 15%): Ratio of quoted to requested quantity
/// - **Reliability** (default 10%): MM response rate and reject rate
/// - **Freshness** (default 5%): Newest quotes score highest
///
/// # Example
///
/// ```ignore
/// use otc_rfq::application::services::ranking_strategy::WeightedMultiFactorStrategy;
///
/// let strategy = WeightedMultiFactorStrategy::new();
/// ```
#[derive(Debug, Clone)]
pub struct WeightedMultiFactorStrategy {
    weights: RankingWeights,
    /// Reliability scores by venue ID (pre-computed or fetched).
    /// Maps venue_id string to reliability score [0.0, 1.0].
    reliability_scores: std::collections::HashMap<String, f64>,
    /// Requested quantity for quantity score normalization.
    requested_quantity: Option<f64>,
}

impl WeightedMultiFactorStrategy {
    /// Creates a new weighted multi-factor strategy with default weights.
    #[must_use]
    pub fn new() -> Self {
        Self {
            weights: RankingWeights::default(),
            reliability_scores: std::collections::HashMap::new(),
            requested_quantity: None,
        }
    }

    /// Creates a new strategy with custom weights.
    #[must_use]
    pub fn with_weights(weights: RankingWeights) -> Self {
        Self {
            weights,
            reliability_scores: std::collections::HashMap::new(),
            requested_quantity: None,
        }
    }

    /// Sets the requested quantity for quantity score calculation.
    #[must_use]
    pub fn with_requested_quantity(mut self, quantity: f64) -> Self {
        self.requested_quantity = Some(quantity);
        self
    }

    /// Sets reliability scores for venues.
    ///
    /// Reliability score should be in [0.0, 1.0] range.
    #[must_use]
    pub fn with_reliability_scores(
        mut self,
        scores: std::collections::HashMap<String, f64>,
    ) -> Self {
        self.reliability_scores = scores;
        self
    }

    /// Adds a reliability score for a specific venue.
    pub fn add_reliability_score(&mut self, venue_id: &str, score: f64) {
        self.reliability_scores.insert(venue_id.to_string(), score);
    }

    /// Computes reliability score from response_rate_pct and reject_rate_pct.
    ///
    /// Formula: (response_rate/100 + (1 - reject_rate/100)) / 2
    /// Result is clamped to [0.0, 1.0].
    #[must_use]
    pub fn compute_reliability_score(
        response_rate_pct: Option<f64>,
        reject_rate_pct: Option<f64>,
    ) -> f64 {
        // Clamp inputs to valid percentage range
        let response_rate = response_rate_pct.unwrap_or(50.0).clamp(0.0, 100.0) / 100.0;
        let reject_rate = reject_rate_pct.unwrap_or(0.0).clamp(0.0, 100.0) / 100.0;
        ((response_rate + (1.0 - reject_rate)) / 2.0).clamp(0.0, 1.0)
    }

    /// Normalizes price score to [0, 1] where 1 is best.
    fn normalize_price(&self, price: f64, min_price: f64, max_price: f64, side: OrderSide) -> f64 {
        let price_range = (max_price - min_price).max(f64::EPSILON);
        match side {
            OrderSide::Buy => (max_price - price) / price_range, // Lower is better
            OrderSide::Sell => (price - min_price) / price_range, // Higher is better
        }
    }

    /// Computes quantity score as ratio to requested quantity (capped at 1.0).
    fn quantity_score(&self, quoted_qty: f64) -> f64 {
        match self.requested_quantity {
            Some(requested) if requested > 0.0 => (quoted_qty / requested).min(1.0),
            _ => 1.0, // If no requested quantity, assume full fill
        }
    }

    /// Gets reliability score for a venue, clamped to [0.0, 1.0].
    fn reliability_score(&self, venue_id: &str) -> f64 {
        self.reliability_scores
            .get(venue_id)
            .copied()
            .unwrap_or(0.5) // Default to neutral 0.5 for unknown venues
            .clamp(0.0, 1.0)
    }

    /// Computes freshness score based on quote age.
    /// Newest quote gets 1.0, oldest gets 0.0.
    fn freshness_score(&self, quote_created_at: i64, oldest: i64, newest: i64) -> f64 {
        let age_range = (newest - oldest).max(1);
        let quote_age = newest - quote_created_at;
        1.0 - (quote_age as f64 / age_range as f64)
    }
}

impl Default for WeightedMultiFactorStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl RankingStrategy for WeightedMultiFactorStrategy {
    fn rank(&self, quotes: &[Quote], side: OrderSide) -> Vec<RankedQuote> {
        if quotes.is_empty() {
            return Vec::new();
        }

        // Extract prices and quantities
        let prices: Vec<f64> = quotes
            .iter()
            .map(|q| q.price().get().to_f64().unwrap_or(0.0))
            .collect();
        let quantities: Vec<f64> = quotes
            .iter()
            .map(|q| q.quantity().get().to_f64().unwrap_or(0.0))
            .collect();
        let timestamps: Vec<i64> = quotes
            .iter()
            .map(|q| q.created_at().timestamp_millis())
            .collect();

        // Find min/max for normalization
        let min_price = prices.iter().cloned().fold(f64::INFINITY, f64::min);
        let max_price = prices.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let oldest_ts = timestamps.iter().cloned().min().unwrap_or(0);
        let newest_ts = timestamps.iter().cloned().max().unwrap_or(0);

        // Score each quote
        let mut scored: Vec<(usize, f64)> = quotes
            .iter()
            .enumerate()
            .filter_map(|(i, q)| {
                let price = prices.get(i)?;
                let qty = quantities.get(i)?;
                let ts = timestamps.get(i)?;

                let price_score = self.normalize_price(*price, min_price, max_price, side);
                let qty_score = self.quantity_score(*qty);
                let reliability = self.reliability_score(q.venue_id().as_str());
                let freshness = self.freshness_score(*ts, oldest_ts, newest_ts);

                let total = self.weights.price * price_score
                    + self.weights.quantity * qty_score
                    + self.weights.reliability * reliability
                    + self.weights.freshness * freshness;

                Some((i, total))
            })
            .collect();

        // Sort by score descending (higher is better)
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

    fn score_normalized(&self, quote: &NormalizedQuote, side: OrderSide) -> f64 {
        let price = quote.all_in_price().get().to_f64().unwrap_or(0.0);
        let qty = quote.normalized_quantity().get().to_f64().unwrap_or(0.0);
        let ts = quote.adjusted_timestamp().timestamp_millis();

        // Simple scoring for normalized quotes
        let price_score = match side {
            OrderSide::Buy => 1.0 / (1.0 + price),
            OrderSide::Sell => price,
        };
        let qty_score = self.quantity_score(qty);
        let reliability = self.reliability_score(quote.venue_id().as_str());
        let freshness = 1.0 - (ts as f64 / 1000000000.0).fract();

        self.weights.price * price_score
            + self.weights.quantity * qty_score
            + self.weights.reliability * reliability
            + self.weights.freshness * freshness
    }

    fn name(&self) -> &'static str {
        "WeightedMultiFactor"
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

    fn score_normalized(&self, quote: &NormalizedQuote, side: OrderSide) -> f64 {
        if self.strategies.is_empty() {
            return 0.0;
        }

        let mut total_score = 0.0;
        let mut total_weight = 0.0;

        for (strategy, weight) in &self.strategies {
            total_score += strategy.score_normalized(quote, side) * weight;
            total_weight += weight;
        }

        if total_weight > 0.0 {
            total_score / total_weight
        } else {
            0.0
        }
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

    // WeightedMultiFactorStrategy tests

    #[test]
    fn ranking_weights_default() {
        let weights = RankingWeights::default();
        assert!((weights.price - 0.70).abs() < f64::EPSILON);
        assert!((weights.quantity - 0.15).abs() < f64::EPSILON);
        assert!((weights.reliability - 0.10).abs() < f64::EPSILON);
        assert!((weights.freshness - 0.05).abs() < f64::EPSILON);
        assert!((weights.sum() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn ranking_weights_custom() {
        let weights = RankingWeights::new(0.5, 0.2, 0.2, 0.1);
        assert!((weights.price - 0.5).abs() < f64::EPSILON);
        assert!((weights.sum() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn weighted_multi_factor_default() {
        let strategy = WeightedMultiFactorStrategy::new();
        assert_eq!(strategy.name(), "WeightedMultiFactor");
    }

    #[test]
    fn weighted_multi_factor_empty_quotes() {
        let strategy = WeightedMultiFactorStrategy::new();
        let ranked = strategy.rank(&[], OrderSide::Buy);
        assert!(ranked.is_empty());
    }

    #[test]
    fn weighted_multi_factor_price_dominant_buy() {
        // Price has 70% weight, so best price should win
        let strategy = WeightedMultiFactorStrategy::new();
        let quotes = vec![
            create_quote(100.0, 1.0, "venue-1"),
            create_quote(95.0, 1.0, "venue-2"), // Best price for buy
            create_quote(105.0, 1.0, "venue-3"),
        ];

        let ranked = strategy.rank(&quotes, OrderSide::Buy);

        assert_eq!(ranked.len(), 3);
        assert_eq!(ranked[0].rank, 1);
        // Best price (95.0) should be ranked first
        assert!((ranked[0].quote.price().get().to_f64().unwrap() - 95.0).abs() < f64::EPSILON);
    }

    #[test]
    fn weighted_multi_factor_price_dominant_sell() {
        let strategy = WeightedMultiFactorStrategy::new();
        let quotes = vec![
            create_quote(100.0, 1.0, "venue-1"),
            create_quote(95.0, 1.0, "venue-2"),
            create_quote(105.0, 1.0, "venue-3"), // Best price for sell
        ];

        let ranked = strategy.rank(&quotes, OrderSide::Sell);

        assert_eq!(ranked.len(), 3);
        assert_eq!(ranked[0].rank, 1);
        // Best price (105.0) should be ranked first
        assert!((ranked[0].quote.price().get().to_f64().unwrap() - 105.0).abs() < f64::EPSILON);
    }

    #[test]
    fn weighted_multi_factor_with_reliability() {
        let mut scores = std::collections::HashMap::new();
        scores.insert("venue-1".to_string(), 1.0); // Perfect reliability
        scores.insert("venue-2".to_string(), 0.5); // Medium reliability
        scores.insert("venue-3".to_string(), 0.0); // Poor reliability

        let strategy = WeightedMultiFactorStrategy::new().with_reliability_scores(scores);

        // Same price, same quantity - reliability should differentiate
        let quotes = vec![
            create_quote(100.0, 1.0, "venue-1"),
            create_quote(100.0, 1.0, "venue-2"),
            create_quote(100.0, 1.0, "venue-3"),
        ];

        let ranked = strategy.rank(&quotes, OrderSide::Buy);

        assert_eq!(ranked.len(), 3);
        // venue-1 has best reliability
        assert_eq!(ranked[0].quote.venue_id().as_str(), "venue-1");
        assert_eq!(ranked[2].quote.venue_id().as_str(), "venue-3");
    }

    #[test]
    fn weighted_multi_factor_with_requested_quantity() {
        let strategy = WeightedMultiFactorStrategy::new().with_requested_quantity(10.0);

        // Same price, different quantities
        let quotes = vec![
            create_quote(100.0, 10.0, "venue-1"), // Full fill
            create_quote(100.0, 5.0, "venue-2"),  // Partial fill
            create_quote(100.0, 2.0, "venue-3"),  // Small fill
        ];

        let ranked = strategy.rank(&quotes, OrderSide::Buy);

        assert_eq!(ranked.len(), 3);
        // venue-1 has full quantity
        assert_eq!(ranked[0].quote.venue_id().as_str(), "venue-1");
    }

    #[test]
    fn weighted_multi_factor_quantity_capped_at_one() {
        let strategy = WeightedMultiFactorStrategy::new().with_requested_quantity(5.0);

        // Quote offers more than requested
        let quote1 = create_quote(100.0, 10.0, "venue-1"); // Over-fill should cap at 1.0
        let quote2 = create_quote(100.0, 5.0, "venue-2"); // Exact fill

        let quotes = vec![quote1, quote2];

        let ranked = strategy.rank(&quotes, OrderSide::Buy);

        // Both should have quantity score of 1.0
        // The only difference might be freshness (5% weight), so scores should be very close
        let score_diff = (ranked[0].score - ranked[1].score).abs();
        assert!(
            score_diff < 0.06,
            "Score difference {} exceeds threshold (freshness factor is 5%)",
            score_diff
        );
    }

    #[test]
    fn compute_reliability_score_full_data() {
        // 100% response rate, 0% reject rate
        let score = WeightedMultiFactorStrategy::compute_reliability_score(Some(100.0), Some(0.0));
        assert!((score - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn compute_reliability_score_poor_mm() {
        // 50% response rate, 50% reject rate
        let score = WeightedMultiFactorStrategy::compute_reliability_score(Some(50.0), Some(50.0));
        assert!((score - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn compute_reliability_score_defaults() {
        // No data - should use defaults (50% response, 0% reject)
        let score = WeightedMultiFactorStrategy::compute_reliability_score(None, None);
        assert!((score - 0.75).abs() < f64::EPSILON); // (0.5 + 1.0) / 2
    }

    #[test]
    fn weighted_multi_factor_custom_weights() {
        // Extreme weights: 100% price
        let weights = RankingWeights::new(1.0, 0.0, 0.0, 0.0);
        let strategy = WeightedMultiFactorStrategy::with_weights(weights);

        let quotes = vec![
            create_quote(100.0, 1.0, "venue-1"),
            create_quote(95.0, 1.0, "venue-2"),
        ];

        let ranked = strategy.rank(&quotes, OrderSide::Buy);

        // With 100% price weight, should behave like BestPriceStrategy
        assert!((ranked[0].quote.price().get().to_f64().unwrap() - 95.0).abs() < f64::EPSILON);
    }

    #[test]
    fn weighted_multi_factor_add_reliability_score() {
        // Strategy with an explicit reliability score for venue-1
        let mut strategy_with_reliability = WeightedMultiFactorStrategy::new();
        strategy_with_reliability.add_reliability_score("venue-1", 0.9);

        // Baseline strategy using default/neutral reliability for venue-1
        let strategy_default = WeightedMultiFactorStrategy::new();

        let quotes = vec![create_quote(100.0, 1.0, "venue-1")];

        let ranked_with_reliability = strategy_with_reliability.rank(&quotes, OrderSide::Buy);
        let ranked_default = strategy_default.rank(&quotes, OrderSide::Buy);

        assert_eq!(ranked_with_reliability.len(), 1);
        assert_eq!(ranked_default.len(), 1);

        // The custom reliability score (0.9) should increase the final score
        // compared to the default/neutral reliability (0.5) used by strategy_default.
        assert!(
            ranked_with_reliability[0].score > ranked_default[0].score,
            "Expected custom reliability score to produce a higher ranking score"
        );
    }

    #[test]
    fn weighted_multi_factor_freshness_affects_ordering() {
        // Use 100% freshness weight to isolate the freshness factor
        let weights = RankingWeights::new(0.0, 0.0, 0.0, 1.0);
        let strategy = WeightedMultiFactorStrategy::with_weights(weights);

        // Create quotes with different timestamps
        // Note: create_quote uses Timestamp::now(), so we need quotes created at different times
        let quote1 = create_quote(100.0, 1.0, "venue-1"); // Created first (older)
        std::thread::sleep(std::time::Duration::from_millis(2));
        let quote2 = create_quote(100.0, 1.0, "venue-2"); // Created second (newer)

        let quotes = vec![quote1, quote2];
        let ranked = strategy.rank(&quotes, OrderSide::Buy);

        assert_eq!(ranked.len(), 2);
        // With 100% freshness weight, the newer quote (venue-2) should rank first
        assert_eq!(ranked[0].quote.venue_id().as_str(), "venue-2");
        assert_eq!(ranked[1].quote.venue_id().as_str(), "venue-1");
        // Newer quote should have higher score
        assert!(ranked[0].score > ranked[1].score);
    }

    #[test]
    fn compute_reliability_score_clamps_out_of_range() {
        // Test clamping when inputs exceed valid range
        let score =
            WeightedMultiFactorStrategy::compute_reliability_score(Some(150.0), Some(-10.0));
        // 150% clamped to 100%, -10% clamped to 0%
        // (1.0 + 1.0) / 2 = 1.0
        assert!((score - 1.0).abs() < f64::EPSILON);

        let score2 =
            WeightedMultiFactorStrategy::compute_reliability_score(Some(-50.0), Some(200.0));
        // -50% clamped to 0%, 200% clamped to 100%
        // (0.0 + 0.0) / 2 = 0.0
        assert!((score2 - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn reliability_score_clamps_invalid_values() {
        let mut strategy = WeightedMultiFactorStrategy::new();
        strategy.add_reliability_score("venue-high", 1.5); // Above 1.0
        strategy.add_reliability_score("venue-low", -0.5); // Below 0.0

        // Same price/qty, reliability should differentiate but be clamped
        let quotes = vec![
            create_quote(100.0, 1.0, "venue-high"),
            create_quote(100.0, 1.0, "venue-low"),
        ];

        let ranked = strategy.rank(&quotes, OrderSide::Buy);

        // venue-high (clamped to 1.0) should rank above venue-low (clamped to 0.0)
        assert_eq!(ranked[0].quote.venue_id().as_str(), "venue-high");
        assert_eq!(ranked[1].quote.venue_id().as_str(), "venue-low");
    }
}
