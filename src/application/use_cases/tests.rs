//! # Use Case Integration Tests
//!
//! This module provides comprehensive tests for application use cases,
//! including reusable mock implementations and integration scenarios.
//!
//! # Test Categories
//!
//! - **CreateRFQ**: RFQ creation workflow tests
//! - **CollectQuotes**: Quote collection with mock venues
//! - **ExecuteTrade**: Trade execution workflow tests
//! - **Error Handling**: Comprehensive error path coverage
//! - **Integration**: End-to-end workflow tests

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::clone_on_ref_ptr)]
#![allow(dead_code)]
#![allow(clippy::panic)]

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;

use crate::application::dto::rfq_dto::CreateRfqRequest;
use crate::application::error::{ApplicationError, ApplicationResult};
use crate::application::use_cases::collect_quotes::{
    CollectQuotesConfig, CollectQuotesUseCase, QuoteEventPublisher, VenueRegistry,
};
use crate::application::use_cases::create_rfq::{
    ClientRepository, ComplianceService, CreateRfqUseCase, EventPublisher, InstrumentRegistry,
    RfqRepository,
};
use crate::application::use_cases::execute_trade::{
    ExecuteTradeRequest, ExecuteTradeUseCase, TradeEventPublisher, TradeRepository,
};
use crate::domain::entities::quote::Quote;
use crate::domain::entities::rfq::{ComplianceResult, Rfq, RfqBuilder};
use crate::domain::entities::trade::Trade;
use crate::domain::events::rfq_events::{QuoteReceived, RfqCreated};
use crate::domain::events::TradeExecuted;
use crate::domain::value_objects::enums::{AssetClass, SettlementMethod};
use crate::domain::value_objects::symbol::Symbol;
use crate::domain::value_objects::timestamp::Timestamp;
use crate::domain::value_objects::{
    CounterpartyId, Instrument, OrderSide, Price, Quantity, QuoteId, RfqId, TradeId, VenueId,
};
use crate::infrastructure::venues::error::{VenueError, VenueResult};
use crate::infrastructure::venues::traits::{ExecutionResult, VenueAdapter, VenueHealth};

// ============================================================================
// Reusable Mock Implementations
// ============================================================================

/// Mock RFQ repository with configurable behavior.
#[derive(Debug, Default)]
pub struct MockRfqRepository {
    rfqs: Mutex<HashMap<RfqId, Rfq>>,
    save_count: AtomicUsize,
    should_fail_save: Mutex<bool>,
    should_fail_find: Mutex<bool>,
}

impl MockRfqRepository {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_rfq(rfq: Rfq) -> Self {
        let repo = Self::new();
        repo.rfqs.lock().unwrap().insert(rfq.id(), rfq);
        repo
    }

    pub fn set_fail_save(&self, fail: bool) {
        *self.should_fail_save.lock().unwrap() = fail;
    }

    pub fn set_fail_find(&self, fail: bool) {
        *self.should_fail_find.lock().unwrap() = fail;
    }

    pub fn save_count(&self) -> usize {
        self.save_count.load(Ordering::SeqCst)
    }

    pub fn get_rfq(&self, id: RfqId) -> Option<Rfq> {
        self.rfqs.lock().unwrap().get(&id).cloned()
    }
}

#[async_trait]
impl RfqRepository for MockRfqRepository {
    async fn save(&self, rfq: &Rfq) -> Result<(), String> {
        if *self.should_fail_save.lock().unwrap() {
            return Err("mock save failure".to_string());
        }
        self.rfqs.lock().unwrap().insert(rfq.id(), rfq.clone());
        self.save_count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    async fn find_by_id(&self, id: RfqId) -> Result<Option<Rfq>, String> {
        if *self.should_fail_find.lock().unwrap() {
            return Err("mock find failure".to_string());
        }
        Ok(self.rfqs.lock().unwrap().get(&id).cloned())
    }
}

/// Mock event publisher that tracks published events.
#[derive(Debug, Default)]
pub struct MockEventPublisher {
    rfq_created_events: Mutex<Vec<RfqCreated>>,
    should_fail: Mutex<bool>,
}

impl MockEventPublisher {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_fail(&self, fail: bool) {
        *self.should_fail.lock().unwrap() = fail;
    }

    pub fn rfq_created_count(&self) -> usize {
        self.rfq_created_events.lock().unwrap().len()
    }
}

#[async_trait]
impl EventPublisher for MockEventPublisher {
    async fn publish_rfq_created(&self, event: RfqCreated) -> Result<(), String> {
        if *self.should_fail.lock().unwrap() {
            return Err("mock publish failure".to_string());
        }
        self.rfq_created_events.lock().unwrap().push(event);
        Ok(())
    }
}

/// Mock compliance service with configurable results.
#[derive(Debug)]
pub struct MockComplianceService {
    should_pass: bool,
    failure_reason: Option<String>,
    call_count: AtomicUsize,
}

impl MockComplianceService {
    pub fn passing() -> Self {
        Self {
            should_pass: true,
            failure_reason: None,
            call_count: AtomicUsize::new(0),
        }
    }

    pub fn failing(reason: &str) -> Self {
        Self {
            should_pass: false,
            failure_reason: Some(reason.to_string()),
            call_count: AtomicUsize::new(0),
        }
    }

    pub fn call_count(&self) -> usize {
        self.call_count.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl ComplianceService for MockComplianceService {
    async fn pre_check(
        &self,
        _client_id: &CounterpartyId,
        _base_asset: &str,
        _quote_asset: &str,
        _quantity: f64,
    ) -> Result<ComplianceResult, String> {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        if self.should_pass {
            Ok(ComplianceResult::passed())
        } else {
            Ok(ComplianceResult::failed(
                self.failure_reason
                    .clone()
                    .unwrap_or_else(|| "compliance failed".to_string()),
            ))
        }
    }
}

/// Mock client repository.
#[derive(Debug)]
pub struct MockClientRepository {
    existing_clients: Vec<String>,
    active_clients: Vec<String>,
}

impl MockClientRepository {
    pub fn with_active_client(client_id: &str) -> Self {
        Self {
            existing_clients: vec![client_id.to_string()],
            active_clients: vec![client_id.to_string()],
        }
    }

    pub fn with_inactive_client(client_id: &str) -> Self {
        Self {
            existing_clients: vec![client_id.to_string()],
            active_clients: vec![],
        }
    }
}

#[async_trait]
impl ClientRepository for MockClientRepository {
    async fn exists(&self, client_id: &str) -> Result<bool, String> {
        Ok(self.existing_clients.contains(&client_id.to_string()))
    }

    async fn is_active(&self, client_id: &str) -> Result<bool, String> {
        Ok(self.active_clients.contains(&client_id.to_string()))
    }
}

/// Mock instrument registry.
#[derive(Debug)]
pub struct MockInstrumentRegistry {
    supported: Vec<(String, String)>,
}

impl MockInstrumentRegistry {
    pub fn with_instrument(base: &str, quote: &str) -> Self {
        Self {
            supported: vec![(base.to_string(), quote.to_string())],
        }
    }

    pub fn with_instruments(instruments: Vec<(&str, &str)>) -> Self {
        Self {
            supported: instruments
                .into_iter()
                .map(|(b, q)| (b.to_string(), q.to_string()))
                .collect(),
        }
    }
}

#[async_trait]
impl InstrumentRegistry for MockInstrumentRegistry {
    async fn is_supported(&self, base_asset: &str, quote_asset: &str) -> Result<bool, String> {
        Ok(self
            .supported
            .iter()
            .any(|(b, q)| b == base_asset && q == quote_asset))
    }
}

/// Mock quote event publisher.
#[derive(Debug, Default)]
pub struct MockQuoteEventPublisher {
    events: Mutex<Vec<QuoteReceived>>,
}

#[async_trait]
impl QuoteEventPublisher for MockQuoteEventPublisher {
    async fn publish_quote_received(&self, event: QuoteReceived) -> Result<(), String> {
        self.events.lock().unwrap().push(event);
        Ok(())
    }
}

/// Mock venue adapter with configurable behavior.
#[derive(Debug)]
pub struct MockVenueAdapter {
    venue_id: VenueId,
    quote_result: Mutex<Option<VenueResult<Quote>>>,
    execution_result: Mutex<Option<VenueResult<ExecutionResult>>>,
    delay_ms: u64,
    request_count: AtomicUsize,
}

impl MockVenueAdapter {
    pub fn successful_quote(venue_id: &str, rfq_id: RfqId) -> Self {
        let quote = create_test_quote(rfq_id, venue_id);
        Self {
            venue_id: VenueId::new(venue_id),
            quote_result: Mutex::new(Some(Ok(quote))),
            execution_result: Mutex::new(None),
            delay_ms: 0,
            request_count: AtomicUsize::new(0),
        }
    }

    pub fn failing_quote(venue_id: &str, error: &str) -> Self {
        Self {
            venue_id: VenueId::new(venue_id),
            quote_result: Mutex::new(Some(Err(VenueError::QuoteUnavailable {
                message: error.to_string(),
            }))),
            execution_result: Mutex::new(None),
            delay_ms: 0,
            request_count: AtomicUsize::new(0),
        }
    }

    pub fn slow_quote(venue_id: &str, delay_ms: u64) -> Self {
        Self {
            venue_id: VenueId::new(venue_id),
            quote_result: Mutex::new(None),
            execution_result: Mutex::new(None),
            delay_ms,
            request_count: AtomicUsize::new(0),
        }
    }

    pub fn successful_execution(venue_id: &str, quote_id: QuoteId) -> Self {
        let result = ExecutionResult::new(
            quote_id,
            VenueId::new(venue_id),
            Price::new(50000.0).unwrap(),
            Quantity::new(1.0).unwrap(),
            SettlementMethod::default(),
        );
        Self {
            venue_id: VenueId::new(venue_id),
            quote_result: Mutex::new(None),
            execution_result: Mutex::new(Some(Ok(result))),
            delay_ms: 0,
            request_count: AtomicUsize::new(0),
        }
    }

    pub fn failing_execution(venue_id: &str, error: &str) -> Self {
        Self {
            venue_id: VenueId::new(venue_id),
            quote_result: Mutex::new(None),
            execution_result: Mutex::new(Some(Err(VenueError::ExecutionFailed {
                message: error.to_string(),
                error_code: None,
            }))),
            delay_ms: 0,
            request_count: AtomicUsize::new(0),
        }
    }

    pub fn request_count(&self) -> usize {
        self.request_count.load(Ordering::SeqCst)
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
        self.request_count.fetch_add(1, Ordering::SeqCst);
        if self.delay_ms > 0 {
            tokio::time::sleep(Duration::from_millis(self.delay_ms)).await;
        }
        self.quote_result
            .lock()
            .unwrap()
            .take()
            .unwrap_or(Err(VenueError::QuoteUnavailable {
                message: "no result configured".to_string(),
            }))
    }

    async fn execute_trade(&self, _quote: &Quote) -> VenueResult<ExecutionResult> {
        self.request_count.fetch_add(1, Ordering::SeqCst);
        self.execution_result
            .lock()
            .unwrap()
            .take()
            .unwrap_or(Err(VenueError::ExecutionFailed {
                message: "no result configured".to_string(),
                error_code: None,
            }))
    }

    async fn health_check(&self) -> VenueResult<VenueHealth> {
        Ok(VenueHealth::healthy(self.venue_id.clone()))
    }
}

/// Mock venue registry.
#[derive(Debug)]
pub struct MockVenueRegistry {
    venues: Vec<Arc<dyn VenueAdapter>>,
}

impl MockVenueRegistry {
    pub fn with_venues(venues: Vec<Arc<dyn VenueAdapter>>) -> Self {
        Self { venues }
    }

    pub fn empty() -> Self {
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

/// Mock trade repository.
#[derive(Debug, Default)]
pub struct MockTradeRepository {
    trades: Mutex<HashMap<TradeId, Trade>>,
    save_count: AtomicUsize,
}

impl MockTradeRepository {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn save_count(&self) -> usize {
        self.save_count.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl TradeRepository for MockTradeRepository {
    async fn save(&self, trade: &Trade) -> ApplicationResult<()> {
        self.trades
            .lock()
            .unwrap()
            .insert(trade.id(), trade.clone());
        self.save_count.fetch_add(1, Ordering::SeqCst);
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

/// Mock trade event publisher.
#[derive(Debug, Default)]
pub struct MockTradeEventPublisher {
    executed_events: Mutex<Vec<TradeExecuted>>,
    failed_events: Mutex<Vec<(RfqId, QuoteId, String)>>,
}

impl MockTradeEventPublisher {
    pub fn executed_count(&self) -> usize {
        self.executed_events.lock().unwrap().len()
    }
}

#[async_trait]
impl TradeEventPublisher for MockTradeEventPublisher {
    async fn publish_trade_executed(&self, event: TradeExecuted) -> ApplicationResult<()> {
        self.executed_events.lock().unwrap().push(event);
        Ok(())
    }

    async fn publish_execution_failed(
        &self,
        rfq_id: RfqId,
        quote_id: QuoteId,
        reason: &str,
    ) -> ApplicationResult<()> {
        self.failed_events
            .lock()
            .unwrap()
            .push((rfq_id, quote_id, reason.to_string()));
        Ok(())
    }
}

// ============================================================================
// Test Helpers
// ============================================================================

fn create_test_instrument() -> Instrument {
    let symbol = Symbol::new("BTC/USD").unwrap();
    Instrument::new(symbol, AssetClass::CryptoSpot, SettlementMethod::default())
}

fn create_test_rfq() -> Rfq {
    RfqBuilder::new(
        CounterpartyId::new("client-1"),
        create_test_instrument(),
        OrderSide::Buy,
        Quantity::new(1.0).unwrap(),
        Timestamp::now().add_secs(300),
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

fn create_rfq_with_quote() -> (Rfq, Quote) {
    let mut rfq = create_test_rfq();
    let quote = create_test_quote(rfq.id(), "venue-1");

    rfq.start_quote_collection().unwrap();
    rfq.receive_quote(quote.clone()).unwrap();

    (rfq, quote)
}

// ============================================================================
// CreateRFQ Use Case Tests
// ============================================================================

#[cfg(test)]
mod create_rfq_tests {
    use super::*;

    fn create_use_case(
        rfq_repo: Arc<MockRfqRepository>,
        event_pub: Arc<MockEventPublisher>,
        compliance: Arc<MockComplianceService>,
        client_repo: Arc<dyn ClientRepository>,
        instrument_reg: Arc<dyn InstrumentRegistry>,
    ) -> CreateRfqUseCase {
        CreateRfqUseCase::new(rfq_repo, event_pub, compliance, client_repo, instrument_reg)
    }

    #[tokio::test]
    async fn successful_rfq_creation_persists_and_publishes() {
        let rfq_repo = Arc::new(MockRfqRepository::new());
        let event_pub = Arc::new(MockEventPublisher::new());
        let compliance = Arc::new(MockComplianceService::passing());

        let use_case = create_use_case(
            rfq_repo.clone(),
            event_pub.clone(),
            compliance.clone(),
            Arc::new(MockClientRepository::with_active_client("client-1")),
            Arc::new(MockInstrumentRegistry::with_instrument("BTC", "USD")),
        );

        let request = CreateRfqRequest::new("client-1", "BTC", "USD", OrderSide::Buy, 1.5, 300);
        let result = use_case.execute(request).await;

        assert!(result.is_ok());
        assert_eq!(rfq_repo.save_count(), 1);
        assert_eq!(event_pub.rfq_created_count(), 1);
        assert_eq!(compliance.call_count(), 1);
    }

    #[tokio::test]
    async fn inactive_client_rejected() {
        let use_case = create_use_case(
            Arc::new(MockRfqRepository::new()),
            Arc::new(MockEventPublisher::new()),
            Arc::new(MockComplianceService::passing()),
            Arc::new(MockClientRepository::with_inactive_client("client-1")),
            Arc::new(MockInstrumentRegistry::with_instrument("BTC", "USD")),
        );

        let request = CreateRfqRequest::new("client-1", "BTC", "USD", OrderSide::Buy, 1.5, 300);
        let result = use_case.execute(request).await;

        assert!(matches!(result, Err(ApplicationError::ClientNotActive(_))));
    }

    #[tokio::test]
    async fn repository_failure_propagates_error() {
        let rfq_repo = Arc::new(MockRfqRepository::new());
        rfq_repo.set_fail_save(true);

        let use_case = create_use_case(
            rfq_repo,
            Arc::new(MockEventPublisher::new()),
            Arc::new(MockComplianceService::passing()),
            Arc::new(MockClientRepository::with_active_client("client-1")),
            Arc::new(MockInstrumentRegistry::with_instrument("BTC", "USD")),
        );

        let request = CreateRfqRequest::new("client-1", "BTC", "USD", OrderSide::Buy, 1.5, 300);
        let result = use_case.execute(request).await;

        assert!(matches!(result, Err(ApplicationError::RepositoryError(_))));
    }

    #[tokio::test]
    async fn event_publish_failure_propagates_error() {
        let event_pub = Arc::new(MockEventPublisher::new());
        event_pub.set_fail(true);

        let use_case = create_use_case(
            Arc::new(MockRfqRepository::new()),
            event_pub,
            Arc::new(MockComplianceService::passing()),
            Arc::new(MockClientRepository::with_active_client("client-1")),
            Arc::new(MockInstrumentRegistry::with_instrument("BTC", "USD")),
        );

        let request = CreateRfqRequest::new("client-1", "BTC", "USD", OrderSide::Buy, 1.5, 300);
        let result = use_case.execute(request).await;

        assert!(matches!(
            result,
            Err(ApplicationError::EventPublishError(_))
        ));
    }

    #[tokio::test]
    async fn compliance_failure_with_custom_reason() {
        let use_case = create_use_case(
            Arc::new(MockRfqRepository::new()),
            Arc::new(MockEventPublisher::new()),
            Arc::new(MockComplianceService::failing("AML check failed")),
            Arc::new(MockClientRepository::with_active_client("client-1")),
            Arc::new(MockInstrumentRegistry::with_instrument("BTC", "USD")),
        );

        let request = CreateRfqRequest::new("client-1", "BTC", "USD", OrderSide::Buy, 1.5, 300);
        let result = use_case.execute(request).await;

        match result {
            Err(ApplicationError::ComplianceFailed(reason)) => {
                assert!(reason.contains("AML"));
            }
            _ => panic!("Expected ComplianceFailed error"),
        }
    }

    #[tokio::test]
    async fn invalid_quantity_rejected() {
        let use_case = create_use_case(
            Arc::new(MockRfqRepository::new()),
            Arc::new(MockEventPublisher::new()),
            Arc::new(MockComplianceService::passing()),
            Arc::new(MockClientRepository::with_active_client("client-1")),
            Arc::new(MockInstrumentRegistry::with_instrument("BTC", "USD")),
        );

        let request = CreateRfqRequest::new("client-1", "BTC", "USD", OrderSide::Buy, -1.0, 300);
        let result = use_case.execute(request).await;

        assert!(matches!(result, Err(ApplicationError::Validation(_))));
    }
}

// ============================================================================
// CollectQuotes Use Case Tests
// ============================================================================

#[cfg(test)]
mod collect_quotes_tests {
    use super::*;

    fn create_use_case(
        rfq_repo: Arc<MockRfqRepository>,
        venue_registry: Arc<MockVenueRegistry>,
        config: CollectQuotesConfig,
    ) -> CollectQuotesUseCase {
        CollectQuotesUseCase::new(
            rfq_repo,
            Arc::new(MockQuoteEventPublisher::default()),
            venue_registry,
            config,
        )
    }

    #[tokio::test]
    async fn concurrent_venue_requests() {
        let rfq = create_test_rfq();
        let rfq_id = rfq.id();

        let venue1 = Arc::new(MockVenueAdapter::successful_quote("venue-1", rfq_id));
        let venue2 = Arc::new(MockVenueAdapter::successful_quote("venue-2", rfq_id));
        let venue3 = Arc::new(MockVenueAdapter::successful_quote("venue-3", rfq_id));

        let venues: Vec<Arc<dyn VenueAdapter>> =
            vec![venue1.clone(), venue2.clone(), venue3.clone()];

        let use_case = create_use_case(
            Arc::new(MockRfqRepository::with_rfq(rfq)),
            Arc::new(MockVenueRegistry::with_venues(venues)),
            CollectQuotesConfig::with_timeout(1000),
        );

        let result = use_case.execute(rfq_id).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        assert_eq!(response.success_count(), 3);
        assert_eq!(response.venues_queried, 3);

        // Verify all venues were called
        assert_eq!(venue1.request_count(), 1);
        assert_eq!(venue2.request_count(), 1);
        assert_eq!(venue3.request_count(), 1);
    }

    #[tokio::test]
    async fn mixed_success_and_failure() {
        let rfq = create_test_rfq();
        let rfq_id = rfq.id();

        let venues: Vec<Arc<dyn VenueAdapter>> = vec![
            Arc::new(MockVenueAdapter::successful_quote("venue-1", rfq_id)),
            Arc::new(MockVenueAdapter::failing_quote("venue-2", "no liquidity")),
            Arc::new(MockVenueAdapter::successful_quote("venue-3", rfq_id)),
        ];

        let use_case = create_use_case(
            Arc::new(MockRfqRepository::with_rfq(rfq)),
            Arc::new(MockVenueRegistry::with_venues(venues)),
            CollectQuotesConfig::with_timeout(1000),
        );

        let result = use_case.execute(rfq_id).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        assert_eq!(response.success_count(), 2);
        assert_eq!(response.failure_count(), 1);
    }

    #[tokio::test]
    async fn timeout_handling() {
        let rfq = create_test_rfq();
        let rfq_id = rfq.id();

        let venues: Vec<Arc<dyn VenueAdapter>> = vec![
            Arc::new(MockVenueAdapter::slow_quote("venue-1", 500)),
            Arc::new(MockVenueAdapter::successful_quote("venue-2", rfq_id)),
        ];

        let use_case = create_use_case(
            Arc::new(MockRfqRepository::with_rfq(rfq)),
            Arc::new(MockVenueRegistry::with_venues(venues)),
            CollectQuotesConfig::with_timeout(100), // Short timeout
        );

        let result = use_case.execute(rfq_id).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        assert_eq!(response.success_count(), 1);
        assert_eq!(response.failure_count(), 1);

        // Verify timeout error message
        let timeout_failure = response
            .failures
            .iter()
            .find(|f| f.venue_id.as_str() == "venue-1");
        assert!(timeout_failure.is_some());
        assert!(timeout_failure
            .unwrap()
            .error
            .as_ref()
            .unwrap()
            .contains("timed out"));
    }

    #[tokio::test]
    async fn min_quotes_enforcement() {
        let rfq = create_test_rfq();
        let rfq_id = rfq.id();

        let venues: Vec<Arc<dyn VenueAdapter>> = vec![
            Arc::new(MockVenueAdapter::failing_quote("venue-1", "error")),
            Arc::new(MockVenueAdapter::failing_quote("venue-2", "error")),
        ];

        let use_case = create_use_case(
            Arc::new(MockRfqRepository::with_rfq(rfq)),
            Arc::new(MockVenueRegistry::with_venues(venues)),
            CollectQuotesConfig::with_timeout(1000).with_min_quotes(1),
        );

        let result = use_case.execute(rfq_id).await;
        assert!(matches!(result, Err(ApplicationError::Validation(_))));
    }

    #[tokio::test]
    async fn rfq_state_updated_after_quotes() {
        let rfq = create_test_rfq();
        let rfq_id = rfq.id();
        let rfq_repo = Arc::new(MockRfqRepository::with_rfq(rfq));

        let venues: Vec<Arc<dyn VenueAdapter>> = vec![Arc::new(
            MockVenueAdapter::successful_quote("venue-1", rfq_id),
        )];

        let use_case = create_use_case(
            rfq_repo.clone(),
            Arc::new(MockVenueRegistry::with_venues(venues)),
            CollectQuotesConfig::with_timeout(1000),
        );

        let result = use_case.execute(rfq_id).await;
        assert!(result.is_ok());

        // Verify RFQ was saved with quotes
        let saved_rfq = rfq_repo.get_rfq(rfq_id).unwrap();
        assert!(saved_rfq.has_quotes());
        assert_eq!(saved_rfq.quote_count(), 1);
    }
}

// ============================================================================
// ExecuteTrade Use Case Tests
// ============================================================================

#[cfg(test)]
mod execute_trade_tests {
    use super::*;

    fn create_use_case(
        rfq_repo: Arc<MockRfqRepository>,
        trade_repo: Arc<MockTradeRepository>,
        event_pub: Arc<MockTradeEventPublisher>,
        venue_registry: Arc<MockVenueRegistry>,
    ) -> ExecuteTradeUseCase {
        ExecuteTradeUseCase::new(rfq_repo, trade_repo, event_pub, venue_registry)
    }

    #[tokio::test]
    async fn successful_execution_creates_trade() {
        let (rfq, quote) = create_rfq_with_quote();
        let rfq_id = rfq.id();
        let quote_id = quote.id();

        let trade_repo = Arc::new(MockTradeRepository::new());
        let event_pub = Arc::new(MockTradeEventPublisher::default());

        let venue = Arc::new(MockVenueAdapter::successful_execution("venue-1", quote_id));
        let venues: Vec<Arc<dyn VenueAdapter>> = vec![venue];

        let use_case = create_use_case(
            Arc::new(MockRfqRepository::with_rfq(rfq)),
            trade_repo.clone(),
            event_pub.clone(),
            Arc::new(MockVenueRegistry::with_venues(venues)),
        );

        let request = ExecuteTradeRequest::new(rfq_id, quote_id);
        let result = use_case.execute(request).await;

        assert!(result.is_ok());
        assert_eq!(trade_repo.save_count(), 1);
        assert_eq!(event_pub.executed_count(), 1);
    }

    #[tokio::test]
    async fn execution_failure_returns_error() {
        let (rfq, quote) = create_rfq_with_quote();
        let rfq_id = rfq.id();
        let quote_id = quote.id();

        let venue = Arc::new(MockVenueAdapter::failing_execution(
            "venue-1",
            "insufficient funds",
        ));
        let venues: Vec<Arc<dyn VenueAdapter>> = vec![venue];

        let use_case = create_use_case(
            Arc::new(MockRfqRepository::with_rfq(rfq)),
            Arc::new(MockTradeRepository::new()),
            Arc::new(MockTradeEventPublisher::default()),
            Arc::new(MockVenueRegistry::with_venues(venues)),
        );

        let request = ExecuteTradeRequest::new(rfq_id, quote_id);
        let result = use_case.execute(request).await;

        assert!(matches!(result, Err(ApplicationError::ExecutionFailed(_))));
    }

    #[tokio::test]
    async fn venue_not_found_returns_error() {
        let (rfq, quote) = create_rfq_with_quote();
        let rfq_id = rfq.id();
        let quote_id = quote.id();

        let use_case = create_use_case(
            Arc::new(MockRfqRepository::with_rfq(rfq)),
            Arc::new(MockTradeRepository::new()),
            Arc::new(MockTradeEventPublisher::default()),
            Arc::new(MockVenueRegistry::empty()),
        );

        let request = ExecuteTradeRequest::new(rfq_id, quote_id);
        let result = use_case.execute(request).await;

        assert!(matches!(
            result,
            Err(ApplicationError::VenueNotAvailable(_))
        ));
    }

    #[tokio::test]
    async fn rfq_state_updated_after_execution() {
        let (rfq, quote) = create_rfq_with_quote();
        let rfq_id = rfq.id();
        let quote_id = quote.id();
        let rfq_repo = Arc::new(MockRfqRepository::with_rfq(rfq));

        let venue = Arc::new(MockVenueAdapter::successful_execution("venue-1", quote_id));
        let venues: Vec<Arc<dyn VenueAdapter>> = vec![venue];

        let use_case = create_use_case(
            rfq_repo.clone(),
            Arc::new(MockTradeRepository::new()),
            Arc::new(MockTradeEventPublisher::default()),
            Arc::new(MockVenueRegistry::with_venues(venues)),
        );

        let request = ExecuteTradeRequest::new(rfq_id, quote_id);
        let result = use_case.execute(request).await;

        assert!(result.is_ok());

        // Verify RFQ state is Executed
        let saved_rfq = rfq_repo.get_rfq(rfq_id).unwrap();
        assert_eq!(
            saved_rfq.state(),
            crate::domain::value_objects::RfqState::Executed
        );
    }
}

// ============================================================================
// Integration Tests (End-to-End Workflows)
// ============================================================================

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[tokio::test]
    async fn full_rfq_lifecycle() {
        // 1. Create RFQ
        let rfq_repo = Arc::new(MockRfqRepository::new());
        let event_pub = Arc::new(MockEventPublisher::new());

        let create_use_case = CreateRfqUseCase::new(
            rfq_repo.clone(),
            event_pub.clone(),
            Arc::new(MockComplianceService::passing()),
            Arc::new(MockClientRepository::with_active_client("client-1")),
            Arc::new(MockInstrumentRegistry::with_instrument("BTC", "USD")),
        );

        let create_request =
            CreateRfqRequest::new("client-1", "BTC", "USD", OrderSide::Buy, 1.0, 300);
        let create_result = create_use_case.execute(create_request).await;
        assert!(create_result.is_ok());

        let rfq_id = create_result.unwrap().rfq_id;

        // 2. Collect Quotes
        let venue = Arc::new(MockVenueAdapter::successful_quote("venue-1", rfq_id));
        let venues: Vec<Arc<dyn VenueAdapter>> = vec![venue];

        let collect_use_case = CollectQuotesUseCase::new(
            rfq_repo.clone(),
            Arc::new(MockQuoteEventPublisher::default()),
            Arc::new(MockVenueRegistry::with_venues(venues.clone())),
            CollectQuotesConfig::with_timeout(1000),
        );

        let collect_result = collect_use_case.execute(rfq_id).await;
        assert!(collect_result.is_ok());

        let quote_id = collect_result.unwrap().quotes.first().unwrap().id();

        // 3. Execute Trade
        // Need to create a new venue adapter for execution
        let exec_venue = Arc::new(MockVenueAdapter::successful_execution("venue-1", quote_id));
        let exec_venues: Vec<Arc<dyn VenueAdapter>> = vec![exec_venue];

        let execute_use_case = ExecuteTradeUseCase::new(
            rfq_repo.clone(),
            Arc::new(MockTradeRepository::new()),
            Arc::new(MockTradeEventPublisher::default()),
            Arc::new(MockVenueRegistry::with_venues(exec_venues)),
        );

        let execute_request = ExecuteTradeRequest::new(rfq_id, quote_id);
        let execute_result = execute_use_case.execute(execute_request).await;
        assert!(execute_result.is_ok());

        // Verify final state
        let final_rfq = rfq_repo.get_rfq(rfq_id).unwrap();
        assert_eq!(
            final_rfq.state(),
            crate::domain::value_objects::RfqState::Executed
        );
    }
}

// ============================================================================
// End-to-End RFQ Workflow Tests
// ============================================================================

/// Comprehensive end-to-end tests for the complete RFQ lifecycle.
///
/// These tests verify the full workflow from RFQ creation through settlement,
/// including state transitions, event emission, and failure scenarios.
#[cfg(test)]
#[allow(clippy::indexing_slicing)]
mod e2e_rfq_workflow_tests {
    use super::*;
    use crate::domain::value_objects::RfqState;

    /// Test fixture containing all mocks and use cases for E2E testing.
    struct E2ETestFixture {
        rfq_repo: Arc<MockRfqRepository>,
        trade_repo: Arc<MockTradeRepository>,
        rfq_event_pub: Arc<MockEventPublisher>,
        quote_event_pub: Arc<MockQuoteEventPublisher>,
        trade_event_pub: Arc<MockTradeEventPublisher>,
        compliance: Arc<MockComplianceService>,
        client_repo: Arc<MockClientRepository>,
        instrument_reg: Arc<MockInstrumentRegistry>,
    }

    impl E2ETestFixture {
        fn new() -> Self {
            Self {
                rfq_repo: Arc::new(MockRfqRepository::new()),
                trade_repo: Arc::new(MockTradeRepository::new()),
                rfq_event_pub: Arc::new(MockEventPublisher::new()),
                quote_event_pub: Arc::new(MockQuoteEventPublisher::default()),
                trade_event_pub: Arc::new(MockTradeEventPublisher::default()),
                compliance: Arc::new(MockComplianceService::passing()),
                client_repo: Arc::new(MockClientRepository::with_active_client("client-1")),
                instrument_reg: Arc::new(MockInstrumentRegistry::with_instruments(vec![
                    ("BTC", "USD"),
                    ("ETH", "USD"),
                    ("BTC", "EUR"),
                ])),
            }
        }

        fn create_rfq_use_case(&self) -> CreateRfqUseCase {
            CreateRfqUseCase::new(
                self.rfq_repo.clone(),
                self.rfq_event_pub.clone(),
                self.compliance.clone(),
                self.client_repo.clone(),
                self.instrument_reg.clone(),
            )
        }

        fn collect_quotes_use_case(
            &self,
            venues: Vec<Arc<dyn VenueAdapter>>,
        ) -> CollectQuotesUseCase {
            CollectQuotesUseCase::new(
                self.rfq_repo.clone(),
                self.quote_event_pub.clone(),
                Arc::new(MockVenueRegistry::with_venues(venues)),
                CollectQuotesConfig::with_timeout(1000),
            )
        }

        fn execute_trade_use_case(
            &self,
            venues: Vec<Arc<dyn VenueAdapter>>,
        ) -> ExecuteTradeUseCase {
            ExecuteTradeUseCase::new(
                self.rfq_repo.clone(),
                self.trade_repo.clone(),
                self.trade_event_pub.clone(),
                Arc::new(MockVenueRegistry::with_venues(venues)),
            )
        }
    }

    // ========================================================================
    // Full Workflow Tests
    // ========================================================================

    /// Tests the complete happy path: Create RFQ -> Collect Quotes -> Execute Trade.
    #[tokio::test]
    async fn e2e_complete_rfq_workflow_happy_path() {
        let fixture = E2ETestFixture::new();

        // Step 1: Create RFQ via use case
        let create_uc = fixture.create_rfq_use_case();
        let create_req = CreateRfqRequest::new("client-1", "BTC", "USD", OrderSide::Buy, 2.5, 300);
        let create_result = create_uc.execute(create_req).await;

        assert!(create_result.is_ok(), "RFQ creation should succeed");
        let rfq_response = create_result.unwrap();
        let rfq_id = rfq_response.rfq_id;

        // Verify RFQ created event was published
        assert_eq!(fixture.rfq_event_pub.rfq_created_count(), 1);

        // Verify RFQ is in Created state
        let rfq = fixture.rfq_repo.get_rfq(rfq_id).unwrap();
        assert_eq!(rfq.state(), RfqState::Created);

        // Step 2: Collect quotes from multiple venues
        let venues: Vec<Arc<dyn VenueAdapter>> = vec![
            Arc::new(MockVenueAdapter::successful_quote("venue-alpha", rfq_id)),
            Arc::new(MockVenueAdapter::successful_quote("venue-beta", rfq_id)),
            Arc::new(MockVenueAdapter::successful_quote("venue-gamma", rfq_id)),
        ];

        let collect_uc = fixture.collect_quotes_use_case(venues);
        let collect_result = collect_uc.execute(rfq_id).await;

        assert!(collect_result.is_ok(), "Quote collection should succeed");
        let quotes_response = collect_result.unwrap();

        // Verify we got quotes from all venues
        assert_eq!(quotes_response.success_count(), 3);
        assert_eq!(quotes_response.venues_queried, 3);
        assert_eq!(quotes_response.failure_count(), 0);

        // Verify RFQ state is now QuotesReceived
        let rfq = fixture.rfq_repo.get_rfq(rfq_id).unwrap();
        assert_eq!(rfq.state(), RfqState::QuotesReceived);
        assert_eq!(rfq.quote_count(), 3);

        // Step 3: Select best quote and execute trade
        let best_quote = quotes_response.quotes.first().unwrap();
        let quote_id = best_quote.id();

        let exec_venues: Vec<Arc<dyn VenueAdapter>> = vec![Arc::new(
            MockVenueAdapter::successful_execution("venue-alpha", quote_id),
        )];

        let execute_uc = fixture.execute_trade_use_case(exec_venues);
        let execute_req = ExecuteTradeRequest::new(rfq_id, quote_id);
        let execute_result = execute_uc.execute(execute_req).await;

        assert!(execute_result.is_ok(), "Trade execution should succeed");
        let trade_response = execute_result.unwrap();

        // Verify trade was created
        assert_eq!(fixture.trade_repo.save_count(), 1);
        assert_eq!(fixture.trade_event_pub.executed_count(), 1);

        // Verify trade details
        assert_eq!(trade_response.rfq_id, rfq_id);
        assert!(trade_response.execution_time_ms < 1000);

        // Verify final RFQ state is Executed
        let final_rfq = fixture.rfq_repo.get_rfq(rfq_id).unwrap();
        assert_eq!(final_rfq.state(), RfqState::Executed);
    }

    /// Tests workflow with multiple quotes and selecting the best one.
    #[tokio::test]
    async fn e2e_select_best_quote_from_multiple_venues() {
        let fixture = E2ETestFixture::new();

        // Create RFQ
        let create_uc = fixture.create_rfq_use_case();
        let create_req =
            CreateRfqRequest::new("client-1", "ETH", "USD", OrderSide::Sell, 10.0, 300);
        let rfq_id = create_uc.execute(create_req).await.unwrap().rfq_id;

        // Collect quotes from multiple venues
        let venues: Vec<Arc<dyn VenueAdapter>> = vec![
            Arc::new(MockVenueAdapter::successful_quote("venue-1", rfq_id)),
            Arc::new(MockVenueAdapter::successful_quote("venue-2", rfq_id)),
        ];

        let collect_uc = fixture.collect_quotes_use_case(venues);
        let quotes_response = collect_uc.execute(rfq_id).await.unwrap();

        assert_eq!(quotes_response.quotes.len(), 2);

        // In a real scenario, we'd select the best quote by price
        // For this test, we just verify we can execute any of them
        let selected_quote = &quotes_response.quotes[0];

        let exec_venues: Vec<Arc<dyn VenueAdapter>> = vec![Arc::new(
            MockVenueAdapter::successful_execution("venue-1", selected_quote.id()),
        )];

        let execute_uc = fixture.execute_trade_use_case(exec_venues);
        let result = execute_uc
            .execute(ExecuteTradeRequest::new(rfq_id, selected_quote.id()))
            .await;

        assert!(result.is_ok());
    }

    // ========================================================================
    // State Transition Tests
    // ========================================================================

    /// Verifies correct state transitions throughout the workflow.
    #[tokio::test]
    async fn e2e_verify_state_transitions() {
        let fixture = E2ETestFixture::new();

        // Initial: No RFQ exists
        let create_uc = fixture.create_rfq_use_case();
        let create_req = CreateRfqRequest::new("client-1", "BTC", "USD", OrderSide::Buy, 1.0, 300);
        let rfq_id = create_uc.execute(create_req).await.unwrap().rfq_id;

        // State 1: Created
        let rfq = fixture.rfq_repo.get_rfq(rfq_id).unwrap();
        assert_eq!(rfq.state(), RfqState::Created, "After creation");

        // Collect quotes
        let venues: Vec<Arc<dyn VenueAdapter>> = vec![Arc::new(
            MockVenueAdapter::successful_quote("venue-1", rfq_id),
        )];
        let collect_uc = fixture.collect_quotes_use_case(venues);
        let quotes = collect_uc.execute(rfq_id).await.unwrap();

        // State 2: QuotesReceived
        let rfq = fixture.rfq_repo.get_rfq(rfq_id).unwrap();
        assert_eq!(
            rfq.state(),
            RfqState::QuotesReceived,
            "After quotes received"
        );

        // Execute trade
        let quote_id = quotes.quotes[0].id();
        let exec_venues: Vec<Arc<dyn VenueAdapter>> = vec![Arc::new(
            MockVenueAdapter::successful_execution("venue-1", quote_id),
        )];
        let execute_uc = fixture.execute_trade_use_case(exec_venues);
        execute_uc
            .execute(ExecuteTradeRequest::new(rfq_id, quote_id))
            .await
            .unwrap();

        // State 3: Executed
        let rfq = fixture.rfq_repo.get_rfq(rfq_id).unwrap();
        assert_eq!(rfq.state(), RfqState::Executed, "After trade executed");
    }

    // ========================================================================
    // Domain Event Tests
    // ========================================================================

    /// Verifies all domain events are emitted correctly throughout the workflow.
    #[tokio::test]
    async fn e2e_verify_domain_events_emitted() {
        let fixture = E2ETestFixture::new();

        // Create RFQ
        let create_uc = fixture.create_rfq_use_case();
        let create_req = CreateRfqRequest::new("client-1", "BTC", "USD", OrderSide::Buy, 1.0, 300);
        let rfq_id = create_uc.execute(create_req).await.unwrap().rfq_id;

        // Verify RfqCreated event
        assert_eq!(
            fixture.rfq_event_pub.rfq_created_count(),
            1,
            "RfqCreated event should be published"
        );

        // Collect quotes
        let venues: Vec<Arc<dyn VenueAdapter>> = vec![
            Arc::new(MockVenueAdapter::successful_quote("venue-1", rfq_id)),
            Arc::new(MockVenueAdapter::successful_quote("venue-2", rfq_id)),
        ];
        let collect_uc = fixture.collect_quotes_use_case(venues);
        let quotes = collect_uc.execute(rfq_id).await.unwrap();

        // Execute trade
        let quote_id = quotes.quotes[0].id();
        let exec_venues: Vec<Arc<dyn VenueAdapter>> = vec![Arc::new(
            MockVenueAdapter::successful_execution("venue-1", quote_id),
        )];
        let execute_uc = fixture.execute_trade_use_case(exec_venues);
        execute_uc
            .execute(ExecuteTradeRequest::new(rfq_id, quote_id))
            .await
            .unwrap();

        // Verify TradeExecuted event
        assert_eq!(
            fixture.trade_event_pub.executed_count(),
            1,
            "TradeExecuted event should be published"
        );
    }

    // ========================================================================
    // Failure Scenario Tests
    // ========================================================================

    /// Tests workflow when all venues fail to provide quotes.
    #[tokio::test]
    async fn e2e_all_venues_fail_to_quote() {
        let fixture = E2ETestFixture::new();

        // Create RFQ
        let create_uc = fixture.create_rfq_use_case();
        let create_req = CreateRfqRequest::new("client-1", "BTC", "USD", OrderSide::Buy, 1.0, 300);
        let rfq_id = create_uc.execute(create_req).await.unwrap().rfq_id;

        // All venues fail
        let venues: Vec<Arc<dyn VenueAdapter>> = vec![
            Arc::new(MockVenueAdapter::failing_quote("venue-1", "no liquidity")),
            Arc::new(MockVenueAdapter::failing_quote("venue-2", "market closed")),
        ];

        let collect_uc = CollectQuotesUseCase::new(
            fixture.rfq_repo.clone(),
            fixture.quote_event_pub.clone(),
            Arc::new(MockVenueRegistry::with_venues(venues)),
            CollectQuotesConfig::with_timeout(1000).with_min_quotes(1),
        );

        let result = collect_uc.execute(rfq_id).await;

        // Should fail because min_quotes=1 but no quotes received
        assert!(result.is_err());
        assert!(matches!(result, Err(ApplicationError::Validation(_))));
    }

    /// Tests workflow when trade execution fails.
    #[tokio::test]
    async fn e2e_trade_execution_fails() {
        let fixture = E2ETestFixture::new();

        // Create RFQ and collect quotes
        let create_uc = fixture.create_rfq_use_case();
        let create_req = CreateRfqRequest::new("client-1", "BTC", "USD", OrderSide::Buy, 1.0, 300);
        let rfq_id = create_uc.execute(create_req).await.unwrap().rfq_id;

        let venues: Vec<Arc<dyn VenueAdapter>> = vec![Arc::new(
            MockVenueAdapter::successful_quote("venue-1", rfq_id),
        )];
        let collect_uc = fixture.collect_quotes_use_case(venues);
        let quotes = collect_uc.execute(rfq_id).await.unwrap();

        let quote_id = quotes.quotes[0].id();

        // Execution fails
        let exec_venues: Vec<Arc<dyn VenueAdapter>> = vec![Arc::new(
            MockVenueAdapter::failing_execution("venue-1", "insufficient balance"),
        )];
        let execute_uc = fixture.execute_trade_use_case(exec_venues);
        let result = execute_uc
            .execute(ExecuteTradeRequest::new(rfq_id, quote_id))
            .await;

        assert!(result.is_err());
        assert!(matches!(result, Err(ApplicationError::ExecutionFailed(_))));

        // Trade should not be saved
        assert_eq!(fixture.trade_repo.save_count(), 0);
    }

    /// Tests workflow with partial venue failures during quote collection.
    #[tokio::test]
    async fn e2e_partial_venue_failures() {
        let fixture = E2ETestFixture::new();

        // Create RFQ
        let create_uc = fixture.create_rfq_use_case();
        let create_req = CreateRfqRequest::new("client-1", "BTC", "USD", OrderSide::Buy, 1.0, 300);
        let rfq_id = create_uc.execute(create_req).await.unwrap().rfq_id;

        // Mixed success and failure
        let venues: Vec<Arc<dyn VenueAdapter>> = vec![
            Arc::new(MockVenueAdapter::successful_quote("venue-1", rfq_id)),
            Arc::new(MockVenueAdapter::failing_quote("venue-2", "timeout")),
            Arc::new(MockVenueAdapter::successful_quote("venue-3", rfq_id)),
        ];

        let collect_uc = fixture.collect_quotes_use_case(venues);
        let result = collect_uc.execute(rfq_id).await;

        assert!(result.is_ok());
        let response = result.unwrap();

        // Should have 2 successful quotes, 1 failure
        assert_eq!(response.success_count(), 2);
        assert_eq!(response.failure_count(), 1);

        // Workflow can continue with available quotes
        let quote_id = response.quotes[0].id();
        let exec_venues: Vec<Arc<dyn VenueAdapter>> = vec![Arc::new(
            MockVenueAdapter::successful_execution("venue-1", quote_id),
        )];
        let execute_uc = fixture.execute_trade_use_case(exec_venues);
        let exec_result = execute_uc
            .execute(ExecuteTradeRequest::new(rfq_id, quote_id))
            .await;

        assert!(exec_result.is_ok());
    }

    /// Tests workflow when venue times out during quote collection.
    #[tokio::test]
    async fn e2e_venue_timeout_during_quotes() {
        let fixture = E2ETestFixture::new();

        // Create RFQ
        let create_uc = fixture.create_rfq_use_case();
        let create_req = CreateRfqRequest::new("client-1", "BTC", "USD", OrderSide::Buy, 1.0, 300);
        let rfq_id = create_uc.execute(create_req).await.unwrap().rfq_id;

        // One venue is slow
        let venues: Vec<Arc<dyn VenueAdapter>> = vec![
            Arc::new(MockVenueAdapter::successful_quote("venue-fast", rfq_id)),
            Arc::new(MockVenueAdapter::slow_quote("venue-slow", 500)), // 500ms delay
        ];

        let collect_uc = CollectQuotesUseCase::new(
            fixture.rfq_repo.clone(),
            fixture.quote_event_pub.clone(),
            Arc::new(MockVenueRegistry::with_venues(venues)),
            CollectQuotesConfig::with_timeout(100), // 100ms timeout
        );

        let result = collect_uc.execute(rfq_id).await;

        assert!(result.is_ok());
        let response = result.unwrap();

        // Fast venue succeeded, slow venue timed out
        assert_eq!(response.success_count(), 1);
        assert_eq!(response.failure_count(), 1);

        // Verify timeout error
        let timeout_failure = response
            .failures
            .iter()
            .find(|f| f.venue_id.as_str() == "venue-slow");
        assert!(timeout_failure.is_some());
        assert!(timeout_failure
            .unwrap()
            .error
            .as_ref()
            .unwrap()
            .contains("timed out"));
    }

    /// Tests that compliance failure prevents RFQ creation.
    #[tokio::test]
    async fn e2e_compliance_failure_blocks_workflow() {
        let fixture = E2ETestFixture {
            compliance: Arc::new(MockComplianceService::failing("Sanctioned entity")),
            ..E2ETestFixture::new()
        };

        let create_uc = fixture.create_rfq_use_case();
        let create_req = CreateRfqRequest::new("client-1", "BTC", "USD", OrderSide::Buy, 1.0, 300);
        let result = create_uc.execute(create_req).await;

        assert!(result.is_err());
        assert!(matches!(result, Err(ApplicationError::ComplianceFailed(_))));

        // No RFQ should be created
        assert_eq!(fixture.rfq_repo.save_count(), 0);
    }

    /// Tests workflow with invalid RFQ ID during quote collection.
    #[tokio::test]
    async fn e2e_invalid_rfq_id_during_quotes() {
        let fixture = E2ETestFixture::new();

        let invalid_rfq_id = RfqId::new_v4();
        let venues: Vec<Arc<dyn VenueAdapter>> = vec![Arc::new(
            MockVenueAdapter::successful_quote("venue-1", invalid_rfq_id),
        )];

        let collect_uc = fixture.collect_quotes_use_case(venues);
        let result = collect_uc.execute(invalid_rfq_id).await;

        // Should fail because RFQ doesn't exist in repository
        assert!(result.is_err());
        // The error message should indicate RFQ not found
        let err_msg = format!("{:?}", result.unwrap_err());
        assert!(
            err_msg.contains("not found") || err_msg.contains("RFQ"),
            "Error should indicate RFQ not found: {}",
            err_msg
        );
    }

    /// Tests workflow with invalid quote ID during execution.
    #[tokio::test]
    async fn e2e_invalid_quote_id_during_execution() {
        let fixture = E2ETestFixture::new();

        // Create RFQ and collect quotes
        let create_uc = fixture.create_rfq_use_case();
        let create_req = CreateRfqRequest::new("client-1", "BTC", "USD", OrderSide::Buy, 1.0, 300);
        let rfq_id = create_uc.execute(create_req).await.unwrap().rfq_id;

        let venues: Vec<Arc<dyn VenueAdapter>> = vec![Arc::new(
            MockVenueAdapter::successful_quote("venue-1", rfq_id),
        )];
        let collect_uc = fixture.collect_quotes_use_case(venues);
        collect_uc.execute(rfq_id).await.unwrap();

        // Try to execute with invalid quote ID
        let invalid_quote_id = QuoteId::new_v4();
        let exec_venues: Vec<Arc<dyn VenueAdapter>> = vec![Arc::new(
            MockVenueAdapter::successful_execution("venue-1", invalid_quote_id),
        )];
        let execute_uc = fixture.execute_trade_use_case(exec_venues);
        let result = execute_uc
            .execute(ExecuteTradeRequest::new(rfq_id, invalid_quote_id))
            .await;

        assert!(result.is_err());
        assert!(matches!(result, Err(ApplicationError::QuoteNotFound(_))));
    }

    // ========================================================================
    // Concurrent Workflow Tests
    // ========================================================================

    /// Tests multiple concurrent RFQ workflows.
    #[tokio::test]
    async fn e2e_concurrent_rfq_workflows() {
        let fixture = E2ETestFixture::new();

        // Create use cases that will be moved into futures
        let uc1 = CreateRfqUseCase::new(
            fixture.rfq_repo.clone(),
            fixture.rfq_event_pub.clone(),
            fixture.compliance.clone(),
            fixture.client_repo.clone(),
            fixture.instrument_reg.clone(),
        );
        let uc2 = CreateRfqUseCase::new(
            fixture.rfq_repo.clone(),
            fixture.rfq_event_pub.clone(),
            fixture.compliance.clone(),
            fixture.client_repo.clone(),
            fixture.instrument_reg.clone(),
        );

        let req1 = CreateRfqRequest::new("client-1", "BTC", "USD", OrderSide::Buy, 1.0, 300);
        let req2 = CreateRfqRequest::new("client-1", "ETH", "USD", OrderSide::Sell, 5.0, 300);

        // Execute concurrently
        let (result1, result2) = tokio::join!(uc1.execute(req1), uc2.execute(req2));

        assert!(result1.is_ok());
        assert!(result2.is_ok());

        let rfq_id1 = result1.unwrap().rfq_id;
        let rfq_id2 = result2.unwrap().rfq_id;

        // Both RFQs should be created
        assert!(fixture.rfq_repo.get_rfq(rfq_id1).is_some());
        assert!(fixture.rfq_repo.get_rfq(rfq_id2).is_some());
        assert_ne!(rfq_id1, rfq_id2);
    }
}
