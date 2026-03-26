//! # Atomic Execution Events
//!
//! Domain events emitted during atomic trade execution.
//!
//! These events provide an audit trail for lock acquisition,
//! execution commits, and rollbacks.

use crate::domain::events::domain_event::{DomainEvent, EventMetadata, EventType};
use crate::domain::services::quote_lock::LockHolderId;
use crate::domain::services::resource_lock::ResourceLock;
use crate::domain::value_objects::{EventId, QuoteId, RfqId, Timestamp, TradeId};
use serde::{Deserialize, Serialize};

/// Event emitted when locks are successfully acquired.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocksAcquired {
    /// Event metadata.
    pub metadata: EventMetadata,
    /// The quote ID being executed.
    pub quote_id: QuoteId,
    /// Resources that were locked.
    pub resources: Vec<ResourceLock>,
    /// The holder that acquired the locks.
    pub holder_id: LockHolderId,
}

impl LocksAcquired {
    /// Creates a new locks acquired event.
    #[must_use]
    pub fn new(
        rfq_id: RfqId,
        quote_id: QuoteId,
        resources: Vec<ResourceLock>,
        holder_id: LockHolderId,
    ) -> Self {
        Self {
            metadata: EventMetadata::for_rfq(rfq_id),
            quote_id,
            resources,
            holder_id,
        }
    }

    /// Returns the number of resources locked.
    #[must_use]
    #[inline]
    pub fn resource_count(&self) -> usize {
        self.resources.len()
    }
}

impl DomainEvent for LocksAcquired {
    #[inline]
    fn event_id(&self) -> EventId {
        self.metadata.event_id
    }

    #[inline]
    fn rfq_id(&self) -> Option<RfqId> {
        self.metadata.rfq_id
    }

    #[inline]
    fn timestamp(&self) -> Timestamp {
        self.metadata.timestamp
    }

    fn event_type(&self) -> EventType {
        EventType::Trade
    }

    fn event_name(&self) -> &'static str {
        "LocksAcquired"
    }
}

/// Event emitted when execution is successfully committed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionCommitted {
    /// Event metadata.
    pub metadata: EventMetadata,
    /// The trade ID created.
    pub trade_id: TradeId,
    /// The quote ID that was executed.
    pub quote_id: QuoteId,
    /// Time locks were held in milliseconds.
    pub locks_held_ms: u64,
}

impl ExecutionCommitted {
    /// Creates a new execution committed event.
    #[must_use]
    pub fn new(rfq_id: RfqId, trade_id: TradeId, quote_id: QuoteId, locks_held_ms: u64) -> Self {
        Self {
            metadata: EventMetadata::for_rfq(rfq_id),
            trade_id,
            quote_id,
            locks_held_ms,
        }
    }
}

impl DomainEvent for ExecutionCommitted {
    #[inline]
    fn event_id(&self) -> EventId {
        self.metadata.event_id
    }

    #[inline]
    fn rfq_id(&self) -> Option<RfqId> {
        self.metadata.rfq_id
    }

    #[inline]
    fn timestamp(&self) -> Timestamp {
        self.metadata.timestamp
    }

    fn event_type(&self) -> EventType {
        EventType::Trade
    }

    fn event_name(&self) -> &'static str {
        "ExecutionCommitted"
    }
}

/// Event emitted when execution is rolled back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionRolledBack {
    /// Event metadata.
    pub metadata: EventMetadata,
    /// The quote ID that failed execution.
    pub quote_id: QuoteId,
    /// Reason for the rollback.
    pub reason: String,
    /// Resources that were released.
    pub locks_released: Vec<ResourceLock>,
}

impl ExecutionRolledBack {
    /// Creates a new execution rolled back event.
    #[must_use]
    pub fn new(
        rfq_id: RfqId,
        quote_id: QuoteId,
        reason: String,
        locks_released: Vec<ResourceLock>,
    ) -> Self {
        Self {
            metadata: EventMetadata::for_rfq(rfq_id),
            quote_id,
            reason,
            locks_released,
        }
    }

    /// Returns the number of locks that were released.
    #[must_use]
    #[inline]
    pub fn locks_released_count(&self) -> usize {
        self.locks_released.len()
    }
}

impl DomainEvent for ExecutionRolledBack {
    #[inline]
    fn event_id(&self) -> EventId {
        self.metadata.event_id
    }

    #[inline]
    fn rfq_id(&self) -> Option<RfqId> {
        self.metadata.rfq_id
    }

    #[inline]
    fn timestamp(&self) -> Timestamp {
        self.metadata.timestamp
    }

    fn event_type(&self) -> EventType {
        EventType::Trade
    }

    fn event_name(&self) -> &'static str {
        "ExecutionRolledBack"
    }
}

/// Event emitted when lock acquisition fails.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LockAcquisitionFailed {
    /// Event metadata.
    pub metadata: EventMetadata,
    /// The quote ID that was being executed.
    pub quote_id: QuoteId,
    /// Resources that were requested.
    pub requested_resources: Vec<ResourceLock>,
    /// Reason for the failure.
    pub reason: String,
}

impl LockAcquisitionFailed {
    /// Creates a new lock acquisition failed event.
    #[must_use]
    pub fn new(
        rfq_id: RfqId,
        quote_id: QuoteId,
        requested_resources: Vec<ResourceLock>,
        reason: String,
    ) -> Self {
        Self {
            metadata: EventMetadata::for_rfq(rfq_id),
            quote_id,
            requested_resources,
            reason,
        }
    }
}

impl DomainEvent for LockAcquisitionFailed {
    #[inline]
    fn event_id(&self) -> EventId {
        self.metadata.event_id
    }

    #[inline]
    fn rfq_id(&self) -> Option<RfqId> {
        self.metadata.rfq_id
    }

    #[inline]
    fn timestamp(&self) -> Timestamp {
        self.metadata.timestamp
    }

    fn event_type(&self) -> EventType {
        EventType::Trade
    }

    fn event_name(&self) -> &'static str {
        "LockAcquisitionFailed"
    }
}

/// Enum representing all atomic execution events.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AtomicExecutionEvent {
    /// Locks were acquired.
    LocksAcquired(LocksAcquired),
    /// Execution was committed.
    ExecutionCommitted(ExecutionCommitted),
    /// Execution was rolled back.
    ExecutionRolledBack(ExecutionRolledBack),
    /// Lock acquisition failed.
    LockAcquisitionFailed(LockAcquisitionFailed),
}

impl From<LocksAcquired> for AtomicExecutionEvent {
    fn from(event: LocksAcquired) -> Self {
        Self::LocksAcquired(event)
    }
}

impl From<ExecutionCommitted> for AtomicExecutionEvent {
    fn from(event: ExecutionCommitted) -> Self {
        Self::ExecutionCommitted(event)
    }
}

impl From<ExecutionRolledBack> for AtomicExecutionEvent {
    fn from(event: ExecutionRolledBack) -> Self {
        Self::ExecutionRolledBack(event)
    }
}

impl From<LockAcquisitionFailed> for AtomicExecutionEvent {
    fn from(event: LockAcquisitionFailed) -> Self {
        Self::LockAcquisitionFailed(event)
    }
}

impl DomainEvent for AtomicExecutionEvent {
    #[inline]
    fn event_id(&self) -> EventId {
        match self {
            Self::LocksAcquired(e) => e.event_id(),
            Self::ExecutionCommitted(e) => e.event_id(),
            Self::ExecutionRolledBack(e) => e.event_id(),
            Self::LockAcquisitionFailed(e) => e.event_id(),
        }
    }

    #[inline]
    fn rfq_id(&self) -> Option<RfqId> {
        match self {
            Self::LocksAcquired(e) => e.rfq_id(),
            Self::ExecutionCommitted(e) => e.rfq_id(),
            Self::ExecutionRolledBack(e) => e.rfq_id(),
            Self::LockAcquisitionFailed(e) => e.rfq_id(),
        }
    }

    #[inline]
    fn timestamp(&self) -> Timestamp {
        match self {
            Self::LocksAcquired(e) => e.timestamp(),
            Self::ExecutionCommitted(e) => e.timestamp(),
            Self::ExecutionRolledBack(e) => e.timestamp(),
            Self::LockAcquisitionFailed(e) => e.timestamp(),
        }
    }

    fn event_type(&self) -> EventType {
        EventType::Trade
    }

    fn event_name(&self) -> &'static str {
        match self {
            Self::LocksAcquired(e) => e.event_name(),
            Self::ExecutionCommitted(e) => e.event_name(),
            Self::ExecutionRolledBack(e) => e.event_name(),
            Self::LockAcquisitionFailed(e) => e.event_name(),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::domain::value_objects::CounterpartyId;

    #[test]
    fn locks_acquired_creation() {
        let rfq_id = RfqId::new_v4();
        let quote_id = QuoteId::new_v4();
        let resources = vec![
            ResourceLock::Quote(quote_id),
            ResourceLock::Account(CounterpartyId::new("client")),
        ];
        let holder_id = LockHolderId::new();

        let event = LocksAcquired::new(rfq_id, quote_id, resources.clone(), holder_id);

        assert_eq!(event.rfq_id(), Some(rfq_id));
        assert_eq!(event.quote_id, quote_id);
        assert_eq!(event.resource_count(), 2);
        assert_eq!(event.holder_id, holder_id);
    }

    #[test]
    fn execution_committed_creation() {
        let rfq_id = RfqId::new_v4();
        let trade_id = TradeId::new_v4();
        let quote_id = QuoteId::new_v4();

        let event = ExecutionCommitted::new(rfq_id, trade_id, quote_id, 50);

        assert_eq!(event.rfq_id(), Some(rfq_id));
        assert_eq!(event.trade_id, trade_id);
        assert_eq!(event.quote_id, quote_id);
        assert_eq!(event.locks_held_ms, 50);
    }

    #[test]
    fn execution_rolled_back_creation() {
        let rfq_id = RfqId::new_v4();
        let quote_id = QuoteId::new_v4();
        let resources = vec![ResourceLock::Quote(quote_id)];

        let event = ExecutionRolledBack::new(
            rfq_id,
            quote_id,
            "test failure".to_string(),
            resources.clone(),
        );

        assert_eq!(event.rfq_id(), Some(rfq_id));
        assert_eq!(event.quote_id, quote_id);
        assert_eq!(event.reason, "test failure");
        assert_eq!(event.locks_released_count(), 1);
    }

    #[test]
    fn lock_acquisition_failed_creation() {
        let rfq_id = RfqId::new_v4();
        let quote_id = QuoteId::new_v4();
        let resources = vec![ResourceLock::Quote(quote_id)];

        let event =
            LockAcquisitionFailed::new(rfq_id, quote_id, resources.clone(), "timeout".to_string());

        assert_eq!(event.rfq_id(), Some(rfq_id));
        assert_eq!(event.quote_id, quote_id);
        assert_eq!(event.reason, "timeout");
    }

    #[test]
    fn atomic_execution_event_from_conversions() {
        let rfq_id = RfqId::new_v4();
        let quote_id = QuoteId::new_v4();
        let trade_id = TradeId::new_v4();

        let locks_acquired = LocksAcquired::new(rfq_id, quote_id, vec![], LockHolderId::new());
        let event: AtomicExecutionEvent = locks_acquired.into();
        assert!(matches!(event, AtomicExecutionEvent::LocksAcquired(_)));

        let committed = ExecutionCommitted::new(rfq_id, trade_id, quote_id, 10);
        let event: AtomicExecutionEvent = committed.into();
        assert!(matches!(event, AtomicExecutionEvent::ExecutionCommitted(_)));

        let rolled_back = ExecutionRolledBack::new(rfq_id, quote_id, "error".to_string(), vec![]);
        let event: AtomicExecutionEvent = rolled_back.into();
        assert!(matches!(
            event,
            AtomicExecutionEvent::ExecutionRolledBack(_)
        ));

        let failed = LockAcquisitionFailed::new(rfq_id, quote_id, vec![], "timeout".to_string());
        let event: AtomicExecutionEvent = failed.into();
        assert!(matches!(
            event,
            AtomicExecutionEvent::LockAcquisitionFailed(_)
        ));
    }

    #[test]
    fn locks_acquired_implements_domain_event() {
        let rfq_id = RfqId::new_v4();
        let event = LocksAcquired::new(rfq_id, QuoteId::new_v4(), vec![], LockHolderId::new());

        assert_eq!(event.rfq_id(), Some(rfq_id));
        assert_eq!(event.event_type(), EventType::Trade);
        assert_eq!(event.event_name(), "LocksAcquired");
        assert!(!event.event_id().to_string().is_empty());
    }

    #[test]
    fn execution_committed_implements_domain_event() {
        let rfq_id = RfqId::new_v4();
        let event = ExecutionCommitted::new(rfq_id, TradeId::new_v4(), QuoteId::new_v4(), 100);

        assert_eq!(event.rfq_id(), Some(rfq_id));
        assert_eq!(event.event_type(), EventType::Trade);
        assert_eq!(event.event_name(), "ExecutionCommitted");
    }

    #[test]
    fn execution_rolled_back_implements_domain_event() {
        let rfq_id = RfqId::new_v4();
        let event =
            ExecutionRolledBack::new(rfq_id, QuoteId::new_v4(), "error".to_string(), vec![]);

        assert_eq!(event.rfq_id(), Some(rfq_id));
        assert_eq!(event.event_type(), EventType::Trade);
        assert_eq!(event.event_name(), "ExecutionRolledBack");
    }

    #[test]
    fn lock_acquisition_failed_implements_domain_event() {
        let rfq_id = RfqId::new_v4();
        let event =
            LockAcquisitionFailed::new(rfq_id, QuoteId::new_v4(), vec![], "timeout".to_string());

        assert_eq!(event.rfq_id(), Some(rfq_id));
        assert_eq!(event.event_type(), EventType::Trade);
        assert_eq!(event.event_name(), "LockAcquisitionFailed");
    }

    #[test]
    fn atomic_execution_event_enum_implements_domain_event() {
        let rfq_id = RfqId::new_v4();
        let locks_acquired =
            LocksAcquired::new(rfq_id, QuoteId::new_v4(), vec![], LockHolderId::new());
        let event: AtomicExecutionEvent = locks_acquired.into();

        assert_eq!(event.rfq_id(), Some(rfq_id));
        assert_eq!(event.event_type(), EventType::Trade);
        assert_eq!(event.event_name(), "LocksAcquired");
    }
}
