//! # Delayed Report Entity
//!
//! Entity representing a delayed trade report for regulatory compliance.
//!
//! Block trades are reported with tier-based delays to minimize market impact:
//! - Standard: 15 minutes
//! - Large: 60 minutes
//! - VeryLarge: End of Day (EOD)

use crate::domain::services::ReportingTier;
use crate::domain::value_objects::{Instrument, Price, Quantity, Timestamp};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Summary of a trade for public reporting.
///
/// Contains only non-identifying information - no counterparty details.
/// This ensures privacy while meeting regulatory reporting requirements.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TradeSummary {
    /// The traded instrument.
    instrument: Instrument,
    /// Execution price.
    price: Price,
    /// Trade quantity.
    quantity: Quantity,
    /// When the trade was executed.
    executed_at: Timestamp,
}

impl TradeSummary {
    /// Creates a new trade summary.
    #[must_use]
    pub fn new(
        instrument: Instrument,
        price: Price,
        quantity: Quantity,
        executed_at: Timestamp,
    ) -> Self {
        Self {
            instrument,
            price,
            quantity,
            executed_at,
        }
    }

    /// Returns the instrument.
    #[must_use]
    pub fn instrument(&self) -> &Instrument {
        &self.instrument
    }

    /// Returns the price.
    #[must_use]
    pub fn price(&self) -> Price {
        self.price
    }

    /// Returns the quantity.
    #[must_use]
    pub fn quantity(&self) -> Quantity {
        self.quantity
    }

    /// Returns when the trade was executed.
    #[must_use]
    pub fn executed_at(&self) -> Timestamp {
        self.executed_at
    }
}

impl std::fmt::Display for TradeSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} {} @ {} ({})",
            self.quantity,
            self.instrument.symbol(),
            self.price,
            self.executed_at
        )
    }
}

/// A delayed report entity for persistence and processing.
///
/// Tracks the lifecycle of a delayed trade report from scheduling
/// through publication.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DelayedReport {
    /// Unique report identifier.
    id: Uuid,
    /// Associated block trade ID.
    trade_id: String,
    /// Reporting tier determining the delay.
    reporting_tier: ReportingTier,
    /// Trade summary for publication.
    trade_summary: TradeSummary,
    /// Scheduled publication time.
    publish_at: Timestamp,
    /// Actual publication time (None if not yet published).
    published_at: Option<Timestamp>,
    /// When the report was created/scheduled.
    created_at: Timestamp,
}

impl DelayedReport {
    /// Creates a new delayed report.
    #[must_use]
    pub fn new(
        trade_id: String,
        reporting_tier: ReportingTier,
        trade_summary: TradeSummary,
        publish_at: Timestamp,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            trade_id,
            reporting_tier,
            trade_summary,
            publish_at,
            published_at: None,
            created_at: Timestamp::now(),
        }
    }

    /// Creates a delayed report with a specific ID (for persistence recovery).
    #[must_use]
    pub fn with_id(
        id: Uuid,
        trade_id: String,
        reporting_tier: ReportingTier,
        trade_summary: TradeSummary,
        publish_at: Timestamp,
        published_at: Option<Timestamp>,
        created_at: Timestamp,
    ) -> Self {
        Self {
            id,
            trade_id,
            reporting_tier,
            trade_summary,
            publish_at,
            published_at,
            created_at,
        }
    }

    /// Returns the report ID.
    #[must_use]
    pub fn id(&self) -> Uuid {
        self.id
    }

    /// Returns the trade ID.
    #[must_use]
    pub fn trade_id(&self) -> &str {
        &self.trade_id
    }

    /// Returns the reporting tier.
    pub fn reporting_tier(&self) -> ReportingTier {
        self.reporting_tier
    }

    /// Returns the trade summary.
    #[must_use]
    pub fn trade_summary(&self) -> &TradeSummary {
        &self.trade_summary
    }

    /// Returns the scheduled publication time.
    #[must_use]
    pub fn publish_at(&self) -> Timestamp {
        self.publish_at
    }

    /// Returns the actual publication time, if published.
    #[must_use]
    pub fn published_at(&self) -> Option<Timestamp> {
        self.published_at
    }

    /// Returns when the report was created.
    #[must_use]
    pub fn created_at(&self) -> Timestamp {
        self.created_at
    }

    /// Returns whether the report has been published.
    #[must_use]
    pub fn is_published(&self) -> bool {
        self.published_at.is_some()
    }

    /// Returns whether the report is ready to be published.
    #[must_use]
    pub fn is_ready(&self) -> bool {
        !self.is_published() && Timestamp::now() >= self.publish_at
    }

    /// Marks the report as published.
    ///
    /// Sets the published_at timestamp to the current time.
    pub fn mark_published(&mut self) {
        self.published_at = Some(Timestamp::now());
    }

    /// Marks the report as published at a specific time.
    ///
    /// Useful for testing or recovery scenarios.
    pub fn mark_published_at(&mut self, published_at: Timestamp) {
        self.published_at = Some(published_at);
    }

    /// Returns the delay remaining until publication.
    ///
    /// Returns None if the report is ready or already published.
    #[must_use]
    pub fn delay_remaining(&self) -> Option<std::time::Duration> {
        if self.is_published() {
            return None;
        }

        let now = Timestamp::now();
        if now >= self.publish_at {
            return None;
        }

        // Calculate difference in milliseconds
        let remaining_ms = self.publish_at.timestamp_millis() - now.timestamp_millis();
        if remaining_ms <= 0 {
            None
        } else {
            Some(std::time::Duration::from_millis(remaining_ms as u64))
        }
    }
}

impl std::fmt::Display for DelayedReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "DelayedReport[{}] trade={} tier={:?} publish_at={} published={}",
            self.id,
            self.trade_id,
            self.reporting_tier,
            self.publish_at,
            self.is_published()
        )
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::domain::value_objects::Symbol;
    use crate::domain::value_objects::enums::{AssetClass, SettlementMethod};

    fn create_test_summary() -> TradeSummary {
        let symbol = Symbol::new("BTC/USD").unwrap();
        let instrument =
            Instrument::new(symbol, AssetClass::CryptoSpot, SettlementMethod::default());
        TradeSummary::new(
            instrument,
            Price::new(50000.0).unwrap(),
            Quantity::new(10.0).unwrap(),
            Timestamp::now(),
        )
    }

    #[test]
    fn trade_summary_creation() {
        let summary = create_test_summary();
        assert_eq!(summary.instrument().symbol().as_str(), "BTC/USD");
        assert_eq!(summary.price().get(), rust_decimal::Decimal::new(50000, 0));
        assert_eq!(summary.quantity().get(), rust_decimal::Decimal::new(10, 0));
    }

    #[test]
    fn delayed_report_creation() {
        let summary = create_test_summary();
        let publish_at = Timestamp::now().add_secs(900); // 15 min
        let report = DelayedReport::new(
            "trade-123".to_string(),
            ReportingTier::Standard,
            summary,
            publish_at,
        );

        assert_eq!(report.trade_id(), "trade-123");
        assert_eq!(report.reporting_tier(), ReportingTier::Standard);
        assert!(!report.is_published());
        assert!(!report.is_ready());
    }

    #[test]
    fn delayed_report_mark_published() {
        let summary = create_test_summary();
        let mut report = DelayedReport::new(
            "trade-123".to_string(),
            ReportingTier::Standard,
            summary,
            Timestamp::now(),
        );

        assert!(!report.is_published());
        report.mark_published();
        assert!(report.is_published());
        assert!(report.published_at().is_some());
    }

    #[test]
    fn delayed_report_ready_when_time_passed() {
        let summary = create_test_summary();
        let publish_at = Timestamp::now().add_secs(-1); // 1 second ago
        let report = DelayedReport::new(
            "trade-123".to_string(),
            ReportingTier::Standard,
            summary,
            publish_at,
        );

        assert!(report.is_ready());
    }

    #[test]
    fn delayed_report_delay_remaining() {
        let summary = create_test_summary();
        let publish_at = Timestamp::now().add_secs(60); // 60 seconds from now
        let report = DelayedReport::new(
            "trade-123".to_string(),
            ReportingTier::Standard,
            summary,
            publish_at,
        );

        let remaining = report.delay_remaining();
        assert!(remaining.is_some());
        // Should be roughly 60 seconds (allowing some margin)
        let secs = remaining.unwrap().as_secs();
        assert!(secs > 55 && secs <= 60);
    }

    #[test]
    fn trade_summary_display() {
        let summary = create_test_summary();
        let display = format!("{}", summary);
        assert!(display.contains("BTC/USD"));
        assert!(display.contains("50000"));
    }
}
