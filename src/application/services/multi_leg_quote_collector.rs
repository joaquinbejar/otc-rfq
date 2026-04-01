//! # Multi-Leg Quote Collector
//!
//! Collects package quotes from multiple venues for multi-leg strategies.
//!
//! This service coordinates quote collection from venues that support multi-leg
//! quoting, with fallback to individual leg quotes for venues that don't.
//!
//! # Examples
//!
//! ```ignore
//! use otc_rfq::application::services::multi_leg_quote_collector::MultiLegQuoteCollector;
//!
//! let collector = MultiLegQuoteCollector::new(venues, Duration::from_secs(5));
//! let quotes = collector.collect_quotes(&strategy, &rfq).await;
//! ```

use crate::domain::entities::package_quote::{LegPrice, PackageQuote};
use crate::domain::entities::rfq::Rfq;
use crate::domain::value_objects::strategy::Strategy;
use crate::domain::value_objects::{Timestamp, VenueId};
use crate::infrastructure::venues::traits::VenueAdapter;
use rust_decimal::Decimal;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;
use tracing::{debug, warn};

/// Default timeout for quote collection.
pub const DEFAULT_COLLECTION_TIMEOUT: Duration = Duration::from_secs(5);

/// Result of collecting quotes from a single venue.
#[derive(Debug)]
pub enum VenueQuoteResult {
    /// Successfully received a package quote.
    Success(PackageQuote),
    /// Venue doesn't support multi-leg and fallback was used.
    Fallback(PackageQuote),
    /// Failed to get a quote from this venue.
    Failed {
        /// The venue ID.
        venue_id: VenueId,
        /// The error message.
        reason: String,
    },
    /// Request timed out.
    Timeout {
        /// The venue ID.
        venue_id: VenueId,
    },
}

/// Collects package quotes from multiple venues for multi-leg strategies.
///
/// The collector sends quote requests to all configured venues concurrently,
/// using multi-leg quoting for venues that support it and falling back to
/// individual leg quotes for those that don't.
#[derive(Debug)]
pub struct MultiLegQuoteCollector<V: VenueAdapter> {
    /// The venues to collect quotes from.
    venues: Vec<Arc<V>>,
    /// Timeout for each venue request.
    timeout: Duration,
}

impl<V: VenueAdapter + 'static> MultiLegQuoteCollector<V> {
    /// Creates a new quote collector.
    ///
    /// # Arguments
    ///
    /// * `venues` - The venues to collect quotes from
    /// * `timeout` - Timeout for each venue request
    #[must_use]
    pub fn new(venues: Vec<Arc<V>>, timeout: Duration) -> Self {
        Self { venues, timeout }
    }

    /// Creates a new quote collector with the default timeout.
    #[must_use]
    pub fn with_default_timeout(venues: Vec<Arc<V>>) -> Self {
        Self::new(venues, DEFAULT_COLLECTION_TIMEOUT)
    }

    /// Returns the number of configured venues.
    #[must_use]
    pub fn venue_count(&self) -> usize {
        self.venues.len()
    }

    /// Returns the configured timeout.
    #[must_use]
    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    /// Collects package quotes from all venues.
    ///
    /// Sends quote requests to all venues concurrently and returns all
    /// successful quotes. Failed requests are logged but don't cause
    /// the entire collection to fail.
    ///
    /// # Arguments
    ///
    /// * `strategy` - The multi-leg strategy to quote
    /// * `rfq` - The RFQ containing quantity and other parameters
    ///
    /// # Returns
    ///
    /// A vector of package quotes from venues that responded successfully.
    pub async fn collect_quotes(&self, strategy: &Strategy, rfq: &Rfq) -> Vec<PackageQuote> {
        let futures: Vec<_> = self
            .venues
            .iter()
            .map(|venue| self.collect_from_venue(Arc::<V>::clone(venue), strategy, rfq))
            .collect();

        let results = futures::future::join_all(futures).await;

        results
            .into_iter()
            .filter_map(|result| match result {
                VenueQuoteResult::Success(quote) => Some(quote),
                VenueQuoteResult::Fallback(quote) => Some(quote),
                VenueQuoteResult::Failed { venue_id, reason } => {
                    warn!(%venue_id, %reason, "Failed to collect quote from venue");
                    None
                }
                VenueQuoteResult::Timeout { venue_id } => {
                    warn!(%venue_id, "Quote collection timed out");
                    None
                }
            })
            .collect()
    }

    /// Collects detailed results from all venues.
    ///
    /// Similar to `collect_quotes` but returns detailed results including
    /// failures and timeouts.
    pub async fn collect_with_details(
        &self,
        strategy: &Strategy,
        rfq: &Rfq,
    ) -> Vec<VenueQuoteResult> {
        let futures: Vec<_> = self
            .venues
            .iter()
            .map(|venue| self.collect_from_venue(Arc::<V>::clone(venue), strategy, rfq))
            .collect();

        futures::future::join_all(futures).await
    }

    /// Collects a quote from a single venue.
    async fn collect_from_venue(
        &self,
        venue: Arc<V>,
        strategy: &Strategy,
        rfq: &Rfq,
    ) -> VenueQuoteResult {
        let venue_id = venue.venue_id().clone();

        let result = timeout(
            self.timeout,
            self.request_quote_from_venue(&venue, strategy, rfq),
        )
        .await;

        match result {
            Ok(inner_result) => inner_result,
            Err(_) => VenueQuoteResult::Timeout { venue_id },
        }
    }

    /// Requests a quote from a venue, using multi-leg or fallback as appropriate.
    async fn request_quote_from_venue(
        &self,
        venue: &V,
        strategy: &Strategy,
        rfq: &Rfq,
    ) -> VenueQuoteResult {
        let venue_id = venue.venue_id().clone();

        if venue.supports_multi_leg() {
            debug!(%venue_id, "Requesting multi-leg quote");
            match venue.request_multi_leg_quote(strategy, rfq).await {
                Ok(quote) => VenueQuoteResult::Success(quote),
                Err(e) => {
                    debug!(%venue_id, error = %e, "Multi-leg quote failed, trying fallback");
                    self.fallback_to_individual_legs(venue, strategy, rfq).await
                }
            }
        } else {
            debug!(%venue_id, "Venue doesn't support multi-leg, using fallback");
            self.fallback_to_individual_legs(venue, strategy, rfq).await
        }
    }

    /// Falls back to requesting individual leg quotes and synthesizing a package quote.
    async fn fallback_to_individual_legs(
        &self,
        venue: &V,
        strategy: &Strategy,
        rfq: &Rfq,
    ) -> VenueQuoteResult {
        let venue_id = venue.venue_id().clone();
        let mut leg_prices = Vec::with_capacity(strategy.leg_count());
        let mut net_price = Decimal::ZERO;

        // TODO(#14): This sequential loop could be parallelized with join_all for better latency.
        // TODO(#14): Each leg currently uses the same RFQ, which means all legs get the same price.
        //            A proper implementation should create single-leg RFQs per instrument.
        for leg in strategy.legs() {
            match venue.request_quote(rfq).await {
                Ok(quote) => {
                    let leg_price = LegPrice::from_parts(
                        leg.instrument().clone(),
                        leg.side(),
                        quote.price(),
                        rfq.quantity(),
                    );

                    // Calculate signed contribution to net price
                    if let Some(notional) = leg_price.notional() {
                        match leg.side() {
                            crate::domain::value_objects::OrderSide::Buy => {
                                net_price -= notional.get();
                            }
                            crate::domain::value_objects::OrderSide::Sell => {
                                net_price += notional.get();
                            }
                        }
                    }

                    leg_prices.push(leg_price);
                }
                Err(e) => {
                    return VenueQuoteResult::Failed {
                        venue_id,
                        reason: format!("Failed to get quote for leg: {}", e),
                    };
                }
            }
        }

        // Create synthetic package quote from individual legs
        match PackageQuote::new(
            rfq.id(),
            venue_id.clone(),
            strategy.clone(),
            net_price,
            leg_prices,
            Timestamp::now().add_secs(300), // 5 minute validity
        ) {
            Ok(quote) => VenueQuoteResult::Fallback(quote),
            Err(e) => VenueQuoteResult::Failed {
                venue_id,
                reason: format!("Failed to create synthetic package quote: {}", e),
            },
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::domain::entities::quote::{Quote, QuoteBuilder};
    use crate::domain::value_objects::strategy::{StrategyLeg, StrategyType};
    use crate::domain::value_objects::{
        AssetClass, CounterpartyId, Instrument, OrderSide, Price, Quantity, Symbol,
    };
    use crate::infrastructure::venues::error::{VenueError, VenueResult};
    use crate::infrastructure::venues::traits::{ExecutionResult, VenueHealth};
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicUsize, Ordering};

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

    fn test_rfq() -> Rfq {
        Rfq::new(
            CounterpartyId::new("test-client"),
            test_instrument(),
            OrderSide::Buy,
            Quantity::new(1.0).unwrap(),
            Timestamp::now().add_secs(300),
        )
        .unwrap()
    }

    #[derive(Debug)]
    struct MockVenue {
        id: VenueId,
        supports_multi_leg: bool,
        should_fail: bool,
        call_count: AtomicUsize,
    }

    impl MockVenue {
        fn new(id: &str, supports_multi_leg: bool) -> Self {
            Self {
                id: VenueId::new(id),
                supports_multi_leg,
                should_fail: false,
                call_count: AtomicUsize::new(0),
            }
        }

        fn failing(id: &str) -> Self {
            Self {
                id: VenueId::new(id),
                supports_multi_leg: false,
                should_fail: true,
                call_count: AtomicUsize::new(0),
            }
        }
    }

    #[async_trait]
    impl VenueAdapter for MockVenue {
        fn venue_id(&self) -> &VenueId {
            &self.id
        }

        fn timeout_ms(&self) -> u64 {
            5000
        }

        async fn request_quote(&self, rfq: &Rfq) -> VenueResult<Quote> {
            self.call_count.fetch_add(1, Ordering::SeqCst);

            if self.should_fail {
                return Err(VenueError::quote_unavailable("Mock failure"));
            }

            Ok(QuoteBuilder::new(
                rfq.id(),
                self.id.clone(),
                Price::new(50000.0).unwrap(),
                rfq.quantity(),
                Timestamp::now().add_secs(300),
            )
            .build())
        }

        async fn execute_trade(&self, _quote: &Quote) -> VenueResult<ExecutionResult> {
            unimplemented!()
        }

        async fn health_check(&self) -> VenueResult<VenueHealth> {
            Ok(VenueHealth::healthy(self.id.clone()))
        }

        fn supports_multi_leg(&self) -> bool {
            self.supports_multi_leg
        }

        async fn request_multi_leg_quote(
            &self,
            strategy: &Strategy,
            rfq: &Rfq,
        ) -> VenueResult<PackageQuote> {
            self.call_count.fetch_add(1, Ordering::SeqCst);

            if self.should_fail {
                return Err(VenueError::quote_unavailable("Mock failure"));
            }

            let inst = test_instrument();
            let leg_prices = vec![
                LegPrice::new(
                    inst.clone(),
                    OrderSide::Buy,
                    Price::new(50000.0).unwrap(),
                    rfq.quantity(),
                )
                .unwrap(),
                LegPrice::new(
                    inst,
                    OrderSide::Sell,
                    Price::new(50100.0).unwrap(),
                    rfq.quantity(),
                )
                .unwrap(),
            ];

            PackageQuote::new(
                rfq.id(),
                self.id.clone(),
                strategy.clone(),
                Decimal::new(100, 0), // Net credit of 100
                leg_prices,
                Timestamp::now().add_secs(300),
            )
            .map_err(|e| VenueError::internal_error(e.to_string()))
        }
    }

    #[tokio::test]
    async fn new_creates_collector() {
        let venues: Vec<Arc<MockVenue>> = vec![Arc::new(MockVenue::new("venue1", false))];
        let collector = MultiLegQuoteCollector::new(venues, Duration::from_secs(5));
        assert_eq!(collector.venue_count(), 1);
        assert_eq!(collector.timeout(), Duration::from_secs(5));
    }

    #[tokio::test]
    async fn collect_quotes_from_multi_leg_venue() {
        let venue = Arc::new(MockVenue::new("multi-leg-venue", true));
        let collector =
            MultiLegQuoteCollector::new(vec![Arc::clone(&venue)], Duration::from_secs(5));

        let strategy = test_strategy();
        let rfq = test_rfq();

        let quotes = collector.collect_quotes(&strategy, &rfq).await;

        assert_eq!(quotes.len(), 1);
        let first = quotes.first().expect("should have one quote");
        assert_eq!(first.venue_id().as_str(), "multi-leg-venue");
        assert_eq!(venue.call_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn collect_quotes_uses_fallback_for_non_multi_leg_venue() {
        let venue = Arc::new(MockVenue::new("single-leg-venue", false));
        let collector =
            MultiLegQuoteCollector::new(vec![Arc::clone(&venue)], Duration::from_secs(5));

        let strategy = test_strategy();
        let rfq = test_rfq();

        let quotes = collector.collect_quotes(&strategy, &rfq).await;

        assert_eq!(quotes.len(), 1);
        let first = quotes.first().expect("should have one quote");
        assert_eq!(first.venue_id().as_str(), "single-leg-venue");
        // Should have called request_quote for each leg
        assert!(venue.call_count.load(Ordering::SeqCst) >= 2);
    }

    #[tokio::test]
    async fn collect_quotes_handles_failures() {
        let failing_venue = Arc::new(MockVenue::failing("failing-venue"));
        let working_venue = Arc::new(MockVenue::new("working-venue", true));

        let collector =
            MultiLegQuoteCollector::new(vec![failing_venue, working_venue], Duration::from_secs(5));

        let strategy = test_strategy();
        let rfq = test_rfq();

        let quotes = collector.collect_quotes(&strategy, &rfq).await;

        // Only the working venue should return a quote
        assert_eq!(quotes.len(), 1);
        let first = quotes.first().expect("should have one quote");
        assert_eq!(first.venue_id().as_str(), "working-venue");
    }

    #[tokio::test]
    async fn collect_with_details_returns_all_results() {
        let failing_venue = Arc::new(MockVenue::failing("failing-venue"));
        let working_venue = Arc::new(MockVenue::new("working-venue", true));

        let collector =
            MultiLegQuoteCollector::new(vec![failing_venue, working_venue], Duration::from_secs(5));

        let strategy = test_strategy();
        let rfq = test_rfq();

        let results = collector.collect_with_details(&strategy, &rfq).await;

        assert_eq!(results.len(), 2);

        let success_count = results
            .iter()
            .filter(|r| matches!(r, VenueQuoteResult::Success(_)))
            .count();
        let failed_count = results
            .iter()
            .filter(|r| matches!(r, VenueQuoteResult::Failed { .. }))
            .count();

        assert_eq!(success_count, 1);
        assert_eq!(failed_count, 1);
    }
}
