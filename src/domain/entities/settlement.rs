//! # Incentive Settlement
//!
//! Domain entities for monthly MM incentive settlement cycles.
//!
//! Implements an event-sourced aggregate root pattern where settlements
//! accumulate incentive events over a period and finalize for payout.

use crate::domain::entities::mm_incentive::IncentiveResult;
use crate::domain::value_objects::timestamp::Timestamp;
use crate::domain::value_objects::{CounterpartyId, TradeId};
use chrono::{TimeZone, Utc};
use rust_decimal::{Decimal, RoundingStrategy};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, fmt};
use uuid::Uuid;

// ============================================================================
// SettlementId
// ============================================================================

/// Unique identifier for a settlement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SettlementId(Uuid);

impl SettlementId {
    /// Creates a new random settlement ID.
    #[must_use]
    pub fn new_v4() -> Self {
        Self(Uuid::new_v4())
    }

    /// Creates a settlement ID from a UUID.
    #[must_use]
    pub const fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Returns the inner UUID.
    #[inline]
    #[must_use]
    pub const fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl fmt::Display for SettlementId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ============================================================================
// SettlementPeriod
// ============================================================================

/// Represents a settlement period (typically one calendar month).
///
/// # Invariants
///
/// - Start timestamp must be before end timestamp
/// - Period boundaries are half-open `[start, end)`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettlementPeriod {
    /// Start of the period (inclusive).
    start: Timestamp,
    /// End of the period (exclusive).
    end: Timestamp,
}

impl SettlementPeriod {
    /// Creates a new settlement period.
    ///
    /// # Arguments
    ///
    /// * `start` - Start timestamp (inclusive)
    /// * `end` - End timestamp (exclusive)
    ///
    /// # Returns
    ///
    /// A new `SettlementPeriod` if start < end, otherwise `None`.
    #[must_use]
    pub fn new(start: Timestamp, end: Timestamp) -> Option<Self> {
        if start < end {
            Some(Self { start, end })
        } else {
            None
        }
    }

    /// Creates a settlement period for a specific month and year.
    ///
    /// # Arguments
    ///
    /// * `year` - Year (e.g., 2026)
    /// * `month` - Month (1-12)
    ///
    /// # Returns
    ///
    /// A `SettlementPeriod` covering the entire month, or `None` if invalid month.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let period = SettlementPeriod::from_month_year(2026, 3)?;
    /// // Covers March 1, 2026 00:00:00 to April 1, 2026 00:00:00 (exclusive)
    /// ```
    #[must_use]
    pub fn from_month_year(year: i32, month: u32) -> Option<Self> {
        if !(1..=12).contains(&month) {
            return None;
        }

        let next_month = if month == 12 { 1 } else { month + 1 };
        let next_year = if month == 12 {
            year.checked_add(1)?
        } else {
            year
        };

        let start = Utc
            .with_ymd_and_hms(year, month, 1, 0, 0, 0)
            .single()
            .and_then(|dt| Timestamp::from_millis(dt.timestamp_millis()))?;

        let end = Utc
            .with_ymd_and_hms(next_year, next_month, 1, 0, 0, 0)
            .single()
            .and_then(|dt| Timestamp::from_millis(dt.timestamp_millis()))?;

        Some(Self { start, end })
    }

    /// Returns the start timestamp.
    #[inline]
    #[must_use]
    pub const fn start(&self) -> Timestamp {
        self.start
    }

    /// Returns the end timestamp.
    #[inline]
    #[must_use]
    pub const fn end(&self) -> Timestamp {
        self.end
    }

    /// Checks if a timestamp falls within this period.
    ///
    /// # Arguments
    ///
    /// * `timestamp` - The timestamp to check
    ///
    /// # Returns
    ///
    /// `true` if the timestamp is within `[start, end)`.
    #[inline]
    #[must_use]
    pub fn contains(&self, timestamp: Timestamp) -> bool {
        timestamp >= self.start && timestamp < self.end
    }

    /// Checks if the period has ended (current time is at or after end).
    #[must_use]
    pub fn is_complete(&self) -> bool {
        Timestamp::now() >= self.end
    }
}

impl fmt::Display for SettlementPeriod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}..{}", self.start.to_iso8601(), self.end.to_iso8601())
    }
}

// ============================================================================
// SettlementStatus
// ============================================================================

/// Status of a settlement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SettlementStatus {
    /// Settlement is open and accepting events.
    Open,
    /// Settlement is being finalized (transitional state).
    Finalizing,
    /// Settlement has been finalized and is ready for payout.
    Finalized,
    /// Payout has been processed (external system).
    Paid,
}

impl fmt::Display for SettlementStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Open => write!(f, "OPEN"),
            Self::Finalizing => write!(f, "FINALIZING"),
            Self::Finalized => write!(f, "FINALIZED"),
            Self::Paid => write!(f, "PAID"),
        }
    }
}

// ============================================================================
// IncentiveEvent
// ============================================================================

/// Domain event representing an incentive earned by an MM.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum IncentiveEvent {
    /// A trade incentive was earned.
    TradeIncentiveEarned {
        /// Trade identifier.
        trade_id: TradeId,
        /// Market maker identifier.
        mm_id: CounterpartyId,
        /// Incentive calculation result.
        result: IncentiveResult,
        /// Trade notional in USD.
        notional: Decimal,
        /// Timestamp when the incentive was earned.
        timestamp: Timestamp,
    },
}

impl IncentiveEvent {
    /// Creates a new trade incentive earned event.
    #[must_use]
    pub fn trade_incentive_earned(
        trade_id: TradeId,
        mm_id: CounterpartyId,
        result: IncentiveResult,
        notional: Decimal,
        timestamp: Timestamp,
    ) -> Self {
        Self::TradeIncentiveEarned {
            trade_id,
            mm_id,
            result,
            notional,
            timestamp,
        }
    }

    /// Returns the market maker ID from the event.
    #[must_use]
    pub fn mm_id(&self) -> &CounterpartyId {
        match self {
            Self::TradeIncentiveEarned { mm_id, .. } => mm_id,
        }
    }

    /// Returns the trade ID from the event.
    #[must_use]
    pub fn trade_id(&self) -> &TradeId {
        match self {
            Self::TradeIncentiveEarned { trade_id, .. } => trade_id,
        }
    }

    /// Returns the incentive calculation result from the event.
    #[must_use]
    pub fn result(&self) -> &IncentiveResult {
        match self {
            Self::TradeIncentiveEarned { result, .. } => result,
        }
    }

    /// Returns the trade notional from the event.
    #[must_use]
    pub fn notional(&self) -> Decimal {
        match self {
            Self::TradeIncentiveEarned { notional, .. } => *notional,
        }
    }

    /// Returns the timestamp from the event.
    #[must_use]
    pub fn timestamp(&self) -> Timestamp {
        match self {
            Self::TradeIncentiveEarned { timestamp, .. } => *timestamp,
        }
    }
}

// ============================================================================
// IncentiveSettlement
// ============================================================================

/// Aggregate root for MM incentive settlement.
///
/// Accumulates incentive events over a period and finalizes for payout.
/// Implements event sourcing pattern for idempotency and audit trail.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IncentiveSettlement {
    /// Unique settlement identifier.
    id: SettlementId,
    /// Market maker identifier.
    mm_id: CounterpartyId,
    /// Settlement period.
    period: SettlementPeriod,
    /// Current status.
    status: SettlementStatus,
    /// Total number of trades in this settlement.
    total_trades: u64,
    /// Total trade volume in USD.
    total_volume_usd: Decimal,
    /// Total base rebates earned (negative value = payment to MM).
    total_base_rebates_usd: Decimal,
    /// Total spread bonuses earned (negative value = payment to MM).
    total_bonuses_usd: Decimal,
    /// Net payout amount (negative value = payment to MM).
    net_payout_usd: Decimal,
    /// Events that have been applied to this settlement.
    events: Vec<IncentiveEvent>,
    /// Trade IDs that have already been applied.
    applied_trade_ids: HashSet<TradeId>,
    /// Version number (incremented with each event).
    version: u64,
}

impl IncentiveSettlement {
    /// Creates a new settlement for a market maker and period.
    ///
    /// # Arguments
    ///
    /// * `mm_id` - Market maker identifier
    /// * `period` - Settlement period
    #[must_use]
    pub fn new(mm_id: CounterpartyId, period: SettlementPeriod) -> Self {
        Self {
            id: SettlementId::new_v4(),
            mm_id,
            period,
            status: SettlementStatus::Open,
            total_trades: 0,
            total_volume_usd: Decimal::ZERO,
            total_base_rebates_usd: Decimal::ZERO,
            total_bonuses_usd: Decimal::ZERO,
            net_payout_usd: Decimal::ZERO,
            events: Vec::new(),
            applied_trade_ids: HashSet::new(),
            version: 0,
        }
    }

    /// Returns the settlement ID.
    #[inline]
    #[must_use]
    pub const fn id(&self) -> SettlementId {
        self.id
    }

    /// Returns the market maker ID.
    #[inline]
    #[must_use]
    pub fn mm_id(&self) -> &CounterpartyId {
        &self.mm_id
    }

    /// Returns the settlement period.
    #[inline]
    #[must_use]
    pub const fn period(&self) -> SettlementPeriod {
        self.period
    }

    /// Returns the current status.
    #[inline]
    #[must_use]
    pub const fn status(&self) -> SettlementStatus {
        self.status
    }

    /// Returns the total number of trades.
    #[inline]
    #[must_use]
    pub const fn total_trades(&self) -> u64 {
        self.total_trades
    }

    /// Returns the total volume in USD.
    #[inline]
    #[must_use]
    pub fn total_volume_usd(&self) -> Decimal {
        self.total_volume_usd
    }

    /// Returns the total base rebates in USD.
    #[inline]
    #[must_use]
    pub fn total_base_rebates_usd(&self) -> Decimal {
        self.total_base_rebates_usd
    }

    /// Returns the total bonuses in USD.
    #[inline]
    #[must_use]
    pub fn total_bonuses_usd(&self) -> Decimal {
        self.total_bonuses_usd
    }

    /// Returns the net payout amount in USD.
    #[inline]
    #[must_use]
    pub fn net_payout_usd(&self) -> Decimal {
        self.net_payout_usd
    }

    /// Returns the version number.
    #[inline]
    #[must_use]
    pub const fn version(&self) -> u64 {
        self.version
    }

    /// Returns a reference to the events.
    #[inline]
    #[must_use]
    pub fn events(&self) -> &[IncentiveEvent] {
        &self.events
    }

    /// Applies an incentive event to this settlement.
    ///
    /// Updates aggregated totals based on the event type.
    /// This is a pure function that modifies the settlement state.
    ///
    /// # Arguments
    ///
    /// * `event` - The incentive event to apply
    ///
    /// # Errors
    ///
    /// Returns `SettlementError::SettlementClosed` if the settlement is not open.
    /// Returns `SettlementError::MismatchedMarketMaker` if the event MM does not match.
    /// Returns `SettlementError::Overflow` if any accumulated monetary value overflows.
    pub fn apply_event(&mut self, event: IncentiveEvent) -> Result<(), SettlementError> {
        if self.status != SettlementStatus::Open {
            return Err(SettlementError::SettlementClosed {
                status: self.status,
            });
        }

        let mm_id = event.mm_id().clone();
        if mm_id != self.mm_id {
            return Err(SettlementError::MismatchedMarketMaker {
                expected: self.mm_id.to_string(),
                actual: mm_id.to_string(),
            });
        }

        let trade_id = *event.trade_id();
        if self.applied_trade_ids.contains(&trade_id) {
            return Ok(());
        }

        let notional = event.notional();
        let result = event.result();

        // Increment trade count (checked)
        self.total_trades = self
            .total_trades
            .checked_add(1)
            .ok_or(SettlementError::Overflow("total_trades"))?;

        // Add to total volume (checked)
        self.total_volume_usd = self
            .total_volume_usd
            .checked_add(notional)
            .ok_or(SettlementError::Overflow("total_volume_usd"))?;

        // Add base rebate (explicit rounding, checked arithmetic)
        let base_rebate = amount_from_bps(notional, result.base_rebate_bps())?;
        self.total_base_rebates_usd = self
            .total_base_rebates_usd
            .checked_add(base_rebate)
            .ok_or(SettlementError::Overflow("total_base_rebates_usd"))?;

        // Add spread bonus separately (explicit rounding, checked arithmetic)
        let bonus_amount = amount_from_bps(notional, result.spread_bonus_bps())?;
        self.total_bonuses_usd = self
            .total_bonuses_usd
            .checked_add(bonus_amount)
            .ok_or(SettlementError::Overflow("total_bonuses_usd"))?;

        // Store event and increment version
        self.applied_trade_ids.insert(trade_id);
        self.events.push(event);
        self.version = self
            .version
            .checked_add(1)
            .ok_or(SettlementError::Overflow("version"))?;

        Ok(())
    }

    /// Finalizes the settlement, calculating net payout and transitioning status.
    ///
    /// # Errors
    ///
    /// Returns an error if the settlement is already finalized.
    pub fn finalize(&mut self) -> Result<(), SettlementError> {
        if self.status != SettlementStatus::Open {
            return Err(SettlementError::AlreadyFinalized);
        }

        // Calculate net payout (sum of base rebates and bonuses)
        self.net_payout_usd = self
            .total_base_rebates_usd
            .checked_add(self.total_bonuses_usd)
            .ok_or(SettlementError::Overflow("net_payout_usd"))?;

        // Transition to finalized status
        self.status = SettlementStatus::Finalized;

        Ok(())
    }

    /// Marks the settlement as paid (external payout completed).
    ///
    /// # Errors
    ///
    /// Returns an error if the settlement is not finalized.
    pub fn mark_as_paid(&mut self) -> Result<(), SettlementError> {
        if self.status != SettlementStatus::Finalized {
            return Err(SettlementError::NotFinalized);
        }

        self.status = SettlementStatus::Paid;
        Ok(())
    }
}

// ============================================================================
// SettlementError
// ============================================================================

/// Errors that can occur during settlement operations.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SettlementError {
    /// Settlement is already finalized.
    #[error("settlement is already finalized")]
    AlreadyFinalized,

    /// Settlement is not open for new events.
    #[error("settlement is not open for new events, current status is {status}")]
    SettlementClosed {
        /// Current settlement status.
        status: SettlementStatus,
    },

    /// Settlement is not finalized.
    #[error("settlement is not finalized")]
    NotFinalized,

    /// Settlement not found.
    #[error("settlement not found for MM {mm_id} and period {period}")]
    NotFound {
        /// Market maker identifier.
        mm_id: String,
        /// Settlement period.
        period: SettlementPeriod,
    },

    /// The market maker on the event does not match the settlement.
    #[error("event market maker {actual} does not match settlement market maker {expected}")]
    MismatchedMarketMaker {
        /// Expected MM identifier.
        expected: String,
        /// Actual MM identifier.
        actual: String,
    },

    /// Monetary accumulation overflowed.
    #[error("monetary overflow while updating {0}")]
    Overflow(&'static str),

    /// Invalid settlement period.
    #[error("invalid settlement period: {0}")]
    InvalidPeriod(String),

    /// Repository error.
    #[error("repository error: {0}")]
    Repository(String),
}

fn amount_from_bps(notional: Decimal, bps: Decimal) -> Result<Decimal, SettlementError> {
    let bps_divisor = Decimal::from(10_000);
    notional
        .checked_mul(bps)
        .and_then(|product| product.checked_div(bps_divisor))
        .map(|value| value.round_dp_with_strategy(8, RoundingStrategy::MidpointAwayFromZero))
        .ok_or(SettlementError::Overflow("amount_from_bps"))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::domain::entities::mm_incentive::{
        IncentiveConfig, IncentiveTier, compute_incentive,
    };

    fn make_timestamp(
        year: i32,
        month: u32,
        day: u32,
        hour: u32,
        minute: u32,
        second: u32,
    ) -> Timestamp {
        Timestamp::from_millis(
            Utc.with_ymd_and_hms(year, month, day, hour, minute, second)
                .single()
                .unwrap()
                .timestamp_millis(),
        )
        .unwrap()
    }

    fn make_event(
        trade_id: TradeId,
        mm_id: CounterpartyId,
        notional: Decimal,
        spread_bps: Option<Decimal>,
        timestamp: Timestamp,
    ) -> IncentiveEvent {
        let config = IncentiveConfig::default();
        let result = compute_incentive(IncentiveTier::Bronze, notional, spread_bps, &config);

        IncentiveEvent::trade_incentive_earned(trade_id, mm_id, result, notional, timestamp)
    }

    #[test]
    fn month_period_is_half_open_and_includes_subseconds() {
        let period = SettlementPeriod::from_month_year(2026, 12).unwrap();
        let inside = make_timestamp(2026, 12, 31, 23, 59, 59).add_millis(500);
        let next_month = make_timestamp(2027, 1, 1, 0, 0, 0);

        assert!(period.contains(inside));
        assert!(!period.contains(next_month));
        assert_eq!(period.start(), make_timestamp(2026, 12, 1, 0, 0, 0));
        assert_eq!(period.end(), next_month);
    }

    #[test]
    fn apply_event_deduplicates_trade_ids() {
        let mm_id = CounterpartyId::new("mm-1");
        let period = SettlementPeriod::from_month_year(2026, 3).unwrap();
        let mut settlement = IncentiveSettlement::new(mm_id.clone(), period);
        let trade_id = TradeId::new_v4();
        let timestamp = make_timestamp(2026, 3, 15, 10, 0, 0);
        let event = make_event(trade_id, mm_id, Decimal::from(1_000_000), None, timestamp);

        assert!(settlement.apply_event(event.clone()).is_ok());
        assert!(settlement.apply_event(event).is_ok());
        assert_eq!(settlement.total_trades(), 1);
        assert_eq!(settlement.events().len(), 1);
    }

    #[test]
    fn apply_event_rejects_closed_settlement() {
        let mm_id = CounterpartyId::new("mm-1");
        let period = SettlementPeriod::from_month_year(2026, 3).unwrap();
        let mut settlement = IncentiveSettlement::new(mm_id.clone(), period);
        let event = make_event(
            TradeId::new_v4(),
            mm_id,
            Decimal::from(1_000_000),
            None,
            make_timestamp(2026, 3, 15, 10, 0, 0),
        );

        assert!(settlement.finalize().is_ok());
        let result = settlement.apply_event(event);
        assert!(matches!(
            result,
            Err(SettlementError::SettlementClosed { .. })
        ));
    }

    #[test]
    fn apply_event_calculates_base_and_bonus_separately() {
        let mm_id = CounterpartyId::new("mm-1");
        let period = SettlementPeriod::from_month_year(2026, 3).unwrap();
        let mut settlement = IncentiveSettlement::new(mm_id.clone(), period);
        let notional = Decimal::from(1_000_000);
        let timestamp = make_timestamp(2026, 3, 15, 10, 0, 0);
        let event = make_event(
            TradeId::new_v4(),
            mm_id,
            notional,
            Some(Decimal::from(3)),
            timestamp,
        );

        let result = event.result();
        let base = amount_from_bps(notional, result.base_rebate_bps()).unwrap();
        let bonus = amount_from_bps(notional, result.spread_bonus_bps()).unwrap();

        assert!(settlement.apply_event(event).is_ok());
        assert_eq!(settlement.total_base_rebates_usd(), base);
        assert_eq!(settlement.total_bonuses_usd(), bonus);
        assert_eq!(settlement.net_payout_usd(), Decimal::ZERO);
    }
}
