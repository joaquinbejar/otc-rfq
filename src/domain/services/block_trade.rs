//! # Block Trade Configuration
//!
//! Configurable size thresholds and reporting tiers for block trades.
//!
//! Block trades are large pre-arranged trades that receive special treatment:
//! - Off-book execution
//! - Delayed reporting based on size
//! - Reduced market impact
//!
//! ## Architecture
//!
//! - BTC block trades: ≥ 25 BTC
//! - ETH block trades: ≥ 250 ETH
//! - Reporting tiers based on size multiples (5x = Large, 10x = VeryLarge)

use std::collections::HashMap;

use rust_decimal::Decimal;
use serde::{Deserialize, Deserializer, Serialize, de};

use crate::domain::value_objects::{Instrument, Quantity};

/// Configuration for block trade size thresholds and reporting tiers.
///
/// Determines when a trade qualifies as a block trade and what reporting
/// tier applies based on trade size.
///
/// # Examples
///
/// ```
/// use otc_rfq::domain::services::BlockTradeConfig;
/// use otc_rfq::domain::value_objects::{Instrument, Quantity, Symbol, AssetClass};
///
/// let config = BlockTradeConfig::default();
/// let symbol = Symbol::new("BTC/USD").unwrap();
/// let instrument = Instrument::builder(symbol, AssetClass::CryptoSpot).build();
/// let quantity = Quantity::new(30.0).unwrap();
///
/// assert!(config.qualifies(&instrument, quantity));
/// ```
#[derive(Debug, Clone, Serialize)]
pub struct BlockTradeConfig {
    /// Per-instrument minimum sizes for block trade qualification.
    ///
    /// Maps underlying symbol (e.g., "BTC", "ETH") to minimum quantity.
    pub thresholds: HashMap<String, Quantity>,

    /// Default threshold for instruments without specific configuration.
    ///
    /// If `None`, instruments without specific thresholds will not qualify
    /// as block trades regardless of size.
    pub default_threshold: Option<Quantity>,

    /// Multipliers for determining reporting tiers based on threshold multiples.
    pub tier_multipliers: TierMultipliers,
}

/// Multipliers for determining reporting tier boundaries.
///
/// Tiers are determined by comparing trade size to the base threshold:
/// - Standard: ≥ 1x and < `large` multiplier
/// - Large: ≥ `large` and < `very_large` multiplier
/// - VeryLarge: ≥ `very_large` multiplier
///
/// # Invariants
///
/// - Both multipliers must be finite (not NaN or ±infinity)
/// - Both multipliers must be positive
/// - `large` must be ≥ 1.0
/// - `very_large` must be > `large`
#[derive(Debug, Clone, Copy, Serialize)]
pub struct TierMultipliers {
    /// Multiplier for Large tier threshold (default: 5.0).
    ///
    /// A trade is considered Large if its size is at least this multiple
    /// of the base threshold.
    large: f64,

    /// Multiplier for VeryLarge tier threshold (default: 10.0).
    ///
    /// A trade is considered VeryLarge if its size is at least this multiple
    /// of the base threshold.
    very_large: f64,
}

/// Error type for invalid tier multipliers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TierMultiplierError {
    /// Multiplier is not a finite number (NaN or infinity).
    NotFinite,
    /// Multiplier is not positive.
    NotPositive,
    /// Large multiplier is less than 1.0.
    LargeTooSmall,
    /// VeryLarge multiplier is not greater than Large.
    InvalidOrdering,
}

/// Reporting tier for block trades based on size.
///
/// Larger trades receive longer reporting delays to minimize market impact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[must_use]
pub enum ReportingTier {
    /// Standard block trade with 15 minute reporting delay.
    Standard,

    /// Large block trade with 60 minute reporting delay.
    Large,

    /// Very large block trade with end-of-day reporting delay.
    VeryLarge,
}

impl BlockTradeConfig {
    /// Creates a new block trade configuration with default thresholds.
    ///
    /// Default configuration:
    /// - BTC: 25.0
    /// - ETH: 250.0
    /// - Large tier: 5x threshold
    /// - VeryLarge tier: 10x threshold
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Checks if a trade qualifies as a block trade.
    ///
    /// A trade qualifies if its quantity meets or exceeds the configured
    /// threshold for the instrument's underlying asset.
    ///
    /// # Arguments
    ///
    /// * `instrument` - The trading instrument
    /// * `quantity` - The trade quantity
    ///
    /// # Returns
    ///
    /// `true` if the trade qualifies as a block trade, `false` otherwise.
    ///
    /// # Examples
    ///
    /// ```
    /// use otc_rfq::domain::services::BlockTradeConfig;
    /// use otc_rfq::domain::value_objects::{Instrument, Quantity, Symbol, AssetClass};
    ///
    /// let config = BlockTradeConfig::default();
    /// let symbol = Symbol::new("BTC/USD").unwrap();
    /// let instrument = Instrument::builder(symbol, AssetClass::CryptoSpot).build();
    ///
    /// assert!(config.qualifies(&instrument, Quantity::new(25.0).unwrap()));
    /// assert!(!config.qualifies(&instrument, Quantity::new(24.9).unwrap()));
    /// ```
    #[must_use]
    pub fn qualifies(&self, instrument: &Instrument, quantity: Quantity) -> bool {
        if let Some(threshold) = self.get_threshold(instrument) {
            quantity >= threshold
        } else {
            false
        }
    }

    /// Gets the block trade threshold for a specific instrument.
    ///
    /// Returns the configured threshold for the instrument's underlying asset,
    /// or the default threshold if no specific configuration exists.
    ///
    /// # Arguments
    ///
    /// * `instrument` - The trading instrument
    ///
    /// # Returns
    ///
    /// The threshold quantity, or `None` if no threshold is configured.
    #[must_use]
    pub fn get_threshold(&self, instrument: &Instrument) -> Option<Quantity> {
        let underlying = instrument.base_asset();
        self.thresholds
            .get(underlying)
            .copied()
            .or(self.default_threshold)
    }

    /// Determines the reporting tier for a block trade.
    ///
    /// The tier is based on how many times larger the trade is compared to
    /// the base threshold:
    /// - Standard: ≥ 1x and < 5x threshold
    /// - Large: ≥ 5x and < 10x threshold
    /// - VeryLarge: ≥ 10x threshold
    ///
    /// # Arguments
    ///
    /// * `instrument` - The trading instrument
    /// * `quantity` - The trade quantity
    ///
    /// # Returns
    ///
    /// The reporting tier, or `None` if the trade does not qualify as a block trade.
    ///
    /// # Examples
    ///
    /// ```
    /// use otc_rfq::domain::services::{BlockTradeConfig, ReportingTier};
    /// use otc_rfq::domain::value_objects::{Instrument, Quantity, Symbol, AssetClass};
    ///
    /// let config = BlockTradeConfig::default();
    /// let symbol = Symbol::new("BTC/USD").unwrap();
    /// let instrument = Instrument::builder(symbol, AssetClass::CryptoSpot).build();
    ///
    /// assert_eq!(
    ///     config.determine_tier(&instrument, Quantity::new(25.0).unwrap()),
    ///     Some(ReportingTier::Standard)
    /// );
    /// assert_eq!(
    ///     config.determine_tier(&instrument, Quantity::new(125.0).unwrap()),
    ///     Some(ReportingTier::Large)
    /// );
    /// assert_eq!(
    ///     config.determine_tier(&instrument, Quantity::new(250.0).unwrap()),
    ///     Some(ReportingTier::VeryLarge)
    /// );
    /// ```
    #[must_use]
    pub fn determine_tier(
        &self,
        instrument: &Instrument,
        quantity: Quantity,
    ) -> Option<ReportingTier> {
        let threshold = self.get_threshold(instrument)?;

        // Check qualification using already-fetched threshold
        if quantity < threshold {
            return None;
        }

        // Calculate ratio in Decimal to avoid floating-point precision issues
        let large_threshold = threshold
            .get()
            .checked_mul(Decimal::try_from(self.tier_multipliers.large()).ok()?)?;
        let very_large_threshold = threshold
            .get()
            .checked_mul(Decimal::try_from(self.tier_multipliers.very_large()).ok()?)?;

        if quantity.get() >= very_large_threshold {
            Some(ReportingTier::VeryLarge)
        } else if quantity.get() >= large_threshold {
            Some(ReportingTier::Large)
        } else {
            Some(ReportingTier::Standard)
        }
    }
}

impl Default for BlockTradeConfig {
    fn default() -> Self {
        let mut thresholds = HashMap::new();

        // BTC threshold: 25.0
        // These conversions should always succeed for valid positive decimals.
        // If they fail, we log the error and skip that threshold.
        match Quantity::from_decimal(Decimal::new(25, 0)) {
            Ok(btc_threshold) => {
                thresholds.insert("BTC".to_string(), btc_threshold);
            }
            Err(e) => {
                // This should never happen with valid positive decimals
                tracing::error!(
                    "Failed to create BTC threshold in BlockTradeConfig::default: {}",
                    e
                );
            }
        }

        // ETH threshold: 250.0
        match Quantity::from_decimal(Decimal::new(250, 0)) {
            Ok(eth_threshold) => {
                thresholds.insert("ETH".to_string(), eth_threshold);
            }
            Err(e) => {
                // This should never happen with valid positive decimals
                tracing::error!(
                    "Failed to create ETH threshold in BlockTradeConfig::default: {}",
                    e
                );
            }
        }

        Self {
            thresholds,
            default_threshold: None,
            tier_multipliers: TierMultipliers::default(),
        }
    }
}

impl TierMultipliers {
    /// Creates new tier multipliers with validation.
    ///
    /// # Arguments
    ///
    /// * `large` - Multiplier for Large tier (must be ≥ 1.0 and finite)
    /// * `very_large` - Multiplier for VeryLarge tier (must be > `large` and finite)
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Either multiplier is not finite (NaN or ±infinity)
    /// - Either multiplier is not positive
    /// - `large` is less than 1.0
    /// - `very_large` is not greater than `large`
    pub fn new(large: f64, very_large: f64) -> Result<Self, TierMultiplierError> {
        if !large.is_finite() || !very_large.is_finite() {
            return Err(TierMultiplierError::NotFinite);
        }
        if large <= 0.0 || very_large <= 0.0 {
            return Err(TierMultiplierError::NotPositive);
        }
        if large < 1.0 {
            return Err(TierMultiplierError::LargeTooSmall);
        }
        if very_large <= large {
            return Err(TierMultiplierError::InvalidOrdering);
        }
        Ok(Self { large, very_large })
    }

    /// Returns the Large tier multiplier.
    #[inline]
    #[must_use]
    pub const fn large(&self) -> f64 {
        self.large
    }

    /// Returns the VeryLarge tier multiplier.
    #[inline]
    #[must_use]
    pub const fn very_large(&self) -> f64 {
        self.very_large
    }
}

impl Default for TierMultipliers {
    fn default() -> Self {
        Self {
            large: 5.0,
            very_large: 10.0,
        }
    }
}

impl<'de> Deserialize<'de> for TierMultipliers {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct TierMultipliersHelper {
            large: f64,
            very_large: f64,
        }

        let helper = TierMultipliersHelper::deserialize(deserializer)?;
        Self::new(helper.large, helper.very_large)
            .map_err(|e| de::Error::custom(format!("Invalid tier multipliers: {:?}", e)))
    }
}

impl<'de> Deserialize<'de> for BlockTradeConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct BlockTradeConfigHelper {
            thresholds: HashMap<String, Quantity>,
            default_threshold: Option<Quantity>,
            tier_multipliers: TierMultipliers,
        }

        let helper = BlockTradeConfigHelper::deserialize(deserializer)?;

        // Validate that all thresholds are positive (Quantity already enforces this)
        // The TierMultipliers validation happens in its own Deserialize impl
        Ok(Self {
            thresholds: helper.thresholds,
            default_threshold: helper.default_threshold,
            tier_multipliers: helper.tier_multipliers,
        })
    }
}

impl ReportingTier {
    /// Returns the reporting delay in minutes for this tier.
    ///
    /// - Standard: 15 minutes
    /// - Large: 60 minutes
    /// - VeryLarge: 1440 minutes (end of day / 24 hours)
    #[must_use]
    pub const fn delay_minutes(&self) -> u32 {
        match self {
            Self::Standard => 15,
            Self::Large => 60,
            Self::VeryLarge => 1440,
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::domain::value_objects::{AssetClass, Symbol};

    fn create_btc_instrument() -> Instrument {
        let symbol = Symbol::new("BTC/USD").unwrap();
        Instrument::builder(symbol, AssetClass::CryptoSpot).build()
    }

    fn create_eth_instrument() -> Instrument {
        let symbol = Symbol::new("ETH/USD").unwrap();
        Instrument::builder(symbol, AssetClass::CryptoSpot).build()
    }

    fn create_unknown_instrument() -> Instrument {
        let symbol = Symbol::new("SOL/USD").unwrap();
        Instrument::builder(symbol, AssetClass::CryptoSpot).build()
    }

    #[test]
    fn default_configuration_has_correct_thresholds() {
        let config = BlockTradeConfig::default();

        assert_eq!(
            config.thresholds.get("BTC"),
            Some(&Quantity::new(25.0).unwrap())
        );
        assert_eq!(
            config.thresholds.get("ETH"),
            Some(&Quantity::new(250.0).unwrap())
        );
        assert_eq!(config.default_threshold, None);
    }

    #[test]
    fn default_tier_multipliers_are_correct() {
        let multipliers = TierMultipliers::default();

        assert_eq!(multipliers.large, 5.0);
        assert_eq!(multipliers.very_large, 10.0);
    }

    #[test]
    fn btc_at_threshold_qualifies() {
        let config = BlockTradeConfig::default();
        let instrument = create_btc_instrument();
        let quantity = Quantity::new(25.0).unwrap();

        assert!(config.qualifies(&instrument, quantity));
    }

    #[test]
    fn btc_below_threshold_does_not_qualify() {
        let config = BlockTradeConfig::default();
        let instrument = create_btc_instrument();
        let quantity = Quantity::new(24.9).unwrap();

        assert!(!config.qualifies(&instrument, quantity));
    }

    #[test]
    fn btc_above_threshold_qualifies() {
        let config = BlockTradeConfig::default();
        let instrument = create_btc_instrument();
        let quantity = Quantity::new(30.0).unwrap();

        assert!(config.qualifies(&instrument, quantity));
    }

    #[test]
    fn eth_at_threshold_qualifies() {
        let config = BlockTradeConfig::default();
        let instrument = create_eth_instrument();
        let quantity = Quantity::new(250.0).unwrap();

        assert!(config.qualifies(&instrument, quantity));
    }

    #[test]
    fn eth_below_threshold_does_not_qualify() {
        let config = BlockTradeConfig::default();
        let instrument = create_eth_instrument();
        let quantity = Quantity::new(249.9).unwrap();

        assert!(!config.qualifies(&instrument, quantity));
    }

    #[test]
    fn unknown_instrument_without_default_does_not_qualify() {
        let config = BlockTradeConfig::default();
        let instrument = create_unknown_instrument();
        let quantity = Quantity::new(1000.0).unwrap();

        assert!(!config.qualifies(&instrument, quantity));
    }

    #[test]
    fn unknown_instrument_with_default_qualifies_if_above_default() {
        let config = BlockTradeConfig {
            default_threshold: Some(Quantity::new(100.0).unwrap()),
            ..Default::default()
        };

        let instrument = create_unknown_instrument();
        let quantity = Quantity::new(150.0).unwrap();

        assert!(config.qualifies(&instrument, quantity));
    }

    #[test]
    fn btc_1x_threshold_is_standard_tier() {
        let config = BlockTradeConfig::default();
        let instrument = create_btc_instrument();
        let quantity = Quantity::new(25.0).unwrap();

        assert_eq!(
            config.determine_tier(&instrument, quantity),
            Some(ReportingTier::Standard)
        );
    }

    #[test]
    fn btc_5x_threshold_is_large_tier() {
        let config = BlockTradeConfig::default();
        let instrument = create_btc_instrument();
        let quantity = Quantity::new(125.0).unwrap();

        assert_eq!(
            config.determine_tier(&instrument, quantity),
            Some(ReportingTier::Large)
        );
    }

    #[test]
    fn btc_10x_threshold_is_very_large_tier() {
        let config = BlockTradeConfig::default();
        let instrument = create_btc_instrument();
        let quantity = Quantity::new(250.0).unwrap();

        assert_eq!(
            config.determine_tier(&instrument, quantity),
            Some(ReportingTier::VeryLarge)
        );
    }

    #[test]
    fn eth_1x_threshold_is_standard_tier() {
        let config = BlockTradeConfig::default();
        let instrument = create_eth_instrument();
        let quantity = Quantity::new(250.0).unwrap();

        assert_eq!(
            config.determine_tier(&instrument, quantity),
            Some(ReportingTier::Standard)
        );
    }

    #[test]
    fn eth_5x_threshold_is_large_tier() {
        let config = BlockTradeConfig::default();
        let instrument = create_eth_instrument();
        let quantity = Quantity::new(1250.0).unwrap();

        assert_eq!(
            config.determine_tier(&instrument, quantity),
            Some(ReportingTier::Large)
        );
    }

    #[test]
    fn eth_10x_threshold_is_very_large_tier() {
        let config = BlockTradeConfig::default();
        let instrument = create_eth_instrument();
        let quantity = Quantity::new(2500.0).unwrap();

        assert_eq!(
            config.determine_tier(&instrument, quantity),
            Some(ReportingTier::VeryLarge)
        );
    }

    #[test]
    fn below_threshold_returns_none_tier() {
        let config = BlockTradeConfig::default();
        let instrument = create_btc_instrument();
        let quantity = Quantity::new(24.9).unwrap();

        assert_eq!(config.determine_tier(&instrument, quantity), None);
    }

    #[test]
    fn custom_thresholds_work() {
        let mut config = BlockTradeConfig::default();
        config
            .thresholds
            .insert("SOL".to_string(), Quantity::new(1000.0).unwrap());

        let instrument = create_unknown_instrument();
        let quantity = Quantity::new(1000.0).unwrap();

        assert!(config.qualifies(&instrument, quantity));
    }

    #[test]
    fn custom_tier_multipliers_work() {
        let config = BlockTradeConfig {
            tier_multipliers: TierMultipliers::new(2.0, 4.0).unwrap(),
            ..Default::default()
        };

        let instrument = create_btc_instrument();

        assert_eq!(
            config.determine_tier(&instrument, Quantity::new(50.0).unwrap()),
            Some(ReportingTier::Large)
        );
        assert_eq!(
            config.determine_tier(&instrument, Quantity::new(100.0).unwrap()),
            Some(ReportingTier::VeryLarge)
        );
    }

    #[test]
    fn get_threshold_returns_correct_value() {
        let config = BlockTradeConfig::default();

        assert_eq!(
            config.get_threshold(&create_btc_instrument()),
            Some(Quantity::new(25.0).unwrap())
        );
        assert_eq!(
            config.get_threshold(&create_eth_instrument()),
            Some(Quantity::new(250.0).unwrap())
        );
        assert_eq!(config.get_threshold(&create_unknown_instrument()), None);
    }

    #[test]
    fn reporting_tier_delay_minutes_are_correct() {
        assert_eq!(ReportingTier::Standard.delay_minutes(), 15);
        assert_eq!(ReportingTier::Large.delay_minutes(), 60);
        assert_eq!(ReportingTier::VeryLarge.delay_minutes(), 1440);
    }

    #[test]
    fn serialization_round_trip_preserves_config() {
        let config = BlockTradeConfig::default();

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: BlockTradeConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(config.thresholds.len(), deserialized.thresholds.len());
        assert_eq!(
            config.thresholds.get("BTC"),
            deserialized.thresholds.get("BTC")
        );
        assert_eq!(
            config.thresholds.get("ETH"),
            deserialized.thresholds.get("ETH")
        );
        assert_eq!(config.default_threshold, deserialized.default_threshold);
        assert_eq!(
            config.tier_multipliers.large,
            deserialized.tier_multipliers.large
        );
        assert_eq!(
            config.tier_multipliers.very_large,
            deserialized.tier_multipliers.very_large
        );
    }

    #[test]
    fn serialization_round_trip_preserves_tier() {
        let tier = ReportingTier::Large;

        let json = serde_json::to_string(&tier).unwrap();
        let deserialized: ReportingTier = serde_json::from_str(&json).unwrap();

        assert_eq!(tier, deserialized);
    }

    #[test]
    fn zero_quantity_does_not_qualify() {
        let config = BlockTradeConfig::default();
        let instrument = create_btc_instrument();
        let quantity = Quantity::new(0.0).unwrap();

        assert!(!config.qualifies(&instrument, quantity));
    }
}
