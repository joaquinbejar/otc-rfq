//! # Conflict Resolver Service
//!
//! Handles race conditions during quote acceptance with first-commit-wins semantics.

use crate::domain::errors::{DomainError, DomainResult};
use crate::domain::events::conflict_events::{ConflictType, Resolution};
use crate::domain::services::quote_lock::LockHolderId;
use crate::domain::value_objects::{OrderSide, Price, QuoteId, Timestamp};
use async_trait::async_trait;
use std::fmt;

/// Counter price information including trade direction.
#[derive(Debug, Clone, Copy)]
pub struct CounterPriceInfo {
    /// The requester's price.
    pub requester_price: Price,
    /// The existing holder's price.
    pub existing_price: Price,
    /// The trade side (Buy or Sell) to determine which price is better.
    pub side: OrderSide,
}

impl CounterPriceInfo {
    /// Creates new counter price info.
    #[must_use]
    pub fn new(requester_price: Price, existing_price: Price, side: OrderSide) -> Self {
        Self {
            requester_price,
            existing_price,
            side,
        }
    }

    /// Returns the better price based on trade side.
    /// For Buy: lower is better. For Sell: higher is better.
    #[must_use]
    pub fn better_price(&self) -> Price {
        match self.side {
            OrderSide::Buy => {
                if self.requester_price <= self.existing_price {
                    self.requester_price
                } else {
                    self.existing_price
                }
            }
            OrderSide::Sell => {
                if self.requester_price >= self.existing_price {
                    self.requester_price
                } else {
                    self.existing_price
                }
            }
        }
    }
}

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
    /// Counter prices for counter-crossing resolution (with side info).
    pub counter_prices: Option<CounterPriceInfo>,
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

    /// Sets the quote expiration time.
    #[must_use]
    pub fn with_quote_expires_at(mut self, expires_at: Timestamp) -> Self {
        self.quote_expires_at = Some(expires_at);
        self
    }

    /// Sets the counter prices for crossing resolution with trade side.
    #[must_use]
    pub fn with_counter_prices(
        mut self,
        requester_price: Price,
        existing_price: Price,
        side: OrderSide,
    ) -> Self {
        self.counter_prices = Some(CounterPriceInfo::new(requester_price, existing_price, side));
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
                match context.existing_holder {
                    Some(holder) => Ok(Resolution::first_commit_wins(holder)),
                    None => Err(DomainError::ConflictDetected(
                        "existing holder is required for first-commit conflicts".to_string(),
                    )),
                }
            }
            ConflictType::ExpiredMidTransaction => Ok(Resolution::cancel(format!(
                "quote {} expired",
                context.quote_id
            ))),
            ConflictType::CounterCrossing => match context.counter_prices {
                Some(price_info) => Ok(Resolution::accept_better_price(price_info.better_price())),
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
        assert!(res.can_proceed_as(&winner));
    }

    #[test]
    fn resolution_requires_followup() {
        let retry = Resolution::Retry {
            delay: std::time::Duration::from_secs(1),
            max_attempts: 3,
        };
        assert!(retry.requires_followup());

        let cancel = Resolution::cancel("test");
        assert!(!cancel.requires_followup());
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
    async fn resolver_quote_already_taken_missing_holder() {
        let resolver = FirstCommitWinsResolver::new();
        let ctx = ConflictContext::new(QuoteId::new_v4(), LockHolderId::new());
        let res = resolver.resolve(ConflictType::QuoteAlreadyTaken, ctx).await;
        assert!(matches!(res, Err(DomainError::ConflictDetected(_))));
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
    async fn resolver_counter_crossing_buy_side() {
        let resolver = FirstCommitWinsResolver::new();
        let p1 = Price::new(100.0).unwrap();
        let p2 = Price::new(105.0).unwrap();
        let ctx = ConflictContext::new(QuoteId::new_v4(), LockHolderId::new()).with_counter_prices(
            p1,
            p2,
            OrderSide::Buy,
        );
        let res = resolver
            .resolve(ConflictType::CounterCrossing, ctx)
            .await
            .unwrap();
        // For buy side, lower price (p1=100) is better
        assert!(
            matches!(res, Resolution::AcceptBetterPrice { accepted_price } if accepted_price == p1)
        );
    }

    #[tokio::test]
    async fn resolver_counter_crossing_sell_side() {
        let resolver = FirstCommitWinsResolver::new();
        let p1 = Price::new(100.0).unwrap();
        let p2 = Price::new(105.0).unwrap();
        let ctx = ConflictContext::new(QuoteId::new_v4(), LockHolderId::new()).with_counter_prices(
            p1,
            p2,
            OrderSide::Sell,
        );
        let res = resolver
            .resolve(ConflictType::CounterCrossing, ctx)
            .await
            .unwrap();
        // For sell side, higher price (p2=105) is better
        assert!(
            matches!(res, Resolution::AcceptBetterPrice { accepted_price } if accepted_price == p2)
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
        assert!(matches!(res, Resolution::FirstCommitWins { winner_id } if winner_id == winner));
    }

    #[test]
    fn counter_price_info_better_price_buy() {
        let p1 = Price::new(100.0).unwrap();
        let p2 = Price::new(105.0).unwrap();
        let info = CounterPriceInfo::new(p1, p2, OrderSide::Buy);
        assert_eq!(info.better_price(), p1);
    }

    #[test]
    fn counter_price_info_better_price_sell() {
        let p1 = Price::new(100.0).unwrap();
        let p2 = Price::new(105.0).unwrap();
        let info = CounterPriceInfo::new(p1, p2, OrderSide::Sell);
        assert_eq!(info.better_price(), p2);
    }

    #[test]
    fn with_quote_expires_at_builder() {
        let expires = Timestamp::now().add_secs(60);
        let ctx = ConflictContext::new(QuoteId::new_v4(), LockHolderId::new())
            .with_quote_expires_at(expires);
        assert_eq!(ctx.quote_expires_at, Some(expires));
    }
}
