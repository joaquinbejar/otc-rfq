//! # Incentive Settlement Service
//!
//! Domain service for orchestrating incentive settlement lifecycle.
//!
//! Manages the creation, event recording, and finalization of settlements.

use crate::domain::entities::settlement::{
    IncentiveEvent, IncentiveSettlement, SettlementError, SettlementPeriod,
};
use crate::domain::repositories::settlement_repository::SettlementRepository;
use crate::domain::value_objects::CounterpartyId;
use std::sync::Arc;

/// Service for managing incentive settlements.
///
/// Orchestrates the settlement lifecycle: creation, event recording, and finalization.
#[derive(Debug)]
pub struct IncentiveSettlementService {
    /// Repository for settlement persistence.
    repository: Arc<dyn SettlementRepository>,
}

impl IncentiveSettlementService {
    /// Creates a new settlement service.
    ///
    /// # Arguments
    ///
    /// * `repository` - Settlement repository implementation
    #[must_use]
    pub fn new(repository: Arc<dyn SettlementRepository>) -> Self {
        Self { repository }
    }

    /// Gets an existing settlement or creates a new one for the period.
    ///
    /// # Arguments
    ///
    /// * `mm_id` - Market maker identifier
    /// * `period` - Settlement period
    ///
    /// # Returns
    ///
    /// The existing or newly created settlement.
    ///
    /// # Errors
    ///
    /// Returns `SettlementError::Repository` if persistence fails.
    pub async fn get_or_create_settlement(
        &self,
        mm_id: &CounterpartyId,
        period: SettlementPeriod,
    ) -> Result<IncentiveSettlement, SettlementError> {
        // Try to find existing settlement
        if let Some(settlement) = self
            .repository
            .find_by_mm_and_period(mm_id, &period)
            .await?
        {
            return Ok(settlement);
        }

        // Create new settlement
        let settlement = IncentiveSettlement::new(mm_id.clone(), period);

        // Persist the new settlement
        self.repository.save(&settlement).await?;

        Ok(settlement)
    }

    /// Records an incentive event to the appropriate settlement.
    ///
    /// # Arguments
    ///
    /// * `mm_id` - Market maker identifier
    /// * `event` - Incentive event to record
    ///
    /// # Errors
    ///
    /// Returns `SettlementError::InvalidPeriod` if event timestamp is outside any valid period.
    /// Returns `SettlementError::Repository` if persistence fails.
    pub async fn record_incentive(
        &self,
        mm_id: &CounterpartyId,
        event: IncentiveEvent,
    ) -> Result<(), SettlementError> {
        if event.mm_id() != mm_id {
            return Err(SettlementError::MismatchedMarketMaker {
                expected: mm_id.to_string(),
                actual: event.mm_id().to_string(),
            });
        }

        let timestamp = event.timestamp();

        // Determine the period from the event timestamp
        let period = Self::period_from_timestamp(timestamp)?;

        // Get or create settlement for this period
        let mut settlement = self.get_or_create_settlement(mm_id, period).await?;

        if settlement.status() != crate::domain::entities::settlement::SettlementStatus::Open {
            return Err(SettlementError::SettlementClosed {
                status: settlement.status(),
            });
        }

        // Apply the event
        settlement.apply_event(event)?;

        // Persist the updated settlement
        self.repository.save(&settlement).await?;

        Ok(())
    }

    /// Finalizes a settlement for a specific MM and period.
    ///
    /// # Arguments
    ///
    /// * `mm_id` - Market maker identifier
    /// * `period` - Settlement period to finalize
    ///
    /// # Returns
    ///
    /// The finalized settlement.
    ///
    /// # Errors
    ///
    /// Returns `SettlementError::NotFound` if no settlement exists.
    /// Returns `SettlementError::AlreadyFinalized` if already finalized.
    /// Returns `SettlementError::Repository` if persistence fails.
    pub async fn finalize_settlement(
        &self,
        mm_id: &CounterpartyId,
        period: SettlementPeriod,
    ) -> Result<IncentiveSettlement, SettlementError> {
        // Find the settlement
        let mut settlement = self
            .repository
            .find_by_mm_and_period(mm_id, &period)
            .await?
            .ok_or_else(|| SettlementError::NotFound {
                mm_id: mm_id.to_string(),
                period,
            })?;

        // Finalize it
        settlement.finalize()?;

        // Persist the finalized settlement
        self.repository.save(&settlement).await?;

        Ok(settlement)
    }

    /// Finalizes all open settlements for a given period.
    ///
    /// Used for month-end batch processing.
    ///
    /// # Arguments
    ///
    /// * `period` - Settlement period to finalize
    ///
    /// # Returns
    ///
    /// Vector of all finalized settlements.
    ///
    /// # Errors
    ///
    /// Returns `SettlementError::Repository` if persistence fails.
    /// Individual settlement finalization errors are logged but don't stop the batch.
    pub async fn finalize_all_for_period(
        &self,
        period: SettlementPeriod,
    ) -> Result<Vec<IncentiveSettlement>, SettlementError> {
        // Get all settlements for the period
        let settlements = self.repository.find_all_by_period(&period).await?;

        let mut finalized = Vec::new();

        for mut settlement in settlements {
            // Skip already finalized settlements
            if settlement.status() != crate::domain::entities::settlement::SettlementStatus::Open {
                finalized.push(settlement);
                continue;
            }

            // Finalize the settlement
            if let Err(e) = settlement.finalize() {
                // Log error but continue with other settlements
                tracing::error!(
                    error = %e,
                    settlement_id = %settlement.id(),
                    mm_id = %settlement.mm_id(),
                    period = %period,
                    "Failed to finalize settlement"
                );
                continue;
            }

            // Persist the finalized settlement
            if let Err(e) = self.repository.save(&settlement).await {
                tracing::error!(
                    error = %e,
                    settlement_id = %settlement.id(),
                    mm_id = %settlement.mm_id(),
                    period = %period,
                    "Failed to save finalized settlement"
                );
                continue;
            }

            finalized.push(settlement);
        }

        Ok(finalized)
    }

    /// Determines the settlement period from a timestamp.
    ///
    /// # Arguments
    ///
    /// * `timestamp` - The timestamp to determine the period for
    ///
    /// # Returns
    ///
    /// The settlement period containing the timestamp.
    ///
    /// # Errors
    ///
    /// Returns `SettlementError::InvalidPeriod` if the timestamp cannot be converted to a period.
    fn period_from_timestamp(
        timestamp: crate::domain::value_objects::timestamp::Timestamp,
    ) -> Result<SettlementPeriod, SettlementError> {
        use chrono::Datelike;

        // Extract year and month from timestamp
        let dt = timestamp.as_datetime();
        let year = dt.year();
        let month = dt.month();

        // Create period for that month
        SettlementPeriod::from_month_year(year, month).ok_or_else(|| {
            SettlementError::InvalidPeriod(format!(
                "Cannot create period for year {} month {}",
                year, month
            ))
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::domain::entities::mm_incentive::{IncentiveConfig, IncentiveTier};
    use crate::domain::entities::settlement::SettlementStatus;
    use crate::domain::value_objects::TradeId;
    use crate::domain::value_objects::timestamp::Timestamp;
    use async_trait::async_trait;
    use chrono::TimeZone;
    use rust_decimal::Decimal;
    use std::collections::HashMap;
    use std::sync::Mutex;

    // Mock repository for testing
    #[derive(Debug)]
    struct MockSettlementRepository {
        settlements: Mutex<HashMap<String, IncentiveSettlement>>,
    }

    impl MockSettlementRepository {
        fn new() -> Self {
            Self {
                settlements: Mutex::new(HashMap::new()),
            }
        }

        fn key(mm_id: &CounterpartyId, period: &SettlementPeriod) -> String {
            format!("{}_{}", mm_id, period.start().timestamp_secs())
        }
    }

    #[async_trait]
    impl SettlementRepository for MockSettlementRepository {
        async fn save(&self, settlement: &IncentiveSettlement) -> Result<(), SettlementError> {
            let key = Self::key(settlement.mm_id(), &settlement.period());
            self.settlements
                .lock()
                .unwrap()
                .insert(key, settlement.clone());
            Ok(())
        }

        async fn find_by_mm_and_period(
            &self,
            mm_id: &CounterpartyId,
            period: &SettlementPeriod,
        ) -> Result<Option<IncentiveSettlement>, SettlementError> {
            let key = Self::key(mm_id, period);
            Ok(self.settlements.lock().unwrap().get(&key).cloned())
        }

        async fn find_all_by_period(
            &self,
            period: &SettlementPeriod,
        ) -> Result<Vec<IncentiveSettlement>, SettlementError> {
            let settlements: Vec<IncentiveSettlement> = self
                .settlements
                .lock()
                .unwrap()
                .values()
                .filter(|s| s.period() == *period)
                .cloned()
                .collect();
            Ok(settlements)
        }
    }

    #[tokio::test]
    async fn get_or_create_creates_new_settlement() {
        let repo = Arc::new(MockSettlementRepository::new());
        let service = IncentiveSettlementService::new(repo);

        let mm_id = CounterpartyId::new("mm-1");
        let period = SettlementPeriod::from_month_year(2026, 3).unwrap();

        let settlement = service
            .get_or_create_settlement(&mm_id, period)
            .await
            .unwrap();

        assert_eq!(settlement.mm_id(), &mm_id);
        assert_eq!(settlement.period(), period);
        assert_eq!(settlement.status(), SettlementStatus::Open);
        assert_eq!(settlement.total_trades(), 0);
    }

    #[tokio::test]
    async fn get_or_create_returns_existing_settlement() {
        let repo = Arc::new(MockSettlementRepository::new());
        let service = IncentiveSettlementService::new(repo);

        let mm_id = CounterpartyId::new("mm-1");
        let period = SettlementPeriod::from_month_year(2026, 3).unwrap();

        // Create first time
        let settlement1 = service
            .get_or_create_settlement(&mm_id, period)
            .await
            .unwrap();
        let id1 = settlement1.id();

        // Get second time
        let settlement2 = service
            .get_or_create_settlement(&mm_id, period)
            .await
            .unwrap();
        let id2 = settlement2.id();

        assert_eq!(id1, id2);
    }

    #[tokio::test]
    async fn record_incentive_creates_and_updates_settlement() {
        let repo: Arc<dyn SettlementRepository> = Arc::new(MockSettlementRepository::new());
        let service = IncentiveSettlementService::new(Arc::clone(&repo));

        let mm_id = CounterpartyId::new("mm-1");
        let timestamp = Timestamp::from_millis(
            chrono::Utc
                .with_ymd_and_hms(2026, 3, 15, 10, 0, 0)
                .single()
                .unwrap()
                .timestamp_millis(),
        )
        .unwrap();
        let trade_id = TradeId::new_v4();

        let config = IncentiveConfig::default();
        let result = crate::domain::entities::mm_incentive::compute_incentive(
            IncentiveTier::Bronze,
            Decimal::from(1_000_000),
            None,
            &config,
        );

        let event = IncentiveEvent::trade_incentive_earned(
            trade_id,
            mm_id.clone(),
            result,
            Decimal::from(1_000_000),
            timestamp,
        );

        service.record_incentive(&mm_id, event).await.unwrap();

        // Verify settlement was created and updated
        let period = SettlementPeriod::from_month_year(2026, 3).unwrap();
        let settlement = repo
            .find_by_mm_and_period(&mm_id, &period)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(settlement.total_trades(), 1);
        assert_eq!(settlement.total_volume_usd(), Decimal::from(1_000_000));
    }

    #[tokio::test]
    async fn finalize_settlement_transitions_status() {
        let repo = Arc::new(MockSettlementRepository::new());
        let service = IncentiveSettlementService::new(repo);

        let mm_id = CounterpartyId::new("mm-1");
        let period = SettlementPeriod::from_month_year(2026, 3).unwrap();

        // Create settlement
        service
            .get_or_create_settlement(&mm_id, period)
            .await
            .unwrap();

        // Finalize it
        let finalized = service.finalize_settlement(&mm_id, period).await.unwrap();

        assert_eq!(finalized.status(), SettlementStatus::Finalized);
    }

    #[tokio::test]
    async fn record_incentive_rejects_mismatched_mm_id() {
        let repo = Arc::new(MockSettlementRepository::new());
        let service = IncentiveSettlementService::new(repo);

        let mm_id = CounterpartyId::new("mm-1");
        let other_mm_id = CounterpartyId::new("mm-2");
        let timestamp = Timestamp::from_millis(
            chrono::Utc
                .with_ymd_and_hms(2026, 3, 15, 10, 0, 0)
                .single()
                .unwrap()
                .timestamp_millis(),
        )
        .unwrap();
        let trade_id = TradeId::new_v4();
        let config = IncentiveConfig::default();
        let result = crate::domain::entities::mm_incentive::compute_incentive(
            IncentiveTier::Bronze,
            Decimal::from(1_000_000),
            None,
            &config,
        );

        let event = IncentiveEvent::trade_incentive_earned(
            trade_id,
            other_mm_id,
            result,
            Decimal::from(1_000_000),
            timestamp,
        );

        let outcome = service.record_incentive(&mm_id, event).await;
        assert!(matches!(
            outcome,
            Err(SettlementError::MismatchedMarketMaker { .. })
        ));
    }

    #[tokio::test]
    async fn record_incentive_rejects_closed_settlement() {
        let repo: Arc<dyn SettlementRepository> = Arc::new(MockSettlementRepository::new());
        let service = IncentiveSettlementService::new(Arc::clone(&repo));

        let mm_id = CounterpartyId::new("mm-1");
        let period = SettlementPeriod::from_month_year(2026, 3).unwrap();

        let settlement = service
            .get_or_create_settlement(&mm_id, period)
            .await
            .unwrap();
        let mut finalized = settlement.clone();
        finalized.finalize().unwrap();
        repo.save(&finalized).await.unwrap();

        let timestamp = Timestamp::from_millis(
            chrono::Utc
                .with_ymd_and_hms(2026, 3, 15, 10, 0, 0)
                .single()
                .unwrap()
                .timestamp_millis(),
        )
        .unwrap();
        let trade_id = TradeId::new_v4();
        let config = IncentiveConfig::default();
        let result = crate::domain::entities::mm_incentive::compute_incentive(
            IncentiveTier::Bronze,
            Decimal::from(1_000_000),
            None,
            &config,
        );
        let event = IncentiveEvent::trade_incentive_earned(
            trade_id,
            mm_id.clone(),
            result,
            Decimal::from(1_000_000),
            timestamp,
        );

        let outcome = service.record_incentive(&mm_id, event).await;
        assert!(matches!(
            outcome,
            Err(SettlementError::SettlementClosed { .. })
        ));
    }

    #[tokio::test]
    async fn finalize_all_for_period_processes_multiple_settlements() {
        let repo = Arc::new(MockSettlementRepository::new());
        let service = IncentiveSettlementService::new(repo);

        let period = SettlementPeriod::from_month_year(2026, 3).unwrap();

        // Create settlements for multiple MMs
        let mm1 = CounterpartyId::new("mm-1");
        let mm2 = CounterpartyId::new("mm-2");

        service
            .get_or_create_settlement(&mm1, period)
            .await
            .unwrap();
        service
            .get_or_create_settlement(&mm2, period)
            .await
            .unwrap();

        // Finalize all
        let finalized = service.finalize_all_for_period(period).await.unwrap();

        assert_eq!(finalized.len(), 2);
        assert!(
            finalized
                .iter()
                .all(|s| s.status() == SettlementStatus::Finalized)
        );
    }

    #[tokio::test]
    async fn period_from_timestamp_keeps_december_in_december() {
        let repo = Arc::new(MockSettlementRepository::new());
        let service = IncentiveSettlementService::new(repo);

        let mm_id = CounterpartyId::new("mm-1");
        let timestamp = Timestamp::from_millis(
            chrono::Utc
                .with_ymd_and_hms(2026, 12, 31, 23, 59, 59)
                .single()
                .unwrap()
                .timestamp_millis(),
        )
        .unwrap()
        .add_millis(500);

        let period = IncentiveSettlementService::period_from_timestamp(timestamp).unwrap();

        let december = SettlementPeriod::from_month_year(2026, 12).unwrap();
        let january = SettlementPeriod::from_month_year(2027, 1).unwrap();

        assert_eq!(period, december);
        assert!(december.contains(timestamp));
        assert!(!january.contains(timestamp));

        let trade_id = TradeId::new_v4();
        let config = IncentiveConfig::default();
        let result = crate::domain::entities::mm_incentive::compute_incentive(
            IncentiveTier::Bronze,
            Decimal::from(1_000_000),
            None,
            &config,
        );
        let event = IncentiveEvent::trade_incentive_earned(
            trade_id,
            mm_id.clone(),
            result,
            Decimal::from(1_000_000),
            timestamp,
        );

        assert!(service.record_incentive(&mm_id, event).await.is_ok());
    }
}
