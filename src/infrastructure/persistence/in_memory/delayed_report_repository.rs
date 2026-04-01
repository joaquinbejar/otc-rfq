//! # In-Memory Delayed Report Repository
//!
//! In-memory implementation of [`DelayedReportRepository`] for testing
//! and development purposes.

use crate::domain::entities::delayed_report::DelayedReport;
use crate::domain::value_objects::{BlockTradeId, Timestamp};
use crate::infrastructure::persistence::traits::{
    DelayedReportRepository, RepositoryError, RepositoryResult,
};
use async_trait::async_trait;
use std::collections::HashMap;
use tokio::sync::RwLock;
use uuid::Uuid;

/// In-memory implementation of [`DelayedReportRepository`].
///
/// Stores delayed reports in a `HashMap` protected by a `tokio::sync::RwLock`.
/// Suitable for testing and development; not for production use.
#[derive(Debug)]
pub struct InMemoryDelayedReportRepository {
    reports: RwLock<HashMap<Uuid, DelayedReport>>,
}

impl Default for InMemoryDelayedReportRepository {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryDelayedReportRepository {
    /// Creates a new empty repository.
    #[must_use]
    pub fn new() -> Self {
        Self {
            reports: RwLock::new(HashMap::new()),
        }
    }

    /// Returns the number of stored reports.
    pub async fn len(&self) -> usize {
        self.reports.read().await.len()
    }

    /// Returns whether the repository is empty.
    pub async fn is_empty(&self) -> bool {
        self.len().await == 0
    }

    /// Clears all stored reports.
    pub async fn clear(&self) {
        self.reports.write().await.clear();
    }
}

#[async_trait]
impl DelayedReportRepository for InMemoryDelayedReportRepository {
    async fn save(&self, report: &DelayedReport) -> RepositoryResult<()> {
        let mut reports = self.reports.write().await;
        reports.insert(report.id(), report.clone());
        Ok(())
    }

    async fn find_by_id(&self, id: &Uuid) -> RepositoryResult<Option<DelayedReport>> {
        let reports = self.reports.read().await;
        Ok(reports.get(id).cloned())
    }

    async fn find_by_trade_id(
        &self,
        trade_id: BlockTradeId,
    ) -> RepositoryResult<Option<DelayedReport>> {
        let reports = self.reports.read().await;
        Ok(reports.values().find(|r| r.trade_id() == trade_id).cloned())
    }

    async fn find_pending(&self) -> RepositoryResult<Vec<DelayedReport>> {
        let reports = self.reports.read().await;
        Ok(reports
            .values()
            .filter(|r| !r.is_published())
            .cloned()
            .collect())
    }

    async fn find_ready_to_publish(&self, now: Timestamp) -> RepositoryResult<Vec<DelayedReport>> {
        let reports = self.reports.read().await;
        Ok(reports
            .values()
            .filter(|r| !r.is_published() && r.publish_at() <= now)
            .cloned()
            .collect())
    }

    async fn mark_published(&self, id: &Uuid, published_at: Timestamp) -> RepositoryResult<()> {
        let mut reports = self.reports.write().await;
        let report = reports
            .get_mut(id)
            .ok_or_else(|| RepositoryError::not_found("DelayedReport", id.to_string()))?;
        report.mark_published_at(published_at);
        Ok(())
    }

    async fn delete(&self, id: &Uuid) -> RepositoryResult<bool> {
        let mut reports = self.reports.write().await;
        Ok(reports.remove(id).is_some())
    }

    async fn count(&self) -> RepositoryResult<u64> {
        let reports = self.reports.read().await;
        Ok(reports.len() as u64)
    }

    async fn count_pending(&self) -> RepositoryResult<u64> {
        let reports = self.reports.read().await;
        Ok(reports.values().filter(|r| !r.is_published()).count() as u64)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::domain::entities::delayed_report::TradeSummary;
    use crate::domain::services::ReportingTier;
    use crate::domain::value_objects::enums::{AssetClass, SettlementMethod};
    use crate::domain::value_objects::{Instrument, Price, Quantity, Symbol};

    fn create_test_report(trade_id: BlockTradeId, publish_secs_from_now: i64) -> DelayedReport {
        let symbol = Symbol::new("BTC/USD").unwrap();
        let instrument =
            Instrument::new(symbol, AssetClass::CryptoSpot, SettlementMethod::default());
        let summary = TradeSummary::new(
            instrument,
            Price::new(50000.0).unwrap(),
            Quantity::new(10.0).unwrap(),
            Timestamp::now(),
        );
        let publish_at = Timestamp::now().add_secs(publish_secs_from_now);
        DelayedReport::new(trade_id, ReportingTier::Standard, summary, publish_at)
    }

    #[tokio::test]
    async fn save_and_find_by_id() {
        let repo = InMemoryDelayedReportRepository::new();
        let trade_id = BlockTradeId::new_v4();
        let report = create_test_report(trade_id, 900);
        let id = report.id();

        repo.save(&report).await.unwrap();

        let found = repo.find_by_id(&id).await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().trade_id(), trade_id);
    }

    #[tokio::test]
    async fn find_by_trade_id() {
        let repo = InMemoryDelayedReportRepository::new();
        let trade_id = BlockTradeId::new_v4();
        let report = create_test_report(trade_id, 900);

        repo.save(&report).await.unwrap();

        let found = repo.find_by_trade_id(trade_id).await.unwrap();
        assert!(found.is_some());

        let not_found = repo.find_by_trade_id(BlockTradeId::new_v4()).await.unwrap();
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn find_pending() {
        let repo = InMemoryDelayedReportRepository::new();

        let trade1_id = BlockTradeId::new_v4();
        let report1 = create_test_report(trade1_id, 900);
        let mut report2 = create_test_report(BlockTradeId::new_v4(), 900);
        report2.mark_published();

        repo.save(&report1).await.unwrap();
        repo.save(&report2).await.unwrap();

        let pending = repo.find_pending().await.unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].trade_id(), trade1_id);
    }

    #[tokio::test]
    async fn find_ready_to_publish() {
        let repo = InMemoryDelayedReportRepository::new();

        let trade2_id = BlockTradeId::new_v4();
        let report_future = create_test_report(BlockTradeId::new_v4(), 900); // 15 min from now
        let report_past = create_test_report(trade2_id, -60); // 1 min ago

        repo.save(&report_future).await.unwrap();
        repo.save(&report_past).await.unwrap();

        let ready = repo.find_ready_to_publish(Timestamp::now()).await.unwrap();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].trade_id(), trade2_id);
    }

    #[tokio::test]
    async fn mark_published() {
        let repo = InMemoryDelayedReportRepository::new();
        let report = create_test_report(BlockTradeId::new_v4(), -60);
        let id = report.id();

        repo.save(&report).await.unwrap();

        let before = repo.find_by_id(&id).await.unwrap().unwrap();
        assert!(!before.is_published());

        repo.mark_published(&id, Timestamp::now()).await.unwrap();

        let after = repo.find_by_id(&id).await.unwrap().unwrap();
        assert!(after.is_published());
    }

    #[tokio::test]
    async fn count_and_count_pending() {
        let repo = InMemoryDelayedReportRepository::new();

        let report1 = create_test_report(BlockTradeId::new_v4(), 900);
        let mut report2 = create_test_report(BlockTradeId::new_v4(), 900);
        report2.mark_published();

        repo.save(&report1).await.unwrap();
        repo.save(&report2).await.unwrap();

        assert_eq!(repo.count().await.unwrap(), 2);
        assert_eq!(repo.count_pending().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn delete() {
        let repo = InMemoryDelayedReportRepository::new();
        let report = create_test_report(BlockTradeId::new_v4(), 900);
        let id = report.id();

        repo.save(&report).await.unwrap();
        assert_eq!(repo.count().await.unwrap(), 1);

        let deleted = repo.delete(&id).await.unwrap();
        assert!(deleted);
        assert_eq!(repo.count().await.unwrap(), 0);

        let not_deleted = repo.delete(&id).await.unwrap();
        assert!(!not_deleted);
    }
}
