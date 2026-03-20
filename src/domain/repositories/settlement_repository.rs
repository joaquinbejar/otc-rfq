//! # Settlement Repository
//!
//! Repository trait for persisting and retrieving incentive settlements.

use crate::domain::entities::settlement::{IncentiveSettlement, SettlementError, SettlementPeriod};
use crate::domain::value_objects::CounterpartyId;
use async_trait::async_trait;

/// Repository for settlement persistence.
///
/// Provides methods to save and retrieve settlement aggregates.
#[async_trait]
pub trait SettlementRepository: Send + Sync + std::fmt::Debug {
    /// Saves a settlement to the repository.
    ///
    /// # Arguments
    ///
    /// * `settlement` - The settlement to save
    ///
    /// # Errors
    ///
    /// Returns `SettlementError::Repository` if persistence fails.
    async fn save(&self, settlement: &IncentiveSettlement) -> Result<(), SettlementError>;

    /// Finds a settlement by market maker and period.
    ///
    /// # Arguments
    ///
    /// * `mm_id` - Market maker identifier
    /// * `period` - Settlement period
    ///
    /// # Returns
    ///
    /// The settlement if found, `None` otherwise.
    ///
    /// # Errors
    ///
    /// Returns `SettlementError::Repository` if retrieval fails.
    async fn find_by_mm_and_period(
        &self,
        mm_id: &CounterpartyId,
        period: &SettlementPeriod,
    ) -> Result<Option<IncentiveSettlement>, SettlementError>;

    /// Finds all settlements for a given period.
    ///
    /// # Arguments
    ///
    /// * `period` - Settlement period
    ///
    /// # Returns
    ///
    /// Vector of all settlements in the period.
    ///
    /// # Errors
    ///
    /// Returns `SettlementError::Repository` if retrieval fails.
    async fn find_all_by_period(
        &self,
        period: &SettlementPeriod,
    ) -> Result<Vec<IncentiveSettlement>, SettlementError>;
}
