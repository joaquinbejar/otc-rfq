//! # Quote Aggregation Engine
//!
//! Orchestrates quote collection and ranking.
//!
//! This module provides the [`QuoteAggregationEngine`] which coordinates
//! concurrent quote collection from multiple venues and applies ranking
//! strategies to the results.

use crate::application::services::ranking_strategy::{RankedQuote, RankingStrategy};
use crate::application::use_cases::collect_quotes::VenueRegistry;
use crate::domain::entities::quote::Quote;
use crate::domain::entities::rfq::Rfq;
use crate::infrastructure::venues::error::VenueError;
use std::fmt;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;

/// Configuration for quote aggregation.
#[derive(Debug, Clone)]
pub struct AggregationConfig {
    /// Overall timeout for quote collection in milliseconds.
    pub timeout_ms: u64,
    /// Minimum number of quotes required.
    pub min_quotes: usize,
    /// Maximum number of quotes to return.
    pub max_quotes: Option<usize>,
    /// Per-venue timeout in milliseconds.
    pub per_venue_timeout_ms: u64,
}

impl Default for AggregationConfig {
    fn default() -> Self {
        Self {
            timeout_ms: 10000,
            min_quotes: 1,
            max_quotes: None,
            per_venue_timeout_ms: 5000,
        }
    }
}

impl AggregationConfig {
    /// Creates a new configuration with the specified overall timeout.
    #[must_use]
    pub fn with_timeout(timeout_ms: u64) -> Self {
        Self {
            timeout_ms,
            ..Default::default()
        }
    }

    /// Sets the minimum number of quotes required.
    #[must_use]
    pub fn with_min_quotes(mut self, min: usize) -> Self {
        self.min_quotes = min;
        self
    }

    /// Sets the maximum number of quotes to return.
    #[must_use]
    pub fn with_max_quotes(mut self, max: usize) -> Self {
        self.max_quotes = Some(max);
        self
    }

    /// Sets the per-venue timeout.
    #[must_use]
    pub fn with_per_venue_timeout(mut self, timeout_ms: u64) -> Self {
        self.per_venue_timeout_ms = timeout_ms;
        self
    }
}

/// Result of quote aggregation.
#[derive(Debug)]
pub struct AggregationResult {
    /// Ranked quotes (best first).
    pub ranked_quotes: Vec<RankedQuote>,
    /// Total quotes collected before filtering.
    pub total_collected: usize,
    /// Number of venues queried.
    pub venues_queried: usize,
    /// Number of venues that responded.
    pub venues_responded: usize,
    /// Number of quotes filtered out (expired, invalid).
    pub filtered_count: usize,
}

impl AggregationResult {
    /// Returns true if the minimum quotes requirement was met.
    #[must_use]
    pub fn has_sufficient_quotes(&self, min_quotes: usize) -> bool {
        self.ranked_quotes.len() >= min_quotes
    }

    /// Returns the best quote, if any.
    #[must_use]
    pub fn best_quote(&self) -> Option<&RankedQuote> {
        self.ranked_quotes.first()
    }
}

/// Error type for aggregation operations.
#[derive(Debug, Clone)]
pub enum AggregationError {
    /// No venues available.
    NoVenuesAvailable,
    /// Insufficient quotes collected.
    InsufficientQuotes {
        /// Number of quotes collected.
        collected: usize,
        /// Number of quotes required.
        required: usize,
    },
    /// Overall timeout exceeded.
    Timeout,
    /// All venues failed.
    AllVenuesFailed(Vec<String>),
}

impl fmt::Display for AggregationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoVenuesAvailable => write!(f, "no venues available"),
            Self::InsufficientQuotes {
                collected,
                required,
            } => {
                write!(
                    f,
                    "insufficient quotes: got {}, need {}",
                    collected, required
                )
            }
            Self::Timeout => write!(f, "quote collection timed out"),
            Self::AllVenuesFailed(errors) => {
                write!(f, "all venues failed: {}", errors.join(", "))
            }
        }
    }
}

impl std::error::Error for AggregationError {}

/// Result type for aggregation operations.
pub type AggregationResultType<T> = Result<T, AggregationError>;

/// Engine for collecting and ranking quotes from multiple venues.
#[derive(Debug)]
pub struct QuoteAggregationEngine {
    venue_registry: Arc<dyn VenueRegistry>,
    ranking_strategy: Arc<dyn RankingStrategy>,
    config: AggregationConfig,
    quote_normalizer: Option<Arc<crate::domain::services::quote_normalizer::QuoteNormalizer>>,
}

impl QuoteAggregationEngine {
    /// Creates a new QuoteAggregationEngine.
    #[must_use]
    pub fn new(
        venue_registry: Arc<dyn VenueRegistry>,
        ranking_strategy: Arc<dyn RankingStrategy>,
        config: AggregationConfig,
    ) -> Self {
        Self {
            venue_registry,
            ranking_strategy,
            config,
            quote_normalizer: None,
        }
    }

    /// Creates a new engine with default configuration.
    #[must_use]
    pub fn with_defaults(
        venue_registry: Arc<dyn VenueRegistry>,
        ranking_strategy: Arc<dyn RankingStrategy>,
    ) -> Self {
        Self::new(
            venue_registry,
            ranking_strategy,
            AggregationConfig::default(),
        )
    }

    /// Creates a new engine with quote normalization enabled.
    ///
    /// When a normalizer is provided, quotes will be normalized before ranking
    /// to ensure fair multi-venue comparison with FX conversion, fee inclusion,
    /// and quantity adjustments.
    #[must_use]
    pub fn with_normalizer(
        venue_registry: Arc<dyn VenueRegistry>,
        ranking_strategy: Arc<dyn RankingStrategy>,
        config: AggregationConfig,
        quote_normalizer: Arc<crate::domain::services::quote_normalizer::QuoteNormalizer>,
    ) -> Self {
        Self {
            venue_registry,
            ranking_strategy,
            config,
            quote_normalizer: Some(quote_normalizer),
        }
    }

    /// Collects quotes from all venues and ranks them.
    ///
    /// # Arguments
    ///
    /// * `rfq` - The RFQ to collect quotes for
    ///
    /// # Returns
    ///
    /// An aggregation result with ranked quotes.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - No venues are available
    /// - Overall timeout is exceeded
    /// - Insufficient quotes are collected
    pub async fn collect_and_rank(&self, rfq: &Rfq) -> AggregationResultType<AggregationResult> {
        // Get available venues
        let venues = self.venue_registry.get_available_venues().await;
        let venues_queried = venues.len();

        if venues.is_empty() {
            return Err(AggregationError::NoVenuesAvailable);
        }

        // Collect quotes with overall timeout
        let overall_timeout = Duration::from_millis(self.config.timeout_ms);
        let collection_result = timeout(overall_timeout, self.collect_from_venues(rfq)).await;

        let (quotes, errors) = match collection_result {
            Ok(result) => result,
            Err(_) => return Err(AggregationError::Timeout),
        };

        let total_collected = quotes.len();
        let venues_responded = venues_queried - errors.len();

        // Filter expired quotes
        let valid_quotes: Vec<Quote> = quotes.into_iter().filter(|q| !q.is_expired()).collect();
        let filtered_count = total_collected - valid_quotes.len();

        // Check if all venues failed
        if valid_quotes.is_empty() && !errors.is_empty() {
            return Err(AggregationError::AllVenuesFailed(errors));
        }

        // Check minimum quotes requirement
        if valid_quotes.len() < self.config.min_quotes {
            return Err(AggregationError::InsufficientQuotes {
                collected: valid_quotes.len(),
                required: self.config.min_quotes,
            });
        }

        // Normalize quotes if normalizer is configured
        let quotes_to_rank = if let Some(normalizer) = &self.quote_normalizer {
            use crate::domain::entities::quote_normalizer::QuoteType;

            valid_quotes
                .iter()
                .map(|q| {
                    // Normalize with QuoteType::Firm (default for aggregation)
                    let normalized = normalizer.normalize(q, QuoteType::Firm, None);
                    normalized.to_quote()
                })
                .collect()
        } else {
            valid_quotes
        };

        // Rank quotes
        let mut ranked_quotes = self.ranking_strategy.rank(&quotes_to_rank, rfq.side());

        // Apply max quotes limit
        if let Some(max) = self.config.max_quotes {
            ranked_quotes.truncate(max);
        }

        Ok(AggregationResult {
            ranked_quotes,
            total_collected,
            venues_queried,
            venues_responded,
            filtered_count,
        })
    }

    /// Collects quotes from all venues concurrently.
    async fn collect_from_venues(&self, rfq: &Rfq) -> (Vec<Quote>, Vec<String>) {
        let venues = self.venue_registry.get_available_venues().await;
        let mut handles = Vec::with_capacity(venues.len());

        for venue in venues {
            let rfq_clone = rfq.clone();
            let per_venue_timeout = Duration::from_millis(self.config.per_venue_timeout_ms);

            let handle = tokio::spawn(async move {
                match timeout(per_venue_timeout, venue.request_quote(&rfq_clone)).await {
                    Ok(Ok(quote)) => Ok(quote),
                    Ok(Err(e)) => Err(format_venue_error(&e)),
                    Err(_) => Err("venue request timed out".to_string()),
                }
            });

            handles.push(handle);
        }

        // Collect results
        let mut quotes = Vec::new();
        let mut errors = Vec::new();

        for handle in handles {
            match handle.await {
                Ok(Ok(quote)) => quotes.push(quote),
                Ok(Err(e)) => errors.push(e),
                Err(e) => errors.push(format!("task panicked: {}", e)),
            }
        }

        (quotes, errors)
    }

    /// Returns the current configuration.
    #[must_use]
    pub fn config(&self) -> &AggregationConfig {
        &self.config
    }

    /// Returns the ranking strategy name.
    #[must_use]
    pub fn ranking_strategy_name(&self) -> &'static str {
        self.ranking_strategy.name()
    }
}

/// Formats a venue error for display.
fn format_venue_error(error: &VenueError) -> String {
    error.to_string()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::application::services::ranking_strategy::BestPriceStrategy;
    use crate::domain::entities::rfq::RfqBuilder;
    use crate::domain::value_objects::enums::{AssetClass, SettlementMethod};
    use crate::domain::value_objects::symbol::Symbol;
    use crate::domain::value_objects::timestamp::Timestamp;
    use crate::domain::value_objects::{
        CounterpartyId, Instrument, OrderSide, Price, Quantity, VenueId,
    };
    use crate::infrastructure::venues::error::VenueResult;
    use crate::infrastructure::venues::traits::{ExecutionResult, VenueAdapter, VenueHealth};
    use async_trait::async_trait;
    use rust_decimal::prelude::ToPrimitive;
    use std::sync::Mutex;

    #[derive(Debug)]
    struct MockVenueAdapter {
        venue_id: VenueId,
        quote_result: Mutex<Option<Result<Quote, VenueError>>>,
        delay_ms: u64,
    }

    impl MockVenueAdapter {
        fn successful(
            venue_id: &str,
            rfq_id: crate::domain::value_objects::RfqId,
            price: f64,
        ) -> Self {
            let quote = Quote::new(
                rfq_id,
                VenueId::new(venue_id),
                Price::new(price).unwrap(),
                Quantity::new(1.0).unwrap(),
                Timestamp::now().add_secs(60),
            )
            .unwrap();
            Self {
                venue_id: VenueId::new(venue_id),
                quote_result: Mutex::new(Some(Ok(quote))),
                delay_ms: 0,
            }
        }

        fn failing(venue_id: &str) -> Self {
            Self {
                venue_id: VenueId::new(venue_id),
                quote_result: Mutex::new(Some(Err(VenueError::QuoteUnavailable {
                    message: "no liquidity".to_string(),
                }))),
                delay_ms: 0,
            }
        }

        fn slow(venue_id: &str, delay_ms: u64) -> Self {
            Self {
                venue_id: VenueId::new(venue_id),
                quote_result: Mutex::new(None),
                delay_ms,
            }
        }
    }

    #[async_trait]
    impl VenueAdapter for MockVenueAdapter {
        fn venue_id(&self) -> &VenueId {
            &self.venue_id
        }

        fn timeout_ms(&self) -> u64 {
            1000
        }

        async fn request_quote(&self, _rfq: &Rfq) -> VenueResult<Quote> {
            if self.delay_ms > 0 {
                tokio::time::sleep(Duration::from_millis(self.delay_ms)).await;
            }

            self.quote_result
                .lock()
                .unwrap()
                .take()
                .unwrap_or(Err(VenueError::QuoteUnavailable {
                    message: "no result set".to_string(),
                }))
        }

        async fn execute_trade(&self, _quote: &Quote) -> VenueResult<ExecutionResult> {
            unimplemented!()
        }

        async fn health_check(&self) -> VenueResult<VenueHealth> {
            Ok(VenueHealth::healthy(self.venue_id.clone()))
        }
    }

    #[derive(Debug)]
    struct MockVenueRegistry {
        venues: Vec<Arc<dyn VenueAdapter>>,
    }

    impl MockVenueRegistry {
        fn with_venues(venues: Vec<Arc<dyn VenueAdapter>>) -> Self {
            Self { venues }
        }

        fn empty() -> Self {
            Self { venues: vec![] }
        }
    }

    #[async_trait]
    impl VenueRegistry for MockVenueRegistry {
        async fn get_available_venues(
            &self,
        ) -> Vec<Arc<dyn crate::infrastructure::venues::traits::VenueAdapter>> {
            self.venues.clone()
        }

        async fn get_venue(
            &self,
            venue_id: &VenueId,
        ) -> Option<Arc<dyn crate::infrastructure::venues::traits::VenueAdapter>> {
            self.venues
                .iter()
                .find(|v| v.venue_id() == venue_id)
                .cloned()
        }
    }

    fn create_test_rfq() -> Rfq {
        let symbol = Symbol::new("BTC/USD").unwrap();
        let instrument =
            Instrument::new(symbol, AssetClass::CryptoSpot, SettlementMethod::default());
        let quantity = Quantity::new(1.0).unwrap();
        let expires_at = Timestamp::now().add_secs(300);

        RfqBuilder::new(
            CounterpartyId::new("client-1"),
            instrument,
            OrderSide::Buy,
            quantity,
            expires_at,
        )
        .build()
    }

    #[tokio::test]
    async fn collect_and_rank_success() {
        let rfq = create_test_rfq();
        let rfq_id = rfq.id();

        let venues: Vec<Arc<dyn VenueAdapter>> = vec![
            Arc::new(MockVenueAdapter::successful("venue-1", rfq_id, 100.0)),
            Arc::new(MockVenueAdapter::successful("venue-2", rfq_id, 95.0)),
            Arc::new(MockVenueAdapter::successful("venue-3", rfq_id, 105.0)),
        ];

        let engine = QuoteAggregationEngine::new(
            Arc::new(MockVenueRegistry::with_venues(venues)),
            Arc::new(BestPriceStrategy::new()),
            AggregationConfig::with_timeout(5000),
        );

        let result = engine.collect_and_rank(&rfq).await;
        assert!(result.is_ok());

        let agg_result = result.unwrap();
        assert_eq!(agg_result.ranked_quotes.len(), 3);
        assert_eq!(agg_result.venues_queried, 3);
        assert!(agg_result.best_quote().is_some());

        // Best quote should be lowest price for buy side
        let best = agg_result.best_quote().unwrap();
        assert!((best.quote.price().get().to_f64().unwrap() - 95.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn collect_and_rank_no_venues() {
        let rfq = create_test_rfq();

        let engine = QuoteAggregationEngine::new(
            Arc::new(MockVenueRegistry::empty()),
            Arc::new(BestPriceStrategy::new()),
            AggregationConfig::default(),
        );

        let result = engine.collect_and_rank(&rfq).await;
        assert!(matches!(result, Err(AggregationError::NoVenuesAvailable)));
    }

    #[tokio::test]
    async fn collect_and_rank_all_fail() {
        let rfq = create_test_rfq();

        let venues: Vec<Arc<dyn VenueAdapter>> = vec![
            Arc::new(MockVenueAdapter::failing("venue-1")),
            Arc::new(MockVenueAdapter::failing("venue-2")),
        ];

        let engine = QuoteAggregationEngine::new(
            Arc::new(MockVenueRegistry::with_venues(venues)),
            Arc::new(BestPriceStrategy::new()),
            AggregationConfig::with_timeout(5000).with_min_quotes(1),
        );

        let result = engine.collect_and_rank(&rfq).await;
        assert!(matches!(result, Err(AggregationError::AllVenuesFailed(_))));
    }

    #[tokio::test]
    async fn collect_and_rank_partial_failure() {
        let rfq = create_test_rfq();
        let rfq_id = rfq.id();

        let venues: Vec<Arc<dyn VenueAdapter>> = vec![
            Arc::new(MockVenueAdapter::successful("venue-1", rfq_id, 100.0)),
            Arc::new(MockVenueAdapter::failing("venue-2")),
        ];

        let engine = QuoteAggregationEngine::new(
            Arc::new(MockVenueRegistry::with_venues(venues)),
            Arc::new(BestPriceStrategy::new()),
            AggregationConfig::with_timeout(5000).with_min_quotes(1),
        );

        let result = engine.collect_and_rank(&rfq).await;
        assert!(result.is_ok());

        let agg_result = result.unwrap();
        assert_eq!(agg_result.ranked_quotes.len(), 1);
    }

    #[tokio::test]
    async fn collect_and_rank_timeout() {
        let rfq = create_test_rfq();

        let venues: Vec<Arc<dyn VenueAdapter>> =
            vec![Arc::new(MockVenueAdapter::slow("venue-1", 500))];

        let engine = QuoteAggregationEngine::new(
            Arc::new(MockVenueRegistry::with_venues(venues)),
            Arc::new(BestPriceStrategy::new()),
            AggregationConfig::with_timeout(50),
        );

        let result = engine.collect_and_rank(&rfq).await;
        assert!(matches!(result, Err(AggregationError::Timeout)));
    }

    #[tokio::test]
    async fn collect_and_rank_max_quotes() {
        let rfq = create_test_rfq();
        let rfq_id = rfq.id();

        let venues: Vec<Arc<dyn VenueAdapter>> = vec![
            Arc::new(MockVenueAdapter::successful("venue-1", rfq_id, 100.0)),
            Arc::new(MockVenueAdapter::successful("venue-2", rfq_id, 95.0)),
            Arc::new(MockVenueAdapter::successful("venue-3", rfq_id, 105.0)),
        ];

        let engine = QuoteAggregationEngine::new(
            Arc::new(MockVenueRegistry::with_venues(venues)),
            Arc::new(BestPriceStrategy::new()),
            AggregationConfig::with_timeout(5000).with_max_quotes(2),
        );

        let result = engine.collect_and_rank(&rfq).await;
        assert!(result.is_ok());

        let agg_result = result.unwrap();
        assert_eq!(agg_result.ranked_quotes.len(), 2);
    }

    #[test]
    fn aggregation_config_default() {
        let config = AggregationConfig::default();
        assert_eq!(config.timeout_ms, 10000);
        assert_eq!(config.min_quotes, 1);
        assert!(config.max_quotes.is_none());
    }

    #[test]
    fn aggregation_config_builder() {
        let config = AggregationConfig::with_timeout(5000)
            .with_min_quotes(2)
            .with_max_quotes(10)
            .with_per_venue_timeout(3000);

        assert_eq!(config.timeout_ms, 5000);
        assert_eq!(config.min_quotes, 2);
        assert_eq!(config.max_quotes, Some(10));
        assert_eq!(config.per_venue_timeout_ms, 3000);
    }

    #[test]
    fn aggregation_error_display() {
        let err = AggregationError::NoVenuesAvailable;
        assert_eq!(err.to_string(), "no venues available");

        let err = AggregationError::InsufficientQuotes {
            collected: 1,
            required: 3,
        };
        assert!(err.to_string().contains("1"));
        assert!(err.to_string().contains("3"));

        let err = AggregationError::Timeout;
        assert!(err.to_string().contains("timed out"));
    }

    #[test]
    fn aggregation_result_has_sufficient_quotes() {
        let result = AggregationResult {
            ranked_quotes: vec![],
            total_collected: 0,
            venues_queried: 2,
            venues_responded: 0,
            filtered_count: 0,
        };

        assert!(!result.has_sufficient_quotes(1));
        assert!(result.has_sufficient_quotes(0));
    }
}
