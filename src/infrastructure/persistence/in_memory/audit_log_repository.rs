//! # In-Memory Negotiation Audit Log
//!
//! In-memory implementation of [`NegotiationAuditLog`] for testing.
//!
//! This implementation stores audit entries in a `Mutex<Vec<...>>` for thread-safe access.
//! Callers can wrap an instance in `Arc<InMemoryNegotiationAuditLog>` when shared ownership is required.
//!
//! # Examples
//!
//! ```
//! use otc_rfq::infrastructure::persistence::in_memory::InMemoryNegotiationAuditLog;
//! use otc_rfq::infrastructure::persistence::NegotiationAuditLog;
//! use otc_rfq::domain::audit::{AuditAction, NegotiationAuditEntry, ExportFormat};
//! use otc_rfq::domain::value_objects::{CounterpartyId, RfqId};
//!
//! # tokio_test::block_on(async {
//! let audit_log = InMemoryNegotiationAuditLog::new();
//! let rfq_id = RfqId::new_v4();
//!
//! let entry = NegotiationAuditEntry::new(
//!     rfq_id,
//!     AuditAction::QuoteSent,
//!     CounterpartyId::new("mm-1"),
//! );
//!
//! audit_log.append_audit(entry).await.unwrap();
//!
//! let trail = audit_log.get_audit_trail(rfq_id).await.unwrap();
//! assert_eq!(trail.len(), 1);
//! # });
//! ```

use crate::domain::audit::{ExportFormat, NegotiationAuditEntry};
use crate::domain::value_objects::RfqId;
use crate::infrastructure::persistence::audit_log::{AuditLogResult, NegotiationAuditLog};
use crate::infrastructure::persistence::traits::RepositoryError;
use async_trait::async_trait;
use std::sync::Mutex;

/// In-memory implementation of [`NegotiationAuditLog`].
///
/// Thread-safe for concurrent access within a single process.
/// Suitable for development, testing, and single-instance deployments.
#[derive(Debug)]
pub struct InMemoryNegotiationAuditLog {
    /// Audit entries stored in memory.
    entries: Mutex<Vec<NegotiationAuditEntry>>,
}

impl InMemoryNegotiationAuditLog {
    /// Creates a new in-memory audit log.
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: Mutex::new(Vec::new()),
        }
    }

    /// Returns the number of entries in the log.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.lock().map(|e| e.len()).unwrap_or(0)
    }

    /// Returns true if the log is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Clears all entries from the log.
    ///
    /// This method is only available in test builds to maintain the append-only
    /// contract specified by the [`NegotiationAuditLog`] trait.
    #[cfg(test)]
    pub fn clear(&self) {
        if let Ok(mut entries) = self.entries.lock() {
            entries.clear();
        }
    }

    /// Generates a CSV report from the given entries.
    fn generate_csv(entries: &[NegotiationAuditEntry]) -> Vec<u8> {
        let mut csv = String::new();

        // Header
        csv.push_str("entry_id,rfq_id,negotiation_id,action,actor,price,quantity,round,details,timestamp_us,timestamp_iso8601,created_at\n");

        // Rows
        for entry in entries {
            let neg_id = entry
                .negotiation_id
                .map(|id| id.to_string())
                .unwrap_or_default();
            let price = entry.price.map(|p| p.to_string()).unwrap_or_default();
            let quantity = entry.quantity.map(|q| q.to_string()).unwrap_or_default();
            let round = entry.round.map(|r| r.to_string()).unwrap_or_default();
            let details = entry
                .details
                .as_deref()
                .unwrap_or_default()
                .replace('"', "\"\"");

            csv.push_str(&format!(
                "{},{},{},{},{},{},{},{},\"{}\",{},{},{}\n",
                entry.entry_id,
                entry.rfq_id,
                neg_id,
                entry.action,
                entry.actor,
                price,
                quantity,
                round,
                details,
                entry.timestamp_us,
                entry.timestamp_iso8601(),
                entry.created_at,
            ));
        }

        csv.into_bytes()
    }

    /// Generates a JSON report from the given entries.
    fn generate_json(entries: &[NegotiationAuditEntry]) -> Result<Vec<u8>, RepositoryError> {
        serde_json::to_vec_pretty(entries)
            .map_err(|e| RepositoryError::Serialization(e.to_string()))
    }
}

impl Default for InMemoryNegotiationAuditLog {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl NegotiationAuditLog for InMemoryNegotiationAuditLog {
    async fn append_audit(&self, entry: NegotiationAuditEntry) -> AuditLogResult<()> {
        let mut entries = self
            .entries
            .lock()
            .map_err(|_| RepositoryError::Connection("Mutex poisoned".to_string()))?;

        entries.push(entry);
        Ok(())
    }

    async fn get_audit_trail(&self, rfq_id: RfqId) -> AuditLogResult<Vec<NegotiationAuditEntry>> {
        let entries = self
            .entries
            .lock()
            .map_err(|_| RepositoryError::Connection("Mutex poisoned".to_string()))?;

        let mut trail: Vec<_> = entries
            .iter()
            .filter(|e| e.rfq_id == rfq_id)
            .cloned()
            .collect();

        // Sort by timestamp_us for proper ordering
        trail.sort_by_key(|e| e.timestamp_us);

        Ok(trail)
    }

    async fn export_compliance_report(
        &self,
        rfq_id: RfqId,
        format: ExportFormat,
    ) -> AuditLogResult<Vec<u8>> {
        let trail = self.get_audit_trail(rfq_id).await?;

        match format {
            ExportFormat::Csv => Ok(Self::generate_csv(&trail)),
            ExportFormat::Json => Self::generate_json(&trail),
        }
    }

    async fn count(&self) -> AuditLogResult<u64> {
        let entries = self
            .entries
            .lock()
            .map_err(|_| RepositoryError::Connection("Mutex poisoned".to_string()))?;

        Ok(entries.len() as u64)
    }

    async fn count_for_rfq(&self, rfq_id: RfqId) -> AuditLogResult<u64> {
        let entries = self
            .entries
            .lock()
            .map_err(|_| RepositoryError::Connection("Mutex poisoned".to_string()))?;

        let count = entries.iter().filter(|e| e.rfq_id == rfq_id).count();
        Ok(count as u64)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::domain::audit::AuditAction;
    use crate::domain::value_objects::{CounterpartyId, NegotiationId, Price};

    fn test_rfq_id() -> RfqId {
        RfqId::new_v4()
    }

    fn test_actor() -> CounterpartyId {
        CounterpartyId::new("test-actor")
    }

    fn create_entry(rfq_id: RfqId, action: AuditAction) -> NegotiationAuditEntry {
        NegotiationAuditEntry::new(rfq_id, action, test_actor())
    }

    #[tokio::test]
    async fn append_and_retrieve() {
        let log = InMemoryNegotiationAuditLog::new();
        let rfq_id = test_rfq_id();

        let entry = create_entry(rfq_id, AuditAction::QuoteSent);
        log.append_audit(entry).await.unwrap();

        let trail = log.get_audit_trail(rfq_id).await.unwrap();
        assert_eq!(trail.len(), 1);
        assert!(matches!(trail.first(), Some(e) if e.action == AuditAction::QuoteSent));
    }

    #[tokio::test]
    async fn entries_ordered_by_timestamp() {
        let log = InMemoryNegotiationAuditLog::new();
        let rfq_id = test_rfq_id();

        // Create entries with explicit out-of-order timestamps to test sorting
        let mut entry1 = create_entry(rfq_id, AuditAction::QuoteSent);
        entry1.timestamp_us = 3000; // Third chronologically

        let mut entry2 = create_entry(rfq_id, AuditAction::CounterReceived);
        entry2.timestamp_us = 1000; // First chronologically

        let mut entry3 = create_entry(rfq_id, AuditAction::Accepted);
        entry3.timestamp_us = 2000; // Second chronologically

        // Add in non-chronological order
        log.append_audit(entry1).await.unwrap();
        log.append_audit(entry2).await.unwrap();
        log.append_audit(entry3).await.unwrap();

        let trail = log.get_audit_trail(rfq_id).await.unwrap();
        assert_eq!(trail.len(), 3);

        // Verify entries are sorted by timestamp_us using pattern matching
        let timestamps: Vec<i64> = trail.iter().map(|e| e.timestamp_us).collect();
        assert_eq!(timestamps, vec![1000, 2000, 3000]);

        // Verify actions match expected order after sorting
        let actions: Vec<AuditAction> = trail.iter().map(|e| e.action).collect();
        assert_eq!(
            actions,
            vec![
                AuditAction::CounterReceived,
                AuditAction::Accepted,
                AuditAction::QuoteSent
            ]
        );
    }

    #[tokio::test]
    async fn filter_by_rfq_id() {
        let log = InMemoryNegotiationAuditLog::new();
        let rfq_id1 = test_rfq_id();
        let rfq_id2 = test_rfq_id();

        log.append_audit(create_entry(rfq_id1, AuditAction::QuoteSent))
            .await
            .unwrap();
        log.append_audit(create_entry(rfq_id1, AuditAction::Accepted))
            .await
            .unwrap();
        log.append_audit(create_entry(rfq_id2, AuditAction::QuoteSent))
            .await
            .unwrap();

        let trail1 = log.get_audit_trail(rfq_id1).await.unwrap();
        let trail2 = log.get_audit_trail(rfq_id2).await.unwrap();

        assert_eq!(trail1.len(), 2);
        assert_eq!(trail2.len(), 1);
    }

    #[tokio::test]
    async fn count_operations() {
        let log = InMemoryNegotiationAuditLog::new();
        let rfq_id = test_rfq_id();

        assert_eq!(log.count().await.unwrap(), 0);
        assert_eq!(log.count_for_rfq(rfq_id).await.unwrap(), 0);

        log.append_audit(create_entry(rfq_id, AuditAction::QuoteSent))
            .await
            .unwrap();
        log.append_audit(create_entry(rfq_id, AuditAction::Accepted))
            .await
            .unwrap();
        log.append_audit(create_entry(test_rfq_id(), AuditAction::Rejected))
            .await
            .unwrap();

        assert_eq!(log.count().await.unwrap(), 3);
        assert_eq!(log.count_for_rfq(rfq_id).await.unwrap(), 2);
    }

    #[tokio::test]
    async fn export_csv() {
        let log = InMemoryNegotiationAuditLog::new();
        let rfq_id = test_rfq_id();

        let entry = NegotiationAuditEntry::new(rfq_id, AuditAction::QuoteSent, test_actor())
            .with_negotiation_id(NegotiationId::new_v4())
            .with_price(Price::new(50000.0).unwrap())
            .with_round(1)
            .with_details("Test quote");

        log.append_audit(entry).await.unwrap();

        let csv = log
            .export_compliance_report(rfq_id, ExportFormat::Csv)
            .await
            .unwrap();
        let csv_str = String::from_utf8(csv).unwrap();

        assert!(csv_str.contains("entry_id,rfq_id,negotiation_id"));
        assert!(csv_str.contains("QUOTE_SENT"));
        assert!(csv_str.contains("test-actor"));
        assert!(csv_str.contains("50000"));
    }

    #[tokio::test]
    async fn export_json() {
        let log = InMemoryNegotiationAuditLog::new();
        let rfq_id = test_rfq_id();

        let entry =
            NegotiationAuditEntry::new(rfq_id, AuditAction::Accepted, test_actor()).with_round(3);

        log.append_audit(entry).await.unwrap();

        let json = log
            .export_compliance_report(rfq_id, ExportFormat::Json)
            .await
            .unwrap();
        let json_str = String::from_utf8(json).unwrap();

        assert!(json_str.contains("\"action\": \"ACCEPTED\""));
        assert!(json_str.contains("\"round\": 3"));
    }

    #[tokio::test]
    async fn empty_trail_returns_empty_vec() {
        let log = InMemoryNegotiationAuditLog::new();
        let rfq_id = test_rfq_id();

        let trail = log.get_audit_trail(rfq_id).await.unwrap();
        assert!(trail.is_empty());
    }

    #[tokio::test]
    async fn clear_removes_all_entries() {
        let log = InMemoryNegotiationAuditLog::new();
        let rfq_id = test_rfq_id();

        log.append_audit(create_entry(rfq_id, AuditAction::QuoteSent))
            .await
            .unwrap();
        assert!(!log.is_empty());

        log.clear();
        assert!(log.is_empty());
    }

    #[test]
    fn default_creates_empty_log() {
        let log = InMemoryNegotiationAuditLog::default();
        assert!(log.is_empty());
    }

    #[test]
    fn len_and_is_empty() {
        let log = InMemoryNegotiationAuditLog::new();
        assert!(log.is_empty());
        assert_eq!(log.len(), 0);
    }
}
