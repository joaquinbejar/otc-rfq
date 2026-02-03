//! # PostgreSQL Counterparty Repository
//!
//! PostgreSQL implementation of [`CounterpartyRepository`] using sqlx.

use crate::domain::entities::counterparty::Counterparty;
use crate::domain::value_objects::CounterpartyId;
use crate::infrastructure::persistence::traits::{
    CounterpartyRepository, RepositoryError, RepositoryResult,
};
use async_trait::async_trait;
use sqlx::PgPool;

/// PostgreSQL implementation of [`CounterpartyRepository`].
///
/// Uses connection pooling via `sqlx::PgPool`.
#[derive(Debug, Clone)]
pub struct PostgresCounterpartyRepository {
    pool: PgPool,
}

impl PostgresCounterpartyRepository {
    /// Creates a new PostgreSQL counterparty repository.
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
impl CounterpartyRepository for PostgresCounterpartyRepository {
    async fn save(&self, counterparty: &Counterparty) -> RepositoryResult<()> {
        let id = counterparty.id().as_str();
        let name = counterparty.name();
        let counterparty_type = counterparty.counterparty_type().to_string();
        let kyc_status = counterparty.kyc_status().to_string();
        let limits = serde_json::to_value(counterparty.limits())
            .map_err(|e| RepositoryError::serialization(e.to_string()))?;
        let wallet_addresses = serde_json::to_value(counterparty.wallet_addresses())
            .map_err(|e| RepositoryError::serialization(e.to_string()))?;
        let active = counterparty.is_active();
        let created_at = counterparty.created_at().timestamp_millis();
        let updated_at = counterparty.updated_at().timestamp_millis();

        sqlx::query(
            r#"
            INSERT INTO counterparties (
                id, name, counterparty_type, kyc_status, limits,
                wallet_addresses, active, created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            ON CONFLICT (id) DO UPDATE SET
                name = EXCLUDED.name,
                counterparty_type = EXCLUDED.counterparty_type,
                kyc_status = EXCLUDED.kyc_status,
                limits = EXCLUDED.limits,
                wallet_addresses = EXCLUDED.wallet_addresses,
                active = EXCLUDED.active,
                updated_at = EXCLUDED.updated_at
            "#,
        )
        .bind(id)
        .bind(name)
        .bind(&counterparty_type)
        .bind(&kyc_status)
        .bind(&limits)
        .bind(&wallet_addresses)
        .bind(active)
        .bind(created_at)
        .bind(updated_at)
        .execute(&self.pool)
        .await
        .map_err(|e| RepositoryError::query(e.to_string()))?;

        Ok(())
    }

    async fn get(&self, id: &CounterpartyId) -> RepositoryResult<Option<Counterparty>> {
        let id_str = id.as_str();

        let row: Option<CounterpartyRow> = sqlx::query_as(
            r#"
            SELECT id, name, counterparty_type, kyc_status, limits,
                   wallet_addresses, active, created_at, updated_at
            FROM counterparties WHERE id = $1
            "#,
        )
        .bind(id_str)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RepositoryError::query(e.to_string()))?;

        row.map(|r| r.try_into_counterparty()).transpose()
    }

    async fn get_all(&self) -> RepositoryResult<Vec<Counterparty>> {
        let rows: Vec<CounterpartyRow> = sqlx::query_as(
            r#"
            SELECT id, name, counterparty_type, kyc_status, limits,
                   wallet_addresses, active, created_at, updated_at
            FROM counterparties ORDER BY name ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RepositoryError::query(e.to_string()))?;

        rows.into_iter()
            .map(|r| r.try_into_counterparty())
            .collect()
    }

    async fn find_active(&self) -> RepositoryResult<Vec<Counterparty>> {
        let rows: Vec<CounterpartyRow> = sqlx::query_as(
            r#"
            SELECT id, name, counterparty_type, kyc_status, limits,
                   wallet_addresses, active, created_at, updated_at
            FROM counterparties WHERE active = true ORDER BY name ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RepositoryError::query(e.to_string()))?;

        rows.into_iter()
            .map(|r| r.try_into_counterparty())
            .collect()
    }

    async fn find_by_name(&self, name: &str) -> RepositoryResult<Vec<Counterparty>> {
        let pattern = format!("%{}%", name.to_lowercase());

        let rows: Vec<CounterpartyRow> = sqlx::query_as(
            r#"
            SELECT id, name, counterparty_type, kyc_status, limits,
                   wallet_addresses, active, created_at, updated_at
            FROM counterparties WHERE LOWER(name) LIKE $1 ORDER BY name ASC
            "#,
        )
        .bind(&pattern)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RepositoryError::query(e.to_string()))?;

        rows.into_iter()
            .map(|r| r.try_into_counterparty())
            .collect()
    }

    async fn delete(&self, id: &CounterpartyId) -> RepositoryResult<bool> {
        let id_str = id.as_str();

        let result = sqlx::query("DELETE FROM counterparties WHERE id = $1")
            .bind(id_str)
            .execute(&self.pool)
            .await
            .map_err(|e| RepositoryError::query(e.to_string()))?;

        Ok(result.rows_affected() > 0)
    }

    async fn count(&self) -> RepositoryResult<u64> {
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM counterparties")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| RepositoryError::query(e.to_string()))?;

        Ok(count as u64)
    }

    async fn count_active(&self) -> RepositoryResult<u64> {
        let (count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM counterparties WHERE active = true")
                .fetch_one(&self.pool)
                .await
                .map_err(|e| RepositoryError::query(e.to_string()))?;

        Ok(count as u64)
    }
}

/// Row type for counterparty queries.
#[derive(Debug, sqlx::FromRow)]
struct CounterpartyRow {
    id: String,
    name: String,
    counterparty_type: String,
    kyc_status: String,
    limits: serde_json::Value,
    wallet_addresses: serde_json::Value,
    active: bool,
    created_at: i64,
    updated_at: i64,
}

impl CounterpartyRow {
    /// Converts the row into a Counterparty entity.
    fn try_into_counterparty(self) -> RepositoryResult<Counterparty> {
        use crate::domain::entities::CounterpartyType;
        use crate::domain::entities::counterparty::{CounterpartyLimits, KycStatus, WalletAddress};
        use crate::domain::value_objects::timestamp::Timestamp;

        let id = CounterpartyId::new(&self.id);
        let counterparty_type: CounterpartyType =
            serde_json::from_str(&format!("\"{}\"", self.counterparty_type))
                .map_err(|e| RepositoryError::serialization(e.to_string()))?;
        let kyc_status: KycStatus = serde_json::from_str(&format!("\"{}\"", self.kyc_status))
            .map_err(|e| RepositoryError::serialization(e.to_string()))?;
        let limits: CounterpartyLimits = serde_json::from_value(self.limits)
            .map_err(|e| RepositoryError::serialization(e.to_string()))?;
        let wallet_addresses: Vec<WalletAddress> = serde_json::from_value(self.wallet_addresses)
            .map_err(|e| RepositoryError::serialization(e.to_string()))?;
        let created_at = Timestamp::from_millis(self.created_at).ok_or_else(|| {
            RepositoryError::serialization("invalid created_at timestamp".to_string())
        })?;
        let updated_at = Timestamp::from_millis(self.updated_at).ok_or_else(|| {
            RepositoryError::serialization("invalid updated_at timestamp".to_string())
        })?;

        Ok(Counterparty::from_parts(
            id,
            self.name,
            counterparty_type,
            kyc_status,
            limits,
            wallet_addresses,
            self.active,
            created_at,
            updated_at,
        ))
    }
}
