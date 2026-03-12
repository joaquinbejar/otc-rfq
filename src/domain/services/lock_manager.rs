//! # Lock Manager
//!
//! Distributed lock management for atomic trade execution.
//!
//! This module provides the [`LockManager`] trait and [`InMemoryLockManager`]
//! implementation for coordinating resource locks across concurrent operations.
//!
//! # Architecture
//!
//! The lock manager ensures atomic acquisition of multiple resources:
//!
//! ```text
//! Process A: acquire_all([Quote, Account, Instrument]) -> OK
//! Process B: acquire_all([Quote, Account, Instrument]) -> TIMEOUT (Quote locked)
//! Process A: release_all() -> OK
//! Process B: acquire_all([Quote, Account, Instrument]) -> OK
//! ```
//!
//! # Lock Ordering
//!
//! Locks are always acquired in deterministic order to prevent deadlocks:
//! Quote → Account → Instrument (within each category, lexicographically by ID).

use crate::domain::errors::{DomainError, DomainResult};
use crate::domain::services::quote_lock::LockHolderId;
use crate::domain::services::resource_lock::{ResourceLock, sort_locks};
use crate::domain::value_objects::Timestamp;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

/// Configuration for the lock manager.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LockManagerConfig {
    /// Default timeout for lock acquisition.
    pub default_timeout: Duration,
    /// Maximum timeout allowed.
    pub max_timeout: Duration,
    /// Time-to-live for locks (auto-expire).
    pub lock_ttl: Duration,
    /// Retry interval when waiting for locks.
    pub retry_interval: Duration,
}

impl Default for LockManagerConfig {
    fn default() -> Self {
        Self {
            default_timeout: Duration::from_millis(100),
            max_timeout: Duration::from_secs(5),
            lock_ttl: Duration::from_secs(10),
            retry_interval: Duration::from_millis(5),
        }
    }
}

impl LockManagerConfig {
    /// Creates a new lock manager configuration.
    #[must_use]
    pub fn new(
        default_timeout: Duration,
        max_timeout: Duration,
        lock_ttl: Duration,
        retry_interval: Duration,
    ) -> Self {
        Self {
            default_timeout,
            max_timeout,
            lock_ttl,
            retry_interval,
        }
    }

    /// Returns the effective timeout, clamped to max_timeout.
    #[must_use]
    #[inline]
    pub fn effective_timeout(&self, requested: Option<Duration>) -> Duration {
        requested
            .unwrap_or(self.default_timeout)
            .min(self.max_timeout)
    }
}

/// Information about an active lock.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockInfo {
    /// The locked resource.
    pub resource: ResourceLock,
    /// The holder of this lock.
    pub holder_id: LockHolderId,
    /// When the lock was acquired.
    pub acquired_at: Timestamp,
    /// When the lock expires.
    pub expires_at: Timestamp,
}

impl LockInfo {
    /// Creates a new lock info.
    #[must_use]
    pub fn new(resource: ResourceLock, holder_id: LockHolderId, ttl: Duration) -> Self {
        let acquired_at = Timestamp::now();
        let ttl_ms = ttl.as_millis().min(i64::MAX as u128) as i64;
        let expires_at = acquired_at.add_millis(ttl_ms);
        Self {
            resource,
            holder_id,
            acquired_at,
            expires_at,
        }
    }

    /// Returns true if the lock has expired.
    #[must_use]
    #[inline]
    pub fn is_expired(&self) -> bool {
        Timestamp::now() >= self.expires_at
    }
}

/// RAII guard that automatically releases locks when dropped.
///
/// This ensures locks are always released, even if an error occurs
/// during execution.
pub struct LockGuard {
    /// The acquired locks.
    locks: Vec<ResourceLock>,
    /// The holder ID.
    holder_id: LockHolderId,
    /// Reference to the lock manager for release.
    lock_manager: Arc<dyn LockManager>,
    /// Whether locks have been explicitly released.
    released: bool,
}

impl LockGuard {
    /// Creates a new lock guard.
    #[must_use]
    pub fn new(
        locks: Vec<ResourceLock>,
        holder_id: LockHolderId,
        lock_manager: Arc<dyn LockManager>,
    ) -> Self {
        Self {
            locks,
            holder_id,
            lock_manager,
            released: false,
        }
    }

    /// Returns the acquired locks.
    #[must_use]
    pub fn locks(&self) -> &[ResourceLock] {
        &self.locks
    }

    /// Returns the holder ID.
    #[must_use]
    pub fn holder_id(&self) -> LockHolderId {
        self.holder_id
    }

    /// Returns the number of locks held.
    #[must_use]
    #[inline]
    pub fn lock_count(&self) -> usize {
        self.locks.len()
    }

    /// Returns true if locks have been released.
    #[must_use]
    #[inline]
    pub fn is_released(&self) -> bool {
        self.released
    }

    /// Explicitly releases all locks.
    ///
    /// This is called automatically on drop, but can be called early
    /// if needed.
    ///
    /// # Errors
    ///
    /// Returns an error if lock release fails.
    pub async fn release(mut self) -> DomainResult<()> {
        if !self.released {
            self.released = true;
            self.lock_manager
                .release_all(&self.locks, self.holder_id)
                .await?;
        }
        Ok(())
    }
}

impl Drop for LockGuard {
    /// Releases locks on drop if not already released.
    ///
    /// # Note
    ///
    /// If dropped outside a Tokio runtime context, the locks will not be released
    /// automatically. In practice, `LockGuard` is always used within async contexts
    /// (via `AtomicMatcher`), so this should not occur. If using `LockGuard` directly,
    /// always call `release()` explicitly before dropping.
    fn drop(&mut self) {
        if !self.released {
            // Spawn a task to release locks asynchronously when a Tokio runtime is available
            let locks = std::mem::take(&mut self.locks);
            let holder_id = self.holder_id;
            let lock_manager = Arc::clone(&self.lock_manager);
            self.released = true;

            if let Ok(handle) = tokio::runtime::Handle::try_current() {
                handle.spawn(async move {
                    let _ = lock_manager.release_all(&locks, holder_id).await;
                });
            }
            // Note: If no runtime is available, locks will leak. This is acceptable
            // since LockGuard is designed for use in async contexts only.
        }
    }
}

impl fmt::Debug for LockGuard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LockGuard")
            .field("locks", &self.locks)
            .field("holder_id", &self.holder_id)
            .field("released", &self.released)
            .finish()
    }
}

/// Service for managing distributed resource locks.
///
/// Implementations must ensure atomic lock acquisition and proper
/// ordering to prevent deadlocks.
#[async_trait]
pub trait LockManager: Send + Sync + fmt::Debug {
    /// Attempts to acquire locks on all specified resources.
    ///
    /// Locks are acquired in deterministic order to prevent deadlocks.
    /// If any lock cannot be acquired within the timeout, all acquired
    /// locks are released and an error is returned.
    ///
    /// # Arguments
    ///
    /// * `resources` - Resources to lock
    /// * `timeout` - Maximum time to wait for all locks
    ///
    /// # Returns
    ///
    /// A `LockGuard` that automatically releases locks on drop.
    ///
    /// # Errors
    ///
    /// - `DomainError::LockAcquisitionFailed` if timeout expires
    async fn acquire_all(
        &self,
        resources: Vec<ResourceLock>,
        timeout: Duration,
    ) -> DomainResult<LockGuard>;

    /// Releases all specified locks.
    ///
    /// # Arguments
    ///
    /// * `locks` - Locks to release
    /// * `holder_id` - The holder releasing the locks
    ///
    /// # Errors
    ///
    /// Returns an error if release fails (e.g., locks held by different holder).
    async fn release_all(
        &self,
        locks: &[ResourceLock],
        holder_id: LockHolderId,
    ) -> DomainResult<()>;

    /// Checks if a resource is currently locked.
    ///
    /// # Arguments
    ///
    /// * `resource` - Resource to check
    ///
    /// # Returns
    ///
    /// `Some(LockInfo)` if locked, `None` otherwise.
    async fn get_lock_info(&self, resource: &ResourceLock) -> Option<LockInfo>;

    /// Returns the holder ID for this lock manager instance.
    fn holder_id(&self) -> LockHolderId;
}

/// In-memory implementation of [`LockManager`].
///
/// Suitable for single-instance deployments or testing.
/// For multi-instance deployments, use a Redis-based implementation.
///
/// This implementation must be wrapped in `Arc` and used via `Arc<InMemoryLockManager>`
/// to ensure proper lock release through the `LockGuard`.
#[derive(Debug)]
pub struct InMemoryLockManager {
    /// Active locks by resource.
    locks: RwLock<HashMap<String, LockInfo>>,
    /// Configuration.
    config: LockManagerConfig,
}

impl InMemoryLockManager {
    /// Creates a new in-memory lock manager wrapped in Arc.
    #[must_use]
    pub fn new(config: LockManagerConfig) -> Arc<Self> {
        Arc::new(Self {
            locks: RwLock::new(HashMap::new()),
            config,
        })
    }

    /// Creates a new lock manager with default configuration wrapped in Arc.
    #[must_use]
    pub fn with_defaults() -> Arc<Self> {
        Self::new(LockManagerConfig::default())
    }

    /// Creates a raw instance (for internal use).
    fn new_raw(config: LockManagerConfig) -> Self {
        Self {
            locks: RwLock::new(HashMap::new()),
            config,
        }
    }

    /// Returns a key for the lock map.
    fn lock_key(resource: &ResourceLock) -> String {
        format!("{}", resource)
    }

    /// Attempts to acquire a single lock.
    async fn try_acquire_one(
        &self,
        resource: &ResourceLock,
        holder_id: LockHolderId,
    ) -> DomainResult<()> {
        let key = Self::lock_key(resource);
        let mut locks = self.locks.write().await;

        // Check if already locked by another holder
        if let Some(existing) = locks.get(&key)
            && !existing.is_expired()
            && existing.holder_id != holder_id
        {
            return Err(DomainError::LockAcquisitionFailed(format!(
                "resource {} is locked by another holder",
                resource
            )));
        }

        // Acquire lock
        let lock_info = LockInfo::new(resource.clone(), holder_id, self.config.lock_ttl);
        locks.insert(key, lock_info);
        Ok(())
    }

    /// Releases a single lock.
    ///
    /// # Errors
    ///
    /// Returns an error if the lock is held by a different, non-expired holder.
    async fn release_one(
        &self,
        resource: &ResourceLock,
        holder_id: LockHolderId,
    ) -> DomainResult<()> {
        let key = Self::lock_key(resource);
        let mut locks = self.locks.write().await;

        if let Some(existing) = locks.get(&key) {
            if existing.holder_id == holder_id || existing.is_expired() {
                locks.remove(&key);
                Ok(())
            } else {
                Err(DomainError::LockAcquisitionFailed(format!(
                    "cannot release resource {}: lock is held by a different holder",
                    resource
                )))
            }
        } else {
            Ok(())
        }
    }

    /// Cleans up expired locks.
    pub async fn cleanup_expired(&self) {
        let mut locks = self.locks.write().await;
        locks.retain(|_, info| !info.is_expired());
    }
}

/// Wrapper that holds `Arc<InMemoryLockManager>` and implements `LockManager`.
/// This allows proper Arc sharing for `LockGuard` release.
#[derive(Debug, Clone)]
pub struct SharedLockManager {
    inner: Arc<InMemoryLockManager>,
}

impl SharedLockManager {
    /// Creates a new shared lock manager.
    #[must_use]
    pub fn new(config: LockManagerConfig) -> Arc<Self> {
        Arc::new(Self {
            inner: Arc::new(InMemoryLockManager::new_raw(config)),
        })
    }

    /// Creates a new shared lock manager with default configuration.
    #[must_use]
    pub fn with_defaults() -> Arc<Self> {
        Self::new(LockManagerConfig::default())
    }
}

#[async_trait]
impl LockManager for SharedLockManager {
    async fn acquire_all(
        &self,
        resources: Vec<ResourceLock>,
        timeout: Duration,
    ) -> DomainResult<LockGuard> {
        // Generate a unique holder_id for this acquisition
        let holder_id = LockHolderId::new();

        if resources.is_empty() {
            // Create a dummy guard - we need a reference to self as Arc<dyn LockManager>
            // This is handled by the caller wrapping SharedLockManager in Arc
            return Ok(LockGuard {
                locks: vec![],
                holder_id,
                lock_manager: Arc::new(Self {
                    inner: Arc::clone(&self.inner),
                }),
                released: true, // Empty guard, nothing to release
            });
        }

        // Sort locks to ensure deterministic ordering
        let mut sorted_resources = resources;
        sort_locks(&mut sorted_resources);

        let effective_timeout = self.inner.config.effective_timeout(Some(timeout));
        let deadline = tokio::time::Instant::now() + effective_timeout;
        let mut acquired: Vec<ResourceLock> = Vec::with_capacity(sorted_resources.len());

        for resource in &sorted_resources {
            loop {
                match self.inner.try_acquire_one(resource, holder_id).await {
                    Ok(()) => {
                        acquired.push(resource.clone());
                        break;
                    }
                    Err(_) if tokio::time::Instant::now() < deadline => {
                        // Retry after interval
                        tokio::time::sleep(self.inner.config.retry_interval).await;
                    }
                    Err(e) => {
                        // Timeout or error - release all acquired locks
                        for acq in &acquired {
                            let _ = self.inner.release_one(acq, holder_id).await;
                        }
                        return Err(DomainError::LockAcquisitionFailed(format!(
                            "failed to acquire lock on {}: {}",
                            resource, e
                        )));
                    }
                }
            }
        }

        // Create guard with a clone of self that shares the same inner state
        Ok(LockGuard {
            locks: acquired,
            holder_id,
            lock_manager: Arc::new(Self {
                inner: Arc::clone(&self.inner),
            }),
            released: false,
        })
    }

    async fn release_all(
        &self,
        locks: &[ResourceLock],
        holder_id: LockHolderId,
    ) -> DomainResult<()> {
        // Release in reverse order
        for resource in locks.iter().rev() {
            self.inner.release_one(resource, holder_id).await?;
        }
        Ok(())
    }

    async fn get_lock_info(&self, resource: &ResourceLock) -> Option<LockInfo> {
        let key = InMemoryLockManager::lock_key(resource);
        let locks = self.inner.locks.read().await;
        locks.get(&key).filter(|info| !info.is_expired()).cloned()
    }

    fn holder_id(&self) -> LockHolderId {
        // Returns a new holder_id each time since we generate per-acquisition
        LockHolderId::new()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::domain::value_objects::enums::{AssetClass, SettlementMethod};
    use crate::domain::value_objects::symbol::Symbol;
    use crate::domain::value_objects::{CounterpartyId, Instrument, QuoteId};
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn create_instrument(symbol: &str) -> Instrument {
        Instrument::new(
            Symbol::new(symbol).unwrap(),
            AssetClass::CryptoSpot,
            SettlementMethod::default(),
        )
    }

    #[test]
    fn lock_manager_config_default() {
        let config = LockManagerConfig::default();
        assert_eq!(config.default_timeout, Duration::from_millis(100));
        assert_eq!(config.max_timeout, Duration::from_secs(5));
        assert_eq!(config.lock_ttl, Duration::from_secs(10));
    }

    #[test]
    fn lock_manager_config_effective_timeout() {
        let config = LockManagerConfig::default();

        // Uses default when None
        assert_eq!(config.effective_timeout(None), Duration::from_millis(100));

        // Uses requested when within max
        assert_eq!(
            config.effective_timeout(Some(Duration::from_millis(50))),
            Duration::from_millis(50)
        );

        // Clamps to max when exceeds
        assert_eq!(
            config.effective_timeout(Some(Duration::from_secs(10))),
            Duration::from_secs(5)
        );
    }

    #[test]
    fn lock_info_creation() {
        let resource = ResourceLock::Quote(QuoteId::new_v4());
        let holder_id = LockHolderId::new();
        let ttl = Duration::from_secs(5);

        let info = LockInfo::new(resource.clone(), holder_id, ttl);

        assert_eq!(info.resource, resource);
        assert_eq!(info.holder_id, holder_id);
        assert!(!info.is_expired());
    }

    #[test]
    fn lock_info_expiration() {
        let resource = ResourceLock::Quote(QuoteId::new_v4());
        let holder_id = LockHolderId::new();
        // Create with 0 TTL - should be expired immediately
        let ttl = Duration::ZERO;

        let info = LockInfo::new(resource, holder_id, ttl);
        assert!(info.is_expired());
    }

    #[tokio::test]
    async fn shared_lock_manager_acquire_single() {
        let manager = SharedLockManager::with_defaults();
        let resource = ResourceLock::Quote(QuoteId::new_v4());

        let guard = manager
            .acquire_all(vec![resource.clone()], Duration::from_millis(100))
            .await;

        assert!(guard.is_ok());
        let guard = guard.unwrap();
        assert_eq!(guard.lock_count(), 1);

        // Verify lock is held
        let info = manager.get_lock_info(&resource).await;
        assert!(info.is_some());
    }

    #[tokio::test]
    async fn shared_lock_manager_acquire_multiple() {
        let manager = SharedLockManager::with_defaults();
        let resources = vec![
            ResourceLock::Quote(QuoteId::new_v4()),
            ResourceLock::Account(CounterpartyId::new("client")),
            ResourceLock::Instrument(create_instrument("BTC/USD")),
        ];

        let guard = manager
            .acquire_all(resources.clone(), Duration::from_millis(100))
            .await;

        assert!(guard.is_ok());
        let guard = guard.unwrap();
        assert_eq!(guard.lock_count(), 3);
    }

    #[tokio::test]
    async fn shared_lock_manager_acquire_empty() {
        let manager = SharedLockManager::with_defaults();

        let guard = manager
            .acquire_all(vec![], Duration::from_millis(100))
            .await;

        assert!(guard.is_ok());
        assert_eq!(guard.unwrap().lock_count(), 0);
    }

    #[tokio::test]
    async fn shared_lock_manager_release_via_guard() {
        let manager = SharedLockManager::with_defaults();
        let resource = ResourceLock::Quote(QuoteId::new_v4());

        // Acquire
        let guard = manager
            .acquire_all(vec![resource.clone()], Duration::from_millis(100))
            .await
            .unwrap();

        // Verify lock is held
        let info = manager.get_lock_info(&resource).await;
        assert!(info.is_some());

        // Release via guard
        guard.release().await.unwrap();

        // Verify released
        let info = manager.get_lock_info(&resource).await;
        assert!(info.is_none());
    }

    #[tokio::test]
    async fn shared_lock_manager_contention() {
        // Test that concurrent acquisitions on same resource cause contention
        let manager = SharedLockManager::with_defaults();
        let resource = ResourceLock::Quote(QuoteId::new_v4());

        // First acquisition succeeds
        let _guard1 = manager
            .acquire_all(vec![resource.clone()], Duration::from_millis(50))
            .await
            .unwrap();

        // Verify lock is held
        let info = manager.get_lock_info(&resource).await;
        assert!(info.is_some());

        // Second acquisition should timeout (different holder_id generated)
        let result = manager
            .acquire_all(vec![resource.clone()], Duration::from_millis(20))
            .await;

        // Should fail due to contention
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn lock_guard_debug() {
        let manager = SharedLockManager::with_defaults();
        let resource = ResourceLock::Quote(QuoteId::new_v4());

        let guard = manager
            .acquire_all(vec![resource], Duration::from_millis(100))
            .await
            .unwrap();

        let debug_str = format!("{:?}", guard);
        assert!(debug_str.contains("LockGuard"));
        assert!(debug_str.contains("released: false"));
    }

    #[tokio::test]
    async fn cleanup_expired_locks() {
        let config = LockManagerConfig {
            default_timeout: Duration::from_millis(100),
            max_timeout: Duration::from_secs(5),
            lock_ttl: Duration::from_millis(10), // Very short TTL
            retry_interval: Duration::from_millis(5),
        };
        let manager = SharedLockManager::new(config);
        let resource = ResourceLock::Quote(QuoteId::new_v4());

        // Acquire lock
        let _guard = manager
            .acquire_all(vec![resource.clone()], Duration::from_millis(100))
            .await
            .unwrap();

        // Wait for expiration
        tokio::time::sleep(Duration::from_millis(20)).await;

        // Lock should be gone (expired)
        let info = manager.get_lock_info(&resource).await;
        assert!(info.is_none());
    }

    #[tokio::test]
    async fn concurrent_lock_acquisition_shared_manager() {
        // Test concurrent acquisition with SHARED manager - only one should succeed
        let manager = SharedLockManager::with_defaults();
        let resource = ResourceLock::Quote(QuoteId::new_v4());
        let success_count = Arc::new(AtomicUsize::new(0));
        let failure_count = Arc::new(AtomicUsize::new(0));

        let mut handles = vec![];

        for _ in 0..10 {
            let mgr = Arc::clone(&manager);
            let res = resource.clone();
            let success = Arc::clone(&success_count);
            let failure = Arc::clone(&failure_count);

            handles.push(tokio::spawn(async move {
                match mgr.acquire_all(vec![res], Duration::from_millis(5)).await {
                    Ok(guard) => {
                        success.fetch_add(1, Ordering::SeqCst);
                        // Hold lock briefly
                        tokio::time::sleep(Duration::from_millis(10)).await;
                        let _ = guard.release().await;
                    }
                    Err(_) => {
                        failure.fetch_add(1, Ordering::SeqCst);
                    }
                }
            }));
        }

        for handle in handles {
            let _ = handle.await;
        }

        // With short timeout and shared manager, most should fail due to contention
        let successes = success_count.load(Ordering::SeqCst);
        let failures = failure_count.load(Ordering::SeqCst);
        assert!(successes >= 1, "At least one should succeed");
        assert!(failures >= 1, "At least one should fail due to contention");
        assert_eq!(successes + failures, 10);
    }
}
