//! # Price Discovery Service
//!
//! Orchestrates price discovery method selection based on instrument liquidity.

use crate::domain::errors::DomainError;
use crate::domain::events::{PriceDiscoveryMethodSelected, TheoreticalPriceComputed};
use crate::domain::services::TheoreticalPricer;
use crate::domain::value_objects::{
    Instrument, Price, PriceDiscoveryMethod, RfqId, TheoreticalPrice,
};
use rust_decimal::prelude::ToPrimitive;

/// Configuration for price discovery service.
#[derive(Debug, Clone)]
pub struct PriceDiscoveryConfig {
    /// Minimum CLOB depth to consider liquid (in base currency units).
    pub min_clob_depth: f64,
    /// Maximum bid-ask spread ratio to consider liquid (e.g., 0.01 = 1%).
    pub max_spread_ratio: f64,
    /// Minimum recent trade volume to consider liquid.
    pub min_recent_volume: f64,
    /// Spread widening multiplier for theoretical prices (e.g., 1.5 = 50% wider).
    pub theoretical_spread_multiplier: f64,
}

impl Default for PriceDiscoveryConfig {
    fn default() -> Self {
        Self {
            min_clob_depth: 10_000.0,
            max_spread_ratio: 0.02, // 2%
            min_recent_volume: 5_000.0,
            theoretical_spread_multiplier: 1.5,
        }
    }
}

/// Liquidity metrics for an instrument.
#[derive(Debug, Clone)]
pub struct LiquidityMetrics {
    /// Total depth on the order book (bid + ask).
    pub clob_depth: f64,
    /// Bid-ask spread ratio (spread / mid_price).
    pub spread_ratio: f64,
    /// Recent trading volume (last 24h).
    pub recent_volume: f64,
    /// Number of active market makers.
    pub active_mm_count: usize,
}

impl LiquidityMetrics {
    /// Creates new liquidity metrics.
    #[must_use]
    pub fn new(
        clob_depth: f64,
        spread_ratio: f64,
        recent_volume: f64,
        active_mm_count: usize,
    ) -> Self {
        Self {
            clob_depth,
            spread_ratio,
            recent_volume,
            active_mm_count,
        }
    }

    /// Returns whether the instrument is considered liquid.
    #[must_use]
    pub fn is_liquid(&self, config: &PriceDiscoveryConfig) -> bool {
        self.clob_depth >= config.min_clob_depth
            && self.spread_ratio <= config.max_spread_ratio
            && self.recent_volume >= config.min_recent_volume
    }
}

/// Price discovery service for illiquid instruments.
#[derive(Debug, Clone)]
pub struct PriceDiscoveryService {
    config: PriceDiscoveryConfig,
    theoretical_pricer: TheoreticalPricer,
}

impl PriceDiscoveryService {
    /// Creates a new price discovery service.
    #[must_use]
    pub fn new(config: PriceDiscoveryConfig) -> Self {
        Self {
            config,
            theoretical_pricer: TheoreticalPricer::new(),
        }
    }

    /// Selects the appropriate price discovery method based on liquidity.
    ///
    /// # Arguments
    ///
    /// * `rfq_id` - RFQ ID for event emission
    /// * `instrument` - Instrument to price
    /// * `metrics` - Current liquidity metrics
    ///
    /// # Returns
    ///
    /// Selected method and reason for selection.
    #[must_use]
    pub fn select_method(
        &self,
        rfq_id: RfqId,
        instrument: &Instrument,
        metrics: &LiquidityMetrics,
    ) -> (PriceDiscoveryMethod, PriceDiscoveryMethodSelected) {
        let (method, reason) = if metrics.is_liquid(&self.config) {
            (PriceDiscoveryMethod::Clob, "Sufficient CLOB liquidity")
        } else if metrics.active_mm_count >= 3 {
            (
                PriceDiscoveryMethod::Indicative,
                "Multiple MMs available for indicative quotes",
            )
        } else if metrics.active_mm_count > 0 {
            (
                PriceDiscoveryMethod::InterestGathering,
                "Limited MMs - gather interest first",
            )
        } else {
            (
                PriceDiscoveryMethod::Theoretical,
                "No CLOB liquidity or active MMs - use theoretical pricing",
            )
        };

        let event = PriceDiscoveryMethodSelected::new(
            rfq_id,
            instrument.clone(),
            method,
            reason.to_string(),
        );

        (method, event)
    }

    /// Computes a theoretical price for an illiquid instrument.
    ///
    /// # Arguments
    ///
    /// * `rfq_id` - RFQ ID for event emission
    /// * `instrument` - Instrument to price
    /// * `underlying_price` - Current underlying price
    /// * `strike` - Strike price
    /// * `time_to_expiry` - Time to expiry in years
    /// * `risk_free_rate` - Risk-free rate
    /// * `nearby_ivs` - Reference IVs from nearby strikes
    /// * `is_call` - True for call, false for put
    ///
    /// # Returns
    ///
    /// Theoretical price and event.
    pub fn compute_theoretical_price(
        &self,
        rfq_id: RfqId,
        instrument: &Instrument,
        underlying_price: Price,
        strike: Price,
        time_to_expiry: f64,
        risk_free_rate: f64,
        nearby_ivs: &[(Price, f64)],
        is_call: bool,
    ) -> Result<(TheoreticalPrice, TheoreticalPriceComputed), DomainError> {
        let theoretical_price = self.theoretical_pricer.compute(
            underlying_price,
            strike,
            time_to_expiry,
            risk_free_rate,
            nearby_ivs,
            is_call,
        )?;

        let event = TheoreticalPriceComputed::new(
            rfq_id,
            instrument.clone(),
            theoretical_price.clone(),
            nearby_ivs.len(),
        );

        Ok((theoretical_price, event))
    }

    /// Applies spread widening to a theoretical price based on confidence.
    ///
    /// Lower confidence = wider spread.
    ///
    /// # Arguments
    ///
    /// * `base_price` - Base theoretical price
    /// * `confidence` - Confidence score (0.0-1.0)
    ///
    /// # Returns
    ///
    /// (bid_price, ask_price) with widened spread.
    pub fn apply_spread_widening(
        &self,
        base_price: Price,
        confidence: f64,
    ) -> Result<(Price, Price), DomainError> {
        // Base spread from config
        let base_spread_ratio = self.config.theoretical_spread_multiplier * 0.01; // 1.5% default

        // Widen spread inversely to confidence
        // Low confidence (0.1) = 10x wider, high confidence (0.9) = ~1.1x wider
        let confidence_multiplier = 1.0 / confidence.max(0.1);
        let total_spread_ratio = base_spread_ratio * confidence_multiplier;

        let base_value = base_price.get().to_f64().ok_or_else(|| {
            DomainError::ValidationError("Failed to convert price to f64".to_string())
        })?;

        let half_spread = base_value * total_spread_ratio / 2.0;
        let bid_value = base_value - half_spread;
        let ask_value = base_value + half_spread;

        let bid = Price::new(bid_value.max(0.0))
            .map_err(|e| DomainError::ValidationError(format!("Invalid bid price: {}", e)))?;
        let ask = Price::new(ask_value)
            .map_err(|e| DomainError::ValidationError(format!("Invalid ask price: {}", e)))?;

        Ok((bid, ask))
    }

    /// Returns the configuration.
    #[must_use]
    pub fn config(&self) -> &PriceDiscoveryConfig {
        &self.config
    }
}

impl Default for PriceDiscoveryService {
    fn default() -> Self {
        Self::new(PriceDiscoveryConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::value_objects::{AssetClass, SettlementMethod, Symbol};
    use rust_decimal::prelude::ToPrimitive;
    use uuid::Uuid;

    fn create_test_instrument() -> Instrument {
        let symbol = Symbol::new("BTC/USD").unwrap();
        Instrument::new(symbol, AssetClass::CryptoDerivs, SettlementMethod::OffChain)
    }

    #[test]
    fn select_method_liquid_instrument() {
        let service = PriceDiscoveryService::default();
        let rfq_id = RfqId::new(Uuid::new_v4());
        let instrument = create_test_instrument();

        let metrics = LiquidityMetrics::new(
            15_000.0, // Good depth
            0.01,     // 1% spread
            10_000.0, // Good volume
            5,        // 5 MMs
        );

        let (method, event) = service.select_method(rfq_id, &instrument, &metrics);

        assert_eq!(method, PriceDiscoveryMethod::Clob);
        assert_eq!(event.method(), PriceDiscoveryMethod::Clob);
        assert!(event.reason().contains("CLOB"));
    }

    #[test]
    fn select_method_illiquid_with_mms() {
        let service = PriceDiscoveryService::default();
        let rfq_id = RfqId::new(Uuid::new_v4());
        let instrument = create_test_instrument();

        let metrics = LiquidityMetrics::new(
            1_000.0, // Low depth
            0.05,    // Wide spread
            500.0,   // Low volume
            3,       // But 3 MMs available
        );

        let (method, _) = service.select_method(rfq_id, &instrument, &metrics);

        assert_eq!(method, PriceDiscoveryMethod::Indicative);
    }

    #[test]
    fn select_method_illiquid_few_mms() {
        let service = PriceDiscoveryService::default();
        let rfq_id = RfqId::new(Uuid::new_v4());
        let instrument = create_test_instrument();

        let metrics = LiquidityMetrics::new(
            1_000.0, // Low depth
            0.05,    // Wide spread
            500.0,   // Low volume
            1,       // Only 1 MM
        );

        let (method, _) = service.select_method(rfq_id, &instrument, &metrics);

        assert_eq!(method, PriceDiscoveryMethod::InterestGathering);
    }

    #[test]
    fn select_method_no_liquidity() {
        let service = PriceDiscoveryService::default();
        let rfq_id = RfqId::new(Uuid::new_v4());
        let instrument = create_test_instrument();

        let metrics = LiquidityMetrics::new(
            0.0, // No depth
            1.0, // Very wide spread
            0.0, // No volume
            0,   // No MMs
        );

        let (method, event) = service.select_method(rfq_id, &instrument, &metrics);

        assert_eq!(method, PriceDiscoveryMethod::Theoretical);
        assert!(event.reason().contains("theoretical"));
    }

    #[test]
    fn compute_theoretical_price_success() {
        let service = PriceDiscoveryService::default();
        let rfq_id = RfqId::new(Uuid::new_v4());
        let instrument = create_test_instrument();

        let underlying = Price::new(100.0).unwrap();
        let strike = Price::new(105.0).unwrap();
        let nearby_ivs = vec![
            (Price::new(100.0).unwrap(), 0.20),
            (Price::new(110.0).unwrap(), 0.25),
        ];

        let result = service.compute_theoretical_price(
            rfq_id,
            &instrument,
            underlying,
            strike,
            1.0,
            0.05,
            &nearby_ivs,
            true,
        );

        assert!(result.is_ok());
        let (price, event) = result.unwrap();
        assert!(price.price().get() > rust_decimal::Decimal::ZERO);
        assert_eq!(event.reference_count(), 2);
    }

    #[test]
    fn apply_spread_widening_high_confidence() {
        let service = PriceDiscoveryService::default();
        let base_price = Price::new(100.0).unwrap();

        let (bid, ask) = service.apply_spread_widening(base_price, 0.9).unwrap();

        // High confidence should have narrow spread
        let bid_f64 = bid.get().to_f64().unwrap();
        let ask_f64 = ask.get().to_f64().unwrap();
        let spread = ask_f64 - bid_f64;

        assert!(spread < 5.0); // Less than 5% spread
        assert!(bid_f64 < 100.0);
        assert!(ask_f64 > 100.0);
    }

    #[test]
    fn apply_spread_widening_low_confidence() {
        let service = PriceDiscoveryService::default();
        let base_price = Price::new(100.0).unwrap();

        let (bid, ask) = service.apply_spread_widening(base_price, 0.1).unwrap();

        // Low confidence should have wide spread
        let bid_f64 = bid.get().to_f64().unwrap();
        let ask_f64 = ask.get().to_f64().unwrap();
        let spread = ask_f64 - bid_f64;

        assert!(spread > 10.0); // More than 10% spread
        assert!(bid_f64 < 100.0);
        assert!(ask_f64 > 100.0);
    }

    #[test]
    fn liquidity_metrics_is_liquid() {
        let config = PriceDiscoveryConfig::default();

        let liquid = LiquidityMetrics::new(15_000.0, 0.01, 10_000.0, 5);
        assert!(liquid.is_liquid(&config));

        let illiquid_depth = LiquidityMetrics::new(5_000.0, 0.01, 10_000.0, 5);
        assert!(!illiquid_depth.is_liquid(&config));

        let illiquid_spread = LiquidityMetrics::new(15_000.0, 0.05, 10_000.0, 5);
        assert!(!illiquid_spread.is_liquid(&config));

        let illiquid_volume = LiquidityMetrics::new(15_000.0, 0.01, 1_000.0, 5);
        assert!(!illiquid_volume.is_liquid(&config));
    }
}
