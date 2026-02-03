//! # In-Memory RFQ Repository
//!
//! In-memory implementation of [`RfqRepository`] for testing.
//!
//! This implementation uses a thread-safe `HashMap` for storage,
//! making it suitable for unit tests without database dependencies.
//!
//! # Examples
//!
//! ```
//! use otc_rfq::infrastructure::persistence::in_memory::InMemoryRfqRepository;
//! use otc_rfq::infrastructure::persistence::RfqRepository;
//!
//! let repo = InMemoryRfqRepository::new();
//! ```

use crate::domain::entities::rfq::Rfq;
use crate::domain::value_objects::RfqState;
use crate::domain::value_objects::{CounterpartyId, RfqId, VenueId};
use crate::infrastructure::persistence::traits::{
    RepositoryError, RepositoryResult, RfqRepository,
};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// In-memory implementation of [`RfqRepository`].
///
/// Uses a thread-safe `HashMap` for storage. Suitable for unit tests
/// without database dependencies.
///
/// # Thread Safety
///
/// This implementation uses `Arc<RwLock<HashMap>>` for thread-safe access.
///
/// # Examples
///
/// ```
/// use otc_rfq::infrastructure::persistence::in_memory::InMemoryRfqRepository;
/// use otc_rfq::infrastructure::persistence::RfqRepository;
///
/// let repo = InMemoryRfqRepository::new();
/// assert_eq!(repo.len(), 0);
/// ```
#[derive(Debug, Clone)]
pub struct InMemoryRfqRepository {
    storage: Arc<RwLock<HashMap<RfqId, Rfq>>>,
}

impl InMemoryRfqRepository {
    /// Creates a new empty in-memory RFQ repository.
    #[must_use]
    pub fn new() -> Self {
        Self {
            storage: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Returns the number of RFQs in the repository.
    #[must_use]
    pub fn len(&self) -> usize {
        // Use try_read to avoid blocking in sync context
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

    /// Clears all RFQs from the repository.
    pub async fn clear(&self) {
        let mut storage = self.storage.write().await;
        storage.clear();
    }
}

impl Default for InMemoryRfqRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl RfqRepository for InMemoryRfqRepository {
    async fn save(&self, rfq: &Rfq) -> RepositoryResult<()> {
        let mut storage = self.storage.write().await;

        // Check for version conflict if updating
        if let Some(existing) = storage.get(&rfq.id())
            && existing.version() >= rfq.version()
        {
            return Err(RepositoryError::version_conflict(
                "Rfq",
                rfq.id().to_string(),
                rfq.version(),
                existing.version(),
            ));
        }

        storage.insert(rfq.id(), rfq.clone());
        Ok(())
    }

    async fn get(&self, id: &RfqId) -> RepositoryResult<Option<Rfq>> {
        let storage = self.storage.read().await;
        Ok(storage.get(id).cloned())
    }

    async fn find_active(&self) -> RepositoryResult<Vec<Rfq>> {
        let storage = self.storage.read().await;
        let active: Vec<Rfq> = storage
            .values()
            .filter(|rfq| {
                matches!(
                    rfq.state(),
                    RfqState::Created
                        | RfqState::QuoteRequesting
                        | RfqState::QuotesReceived
                        | RfqState::ClientSelecting
                        | RfqState::Executing
                )
            })
            .cloned()
            .collect();
        Ok(active)
    }

    async fn find_by_client(&self, client_id: &CounterpartyId) -> RepositoryResult<Vec<Rfq>> {
        let storage = self.storage.read().await;
        let rfqs: Vec<Rfq> = storage
            .values()
            .filter(|rfq| rfq.client_id() == client_id)
            .cloned()
            .collect();
        Ok(rfqs)
    }

    async fn find_by_venue(&self, venue_id: &VenueId) -> RepositoryResult<Vec<Rfq>> {
        let storage = self.storage.read().await;
        let rfqs: Vec<Rfq> = storage
            .values()
            .filter(|rfq| rfq.quotes().iter().any(|q| q.venue_id() == venue_id))
            .cloned()
            .collect();
        Ok(rfqs)
    }

    async fn delete(&self, id: &RfqId) -> RepositoryResult<bool> {
        let mut storage = self.storage.write().await;
        Ok(storage.remove(id).is_some())
    }

    async fn count(&self) -> RepositoryResult<u64> {
        let storage = self.storage.read().await;
        Ok(storage.len() as u64)
    }

    async fn count_active(&self) -> RepositoryResult<u64> {
        let active = self.find_active().await?;
        Ok(active.len() as u64)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::domain::entities::rfq::RfqBuilder;
    use crate::domain::value_objects::enums::OrderSide;
    use crate::domain::value_objects::timestamp::Timestamp;
    use crate::domain::value_objects::{Instrument, Quantity, Symbol};

    fn create_test_rfq(client_id: &str) -> Rfq {
        use crate::domain::value_objects::enums::AssetClass;
        let symbol = Symbol::new("ETH/USDC").unwrap();
        let instrument = Instrument::builder(symbol, AssetClass::CryptoSpot).build();
        RfqBuilder::new(
            CounterpartyId::new(client_id),
            instrument,
            OrderSide::Buy,
            Quantity::new(10.0).unwrap(),
            Timestamp::now().add_secs(3600),
        )
        .build()
    }

    #[tokio::test]
    async fn new_repository_is_empty() {
        let repo = InMemoryRfqRepository::new();
        assert!(repo.is_empty());
        assert_eq!(repo.count().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn save_and_get() {
        let repo = InMemoryRfqRepository::new();
        let rfq = create_test_rfq("client-1");
        let id = rfq.id();

        repo.save(&rfq).await.unwrap();

        let retrieved = repo.get(&id).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id(), id);
    }

    #[tokio::test]
    async fn get_nonexistent_returns_none() {
        let repo = InMemoryRfqRepository::new();
        let id = RfqId::new_v4();

        let result = repo.get(&id).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn find_by_client() {
        let repo = InMemoryRfqRepository::new();

        let rfq1 = create_test_rfq("client-1");
        let rfq2 = create_test_rfq("client-1");
        let rfq3 = create_test_rfq("client-2");

        repo.save(&rfq1).await.unwrap();
        repo.save(&rfq2).await.unwrap();
        repo.save(&rfq3).await.unwrap();

        let client1_rfqs = repo
            .find_by_client(&CounterpartyId::new("client-1"))
            .await
            .unwrap();
        assert_eq!(client1_rfqs.len(), 2);

        let client2_rfqs = repo
            .find_by_client(&CounterpartyId::new("client-2"))
            .await
            .unwrap();
        assert_eq!(client2_rfqs.len(), 1);
    }

    #[tokio::test]
    async fn find_active() {
        let repo = InMemoryRfqRepository::new();

        let rfq = create_test_rfq("client-1");
        repo.save(&rfq).await.unwrap();

        let active = repo.find_active().await.unwrap();
        assert_eq!(active.len(), 1);
    }

    #[tokio::test]
    async fn delete() {
        let repo = InMemoryRfqRepository::new();
        let rfq = create_test_rfq("client-1");
        let id = rfq.id();

        repo.save(&rfq).await.unwrap();
        assert_eq!(repo.count().await.unwrap(), 1);

        let deleted = repo.delete(&id).await.unwrap();
        assert!(deleted);
        assert_eq!(repo.count().await.unwrap(), 0);

        // Delete again returns false
        let deleted_again = repo.delete(&id).await.unwrap();
        assert!(!deleted_again);
    }

    #[tokio::test]
    async fn clear() {
        let repo = InMemoryRfqRepository::new();

        repo.save(&create_test_rfq("client-1")).await.unwrap();
        repo.save(&create_test_rfq("client-2")).await.unwrap();
        assert_eq!(repo.count().await.unwrap(), 2);

        repo.clear().await;
        assert_eq!(repo.count().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn count_active() {
        let repo = InMemoryRfqRepository::new();

        repo.save(&create_test_rfq("client-1")).await.unwrap();
        repo.save(&create_test_rfq("client-2")).await.unwrap();

        let count = repo.count_active().await.unwrap();
        assert_eq!(count, 2);
    }
}
