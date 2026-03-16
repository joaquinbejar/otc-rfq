//! # Quote Normalizer Entities
//!
//! Domain entities for quote normalization across different venues.
//!
//! This module provides types for normalizing quotes from different venues
//! into a standard format for fair comparison. It handles:
//! - Price normalization (FX conversion to base currency)
//! - Quantity normalization (contract unit conversion)
//! - Fee normalization (all-in price calculation)
//! - Timestamp normalization (latency adjustment)
//!
//! # Examples
//!
//! ```
//! use otc_rfq::domain::entities::quote_normalizer::{
//!     QuoteType, NormalizationConfig, NormalizedQuote,
//! };
//! use rust_decimal::Decimal;
//!
//! let config = NormalizationConfig::default();
//! assert!(config.include_fees());
//! ```

use crate::domain::value_objects::ids::{QuoteId, RfqId, VenueId};
use crate::domain::value_objects::symbol::Symbol;
use crate::domain::value_objects::timestamp::Timestamp;
use crate::domain::value_objects::{Price, Quantity};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

/// Type of quote indicating its executability and confirmation requirements.
///
/// Different quote types have different semantics:
/// - **Firm**: Executable immediately at the quoted price
/// - **Indicative**: Display only, not executable (used for price discovery)
/// - **LastLook**: Requires MM confirmation before execution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum QuoteType {
    /// Firm quote - executable immediately at quoted price.
    #[default]
    Firm,
    /// Indicative quote - display only, not executable.
    Indicative,
    /// Last-look quote - requires MM confirmation before execution.
    LastLook,
}

impl QuoteType {
    /// Returns true if this quote type is executable.
    #[must_use]
    pub const fn is_executable(&self) -> bool {
        matches!(self, Self::Firm | Self::LastLook)
    }

    /// Returns true if this quote requires last-look confirmation.
    #[must_use]
    pub const fn requires_last_look(&self) -> bool {
        matches!(self, Self::LastLook)
    }

    /// Returns true if this quote is indicative only.
    #[must_use]
    pub const fn is_indicative(&self) -> bool {
        matches!(self, Self::Indicative)
    }
}

impl fmt::Display for QuoteType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Firm => write!(f, "Firm"),
            Self::Indicative => write!(f, "Indicative"),
            Self::LastLook => write!(f, "LastLook"),
        }
    }
}

/// Foreign exchange rate for currency conversion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FxRate {
    /// Source currency (e.g., "EUR").
    from_currency: String,
    /// Target currency (e.g., "USD").
    to_currency: String,
    /// Exchange rate (multiply source by this to get target).
    rate: Decimal,
    /// When this rate was observed.
    timestamp: Timestamp,
}

impl FxRate {
    /// Creates a new FX rate.
    ///
    /// # Arguments
    ///
    /// * `from_currency` - Source currency code
    /// * `to_currency` - Target currency code
    /// * `rate` - Exchange rate (from * rate = to)
    #[must_use]
    pub fn new(
        from_currency: impl Into<String>,
        to_currency: impl Into<String>,
        rate: Decimal,
    ) -> Self {
        Self {
            from_currency: from_currency.into(),
            to_currency: to_currency.into(),
            rate,
            timestamp: Timestamp::now(),
        }
    }

    /// Creates a new FX rate with a specific timestamp.
    #[must_use]
    pub fn with_timestamp(
        from_currency: impl Into<String>,
        to_currency: impl Into<String>,
        rate: Decimal,
        timestamp: Timestamp,
    ) -> Self {
        Self {
            from_currency: from_currency.into(),
            to_currency: to_currency.into(),
            rate,
            timestamp,
        }
    }

    /// Returns the source currency.
    #[must_use]
    pub fn from_currency(&self) -> &str {
        &self.from_currency
    }

    /// Returns the target currency.
    #[must_use]
    pub fn to_currency(&self) -> &str {
        &self.to_currency
    }

    /// Returns the exchange rate.
    #[must_use]
    pub fn rate(&self) -> Decimal {
        self.rate
    }

    /// Returns when this rate was observed.
    #[must_use]
    pub fn timestamp(&self) -> Timestamp {
        self.timestamp
    }

    /// Converts an amount from source to target currency.
    #[must_use]
    pub fn convert(&self, amount: Decimal) -> Decimal {
        amount * self.rate
    }

    /// Returns the inverse rate (for reverse conversion).
    ///
    /// Note: If the original rate is zero, the inverse rate will also be zero
    /// to prevent division by zero. Any subsequent conversion using this inverse
    /// rate will result in zero.
    #[must_use]
    pub fn inverse(&self) -> Self {
        Self {
            from_currency: self.to_currency.clone(),
            to_currency: self.from_currency.clone(),
            rate: if self.rate.is_zero() {
                Decimal::ZERO
            } else {
                Decimal::ONE / self.rate
            },
            timestamp: self.timestamp,
        }
    }
}

impl fmt::Display for FxRate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}/{} = {}",
            self.from_currency, self.to_currency, self.rate
        )
    }
}

/// Configuration for normalizing quotes from a specific venue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NormalizationConfig {
    /// Venue identifier this config applies to.
    venue_id: Option<VenueId>,
    /// Base currency for price normalization (e.g., "USD").
    base_currency: String,
    /// Whether to include fees in the normalized price.
    include_fees: bool,
    /// Whether venue quotes already include fees.
    fees_included_in_quote: bool,
    /// Default fee rate if not specified in quote (as decimal, e.g., 0.001 = 0.1%).
    default_fee_rate: Decimal,
    /// Contract size multiplier for quantity normalization.
    contract_multiplier: Decimal,
    /// Latency adjustment in milliseconds (added to quote timestamp).
    latency_adjustment_ms: i64,
}

impl Default for NormalizationConfig {
    fn default() -> Self {
        Self {
            venue_id: None,
            base_currency: "USD".to_string(),
            include_fees: true,
            fees_included_in_quote: false,
            default_fee_rate: Decimal::new(1, 3), // 0.001 = 0.1%
            contract_multiplier: Decimal::ONE,
            latency_adjustment_ms: 0,
        }
    }
}

impl NormalizationConfig {
    /// Creates a new normalization config for a venue.
    #[must_use]
    pub fn new(venue_id: VenueId) -> Self {
        Self {
            venue_id: Some(venue_id),
            ..Default::default()
        }
    }

    /// Creates a builder for constructing a normalization config.
    #[must_use]
    pub fn builder() -> NormalizationConfigBuilder {
        NormalizationConfigBuilder::default()
    }

    /// Returns the venue ID this config applies to.
    #[must_use]
    pub fn venue_id(&self) -> Option<&VenueId> {
        self.venue_id.as_ref()
    }

    /// Returns the base currency.
    #[must_use]
    pub fn base_currency(&self) -> &str {
        &self.base_currency
    }

    /// Returns whether to include fees in normalized price.
    #[must_use]
    pub const fn include_fees(&self) -> bool {
        self.include_fees
    }

    /// Returns whether venue quotes already include fees.
    #[must_use]
    pub const fn fees_included_in_quote(&self) -> bool {
        self.fees_included_in_quote
    }

    /// Returns the default fee rate.
    #[must_use]
    pub fn default_fee_rate(&self) -> Decimal {
        self.default_fee_rate
    }

    /// Returns the contract multiplier.
    #[must_use]
    pub fn contract_multiplier(&self) -> Decimal {
        self.contract_multiplier
    }

    /// Returns the latency adjustment in milliseconds.
    #[must_use]
    pub const fn latency_adjustment_ms(&self) -> i64 {
        self.latency_adjustment_ms
    }
}

/// Builder for [`NormalizationConfig`].
#[derive(Debug, Clone, Default)]
pub struct NormalizationConfigBuilder {
    venue_id: Option<VenueId>,
    base_currency: Option<String>,
    include_fees: Option<bool>,
    fees_included_in_quote: Option<bool>,
    default_fee_rate: Option<Decimal>,
    contract_multiplier: Option<Decimal>,
    latency_adjustment_ms: Option<i64>,
}

impl NormalizationConfigBuilder {
    /// Sets the venue ID.
    #[must_use]
    pub fn venue_id(mut self, venue_id: VenueId) -> Self {
        self.venue_id = Some(venue_id);
        self
    }

    /// Sets the base currency.
    #[must_use]
    pub fn base_currency(mut self, currency: impl Into<String>) -> Self {
        self.base_currency = Some(currency.into());
        self
    }

    /// Sets whether to include fees.
    #[must_use]
    pub const fn include_fees(mut self, include: bool) -> Self {
        self.include_fees = Some(include);
        self
    }

    /// Sets whether venue quotes already include fees.
    #[must_use]
    pub const fn fees_included_in_quote(mut self, included: bool) -> Self {
        self.fees_included_in_quote = Some(included);
        self
    }

    /// Sets the default fee rate.
    #[must_use]
    pub fn default_fee_rate(mut self, rate: Decimal) -> Self {
        self.default_fee_rate = Some(rate);
        self
    }

    /// Sets the contract multiplier.
    #[must_use]
    pub fn contract_multiplier(mut self, multiplier: Decimal) -> Self {
        self.contract_multiplier = Some(multiplier);
        self
    }

    /// Sets the latency adjustment.
    #[must_use]
    pub const fn latency_adjustment_ms(mut self, ms: i64) -> Self {
        self.latency_adjustment_ms = Some(ms);
        self
    }

    /// Builds the normalization config.
    #[must_use]
    pub fn build(self) -> NormalizationConfig {
        let default = NormalizationConfig::default();
        NormalizationConfig {
            venue_id: self.venue_id,
            base_currency: self.base_currency.unwrap_or(default.base_currency),
            include_fees: self.include_fees.unwrap_or(default.include_fees),
            fees_included_in_quote: self
                .fees_included_in_quote
                .unwrap_or(default.fees_included_in_quote),
            default_fee_rate: self.default_fee_rate.unwrap_or(default.default_fee_rate),
            contract_multiplier: self
                .contract_multiplier
                .unwrap_or(default.contract_multiplier),
            latency_adjustment_ms: self
                .latency_adjustment_ms
                .unwrap_or(default.latency_adjustment_ms),
        }
    }
}

/// A quote that has been normalized to a standard format.
///
/// Contains the original quote data plus normalized values for fair comparison.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NormalizedQuote {
    /// Original quote ID.
    quote_id: QuoteId,
    /// RFQ this quote responds to.
    rfq_id: RfqId,
    /// Venue that provided the quote.
    venue_id: VenueId,
    /// Instrument symbol.
    symbol: Option<Symbol>,
    /// Quote type (Firm, Indicative, LastLook).
    quote_type: QuoteType,
    /// Original price before normalization.
    original_price: Price,
    /// Normalized price in base currency.
    normalized_price: Price,
    /// Original quantity before normalization.
    original_quantity: Quantity,
    /// Normalized quantity in standard contract units.
    normalized_quantity: Quantity,
    /// All-in price including fees.
    all_in_price: Price,
    /// Fee amount (absolute value in base currency).
    fee_amount: Decimal,
    /// Fee rate applied (as decimal).
    fee_rate: Decimal,
    /// Original timestamp from venue.
    original_timestamp: Timestamp,
    /// Latency-adjusted timestamp.
    adjusted_timestamp: Timestamp,
    /// When the quote expires.
    valid_until: Timestamp,
    /// Whether this quote is executable.
    is_executable: bool,
    /// Whether this quote requires last-look confirmation.
    requires_last_look: bool,
    /// Currency the normalized price is in.
    currency: String,
    /// FX rate used for conversion (if any).
    fx_rate_used: Option<Decimal>,
}

impl NormalizedQuote {
    /// Creates a new normalized quote.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        quote_id: QuoteId,
        rfq_id: RfqId,
        venue_id: VenueId,
        quote_type: QuoteType,
        original_price: Price,
        normalized_price: Price,
        original_quantity: Quantity,
        normalized_quantity: Quantity,
        all_in_price: Price,
        fee_amount: Decimal,
        fee_rate: Decimal,
        original_timestamp: Timestamp,
        adjusted_timestamp: Timestamp,
        valid_until: Timestamp,
        currency: String,
    ) -> Self {
        Self {
            quote_id,
            rfq_id,
            venue_id,
            symbol: None,
            quote_type,
            original_price,
            normalized_price,
            original_quantity,
            normalized_quantity,
            all_in_price,
            fee_amount,
            fee_rate,
            original_timestamp,
            adjusted_timestamp,
            valid_until,
            is_executable: quote_type.is_executable(),
            requires_last_look: quote_type.requires_last_look(),
            currency,
            fx_rate_used: None,
        }
    }

    /// Sets the symbol.
    #[must_use]
    pub fn with_symbol(mut self, symbol: Symbol) -> Self {
        self.symbol = Some(symbol);
        self
    }

    /// Sets the FX rate used for conversion.
    #[must_use]
    pub fn with_fx_rate(mut self, rate: Decimal) -> Self {
        self.fx_rate_used = Some(rate);
        self
    }

    /// Returns the quote ID.
    #[must_use]
    pub fn quote_id(&self) -> QuoteId {
        self.quote_id
    }

    /// Returns the RFQ ID.
    #[must_use]
    pub fn rfq_id(&self) -> RfqId {
        self.rfq_id
    }

    /// Returns the venue ID.
    #[must_use]
    pub fn venue_id(&self) -> &VenueId {
        &self.venue_id
    }

    /// Returns the symbol if set.
    #[must_use]
    pub fn symbol(&self) -> Option<&Symbol> {
        self.symbol.as_ref()
    }

    /// Returns the quote type.
    #[must_use]
    pub fn quote_type(&self) -> QuoteType {
        self.quote_type
    }

    /// Returns the original price.
    #[must_use]
    pub fn original_price(&self) -> &Price {
        &self.original_price
    }

    /// Returns the normalized price.
    #[must_use]
    pub fn normalized_price(&self) -> &Price {
        &self.normalized_price
    }

    /// Returns the original quantity.
    #[must_use]
    pub fn original_quantity(&self) -> &Quantity {
        &self.original_quantity
    }

    /// Returns the normalized quantity.
    #[must_use]
    pub fn normalized_quantity(&self) -> &Quantity {
        &self.normalized_quantity
    }

    /// Returns the all-in price (including fees).
    #[must_use]
    pub fn all_in_price(&self) -> &Price {
        &self.all_in_price
    }

    /// Returns the fee amount.
    #[must_use]
    pub fn fee_amount(&self) -> Decimal {
        self.fee_amount
    }

    /// Returns the fee rate.
    #[must_use]
    pub fn fee_rate(&self) -> Decimal {
        self.fee_rate
    }

    /// Returns the original timestamp.
    #[must_use]
    pub fn original_timestamp(&self) -> Timestamp {
        self.original_timestamp
    }

    /// Returns the latency-adjusted timestamp.
    #[must_use]
    pub fn adjusted_timestamp(&self) -> Timestamp {
        self.adjusted_timestamp
    }

    /// Returns when the quote expires.
    #[must_use]
    pub fn valid_until(&self) -> Timestamp {
        self.valid_until
    }

    /// Returns whether this quote is executable.
    #[must_use]
    pub const fn is_executable(&self) -> bool {
        self.is_executable
    }

    /// Returns whether this quote requires last-look confirmation.
    #[must_use]
    pub const fn requires_last_look(&self) -> bool {
        self.requires_last_look
    }

    /// Returns the currency.
    #[must_use]
    pub fn currency(&self) -> &str {
        &self.currency
    }

    /// Returns the FX rate used for conversion.
    #[must_use]
    pub fn fx_rate_used(&self) -> Option<Decimal> {
        self.fx_rate_used
    }

    /// Returns true if the quote has expired.
    #[must_use]
    pub fn is_expired(&self) -> bool {
        self.valid_until < Timestamp::now()
    }
}

impl fmt::Display for NormalizedQuote {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "NormalizedQuote({} {} @ {} {} all-in={} {})",
            self.quote_type,
            self.venue_id,
            self.normalized_price,
            self.currency,
            self.all_in_price,
            if self.is_executable {
                "[executable]"
            } else {
                "[indicative]"
            }
        )
    }
}

/// Registry of normalization configs by venue.
#[derive(Debug, Clone, Default)]
pub struct NormalizationConfigRegistry {
    /// Configs by venue ID.
    configs: HashMap<String, NormalizationConfig>,
    /// Default config for unknown venues.
    default_config: NormalizationConfig,
}

impl NormalizationConfigRegistry {
    /// Creates a new empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a registry with a custom default config.
    #[must_use]
    pub fn with_default(default_config: NormalizationConfig) -> Self {
        Self {
            configs: HashMap::new(),
            default_config,
        }
    }

    /// Registers a config for a venue.
    pub fn register(&mut self, venue_id: &VenueId, config: NormalizationConfig) {
        self.configs.insert(venue_id.as_str().to_string(), config);
    }

    /// Gets the config for a venue, or the default if not found.
    #[must_use]
    pub fn get(&self, venue_id: &VenueId) -> &NormalizationConfig {
        self.configs
            .get(venue_id.as_str())
            .unwrap_or(&self.default_config)
    }

    /// Returns the default config.
    #[must_use]
    pub fn default_config(&self) -> &NormalizationConfig {
        &self.default_config
    }

    /// Returns the number of registered configs.
    #[must_use]
    pub fn len(&self) -> usize {
        self.configs.len()
    }

    /// Returns true if no configs are registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.configs.is_empty()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn quote_type_is_executable() {
        assert!(QuoteType::Firm.is_executable());
        assert!(!QuoteType::Indicative.is_executable());
        assert!(QuoteType::LastLook.is_executable());
    }

    #[test]
    fn quote_type_requires_last_look() {
        assert!(!QuoteType::Firm.requires_last_look());
        assert!(!QuoteType::Indicative.requires_last_look());
        assert!(QuoteType::LastLook.requires_last_look());
    }

    #[test]
    fn quote_type_is_indicative() {
        assert!(!QuoteType::Firm.is_indicative());
        assert!(QuoteType::Indicative.is_indicative());
        assert!(!QuoteType::LastLook.is_indicative());
    }

    #[test]
    fn quote_type_display() {
        assert_eq!(QuoteType::Firm.to_string(), "Firm");
        assert_eq!(QuoteType::Indicative.to_string(), "Indicative");
        assert_eq!(QuoteType::LastLook.to_string(), "LastLook");
    }

    #[test]
    fn fx_rate_new() {
        let rate = FxRate::new("EUR", "USD", Decimal::new(110, 2));
        assert_eq!(rate.from_currency(), "EUR");
        assert_eq!(rate.to_currency(), "USD");
        assert_eq!(rate.rate(), Decimal::new(110, 2));
    }

    #[test]
    fn fx_rate_convert() {
        let rate = FxRate::new("EUR", "USD", Decimal::new(110, 2)); // 1.10
        let result = rate.convert(Decimal::new(100, 0)); // 100 EUR
        assert_eq!(result, Decimal::new(110, 0)); // 110 USD
    }

    #[test]
    fn fx_rate_inverse() {
        let rate = FxRate::new("EUR", "USD", Decimal::new(2, 0)); // 2.0
        let inverse = rate.inverse();
        assert_eq!(inverse.from_currency(), "USD");
        assert_eq!(inverse.to_currency(), "EUR");
        assert_eq!(inverse.rate(), Decimal::new(5, 1)); // 0.5
    }

    #[test]
    fn fx_rate_display() {
        let rate = FxRate::new("EUR", "USD", Decimal::new(110, 2));
        assert_eq!(rate.to_string(), "EUR/USD = 1.10");
    }

    #[test]
    fn normalization_config_default() {
        let config = NormalizationConfig::default();
        assert_eq!(config.base_currency(), "USD");
        assert!(config.include_fees());
        assert!(!config.fees_included_in_quote());
        assert_eq!(config.default_fee_rate(), Decimal::new(1, 3));
        assert_eq!(config.contract_multiplier(), Decimal::ONE);
        assert_eq!(config.latency_adjustment_ms(), 0);
    }

    #[test]
    fn normalization_config_builder() {
        let config = NormalizationConfig::builder()
            .venue_id(VenueId::new("binance"))
            .base_currency("EUR")
            .include_fees(false)
            .fees_included_in_quote(true)
            .default_fee_rate(Decimal::new(5, 4)) // 0.0005
            .contract_multiplier(Decimal::new(10, 0))
            .latency_adjustment_ms(50)
            .build();

        assert_eq!(config.venue_id().unwrap().as_str(), "binance");
        assert_eq!(config.base_currency(), "EUR");
        assert!(!config.include_fees());
        assert!(config.fees_included_in_quote());
        assert_eq!(config.default_fee_rate(), Decimal::new(5, 4));
        assert_eq!(config.contract_multiplier(), Decimal::new(10, 0));
        assert_eq!(config.latency_adjustment_ms(), 50);
    }

    #[test]
    fn normalization_config_registry() {
        let mut registry = NormalizationConfigRegistry::new();

        let binance_config = NormalizationConfig::builder()
            .venue_id(VenueId::new("binance"))
            .default_fee_rate(Decimal::new(1, 4)) // 0.0001
            .build();

        registry.register(&VenueId::new("binance"), binance_config);

        assert_eq!(registry.len(), 1);
        assert!(!registry.is_empty());

        let config = registry.get(&VenueId::new("binance"));
        assert_eq!(config.default_fee_rate(), Decimal::new(1, 4));

        // Unknown venue returns default
        let unknown = registry.get(&VenueId::new("unknown"));
        assert_eq!(unknown.default_fee_rate(), Decimal::new(1, 3));
    }

    #[test]
    fn normalized_quote_is_executable() {
        let quote = NormalizedQuote::new(
            QuoteId::new_v4(),
            RfqId::new_v4(),
            VenueId::new("test"),
            QuoteType::Firm,
            Price::new(100.0).unwrap(),
            Price::new(100.0).unwrap(),
            Quantity::new(10.0).unwrap(),
            Quantity::new(10.0).unwrap(),
            Price::new(100.1).unwrap(),
            Decimal::new(1, 1),
            Decimal::new(1, 3),
            Timestamp::now(),
            Timestamp::now(),
            Timestamp::now().add_secs(60),
            "USD".to_string(),
        );

        assert!(quote.is_executable());
        assert!(!quote.requires_last_look());
    }

    #[test]
    fn normalized_quote_indicative_not_executable() {
        let quote = NormalizedQuote::new(
            QuoteId::new_v4(),
            RfqId::new_v4(),
            VenueId::new("test"),
            QuoteType::Indicative,
            Price::new(100.0).unwrap(),
            Price::new(100.0).unwrap(),
            Quantity::new(10.0).unwrap(),
            Quantity::new(10.0).unwrap(),
            Price::new(100.1).unwrap(),
            Decimal::new(1, 1),
            Decimal::new(1, 3),
            Timestamp::now(),
            Timestamp::now(),
            Timestamp::now().add_secs(60),
            "USD".to_string(),
        );

        assert!(!quote.is_executable());
        assert!(!quote.requires_last_look());
    }

    #[test]
    fn normalized_quote_last_look() {
        let quote = NormalizedQuote::new(
            QuoteId::new_v4(),
            RfqId::new_v4(),
            VenueId::new("test"),
            QuoteType::LastLook,
            Price::new(100.0).unwrap(),
            Price::new(100.0).unwrap(),
            Quantity::new(10.0).unwrap(),
            Quantity::new(10.0).unwrap(),
            Price::new(100.1).unwrap(),
            Decimal::new(1, 1),
            Decimal::new(1, 3),
            Timestamp::now(),
            Timestamp::now(),
            Timestamp::now().add_secs(60),
            "USD".to_string(),
        );

        assert!(quote.is_executable());
        assert!(quote.requires_last_look());
    }
}
