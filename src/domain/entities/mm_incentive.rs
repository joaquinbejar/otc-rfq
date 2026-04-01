//! # Market Maker Incentive Program
//!
//! Defines incentive tiers, rebate calculations, and penalty evaluation for market makers.

use crate::domain::entities::mm_performance::MmPerformanceMetrics;
use crate::domain::value_objects::CounterpartyId;
use crate::domain::value_objects::timestamp::Timestamp;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Incentive tier levels for market makers.
///
/// Tiers are assigned based on monthly trading volume and determine
/// the rebate rates and bonuses available to the market maker.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[repr(u8)]
pub enum IncentiveTier {
    /// Entry-level tier for new or low-volume market makers.
    #[default]
    Bronze = 0,
    /// Mid-tier for moderate volume market makers.
    Silver = 1,
    /// High-tier for significant volume market makers.
    Gold = 2,
    /// Top-tier for highest volume market makers.
    Platinum = 3,
}

impl fmt::Display for IncentiveTier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bronze => write!(f, "BRONZE"),
            Self::Silver => write!(f, "SILVER"),
            Self::Gold => write!(f, "GOLD"),
            Self::Platinum => write!(f, "PLATINUM"),
        }
    }
}

impl IncentiveTier {
    /// Determines the tier based on monthly volume.
    #[must_use]
    pub fn from_volume(volume: Decimal, config: &IncentiveConfig) -> Self {
        if volume >= config.platinum_threshold() {
            Self::Platinum
        } else if volume >= config.gold_threshold() {
            Self::Gold
        } else if volume >= config.silver_threshold() {
            Self::Silver
        } else {
            Self::Bronze
        }
    }

    /// Returns the rebate rate in basis points for this tier.
    #[must_use]
    pub fn rebate_bps(&self, config: &IncentiveConfig) -> Decimal {
        match self {
            Self::Bronze => config.bronze_rebate_bps(),
            Self::Silver => config.silver_rebate_bps(),
            Self::Gold => config.gold_rebate_bps(),
            Self::Platinum => config.platinum_rebate_bps(),
        }
    }

    /// Returns the next tier down (for downgrades). Bronze stays Bronze.
    #[must_use]
    pub const fn downgrade(&self) -> Self {
        match self {
            Self::Platinum => Self::Gold,
            Self::Gold => Self::Silver,
            Self::Silver => Self::Bronze,
            Self::Bronze => Self::Bronze,
        }
    }

    /// Returns the numeric value of this tier.
    #[inline]
    #[must_use]
    pub const fn as_u8(&self) -> u8 {
        *self as u8
    }

    /// Returns the next tier up (for upgrades). Platinum returns `None`.
    #[must_use]
    pub const fn next_tier(&self) -> Option<Self> {
        match self {
            Self::Bronze => Some(Self::Silver),
            Self::Silver => Some(Self::Gold),
            Self::Gold => Some(Self::Platinum),
            Self::Platinum => None,
        }
    }

    /// Returns the volume threshold for this tier.
    ///
    /// Bronze has no threshold (returns `Decimal::ZERO`).
    #[must_use]
    pub fn threshold(&self, config: &IncentiveConfig) -> Decimal {
        match self {
            Self::Bronze => Decimal::ZERO,
            Self::Silver => config.silver_threshold(),
            Self::Gold => config.gold_threshold(),
            Self::Platinum => config.platinum_threshold(),
        }
    }
}

/// Configuration for the incentive program.
///
/// All monetary values are in USD. Rebate rates are in basis points
/// (negative values indicate rebates paid to the MM).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IncentiveConfig {
    /// Volume threshold for Silver tier (USD).
    silver_threshold: Decimal,
    /// Volume threshold for Gold tier (USD).
    gold_threshold: Decimal,
    /// Volume threshold for Platinum tier (USD).
    platinum_threshold: Decimal,
    /// Rebate rate for Bronze tier (basis points).
    bronze_rebate_bps: Decimal,
    /// Rebate rate for Silver tier (basis points).
    silver_rebate_bps: Decimal,
    /// Rebate rate for Gold tier (basis points).
    gold_rebate_bps: Decimal,
    /// Rebate rate for Platinum tier (basis points).
    platinum_rebate_bps: Decimal,
    /// Spread threshold for bonus (basis points vs reference).
    spread_bonus_threshold_bps: Decimal,
    /// Extra rebate for tight spreads (basis points).
    spread_bonus_bps: Decimal,
    /// Response rate below this triggers penalty (percentage).
    low_response_rate_pct: f64,
    /// Reject rate above this triggers penalty (percentage).
    high_reject_rate_pct: f64,
    /// Capacity reduction for penalties (as decimal, e.g., 0.25 = 25%).
    capacity_penalty_pct: Decimal,
}

impl Default for IncentiveConfig {
    fn default() -> Self {
        Self {
            silver_threshold: Decimal::from(1_000_000),
            gold_threshold: Decimal::from(10_000_000),
            platinum_threshold: Decimal::from(50_000_000),
            bronze_rebate_bps: Decimal::new(-10, 1), // -1.0 bps
            silver_rebate_bps: Decimal::new(-20, 1), // -2.0 bps
            gold_rebate_bps: Decimal::new(-30, 1),   // -3.0 bps
            platinum_rebate_bps: Decimal::new(-50, 1), // -5.0 bps
            spread_bonus_threshold_bps: Decimal::new(50, 1), // 5.0 bps
            spread_bonus_bps: Decimal::new(-5, 1),   // -0.5 bps extra
            low_response_rate_pct: 80.0,
            high_reject_rate_pct: 20.0,
            capacity_penalty_pct: Decimal::new(25, 2), // 0.25 = 25%
        }
    }
}

impl IncentiveConfig {
    /// Creates a new incentive configuration with custom values.
    #[must_use]
    pub fn new(
        silver_threshold: Decimal,
        gold_threshold: Decimal,
        platinum_threshold: Decimal,
    ) -> Self {
        Self {
            silver_threshold,
            gold_threshold,
            platinum_threshold,
            ..Self::default()
        }
    }

    /// Creates a builder for constructing an incentive configuration.
    pub fn builder() -> IncentiveConfigBuilder {
        IncentiveConfigBuilder::default()
    }

    /// Returns the Silver tier volume threshold.
    #[inline]
    #[must_use]
    pub fn silver_threshold(&self) -> Decimal {
        self.silver_threshold
    }

    /// Returns the Gold tier volume threshold.
    #[inline]
    #[must_use]
    pub fn gold_threshold(&self) -> Decimal {
        self.gold_threshold
    }

    /// Returns the Platinum tier volume threshold.
    #[inline]
    #[must_use]
    pub fn platinum_threshold(&self) -> Decimal {
        self.platinum_threshold
    }

    /// Returns the Bronze tier rebate rate in basis points.
    #[inline]
    #[must_use]
    pub fn bronze_rebate_bps(&self) -> Decimal {
        self.bronze_rebate_bps
    }

    /// Returns the Silver tier rebate rate in basis points.
    #[inline]
    #[must_use]
    pub fn silver_rebate_bps(&self) -> Decimal {
        self.silver_rebate_bps
    }

    /// Returns the Gold tier rebate rate in basis points.
    #[inline]
    #[must_use]
    pub fn gold_rebate_bps(&self) -> Decimal {
        self.gold_rebate_bps
    }

    /// Returns the Platinum tier rebate rate in basis points.
    #[inline]
    #[must_use]
    pub fn platinum_rebate_bps(&self) -> Decimal {
        self.platinum_rebate_bps
    }

    /// Returns the spread bonus threshold in basis points.
    #[inline]
    #[must_use]
    pub fn spread_bonus_threshold_bps(&self) -> Decimal {
        self.spread_bonus_threshold_bps
    }

    /// Returns the spread bonus rate in basis points.
    #[inline]
    #[must_use]
    pub fn spread_bonus_bps(&self) -> Decimal {
        self.spread_bonus_bps
    }

    /// Returns the low response rate penalty threshold (percentage).
    #[inline]
    #[must_use]
    pub const fn low_response_rate_pct(&self) -> f64 {
        self.low_response_rate_pct
    }

    /// Returns the high reject rate penalty threshold (percentage).
    #[inline]
    #[must_use]
    pub const fn high_reject_rate_pct(&self) -> f64 {
        self.high_reject_rate_pct
    }

    /// Returns the capacity penalty percentage.
    #[inline]
    #[must_use]
    pub fn capacity_penalty_pct(&self) -> Decimal {
        self.capacity_penalty_pct
    }
}

/// Builder for [`IncentiveConfig`].
#[derive(Debug, Clone, Default)]
#[must_use]
pub struct IncentiveConfigBuilder {
    config: IncentiveConfig,
}

impl IncentiveConfigBuilder {
    /// Sets the Silver tier volume threshold.
    pub fn silver_threshold(mut self, value: Decimal) -> Self {
        self.config.silver_threshold = value;
        self
    }

    /// Sets the Gold tier volume threshold.
    pub fn gold_threshold(mut self, value: Decimal) -> Self {
        self.config.gold_threshold = value;
        self
    }

    /// Sets the Platinum tier volume threshold.
    pub fn platinum_threshold(mut self, value: Decimal) -> Self {
        self.config.platinum_threshold = value;
        self
    }

    /// Sets the Bronze tier rebate rate.
    pub fn bronze_rebate_bps(mut self, value: Decimal) -> Self {
        self.config.bronze_rebate_bps = value;
        self
    }

    /// Sets the Silver tier rebate rate.
    pub fn silver_rebate_bps(mut self, value: Decimal) -> Self {
        self.config.silver_rebate_bps = value;
        self
    }

    /// Sets the Gold tier rebate rate.
    pub fn gold_rebate_bps(mut self, value: Decimal) -> Self {
        self.config.gold_rebate_bps = value;
        self
    }

    /// Sets the Platinum tier rebate rate.
    pub fn platinum_rebate_bps(mut self, value: Decimal) -> Self {
        self.config.platinum_rebate_bps = value;
        self
    }

    /// Sets the spread bonus threshold.
    pub fn spread_bonus_threshold_bps(mut self, value: Decimal) -> Self {
        self.config.spread_bonus_threshold_bps = value;
        self
    }

    /// Sets the spread bonus rate.
    pub fn spread_bonus_bps(mut self, value: Decimal) -> Self {
        self.config.spread_bonus_bps = value;
        self
    }

    /// Sets the low response rate penalty threshold.
    pub fn low_response_rate_pct(mut self, value: f64) -> Self {
        self.config.low_response_rate_pct = value;
        self
    }

    /// Sets the high reject rate penalty threshold.
    pub fn high_reject_rate_pct(mut self, value: f64) -> Self {
        self.config.high_reject_rate_pct = value;
        self
    }

    /// Sets the capacity penalty percentage.
    pub fn capacity_penalty_pct(mut self, value: Decimal) -> Self {
        self.config.capacity_penalty_pct = value;
        self
    }

    /// Builds the configuration.
    pub fn build(self) -> IncentiveConfig {
        self.config
    }
}

/// Result of computing incentive for a trade.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IncentiveResult {
    /// The MM's current tier.
    tier: IncentiveTier,
    /// Base rebate from tier (basis points).
    base_rebate_bps: Decimal,
    /// Extra rebate for tight spread (basis points).
    spread_bonus_bps: Decimal,
    /// Total rebate (base + spread bonus).
    total_rebate_bps: Decimal,
    /// Rebate amount in USD (notional * total_rebate_bps / 10000).
    rebate_amount: Decimal,
}

impl IncentiveResult {
    /// Returns the tier.
    #[inline]
    #[must_use]
    pub const fn tier(&self) -> IncentiveTier {
        self.tier
    }

    /// Returns the base rebate in basis points.
    #[inline]
    #[must_use]
    pub fn base_rebate_bps(&self) -> Decimal {
        self.base_rebate_bps
    }

    /// Returns the spread bonus in basis points.
    #[inline]
    #[must_use]
    pub fn spread_bonus_bps(&self) -> Decimal {
        self.spread_bonus_bps
    }

    /// Returns the total rebate in basis points.
    #[inline]
    #[must_use]
    pub fn total_rebate_bps(&self) -> Decimal {
        self.total_rebate_bps
    }

    /// Returns the rebate amount in USD.
    #[inline]
    #[must_use]
    pub fn rebate_amount(&self) -> Decimal {
        self.rebate_amount
    }
}

impl fmt::Display for IncentiveResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Incentive({} tier, {:.2}bps total, ${:.2} rebate)",
            self.tier, self.total_rebate_bps, self.rebate_amount
        )
    }
}

/// Result of evaluating penalties for a market maker.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PenaltyResult {
    /// Whether capacity should be reduced.
    should_reduce_capacity: bool,
    /// Capacity reduction percentage (e.g., 0.25 = 25%).
    capacity_reduction_pct: Decimal,
    /// Whether the MM should be downgraded a tier.
    should_downgrade_tier: bool,
    /// Reason for the penalty, if any.
    reason: Option<String>,
}

impl PenaltyResult {
    /// Creates a result indicating no penalties.
    #[must_use]
    pub fn none() -> Self {
        Self {
            should_reduce_capacity: false,
            capacity_reduction_pct: Decimal::ZERO,
            should_downgrade_tier: false,
            reason: None,
        }
    }

    /// Returns whether capacity should be reduced.
    #[inline]
    #[must_use]
    pub const fn should_reduce_capacity(&self) -> bool {
        self.should_reduce_capacity
    }

    /// Returns the capacity reduction percentage.
    #[inline]
    #[must_use]
    pub fn capacity_reduction_pct(&self) -> Decimal {
        self.capacity_reduction_pct
    }

    /// Returns whether the MM should be downgraded.
    #[inline]
    #[must_use]
    pub const fn should_downgrade_tier(&self) -> bool {
        self.should_downgrade_tier
    }

    /// Returns the penalty reason.
    #[inline]
    #[must_use]
    pub fn reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }

    /// Returns true if any penalty applies.
    #[inline]
    #[must_use]
    pub fn has_penalty(&self) -> bool {
        self.should_reduce_capacity || self.should_downgrade_tier
    }
}

impl fmt::Display for PenaltyResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.has_penalty() {
            write!(
                f,
                "Penalty(capacity_reduction={}, tier_downgrade={}, reason={:?})",
                self.should_reduce_capacity, self.should_downgrade_tier, self.reason
            )
        } else {
            write!(f, "Penalty(none)")
        }
    }
}

// ============================================================================
// Pure Functions
// ============================================================================

/// Basis points divisor for converting bps to decimal.
const BPS_DIVISOR: Decimal = Decimal::from_parts(10000, 0, 0, false, 0);

/// Computes the incentive for a single trade.
///
/// # Arguments
///
/// * `tier` - The MM's current incentive tier
/// * `notional` - Trade notional in USD
/// * `spread_vs_reference_bps` - Spread vs reference price (None if not available)
/// * `config` - Incentive program configuration
///
/// # Returns
///
/// An [`IncentiveResult`] with the computed rebates.
#[must_use]
pub fn compute_incentive(
    tier: IncentiveTier,
    notional: Decimal,
    spread_vs_reference_bps: Option<Decimal>,
    config: &IncentiveConfig,
) -> IncentiveResult {
    let base_rebate_bps = tier.rebate_bps(config);

    // Check if spread qualifies for bonus
    let spread_bonus_bps = match spread_vs_reference_bps {
        Some(spread) if spread <= config.spread_bonus_threshold_bps() => config.spread_bonus_bps(),
        _ => Decimal::ZERO,
    };

    let total_rebate_bps = base_rebate_bps + spread_bonus_bps;

    // Calculate rebate amount: notional * (total_rebate_bps / 10000)
    // Note: rebate_bps is negative, so rebate_amount will be negative (payment to MM)
    let rebate_amount = notional * total_rebate_bps / BPS_DIVISOR;

    IncentiveResult {
        tier,
        base_rebate_bps,
        spread_bonus_bps,
        total_rebate_bps,
        rebate_amount,
    }
}

/// Evaluates penalties based on performance metrics.
///
/// # Arguments
///
/// * `metrics` - The MM's performance metrics
/// * `config` - Incentive program configuration
///
/// # Returns
///
/// A [`PenaltyResult`] indicating what penalties should be applied.
#[must_use]
pub fn evaluate_penalties(
    metrics: &MmPerformanceMetrics,
    config: &IncentiveConfig,
) -> PenaltyResult {
    let mut should_reduce_capacity = false;
    let mut should_downgrade_tier = false;
    let mut reasons = Vec::new();

    // Check response rate
    if let Some(response_rate) = metrics.response_rate_pct()
        && response_rate < config.low_response_rate_pct()
    {
        should_reduce_capacity = true;
        reasons.push(format!(
            "low response rate ({:.1}% < {:.1}%)",
            response_rate,
            config.low_response_rate_pct()
        ));
    }

    // Check reject rate
    if let Some(reject_rate) = metrics.reject_rate_pct()
        && reject_rate > config.high_reject_rate_pct()
    {
        should_downgrade_tier = true;
        reasons.push(format!(
            "high reject rate ({:.1}% > {:.1}%)",
            reject_rate,
            config.high_reject_rate_pct()
        ));
    }

    if reasons.is_empty() {
        PenaltyResult::none()
    } else {
        PenaltyResult {
            should_reduce_capacity,
            capacity_reduction_pct: if should_reduce_capacity {
                config.capacity_penalty_pct()
            } else {
                Decimal::ZERO
            },
            should_downgrade_tier,
            reason: Some(reasons.join("; ")),
        }
    }
}

/// Calculates the volume needed to reach the next tier.
///
/// Returns `None` if the MM is already at Platinum tier.
///
/// # Arguments
///
/// * `current_volume` - Current monthly volume in USD
/// * `current_tier` - The MM's current tier
/// * `config` - Incentive program configuration
///
/// # Returns
///
/// The additional volume needed to reach the next tier, or `None` if already Platinum.
#[must_use]
pub fn volume_to_next_tier(
    current_volume: Decimal,
    current_tier: IncentiveTier,
    config: &IncentiveConfig,
) -> Option<Decimal> {
    current_tier.next_tier().map(|next| {
        let next_threshold = next.threshold(config);
        // Use checked_sub per project financial math conventions.
        // Returns ZERO if already at or above threshold, otherwise the difference.
        if current_volume >= next_threshold {
            Decimal::ZERO
        } else {
            next_threshold
                .checked_sub(current_volume)
                .unwrap_or(Decimal::ZERO)
        }
    })
}

// ============================================================================
// MmIncentiveStatus
// ============================================================================

/// Complete incentive status for a market maker.
///
/// Aggregates tier information, volume metrics, rebates, and penalty status
/// into a single snapshot for API responses.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MmIncentiveStatus {
    /// Market maker identifier.
    mm_id: CounterpartyId,
    /// Current incentive tier.
    current_tier: IncentiveTier,
    /// Rebate rate for current tier (basis points).
    rebate_rate_bps: Decimal,
    /// Monthly trading volume in USD.
    monthly_volume_usd: Decimal,
    /// Volume needed to reach next tier (None if Platinum).
    volume_to_next_tier_usd: Option<Decimal>,
    /// Next tier (None if Platinum).
    next_tier: Option<IncentiveTier>,
    /// Rebates earned in current period (USD).
    current_period_rebates_usd: Decimal,
    /// Penalty evaluation result.
    penalty_result: PenaltyResult,
    /// Timestamp when this status was computed.
    computed_at: Timestamp,
}

impl MmIncentiveStatus {
    /// Creates a new incentive status.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        mm_id: CounterpartyId,
        current_tier: IncentiveTier,
        rebate_rate_bps: Decimal,
        monthly_volume_usd: Decimal,
        volume_to_next_tier_usd: Option<Decimal>,
        next_tier: Option<IncentiveTier>,
        current_period_rebates_usd: Decimal,
        penalty_result: PenaltyResult,
        computed_at: Timestamp,
    ) -> Self {
        Self {
            mm_id,
            current_tier,
            rebate_rate_bps,
            monthly_volume_usd,
            volume_to_next_tier_usd,
            next_tier,
            current_period_rebates_usd,
            penalty_result,
            computed_at,
        }
    }

    /// Returns the market maker identifier.
    #[inline]
    #[must_use]
    pub fn mm_id(&self) -> &CounterpartyId {
        &self.mm_id
    }

    /// Returns the current incentive tier.
    #[inline]
    #[must_use]
    pub fn current_tier(&self) -> IncentiveTier {
        self.current_tier
    }

    /// Returns the rebate rate in basis points.
    #[inline]
    #[must_use]
    pub fn rebate_rate_bps(&self) -> Decimal {
        self.rebate_rate_bps
    }

    /// Returns the monthly trading volume in USD.
    #[inline]
    #[must_use]
    pub fn monthly_volume_usd(&self) -> Decimal {
        self.monthly_volume_usd
    }

    /// Returns the volume needed to reach the next tier (None if Platinum).
    #[inline]
    #[must_use]
    pub fn volume_to_next_tier_usd(&self) -> Option<Decimal> {
        self.volume_to_next_tier_usd
    }

    /// Returns the next tier (None if Platinum).
    #[inline]
    #[must_use]
    pub fn next_tier(&self) -> Option<IncentiveTier> {
        self.next_tier
    }

    /// Returns the rebates earned in the current period (USD).
    #[inline]
    #[must_use]
    pub fn current_period_rebates_usd(&self) -> Decimal {
        self.current_period_rebates_usd
    }

    /// Returns the penalty evaluation result.
    #[inline]
    #[must_use]
    pub fn penalty_result(&self) -> &PenaltyResult {
        &self.penalty_result
    }

    /// Returns the timestamp when this status was computed.
    #[inline]
    #[must_use]
    pub fn computed_at(&self) -> Timestamp {
        self.computed_at
    }
}

impl fmt::Display for MmIncentiveStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "IncentiveStatus({} tier={} rebate={:.1}bps vol=${:.0})",
            self.mm_id, self.current_tier, self.rebate_rate_bps, self.monthly_volume_usd
        )
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
        MmPerformanceEvent, MmPerformanceEventKind, MmPerformanceMetrics,
    };
    use crate::domain::value_objects::CounterpartyId;
    use crate::domain::value_objects::timestamp::Timestamp;

    fn default_config() -> IncentiveConfig {
        IncentiveConfig::default()
    }

    fn mm_id() -> CounterpartyId {
        CounterpartyId::new("mm-test")
    }

    fn now() -> Timestamp {
        Timestamp::from_secs(1_700_000_000).unwrap()
    }

    fn window_start() -> Timestamp {
        now().sub_secs(86400 * 7)
    }

    mod tier_tests {
        use super::*;

        #[test]
        fn from_volume_bronze() {
            let config = default_config();
            let volume = Decimal::from(500_000); // Below silver threshold
            assert_eq!(
                IncentiveTier::from_volume(volume, &config),
                IncentiveTier::Bronze
            );
        }

        #[test]
        fn from_volume_silver() {
            let config = default_config();
            let volume = Decimal::from(1_000_000); // At silver threshold
            assert_eq!(
                IncentiveTier::from_volume(volume, &config),
                IncentiveTier::Silver
            );
        }

        #[test]
        fn from_volume_gold() {
            let config = default_config();
            let volume = Decimal::from(10_000_000); // At gold threshold
            assert_eq!(
                IncentiveTier::from_volume(volume, &config),
                IncentiveTier::Gold
            );
        }

        #[test]
        fn from_volume_platinum() {
            let config = default_config();
            let volume = Decimal::from(50_000_000); // At platinum threshold
            assert_eq!(
                IncentiveTier::from_volume(volume, &config),
                IncentiveTier::Platinum
            );
        }

        #[test]
        fn from_volume_above_platinum() {
            let config = default_config();
            let volume = Decimal::from(100_000_000); // Above platinum
            assert_eq!(
                IncentiveTier::from_volume(volume, &config),
                IncentiveTier::Platinum
            );
        }

        #[test]
        fn rebate_bps_returns_correct_values() {
            let config = default_config();
            assert_eq!(
                IncentiveTier::Bronze.rebate_bps(&config),
                config.bronze_rebate_bps()
            );
            assert_eq!(
                IncentiveTier::Silver.rebate_bps(&config),
                config.silver_rebate_bps()
            );
            assert_eq!(
                IncentiveTier::Gold.rebate_bps(&config),
                config.gold_rebate_bps()
            );
            assert_eq!(
                IncentiveTier::Platinum.rebate_bps(&config),
                config.platinum_rebate_bps()
            );
        }

        #[test]
        fn downgrade_returns_lower_tier() {
            assert_eq!(IncentiveTier::Platinum.downgrade(), IncentiveTier::Gold);
            assert_eq!(IncentiveTier::Gold.downgrade(), IncentiveTier::Silver);
            assert_eq!(IncentiveTier::Silver.downgrade(), IncentiveTier::Bronze);
            assert_eq!(IncentiveTier::Bronze.downgrade(), IncentiveTier::Bronze);
        }

        #[test]
        fn tier_ordering() {
            assert!(IncentiveTier::Bronze < IncentiveTier::Silver);
            assert!(IncentiveTier::Silver < IncentiveTier::Gold);
            assert!(IncentiveTier::Gold < IncentiveTier::Platinum);
        }

        #[test]
        fn display_format() {
            assert_eq!(IncentiveTier::Bronze.to_string(), "BRONZE");
            assert_eq!(IncentiveTier::Platinum.to_string(), "PLATINUM");
        }

        #[test]
        fn next_tier_returns_higher_tier() {
            assert_eq!(
                IncentiveTier::Bronze.next_tier(),
                Some(IncentiveTier::Silver)
            );
            assert_eq!(IncentiveTier::Silver.next_tier(), Some(IncentiveTier::Gold));
            assert_eq!(
                IncentiveTier::Gold.next_tier(),
                Some(IncentiveTier::Platinum)
            );
        }

        #[test]
        fn next_tier_returns_none_for_platinum() {
            assert_eq!(IncentiveTier::Platinum.next_tier(), None);
        }

        #[test]
        fn threshold_returns_correct_values() {
            let config = default_config();
            assert_eq!(IncentiveTier::Bronze.threshold(&config), Decimal::ZERO);
            assert_eq!(
                IncentiveTier::Silver.threshold(&config),
                config.silver_threshold()
            );
            assert_eq!(
                IncentiveTier::Gold.threshold(&config),
                config.gold_threshold()
            );
            assert_eq!(
                IncentiveTier::Platinum.threshold(&config),
                config.platinum_threshold()
            );
        }
    }

    mod volume_to_next_tier_tests {
        use super::*;

        #[test]
        fn returns_none_for_platinum() {
            let config = default_config();
            let volume = Decimal::from(100_000_000);
            assert_eq!(
                volume_to_next_tier(volume, IncentiveTier::Platinum, &config),
                None
            );
        }

        #[test]
        fn returns_volume_needed_for_bronze() {
            let config = default_config();
            let volume = Decimal::from(500_000);
            let result = volume_to_next_tier(volume, IncentiveTier::Bronze, &config);
            // Silver threshold is 1M, so need 500K more
            assert_eq!(result, Some(config.silver_threshold() - volume));
        }

        #[test]
        fn returns_zero_when_already_at_threshold() {
            let config = default_config();
            let volume = config.silver_threshold();
            let result = volume_to_next_tier(volume, IncentiveTier::Bronze, &config);
            assert_eq!(result, Some(Decimal::ZERO));
        }

        #[test]
        fn returns_zero_when_above_threshold() {
            let config = default_config();
            let volume = config.silver_threshold() + Decimal::from(1_000_000);
            let result = volume_to_next_tier(volume, IncentiveTier::Bronze, &config);
            assert_eq!(result, Some(Decimal::ZERO));
        }
    }

    mod mm_incentive_status_tests {
        use super::*;

        #[test]
        fn new_creates_status_with_all_fields() {
            let config = default_config();
            let id = mm_id();
            let tier = IncentiveTier::Silver;
            let rebate = tier.rebate_bps(&config);
            let volume = Decimal::from(2_000_000);
            let vol_to_next = Some(Decimal::from(8_000_000));
            let next = Some(IncentiveTier::Gold);
            let rebates = Decimal::from(500);
            let penalty = PenaltyResult::none();
            let ts = now();

            let status = MmIncentiveStatus::new(
                id.clone(),
                tier,
                rebate,
                volume,
                vol_to_next,
                next,
                rebates,
                penalty.clone(),
                ts,
            );

            assert_eq!(status.mm_id(), &id);
            assert_eq!(status.current_tier(), tier);
            assert_eq!(status.rebate_rate_bps(), rebate);
            assert_eq!(status.monthly_volume_usd(), volume);
            assert_eq!(status.volume_to_next_tier_usd(), vol_to_next);
            assert_eq!(status.next_tier(), next);
            assert_eq!(status.current_period_rebates_usd(), rebates);
            assert!(!status.penalty_result().has_penalty());
            assert_eq!(status.computed_at(), ts);
        }

        #[test]
        fn display_format_includes_key_info() {
            let status = MmIncentiveStatus::new(
                mm_id(),
                IncentiveTier::Gold,
                Decimal::from(-3),
                Decimal::from(15_000_000),
                Some(Decimal::from(35_000_000)),
                Some(IncentiveTier::Platinum),
                Decimal::from(1500),
                PenaltyResult::none(),
                now(),
            );

            let display = status.to_string();
            assert!(display.contains("mm-test"));
            assert!(display.contains("GOLD"));
        }
    }

    mod config_tests {
        use super::*;

        #[test]
        fn default_config_has_sensible_values() {
            let config = default_config();
            assert!(config.silver_threshold() > Decimal::ZERO);
            assert!(config.gold_threshold() > config.silver_threshold());
            assert!(config.platinum_threshold() > config.gold_threshold());
            // Rebates should be negative (payment to MM)
            assert!(config.bronze_rebate_bps() < Decimal::ZERO);
        }

        #[test]
        fn builder_creates_custom_config() {
            let config = IncentiveConfig::builder()
                .silver_threshold(Decimal::from(2_000_000))
                .gold_threshold(Decimal::from(20_000_000))
                .build();

            assert_eq!(config.silver_threshold(), Decimal::from(2_000_000));
            assert_eq!(config.gold_threshold(), Decimal::from(20_000_000));
        }
    }

    mod compute_incentive_tests {
        use super::*;

        #[test]
        fn computes_base_rebate_for_bronze() {
            let config = default_config();
            let notional = Decimal::from(1_000_000);
            let result = compute_incentive(IncentiveTier::Bronze, notional, None, &config);

            assert_eq!(result.tier(), IncentiveTier::Bronze);
            assert_eq!(result.base_rebate_bps(), config.bronze_rebate_bps());
            assert_eq!(result.spread_bonus_bps(), Decimal::ZERO);
        }

        #[test]
        fn computes_spread_bonus_when_tight() {
            let config = default_config();
            let notional = Decimal::from(1_000_000);
            let tight_spread = Decimal::from(3); // 3 bps, below threshold of 5
            let result =
                compute_incentive(IncentiveTier::Gold, notional, Some(tight_spread), &config);

            assert_eq!(result.spread_bonus_bps(), config.spread_bonus_bps());
            assert_eq!(
                result.total_rebate_bps(),
                config.gold_rebate_bps() + config.spread_bonus_bps()
            );
        }

        #[test]
        fn no_spread_bonus_when_wide() {
            let config = default_config();
            let notional = Decimal::from(1_000_000);
            let wide_spread = Decimal::from(10); // 10 bps, above threshold of 5
            let result =
                compute_incentive(IncentiveTier::Gold, notional, Some(wide_spread), &config);

            assert_eq!(result.spread_bonus_bps(), Decimal::ZERO);
        }

        #[test]
        fn rebate_amount_calculated_correctly() {
            let config = default_config();
            let notional = Decimal::from(1_000_000);
            let result = compute_incentive(IncentiveTier::Bronze, notional, None, &config);

            // -1.0 bps on 1M = -100 USD
            let expected = notional * config.bronze_rebate_bps() / Decimal::from(10000);
            assert_eq!(result.rebate_amount(), expected);
        }
    }

    mod evaluate_penalties_tests {
        use super::*;

        fn make_event(kind: MmPerformanceEventKind) -> MmPerformanceEvent {
            MmPerformanceEvent::new(mm_id(), kind, now().sub_secs(3600))
        }

        #[test]
        fn no_penalty_when_metrics_good() {
            // 100% response rate, 0% reject rate
            let events = vec![
                make_event(MmPerformanceEventKind::RfqSent),
                make_event(MmPerformanceEventKind::QuoteReceived {
                    response_time_ms: 100,
                    rank: 1,
                }),
            ];
            let metrics = MmPerformanceMetrics::compute(&mm_id(), &events, window_start(), now());
            let config = default_config();

            let result = evaluate_penalties(&metrics, &config);

            assert!(!result.has_penalty());
            assert!(!result.should_reduce_capacity());
            assert!(!result.should_downgrade_tier());
        }

        #[test]
        fn capacity_penalty_for_low_response_rate() {
            // 50% response rate (below 80% threshold)
            let events = vec![
                make_event(MmPerformanceEventKind::RfqSent),
                make_event(MmPerformanceEventKind::RfqSent),
                make_event(MmPerformanceEventKind::QuoteReceived {
                    response_time_ms: 100,
                    rank: 1,
                }),
            ];
            let metrics = MmPerformanceMetrics::compute(&mm_id(), &events, window_start(), now());
            let config = default_config();

            let result = evaluate_penalties(&metrics, &config);

            assert!(result.has_penalty());
            assert!(result.should_reduce_capacity());
            assert_eq!(
                result.capacity_reduction_pct(),
                config.capacity_penalty_pct()
            );
            assert!(!result.should_downgrade_tier());
        }

        #[test]
        fn tier_downgrade_for_high_reject_rate() {
            // 40% reject rate (above 20% threshold)
            let events = vec![
                make_event(MmPerformanceEventKind::AcceptRequested),
                make_event(MmPerformanceEventKind::AcceptRequested),
                make_event(MmPerformanceEventKind::AcceptRequested),
                make_event(MmPerformanceEventKind::AcceptRequested),
                make_event(MmPerformanceEventKind::AcceptRequested),
                make_event(MmPerformanceEventKind::LastLookReject),
                make_event(MmPerformanceEventKind::LastLookReject),
            ];
            let metrics = MmPerformanceMetrics::compute(&mm_id(), &events, window_start(), now());
            let config = default_config();

            let result = evaluate_penalties(&metrics, &config);

            assert!(result.has_penalty());
            assert!(result.should_downgrade_tier());
            assert!(!result.should_reduce_capacity());
        }

        #[test]
        fn both_penalties_when_both_thresholds_exceeded() {
            // Low response + high reject
            let events = vec![
                // Low response: 1/4 = 25%
                make_event(MmPerformanceEventKind::RfqSent),
                make_event(MmPerformanceEventKind::RfqSent),
                make_event(MmPerformanceEventKind::RfqSent),
                make_event(MmPerformanceEventKind::RfqSent),
                make_event(MmPerformanceEventKind::QuoteReceived {
                    response_time_ms: 100,
                    rank: 1,
                }),
                // High reject: 2/3 = 66%
                make_event(MmPerformanceEventKind::AcceptRequested),
                make_event(MmPerformanceEventKind::AcceptRequested),
                make_event(MmPerformanceEventKind::AcceptRequested),
                make_event(MmPerformanceEventKind::LastLookReject),
                make_event(MmPerformanceEventKind::LastLookReject),
            ];
            let metrics = MmPerformanceMetrics::compute(&mm_id(), &events, window_start(), now());
            let config = default_config();

            let result = evaluate_penalties(&metrics, &config);

            assert!(result.has_penalty());
            assert!(result.should_reduce_capacity());
            assert!(result.should_downgrade_tier());
            assert!(result.reason().unwrap().contains("low response rate"));
            assert!(result.reason().unwrap().contains("high reject rate"));
        }

        #[test]
        fn no_penalty_when_no_data() {
            let metrics = MmPerformanceMetrics::compute(&mm_id(), &[], window_start(), now());
            let config = default_config();

            let result = evaluate_penalties(&metrics, &config);

            assert!(!result.has_penalty());
        }
    }
}
