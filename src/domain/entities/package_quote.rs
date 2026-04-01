//! # Package Quote Entity
//!
//! Represents a package quote for multi-leg strategies.
//!
//! This module provides the [`PackageQuote`] entity representing a quote
//! for an entire multi-leg strategy with a single net price, and [`LegPrice`]
//! representing the price for each individual leg.
//!
//! # Examples
//!
//! ```ignore
//! use otc_rfq::domain::entities::package_quote::{PackageQuote, PackageQuoteBuilder, LegPrice};
//! use otc_rfq::domain::value_objects::{
//!     RfqId, VenueId, Price, Quantity, Timestamp, Instrument, OrderSide,
//! };
//!
//! let leg_price = LegPrice::new(instrument, OrderSide::Buy, price, quantity);
//! let package_quote = PackageQuoteBuilder::new(
//!     rfq_id,
//!     venue_id,
//!     strategy,
//!     net_price,
//!     Timestamp::now().add_secs(300),
//! )
//! .leg_price(leg_price)
//! .build();
//! ```

use crate::domain::errors::{DomainError, DomainResult};
use crate::domain::value_objects::strategy::Strategy;
use crate::domain::value_objects::timestamp::Timestamp;
use crate::domain::value_objects::{
    Instrument, OrderSide, PackageQuoteId, Price, Quantity, RfqId, VenueId,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;

// ============================================================================
// LegPrice
// ============================================================================

/// Price for a single leg of a multi-leg strategy.
///
/// Represents the price, quantity, and direction for one leg within a package quote.
///
/// # Invariants
///
/// - Price must be positive
/// - Quantity must be positive
///
/// # Examples
///
/// ```ignore
/// use otc_rfq::domain::entities::package_quote::LegPrice;
/// use otc_rfq::domain::value_objects::{Instrument, OrderSide, Price, Quantity};
///
/// let leg = LegPrice::new(
///     instrument,
///     OrderSide::Buy,
///     Price::new(50000.0).unwrap(),
///     Quantity::new(1.0).unwrap(),
/// ).unwrap();
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LegPrice {
    /// The instrument for this leg.
    instrument: Instrument,
    /// Buy or sell direction.
    side: OrderSide,
    /// The price for this leg.
    price: Price,
    /// The quantity for this leg.
    quantity: Quantity,
}

impl LegPrice {
    /// Creates a new leg price with validation.
    ///
    /// # Arguments
    ///
    /// * `instrument` - The instrument for this leg
    /// * `side` - Buy or sell direction
    /// * `price` - The price (must be positive)
    /// * `quantity` - The quantity (must be positive)
    ///
    /// # Errors
    ///
    /// Returns `DomainError::InvalidPrice` if price is not positive.
    /// Returns `DomainError::InvalidQuantity` if quantity is not positive.
    pub fn new(
        instrument: Instrument,
        side: OrderSide,
        price: Price,
        quantity: Quantity,
    ) -> DomainResult<Self> {
        if !price.is_positive() {
            return Err(DomainError::InvalidPrice(
                "leg price must be positive".to_string(),
            ));
        }
        if !quantity.is_positive() {
            return Err(DomainError::InvalidQuantity(
                "leg quantity must be positive".to_string(),
            ));
        }
        Ok(Self {
            instrument,
            side,
            price,
            quantity,
        })
    }

    /// Creates a leg price from parts (for reconstruction from storage).
    ///
    /// # Safety
    ///
    /// This method bypasses validation and should only be used when
    /// reconstructing from trusted storage.
    #[must_use]
    pub fn from_parts(
        instrument: Instrument,
        side: OrderSide,
        price: Price,
        quantity: Quantity,
    ) -> Self {
        Self {
            instrument,
            side,
            price,
            quantity,
        }
    }

    /// Returns the instrument for this leg.
    #[inline]
    #[must_use]
    pub fn instrument(&self) -> &Instrument {
        &self.instrument
    }

    /// Returns the buy/sell direction.
    #[inline]
    #[must_use]
    pub fn side(&self) -> OrderSide {
        self.side
    }

    /// Returns the price for this leg.
    #[inline]
    #[must_use]
    pub fn price(&self) -> Price {
        self.price
    }

    /// Returns the quantity for this leg.
    #[inline]
    #[must_use]
    pub fn quantity(&self) -> Quantity {
        self.quantity
    }

    /// Calculates the notional value for this leg (price * quantity).
    #[must_use]
    pub fn notional(&self) -> Option<Price> {
        self.price.safe_mul(self.quantity.get()).ok()
    }

    /// Returns the signed notional value based on side.
    ///
    /// - Buy: negative (cost to buyer)
    /// - Sell: positive (credit to buyer)
    ///
    /// Returns `Decimal` instead of `Price` because signed notional can be negative.
    #[must_use]
    pub fn signed_notional(&self) -> Option<Decimal> {
        let notional = self.notional()?;
        let notional_value = notional.get();
        Some(match self.side {
            OrderSide::Buy => -notional_value,
            OrderSide::Sell => notional_value,
        })
    }
}

impl fmt::Display for LegPrice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {} {} @ {}",
            self.side,
            self.quantity,
            self.instrument.symbol(),
            self.price
        )
    }
}

// ============================================================================
// PackageQuote
// ============================================================================

/// A package quote for a multi-leg strategy.
///
/// Represents a quote for an entire strategy with a single net price,
/// along with individual leg prices for transparency.
///
/// # Net Price Convention
///
/// - Positive net price = debit (buyer pays)
/// - Negative net price = credit (buyer receives)
///
/// # Invariants
///
/// - Must have at least one leg price
/// - Leg count must match strategy leg count
/// - `valid_until` must be in the future when created
///
/// # Examples
///
/// ```ignore
/// use otc_rfq::domain::entities::package_quote::{PackageQuote, PackageQuoteBuilder};
///
/// let quote = PackageQuoteBuilder::new(
///     rfq_id,
///     venue_id,
///     strategy,
///     net_price,
///     valid_until,
/// )
/// .leg_prices(leg_prices)
/// .build()?;
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageQuote {
    /// Unique identifier for this package quote.
    id: PackageQuoteId,
    /// The RFQ this quote is responding to.
    rfq_id: RfqId,
    /// The venue that provided this quote.
    venue_id: VenueId,
    /// The strategy being quoted.
    strategy: Strategy,
    /// Net price for the entire package (positive = debit, negative = credit).
    /// Uses Decimal directly to allow negative values for credit strategies.
    net_price: Decimal,
    /// Individual leg prices.
    leg_prices: Vec<LegPrice>,
    /// When this quote expires.
    valid_until: Timestamp,
    /// When this quote was created.
    created_at: Timestamp,
}

impl PackageQuote {
    /// Creates a new package quote with validation.
    ///
    /// # Arguments
    ///
    /// * `rfq_id` - The RFQ this quote responds to
    /// * `venue_id` - The venue providing the quote
    /// * `strategy` - The strategy being quoted
    /// * `net_price` - Net price for the package (positive = debit, negative = credit)
    /// * `leg_prices` - Individual leg prices
    /// * `valid_until` - When the quote expires
    ///
    /// # Errors
    ///
    /// Returns `DomainError::InvalidPackageQuote` if:
    /// - `leg_prices` is empty
    /// - Leg count doesn't match strategy leg count
    /// - `valid_until` is in the past
    pub fn new(
        rfq_id: RfqId,
        venue_id: VenueId,
        strategy: Strategy,
        net_price: Decimal,
        leg_prices: Vec<LegPrice>,
        valid_until: Timestamp,
    ) -> DomainResult<Self> {
        Self::validate_legs(&leg_prices, &strategy)?;
        Self::validate_expiry(&valid_until)?;

        Ok(Self {
            id: PackageQuoteId::new_v4(),
            rfq_id,
            venue_id,
            strategy,
            net_price,
            leg_prices,
            valid_until,
            created_at: Timestamp::now(),
        })
    }

    /// Creates a package quote from parts (for reconstruction from storage).
    ///
    /// # Safety
    ///
    /// This method bypasses validation and should only be used when
    /// reconstructing from trusted storage.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn from_parts(
        id: PackageQuoteId,
        rfq_id: RfqId,
        venue_id: VenueId,
        strategy: Strategy,
        net_price: Decimal,
        leg_prices: Vec<LegPrice>,
        valid_until: Timestamp,
        created_at: Timestamp,
    ) -> Self {
        Self {
            id,
            rfq_id,
            venue_id,
            strategy,
            net_price,
            leg_prices,
            valid_until,
            created_at,
        }
    }

    /// Returns a builder for constructing a package quote.
    #[must_use]
    pub fn builder(
        rfq_id: RfqId,
        venue_id: VenueId,
        strategy: Strategy,
        net_price: Decimal,
        valid_until: Timestamp,
    ) -> PackageQuoteBuilder {
        PackageQuoteBuilder::new(rfq_id, venue_id, strategy, net_price, valid_until)
    }

    fn validate_legs(leg_prices: &[LegPrice], strategy: &Strategy) -> DomainResult<()> {
        if leg_prices.is_empty() {
            return Err(DomainError::InvalidPackageQuote(
                "package quote must have at least one leg price".to_string(),
            ));
        }

        if leg_prices.len() != strategy.leg_count() {
            return Err(DomainError::InvalidPackageQuote(format!(
                "leg price count ({}) does not match strategy leg count ({})",
                leg_prices.len(),
                strategy.leg_count()
            )));
        }

        Ok(())
    }

    fn validate_expiry(valid_until: &Timestamp) -> DomainResult<()> {
        if valid_until.is_expired() {
            return Err(DomainError::InvalidPackageQuote(
                "valid_until must be in the future".to_string(),
            ));
        }
        Ok(())
    }

    /// Returns the package quote ID.
    #[inline]
    #[must_use]
    pub fn id(&self) -> PackageQuoteId {
        self.id
    }

    /// Returns the RFQ ID this quote responds to.
    #[inline]
    #[must_use]
    pub fn rfq_id(&self) -> RfqId {
        self.rfq_id
    }

    /// Returns the venue ID.
    #[inline]
    #[must_use]
    pub fn venue_id(&self) -> &VenueId {
        &self.venue_id
    }

    /// Returns the strategy being quoted.
    #[inline]
    #[must_use]
    pub fn strategy(&self) -> &Strategy {
        &self.strategy
    }

    /// Returns the net price for the package.
    ///
    /// - Positive = debit (buyer pays)
    /// - Negative = credit (buyer receives)
    #[inline]
    #[must_use]
    pub fn net_price(&self) -> Decimal {
        self.net_price
    }

    /// Returns the individual leg prices.
    #[inline]
    #[must_use]
    pub fn leg_prices(&self) -> &[LegPrice] {
        &self.leg_prices
    }

    /// Returns when this quote expires.
    #[inline]
    #[must_use]
    pub fn valid_until(&self) -> Timestamp {
        self.valid_until
    }

    /// Returns when this quote was created.
    #[inline]
    #[must_use]
    pub fn created_at(&self) -> Timestamp {
        self.created_at
    }

    /// Returns true if this is a debit strategy (buyer pays net).
    #[must_use]
    pub fn is_debit(&self) -> bool {
        self.net_price > Decimal::ZERO
    }

    /// Returns true if this is a credit strategy (buyer receives net).
    #[must_use]
    pub fn is_credit(&self) -> bool {
        self.net_price < Decimal::ZERO
    }

    /// Returns true if this quote has expired.
    #[must_use]
    pub fn is_expired(&self) -> bool {
        self.valid_until.is_expired()
    }

    /// Returns the time remaining until expiry.
    ///
    /// Returns `Duration::ZERO` if already expired.
    #[must_use]
    pub fn time_to_expiry(&self) -> std::time::Duration {
        Timestamp::now().duration_until(&self.valid_until)
    }

    /// Returns the total quantity across all legs.
    #[must_use]
    pub fn total_quantity(&self) -> Quantity {
        self.leg_prices.iter().fold(Quantity::zero(), |acc, leg| {
            acc.safe_add(leg.quantity()).unwrap_or(acc)
        })
    }

    /// Calculates the sum of individual leg notionals.
    ///
    /// This can be compared with `net_price` to detect pricing discrepancies.
    /// Returns `Decimal` instead of `Price` because the sum can be negative
    /// (credit strategies).
    #[must_use]
    pub fn calculated_net_from_legs(&self) -> Option<Decimal> {
        let mut total = Decimal::ZERO;
        for leg in &self.leg_prices {
            let signed = leg.signed_notional()?;
            total += signed;
        }
        Some(total)
    }
}

impl fmt::Display for PackageQuote {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let direction = if self.is_debit() { "debit" } else { "credit" };
        write!(
            f,
            "PackageQuote({} legs, net {} {} from {})",
            self.leg_prices.len(),
            self.net_price,
            direction,
            self.venue_id
        )
    }
}

// ============================================================================
// PackageQuoteBuilder
// ============================================================================

/// Builder for constructing [`PackageQuote`] instances.
///
/// # Examples
///
/// ```ignore
/// let quote = PackageQuoteBuilder::new(rfq_id, venue_id, strategy, net_price, valid_until)
///     .leg_prices(leg_prices)
///     .build()?;
/// ```
#[derive(Debug, Clone)]
pub struct PackageQuoteBuilder {
    rfq_id: RfqId,
    venue_id: VenueId,
    strategy: Strategy,
    net_price: Decimal,
    leg_prices: Vec<LegPrice>,
    valid_until: Timestamp,
}

impl PackageQuoteBuilder {
    /// Creates a new package quote builder.
    #[must_use]
    pub fn new(
        rfq_id: RfqId,
        venue_id: VenueId,
        strategy: Strategy,
        net_price: Decimal,
        valid_until: Timestamp,
    ) -> Self {
        Self {
            rfq_id,
            venue_id,
            strategy,
            net_price,
            leg_prices: Vec::new(),
            valid_until,
        }
    }

    /// Adds a single leg price.
    #[must_use]
    pub fn leg_price(mut self, leg: LegPrice) -> Self {
        self.leg_prices.push(leg);
        self
    }

    /// Sets all leg prices at once.
    #[must_use]
    pub fn leg_prices(mut self, legs: Vec<LegPrice>) -> Self {
        self.leg_prices = legs;
        self
    }

    /// Builds the package quote with validation.
    ///
    /// # Errors
    ///
    /// Returns `DomainError::InvalidPackageQuote` if validation fails.
    pub fn build(self) -> DomainResult<PackageQuote> {
        PackageQuote::new(
            self.rfq_id,
            self.venue_id,
            self.strategy,
            self.net_price,
            self.leg_prices,
            self.valid_until,
        )
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::domain::value_objects::strategy::{StrategyLeg, StrategyType};
    use crate::domain::value_objects::{AssetClass, Symbol};

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

    fn test_leg_prices() -> Vec<LegPrice> {
        let inst = test_instrument();
        vec![
            LegPrice::new(
                inst.clone(),
                OrderSide::Buy,
                Price::new(50000.0).unwrap(),
                Quantity::new(1.0).unwrap(),
            )
            .unwrap(),
            LegPrice::new(
                inst,
                OrderSide::Sell,
                Price::new(51000.0).unwrap(),
                Quantity::new(1.0).unwrap(),
            )
            .unwrap(),
        ]
    }

    mod leg_price {
        use super::*;

        #[test]
        fn new_creates_valid_leg() {
            let inst = test_instrument();
            let leg = LegPrice::new(
                inst,
                OrderSide::Buy,
                Price::new(50000.0).unwrap(),
                Quantity::new(1.0).unwrap(),
            );
            assert!(leg.is_ok());
        }

        #[test]
        fn new_fails_for_zero_price() {
            let inst = test_instrument();
            let leg = LegPrice::new(
                inst,
                OrderSide::Buy,
                Price::zero(),
                Quantity::new(1.0).unwrap(),
            );
            assert!(leg.is_err());
        }

        #[test]
        fn new_fails_for_zero_quantity() {
            let inst = test_instrument();
            let leg = LegPrice::new(
                inst,
                OrderSide::Buy,
                Price::new(50000.0).unwrap(),
                Quantity::zero(),
            );
            assert!(leg.is_err());
        }

        #[test]
        fn notional_calculates_correctly() {
            let inst = test_instrument();
            let leg = LegPrice::new(
                inst,
                OrderSide::Buy,
                Price::new(100.0).unwrap(),
                Quantity::new(2.0).unwrap(),
            )
            .unwrap();
            assert_eq!(leg.notional(), Some(Price::new(200.0).unwrap()));
        }

        #[test]
        fn signed_notional_negative_for_buy() {
            let inst = test_instrument();
            let leg = LegPrice::new(
                inst,
                OrderSide::Buy,
                Price::new(100.0).unwrap(),
                Quantity::new(1.0).unwrap(),
            )
            .unwrap();
            let signed = leg.signed_notional().unwrap();
            // signed_notional returns Decimal, which can be negative for buy side
            assert!(signed.is_sign_negative());
            assert_eq!(signed, Decimal::new(-100, 0));
        }

        #[test]
        fn signed_notional_positive_for_sell() {
            let inst = test_instrument();
            let leg = LegPrice::new(
                inst,
                OrderSide::Sell,
                Price::new(100.0).unwrap(),
                Quantity::new(1.0).unwrap(),
            )
            .unwrap();
            let signed = leg.signed_notional().unwrap();
            assert!(signed.is_sign_positive());
            assert_eq!(signed, Decimal::new(100, 0));
        }

        #[test]
        fn display_formats_correctly() {
            let inst = test_instrument();
            let leg = LegPrice::new(
                inst,
                OrderSide::Buy,
                Price::new(50000.0).unwrap(),
                Quantity::new(1.0).unwrap(),
            )
            .unwrap();
            let display = leg.to_string();
            assert!(display.contains("BUY") || display.contains("Buy"));
            assert!(display.contains("BTC/USD"));
        }
    }

    mod package_quote {
        use super::*;

        #[test]
        fn new_creates_valid_quote() {
            let strategy = test_strategy();
            let leg_prices = test_leg_prices();
            let quote = PackageQuote::new(
                RfqId::new_v4(),
                VenueId::new("test-venue"),
                strategy,
                Decimal::new(1000, 0),
                leg_prices,
                Timestamp::now().add_secs(300),
            );
            assert!(quote.is_ok());
        }

        #[test]
        fn new_fails_for_empty_legs() {
            let strategy = test_strategy();
            let quote = PackageQuote::new(
                RfqId::new_v4(),
                VenueId::new("test-venue"),
                strategy,
                Decimal::new(1000, 0),
                vec![],
                Timestamp::now().add_secs(300),
            );
            assert!(quote.is_err());
        }

        #[test]
        fn new_fails_for_mismatched_leg_count() {
            let strategy = test_strategy(); // 2 legs
            let inst = test_instrument();
            let leg_prices = vec![
                LegPrice::new(
                    inst,
                    OrderSide::Buy,
                    Price::new(50000.0).unwrap(),
                    Quantity::new(1.0).unwrap(),
                )
                .unwrap(),
            ]; // Only 1 leg

            let quote = PackageQuote::new(
                RfqId::new_v4(),
                VenueId::new("test-venue"),
                strategy,
                Decimal::new(1000, 0),
                leg_prices,
                Timestamp::now().add_secs(300),
            );
            assert!(quote.is_err());
        }

        #[test]
        fn is_debit_returns_true_for_positive_net() {
            let strategy = test_strategy();
            let leg_prices = test_leg_prices();
            let quote = PackageQuote::new(
                RfqId::new_v4(),
                VenueId::new("test-venue"),
                strategy,
                Decimal::new(1000, 0), // Positive = debit
                leg_prices,
                Timestamp::now().add_secs(300),
            )
            .unwrap();
            assert!(quote.is_debit());
            assert!(!quote.is_credit());
        }

        #[test]
        fn is_credit_returns_true_for_negative_net() {
            let strategy = test_strategy();
            let leg_prices = test_leg_prices();
            let quote = PackageQuote::new(
                RfqId::new_v4(),
                VenueId::new("test-venue"),
                strategy,
                Decimal::new(-500, 0), // Negative = credit
                leg_prices,
                Timestamp::now().add_secs(300),
            )
            .unwrap();
            assert!(quote.is_credit());
            assert!(!quote.is_debit());
        }

        #[test]
        fn total_quantity_sums_all_legs() {
            let strategy = test_strategy();
            let leg_prices = test_leg_prices();
            let quote = PackageQuote::new(
                RfqId::new_v4(),
                VenueId::new("test-venue"),
                strategy,
                Decimal::new(1000, 0),
                leg_prices,
                Timestamp::now().add_secs(300),
            )
            .unwrap();
            assert_eq!(quote.total_quantity(), Quantity::new(2.0).unwrap());
        }

        #[test]
        fn builder_creates_valid_quote() {
            let strategy = test_strategy();
            let leg_prices = test_leg_prices();
            let quote = PackageQuote::builder(
                RfqId::new_v4(),
                VenueId::new("test-venue"),
                strategy,
                Decimal::new(1000, 0),
                Timestamp::now().add_secs(300),
            )
            .leg_prices(leg_prices)
            .build();
            assert!(quote.is_ok());
        }

        #[test]
        fn display_formats_correctly() {
            let strategy = test_strategy();
            let leg_prices = test_leg_prices();
            let quote = PackageQuote::new(
                RfqId::new_v4(),
                VenueId::new("test-venue"),
                strategy,
                Decimal::new(1000, 0),
                leg_prices,
                Timestamp::now().add_secs(300),
            )
            .unwrap();
            let display = quote.to_string();
            assert!(display.contains("2 legs"));
            assert!(display.contains("debit"));
            assert!(display.contains("test-venue"));
        }
    }
}
