//! # Package Quote Validator
//!
//! Validates package quotes for consistency and pricing integrity.
//!
//! This service validates that package quotes have consistent leg prices
//! and that the net price aligns with individual leg prices within a
//! configurable tolerance.
//!
//! # Examples
//!
//! ```ignore
//! use otc_rfq::domain::services::package_quote_validator::PackageQuoteValidator;
//!
//! let validator = PackageQuoteValidator::new(50); // 50 basis points tolerance
//! let result = validator.validate(&package_quote, &market_prices);
//! ```

use crate::domain::entities::package_quote::PackageQuote;
use crate::domain::errors::{DomainError, DomainResult};
use crate::domain::value_objects::Price;
use rust_decimal::Decimal;

/// Default tolerance in basis points for price validation.
pub const DEFAULT_TOLERANCE_BPS: u32 = 50;

/// Validates package quotes for consistency and pricing integrity.
///
/// The validator checks that:
/// - Individual leg prices are consistent with market prices (within tolerance)
/// - The net price is consistent with the sum of leg prices
///
/// # Tolerance
///
/// Tolerance is specified in basis points (bps). 1 bps = 0.01%.
/// For example, 50 bps means a 0.5% tolerance.
#[derive(Debug, Clone)]
pub struct PackageQuoteValidator {
    /// Tolerance in basis points for price comparisons.
    tolerance_bps: u32,
}

impl PackageQuoteValidator {
    /// Creates a new validator with the specified tolerance.
    ///
    /// # Arguments
    ///
    /// * `tolerance_bps` - Tolerance in basis points (1 bps = 0.01%)
    #[must_use]
    pub fn new(tolerance_bps: u32) -> Self {
        Self { tolerance_bps }
    }

    /// Creates a new validator with the default tolerance (50 bps).
    #[must_use]
    pub fn with_default_tolerance() -> Self {
        Self::new(DEFAULT_TOLERANCE_BPS)
    }

    /// Returns the tolerance in basis points.
    #[must_use]
    pub fn tolerance_bps(&self) -> u32 {
        self.tolerance_bps
    }

    /// Validates a package quote against market prices.
    ///
    /// # Arguments
    ///
    /// * `quote` - The package quote to validate
    /// * `market_prices` - Market prices for each leg (in order)
    ///
    /// # Returns
    ///
    /// `Ok(())` if the quote is valid, or an error describing the issue.
    ///
    /// # Errors
    ///
    /// - `DomainError::InconsistentLegPrices` - A leg price deviates too much from market
    /// - `DomainError::InvalidPackageQuote` - Net price is inconsistent with legs
    pub fn validate(&self, quote: &PackageQuote, market_prices: &[Price]) -> DomainResult<()> {
        self.validate_leg_count(quote, market_prices)?;
        self.validate_leg_prices(quote, market_prices)?;
        self.validate_net_price_consistency(quote)?;
        Ok(())
    }

    /// Checks if a package quote has consistent pricing.
    ///
    /// This is a simpler check that only validates internal consistency
    /// without requiring market prices.
    ///
    /// # Arguments
    ///
    /// * `quote` - The package quote to check
    ///
    /// # Returns
    ///
    /// `true` if the quote is internally consistent.
    #[must_use]
    pub fn is_consistent(&self, quote: &PackageQuote) -> bool {
        self.validate_net_price_consistency(quote).is_ok()
    }

    /// Validates that the number of market prices matches the number of legs.
    fn validate_leg_count(
        &self,
        quote: &PackageQuote,
        market_prices: &[Price],
    ) -> DomainResult<()> {
        if quote.leg_prices().len() != market_prices.len() {
            return Err(DomainError::InvalidPackageQuote(format!(
                "market price count ({}) does not match leg count ({})",
                market_prices.len(),
                quote.leg_prices().len()
            )));
        }
        Ok(())
    }

    /// Validates each leg price against the corresponding market price.
    ///
    /// # Arguments
    ///
    /// * `quote` - The package quote to validate
    /// * `market_prices` - Market prices for each leg (in order)
    ///
    /// # Errors
    ///
    /// Returns `DomainError::InconsistentLegPrices` if any leg price deviates
    /// from the market price by more than the configured tolerance.
    pub fn validate_leg_prices(
        &self,
        quote: &PackageQuote,
        market_prices: &[Price],
    ) -> DomainResult<()> {
        for (index, (leg, market_price)) in quote
            .leg_prices()
            .iter()
            .zip(market_prices.iter())
            .enumerate()
        {
            let deviation_bps = self.calculate_deviation_bps(leg.price(), *market_price);

            if deviation_bps > Decimal::from(self.tolerance_bps) {
                return Err(DomainError::InconsistentLegPrices {
                    leg_index: index,
                    reason: format!(
                        "leg price {} deviates {}bps from market price {} (tolerance: {}bps)",
                        leg.price(),
                        deviation_bps,
                        market_price,
                        self.tolerance_bps
                    ),
                });
            }
        }
        Ok(())
    }

    /// Validates that the net price is consistent with the sum of leg prices.
    fn validate_net_price_consistency(&self, quote: &PackageQuote) -> DomainResult<()> {
        let calculated_net = match quote.calculated_net_from_legs() {
            Some(net) => net,
            None => {
                return Err(DomainError::InvalidPackageQuote(
                    "could not calculate net price from legs".to_string(),
                ));
            }
        };

        let quoted_net = quote.net_price();

        // Handle zero case to avoid division by zero
        if calculated_net == Decimal::ZERO {
            if quoted_net == Decimal::ZERO {
                return Ok(());
            } else {
                return Err(DomainError::InvalidPackageQuote(format!(
                    "net price {} is inconsistent with calculated net {} from legs",
                    quoted_net, calculated_net
                )));
            }
        }

        let deviation_bps = self.calculate_decimal_deviation_bps(quoted_net, calculated_net);

        if deviation_bps > Decimal::from(self.tolerance_bps) {
            return Err(DomainError::InvalidPackageQuote(format!(
                "net price {} deviates {}bps from calculated net {} (tolerance: {}bps)",
                quoted_net, deviation_bps, calculated_net, self.tolerance_bps
            )));
        }

        Ok(())
    }

    /// Calculates the deviation in basis points between two prices.
    fn calculate_deviation_bps(&self, price: Price, reference: Price) -> Decimal {
        let price_val = price.get();
        let ref_val = reference.get();

        if ref_val == Decimal::ZERO {
            if price_val == Decimal::ZERO {
                return Decimal::ZERO;
            }
            return Decimal::MAX; // Infinite deviation
        }

        let diff = (price_val - ref_val).abs();
        (diff / ref_val.abs()) * Decimal::from(10000) // Convert to basis points
    }

    /// Calculates the deviation in basis points between two decimal values.
    fn calculate_decimal_deviation_bps(&self, value: Decimal, reference: Decimal) -> Decimal {
        if reference == Decimal::ZERO {
            if value == Decimal::ZERO {
                return Decimal::ZERO;
            }
            return Decimal::MAX;
        }

        let diff = (value - reference).abs();
        (diff / reference.abs()) * Decimal::from(10000)
    }
}

impl Default for PackageQuoteValidator {
    fn default() -> Self {
        Self::with_default_tolerance()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::domain::entities::package_quote::LegPrice;
    use crate::domain::value_objects::strategy::{Strategy, StrategyLeg, StrategyType};
    use crate::domain::value_objects::timestamp::Timestamp;
    use crate::domain::value_objects::{
        AssetClass, Instrument, OrderSide, Quantity, RfqId, Symbol, VenueId,
    };

    fn test_instrument() -> Instrument {
        Instrument::builder(Symbol::new("BTC/USD").unwrap(), AssetClass::CryptoSpot).build()
    }

    fn test_strategy() -> Strategy {
        let inst = test_instrument();
        Strategy::new(
            StrategyType::Spread,
            vec![
                StrategyLeg::new(inst.clone(), OrderSide::Buy, 1).unwrap(),
                StrategyLeg::new(inst, OrderSide::Sell, 1).unwrap(),
            ],
            "BTC",
            None,
        )
        .unwrap()
    }

    fn create_package_quote(leg_prices: Vec<LegPrice>, net_price: Decimal) -> PackageQuote {
        PackageQuote::new(
            RfqId::new_v4(),
            VenueId::new("test-venue"),
            test_strategy(),
            net_price,
            leg_prices,
            Timestamp::now().add_secs(300),
        )
        .unwrap()
    }

    #[test]
    fn new_creates_validator_with_tolerance() {
        let validator = PackageQuoteValidator::new(100);
        assert_eq!(validator.tolerance_bps(), 100);
    }

    #[test]
    fn default_creates_validator_with_default_tolerance() {
        let validator = PackageQuoteValidator::default();
        assert_eq!(validator.tolerance_bps(), DEFAULT_TOLERANCE_BPS);
    }

    #[test]
    fn validate_passes_for_consistent_leg_prices() {
        let validator = PackageQuoteValidator::new(100); // 1% tolerance
        let inst = test_instrument();

        let leg_prices = vec![
            LegPrice::new(
                inst.clone(),
                OrderSide::Buy,
                Price::new(100.0).unwrap(),
                Quantity::new(1.0).unwrap(),
            )
            .unwrap(),
            LegPrice::new(
                inst,
                OrderSide::Sell,
                Price::new(101.0).unwrap(),
                Quantity::new(1.0).unwrap(),
            )
            .unwrap(),
        ];

        let quote = create_package_quote(leg_prices, Decimal::new(1, 0));

        let market_prices = vec![Price::new(100.0).unwrap(), Price::new(101.0).unwrap()];

        // validate_leg_prices should pass since prices match within tolerance
        let result = validator.validate_leg_prices(&quote, &market_prices);
        assert!(result.is_ok());

        // Full validate should now pass since signed_notional returns Decimal
        // and can handle negative values for buy legs
        // Net = -100 (buy) + 101 (sell) = 1 (matches the quote's net_price)
        let full_result = validator.validate(&quote, &market_prices);
        assert!(full_result.is_ok());
    }

    #[test]
    fn validate_fails_for_mismatched_market_price_count() {
        let validator = PackageQuoteValidator::new(100);
        let inst = test_instrument();

        let leg_prices = vec![
            LegPrice::new(
                inst.clone(),
                OrderSide::Buy,
                Price::new(100.0).unwrap(),
                Quantity::new(1.0).unwrap(),
            )
            .unwrap(),
            LegPrice::new(
                inst,
                OrderSide::Sell,
                Price::new(101.0).unwrap(),
                Quantity::new(1.0).unwrap(),
            )
            .unwrap(),
        ];

        let quote = create_package_quote(leg_prices, Decimal::new(1, 0));

        // Only one market price for two legs
        let market_prices = vec![Price::new(100.0).unwrap()];

        let result = validator.validate(&quote, &market_prices);
        assert!(result.is_err());
    }

    #[test]
    fn validate_fails_for_leg_price_exceeding_tolerance() {
        let validator = PackageQuoteValidator::new(50); // 0.5% tolerance
        let inst = test_instrument();

        let leg_prices = vec![
            LegPrice::new(
                inst.clone(),
                OrderSide::Buy,
                Price::new(100.0).unwrap(),
                Quantity::new(1.0).unwrap(),
            )
            .unwrap(),
            LegPrice::new(
                inst,
                OrderSide::Sell,
                Price::new(101.0).unwrap(),
                Quantity::new(1.0).unwrap(),
            )
            .unwrap(),
        ];

        let quote = create_package_quote(leg_prices, Decimal::new(1, 0));

        // Market price differs by more than 0.5%
        let market_prices = vec![
            Price::new(99.0).unwrap(), // 1% deviation
            Price::new(101.0).unwrap(),
        ];

        let result = validator.validate(&quote, &market_prices);
        assert!(matches!(
            result,
            Err(DomainError::InconsistentLegPrices { .. })
        ));
    }

    #[test]
    fn is_consistent_returns_true_for_valid_quote() {
        let validator = PackageQuoteValidator::new(100);
        let inst = test_instrument();

        let leg_prices = vec![
            LegPrice::new(
                inst.clone(),
                OrderSide::Buy,
                Price::new(100.0).unwrap(),
                Quantity::new(1.0).unwrap(),
            )
            .unwrap(),
            LegPrice::new(
                inst,
                OrderSide::Sell,
                Price::new(101.0).unwrap(),
                Quantity::new(1.0).unwrap(),
            )
            .unwrap(),
        ];

        // Net = -100 (buy) + 101 (sell) = 1
        let quote = create_package_quote(leg_prices, Decimal::new(1, 0));

        // is_consistent should return true since signed_notional now returns Decimal
        // and can handle negative values for buy legs
        let result = validator.is_consistent(&quote);
        assert!(result);
    }

    #[test]
    fn calculate_deviation_bps_works_correctly() {
        let validator = PackageQuoteValidator::new(50);

        // 1% deviation = 100 bps
        let deviation = validator
            .calculate_deviation_bps(Price::new(101.0).unwrap(), Price::new(100.0).unwrap());
        assert_eq!(deviation, Decimal::from(100));

        // 0.5% deviation = 50 bps
        let deviation = validator
            .calculate_deviation_bps(Price::new(100.5).unwrap(), Price::new(100.0).unwrap());
        assert_eq!(deviation, Decimal::from(50));
    }

    #[test]
    fn calculate_deviation_bps_handles_zero_reference() {
        let validator = PackageQuoteValidator::new(50);

        let deviation =
            validator.calculate_deviation_bps(Price::new(100.0).unwrap(), Price::zero());
        assert_eq!(deviation, Decimal::MAX);

        let deviation = validator.calculate_deviation_bps(Price::zero(), Price::zero());
        assert_eq!(deviation, Decimal::ZERO);
    }
}
