//! # Negotiation Audit Entry
//!
//! Defines the audit entry structure for negotiation events with μs precision.

use crate::domain::audit::audit_action::AuditAction;
use crate::domain::value_objects::timestamp::Timestamp;
use crate::domain::value_objects::{CounterpartyId, NegotiationId, Price, Quantity, RfqId};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// A single audit entry capturing a negotiation action with μs precision.
///
/// Each entry represents a significant event in the negotiation lifecycle,
/// capturing all relevant details for compliance and audit purposes.
///
/// # Timestamp Precision
///
/// The `timestamp_us` field stores microseconds since Unix epoch, providing
/// the precision required for regulatory compliance and event ordering.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NegotiationAuditEntry {
    /// Unique identifier for this audit entry.
    pub entry_id: Uuid,
    /// The RFQ this entry relates to.
    pub rfq_id: RfqId,
    /// The negotiation ID, if applicable.
    pub negotiation_id: Option<NegotiationId>,
    /// The action being recorded.
    pub action: AuditAction,
    /// The actor performing the action.
    pub actor: CounterpartyId,
    /// The price involved, if applicable.
    pub price: Option<Price>,
    /// The quantity involved, if applicable.
    pub quantity: Option<Quantity>,
    /// The negotiation round, if applicable.
    pub round: Option<u8>,
    /// Additional details or context.
    pub details: Option<String>,
    /// Microseconds since Unix epoch (μs precision).
    pub timestamp_us: i64,
    /// When this entry was created (ms precision for storage).
    pub created_at: Timestamp,
}

impl NegotiationAuditEntry {
    /// Creates a new audit entry with the current timestamp.
    #[must_use]
    pub fn new(rfq_id: RfqId, action: AuditAction, actor: CounterpartyId) -> Self {
        let timestamp_us = Self::current_timestamp_us();
        Self {
            entry_id: Uuid::new_v4(),
            rfq_id,
            negotiation_id: None,
            action,
            actor,
            price: None,
            quantity: None,
            round: None,
            details: None,
            timestamp_us,
            created_at: Timestamp::now(),
        }
    }

    /// Returns the current timestamp in microseconds since Unix epoch.
    #[must_use]
    fn current_timestamp_us() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| {
                let micros = d.as_micros();
                if micros > i64::MAX as u128 {
                    i64::MAX
                } else {
                    micros as i64
                }
            })
            .unwrap_or(0)
    }

    /// Sets the negotiation ID.
    #[must_use]
    pub fn with_negotiation_id(mut self, negotiation_id: NegotiationId) -> Self {
        self.negotiation_id = Some(negotiation_id);
        self
    }

    /// Sets the price.
    #[must_use]
    pub fn with_price(mut self, price: Price) -> Self {
        self.price = Some(price);
        self
    }

    /// Sets the quantity.
    #[must_use]
    pub fn with_quantity(mut self, quantity: Quantity) -> Self {
        self.quantity = Some(quantity);
        self
    }

    /// Sets the negotiation round.
    #[must_use]
    pub fn with_round(mut self, round: u8) -> Self {
        self.round = Some(round);
        self
    }

    /// Sets additional details.
    #[must_use]
    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into());
        self
    }

    /// Returns the entry ID.
    #[inline]
    #[must_use]
    pub fn entry_id(&self) -> Uuid {
        self.entry_id
    }

    /// Returns the RFQ ID.
    #[inline]
    #[must_use]
    pub fn rfq_id(&self) -> RfqId {
        self.rfq_id
    }

    /// Returns the negotiation ID.
    #[inline]
    #[must_use]
    pub fn negotiation_id(&self) -> Option<NegotiationId> {
        self.negotiation_id
    }

    /// Returns the action.
    #[inline]
    #[must_use]
    pub fn action(&self) -> AuditAction {
        self.action
    }

    /// Returns the actor.
    #[inline]
    #[must_use]
    pub fn actor(&self) -> &CounterpartyId {
        &self.actor
    }

    /// Returns the price.
    #[inline]
    #[must_use]
    pub fn price(&self) -> Option<Price> {
        self.price
    }

    /// Returns the quantity.
    #[inline]
    #[must_use]
    pub fn quantity(&self) -> Option<Quantity> {
        self.quantity
    }

    /// Returns the round.
    #[inline]
    #[must_use]
    pub fn round(&self) -> Option<u8> {
        self.round
    }

    /// Returns the details.
    #[inline]
    #[must_use]
    pub fn details(&self) -> Option<&str> {
        self.details.as_deref()
    }

    /// Returns the timestamp in microseconds.
    #[inline]
    #[must_use]
    pub fn timestamp_us(&self) -> i64 {
        self.timestamp_us
    }

    /// Returns when this entry was created.
    #[inline]
    #[must_use]
    pub fn created_at(&self) -> Timestamp {
        self.created_at
    }

    /// Returns the timestamp as a formatted ISO 8601 string with μs precision.
    #[must_use]
    pub fn timestamp_iso8601(&self) -> String {
        let secs = self.timestamp_us / 1_000_000;
        let micros = (self.timestamp_us % 1_000_000).unsigned_abs();
        let datetime =
            chrono::DateTime::<chrono::Utc>::from_timestamp(secs, (micros * 1000) as u32)
                .unwrap_or(chrono::DateTime::<chrono::Utc>::UNIX_EPOCH);
        datetime.format("%Y-%m-%dT%H:%M:%S%.6fZ").to_string()
    }
}

impl fmt::Display for NegotiationAuditEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}] {} by {} on RFQ {}",
            self.timestamp_iso8601(),
            self.action,
            self.actor,
            self.rfq_id
        )
    }
}

/// Builder for creating audit entries with all optional fields.
#[derive(Debug, Clone)]
pub struct NegotiationAuditEntryBuilder {
    rfq_id: RfqId,
    action: AuditAction,
    actor: CounterpartyId,
    negotiation_id: Option<NegotiationId>,
    price: Option<Price>,
    quantity: Option<Quantity>,
    round: Option<u8>,
    details: Option<String>,
}

impl NegotiationAuditEntryBuilder {
    /// Creates a new builder with required fields.
    #[must_use]
    pub fn new(rfq_id: RfqId, action: AuditAction, actor: CounterpartyId) -> Self {
        Self {
            rfq_id,
            action,
            actor,
            negotiation_id: None,
            price: None,
            quantity: None,
            round: None,
            details: None,
        }
    }

    /// Sets the negotiation ID.
    #[must_use]
    pub fn negotiation_id(mut self, id: NegotiationId) -> Self {
        self.negotiation_id = Some(id);
        self
    }

    /// Sets the price.
    #[must_use]
    pub fn price(mut self, price: Price) -> Self {
        self.price = Some(price);
        self
    }

    /// Sets the quantity.
    #[must_use]
    pub fn quantity(mut self, quantity: Quantity) -> Self {
        self.quantity = Some(quantity);
        self
    }

    /// Sets the round.
    #[must_use]
    pub fn round(mut self, round: u8) -> Self {
        self.round = Some(round);
        self
    }

    /// Sets additional details.
    #[must_use]
    pub fn details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into());
        self
    }

    /// Builds the audit entry.
    #[must_use]
    pub fn build(self) -> NegotiationAuditEntry {
        let mut entry = NegotiationAuditEntry::new(self.rfq_id, self.action, self.actor);
        entry.negotiation_id = self.negotiation_id;
        entry.price = self.price;
        entry.quantity = self.quantity;
        entry.round = self.round;
        entry.details = self.details;
        entry
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn test_rfq_id() -> RfqId {
        RfqId::new_v4()
    }

    fn test_actor() -> CounterpartyId {
        CounterpartyId::new("test-actor")
    }

    #[test]
    fn new_creates_entry_with_timestamp() {
        let entry = NegotiationAuditEntry::new(test_rfq_id(), AuditAction::QuoteSent, test_actor());

        assert!(entry.timestamp_us > 0);
        assert_eq!(entry.action, AuditAction::QuoteSent);
        assert!(entry.negotiation_id.is_none());
        assert!(entry.price.is_none());
        assert!(entry.quantity.is_none());
        assert!(entry.round.is_none());
        assert!(entry.details.is_none());
    }

    #[test]
    fn builder_creates_complete_entry() {
        let rfq_id = test_rfq_id();
        let neg_id = NegotiationId::new_v4();
        let price = Price::new(50000.0).unwrap();
        let quantity = Quantity::new(1.0).unwrap();

        let entry =
            NegotiationAuditEntryBuilder::new(rfq_id, AuditAction::CounterSent, test_actor())
                .negotiation_id(neg_id)
                .price(price)
                .quantity(quantity)
                .round(2)
                .details("Test counter-quote")
                .build();

        assert_eq!(entry.rfq_id, rfq_id);
        assert_eq!(entry.negotiation_id, Some(neg_id));
        assert_eq!(entry.price, Some(price));
        assert_eq!(entry.quantity, Some(quantity));
        assert_eq!(entry.round, Some(2));
        assert_eq!(entry.details, Some("Test counter-quote".to_string()));
    }

    #[test]
    fn with_methods_chain_correctly() {
        let neg_id = NegotiationId::new_v4();
        let price = Price::new(49500.0).unwrap();

        let entry = NegotiationAuditEntry::new(test_rfq_id(), AuditAction::Accepted, test_actor())
            .with_negotiation_id(neg_id)
            .with_price(price)
            .with_round(3)
            .with_details("Final acceptance");

        assert_eq!(entry.negotiation_id, Some(neg_id));
        assert_eq!(entry.price, Some(price));
        assert_eq!(entry.round, Some(3));
        assert_eq!(entry.details, Some("Final acceptance".to_string()));
    }

    #[test]
    fn timestamp_iso8601_formats_correctly() {
        let entry = NegotiationAuditEntry::new(test_rfq_id(), AuditAction::QuoteSent, test_actor());
        let iso = entry.timestamp_iso8601();

        // Should be in format: 2024-01-15T10:30:45.123456Z
        assert!(iso.contains('T'));
        assert!(iso.ends_with('Z'));
        assert!(iso.len() > 20); // Has microseconds
    }

    #[test]
    fn display_formats_correctly() {
        let entry = NegotiationAuditEntry::new(test_rfq_id(), AuditAction::QuoteSent, test_actor());
        let display = entry.to_string();

        assert!(display.contains("QUOTE_SENT"));
        assert!(display.contains("test-actor"));
    }

    #[test]
    fn serde_roundtrip() {
        let entry = NegotiationAuditEntryBuilder::new(
            test_rfq_id(),
            AuditAction::CounterReceived,
            test_actor(),
        )
        .negotiation_id(NegotiationId::new_v4())
        .price(Price::new(50000.0).unwrap())
        .round(1)
        .build();

        let json = serde_json::to_string(&entry).unwrap();
        let deserialized: NegotiationAuditEntry = serde_json::from_str(&json).unwrap();

        assert_eq!(entry.entry_id, deserialized.entry_id);
        assert_eq!(entry.rfq_id, deserialized.rfq_id);
        assert_eq!(entry.action, deserialized.action);
        assert_eq!(entry.timestamp_us, deserialized.timestamp_us);
    }

    #[test]
    fn timestamp_us_has_microsecond_precision() {
        let entry1 =
            NegotiationAuditEntry::new(test_rfq_id(), AuditAction::QuoteSent, test_actor());
        std::thread::sleep(std::time::Duration::from_micros(10));
        let entry2 =
            NegotiationAuditEntry::new(test_rfq_id(), AuditAction::QuoteSent, test_actor());

        // Timestamps should be different (with μs precision)
        assert!(entry2.timestamp_us >= entry1.timestamp_us);
    }

    #[test]
    fn accessors_return_correct_values() {
        let rfq_id = test_rfq_id();
        let actor = test_actor();
        let entry = NegotiationAuditEntry::new(rfq_id, AuditAction::Rejected, actor.clone());

        assert_eq!(entry.rfq_id(), rfq_id);
        assert_eq!(entry.action(), AuditAction::Rejected);
        assert_eq!(entry.actor(), &actor);
        assert!(entry.timestamp_us() > 0);
    }
}
