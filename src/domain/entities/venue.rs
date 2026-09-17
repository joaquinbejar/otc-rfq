//! # Venue Entity
//!
//! Represents a liquidity venue configuration.
//!
//! This module provides the [`Venue`] entity representing a liquidity source,
//! including health monitoring, configuration, and performance metrics.
//!
//! # Examples
//!
//! ```
//! use otc_rfq::domain::entities::venue::{Venue, VenueHealth};
//! use otc_rfq::domain::value_objects::{VenueId, VenueType};
//!
//! let venue = Venue::new(
//!     VenueId::new("binance"),
//!     "Binance",
//!     VenueType::ExternalMM,
//! );
//!
//! assert!(venue.is_available());
//! assert!(venue.is_healthy());
//! ```

use crate::domain::value_objects::timestamp::Timestamp;
use crate::domain::value_objects::{Instrument, VenueId, VenueType};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

/// Venue health status.
///
/// Represents the operational health of a liquidity venue.
///
/// # Examples
///
/// ```
/// use otc_rfq::domain::entities::venue::VenueHealth;
///
/// let health = VenueHealth::Healthy;
/// assert!(health.is_operational());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[repr(u8)]
pub enum VenueHealth {
    /// Venue is fully operational.
    #[default]
    Healthy = 0,

    /// Venue is operational but experiencing issues.
    Degraded = 1,

    /// Venue is not operational.
    Unhealthy = 2,

    /// Venue health status is unknown.
    Unknown = 3,
}

impl VenueHealth {
    /// Returns true if the venue is operational (Healthy or Degraded).
    ///
    /// # Examples
    ///
    /// ```
    /// use otc_rfq::domain::entities::venue::VenueHealth;
    ///
    /// assert!(VenueHealth::Healthy.is_operational());
    /// assert!(VenueHealth::Degraded.is_operational());
    /// assert!(!VenueHealth::Unhealthy.is_operational());
    /// ```
    #[inline]
    #[must_use]
    pub const fn is_operational(&self) -> bool {
        matches!(self, Self::Healthy | Self::Degraded)
    }

    /// Returns true if the venue is fully healthy.
    #[inline]
    #[must_use]
    pub const fn is_healthy(&self) -> bool {
        matches!(self, Self::Healthy)
    }

    /// Returns true if the venue is degraded.
    #[inline]
    #[must_use]
    pub const fn is_degraded(&self) -> bool {
        matches!(self, Self::Degraded)
    }

    /// Returns true if the venue is unhealthy.
    #[inline]
    #[must_use]
    pub const fn is_unhealthy(&self) -> bool {
        matches!(self, Self::Unhealthy)
    }

    /// Returns the numeric value of this health status.
    #[inline]
    #[must_use]
    pub const fn as_u8(&self) -> u8 {
        *self as u8
    }
}

impl fmt::Display for VenueHealth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Healthy => "HEALTHY",
            Self::Degraded => "DEGRADED",
            Self::Unhealthy => "UNHEALTHY",
            Self::Unknown => "UNKNOWN",
        };
        write!(f, "{}", s)
    }
}

impl TryFrom<u8> for VenueHealth {
    type Error = InvalidVenueHealthError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Healthy),
            1 => Ok(Self::Degraded),
            2 => Ok(Self::Unhealthy),
            3 => Ok(Self::Unknown),
            _ => Err(InvalidVenueHealthError(value)),
        }
    }
}

/// Error returned when converting an invalid u8 to VenueHealth.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidVenueHealthError(pub u8);

impl fmt::Display for InvalidVenueHealthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid venue health value: {}", self.0)
    }
}

impl std::error::Error for InvalidVenueHealthError {}

/// Venue-specific configuration.
///
/// Contains configuration parameters for connecting to and interacting
/// with a liquidity venue.
///
/// # Examples
///
/// ```
/// use otc_rfq::domain::entities::venue::VenueConfig;
///
/// let mut config = VenueConfig::new();
/// config.set("api_url", "https://api.venue.com");
/// config.set("timeout_ms", "5000");
///
/// assert_eq!(config.get("api_url"), Some(&"https://api.venue.com".to_string()));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct VenueConfig {
    /// Configuration key-value pairs.
    settings: HashMap<String, String>,
    /// Request timeout in milliseconds.
    timeout_ms: u64,
    /// Maximum concurrent requests.
    max_concurrent_requests: u32,
    /// Whether to use TLS.
    use_tls: bool,
}

impl VenueConfig {
    /// Creates a new empty configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            settings: HashMap::new(),
            timeout_ms: 5000,
            max_concurrent_requests: 10,
            use_tls: true,
        }
    }

    /// Creates a configuration with custom defaults.
    #[must_use]
    pub fn with_defaults(timeout_ms: u64, max_concurrent_requests: u32, use_tls: bool) -> Self {
        Self {
            settings: HashMap::new(),
            timeout_ms,
            max_concurrent_requests,
            use_tls,
        }
    }

    /// Sets a configuration value.
    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.settings.insert(key.into(), value.into());
    }

    /// Gets a configuration value.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&String> {
        self.settings.get(key)
    }

    /// Returns the request timeout in milliseconds.
    #[inline]
    #[must_use]
    pub fn timeout_ms(&self) -> u64 {
        self.timeout_ms
    }

    /// Sets the request timeout in milliseconds.
    pub fn set_timeout_ms(&mut self, timeout_ms: u64) {
        self.timeout_ms = timeout_ms;
    }

    /// Returns the maximum concurrent requests.
    #[inline]
    #[must_use]
    pub fn max_concurrent_requests(&self) -> u32 {
        self.max_concurrent_requests
    }

    /// Sets the maximum concurrent requests.
    pub fn set_max_concurrent_requests(&mut self, max: u32) {
        self.max_concurrent_requests = max;
    }

    /// Returns whether TLS is enabled.
    #[inline]
    #[must_use]
    pub fn use_tls(&self) -> bool {
        self.use_tls
    }

    /// Sets whether to use TLS.
    pub fn set_use_tls(&mut self, use_tls: bool) {
        self.use_tls = use_tls;
    }
}

/// Venue performance metrics.
///
/// Tracks performance statistics for a liquidity venue.
///
/// # Examples
///
/// ```
/// use otc_rfq::domain::entities::venue::VenueMetrics;
///
/// let mut metrics = VenueMetrics::new();
/// metrics.record_request(150, true);
/// metrics.record_request(200, true);
/// metrics.record_request(100, false);
///
/// assert_eq!(metrics.total_requests(), 3);
/// assert_eq!(metrics.successful_requests(), 2);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct VenueMetrics {
    /// Total number of requests made.
    total_requests: u64,
    /// Number of successful requests.
    successful_requests: u64,
    /// Number of failed requests.
    failed_requests: u64,
    /// Total latency in milliseconds (for averaging).
    total_latency_ms: u64,
    /// Last request timestamp.
    last_request_at: Option<Timestamp>,
    /// Last successful request timestamp.
    last_success_at: Option<Timestamp>,
    /// Last failure timestamp.
    last_failure_at: Option<Timestamp>,
}

impl VenueMetrics {
    /// Creates new empty metrics.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a request with its latency and success status.
    pub fn record_request(&mut self, latency_ms: u64, success: bool) {
        let now = Timestamp::now();
        self.total_requests = self.total_requests.saturating_add(1);
        self.total_latency_ms = self.total_latency_ms.saturating_add(latency_ms);
        self.last_request_at = Some(now);

        if success {
            self.successful_requests = self.successful_requests.saturating_add(1);
            self.last_success_at = Some(now);
        } else {
            self.failed_requests = self.failed_requests.saturating_add(1);
            self.last_failure_at = Some(now);
        }
    }

    /// Returns the total number of requests.
    #[inline]
    #[must_use]
    pub fn total_requests(&self) -> u64 {
        self.total_requests
    }

    /// Returns the number of successful requests.
    #[inline]
    #[must_use]
    pub fn successful_requests(&self) -> u64 {
        self.successful_requests
    }

    /// Returns the number of failed requests.
    #[inline]
    #[must_use]
    pub fn failed_requests(&self) -> u64 {
        self.failed_requests
    }

    /// Returns the average latency in milliseconds.
    #[must_use]
    pub fn average_latency_ms(&self) -> Option<u64> {
        self.total_latency_ms.checked_div(self.total_requests)
    }

    /// Returns the success rate as a percentage (0-100).
    #[must_use]
    pub fn success_rate(&self) -> Option<f64> {
        if self.total_requests == 0 {
            None
        } else {
            Some((self.successful_requests as f64 / self.total_requests as f64) * 100.0)
        }
    }

    /// Returns the last request timestamp.
    #[inline]
    #[must_use]
    pub fn last_request_at(&self) -> Option<Timestamp> {
        self.last_request_at
    }

    /// Returns the last successful request timestamp.
    #[inline]
    #[must_use]
    pub fn last_success_at(&self) -> Option<Timestamp> {
        self.last_success_at
    }

    /// Returns the last failure timestamp.
    #[inline]
    #[must_use]
    pub fn last_failure_at(&self) -> Option<Timestamp> {
        self.last_failure_at
    }

    /// Resets all metrics.
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// A liquidity venue.
///
/// Represents a source of liquidity for executing trades, including
/// configuration, health status, and performance metrics.
///
/// # Examples
///
/// ```
/// use otc_rfq::domain::entities::venue::{Venue, VenueHealth};
/// use otc_rfq::domain::value_objects::{VenueId, VenueType};
///
/// let mut venue = Venue::new(
///     VenueId::new("binance"),
///     "Binance",
///     VenueType::ExternalMM,
/// );
///
/// assert!(venue.is_available());
///
/// // Disable the venue
/// venue.set_enabled(false);
/// assert!(!venue.is_available());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Venue {
    /// Unique identifier for this venue.
    id: VenueId,
    /// Human-readable name.
    name: String,
    /// Type of venue.
    venue_type: VenueType,
    /// Whether the venue is enabled.
    enabled: bool,
    /// Current health status.
    health: VenueHealth,
    /// Venue configuration.
    config: VenueConfig,
    /// Performance metrics.
    metrics: VenueMetrics,
    /// Supported instruments.
    supported_instruments: Vec<Instrument>,
    /// When this venue was created.
    created_at: Timestamp,
    /// When this venue was last updated.
    updated_at: Timestamp,
}

impl Venue {
    /// Creates a new venue.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique venue identifier
    /// * `name` - Human-readable name
    /// * `venue_type` - Type of venue
    #[must_use]
    pub fn new(id: VenueId, name: impl Into<String>, venue_type: VenueType) -> Self {
        let now = Timestamp::now();
        Self {
            id,
            name: name.into(),
            venue_type,
            enabled: true,
            health: VenueHealth::Healthy,
            config: VenueConfig::new(),
            metrics: VenueMetrics::new(),
            supported_instruments: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Creates a venue with a specific configuration (for reconstruction from storage).
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn from_parts(
        id: VenueId,
        name: String,
        venue_type: VenueType,
        enabled: bool,
        health: VenueHealth,
        config: VenueConfig,
        metrics: VenueMetrics,
        supported_instruments: Vec<Instrument>,
        created_at: Timestamp,
        updated_at: Timestamp,
    ) -> Self {
        Self {
            id,
            name,
            venue_type,
            enabled,
            health,
            config,
            metrics,
            supported_instruments,
            created_at,
            updated_at,
        }
    }

    // ========== Accessors ==========

    /// Returns the venue ID.
    #[inline]
    #[must_use]
    pub fn id(&self) -> &VenueId {
        &self.id
    }

    /// Returns the venue name.
    #[inline]
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the venue type.
    #[inline]
    #[must_use]
    pub fn venue_type(&self) -> VenueType {
        self.venue_type
    }

    /// Returns whether the venue is enabled.
    #[inline]
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Returns the current health status.
    #[inline]
    #[must_use]
    pub fn health(&self) -> VenueHealth {
        self.health
    }

    /// Returns the venue configuration.
    #[inline]
    #[must_use]
    pub fn config(&self) -> &VenueConfig {
        &self.config
    }

    /// Returns a mutable reference to the venue configuration.
    #[inline]
    pub fn config_mut(&mut self) -> &mut VenueConfig {
        self.updated_at = Timestamp::now();
        &mut self.config
    }

    /// Returns the venue metrics.
    #[inline]
    #[must_use]
    pub fn metrics(&self) -> &VenueMetrics {
        &self.metrics
    }

    /// Returns a mutable reference to the venue metrics.
    #[inline]
    pub fn metrics_mut(&mut self) -> &mut VenueMetrics {
        &mut self.metrics
    }

    /// Returns the supported instruments.
    #[inline]
    #[must_use]
    pub fn supported_instruments(&self) -> &[Instrument] {
        &self.supported_instruments
    }

    /// Returns when this venue was created.
    #[inline]
    #[must_use]
    pub fn created_at(&self) -> Timestamp {
        self.created_at
    }

    /// Returns when this venue was last updated.
    #[inline]
    #[must_use]
    pub fn updated_at(&self) -> Timestamp {
        self.updated_at
    }

    // ========== State Helpers ==========

    /// Returns true if the venue is available for trading.
    ///
    /// A venue is available if it is both enabled and operational (healthy or degraded).
    #[inline]
    #[must_use]
    pub fn is_available(&self) -> bool {
        self.enabled && self.health.is_operational()
    }

    /// Returns true if the venue is fully healthy.
    #[inline]
    #[must_use]
    pub fn is_healthy(&self) -> bool {
        self.health.is_healthy()
    }

    /// Returns true if the venue is a market maker.
    #[inline]
    #[must_use]
    pub fn is_market_maker(&self) -> bool {
        self.venue_type.is_market_maker()
    }

    /// Returns true if the venue is a DeFi venue.
    #[inline]
    #[must_use]
    pub fn is_defi(&self) -> bool {
        self.venue_type.is_defi()
    }

    /// Returns true if the venue supports the given instrument.
    #[must_use]
    pub fn supports_instrument(&self, instrument: &Instrument) -> bool {
        self.supported_instruments.iter().any(|i| i == instrument)
    }

    // ========== Mutators ==========

    /// Sets whether the venue is enabled.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        self.updated_at = Timestamp::now();
    }

    /// Sets the venue health status.
    pub fn set_health(&mut self, health: VenueHealth) {
        self.health = health;
        self.updated_at = Timestamp::now();
    }

    /// Adds a supported instrument.
    pub fn add_instrument(&mut self, instrument: Instrument) {
        if !self.supported_instruments.contains(&instrument) {
            self.supported_instruments.push(instrument);
            self.updated_at = Timestamp::now();
        }
    }

    /// Removes a supported instrument.
    pub fn remove_instrument(&mut self, instrument: &Instrument) {
        if let Some(pos) = self
            .supported_instruments
            .iter()
            .position(|i| i == instrument)
        {
            self.supported_instruments.remove(pos);
            self.updated_at = Timestamp::now();
        }
    }

    /// Clears all supported instruments.
    pub fn clear_instruments(&mut self) {
        if !self.supported_instruments.is_empty() {
            self.supported_instruments.clear();
            self.updated_at = Timestamp::now();
        }
    }
}

impl fmt::Display for Venue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Venue({} {} [{}] {})",
            self.id,
            self.name,
            self.venue_type,
            if self.is_available() {
                "available"
            } else {
                "unavailable"
            }
        )
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::domain::value_objects::{AssetClass, Symbol};

    fn test_venue_id() -> VenueId {
        VenueId::new("test-venue")
    }

    fn create_test_venue() -> Venue {
        Venue::new(test_venue_id(), "Test Venue", VenueType::ExternalMM)
    }

    fn test_instrument() -> Instrument {
        let symbol = Symbol::new("BTC/USD").unwrap();
        Instrument::builder(symbol, AssetClass::CryptoSpot).build()
    }

    mod venue_health {
        use super::*;

        #[test]
        fn healthy_is_operational() {
            assert!(VenueHealth::Healthy.is_operational());
            assert!(VenueHealth::Healthy.is_healthy());
        }

        #[test]
        fn degraded_is_operational() {
            assert!(VenueHealth::Degraded.is_operational());
            assert!(VenueHealth::Degraded.is_degraded());
            assert!(!VenueHealth::Degraded.is_healthy());
        }

        #[test]
        fn unhealthy_is_not_operational() {
            assert!(!VenueHealth::Unhealthy.is_operational());
            assert!(VenueHealth::Unhealthy.is_unhealthy());
        }

        #[test]
        fn unknown_is_not_operational() {
            assert!(!VenueHealth::Unknown.is_operational());
        }

        #[test]
        fn as_u8() {
            assert_eq!(VenueHealth::Healthy.as_u8(), 0);
            assert_eq!(VenueHealth::Degraded.as_u8(), 1);
            assert_eq!(VenueHealth::Unhealthy.as_u8(), 2);
            assert_eq!(VenueHealth::Unknown.as_u8(), 3);
        }

        #[test]
        fn try_from_u8() {
            assert_eq!(VenueHealth::try_from(0).unwrap(), VenueHealth::Healthy);
            assert_eq!(VenueHealth::try_from(2).unwrap(), VenueHealth::Unhealthy);
            assert!(VenueHealth::try_from(99).is_err());
        }

        #[test]
        fn display() {
            assert_eq!(VenueHealth::Healthy.to_string(), "HEALTHY");
            assert_eq!(VenueHealth::Degraded.to_string(), "DEGRADED");
            assert_eq!(VenueHealth::Unhealthy.to_string(), "UNHEALTHY");
            assert_eq!(VenueHealth::Unknown.to_string(), "UNKNOWN");
        }
    }

    mod venue_config {
        use super::*;

        #[test]
        fn new_has_defaults() {
            let config = VenueConfig::new();
            assert_eq!(config.timeout_ms(), 5000);
            assert_eq!(config.max_concurrent_requests(), 10);
            assert!(config.use_tls());
        }

        #[test]
        fn set_and_get() {
            let mut config = VenueConfig::new();
            config.set("api_key", "secret123");

            assert_eq!(config.get("api_key"), Some(&"secret123".to_string()));
            assert_eq!(config.get("nonexistent"), None);
        }

        #[test]
        fn with_defaults() {
            let config = VenueConfig::with_defaults(10000, 20, false);
            assert_eq!(config.timeout_ms(), 10000);
            assert_eq!(config.max_concurrent_requests(), 20);
            assert!(!config.use_tls());
        }
    }

    mod venue_metrics {
        use super::*;

        #[test]
        fn new_is_empty() {
            let metrics = VenueMetrics::new();
            assert_eq!(metrics.total_requests(), 0);
            assert_eq!(metrics.successful_requests(), 0);
            assert_eq!(metrics.failed_requests(), 0);
        }

        #[test]
        fn record_successful_request() {
            let mut metrics = VenueMetrics::new();
            metrics.record_request(100, true);

            assert_eq!(metrics.total_requests(), 1);
            assert_eq!(metrics.successful_requests(), 1);
            assert_eq!(metrics.failed_requests(), 0);
            assert!(metrics.last_success_at().is_some());
        }

        #[test]
        fn record_failed_request() {
            let mut metrics = VenueMetrics::new();
            metrics.record_request(100, false);

            assert_eq!(metrics.total_requests(), 1);
            assert_eq!(metrics.successful_requests(), 0);
            assert_eq!(metrics.failed_requests(), 1);
            assert!(metrics.last_failure_at().is_some());
        }

        #[test]
        fn average_latency() {
            let mut metrics = VenueMetrics::new();
            metrics.record_request(100, true);
            metrics.record_request(200, true);
            metrics.record_request(300, true);

            assert_eq!(metrics.average_latency_ms(), Some(200));
        }

        #[test]
        fn success_rate() {
            let mut metrics = VenueMetrics::new();
            metrics.record_request(100, true);
            metrics.record_request(100, true);
            metrics.record_request(100, false);
            metrics.record_request(100, false);

            let rate = metrics.success_rate().unwrap();
            assert!((rate - 50.0).abs() < 0.001);
        }

        #[test]
        fn reset() {
            let mut metrics = VenueMetrics::new();
            metrics.record_request(100, true);
            metrics.reset();

            assert_eq!(metrics.total_requests(), 0);
        }
    }

    mod venue_construction {
        use super::*;

        #[test]
        fn new_creates_enabled_healthy_venue() {
            let venue = create_test_venue();

            assert!(venue.is_enabled());
            assert!(venue.is_healthy());
            assert!(venue.is_available());
            assert!(venue.supported_instruments().is_empty());
        }

        #[test]
        fn venue_type_helpers() {
            let mm_venue = Venue::new(test_venue_id(), "MM", VenueType::InternalMM);
            assert!(mm_venue.is_market_maker());
            assert!(!mm_venue.is_defi());

            let defi_venue = Venue::new(test_venue_id(), "DEX", VenueType::DexAggregator);
            assert!(!defi_venue.is_market_maker());
            assert!(defi_venue.is_defi());
        }
    }

    mod venue_availability {
        use super::*;

        #[test]
        fn available_when_enabled_and_healthy() {
            let venue = create_test_venue();
            assert!(venue.is_available());
        }

        #[test]
        fn available_when_enabled_and_degraded() {
            let mut venue = create_test_venue();
            venue.set_health(VenueHealth::Degraded);
            assert!(venue.is_available());
        }

        #[test]
        fn unavailable_when_disabled() {
            let mut venue = create_test_venue();
            venue.set_enabled(false);
            assert!(!venue.is_available());
        }

        #[test]
        fn unavailable_when_unhealthy() {
            let mut venue = create_test_venue();
            venue.set_health(VenueHealth::Unhealthy);
            assert!(!venue.is_available());
        }

        #[test]
        fn unavailable_when_unknown_health() {
            let mut venue = create_test_venue();
            venue.set_health(VenueHealth::Unknown);
            assert!(!venue.is_available());
        }
    }

    mod venue_instruments {
        use super::*;

        #[test]
        fn add_instrument() {
            let mut venue = create_test_venue();
            let instrument = test_instrument();

            venue.add_instrument(instrument.clone());

            assert_eq!(venue.supported_instruments().len(), 1);
            assert!(venue.supports_instrument(&instrument));
        }

        #[test]
        fn add_duplicate_instrument_is_noop() {
            let mut venue = create_test_venue();
            let instrument = test_instrument();

            venue.add_instrument(instrument.clone());
            venue.add_instrument(instrument.clone());

            assert_eq!(venue.supported_instruments().len(), 1);
        }

        #[test]
        fn remove_instrument() {
            let mut venue = create_test_venue();
            let instrument = test_instrument();

            venue.add_instrument(instrument.clone());
            venue.remove_instrument(&instrument);

            assert!(venue.supported_instruments().is_empty());
            assert!(!venue.supports_instrument(&instrument));
        }

        #[test]
        fn clear_instruments() {
            let mut venue = create_test_venue();
            venue.add_instrument(test_instrument());

            venue.clear_instruments();

            assert!(venue.supported_instruments().is_empty());
        }
    }

    mod display {
        use super::*;

        #[test]
        fn display_format() {
            let venue = create_test_venue();
            let display = venue.to_string();

            assert!(display.contains("Venue"));
            assert!(display.contains("Test Venue"));
            assert!(display.contains("available"));
        }

        #[test]
        fn display_unavailable() {
            let mut venue = create_test_venue();
            venue.set_enabled(false);
            let display = venue.to_string();

            assert!(display.contains("unavailable"));
        }
    }

    mod serde {
        use super::*;

        #[test]
        fn venue_serde_roundtrip() {
            let mut venue = create_test_venue();
            venue.add_instrument(test_instrument());
            venue.config_mut().set("key", "value");
            venue.metrics_mut().record_request(100, true);

            let json = serde_json::to_string(&venue).unwrap();
            let deserialized: Venue = serde_json::from_str(&json).unwrap();

            assert_eq!(venue.id(), deserialized.id());
            assert_eq!(venue.name(), deserialized.name());
            assert_eq!(venue.venue_type(), deserialized.venue_type());
            assert_eq!(
                venue.supported_instruments().len(),
                deserialized.supported_instruments().len()
            );
        }

        #[test]
        fn venue_health_serde_roundtrip() {
            for health in [
                VenueHealth::Healthy,
                VenueHealth::Degraded,
                VenueHealth::Unhealthy,
                VenueHealth::Unknown,
            ] {
                let json = serde_json::to_string(&health).unwrap();
                let deserialized: VenueHealth = serde_json::from_str(&json).unwrap();
                assert_eq!(health, deserialized);
            }
        }

        #[test]
        fn venue_health_serde_screaming_snake_case() {
            let json = serde_json::to_string(&VenueHealth::Unhealthy).unwrap();
            assert_eq!(json, "\"UNHEALTHY\"");
        }
    }
}
