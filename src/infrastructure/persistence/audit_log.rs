//! # Negotiation Audit Log Trait
//!
//! Port definition for negotiation audit log persistence.
//!
//! The audit log provides append-only semantics for negotiation events,
//! capturing all actions with μs precision for compliance purposes.
//!
//! # Examples
//!
//! ```ignore
//! use otc_rfq::infrastructure::persistence::audit_log::NegotiationAuditLog;
//! use otc_rfq::domain::audit::{AuditAction, NegotiationAuditEntry, ExportFormat};
//!
//! // Append an audit entry
//! let entry = NegotiationAuditEntry::new(rfq_id, AuditAction::QuoteSent, actor);
//! audit_log.append_audit(entry).await?;
//!
//! // Retrieve audit trail
//! let trail = audit_log.get_audit_trail(rfq_id).await?;
//!
//! // Export compliance report
//! let csv = audit_log.export_compliance_report(rfq_id, ExportFormat::Csv).await?;
//! ```

use crate::domain::audit::{ExportFormat, NegotiationAuditEntry};
use crate::domain::value_objects::RfqId;
use crate::infrastructure::persistence::traits::RepositoryError;
use async_trait::async_trait;
use std::fmt;

/// Result type for audit log operations.
pub type AuditLogResult<T> = Result<T, RepositoryError>;

/// Trait for negotiation audit log persistence.
///
/// The audit log provides append-only semantics - entries can only be
/// added, never modified or deleted. This ensures a complete audit trail
/// for compliance and regulatory purposes.
///
/// # Thread Safety
///
/// Implementations must be `Send + Sync` for use in async contexts.
///
/// # Ordering
///
/// Entries are ordered by their `timestamp_us` field, providing μs-precision
/// ordering for compliance requirements.
#[async_trait]
pub trait NegotiationAuditLog: Send + Sync + fmt::Debug {
    /// Appends an audit entry to the log.
    ///
    /// # Arguments
    ///
    /// * `entry` - The audit entry to append
    ///
    /// # Errors
    ///
    /// Returns an error if the entry cannot be stored.
    async fn append_audit(&self, entry: NegotiationAuditEntry) -> AuditLogResult<()>;

    /// Retrieves all audit entries for an RFQ.
    ///
    /// Entries are returned in timestamp order (oldest first).
    ///
    /// # Arguments
    ///
    /// * `rfq_id` - The RFQ to get audit entries for
    ///
    /// # Errors
    ///
    /// Returns an error if entries cannot be retrieved.
    async fn get_audit_trail(&self, rfq_id: RfqId) -> AuditLogResult<Vec<NegotiationAuditEntry>>;

    /// Exports a compliance report for an RFQ.
    ///
    /// Generates a formatted report containing all audit entries for the
    /// specified RFQ in the requested format.
    ///
    /// # Arguments
    ///
    /// * `rfq_id` - The RFQ to generate a report for
    /// * `format` - The export format (CSV or JSON)
    ///
    /// # Returns
    ///
    /// The report as a byte vector in the requested format.
    ///
    /// # Errors
    ///
    /// Returns an error if the report cannot be generated.
    async fn export_compliance_report(
        &self,
        rfq_id: RfqId,
        format: ExportFormat,
    ) -> AuditLogResult<Vec<u8>>;

    /// Returns the total number of audit entries.
    ///
    /// # Errors
    ///
    /// Returns an error if the count cannot be retrieved.
    async fn count(&self) -> AuditLogResult<u64>;

    /// Returns the number of audit entries for an RFQ.
    ///
    /// # Arguments
    ///
    /// * `rfq_id` - The RFQ to count entries for
    ///
    /// # Errors
    ///
    /// Returns an error if the count cannot be retrieved.
    async fn count_for_rfq(&self, rfq_id: RfqId) -> AuditLogResult<u64>;
}
