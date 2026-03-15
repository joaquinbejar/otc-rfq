//! # Repository Traits
//!
//! Port definitions for persistence abstraction.
//!
//! This module defines the repository traits (ports) that abstract
//! persistence operations. Implementations can use different backends
//! like PostgreSQL, in-memory storage, or event sourcing.
//!
//! # Available Repositories
//!
//! - [`RfqRepository`]: Persistence for RFQ entities
//! - [`TradeRepository`]: Persistence for Trade entities
//! - [`VenueRepository`]: Persistence for venue configurations
//! - [`CounterpartyRepository`]: Persistence for counterparty data
//!
//! # Examples
//!
//! ```ignore
//! use otc_rfq::infrastructure::persistence::traits::RfqRepository;
//!
//! async fn find_active_rfqs(repo: &impl RfqRepository) {
//!     let active = repo.find_active().await.unwrap();
//!     println!("Found {} active RFQs", active.len());
//! }
//! ```

use crate::domain::entities::anonymity::IdentityMapping;
use crate::domain::entities::counterparty::Counterparty;
use crate::domain::entities::rfq::Rfq;
use crate::domain::entities::trade::Trade;
use crate::domain::value_objects::{CounterpartyId, RfqId, TradeId, VenueId};
use crate::infrastructure::venues::registry::VenueConfig;
use async_trait::async_trait;
use std::fmt;
use thiserror::Error;

/// Error type for repository operations.
#[derive(Debug, Error)]
pub enum RepositoryError {
    /// Entity not found.
    #[error("Entity not found: {entity_type} with id {id}")]
    NotFound {
        /// Type of entity.
        entity_type: &'static str,
        /// Entity identifier.
        id: String,
    },

    /// Duplicate entity.
    #[error("Duplicate entity: {entity_type} with id {id} already exists")]
    Duplicate {
        /// Type of entity.
        entity_type: &'static str,
        /// Entity identifier.
        id: String,
    },

    /// Optimistic locking conflict.
    #[error("Version conflict: {entity_type} with id {id} has been modified")]
    VersionConflict {
        /// Type of entity.
        entity_type: &'static str,
        /// Entity identifier.
        id: String,
        /// Expected version.
        expected: u64,
        /// Actual version.
        actual: u64,
    },

    /// Connection error.
    #[error("Connection error: {0}")]
    Connection(String),

    /// Query error.
    #[error("Query error: {0}")]
    Query(String),

    /// Serialization error.
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// Internal error.
    #[error("Internal error: {0}")]
    Internal(String),
}

impl RepositoryError {
    /// Creates a not found error.
    #[must_use]
    pub fn not_found(entity_type: &'static str, id: impl Into<String>) -> Self {
        Self::NotFound {
            entity_type,
            id: id.into(),
        }
    }

    /// Creates a duplicate error.
    #[must_use]
    pub fn duplicate(entity_type: &'static str, id: impl Into<String>) -> Self {
        Self::Duplicate {
            entity_type,
            id: id.into(),
        }
    }

    /// Creates a version conflict error.
    #[must_use]
    pub fn version_conflict(
        entity_type: &'static str,
        id: impl Into<String>,
        expected: u64,
        actual: u64,
    ) -> Self {
        Self::VersionConflict {
            entity_type,
            id: id.into(),
            expected,
            actual,
        }
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

    /// Creates a serialization error.
    #[must_use]
    pub fn serialization(msg: impl Into<String>) -> Self {
        Self::Serialization(msg.into())
    }

    /// Creates an internal error.
    #[must_use]
    pub fn internal(msg: impl Into<String>) -> Self {
        Self::Internal(msg.into())
    }

    /// Returns true if this is a not found error.
    #[must_use]
    pub fn is_not_found(&self) -> bool {
        matches!(self, Self::NotFound { .. })
    }

    /// Returns true if this is a duplicate error.
    #[must_use]
    pub fn is_duplicate(&self) -> bool {
        matches!(self, Self::Duplicate { .. })
    }

    /// Returns true if this is a version conflict error.
    #[must_use]
    pub fn is_version_conflict(&self) -> bool {
        matches!(self, Self::VersionConflict { .. })
    }
}

/// Result type for repository operations.
pub type RepositoryResult<T> = Result<T, RepositoryError>;

/// Repository for RFQ entities.
///
/// Provides persistence operations for RFQ (Request for Quote) entities.
///
/// # Examples
///
/// ```ignore
/// use otc_rfq::infrastructure::persistence::traits::RfqRepository;
///
/// async fn example(repo: &impl RfqRepository) {
///     // Find all active RFQs
///     let active = repo.find_active().await?;
///     
///     // Find RFQs for a specific client
///     let client_rfqs = repo.find_by_client(&client_id).await?;
/// }
/// ```
#[async_trait]
pub trait RfqRepository: Send + Sync + fmt::Debug {
    /// Saves an RFQ.
    ///
    /// If the RFQ already exists, it will be updated.
    /// Uses optimistic locking via the version field.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::VersionConflict` if the RFQ has been
    /// modified since it was loaded.
    async fn save(&self, rfq: &Rfq) -> RepositoryResult<()>;

    /// Gets an RFQ by ID.
    ///
    /// Returns `None` if the RFQ does not exist.
    async fn get(&self, id: &RfqId) -> RepositoryResult<Option<Rfq>>;

    /// Finds all active RFQs.
    ///
    /// Active RFQs are those in states that can still receive quotes
    /// or be executed (not cancelled, expired, or completed).
    async fn find_active(&self) -> RepositoryResult<Vec<Rfq>>;

    /// Finds RFQs by client ID.
    ///
    /// Returns all RFQs created by the specified client.
    async fn find_by_client(&self, client_id: &CounterpartyId) -> RepositoryResult<Vec<Rfq>>;

    /// Finds RFQs by venue ID.
    ///
    /// Returns all RFQs that have been sent to the specified venue.
    async fn find_by_venue(&self, venue_id: &VenueId) -> RepositoryResult<Vec<Rfq>>;

    /// Deletes an RFQ by ID.
    ///
    /// Returns `Ok(true)` if the RFQ was deleted, `Ok(false)` if it didn't exist.
    async fn delete(&self, id: &RfqId) -> RepositoryResult<bool>;

    /// Counts all RFQs.
    async fn count(&self) -> RepositoryResult<u64>;

    /// Counts active RFQs.
    async fn count_active(&self) -> RepositoryResult<u64>;
}

/// Repository for Trade entities.
///
/// Provides persistence operations for Trade entities.
///
/// # Examples
///
/// ```ignore
/// use otc_rfq::infrastructure::persistence::traits::TradeRepository;
///
/// async fn example(repo: &impl TradeRepository) {
///     // Find trades pending settlement
///     let pending = repo.find_pending_settlement().await?;
///     
///     // Get a specific trade
///     let trade = repo.get(&trade_id).await?;
/// }
/// ```
#[async_trait]
pub trait TradeRepository: Send + Sync + fmt::Debug {
    /// Saves a trade.
    ///
    /// If the trade already exists, it will be updated.
    /// Uses optimistic locking via the version field.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::VersionConflict` if the trade has been
    /// modified since it was loaded.
    async fn save(&self, trade: &Trade) -> RepositoryResult<()>;

    /// Gets a trade by ID.
    ///
    /// Returns `None` if the trade does not exist.
    async fn get(&self, id: &TradeId) -> RepositoryResult<Option<Trade>>;

    /// Gets a trade by RFQ ID.
    ///
    /// Returns the trade associated with the specified RFQ, if any.
    async fn get_by_rfq(&self, rfq_id: &RfqId) -> RepositoryResult<Option<Trade>>;

    /// Finds trades pending settlement.
    ///
    /// Returns all trades in the `Pending` settlement state.
    async fn find_pending_settlement(&self) -> RepositoryResult<Vec<Trade>>;

    /// Finds trades by venue ID.
    ///
    /// Returns all trades executed at the specified venue.
    async fn find_by_venue(&self, venue_id: &VenueId) -> RepositoryResult<Vec<Trade>>;

    /// Finds settled trades.
    ///
    /// Returns all trades in the `Settled` settlement state.
    async fn find_settled(&self) -> RepositoryResult<Vec<Trade>>;

    /// Finds failed trades.
    ///
    /// Returns all trades in the `Failed` settlement state.
    async fn find_failed(&self) -> RepositoryResult<Vec<Trade>>;

    /// Deletes a trade by ID.
    ///
    /// Returns `Ok(true)` if the trade was deleted, `Ok(false)` if it didn't exist.
    async fn delete(&self, id: &TradeId) -> RepositoryResult<bool>;

    /// Counts all trades.
    async fn count(&self) -> RepositoryResult<u64>;

    /// Counts trades pending settlement.
    async fn count_pending_settlement(&self) -> RepositoryResult<u64>;
}

/// Repository for venue configurations.
///
/// Provides persistence operations for venue configuration data.
///
/// # Examples
///
/// ```ignore
/// use otc_rfq::infrastructure::persistence::traits::VenueRepository;
///
/// async fn example(repo: &impl VenueRepository) {
///     // Get all enabled venues
///     let enabled = repo.find_enabled().await?;
///     
///     // Get a specific venue config
///     let config = repo.get(&venue_id).await?;
/// }
/// ```
#[async_trait]
pub trait VenueRepository: Send + Sync + fmt::Debug {
    /// Saves a venue configuration.
    ///
    /// If the venue already exists, it will be updated.
    async fn save(&self, config: &VenueConfig) -> RepositoryResult<()>;

    /// Gets a venue configuration by ID.
    ///
    /// Returns `None` if the venue does not exist.
    async fn get(&self, id: &VenueId) -> RepositoryResult<Option<VenueConfig>>;

    /// Gets all venue configurations.
    async fn get_all(&self) -> RepositoryResult<Vec<VenueConfig>>;

    /// Finds enabled venue configurations.
    async fn find_enabled(&self) -> RepositoryResult<Vec<VenueConfig>>;

    /// Deletes a venue configuration by ID.
    ///
    /// Returns `Ok(true)` if the venue was deleted, `Ok(false)` if it didn't exist.
    async fn delete(&self, id: &VenueId) -> RepositoryResult<bool>;

    /// Counts all venues.
    async fn count(&self) -> RepositoryResult<u64>;
}

/// Repository for counterparty data.
///
/// Provides persistence operations for counterparty entities.
///
/// # Examples
///
/// ```ignore
/// use otc_rfq::infrastructure::persistence::traits::CounterpartyRepository;
///
/// async fn example(repo: &impl CounterpartyRepository) {
///     // Get all active counterparties
///     let active = repo.find_active().await?;
///     
///     // Get a specific counterparty
///     let cp = repo.get(&counterparty_id).await?;
/// }
/// ```
#[async_trait]
pub trait CounterpartyRepository: Send + Sync + fmt::Debug {
    /// Saves a counterparty.
    ///
    /// If the counterparty already exists, it will be updated.
    async fn save(&self, counterparty: &Counterparty) -> RepositoryResult<()>;

    /// Gets a counterparty by ID.
    ///
    /// Returns `None` if the counterparty does not exist.
    async fn get(&self, id: &CounterpartyId) -> RepositoryResult<Option<Counterparty>>;

    /// Gets all counterparties.
    async fn get_all(&self) -> RepositoryResult<Vec<Counterparty>>;

    /// Finds active counterparties.
    ///
    /// Active counterparties are those that can currently trade.
    async fn find_active(&self) -> RepositoryResult<Vec<Counterparty>>;

    /// Finds counterparties by name (partial match).
    async fn find_by_name(&self, name: &str) -> RepositoryResult<Vec<Counterparty>>;

    /// Deletes a counterparty by ID.
    ///
    /// Returns `Ok(true)` if the counterparty was deleted, `Ok(false)` if it didn't exist.
    async fn delete(&self, id: &CounterpartyId) -> RepositoryResult<bool>;

    /// Counts all counterparties.
    async fn count(&self) -> RepositoryResult<u64>;

    /// Counts active counterparties.
    async fn count_active(&self) -> RepositoryResult<u64>;
}

/// Repository for delayed trade reports.
///
/// Provides persistence operations for delayed report entities,
/// ensuring reports survive process restarts.
///
/// # Examples
///
/// ```ignore
/// use otc_rfq::infrastructure::persistence::traits::DelayedReportRepository;
///
/// async fn example(repo: &impl DelayedReportRepository) {
///     // Get reports ready to publish
///     let ready = repo.find_ready_to_publish(Timestamp::now()).await?;
///     
///     // Mark as published
///     for report in ready {
///         repo.mark_published(&report.id(), Timestamp::now()).await?;
///     }
/// }
/// ```
#[async_trait]
pub trait DelayedReportRepository: Send + Sync + fmt::Debug {
    /// Saves a delayed report.
    ///
    /// If the report already exists (by ID), it will be updated.
    async fn save(
        &self,
        report: &crate::domain::entities::delayed_report::DelayedReport,
    ) -> RepositoryResult<()>;

    /// Gets a delayed report by ID.
    ///
    /// Returns `None` if the report does not exist.
    async fn find_by_id(
        &self,
        id: &uuid::Uuid,
    ) -> RepositoryResult<Option<crate::domain::entities::delayed_report::DelayedReport>>;

    /// Gets a delayed report by trade ID.
    ///
    /// Returns `None` if no report exists for the trade.
    async fn find_by_trade_id(
        &self,
        trade_id: &str,
    ) -> RepositoryResult<Option<crate::domain::entities::delayed_report::DelayedReport>>;

    /// Finds all pending (unpublished) reports.
    async fn find_pending(
        &self,
    ) -> RepositoryResult<Vec<crate::domain::entities::delayed_report::DelayedReport>>;

    /// Finds reports that are ready to be published.
    ///
    /// A report is ready when its `publish_at` time has passed
    /// and it has not yet been published.
    async fn find_ready_to_publish(
        &self,
        now: crate::domain::value_objects::Timestamp,
    ) -> RepositoryResult<Vec<crate::domain::entities::delayed_report::DelayedReport>>;

    /// Marks a report as published.
    ///
    /// # Errors
    ///
    /// Returns `NotFound` if the report does not exist.
    async fn mark_published(
        &self,
        id: &uuid::Uuid,
        published_at: crate::domain::value_objects::Timestamp,
    ) -> RepositoryResult<()>;

    /// Deletes a report by ID.
    ///
    /// Returns `Ok(true)` if the report was deleted, `Ok(false)` if it didn't exist.
    async fn delete(&self, id: &uuid::Uuid) -> RepositoryResult<bool>;

    /// Counts all reports.
    async fn count(&self) -> RepositoryResult<u64>;

    /// Counts pending reports.
    async fn count_pending(&self) -> RepositoryResult<u64>;
}

/// Repository for identity mappings.
///
/// Provides persistence operations for anonymous RFQ identity mappings.
/// This repository is critical for compliance and settlement, maintaining
/// the link between anonymous RFQs and actual requester identities.
///
/// # Security
///
/// Access to this repository should be restricted to authorized services only.
/// All operations should be logged for audit purposes.
///
/// # Examples
///
/// ```ignore
/// use otc_rfq::infrastructure::persistence::traits::IdentityMappingRepository;
///
/// async fn example(repo: &impl IdentityMappingRepository) {
///     // Save mapping when anonymous RFQ is created
///     repo.save(&mapping).await?;
///     
///     // Retrieve for settlement
///     let mapping = repo.get(&rfq_id).await?;
///     
///     // Record identity reveal
///     repo.record_reveal(&rfq_id, &counterparty_id).await?;
/// }
/// ```
#[async_trait]
pub trait IdentityMappingRepository: Send + Sync + fmt::Debug {
    /// Saves an identity mapping.
    ///
    /// Creates a new mapping for an anonymous RFQ.
    /// This should be called when an anonymous RFQ is created.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Duplicate` if a mapping already exists for this RFQ.
    async fn save(&self, mapping: &IdentityMapping) -> RepositoryResult<()>;

    /// Gets an identity mapping by RFQ ID.
    ///
    /// Returns `None` if no mapping exists for the RFQ.
    ///
    /// # Security
    ///
    /// Callers must verify they have authorization to access identity data.
    async fn get(&self, rfq_id: &RfqId) -> RepositoryResult<Option<IdentityMapping>>;

    /// Records that identity was revealed to a counterparty.
    ///
    /// This is an append-only operation for audit purposes.
    /// Updates the mapping's `revealed_at` timestamp (if first reveal)
    /// and adds the counterparty to the `revealed_to` list.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::NotFound` if no mapping exists for the RFQ.
    async fn record_reveal(
        &self,
        rfq_id: &RfqId,
        revealed_to: &CounterpartyId,
    ) -> RepositoryResult<()>;

    /// Finds all mappings that have been revealed.
    ///
    /// Returns mappings where identity has been revealed to at least one party.
    async fn find_revealed(&self) -> RepositoryResult<Vec<IdentityMapping>>;

    /// Finds all mappings that have not been revealed.
    ///
    /// Returns mappings where identity has not been revealed to anyone.
    async fn find_unrevealed(&self) -> RepositoryResult<Vec<IdentityMapping>>;

    /// Deletes an identity mapping by RFQ ID.
    ///
    /// Returns `Ok(true)` if the mapping was deleted, `Ok(false)` if it didn't exist.
    ///
    /// # Warning
    ///
    /// Deleting identity mappings may have compliance implications.
    /// Consider soft-delete or archival instead.
    async fn delete(&self, rfq_id: &RfqId) -> RepositoryResult<bool>;

    /// Counts all identity mappings.
    async fn count(&self) -> RepositoryResult<u64>;
}

#[cfg(test)]
mod tests {
    use super::*;

    mod repository_error {
        use super::*;

        #[test]
        fn not_found_error() {
            let err = RepositoryError::not_found("Rfq", "rfq-123");
            assert!(err.is_not_found());
            assert!(!err.is_duplicate());
            assert!(!err.is_version_conflict());
            assert!(err.to_string().contains("not found"));
            assert!(err.to_string().contains("Rfq"));
            assert!(err.to_string().contains("rfq-123"));
        }

        #[test]
        fn duplicate_error() {
            let err = RepositoryError::duplicate("Trade", "trade-456");
            assert!(!err.is_not_found());
            assert!(err.is_duplicate());
            assert!(!err.is_version_conflict());
            assert!(err.to_string().contains("Duplicate"));
            assert!(err.to_string().contains("Trade"));
        }

        #[test]
        fn version_conflict_error() {
            let err = RepositoryError::version_conflict("Rfq", "rfq-123", 1, 2);
            assert!(!err.is_not_found());
            assert!(!err.is_duplicate());
            assert!(err.is_version_conflict());
            assert!(err.to_string().contains("conflict"));
        }

        #[test]
        fn connection_error() {
            let err = RepositoryError::connection("Connection refused");
            assert!(err.to_string().contains("Connection"));
            assert!(err.to_string().contains("refused"));
        }

        #[test]
        fn query_error() {
            let err = RepositoryError::query("Invalid SQL");
            assert!(err.to_string().contains("Query"));
            assert!(err.to_string().contains("Invalid SQL"));
        }

        #[test]
        fn serialization_error() {
            let err = RepositoryError::serialization("JSON parse error");
            assert!(err.to_string().contains("Serialization"));
        }

        #[test]
        fn internal_error() {
            let err = RepositoryError::internal("Unexpected state");
            assert!(err.to_string().contains("Internal"));
        }
    }
}
