//! # Internal Market Maker Adapter
//!
//! Adapter for internal market making.
//!
//! This module provides the [`InternalMMAdapter`] which implements the
//! [`VenueAdapter`] trait for the platform's internal market making service.
//!
//! # Features
//!
//! - Configurable spread and commission
//! - Immediate execution (no external API calls)
//! - Always healthy (internal service)
//!
//! # Examples
//!
//! ```ignore
//! use otc_rfq::infrastructure::venues::internal_mm::{InternalMMAdapter, InternalMMConfig};
//!
//! let config = InternalMMConfig::new()
//!     .with_spread_bps(50)  // 0.5% spread
//!     .with_commission_bps(10);  // 0.1% commission
//!
//! let adapter = InternalMMAdapter::new(config);
//! ```

use crate::domain::entities::quote::{Quote, QuoteBuilder};
use crate::domain::entities::rfq::Rfq;
use crate::domain::value_objects::timestamp::Timestamp;
use crate::domain::value_objects::{OrderSide, Price, SettlementMethod, VenueId};
use crate::infrastructure::venues::error::{VenueError, VenueResult};
use crate::infrastructure::venues::traits::{ExecutionResult, VenueAdapter, VenueHealth};
use async_trait::async_trait;
use rust_decimal::prelude::*;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Default venue ID for the internal market maker.
const DEFAULT_VENUE_ID: &str = "internal-mm";

/// Default timeout in milliseconds.
const DEFAULT_TIMEOUT_MS: u64 = 1000;

/// Default spread in basis points (0.5%).
const DEFAULT_SPREAD_BPS: u32 = 50;

/// Default commission in basis points (0.1%).
const DEFAULT_COMMISSION_BPS: u32 = 10;

/// Default quote validity in seconds.
const DEFAULT_QUOTE_VALIDITY_SECS: u64 = 60;

/// Configuration for the internal market maker.
///
/// Controls pricing parameters like spread and commission.
///
/// # Examples
///
/// ```
/// use otc_rfq::infrastructure::venues::internal_mm::InternalMMConfig;
///
/// let config = InternalMMConfig::new()
///     .with_spread_bps(100)  // 1% spread
///     .with_commission_bps(25);  // 0.25% commission
///
/// assert_eq!(config.spread_bps(), 100);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InternalMMConfig {
    /// Venue ID for this market maker.
    venue_id: VenueId,
    /// Spread in basis points (1 bp = 0.01%).
    spread_bps: u32,
    /// Commission in basis points.
    commission_bps: u32,
    /// Quote validity duration in seconds.
    quote_validity_secs: u64,
    /// Timeout for operations in milliseconds.
    timeout_ms: u64,
    /// Whether the market maker is enabled.
    enabled: bool,
}

impl InternalMMConfig {
    /// Creates a new configuration with default values.
    #[must_use]
    pub fn new() -> Self {
        Self {
            venue_id: VenueId::new(DEFAULT_VENUE_ID),
            spread_bps: DEFAULT_SPREAD_BPS,
            commission_bps: DEFAULT_COMMISSION_BPS,
            quote_validity_secs: DEFAULT_QUOTE_VALIDITY_SECS,
            timeout_ms: DEFAULT_TIMEOUT_MS,
            enabled: true,
        }
    }

    /// Sets the venue ID.
    #[must_use]
    pub fn with_venue_id(mut self, venue_id: impl Into<String>) -> Self {
        self.venue_id = VenueId::new(venue_id);
        self
    }

    /// Sets the spread in basis points.
    #[must_use]
    pub fn with_spread_bps(mut self, spread_bps: u32) -> Self {
        self.spread_bps = spread_bps;
        self
    }

    /// Sets the commission in basis points.
    #[must_use]
    pub fn with_commission_bps(mut self, commission_bps: u32) -> Self {
        self.commission_bps = commission_bps;
        self
    }

    /// Sets the quote validity duration in seconds.
    #[must_use]
    pub fn with_quote_validity_secs(mut self, secs: u64) -> Self {
        self.quote_validity_secs = secs;
        self
    }

    /// Sets the timeout in milliseconds.
    #[must_use]
    pub fn with_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }

    /// Sets whether the market maker is enabled.
    #[must_use]
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Returns the venue ID.
    #[inline]
    #[must_use]
    pub fn venue_id(&self) -> &VenueId {
        &self.venue_id
    }

    /// Returns the spread in basis points.
    #[inline]
    #[must_use]
    pub fn spread_bps(&self) -> u32 {
        self.spread_bps
    }

    /// Returns the commission in basis points.
    #[inline]
    #[must_use]
    pub fn commission_bps(&self) -> u32 {
        self.commission_bps
    }

    /// Returns the quote validity duration in seconds.
    #[inline]
    #[must_use]
    pub fn quote_validity_secs(&self) -> u64 {
        self.quote_validity_secs
    }

    /// Returns the timeout in milliseconds.
    #[inline]
    #[must_use]
    pub fn timeout_ms(&self) -> u64 {
        self.timeout_ms
    }

    /// Returns whether the market maker is enabled.
    #[inline]
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Calculates the spread as a decimal multiplier.
    ///
    /// For example, 50 bps = 0.005.
    #[must_use]
    pub fn spread_multiplier(&self) -> f64 {
        f64::from(self.spread_bps) / 10_000.0
    }

    /// Calculates the commission as a decimal multiplier.
    #[must_use]
    pub fn commission_multiplier(&self) -> f64 {
        f64::from(self.commission_bps) / 10_000.0
    }
}

impl Default for InternalMMConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Internal Market Maker adapter.
///
/// Implements the [`VenueAdapter`] trait for the platform's internal
/// market making service. Provides immediate quote generation and
/// execution without external API calls.
///
/// # Pricing
///
/// Quotes are generated by applying a spread to a base price:
/// - Buy orders: base_price * (1 + spread/2)
/// - Sell orders: base_price * (1 - spread/2)
///
/// # Examples
///
/// ```ignore
/// use otc_rfq::infrastructure::venues::internal_mm::{InternalMMAdapter, InternalMMConfig};
///
/// let adapter = InternalMMAdapter::new(InternalMMConfig::new());
///
/// // Request a quote
/// let quote = adapter.request_quote(&rfq).await?;
///
/// // Execute the trade
/// let result = adapter.execute_trade(&quote).await?;
/// ```
pub struct InternalMMAdapter {
    config: InternalMMConfig,
    /// Base price for quote generation (simulated market price).
    /// In a real implementation, this would come from a pricing engine.
    base_price: Price,
}

impl InternalMMAdapter {
    /// Creates a new internal market maker adapter.
    #[must_use]
    pub fn new(config: InternalMMConfig) -> Self {
        Self {
            config,
            base_price: Price::new(50_000.0).unwrap_or(Price::ZERO),
        }
    }

    /// Creates an adapter with a specific base price for testing.
    #[must_use]
    pub fn with_base_price(config: InternalMMConfig, base_price: Price) -> Self {
        Self { config, base_price }
    }

    /// Returns the configuration.
    #[inline]
    #[must_use]
    pub fn config(&self) -> &InternalMMConfig {
        &self.config
    }

    /// Calculates the quote price based on order side.
    ///
    /// Applies spread to the base price:
    /// - Buy: price goes up (client pays more)
    /// - Sell: price goes down (client receives less)
    fn calculate_quote_price(&self, side: OrderSide) -> Option<Price> {
        let spread_half = Decimal::from_f64(self.config.spread_multiplier() / 2.0)?;
        let one = Decimal::ONE;

        let multiplier = match side {
            OrderSide::Buy => one + spread_half,
            OrderSide::Sell => one - spread_half,
        };

        self.base_price.safe_mul(multiplier).ok()
    }

    /// Calculates the commission for a given notional value.
    fn calculate_commission(&self, notional: Price) -> Option<Price> {
        let commission_rate = Decimal::from_f64(self.config.commission_multiplier())?;
        notional.safe_mul(commission_rate).ok()
    }
}

impl fmt::Debug for InternalMMAdapter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InternalMMAdapter")
            .field("venue_id", self.config.venue_id())
            .field("spread_bps", &self.config.spread_bps())
            .field("commission_bps", &self.config.commission_bps())
            .field("enabled", &self.config.is_enabled())
            .finish()
    }
}

#[async_trait]
impl VenueAdapter for InternalMMAdapter {
    fn venue_id(&self) -> &VenueId {
        self.config.venue_id()
    }

    fn timeout_ms(&self) -> u64 {
        self.config.timeout_ms()
    }

    async fn request_quote(&self, rfq: &Rfq) -> VenueResult<Quote> {
        // Check if enabled
        if !self.config.is_enabled() {
            return Err(VenueError::venue_unavailable(
                self.config.venue_id().clone(),
                "Internal MM is disabled",
            ));
        }

        // Calculate quote price based on side
        let quote_price = self
            .calculate_quote_price(rfq.side())
            .ok_or_else(|| VenueError::quote_unavailable("Failed to calculate quote price"))?;

        // Calculate commission
        let notional = quote_price
            .safe_mul(rfq.quantity().get())
            .map_err(|_| VenueError::quote_unavailable("Failed to calculate notional"))?;

        let commission = self.calculate_commission(notional);

        // Calculate validity
        let valid_until = Timestamp::now().add_secs(self.config.quote_validity_secs() as i64);

        // Build the quote
        let mut builder = QuoteBuilder::new(
            rfq.id(),
            self.config.venue_id().clone(),
            quote_price,
            rfq.quantity(),
            valid_until,
        );

        if let Some(comm) = commission {
            builder = builder.commission(comm);
        }

        Ok(builder.build())
    }

    async fn execute_trade(&self, quote: &Quote) -> VenueResult<ExecutionResult> {
        // Check if enabled
        if !self.config.is_enabled() {
            return Err(VenueError::venue_unavailable(
                self.config.venue_id().clone(),
                "Internal MM is disabled",
            ));
        }

        // Check if quote is expired
        if quote.is_expired() {
            return Err(VenueError::quote_expired("Quote has expired"));
        }

        // Verify quote is from this venue
        if quote.venue_id() != self.config.venue_id() {
            return Err(VenueError::invalid_request("Quote is not from this venue"));
        }

        // Create execution result (immediate execution)
        let result = ExecutionResult::new(
            quote.id(),
            self.config.venue_id().clone(),
            quote.price(),
            quote.quantity(),
            SettlementMethod::OffChain,
        )
        .with_venue_execution_id(format!("imm-{}", uuid::Uuid::new_v4()));

        Ok(result)
    }

    async fn health_check(&self) -> VenueResult<VenueHealth> {
        // Internal service is always healthy
        Ok(VenueHealth::healthy(self.config.venue_id().clone()))
    }

    async fn is_available(&self) -> bool {
        self.config.is_enabled()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::domain::entities::rfq::RfqBuilder;
    use crate::domain::value_objects::{AssetClass, CounterpartyId, Instrument, Quantity, Symbol};

    fn test_config() -> InternalMMConfig {
        InternalMMConfig::new()
            .with_spread_bps(100) // 1%
            .with_commission_bps(10) // 0.1%
    }

    fn test_instrument() -> Instrument {
        let symbol = Symbol::new("BTC/USD").unwrap();
        Instrument::builder(symbol, AssetClass::CryptoSpot).build()
    }

    fn test_rfq(side: OrderSide) -> Rfq {
        RfqBuilder::new(
            CounterpartyId::new("test-client"),
            test_instrument(),
            side,
            Quantity::new(1.0).unwrap(),
            Timestamp::now().add_secs(300),
        )
        .build()
    }

    mod config {
        use super::*;

        #[test]
        fn default_values() {
            let config = InternalMMConfig::new();
            assert_eq!(config.venue_id(), &VenueId::new("internal-mm"));
            assert_eq!(config.spread_bps(), 50);
            assert_eq!(config.commission_bps(), 10);
            assert!(config.is_enabled());
        }

        #[test]
        fn with_spread() {
            let config = InternalMMConfig::new().with_spread_bps(100);
            assert_eq!(config.spread_bps(), 100);
        }

        #[test]
        fn spread_multiplier() {
            let config = InternalMMConfig::new().with_spread_bps(100); // 1%
            assert!((config.spread_multiplier() - 0.01).abs() < 0.0001);
        }

        #[test]
        fn commission_multiplier() {
            let config = InternalMMConfig::new().with_commission_bps(25); // 0.25%
            assert!((config.commission_multiplier() - 0.0025).abs() < 0.0001);
        }

        #[test]
        fn disabled() {
            let config = InternalMMConfig::new().with_enabled(false);
            assert!(!config.is_enabled());
        }
    }

    mod adapter {
        use super::*;

        #[test]
        fn debug_format() {
            let adapter = InternalMMAdapter::new(test_config());
            let debug = format!("{:?}", adapter);
            assert!(debug.contains("InternalMMAdapter"));
            assert!(debug.contains("internal-mm"));
        }

        #[test]
        fn venue_id() {
            let adapter = InternalMMAdapter::new(test_config());
            assert_eq!(adapter.venue_id(), &VenueId::new("internal-mm"));
        }

        #[test]
        fn timeout_ms() {
            let config = InternalMMConfig::new().with_timeout_ms(5000);
            let adapter = InternalMMAdapter::new(config);
            assert_eq!(adapter.timeout_ms(), 5000);
        }
    }

    mod quote_generation {
        use super::*;

        #[tokio::test]
        async fn request_quote_buy() {
            let base_price = Price::new(50_000.0).unwrap();
            let adapter = InternalMMAdapter::with_base_price(test_config(), base_price);
            let rfq = test_rfq(OrderSide::Buy);

            let quote = adapter.request_quote(&rfq).await.unwrap();

            // Buy price should be higher than base (spread applied)
            assert!(quote.price() > base_price);
            assert_eq!(quote.venue_id(), &VenueId::new("internal-mm"));
            assert_eq!(quote.rfq_id(), rfq.id());
            assert!(quote.commission().is_some());
        }

        #[tokio::test]
        async fn request_quote_sell() {
            let base_price = Price::new(50_000.0).unwrap();
            let adapter = InternalMMAdapter::with_base_price(test_config(), base_price);
            let rfq = test_rfq(OrderSide::Sell);

            let quote = adapter.request_quote(&rfq).await.unwrap();

            // Sell price should be lower than base (spread applied)
            assert!(quote.price() < base_price);
        }

        #[tokio::test]
        async fn request_quote_disabled() {
            let config = test_config().with_enabled(false);
            let adapter = InternalMMAdapter::new(config);
            let rfq = test_rfq(OrderSide::Buy);

            let result = adapter.request_quote(&rfq).await;
            assert!(result.is_err());
        }
    }

    mod execution {
        use super::*;

        #[tokio::test]
        async fn execute_trade_success() {
            let adapter = InternalMMAdapter::new(test_config());
            let rfq = test_rfq(OrderSide::Buy);

            let quote = adapter.request_quote(&rfq).await.unwrap();
            let result = adapter.execute_trade(&quote).await.unwrap();

            assert_eq!(result.venue_id(), &VenueId::new("internal-mm"));
            assert_eq!(result.quote_id(), quote.id());
            assert_eq!(result.execution_price(), quote.price());
            assert!(result.venue_execution_id().is_some());
        }

        #[tokio::test]
        async fn execute_trade_disabled() {
            let adapter = InternalMMAdapter::new(test_config());
            let rfq = test_rfq(OrderSide::Buy);

            let quote = adapter.request_quote(&rfq).await.unwrap();

            // Disable after getting quote
            let disabled_config = test_config().with_enabled(false);
            let disabled_adapter = InternalMMAdapter::new(disabled_config);

            let result = disabled_adapter.execute_trade(&quote).await;
            assert!(result.is_err());
        }

        #[tokio::test]
        async fn execute_trade_wrong_venue() {
            let adapter = InternalMMAdapter::new(test_config());
            let rfq = test_rfq(OrderSide::Buy);

            // Create quote from different venue
            let quote = QuoteBuilder::new(
                rfq.id(),
                VenueId::new("other-venue"),
                Price::new(50_000.0).unwrap(),
                Quantity::new(1.0).unwrap(),
                Timestamp::now().add_secs(60),
            )
            .build();

            let result = adapter.execute_trade(&quote).await;
            assert!(result.is_err());
        }
    }

    mod health {
        use super::*;

        #[tokio::test]
        async fn health_check_always_healthy() {
            let adapter = InternalMMAdapter::new(test_config());
            let health = adapter.health_check().await.unwrap();

            assert!(health.is_healthy());
            assert_eq!(health.venue_id(), &VenueId::new("internal-mm"));
        }

        #[tokio::test]
        async fn is_available_when_enabled() {
            let adapter = InternalMMAdapter::new(test_config());
            assert!(adapter.is_available().await);
        }

        #[tokio::test]
        async fn is_not_available_when_disabled() {
            let config = test_config().with_enabled(false);
            let adapter = InternalMMAdapter::new(config);
            assert!(!adapter.is_available().await);
        }
    }
}
