//! # In-Memory Trade Repository
//!
//! In-memory implementation of [`TradeRepository`] for testing.
//!
//! This implementation uses a thread-safe `HashMap` for storage,
//! making it suitable for unit tests without database dependencies.

use crate::domain::entities::SettlementState;
use crate::domain::entities::trade::Trade;
use crate::domain::value_objects::{RfqId, TradeId, VenueId};
use crate::infrastructure::persistence::traits::{
    RepositoryError, RepositoryResult, TradeRepository,
};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// In-memory implementation of [`TradeRepository`].
///
/// Uses a thread-safe `HashMap` for storage. Suitable for unit tests
/// without database dependencies.
#[derive(Debug, Clone)]
pub struct InMemoryTradeRepository {
    storage: Arc<RwLock<HashMap<TradeId, Trade>>>,
}

impl InMemoryTradeRepository {
    /// Creates a new empty in-memory trade repository.
    #[must_use]
    pub fn new() -> Self {
        Self {
            storage: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Returns the number of trades in the repository.
    #[must_use]
    pub fn len(&self) -> usize {
        self.storage
            .try_read()
            .map(|guard| guard.len())
            .unwrap_or(0)
    }

    /// Returns true if the repository is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Clears all trades from the repository.
    pub async fn clear(&self) {
        let mut storage = self.storage.write().await;
        storage.clear();
    }
}

impl Default for InMemoryTradeRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TradeRepository for InMemoryTradeRepository {
    async fn save(&self, trade: &Trade) -> RepositoryResult<()> {
        let mut storage = self.storage.write().await;

        // Check for version conflict if updating
        if let Some(existing) = storage.get(&trade.id())
            && existing.version() >= trade.version()
        {
            return Err(RepositoryError::version_conflict(
                "Trade",
                trade.id().to_string(),
                trade.version(),
                existing.version(),
            ));
        }

        storage.insert(trade.id(), trade.clone());
        Ok(())
    }

    async fn get(&self, id: &TradeId) -> RepositoryResult<Option<Trade>> {
        let storage = self.storage.read().await;
        Ok(storage.get(id).cloned())
    }

    async fn get_by_rfq(&self, rfq_id: &RfqId) -> RepositoryResult<Option<Trade>> {
        let storage = self.storage.read().await;
        let trade = storage.values().find(|t| t.rfq_id() == *rfq_id).cloned();
        Ok(trade)
    }

    async fn find_pending_settlement(&self) -> RepositoryResult<Vec<Trade>> {
        let storage = self.storage.read().await;
        let pending: Vec<Trade> = storage
            .values()
            .filter(|t| t.settlement_state() == SettlementState::Pending)
            .cloned()
            .collect();
        Ok(pending)
    }

    async fn find_by_venue(&self, venue_id: &VenueId) -> RepositoryResult<Vec<Trade>> {
        let storage = self.storage.read().await;
        let trades: Vec<Trade> = storage
            .values()
            .filter(|t| t.venue_id() == venue_id)
            .cloned()
            .collect();
        Ok(trades)
    }

    async fn find_settled(&self) -> RepositoryResult<Vec<Trade>> {
        let storage = self.storage.read().await;
        let settled: Vec<Trade> = storage
            .values()
            .filter(|t| t.settlement_state() == SettlementState::Settled)
            .cloned()
            .collect();
        Ok(settled)
    }

    async fn find_failed(&self) -> RepositoryResult<Vec<Trade>> {
        let storage = self.storage.read().await;
        let failed: Vec<Trade> = storage
            .values()
            .filter(|t| t.settlement_state() == SettlementState::Failed)
            .cloned()
            .collect();
        Ok(failed)
    }

    async fn delete(&self, id: &TradeId) -> RepositoryResult<bool> {
        let mut storage = self.storage.write().await;
        Ok(storage.remove(id).is_some())
    }

    async fn count(&self) -> RepositoryResult<u64> {
        let storage = self.storage.read().await;
        Ok(storage.len() as u64)
    }

    async fn count_pending_settlement(&self) -> RepositoryResult<u64> {
        let pending = self.find_pending_settlement().await?;
        Ok(pending.len() as u64)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::domain::value_objects::{Price, Quantity, QuoteId, RfqId, VenueId};

    fn create_test_trade(venue_id: &str) -> Trade {
        Trade::new(
            RfqId::new_v4(),
            QuoteId::new_v4(),
            VenueId::new(venue_id),
            Price::new(100.0).unwrap(),
            Quantity::new(10.0).unwrap(),
        )
    }

    #[tokio::test]
    async fn new_repository_is_empty() {
        let repo = InMemoryTradeRepository::new();
        assert!(repo.is_empty());
        assert_eq!(repo.count().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn save_and_get() {
        let repo = InMemoryTradeRepository::new();
        let trade = create_test_trade("venue-1");
        let id = trade.id();

        repo.save(&trade).await.unwrap();

        let retrieved = repo.get(&id).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id(), id);
    }

    #[tokio::test]
    async fn get_nonexistent_returns_none() {
        let repo = InMemoryTradeRepository::new();
        let id = TradeId::new_v4();

        let result = repo.get(&id).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn get_by_rfq() {
        let repo = InMemoryTradeRepository::new();
        let trade = create_test_trade("venue-1");
        let rfq_id = trade.rfq_id();

        repo.save(&trade).await.unwrap();

        let retrieved = repo.get_by_rfq(&rfq_id).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().rfq_id(), rfq_id);
    }

    #[tokio::test]
    async fn find_by_venue() {
        let repo = InMemoryTradeRepository::new();

        let trade1 = create_test_trade("venue-1");
        let trade2 = create_test_trade("venue-1");
        let trade3 = create_test_trade("venue-2");

        repo.save(&trade1).await.unwrap();
        repo.save(&trade2).await.unwrap();
        repo.save(&trade3).await.unwrap();

        let venue1_trades = repo.find_by_venue(&VenueId::new("venue-1")).await.unwrap();
        assert_eq!(venue1_trades.len(), 2);

        let venue2_trades = repo.find_by_venue(&VenueId::new("venue-2")).await.unwrap();
        assert_eq!(venue2_trades.len(), 1);
    }

    #[tokio::test]
    async fn find_pending_settlement() {
        let repo = InMemoryTradeRepository::new();

        let trade = create_test_trade("venue-1");
        repo.save(&trade).await.unwrap();

        let pending = repo.find_pending_settlement().await.unwrap();
        assert_eq!(pending.len(), 1);
    }

    #[tokio::test]
    async fn delete() {
        let repo = InMemoryTradeRepository::new();
        let trade = create_test_trade("venue-1");
        let id = trade.id();

        repo.save(&trade).await.unwrap();
        assert_eq!(repo.count().await.unwrap(), 1);

        let deleted = repo.delete(&id).await.unwrap();
        assert!(deleted);
        assert_eq!(repo.count().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn clear() {
        let repo = InMemoryTradeRepository::new();

        repo.save(&create_test_trade("venue-1")).await.unwrap();
        repo.save(&create_test_trade("venue-2")).await.unwrap();
        assert_eq!(repo.count().await.unwrap(), 2);

        repo.clear().await;
        assert_eq!(repo.count().await.unwrap(), 0);
    }
}
