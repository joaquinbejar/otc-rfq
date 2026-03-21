//! # Incentive Report Service
//!
//! Service for generating comprehensive incentive reports for market makers.
//!
//! Orchestrates report generation from settlement data, performance metrics,
//! and penalty calculations.

use crate::domain::entities::mm_incentive::{
    IncentiveConfig, IncentiveTier, PenaltyResult, evaluate_penalties,
};
use crate::domain::entities::settlement::{
    IncentiveEvent, IncentiveReport, IncentiveSettlement, IncentiveSummary, ReportDetailLevel,
    SettlementError, SettlementPeriod, TradeIncentiveDetail,
};
use crate::domain::repositories::settlement_repository::SettlementRepository;
use crate::domain::services::mm_performance::MmPerformanceTracker;
use crate::domain::value_objects::CounterpartyId;
use crate::domain::value_objects::timestamp::Timestamp;
use std::sync::Arc;

/// Service for generating incentive reports.
#[derive(Debug)]
pub struct IncentiveReportService {
    /// Repository for settlement data.
    settlement_repository: Arc<dyn SettlementRepository>,
    /// Tracker for performance metrics.
    performance_tracker: Arc<MmPerformanceTracker>,
    /// Incentive configuration for penalty calculation.
    incentive_config: Arc<IncentiveConfig>,
}

impl IncentiveReportService {
    /// Creates a new incentive report service.
    ///
    /// # Arguments
    ///
    /// * `settlement_repository` - Settlement repository implementation
    /// * `performance_tracker` - Performance tracker for metrics
    /// * `incentive_config` - Incentive configuration for penalty calculation
    #[must_use]
    pub fn new(
        settlement_repository: Arc<dyn SettlementRepository>,
        performance_tracker: Arc<MmPerformanceTracker>,
        incentive_config: Arc<IncentiveConfig>,
    ) -> Self {
        Self {
            settlement_repository,
            performance_tracker,
            incentive_config,
        }
    }

    /// Generates an incentive report for a market maker and period.
    ///
    /// # Arguments
    ///
    /// * `mm_id` - Market maker identifier
    /// * `period` - Settlement period
    /// * `detail_level` - Level of detail for the report
    ///
    /// # Returns
    ///
    /// The generated incentive report.
    ///
    /// # Errors
    ///
    /// Returns `SettlementError::NotFound` if no settlement exists for the period.
    /// Returns `SettlementError::Repository` if data cannot be retrieved.
    pub async fn generate_report(
        &self,
        mm_id: &CounterpartyId,
        period: SettlementPeriod,
        detail_level: ReportDetailLevel,
    ) -> Result<IncentiveReport, SettlementError> {
        let settlement = self
            .settlement_repository
            .find_by_mm_and_period(mm_id, &period)
            .await?
            .ok_or_else(|| SettlementError::NotFound {
                mm_id: mm_id.to_string(),
                period,
            })?;

        self.generate_report_from_settlement(&settlement, detail_level)
            .await
    }

    /// Generates an incentive report from an existing settlement.
    ///
    /// # Arguments
    ///
    /// * `settlement` - The settlement to generate a report from
    /// * `detail_level` - Level of detail for the report
    ///
    /// # Returns
    ///
    /// The generated incentive report.
    ///
    /// # Errors
    ///
    /// Returns `SettlementError::NotFound` if no settlement exists for the period.
    /// Returns `SettlementError::Repository` if data cannot be retrieved.
    pub async fn generate_report_from_settlement(
        &self,
        settlement: &IncentiveSettlement,
        detail_level: ReportDetailLevel,
    ) -> Result<IncentiveReport, SettlementError> {
        let generated_at = Timestamp::now();

        // Extract summary from settlement
        let summary = IncentiveSummary::new(
            settlement.total_trades(),
            settlement.total_volume_usd(),
            settlement.total_base_rebates_usd(),
            settlement.total_bonuses_usd(),
            settlement.net_payout_usd(),
        );

        // Get current tier from the last event in the settlement (if any)
        // Note: This is the tier at the time of the last trade event, not necessarily
        // the MM's current tier if tier changes occurred outside this settlement.
        let current_tier = settlement
            .events()
            .last()
            .map(|event| event.result().tier())
            .unwrap_or(IncentiveTier::Bronze);

        // Calculate penalties from performance metrics
        let penalties = self.calculate_penalties(settlement).await;

        // Generate trade details if requested
        let trade_details = if detail_level == ReportDetailLevel::Detailed {
            Some(self.extract_trade_details(settlement))
        } else {
            None
        };

        Ok(IncentiveReport::new(
            settlement.id(),
            settlement.mm_id().clone(),
            settlement.period(),
            generated_at,
            current_tier,
            summary,
            penalties,
            trade_details,
        ))
    }

    /// Calculates penalties from performance metrics for the settlement period.
    ///
    /// Returns `None` if metrics are unavailable.
    ///
    /// # Note
    ///
    /// Currently uses rolling window metrics from `MmPerformanceTracker::get_metrics()`
    /// which may not align exactly with the settlement period. Future enhancement
    /// would add period-specific metrics retrieval.
    async fn calculate_penalties(&self, settlement: &IncentiveSettlement) -> Option<PenaltyResult> {
        // Get performance metrics (currently uses rolling window, not settlement period)
        let metrics = self
            .performance_tracker
            .get_metrics(settlement.mm_id())
            .await
            .ok()?;

        Some(evaluate_penalties(&metrics, &self.incentive_config))
    }

    /// Extracts trade details from settlement events.
    fn extract_trade_details(&self, settlement: &IncentiveSettlement) -> Vec<TradeIncentiveDetail> {
        settlement
            .events()
            .iter()
            .map(|event| match event {
                IncentiveEvent::TradeIncentiveEarned {
                    trade_id,
                    result,
                    notional,
                    timestamp,
                    ..
                } => TradeIncentiveDetail::new(
                    *trade_id,
                    *timestamp,
                    *notional,
                    result.tier(),
                    result.base_rebate_bps(),
                    result.spread_bonus_bps(),
                    result.rebate_amount(),
                ),
            })
            .collect()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::domain::entities::mm_incentive::{
        IncentiveConfig, IncentiveTier, compute_incentive,
    };
    use crate::domain::entities::settlement::SettlementPeriod;
    use crate::domain::repositories::settlement_repository::SettlementRepository;
    use crate::domain::services::mm_performance::MmPerformanceTracker;
    use crate::domain::value_objects::{CounterpartyId, TradeId};
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
    }

    #[async_trait]
    impl SettlementRepository for MockSettlementRepository {
        async fn save(&self, settlement: &IncentiveSettlement) -> Result<(), SettlementError> {
            let key = format!("{}:{}", settlement.mm_id(), settlement.period().start());
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
            let key = format!("{}:{}", mm_id, period.start());
            Ok(self.settlements.lock().unwrap().get(&key).cloned())
        }

        async fn find_all_by_period(
            &self,
            _period: &SettlementPeriod,
        ) -> Result<Vec<IncentiveSettlement>, SettlementError> {
            Ok(Vec::new())
        }
    }

    fn make_timestamp(
        year: i32,
        month: u32,
        day: u32,
        hour: u32,
        minute: u32,
        second: u32,
    ) -> Timestamp {
        Timestamp::from_millis(
            chrono::Utc
                .with_ymd_and_hms(year, month, day, hour, minute, second)
                .single()
                .unwrap()
                .timestamp_millis(),
        )
        .unwrap()
    }

    #[tokio::test]
    async fn generate_summary_report_from_settlement() {
        let repo = Arc::new(MockSettlementRepository::new());
        let perf_tracker = Arc::new(MmPerformanceTracker::new(
            Arc::new(crate::infrastructure::persistence::in_memory::mm_performance_repository::InMemoryMmPerformanceRepository::new()),
            7,
        ));
        let config = Arc::new(IncentiveConfig::default());
        let service = IncentiveReportService::new(repo, perf_tracker, config);

        let mm_id = CounterpartyId::new("mm-1");
        let period = SettlementPeriod::from_month_year(2026, 3).unwrap();
        let mut settlement = IncentiveSettlement::new(mm_id.clone(), period);

        // Add a trade event
        let trade_id = TradeId::new_v4();
        let timestamp = make_timestamp(2026, 3, 15, 10, 0, 0);
        let config = IncentiveConfig::default();
        let result = compute_incentive(
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
        settlement.apply_event(event).unwrap();

        let report = service
            .generate_report_from_settlement(&settlement, ReportDetailLevel::Summary)
            .await
            .unwrap();

        assert_eq!(report.mm_id(), &mm_id);
        assert_eq!(report.period(), period);
        assert_eq!(report.summary().total_trades(), 1);
        assert_eq!(
            report.summary().total_volume_usd(),
            Decimal::from(1_000_000)
        );
        assert_eq!(report.detail_level(), ReportDetailLevel::Summary);
        assert!(report.trade_details().is_none());
    }

    #[tokio::test]
    async fn generate_detailed_report_from_settlement() {
        let repo = Arc::new(MockSettlementRepository::new());
        let perf_tracker = Arc::new(MmPerformanceTracker::new(
            Arc::new(crate::infrastructure::persistence::in_memory::mm_performance_repository::InMemoryMmPerformanceRepository::new()),
            7,
        ));
        let config = Arc::new(IncentiveConfig::default());
        let service = IncentiveReportService::new(repo, perf_tracker, config);

        let mm_id = CounterpartyId::new("mm-1");
        let period = SettlementPeriod::from_month_year(2026, 3).unwrap();
        let mut settlement = IncentiveSettlement::new(mm_id.clone(), period);

        // Add a trade event
        let trade_id = TradeId::new_v4();
        let timestamp = make_timestamp(2026, 3, 15, 10, 0, 0);
        let config = IncentiveConfig::default();
        let result = compute_incentive(
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
        settlement.apply_event(event).unwrap();

        let report = service
            .generate_report_from_settlement(&settlement, ReportDetailLevel::Detailed)
            .await
            .unwrap();

        assert_eq!(report.detail_level(), ReportDetailLevel::Detailed);
        assert!(report.trade_details().is_some());
        let details = report.trade_details().unwrap();
        assert_eq!(details.len(), 1);
        assert_eq!(details[0].trade_id(), trade_id);
        assert_eq!(details[0].notional(), Decimal::from(1_000_000));
    }
}
