//! # Atomic Execution Events
//!
//! Domain events emitted during atomic trade execution.
//!
//! These events provide an audit trail for lock acquisition,
//! execution commits, and rollbacks.

use crate::domain::services::quote_lock::LockHolderId;
use crate::domain::services::resource_lock::ResourceLock;
use crate::domain::value_objects::{QuoteId, RfqId, Timestamp, TradeId};
use serde::{Deserialize, Serialize};

/// Event emitted when locks are successfully acquired.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocksAcquired {
    /// The RFQ ID associated with this execution.
    pub rfq_id: RfqId,
    /// The quote ID being executed.
    pub quote_id: QuoteId,
    /// Resources that were locked.
    pub resources: Vec<ResourceLock>,
    /// The holder that acquired the locks.
    pub holder_id: LockHolderId,
    /// When the locks were acquired.
    pub acquired_at: Timestamp,
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
            rfq_id,
            quote_id,
            resources,
            holder_id,
            acquired_at: Timestamp::now(),
        }
    }

    /// Returns the number of resources locked.
    #[must_use]
    #[inline]
    pub fn resource_count(&self) -> usize {
        self.resources.len()
    }
}

/// Event emitted when execution is successfully committed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionCommitted {
    /// The RFQ ID.
    pub rfq_id: RfqId,
    /// The trade ID created.
    pub trade_id: TradeId,
    /// The quote ID that was executed.
    pub quote_id: QuoteId,
    /// Time locks were held in milliseconds.
    pub locks_held_ms: u64,
    /// When the execution was committed.
    pub committed_at: Timestamp,
}

impl ExecutionCommitted {
    /// Creates a new execution committed event.
    #[must_use]
    pub fn new(rfq_id: RfqId, trade_id: TradeId, quote_id: QuoteId, locks_held_ms: u64) -> Self {
        Self {
            rfq_id,
            trade_id,
            quote_id,
            locks_held_ms,
            committed_at: Timestamp::now(),
        }
    }
}

/// Event emitted when execution is rolled back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionRolledBack {
    /// The RFQ ID.
    pub rfq_id: RfqId,
    /// The quote ID that failed execution.
    pub quote_id: QuoteId,
    /// Reason for the rollback.
    pub reason: String,
    /// Resources that were released.
    pub locks_released: Vec<ResourceLock>,
    /// When the rollback occurred.
    pub rolled_back_at: Timestamp,
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
            rfq_id,
            quote_id,
            reason,
            locks_released,
            rolled_back_at: Timestamp::now(),
        }
    }

    /// Returns the number of locks that were released.
    #[must_use]
    #[inline]
    pub fn locks_released_count(&self) -> usize {
        self.locks_released.len()
    }
}

/// Event emitted when lock acquisition fails.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LockAcquisitionFailed {
    /// The RFQ ID.
    pub rfq_id: RfqId,
    /// The quote ID that was being executed.
    pub quote_id: QuoteId,
    /// Resources that were requested.
    pub requested_resources: Vec<ResourceLock>,
    /// Reason for the failure.
    pub reason: String,
    /// When the failure occurred.
    pub failed_at: Timestamp,
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
            rfq_id,
            quote_id,
            requested_resources,
            reason,
            failed_at: Timestamp::now(),
        }
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

        assert_eq!(event.rfq_id, rfq_id);
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

        assert_eq!(event.rfq_id, rfq_id);
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

        assert_eq!(event.rfq_id, rfq_id);
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

        assert_eq!(event.rfq_id, rfq_id);
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
}
