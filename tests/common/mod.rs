//! Shared test infrastructure for SBE integration tests.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    dead_code
)]

use otc_rfq::application::use_cases::create_rfq::RfqRepository;
use otc_rfq::domain::entities::rfq::Rfq;
use otc_rfq::domain::value_objects::RfqId;
use std::collections::HashMap;
use tokio::sync::RwLock;

/// Mock RFQ repository for integration tests.
#[derive(Debug)]
pub struct MockRfqRepository {
    rfqs: RwLock<HashMap<RfqId, Rfq>>,
}

impl MockRfqRepository {
    /// Creates a new empty mock repository.
    pub fn new() -> Self {
        Self {
            rfqs: RwLock::new(HashMap::new()),
        }
    }
}

#[async_trait::async_trait]
impl RfqRepository for MockRfqRepository {
    async fn save(&self, rfq: &Rfq) -> Result<(), String> {
        self.rfqs.write().await.insert(rfq.id(), rfq.clone());
        Ok(())
    }

    async fn find_by_id(&self, id: RfqId) -> Result<Option<Rfq>, String> {
        Ok(self.rfqs.read().await.get(&id).cloned())
    }
}
