//! # PostgreSQL Venue Repository
//!
//! PostgreSQL implementation of [`VenueRepository`] using sqlx.

use crate::domain::value_objects::VenueId;
use crate::infrastructure::persistence::traits::{RepositoryResult, VenueRepository};
use crate::infrastructure::venues::registry::VenueConfig;
use async_trait::async_trait;
use sqlx::PgPool;

/// PostgreSQL implementation of [`VenueRepository`].
///
/// Uses connection pooling via `sqlx::PgPool`.
#[derive(Debug, Clone)]
pub struct PostgresVenueRepository {
    pool: PgPool,
}

impl PostgresVenueRepository {
    /// Creates a new PostgreSQL venue repository.
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
impl VenueRepository for PostgresVenueRepository {
    async fn save(&self, config: &VenueConfig) -> RepositoryResult<()> {
        use crate::infrastructure::persistence::traits::RepositoryError;

        let venue_id = config.venue_id().as_str();
        let enabled = config.is_enabled();
        let priority = config.priority() as i32;
        let supported_instruments = serde_json::to_value(config.supported_instruments())
            .map_err(|e| RepositoryError::serialization(e.to_string()))?;

        sqlx::query(
            r#"
            INSERT INTO venues (venue_id, enabled, priority, supported_instruments)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (venue_id) DO UPDATE SET
                enabled = EXCLUDED.enabled,
                priority = EXCLUDED.priority,
                supported_instruments = EXCLUDED.supported_instruments
            "#,
        )
        .bind(venue_id)
        .bind(enabled)
        .bind(priority)
        .bind(&supported_instruments)
        .execute(&self.pool)
        .await
        .map_err(|e| RepositoryError::query(e.to_string()))?;

        Ok(())
    }

    async fn get(&self, id: &VenueId) -> RepositoryResult<Option<VenueConfig>> {
        use crate::infrastructure::persistence::traits::RepositoryError;

        let venue_id = id.as_str();

        let row: Option<VenueRow> = sqlx::query_as(
            r#"
            SELECT venue_id, enabled, priority, supported_instruments
            FROM venues WHERE venue_id = $1
            "#,
        )
        .bind(venue_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RepositoryError::query(e.to_string()))?;

        row.map(|r| r.try_into_config()).transpose()
    }

    async fn get_all(&self) -> RepositoryResult<Vec<VenueConfig>> {
        use crate::infrastructure::persistence::traits::RepositoryError;

        let rows: Vec<VenueRow> = sqlx::query_as(
            r#"
            SELECT venue_id, enabled, priority, supported_instruments
            FROM venues ORDER BY priority ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RepositoryError::query(e.to_string()))?;

        rows.into_iter().map(|r| r.try_into_config()).collect()
    }

    async fn find_enabled(&self) -> RepositoryResult<Vec<VenueConfig>> {
        use crate::infrastructure::persistence::traits::RepositoryError;

        let rows: Vec<VenueRow> = sqlx::query_as(
            r#"
            SELECT venue_id, enabled, priority, supported_instruments
            FROM venues WHERE enabled = true ORDER BY priority ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RepositoryError::query(e.to_string()))?;

        rows.into_iter().map(|r| r.try_into_config()).collect()
    }

    async fn delete(&self, id: &VenueId) -> RepositoryResult<bool> {
        use crate::infrastructure::persistence::traits::RepositoryError;

        let venue_id = id.as_str();

        let result = sqlx::query("DELETE FROM venues WHERE venue_id = $1")
            .bind(venue_id)
            .execute(&self.pool)
            .await
            .map_err(|e| RepositoryError::query(e.to_string()))?;

        Ok(result.rows_affected() > 0)
    }

    async fn count(&self) -> RepositoryResult<u64> {
        use crate::infrastructure::persistence::traits::RepositoryError;

        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM venues")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| RepositoryError::query(e.to_string()))?;

        Ok(count as u64)
    }
}

/// Row type for venue queries.
#[derive(Debug, sqlx::FromRow)]
struct VenueRow {
    venue_id: String,
    enabled: bool,
    priority: i32,
    supported_instruments: serde_json::Value,
}

impl VenueRow {
    /// Converts the row into a VenueConfig.
    fn try_into_config(self) -> RepositoryResult<VenueConfig> {
        use crate::domain::value_objects::Instrument;
        use crate::infrastructure::persistence::traits::RepositoryError;

        let venue_id = VenueId::new(&self.venue_id);
        let supported_instruments: Vec<Instrument> =
            serde_json::from_value(self.supported_instruments)
                .map_err(|e| RepositoryError::serialization(e.to_string()))?;

        let config = VenueConfig::new(venue_id)
            .with_enabled(self.enabled)
            .with_priority(self.priority as u32)
            .with_instruments(supported_instruments);

        Ok(config)
    }
}
