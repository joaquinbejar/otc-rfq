//! # Mock Services for Acceptance Flow
//!
//! Mock implementations of [`RiskCheckService`] and [`LastLookService`]
//! for testing the acceptance flow without external dependencies.

use crate::domain::entities::quote::Quote;
use crate::domain::entities::rfq::Rfq;
use crate::domain::errors::DomainResult;
use crate::domain::services::last_look::{
    LastLookRejectReason, LastLookResult, LastLookService, LastLookStats,
};
use crate::domain::services::risk_check::{RiskCheckService, RiskResult};
use crate::domain::value_objects::{CounterpartyId, VenueId};
use async_trait::async_trait;
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

/// Mock implementation of [`RiskCheckService`] for testing.
///
/// Can be configured to pass or fail risk checks.
#[derive(Debug)]
pub struct MockRiskCheckService {
    /// Whether risk checks should pass.
    should_pass: Mutex<bool>,
    /// Custom failure reason.
    failure_reason: Mutex<Option<String>>,
}

impl MockRiskCheckService {
    /// Creates a new mock that always passes.
    #[must_use]
    pub fn passing() -> Self {
        Self {
            should_pass: Mutex::new(true),
            failure_reason: Mutex::new(None),
        }
    }

    /// Creates a new mock that always fails.
    #[must_use]
    pub fn failing(reason: impl Into<String>) -> Self {
        Self {
            should_pass: Mutex::new(false),
            failure_reason: Mutex::new(Some(reason.into())),
        }
    }

    /// Sets whether the mock should pass.
    pub fn set_should_pass(&self, pass: bool) {
        if let Ok(mut guard) = self.should_pass.lock() {
            *guard = pass;
        }
    }

    /// Sets the failure reason.
    pub fn set_failure_reason(&self, reason: impl Into<String>) {
        if let Ok(mut guard) = self.failure_reason.lock() {
            *guard = Some(reason.into());
        }
    }
}

impl Default for MockRiskCheckService {
    fn default() -> Self {
        Self::passing()
    }
}

#[async_trait]
impl RiskCheckService for MockRiskCheckService {
    async fn check(&self, rfq: &Rfq, quote: &Quote) -> RiskResult {
        let should_pass = self.should_pass.lock().map(|g| *g).unwrap_or(true);
        if should_pass {
            RiskResult::passed(rfq.id(), quote.id())
        } else {
            let reason = self
                .failure_reason
                .lock()
                .ok()
                .and_then(|g| g.clone())
                .unwrap_or_else(|| "Mock risk check failure".to_string());
            RiskResult::failed(rfq.id(), quote.id(), reason)
        }
    }

    async fn check_margin(
        &self,
        _counterparty_id: &str,
        _notional: Decimal,
    ) -> DomainResult<Decimal> {
        Ok(Decimal::new(100000, 2)) // $1000.00 available
    }

    async fn check_collateral(
        &self,
        _counterparty_id: &str,
        _required: Decimal,
    ) -> DomainResult<()> {
        Ok(())
    }

    async fn check_position_limit(
        &self,
        _counterparty_id: &str,
        _instrument: &str,
        _additional_size: Decimal,
    ) -> DomainResult<()> {
        Ok(())
    }
}

/// Mock implementation of [`LastLookService`] for testing.
///
/// Can be configured to confirm, reject, or timeout.
#[derive(Debug)]
pub struct MockLastLookService {
    /// The result to return.
    result: Mutex<MockLastLookBehavior>,
    /// Whether venues require last-look.
    requires_last_look: bool,
    /// Stats per venue.
    stats: Mutex<HashMap<String, LastLookStats>>,
}

/// Behavior configuration for mock last-look.
#[derive(Debug, Clone)]
pub enum MockLastLookBehavior {
    /// Always confirm.
    Confirm,
    /// Always reject with reason.
    Reject(LastLookRejectReason),
    /// Always timeout.
    Timeout,
}

impl MockLastLookService {
    /// Creates a mock that always confirms.
    #[must_use]
    pub fn confirming() -> Self {
        Self {
            result: Mutex::new(MockLastLookBehavior::Confirm),
            requires_last_look: true,
            stats: Mutex::new(HashMap::new()),
        }
    }

    /// Creates a mock that always rejects with a typed reason.
    #[must_use]
    pub fn rejecting(reason: LastLookRejectReason) -> Self {
        Self {
            result: Mutex::new(MockLastLookBehavior::Reject(reason)),
            requires_last_look: true,
            stats: Mutex::new(HashMap::new()),
        }
    }

    /// Creates a mock that always rejects with a string reason.
    #[must_use]
    pub fn rejecting_other(reason: impl Into<String>) -> Self {
        Self {
            result: Mutex::new(MockLastLookBehavior::Reject(LastLookRejectReason::Other(
                reason.into(),
            ))),
            requires_last_look: true,
            stats: Mutex::new(HashMap::new()),
        }
    }

    /// Creates a mock that always times out.
    #[must_use]
    pub fn timing_out() -> Self {
        Self {
            result: Mutex::new(MockLastLookBehavior::Timeout),
            requires_last_look: true,
            stats: Mutex::new(HashMap::new()),
        }
    }

    /// Creates a mock that doesn't require last-look.
    #[must_use]
    pub fn not_required() -> Self {
        Self {
            result: Mutex::new(MockLastLookBehavior::Confirm),
            requires_last_look: false,
            stats: Mutex::new(HashMap::new()),
        }
    }

    /// Sets the behavior.
    pub fn set_behavior(&self, behavior: MockLastLookBehavior) {
        if let Ok(mut guard) = self.result.lock() {
            *guard = behavior;
        }
    }
}

impl Default for MockLastLookService {
    fn default() -> Self {
        Self::confirming()
    }
}

#[async_trait]
impl LastLookService for MockLastLookService {
    async fn request(&self, quote: &Quote, timeout: Duration) -> LastLookResult {
        let behavior = self
            .result
            .lock()
            .map(|g| g.clone())
            .unwrap_or(MockLastLookBehavior::Confirm);
        match behavior {
            MockLastLookBehavior::Confirm => LastLookResult::confirmed(quote.id()),
            MockLastLookBehavior::Reject(reason) => LastLookResult::rejected(quote.id(), reason),
            MockLastLookBehavior::Timeout => LastLookResult::timeout(quote.id(), timeout),
        }
    }

    fn requires_last_look(&self, _venue_id: &VenueId) -> bool {
        self.requires_last_look
    }

    async fn get_stats(&self, venue_id: &VenueId) -> Option<LastLookStats> {
        self.stats
            .lock()
            .ok()
            .and_then(|stats| stats.get(&venue_id.to_string()).cloned())
    }

    async fn record_result(&self, venue_id: &VenueId, result: &LastLookResult) {
        if let Ok(mut stats) = self.stats.lock() {
            let entry = stats
                .entry(venue_id.to_string())
                .or_insert_with(LastLookStats::new);

            match result {
                LastLookResult::Confirmed { .. } => entry.record_confirmation(),
                LastLookResult::Rejected { .. } => entry.record_rejection(),
                LastLookResult::Timeout { .. } => entry.record_timeout(),
            }
        }
    }
}

// ============================================================================
// Mock implementations for Off-Book Execution services
// ============================================================================

use crate::domain::entities::block_trade::BlockTrade;
use crate::domain::events::TradeHash;
use crate::domain::services::ReportingTier;
use crate::domain::services::collateral_lock::{CollateralLockHandle, CollateralLockService};
use crate::domain::services::position_service::{Position, PositionUpdateService};
use crate::domain::services::report_scheduler::{
    ReportScheduler, ReportSchedulerConfig, ScheduledReport,
};
use crate::domain::services::settlement::{Fees, SettlementResult, SettlementService};
use crate::domain::value_objects::{Instrument, Price as VoPrice};

/// Mock implementation of [`CollateralLockService`] for testing.
#[derive(Debug)]
pub struct MockCollateralLockService {
    should_pass: Mutex<bool>,
    failure_reason: Mutex<Option<String>>,
}

impl MockCollateralLockService {
    /// Creates a mock that always succeeds.
    #[must_use]
    pub fn passing() -> Self {
        Self {
            should_pass: Mutex::new(true),
            failure_reason: Mutex::new(None),
        }
    }

    /// Creates a mock that always fails.
    #[must_use]
    pub fn failing(reason: impl Into<String>) -> Self {
        Self {
            should_pass: Mutex::new(false),
            failure_reason: Mutex::new(Some(reason.into())),
        }
    }
}

impl Default for MockCollateralLockService {
    fn default() -> Self {
        Self::passing()
    }
}

#[async_trait]
impl CollateralLockService for MockCollateralLockService {
    async fn lock_both(
        &self,
        buyer_id: &CounterpartyId,
        seller_id: &CounterpartyId,
        _instrument: &Instrument,
        _quantity: crate::domain::value_objects::Quantity,
        _price: VoPrice,
    ) -> DomainResult<CollateralLockHandle> {
        let should_pass = self.should_pass.lock().map(|g| *g).unwrap_or(true);
        if should_pass {
            Ok(CollateralLockHandle::new(
                buyer_id.clone(),
                seller_id.clone(),
                Decimal::new(10000, 2),
                Decimal::new(10000, 2),
            ))
        } else {
            let reason = self
                .failure_reason
                .lock()
                .ok()
                .and_then(|g| g.clone())
                .unwrap_or_else(|| "Mock collateral lock failure".to_string());
            Err(crate::domain::errors::DomainError::CollateralLockFailed(
                reason,
            ))
        }
    }

    async fn release(&self, _lock: &CollateralLockHandle) -> DomainResult<()> {
        Ok(())
    }

    async fn check_available(
        &self,
        _counterparty_id: &CounterpartyId,
        _required_amount: Decimal,
    ) -> DomainResult<Decimal> {
        Ok(Decimal::new(100000, 2))
    }
}

/// Mock implementation of [`SettlementService`] for testing.
#[derive(Debug)]
pub struct MockSettlementService {
    should_pass: Mutex<bool>,
    price_bounds_ok: Mutex<bool>,
}

impl MockSettlementService {
    /// Creates a mock that always succeeds.
    #[must_use]
    pub fn passing() -> Self {
        Self {
            should_pass: Mutex::new(true),
            price_bounds_ok: Mutex::new(true),
        }
    }

    /// Creates a mock that fails settlement.
    #[must_use]
    pub fn failing() -> Self {
        Self {
            should_pass: Mutex::new(false),
            price_bounds_ok: Mutex::new(true),
        }
    }

    /// Creates a mock that fails price bounds check.
    #[must_use]
    pub fn price_bounds_fail() -> Self {
        Self {
            should_pass: Mutex::new(true),
            price_bounds_ok: Mutex::new(false),
        }
    }
}

impl Default for MockSettlementService {
    fn default() -> Self {
        Self::passing()
    }
}

#[async_trait]
impl SettlementService for MockSettlementService {
    async fn settle(&self, trade: &BlockTrade) -> DomainResult<SettlementResult> {
        let should_pass = self.should_pass.lock().map(|g| *g).unwrap_or(true);
        if should_pass {
            Ok(SettlementResult::new(
                TradeHash::new(format!("0x{}", trade.id())),
                crate::domain::value_objects::Timestamp::now(),
                trade.quantity().get(),
                -trade.quantity().get(),
                Fees::zero(),
            ))
        } else {
            Err(crate::domain::errors::DomainError::SettlementFailed(
                "Mock settlement failure".to_string(),
            ))
        }
    }

    async fn verify_price_bounds(&self, _trade: &BlockTrade) -> DomainResult<bool> {
        Ok(self.price_bounds_ok.lock().map(|g| *g).unwrap_or(true))
    }

    async fn get_oracle_price(&self, _instrument_symbol: &str) -> DomainResult<VoPrice> {
        VoPrice::new(50000.0)
            .map_err(|e| crate::domain::errors::DomainError::ValidationError(e.to_string()))
    }
}

/// Mock implementation of [`PositionUpdateService`] for testing.
#[derive(Debug, Default)]
pub struct MockPositionUpdateService {
    should_pass: Mutex<bool>,
}

impl MockPositionUpdateService {
    /// Creates a mock that always succeeds.
    #[must_use]
    pub fn passing() -> Self {
        Self {
            should_pass: Mutex::new(true),
        }
    }

    /// Creates a mock that always fails.
    #[must_use]
    pub fn failing() -> Self {
        Self {
            should_pass: Mutex::new(false),
        }
    }
}

#[async_trait]
impl PositionUpdateService for MockPositionUpdateService {
    async fn update_both(
        &self,
        _trade: &BlockTrade,
        _settlement: &SettlementResult,
    ) -> DomainResult<()> {
        let should_pass = self.should_pass.lock().map(|g| *g).unwrap_or(true);
        if should_pass {
            Ok(())
        } else {
            Err(crate::domain::errors::DomainError::PositionUpdateFailed(
                "Mock position update failure".to_string(),
            ))
        }
    }

    async fn get_position(
        &self,
        counterparty_id: &CounterpartyId,
        instrument_symbol: &str,
    ) -> DomainResult<Option<Position>> {
        Ok(Some(Position::new(
            counterparty_id.clone(),
            instrument_symbol.to_string(),
            Decimal::ZERO,
            Decimal::ZERO,
        )))
    }
}

/// Mock implementation of [`ReportScheduler`] for testing.
#[derive(Debug)]
pub struct MockReportScheduler {
    config: ReportSchedulerConfig,
}

impl MockReportScheduler {
    /// Creates a new mock report scheduler.
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: ReportSchedulerConfig::default(),
        }
    }
}

impl Default for MockReportScheduler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ReportScheduler for MockReportScheduler {
    async fn schedule(&self, trade: &BlockTrade) -> DomainResult<ScheduledReport> {
        let tier = trade.reporting_tier().unwrap_or(ReportingTier::Standard);
        let delay = self.delay_for_tier(tier);
        let publish_at = crate::domain::value_objects::Timestamp::now()
            .add_secs(delay.as_secs().try_into().unwrap_or(i64::MAX));

        Ok(ScheduledReport::new(trade.id(), tier, publish_at))
    }

    fn delay_for_tier(&self, tier: ReportingTier) -> Duration {
        self.config.delay_for_tier(tier)
    }

    async fn publish(&self, report: &mut ScheduledReport) -> DomainResult<()> {
        report.mark_published();
        Ok(())
    }

    async fn get_pending(&self) -> DomainResult<Vec<ScheduledReport>> {
        Ok(Vec::new())
    }

    async fn process_ready(&self) -> DomainResult<usize> {
        Ok(0)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::domain::entities::rfq::RfqBuilder;
    use crate::domain::value_objects::enums::{AssetClass, SettlementMethod};
    use crate::domain::value_objects::symbol::Symbol;
    use crate::domain::value_objects::timestamp::Timestamp;
    use crate::domain::value_objects::{
        CounterpartyId, Instrument, OrderSide, Price, Quantity, VenueId,
    };

    fn create_test_rfq() -> crate::domain::entities::rfq::Rfq {
        let symbol = Symbol::new("BTC/USD").unwrap();
        let instrument =
            Instrument::new(symbol, AssetClass::CryptoSpot, SettlementMethod::default());
        RfqBuilder::new(
            CounterpartyId::new("client-1"),
            instrument,
            OrderSide::Buy,
            Quantity::new(1.0).unwrap(),
            Timestamp::now().add_secs(300),
        )
        .build()
    }

    fn create_test_quote(rfq: &crate::domain::entities::rfq::Rfq) -> Quote {
        Quote::new(
            rfq.id(),
            VenueId::new("venue-1"),
            Price::new(50000.0).unwrap(),
            Quantity::new(1.0).unwrap(),
            Timestamp::now().add_secs(60),
        )
        .unwrap()
    }

    #[tokio::test]
    async fn mock_risk_check_passing() {
        let service = MockRiskCheckService::passing();
        let rfq = create_test_rfq();
        let quote = create_test_quote(&rfq);

        let result = service.check(&rfq, &quote).await;
        assert!(result.is_passed());
    }

    #[tokio::test]
    async fn mock_risk_check_failing() {
        let service = MockRiskCheckService::failing("Insufficient margin");
        let rfq = create_test_rfq();
        let quote = create_test_quote(&rfq);

        let result = service.check(&rfq, &quote).await;
        assert!(!result.is_passed());
        assert_eq!(result.failure_reason(), Some("Insufficient margin"));
    }

    #[tokio::test]
    async fn mock_last_look_confirming() {
        let service = MockLastLookService::confirming();
        let rfq = create_test_rfq();
        let quote = create_test_quote(&rfq);

        let result = service.request(&quote, Duration::from_millis(200)).await;
        assert!(result.is_confirmed());
    }

    #[tokio::test]
    async fn mock_last_look_rejecting() {
        let service = MockLastLookService::rejecting(LastLookRejectReason::PriceMoved);
        let rfq = create_test_rfq();
        let quote = create_test_quote(&rfq);

        let result = service.request(&quote, Duration::from_millis(200)).await;
        assert!(result.is_rejected());
        assert_eq!(
            result.rejection_reason(),
            Some(&LastLookRejectReason::PriceMoved)
        );
    }

    #[tokio::test]
    async fn mock_last_look_rejecting_other() {
        let service = MockLastLookService::rejecting_other("Custom reason");
        let rfq = create_test_rfq();
        let quote = create_test_quote(&rfq);

        let result = service.request(&quote, Duration::from_millis(200)).await;
        assert!(result.is_rejected());
        assert!(matches!(
            result.rejection_reason(),
            Some(&LastLookRejectReason::Other(_))
        ));
    }

    #[tokio::test]
    async fn mock_last_look_timeout() {
        let service = MockLastLookService::timing_out();
        let rfq = create_test_rfq();
        let quote = create_test_quote(&rfq);

        let result = service.request(&quote, Duration::from_millis(200)).await;
        assert!(result.is_timeout());
    }

    #[tokio::test]
    async fn mock_last_look_not_required() {
        let service = MockLastLookService::not_required();
        let venue_id = VenueId::new("venue-1");

        assert!(!service.requires_last_look(&venue_id));
    }

    #[tokio::test]
    async fn mock_last_look_records_stats() {
        let service = MockLastLookService::confirming();
        let rfq = create_test_rfq();
        let quote = create_test_quote(&rfq);
        let venue_id = quote.venue_id().clone();

        let result = service.request(&quote, Duration::from_millis(200)).await;
        service.record_result(&venue_id, &result).await;

        let stats = service.get_stats(&venue_id).await.unwrap();
        assert_eq!(stats.confirmations, 1);
        assert_eq!(stats.total_requests, 1);
    }
}
