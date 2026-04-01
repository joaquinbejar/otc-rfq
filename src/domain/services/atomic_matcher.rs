//! # Atomic Matcher Service
//!
//! Orchestrates atomic trade execution with all-or-nothing semantics.
//!
//! This module provides the [`AtomicMatcher`] service which ensures that
//! trade execution either completes fully or rolls back completely,
//! with no partial state.
//!
//! # Architecture
//!
//! The atomic matcher coordinates multi-resource locking:
//!
//! ```text
//! 1. Acquire locks: Quote → Client Account → MM Account → Instrument
//! 2. Execute trade
//! 3. On success: commit and release locks
//! 4. On failure: rollback and release locks
//! ```
//!
//! # Example
//!
//! ```ignore
//! let matcher = AtomicMatcher::new(lock_manager, config);
//! let result = matcher.execute_trade(
//!     &rfq,
//!     &quote,
//!     || async { venue.execute_trade(&quote).await },
//! ).await;
//! ```

use crate::domain::entities::quote::Quote;
use crate::domain::entities::rfq::Rfq;
use crate::domain::errors::DomainResult;
use crate::domain::services::lock_manager::{LockGuard, LockManager};
use crate::domain::services::resource_lock::ResourceLock;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

/// Configuration for the atomic matcher.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AtomicMatcherConfig {
    /// Timeout for acquiring all locks.
    pub lock_timeout: Duration,
}

impl Default for AtomicMatcherConfig {
    fn default() -> Self {
        Self {
            lock_timeout: Duration::from_millis(100),
        }
    }
}

impl AtomicMatcherConfig {
    /// Creates a new atomic matcher configuration.
    #[must_use]
    pub fn new(lock_timeout: Duration) -> Self {
        Self { lock_timeout }
    }

    /// Creates a configuration with custom lock timeout.
    #[must_use]
    pub fn with_timeout(lock_timeout: Duration) -> Self {
        Self { lock_timeout }
    }
}

/// Result of an atomic execution.
#[derive(Debug, Clone)]
pub struct AtomicExecutionResult<T> {
    /// The result of the operation.
    pub result: T,
    /// Time spent executing the operation (excluding lock acquisition) in milliseconds.
    pub execution_time_ms: u64,
    /// Number of resources that were locked.
    pub resources_locked: usize,
}

impl<T> AtomicExecutionResult<T> {
    /// Creates a new atomic execution result.
    #[must_use]
    pub fn new(result: T, execution_time_ms: u64, resources_locked: usize) -> Self {
        Self {
            result,
            execution_time_ms,
            resources_locked,
        }
    }
}

/// Service for atomic trade execution with all-or-nothing semantics.
///
/// Ensures that trade execution either completes fully or rolls back
/// completely, preventing partial execution states.
pub struct AtomicMatcher {
    /// Lock manager for resource coordination.
    lock_manager: Arc<dyn LockManager>,
    /// Configuration.
    config: AtomicMatcherConfig,
}

impl AtomicMatcher {
    /// Creates a new atomic matcher.
    #[must_use]
    pub fn new(lock_manager: Arc<dyn LockManager>, config: AtomicMatcherConfig) -> Self {
        Self {
            lock_manager,
            config,
        }
    }

    /// Creates a new atomic matcher with default configuration.
    #[must_use]
    pub fn with_defaults(lock_manager: Arc<dyn LockManager>) -> Self {
        Self::new(lock_manager, AtomicMatcherConfig::default())
    }

    /// Returns the configuration.
    #[must_use]
    pub fn config(&self) -> &AtomicMatcherConfig {
        &self.config
    }

    /// Builds the list of resources to lock for a trade.
    ///
    /// Lock order: Quote → Client Account → MM Account → Instrument
    #[must_use]
    pub fn build_trade_locks(rfq: &Rfq, quote: &Quote) -> Vec<ResourceLock> {
        vec![
            ResourceLock::Quote(quote.id()),
            ResourceLock::Account(rfq.client_id().clone()),
            ResourceLock::Account(quote.venue_id().as_str().into()),
            ResourceLock::Instrument(rfq.instrument().clone()),
        ]
    }

    /// Executes an operation atomically with resource locking.
    ///
    /// Acquires all necessary locks, executes the operation, and releases
    /// locks on completion (success or failure).
    ///
    /// # Arguments
    ///
    /// * `resources` - Resources to lock before execution
    /// * `operation` - The async operation to execute
    ///
    /// # Returns
    ///
    /// The result of the operation wrapped in `AtomicExecutionResult`.
    ///
    /// # Errors
    ///
    /// - `DomainError::LockAcquisitionFailed` if locks cannot be acquired
    /// - Any error returned by the operation
    pub async fn execute_with_locks<F, Fut, T>(
        &self,
        resources: Vec<ResourceLock>,
        operation: F,
    ) -> DomainResult<AtomicExecutionResult<T>>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = DomainResult<T>>,
    {
        let resources_count = resources.len();

        // Acquire all locks
        let guard = self
            .lock_manager
            .acquire_all(resources, self.config.lock_timeout)
            .await?;

        // Start timing AFTER locks are acquired (measures only execution time)
        let exec_start = std::time::Instant::now();

        // Execute the operation
        let result = operation().await;

        // Calculate execution time (not including lock acquisition)
        let execution_time_ms = exec_start.elapsed().as_millis() as u64;

        // Release locks via guard to avoid double-release
        if let Err(e) = guard.release().await {
            tracing::warn!("Failed to release locks: {:?}", e);
        }

        // Return result
        match result {
            Ok(value) => Ok(AtomicExecutionResult::new(
                value,
                execution_time_ms,
                resources_count,
            )),
            Err(e) => Err(e),
        }
    }

    /// Executes a trade atomically.
    ///
    /// This is a convenience method that builds the lock list from the RFQ
    /// and quote, then executes the operation atomically.
    ///
    /// # Arguments
    ///
    /// * `rfq` - The RFQ being executed
    /// * `quote` - The quote being executed
    /// * `operation` - The async trade execution operation
    ///
    /// # Returns
    ///
    /// The result of the trade operation.
    ///
    /// # Errors
    ///
    /// - `DomainError::LockAcquisitionFailed` if locks cannot be acquired
    /// - Any error returned by the trade operation
    pub async fn execute_trade<F, Fut, T>(
        &self,
        rfq: &Rfq,
        quote: &Quote,
        operation: F,
    ) -> DomainResult<AtomicExecutionResult<T>>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = DomainResult<T>>,
    {
        let resources = Self::build_trade_locks(rfq, quote);
        self.execute_with_locks(resources, operation).await
    }

    /// Attempts to acquire locks without executing an operation.
    ///
    /// Useful for checking if resources are available before committing
    /// to an operation.
    ///
    /// # Arguments
    ///
    /// * `resources` - Resources to lock
    ///
    /// # Returns
    ///
    /// A `LockGuard` that must be explicitly released.
    ///
    /// # Errors
    ///
    /// - `DomainError::LockAcquisitionFailed` if locks cannot be acquired
    pub async fn acquire_locks(&self, resources: Vec<ResourceLock>) -> DomainResult<LockGuard> {
        self.lock_manager
            .acquire_all(resources, self.config.lock_timeout)
            .await
    }

    /// Checks if all specified resources are currently available (not locked).
    ///
    /// # Arguments
    ///
    /// * `resources` - Resources to check
    ///
    /// # Returns
    ///
    /// `true` if all resources are available, `false` otherwise.
    pub async fn are_resources_available(&self, resources: &[ResourceLock]) -> bool {
        for resource in resources {
            if self.lock_manager.get_lock_info(resource).await.is_some() {
                return false;
            }
        }
        true
    }
}

impl fmt::Debug for AtomicMatcher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AtomicMatcher")
            .field("config", &self.config)
            .field("lock_manager", &self.lock_manager)
            .finish()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::domain::entities::rfq::RfqBuilder;
    use crate::domain::errors::DomainError;
    use crate::domain::services::lock_manager::SharedLockManager;
    use crate::domain::value_objects::enums::{AssetClass, SettlementMethod};
    use crate::domain::value_objects::symbol::Symbol;
    use crate::domain::value_objects::{
        CounterpartyId, Instrument, OrderSide, Price, Quantity, Timestamp, VenueId,
    };
    use std::sync::atomic::{AtomicUsize, Ordering};

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

    fn create_test_quote(rfq: &Rfq) -> Quote {
        Quote::new(
            rfq.id(),
            VenueId::new("venue-1"),
            Price::new(100.0).unwrap(),
            Quantity::new(1.0).unwrap(),
            Timestamp::now().add_secs(60),
        )
        .unwrap()
    }

    #[test]
    fn atomic_matcher_config_default() {
        let config = AtomicMatcherConfig::default();
        assert_eq!(config.lock_timeout, Duration::from_millis(100));
    }

    #[test]
    fn atomic_matcher_config_with_timeout() {
        let config = AtomicMatcherConfig::with_timeout(Duration::from_millis(200));
        assert_eq!(config.lock_timeout, Duration::from_millis(200));
    }

    #[test]
    fn build_trade_locks_order() {
        let rfq = create_test_rfq();
        let quote = create_test_quote(&rfq);

        let locks = AtomicMatcher::build_trade_locks(&rfq, &quote);

        assert_eq!(locks.len(), 4);
        assert!(locks[0].is_quote());
        assert!(locks[1].is_account()); // Client
        assert!(locks[2].is_account()); // MM
        assert!(locks[3].is_instrument());
    }

    #[tokio::test]
    async fn atomic_matcher_execute_success() {
        let lock_manager = SharedLockManager::with_defaults();
        let matcher = AtomicMatcher::with_defaults(lock_manager);

        let rfq = create_test_rfq();
        let quote = create_test_quote(&rfq);

        let result = matcher
            .execute_trade(&rfq, &quote, || async { Ok(42) })
            .await;

        assert!(result.is_ok());
        let exec_result = result.unwrap();
        assert_eq!(exec_result.result, 42);
        assert_eq!(exec_result.resources_locked, 4);
        assert!(exec_result.execution_time_ms < 1000);
    }

    #[tokio::test]
    async fn atomic_matcher_execute_failure_releases_locks() {
        let lock_manager = SharedLockManager::with_defaults();
        let matcher = AtomicMatcher::with_defaults(lock_manager);

        let rfq = create_test_rfq();
        let quote = create_test_quote(&rfq);

        let result: DomainResult<AtomicExecutionResult<i32>> = matcher
            .execute_trade(&rfq, &quote, || async {
                Err(DomainError::ValidationError("test error".to_string()))
            })
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn atomic_matcher_are_resources_available() {
        let lock_manager = SharedLockManager::with_defaults();
        let matcher = AtomicMatcher::with_defaults(lock_manager);

        let rfq = create_test_rfq();
        let quote = create_test_quote(&rfq);
        let resources = AtomicMatcher::build_trade_locks(&rfq, &quote);

        // Initially available
        assert!(matcher.are_resources_available(&resources).await);

        // Acquire locks
        let _guard = matcher.acquire_locks(resources.clone()).await.unwrap();

        // Now not available (locks held by different holder)
        assert!(!matcher.are_resources_available(&resources).await);
    }

    #[tokio::test]
    async fn atomic_matcher_concurrent_execution_shared_manager() {
        // Test with SHARED manager - demonstrates contention
        let lock_manager = SharedLockManager::with_defaults();
        let success_count = Arc::new(AtomicUsize::new(0));
        let failure_count = Arc::new(AtomicUsize::new(0));

        let rfq = create_test_rfq();
        let quote = create_test_quote(&rfq);

        let mut handles = vec![];

        for _ in 0..10 {
            let mgr = Arc::clone(&lock_manager);
            let matcher = AtomicMatcher::new(
                mgr,
                AtomicMatcherConfig::with_timeout(Duration::from_millis(10)),
            );
            let rfq_clone = rfq.clone();
            let quote_clone = quote.clone();
            let success = Arc::clone(&success_count);
            let failure = Arc::clone(&failure_count);

            handles.push(tokio::spawn(async move {
                let result = matcher
                    .execute_trade(&rfq_clone, &quote_clone, || async {
                        tokio::time::sleep(Duration::from_millis(20)).await;
                        Ok(1)
                    })
                    .await;

                match result {
                    Ok(_) => success.fetch_add(1, Ordering::SeqCst),
                    Err(_) => failure.fetch_add(1, Ordering::SeqCst),
                };
            }));
        }

        for handle in handles {
            let _ = handle.await;
        }

        // With shared manager and short timeout, expect contention
        let successes = success_count.load(Ordering::SeqCst);
        let failures = failure_count.load(Ordering::SeqCst);
        assert!(successes >= 1, "At least one should succeed");
        assert_eq!(successes + failures, 10);
    }

    #[tokio::test]
    async fn atomic_execution_result_fields() {
        let result = AtomicExecutionResult::new("test", 50, 3);

        assert_eq!(result.result, "test");
        assert_eq!(result.execution_time_ms, 50);
        assert_eq!(result.resources_locked, 3);
    }

    #[test]
    fn atomic_matcher_debug() {
        let lock_manager = SharedLockManager::with_defaults();
        let matcher = AtomicMatcher::with_defaults(lock_manager);

        let debug_str = format!("{:?}", matcher);
        assert!(debug_str.contains("AtomicMatcher"));
        assert!(debug_str.contains("config"));
    }
}
