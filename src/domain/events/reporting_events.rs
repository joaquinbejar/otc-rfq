//! # Reporting Domain Events
//!
//! Events related to trade reporting and publication.

use crate::domain::entities::delayed_report::TradeSummary;
use crate::domain::events::domain_event::{DomainEvent, EventMetadata, EventType};
use crate::domain::services::ReportingTier;
use crate::domain::value_objects::{BlockTradeId, EventId, RfqId, Timestamp};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Event emitted when a block trade report is published.
///
/// This event is emitted after the required delay has passed
/// and the trade summary is made available to the market data feed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockTradeReported {
    /// Event metadata.
    metadata: EventMetadata,
    /// Report ID.
    report_id: Uuid,
    /// Associated trade ID.
    trade_id: BlockTradeId,
    /// Reporting tier that determined the delay.
    reporting_tier: ReportingTier,
    /// Trade summary (no counterparty info).
    trade_summary: TradeSummary,
    /// When the report was scheduled.
    scheduled_at: Timestamp,
    /// When the report was actually published.
    published_at: Timestamp,
}

impl BlockTradeReported {
    /// Creates a new BlockTradeReported event.
    #[must_use]
    pub fn new(
        report_id: Uuid,
        trade_id: BlockTradeId,
        reporting_tier: ReportingTier,
        trade_summary: TradeSummary,
        scheduled_at: Timestamp,
        published_at: Timestamp,
    ) -> Self {
        Self {
            metadata: EventMetadata::new(None),
            report_id,
            trade_id,
            reporting_tier,
            trade_summary,
            scheduled_at,
            published_at,
        }
    }

    /// Returns the report ID.
    #[must_use]
    pub fn report_id(&self) -> Uuid {
        self.report_id
    }

    /// Returns the trade ID.
    #[must_use]
    pub fn trade_id(&self) -> BlockTradeId {
        self.trade_id
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

    /// Returns when the report was scheduled.
    #[must_use]
    pub fn scheduled_at(&self) -> Timestamp {
        self.scheduled_at
    }

    /// Returns when the report was published.
    #[must_use]
    pub fn published_at(&self) -> Timestamp {
        self.published_at
    }

    /// Returns the actual delay from scheduling to publication.
    ///
    /// Returns the delay with millisecond precision for accurate audit trails.
    #[must_use]
    pub fn actual_delay(&self) -> std::time::Duration {
        self.scheduled_at.duration_until(&self.published_at)
    }
}

impl DomainEvent for BlockTradeReported {
    fn event_id(&self) -> EventId {
        self.metadata.event_id
    }

    fn rfq_id(&self) -> Option<RfqId> {
        self.metadata.rfq_id
    }

    fn timestamp(&self) -> Timestamp {
        self.metadata.timestamp
    }

    fn event_type(&self) -> EventType {
        EventType::Trade
    }

    fn event_name(&self) -> &'static str {
        "BlockTradeReported"
    }
}

/// Event emitted when a report is scheduled for delayed publication.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportScheduled {
    /// Event metadata.
    metadata: EventMetadata,
    /// Report ID.
    report_id: Uuid,
    /// Associated trade ID.
    trade_id: BlockTradeId,
    /// Reporting tier.
    reporting_tier: ReportingTier,
    /// Scheduled publication time.
    publish_at: Timestamp,
}

impl ReportScheduled {
    /// Creates a new ReportScheduled event.
    #[must_use]
    pub fn new(
        report_id: Uuid,
        trade_id: BlockTradeId,
        reporting_tier: ReportingTier,
        publish_at: Timestamp,
    ) -> Self {
        Self {
            metadata: EventMetadata::new(None),
            report_id,
            trade_id,
            reporting_tier,
            publish_at,
        }
    }

    /// Returns the report ID.
    #[must_use]
    pub fn report_id(&self) -> Uuid {
        self.report_id
    }

    /// Returns the trade ID.
    #[must_use]
    pub fn trade_id(&self) -> BlockTradeId {
        self.trade_id
    }

    /// Returns the reporting tier.
    pub fn reporting_tier(&self) -> ReportingTier {
        self.reporting_tier
    }

    /// Returns the scheduled publication time.
    #[must_use]
    pub fn publish_at(&self) -> Timestamp {
        self.publish_at
    }
}

impl DomainEvent for ReportScheduled {
    fn event_id(&self) -> EventId {
        self.metadata.event_id
    }

    fn rfq_id(&self) -> Option<RfqId> {
        self.metadata.rfq_id
    }

    fn timestamp(&self) -> Timestamp {
        self.metadata.timestamp
    }

    fn event_type(&self) -> EventType {
        EventType::Trade
    }

    fn event_name(&self) -> &'static str {
        "ReportScheduled"
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::manual_range_contains)]
mod tests {
    use super::*;
    use crate::domain::value_objects::enums::{AssetClass, SettlementMethod};
    use crate::domain::value_objects::{Instrument, Price, Quantity, Symbol};

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
    fn block_trade_reported_creation() {
        let summary = create_test_summary();
        let scheduled_at = Timestamp::now().add_secs(-900);
        let published_at = Timestamp::now();
        let expected_id = BlockTradeId::new_v4();

        let event = BlockTradeReported::new(
            Uuid::new_v4(),
            expected_id,
            ReportingTier::Standard,
            summary,
            scheduled_at,
            published_at,
        );

        assert_eq!(event.trade_id(), expected_id);
        assert_eq!(event.reporting_tier(), ReportingTier::Standard);
        assert_eq!(event.event_type(), EventType::Trade);
        assert_eq!(event.event_name(), "BlockTradeReported");
    }

    #[test]
    fn report_scheduled_creation() {
        let publish_at = Timestamp::now().add_secs(900);
        let expected_id = BlockTradeId::new_v4();
        let event = ReportScheduled::new(
            Uuid::new_v4(),
            expected_id,
            ReportingTier::Standard,
            publish_at,
        );

        assert_eq!(event.trade_id(), expected_id);
        assert_eq!(event.event_type(), EventType::Trade);
        assert_eq!(event.event_name(), "ReportScheduled");
    }

    #[test]
    fn actual_delay_calculation() {
        let summary = create_test_summary();
        let scheduled_at = Timestamp::now().add_secs(-900); // 15 min ago
        let published_at = Timestamp::now();

        let event = BlockTradeReported::new(
            Uuid::new_v4(),
            BlockTradeId::new_v4(),
            ReportingTier::Standard,
            summary,
            scheduled_at,
            published_at,
        );

        let delay = event.actual_delay();
        assert!(delay.as_secs() >= 899 && delay.as_secs() <= 901);
    }
}
