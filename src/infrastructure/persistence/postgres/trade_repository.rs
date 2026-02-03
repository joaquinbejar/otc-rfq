//! # PostgreSQL Trade Repository
//!
//! PostgreSQL implementation of [`TradeRepository`] using sqlx.
//!
//! This implementation uses PostgreSQL with optimistic locking via version fields.

use crate::domain::entities::SettlementState;
use crate::domain::entities::trade::Trade;
use crate::domain::value_objects::{RfqId, TradeId, VenueId};
use crate::infrastructure::persistence::traits::{
    RepositoryError, RepositoryResult, TradeRepository,
};
use async_trait::async_trait;
use sqlx::PgPool;

/// PostgreSQL implementation of [`TradeRepository`].
///
/// Uses connection pooling via `sqlx::PgPool` and optimistic locking.
#[derive(Debug, Clone)]
pub struct PostgresTradeRepository {
    pool: PgPool,
}

impl PostgresTradeRepository {
    /// Creates a new PostgreSQL trade repository.
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
impl TradeRepository for PostgresTradeRepository {
    async fn save(&self, trade: &Trade) -> RepositoryResult<()> {
        let id = trade.id().to_string();
        let rfq_id = trade.rfq_id().to_string();
        let quote_id = trade.quote_id().to_string();
        let venue_id = trade.venue_id().as_str();
        let price = trade.price().get();
        let quantity = trade.quantity().get();
        let venue_execution_ref = trade.venue_execution_ref().map(|s| s.to_string());
        let settlement_state = trade.settlement_state().to_string();
        let settlement_tx_ref = trade.settlement_tx_ref().map(|s| s.to_string());
        let failure_reason = trade.failure_reason().map(|s| s.to_string());
        let version = trade.version() as i64;
        let created_at = trade.created_at().timestamp_millis();
        let updated_at = trade.updated_at().timestamp_millis();

        let result = sqlx::query(
            r#"
            INSERT INTO trades (
                id, rfq_id, quote_id, venue_id, price, quantity,
                venue_execution_ref, settlement_state, settlement_tx_ref,
                failure_reason, version, created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            ON CONFLICT (id) DO UPDATE SET
                rfq_id = EXCLUDED.rfq_id,
                quote_id = EXCLUDED.quote_id,
                venue_id = EXCLUDED.venue_id,
                price = EXCLUDED.price,
                quantity = EXCLUDED.quantity,
                venue_execution_ref = EXCLUDED.venue_execution_ref,
                settlement_state = EXCLUDED.settlement_state,
                settlement_tx_ref = EXCLUDED.settlement_tx_ref,
                failure_reason = EXCLUDED.failure_reason,
                version = EXCLUDED.version,
                updated_at = EXCLUDED.updated_at
            WHERE trades.version < EXCLUDED.version
            "#,
        )
        .bind(&id)
        .bind(&rfq_id)
        .bind(&quote_id)
        .bind(venue_id)
        .bind(price)
        .bind(quantity)
        .bind(&venue_execution_ref)
        .bind(&settlement_state)
        .bind(&settlement_tx_ref)
        .bind(&failure_reason)
        .bind(version)
        .bind(created_at)
        .bind(updated_at)
        .execute(&self.pool)
        .await
        .map_err(|e| RepositoryError::query(e.to_string()))?;

        if result.rows_affected() == 0 {
            let exists: Option<(i64,)> = sqlx::query_as("SELECT version FROM trades WHERE id = $1")
                .bind(&id)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| RepositoryError::query(e.to_string()))?;

            if let Some((existing_version,)) = exists
                && existing_version >= version
            {
                return Err(RepositoryError::version_conflict(
                    "Trade",
                    id,
                    version as u64,
                    existing_version as u64,
                ));
            }
        }

        Ok(())
    }

    async fn get(&self, id: &TradeId) -> RepositoryResult<Option<Trade>> {
        let id_str = id.to_string();

        let row: Option<TradeRow> = sqlx::query_as(
            r#"
            SELECT id, rfq_id, quote_id, venue_id, price, quantity,
                   venue_execution_ref, settlement_state, settlement_tx_ref,
                   failure_reason, version, created_at, updated_at
            FROM trades WHERE id = $1
            "#,
        )
        .bind(&id_str)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RepositoryError::query(e.to_string()))?;

        row.map(|r| r.try_into_trade()).transpose()
    }

    async fn get_by_rfq(&self, rfq_id: &RfqId) -> RepositoryResult<Option<Trade>> {
        let rfq_id_str = rfq_id.to_string();

        let row: Option<TradeRow> = sqlx::query_as(
            r#"
            SELECT id, rfq_id, quote_id, venue_id, price, quantity,
                   venue_execution_ref, settlement_state, settlement_tx_ref,
                   failure_reason, version, created_at, updated_at
            FROM trades WHERE rfq_id = $1
            "#,
        )
        .bind(&rfq_id_str)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RepositoryError::query(e.to_string()))?;

        row.map(|r| r.try_into_trade()).transpose()
    }

    async fn find_pending_settlement(&self) -> RepositoryResult<Vec<Trade>> {
        let state = SettlementState::Pending.to_string();

        let rows: Vec<TradeRow> = sqlx::query_as(
            r#"
            SELECT id, rfq_id, quote_id, venue_id, price, quantity,
                   venue_execution_ref, settlement_state, settlement_tx_ref,
                   failure_reason, version, created_at, updated_at
            FROM trades WHERE settlement_state = $1
            "#,
        )
        .bind(&state)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RepositoryError::query(e.to_string()))?;

        rows.into_iter().map(|r| r.try_into_trade()).collect()
    }

    async fn find_by_venue(&self, venue_id: &VenueId) -> RepositoryResult<Vec<Trade>> {
        let venue_id_str = venue_id.as_str();

        let rows: Vec<TradeRow> = sqlx::query_as(
            r#"
            SELECT id, rfq_id, quote_id, venue_id, price, quantity,
                   venue_execution_ref, settlement_state, settlement_tx_ref,
                   failure_reason, version, created_at, updated_at
            FROM trades WHERE venue_id = $1
            "#,
        )
        .bind(venue_id_str)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RepositoryError::query(e.to_string()))?;

        rows.into_iter().map(|r| r.try_into_trade()).collect()
    }

    async fn find_settled(&self) -> RepositoryResult<Vec<Trade>> {
        let state = SettlementState::Settled.to_string();

        let rows: Vec<TradeRow> = sqlx::query_as(
            r#"
            SELECT id, rfq_id, quote_id, venue_id, price, quantity,
                   venue_execution_ref, settlement_state, settlement_tx_ref,
                   failure_reason, version, created_at, updated_at
            FROM trades WHERE settlement_state = $1
            "#,
        )
        .bind(&state)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RepositoryError::query(e.to_string()))?;

        rows.into_iter().map(|r| r.try_into_trade()).collect()
    }

    async fn find_failed(&self) -> RepositoryResult<Vec<Trade>> {
        let state = SettlementState::Failed.to_string();

        let rows: Vec<TradeRow> = sqlx::query_as(
            r#"
            SELECT id, rfq_id, quote_id, venue_id, price, quantity,
                   venue_execution_ref, settlement_state, settlement_tx_ref,
                   failure_reason, version, created_at, updated_at
            FROM trades WHERE settlement_state = $1
            "#,
        )
        .bind(&state)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RepositoryError::query(e.to_string()))?;

        rows.into_iter().map(|r| r.try_into_trade()).collect()
    }

    async fn delete(&self, id: &TradeId) -> RepositoryResult<bool> {
        let id_str = id.to_string();

        let result = sqlx::query("DELETE FROM trades WHERE id = $1")
            .bind(&id_str)
            .execute(&self.pool)
            .await
            .map_err(|e| RepositoryError::query(e.to_string()))?;

        Ok(result.rows_affected() > 0)
    }

    async fn count(&self) -> RepositoryResult<u64> {
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM trades")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| RepositoryError::query(e.to_string()))?;

        Ok(count as u64)
    }

    async fn count_pending_settlement(&self) -> RepositoryResult<u64> {
        let state = SettlementState::Pending.to_string();

        let (count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM trades WHERE settlement_state = $1")
                .bind(&state)
                .fetch_one(&self.pool)
                .await
                .map_err(|e| RepositoryError::query(e.to_string()))?;

        Ok(count as u64)
    }
}

/// Row type for trade queries.
#[derive(Debug, sqlx::FromRow)]
struct TradeRow {
    id: String,
    rfq_id: String,
    quote_id: String,
    venue_id: String,
    price: rust_decimal::Decimal,
    quantity: rust_decimal::Decimal,
    venue_execution_ref: Option<String>,
    settlement_state: String,
    settlement_tx_ref: Option<String>,
    failure_reason: Option<String>,
    version: i64,
    created_at: i64,
    updated_at: i64,
}

impl TradeRow {
    /// Converts the row into a Trade entity.
    fn try_into_trade(self) -> RepositoryResult<Trade> {
        use crate::domain::value_objects::timestamp::Timestamp;
        use crate::domain::value_objects::{Price, Quantity, QuoteId};
        use uuid::Uuid;

        let id_uuid =
            Uuid::parse_str(&self.id).map_err(|e| RepositoryError::serialization(e.to_string()))?;
        let id = TradeId::new(id_uuid);
        let rfq_uuid = Uuid::parse_str(&self.rfq_id)
            .map_err(|e| RepositoryError::serialization(e.to_string()))?;
        let rfq_id = RfqId::new(rfq_uuid);
        let quote_uuid = Uuid::parse_str(&self.quote_id)
            .map_err(|e| RepositoryError::serialization(e.to_string()))?;
        let quote_id = QuoteId::new(quote_uuid);
        let venue_id = VenueId::new(&self.venue_id);
        let price = Price::new(self.price.to_string().parse().unwrap_or(0.0))
            .map_err(|e| RepositoryError::serialization(e.to_string()))?;
        let quantity = Quantity::new(self.quantity.to_string().parse().unwrap_or(0.0))
            .map_err(|e| RepositoryError::serialization(e.to_string()))?;
        let settlement_state: SettlementState =
            serde_json::from_str(&format!("\"{}\"", self.settlement_state))
                .map_err(|e| RepositoryError::serialization(e.to_string()))?;
        let created_at = Timestamp::from_millis(self.created_at).ok_or_else(|| {
            RepositoryError::serialization("invalid created_at timestamp".to_string())
        })?;
        let updated_at = Timestamp::from_millis(self.updated_at).ok_or_else(|| {
            RepositoryError::serialization("invalid updated_at timestamp".to_string())
        })?;

        Ok(Trade::from_parts(
            id,
            rfq_id,
            quote_id,
            venue_id,
            price,
            quantity,
            self.venue_execution_ref,
            settlement_state,
            self.settlement_tx_ref,
            self.failure_reason,
            self.version as u64,
            created_at,
            updated_at,
        ))
    }
}
