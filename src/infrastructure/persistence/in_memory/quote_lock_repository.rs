//! # In-Memory Quote Lock Repository
//!
//! In-memory implementation of [`QuoteLockService`] for single-instance deployments.
//!
//! This implementation stores its state in a `Mutex<HashMap<...>>` for thread-safe access.
//! Callers can wrap an instance in `Arc<InMemoryQuoteLockRepository>` when shared ownership is required.
//! It is suitable for development, testing, and single-instance production deployments.
//!
//! For distributed deployments, use a Redis-based implementation instead.
//!
//! # Examples
//!
//! ```
//! use otc_rfq::infrastructure::persistence::in_memory::InMemoryQuoteLockRepository;
//! use otc_rfq::domain::services::{QuoteLockService, LockHolderId};
//! use otc_rfq::domain::value_objects::QuoteId;
//! use std::time::Duration;
//!
//! # tokio_test::block_on(async {
//! let repo = InMemoryQuoteLockRepository::new();
//! let quote_id = QuoteId::new_v4();
//! let holder_id = LockHolderId::new();
//!
//! let lock = repo.lock(quote_id, holder_id, Duration::from_secs(1)).await.unwrap();
//! assert!(!lock.is_expired());
//!
//! repo.release(&lock).await.unwrap();
//! # });
//! ```

use crate::domain::errors::{DomainError, DomainResult};
use crate::domain::services::quote_lock::{LockHolderId, QuoteLock, QuoteLockService};
use crate::domain::value_objects::{QuoteId, Timestamp};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

/// In-memory implementation of [`QuoteLockService`].
///
/// Thread-safe for concurrent access within a single process.
/// Does not provide distributed locking across multiple instances.
#[derive(Debug)]
pub struct InMemoryQuoteLockRepository {
    /// Active locks indexed by quote ID.
    locks: Mutex<HashMap<QuoteId, QuoteLock>>,
}

impl InMemoryQuoteLockRepository {
    /// Creates a new in-memory quote lock repository.
    #[must_use]
    pub fn new() -> Self {
        Self {
            locks: Mutex::new(HashMap::new()),
        }
    }

    /// Removes expired locks from the repository.
    ///
    /// This is called automatically during lock operations.
    fn cleanup_expired(&self) {
        if let Ok(mut locks) = self.locks.lock() {
            locks.retain(|_, lock| !lock.is_expired());
        }
    }

    /// Returns the number of active (non-expired) locks.
    #[must_use]
    pub fn active_lock_count(&self) -> usize {
        self.cleanup_expired();
        self.locks.lock().map(|locks| locks.len()).unwrap_or(0)
    }
}

impl Default for InMemoryQuoteLockRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl QuoteLockService for InMemoryQuoteLockRepository {
    async fn lock(
        &self,
        quote_id: QuoteId,
        holder_id: LockHolderId,
        ttl: Duration,
    ) -> DomainResult<QuoteLock> {
        self.cleanup_expired();

        let mut locks = self
            .locks
            .lock()
            .map_err(|_| DomainError::LockAcquisitionFailed("Lock mutex poisoned".to_string()))?;

        // Check if already locked by another holder
        if let Some(existing) = locks.get(&quote_id)
            && !existing.is_expired()
            && existing.holder_id() != holder_id
        {
            return Err(DomainError::QuoteLocked(format!(
                "Quote {} is already locked by another holder",
                quote_id
            )));
        }

        // Create and store the lock
        let lock = QuoteLock::new(quote_id, holder_id, Timestamp::now(), ttl);
        locks.insert(quote_id, lock.clone());

        Ok(lock)
    }

    async fn release(&self, lock: &QuoteLock) -> DomainResult<()> {
        let mut locks = self
            .locks
            .lock()
            .map_err(|_| DomainError::LockAcquisitionFailed("Lock mutex poisoned".to_string()))?;

        // Only release if the lock is held by the same holder
        if let Some(existing) = locks.get(&lock.quote_id())
            && existing.holder_id() == lock.holder_id()
        {
            locks.remove(&lock.quote_id());
        }

        Ok(())
    }

    async fn is_locked(&self, quote_id: QuoteId) -> Option<QuoteLock> {
        self.cleanup_expired();

        self.locks
            .lock()
            .ok()
            .and_then(|locks| locks.get(&quote_id).cloned())
            .filter(|lock| !lock.is_expired())
    }

    async fn extend(&self, lock: &QuoteLock, additional_ttl: Duration) -> DomainResult<QuoteLock> {
        let mut locks = self
            .locks
            .lock()
            .map_err(|_| DomainError::LockAcquisitionFailed("Lock mutex poisoned".to_string()))?;

        // Verify the lock exists and is held by the same holder
        if let Some(existing) = locks.get(&lock.quote_id()) {
            if existing.is_expired() {
                return Err(DomainError::LockAcquisitionFailed(
                    "Lock has expired".to_string(),
                ));
            }
            if existing.holder_id() != lock.holder_id() {
                return Err(DomainError::LockAcquisitionFailed(
                    "Lock held by different holder".to_string(),
                ));
            }

            // Extend by adding additional_ttl to the existing expiry (or now if already past)
            let base_time = if existing.expires_at() > Timestamp::now() {
                existing.expires_at()
            } else {
                Timestamp::now()
            };
            let additional_ms = additional_ttl.as_millis().min(i64::MAX as u128) as i64;
            let new_expires_at = base_time.add_millis(additional_ms);

            // Create new lock preserving original locked_at but with extended expiry
            let new_lock = QuoteLock::new_with_expiry(
                lock.quote_id(),
                lock.holder_id(),
                existing.locked_at(),
                new_expires_at,
            );
            locks.insert(lock.quote_id(), new_lock.clone());

            Ok(new_lock)
        } else {
            Err(DomainError::LockAcquisitionFailed(
                "Lock not found".to_string(),
            ))
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn lock_and_release() {
        let repo = InMemoryQuoteLockRepository::new();
        let quote_id = QuoteId::new_v4();
        let holder_id = LockHolderId::new();

        // Lock should succeed
        let lock = repo
            .lock(quote_id, holder_id, Duration::from_secs(10))
            .await
            .unwrap();

        assert_eq!(lock.quote_id(), quote_id);
        assert_eq!(lock.holder_id(), holder_id);
        assert!(!lock.is_expired());

        // Release should succeed
        repo.release(&lock).await.unwrap();

        // Should no longer be locked
        assert!(repo.is_locked(quote_id).await.is_none());
    }

    #[tokio::test]
    async fn lock_prevents_double_locking() {
        let repo = InMemoryQuoteLockRepository::new();
        let quote_id = QuoteId::new_v4();
        let holder_1 = LockHolderId::new();
        let holder_2 = LockHolderId::new();

        // First lock should succeed
        let _lock = repo
            .lock(quote_id, holder_1, Duration::from_secs(10))
            .await
            .unwrap();

        // Second lock by different holder should fail
        let result = repo.lock(quote_id, holder_2, Duration::from_secs(10)).await;

        assert!(matches!(result, Err(DomainError::QuoteLocked(_))));
    }

    #[tokio::test]
    async fn same_holder_can_relock() {
        let repo = InMemoryQuoteLockRepository::new();
        let quote_id = QuoteId::new_v4();
        let holder_id = LockHolderId::new();

        // First lock
        let _lock1 = repo
            .lock(quote_id, holder_id, Duration::from_secs(10))
            .await
            .unwrap();

        // Same holder can lock again (idempotent)
        let lock2 = repo
            .lock(quote_id, holder_id, Duration::from_secs(10))
            .await
            .unwrap();

        assert_eq!(lock2.holder_id(), holder_id);
    }

    #[tokio::test]
    async fn expired_lock_can_be_acquired() {
        let repo = InMemoryQuoteLockRepository::new();
        let quote_id = QuoteId::new_v4();
        let holder_1 = LockHolderId::new();
        let holder_2 = LockHolderId::new();

        // Lock with very short TTL
        let _lock = repo
            .lock(quote_id, holder_1, Duration::from_millis(1))
            .await
            .unwrap();

        // Wait for expiration
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Different holder should now be able to lock
        let lock2 = repo
            .lock(quote_id, holder_2, Duration::from_secs(10))
            .await
            .unwrap();

        assert_eq!(lock2.holder_id(), holder_2);
    }

    #[tokio::test]
    async fn is_locked_returns_active_lock() {
        let repo = InMemoryQuoteLockRepository::new();
        let quote_id = QuoteId::new_v4();
        let holder_id = LockHolderId::new();

        // Not locked initially
        assert!(repo.is_locked(quote_id).await.is_none());

        // Lock it
        let _lock = repo
            .lock(quote_id, holder_id, Duration::from_secs(10))
            .await
            .unwrap();

        // Should be locked
        let active = repo.is_locked(quote_id).await;
        assert!(active.is_some());
        assert_eq!(active.unwrap().holder_id(), holder_id);
    }

    #[tokio::test]
    async fn extend_lock() {
        let repo = InMemoryQuoteLockRepository::new();
        let quote_id = QuoteId::new_v4();
        let holder_id = LockHolderId::new();

        // Lock with short TTL
        let lock = repo
            .lock(quote_id, holder_id, Duration::from_secs(1))
            .await
            .unwrap();

        // Extend the lock
        let extended = repo.extend(&lock, Duration::from_secs(60)).await.unwrap();

        assert_eq!(extended.quote_id(), quote_id);
        assert!(extended.remaining_ttl() > Duration::from_secs(30));
    }

    #[tokio::test]
    async fn extend_fails_for_different_holder() {
        let repo = InMemoryQuoteLockRepository::new();
        let quote_id = QuoteId::new_v4();
        let holder_1 = LockHolderId::new();
        let holder_2 = LockHolderId::new();

        // Lock with holder 1
        let _lock = repo
            .lock(quote_id, holder_1, Duration::from_secs(10))
            .await
            .unwrap();

        // Create a fake lock with holder 2
        let fake_lock = QuoteLock::new(
            quote_id,
            holder_2,
            Timestamp::now(),
            Duration::from_secs(10),
        );

        // Extend should fail
        let result = repo.extend(&fake_lock, Duration::from_secs(60)).await;
        assert!(matches!(result, Err(DomainError::LockAcquisitionFailed(_))));
    }

    #[tokio::test]
    async fn active_lock_count() {
        let repo = InMemoryQuoteLockRepository::new();
        let holder_id = LockHolderId::new();

        assert_eq!(repo.active_lock_count(), 0);

        // Add some locks
        let _lock1 = repo
            .lock(QuoteId::new_v4(), holder_id, Duration::from_secs(10))
            .await
            .unwrap();
        let _lock2 = repo
            .lock(QuoteId::new_v4(), holder_id, Duration::from_secs(10))
            .await
            .unwrap();

        assert_eq!(repo.active_lock_count(), 2);
    }
}
