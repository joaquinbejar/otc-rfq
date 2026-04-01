//! # Execute Trade Use Case
//!
//! Use case for executing trades against quotes.
//!
//! This module provides the [`ExecuteTradeUseCase`] which orchestrates
//! trade execution against a selected quote from a venue.

use crate::application::error::{ApplicationError, ApplicationResult};
use crate::application::use_cases::collect_quotes::VenueRegistry;
use crate::application::use_cases::create_rfq::RfqRepository;
use crate::domain::entities::rfq::Rfq;
use crate::domain::entities::trade::Trade;
use crate::domain::events::TradeExecuted;
use crate::domain::value_objects::{QuoteId, RfqId, TradeId, TradeParticipant};
use crate::infrastructure::venues::traits::ExecutionResult;
use async_trait::async_trait;
use std::fmt;
use std::sync::Arc;
use std::time::Instant;

/// Repository for persisting trades.
#[async_trait]
pub trait TradeRepository: Send + Sync + fmt::Debug {
    /// Saves a trade.
    async fn save(&self, trade: &Trade) -> ApplicationResult<()>;

    /// Finds a trade by ID.
    async fn find_by_id(&self, id: TradeId) -> ApplicationResult<Option<Trade>>;

    /// Finds trades by RFQ ID.
    async fn find_by_rfq_id(&self, rfq_id: RfqId) -> ApplicationResult<Vec<Trade>>;
}

/// Publisher for trade-related events.
#[async_trait]
pub trait TradeEventPublisher: Send + Sync + fmt::Debug {
    /// Publishes a trade executed event.
    async fn publish_trade_executed(&self, event: TradeExecuted) -> ApplicationResult<()>;

    /// Publishes an execution failed event.
    async fn publish_execution_failed(
        &self,
        rfq_id: RfqId,
        quote_id: QuoteId,
        reason: &str,
    ) -> ApplicationResult<()>;

    /// Publishes a position updated event.
    ///
    /// This event notifies the Position Manager to update positions,
    /// trigger Greeks recalculation, and compute margin impact.
    ///
    /// # Errors
    ///
    /// Returns an error if the event cannot be published.
    async fn publish_position_updated(
        &self,
        event: crate::domain::events::PositionUpdated,
    ) -> ApplicationResult<()>;
}

/// Request to execute a trade.
#[derive(Debug, Clone)]
pub struct ExecuteTradeRequest {
    /// The RFQ ID.
    pub rfq_id: RfqId,
    /// The quote ID to execute.
    pub quote_id: QuoteId,
}

impl ExecuteTradeRequest {
    /// Creates a new execute trade request.
    #[must_use]
    pub fn new(rfq_id: RfqId, quote_id: QuoteId) -> Self {
        Self { rfq_id, quote_id }
    }
}

/// Response from trade execution.
#[derive(Debug, Clone)]
pub struct ExecuteTradeResponse {
    /// The executed trade.
    pub trade: Trade,
    /// The RFQ ID.
    pub rfq_id: RfqId,
    /// Execution time in milliseconds.
    pub execution_time_ms: u64,
}

impl ExecuteTradeResponse {
    /// Creates a new execute trade response.
    #[must_use]
    pub fn new(trade: Trade, rfq_id: RfqId, execution_time_ms: u64) -> Self {
        Self {
            trade,
            rfq_id,
            execution_time_ms,
        }
    }

    /// Returns the trade ID.
    #[must_use]
    pub fn trade_id(&self) -> TradeId {
        self.trade.id()
    }
}

/// Use case for executing trades against quotes.
///
/// Orchestrates the trade execution workflow:
/// 1. Load RFQ and validate state
/// 2. Find and validate quote
/// 3. Get venue adapter
/// 4. Execute trade via venue
/// 5. Create Trade aggregate
/// 6. Update RFQ state
/// 7. Persist trade and RFQ
/// 8. Publish events
#[derive(Debug)]
pub struct ExecuteTradeUseCase {
    rfq_repository: Arc<dyn RfqRepository>,
    trade_repository: Arc<dyn TradeRepository>,
    event_publisher: Arc<dyn TradeEventPublisher>,
    venue_registry: Arc<dyn VenueRegistry>,
    confirmation_service: Option<Arc<dyn crate::domain::services::ConfirmationService>>,
    counterparty_repository:
        Option<Arc<dyn crate::infrastructure::persistence::traits::CounterpartyRepository>>,
}

impl ExecuteTradeUseCase {
    /// Creates a new ExecuteTradeUseCase.
    #[must_use]
    pub fn new(
        rfq_repository: Arc<dyn RfqRepository>,
        trade_repository: Arc<dyn TradeRepository>,
        event_publisher: Arc<dyn TradeEventPublisher>,
        venue_registry: Arc<dyn VenueRegistry>,
    ) -> Self {
        Self {
            rfq_repository,
            trade_repository,
            event_publisher,
            venue_registry,
            confirmation_service: None,
            counterparty_repository: None,
        }
    }

    /// Sets the confirmation service for multi-channel trade confirmations.
    #[must_use]
    pub fn with_confirmation_service(
        mut self,
        confirmation_service: Arc<dyn crate::domain::services::ConfirmationService>,
        counterparty_repository: Arc<
            dyn crate::infrastructure::persistence::traits::CounterpartyRepository,
        >,
    ) -> Self {
        self.confirmation_service = Some(confirmation_service);
        self.counterparty_repository = Some(counterparty_repository);
        self
    }

    /// Executes a trade for the given request.
    ///
    /// # Arguments
    ///
    /// * `request` - The execute trade request
    ///
    /// # Returns
    ///
    /// The execute trade response with the created trade.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - RFQ is not found
    /// - Quote is not found
    /// - Quote is expired
    /// - RFQ is in invalid state
    /// - Venue is not available
    /// - Execution fails
    pub async fn execute(
        &self,
        request: ExecuteTradeRequest,
    ) -> ApplicationResult<ExecuteTradeResponse> {
        let start = Instant::now();

        // Load RFQ
        let mut rfq = self
            .rfq_repository
            .find_by_id(request.rfq_id)
            .await
            .map_err(ApplicationError::RepositoryError)?
            .ok_or_else(|| ApplicationError::RfqNotFound(request.rfq_id.to_string()))?;

        // Find quote in RFQ
        let quote = rfq
            .quotes()
            .iter()
            .find(|q| q.id() == request.quote_id)
            .cloned()
            .ok_or_else(|| ApplicationError::QuoteNotFound(request.quote_id.to_string()))?;

        // Validate quote is not expired
        if quote.is_expired() {
            return Err(ApplicationError::QuoteExpired(request.quote_id.to_string()));
        }

        // Select quote and start execution
        rfq.select_quote(request.quote_id)
            .map_err(|e| ApplicationError::InvalidState(e.to_string()))?;

        rfq.start_execution()
            .map_err(|e| ApplicationError::InvalidState(e.to_string()))?;

        // Get venue adapter
        let venue_adapter = self
            .venue_registry
            .get_venue(quote.venue_id())
            .await
            .ok_or_else(|| ApplicationError::VenueNotAvailable(quote.venue_id().to_string()))?;

        // Execute trade via venue
        let execution_result = venue_adapter
            .execute_trade(&quote)
            .await
            .map_err(|e| ApplicationError::ExecutionFailed(e.to_string()))?;

        // Create trade from execution result
        let trade = self.create_trade_from_result(&rfq, &execution_result);

        // Mark RFQ as executed
        rfq.mark_executed()
            .map_err(|e| ApplicationError::InvalidState(e.to_string()))?;

        // Persist trade and RFQ
        self.trade_repository.save(&trade).await?;
        self.rfq_repository
            .save(&rfq)
            .await
            .map_err(ApplicationError::RepositoryError)?;

        // Publish trade executed event
        let event = TradeExecuted::builder()
            .rfq_id(rfq.id())
            .trade_id(trade.id())
            .quote_id(quote.id())
            .venue_id(quote.venue_id().clone())
            .counterparty_id(rfq.client_id().clone())
            .price(trade.price())
            .quantity(trade.quantity())
            .settlement_method(execution_result.settlement_method())
            .build();
        self.event_publisher
            .publish_trade_executed(event.clone())
            .await?;

        // Publish position update event (Position Manager will handle Greeks + margin)
        let position_event = crate::domain::events::PositionUpdated::new(
            rfq.id(),
            trade.id(),
            rfq.client_id().clone(),
            rfq.side(),
            quote.venue_id().clone(),
            rfq.side().opposite(),
            rfq.instrument().clone(),
            trade.quantity(),
            trade.price(),
        );
        self.event_publisher
            .publish_position_updated(position_event)
            .await?;

        // Send multi-channel trade confirmations (non-blocking)
        if let (Some(confirmation_service), Some(counterparty_repo)) =
            (&self.confirmation_service, &self.counterparty_repository)
        {
            self.send_trade_confirmations(
                &trade,
                &event,
                &rfq,
                Arc::clone(confirmation_service),
                Arc::clone(counterparty_repo),
            )
            .await;
        }

        let execution_time_ms = start.elapsed().as_millis() as u64;

        Ok(ExecuteTradeResponse::new(
            trade,
            rfq.id(),
            execution_time_ms,
        ))
    }

    /// Creates a Trade from an ExecutionResult.
    fn create_trade_from_result(&self, rfq: &Rfq, result: &ExecutionResult) -> Trade {
        if let Some(venue_ref) = result.venue_execution_id() {
            Trade::with_venue_ref(
                rfq.id(),
                result.quote_id(),
                result.venue_id().clone(),
                result.execution_price(),
                result.executed_quantity(),
                venue_ref,
            )
        } else {
            Trade::new(
                rfq.id(),
                result.quote_id(),
                result.venue_id().clone(),
                result.execution_price(),
                result.executed_quantity(),
            )
        }
    }

    /// Sends trade confirmations to counterparty via configured channels.
    ///
    /// This is a fire-and-forget operation that doesn't block trade execution.
    /// Failures in confirmation delivery are logged but don't affect the trade.
    async fn send_trade_confirmations(
        &self,
        trade: &Trade,
        event: &TradeExecuted,
        rfq: &Rfq,
        confirmation_service: Arc<dyn crate::domain::services::ConfirmationService>,
        counterparty_repo: Arc<
            dyn crate::infrastructure::persistence::traits::CounterpartyRepository,
        >,
    ) {
        use crate::domain::value_objects::{OrderSide, TradeConfirmation};

        let counterparty_id = rfq.client_id();

        // Load counterparty to get notification preferences
        let counterparty = match counterparty_repo.get(counterparty_id).await {
            Ok(Some(cp)) => cp,
            Ok(None) => {
                tracing::warn!(
                    trade_id = %trade.id(),
                    counterparty_id = %counterparty_id,
                    "Counterparty not found, skipping confirmation"
                );
                return;
            }
            Err(e) => {
                tracing::error!(
                    trade_id = %trade.id(),
                    counterparty_id = %counterparty_id,
                    error = %e,
                    "Failed to load counterparty for confirmation"
                );
                return;
            }
        };

        // Determine buyer and seller based on RFQ side
        // Client is the requester, venue is the liquidity provider
        let (buyer, seller) = match rfq.side() {
            OrderSide::Buy => (
                TradeParticipant::Counterparty(counterparty_id.clone()),
                TradeParticipant::Venue(event.venue_id.clone()),
            ),
            OrderSide::Sell => (
                TradeParticipant::Venue(event.venue_id.clone()),
                TradeParticipant::Counterparty(counterparty_id.clone()),
            ),
        };

        // Build trade confirmation
        let confirmation = TradeConfirmation::new(
            trade.id(),
            trade.rfq_id(),
            trade.price(),
            trade.quantity(),
            event.taker_fee.unwrap_or_default(),
            event.maker_fee.unwrap_or_default(),
            event.net_fee.unwrap_or_default(),
            event.settlement_method,
            buyer,
            seller,
        );

        // Send confirmation (async, non-blocking)
        let preferences = counterparty.notification_preferences().clone();
        tokio::spawn(async move {
            let status = confirmation_service
                .send_confirmation(&confirmation, &preferences)
                .await;

            tracing::info!(
                trade_id = %confirmation.trade_id(),
                status = %status,
                "Trade confirmation delivery completed"
            );
        });
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::domain::entities::quote::Quote;
    use crate::domain::entities::rfq::RfqBuilder;
    use crate::domain::value_objects::enums::{AssetClass, SettlementMethod};
    use crate::domain::value_objects::symbol::Symbol;
    use crate::domain::value_objects::timestamp::Timestamp;
    use crate::domain::value_objects::{
        CounterpartyId, Instrument, OrderSide, Price, Quantity, VenueId,
    };
    use crate::infrastructure::venues::error::{VenueError, VenueResult};
    use crate::infrastructure::venues::traits::{VenueAdapter, VenueHealth};
    use std::collections::HashMap;
    use std::sync::Mutex;

    #[derive(Debug, Default)]
    struct MockRfqRepository {
        rfqs: Mutex<HashMap<RfqId, Rfq>>,
    }

    impl MockRfqRepository {
        fn with_rfq(rfq: Rfq) -> Self {
            let mut map = HashMap::new();
            map.insert(rfq.id(), rfq);
            Self {
                rfqs: Mutex::new(map),
            }
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
    struct MockTradeRepository {
        trades: Mutex<HashMap<TradeId, Trade>>,
    }

    #[async_trait]
    impl TradeRepository for MockTradeRepository {
        async fn save(&self, trade: &Trade) -> ApplicationResult<()> {
            self.trades
                .lock()
                .unwrap()
                .insert(trade.id(), trade.clone());
            Ok(())
        }

        async fn find_by_id(&self, id: TradeId) -> ApplicationResult<Option<Trade>> {
            Ok(self.trades.lock().unwrap().get(&id).cloned())
        }

        async fn find_by_rfq_id(&self, rfq_id: RfqId) -> ApplicationResult<Vec<Trade>> {
            Ok(self
                .trades
                .lock()
                .unwrap()
                .values()
                .filter(|t| t.rfq_id() == rfq_id)
                .cloned()
                .collect())
        }
    }

    #[derive(Debug, Default)]
    struct MockTradeEventPublisher {
        events: Mutex<Vec<TradeExecuted>>,
        position_events: Mutex<Vec<crate::domain::events::PositionUpdated>>,
    }

    impl MockTradeEventPublisher {
        fn position_event_count(&self) -> usize {
            self.position_events.lock().unwrap().len()
        }

        fn last_position_event(&self) -> Option<crate::domain::events::PositionUpdated> {
            self.position_events.lock().unwrap().last().cloned()
        }
    }

    #[async_trait]
    impl TradeEventPublisher for MockTradeEventPublisher {
        async fn publish_trade_executed(&self, event: TradeExecuted) -> ApplicationResult<()> {
            self.events.lock().unwrap().push(event);
            Ok(())
        }

        async fn publish_execution_failed(
            &self,
            _rfq_id: RfqId,
            _quote_id: QuoteId,
            _reason: &str,
        ) -> ApplicationResult<()> {
            Ok(())
        }

        async fn publish_position_updated(
            &self,
            event: crate::domain::events::PositionUpdated,
        ) -> ApplicationResult<()> {
            self.position_events.lock().unwrap().push(event);
            Ok(())
        }
    }

    #[derive(Debug)]
    struct MockVenueAdapter {
        venue_id: VenueId,
        execution_result: Mutex<Option<VenueResult<ExecutionResult>>>,
    }

    impl MockVenueAdapter {
        fn successful(venue_id: &str, quote_id: QuoteId) -> Self {
            let result = ExecutionResult::new(
                quote_id,
                VenueId::new(venue_id),
                Price::new(100.0).unwrap(),
                Quantity::new(1.0).unwrap(),
                SettlementMethod::default(),
            );
            Self {
                venue_id: VenueId::new(venue_id),
                execution_result: Mutex::new(Some(Ok(result))),
            }
        }

        fn failing(venue_id: &str) -> Self {
            Self {
                venue_id: VenueId::new(venue_id),
                execution_result: Mutex::new(Some(Err(VenueError::ExecutionFailed {
                    message: "execution failed".to_string(),
                    error_code: None,
                }))),
            }
        }
    }

    #[async_trait]
    impl VenueAdapter for MockVenueAdapter {
        fn venue_id(&self) -> &VenueId {
            &self.venue_id
        }

        fn timeout_ms(&self) -> u64 {
            5000
        }

        async fn request_quote(&self, _rfq: &Rfq) -> VenueResult<Quote> {
            unimplemented!()
        }

        async fn execute_trade(&self, _quote: &Quote) -> VenueResult<ExecutionResult> {
            self.execution_result.lock().unwrap().take().unwrap_or(Err(
                VenueError::ExecutionFailed {
                    message: "no result".to_string(),
                    error_code: None,
                },
            ))
        }

        async fn health_check(&self) -> VenueResult<VenueHealth> {
            Ok(VenueHealth::healthy(self.venue_id.clone()))
        }
    }

    #[derive(Debug)]
    struct MockVenueRegistry {
        venues: HashMap<String, Arc<dyn VenueAdapter>>,
    }

    impl MockVenueRegistry {
        fn with_venue(venue: Arc<dyn VenueAdapter>) -> Self {
            let mut venues = HashMap::new();
            venues.insert(venue.venue_id().to_string(), venue);
            Self { venues }
        }

        fn empty() -> Self {
            Self {
                venues: HashMap::new(),
            }
        }
    }

    #[async_trait]
    impl VenueRegistry for MockVenueRegistry {
        async fn get_available_venues(&self) -> Vec<Arc<dyn VenueAdapter>> {
            self.venues.values().cloned().collect()
        }

        async fn get_venue(&self, venue_id: &VenueId) -> Option<Arc<dyn VenueAdapter>> {
            self.venues.get(&venue_id.to_string()).cloned()
        }
    }

    fn create_test_rfq_with_quote() -> (Rfq, Quote) {
        let symbol = Symbol::new("BTC/USD").unwrap();
        let instrument =
            Instrument::new(symbol, AssetClass::CryptoSpot, SettlementMethod::default());
        let quantity = Quantity::new(1.0).unwrap();
        let expires_at = Timestamp::now().add_secs(300);

        let mut rfq = RfqBuilder::new(
            CounterpartyId::new("client-1"),
            instrument,
            OrderSide::Buy,
            quantity,
            expires_at,
        )
        .build();

        let quote = Quote::new(
            rfq.id(),
            VenueId::new("venue-1"),
            Price::new(100.0).unwrap(),
            Quantity::new(1.0).unwrap(),
            Timestamp::now().add_secs(60),
        )
        .unwrap();

        // Transition RFQ to QuotesReceived state
        rfq.start_quote_collection().unwrap();
        rfq.receive_quote(quote.clone()).unwrap();

        (rfq, quote)
    }

    fn create_use_case(
        rfq_repo: impl RfqRepository + 'static,
        trade_repo: impl TradeRepository + 'static,
        venue_registry: impl VenueRegistry + 'static,
    ) -> ExecuteTradeUseCase {
        ExecuteTradeUseCase::new(
            Arc::new(rfq_repo),
            Arc::new(trade_repo),
            Arc::new(MockTradeEventPublisher::default()),
            Arc::new(venue_registry),
        )
    }

    #[tokio::test]
    async fn execute_trade_success() {
        let (rfq, quote) = create_test_rfq_with_quote();
        let rfq_id = rfq.id();
        let quote_id = quote.id();
        let client_id = rfq.client_id().clone();
        let venue_id = quote.venue_id().clone();
        let instrument = rfq.instrument().clone();
        let side = rfq.side();

        let venue_adapter = Arc::new(MockVenueAdapter::successful("venue-1", quote_id));
        let event_publisher = Arc::new(MockTradeEventPublisher::default());
        let use_case = ExecuteTradeUseCase::new(
            Arc::new(MockRfqRepository::with_rfq(rfq)),
            Arc::new(MockTradeRepository::default()),
            Arc::clone(&event_publisher) as Arc<dyn TradeEventPublisher>,
            Arc::new(MockVenueRegistry::with_venue(venue_adapter)),
        );

        let request = ExecuteTradeRequest::new(rfq_id, quote_id);
        let result = use_case.execute(request).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.rfq_id, rfq_id);
        assert!(response.execution_time_ms < 1000);

        // Verify PositionUpdated event was published
        assert_eq!(event_publisher.position_event_count(), 1);
        let position_event = event_publisher.last_position_event().unwrap();
        assert_eq!(position_event.trade_id, response.trade.id());
        assert_eq!(position_event.requester_id, client_id);
        assert_eq!(position_event.requester_side, side);
        assert_eq!(position_event.mm_id, venue_id);
        assert_eq!(position_event.mm_side, side.opposite());
        assert_eq!(position_event.instrument, instrument);
        assert_eq!(position_event.quantity, response.trade.quantity());
        assert_eq!(position_event.price, response.trade.price());
    }

    #[tokio::test]
    async fn execute_trade_rfq_not_found() {
        let use_case = create_use_case(
            MockRfqRepository::default(),
            MockTradeRepository::default(),
            MockVenueRegistry::empty(),
        );

        let request = ExecuteTradeRequest::new(RfqId::new_v4(), QuoteId::new_v4());
        let result = use_case.execute(request).await;

        assert!(matches!(result, Err(ApplicationError::RfqNotFound(_))));
    }

    #[tokio::test]
    async fn execute_trade_quote_not_found() {
        let (rfq, _quote) = create_test_rfq_with_quote();
        let rfq_id = rfq.id();

        let use_case = create_use_case(
            MockRfqRepository::with_rfq(rfq),
            MockTradeRepository::default(),
            MockVenueRegistry::empty(),
        );

        let request = ExecuteTradeRequest::new(rfq_id, QuoteId::new_v4());
        let result = use_case.execute(request).await;

        assert!(matches!(result, Err(ApplicationError::QuoteNotFound(_))));
    }

    #[tokio::test]
    async fn execute_trade_venue_not_available() {
        let (rfq, quote) = create_test_rfq_with_quote();
        let rfq_id = rfq.id();
        let quote_id = quote.id();

        let use_case = create_use_case(
            MockRfqRepository::with_rfq(rfq),
            MockTradeRepository::default(),
            MockVenueRegistry::empty(),
        );

        let request = ExecuteTradeRequest::new(rfq_id, quote_id);
        let result = use_case.execute(request).await;

        assert!(matches!(
            result,
            Err(ApplicationError::VenueNotAvailable(_))
        ));
    }

    #[tokio::test]
    async fn execute_trade_execution_failed() {
        let (rfq, quote) = create_test_rfq_with_quote();
        let rfq_id = rfq.id();
        let quote_id = quote.id();

        let venue_adapter = Arc::new(MockVenueAdapter::failing("venue-1"));
        let use_case = create_use_case(
            MockRfqRepository::with_rfq(rfq),
            MockTradeRepository::default(),
            MockVenueRegistry::with_venue(venue_adapter),
        );

        let request = ExecuteTradeRequest::new(rfq_id, quote_id);
        let result = use_case.execute(request).await;

        assert!(matches!(result, Err(ApplicationError::ExecutionFailed(_))));
    }

    #[test]
    fn execute_trade_request_new() {
        let rfq_id = RfqId::new_v4();
        let quote_id = QuoteId::new_v4();
        let request = ExecuteTradeRequest::new(rfq_id, quote_id);

        assert_eq!(request.rfq_id, rfq_id);
        assert_eq!(request.quote_id, quote_id);
    }
}
