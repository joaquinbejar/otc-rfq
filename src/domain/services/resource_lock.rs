//! # Resource Lock Types
//!
//! Defines resource types that can be locked for atomic execution.
//!
//! This module provides the [`ResourceLock`] enum representing different
//! resources that must be locked during trade execution to ensure atomicity.
//!
//! # Lock Ordering
//!
//! To prevent deadlocks, locks must always be acquired in a deterministic order:
//! 1. Quote locks (by QuoteId)
//! 2. Account locks (by CounterpartyId)
//! 3. Instrument locks (by Instrument)
//!
//! Within each category, locks are ordered lexicographically by their string
//! representation.

use crate::domain::value_objects::{CounterpartyId, Instrument, QuoteId};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};

/// A resource that can be locked for atomic execution.
///
/// Resources are ordered to ensure deterministic lock acquisition,
/// preventing deadlocks in concurrent scenarios.
#[derive(Debug, Clone, Eq, Serialize, Deserialize)]
pub enum ResourceLock {
    /// Lock on a specific quote.
    Quote(QuoteId),
    /// Lock on a counterparty account.
    Account(CounterpartyId),
    /// Lock on an instrument.
    Instrument(Instrument),
}

impl ResourceLock {
    /// Returns the lock category for ordering purposes.
    ///
    /// Categories are ordered: Quote (0) < Account (1) < Instrument (2)
    #[must_use]
    #[inline]
    fn category(&self) -> u8 {
        match self {
            Self::Quote(_) => 0,
            Self::Account(_) => 1,
            Self::Instrument(_) => 2,
        }
    }

    /// Returns a string key for ordering within the same category.
    #[must_use]
    fn sort_key(&self) -> String {
        match self {
            Self::Quote(id) => id.to_string(),
            Self::Account(id) => id.to_string(),
            Self::Instrument(inst) => inst.symbol().to_string(),
        }
    }

    /// Returns true if this is a quote lock.
    #[must_use]
    #[inline]
    pub fn is_quote(&self) -> bool {
        matches!(self, Self::Quote(_))
    }

    /// Returns true if this is an account lock.
    #[must_use]
    #[inline]
    pub fn is_account(&self) -> bool {
        matches!(self, Self::Account(_))
    }

    /// Returns true if this is an instrument lock.
    #[must_use]
    #[inline]
    pub fn is_instrument(&self) -> bool {
        matches!(self, Self::Instrument(_))
    }

    /// Returns the quote ID if this is a quote lock.
    #[must_use]
    pub fn as_quote_id(&self) -> Option<&QuoteId> {
        match self {
            Self::Quote(id) => Some(id),
            _ => None,
        }
    }

    /// Returns the counterparty ID if this is an account lock.
    #[must_use]
    pub fn as_account_id(&self) -> Option<&CounterpartyId> {
        match self {
            Self::Account(id) => Some(id),
            _ => None,
        }
    }

    /// Returns the instrument if this is an instrument lock.
    #[must_use]
    pub fn as_instrument(&self) -> Option<&Instrument> {
        match self {
            Self::Instrument(inst) => Some(inst),
            _ => None,
        }
    }
}

impl PartialEq for ResourceLock {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Quote(a), Self::Quote(b)) => a == b,
            (Self::Account(a), Self::Account(b)) => a == b,
            (Self::Instrument(a), Self::Instrument(b)) => a == b,
            _ => false,
        }
    }
}

impl Hash for ResourceLock {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.category().hash(state);
        match self {
            Self::Quote(id) => id.hash(state),
            Self::Account(id) => id.hash(state),
            Self::Instrument(inst) => {
                inst.symbol().hash(state);
                inst.asset_class().hash(state);
                inst.settlement_method().hash(state);
            }
        }
    }
}

impl Ord for ResourceLock {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.category().cmp(&other.category()) {
            Ordering::Equal => self.sort_key().cmp(&other.sort_key()),
            ord => ord,
        }
    }
}

impl PartialOrd for ResourceLock {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for ResourceLock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Quote(id) => write!(f, "Quote({})", id),
            Self::Account(id) => write!(f, "Account({})", id),
            Self::Instrument(inst) => write!(f, "Instrument({})", inst.symbol()),
        }
    }
}

/// Sorts a list of resource locks in deterministic order for deadlock prevention.
///
/// # Arguments
///
/// * `locks` - Mutable slice of resource locks to sort
///
/// # Example
///
/// ```ignore
/// let mut locks = vec![
///     ResourceLock::Account(counterparty_id),
///     ResourceLock::Quote(quote_id),
/// ];
/// sort_locks(&mut locks);
/// // Now locks are in order: Quote, Account
/// ```
#[inline]
pub fn sort_locks(locks: &mut [ResourceLock]) {
    locks.sort();
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::domain::value_objects::enums::{AssetClass, SettlementMethod};
    use crate::domain::value_objects::symbol::Symbol;

    fn create_instrument(symbol: &str) -> Instrument {
        Instrument::new(
            Symbol::new(symbol).unwrap(),
            AssetClass::CryptoSpot,
            SettlementMethod::default(),
        )
    }

    #[test]
    fn resource_lock_category_ordering() {
        let quote = ResourceLock::Quote(QuoteId::new_v4());
        let account = ResourceLock::Account(CounterpartyId::new("test"));
        let instrument = ResourceLock::Instrument(create_instrument("BTC/USD"));

        assert!(quote < account);
        assert!(account < instrument);
        assert!(quote < instrument);
    }

    #[test]
    fn resource_lock_same_category_ordering() {
        let account_a = ResourceLock::Account(CounterpartyId::new("aaa"));
        let account_b = ResourceLock::Account(CounterpartyId::new("bbb"));
        let account_c = ResourceLock::Account(CounterpartyId::new("ccc"));

        assert!(account_a < account_b);
        assert!(account_b < account_c);
    }

    #[test]
    fn sort_locks_deterministic() {
        let quote = ResourceLock::Quote(QuoteId::new_v4());
        let account1 = ResourceLock::Account(CounterpartyId::new("client"));
        let account2 = ResourceLock::Account(CounterpartyId::new("mm"));
        let instrument = ResourceLock::Instrument(create_instrument("ETH/USD"));

        let mut locks = vec![
            instrument.clone(),
            account2.clone(),
            quote.clone(),
            account1.clone(),
        ];
        sort_locks(&mut locks);

        assert_eq!(locks[0], quote);
        assert_eq!(locks[1], account1);
        assert_eq!(locks[2], account2);
        assert_eq!(locks[3], instrument);
    }

    #[test]
    fn resource_lock_equality() {
        let id = QuoteId::new_v4();
        let lock1 = ResourceLock::Quote(id);
        let lock2 = ResourceLock::Quote(id);
        assert_eq!(lock1, lock2);

        let lock3 = ResourceLock::Quote(QuoteId::new_v4());
        assert_ne!(lock1, lock3);
    }

    #[test]
    fn resource_lock_display() {
        let quote = ResourceLock::Quote(QuoteId::new_v4());
        let display = format!("{}", quote);
        assert!(display.starts_with("Quote("));

        let account = ResourceLock::Account(CounterpartyId::new("test-client"));
        assert_eq!(format!("{}", account), "Account(test-client)");

        let instrument = ResourceLock::Instrument(create_instrument("BTC/USD"));
        assert_eq!(format!("{}", instrument), "Instrument(BTC/USD)");
    }

    #[test]
    fn resource_lock_type_checks() {
        let quote = ResourceLock::Quote(QuoteId::new_v4());
        assert!(quote.is_quote());
        assert!(!quote.is_account());
        assert!(!quote.is_instrument());

        let account = ResourceLock::Account(CounterpartyId::new("test"));
        assert!(!account.is_quote());
        assert!(account.is_account());
        assert!(!account.is_instrument());

        let instrument = ResourceLock::Instrument(create_instrument("BTC/USD"));
        assert!(!instrument.is_quote());
        assert!(!instrument.is_account());
        assert!(instrument.is_instrument());
    }

    #[test]
    fn resource_lock_accessors() {
        let quote_id = QuoteId::new_v4();
        let quote = ResourceLock::Quote(quote_id);
        assert_eq!(quote.as_quote_id(), Some(&quote_id));
        assert_eq!(quote.as_account_id(), None);
        assert_eq!(quote.as_instrument(), None);

        let counterparty_id = CounterpartyId::new("test");
        let account = ResourceLock::Account(counterparty_id.clone());
        assert_eq!(account.as_quote_id(), None);
        assert_eq!(account.as_account_id(), Some(&counterparty_id));
        assert_eq!(account.as_instrument(), None);
    }

    #[test]
    fn resource_lock_hash_consistency() {
        use std::collections::HashSet;

        let id = QuoteId::new_v4();
        let lock1 = ResourceLock::Quote(id);
        let lock2 = ResourceLock::Quote(id);

        let mut set = HashSet::new();
        set.insert(lock1.clone());
        assert!(set.contains(&lock2));
        assert_eq!(set.len(), 1);

        set.insert(lock2);
        assert_eq!(set.len(), 1); // Same lock, no duplicate
    }
}
