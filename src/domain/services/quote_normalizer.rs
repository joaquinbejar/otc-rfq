//! # Quote Normalizer Service
//!
//! Service for normalizing quotes from different venues into a standard format.
//!
//! The normalizer handles:
//! - **Price normalization**: Convert venue-native currency to base currency using FX rates
//! - **Quantity normalization**: Convert venue-native units to standard contract units
//! - **Fee normalization**: Compute all-in price (price + commission) for fair comparison
//! - **Timestamp normalization**: Align to server clock with latency adjustment
//!
//! # Examples
//!
//! ```ignore
//! use otc_rfq::domain::services::quote_normalizer::QuoteNormalizer;
//! use otc_rfq::domain::entities::quote_normalizer::{NormalizationConfig, QuoteType};
//!
//! let normalizer = QuoteNormalizer::new();
//! let config = NormalizationConfig::default();
//! let normalized = normalizer.normalize(&quote, &config, QuoteType::Firm, None);
//! ```

use crate::domain::entities::quote::Quote;
use crate::domain::entities::quote_normalizer::{
    FxRate, NormalizationConfig, NormalizationConfigRegistry, NormalizedQuote, QuoteType,
};
use crate::domain::value_objects::{Price, Quantity};
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::fmt;

/// Service for normalizing quotes to a standard format.
///
/// Converts quotes from different venues into a comparable format by:
/// - Converting prices to a base currency
/// - Normalizing quantities to standard contract units
/// - Computing all-in prices including fees
/// - Adjusting timestamps for latency
#[derive(Debug, Clone)]
pub struct QuoteNormalizer {
    /// Registry of per-venue normalization configs.
    config_registry: NormalizationConfigRegistry,
    /// FX rates for currency conversion (key: "FROM/TO").
    fx_rates: HashMap<String, FxRate>,
}

impl Default for QuoteNormalizer {
    fn default() -> Self {
        Self::new()
    }
}

impl QuoteNormalizer {
    /// Creates a new quote normalizer with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            config_registry: NormalizationConfigRegistry::new(),
            fx_rates: HashMap::new(),
        }
    }

    /// Creates a normalizer with a custom config registry.
    #[must_use]
    pub fn with_config_registry(config_registry: NormalizationConfigRegistry) -> Self {
        Self {
            config_registry,
            fx_rates: HashMap::new(),
        }
    }

    /// Adds an FX rate for currency conversion.
    pub fn add_fx_rate(&mut self, rate: FxRate) {
        let key = format!("{}/{}", rate.from_currency(), rate.to_currency());
        self.fx_rates.insert(key, rate);
    }

    /// Gets an FX rate for a currency pair.
    #[must_use]
    pub fn get_fx_rate(&self, from: &str, to: &str) -> Option<&FxRate> {
        let key = format!("{from}/{to}");
        self.fx_rates.get(&key)
    }

    /// Returns the config registry.
    #[must_use]
    pub fn config_registry(&self) -> &NormalizationConfigRegistry {
        &self.config_registry
    }

    /// Returns a mutable reference to the config registry.
    pub fn config_registry_mut(&mut self) -> &mut NormalizationConfigRegistry {
        &mut self.config_registry
    }

    /// Normalizes a quote using the venue's configuration.
    ///
    /// # Arguments
    ///
    /// * `quote` - The quote to normalize
    /// * `quote_type` - The type of quote (Firm, Indicative, LastLook)
    /// * `source_currency` - Optional source currency (defaults to base currency)
    ///
    /// # Returns
    ///
    /// A normalized quote with standardized price, quantity, and fees.
    #[must_use]
    pub fn normalize(
        &self,
        quote: &Quote,
        quote_type: QuoteType,
        source_currency: Option<&str>,
    ) -> NormalizedQuote {
        let config = self.config_registry.get(quote.venue_id());
        self.normalize_with_config(quote, config, quote_type, source_currency)
    }

    /// Normalizes a quote using a specific configuration.
    ///
    /// # Arguments
    ///
    /// * `quote` - The quote to normalize
    /// * `config` - The normalization configuration to use
    /// * `quote_type` - The type of quote (Firm, Indicative, LastLook)
    /// * `source_currency` - Optional source currency (defaults to base currency)
    ///
    /// # Returns
    ///
    /// A normalized quote with standardized price, quantity, and fees.
    #[must_use]
    pub fn normalize_with_config(
        &self,
        quote: &Quote,
        config: &NormalizationConfig,
        quote_type: QuoteType,
        source_currency: Option<&str>,
    ) -> NormalizedQuote {
        let original_price = quote.price();
        let original_quantity = quote.quantity();
        let original_timestamp = quote.created_at();

        // 1. Price normalization (FX conversion)
        let (normalized_price, fx_rate_used) =
            self.normalize_price(&original_price, config, source_currency);

        // 2. Quantity normalization (contract multiplier)
        let normalized_quantity = self.normalize_quantity(&original_quantity, config);

        // 3. Fee normalization (all-in price)
        let (all_in_price, fee_amount, fee_rate) =
            self.normalize_fees(&normalized_price, quote, config);

        // 4. Timestamp normalization (latency adjustment)
        let adjusted_timestamp = self.normalize_timestamp(original_timestamp, config);

        let mut normalized = NormalizedQuote::new(
            quote.id(),
            quote.rfq_id(),
            quote.venue_id().clone(),
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
            quote.valid_until(),
            config.base_currency().to_string(),
        );

        if let Some(rate) = fx_rate_used {
            normalized = normalized.with_fx_rate(rate);
        }

        normalized
    }

    /// Normalizes multiple quotes.
    ///
    /// # Arguments
    ///
    /// * `quotes` - The quotes to normalize
    /// * `quote_types` - Map of quote ID to quote type (defaults to Firm if not found)
    /// * `source_currency` - Optional source currency for all quotes
    ///
    /// # Returns
    ///
    /// A vector of normalized quotes.
    #[must_use]
    pub fn normalize_batch(
        &self,
        quotes: &[Quote],
        quote_types: &HashMap<String, QuoteType>,
        source_currency: Option<&str>,
    ) -> Vec<NormalizedQuote> {
        quotes
            .iter()
            .map(|q| {
                let quote_type = quote_types
                    .get(&q.id().to_string())
                    .copied()
                    .unwrap_or(QuoteType::Firm);
                self.normalize(q, quote_type, source_currency)
            })
            .collect()
    }

    /// Normalizes price by converting to base currency.
    fn normalize_price(
        &self,
        price: &Price,
        config: &NormalizationConfig,
        source_currency: Option<&str>,
    ) -> (Price, Option<Decimal>) {
        let source = source_currency.unwrap_or(config.base_currency());

        // If already in base currency, no conversion needed
        if source == config.base_currency() {
            return (*price, None);
        }

        // Look up FX rate
        if let Some(fx_rate) = self.get_fx_rate(source, config.base_currency()) {
            let converted = fx_rate.convert(price.get());
            if let Ok(new_price) = Price::from_decimal(converted) {
                return (new_price, Some(fx_rate.rate()));
            }
        }

        // No conversion available, return original
        (*price, None)
    }

    /// Normalizes quantity by applying contract multiplier.
    fn normalize_quantity(&self, quantity: &Quantity, config: &NormalizationConfig) -> Quantity {
        let multiplier = config.contract_multiplier();

        if multiplier == Decimal::ONE {
            return *quantity;
        }

        let normalized = quantity.get() * multiplier;
        Quantity::from_decimal(normalized).unwrap_or(*quantity)
    }

    /// Normalizes fees and computes all-in price.
    fn normalize_fees(
        &self,
        normalized_price: &Price,
        quote: &Quote,
        config: &NormalizationConfig,
    ) -> (Price, Decimal, Decimal) {
        // If fees already included in quote and we don't want to include them, return as-is
        if config.fees_included_in_quote() || !config.include_fees() {
            return (*normalized_price, Decimal::ZERO, Decimal::ZERO);
        }

        // Get fee rate from quote commission or use default
        let fee_rate = quote
            .commission()
            .map(|c| {
                // Commission is absolute, convert to rate
                if normalized_price.get().is_zero() {
                    Decimal::ZERO
                } else {
                    c.get() / normalized_price.get()
                }
            })
            .unwrap_or_else(|| config.default_fee_rate());

        // Calculate fee amount
        let fee_amount = normalized_price.get() * fee_rate;

        // Calculate all-in price
        let all_in = normalized_price.get() + fee_amount;
        let all_in_price = Price::from_decimal(all_in).unwrap_or(*normalized_price);

        (all_in_price, fee_amount, fee_rate)
    }

    /// Normalizes timestamp by applying latency adjustment.
    fn normalize_timestamp(
        &self,
        timestamp: crate::domain::value_objects::timestamp::Timestamp,
        config: &NormalizationConfig,
    ) -> crate::domain::value_objects::timestamp::Timestamp {
        let adjustment_ms = config.latency_adjustment_ms();

        if adjustment_ms == 0 {
            return timestamp;
        }

        timestamp.add_millis(adjustment_ms)
    }

    /// Filters normalized quotes to only executable ones.
    #[must_use]
    pub fn filter_executable(quotes: &[NormalizedQuote]) -> Vec<&NormalizedQuote> {
        quotes.iter().filter(|q| q.is_executable()).collect()
    }

    /// Filters normalized quotes to only non-expired ones.
    #[must_use]
    pub fn filter_valid(quotes: &[NormalizedQuote]) -> Vec<&NormalizedQuote> {
        quotes.iter().filter(|q| !q.is_expired()).collect()
    }

    /// Sorts normalized quotes by all-in price (best first for the given side).
    ///
    /// For buy side: lower price is better.
    /// For sell side: higher price is better.
    #[must_use]
    pub fn sort_by_price(
        quotes: &mut [NormalizedQuote],
        is_buy_side: bool,
    ) -> &mut [NormalizedQuote] {
        quotes.sort_by(|a, b| {
            let price_a = a.all_in_price().get();
            let price_b = b.all_in_price().get();

            if is_buy_side {
                price_a.cmp(&price_b) // Lower is better for buy
            } else {
                price_b.cmp(&price_a) // Higher is better for sell
            }
        });
        quotes
    }
}

impl fmt::Display for QuoteNormalizer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "QuoteNormalizer(configs={}, fx_rates={})",
            self.config_registry.len(),
            self.fx_rates.len()
        )
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::domain::value_objects::ids::{RfqId, VenueId};
    use crate::domain::value_objects::timestamp::Timestamp;

    fn create_test_quote(price: f64, quantity: f64, venue: &str) -> Quote {
        Quote::new(
            RfqId::new_v4(),
            VenueId::new(venue),
            Price::new(price).unwrap(),
            Quantity::new(quantity).unwrap(),
            Timestamp::now().add_secs(60),
        )
        .unwrap()
    }

    #[test]
    fn normalizer_default() {
        let normalizer = QuoteNormalizer::new();
        assert!(normalizer.config_registry.is_empty());
        assert!(normalizer.fx_rates.is_empty());
    }

    #[test]
    fn normalize_simple_quote() {
        let normalizer = QuoteNormalizer::new();
        let quote = create_test_quote(100.0, 10.0, "test-venue");

        let normalized = normalizer.normalize(&quote, QuoteType::Firm, None);

        assert_eq!(normalized.quote_id(), quote.id());
        assert_eq!(normalized.venue_id().as_str(), "test-venue");
        assert_eq!(normalized.quote_type(), QuoteType::Firm);
        assert!(normalized.is_executable());
        assert!(!normalized.requires_last_look());
        assert_eq!(normalized.currency(), "USD");
    }

    #[test]
    fn normalize_indicative_quote() {
        let normalizer = QuoteNormalizer::new();
        let quote = create_test_quote(100.0, 10.0, "test-venue");

        let normalized = normalizer.normalize(&quote, QuoteType::Indicative, None);

        assert!(!normalized.is_executable());
        assert!(!normalized.requires_last_look());
    }

    #[test]
    fn normalize_last_look_quote() {
        let normalizer = QuoteNormalizer::new();
        let quote = create_test_quote(100.0, 10.0, "test-venue");

        let normalized = normalizer.normalize(&quote, QuoteType::LastLook, None);

        assert!(normalized.is_executable());
        assert!(normalized.requires_last_look());
    }

    #[test]
    fn normalize_with_fx_conversion() {
        let mut normalizer = QuoteNormalizer::new();
        normalizer.add_fx_rate(FxRate::new("EUR", "USD", Decimal::new(110, 2))); // 1.10

        let quote = create_test_quote(100.0, 10.0, "eu-venue");

        let normalized = normalizer.normalize_with_config(
            &quote,
            &NormalizationConfig::default(),
            QuoteType::Firm,
            Some("EUR"),
        );

        // Price should be converted: 100 EUR * 1.10 = 110 USD
        assert_eq!(normalized.normalized_price().get(), Decimal::new(110, 0));
        assert_eq!(normalized.fx_rate_used(), Some(Decimal::new(110, 2)));
    }

    #[test]
    fn normalize_with_fees() {
        let normalizer = QuoteNormalizer::new();
        let config = NormalizationConfig::builder()
            .default_fee_rate(Decimal::new(1, 2)) // 1%
            .include_fees(true)
            .build();

        let quote = create_test_quote(100.0, 10.0, "test-venue");

        let normalized = normalizer.normalize_with_config(&quote, &config, QuoteType::Firm, None);

        // All-in price should include 1% fee: 100 + 1 = 101
        assert_eq!(normalized.all_in_price().get(), Decimal::new(101, 0));
        assert_eq!(normalized.fee_amount(), Decimal::ONE);
        assert_eq!(normalized.fee_rate(), Decimal::new(1, 2));
    }

    #[test]
    fn normalize_with_contract_multiplier() {
        let normalizer = QuoteNormalizer::new();
        let config = NormalizationConfig::builder()
            .contract_multiplier(Decimal::new(10, 0)) // 10x
            .build();

        let quote = create_test_quote(100.0, 5.0, "test-venue");

        let normalized = normalizer.normalize_with_config(&quote, &config, QuoteType::Firm, None);

        // Quantity should be multiplied: 5 * 10 = 50
        assert_eq!(normalized.normalized_quantity().get(), Decimal::new(50, 0));
    }

    #[test]
    fn normalize_with_latency_adjustment() {
        let normalizer = QuoteNormalizer::new();
        let config = NormalizationConfig::builder()
            .latency_adjustment_ms(100)
            .build();

        let quote = create_test_quote(100.0, 10.0, "test-venue");
        let original_ts = quote.created_at();

        let normalized = normalizer.normalize_with_config(&quote, &config, QuoteType::Firm, None);

        // Adjusted timestamp should be 100ms later
        assert_eq!(
            normalized.adjusted_timestamp().timestamp_millis(),
            original_ts.timestamp_millis() + 100
        );
    }

    #[test]
    fn normalize_batch() {
        let normalizer = QuoteNormalizer::new();
        let quotes = vec![
            create_test_quote(100.0, 10.0, "venue-1"),
            create_test_quote(95.0, 15.0, "venue-2"),
            create_test_quote(105.0, 8.0, "venue-3"),
        ];

        let quote_types = HashMap::new(); // All default to Firm

        let normalized = normalizer.normalize_batch(&quotes, &quote_types, None);

        assert_eq!(normalized.len(), 3);
        assert!(normalized.iter().all(|q| q.quote_type() == QuoteType::Firm));
    }

    #[test]
    fn filter_executable() {
        let normalizer = QuoteNormalizer::new();
        let quote = create_test_quote(100.0, 10.0, "test-venue");

        let firm = normalizer.normalize(&quote, QuoteType::Firm, None);
        let indicative = normalizer.normalize(&quote, QuoteType::Indicative, None);
        let last_look = normalizer.normalize(&quote, QuoteType::LastLook, None);

        let all = vec![firm, indicative, last_look];
        let executable = QuoteNormalizer::filter_executable(&all);

        assert_eq!(executable.len(), 2);
        assert!(executable.iter().all(|q| q.is_executable()));
    }

    #[test]
    fn sort_by_price_buy_side() {
        let normalizer = QuoteNormalizer::new();
        let quotes = vec![
            create_test_quote(100.0, 10.0, "venue-1"),
            create_test_quote(95.0, 10.0, "venue-2"),
            create_test_quote(105.0, 10.0, "venue-3"),
        ];

        let mut normalized: Vec<_> = quotes
            .iter()
            .map(|q| normalizer.normalize(q, QuoteType::Firm, None))
            .collect();

        let _ = QuoteNormalizer::sort_by_price(&mut normalized, true);

        // For buy side, lowest price first
        assert_eq!(normalized[0].normalized_price().get(), Decimal::new(95, 0));
        assert_eq!(normalized[1].normalized_price().get(), Decimal::new(100, 0));
        assert_eq!(normalized[2].normalized_price().get(), Decimal::new(105, 0));
    }

    #[test]
    fn sort_by_price_sell_side() {
        let normalizer = QuoteNormalizer::new();
        let quotes = vec![
            create_test_quote(100.0, 10.0, "venue-1"),
            create_test_quote(95.0, 10.0, "venue-2"),
            create_test_quote(105.0, 10.0, "venue-3"),
        ];

        let mut normalized: Vec<_> = quotes
            .iter()
            .map(|q| normalizer.normalize(q, QuoteType::Firm, None))
            .collect();

        let _ = QuoteNormalizer::sort_by_price(&mut normalized, false);

        // For sell side, highest price first
        assert_eq!(normalized[0].normalized_price().get(), Decimal::new(105, 0));
        assert_eq!(normalized[1].normalized_price().get(), Decimal::new(100, 0));
        assert_eq!(normalized[2].normalized_price().get(), Decimal::new(95, 0));
    }

    #[test]
    fn normalizer_display() {
        let normalizer = QuoteNormalizer::new();
        let display = normalizer.to_string();
        assert!(display.contains("QuoteNormalizer"));
        assert!(display.contains("configs=0"));
        assert!(display.contains("fx_rates=0"));
    }

    #[test]
    fn venue_specific_config() {
        let mut registry = NormalizationConfigRegistry::new();

        let binance_config = NormalizationConfig::builder()
            .venue_id(VenueId::new("binance"))
            .default_fee_rate(Decimal::new(1, 4)) // 0.01%
            .latency_adjustment_ms(10)
            .build();

        registry.register(&VenueId::new("binance"), binance_config);

        let normalizer = QuoteNormalizer::with_config_registry(registry);

        let binance_quote = create_test_quote(100.0, 10.0, "binance");
        let other_quote = create_test_quote(100.0, 10.0, "other");

        let normalized_binance = normalizer.normalize(&binance_quote, QuoteType::Firm, None);
        let normalized_other = normalizer.normalize(&other_quote, QuoteType::Firm, None);

        // Binance has lower fee rate
        assert!(normalized_binance.fee_rate() < normalized_other.fee_rate());
    }
}
