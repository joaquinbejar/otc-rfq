//! # Market Maker Incentive Service
//!
//! Domain service for computing market maker incentive status.
//!
//! Orchestrates the [`MmPerformanceTracker`], [`IncentiveConfig`], and penalty
//! evaluation to produce a complete [`MmIncentiveStatus`] snapshot.
//!
//! # Examples
//!
//! ```ignore
//! use otc_rfq::domain::services::mm_incentive_service::MmIncentiveService;
//!
//! let service = MmIncentiveService::new(tracker, config);
//! let status = service.get_status(&mm_id).await?;
//! println!("Tier: {}, Rebate: {}bps", status.current_tier(), status.rebate_rate_bps());
//! ```

use crate::domain::entities::mm_incentive::{
    IncentiveConfig, IncentiveTier, MmIncentiveStatus, evaluate_penalties, volume_to_next_tier,
};
use crate::domain::services::mm_performance::{MmPerformanceError, MmPerformanceTracker};
use crate::domain::value_objects::CounterpartyId;
use crate::domain::value_objects::timestamp::Timestamp;
use rust_decimal::Decimal;
use std::sync::Arc;
use thiserror::Error;

/// Error type for MM incentive operations.
#[derive(Debug, Error)]
pub enum MmIncentiveError {
    /// Performance tracker error.
    #[error("performance error: {0}")]
    Performance(#[from] MmPerformanceError),

    /// Market maker not found.
    #[error("market maker not found: {0}")]
    MmNotFound(String),
}

/// Result type for MM incentive operations.
pub type MmIncentiveResult<T> = Result<T, MmIncentiveError>;

/// Service for computing market maker incentive status.
///
/// Combines performance metrics, tier calculation, and penalty evaluation
/// into a unified incentive status view.
#[derive(Debug)]
pub struct MmIncentiveService {
    /// Performance tracker for retrieving MM metrics.
    performance_tracker: Arc<MmPerformanceTracker>,
    /// Incentive program configuration.
    config: IncentiveConfig,
}

impl MmIncentiveService {
    /// Creates a new incentive service.
    ///
    /// # Arguments
    ///
    /// * `performance_tracker` - Tracker for MM performance metrics
    /// * `config` - Incentive program configuration
    #[must_use]
    pub fn new(performance_tracker: Arc<MmPerformanceTracker>, config: IncentiveConfig) -> Self {
        Self {
            performance_tracker,
            config,
        }
    }

    /// Returns the incentive configuration.
    #[inline]
    #[must_use]
    pub fn config(&self) -> &IncentiveConfig {
        &self.config
    }

    /// Computes the complete incentive status for a market maker.
    ///
    /// # Arguments
    ///
    /// * `mm_id` - Market maker identifier
    ///
    /// # Returns
    ///
    /// Complete [`MmIncentiveStatus`] including tier, rebates, and penalties.
    ///
    /// # Errors
    ///
    /// Returns `MmIncentiveError::MmNotFound` if the MM has no recorded data.
    /// Returns `MmIncentiveError::Performance` if metrics retrieval fails.
    pub async fn get_status(&self, mm_id: &CounterpartyId) -> MmIncentiveResult<MmIncentiveStatus> {
        // Get performance metrics
        let metrics = self.performance_tracker.get_metrics(mm_id).await?;

        // Check if MM exists (has any data)
        if !metrics.has_data() {
            return Err(MmIncentiveError::MmNotFound(mm_id.to_string()));
        }

        // TODO: Integrate with VolumeTracker when available
        // For now, use placeholder volume
        let monthly_volume_usd = Decimal::ZERO;

        // Calculate tier from volume
        let current_tier = IncentiveTier::from_volume(monthly_volume_usd, &self.config);

        // Get rebate rate for tier
        let rebate_rate_bps = current_tier.rebate_bps(&self.config);

        // Calculate volume to next tier
        let volume_to_next = volume_to_next_tier(monthly_volume_usd, current_tier, &self.config);

        // Get next tier
        let next_tier = current_tier.next_tier();

        // TODO: Integrate with rebate tracking when available
        // For now, use placeholder
        let current_period_rebates_usd = Decimal::ZERO;

        // Evaluate penalties based on performance
        let penalty_result = evaluate_penalties(&metrics, &self.config);

        let computed_at = Timestamp::now();

        Ok(MmIncentiveStatus::new(
            mm_id.clone(),
            current_tier,
            rebate_rate_bps,
            monthly_volume_usd,
            volume_to_next,
            next_tier,
            current_period_rebates_usd,
            penalty_result,
            computed_at,
        ))
    }

    /// Checks if a market maker exists in the system.
    ///
    /// # Arguments
    ///
    /// * `mm_id` - Market maker identifier
    ///
    /// # Returns
    ///
    /// `true` if the MM has recorded performance data.
    ///
    /// # Errors
    ///
    /// Returns `MmIncentiveError::Performance` if metrics retrieval fails.
    pub async fn mm_exists(&self, mm_id: &CounterpartyId) -> MmIncentiveResult<bool> {
        let metrics = self.performance_tracker.get_metrics(mm_id).await?;
        Ok(metrics.has_data())
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::useless_vec
)]
mod tests {
    use super::*;
    use crate::domain::entities::mm_performance::{
        DEFAULT_WINDOW_DAYS, MmPerformanceEvent, MmPerformanceEventKind,
    };
    use crate::domain::services::mm_performance::MmPerformanceRepository;
    use async_trait::async_trait;
    use dashmap::DashMap;

    /// In-memory implementation for testing.
    #[derive(Debug, Default)]
    struct MockMmPerformanceRepo {
        events: DashMap<String, Vec<MmPerformanceEvent>>,
    }

    #[async_trait]
    impl MmPerformanceRepository for MockMmPerformanceRepo {
        async fn record_event(&self, event: MmPerformanceEvent) -> Result<(), MmPerformanceError> {
            self.events
                .entry(event.mm_id().to_string())
                .or_default()
                .push(event);
            Ok(())
        }

        async fn get_events(
            &self,
            mm_id: &CounterpartyId,
            from: Timestamp,
            to: Timestamp,
        ) -> Result<Vec<MmPerformanceEvent>, MmPerformanceError> {
            let key = mm_id.to_string();
            match self.events.get(&key) {
                Some(events) => Ok(events
                    .iter()
                    .filter(|e| e.is_within_window(from, to))
                    .cloned()
                    .collect()),
                None => Ok(Vec::new()),
            }
        }

        async fn get_all_mm_ids(&self) -> Result<Vec<CounterpartyId>, MmPerformanceError> {
            Ok(self
                .events
                .iter()
                .map(|entry| CounterpartyId::new(entry.key().as_str()))
                .collect())
        }

        async fn trim_before(&self, before: Timestamp) -> Result<u64, MmPerformanceError> {
            let mut removed = 0u64;
            for mut entry in self.events.iter_mut() {
                let initial_len = entry.value().len() as u64;
                entry
                    .value_mut()
                    .retain(|e| !e.timestamp().is_before(&before));
                let final_len = entry.value().len() as u64;
                removed = removed.saturating_add(initial_len.saturating_sub(final_len));
            }
            Ok(removed)
        }
    }

    fn mm_id(name: &str) -> CounterpartyId {
        CounterpartyId::new(name)
    }

    fn create_service() -> (MmIncentiveService, Arc<MockMmPerformanceRepo>) {
        let repo = Arc::new(MockMmPerformanceRepo::default());
        let tracker = Arc::new(MmPerformanceTracker::new(
            Arc::clone(&repo) as Arc<dyn MmPerformanceRepository>,
            DEFAULT_WINDOW_DAYS,
        ));
        let config = IncentiveConfig::default();
        let service = MmIncentiveService::new(tracker, config);
        (service, repo)
    }

    #[tokio::test]
    async fn get_status_returns_error_for_unknown_mm() {
        let (service, _repo) = create_service();
        let id = mm_id("unknown-mm");

        let result = service.get_status(&id).await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            MmIncentiveError::MmNotFound(_)
        ));
    }

    #[tokio::test]
    async fn get_status_returns_status_for_known_mm() {
        let (service, repo) = create_service();
        let id = mm_id("mm-test");

        // Record some events
        let event = MmPerformanceEvent::new(
            id.clone(),
            MmPerformanceEventKind::RfqSent,
            Timestamp::now(),
        );
        repo.record_event(event).await.unwrap();

        let result = service.get_status(&id).await;

        assert!(result.is_ok());
        let status = result.unwrap();
        assert_eq!(status.mm_id(), &id);
        assert_eq!(status.current_tier(), IncentiveTier::Bronze);
    }

    #[tokio::test]
    async fn mm_exists_returns_false_for_unknown() {
        let (service, _repo) = create_service();
        let id = mm_id("unknown-mm");

        let exists = service.mm_exists(&id).await.unwrap();

        assert!(!exists);
    }

    #[tokio::test]
    async fn mm_exists_returns_true_for_known() {
        let (service, repo) = create_service();
        let id = mm_id("mm-known");

        let event = MmPerformanceEvent::new(
            id.clone(),
            MmPerformanceEventKind::RfqSent,
            Timestamp::now(),
        );
        repo.record_event(event).await.unwrap();

        let exists = service.mm_exists(&id).await.unwrap();

        assert!(exists);
    }

    #[tokio::test]
    async fn status_includes_penalty_for_low_response_rate() {
        let (service, repo) = create_service();
        let id = mm_id("mm-low-response");

        // Create low response rate: 2 RFQs, 0 quotes = 0% response
        for _ in 0..5 {
            let event = MmPerformanceEvent::new(
                id.clone(),
                MmPerformanceEventKind::RfqSent,
                Timestamp::now(),
            );
            repo.record_event(event).await.unwrap();
        }

        let status = service.get_status(&id).await.unwrap();

        assert!(status.penalty_result().has_penalty());
        assert!(status.penalty_result().should_reduce_capacity());
    }
}
