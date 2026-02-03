//! # Collect Quotes Use Case
//!
//! Use case for gathering quotes from venues.
//!
//! This use case orchestrates concurrent quote collection from multiple venues,
//! handling timeouts, partial failures, and state management.

use crate::application::error::{ApplicationError, ApplicationResult};
use crate::application::use_cases::create_rfq::RfqRepository;
use crate::domain::entities::quote::Quote;
use crate::domain::entities::rfq::Rfq;
use crate::domain::events::rfq_events::QuoteReceived;
use crate::domain::value_objects::{RfqId, VenueId};
use crate::infrastructure::venues::error::VenueError;
use crate::infrastructure::venues::traits::VenueAdapter;
use async_trait::async_trait;
use std::fmt;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;

/// Publisher for quote-related domain events.
#[async_trait]
pub trait QuoteEventPublisher: Send + Sync + fmt::Debug {
    /// Publishes a QuoteReceived event.
    ///
    /// # Errors
    ///
    /// Returns an error if publishing fails.
    async fn publish_quote_received(&self, event: QuoteReceived) -> Result<(), String>;
}

/// Registry for available venues.
#[async_trait]
pub trait VenueRegistry: Send + Sync + fmt::Debug {
    /// Returns all available venue adapters.
    async fn get_available_venues(&self) -> Vec<Arc<dyn VenueAdapter>>;

    /// Returns a specific venue adapter by ID.
    async fn get_venue(&self, venue_id: &VenueId) -> Option<Arc<dyn VenueAdapter>>;
}

/// Result of a quote collection attempt from a single venue.
#[derive(Debug)]
pub struct VenueQuoteResult {
    /// The venue ID.
    pub venue_id: VenueId,
    /// The quote if successful.
    pub quote: Option<Quote>,
    /// The error if failed.
    pub error: Option<String>,
}

impl VenueQuoteResult {
    /// Creates a successful result.
    #[must_use]
    pub fn success(venue_id: VenueId, quote: Quote) -> Self {
        Self {
            venue_id,
            quote: Some(quote),
            error: None,
        }
    }

    /// Creates a failed result.
    #[must_use]
    pub fn failure(venue_id: VenueId, error: impl Into<String>) -> Self {
        Self {
            venue_id,
            quote: None,
            error: Some(error.into()),
        }
    }

    /// Returns true if the result is successful.
    #[must_use]
    pub fn is_success(&self) -> bool {
        self.quote.is_some()
    }
}

/// Response from the collect quotes use case.
#[derive(Debug)]
pub struct CollectQuotesResponse {
    /// The RFQ ID.
    pub rfq_id: RfqId,
    /// Quotes successfully collected.
    pub quotes: Vec<Quote>,
    /// Venues that failed to provide quotes.
    pub failures: Vec<VenueQuoteResult>,
    /// Total venues queried.
    pub venues_queried: usize,
}

impl CollectQuotesResponse {
    /// Returns the number of successful quotes.
    #[must_use]
    pub fn success_count(&self) -> usize {
        self.quotes.len()
    }

    /// Returns the number of failures.
    #[must_use]
    pub fn failure_count(&self) -> usize {
        self.failures.len()
    }

    /// Returns true if at least one quote was collected.
    #[must_use]
    pub fn has_quotes(&self) -> bool {
        !self.quotes.is_empty()
    }
}

/// Configuration for quote collection.
#[derive(Debug, Clone)]
pub struct CollectQuotesConfig {
    /// Default timeout per venue in milliseconds.
    pub default_timeout_ms: u64,
    /// Minimum number of quotes required (0 = any).
    pub min_quotes: usize,
}

impl Default for CollectQuotesConfig {
    fn default() -> Self {
        Self {
            default_timeout_ms: 5000,
            min_quotes: 0,
        }
    }
}

impl CollectQuotesConfig {
    /// Creates a new configuration with the specified timeout.
    #[must_use]
    pub fn with_timeout(timeout_ms: u64) -> Self {
        Self {
            default_timeout_ms: timeout_ms,
            ..Default::default()
        }
    }

    /// Sets the minimum number of quotes required.
    #[must_use]
    pub fn with_min_quotes(mut self, min: usize) -> Self {
        self.min_quotes = min;
        self
    }
}

/// Use case for collecting quotes from multiple venues.
#[derive(Debug)]
pub struct CollectQuotesUseCase {
    rfq_repository: Arc<dyn RfqRepository>,
    event_publisher: Arc<dyn QuoteEventPublisher>,
    venue_registry: Arc<dyn VenueRegistry>,
    config: CollectQuotesConfig,
}

impl CollectQuotesUseCase {
    /// Creates a new CollectQuotesUseCase with all dependencies.
    #[must_use]
    pub fn new(
        rfq_repository: Arc<dyn RfqRepository>,
        event_publisher: Arc<dyn QuoteEventPublisher>,
        venue_registry: Arc<dyn VenueRegistry>,
        config: CollectQuotesConfig,
    ) -> Self {
        Self {
            rfq_repository,
            event_publisher,
            venue_registry,
            config,
        }
    }

    /// Creates a new CollectQuotesUseCase with default configuration.
    #[must_use]
    pub fn with_defaults(
        rfq_repository: Arc<dyn RfqRepository>,
        event_publisher: Arc<dyn QuoteEventPublisher>,
        venue_registry: Arc<dyn VenueRegistry>,
    ) -> Self {
        Self::new(
            rfq_repository,
            event_publisher,
            venue_registry,
            CollectQuotesConfig::default(),
        )
    }

    /// Executes the collect quotes use case.
    ///
    /// # Arguments
    ///
    /// * `rfq_id` - The RFQ to collect quotes for
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - RFQ is not found
    /// - RFQ is not in the correct state
    /// - No venues are available
    /// - All venues fail and min_quotes > 0
    /// - Persistence fails
    pub async fn execute(&self, rfq_id: RfqId) -> ApplicationResult<CollectQuotesResponse> {
        // 1. Load RFQ from repository
        let mut rfq = self
            .rfq_repository
            .find_by_id(rfq_id)
            .await
            .map_err(ApplicationError::repository)?
            .ok_or_else(|| ApplicationError::validation(format!("RFQ not found: {}", rfq_id)))?;

        // 2. Validate RFQ state and start quote collection
        rfq.start_quote_collection()
            .map_err(ApplicationError::from)?;

        // 3. Get available venues
        let venues = self.venue_registry.get_available_venues().await;
        let venues_queried = venues.len();

        if venues.is_empty() {
            return Err(ApplicationError::validation("no venues available"));
        }

        // 4. Fan-out concurrent requests to all venues
        let results = self.collect_quotes_from_venues(&rfq, venues).await;

        // 5. Separate successes and failures
        let (quotes, failures): (Vec<_>, Vec<_>) =
            results.into_iter().partition(|r| r.is_success());

        let successful_quotes: Vec<Quote> = quotes.into_iter().filter_map(|r| r.quote).collect();

        // 6. Check minimum quotes requirement
        if successful_quotes.len() < self.config.min_quotes {
            return Err(ApplicationError::validation(format!(
                "insufficient quotes: got {}, need {}",
                successful_quotes.len(),
                self.config.min_quotes
            )));
        }

        // 7. Add quotes to RFQ
        for quote in &successful_quotes {
            if let Err(e) = rfq.receive_quote(quote.clone()) {
                tracing::warn!("Failed to add quote to RFQ: {}", e);
            }
        }

        // 8. Persist updated RFQ
        self.rfq_repository
            .save(&rfq)
            .await
            .map_err(ApplicationError::repository)?;

        // 9. Publish QuoteReceived events
        for quote in &successful_quotes {
            let event = QuoteReceived::new(
                rfq_id,
                quote.id(),
                quote.venue_id().clone(),
                quote.price(),
                quote.quantity(),
                quote.valid_until(),
            );

            if let Err(e) = self.event_publisher.publish_quote_received(event).await {
                tracing::warn!("Failed to publish QuoteReceived event: {}", e);
            }
        }

        // 10. Return response
        Ok(CollectQuotesResponse {
            rfq_id,
            quotes: successful_quotes,
            failures,
            venues_queried,
        })
    }

    /// Collects quotes from all venues concurrently.
    async fn collect_quotes_from_venues(
        &self,
        rfq: &Rfq,
        venues: Vec<Arc<dyn VenueAdapter>>,
    ) -> Vec<VenueQuoteResult> {
        let mut handles = Vec::with_capacity(venues.len());

        for venue in venues {
            let rfq_clone = rfq.clone();
            let timeout_ms = if venue.timeout_ms() > 0 {
                venue.timeout_ms().min(self.config.default_timeout_ms)
            } else {
                self.config.default_timeout_ms
            };

            let handle = tokio::spawn(async move {
                let venue_id = venue.venue_id().clone();
                let duration = Duration::from_millis(timeout_ms);

                match timeout(duration, venue.request_quote(&rfq_clone)).await {
                    Ok(Ok(quote)) => VenueQuoteResult::success(venue_id, quote),
                    Ok(Err(e)) => VenueQuoteResult::failure(venue_id, format_venue_error(&e)),
                    Err(_) => VenueQuoteResult::failure(venue_id, "request timed out"),
                }
            });

            handles.push(handle);
        }

        // Collect all results
        let mut results = Vec::with_capacity(handles.len());
        for handle in handles {
            match handle.await {
                Ok(result) => results.push(result),
                Err(e) => {
                    tracing::error!("Task panicked: {}", e);
                }
            }
        }

        results
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
    use crate::application::use_cases::create_rfq::RfqRepository;
    use crate::domain::entities::rfq::RfqBuilder;
    use crate::domain::value_objects::enums::{AssetClass, SettlementMethod};
    use crate::domain::value_objects::symbol::Symbol;
    use crate::domain::value_objects::timestamp::Timestamp;
    use crate::domain::value_objects::{CounterpartyId, Instrument, OrderSide, Price, Quantity};
    use crate::infrastructure::venues::error::VenueResult;
    use crate::infrastructure::venues::traits::ExecutionResult;
    use std::collections::HashMap;
    use std::sync::Mutex;

    #[derive(Debug, Default)]
    struct MockRfqRepository {
        rfqs: Mutex<HashMap<RfqId, Rfq>>,
    }

    impl MockRfqRepository {
        fn with_rfq(rfq: Rfq) -> Self {
            let repo = Self::default();
            repo.rfqs.lock().unwrap().insert(rfq.id(), rfq);
            repo
        }
    }

    #[async_trait]
    impl RfqRepository for MockRfqRepository {
        async fn save(&self, rfq: &Rfq) -> Result<(), String> {
            self.rfqs.lock().unwrap().insert(rfq.id(), rfq.clone());
            Ok(())
        }

        async fn find_by_id(&self, id: RfqId) -> Result<Option<Rfq>, String> {
            Ok(self.rfqs.lock().unwrap().get(&id).cloned())
        }
    }

    #[derive(Debug, Default)]
    struct MockQuoteEventPublisher {
        events: Mutex<Vec<QuoteReceived>>,
    }

    #[async_trait]
    impl QuoteEventPublisher for MockQuoteEventPublisher {
        async fn publish_quote_received(&self, event: QuoteReceived) -> Result<(), String> {
            self.events.lock().unwrap().push(event);
            Ok(())
        }
    }

    #[derive(Debug)]
    struct MockVenueAdapter {
        venue_id: VenueId,
        quote_result: Mutex<Option<Result<Quote, VenueError>>>,
        delay_ms: u64,
    }

    impl MockVenueAdapter {
        fn successful(venue_id: &str, rfq_id: RfqId) -> Self {
            let quote = create_test_quote(rfq_id, venue_id);
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

        async fn health_check(
            &self,
        ) -> VenueResult<crate::infrastructure::venues::traits::VenueHealth> {
            Ok(crate::infrastructure::venues::traits::VenueHealth::healthy(
                self.venue_id.clone(),
            ))
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
        async fn get_available_venues(&self) -> Vec<Arc<dyn VenueAdapter>> {
            self.venues.clone()
        }

        async fn get_venue(&self, venue_id: &VenueId) -> Option<Arc<dyn VenueAdapter>> {
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

    fn create_test_quote(rfq_id: RfqId, venue_id: &str) -> Quote {
        Quote::new(
            rfq_id,
            VenueId::new(venue_id),
            Price::new(50000.0).unwrap(),
            Quantity::new(1.0).unwrap(),
            Timestamp::now().add_secs(60),
        )
        .unwrap()
    }

    fn create_use_case(
        rfq_repo: impl RfqRepository + 'static,
        venue_registry: impl VenueRegistry + 'static,
    ) -> CollectQuotesUseCase {
        CollectQuotesUseCase::new(
            Arc::new(rfq_repo),
            Arc::new(MockQuoteEventPublisher::default()),
            Arc::new(venue_registry),
            CollectQuotesConfig::with_timeout(100),
        )
    }

    #[tokio::test]
    async fn execute_success_multiple_venues() {
        let rfq = create_test_rfq();
        let rfq_id = rfq.id();

        let venues: Vec<Arc<dyn VenueAdapter>> = vec![
            Arc::new(MockVenueAdapter::successful("venue-1", rfq_id)),
            Arc::new(MockVenueAdapter::successful("venue-2", rfq_id)),
        ];

        let use_case = create_use_case(
            MockRfqRepository::with_rfq(rfq),
            MockVenueRegistry::with_venues(venues),
        );

        let result = use_case.execute(rfq_id).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        assert_eq!(response.rfq_id, rfq_id);
        assert_eq!(response.success_count(), 2);
        assert_eq!(response.failure_count(), 0);
        assert!(response.has_quotes());
    }

    #[tokio::test]
    async fn execute_partial_failure() {
        let rfq = create_test_rfq();
        let rfq_id = rfq.id();

        let venues: Vec<Arc<dyn VenueAdapter>> = vec![
            Arc::new(MockVenueAdapter::successful("venue-1", rfq_id)),
            Arc::new(MockVenueAdapter::failing("venue-2")),
        ];

        let use_case = create_use_case(
            MockRfqRepository::with_rfq(rfq),
            MockVenueRegistry::with_venues(venues),
        );

        let result = use_case.execute(rfq_id).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        assert_eq!(response.success_count(), 1);
        assert_eq!(response.failure_count(), 1);
    }

    #[tokio::test]
    async fn execute_all_venues_fail() {
        let rfq = create_test_rfq();
        let rfq_id = rfq.id();

        let venues: Vec<Arc<dyn VenueAdapter>> = vec![
            Arc::new(MockVenueAdapter::failing("venue-1")),
            Arc::new(MockVenueAdapter::failing("venue-2")),
        ];

        let use_case = create_use_case(
            MockRfqRepository::with_rfq(rfq),
            MockVenueRegistry::with_venues(venues),
        );

        let result = use_case.execute(rfq_id).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        assert_eq!(response.success_count(), 0);
        assert_eq!(response.failure_count(), 2);
        assert!(!response.has_quotes());
    }

    #[tokio::test]
    async fn execute_timeout_handling() {
        let rfq = create_test_rfq();
        let rfq_id = rfq.id();

        let venues: Vec<Arc<dyn VenueAdapter>> =
            vec![Arc::new(MockVenueAdapter::slow("venue-1", 500))];

        let use_case = CollectQuotesUseCase::new(
            Arc::new(MockRfqRepository::with_rfq(rfq)),
            Arc::new(MockQuoteEventPublisher::default()),
            Arc::new(MockVenueRegistry::with_venues(venues)),
            CollectQuotesConfig::with_timeout(50),
        );

        let result = use_case.execute(rfq_id).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        assert_eq!(response.success_count(), 0);
        assert_eq!(response.failure_count(), 1);
        assert!(
            response
                .failures
                .first()
                .and_then(|f| f.error.as_ref())
                .map(|e| e.contains("timed out"))
                .unwrap_or(false)
        );
    }

    #[tokio::test]
    async fn execute_rfq_not_found() {
        let use_case = create_use_case(MockRfqRepository::default(), MockVenueRegistry::empty());

        let result = use_case.execute(RfqId::new_v4()).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ApplicationError::Validation(_)
        ));
    }

    #[tokio::test]
    async fn execute_no_venues_available() {
        let rfq = create_test_rfq();
        let rfq_id = rfq.id();

        let use_case =
            create_use_case(MockRfqRepository::with_rfq(rfq), MockVenueRegistry::empty());

        let result = use_case.execute(rfq_id).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ApplicationError::Validation(_)
        ));
    }

    #[tokio::test]
    async fn execute_min_quotes_not_met() {
        let rfq = create_test_rfq();
        let rfq_id = rfq.id();

        let venues: Vec<Arc<dyn VenueAdapter>> =
            vec![Arc::new(MockVenueAdapter::failing("venue-1"))];

        let use_case = CollectQuotesUseCase::new(
            Arc::new(MockRfqRepository::with_rfq(rfq)),
            Arc::new(MockQuoteEventPublisher::default()),
            Arc::new(MockVenueRegistry::with_venues(venues)),
            CollectQuotesConfig::with_timeout(100).with_min_quotes(1),
        );

        let result = use_case.execute(rfq_id).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ApplicationError::Validation(_)
        ));
    }

    #[test]
    fn venue_quote_result_success() {
        let venue_id = VenueId::new("test");
        let quote = create_test_quote(RfqId::new_v4(), "test");
        let result = VenueQuoteResult::success(venue_id, quote);
        assert!(result.is_success());
    }

    #[test]
    fn venue_quote_result_failure() {
        let venue_id = VenueId::new("test");
        let result = VenueQuoteResult::failure(venue_id, "error");
        assert!(!result.is_success());
    }

    #[test]
    fn collect_quotes_config_default() {
        let config = CollectQuotesConfig::default();
        assert_eq!(config.default_timeout_ms, 5000);
        assert_eq!(config.min_quotes, 0);
    }

    #[test]
    fn collect_quotes_config_with_timeout() {
        let config = CollectQuotesConfig::with_timeout(1000);
        assert_eq!(config.default_timeout_ms, 1000);
    }

    #[test]
    fn collect_quotes_config_with_min_quotes() {
        let config = CollectQuotesConfig::default().with_min_quotes(3);
        assert_eq!(config.min_quotes, 3);
    }
}
