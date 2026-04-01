//! # Audit Module
//!
//! Domain types for negotiation audit logging with μs precision.
//!
//! This module provides the core types for capturing and exporting
//! negotiation audit trails for compliance purposes.
//!
//! # Components
//!
//! - [`AuditAction`]: Types of actions that can be recorded
//! - [`ExportFormat`]: Supported export formats (CSV, JSON)
//! - [`NegotiationAuditEntry`]: Individual audit log entries
//! - [`NegotiationAuditEntryBuilder`]: Builder for creating entries
//!
//! # Example
//!
//! ```
//! use otc_rfq::domain::audit::{AuditAction, NegotiationAuditEntry, ExportFormat};
//! use otc_rfq::domain::value_objects::{CounterpartyId, RfqId};
//!
//! let entry = NegotiationAuditEntry::new(
//!     RfqId::new_v4(),
//!     AuditAction::QuoteSent,
//!     CounterpartyId::new("mm-1"),
//! );
//!
//! assert!(entry.timestamp_us() > 0);
//! assert_eq!(entry.action(), AuditAction::QuoteSent);
//! ```

pub mod audit_action;
pub mod export_format;
pub mod negotiation_audit_entry;

pub use audit_action::AuditAction;
pub use export_format::ExportFormat;
pub use negotiation_audit_entry::{NegotiationAuditEntry, NegotiationAuditEntryBuilder};
