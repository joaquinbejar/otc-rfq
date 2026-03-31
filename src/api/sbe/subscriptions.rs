//! # SBE Subscription Management
//!
//! Per-connection subscription tracking for streaming quote and RFQ status updates.

use crate::api::sbe::error::{SbeApiError, SbeApiResult};
use std::collections::HashSet;
use uuid::Uuid;

/// Per-connection subscription tracking.
///
/// Tracks which RFQs a session is subscribed to for quote and status updates.
/// Designed for single-owner use within a connection task; sharing between
/// reader and forwarder tasks requires `Arc<RwLock<Self>>`.
#[derive(Debug)]
pub(crate) struct SessionSubscriptions {
    /// RFQ IDs subscribed for quote updates.
    quotes: HashSet<Uuid>,
    /// RFQ IDs subscribed for status updates.
    status: HashSet<Uuid>,
    /// Maximum total subscriptions allowed.
    max: usize,
}

impl SessionSubscriptions {
    /// Creates a new subscription tracker with the given capacity limit.
    #[must_use]
    pub(crate) fn new(max_subscriptions: usize) -> Self {
        Self {
            quotes: HashSet::new(),
            status: HashSet::new(),
            max: max_subscriptions,
        }
    }

    /// Subscribes to quote updates for the given RFQ.
    ///
    /// # Errors
    ///
    /// Returns `SbeApiError::InvalidValue` if subscription limit is reached.
    pub(crate) fn subscribe_quotes(&mut self, rfq_id: Uuid) -> SbeApiResult<()> {
        if self.count() >= self.max && !self.quotes.contains(&rfq_id) {
            return Err(subscription_limit_error());
        }
        self.quotes.insert(rfq_id);
        Ok(())
    }

    /// Subscribes to status updates for the given RFQ.
    ///
    /// # Errors
    ///
    /// Returns `SbeApiError::InvalidValue` if subscription limit is reached.
    pub(crate) fn subscribe_status(&mut self, rfq_id: Uuid) -> SbeApiResult<()> {
        if self.count() >= self.max && !self.status.contains(&rfq_id) {
            return Err(subscription_limit_error());
        }
        self.status.insert(rfq_id);
        Ok(())
    }

    /// Unsubscribes from all updates for the given RFQ.
    ///
    /// Removes the RFQ from both quote and status subscription sets.
    /// Idempotent - does nothing if RFQ is not subscribed.
    pub(crate) fn unsubscribe(&mut self, rfq_id: Uuid) {
        self.quotes.remove(&rfq_id);
        self.status.remove(&rfq_id);
    }

    /// Checks if subscribed to quote updates for the given RFQ.
    #[inline]
    #[must_use]
    pub(crate) fn is_subscribed_quotes(&self, rfq_id: &Uuid) -> bool {
        self.quotes.contains(rfq_id)
    }

    /// Checks if subscribed to status updates for the given RFQ.
    #[inline]
    #[must_use]
    pub(crate) fn is_subscribed_status(&self, rfq_id: &Uuid) -> bool {
        self.status.contains(rfq_id)
    }

    /// Returns the total number of subscriptions across both channels.
    #[must_use]
    pub(crate) fn count(&self) -> usize {
        self.quotes.len().saturating_add(self.status.len())
    }
}

/// Constructs subscription limit error.
#[inline(never)]
#[cold]
fn subscription_limit_error() -> SbeApiError {
    SbeApiError::InvalidValue {
        field: "subscription",
        message: "subscription limit reached".to_string(),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn new_creates_empty_subscriptions() {
        let subs = SessionSubscriptions::new(10);
        assert_eq!(subs.count(), 0);
    }

    #[test]
    fn subscribe_quotes_adds_subscription() {
        let mut subs = SessionSubscriptions::new(10);
        let rfq_id = Uuid::new_v4();

        subs.subscribe_quotes(rfq_id).unwrap();

        assert!(subs.is_subscribed_quotes(&rfq_id));
        assert!(!subs.is_subscribed_status(&rfq_id));
        assert_eq!(subs.count(), 1);
    }

    #[test]
    fn subscribe_status_adds_subscription() {
        let mut subs = SessionSubscriptions::new(10);
        let rfq_id = Uuid::new_v4();

        subs.subscribe_status(rfq_id).unwrap();

        assert!(!subs.is_subscribed_quotes(&rfq_id));
        assert!(subs.is_subscribed_status(&rfq_id));
        assert_eq!(subs.count(), 1);
    }

    #[test]
    fn subscribe_both_channels_counts_separately() {
        let mut subs = SessionSubscriptions::new(10);
        let rfq_id = Uuid::new_v4();

        subs.subscribe_quotes(rfq_id).unwrap();
        subs.subscribe_status(rfq_id).unwrap();

        assert!(subs.is_subscribed_quotes(&rfq_id));
        assert!(subs.is_subscribed_status(&rfq_id));
        assert_eq!(subs.count(), 2);
    }

    #[test]
    fn duplicate_subscribe_quotes_is_idempotent() {
        let mut subs = SessionSubscriptions::new(10);
        let rfq_id = Uuid::new_v4();

        subs.subscribe_quotes(rfq_id).unwrap();
        subs.subscribe_quotes(rfq_id).unwrap();

        assert_eq!(subs.count(), 1);
    }

    #[test]
    fn duplicate_subscribe_status_is_idempotent() {
        let mut subs = SessionSubscriptions::new(10);
        let rfq_id = Uuid::new_v4();

        subs.subscribe_status(rfq_id).unwrap();
        subs.subscribe_status(rfq_id).unwrap();

        assert_eq!(subs.count(), 1);
    }

    #[test]
    fn unsubscribe_removes_from_both_channels() {
        let mut subs = SessionSubscriptions::new(10);
        let rfq_id = Uuid::new_v4();

        subs.subscribe_quotes(rfq_id).unwrap();
        subs.subscribe_status(rfq_id).unwrap();
        assert_eq!(subs.count(), 2);

        subs.unsubscribe(rfq_id);

        assert!(!subs.is_subscribed_quotes(&rfq_id));
        assert!(!subs.is_subscribed_status(&rfq_id));
        assert_eq!(subs.count(), 0);
    }

    #[test]
    fn unsubscribe_unknown_rfq_is_noop() {
        let mut subs = SessionSubscriptions::new(10);
        let rfq_id = Uuid::new_v4();

        subs.unsubscribe(rfq_id);

        assert_eq!(subs.count(), 0);
    }

    #[test]
    fn subscription_limit_enforced_for_quotes() {
        let mut subs = SessionSubscriptions::new(2);
        let rfq1 = Uuid::new_v4();
        let rfq2 = Uuid::new_v4();
        let rfq3 = Uuid::new_v4();

        subs.subscribe_quotes(rfq1).unwrap();
        subs.subscribe_quotes(rfq2).unwrap();

        let result = subs.subscribe_quotes(rfq3);
        assert!(matches!(
            result,
            Err(SbeApiError::InvalidValue {
                field: "subscription",
                ..
            })
        ));
    }

    #[test]
    fn subscription_limit_enforced_for_status() {
        let mut subs = SessionSubscriptions::new(2);
        let rfq1 = Uuid::new_v4();
        let rfq2 = Uuid::new_v4();
        let rfq3 = Uuid::new_v4();

        subs.subscribe_status(rfq1).unwrap();
        subs.subscribe_status(rfq2).unwrap();

        let result = subs.subscribe_status(rfq3);
        assert!(matches!(
            result,
            Err(SbeApiError::InvalidValue {
                field: "subscription",
                ..
            })
        ));
    }

    #[test]
    fn subscription_limit_counts_both_channels() {
        let mut subs = SessionSubscriptions::new(2);
        let rfq1 = Uuid::new_v4();
        let rfq2 = Uuid::new_v4();

        subs.subscribe_quotes(rfq1).unwrap();
        subs.subscribe_status(rfq2).unwrap();

        let rfq3 = Uuid::new_v4();
        let result = subs.subscribe_quotes(rfq3);
        assert!(result.is_err());
    }

    #[test]
    fn duplicate_subscribe_does_not_count_against_limit() {
        let mut subs = SessionSubscriptions::new(1);
        let rfq_id = Uuid::new_v4();

        subs.subscribe_quotes(rfq_id).unwrap();
        subs.subscribe_quotes(rfq_id).unwrap();

        assert_eq!(subs.count(), 1);
    }

    #[test]
    fn count_returns_total_across_channels() {
        let mut subs = SessionSubscriptions::new(10);
        let rfq1 = Uuid::new_v4();
        let rfq2 = Uuid::new_v4();
        let rfq3 = Uuid::new_v4();

        subs.subscribe_quotes(rfq1).unwrap();
        subs.subscribe_quotes(rfq2).unwrap();
        subs.subscribe_status(rfq3).unwrap();

        assert_eq!(subs.count(), 3);
    }
}
