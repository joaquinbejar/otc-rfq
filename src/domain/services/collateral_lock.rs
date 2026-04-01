//! # Collateral Lock Service
//!
//! Service for atomically locking collateral for block trade execution.
//!
//! This module provides the [`CollateralLockService`] trait for locking and
//! releasing collateral during off-book block trade settlement.

use crate::domain::errors::DomainResult;
use crate::domain::value_objects::timestamp::Timestamp;
use crate::domain::value_objects::{CounterpartyId, Instrument, Price, Quantity};
use async_trait::async_trait;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// Handle representing a collateral lock.
///
/// This handle is used to track and release collateral locks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollateralLockHandle {
    /// Unique identifier for this lock.
    lock_id: Uuid,
    /// Buyer counterparty ID.
    buyer_id: CounterpartyId,
    /// Seller counterparty ID.
    seller_id: CounterpartyId,
    /// Amount locked for buyer.
    buyer_amount: Decimal,
    /// Amount locked for seller.
    seller_amount: Decimal,
    /// When the lock was acquired.
    locked_at: Timestamp,
}

impl CollateralLockHandle {
    /// Creates a new collateral lock handle.
    #[must_use]
    pub fn new(
        buyer_id: CounterpartyId,
        seller_id: CounterpartyId,
        buyer_amount: Decimal,
        seller_amount: Decimal,
    ) -> Self {
        Self {
            lock_id: Uuid::new_v4(),
            buyer_id,
            seller_id,
            buyer_amount,
            seller_amount,
            locked_at: Timestamp::now(),
        }
    }

    /// Creates a lock handle from existing parts (for reconstruction).
    #[must_use]
    pub fn from_parts(
        lock_id: Uuid,
        buyer_id: CounterpartyId,
        seller_id: CounterpartyId,
        buyer_amount: Decimal,
        seller_amount: Decimal,
        locked_at: Timestamp,
    ) -> Self {
        Self {
            lock_id,
            buyer_id,
            seller_id,
            buyer_amount,
            seller_amount,
            locked_at,
        }
    }

    /// Returns the lock ID.
    #[must_use]
    pub fn lock_id(&self) -> Uuid {
        self.lock_id
    }

    /// Returns the buyer counterparty ID.
    #[must_use]
    pub fn buyer_id(&self) -> &CounterpartyId {
        &self.buyer_id
    }

    /// Returns the seller counterparty ID.
    #[must_use]
    pub fn seller_id(&self) -> &CounterpartyId {
        &self.seller_id
    }

    /// Returns the amount locked for the buyer.
    #[must_use]
    pub fn buyer_amount(&self) -> Decimal {
        self.buyer_amount
    }

    /// Returns the amount locked for the seller.
    #[must_use]
    pub fn seller_amount(&self) -> Decimal {
        self.seller_amount
    }

    /// Returns when the lock was acquired.
    #[must_use]
    pub fn locked_at(&self) -> Timestamp {
        self.locked_at
    }

    /// Returns the total amount locked.
    #[must_use]
    pub fn total_amount(&self) -> Decimal {
        self.buyer_amount.saturating_add(self.seller_amount)
    }
}

/// Service for locking collateral atomically.
///
/// Implementations handle the atomic locking and release of collateral
/// for both counterparties in a block trade.
#[async_trait]
pub trait CollateralLockService: Send + Sync + fmt::Debug {
    /// Locks collateral for both parties atomically.
    ///
    /// This operation must be atomic - either both parties' collateral
    /// is locked, or neither is.
    ///
    /// # Arguments
    ///
    /// * `buyer_id` - The buyer counterparty
    /// * `seller_id` - The seller counterparty
    /// * `instrument` - The instrument being traded
    /// * `quantity` - The trade quantity
    /// * `price` - The trade price
    ///
    /// # Returns
    ///
    /// A `CollateralLockHandle` that can be used to release the lock.
    ///
    /// # Errors
    ///
    /// Returns `DomainError::CollateralLockFailed` if:
    /// - Either party has insufficient collateral
    /// - The locking operation fails for any other reason
    async fn lock_both(
        &self,
        buyer_id: &CounterpartyId,
        seller_id: &CounterpartyId,
        instrument: &Instrument,
        quantity: Quantity,
        price: Price,
    ) -> DomainResult<CollateralLockHandle>;

    /// Releases a collateral lock.
    ///
    /// This should be called on failure to release locked collateral.
    ///
    /// # Arguments
    ///
    /// * `lock` - The lock handle to release
    ///
    /// # Errors
    ///
    /// Returns an error if the lock cannot be released.
    async fn release(&self, lock: &CollateralLockHandle) -> DomainResult<()>;

    /// Checks if a counterparty has sufficient collateral.
    ///
    /// # Arguments
    ///
    /// * `counterparty_id` - The counterparty to check
    /// * `required_amount` - The required collateral amount
    ///
    /// # Returns
    ///
    /// The available collateral amount if sufficient, error otherwise.
    async fn check_available(
        &self,
        counterparty_id: &CounterpartyId,
        required_amount: Decimal,
    ) -> DomainResult<Decimal>;
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn collateral_lock_handle_creation() {
        let handle = CollateralLockHandle::new(
            CounterpartyId::new("buyer-1"),
            CounterpartyId::new("seller-1"),
            Decimal::new(10000, 2),
            Decimal::new(10000, 2),
        );

        assert_eq!(handle.buyer_id().as_str(), "buyer-1");
        assert_eq!(handle.seller_id().as_str(), "seller-1");
        assert_eq!(handle.buyer_amount(), Decimal::new(10000, 2));
        assert_eq!(handle.seller_amount(), Decimal::new(10000, 2));
        assert_eq!(handle.total_amount(), Decimal::new(20000, 2));
    }

    #[test]
    fn collateral_lock_handle_from_parts() {
        let lock_id = Uuid::new_v4();
        let locked_at = Timestamp::now();

        let handle = CollateralLockHandle::from_parts(
            lock_id,
            CounterpartyId::new("buyer-1"),
            CounterpartyId::new("seller-1"),
            Decimal::new(5000, 2),
            Decimal::new(5000, 2),
            locked_at,
        );

        assert_eq!(handle.lock_id(), lock_id);
        assert_eq!(handle.locked_at(), locked_at);
    }
}
