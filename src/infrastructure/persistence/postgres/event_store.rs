//! # PostgreSQL Event Store
//!
//! PostgreSQL implementation of [`EventStore`] using sqlx.
//!
//! This implementation provides append-only event storage with JSONB
//! serialization for event payloads.

use crate::domain::events::domain_event::EventType;
use crate::domain::value_objects::timestamp::Timestamp;
use crate::domain::value_objects::{EventId, RfqId};
use crate::infrastructure::persistence::event_store::{
    EventStore, EventStoreError, EventStoreResult, StoredEvent,
};
use async_trait::async_trait;
use sqlx::PgPool;

/// PostgreSQL implementation of [`EventStore`].
///
/// Uses connection pooling via `sqlx::PgPool` and JSONB for event payloads.
/// Provides append-only semantics - events can only be inserted, never
/// updated or deleted.
///
/// # Examples
///
/// ```ignore
/// use sqlx::PgPool;
/// use otc_rfq::infrastructure::persistence::postgres::PostgresEventStore;
///
/// let pool = PgPool::connect("postgres://...").await?;
/// let store = PostgresEventStore::new(pool);
/// ```
#[derive(Debug, Clone)]
pub struct PostgresEventStore {
    pool: PgPool,
}

impl PostgresEventStore {
    /// Creates a new PostgreSQL event store.
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Returns a reference to the connection pool.
    #[must_use]
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

#[async_trait]
impl EventStore for PostgresEventStore {
    async fn append(&self, event: StoredEvent) -> EventStoreResult<()> {
        let event_id = event.event_id.to_string();
        let rfq_id = event.rfq_id.map(|id| id.to_string());
        let event_type = event.event_type.to_string();
        let event_name = &event.event_name;
        let timestamp = event.timestamp.timestamp_millis();
        let payload = &event.payload;
        let sequence = event.sequence as i64;

        sqlx::query(
            r#"
            INSERT INTO domain_events (
                event_id, rfq_id, event_type, event_name,
                timestamp, payload, sequence
            ) VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
        )
        .bind(&event_id)
        .bind(&rfq_id)
        .bind(&event_type)
        .bind(event_name)
        .bind(timestamp)
        .bind(payload)
        .bind(sequence)
        .execute(&self.pool)
        .await
        .map_err(|e| EventStoreError::query(e.to_string()))?;

        Ok(())
    }

    async fn get_events(&self, rfq_id: RfqId) -> EventStoreResult<Vec<StoredEvent>> {
        let rfq_id_str = rfq_id.to_string();

        let rows: Vec<EventRow> = sqlx::query_as(
            r#"
            SELECT event_id, rfq_id, event_type, event_name,
                   timestamp, payload, sequence
            FROM domain_events
            WHERE rfq_id = $1
            ORDER BY sequence ASC
            "#,
        )
        .bind(&rfq_id_str)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| EventStoreError::query(e.to_string()))?;

        rows.into_iter()
            .map(|r| r.try_into_stored_event())
            .collect()
    }

    async fn get_events_since(&self, since: Timestamp) -> EventStoreResult<Vec<StoredEvent>> {
        let since_millis = since.timestamp_millis();

        let rows: Vec<EventRow> = sqlx::query_as(
            r#"
            SELECT event_id, rfq_id, event_type, event_name,
                   timestamp, payload, sequence
            FROM domain_events
            WHERE timestamp > $1
            ORDER BY timestamp ASC, sequence ASC
            "#,
        )
        .bind(since_millis)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| EventStoreError::query(e.to_string()))?;

        rows.into_iter()
            .map(|r| r.try_into_stored_event())
            .collect()
    }

    async fn get_events_by_type(
        &self,
        event_type: EventType,
    ) -> EventStoreResult<Vec<StoredEvent>> {
        let event_type_str = event_type.to_string();

        let rows: Vec<EventRow> = sqlx::query_as(
            r#"
            SELECT event_id, rfq_id, event_type, event_name,
                   timestamp, payload, sequence
            FROM domain_events
            WHERE event_type = $1
            ORDER BY timestamp ASC, sequence ASC
            "#,
        )
        .bind(&event_type_str)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| EventStoreError::query(e.to_string()))?;

        rows.into_iter()
            .map(|r| r.try_into_stored_event())
            .collect()
    }

    async fn count(&self) -> EventStoreResult<u64> {
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM domain_events")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| EventStoreError::query(e.to_string()))?;

        Ok(count as u64)
    }

    async fn count_for_rfq(&self, rfq_id: RfqId) -> EventStoreResult<u64> {
        let rfq_id_str = rfq_id.to_string();

        let (count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM domain_events WHERE rfq_id = $1")
                .bind(&rfq_id_str)
                .fetch_one(&self.pool)
                .await
                .map_err(|e| EventStoreError::query(e.to_string()))?;

        Ok(count as u64)
    }

    async fn next_sequence(&self, rfq_id: RfqId) -> EventStoreResult<u64> {
        let rfq_id_str = rfq_id.to_string();

        let result: Option<(i64,)> =
            sqlx::query_as("SELECT MAX(sequence) FROM domain_events WHERE rfq_id = $1")
                .bind(&rfq_id_str)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| EventStoreError::query(e.to_string()))?;

        match result {
            Some((max_seq,)) => Ok((max_seq + 1) as u64),
            None => Ok(1),
        }
    }
}

/// Row type for event queries.
#[derive(Debug, sqlx::FromRow)]
struct EventRow {
    event_id: String,
    rfq_id: Option<String>,
    event_type: String,
    event_name: String,
    timestamp: i64,
    payload: serde_json::Value,
    sequence: i64,
}

impl EventRow {
    /// Converts the row into a StoredEvent.
    fn try_into_stored_event(self) -> EventStoreResult<StoredEvent> {
        use uuid::Uuid;

        let event_uuid = Uuid::parse_str(&self.event_id)
            .map_err(|e| EventStoreError::deserialization(e.to_string()))?;
        let event_id = EventId::new(event_uuid);

        let rfq_id = self
            .rfq_id
            .map(|s| Uuid::parse_str(&s).map(RfqId::new))
            .transpose()
            .map_err(|e| EventStoreError::deserialization(e.to_string()))?;

        let event_type: EventType = serde_json::from_str(&format!("\"{}\"", self.event_type))
            .map_err(|e| EventStoreError::deserialization(e.to_string()))?;

        let timestamp = Timestamp::from_millis(self.timestamp)
            .ok_or_else(|| EventStoreError::deserialization("invalid timestamp".to_string()))?;

        Ok(StoredEvent {
            event_id,
            rfq_id,
            event_type,
            event_name: self.event_name,
            timestamp,
            payload: self.payload,
            sequence: self.sequence as u64,
        })
    }
}
