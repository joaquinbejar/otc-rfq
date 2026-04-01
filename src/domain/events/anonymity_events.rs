//! # Anonymity Events
//!
//! Domain events for anonymous RFQ lifecycle.
//!
//! This module provides events that track anonymity-related actions:
//! - Anonymous RFQ broadcast to market makers
//! - Identity reveal post-trade for settlement
//!
//! # Event Flow
//!
//! ```text
//! RfqCreated (anonymous=true)
//!     ↓
//! AnonymousRfqBroadcast (to MMs, no client_id)
//!     ↓
//! [Quote collection, selection, execution]
//!     ↓
//! IdentityRevealed (to winning MM for settlement)
//! ```

use crate::domain::entities::anonymity::{AnonymityLevel, AnonymousRfqView};
use crate::domain::events::domain_event::{DomainEvent, EventMetadata, EventType};
use crate::domain::value_objects::timestamp::Timestamp;
use crate::domain::value_objects::{
    CounterpartyId, EventId, Instrument, OrderSide, Quantity, RfqId, VenueId,
};
use serde::{Deserialize, Serialize};

/// Event emitted when an anonymous RFQ is broadcast to market makers.
///
/// This event contains the anonymized view of the RFQ, without the
/// requester's identity. It is used for audit purposes and to track
/// which venues received the anonymous RFQ.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnonymousRfqBroadcast {
    /// Event metadata.
    pub metadata: EventMetadata,
    /// The instrument being traded.
    pub instrument: Instrument,
    /// Buy or sell.
    pub side: OrderSide,
    /// Requested quantity.
    pub quantity: Quantity,
    /// When the RFQ expires.
    pub expires_at: Timestamp,
    /// The anonymity level applied.
    pub anonymity_level: AnonymityLevel,
    /// Venues that received the broadcast.
    pub venue_ids: Vec<VenueId>,
}

impl AnonymousRfqBroadcast {
    /// Creates a new AnonymousRfqBroadcast event.
    #[must_use]
    pub fn new(view: &AnonymousRfqView, venue_ids: Vec<VenueId>) -> Self {
        Self {
            metadata: EventMetadata::for_rfq(view.rfq_id()),
            instrument: view.instrument().clone(),
            side: view.side(),
            quantity: view.quantity(),
            expires_at: view.expires_at(),
            anonymity_level: view.anonymity_level(),
            venue_ids,
        }
    }

    /// Creates from individual fields.
    #[must_use]
    pub fn from_parts(
        rfq_id: RfqId,
        instrument: Instrument,
        side: OrderSide,
        quantity: Quantity,
        expires_at: Timestamp,
        anonymity_level: AnonymityLevel,
        venue_ids: Vec<VenueId>,
    ) -> Self {
        Self {
            metadata: EventMetadata::for_rfq(rfq_id),
            instrument,
            side,
            quantity,
            expires_at,
            anonymity_level,
            venue_ids,
        }
    }
}

impl DomainEvent for AnonymousRfqBroadcast {
    fn event_id(&self) -> EventId {
        self.metadata.event_id
    }

    fn rfq_id(&self) -> Option<RfqId> {
        self.metadata.rfq_id
    }

    fn timestamp(&self) -> Timestamp {
        self.metadata.timestamp
    }

    fn event_type(&self) -> EventType {
        EventType::Rfq
    }

    fn event_name(&self) -> &'static str {
        "AnonymousRfqBroadcast"
    }
}

/// Event emitted when a requester's identity is revealed post-trade.
///
/// This event is emitted when the anonymity service reveals the
/// requester's identity to a counterparty (typically the winning
/// market maker) for settlement purposes.
///
/// # Compliance
///
/// This event is critical for audit trails. It records:
/// - Who the identity was revealed to
/// - When the reveal occurred
/// - The actual requester identity
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentityRevealed {
    /// Event metadata.
    pub metadata: EventMetadata,
    /// The counterparty to whom identity was revealed.
    pub revealed_to: CounterpartyId,
    /// The requester's actual identity.
    pub requester_id: CounterpartyId,
    /// The anonymity level that was in effect.
    pub anonymity_level: AnonymityLevel,
    /// Reason for the reveal (e.g., "settlement", "compliance").
    pub reason: String,
}

impl IdentityRevealed {
    /// Creates a new IdentityRevealed event.
    #[must_use]
    pub fn new(
        rfq_id: RfqId,
        revealed_to: CounterpartyId,
        requester_id: CounterpartyId,
        anonymity_level: AnonymityLevel,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            metadata: EventMetadata::for_rfq(rfq_id),
            revealed_to,
            requester_id,
            anonymity_level,
            reason: reason.into(),
        }
    }

    /// Creates an event for settlement reveal.
    #[must_use]
    pub fn for_settlement(
        rfq_id: RfqId,
        revealed_to: CounterpartyId,
        requester_id: CounterpartyId,
        anonymity_level: AnonymityLevel,
    ) -> Self {
        Self::new(
            rfq_id,
            revealed_to,
            requester_id,
            anonymity_level,
            "settlement",
        )
    }

    /// Creates an event for compliance reveal.
    #[must_use]
    pub fn for_compliance(
        rfq_id: RfqId,
        revealed_to: CounterpartyId,
        requester_id: CounterpartyId,
        anonymity_level: AnonymityLevel,
    ) -> Self {
        Self::new(
            rfq_id,
            revealed_to,
            requester_id,
            anonymity_level,
            "compliance",
        )
    }
}

impl DomainEvent for IdentityRevealed {
    fn event_id(&self) -> EventId {
        self.metadata.event_id
    }

    fn rfq_id(&self) -> Option<RfqId> {
        self.metadata.rfq_id
    }

    fn timestamp(&self) -> Timestamp {
        self.metadata.timestamp
    }

    fn event_type(&self) -> EventType {
        EventType::Rfq
    }

    fn event_name(&self) -> &'static str {
        "IdentityRevealed"
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::useless_vec
)]
mod tests {
    use super::*;
    use crate::domain::value_objects::Symbol;
    use crate::domain::value_objects::enums::AssetClass;

    fn create_test_instrument() -> Instrument {
        let symbol = Symbol::new("BTC/USD").unwrap();
        Instrument::builder(symbol, AssetClass::CryptoSpot).build()
    }

    fn create_test_view() -> AnonymousRfqView {
        AnonymousRfqView::new(
            RfqId::new_v4(),
            create_test_instrument(),
            OrderSide::Buy,
            Quantity::new(10.0).unwrap(),
            Timestamp::now().add_secs(300),
            AnonymityLevel::FullAnonymous,
        )
    }

    #[test]
    fn anonymous_rfq_broadcast_from_view() {
        let view = create_test_view();
        let venue_ids = vec![VenueId::new("venue-1"), VenueId::new("venue-2")];

        let event = AnonymousRfqBroadcast::new(&view, venue_ids.clone());

        assert_eq!(event.rfq_id(), Some(view.rfq_id()));
        assert_eq!(event.instrument, *view.instrument());
        assert_eq!(event.side, view.side());
        assert_eq!(event.quantity, view.quantity());
        assert_eq!(event.anonymity_level, AnonymityLevel::FullAnonymous);
        assert_eq!(event.venue_ids.len(), 2);
        assert_eq!(event.event_name(), "AnonymousRfqBroadcast");
    }

    #[test]
    fn anonymous_rfq_broadcast_from_parts() {
        let rfq_id = RfqId::new_v4();
        let instrument = create_test_instrument();
        let venue_ids = vec![VenueId::new("venue-1")];

        let event = AnonymousRfqBroadcast::from_parts(
            rfq_id,
            instrument.clone(),
            OrderSide::Sell,
            Quantity::new(5.0).unwrap(),
            Timestamp::now().add_secs(300),
            AnonymityLevel::SemiAnonymous,
            venue_ids,
        );

        assert_eq!(event.rfq_id(), Some(rfq_id));
        assert_eq!(event.side, OrderSide::Sell);
        assert_eq!(event.anonymity_level, AnonymityLevel::SemiAnonymous);
    }

    #[test]
    fn identity_revealed_for_settlement() {
        let rfq_id = RfqId::new_v4();
        let mm_id = CounterpartyId::new("mm-1");
        let requester_id = CounterpartyId::new("client-1");

        let event = IdentityRevealed::for_settlement(
            rfq_id,
            mm_id.clone(),
            requester_id.clone(),
            AnonymityLevel::FullAnonymous,
        );

        assert_eq!(event.rfq_id(), Some(rfq_id));
        assert_eq!(event.revealed_to, mm_id);
        assert_eq!(event.requester_id, requester_id);
        assert_eq!(event.reason, "settlement");
        assert_eq!(event.event_name(), "IdentityRevealed");
    }

    #[test]
    fn identity_revealed_for_compliance() {
        let rfq_id = RfqId::new_v4();
        let compliance_id = CounterpartyId::new("compliance-team");
        let requester_id = CounterpartyId::new("client-1");

        let event = IdentityRevealed::for_compliance(
            rfq_id,
            compliance_id.clone(),
            requester_id.clone(),
            AnonymityLevel::FullAnonymous,
        );

        assert_eq!(event.reason, "compliance");
        assert_eq!(event.revealed_to, compliance_id);
    }

    #[test]
    fn identity_revealed_custom_reason() {
        let rfq_id = RfqId::new_v4();
        let mm_id = CounterpartyId::new("mm-1");
        let requester_id = CounterpartyId::new("client-1");

        let event = IdentityRevealed::new(
            rfq_id,
            mm_id,
            requester_id,
            AnonymityLevel::FullAnonymous,
            "dispute_resolution",
        );

        assert_eq!(event.reason, "dispute_resolution");
    }
}
