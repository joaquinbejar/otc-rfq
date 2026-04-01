//! # Fee Engine
//!
//! Centralized fee calculation service for RFQ and Block trades.
//!
//! This module provides the [`FeeEngine`] service which computes fees based on:
//! - Trade type (RFQ vs Block)
//! - Default fee schedules with taker/maker rates
//! - Volume-based discounts (30-day counterparty volume)
//! - Per-counterparty rate overrides
//!
//! # Fee Structure
//!
//! - **RFQ trades**: Taker pays 3 bps (0.03%), Maker receives -1 bp rebate (-0.01%)
//! - **Block trades**: Both sides pay 2 bps (0.02%)
//!
//! # Volume Discounts
//!
//! Discounts are applied based on 30-day USD volume:
//! - $1M+: 10% discount
//! - $10M+: 25% discount
//! - $100M+: 50% discount
//!
//! # Examples
//!
//! ```ignore
//! use otc_rfq::domain::services::fee_engine::{FeeEngine, FeeSchedule};
//! use otc_rfq::domain::value_objects::{CounterpartyId, TradeType};
//! use rust_decimal::Decimal;
//!
//! let engine = FeeEngine::new(
//!     FeeSchedule::default(),
//!     Arc::new(NoOpVolumeProvider),
//!     None,
//! );
//!
//! let breakdown = engine.calculate(
//!     TradeType::Rfq,
//!     Decimal::new(1_000_000, 0), // $1M notional
//!     &CounterpartyId::new("client-1"),
//! ).await?;
//! ```

use crate::domain::errors::{DomainError, DomainResult};
use crate::domain::value_objects::{CounterpartyId, TradeType};
use async_trait::async_trait;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::Arc;

/// Default RFQ taker rate in basis points (0.03%).
pub const DEFAULT_RFQ_TAKER_BPS: i32 = 3;

/// Default RFQ maker rate in basis points (-0.01% rebate).
pub const DEFAULT_RFQ_MAKER_BPS: i32 = -1;

/// Default Block trade rate in basis points (0.02% for both sides).
pub const DEFAULT_BLOCK_BPS: i32 = 2;

/// Basis points divisor for converting to decimal (1 bp = 0.0001 or 0.01%).
const BPS_DIVISOR: Decimal = Decimal::from_parts(10000, 0, 0, false, 0);

/// Fee rates for a trade type.
///
/// Rates are expressed in basis points (1 bp = 0.01%).
/// Negative rates indicate rebates.
///
/// # Examples
///
/// ```
/// use otc_rfq::domain::services::fee_engine::FeeRates;
///
/// // RFQ: taker pays, maker receives rebate
/// let rfq_rates = FeeRates::new(3, -1);
/// assert_eq!(rfq_rates.taker_rate_bps(), 3);
/// assert_eq!(rfq_rates.maker_rate_bps(), -1);
///
/// // Block: symmetric fees
/// let block_rates = FeeRates::symmetric(2);
/// assert_eq!(block_rates.taker_rate_bps(), 2);
/// assert_eq!(block_rates.maker_rate_bps(), 2);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeeRates {
    /// Taker fee rate in basis points.
    taker_rate_bps: i32,
    /// Maker fee rate in basis points (negative = rebate).
    maker_rate_bps: i32,
}

impl FeeRates {
    /// Creates new fee rates.
    ///
    /// # Arguments
    ///
    /// * `taker_rate_bps` - Taker fee in basis points
    /// * `maker_rate_bps` - Maker fee in basis points (negative = rebate)
    #[must_use]
    pub const fn new(taker_rate_bps: i32, maker_rate_bps: i32) -> Self {
        Self {
            taker_rate_bps,
            maker_rate_bps,
        }
    }

    /// Creates symmetric fee rates (same for taker and maker).
    #[must_use]
    pub const fn symmetric(rate_bps: i32) -> Self {
        Self {
            taker_rate_bps: rate_bps,
            maker_rate_bps: rate_bps,
        }
    }

    /// Returns the taker rate in basis points.
    #[inline]
    #[must_use]
    pub const fn taker_rate_bps(&self) -> i32 {
        self.taker_rate_bps
    }

    /// Returns the maker rate in basis points.
    #[inline]
    #[must_use]
    pub const fn maker_rate_bps(&self) -> i32 {
        self.maker_rate_bps
    }

    /// Converts taker rate to decimal multiplier.
    #[must_use]
    pub fn taker_rate_decimal(&self) -> Decimal {
        Decimal::from(self.taker_rate_bps) / BPS_DIVISOR
    }

    /// Converts maker rate to decimal multiplier.
    #[must_use]
    pub fn maker_rate_decimal(&self) -> Decimal {
        Decimal::from(self.maker_rate_bps) / BPS_DIVISOR
    }
}

impl Default for FeeRates {
    fn default() -> Self {
        Self::new(DEFAULT_RFQ_TAKER_BPS, DEFAULT_RFQ_MAKER_BPS)
    }
}

/// Volume discount tier.
///
/// Discounts are applied when counterparty 30-day volume exceeds the threshold.
///
/// # Examples
///
/// ```
/// use otc_rfq::domain::services::fee_engine::VolumeDiscount;
///
/// let tier = VolumeDiscount::new(1_000_000, 10);
/// assert_eq!(tier.threshold_usd(), 1_000_000);
/// assert_eq!(tier.discount_pct(), 10);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct VolumeDiscount {
    /// Volume threshold in USD.
    threshold_usd: u64,
    /// Discount percentage (0-100).
    discount_pct: u8,
}

impl VolumeDiscount {
    /// Creates a new volume discount tier.
    ///
    /// # Arguments
    ///
    /// * `threshold_usd` - Minimum 30-day volume in USD
    /// * `discount_pct` - Discount percentage (0-100)
    #[must_use]
    pub const fn new(threshold_usd: u64, discount_pct: u8) -> Self {
        Self {
            threshold_usd,
            discount_pct,
        }
    }

    /// Returns the volume threshold in USD.
    #[inline]
    #[must_use]
    pub const fn threshold_usd(&self) -> u64 {
        self.threshold_usd
    }

    /// Returns the discount percentage.
    #[inline]
    #[must_use]
    pub const fn discount_pct(&self) -> u8 {
        self.discount_pct
    }

    /// Returns the discount as a decimal multiplier (e.g., 10% -> 0.10).
    #[must_use]
    pub fn discount_decimal(&self) -> Decimal {
        Decimal::from(self.discount_pct) / Decimal::ONE_HUNDRED
    }
}

/// Fee schedule configuration.
///
/// Contains fee rates for different trade types and volume discount tiers.
///
/// # Examples
///
/// ```
/// use otc_rfq::domain::services::fee_engine::FeeSchedule;
///
/// let schedule = FeeSchedule::default();
/// assert_eq!(schedule.rfq_rates().taker_rate_bps(), 3);
/// assert_eq!(schedule.block_rates().taker_rate_bps(), 2);
/// assert_eq!(schedule.volume_discounts().len(), 3);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeeSchedule {
    /// Fee rates for RFQ trades.
    rfq_rates: FeeRates,
    /// Fee rates for Block trades.
    block_rates: FeeRates,
    /// Volume discount tiers (sorted by threshold ascending).
    volume_discounts: Vec<VolumeDiscount>,
}

impl FeeSchedule {
    /// Creates a new fee schedule.
    ///
    /// # Arguments
    ///
    /// * `rfq_rates` - Fee rates for RFQ trades
    /// * `block_rates` - Fee rates for Block trades
    /// * `volume_discounts` - Volume discount tiers
    #[must_use]
    pub fn new(
        rfq_rates: FeeRates,
        block_rates: FeeRates,
        volume_discounts: Vec<VolumeDiscount>,
    ) -> Self {
        let mut discounts = volume_discounts;
        discounts.sort_by_key(|d| d.threshold_usd);
        Self {
            rfq_rates,
            block_rates,
            volume_discounts: discounts,
        }
    }

    /// Returns the RFQ fee rates.
    #[inline]
    #[must_use]
    pub const fn rfq_rates(&self) -> &FeeRates {
        &self.rfq_rates
    }

    /// Returns the Block trade fee rates.
    #[inline]
    #[must_use]
    pub const fn block_rates(&self) -> &FeeRates {
        &self.block_rates
    }

    /// Returns the volume discount tiers.
    #[inline]
    #[must_use]
    pub fn volume_discounts(&self) -> &[VolumeDiscount] {
        &self.volume_discounts
    }

    /// Gets fee rates for a trade type.
    #[must_use]
    pub const fn rates_for(&self, trade_type: TradeType) -> &FeeRates {
        match trade_type {
            TradeType::Rfq => &self.rfq_rates,
            TradeType::Block => &self.block_rates,
        }
    }

    /// Finds the applicable discount for a given volume.
    ///
    /// Returns the highest discount tier where volume >= threshold.
    #[must_use]
    pub fn find_discount(&self, volume_usd: Decimal) -> Option<&VolumeDiscount> {
        self.volume_discounts
            .iter()
            .rev()
            .find(|d| volume_usd >= Decimal::from(d.threshold_usd))
    }
}

impl Default for FeeSchedule {
    fn default() -> Self {
        Self::new(
            FeeRates::new(DEFAULT_RFQ_TAKER_BPS, DEFAULT_RFQ_MAKER_BPS),
            FeeRates::symmetric(DEFAULT_BLOCK_BPS),
            vec![
                VolumeDiscount::new(1_000_000, 10),
                VolumeDiscount::new(10_000_000, 25),
                VolumeDiscount::new(100_000_000, 50),
            ],
        )
    }
}

/// Fee calculation breakdown.
///
/// Contains the computed fees for a trade, including any volume discounts applied.
///
/// # Examples
///
/// ```
/// use otc_rfq::domain::services::fee_engine::FeeBreakdown;
/// use rust_decimal::Decimal;
///
/// let breakdown = FeeBreakdown::new(
///     Decimal::new(300, 2),  // $3.00 taker fee
///     Decimal::new(-100, 2), // -$1.00 maker rebate
///     Some(10),              // 10% discount applied
/// ).unwrap();
///
/// assert_eq!(breakdown.net_fee(), Decimal::new(200, 2)); // $2.00 net
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeeBreakdown {
    /// Taker fee (positive = fee, negative = rebate).
    taker_fee: Decimal,
    /// Maker fee (positive = fee, negative = rebate).
    maker_fee: Decimal,
    /// Net fee (taker_fee + maker_fee).
    net_fee: Decimal,
    /// Volume discount percentage applied, if any.
    volume_discount_applied: Option<u8>,
}

impl FeeBreakdown {
    /// Creates a new fee breakdown.
    ///
    /// # Arguments
    ///
    /// * `taker_fee` - Taker fee amount
    /// * `maker_fee` - Maker fee amount
    /// * `volume_discount_applied` - Discount percentage applied, if any
    ///
    /// # Errors
    ///
    /// Returns `DomainError::FeeCalculationFailed` if net fee calculation overflows.
    pub fn new(
        taker_fee: Decimal,
        maker_fee: Decimal,
        volume_discount_applied: Option<u8>,
    ) -> DomainResult<Self> {
        let net_fee =
            taker_fee
                .checked_add(maker_fee)
                .ok_or_else(|| DomainError::FeeCalculationFailed {
                    reason: "net fee calculation overflow".to_string(),
                })?;

        Ok(Self {
            taker_fee,
            maker_fee,
            net_fee,
            volume_discount_applied,
        })
    }

    /// Creates a zero fee breakdown.
    #[must_use]
    pub fn zero() -> Self {
        Self {
            taker_fee: Decimal::ZERO,
            maker_fee: Decimal::ZERO,
            net_fee: Decimal::ZERO,
            volume_discount_applied: None,
        }
    }

    /// Returns the taker fee.
    #[inline]
    #[must_use]
    pub const fn taker_fee(&self) -> Decimal {
        self.taker_fee
    }

    /// Returns the maker fee.
    #[inline]
    #[must_use]
    pub const fn maker_fee(&self) -> Decimal {
        self.maker_fee
    }

    /// Returns the net fee (taker + maker).
    #[inline]
    #[must_use]
    pub const fn net_fee(&self) -> Decimal {
        self.net_fee
    }

    /// Returns the volume discount applied, if any.
    #[inline]
    #[must_use]
    pub const fn volume_discount_applied(&self) -> Option<u8> {
        self.volume_discount_applied
    }
}

/// Provider for counterparty 30-day trading volume.
///
/// Implementations should return the total USD volume traded by a counterparty
/// over the last 30 days for volume discount calculation.
#[async_trait]
pub trait VolumeProvider: Send + Sync + fmt::Debug {
    /// Gets the 30-day trading volume in USD for a counterparty.
    ///
    /// # Arguments
    ///
    /// * `counterparty_id` - The counterparty to look up
    ///
    /// # Errors
    ///
    /// Returns an error if the volume cannot be retrieved.
    async fn get_30d_volume_usd(&self, counterparty_id: &CounterpartyId) -> DomainResult<Decimal>;
}

/// Provider for per-counterparty fee rate overrides.
///
/// Implementations should return custom fee rates for counterparties
/// that have negotiated special pricing.
#[async_trait]
pub trait FeeOverrideProvider: Send + Sync + fmt::Debug {
    /// Gets the fee rate override for a counterparty, if any.
    ///
    /// # Arguments
    ///
    /// * `counterparty_id` - The counterparty to look up
    ///
    /// # Errors
    ///
    /// Returns an error if the lookup fails.
    async fn get_override(
        &self,
        counterparty_id: &CounterpartyId,
    ) -> DomainResult<Option<FeeRates>>;
}

/// No-op volume provider that always returns zero volume.
///
/// Useful for testing or when volume tracking is not yet implemented.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoOpVolumeProvider;

#[async_trait]
impl VolumeProvider for NoOpVolumeProvider {
    async fn get_30d_volume_usd(&self, _counterparty_id: &CounterpartyId) -> DomainResult<Decimal> {
        Ok(Decimal::ZERO)
    }
}

/// No-op override provider that never returns overrides.
///
/// Useful for testing or when overrides are not configured.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoOpOverrideProvider;

#[async_trait]
impl FeeOverrideProvider for NoOpOverrideProvider {
    async fn get_override(
        &self,
        _counterparty_id: &CounterpartyId,
    ) -> DomainResult<Option<FeeRates>> {
        Ok(None)
    }
}

/// Centralized fee calculation engine.
///
/// Computes fees for RFQ and Block trades based on:
/// - Trade type (RFQ vs Block)
/// - Default fee schedule
/// - Counterparty 30-day volume (for discounts)
/// - Per-counterparty rate overrides
///
/// # Examples
///
/// ```ignore
/// use otc_rfq::domain::services::fee_engine::{FeeEngine, FeeSchedule, NoOpVolumeProvider};
/// use otc_rfq::domain::value_objects::{CounterpartyId, TradeType};
/// use rust_decimal::Decimal;
/// use std::sync::Arc;
///
/// let engine = FeeEngine::new(
///     FeeSchedule::default(),
///     Arc::new(NoOpVolumeProvider),
///     None,
/// );
///
/// let breakdown = engine.calculate(
///     TradeType::Rfq,
///     Decimal::new(1_000_000, 0),
///     &CounterpartyId::new("client-1"),
/// ).await?;
///
/// // RFQ: 3 bps taker, -1 bp maker
/// // $1M * 0.0003 = $300 taker fee
/// // $1M * -0.0001 = -$100 maker rebate
/// assert_eq!(breakdown.taker_fee(), Decimal::new(300, 0));
/// assert_eq!(breakdown.maker_fee(), Decimal::new(-100, 0));
/// ```
#[derive(Debug)]
pub struct FeeEngine {
    /// Fee schedule configuration.
    schedule: FeeSchedule,
    /// Provider for 30-day volume lookups.
    volume_provider: Arc<dyn VolumeProvider>,
    /// Optional provider for per-counterparty overrides.
    override_provider: Option<Arc<dyn FeeOverrideProvider>>,
}

impl FeeEngine {
    /// Creates a new fee engine.
    ///
    /// # Arguments
    ///
    /// * `schedule` - Fee schedule configuration
    /// * `volume_provider` - Provider for 30-day volume lookups
    /// * `override_provider` - Optional provider for per-counterparty overrides
    #[must_use]
    pub fn new(
        schedule: FeeSchedule,
        volume_provider: Arc<dyn VolumeProvider>,
        override_provider: Option<Arc<dyn FeeOverrideProvider>>,
    ) -> Self {
        Self {
            schedule,
            volume_provider,
            override_provider,
        }
    }

    /// Creates a fee engine with default schedule and no-op providers.
    ///
    /// Useful for testing.
    #[must_use]
    pub fn default_with_noop() -> Self {
        Self::new(FeeSchedule::default(), Arc::new(NoOpVolumeProvider), None)
    }

    /// Returns the fee schedule.
    #[inline]
    #[must_use]
    pub const fn schedule(&self) -> &FeeSchedule {
        &self.schedule
    }

    /// Calculates fees for a trade.
    ///
    /// # Arguments
    ///
    /// * `trade_type` - Type of trade (RFQ or Block)
    /// * `notional_usd` - Trade notional value in USD
    /// * `counterparty_id` - The counterparty for volume/override lookups
    ///
    /// # Returns
    ///
    /// Fee breakdown with taker fee, maker fee, net fee, and discount applied.
    ///
    /// # Errors
    ///
    /// Returns `DomainError::FeeCalculationFailed` if:
    /// - Volume lookup fails
    /// - Override lookup fails
    /// - Arithmetic overflow occurs
    pub async fn calculate(
        &self,
        trade_type: TradeType,
        notional_usd: Decimal,
        counterparty_id: &CounterpartyId,
    ) -> DomainResult<FeeBreakdown> {
        // Handle zero notional
        if notional_usd.is_zero() {
            return Ok(FeeBreakdown::zero());
        }

        // Get base rates (check override first)
        let base_rates = if let Some(ref override_provider) = self.override_provider {
            match override_provider.get_override(counterparty_id).await? {
                Some(override_rates) => override_rates,
                None => *self.schedule.rates_for(trade_type),
            }
        } else {
            *self.schedule.rates_for(trade_type)
        };

        // Get 30-day volume for discount calculation
        let volume = self
            .volume_provider
            .get_30d_volume_usd(counterparty_id)
            .await?;

        // Find applicable discount
        let discount = self.schedule.find_discount(volume);
        let discount_pct = discount.map(|d| d.discount_pct());

        // Calculate fees
        let (taker_fee, maker_fee) = self.compute_fees(notional_usd, &base_rates, discount)?;

        FeeBreakdown::new(taker_fee, maker_fee, discount_pct)
    }

    /// Computes taker and maker fees with optional discount.
    fn compute_fees(
        &self,
        notional_usd: Decimal,
        rates: &FeeRates,
        discount: Option<&VolumeDiscount>,
    ) -> DomainResult<(Decimal, Decimal)> {
        // Calculate base fees
        let base_taker = notional_usd
            .checked_mul(rates.taker_rate_decimal())
            .ok_or_else(|| DomainError::FeeCalculationFailed {
                reason: "taker fee overflow".to_string(),
            })?;

        let base_maker = notional_usd
            .checked_mul(rates.maker_rate_decimal())
            .ok_or_else(|| DomainError::FeeCalculationFailed {
                reason: "maker fee overflow".to_string(),
            })?;

        // Apply discount if applicable
        let (taker_fee, maker_fee) = if let Some(d) = discount {
            let discount_multiplier = Decimal::ONE - d.discount_decimal();

            // For positive fees, apply discount (reduce fee)
            // For negative fees (rebates), apply discount (reduce rebate magnitude)
            let discounted_taker =
                base_taker.checked_mul(discount_multiplier).ok_or_else(|| {
                    DomainError::FeeCalculationFailed {
                        reason: "discounted taker fee overflow".to_string(),
                    }
                })?;

            let discounted_maker =
                base_maker.checked_mul(discount_multiplier).ok_or_else(|| {
                    DomainError::FeeCalculationFailed {
                        reason: "discounted maker fee overflow".to_string(),
                    }
                })?;

            (discounted_taker, discounted_maker)
        } else {
            (base_taker, base_maker)
        };

        Ok((taker_fee, maker_fee))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    // Helper to create test counterparty
    fn test_counterparty() -> CounterpartyId {
        CounterpartyId::new("test-client")
    }

    // Mock volume provider that returns configurable volume
    #[derive(Debug)]
    struct MockVolumeProvider {
        volume: Decimal,
    }

    impl MockVolumeProvider {
        fn new(volume: u64) -> Self {
            Self {
                volume: Decimal::from(volume),
            }
        }
    }

    #[async_trait]
    impl VolumeProvider for MockVolumeProvider {
        async fn get_30d_volume_usd(
            &self,
            _counterparty_id: &CounterpartyId,
        ) -> DomainResult<Decimal> {
            Ok(self.volume)
        }
    }

    // Mock override provider
    #[derive(Debug)]
    struct MockOverrideProvider {
        override_rates: Option<FeeRates>,
    }

    impl MockOverrideProvider {
        fn with_override(rates: FeeRates) -> Self {
            Self {
                override_rates: Some(rates),
            }
        }

        fn no_override() -> Self {
            Self {
                override_rates: None,
            }
        }
    }

    #[async_trait]
    impl FeeOverrideProvider for MockOverrideProvider {
        async fn get_override(
            &self,
            _counterparty_id: &CounterpartyId,
        ) -> DomainResult<Option<FeeRates>> {
            Ok(self.override_rates)
        }
    }

    mod fee_rates_tests {
        use super::*;

        #[test]
        fn new_creates_rates() {
            let rates = FeeRates::new(3, -1);
            assert_eq!(rates.taker_rate_bps(), 3);
            assert_eq!(rates.maker_rate_bps(), -1);
        }

        #[test]
        fn symmetric_creates_equal_rates() {
            let rates = FeeRates::symmetric(2);
            assert_eq!(rates.taker_rate_bps(), 2);
            assert_eq!(rates.maker_rate_bps(), 2);
        }

        #[test]
        fn taker_rate_decimal_conversion() {
            let rates = FeeRates::new(3, 0);
            // 3 bps = 0.0003
            assert_eq!(rates.taker_rate_decimal(), Decimal::new(3, 4));
        }

        #[test]
        fn maker_rate_decimal_negative() {
            let rates = FeeRates::new(0, -1);
            // -1 bp = -0.0001
            assert_eq!(rates.maker_rate_decimal(), Decimal::new(-1, 4));
        }

        #[test]
        fn default_is_rfq_rates() {
            let rates = FeeRates::default();
            assert_eq!(rates.taker_rate_bps(), DEFAULT_RFQ_TAKER_BPS);
            assert_eq!(rates.maker_rate_bps(), DEFAULT_RFQ_MAKER_BPS);
        }
    }

    mod volume_discount_tests {
        use super::*;

        #[test]
        fn new_creates_discount() {
            let discount = VolumeDiscount::new(1_000_000, 10);
            assert_eq!(discount.threshold_usd(), 1_000_000);
            assert_eq!(discount.discount_pct(), 10);
        }

        #[test]
        fn discount_decimal_conversion() {
            let discount = VolumeDiscount::new(0, 25);
            // 25% = 0.25
            assert_eq!(discount.discount_decimal(), Decimal::new(25, 2));
        }
    }

    mod fee_schedule_tests {
        use super::*;

        #[test]
        fn default_schedule() {
            let schedule = FeeSchedule::default();

            assert_eq!(schedule.rfq_rates().taker_rate_bps(), 3);
            assert_eq!(schedule.rfq_rates().maker_rate_bps(), -1);
            assert_eq!(schedule.block_rates().taker_rate_bps(), 2);
            assert_eq!(schedule.block_rates().maker_rate_bps(), 2);
            assert_eq!(schedule.volume_discounts().len(), 3);
        }

        #[test]
        fn rates_for_rfq() {
            let schedule = FeeSchedule::default();
            let rates = schedule.rates_for(TradeType::Rfq);
            assert_eq!(rates.taker_rate_bps(), 3);
        }

        #[test]
        fn rates_for_block() {
            let schedule = FeeSchedule::default();
            let rates = schedule.rates_for(TradeType::Block);
            assert_eq!(rates.taker_rate_bps(), 2);
        }

        #[test]
        fn find_discount_no_volume() {
            let schedule = FeeSchedule::default();
            let discount = schedule.find_discount(Decimal::ZERO);
            assert!(discount.is_none());
        }

        #[test]
        fn find_discount_tier_1() {
            let schedule = FeeSchedule::default();
            let discount = schedule.find_discount(Decimal::from(1_500_000));
            assert!(discount.is_some());
            assert_eq!(discount.unwrap().discount_pct(), 10);
        }

        #[test]
        fn find_discount_tier_2() {
            let schedule = FeeSchedule::default();
            let discount = schedule.find_discount(Decimal::from(15_000_000));
            assert!(discount.is_some());
            assert_eq!(discount.unwrap().discount_pct(), 25);
        }

        #[test]
        fn find_discount_tier_3() {
            let schedule = FeeSchedule::default();
            let discount = schedule.find_discount(Decimal::from(150_000_000));
            assert!(discount.is_some());
            assert_eq!(discount.unwrap().discount_pct(), 50);
        }

        #[test]
        fn find_discount_exact_threshold() {
            let schedule = FeeSchedule::default();
            let discount = schedule.find_discount(Decimal::from(10_000_000));
            assert!(discount.is_some());
            assert_eq!(discount.unwrap().discount_pct(), 25);
        }

        #[test]
        fn volume_discounts_sorted() {
            let schedule = FeeSchedule::new(
                FeeRates::default(),
                FeeRates::symmetric(2),
                vec![
                    VolumeDiscount::new(100_000_000, 50),
                    VolumeDiscount::new(1_000_000, 10),
                    VolumeDiscount::new(10_000_000, 25),
                ],
            );

            let discounts = schedule.volume_discounts();
            assert_eq!(discounts[0].threshold_usd(), 1_000_000);
            assert_eq!(discounts[1].threshold_usd(), 10_000_000);
            assert_eq!(discounts[2].threshold_usd(), 100_000_000);
        }
    }

    mod fee_breakdown_tests {
        use super::*;

        #[test]
        fn new_calculates_net_fee() {
            let breakdown =
                FeeBreakdown::new(Decimal::new(300, 0), Decimal::new(-100, 0), None).unwrap();
            assert_eq!(breakdown.net_fee(), Decimal::new(200, 0));
        }

        #[test]
        fn zero_breakdown() {
            let breakdown = FeeBreakdown::zero();
            assert_eq!(breakdown.taker_fee(), Decimal::ZERO);
            assert_eq!(breakdown.maker_fee(), Decimal::ZERO);
            assert_eq!(breakdown.net_fee(), Decimal::ZERO);
            assert!(breakdown.volume_discount_applied().is_none());
        }

        #[test]
        fn with_discount() {
            let breakdown =
                FeeBreakdown::new(Decimal::new(270, 0), Decimal::new(-90, 0), Some(10)).unwrap();
            assert_eq!(breakdown.volume_discount_applied(), Some(10));
        }
    }

    mod fee_engine_tests {
        use super::*;

        #[tokio::test]
        async fn calculate_rfq_no_discount() {
            let engine = FeeEngine::new(FeeSchedule::default(), Arc::new(NoOpVolumeProvider), None);

            let breakdown = engine
                .calculate(
                    TradeType::Rfq,
                    Decimal::from(1_000_000),
                    &test_counterparty(),
                )
                .await
                .unwrap();

            // 3 bps taker = $1M * 0.0003 = $300
            assert_eq!(breakdown.taker_fee(), Decimal::new(300, 0));
            // -1 bp maker = $1M * -0.0001 = -$100
            assert_eq!(breakdown.maker_fee(), Decimal::new(-100, 0));
            // Net = $200
            assert_eq!(breakdown.net_fee(), Decimal::new(200, 0));
            assert!(breakdown.volume_discount_applied().is_none());
        }

        #[tokio::test]
        async fn calculate_block_no_discount() {
            let engine = FeeEngine::new(FeeSchedule::default(), Arc::new(NoOpVolumeProvider), None);

            let breakdown = engine
                .calculate(
                    TradeType::Block,
                    Decimal::from(1_000_000),
                    &test_counterparty(),
                )
                .await
                .unwrap();

            // 2 bps both = $1M * 0.0002 = $200 each
            assert_eq!(breakdown.taker_fee(), Decimal::new(200, 0));
            assert_eq!(breakdown.maker_fee(), Decimal::new(200, 0));
            assert_eq!(breakdown.net_fee(), Decimal::new(400, 0));
        }

        #[tokio::test]
        async fn calculate_with_volume_discount() {
            let engine = FeeEngine::new(
                FeeSchedule::default(),
                Arc::new(MockVolumeProvider::new(15_000_000)), // 25% discount tier
                None,
            );

            let breakdown = engine
                .calculate(
                    TradeType::Rfq,
                    Decimal::from(1_000_000),
                    &test_counterparty(),
                )
                .await
                .unwrap();

            // Base: $300 taker, -$100 maker
            // 25% discount: $300 * 0.75 = $225, -$100 * 0.75 = -$75
            assert_eq!(breakdown.taker_fee(), Decimal::new(225, 0));
            assert_eq!(breakdown.maker_fee(), Decimal::new(-75, 0));
            assert_eq!(breakdown.volume_discount_applied(), Some(25));
        }

        #[tokio::test]
        async fn calculate_with_override() {
            let custom_rates = FeeRates::new(5, 0); // 5 bps taker, 0 maker
            let engine = FeeEngine::new(
                FeeSchedule::default(),
                Arc::new(NoOpVolumeProvider),
                Some(Arc::new(MockOverrideProvider::with_override(custom_rates))),
            );

            let breakdown = engine
                .calculate(
                    TradeType::Rfq,
                    Decimal::from(1_000_000),
                    &test_counterparty(),
                )
                .await
                .unwrap();

            // Override: 5 bps taker = $500, 0 maker
            assert_eq!(breakdown.taker_fee(), Decimal::new(500, 0));
            assert_eq!(breakdown.maker_fee(), Decimal::ZERO);
        }

        #[tokio::test]
        async fn calculate_zero_notional() {
            let engine = FeeEngine::default_with_noop();

            let breakdown = engine
                .calculate(TradeType::Rfq, Decimal::ZERO, &test_counterparty())
                .await
                .unwrap();

            assert_eq!(breakdown, FeeBreakdown::zero());
        }

        #[tokio::test]
        async fn calculate_no_override_falls_back() {
            let engine = FeeEngine::new(
                FeeSchedule::default(),
                Arc::new(NoOpVolumeProvider),
                Some(Arc::new(MockOverrideProvider::no_override())),
            );

            let breakdown = engine
                .calculate(
                    TradeType::Rfq,
                    Decimal::from(1_000_000),
                    &test_counterparty(),
                )
                .await
                .unwrap();

            // Should use default RFQ rates
            assert_eq!(breakdown.taker_fee(), Decimal::new(300, 0));
            assert_eq!(breakdown.maker_fee(), Decimal::new(-100, 0));
        }

        #[tokio::test]
        async fn calculate_50_percent_discount() {
            let engine = FeeEngine::new(
                FeeSchedule::default(),
                Arc::new(MockVolumeProvider::new(200_000_000)), // 50% discount tier
                None,
            );

            let breakdown = engine
                .calculate(
                    TradeType::Block,
                    Decimal::from(1_000_000),
                    &test_counterparty(),
                )
                .await
                .unwrap();

            // Base: $200 each
            // 50% discount: $200 * 0.5 = $100 each
            assert_eq!(breakdown.taker_fee(), Decimal::new(100, 0));
            assert_eq!(breakdown.maker_fee(), Decimal::new(100, 0));
            assert_eq!(breakdown.volume_discount_applied(), Some(50));
        }
    }
}
