//! # Multi-Leg Executor Service
//!
//! Orchestrates atomic all-or-none execution of multi-leg strategies.
//!
//! This module provides the [`MultiLegExecutor`] service which ensures that
//! either all legs of a multi-leg strategy execute successfully, or none do.
//! If any leg fails, all previously executed legs are rolled back using
//! compensating trades.
//!
//! # Architecture
//!
//! The executor follows the saga pattern for distributed transaction coordination:
//!
//! ```text
//! 1. Start execution (state: InProgress)
//! 2. Execute legs sequentially
//!    - On success: continue to next leg
//!    - On failure: trigger rollback
//! 3. On all success: state = Completed
//! 4. On failure: rollback all executed legs
//!    - Rollback success: state = RolledBack
//!    - Rollback failure: state = PartialFailure
//! ```
//!
//! # Example
//!
//! ```ignore
//! let executor = MultiLegExecutor::new(config, leg_executor);
//! let result = executor.execute(&package_quote).await;
//! match result {
//!     Ok(execution) => println!("All legs executed: {:?}", execution),
//!     Err(e) => println!("Execution failed: {}", e),
//! }
//! ```

use crate::domain::entities::package_quote::{LegPrice, PackageQuote};
use crate::domain::errors::{DomainError, DomainResult};
use crate::domain::events::{
    LegExecuted, LegFailed, MultiLegExecutionCompleted, MultiLegExecutionEvent,
    MultiLegExecutionId, MultiLegExecutionStarted, MultiLegPartialFailure,
    MultiLegRollbackCompleted, MultiLegRollbackStarted,
};
use crate::domain::services::compensating_trade::{
    CompensatingTrade, CompensatingTradeGenerator, LegExecutionResult,
};
use crate::domain::value_objects::Timestamp;
use async_trait::async_trait;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::Arc;
use std::time::Duration;

/// Configuration for the multi-leg executor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiLegExecutorConfig {
    /// Timeout for executing a single leg.
    pub leg_timeout: Duration,
    /// Overall timeout for the entire multi-leg execution.
    pub overall_timeout: Duration,
    /// Maximum number of retry attempts for rollback operations.
    pub max_rollback_attempts: u32,
}

impl Default for MultiLegExecutorConfig {
    fn default() -> Self {
        Self {
            leg_timeout: Duration::from_secs(5),
            overall_timeout: Duration::from_secs(30),
            max_rollback_attempts: 3,
        }
    }
}

impl MultiLegExecutorConfig {
    /// Creates a new configuration.
    #[must_use]
    pub fn new(
        leg_timeout: Duration,
        overall_timeout: Duration,
        max_rollback_attempts: u32,
    ) -> Self {
        Self {
            leg_timeout,
            overall_timeout,
            max_rollback_attempts,
        }
    }

    /// Creates a configuration for testing with shorter timeouts.
    #[must_use]
    pub fn for_testing() -> Self {
        Self {
            leg_timeout: Duration::from_millis(100),
            overall_timeout: Duration::from_secs(1),
            max_rollback_attempts: 1,
        }
    }
}

/// State of a multi-leg execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MultiLegExecutionState {
    /// Execution has not started.
    Pending,
    /// Execution is in progress.
    InProgress {
        /// Number of legs completed so far.
        completed_legs: usize,
        /// Total number of legs.
        total_legs: usize,
    },
    /// All legs executed successfully.
    Completed,
    /// Rollback is in progress.
    RollingBack {
        /// Number of legs rolled back so far.
        rolled_back: usize,
        /// Total number of legs to roll back.
        total: usize,
    },
    /// All executed legs were successfully rolled back.
    RolledBack,
    /// Partial failure: some legs could not be rolled back.
    PartialFailure {
        /// Number of legs that were executed.
        executed: usize,
        /// Number of legs successfully rolled back.
        rolled_back: usize,
        /// Number of legs that failed to roll back.
        failed_rollback: usize,
    },
}

impl fmt::Display for MultiLegExecutionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pending => write!(f, "Pending"),
            Self::InProgress {
                completed_legs,
                total_legs,
            } => write!(f, "InProgress({}/{})", completed_legs, total_legs),
            Self::Completed => write!(f, "Completed"),
            Self::RollingBack { rolled_back, total } => {
                write!(f, "RollingBack({}/{})", rolled_back, total)
            }
            Self::RolledBack => write!(f, "RolledBack"),
            Self::PartialFailure {
                executed,
                rolled_back,
                failed_rollback,
            } => write!(
                f,
                "PartialFailure(executed={}, rolled_back={}, failed={})",
                executed, rolled_back, failed_rollback
            ),
        }
    }
}

/// Result of a successful multi-leg execution.
#[derive(Debug, Clone)]
pub struct MultiLegExecutionResult {
    /// Unique execution ID.
    pub execution_id: MultiLegExecutionId,
    /// Results for each executed leg.
    pub legs: Vec<LegExecutionResult>,
    /// Net price for the entire package.
    pub net_price: Decimal,
    /// Final state of the execution.
    pub state: MultiLegExecutionState,
    /// When execution started.
    pub started_at: Timestamp,
    /// When execution completed.
    pub completed_at: Timestamp,
    /// Events emitted during execution.
    pub events: Vec<MultiLegExecutionEvent>,
}

impl MultiLegExecutionResult {
    /// Creates a new execution result.
    #[must_use]
    pub fn new(
        execution_id: MultiLegExecutionId,
        legs: Vec<LegExecutionResult>,
        net_price: Decimal,
        started_at: Timestamp,
    ) -> Self {
        Self {
            execution_id,
            legs,
            net_price,
            state: MultiLegExecutionState::Completed,
            started_at,
            completed_at: Timestamp::now(),
            events: Vec::new(),
        }
    }

    /// Returns the total number of legs executed.
    #[must_use]
    #[inline]
    pub fn leg_count(&self) -> usize {
        self.legs.len()
    }

    /// Returns the execution duration in milliseconds.
    #[must_use]
    pub fn duration_ms(&self) -> u64 {
        self.started_at
            .duration_until(&self.completed_at)
            .as_millis() as u64
    }
}

/// Trait for executing individual legs.
///
/// This abstraction allows for different leg execution strategies
/// (e.g., venue-specific, mock for testing).
#[async_trait]
pub trait LegExecutor: Send + Sync + fmt::Debug {
    /// Executes a single leg.
    ///
    /// # Arguments
    ///
    /// * `leg` - The leg price information
    /// * `leg_index` - Index of the leg in the strategy
    /// * `timeout` - Maximum time to wait for execution
    ///
    /// # Returns
    ///
    /// The execution result on success.
    ///
    /// # Errors
    ///
    /// Returns an error if execution fails or times out.
    async fn execute_leg(
        &self,
        leg: &LegPrice,
        leg_index: usize,
        timeout: Duration,
    ) -> DomainResult<LegExecutionResult>;

    /// Executes a compensating trade to reverse an executed leg.
    ///
    /// # Arguments
    ///
    /// * `compensating` - The compensating trade to execute
    ///
    /// # Errors
    ///
    /// Returns an error if the compensating trade fails.
    async fn execute_compensating(&self, compensating: &CompensatingTrade) -> DomainResult<()>;
}

/// Multi-leg executor service.
///
/// Orchestrates atomic all-or-none execution of multi-leg strategies.
pub struct MultiLegExecutor<E: LegExecutor> {
    /// Configuration.
    config: MultiLegExecutorConfig,
    /// Leg executor implementation.
    leg_executor: Arc<E>,
    /// Compensating trade generator.
    compensator: CompensatingTradeGenerator,
}

impl<E: LegExecutor> fmt::Debug for MultiLegExecutor<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MultiLegExecutor")
            .field("config", &self.config)
            .field("leg_executor", &self.leg_executor)
            .finish()
    }
}

impl<E: LegExecutor> MultiLegExecutor<E> {
    /// Creates a new multi-leg executor.
    #[must_use]
    pub fn new(config: MultiLegExecutorConfig, leg_executor: Arc<E>) -> Self {
        Self {
            config,
            leg_executor,
            compensator: CompensatingTradeGenerator::new(),
        }
    }

    /// Creates a new executor with default configuration.
    #[must_use]
    pub fn with_defaults(leg_executor: Arc<E>) -> Self {
        Self::new(MultiLegExecutorConfig::default(), leg_executor)
    }

    /// Returns the configuration.
    #[must_use]
    pub fn config(&self) -> &MultiLegExecutorConfig {
        &self.config
    }

    /// Executes all legs of a package quote atomically.
    ///
    /// Either all legs execute successfully, or all executed legs are rolled back.
    ///
    /// # Arguments
    ///
    /// * `package_quote` - The package quote to execute
    ///
    /// # Returns
    ///
    /// The execution result containing all leg results.
    ///
    /// # Errors
    ///
    /// - `MultiLegExecutionFailed` if a leg fails and rollback succeeds
    /// - `RollbackFailed` if rollback also fails (partial failure)
    /// - `LegExecutionTimeout` if a leg times out
    pub async fn execute(
        &self,
        package_quote: &PackageQuote,
    ) -> DomainResult<MultiLegExecutionResult> {
        let overall_timeout = self.config.overall_timeout;

        match tokio::time::timeout(overall_timeout, self.execute_inner(package_quote)).await {
            Ok(result) => result,
            Err(_) => Err(DomainError::LegExecutionTimeout {
                leg_index: 0,
                instrument: "overall execution".to_string(),
                timeout_ms: self.config.overall_timeout.as_millis() as u64,
            }),
        }
    }

    /// Inner execution logic wrapped by overall timeout.
    async fn execute_inner(
        &self,
        package_quote: &PackageQuote,
    ) -> DomainResult<MultiLegExecutionResult> {
        let execution_id = MultiLegExecutionId::generate();
        let started_at = Timestamp::now();
        let leg_prices = package_quote.leg_prices();
        let total_legs = leg_prices.len();
        let mut events: Vec<MultiLegExecutionEvent> = Vec::new();

        if total_legs == 0 {
            return Ok(MultiLegExecutionResult {
                execution_id,
                legs: Vec::new(),
                net_price: Decimal::ZERO,
                state: MultiLegExecutionState::Completed,
                started_at,
                completed_at: Timestamp::now(),
                events,
            });
        }

        let start_event = MultiLegExecutionStarted::new(
            execution_id.clone(),
            package_quote.id(),
            total_legs,
            package_quote.net_price(),
        );
        events.push(start_event.into());

        let mut executed_legs: Vec<LegExecutionResult> = Vec::with_capacity(total_legs);

        for (leg_index, leg) in leg_prices.iter().enumerate() {
            match self
                .leg_executor
                .execute_leg(leg, leg_index, self.config.leg_timeout)
                .await
            {
                Ok(result) => {
                    let leg_event = LegExecuted::new(
                        execution_id.clone(),
                        leg_index,
                        leg.instrument().clone(),
                        leg.side(),
                        leg.price(),
                        leg.quantity(),
                        result.execution_id.clone(),
                    );
                    events.push(leg_event.into());
                    executed_legs.push(result);
                }
                Err(e) => {
                    let fail_event = LegFailed::new(
                        execution_id.clone(),
                        leg_index,
                        leg.instrument().clone(),
                        e.to_string(),
                        executed_legs.len(),
                    );
                    events.push(fail_event.into());

                    return self
                        .handle_failure(
                            execution_id,
                            &executed_legs,
                            leg_index,
                            leg.instrument().symbol().to_string(),
                            e.to_string(),
                            events,
                            started_at,
                            package_quote.net_price(),
                        )
                        .await;
                }
            }
        }

        let completed_event = MultiLegExecutionCompleted::new(
            execution_id.clone(),
            package_quote.id(),
            total_legs,
            package_quote.net_price(),
            started_at.duration_until(&Timestamp::now()).as_millis() as u64,
        );
        events.push(completed_event.into());

        Ok(MultiLegExecutionResult {
            execution_id,
            legs: executed_legs,
            net_price: package_quote.net_price(),
            state: MultiLegExecutionState::Completed,
            started_at,
            completed_at: Timestamp::now(),
            events,
        })
    }

    /// Handles a leg execution failure by rolling back all executed legs.
    ///
    /// Returns `Ok(MultiLegExecutionResult)` with state `RolledBack` or `PartialFailure`.
    /// Events are always preserved in the result for audit trail.
    #[allow(clippy::too_many_arguments)]
    async fn handle_failure(
        &self,
        execution_id: MultiLegExecutionId,
        executed_legs: &[LegExecutionResult],
        failed_leg_index: usize,
        failed_leg_instrument: String,
        failure_reason: String,
        mut events: Vec<MultiLegExecutionEvent>,
        started_at: Timestamp,
        net_price: Decimal,
    ) -> DomainResult<MultiLegExecutionResult> {
        // Enrich failure reason with leg context
        let enriched_reason = format!(
            "Leg {} ({}) failed: {}",
            failed_leg_index, failed_leg_instrument, failure_reason
        );
        if executed_legs.is_empty() {
            // No legs to roll back - return result with RolledBack state
            return Ok(MultiLegExecutionResult {
                execution_id,
                legs: Vec::new(),
                net_price,
                state: MultiLegExecutionState::RolledBack,
                started_at,
                completed_at: Timestamp::now(),
                events,
            });
        }

        let rollback_start = MultiLegRollbackStarted::new(
            execution_id.clone(),
            enriched_reason.clone(),
            executed_legs.len(),
        );
        events.push(rollback_start.into());

        match self.rollback_all(executed_legs, &enriched_reason).await {
            Ok(rolled_back_count) => {
                let rollback_complete =
                    MultiLegRollbackCompleted::new(execution_id.clone(), rolled_back_count);
                events.push(rollback_complete.into());

                // Successful rollback - return Ok with RolledBack state
                Ok(MultiLegExecutionResult {
                    execution_id,
                    legs: executed_legs.to_vec(),
                    net_price,
                    state: MultiLegExecutionState::RolledBack,
                    started_at,
                    completed_at: Timestamp::now(),
                    events,
                })
            }
            Err(rollback_error) => {
                // Extract partially_rolled_back from the RollbackFailed error
                let partially_rolled_back = match &rollback_error {
                    DomainError::RollbackFailed {
                        partially_rolled_back,
                        ..
                    } => *partially_rolled_back,
                    _ => 0,
                };

                let partial_failure = MultiLegPartialFailure::new(
                    execution_id.clone(),
                    executed_legs.len(),
                    partially_rolled_back,
                    executed_legs.len() - partially_rolled_back,
                    enriched_reason.clone(),
                    rollback_error.to_string(),
                );
                events.push(partial_failure.into());

                // Partial rollback failure - return Ok with PartialFailure state
                Ok(MultiLegExecutionResult {
                    execution_id,
                    legs: executed_legs.to_vec(),
                    net_price,
                    state: MultiLegExecutionState::PartialFailure {
                        executed: executed_legs.len(),
                        rolled_back: partially_rolled_back,
                        failed_rollback: executed_legs.len() - partially_rolled_back,
                    },
                    started_at,
                    completed_at: Timestamp::now(),
                    events,
                })
            }
        }
    }

    /// Rolls back all executed legs using compensating trades.
    ///
    /// Legs are rolled back in reverse order (LIFO).
    async fn rollback_all(
        &self,
        executed_legs: &[LegExecutionResult],
        original_failure: &str,
    ) -> DomainResult<usize> {
        let compensating_trades = self.compensator.generate_all(executed_legs);
        let mut rolled_back = 0;

        for compensating in &compensating_trades {
            let mut attempts = 0;
            let mut last_error = None;

            while attempts < self.config.max_rollback_attempts {
                match self.leg_executor.execute_compensating(compensating).await {
                    Ok(()) => {
                        rolled_back += 1;
                        last_error = None;
                        break;
                    }
                    Err(e) => {
                        attempts += 1;
                        last_error = Some(e);
                    }
                }
            }

            if let Some(e) = last_error {
                return Err(DomainError::RollbackFailed {
                    original_failure: original_failure.to_string(),
                    rollback_failure: e.to_string(),
                    partially_rolled_back: rolled_back,
                });
            }
        }

        Ok(rolled_back)
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;
    use crate::domain::entities::package_quote::PackageQuoteBuilder;
    use crate::domain::value_objects::strategy::{Strategy, StrategyLeg, StrategyType};
    use crate::domain::value_objects::{
        AssetClass, Instrument, OrderSide, Price, Quantity, RfqId, Symbol, VenueId,
    };
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Debug)]
    struct MockLegExecutor {
        fail_at_leg: Option<usize>,
        fail_compensating: bool,
        executed_count: AtomicUsize,
        compensated_count: AtomicUsize,
        execution_delay: Duration,
    }

    impl MockLegExecutor {
        fn new() -> Self {
            Self {
                fail_at_leg: None,
                fail_compensating: false,
                executed_count: AtomicUsize::new(0),
                compensated_count: AtomicUsize::new(0),
                execution_delay: Duration::from_millis(1),
            }
        }

        fn fail_at(mut self, leg_index: usize) -> Self {
            self.fail_at_leg = Some(leg_index);
            self
        }

        fn fail_compensating(mut self) -> Self {
            self.fail_compensating = true;
            self
        }

        fn executed_count(&self) -> usize {
            self.executed_count.load(Ordering::SeqCst)
        }

        fn compensated_count(&self) -> usize {
            self.compensated_count.load(Ordering::SeqCst)
        }
    }

    #[async_trait]
    impl LegExecutor for MockLegExecutor {
        async fn execute_leg(
            &self,
            leg: &LegPrice,
            leg_index: usize,
            _timeout: Duration,
        ) -> DomainResult<LegExecutionResult> {
            tokio::time::sleep(self.execution_delay).await;

            if self.fail_at_leg == Some(leg_index) {
                return Err(DomainError::ValidationError(format!(
                    "Mock failure at leg {}",
                    leg_index
                )));
            }

            self.executed_count.fetch_add(1, Ordering::SeqCst);

            Ok(LegExecutionResult::new(
                leg_index,
                leg.instrument().clone(),
                leg.side(),
                leg.price(),
                leg.quantity(),
                format!("mock-exec-{}", leg_index),
            ))
        }

        async fn execute_compensating(
            &self,
            _compensating: &CompensatingTrade,
        ) -> DomainResult<()> {
            if self.fail_compensating {
                return Err(DomainError::ValidationError(
                    "Mock compensating failure".to_string(),
                ));
            }

            self.compensated_count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    fn create_test_instrument() -> Instrument {
        let symbol = Symbol::new("BTC/USD").unwrap();
        Instrument::builder(symbol, AssetClass::CryptoSpot).build()
    }

    fn create_test_strategy(num_legs: usize) -> Strategy {
        let legs: Vec<StrategyLeg> = (0..num_legs)
            .map(|i| {
                let side = if i % 2 == 0 {
                    OrderSide::Buy
                } else {
                    OrderSide::Sell
                };
                StrategyLeg::new(create_test_instrument(), side, 1).unwrap()
            })
            .collect();

        Strategy::new(StrategyType::Custom, legs, "BTC", None).unwrap()
    }

    fn create_test_package_quote(num_legs: usize) -> PackageQuote {
        let strategy = create_test_strategy(num_legs);
        let mut builder = PackageQuoteBuilder::new(
            RfqId::new_v4(),
            VenueId::new("test-venue"),
            strategy,
            Decimal::from(100),
            Timestamp::now().add_secs(300),
        );

        for i in 0..num_legs {
            let side = if i % 2 == 0 {
                OrderSide::Buy
            } else {
                OrderSide::Sell
            };
            let leg_price = LegPrice::new(
                create_test_instrument(),
                side,
                Price::new(50000.0).unwrap(),
                Quantity::new(1.0).unwrap(),
            )
            .unwrap();
            builder = builder.leg_price(leg_price);
        }

        builder.build().unwrap()
    }

    #[tokio::test]
    async fn test_execute_all_legs_success() {
        let executor = MockLegExecutor::new();
        let multi_leg =
            MultiLegExecutor::new(MultiLegExecutorConfig::for_testing(), Arc::new(executor));
        let package_quote = create_test_package_quote(3);

        let result = multi_leg.execute(&package_quote).await;

        assert!(result.is_ok());
        let execution = result.unwrap();
        assert_eq!(execution.legs.len(), 3);
        assert_eq!(execution.state, MultiLegExecutionState::Completed);
    }

    #[tokio::test]
    async fn test_execute_first_leg_fails() {
        let executor = MockLegExecutor::new().fail_at(0);
        let multi_leg =
            MultiLegExecutor::new(MultiLegExecutorConfig::for_testing(), Arc::new(executor));
        let package_quote = create_test_package_quote(3);

        let result = multi_leg.execute(&package_quote).await;

        assert!(result.is_ok());
        let execution = result.unwrap();
        assert_eq!(execution.state, MultiLegExecutionState::RolledBack);
        assert!(execution.legs.is_empty());
        assert!(!execution.events.is_empty());
    }

    #[tokio::test]
    async fn test_execute_middle_leg_fails_rollback_success() {
        let executor = Arc::new(MockLegExecutor::new().fail_at(1));
        let multi_leg =
            MultiLegExecutor::new(MultiLegExecutorConfig::for_testing(), Arc::clone(&executor));
        let package_quote = create_test_package_quote(3);

        let result = multi_leg.execute(&package_quote).await;

        assert!(result.is_ok());
        let execution = result.unwrap();
        assert_eq!(execution.state, MultiLegExecutionState::RolledBack);
        assert_eq!(execution.legs.len(), 1);
        assert!(!execution.events.is_empty());
        assert_eq!(executor.executed_count(), 1);
        assert_eq!(executor.compensated_count(), 1);
    }

    #[tokio::test]
    async fn test_execute_last_leg_fails_rollback_success() {
        let executor = Arc::new(MockLegExecutor::new().fail_at(2));
        let multi_leg =
            MultiLegExecutor::new(MultiLegExecutorConfig::for_testing(), Arc::clone(&executor));
        let package_quote = create_test_package_quote(3);

        let result = multi_leg.execute(&package_quote).await;

        assert!(result.is_ok());
        let execution = result.unwrap();
        assert_eq!(execution.state, MultiLegExecutionState::RolledBack);
        assert_eq!(execution.legs.len(), 2);
        assert!(!execution.events.is_empty());
        assert_eq!(executor.executed_count(), 2);
        assert_eq!(executor.compensated_count(), 2);
    }

    #[tokio::test]
    async fn test_rollback_fails_partial_failure() {
        let executor = Arc::new(MockLegExecutor::new().fail_at(2).fail_compensating());
        let multi_leg =
            MultiLegExecutor::new(MultiLegExecutorConfig::for_testing(), Arc::clone(&executor));
        let package_quote = create_test_package_quote(3);

        let result = multi_leg.execute(&package_quote).await;

        assert!(result.is_ok());
        let execution = result.unwrap();
        match execution.state {
            MultiLegExecutionState::PartialFailure {
                executed,
                rolled_back,
                failed_rollback,
            } => {
                assert_eq!(executed, 2);
                assert_eq!(rolled_back, 0);
                assert_eq!(failed_rollback, 2);
            }
            _ => panic!("Expected PartialFailure state"),
        }
        assert!(!execution.events.is_empty());
    }

    #[tokio::test]
    async fn test_single_leg_success() {
        let executor = MockLegExecutor::new();
        let multi_leg =
            MultiLegExecutor::new(MultiLegExecutorConfig::for_testing(), Arc::new(executor));
        let package_quote = create_test_package_quote(1);

        let result = multi_leg.execute(&package_quote).await;

        assert!(result.is_ok());
        let execution = result.unwrap();
        assert_eq!(execution.legs.len(), 1);
    }

    #[tokio::test]
    async fn test_single_leg_failure() {
        let executor = MockLegExecutor::new().fail_at(0);
        let multi_leg =
            MultiLegExecutor::new(MultiLegExecutorConfig::for_testing(), Arc::new(executor));
        let package_quote = create_test_package_quote(1);

        let result = multi_leg.execute(&package_quote).await;

        assert!(result.is_ok());
        let execution = result.unwrap();
        assert_eq!(execution.state, MultiLegExecutionState::RolledBack);
        assert!(execution.legs.is_empty());
        assert!(!execution.events.is_empty());
    }

    #[tokio::test]
    async fn test_events_preserved_on_failure_paths() {
        // Test 1: Events preserved when first leg fails (no rollback needed)
        let executor = MockLegExecutor::new().fail_at(0);
        let multi_leg =
            MultiLegExecutor::new(MultiLegExecutorConfig::for_testing(), Arc::new(executor));
        let package_quote = create_test_package_quote(3);

        let result = multi_leg.execute(&package_quote).await;
        assert!(result.is_ok());
        let execution = result.unwrap();
        assert!(execution.events.len() >= 2, "Expected at least 2 events");
        assert!(matches!(
            execution.events[0],
            MultiLegExecutionEvent::Started(_)
        ));
        assert!(matches!(
            execution.events[1],
            MultiLegExecutionEvent::LegFailed(_)
        ));

        // Test 2: Events preserved when middle leg fails with successful rollback
        let executor = Arc::new(MockLegExecutor::new().fail_at(1));
        let multi_leg =
            MultiLegExecutor::new(MultiLegExecutorConfig::for_testing(), Arc::clone(&executor));
        let package_quote = create_test_package_quote(3);

        let result = multi_leg.execute(&package_quote).await;
        assert!(result.is_ok());
        let execution = result.unwrap();
        assert!(!execution.events.is_empty());
        // Should have: Started, LegExecuted, LegFailed, RollbackStarted, RollbackCompleted
        assert!(execution.events.len() >= 5);

        // Test 3: Events preserved when rollback fails (partial failure)
        let executor = Arc::new(MockLegExecutor::new().fail_at(2).fail_compensating());
        let multi_leg =
            MultiLegExecutor::new(MultiLegExecutorConfig::for_testing(), Arc::clone(&executor));
        let package_quote = create_test_package_quote(3);

        let result = multi_leg.execute(&package_quote).await;
        assert!(result.is_ok());
        let execution = result.unwrap();
        assert!(!execution.events.is_empty());
        // Should have: Started, LegExecuted x2, LegFailed, RollbackStarted, PartialFailure
        assert!(execution.events.len() >= 5);
        assert!(matches!(
            execution.events.last().unwrap(),
            MultiLegExecutionEvent::PartialFailure(_)
        ));
    }

    #[test]
    fn test_execution_state_display() {
        assert_eq!(format!("{}", MultiLegExecutionState::Pending), "Pending");
        assert_eq!(
            format!("{}", MultiLegExecutionState::Completed),
            "Completed"
        );
        assert_eq!(
            format!(
                "{}",
                MultiLegExecutionState::InProgress {
                    completed_legs: 2,
                    total_legs: 5
                }
            ),
            "InProgress(2/5)"
        );
        assert_eq!(
            format!(
                "{}",
                MultiLegExecutionState::PartialFailure {
                    executed: 3,
                    rolled_back: 1,
                    failed_rollback: 2
                }
            ),
            "PartialFailure(executed=3, rolled_back=1, failed=2)"
        );
    }
}
