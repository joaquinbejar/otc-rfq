//! # Audit Action Types
//!
//! Defines the types of actions that can be recorded in the negotiation audit log.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Actions that can be recorded in the negotiation audit log.
///
/// Each action represents a significant event in the negotiation lifecycle
/// that must be captured for compliance and audit purposes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AuditAction {
    /// A quote was sent to a counterparty.
    QuoteSent,
    /// A quote was received from a counterparty.
    QuoteReceived,
    /// A counter-quote was sent.
    CounterSent,
    /// A counter-quote was received.
    CounterReceived,
    /// The negotiation was accepted.
    Accepted,
    /// The negotiation was rejected.
    Rejected,
    /// The negotiation expired.
    Expired,
    /// A conflict was resolved (e.g., race condition handling).
    ConflictResolved,
}

impl AuditAction {
    /// Returns all possible audit actions.
    #[must_use]
    pub const fn all() -> &'static [AuditAction] {
        &[
            AuditAction::QuoteSent,
            AuditAction::QuoteReceived,
            AuditAction::CounterSent,
            AuditAction::CounterReceived,
            AuditAction::Accepted,
            AuditAction::Rejected,
            AuditAction::Expired,
            AuditAction::ConflictResolved,
        ]
    }

    /// Returns a human-readable description of the action.
    #[must_use]
    pub const fn description(&self) -> &'static str {
        match self {
            Self::QuoteSent => "Quote sent to counterparty",
            Self::QuoteReceived => "Quote received from counterparty",
            Self::CounterSent => "Counter-quote sent",
            Self::CounterReceived => "Counter-quote received",
            Self::Accepted => "Negotiation accepted",
            Self::Rejected => "Negotiation rejected",
            Self::Expired => "Negotiation expired",
            Self::ConflictResolved => "Conflict resolved",
        }
    }

    /// Returns true if this action represents a terminal state.
    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        matches!(self, Self::Accepted | Self::Rejected | Self::Expired)
    }
}

impl fmt::Display for AuditAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::QuoteSent => write!(f, "QUOTE_SENT"),
            Self::QuoteReceived => write!(f, "QUOTE_RECEIVED"),
            Self::CounterSent => write!(f, "COUNTER_SENT"),
            Self::CounterReceived => write!(f, "COUNTER_RECEIVED"),
            Self::Accepted => write!(f, "ACCEPTED"),
            Self::Rejected => write!(f, "REJECTED"),
            Self::Expired => write!(f, "EXPIRED"),
            Self::ConflictResolved => write!(f, "CONFLICT_RESOLVED"),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn display_formats_correctly() {
        assert_eq!(AuditAction::QuoteSent.to_string(), "QUOTE_SENT");
        assert_eq!(AuditAction::QuoteReceived.to_string(), "QUOTE_RECEIVED");
        assert_eq!(AuditAction::CounterSent.to_string(), "COUNTER_SENT");
        assert_eq!(AuditAction::CounterReceived.to_string(), "COUNTER_RECEIVED");
        assert_eq!(AuditAction::Accepted.to_string(), "ACCEPTED");
        assert_eq!(AuditAction::Rejected.to_string(), "REJECTED");
        assert_eq!(AuditAction::Expired.to_string(), "EXPIRED");
        assert_eq!(
            AuditAction::ConflictResolved.to_string(),
            "CONFLICT_RESOLVED"
        );
    }

    #[test]
    fn serde_roundtrip() {
        for action in AuditAction::all() {
            let json = serde_json::to_string(action).unwrap();
            let deserialized: AuditAction = serde_json::from_str(&json).unwrap();
            assert_eq!(*action, deserialized);
        }
    }

    #[test]
    fn is_terminal() {
        assert!(AuditAction::Accepted.is_terminal());
        assert!(AuditAction::Rejected.is_terminal());
        assert!(AuditAction::Expired.is_terminal());
        assert!(!AuditAction::QuoteSent.is_terminal());
        assert!(!AuditAction::CounterSent.is_terminal());
        assert!(!AuditAction::ConflictResolved.is_terminal());
    }

    #[test]
    fn all_returns_all_variants() {
        assert_eq!(AuditAction::all().len(), 8);
    }

    #[test]
    fn description_returns_human_readable() {
        assert!(!AuditAction::QuoteSent.description().is_empty());
        assert!(!AuditAction::Accepted.description().is_empty());
    }
}
