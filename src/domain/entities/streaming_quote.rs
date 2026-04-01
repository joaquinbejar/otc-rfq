//! # Streaming Quote Entity
//!
//! Represents continuous two-sided quotes from market makers.
//!
//! This module provides the [`StreamingQuote`] entity for real-time bid/ask
//! quotes pushed by market makers, along with configuration and aggregation types.
//!
//! # Architecture
//!
//! ```text
//! MM pushes quote via WebSocket/gRPC/FIX
//!        |
//!        v
//! StreamingQuote created with TTL
//!        |
//!        v
//! Stored in StreamingQuoteBook per instrument
//!        |
//!        +---> Best bid/ask aggregated across MMs
//!        |
//!        +---> Stale quotes expired after TTL
//! ```
//!
//! # Examples
//!
//! ```
//! use otc_rfq::domain::entities::streaming_quote::{StreamingQuote, StreamingQuoteConfig};
//! use otc_rfq::domain::value_objects::{CounterpartyId, Instrument, Price, Quantity};
//!
//! let config = StreamingQuoteConfig::default();
//! assert_eq!(config.default_ttl_ms(), 5000);
//! ```

use crate::domain::errors::{DomainError, DomainResult};
use crate::domain::value_objects::timestamp::Timestamp;
use crate::domain::value_objects::{CounterpartyId, Instrument, Price, Quantity};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

// ============================================================================
// StreamingQuoteId
// ============================================================================

/// Unique identifier for a streaming quote.
///
/// A UUID-based identifier uniquely identifying a streaming quote update.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StreamingQuoteId(Uuid);

impl StreamingQuoteId {
    /// Creates a new streaming quote ID from an existing UUID.
    #[inline]
    #[must_use]
    pub const fn new(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Generates a new random streaming quote ID using UUID v4.
    #[must_use]
    pub fn new_v4() -> Self {
        Self(Uuid::new_v4())
    }

    /// Returns the inner UUID value.
    #[inline]
    #[must_use]
    pub const fn get(self) -> Uuid {
        self.0
    }
}

impl fmt::Display for StreamingQuoteId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.hyphenated())
    }
}

impl From<Uuid> for StreamingQuoteId {
    #[inline]
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

// ============================================================================
// PriceLevel
// ============================================================================

/// A price level with associated size.
///
/// Represents one side of a two-sided quote (bid or ask) with price and quantity.
///
/// # Invariants
///
/// - Price must be positive
/// - Size must be positive
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PriceLevel {
    /// The price at this level.
    price: Price,
    /// The available size at this price.
    size: Quantity,
}

impl PriceLevel {
    /// Creates a new price level with validation.
    ///
    /// # Arguments
    ///
    /// * `price` - The price (must be positive)
    /// * `size` - The available size (must be positive)
    ///
    /// # Errors
    ///
    /// Returns `DomainError::InvalidPrice` if price is not positive.
    /// Returns `DomainError::InvalidQuantity` if size is not positive.
    pub fn new(price: Price, size: Quantity) -> DomainResult<Self> {
        if !price.is_positive() {
            return Err(DomainError::InvalidPrice(
                "price must be positive".to_string(),
            ));
        }
        if !size.is_positive() {
            return Err(DomainError::InvalidQuantity(
                "size must be positive".to_string(),
            ));
        }
        Ok(Self { price, size })
    }

    /// Returns the price.
    #[inline]
    #[must_use]
    pub fn price(&self) -> Price {
        self.price
    }

    /// Returns the size.
    #[inline]
    #[must_use]
    pub fn size(&self) -> Quantity {
        self.size
    }
}

// ============================================================================
// StreamingQuote
// ============================================================================

/// A streaming two-sided quote from a market maker.
///
/// Represents a continuous quote update pushed by an MM, containing both
/// bid and ask prices with sizes. Quotes have a TTL after which they become stale.
///
/// # Invariants
///
/// - Bid price must be less than ask price (positive spread)
/// - TTL must be positive
///
/// # Examples
///
/// ```
/// use otc_rfq::domain::entities::streaming_quote::{StreamingQuote, PriceLevel};
/// use otc_rfq::domain::value_objects::{CounterpartyId, Instrument, Price, Quantity};
/// use otc_rfq::domain::value_objects::enums::{AssetClass, SettlementMethod};
/// use otc_rfq::domain::value_objects::symbol::Symbol;
///
/// let symbol = Symbol::new("BTC/USD").unwrap();
/// let instrument = Instrument::new(symbol, AssetClass::CryptoSpot, SettlementMethod::default());
/// let bid = PriceLevel::new(Price::new(50000.0).unwrap(), Quantity::new(1.0).unwrap()).unwrap();
/// let ask = PriceLevel::new(Price::new(50010.0).unwrap(), Quantity::new(1.0).unwrap()).unwrap();
///
/// let quote = StreamingQuote::new(
///     CounterpartyId::new("mm-1"),
///     instrument,
///     bid,
///     ask,
///     5000, // 5 second TTL
/// ).unwrap();
///
/// assert!(!quote.is_stale());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamingQuote {
    /// Unique identifier for this quote update.
    id: StreamingQuoteId,
    /// The market maker providing this quote.
    mm_id: CounterpartyId,
    /// The instrument being quoted.
    instrument: Instrument,
    /// Bid price level (buy side).
    bid: PriceLevel,
    /// Ask price level (sell side).
    ask: PriceLevel,
    /// MM's timestamp when quote was generated.
    timestamp: Timestamp,
    /// Our timestamp when quote was received.
    received_at: Timestamp,
    /// Time-to-live in milliseconds.
    ttl_ms: u64,
}

impl StreamingQuote {
    /// Creates a new streaming quote with validation.
    ///
    /// # Arguments
    ///
    /// * `mm_id` - The market maker identifier
    /// * `instrument` - The instrument being quoted
    /// * `bid` - Bid price level
    /// * `ask` - Ask price level
    /// * `ttl_ms` - Time-to-live in milliseconds
    ///
    /// # Errors
    ///
    /// Returns `DomainError::InvalidPrice` if bid >= ask (crossed/locked market).
    /// Returns `DomainError::InvalidQuantity` if ttl_ms is zero.
    pub fn new(
        mm_id: CounterpartyId,
        instrument: Instrument,
        bid: PriceLevel,
        ask: PriceLevel,
        ttl_ms: u64,
    ) -> DomainResult<Self> {
        // Validate spread: bid must be less than ask
        if bid.price().get() >= ask.price().get() {
            return Err(DomainError::InvalidPrice(
                "bid price must be less than ask price (crossed market)".to_string(),
            ));
        }

        // Validate TTL
        if ttl_ms == 0 {
            return Err(DomainError::InvalidQuantity(
                "TTL must be positive".to_string(),
            ));
        }

        let now = Timestamp::now();

        Ok(Self {
            id: StreamingQuoteId::new_v4(),
            mm_id,
            instrument,
            bid,
            ask,
            timestamp: now,
            received_at: now,
            ttl_ms,
        })
    }

    /// Creates a streaming quote with explicit timestamps.
    ///
    /// Used when MM provides its own timestamp. Note that a new quote ID is
    /// always generated; if you need to preserve an existing ID, use a
    /// different constructor.
    ///
    /// # Errors
    ///
    /// Returns `DomainError::InvalidPrice` if bid >= ask.
    /// Returns `DomainError::InvalidQuantity` if ttl_ms is zero.
    pub fn with_timestamps(
        mm_id: CounterpartyId,
        instrument: Instrument,
        bid: PriceLevel,
        ask: PriceLevel,
        timestamp: Timestamp,
        received_at: Timestamp,
        ttl_ms: u64,
    ) -> DomainResult<Self> {
        // Validate spread
        if bid.price().get() >= ask.price().get() {
            return Err(DomainError::InvalidPrice(
                "bid price must be less than ask price (crossed market)".to_string(),
            ));
        }

        // Validate TTL
        if ttl_ms == 0 {
            return Err(DomainError::InvalidQuantity(
                "TTL must be positive".to_string(),
            ));
        }

        Ok(Self {
            id: StreamingQuoteId::new_v4(),
            mm_id,
            instrument,
            bid,
            ask,
            timestamp,
            received_at,
            ttl_ms,
        })
    }

    /// Returns the quote ID.
    #[inline]
    #[must_use]
    pub fn id(&self) -> StreamingQuoteId {
        self.id
    }

    /// Returns the market maker ID.
    #[inline]
    #[must_use]
    pub fn mm_id(&self) -> &CounterpartyId {
        &self.mm_id
    }

    /// Returns the instrument.
    #[inline]
    #[must_use]
    pub fn instrument(&self) -> &Instrument {
        &self.instrument
    }

    /// Returns the bid price level.
    #[inline]
    #[must_use]
    pub fn bid(&self) -> &PriceLevel {
        &self.bid
    }

    /// Returns the ask price level.
    #[inline]
    #[must_use]
    pub fn ask(&self) -> &PriceLevel {
        &self.ask
    }

    /// Returns the MM's timestamp.
    #[inline]
    #[must_use]
    pub fn timestamp(&self) -> Timestamp {
        self.timestamp
    }

    /// Returns when the quote was received.
    #[inline]
    #[must_use]
    pub fn received_at(&self) -> Timestamp {
        self.received_at
    }

    /// Returns the TTL in milliseconds.
    #[inline]
    #[must_use]
    pub const fn ttl_ms(&self) -> u64 {
        self.ttl_ms
    }

    /// Checks if the quote has exceeded its TTL and is stale.
    ///
    /// A quote is stale if the current time exceeds `received_at + ttl_ms`.
    #[inline]
    #[must_use]
    pub fn is_stale(&self) -> bool {
        let now = Timestamp::now();
        // Convert TTL to i64 safely; clamp absurdly large values to i64::MAX.
        let ttl_i64 = i64::try_from(self.ttl_ms).unwrap_or(i64::MAX);
        let expiry = self.received_at.add_millis(ttl_i64);
        now > expiry
    }

    /// Calculates the spread in basis points.
    ///
    /// Spread = (ask - bid) / mid * 10000
    ///
    /// Uses checked arithmetic to prevent overflow.
    /// Rounds toward positive infinity (conservative).
    ///
    /// # Returns
    ///
    /// The spread in basis points, or `None` if arithmetic overflow occurs.
    #[inline]
    #[must_use]
    pub fn spread_bps(&self) -> Option<Decimal> {
        let bid = self.bid.price().get();
        let ask = self.ask.price().get();

        // spread = ask - bid (checked)
        let spread = ask.checked_sub(bid)?;

        // mid = (bid + ask) / 2 (checked)
        let sum = bid.checked_add(ask)?;
        let two = Decimal::new(2, 0);
        let mid = sum.checked_div(two)?;

        // Avoid division by zero
        if mid.is_zero() {
            return None;
        }

        // bps = spread / mid * 10000 (checked)
        let bps_multiplier = Decimal::new(10000, 0);
        let ratio = spread.checked_div(mid)?;
        ratio.checked_mul(bps_multiplier)
    }

    /// Calculates the mid price.
    ///
    /// Mid = (bid + ask) / 2
    ///
    /// Uses checked arithmetic to prevent overflow.
    ///
    /// # Returns
    ///
    /// The mid price, or `None` if arithmetic overflow occurs.
    #[inline]
    #[must_use]
    pub fn mid_price(&self) -> Option<Price> {
        let bid = self.bid.price().get();
        let ask = self.ask.price().get();

        let sum = bid.checked_add(ask)?;
        let two = Decimal::new(2, 0);
        let mid = sum.checked_div(two)?;

        Price::from_decimal(mid).ok()
    }

    /// Returns the time remaining until this quote expires, in milliseconds.
    ///
    /// Returns 0 if the quote is already stale.
    #[must_use]
    pub fn time_remaining_ms(&self) -> u64 {
        let now = Timestamp::now();
        // Convert TTL to i64 safely; clamp absurdly large values to i64::MAX.
        let ttl_i64 = i64::try_from(self.ttl_ms).unwrap_or(i64::MAX);
        let expiry = self.received_at.add_millis(ttl_i64);

        if now >= expiry {
            0
        } else {
            let duration = now.duration_until(&expiry);
            duration.as_millis() as u64
        }
    }
}

// ============================================================================
// StreamingQuoteConfig
// ============================================================================

/// Configuration for streaming quote processing.
///
/// Controls TTL, rate limiting, and validation parameters.
///
/// # Examples
///
/// ```
/// use otc_rfq::domain::entities::streaming_quote::StreamingQuoteConfig;
/// use rust_decimal::Decimal;
///
/// let config = StreamingQuoteConfig::builder()
///     .default_ttl_ms(3000)
///     .max_spread_bps(Decimal::new(100, 0))
///     .build();
///
/// assert_eq!(config.default_ttl_ms(), 3000);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamingQuoteConfig {
    /// Default TTL for quotes in milliseconds.
    default_ttl_ms: u64,
    /// Maximum quotes per MM per second (rate limit).
    max_quotes_per_second: u32,
    /// Maximum allowed spread in basis points.
    max_spread_bps: Decimal,
    /// Minimum quote size.
    min_size: Decimal,
    /// Staleness check interval in milliseconds.
    staleness_check_interval_ms: u64,
}

/// Default TTL: 5 seconds.
pub const DEFAULT_TTL_MS: u64 = 5000;

/// Default rate limit: 100 quotes per second per MM.
pub const DEFAULT_MAX_QUOTES_PER_SECOND: u32 = 100;

/// Default max spread: 500 bps (5%).
pub const DEFAULT_MAX_SPREAD_BPS: i64 = 500;

/// Default staleness check interval: 1 second.
pub const DEFAULT_STALENESS_CHECK_INTERVAL_MS: u64 = 1000;

impl Default for StreamingQuoteConfig {
    fn default() -> Self {
        Self {
            default_ttl_ms: DEFAULT_TTL_MS,
            max_quotes_per_second: DEFAULT_MAX_QUOTES_PER_SECOND,
            max_spread_bps: Decimal::new(DEFAULT_MAX_SPREAD_BPS, 0),
            min_size: Decimal::ZERO,
            staleness_check_interval_ms: DEFAULT_STALENESS_CHECK_INTERVAL_MS,
        }
    }
}

impl StreamingQuoteConfig {
    /// Creates a new builder for `StreamingQuoteConfig`.
    pub fn builder() -> StreamingQuoteConfigBuilder {
        StreamingQuoteConfigBuilder::default()
    }

    /// Returns the default TTL in milliseconds.
    #[inline]
    #[must_use]
    pub const fn default_ttl_ms(&self) -> u64 {
        self.default_ttl_ms
    }

    /// Returns the maximum quotes per second per MM.
    #[inline]
    #[must_use]
    pub const fn max_quotes_per_second(&self) -> u32 {
        self.max_quotes_per_second
    }

    /// Returns the maximum allowed spread in basis points.
    #[inline]
    #[must_use]
    pub fn max_spread_bps(&self) -> Decimal {
        self.max_spread_bps
    }

    /// Returns the minimum quote size.
    #[inline]
    #[must_use]
    pub fn min_size(&self) -> Decimal {
        self.min_size
    }

    /// Returns the staleness check interval in milliseconds.
    #[inline]
    #[must_use]
    pub const fn staleness_check_interval_ms(&self) -> u64 {
        self.staleness_check_interval_ms
    }
}

// ============================================================================
// StreamingQuoteConfigBuilder
// ============================================================================

/// Builder for [`StreamingQuoteConfig`].
#[derive(Debug, Clone, Default)]
#[must_use = "builders do nothing unless .build() is called"]
pub struct StreamingQuoteConfigBuilder {
    config: StreamingQuoteConfig,
}

impl StreamingQuoteConfigBuilder {
    /// Sets the default TTL in milliseconds.
    pub fn default_ttl_ms(mut self, value: u64) -> Self {
        self.config.default_ttl_ms = value;
        self
    }

    /// Sets the maximum quotes per second per MM.
    pub fn max_quotes_per_second(mut self, value: u32) -> Self {
        self.config.max_quotes_per_second = value;
        self
    }

    /// Sets the maximum allowed spread in basis points.
    pub fn max_spread_bps(mut self, value: Decimal) -> Self {
        self.config.max_spread_bps = value;
        self
    }

    /// Sets the minimum quote size.
    pub fn min_size(mut self, value: Decimal) -> Self {
        self.config.min_size = value;
        self
    }

    /// Sets the staleness check interval in milliseconds.
    pub fn staleness_check_interval_ms(mut self, value: u64) -> Self {
        self.config.staleness_check_interval_ms = value;
        self
    }

    /// Builds the configuration.
    ///
    /// Enforces basic invariants to avoid constructing invalid configs
    /// (e.g., zero TTL or zero `max_quotes_per_second`).
    #[must_use]
    pub fn build(mut self) -> StreamingQuoteConfig {
        // Ensure a positive default TTL.
        if self.config.default_ttl_ms == 0 {
            self.config.default_ttl_ms = 1;
        }

        // Ensure a positive max quotes per second to avoid "always rate-limited".
        if self.config.max_quotes_per_second == 0 {
            self.config.max_quotes_per_second = 1;
        }

        // Ensure staleness check interval is non-zero.
        if self.config.staleness_check_interval_ms == 0 {
            self.config.staleness_check_interval_ms = self.config.default_ttl_ms;
        }

        self.config
    }
}

// ============================================================================
// BestQuote
// ============================================================================

/// Aggregated best bid/ask across all market makers for an instrument.
///
/// Represents the current best available prices from the streaming quote book.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[must_use]
pub struct BestQuote {
    /// Best bid price level.
    bid: PriceLevel,
    /// Market maker providing the best bid.
    bid_mm: CounterpartyId,
    /// Best ask price level.
    ask: PriceLevel,
    /// Market maker providing the best ask.
    ask_mm: CounterpartyId,
    /// When this best quote was computed.
    computed_at: Timestamp,
}

impl BestQuote {
    /// Creates a new best quote.
    pub fn new(
        bid: PriceLevel,
        bid_mm: CounterpartyId,
        ask: PriceLevel,
        ask_mm: CounterpartyId,
    ) -> Self {
        Self {
            bid,
            bid_mm,
            ask,
            ask_mm,
            computed_at: Timestamp::now(),
        }
    }

    /// Returns the best bid price level.
    #[inline]
    #[must_use]
    pub fn bid(&self) -> &PriceLevel {
        &self.bid
    }

    /// Returns the MM providing the best bid.
    #[inline]
    #[must_use]
    pub fn bid_mm(&self) -> &CounterpartyId {
        &self.bid_mm
    }

    /// Returns the best ask price level.
    #[inline]
    #[must_use]
    pub fn ask(&self) -> &PriceLevel {
        &self.ask
    }

    /// Returns the MM providing the best ask.
    #[inline]
    #[must_use]
    pub fn ask_mm(&self) -> &CounterpartyId {
        &self.ask_mm
    }

    /// Returns when this best quote was computed.
    #[inline]
    #[must_use]
    pub fn computed_at(&self) -> Timestamp {
        self.computed_at
    }

    /// Calculates the spread in basis points.
    #[inline]
    #[must_use]
    pub fn spread_bps(&self) -> Option<Decimal> {
        let bid = self.bid.price().get();
        let ask = self.ask.price().get();

        let spread = ask.checked_sub(bid)?;
        let sum = bid.checked_add(ask)?;
        let two = Decimal::new(2, 0);
        let mid = sum.checked_div(two)?;

        if mid.is_zero() {
            return None;
        }

        let bps_multiplier = Decimal::new(10000, 0);
        let ratio = spread.checked_div(mid)?;
        ratio.checked_mul(bps_multiplier)
    }

    /// Calculates the mid price.
    #[inline]
    #[must_use]
    pub fn mid_price(&self) -> Option<Price> {
        let bid = self.bid.price().get();
        let ask = self.ask.price().get();

        let sum = bid.checked_add(ask)?;
        let two = Decimal::new(2, 0);
        let mid = sum.checked_div(two)?;

        Price::from_decimal(mid).ok()
    }
}

// ============================================================================
// StreamingQuoteStats
// ============================================================================

/// Statistics for streaming quote processing.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamingQuoteStats {
    /// Total quotes received.
    pub quotes_received: u64,
    /// Quotes accepted.
    pub quotes_accepted: u64,
    /// Quotes rejected (validation failed).
    pub quotes_rejected: u64,
    /// Quotes expired (TTL exceeded).
    pub quotes_expired: u64,
    /// Rate limit rejections.
    pub rate_limit_rejections: u64,
}

impl StreamingQuoteStats {
    /// Creates new empty stats.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a received quote.
    pub fn record_received(&mut self) {
        self.quotes_received = self.quotes_received.saturating_add(1);
    }

    /// Records an accepted quote.
    pub fn record_accepted(&mut self) {
        self.quotes_accepted = self.quotes_accepted.saturating_add(1);
    }

    /// Records a rejected quote.
    pub fn record_rejected(&mut self) {
        self.quotes_rejected = self.quotes_rejected.saturating_add(1);
    }

    /// Records an expired quote.
    pub fn record_expired(&mut self) {
        self.quotes_expired = self.quotes_expired.saturating_add(1);
    }

    /// Records a rate limit rejection.
    pub fn record_rate_limited(&mut self) {
        self.rate_limit_rejections = self.rate_limit_rejections.saturating_add(1);
    }

    /// Returns the acceptance rate as a percentage.
    #[must_use]
    pub fn acceptance_rate(&self) -> f64 {
        if self.quotes_received == 0 {
            return 0.0;
        }
        (self.quotes_accepted as f64 / self.quotes_received as f64) * 100.0
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::domain::value_objects::enums::{AssetClass, SettlementMethod};
    use crate::domain::value_objects::symbol::Symbol;

    fn create_test_instrument() -> Instrument {
        let symbol = Symbol::new("BTC/USD").unwrap();
        Instrument::new(symbol, AssetClass::CryptoSpot, SettlementMethod::default())
    }

    fn create_test_price_level(price: f64, size: f64) -> PriceLevel {
        PriceLevel::new(Price::new(price).unwrap(), Quantity::new(size).unwrap()).unwrap()
    }

    #[test]
    fn streaming_quote_id_display() {
        let id = StreamingQuoteId::new_v4();
        let display = format!("{}", id);
        assert!(display.contains('-')); // UUID format
    }

    #[test]
    fn price_level_creation() {
        let level = create_test_price_level(50000.0, 1.0);
        assert_eq!(level.price().get(), Decimal::new(50000, 0));
        assert_eq!(level.size().get(), Decimal::new(1, 0));
    }

    #[test]
    fn price_level_invalid_price() {
        let result = PriceLevel::new(Price::new(0.0).unwrap(), Quantity::new(1.0).unwrap());
        assert!(result.is_err());
    }

    #[test]
    fn streaming_quote_creation() {
        let instrument = create_test_instrument();
        let bid = create_test_price_level(50000.0, 1.0);
        let ask = create_test_price_level(50010.0, 1.0);

        let quote =
            StreamingQuote::new(CounterpartyId::new("mm-1"), instrument, bid, ask, 5000).unwrap();

        assert_eq!(quote.mm_id().to_string(), "mm-1");
        assert_eq!(quote.ttl_ms(), 5000);
        assert!(!quote.is_stale());
    }

    #[test]
    fn streaming_quote_crossed_market_rejected() {
        let instrument = create_test_instrument();
        let bid = create_test_price_level(50010.0, 1.0); // bid > ask
        let ask = create_test_price_level(50000.0, 1.0);

        let result = StreamingQuote::new(CounterpartyId::new("mm-1"), instrument, bid, ask, 5000);

        assert!(result.is_err());
    }

    #[test]
    fn streaming_quote_zero_ttl_rejected() {
        let instrument = create_test_instrument();
        let bid = create_test_price_level(50000.0, 1.0);
        let ask = create_test_price_level(50010.0, 1.0);

        let result = StreamingQuote::new(
            CounterpartyId::new("mm-1"),
            instrument,
            bid,
            ask,
            0, // zero TTL
        );

        assert!(result.is_err());
    }

    #[test]
    fn streaming_quote_spread_bps() {
        let instrument = create_test_instrument();
        let bid = create_test_price_level(50000.0, 1.0);
        let ask = create_test_price_level(50010.0, 1.0);

        let quote =
            StreamingQuote::new(CounterpartyId::new("mm-1"), instrument, bid, ask, 5000).unwrap();

        let spread = quote.spread_bps().unwrap();
        // spread = (50010 - 50000) / 50005 * 10000 ≈ 2 bps
        assert!(spread > Decimal::ZERO);
        assert!(spread < Decimal::new(5, 0)); // Less than 5 bps
    }

    #[test]
    fn streaming_quote_mid_price() {
        let instrument = create_test_instrument();
        let bid = create_test_price_level(50000.0, 1.0);
        let ask = create_test_price_level(50010.0, 1.0);

        let quote =
            StreamingQuote::new(CounterpartyId::new("mm-1"), instrument, bid, ask, 5000).unwrap();

        let mid = quote.mid_price().unwrap();
        assert_eq!(mid.get(), Decimal::new(50005, 0));
    }

    #[test]
    fn streaming_quote_config_default() {
        let config = StreamingQuoteConfig::default();
        assert_eq!(config.default_ttl_ms(), DEFAULT_TTL_MS);
        assert_eq!(
            config.max_quotes_per_second(),
            DEFAULT_MAX_QUOTES_PER_SECOND
        );
    }

    #[test]
    fn streaming_quote_config_builder() {
        let config = StreamingQuoteConfig::builder()
            .default_ttl_ms(3000)
            .max_quotes_per_second(50)
            .max_spread_bps(Decimal::new(100, 0))
            .build();

        assert_eq!(config.default_ttl_ms(), 3000);
        assert_eq!(config.max_quotes_per_second(), 50);
        assert_eq!(config.max_spread_bps(), Decimal::new(100, 0));
    }

    #[test]
    fn best_quote_creation() {
        let bid = create_test_price_level(50000.0, 1.0);
        let ask = create_test_price_level(50010.0, 1.0);

        let best = BestQuote::new(
            bid,
            CounterpartyId::new("mm-1"),
            ask,
            CounterpartyId::new("mm-2"),
        );

        assert_eq!(best.bid_mm().to_string(), "mm-1");
        assert_eq!(best.ask_mm().to_string(), "mm-2");
    }

    #[test]
    fn streaming_quote_stats() {
        let mut stats = StreamingQuoteStats::new();

        stats.record_received();
        stats.record_received();
        stats.record_accepted();
        stats.record_rejected();

        assert_eq!(stats.quotes_received, 2);
        assert_eq!(stats.quotes_accepted, 1);
        assert_eq!(stats.quotes_rejected, 1);
        assert_eq!(stats.acceptance_rate(), 50.0);
    }

    #[test]
    fn streaming_quote_stats_empty() {
        let stats = StreamingQuoteStats::new();
        assert_eq!(stats.acceptance_rate(), 0.0);
    }
}
