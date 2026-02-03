//! # PostgreSQL RFQ Repository
//!
//! PostgreSQL implementation of [`RfqRepository`] using sqlx.
//!
//! This implementation uses PostgreSQL with JSONB for complex fields
//! and optimistic locking via version fields.

use crate::domain::entities::rfq::Rfq;
use crate::domain::value_objects::{CounterpartyId, RfqId, RfqState, VenueId};
use crate::infrastructure::persistence::traits::{
    RepositoryError, RepositoryResult, RfqRepository,
};
use async_trait::async_trait;
use sqlx::PgPool;

/// PostgreSQL implementation of [`RfqRepository`].
///
/// Uses connection pooling via `sqlx::PgPool` and JSONB for complex fields.
///
/// # Examples
///
/// ```ignore
/// use sqlx::PgPool;
/// use otc_rfq::infrastructure::persistence::postgres::PostgresRfqRepository;
///
/// let pool = PgPool::connect("postgres://...").await?;
/// let repo = PostgresRfqRepository::new(pool);
/// ```
#[derive(Debug, Clone)]
pub struct PostgresRfqRepository {
    pool: PgPool,
}

impl PostgresRfqRepository {
    /// Creates a new PostgreSQL RFQ repository.
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
impl RfqRepository for PostgresRfqRepository {
    async fn save(&self, rfq: &Rfq) -> RepositoryResult<()> {
        let id = rfq.id().to_string();
        let client_id = rfq.client_id().as_str();
        let instrument_json = serde_json::to_value(rfq.instrument())
            .map_err(|e| RepositoryError::serialization(e.to_string()))?;
        let side = rfq.side().to_string();
        let quantity = rfq.quantity().get();
        let state = rfq.state().to_string();
        let expires_at = rfq.expires_at().timestamp_millis();
        let quotes_json = serde_json::to_value(rfq.quotes())
            .map_err(|e| RepositoryError::serialization(e.to_string()))?;
        let selected_quote_id = rfq.selected_quote_id().map(|q| q.to_string());
        let compliance_result_json = rfq
            .compliance_result()
            .map(serde_json::to_value)
            .transpose()
            .map_err(|e| RepositoryError::serialization(e.to_string()))?;
        let failure_reason = rfq.failure_reason().map(|s| s.to_string());
        let version = rfq.version() as i64;
        let created_at = rfq.created_at().timestamp_millis();
        let updated_at = rfq.updated_at().timestamp_millis();

        // Use upsert with version check for optimistic locking
        let result = sqlx::query(
            r#"
            INSERT INTO rfqs (
                id, client_id, instrument, side, quantity, state, expires_at,
                quotes, selected_quote_id, compliance_result, failure_reason,
                version, created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
            ON CONFLICT (id) DO UPDATE SET
                client_id = EXCLUDED.client_id,
                instrument = EXCLUDED.instrument,
                side = EXCLUDED.side,
                quantity = EXCLUDED.quantity,
                state = EXCLUDED.state,
                expires_at = EXCLUDED.expires_at,
                quotes = EXCLUDED.quotes,
                selected_quote_id = EXCLUDED.selected_quote_id,
                compliance_result = EXCLUDED.compliance_result,
                failure_reason = EXCLUDED.failure_reason,
                version = EXCLUDED.version,
                updated_at = EXCLUDED.updated_at
            WHERE rfqs.version < EXCLUDED.version
            "#,
        )
        .bind(&id)
        .bind(client_id)
        .bind(&instrument_json)
        .bind(&side)
        .bind(quantity)
        .bind(&state)
        .bind(expires_at)
        .bind(&quotes_json)
        .bind(&selected_quote_id)
        .bind(&compliance_result_json)
        .bind(&failure_reason)
        .bind(version)
        .bind(created_at)
        .bind(updated_at)
        .execute(&self.pool)
        .await
        .map_err(|e| RepositoryError::query(e.to_string()))?;

        // Check if update was applied (for existing records)
        if result.rows_affected() == 0 {
            // Check if record exists with higher version
            let exists: Option<(i64,)> = sqlx::query_as("SELECT version FROM rfqs WHERE id = $1")
                .bind(&id)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| RepositoryError::query(e.to_string()))?;

            if let Some((existing_version,)) = exists
                && existing_version >= version
            {
                return Err(RepositoryError::version_conflict(
                    "Rfq",
                    id,
                    version as u64,
                    existing_version as u64,
                ));
            }
        }

        Ok(())
    }

    async fn get(&self, id: &RfqId) -> RepositoryResult<Option<Rfq>> {
        let id_str = id.to_string();

        let row: Option<RfqRow> = sqlx::query_as(
            r#"
            SELECT id, client_id, instrument, side, quantity, state, expires_at,
                   quotes, selected_quote_id, compliance_result, failure_reason,
                   version, created_at, updated_at
            FROM rfqs WHERE id = $1
            "#,
        )
        .bind(&id_str)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RepositoryError::query(e.to_string()))?;

        row.map(|r| r.try_into_rfq()).transpose()
    }

    async fn find_active(&self) -> RepositoryResult<Vec<Rfq>> {
        let active_states = vec![
            RfqState::Created.to_string(),
            RfqState::QuoteRequesting.to_string(),
            RfqState::QuotesReceived.to_string(),
            RfqState::ClientSelecting.to_string(),
            RfqState::Executing.to_string(),
        ];

        let rows: Vec<RfqRow> = sqlx::query_as(
            r#"
            SELECT id, client_id, instrument, side, quantity, state, expires_at,
                   quotes, selected_quote_id, compliance_result, failure_reason,
                   version, created_at, updated_at
            FROM rfqs WHERE state = ANY($1)
            "#,
        )
        .bind(&active_states)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RepositoryError::query(e.to_string()))?;

        rows.into_iter().map(|r| r.try_into_rfq()).collect()
    }

    async fn find_by_client(&self, client_id: &CounterpartyId) -> RepositoryResult<Vec<Rfq>> {
        let client_id_str = client_id.as_str();

        let rows: Vec<RfqRow> = sqlx::query_as(
            r#"
            SELECT id, client_id, instrument, side, quantity, state, expires_at,
                   quotes, selected_quote_id, compliance_result, failure_reason,
                   version, created_at, updated_at
            FROM rfqs WHERE client_id = $1
            "#,
        )
        .bind(client_id_str)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RepositoryError::query(e.to_string()))?;

        rows.into_iter().map(|r| r.try_into_rfq()).collect()
    }

    async fn find_by_venue(&self, venue_id: &VenueId) -> RepositoryResult<Vec<Rfq>> {
        let venue_id_str = venue_id.as_str();

        // Search for RFQs where quotes array contains the venue_id
        let rows: Vec<RfqRow> = sqlx::query_as(
            r#"
            SELECT id, client_id, instrument, side, quantity, state, expires_at,
                   quotes, selected_quote_id, compliance_result, failure_reason,
                   version, created_at, updated_at
            FROM rfqs WHERE quotes @> $1::jsonb
            "#,
        )
        .bind(format!(r#"[{{"venue_id": "{}"}}]"#, venue_id_str))
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RepositoryError::query(e.to_string()))?;

        rows.into_iter().map(|r| r.try_into_rfq()).collect()
    }

    async fn delete(&self, id: &RfqId) -> RepositoryResult<bool> {
        let id_str = id.to_string();

        let result = sqlx::query("DELETE FROM rfqs WHERE id = $1")
            .bind(&id_str)
            .execute(&self.pool)
            .await
            .map_err(|e| RepositoryError::query(e.to_string()))?;

        Ok(result.rows_affected() > 0)
    }

    async fn count(&self) -> RepositoryResult<u64> {
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM rfqs")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| RepositoryError::query(e.to_string()))?;

        Ok(count as u64)
    }

    async fn count_active(&self) -> RepositoryResult<u64> {
        let active_states = vec![
            RfqState::Created.to_string(),
            RfqState::QuoteRequesting.to_string(),
            RfqState::QuotesReceived.to_string(),
            RfqState::ClientSelecting.to_string(),
            RfqState::Executing.to_string(),
        ];

        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM rfqs WHERE state = ANY($1)")
            .bind(&active_states)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| RepositoryError::query(e.to_string()))?;

        Ok(count as u64)
    }
}

/// Row type for RFQ queries.
#[derive(Debug, sqlx::FromRow)]
struct RfqRow {
    id: String,
    client_id: String,
    instrument: serde_json::Value,
    side: String,
    quantity: rust_decimal::Decimal,
    state: String,
    expires_at: i64,
    quotes: serde_json::Value,
    selected_quote_id: Option<String>,
    compliance_result: Option<serde_json::Value>,
    failure_reason: Option<String>,
    version: i64,
    created_at: i64,
    updated_at: i64,
}

impl RfqRow {
    /// Converts the row into an RFQ entity.
    fn try_into_rfq(self) -> RepositoryResult<Rfq> {
        use crate::domain::entities::ComplianceResult;
        use crate::domain::entities::quote::Quote;
        use crate::domain::value_objects::enums::OrderSide;
        use crate::domain::value_objects::timestamp::Timestamp;
        use crate::domain::value_objects::{Instrument, Quantity, QuoteId};
        use uuid::Uuid;

        let uuid =
            Uuid::parse_str(&self.id).map_err(|e| RepositoryError::serialization(e.to_string()))?;
        let id = RfqId::new(uuid);
        let client_id = CounterpartyId::new(&self.client_id);
        let instrument: Instrument = serde_json::from_value(self.instrument)
            .map_err(|e| RepositoryError::serialization(e.to_string()))?;
        let side: OrderSide = serde_json::from_str(&format!("\"{}\"", self.side))
            .map_err(|e| RepositoryError::serialization(e.to_string()))?;
        let quantity = Quantity::new(self.quantity.to_string().parse().unwrap_or(0.0))
            .map_err(|e| RepositoryError::serialization(e.to_string()))?;
        let state: RfqState = serde_json::from_str(&format!("\"{}\"", self.state))
            .map_err(|e| RepositoryError::serialization(e.to_string()))?;
        let expires_at = Timestamp::from_millis(self.expires_at).ok_or_else(|| {
            RepositoryError::serialization("invalid expires_at timestamp".to_string())
        })?;
        let quotes: Vec<Quote> = serde_json::from_value(self.quotes)
            .map_err(|e| RepositoryError::serialization(e.to_string()))?;
        let selected_quote_id = self
            .selected_quote_id
            .map(|s| Uuid::parse_str(&s).map(QuoteId::new))
            .transpose()
            .map_err(|e| RepositoryError::serialization(e.to_string()))?;
        let compliance_result: Option<ComplianceResult> = self
            .compliance_result
            .map(serde_json::from_value)
            .transpose()
            .map_err(|e| RepositoryError::serialization(e.to_string()))?;
        let created_at = Timestamp::from_millis(self.created_at).ok_or_else(|| {
            RepositoryError::serialization("invalid created_at timestamp".to_string())
        })?;
        let updated_at = Timestamp::from_millis(self.updated_at).ok_or_else(|| {
            RepositoryError::serialization("invalid updated_at timestamp".to_string())
        })?;

        Ok(Rfq::from_parts(
            id,
            client_id,
            instrument,
            side,
            quantity,
            state,
            expires_at,
            quotes,
            selected_quote_id,
            compliance_result,
            self.failure_reason,
            self.version as u64,
            created_at,
            updated_at,
        ))
    }
}
