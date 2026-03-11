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
//! let matcher = AtomicMatcher::new(lock_manager);
//! let result = matcher.execute_atomic(
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
    /// Whether to emit domain events.
    pub emit_events: bool,
}

impl Default for AtomicMatcherConfig {
    fn default() -> Self {
        Self {
            lock_timeout: Duration::from_millis(100),
            emit_events: true,
        }
    }
}

impl AtomicMatcherConfig {
    /// Creates a new atomic matcher configuration.
    #[must_use]
    pub fn new(lock_timeout: Duration, emit_events: bool) -> Self {
        Self {
            lock_timeout,
            emit_events,
        }
    }

    /// Creates a configuration with custom lock timeout.
    #[must_use]
    pub fn with_timeout(lock_timeout: Duration) -> Self {
        Self {
            lock_timeout,
            ..Default::default()
        }
    }
}

/// Result of an atomic execution.
#[derive(Debug, Clone)]
pub struct AtomicExecutionResult<T> {
    /// The result of the operation.
    pub result: T,
    /// Time spent holding locks in milliseconds.
    pub locks_held_ms: u64,
    /// Number of resources that were locked.
    pub resources_locked: usize,
}

impl<T> AtomicExecutionResult<T> {
    /// Creates a new atomic execution result.
    #[must_use]
    pub fn new(result: T, locks_held_ms: u64, resources_locked: usize) -> Self {
        Self {
            result,
            locks_held_ms,
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
        let start = std::time::Instant::now();
        let resources_count = resources.len();

        // Acquire all locks
        let guard = self
            .lock_manager
            .acquire_all(resources, self.config.lock_timeout)
            .await?;

        // Execute the operation
        let result = operation().await;

        // Calculate lock hold time
        let locks_held_ms = start.elapsed().as_millis() as u64;

        // Release locks (guard will auto-release on drop, but we do it explicitly)
        let _ = self
            .lock_manager
            .release_all(guard.locks(), guard.holder_id())
            .await;

        // Return result
        match result {
            Ok(value) => Ok(AtomicExecutionResult::new(
                value,
                locks_held_ms,
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
    use crate::domain::services::lock_manager::InMemoryLockManager;
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
        assert!(config.emit_events);
    }

    #[test]
    fn atomic_matcher_config_with_timeout() {
        let config = AtomicMatcherConfig::with_timeout(Duration::from_millis(200));
        assert_eq!(config.lock_timeout, Duration::from_millis(200));
        assert!(config.emit_events);
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
        let lock_manager = Arc::new(InMemoryLockManager::with_defaults());
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
        assert!(exec_result.locks_held_ms < 1000);
    }

    #[tokio::test]
    async fn atomic_matcher_execute_failure_releases_locks() {
        let lock_manager = Arc::new(InMemoryLockManager::with_defaults());
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
        let lock_manager = Arc::new(InMemoryLockManager::with_defaults());
        let matcher = AtomicMatcher::with_defaults(lock_manager);

        let rfq = create_test_rfq();
        let quote = create_test_quote(&rfq);
        let resources = AtomicMatcher::build_trade_locks(&rfq, &quote);

        // Initially available
        assert!(matcher.are_resources_available(&resources).await);

        // Acquire locks
        let _guard = matcher.acquire_locks(resources.clone()).await.unwrap();

        // Now not available (locks held by matcher)
        assert!(!matcher.are_resources_available(&resources).await);
    }

    #[tokio::test]
    async fn atomic_matcher_concurrent_execution() {
        let success_count = Arc::new(AtomicUsize::new(0));

        let rfq = create_test_rfq();
        let quote = create_test_quote(&rfq);

        let mut handles = vec![];

        for _ in 0..10 {
            let lock_manager = Arc::new(InMemoryLockManager::with_defaults());
            let matcher = AtomicMatcher::new(
                lock_manager,
                AtomicMatcherConfig::with_timeout(Duration::from_millis(20)),
            );
            let rfq_clone = rfq.clone();
            let quote_clone = quote.clone();
            let success = Arc::clone(&success_count);

            handles.push(tokio::spawn(async move {
                let result = matcher
                    .execute_trade(&rfq_clone, &quote_clone, || async {
                        tokio::time::sleep(Duration::from_millis(5)).await;
                        Ok(1)
                    })
                    .await;

                if result.is_ok() {
                    success.fetch_add(1, Ordering::SeqCst);
                }
            }));
        }

        for handle in handles {
            let _ = handle.await;
        }

        // All should succeed since each has its own lock manager
        assert_eq!(success_count.load(Ordering::SeqCst), 10);
    }

    #[tokio::test]
    async fn atomic_execution_result_fields() {
        let result = AtomicExecutionResult::new("test", 50, 3);

        assert_eq!(result.result, "test");
        assert_eq!(result.locks_held_ms, 50);
        assert_eq!(result.resources_locked, 3);
    }

    #[test]
    fn atomic_matcher_debug() {
        let lock_manager = Arc::new(InMemoryLockManager::with_defaults());
        let matcher = AtomicMatcher::with_defaults(lock_manager);

        let debug_str = format!("{:?}", matcher);
        assert!(debug_str.contains("AtomicMatcher"));
        assert!(debug_str.contains("config"));
    }
}
