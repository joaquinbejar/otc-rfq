//! # Conflict Resolver Service
//!
//! Handles race conditions during quote acceptance with first-commit-wins semantics.

use crate::domain::errors::{DomainError, DomainResult};
use crate::domain::events::conflict_events::{ConflictType, Resolution};
use crate::domain::services::quote_lock::LockHolderId;
use crate::domain::value_objects::{Price, QuoteId, Timestamp};
use async_trait::async_trait;
use std::fmt;

/// Context information for conflict resolution.
#[derive(Debug, Clone)]
pub struct ConflictContext {
    /// The quote involved in the conflict.
    pub quote_id: QuoteId,
    /// The requester who triggered the conflict.
    pub requester_id: LockHolderId,
    /// The existing holder (if any) who has the lock.
    pub existing_holder: Option<LockHolderId>,
    /// When the quote expires (if known).
    pub quote_expires_at: Option<Timestamp>,
    /// Counter prices for counter-crossing resolution.
    pub counter_prices: Option<(Price, Price)>,
}

impl ConflictContext {
    /// Creates a new conflict context.
    #[must_use]
    pub fn new(quote_id: QuoteId, requester_id: LockHolderId) -> Self {
        Self {
            quote_id,
            requester_id,
            existing_holder: None,
            quote_expires_at: None,
            counter_prices: None,
        }
    }

    /// Sets the existing holder.
    #[must_use]
    pub fn with_existing_holder(mut self, holder: LockHolderId) -> Self {
        self.existing_holder = Some(holder);
        self
    }

    /// Sets the counter prices for crossing resolution.
    #[must_use]
    pub fn with_counter_prices(mut self, requester_price: Price, existing_price: Price) -> Self {
        self.counter_prices = Some((requester_price, existing_price));
        self
    }
}

/// Service for resolving conflicts in quote acceptance.
#[async_trait]
pub trait ConflictResolver: Send + Sync + fmt::Debug {
    /// Resolves a conflict and returns the appropriate resolution.
    async fn resolve(
        &self,
        conflict: ConflictType,
        context: ConflictContext,
    ) -> DomainResult<Resolution>;
}

/// Default conflict resolver using first-commit-wins semantics.
#[derive(Debug, Clone, Default)]
pub struct FirstCommitWinsResolver;

impl FirstCommitWinsResolver {
    /// Creates a new resolver.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ConflictResolver for FirstCommitWinsResolver {
    async fn resolve(
        &self,
        conflict: ConflictType,
        context: ConflictContext,
    ) -> DomainResult<Resolution> {
        match conflict {
            ConflictType::QuoteAlreadyTaken | ConflictType::SimultaneousAcceptance => {
                let winner = context.existing_holder.unwrap_or(context.requester_id);
                Ok(Resolution::first_commit_wins(winner))
            }
            ConflictType::ExpiredMidTransaction => Ok(Resolution::cancel(format!(
                "quote {} expired",
                context.quote_id
            ))),
            ConflictType::CounterCrossing => match context.counter_prices {
                Some((req_price, exist_price)) => {
                    let better = if req_price <= exist_price {
                        req_price
                    } else {
                        exist_price
                    };
                    Ok(Resolution::accept_better_price(better))
                }
                None => Err(DomainError::ConflictDetected(
                    "counter-crossing requires prices".to_string(),
                )),
            },
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn conflict_type_display() {
        assert_eq!(
            ConflictType::QuoteAlreadyTaken.to_string(),
            "quote_already_taken"
        );
    }

    #[test]
    fn resolution_first_commit_wins() {
        let winner = LockHolderId::new();
        let res = Resolution::first_commit_wins(winner);
        assert!(!res.allows_proceed());
    }

    #[tokio::test]
    async fn resolver_quote_already_taken() {
        let resolver = FirstCommitWinsResolver::new();
        let ctx = ConflictContext::new(QuoteId::new_v4(), LockHolderId::new())
            .with_existing_holder(LockHolderId::new());
        let res = resolver
            .resolve(ConflictType::QuoteAlreadyTaken, ctx)
            .await
            .unwrap();
        assert!(matches!(res, Resolution::FirstCommitWins { .. }));
    }

    #[tokio::test]
    async fn resolver_expired_mid_transaction() {
        let resolver = FirstCommitWinsResolver::new();
        let ctx = ConflictContext::new(QuoteId::new_v4(), LockHolderId::new());
        let res = resolver
            .resolve(ConflictType::ExpiredMidTransaction, ctx)
            .await
            .unwrap();
        assert!(matches!(res, Resolution::Cancel { .. }));
    }

    #[tokio::test]
    async fn resolver_counter_crossing() {
        let resolver = FirstCommitWinsResolver::new();
        let p1 = Price::new(100.0).unwrap();
        let p2 = Price::new(105.0).unwrap();
        let ctx = ConflictContext::new(QuoteId::new_v4(), LockHolderId::new())
            .with_counter_prices(p1, p2);
        let res = resolver
            .resolve(ConflictType::CounterCrossing, ctx)
            .await
            .unwrap();
        assert!(
            matches!(res, Resolution::AcceptBetterPrice { accepted_price } if accepted_price == p1)
        );
    }

    #[tokio::test]
    async fn resolver_counter_crossing_missing_prices() {
        let resolver = FirstCommitWinsResolver::new();
        let ctx = ConflictContext::new(QuoteId::new_v4(), LockHolderId::new());
        let res = resolver.resolve(ConflictType::CounterCrossing, ctx).await;
        assert!(matches!(res, Err(DomainError::ConflictDetected(_))));
    }

    #[tokio::test]
    async fn resolver_simultaneous_acceptance() {
        let resolver = FirstCommitWinsResolver::new();
        let winner = LockHolderId::new();
        let ctx = ConflictContext::new(QuoteId::new_v4(), LockHolderId::new())
            .with_existing_holder(winner);
        let res = resolver
            .resolve(ConflictType::SimultaneousAcceptance, ctx)
            .await
            .unwrap();
        match res {
            Resolution::FirstCommitWins { winner_id } => assert_eq!(winner_id, winner),
            _ => panic!("expected FirstCommitWins"),
        }
    }

    /// Stress test: 100 concurrent conflict resolutions should all succeed.
    #[tokio::test]
    async fn stress_test_concurrent_resolutions() {
        use std::sync::Arc;
        use tokio::task::JoinSet;

        let resolver = Arc::new(FirstCommitWinsResolver::new());
        let quote_id = QuoteId::new_v4();
        let winner = LockHolderId::new();
        let mut tasks = JoinSet::new();

        // Spawn 100 concurrent resolution requests
        for _ in 0..100 {
            let resolver = Arc::clone(&resolver);
            let ctx =
                ConflictContext::new(quote_id, LockHolderId::new()).with_existing_holder(winner);

            tasks.spawn(async move {
                resolver
                    .resolve(ConflictType::SimultaneousAcceptance, ctx)
                    .await
            });
        }

        // All should resolve successfully with the same winner
        let mut success_count = 0;
        while let Some(result) = tasks.join_next().await {
            let resolution = result.unwrap().unwrap();
            match resolution {
                Resolution::FirstCommitWins { winner_id } => {
                    assert_eq!(winner_id, winner);
                    success_count += 1;
                }
                _ => panic!("unexpected resolution"),
            }
        }

        assert_eq!(success_count, 100);
    }
}
