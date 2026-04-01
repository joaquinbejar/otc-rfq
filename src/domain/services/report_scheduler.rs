//! # Report Scheduler Service
//!
//! Service for scheduling delayed trade reports based on reporting tier.
//!
//! This module provides the [`ReportScheduler`] trait for scheduling
//! trade reports according to regulatory-compliant delays.

use crate::domain::entities::block_trade::BlockTrade;
use crate::domain::errors::DomainResult;
use crate::domain::services::ReportingTier;
use crate::domain::value_objects::{BlockTradeId, Timestamp};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::Duration;

/// A scheduled report for a block trade.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduledReport {
    /// Block trade ID.
    block_trade_id: BlockTradeId,
    /// Reporting tier.
    tier: ReportingTier,
    /// When the report should be published.
    publish_at: Timestamp,
    /// Whether the report has been published.
    published: bool,
}

impl ScheduledReport {
    /// Creates a new scheduled report.
    #[must_use]
    pub fn new(block_trade_id: BlockTradeId, tier: ReportingTier, publish_at: Timestamp) -> Self {
        Self {
            block_trade_id,
            tier,
            publish_at,
            published: false,
        }
    }

    /// Returns the block trade ID.
    #[must_use]
    pub fn block_trade_id(&self) -> BlockTradeId {
        self.block_trade_id
    }

    /// Returns the reporting tier.
    pub fn tier(&self) -> ReportingTier {
        self.tier
    }

    /// Returns when the report should be published.
    #[must_use]
    pub fn publish_at(&self) -> Timestamp {
        self.publish_at
    }

    /// Returns whether the report has been published.
    #[must_use]
    pub fn is_published(&self) -> bool {
        self.published
    }

    /// Marks the report as published.
    pub fn mark_published(&mut self) {
        self.published = true;
    }

    /// Returns whether the report is ready to be published.
    #[must_use]
    pub fn is_ready(&self) -> bool {
        !self.published && Timestamp::now() >= self.publish_at
    }
}

/// Configuration for report scheduling.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportSchedulerConfig {
    /// Delay for Standard tier trades.
    pub standard_delay: Duration,
    /// Delay for Large tier trades.
    pub large_delay: Duration,
    /// Delay for VeryLarge tier trades (typically end of day).
    pub very_large_delay: Duration,
}

impl Default for ReportSchedulerConfig {
    fn default() -> Self {
        Self {
            standard_delay: Duration::from_secs(15 * 60), // 15 minutes
            large_delay: Duration::from_secs(60 * 60),    // 60 minutes
            very_large_delay: Duration::from_secs(8 * 60 * 60), // 8 hours (EOD approximation)
        }
    }
}

impl ReportSchedulerConfig {
    /// Creates a new configuration with custom delays.
    #[must_use]
    pub fn new(
        standard_delay: Duration,
        large_delay: Duration,
        very_large_delay: Duration,
    ) -> Self {
        Self {
            standard_delay,
            large_delay,
            very_large_delay,
        }
    }

    /// Returns the delay for a given reporting tier.
    #[must_use]
    pub fn delay_for_tier(&self, tier: ReportingTier) -> Duration {
        match tier {
            ReportingTier::Standard => self.standard_delay,
            ReportingTier::Large => self.large_delay,
            ReportingTier::VeryLarge => self.very_large_delay,
        }
    }
}

/// Service for scheduling delayed trade reports.
///
/// Implementations handle scheduling and publishing trade reports
/// according to the regulatory requirements for each reporting tier.
#[async_trait]
pub trait ReportScheduler: Send + Sync + fmt::Debug {
    /// Schedules a trade report based on reporting tier.
    ///
    /// The report will be published after the appropriate delay
    /// based on the trade's reporting tier.
    ///
    /// # Arguments
    ///
    /// * `trade` - The block trade to report
    ///
    /// # Errors
    ///
    /// Returns an error if the report cannot be scheduled.
    async fn schedule(&self, trade: &BlockTrade) -> DomainResult<ScheduledReport>;

    /// Gets the delay for a reporting tier.
    ///
    /// # Arguments
    ///
    /// * `tier` - The reporting tier
    ///
    /// # Returns
    ///
    /// The delay duration before the report should be published.
    fn delay_for_tier(&self, tier: ReportingTier) -> Duration;

    /// Publishes a scheduled report immediately.
    ///
    /// # Arguments
    ///
    /// * `report` - The scheduled report to publish
    ///
    /// # Errors
    ///
    /// Returns an error if the report cannot be published.
    async fn publish(&self, report: &mut ScheduledReport) -> DomainResult<()>;

    /// Gets all pending (unpublished) reports.
    ///
    /// # Returns
    ///
    /// A list of reports that are scheduled but not yet published.
    async fn get_pending(&self) -> DomainResult<Vec<ScheduledReport>>;

    /// Processes all reports that are ready to be published.
    ///
    /// # Returns
    ///
    /// The number of reports that were published.
    async fn process_ready(&self) -> DomainResult<usize>;
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn scheduled_report_creation() {
        let publish_at = Timestamp::now().add_secs(900); // 15 min
        let expected_id = BlockTradeId::new_v4();
        let report = ScheduledReport::new(expected_id, ReportingTier::Standard, publish_at);

        assert_eq!(report.block_trade_id(), expected_id);
        assert_eq!(report.tier(), ReportingTier::Standard);
        assert!(!report.is_published());
    }

    #[test]
    fn scheduled_report_mark_published() {
        let mut report = ScheduledReport::new(
            BlockTradeId::new_v4(),
            ReportingTier::Standard,
            Timestamp::now(),
        );

        assert!(!report.is_published());
        report.mark_published();
        assert!(report.is_published());
    }

    #[test]
    fn report_scheduler_config_default() {
        let config = ReportSchedulerConfig::default();

        assert_eq!(config.standard_delay, Duration::from_secs(15 * 60));
        assert_eq!(config.large_delay, Duration::from_secs(60 * 60));
        assert_eq!(config.very_large_delay, Duration::from_secs(8 * 60 * 60));
    }

    #[test]
    fn report_scheduler_config_delay_for_tier() {
        let config = ReportSchedulerConfig::default();

        assert_eq!(
            config.delay_for_tier(ReportingTier::Standard),
            Duration::from_secs(15 * 60)
        );
        assert_eq!(
            config.delay_for_tier(ReportingTier::Large),
            Duration::from_secs(60 * 60)
        );
        assert_eq!(
            config.delay_for_tier(ReportingTier::VeryLarge),
            Duration::from_secs(8 * 60 * 60)
        );
    }
}
