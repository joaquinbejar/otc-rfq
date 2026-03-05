//! # Quote Lock Service
//!
//! Distributed locking for quote acceptance to prevent double-execution.
//!
//! This module provides the [`QuoteLockService`] trait and related types
//! for atomic quote locking during the acceptance flow.
//!
//! # Architecture
//!
//! Quote locking ensures that only one process can accept a given quote
//! at any time, preventing race conditions and double-execution.
//!
//! ```text
//! Process A: lock(quote_id) -> OK -> execute -> release
//! Process B: lock(quote_id) -> FAIL (already locked)
//! ```

use crate::domain::errors::DomainResult;
use crate::domain::value_objects::{QuoteId, Timestamp};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::Duration;
use uuid::Uuid;

/// Unique identifier for a lock holder (process/instance).
///
/// Used to track which service instance holds a lock, enabling
/// safe lock release and conflict detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LockHolderId(Uuid);

impl LockHolderId {
    /// Creates a new random lock holder ID.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Creates a lock holder ID from an existing UUID.
    #[must_use]
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Returns the inner UUID.
    #[must_use]
    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl Default for LockHolderId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for LockHolderId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A lock on a quote preventing concurrent acceptance.
///
/// Represents an active lock that must be released after use.
/// The lock automatically expires after its TTL to prevent deadlocks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuoteLock {
    /// The locked quote ID.
    quote_id: QuoteId,
    /// The holder of this lock.
    holder_id: LockHolderId,
    /// When the lock was acquired.
    locked_at: Timestamp,
    /// When the lock expires.
    expires_at: Timestamp,
}

impl QuoteLock {
    /// Creates a new quote lock.
    #[must_use]
    pub fn new(
        quote_id: QuoteId,
        holder_id: LockHolderId,
        locked_at: Timestamp,
        ttl: Duration,
    ) -> Self {
        let ttl_ms = ttl.as_millis().min(i64::MAX as u128) as i64;
        let expires_at = locked_at.add_millis(ttl_ms);
        Self {
            quote_id,
            holder_id,
            locked_at,
            expires_at,
        }
    }

    /// Returns the locked quote ID.
    #[must_use]
    pub fn quote_id(&self) -> QuoteId {
        self.quote_id
    }

    /// Returns the lock holder ID.
    #[must_use]
    pub fn holder_id(&self) -> LockHolderId {
        self.holder_id
    }

    /// Returns when the lock was acquired.
    #[must_use]
    pub fn locked_at(&self) -> Timestamp {
        self.locked_at
    }

    /// Returns when the lock expires.
    #[must_use]
    pub fn expires_at(&self) -> Timestamp {
        self.expires_at
    }

    /// Returns true if the lock has expired.
    #[must_use]
    pub fn is_expired(&self) -> bool {
        Timestamp::now() >= self.expires_at
    }

    /// Returns the remaining time until expiration.
    #[must_use]
    pub fn remaining_ttl(&self) -> Duration {
        let now = Timestamp::now();
        if now >= self.expires_at {
            Duration::ZERO
        } else {
            let remaining_ms = self
                .expires_at
                .timestamp_millis()
                .saturating_sub(now.timestamp_millis());
            Duration::from_millis(remaining_ms as u64)
        }
    }
}

/// Service for acquiring and releasing quote locks.
///
/// Implementations must ensure atomic lock acquisition to prevent
/// race conditions in distributed environments.
#[async_trait]
pub trait QuoteLockService: Send + Sync + fmt::Debug {
    /// Attempts to acquire a lock on the specified quote.
    ///
    /// # Arguments
    ///
    /// * `quote_id` - The quote to lock
    /// * `holder_id` - The ID of the process acquiring the lock
    /// * `ttl` - Time-to-live for the lock
    ///
    /// # Returns
    ///
    /// The acquired lock on success, or an error if the quote is already locked.
    ///
    /// # Errors
    ///
    /// - `DomainError::QuoteLocked` if the quote is already locked by another holder
    /// - `DomainError::LockAcquisitionFailed` if lock acquisition fails for other reasons
    async fn lock(
        &self,
        quote_id: QuoteId,
        holder_id: LockHolderId,
        ttl: Duration,
    ) -> DomainResult<QuoteLock>;

    /// Releases a previously acquired lock.
    ///
    /// # Arguments
    ///
    /// * `lock` - The lock to release
    ///
    /// # Returns
    ///
    /// Ok(()) if the lock was released successfully.
    ///
    /// # Errors
    ///
    /// Returns an error if the lock cannot be released (e.g., already expired
    /// or held by a different holder).
    async fn release(&self, lock: &QuoteLock) -> DomainResult<()>;

    /// Checks if a quote is currently locked.
    ///
    /// # Arguments
    ///
    /// * `quote_id` - The quote to check
    ///
    /// # Returns
    ///
    /// Some(lock) if the quote is locked, None otherwise.
    async fn is_locked(&self, quote_id: QuoteId) -> Option<QuoteLock>;

    /// Attempts to extend the TTL of an existing lock.
    ///
    /// # Arguments
    ///
    /// * `lock` - The lock to extend
    /// * `additional_ttl` - Additional time to add to the lock
    ///
    /// # Returns
    ///
    /// The updated lock with extended TTL.
    ///
    /// # Errors
    ///
    /// Returns an error if the lock has expired or is held by a different holder.
    async fn extend(&self, lock: &QuoteLock, additional_ttl: Duration) -> DomainResult<QuoteLock>;
}

/// Configuration for quote locking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuoteLockConfig {
    /// Default TTL for locks.
    pub default_ttl: Duration,
    /// Maximum TTL allowed for locks.
    pub max_ttl: Duration,
}

impl Default for QuoteLockConfig {
    fn default() -> Self {
        Self {
            default_ttl: Duration::from_millis(1000),
            max_ttl: Duration::from_secs(5),
        }
    }
}

impl QuoteLockConfig {
    /// Creates a new lock configuration.
    #[must_use]
    pub fn new(default_ttl: Duration, max_ttl: Duration) -> Self {
        Self {
            default_ttl,
            max_ttl,
        }
    }

    /// Returns the effective TTL, clamped to max_ttl.
    #[must_use]
    pub fn effective_ttl(&self, requested: Option<Duration>) -> Duration {
        requested.unwrap_or(self.default_ttl).min(self.max_ttl)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn lock_holder_id_creates_unique_ids() {
        let id1 = LockHolderId::new();
        let id2 = LockHolderId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn lock_holder_id_from_uuid() {
        let uuid = Uuid::new_v4();
        let id = LockHolderId::from_uuid(uuid);
        assert_eq!(id.as_uuid(), uuid);
    }

    #[test]
    fn lock_holder_id_display() {
        let uuid = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let id = LockHolderId::from_uuid(uuid);
        assert_eq!(id.to_string(), "550e8400-e29b-41d4-a716-446655440000");
    }

    #[test]
    fn quote_lock_creation() {
        let quote_id = QuoteId::new_v4();
        let holder_id = LockHolderId::new();
        let locked_at = Timestamp::now();
        let ttl = Duration::from_secs(1);

        let lock = QuoteLock::new(quote_id, holder_id, locked_at, ttl);

        assert_eq!(lock.quote_id(), quote_id);
        assert_eq!(lock.holder_id(), holder_id);
        assert_eq!(lock.locked_at(), locked_at);
    }

    #[test]
    fn quote_lock_expiration() {
        let quote_id = QuoteId::new_v4();
        let holder_id = LockHolderId::new();
        let locked_at = Timestamp::now().add_secs(-2); // 2 seconds ago
        let ttl = Duration::from_secs(1);

        let lock = QuoteLock::new(quote_id, holder_id, locked_at, ttl);

        assert!(lock.is_expired());
        assert_eq!(lock.remaining_ttl(), Duration::ZERO);
    }

    #[test]
    fn quote_lock_not_expired() {
        let quote_id = QuoteId::new_v4();
        let holder_id = LockHolderId::new();
        let locked_at = Timestamp::now();
        let ttl = Duration::from_secs(60);

        let lock = QuoteLock::new(quote_id, holder_id, locked_at, ttl);

        assert!(!lock.is_expired());
        assert!(lock.remaining_ttl() > Duration::ZERO);
    }

    #[test]
    fn quote_lock_config_default() {
        let config = QuoteLockConfig::default();
        assert_eq!(config.default_ttl, Duration::from_millis(1000));
        assert_eq!(config.max_ttl, Duration::from_secs(5));
    }

    #[test]
    fn quote_lock_config_effective_ttl() {
        let config = QuoteLockConfig::new(Duration::from_secs(1), Duration::from_secs(5));

        // Uses default when None
        assert_eq!(config.effective_ttl(None), Duration::from_secs(1));

        // Uses requested when within max
        assert_eq!(
            config.effective_ttl(Some(Duration::from_secs(3))),
            Duration::from_secs(3)
        );

        // Clamps to max when exceeds
        assert_eq!(
            config.effective_ttl(Some(Duration::from_secs(10))),
            Duration::from_secs(5)
        );
    }
}
