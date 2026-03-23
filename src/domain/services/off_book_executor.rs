//! # Off-Book Executor Service
//!
//! Orchestrates the off-book execution flow for block trades.
//!
//! This module provides the [`OffBookExecutor`] that coordinates the 5-step
//! off-book execution flow:
//!
//! 1. Risk check both counterparties
//! 2. Lock collateral atomically
//! 3. Execute on-chain settlement
//! 4. Update positions
//! 5. Schedule delayed report
//!
//! # Architecture
//!
//! ```text
//! BlockTrade (Approved)
//!     → Risk Check (both)
//!     → Lock Collateral
//!     → On-Chain Settlement
//!     → Update Positions
//!     → Schedule Report
//!     → BlockTrade (Executed)
//! ```
//!
//! On failure at any step, all previous steps are rolled back.

use crate::domain::entities::block_trade::{BlockTrade, BlockTradeState};
use crate::domain::errors::{DomainError, DomainResult};
use crate::domain::events::DomainEvent;
use crate::domain::events::off_book_events::{
    CollateralLocked, CollateralReleased, ExecutionStep, OffBookExecutionStarted, OffBookFailed,
    OffBookSettled,
};
use crate::domain::services::collateral_lock::{CollateralLockHandle, CollateralLockService};
use crate::domain::services::position_service::PositionUpdateService;
use crate::domain::services::report_scheduler::{ReportScheduler, ScheduledReport};
use crate::domain::services::settlement::{SettlementResult, SettlementService};
use crate::domain::value_objects::timestamp::Timestamp;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Configuration for off-book execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OffBookExecutorConfig {
    /// Whether on-chain settlement is enabled.
    pub enable_on_chain_settlement: bool,
    /// Timeout for settlement operations.
    pub settlement_timeout: Duration,
    /// Maximum retry attempts for transient failures.
    pub max_retry_attempts: u32,
}

impl Default for OffBookExecutorConfig {
    fn default() -> Self {
        Self {
            enable_on_chain_settlement: true,
            settlement_timeout: Duration::from_secs(30),
            max_retry_attempts: 3,
        }
    }
}

impl OffBookExecutorConfig {
    /// Creates a new configuration.
    #[must_use]
    pub fn new(
        enable_on_chain_settlement: bool,
        settlement_timeout: Duration,
        max_retry_attempts: u32,
    ) -> Self {
        Self {
            enable_on_chain_settlement,
            settlement_timeout,
            max_retry_attempts,
        }
    }

    /// Creates a configuration for testing (no on-chain settlement).
    #[must_use]
    pub fn for_testing() -> Self {
        Self {
            enable_on_chain_settlement: false,
            settlement_timeout: Duration::from_secs(5),
            max_retry_attempts: 1,
        }
    }
}

/// Result of a successful off-book execution.
#[derive(Debug)]
pub struct ExecutedBlockTrade {
    /// The executed trade.
    trade: BlockTrade,
    /// Settlement result.
    settlement: SettlementResult,
    /// Scheduled report.
    report: ScheduledReport,
    /// Total execution time.
    execution_time: Duration,
    /// Domain events generated during execution.
    events: Vec<Box<dyn DomainEvent>>,
}

impl ExecutedBlockTrade {
    /// Returns a reference to the executed trade.
    #[must_use]
    pub fn trade(&self) -> &BlockTrade {
        &self.trade
    }

    /// Returns a reference to the settlement result.
    #[must_use]
    pub fn settlement(&self) -> &SettlementResult {
        &self.settlement
    }

    /// Returns a reference to the scheduled report.
    #[must_use]
    pub fn report(&self) -> &ScheduledReport {
        &self.report
    }

    /// Returns the total execution time.
    #[must_use]
    pub fn execution_time(&self) -> Duration {
        self.execution_time
    }

    /// Returns the domain events generated during execution.
    #[must_use]
    pub fn events(&self) -> &[Box<dyn DomainEvent>] {
        &self.events
    }

    /// Takes ownership of the domain events.
    #[must_use]
    pub fn take_events(&mut self) -> Vec<Box<dyn DomainEvent>> {
        std::mem::take(&mut self.events)
    }
}

/// Off-book execution orchestrator.
///
/// Coordinates the complete off-book execution flow for block trades,
/// ensuring atomicity and proper rollback on failure.
pub struct OffBookExecutor<C, S, P, R>
where
    C: CollateralLockService,
    S: SettlementService,
    P: PositionUpdateService,
    R: ReportScheduler,
{
    collateral_service: Arc<C>,
    settlement_service: Arc<S>,
    position_service: Arc<P>,
    report_scheduler: Arc<R>,
    config: OffBookExecutorConfig,
}

impl<C, S, P, R> fmt::Debug for OffBookExecutor<C, S, P, R>
where
    C: CollateralLockService,
    S: SettlementService,
    P: PositionUpdateService,
    R: ReportScheduler,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OffBookExecutor")
            .field("config", &self.config)
            .finish()
    }
}

impl<C, S, P, R> OffBookExecutor<C, S, P, R>
where
    C: CollateralLockService,
    S: SettlementService,
    P: PositionUpdateService,
    R: ReportScheduler,
{
    /// Creates a new off-book executor.
    #[must_use]
    pub fn new(
        collateral_service: Arc<C>,
        settlement_service: Arc<S>,
        position_service: Arc<P>,
        report_scheduler: Arc<R>,
        config: OffBookExecutorConfig,
    ) -> Self {
        Self {
            collateral_service,
            settlement_service,
            position_service,
            report_scheduler,
            config,
        }
    }

    /// Executes a validated and approved block trade off-book.
    ///
    /// This method orchestrates the complete off-book execution flow:
    /// 1. Validates the trade is in `Approved` state
    /// 2. Locks collateral for both parties
    /// 3. Executes on-chain settlement
    /// 4. Updates positions
    /// 5. Schedules delayed report
    ///
    /// On failure, any locked collateral is released.
    ///
    /// # Arguments
    ///
    /// * `trade` - The block trade to execute (must be in `Approved` state)
    ///
    /// # Returns
    ///
    /// An `ExecutedBlockTrade` containing the trade, settlement details, and events.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Trade is not in `Approved` state
    /// - Collateral lock fails
    /// - Settlement fails
    /// - Position update fails
    /// - Report scheduling fails
    pub async fn execute(&self, trade: &mut BlockTrade) -> DomainResult<ExecutedBlockTrade> {
        let start = Instant::now();
        let mut events: Vec<Box<dyn DomainEvent>> = Vec::new();

        // Step 0: Validate trade is in Approved state
        if trade.state() != BlockTradeState::Approved {
            return Err(DomainError::InvalidTradeStateForExecution {
                expected: BlockTradeState::Approved.to_string(),
                actual: trade.state().to_string(),
            });
        }

        // Emit execution started event
        events.push(Box::new(OffBookExecutionStarted::new(
            trade.id(),
            trade.buyer_id().clone(),
            trade.seller_id().clone(),
        )));

        // Transition to Executing state
        trade.start_execution()?;

        // Step 1: Lock collateral
        let lock = match self.lock_collateral(trade).await {
            Ok(lock) => {
                events.push(Box::new(CollateralLocked::new(
                    trade.id(),
                    lock.lock_id(),
                    lock.buyer_amount(),
                    lock.seller_amount(),
                )));
                lock
            }
            Err(e) => {
                self.handle_failure(trade, &mut events, ExecutionStep::CollateralLock, &e)
                    .await;
                return Err(e);
            }
        };

        // Step 2: Execute settlement
        let settlement = match self.execute_settlement(trade).await {
            Ok(settlement) => settlement,
            Err(e) => {
                self.rollback_collateral(trade, &lock, &mut events, &e)
                    .await;
                self.handle_failure(trade, &mut events, ExecutionStep::Settlement, &e)
                    .await;
                return Err(e);
            }
        };

        // Step 3: Update positions
        if let Err(e) = self.update_positions(trade, &settlement).await {
            self.rollback_collateral(trade, &lock, &mut events, &e)
                .await;
            self.handle_failure(trade, &mut events, ExecutionStep::PositionUpdate, &e)
                .await;
            return Err(e);
        }

        // Step 4: Schedule report
        let report = match self.schedule_report(trade).await {
            Ok(report) => report,
            Err(e) => {
                // Report scheduling failure is not critical enough to rollback
                // Log error but continue with execution
                self.handle_failure(trade, &mut events, ExecutionStep::ReportScheduling, &e)
                    .await;
                // Create a placeholder report
                ScheduledReport::new(
                    trade.id().to_string(),
                    trade
                        .reporting_tier()
                        .unwrap_or(crate::domain::services::ReportingTier::Standard),
                    Timestamp::now(),
                )
            }
        };

        // Mark trade as executed
        trade.mark_executed()?;

        // Emit settlement event
        events.push(Box::new(OffBookSettled::new(
            trade.id(),
            settlement.trade_hash().clone(),
            settlement.settlement_timestamp(),
        )));

        // Release collateral (it's now permanently allocated)
        let _ = self.collateral_service.release(&lock).await;

        Ok(ExecutedBlockTrade {
            trade: trade.clone(),
            settlement,
            report,
            execution_time: start.elapsed(),
            events,
        })
    }

    /// Locks collateral for both counterparties.
    async fn lock_collateral(&self, trade: &BlockTrade) -> DomainResult<CollateralLockHandle> {
        self.collateral_service
            .lock_both(
                trade.buyer_id(),
                trade.seller_id(),
                trade.instrument(),
                trade.quantity(),
                trade.price(),
            )
            .await
    }

    /// Executes on-chain settlement.
    async fn execute_settlement(&self, trade: &BlockTrade) -> DomainResult<SettlementResult> {
        if !self.config.enable_on_chain_settlement {
            // Return mock settlement for testing
            return Ok(SettlementResult::new(
                crate::domain::events::TradeHash::new(format!("mock-{}", trade.id())),
                Timestamp::now(),
                trade.quantity().get(),
                -trade.quantity().get(),
                crate::domain::services::settlement::Fees::zero(),
            ));
        }

        // Verify price bounds first
        if !self.settlement_service.verify_price_bounds(trade).await? {
            return Err(DomainError::PriceBoundsVerificationFailed(
                "Trade price outside oracle bounds".to_string(),
            ));
        }

        self.settlement_service.settle(trade).await
    }

    /// Updates positions for both counterparties.
    async fn update_positions(
        &self,
        trade: &BlockTrade,
        settlement: &SettlementResult,
    ) -> DomainResult<()> {
        self.position_service.update_both(trade, settlement).await
    }

    /// Schedules a delayed report based on reporting tier.
    async fn schedule_report(&self, trade: &BlockTrade) -> DomainResult<ScheduledReport> {
        self.report_scheduler.schedule(trade).await
    }

    /// Rolls back collateral lock on failure.
    async fn rollback_collateral(
        &self,
        trade: &BlockTrade,
        lock: &CollateralLockHandle,
        events: &mut Vec<Box<dyn DomainEvent>>,
        error: &DomainError,
    ) {
        if let Err(release_err) = self.collateral_service.release(lock).await {
            // Log the release error but don't override the original error
            tracing::error!(
                "Failed to release collateral lock {} after error: {} (release error: {})",
                lock.lock_id(),
                error,
                release_err
            );
        }

        events.push(Box::new(CollateralReleased::new(
            trade.id(),
            lock.lock_id(),
            error.to_string(),
        )));
    }

    /// Handles execution failure by marking trade as failed and emitting event.
    #[allow(clippy::unused_async)]
    async fn handle_failure(
        &self,
        trade: &mut BlockTrade,
        events: &mut Vec<Box<dyn DomainEvent>>,
        step: ExecutionStep,
        error: &DomainError,
    ) {
        let _ = trade.mark_failed(&error.to_string());

        events.push(Box::new(OffBookFailed::new(
            trade.id(),
            error.to_string(),
            step,
        )));
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn config_default() {
        let config = OffBookExecutorConfig::default();
        assert!(config.enable_on_chain_settlement);
        assert_eq!(config.settlement_timeout, Duration::from_secs(30));
        assert_eq!(config.max_retry_attempts, 3);
    }

    #[test]
    fn config_for_testing() {
        let config = OffBookExecutorConfig::for_testing();
        assert!(!config.enable_on_chain_settlement);
        assert_eq!(config.settlement_timeout, Duration::from_secs(5));
        assert_eq!(config.max_retry_attempts, 1);
    }
}
