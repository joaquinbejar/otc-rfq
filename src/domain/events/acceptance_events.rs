//! # Acceptance Events
//!
//! Domain events for the quote acceptance workflow.
//!
//! These events track each step of the atomic acceptance flow:
//! Lock → Risk Check → Last-Look → Execute → Release.

use crate::domain::events::domain_event::{DomainEvent, EventMetadata, EventType};
use crate::domain::value_objects::timestamp::Timestamp;
use crate::domain::value_objects::{EventId, QuoteId, RfqId, VenueId};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Event emitted when a quote is locked for acceptance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuoteLocked {
    /// Event metadata.
    pub metadata: EventMetadata,
    /// The locked quote ID.
    pub quote_id: QuoteId,
    /// The venue providing the quote.
    pub venue_id: VenueId,
    /// Lock expiration time.
    pub expires_at: Timestamp,
}

impl QuoteLocked {
    /// Creates a new QuoteLocked event.
    #[must_use]
    pub fn new(rfq_id: RfqId, quote_id: QuoteId, venue_id: VenueId, expires_at: Timestamp) -> Self {
        Self {
            metadata: EventMetadata::for_rfq(rfq_id),
            quote_id,
            venue_id,
            expires_at,
        }
    }
}

impl DomainEvent for QuoteLocked {
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
        EventType::Trade
    }
    fn event_name(&self) -> &'static str {
        "QuoteLocked"
    }
}

/// Event emitted when risk check passes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RiskCheckPassed {
    /// Event metadata.
    pub metadata: EventMetadata,
    /// The quote that passed risk check.
    pub quote_id: QuoteId,
}

impl RiskCheckPassed {
    /// Creates a new RiskCheckPassed event.
    #[must_use]
    pub fn new(rfq_id: RfqId, quote_id: QuoteId) -> Self {
        Self {
            metadata: EventMetadata::for_rfq(rfq_id),
            quote_id,
        }
    }
}

impl DomainEvent for RiskCheckPassed {
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
        EventType::Trade
    }
    fn event_name(&self) -> &'static str {
        "RiskCheckPassed"
    }
}

/// Event emitted when risk check fails.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RiskCheckFailed {
    /// Event metadata.
    pub metadata: EventMetadata,
    /// The quote that failed risk check.
    pub quote_id: QuoteId,
    /// Reason for failure.
    pub reason: String,
}

impl RiskCheckFailed {
    /// Creates a new RiskCheckFailed event.
    #[must_use]
    pub fn new(rfq_id: RfqId, quote_id: QuoteId, reason: impl Into<String>) -> Self {
        Self {
            metadata: EventMetadata::for_rfq(rfq_id),
            quote_id,
            reason: reason.into(),
        }
    }
}

impl DomainEvent for RiskCheckFailed {
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
        EventType::Trade
    }
    fn event_name(&self) -> &'static str {
        "RiskCheckFailed"
    }
}

/// Event emitted when last-look request is sent to MM.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LastLookSent {
    /// Event metadata.
    pub metadata: EventMetadata,
    /// The quote being confirmed.
    pub quote_id: QuoteId,
    /// The venue receiving the request.
    pub venue_id: VenueId,
    /// Timeout for the request in milliseconds.
    pub timeout_ms: u64,
}

impl LastLookSent {
    /// Creates a new LastLookSent event.
    #[must_use]
    pub fn new(rfq_id: RfqId, quote_id: QuoteId, venue_id: VenueId, timeout: Duration) -> Self {
        Self {
            metadata: EventMetadata::for_rfq(rfq_id),
            quote_id,
            venue_id,
            timeout_ms: timeout.as_millis() as u64,
        }
    }
}

impl DomainEvent for LastLookSent {
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
        EventType::Trade
    }
    fn event_name(&self) -> &'static str {
        "LastLookSent"
    }
}

/// Event emitted when MM confirms the quote.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LastLookConfirmed {
    /// Event metadata.
    pub metadata: EventMetadata,
    /// The confirmed quote ID.
    pub quote_id: QuoteId,
    /// The venue that confirmed.
    pub venue_id: VenueId,
    /// Response time in milliseconds.
    pub response_time_ms: u64,
}

impl LastLookConfirmed {
    /// Creates a new LastLookConfirmed event.
    #[must_use]
    pub fn new(
        rfq_id: RfqId,
        quote_id: QuoteId,
        venue_id: VenueId,
        response_time: Duration,
    ) -> Self {
        Self {
            metadata: EventMetadata::for_rfq(rfq_id),
            quote_id,
            venue_id,
            response_time_ms: response_time.as_millis() as u64,
        }
    }
}

impl DomainEvent for LastLookConfirmed {
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
        EventType::Trade
    }
    fn event_name(&self) -> &'static str {
        "LastLookConfirmed"
    }
}

/// Event emitted when MM rejects the quote.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LastLookRejected {
    /// Event metadata.
    pub metadata: EventMetadata,
    /// The rejected quote ID.
    pub quote_id: QuoteId,
    /// The venue that rejected.
    pub venue_id: VenueId,
    /// Reason for rejection.
    pub reason: String,
}

impl LastLookRejected {
    /// Creates a new LastLookRejected event.
    #[must_use]
    pub fn new(
        rfq_id: RfqId,
        quote_id: QuoteId,
        venue_id: VenueId,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            metadata: EventMetadata::for_rfq(rfq_id),
            quote_id,
            venue_id,
            reason: reason.into(),
        }
    }
}

impl DomainEvent for LastLookRejected {
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
        EventType::Trade
    }
    fn event_name(&self) -> &'static str {
        "LastLookRejected"
    }
}

/// Event emitted when last-look times out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LastLookTimeout {
    /// Event metadata.
    pub metadata: EventMetadata,
    /// The quote that timed out.
    pub quote_id: QuoteId,
    /// The venue that didn't respond.
    pub venue_id: VenueId,
    /// Timeout duration in milliseconds.
    pub timeout_ms: u64,
}

impl LastLookTimeout {
    /// Creates a new LastLookTimeout event.
    #[must_use]
    pub fn new(rfq_id: RfqId, quote_id: QuoteId, venue_id: VenueId, timeout: Duration) -> Self {
        Self {
            metadata: EventMetadata::for_rfq(rfq_id),
            quote_id,
            venue_id,
            timeout_ms: timeout.as_millis() as u64,
        }
    }
}

impl DomainEvent for LastLookTimeout {
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
        EventType::Trade
    }
    fn event_name(&self) -> &'static str {
        "LastLookTimeout"
    }
}

/// Event emitted when acceptance flow completes successfully.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcceptanceCompleted {
    /// Event metadata.
    pub metadata: EventMetadata,
    /// The accepted quote ID.
    pub quote_id: QuoteId,
    /// Total acceptance duration in milliseconds.
    pub duration_ms: u64,
}

impl AcceptanceCompleted {
    /// Creates a new AcceptanceCompleted event.
    #[must_use]
    pub fn new(rfq_id: RfqId, quote_id: QuoteId, duration: Duration) -> Self {
        Self {
            metadata: EventMetadata::for_rfq(rfq_id),
            quote_id,
            duration_ms: duration.as_millis() as u64,
        }
    }
}

impl DomainEvent for AcceptanceCompleted {
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
        EventType::Trade
    }
    fn event_name(&self) -> &'static str {
        "AcceptanceCompleted"
    }
}

/// Event emitted when acceptance flow fails.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcceptanceFailed {
    /// Event metadata.
    pub metadata: EventMetadata,
    /// The quote that failed acceptance.
    pub quote_id: QuoteId,
    /// Reason for failure.
    pub reason: String,
    /// Step at which failure occurred.
    pub failed_step: String,
}

impl AcceptanceFailed {
    /// Creates a new AcceptanceFailed event.
    #[must_use]
    pub fn new(
        rfq_id: RfqId,
        quote_id: QuoteId,
        reason: impl Into<String>,
        failed_step: impl Into<String>,
    ) -> Self {
        Self {
            metadata: EventMetadata::for_rfq(rfq_id),
            quote_id,
            reason: reason.into(),
            failed_step: failed_step.into(),
        }
    }
}

impl DomainEvent for AcceptanceFailed {
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
        EventType::Trade
    }
    fn event_name(&self) -> &'static str {
        "AcceptanceFailed"
    }
}

/// Enum containing all acceptance events.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AcceptanceEvent {
    /// Quote was locked.
    QuoteLocked(QuoteLocked),
    /// Risk check passed.
    RiskCheckPassed(RiskCheckPassed),
    /// Risk check failed.
    RiskCheckFailed(RiskCheckFailed),
    /// Last-look request sent.
    LastLookSent(LastLookSent),
    /// Last-look confirmed.
    LastLookConfirmed(LastLookConfirmed),
    /// Last-look rejected.
    LastLookRejected(LastLookRejected),
    /// Last-look timed out.
    LastLookTimeout(LastLookTimeout),
    /// Acceptance completed.
    AcceptanceCompleted(AcceptanceCompleted),
    /// Acceptance failed.
    AcceptanceFailed(AcceptanceFailed),
}

impl DomainEvent for AcceptanceEvent {
    fn event_id(&self) -> EventId {
        match self {
            Self::QuoteLocked(e) => e.event_id(),
            Self::RiskCheckPassed(e) => e.event_id(),
            Self::RiskCheckFailed(e) => e.event_id(),
            Self::LastLookSent(e) => e.event_id(),
            Self::LastLookConfirmed(e) => e.event_id(),
            Self::LastLookRejected(e) => e.event_id(),
            Self::LastLookTimeout(e) => e.event_id(),
            Self::AcceptanceCompleted(e) => e.event_id(),
            Self::AcceptanceFailed(e) => e.event_id(),
        }
    }

    fn rfq_id(&self) -> Option<RfqId> {
        match self {
            Self::QuoteLocked(e) => e.rfq_id(),
            Self::RiskCheckPassed(e) => e.rfq_id(),
            Self::RiskCheckFailed(e) => e.rfq_id(),
            Self::LastLookSent(e) => e.rfq_id(),
            Self::LastLookConfirmed(e) => e.rfq_id(),
            Self::LastLookRejected(e) => e.rfq_id(),
            Self::LastLookTimeout(e) => e.rfq_id(),
            Self::AcceptanceCompleted(e) => e.rfq_id(),
            Self::AcceptanceFailed(e) => e.rfq_id(),
        }
    }

    fn timestamp(&self) -> Timestamp {
        match self {
            Self::QuoteLocked(e) => e.timestamp(),
            Self::RiskCheckPassed(e) => e.timestamp(),
            Self::RiskCheckFailed(e) => e.timestamp(),
            Self::LastLookSent(e) => e.timestamp(),
            Self::LastLookConfirmed(e) => e.timestamp(),
            Self::LastLookRejected(e) => e.timestamp(),
            Self::LastLookTimeout(e) => e.timestamp(),
            Self::AcceptanceCompleted(e) => e.timestamp(),
            Self::AcceptanceFailed(e) => e.timestamp(),
        }
    }

    fn event_type(&self) -> EventType {
        EventType::Trade
    }

    fn event_name(&self) -> &'static str {
        match self {
            Self::QuoteLocked(e) => e.event_name(),
            Self::RiskCheckPassed(e) => e.event_name(),
            Self::RiskCheckFailed(e) => e.event_name(),
            Self::LastLookSent(e) => e.event_name(),
            Self::LastLookConfirmed(e) => e.event_name(),
            Self::LastLookRejected(e) => e.event_name(),
            Self::LastLookTimeout(e) => e.event_name(),
            Self::AcceptanceCompleted(e) => e.event_name(),
            Self::AcceptanceFailed(e) => e.event_name(),
        }
    }
}
