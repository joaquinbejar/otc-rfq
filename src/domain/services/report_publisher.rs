//! # Report Publisher Service
//!
//! Service for publishing delayed trade reports when their scheduled
//! time has passed.
//!
//! This module provides the [`ReportPublisher`] which processes
//! pending reports and emits [`BlockTradeReported`] events.

use crate::domain::entities::block_trade::BlockTrade;
use crate::domain::entities::delayed_report::{DelayedReport, TradeSummary};
use crate::domain::errors::DomainResult;
use crate::domain::events::reporting_events::{BlockTradeReported, ReportScheduled};
use crate::domain::services::ReportingTier;
use crate::domain::services::market_calendar::{MarketCalendarConfig, next_market_close};
use crate::domain::services::report_scheduler::ReportSchedulerConfig;
use crate::domain::value_objects::Timestamp;
use crate::infrastructure::persistence::traits::DelayedReportRepository;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, info};

/// Result of a publish operation.
#[derive(Debug, Clone)]
pub struct PublishResult {
    /// Number of reports published.
    pub published_count: usize,
    /// Events generated during publishing.
    pub events: Vec<BlockTradeReported>,
}

impl PublishResult {
    /// Creates a new publish result.
    #[must_use]
    pub fn new(published_count: usize, events: Vec<BlockTradeReported>) -> Self {
        Self {
            published_count,
            events,
        }
    }

    /// Creates an empty result.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            published_count: 0,
            events: Vec::new(),
        }
    }
}

/// Service for publishing delayed trade reports.
///
/// Handles scheduling reports based on tier and processing them
/// when their publication time arrives.
#[derive(Debug)]
pub struct ReportPublisher<R: DelayedReportRepository> {
    repository: Arc<R>,
    scheduler_config: ReportSchedulerConfig,
    calendar_config: MarketCalendarConfig,
}

impl<R: DelayedReportRepository> ReportPublisher<R> {
    /// Creates a new report publisher.
    #[must_use]
    pub fn new(repository: Arc<R>) -> Self {
        Self {
            repository,
            scheduler_config: ReportSchedulerConfig::default(),
            calendar_config: MarketCalendarConfig::default(),
        }
    }

    /// Creates a new report publisher with custom configuration.
    #[must_use]
    pub fn with_config(
        repository: Arc<R>,
        scheduler_config: ReportSchedulerConfig,
        calendar_config: MarketCalendarConfig,
    ) -> Self {
        Self {
            repository,
            scheduler_config,
            calendar_config,
        }
    }

    /// Schedules a report for a block trade.
    ///
    /// Determines the publication time based on the trade's reporting tier
    /// and persists the report for later publication.
    ///
    /// # Returns
    ///
    /// The scheduled report and a [`ReportScheduled`] event.
    ///
    /// # Errors
    ///
    /// Returns an error if the report cannot be persisted.
    pub async fn schedule(
        &self,
        trade: &BlockTrade,
    ) -> DomainResult<(DelayedReport, ReportScheduled)> {
        let tier = trade.reporting_tier().unwrap_or(ReportingTier::Standard);
        let publish_at = self.calculate_publish_time(tier);

        let summary = TradeSummary::new(
            trade.instrument().clone(),
            trade.price(),
            trade.quantity(),
            Timestamp::now(),
        );

        let report = DelayedReport::new(trade.id().to_string(), tier, summary, publish_at);

        self.repository.save(&report).await.map_err(|e| {
            crate::domain::errors::DomainError::ValidationError(format!(
                "Failed to save delayed report: {}",
                e
            ))
        })?;

        let event = ReportScheduled::new(report.id(), trade.id().to_string(), tier, publish_at);

        info!(
            trade_id = %trade.id(),
            tier = ?tier,
            publish_at = %publish_at,
            "Scheduled delayed report"
        );

        Ok((report, event))
    }

    /// Calculates the publication time for a given tier.
    fn calculate_publish_time(&self, tier: ReportingTier) -> Timestamp {
        let now = Timestamp::now();

        match tier {
            ReportingTier::Standard => {
                now.add_secs(self.scheduler_config.standard_delay.as_secs() as i64)
            }
            ReportingTier::Large => {
                now.add_secs(self.scheduler_config.large_delay.as_secs() as i64)
            }
            ReportingTier::VeryLarge => {
                // VeryLarge uses EOD
                next_market_close(now, &self.calendar_config)
            }
        }
    }

    /// Processes all reports that are ready to be published.
    ///
    /// # Returns
    ///
    /// A [`PublishResult`] containing the number of reports published
    /// and the generated events.
    ///
    /// # Errors
    ///
    /// Returns an error if reports cannot be retrieved from the repository.
    pub async fn process_ready(&self) -> DomainResult<PublishResult> {
        let now = Timestamp::now();
        let ready_reports = self
            .repository
            .find_ready_to_publish(now)
            .await
            .map_err(|e| {
                crate::domain::errors::DomainError::ValidationError(format!(
                    "Failed to find ready reports: {}",
                    e
                ))
            })?;

        if ready_reports.is_empty() {
            debug!("No reports ready for publication");
            return Ok(PublishResult::empty());
        }

        let total_ready = ready_reports.len();
        info!(count = total_ready, "Processing ready reports");

        let mut events = Vec::with_capacity(total_ready);
        let mut published_count = 0;

        for report in ready_reports {
            match self.publish_report(&report).await {
                Ok(event) => {
                    events.push(event);
                    published_count += 1;
                }
                Err(e) => {
                    error!(
                        report_id = %report.id(),
                        error = %e,
                        "Failed to publish report"
                    );
                }
            }
        }

        info!(
            published = published_count,
            failed = total_ready.saturating_sub(published_count),
            "Completed report processing"
        );

        Ok(PublishResult::new(published_count, events))
    }

    /// Publishes a single report.
    async fn publish_report(&self, report: &DelayedReport) -> DomainResult<BlockTradeReported> {
        let published_at = Timestamp::now();

        // Mark as published in repository
        self.repository
            .mark_published(&report.id(), published_at)
            .await
            .map_err(|e| {
                crate::domain::errors::DomainError::ValidationError(format!(
                    "Failed to mark report as published: {}",
                    e
                ))
            })?;

        // Create the event
        let event = BlockTradeReported::new(
            report.id(),
            report.trade_id().to_string(),
            report.reporting_tier(),
            report.trade_summary().clone(),
            report.created_at(),
            published_at,
        );

        info!(
            report_id = %report.id(),
            trade_id = %report.trade_id(),
            tier = ?report.reporting_tier(),
            "Published trade report"
        );

        Ok(event)
    }

    /// Gets all pending reports.
    ///
    /// # Errors
    ///
    /// Returns an error if reports cannot be retrieved.
    pub async fn get_pending(&self) -> DomainResult<Vec<DelayedReport>> {
        self.repository.find_pending().await.map_err(|e| {
            crate::domain::errors::DomainError::ValidationError(format!(
                "Failed to get pending reports: {}",
                e
            ))
        })
    }

    /// Gets the count of pending reports.
    ///
    /// # Errors
    ///
    /// Returns an error if the count cannot be retrieved.
    pub async fn count_pending(&self) -> DomainResult<u64> {
        self.repository.count_pending().await.map_err(|e| {
            crate::domain::errors::DomainError::ValidationError(format!(
                "Failed to count pending reports: {}",
                e
            ))
        })
    }

    /// Returns the configured delay for a tier.
    #[must_use]
    pub fn delay_for_tier(&self, tier: ReportingTier) -> Duration {
        self.scheduler_config.delay_for_tier(tier)
    }

    /// Returns the next time reports should be checked.
    ///
    /// Returns the minimum delay until the next report is due,
    /// or a default polling interval if no reports are pending.
    pub async fn next_check_delay(&self) -> Duration {
        const DEFAULT_POLL_INTERVAL: Duration = Duration::from_secs(60);

        let pending = match self.repository.find_pending().await {
            Ok(p) => p,
            Err(_) => return DEFAULT_POLL_INTERVAL,
        };

        if pending.is_empty() {
            return DEFAULT_POLL_INTERVAL;
        }

        // Find the minimum delay until the next report is due
        let now = Timestamp::now();
        let min_delay = pending
            .iter()
            .map(|r| {
                let publish_ms = r.publish_at().timestamp_millis();
                let now_ms = now.timestamp_millis();
                if publish_ms > now_ms {
                    Duration::from_millis((publish_ms - now_ms) as u64)
                } else {
                    Duration::ZERO
                }
            })
            .min()
            .unwrap_or(DEFAULT_POLL_INTERVAL);

        // Return at least 1 second to avoid busy-waiting
        min_delay.max(Duration::from_secs(1))
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::clone_on_ref_ptr,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;
    use crate::domain::value_objects::enums::{AssetClass, SettlementMethod};
    use crate::domain::value_objects::{Instrument, Price, Quantity, Symbol};
    use crate::infrastructure::persistence::in_memory::delayed_report_repository::InMemoryDelayedReportRepository;

    #[tokio::test]
    async fn process_ready_reports() {
        let repo = Arc::new(InMemoryDelayedReportRepository::new());
        let publisher = ReportPublisher::new(repo.clone());

        // Create a report that's ready (publish time in the past)
        let summary = TradeSummary::new(
            Instrument::new(
                Symbol::new("BTC/USD").unwrap(),
                AssetClass::CryptoSpot,
                SettlementMethod::default(),
            ),
            Price::new(50000.0).unwrap(),
            Quantity::new(10.0).unwrap(),
            Timestamp::now(),
        );
        let report = DelayedReport::new(
            "trade-1".to_string(),
            ReportingTier::Standard,
            summary,
            Timestamp::now().add_secs(-60), // 1 minute ago
        );
        repo.save(&report).await.unwrap();

        // Process ready reports
        let result = publisher.process_ready().await.unwrap();

        assert_eq!(result.published_count, 1);
        assert_eq!(result.events.len(), 1);
        assert_eq!(result.events[0].trade_id(), "trade-1");

        // Verify marked as published
        let updated = repo.find_by_id(&report.id()).await.unwrap().unwrap();
        assert!(updated.is_published());
    }

    #[tokio::test]
    async fn no_reports_ready() {
        let repo = Arc::new(InMemoryDelayedReportRepository::new());
        let publisher = ReportPublisher::new(repo.clone());

        // Create a report that's not ready yet
        let summary = TradeSummary::new(
            Instrument::new(
                Symbol::new("BTC/USD").unwrap(),
                AssetClass::CryptoSpot,
                SettlementMethod::default(),
            ),
            Price::new(50000.0).unwrap(),
            Quantity::new(10.0).unwrap(),
            Timestamp::now(),
        );
        let report = DelayedReport::new(
            "trade-1".to_string(),
            ReportingTier::Standard,
            summary,
            Timestamp::now().add_secs(900), // 15 minutes from now
        );
        repo.save(&report).await.unwrap();

        let result = publisher.process_ready().await.unwrap();

        assert_eq!(result.published_count, 0);
        assert!(result.events.is_empty());
    }

    #[tokio::test]
    async fn delay_for_tier() {
        let repo = Arc::new(InMemoryDelayedReportRepository::new());
        let publisher = ReportPublisher::new(repo);

        assert_eq!(
            publisher.delay_for_tier(ReportingTier::Standard),
            Duration::from_secs(15 * 60)
        );
        assert_eq!(
            publisher.delay_for_tier(ReportingTier::Large),
            Duration::from_secs(60 * 60)
        );
    }
}
