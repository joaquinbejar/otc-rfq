//! # Domain Event Trait
//!
//! Base trait for all domain events.
//!
//! This module provides the [`DomainEvent`] trait that all domain events
//! must implement, along with common event metadata.
//!
//! # Examples
//!
//! ```
//! use otc_rfq::domain::events::domain_event::{DomainEvent, EventType};
//! use otc_rfq::domain::value_objects::{EventId, RfqId, Timestamp};
//!
//! // Events implement the DomainEvent trait
//! // See rfq_events, trade_events, and compliance_events for concrete implementations
//! ```

use crate::domain::schema::SchemaVersion;
use crate::domain::value_objects::timestamp::Timestamp;
use crate::domain::value_objects::{EventId, RfqId};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Type of domain event.
///
/// Categorizes events by their domain area.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EventType {
    /// RFQ lifecycle events.
    Rfq,
    /// Quote-related events.
    Quote,
    /// Trade execution events.
    Trade,
    /// Settlement events.
    Settlement,
    /// Compliance events.
    Compliance,
    /// Capacity management events.
    Capacity,
}

impl fmt::Display for EventType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rfq => write!(f, "RFQ"),
            Self::Quote => write!(f, "QUOTE"),
            Self::Trade => write!(f, "TRADE"),
            Self::Settlement => write!(f, "SETTLEMENT"),
            Self::Compliance => write!(f, "COMPLIANCE"),
            Self::Capacity => write!(f, "CAPACITY"),
        }
    }
}

/// Trait for all domain events.
///
/// Domain events represent significant occurrences in the domain that
/// other parts of the system may need to react to. They are immutable
/// records of what happened.
///
/// # Required Methods
///
/// - [`event_id`](DomainEvent::event_id) - Unique identifier for this event
/// - [`rfq_id`](DomainEvent::rfq_id) - The RFQ this event relates to (if any)
/// - [`timestamp`](DomainEvent::timestamp) - When the event occurred
/// - [`event_type`](DomainEvent::event_type) - Category of the event
/// - [`event_name`](DomainEvent::event_name) - Human-readable event name
pub trait DomainEvent: Send + Sync + fmt::Debug {
    /// Returns the unique identifier for this event.
    fn event_id(&self) -> EventId;

    /// Returns the RFQ ID this event relates to, if any.
    fn rfq_id(&self) -> Option<RfqId>;

    /// Returns when this event occurred.
    fn timestamp(&self) -> Timestamp;

    /// Returns the type/category of this event.
    fn event_type(&self) -> EventType;

    /// Returns the human-readable name of this event.
    fn event_name(&self) -> &'static str;
}

/// Common metadata for all domain events.
///
/// This struct contains the fields common to all events and can be
/// embedded in concrete event types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventMetadata {
    /// Unique identifier for this event.
    pub event_id: EventId,
    /// The RFQ this event relates to.
    pub rfq_id: Option<RfqId>,
    /// When this event occurred.
    pub timestamp: Timestamp,
    /// Schema version for this event.
    #[serde(default)]
    pub schema_version: SchemaVersion,
}

impl EventMetadata {
    /// Creates new event metadata with a generated event ID.
    #[must_use]
    pub fn new(rfq_id: Option<RfqId>) -> Self {
        Self {
            event_id: EventId::new_v4(),
            rfq_id,
            timestamp: Timestamp::now(),
            schema_version: SchemaVersion::V1_0_0,
        }
    }

    /// Creates new event metadata for a specific RFQ.
    #[must_use]
    pub fn for_rfq(rfq_id: RfqId) -> Self {
        Self::new(Some(rfq_id))
    }

    /// Creates event metadata with specific values (for reconstruction).
    #[must_use]
    pub fn from_parts(event_id: EventId, rfq_id: Option<RfqId>, timestamp: Timestamp) -> Self {
        Self {
            event_id,
            rfq_id,
            timestamp,
            schema_version: SchemaVersion::V1_0_0,
        }
    }
}

impl Default for EventMetadata {
    fn default() -> Self {
        Self::new(None)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn event_type_display() {
        assert_eq!(EventType::Rfq.to_string(), "RFQ");
        assert_eq!(EventType::Quote.to_string(), "QUOTE");
        assert_eq!(EventType::Trade.to_string(), "TRADE");
        assert_eq!(EventType::Settlement.to_string(), "SETTLEMENT");
        assert_eq!(EventType::Compliance.to_string(), "COMPLIANCE");
    }

    #[test]
    fn event_metadata_new() {
        let metadata = EventMetadata::new(None);
        assert!(metadata.rfq_id.is_none());
    }

    #[test]
    fn event_metadata_for_rfq() {
        let rfq_id = RfqId::new_v4();
        let metadata = EventMetadata::for_rfq(rfq_id);
        assert_eq!(metadata.rfq_id, Some(rfq_id));
    }

    #[test]
    fn event_metadata_serde_roundtrip() {
        let metadata = EventMetadata::new(Some(RfqId::new_v4()));
        let json = serde_json::to_string(&metadata).unwrap();
        let deserialized: EventMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(metadata.event_id, deserialized.event_id);
        assert_eq!(metadata.rfq_id, deserialized.rfq_id);
    }
}
