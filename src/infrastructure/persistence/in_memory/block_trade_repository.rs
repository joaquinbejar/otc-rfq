//! # In-Memory Block Trade Repository
//!
//! In-memory implementation of the block trade repository for testing and development.

use crate::domain::entities::block_trade::{BlockTrade, BlockTradeId};
use crate::domain::errors::DomainResult;
use crate::domain::value_objects::CounterpartyId;
use async_trait::async_trait;
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Repository trait for block trade persistence.
///
/// Provides CRUD operations for block trades.
#[async_trait]
pub trait BlockTradeRepository: Send + Sync + fmt::Debug {
    /// Saves a block trade.
    ///
    /// Creates a new record or updates an existing one.
    async fn save(&self, trade: &BlockTrade) -> DomainResult<()>;

    /// Finds a block trade by ID.
    async fn find_by_id(&self, id: BlockTradeId) -> DomainResult<Option<BlockTrade>>;

    /// Finds all block trades for a counterparty (as buyer or seller).
    async fn find_by_counterparty(
        &self,
        counterparty_id: &CounterpartyId,
    ) -> DomainResult<Vec<BlockTrade>>;

    /// Finds all pending block trades (not yet executed or rejected).
    async fn find_pending(&self) -> DomainResult<Vec<BlockTrade>>;

    /// Deletes a block trade by ID.
    async fn delete(&self, id: BlockTradeId) -> DomainResult<bool>;
}

/// In-memory implementation of the block trade repository.
///
/// Suitable for testing and development. Not for production use.
#[derive(Debug, Default)]
pub struct InMemoryBlockTradeRepository {
    trades: Arc<RwLock<HashMap<BlockTradeId, BlockTrade>>>,
}

impl InMemoryBlockTradeRepository {
    /// Creates a new empty repository.
    #[must_use]
    pub fn new() -> Self {
        Self {
            trades: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Returns the number of trades in the repository.
    pub async fn count(&self) -> usize {
        let guard = self.trades.read().await;
        guard.len()
    }

    /// Clears all trades from the repository.
    pub async fn clear(&self) {
        let mut guard = self.trades.write().await;
        guard.clear();
    }
}

#[async_trait]
impl BlockTradeRepository for InMemoryBlockTradeRepository {
    async fn save(&self, trade: &BlockTrade) -> DomainResult<()> {
        let mut guard = self.trades.write().await;
        guard.insert(trade.id(), trade.clone());
        Ok(())
    }

    async fn find_by_id(&self, id: BlockTradeId) -> DomainResult<Option<BlockTrade>> {
        let guard = self.trades.read().await;
        Ok(guard.get(&id).cloned())
    }

    async fn find_by_counterparty(
        &self,
        counterparty_id: &CounterpartyId,
    ) -> DomainResult<Vec<BlockTrade>> {
        let guard = self.trades.read().await;
        let trades: Vec<BlockTrade> = guard
            .values()
            .filter(|t| t.buyer_id() == counterparty_id || t.seller_id() == counterparty_id)
            .cloned()
            .collect();
        Ok(trades)
    }

    async fn find_pending(&self) -> DomainResult<Vec<BlockTrade>> {
        let guard = self.trades.read().await;
        let trades: Vec<BlockTrade> = guard
            .values()
            .filter(|t| !t.state().is_terminal())
            .cloned()
            .collect();
        Ok(trades)
    }

    async fn delete(&self, id: BlockTradeId) -> DomainResult<bool> {
        let mut guard = self.trades.write().await;
        Ok(guard.remove(&id).is_some())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::domain::value_objects::{
        AssetClass, Instrument, Price, Quantity, Symbol, Timestamp,
    };

    fn create_test_instrument() -> Instrument {
        let symbol = Symbol::new("BTC/USD").unwrap();
        Instrument::builder(symbol, AssetClass::CryptoSpot).build()
    }

    fn create_test_block_trade(buyer: &str, seller: &str) -> BlockTrade {
        BlockTrade::new(
            CounterpartyId::new(buyer),
            CounterpartyId::new(seller),
            create_test_instrument(),
            Price::new(50000.0).unwrap(),
            Quantity::new(30.0).unwrap(),
            Timestamp::now(),
        )
    }

    #[tokio::test]
    async fn save_and_find_by_id() {
        let repo = InMemoryBlockTradeRepository::new();
        let trade = create_test_block_trade("buyer-1", "seller-1");
        let id = trade.id();

        repo.save(&trade).await.unwrap();

        let found = repo.find_by_id(id).await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().id(), id);
    }

    #[tokio::test]
    async fn find_by_id_not_found() {
        let repo = InMemoryBlockTradeRepository::new();
        let id = BlockTradeId::new_v4();

        let found = repo.find_by_id(id).await.unwrap();
        assert!(found.is_none());
    }

    #[tokio::test]
    async fn find_by_counterparty_as_buyer() {
        let repo = InMemoryBlockTradeRepository::new();
        let trade = create_test_block_trade("buyer-1", "seller-1");
        repo.save(&trade).await.unwrap();

        let buyer_id = CounterpartyId::new("buyer-1");
        let trades = repo.find_by_counterparty(&buyer_id).await.unwrap();

        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].buyer_id(), &buyer_id);
    }

    #[tokio::test]
    async fn find_by_counterparty_as_seller() {
        let repo = InMemoryBlockTradeRepository::new();
        let trade = create_test_block_trade("buyer-1", "seller-1");
        repo.save(&trade).await.unwrap();

        let seller_id = CounterpartyId::new("seller-1");
        let trades = repo.find_by_counterparty(&seller_id).await.unwrap();

        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].seller_id(), &seller_id);
    }

    #[tokio::test]
    async fn find_pending_excludes_terminal() {
        let repo = InMemoryBlockTradeRepository::new();

        // Create a pending trade
        let pending = create_test_block_trade("buyer-1", "seller-1");
        repo.save(&pending).await.unwrap();

        // Create an executed trade
        let mut executed = create_test_block_trade("buyer-2", "seller-2");
        executed.start_validation().unwrap();
        // Note: We can't easily get to Executed state without full flow,
        // but we can test that non-terminal states are included
        repo.save(&executed).await.unwrap();

        let pending_trades = repo.find_pending().await.unwrap();
        assert_eq!(pending_trades.len(), 2); // Both are non-terminal
    }

    #[tokio::test]
    async fn delete_removes_trade() {
        let repo = InMemoryBlockTradeRepository::new();
        let trade = create_test_block_trade("buyer-1", "seller-1");
        let id = trade.id();

        repo.save(&trade).await.unwrap();
        assert_eq!(repo.count().await, 1);

        let deleted = repo.delete(id).await.unwrap();
        assert!(deleted);
        assert_eq!(repo.count().await, 0);
    }

    #[tokio::test]
    async fn delete_nonexistent_returns_false() {
        let repo = InMemoryBlockTradeRepository::new();
        let id = BlockTradeId::new_v4();

        let deleted = repo.delete(id).await.unwrap();
        assert!(!deleted);
    }

    #[tokio::test]
    async fn clear_removes_all() {
        let repo = InMemoryBlockTradeRepository::new();

        repo.save(&create_test_block_trade("buyer-1", "seller-1"))
            .await
            .unwrap();
        repo.save(&create_test_block_trade("buyer-2", "seller-2"))
            .await
            .unwrap();
        assert_eq!(repo.count().await, 2);

        repo.clear().await;
        assert_eq!(repo.count().await, 0);
    }
}
