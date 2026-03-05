//! # Conflict Events
//!
//! Domain events for conflict detection and resolution during quote acceptance.
//!
//! These events are emitted when race conditions occur during simultaneous
//! quote acceptance attempts, enabling audit trail and conflict analysis.

use crate::domain::events::domain_event::{DomainEvent, EventMetadata, EventType};
use crate::domain::services::quote_lock::LockHolderId;
use crate::domain::value_objects::{EventId, Price, QuoteId, RfqId, Timestamp};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Types of conflicts that can occur during quote acceptance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConflictType {
    /// Quote already accepted by another party.
    QuoteAlreadyTaken,
    /// Multiple parties attempting to accept simultaneously.
    SimultaneousAcceptance,
    /// Quote expired during transaction processing.
    ExpiredMidTransaction,
    /// Both parties sent counter-quotes that cross.
    CounterCrossing,
}

/// Resolution strategy for a conflict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Resolution {
    /// First commit wins - reject the second requester.
    FirstCommitWins {
        /// The holder who won the lock.
        winner_id: LockHolderId,
    },
    /// Retry the operation after a delay.
    Retry {
        /// Delay before retry.
        delay: Duration,
        /// Maximum retry attempts.
        max_attempts: u8,
    },
    /// Cancel the transaction entirely.
    Cancel {
        /// Reason for cancellation.
        reason: String,
    },
    /// Notify parties of the conflict.
    Notify {
        /// Notification message.
        message: String,
    },
    /// For counter-crossing: accept the better price.
    AcceptBetterPrice {
        /// The accepted price.
        accepted_price: Price,
    },
}

impl std::fmt::Display for ConflictType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::QuoteAlreadyTaken => write!(f, "quote_already_taken"),
            Self::SimultaneousAcceptance => write!(f, "simultaneous_acceptance"),
            Self::ExpiredMidTransaction => write!(f, "expired_mid_transaction"),
            Self::CounterCrossing => write!(f, "counter_crossing"),
        }
    }
}

impl Resolution {
    /// Creates a first-commit-wins resolution.
    #[must_use]
    pub fn first_commit_wins(winner_id: LockHolderId) -> Self {
        Self::FirstCommitWins { winner_id }
    }

    /// Creates a cancel resolution.
    #[must_use]
    pub fn cancel(reason: impl Into<String>) -> Self {
        Self::Cancel {
            reason: reason.into(),
        }
    }

    /// Creates an accept-better-price resolution.
    #[must_use]
    pub fn accept_better_price(price: Price) -> Self {
        Self::AcceptBetterPrice {
            accepted_price: price,
        }
    }

    /// Returns true if this resolution allows the requester to proceed.
    #[must_use]
    pub fn allows_proceed(&self) -> bool {
        matches!(self, Self::AcceptBetterPrice { .. })
    }
}

/// Event emitted when a conflict is detected during quote acceptance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConflictDetectedEvent {
    /// Event metadata.
    pub metadata: EventMetadata,
    /// The quote involved in the conflict.
    pub quote_id: QuoteId,
    /// The type of conflict detected.
    pub conflict_type: ConflictType,
    /// The requester who triggered the conflict.
    pub requester_id: LockHolderId,
    /// The existing holder (if any) who has the lock.
    pub existing_holder: Option<LockHolderId>,
}

impl ConflictDetectedEvent {
    /// Creates a new conflict detected event.
    #[must_use]
    pub fn new(
        rfq_id: RfqId,
        quote_id: QuoteId,
        conflict_type: ConflictType,
        requester_id: LockHolderId,
        existing_holder: Option<LockHolderId>,
    ) -> Self {
        Self {
            metadata: EventMetadata::for_rfq(rfq_id),
            quote_id,
            conflict_type,
            requester_id,
            existing_holder,
        }
    }
}

impl DomainEvent for ConflictDetectedEvent {
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
        EventType::Quote
    }

    fn event_name(&self) -> &'static str {
        "ConflictDetected"
    }
}

impl std::fmt::Display for ConflictDetectedEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ConflictDetected(quote={}, type={:?}, requester={})",
            self.quote_id, self.conflict_type, self.requester_id
        )
    }
}

/// Event emitted when a conflict has been resolved.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConflictResolvedEvent {
    /// Event metadata.
    pub metadata: EventMetadata,
    /// The quote involved in the conflict.
    pub quote_id: QuoteId,
    /// The type of conflict that was resolved.
    pub conflict_type: ConflictType,
    /// The resolution applied.
    pub resolution: Resolution,
}

impl ConflictResolvedEvent {
    /// Creates a new conflict resolved event.
    #[must_use]
    pub fn new(
        rfq_id: RfqId,
        quote_id: QuoteId,
        conflict_type: ConflictType,
        resolution: Resolution,
    ) -> Self {
        Self {
            metadata: EventMetadata::for_rfq(rfq_id),
            quote_id,
            conflict_type,
            resolution,
        }
    }
}

impl DomainEvent for ConflictResolvedEvent {
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
        EventType::Quote
    }

    fn event_name(&self) -> &'static str {
        "ConflictResolved"
    }
}

impl std::fmt::Display for ConflictResolvedEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ConflictResolved(quote={}, type={:?}, resolution={:?})",
            self.quote_id, self.conflict_type, self.resolution
        )
    }
}

/// Enum wrapping all conflict-related events.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ConflictEvent {
    /// A conflict was detected.
    Detected(ConflictDetectedEvent),
    /// A conflict was resolved.
    Resolved(ConflictResolvedEvent),
}

impl From<ConflictDetectedEvent> for ConflictEvent {
    fn from(event: ConflictDetectedEvent) -> Self {
        Self::Detected(event)
    }
}

impl From<ConflictResolvedEvent> for ConflictEvent {
    fn from(event: ConflictResolvedEvent) -> Self {
        Self::Resolved(event)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn test_rfq_id() -> RfqId {
        RfqId::new_v4()
    }

    fn test_quote_id() -> QuoteId {
        QuoteId::new_v4()
    }

    #[test]
    fn conflict_detected_event_creation() {
        let rfq_id = test_rfq_id();
        let quote_id = test_quote_id();
        let requester = LockHolderId::new();
        let existing = LockHolderId::new();

        let event = ConflictDetectedEvent::new(
            rfq_id,
            quote_id,
            ConflictType::SimultaneousAcceptance,
            requester,
            Some(existing),
        );

        assert_eq!(event.quote_id, quote_id);
        assert_eq!(event.conflict_type, ConflictType::SimultaneousAcceptance);
        assert_eq!(event.requester_id, requester);
        assert_eq!(event.existing_holder, Some(existing));
        assert_eq!(event.event_name(), "ConflictDetected");
        assert_eq!(event.event_type(), EventType::Quote);
    }

    #[test]
    fn conflict_resolved_event_creation() {
        let rfq_id = test_rfq_id();
        let quote_id = test_quote_id();
        let winner = LockHolderId::new();

        let event = ConflictResolvedEvent::new(
            rfq_id,
            quote_id,
            ConflictType::QuoteAlreadyTaken,
            Resolution::FirstCommitWins { winner_id: winner },
        );

        assert_eq!(event.quote_id, quote_id);
        assert_eq!(event.conflict_type, ConflictType::QuoteAlreadyTaken);
        assert!(matches!(
            event.resolution,
            Resolution::FirstCommitWins { .. }
        ));
        assert_eq!(event.event_name(), "ConflictResolved");
    }

    #[test]
    fn conflict_event_enum_from() {
        let rfq_id = test_rfq_id();
        let quote_id = test_quote_id();
        let requester = LockHolderId::new();

        let detected = ConflictDetectedEvent::new(
            rfq_id,
            quote_id,
            ConflictType::ExpiredMidTransaction,
            requester,
            None,
        );

        let event: ConflictEvent = detected.into();
        assert!(matches!(event, ConflictEvent::Detected(_)));
    }

    #[test]
    fn conflict_detected_display() {
        let rfq_id = test_rfq_id();
        let quote_id = test_quote_id();
        let requester = LockHolderId::new();

        let event = ConflictDetectedEvent::new(
            rfq_id,
            quote_id,
            ConflictType::CounterCrossing,
            requester,
            None,
        );

        let display = event.to_string();
        assert!(display.contains("ConflictDetected"));
        assert!(display.contains("CounterCrossing"));
    }
}
