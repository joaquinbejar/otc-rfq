//! # Event Store Trait
//!
//! Port definition for append-only domain event persistence.
//!
//! The event store provides append-only semantics for domain events,
//! enabling event sourcing patterns and audit trails.
//!
//! # Examples
//!
//! ```ignore
//! use otc_rfq::infrastructure::persistence::event_store::{EventStore, StoredEvent};
//!
//! // Append an event
//! event_store.append(&rfq_id, event).await?;
//!
//! // Retrieve events for an RFQ
//! let events = event_store.get_events(&rfq_id).await?;
//! ```

use crate::domain::events::domain_event::EventType;
use crate::domain::value_objects::timestamp::Timestamp;
use crate::domain::value_objects::{EventId, RfqId};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

/// Error type for event store operations.
#[derive(Debug, Error)]
pub enum EventStoreError {
    /// Failed to serialize event.
    #[error("serialization error: {0}")]
    Serialization(String),

    /// Failed to deserialize event.
    #[error("deserialization error: {0}")]
    Deserialization(String),

    /// Database connection error.
    #[error("connection error: {0}")]
    Connection(String),

    /// Query execution error.
    #[error("query error: {0}")]
    Query(String),

    /// Internal error.
    #[error("internal error: {0}")]
    Internal(String),
}

impl EventStoreError {
    /// Creates a serialization error.
    #[must_use]
    pub fn serialization(msg: impl Into<String>) -> Self {
        Self::Serialization(msg.into())
    }

    /// Creates a deserialization error.
    #[must_use]
    pub fn deserialization(msg: impl Into<String>) -> Self {
        Self::Deserialization(msg.into())
    }

    /// Creates a connection error.
    #[must_use]
    pub fn connection(msg: impl Into<String>) -> Self {
        Self::Connection(msg.into())
    }

    /// Creates a query error.
    #[must_use]
    pub fn query(msg: impl Into<String>) -> Self {
        Self::Query(msg.into())
    }

    /// Creates an internal error.
    #[must_use]
    pub fn internal(msg: impl Into<String>) -> Self {
        Self::Internal(msg.into())
    }
}

/// Result type for event store operations.
pub type EventStoreResult<T> = Result<T, EventStoreError>;

/// A stored domain event with metadata.
///
/// This struct wraps the serialized event payload with common metadata
/// for storage and retrieval.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredEvent {
    /// Unique identifier for this event.
    pub event_id: EventId,
    /// The RFQ this event relates to.
    pub rfq_id: Option<RfqId>,
    /// Type/category of the event.
    pub event_type: EventType,
    /// Human-readable event name.
    pub event_name: String,
    /// When the event occurred.
    pub timestamp: Timestamp,
    /// Serialized event payload as JSON.
    pub payload: serde_json::Value,
    /// Sequence number for ordering within an aggregate.
    pub sequence: u64,
}

impl StoredEvent {
    /// Creates a new stored event.
    #[must_use]
    pub fn new(
        event_id: EventId,
        rfq_id: Option<RfqId>,
        event_type: EventType,
        event_name: impl Into<String>,
        timestamp: Timestamp,
        payload: serde_json::Value,
        sequence: u64,
    ) -> Self {
        Self {
            event_id,
            rfq_id,
            event_type,
            event_name: event_name.into(),
            timestamp,
            payload,
            sequence,
        }
    }

    /// Creates a stored event from a domain event.
    ///
    /// # Errors
    ///
    /// Returns an error if the event cannot be serialized.
    pub fn from_event<E: Serialize + crate::domain::events::domain_event::DomainEvent>(
        event: &E,
        sequence: u64,
    ) -> EventStoreResult<Self> {
        let payload = serde_json::to_value(event)
            .map_err(|e| EventStoreError::serialization(e.to_string()))?;

        Ok(Self {
            event_id: event.event_id(),
            rfq_id: event.rfq_id(),
            event_type: event.event_type(),
            event_name: event.event_name().to_string(),
            timestamp: event.timestamp(),
            payload,
            sequence,
        })
    }
}

impl fmt::Display for StoredEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}[{}] at {}",
            self.event_name, self.event_id, self.timestamp
        )
    }
}

/// Trait for append-only event storage.
///
/// The event store provides append-only semantics - events can only be
/// added, never modified or deleted. This ensures a complete audit trail
/// and enables event sourcing patterns.
///
/// # Thread Safety
///
/// Implementations must be `Send + Sync` for use in async contexts.
#[async_trait]
pub trait EventStore: Send + Sync + fmt::Debug {
    /// Appends an event to the store.
    ///
    /// # Arguments
    ///
    /// * `event` - The stored event to append
    ///
    /// # Errors
    ///
    /// Returns an error if the event cannot be stored.
    async fn append(&self, event: StoredEvent) -> EventStoreResult<()>;

    /// Retrieves all events for an RFQ.
    ///
    /// Events are returned in sequence order (oldest first).
    ///
    /// # Arguments
    ///
    /// * `rfq_id` - The RFQ to get events for
    ///
    /// # Errors
    ///
    /// Returns an error if events cannot be retrieved.
    async fn get_events(&self, rfq_id: &RfqId) -> EventStoreResult<Vec<StoredEvent>>;

    /// Retrieves all events since a given timestamp.
    ///
    /// Events are returned in timestamp order (oldest first).
    ///
    /// # Arguments
    ///
    /// * `since` - Only return events after this timestamp
    ///
    /// # Errors
    ///
    /// Returns an error if events cannot be retrieved.
    async fn get_events_since(&self, since: Timestamp) -> EventStoreResult<Vec<StoredEvent>>;

    /// Retrieves all events of a specific type.
    ///
    /// # Arguments
    ///
    /// * `event_type` - The type of events to retrieve
    ///
    /// # Errors
    ///
    /// Returns an error if events cannot be retrieved.
    async fn get_events_by_type(&self, event_type: EventType)
    -> EventStoreResult<Vec<StoredEvent>>;

    /// Returns the total number of events in the store.
    ///
    /// # Errors
    ///
    /// Returns an error if the count cannot be retrieved.
    async fn count(&self) -> EventStoreResult<u64>;

    /// Returns the number of events for an RFQ.
    ///
    /// # Arguments
    ///
    /// * `rfq_id` - The RFQ to count events for
    ///
    /// # Errors
    ///
    /// Returns an error if the count cannot be retrieved.
    async fn count_for_rfq(&self, rfq_id: &RfqId) -> EventStoreResult<u64>;

    /// Returns the next sequence number for an RFQ.
    ///
    /// # Arguments
    ///
    /// * `rfq_id` - The RFQ to get the next sequence for
    ///
    /// # Errors
    ///
    /// Returns an error if the sequence cannot be determined.
    async fn next_sequence(&self, rfq_id: &RfqId) -> EventStoreResult<u64>;
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn event_store_error_display() {
        let err = EventStoreError::serialization("test error");
        assert_eq!(err.to_string(), "serialization error: test error");

        let err = EventStoreError::deserialization("test error");
        assert_eq!(err.to_string(), "deserialization error: test error");

        let err = EventStoreError::connection("test error");
        assert_eq!(err.to_string(), "connection error: test error");

        let err = EventStoreError::query("test error");
        assert_eq!(err.to_string(), "query error: test error");

        let err = EventStoreError::internal("test error");
        assert_eq!(err.to_string(), "internal error: test error");
    }

    #[test]
    fn stored_event_display() {
        let event = StoredEvent::new(
            EventId::new_v4(),
            Some(RfqId::new_v4()),
            EventType::Rfq,
            "RfqCreated",
            Timestamp::now(),
            serde_json::json!({}),
            1,
        );

        let display = event.to_string();
        assert!(display.contains("RfqCreated"));
    }

    #[test]
    fn stored_event_serde_roundtrip() {
        let event = StoredEvent::new(
            EventId::new_v4(),
            Some(RfqId::new_v4()),
            EventType::Rfq,
            "RfqCreated",
            Timestamp::now(),
            serde_json::json!({"test": "data"}),
            1,
        );

        let json = serde_json::to_string(&event).unwrap();
        let deserialized: StoredEvent = serde_json::from_str(&json).unwrap();

        assert_eq!(event.event_id, deserialized.event_id);
        assert_eq!(event.rfq_id, deserialized.rfq_id);
        assert_eq!(event.event_type, deserialized.event_type);
        assert_eq!(event.event_name, deserialized.event_name);
        assert_eq!(event.sequence, deserialized.sequence);
    }
}
