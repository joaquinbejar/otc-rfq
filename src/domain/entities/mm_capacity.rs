//! # Market Maker Capacity Entity
//!
//! Defines capacity configuration and state for market makers.
//!
//! This module provides the [`MmCapacityConfig`] for defining capacity limits
//! and [`MmCapacityState`] for tracking current usage. The capacity system
//! prevents market makers from being overloaded with too many concurrent
//! quotes or excessive notional exposure.
//!
//! # Examples
//!
//! ```
//! use otc_rfq::domain::entities::mm_capacity::{
//!     MmCapacityConfig, MmCapacityState, CapacityCheckResult,
//! };
//! use otc_rfq::domain::value_objects::CounterpartyId;
//! use rust_decimal::Decimal;
//!
//! let config = MmCapacityConfig::default();
//! let state = MmCapacityState::new(CounterpartyId::new("mm-citadel"));
//!
//! assert!(state.active_quotes() == 0);
//! assert!(state.current_notional() == Decimal::ZERO);
//! ```

use crate::domain::errors::{DomainError, DomainResult};
use crate::domain::value_objects::CounterpartyId;
use crate::domain::value_objects::ids::RfqId;
use crate::domain::value_objects::symbol::Symbol;
use crate::domain::value_objects::timestamp::Timestamp;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

/// Default maximum concurrent quotes per market maker.
pub const DEFAULT_MAX_CONCURRENT_QUOTES: u32 = 50;

/// Default maximum notional exposure in USD.
pub const DEFAULT_MAX_NOTIONAL_USD: Decimal = Decimal::from_parts(10_000_000, 0, 0, false, 0); // 10M USD

/// Configuration for a market maker's capacity limits.
///
/// Defines the maximum concurrent quotes, notional exposure, and per-instrument
/// limits that a market maker can handle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MmCapacityConfig {
    /// Maximum number of concurrent active quotes.
    max_concurrent_quotes: u32,
    /// Maximum total notional exposure in USD.
    max_notional: Decimal,
    /// Per-instrument limits (symbol -> max notional).
    per_instrument_limits: HashMap<Symbol, Decimal>,
    /// Whether dynamic adjustment based on performance is enabled.
    dynamic_adjustment_enabled: bool,
}

impl MmCapacityConfig {
    /// Creates a new capacity configuration.
    ///
    /// # Arguments
    ///
    /// * `max_concurrent_quotes` - Maximum concurrent quotes allowed
    /// * `max_notional` - Maximum total notional exposure
    #[must_use]
    pub fn new(max_concurrent_quotes: u32, max_notional: Decimal) -> Self {
        Self {
            max_concurrent_quotes,
            max_notional,
            per_instrument_limits: HashMap::new(),
            dynamic_adjustment_enabled: true,
        }
    }

    /// Creates a builder for constructing a capacity configuration.
    pub fn builder() -> MmCapacityConfigBuilder {
        MmCapacityConfigBuilder::new()
    }

    /// Returns the maximum concurrent quotes.
    #[inline]
    #[must_use]
    pub const fn max_concurrent_quotes(&self) -> u32 {
        self.max_concurrent_quotes
    }

    /// Returns the maximum notional exposure.
    #[inline]
    #[must_use]
    pub fn max_notional(&self) -> Decimal {
        self.max_notional
    }

    /// Returns the per-instrument limits.
    #[inline]
    #[must_use]
    pub fn per_instrument_limits(&self) -> &HashMap<Symbol, Decimal> {
        &self.per_instrument_limits
    }

    /// Returns whether dynamic adjustment is enabled.
    #[inline]
    #[must_use]
    pub const fn dynamic_adjustment_enabled(&self) -> bool {
        self.dynamic_adjustment_enabled
    }

    /// Gets the limit for a specific instrument.
    ///
    /// Returns `None` if no specific limit is set for the instrument.
    #[must_use]
    pub fn instrument_limit(&self, symbol: &Symbol) -> Option<Decimal> {
        self.per_instrument_limits.get(symbol).copied()
    }

    /// Sets a per-instrument limit.
    pub fn set_instrument_limit(&mut self, symbol: Symbol, limit: Decimal) {
        self.per_instrument_limits.insert(symbol, limit);
    }

    /// Removes a per-instrument limit.
    pub fn remove_instrument_limit(&mut self, symbol: &Symbol) -> Option<Decimal> {
        self.per_instrument_limits.remove(symbol)
    }

    /// Adjusts the maximum concurrent quotes by a percentage.
    ///
    /// # Arguments
    ///
    /// * `percentage` - Adjustment percentage (e.g., 0.1 for +10%, -0.1 for -10%)
    ///
    /// # Returns
    ///
    /// The new maximum concurrent quotes value.
    #[must_use]
    pub fn adjust_max_quotes(&mut self, percentage: Decimal) -> u32 {
        use rust_decimal::prelude::ToPrimitive;
        let adjustment = Decimal::from(self.max_concurrent_quotes) * percentage;
        let new_value = Decimal::from(self.max_concurrent_quotes) + adjustment;
        // Ensure at least 1 quote allowed, round to nearest integer
        self.max_concurrent_quotes = new_value.round().to_u32().unwrap_or(1).max(1);
        self.max_concurrent_quotes
    }

    /// Adjusts the maximum notional by a percentage.
    ///
    /// # Arguments
    ///
    /// * `percentage` - Adjustment percentage (e.g., 0.1 for +10%, -0.1 for -10%)
    ///
    /// # Returns
    ///
    /// The new maximum notional value.
    #[must_use]
    pub fn adjust_max_notional(&mut self, percentage: Decimal) -> Decimal {
        let adjustment = self.max_notional * percentage;
        self.max_notional = (self.max_notional + adjustment).max(Decimal::ONE);
        self.max_notional
    }
}

impl Default for MmCapacityConfig {
    fn default() -> Self {
        Self {
            max_concurrent_quotes: DEFAULT_MAX_CONCURRENT_QUOTES,
            max_notional: DEFAULT_MAX_NOTIONAL_USD,
            per_instrument_limits: HashMap::new(),
            dynamic_adjustment_enabled: true,
        }
    }
}

/// Builder for [`MmCapacityConfig`].
#[derive(Debug, Clone)]
#[must_use]
pub struct MmCapacityConfigBuilder {
    max_concurrent_quotes: u32,
    max_notional: Decimal,
    per_instrument_limits: HashMap<Symbol, Decimal>,
    dynamic_adjustment_enabled: bool,
}

impl MmCapacityConfigBuilder {
    /// Creates a new builder with default values.
    pub fn new() -> Self {
        Self {
            max_concurrent_quotes: DEFAULT_MAX_CONCURRENT_QUOTES,
            max_notional: DEFAULT_MAX_NOTIONAL_USD,
            per_instrument_limits: HashMap::new(),
            dynamic_adjustment_enabled: true,
        }
    }

    /// Sets the maximum concurrent quotes.
    pub fn max_concurrent_quotes(mut self, value: u32) -> Self {
        self.max_concurrent_quotes = value;
        self
    }

    /// Sets the maximum notional exposure.
    pub fn max_notional(mut self, value: Decimal) -> Self {
        self.max_notional = value;
        self
    }

    /// Adds a per-instrument limit.
    pub fn instrument_limit(mut self, symbol: Symbol, limit: Decimal) -> Self {
        self.per_instrument_limits.insert(symbol, limit);
        self
    }

    /// Sets whether dynamic adjustment is enabled.
    pub fn dynamic_adjustment_enabled(mut self, enabled: bool) -> Self {
        self.dynamic_adjustment_enabled = enabled;
        self
    }

    /// Builds the configuration.
    pub fn build(self) -> MmCapacityConfig {
        MmCapacityConfig {
            max_concurrent_quotes: self.max_concurrent_quotes,
            max_notional: self.max_notional,
            per_instrument_limits: self.per_instrument_limits,
            dynamic_adjustment_enabled: self.dynamic_adjustment_enabled,
        }
    }
}

impl Default for MmCapacityConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// A capacity reservation for a specific RFQ.
///
/// Tracks the notional amount reserved for an RFQ until it expires or completes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapacityReservation {
    /// RFQ identifier.
    rfq_id: RfqId,
    /// Instrument symbol.
    instrument: Symbol,
    /// Reserved notional amount.
    notional: Decimal,
    /// When the reservation was made.
    reserved_at: Timestamp,
}

impl CapacityReservation {
    /// Creates a new capacity reservation.
    #[must_use]
    pub fn new(rfq_id: RfqId, instrument: Symbol, notional: Decimal) -> Self {
        Self {
            rfq_id,
            instrument,
            notional,
            reserved_at: Timestamp::now(),
        }
    }

    /// Returns the RFQ identifier.
    #[inline]
    #[must_use]
    pub fn rfq_id(&self) -> RfqId {
        self.rfq_id
    }

    /// Returns the instrument symbol.
    #[inline]
    #[must_use]
    pub fn instrument(&self) -> &Symbol {
        &self.instrument
    }

    /// Returns the reserved notional amount.
    #[inline]
    #[must_use]
    pub fn notional(&self) -> Decimal {
        self.notional
    }

    /// Returns when the reservation was made.
    #[inline]
    #[must_use]
    pub fn reserved_at(&self) -> Timestamp {
        self.reserved_at
    }
}

impl fmt::Display for CapacityReservation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Reservation {} for {} notional {} at {}",
            self.rfq_id, self.instrument, self.notional, self.reserved_at
        )
    }
}

/// Current capacity state for a market maker.
///
/// Tracks the number of active quotes, current notional exposure,
/// and per-instrument usage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MmCapacityState {
    /// Market maker identifier.
    mm_id: CounterpartyId,
    /// Number of currently active quotes.
    active_quotes: u32,
    /// Current total notional exposure.
    current_notional: Decimal,
    /// Per-instrument current usage (symbol -> current notional).
    per_instrument_usage: HashMap<Symbol, Decimal>,
    /// Active reservations (rfq_id -> reservation details).
    reservations: HashMap<RfqId, CapacityReservation>,
    /// Last updated timestamp.
    updated_at: Timestamp,
}

impl MmCapacityState {
    /// Creates a new empty capacity state for a market maker.
    #[must_use]
    pub fn new(mm_id: CounterpartyId) -> Self {
        Self {
            mm_id,
            active_quotes: 0,
            current_notional: Decimal::ZERO,
            per_instrument_usage: HashMap::new(),
            reservations: HashMap::new(),
            updated_at: Timestamp::now(),
        }
    }

    /// Returns the market maker identifier.
    #[inline]
    #[must_use]
    pub fn mm_id(&self) -> &CounterpartyId {
        &self.mm_id
    }

    /// Returns the number of active quotes.
    #[inline]
    #[must_use]
    pub const fn active_quotes(&self) -> u32 {
        self.active_quotes
    }

    /// Returns the current total notional exposure.
    #[inline]
    #[must_use]
    pub fn current_notional(&self) -> Decimal {
        self.current_notional
    }

    /// Returns the per-instrument usage.
    #[inline]
    #[must_use]
    pub fn per_instrument_usage(&self) -> &HashMap<Symbol, Decimal> {
        &self.per_instrument_usage
    }

    /// Returns the active reservations.
    #[inline]
    #[must_use]
    pub fn reservations(&self) -> &HashMap<RfqId, CapacityReservation> {
        &self.reservations
    }

    /// Returns when the state was last updated.
    #[inline]
    #[must_use]
    pub fn updated_at(&self) -> Timestamp {
        self.updated_at
    }

    /// Gets the current usage for a specific instrument.
    #[must_use]
    pub fn instrument_usage(&self, symbol: &Symbol) -> Decimal {
        self.per_instrument_usage
            .get(symbol)
            .copied()
            .unwrap_or(Decimal::ZERO)
    }

    /// Checks if a reservation exists for the given RFQ.
    #[must_use]
    pub fn has_reservation(&self, rfq_id: &RfqId) -> bool {
        self.reservations.contains_key(rfq_id)
    }

    /// Gets a reservation by RFQ ID.
    #[must_use]
    pub fn get_reservation(&self, rfq_id: &RfqId) -> Option<&CapacityReservation> {
        self.reservations.get(rfq_id)
    }

    /// Adds a reservation and updates usage counters.
    ///
    /// This operation is idempotent - if a reservation with the same RfqId already exists,
    /// it will be replaced and counters adjusted accordingly.
    ///
    /// # Arguments
    ///
    /// * `reservation` - The reservation to add
    ///
    /// # Errors
    ///
    /// Returns `DomainError::CapacityOverflow` if counter arithmetic overflows.
    /// Returns `DomainError::CapacityUnderflow` if counter arithmetic underflows.
    pub fn add_reservation(&mut self, reservation: CapacityReservation) -> DomainResult<()> {
        let rfq_id = reservation.rfq_id();
        let notional = reservation.notional();
        let instrument = reservation.instrument().clone();

        // If reservation already exists, remove old counters first
        if let Some(existing) = self.reservations.get(&rfq_id) {
            let old_notional = existing.notional();
            let old_instrument = existing.instrument();

            // Revert old counters
            self.active_quotes = self.active_quotes.checked_sub(1).ok_or_else(|| {
                DomainError::CapacityUnderflow {
                    field: "active_quotes".to_string(),
                }
            })?;
            self.current_notional = (self.current_notional - old_notional).max(Decimal::ZERO);

            if let Some(usage) = self.per_instrument_usage.get_mut(old_instrument) {
                *usage = (*usage - old_notional).max(Decimal::ZERO);
                if *usage == Decimal::ZERO {
                    self.per_instrument_usage.remove(old_instrument);
                }
            }
        }

        // Update counters with new reservation
        self.active_quotes =
            self.active_quotes
                .checked_add(1)
                .ok_or_else(|| DomainError::CapacityOverflow {
                    field: "active_quotes".to_string(),
                })?;
        self.current_notional = self.current_notional.checked_add(notional).ok_or_else(|| {
            DomainError::CapacityOverflow {
                field: "current_notional".to_string(),
            }
        })?;

        // Update per-instrument usage
        let entry = self
            .per_instrument_usage
            .entry(instrument)
            .or_insert(Decimal::ZERO);
        *entry = entry
            .checked_add(notional)
            .ok_or_else(|| DomainError::CapacityOverflow {
                field: "per_instrument_usage".to_string(),
            })?;

        // Store reservation
        self.reservations.insert(rfq_id, reservation);
        self.updated_at = Timestamp::now();
        Ok(())
    }

    /// Removes a reservation and updates usage counters.
    ///
    /// # Arguments
    ///
    /// * `rfq_id` - The RFQ ID of the reservation to remove
    ///
    /// # Returns
    ///
    /// The removed reservation, if it existed.
    ///
    /// # Errors
    ///
    /// Returns `DomainError::CapacityUnderflow` if counter arithmetic underflows.
    pub fn remove_reservation(
        &mut self,
        rfq_id: &RfqId,
    ) -> DomainResult<Option<CapacityReservation>> {
        if let Some(reservation) = self.reservations.remove(rfq_id) {
            let notional = reservation.notional();
            let instrument = reservation.instrument();

            // Update counters
            self.active_quotes = self.active_quotes.checked_sub(1).ok_or_else(|| {
                DomainError::CapacityUnderflow {
                    field: "active_quotes".to_string(),
                }
            })?;
            self.current_notional = (self.current_notional - notional).max(Decimal::ZERO);

            // Update per-instrument usage
            if let Some(usage) = self.per_instrument_usage.get_mut(instrument) {
                *usage = (*usage - notional).max(Decimal::ZERO);
                if *usage == Decimal::ZERO {
                    self.per_instrument_usage.remove(instrument);
                }
            }

            self.updated_at = Timestamp::now();
            Ok(Some(reservation))
        } else {
            Ok(None)
        }
    }

    /// Clears all reservations and resets counters.
    pub fn clear(&mut self) {
        self.active_quotes = 0;
        self.current_notional = Decimal::ZERO;
        self.per_instrument_usage.clear();
        self.reservations.clear();
        self.updated_at = Timestamp::now();
    }
}

impl fmt::Display for MmCapacityState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "MM {} capacity: {} quotes, {} notional, {} instruments",
            self.mm_id,
            self.active_quotes,
            self.current_notional,
            self.per_instrument_usage.len()
        )
    }
}

/// Result of a capacity check.
///
/// Indicates whether capacity is available or which limit was exceeded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapacityCheckResult {
    /// Capacity is available for the requested operation.
    Available,
    /// At maximum concurrent quotes limit.
    AtMaxQuotes {
        /// Current number of quotes.
        current: u32,
        /// Maximum allowed quotes.
        max: u32,
    },
    /// At maximum notional exposure limit.
    AtMaxNotional {
        /// Current notional exposure.
        current: Decimal,
        /// Maximum allowed notional.
        max: Decimal,
    },
    /// At maximum for this specific instrument.
    AtInstrumentLimit {
        /// The instrument that hit its limit.
        instrument: Symbol,
        /// Current notional for this instrument.
        current: Decimal,
        /// Maximum allowed for this instrument.
        max: Decimal,
    },
}

impl CapacityCheckResult {
    /// Returns `true` if capacity is available.
    #[inline]
    #[must_use]
    pub const fn is_available(&self) -> bool {
        matches!(self, Self::Available)
    }

    /// Returns `true` if capacity is exceeded.
    #[inline]
    #[must_use]
    pub const fn is_exceeded(&self) -> bool {
        !self.is_available()
    }

    /// Returns a human-readable reason for the capacity limit.
    #[must_use]
    pub fn reason(&self) -> String {
        match self {
            Self::Available => "capacity available".to_string(),
            Self::AtMaxQuotes { current, max } => {
                format!("at max quotes: {}/{}", current, max)
            }
            Self::AtMaxNotional { current, max } => {
                format!("at max notional: {}/{}", current, max)
            }
            Self::AtInstrumentLimit {
                instrument,
                current,
                max,
            } => {
                format!(
                    "at instrument limit for {}: {}/{}",
                    instrument, current, max
                )
            }
        }
    }
}

impl fmt::Display for CapacityCheckResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.reason())
    }
}

/// Represents a capacity adjustment made to a market maker's limits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapacityAdjustment {
    /// Previous maximum concurrent quotes.
    pub previous_max_quotes: u32,
    /// New maximum concurrent quotes.
    pub new_max_quotes: u32,
    /// Previous maximum notional.
    pub previous_max_notional: Decimal,
    /// New maximum notional.
    pub new_max_notional: Decimal,
    /// Reason for the adjustment.
    pub reason: String,
}

impl CapacityAdjustment {
    /// Creates a new capacity adjustment record.
    #[must_use]
    pub fn new(
        previous_max_quotes: u32,
        new_max_quotes: u32,
        previous_max_notional: Decimal,
        new_max_notional: Decimal,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            previous_max_quotes,
            new_max_quotes,
            previous_max_notional,
            new_max_notional,
            reason: reason.into(),
        }
    }

    /// Returns `true` if this adjustment increased capacity.
    #[must_use]
    pub fn is_increase(&self) -> bool {
        self.new_max_quotes > self.previous_max_quotes
            || self.new_max_notional > self.previous_max_notional
    }

    /// Returns `true` if this adjustment decreased capacity.
    #[must_use]
    pub fn is_decrease(&self) -> bool {
        self.new_max_quotes < self.previous_max_quotes
            || self.new_max_notional < self.previous_max_notional
    }
}

impl fmt::Display for CapacityAdjustment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Adjustment: quotes {}->{}, notional {}->{} ({})",
            self.previous_max_quotes,
            self.new_max_quotes,
            self.previous_max_notional,
            self.new_max_notional,
            self.reason
        )
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn create_test_symbol() -> Symbol {
        Symbol::new("BTC/USD").unwrap()
    }

    mod capacity_config {
        use super::*;

        #[test]
        fn default_config_has_expected_values() {
            let config = MmCapacityConfig::default();
            assert_eq!(
                config.max_concurrent_quotes(),
                DEFAULT_MAX_CONCURRENT_QUOTES
            );
            assert_eq!(config.max_notional(), DEFAULT_MAX_NOTIONAL_USD);
            assert!(config.per_instrument_limits().is_empty());
            assert!(config.dynamic_adjustment_enabled());
        }

        #[test]
        fn builder_creates_custom_config() {
            let symbol = create_test_symbol();
            let config = MmCapacityConfig::builder()
                .max_concurrent_quotes(100)
                .max_notional(Decimal::from(5_000_000))
                .instrument_limit(symbol.clone(), Decimal::from(1_000_000))
                .dynamic_adjustment_enabled(false)
                .build();

            assert_eq!(config.max_concurrent_quotes(), 100);
            assert_eq!(config.max_notional(), Decimal::from(5_000_000));
            assert_eq!(
                config.instrument_limit(&symbol),
                Some(Decimal::from(1_000_000))
            );
            assert!(!config.dynamic_adjustment_enabled());
        }

        #[test]
        fn adjust_max_quotes_increases() {
            let mut config = MmCapacityConfig::new(100, Decimal::from(1_000_000));
            let new_value = config.adjust_max_quotes(Decimal::from_str_exact("0.1").unwrap());
            assert_eq!(new_value, 110);
        }

        #[test]
        fn adjust_max_quotes_decreases() {
            let mut config = MmCapacityConfig::new(100, Decimal::from(1_000_000));
            let new_value = config.adjust_max_quotes(Decimal::from_str_exact("-0.1").unwrap());
            assert_eq!(new_value, 90);
        }

        #[test]
        fn adjust_max_quotes_minimum_is_one() {
            let mut config = MmCapacityConfig::new(1, Decimal::from(1_000_000));
            let new_value = config.adjust_max_quotes(Decimal::from_str_exact("-0.9").unwrap());
            assert_eq!(new_value, 1);
        }
    }

    mod capacity_state {
        use super::*;

        #[test]
        fn new_state_is_empty() {
            let state = MmCapacityState::new(CounterpartyId::new("mm-test"));
            assert_eq!(state.active_quotes(), 0);
            assert_eq!(state.current_notional(), Decimal::ZERO);
            assert!(state.reservations().is_empty());
        }

        #[test]
        fn add_reservation_updates_counters() {
            let mut state = MmCapacityState::new(CounterpartyId::new("mm-test"));
            let symbol = create_test_symbol();
            let reservation =
                CapacityReservation::new(RfqId::new_v4(), symbol.clone(), Decimal::from(100_000));

            state.add_reservation(reservation).unwrap();

            assert_eq!(state.active_quotes(), 1);
            assert_eq!(state.current_notional(), Decimal::from(100_000));
            assert_eq!(state.instrument_usage(&symbol), Decimal::from(100_000));
        }

        #[test]
        fn remove_reservation_updates_counters() {
            let mut state = MmCapacityState::new(CounterpartyId::new("mm-test"));
            let symbol = create_test_symbol();
            let rfq_id = RfqId::new_v4();
            let reservation =
                CapacityReservation::new(rfq_id, symbol.clone(), Decimal::from(100_000));

            state.add_reservation(reservation).unwrap();
            let removed = state.remove_reservation(&rfq_id).unwrap();

            assert!(removed.is_some());
            assert_eq!(state.active_quotes(), 0);
            assert_eq!(state.current_notional(), Decimal::ZERO);
            assert_eq!(state.instrument_usage(&symbol), Decimal::ZERO);
        }

        #[test]
        fn remove_nonexistent_reservation_returns_none() {
            let mut state = MmCapacityState::new(CounterpartyId::new("mm-test"));
            let removed = state.remove_reservation(&RfqId::new_v4()).unwrap();
            assert!(removed.is_none());
        }

        #[test]
        fn multiple_reservations_accumulate() {
            let mut state = MmCapacityState::new(CounterpartyId::new("mm-test"));
            let symbol = create_test_symbol();

            for _ in 0..3 {
                let reservation = CapacityReservation::new(
                    RfqId::new_v4(),
                    symbol.clone(),
                    Decimal::from(100_000),
                );
                state.add_reservation(reservation).unwrap();
            }

            assert_eq!(state.active_quotes(), 3);
            assert_eq!(state.current_notional(), Decimal::from(300_000));
            assert_eq!(state.instrument_usage(&symbol), Decimal::from(300_000));
        }

        #[test]
        fn clear_resets_all_counters() {
            let mut state = MmCapacityState::new(CounterpartyId::new("mm-test"));
            let symbol = create_test_symbol();
            let reservation =
                CapacityReservation::new(RfqId::new_v4(), symbol, Decimal::from(100_000));
            state.add_reservation(reservation).unwrap();

            state.clear();

            assert_eq!(state.active_quotes(), 0);
            assert_eq!(state.current_notional(), Decimal::ZERO);
            assert!(state.reservations().is_empty());
        }
    }

    mod capacity_check_result {
        use super::*;

        #[test]
        fn available_is_available() {
            let result = CapacityCheckResult::Available;
            assert!(result.is_available());
            assert!(!result.is_exceeded());
        }

        #[test]
        fn at_max_quotes_is_exceeded() {
            let result = CapacityCheckResult::AtMaxQuotes {
                current: 50,
                max: 50,
            };
            assert!(!result.is_available());
            assert!(result.is_exceeded());
        }

        #[test]
        fn reason_formats_correctly() {
            let result = CapacityCheckResult::AtMaxNotional {
                current: Decimal::from(1_000_000),
                max: Decimal::from(1_000_000),
            };
            assert!(result.reason().contains("at max notional"));
        }
    }

    mod capacity_adjustment {
        use super::*;

        #[test]
        fn is_increase_detects_increase() {
            let adjustment = CapacityAdjustment::new(
                50,
                60,
                Decimal::from(1_000_000),
                Decimal::from(1_000_000),
                "performance improvement",
            );
            assert!(adjustment.is_increase());
            assert!(!adjustment.is_decrease());
        }

        #[test]
        fn is_decrease_detects_decrease() {
            let adjustment = CapacityAdjustment::new(
                50,
                40,
                Decimal::from(1_000_000),
                Decimal::from(1_000_000),
                "performance degradation",
            );
            assert!(!adjustment.is_increase());
            assert!(adjustment.is_decrease());
        }
    }
}
