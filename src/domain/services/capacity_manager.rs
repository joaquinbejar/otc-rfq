//! # Capacity Manager Service
//!
//! Domain service for managing market maker capacity limits.
//!
//! The [`CapacityManager`] service provides methods to check, reserve, and release
//! capacity for market makers. It also supports dynamic adjustment of capacity
//! limits based on performance metrics.
//!
//! # Architecture
//!
//! ```text
//! 1. Before RFQ broadcast: check_capacity(mm_id, instrument, notional)
//! 2. On RFQ broadcast: reserve_capacity(mm_id, rfq_id, instrument, notional)
//! 3. On RFQ expiry/completion: release_capacity(mm_id, rfq_id)
//! 4. Periodically: adjust_capacity(mm_id, metrics)
//! ```
//!
//! # Example
//!
//! ```ignore
//! use otc_rfq::domain::services::capacity_manager::{CapacityManager, CapacityManagerConfig};
//!
//! let manager = CapacityManager::new(config, repository);
//!
//! // Check if MM has capacity
//! let result = manager.check_capacity(&mm_id, &symbol, notional).await?;
//! if result.is_available() {
//!     // Reserve capacity
//!     manager.reserve_capacity(&mm_id, rfq_id, &symbol, notional).await?;
//! }
//! ```

use crate::domain::entities::mm_capacity::{
    CapacityAdjustment, CapacityCheckResult, CapacityReservation, MmCapacityConfig, MmCapacityState,
};
use crate::domain::entities::mm_performance::MmPerformanceMetrics;
use crate::domain::errors::{DomainError, DomainResult};
use crate::domain::events::capacity_events::{
    CapacityAdjusted, CapacityReleased, CapacityReserved, MmExcludedForCapacity,
};
use crate::domain::value_objects::CounterpartyId;
use crate::domain::value_objects::ids::RfqId;
use crate::domain::value_objects::symbol::Symbol;
use async_trait::async_trait;
use dashmap::DashMap;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Default performance threshold for capacity increase (response rate).
pub const DEFAULT_INCREASE_THRESHOLD: f64 = 0.90;

/// Default performance threshold for capacity decrease (response rate).
pub const DEFAULT_DECREASE_THRESHOLD: f64 = 0.70;

/// Default adjustment percentage (10%).
pub const DEFAULT_ADJUSTMENT_PERCENTAGE: &str = "0.10";

/// Configuration for the capacity manager.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapacityManagerConfig {
    /// Default capacity config for new MMs.
    pub default_capacity: MmCapacityConfig,
    /// Performance threshold for capacity increase (0.0-1.0).
    pub increase_threshold: f64,
    /// Performance threshold for capacity decrease (0.0-1.0).
    pub decrease_threshold: f64,
    /// Percentage to adjust capacity by.
    pub adjustment_percentage: Decimal,
}

impl CapacityManagerConfig {
    /// Creates a new capacity manager configuration.
    #[must_use]
    pub fn new(
        default_capacity: MmCapacityConfig,
        increase_threshold: f64,
        decrease_threshold: f64,
        adjustment_percentage: Decimal,
    ) -> Self {
        Self {
            default_capacity,
            increase_threshold,
            decrease_threshold,
            adjustment_percentage,
        }
    }

    /// Creates a configuration suitable for testing.
    #[must_use]
    pub fn for_testing() -> Self {
        Self {
            default_capacity: MmCapacityConfig::builder()
                .max_concurrent_quotes(10)
                .max_notional(Decimal::from(1_000_000))
                .build(),
            increase_threshold: DEFAULT_INCREASE_THRESHOLD,
            decrease_threshold: DEFAULT_DECREASE_THRESHOLD,
            adjustment_percentage: Decimal::from_str_exact(DEFAULT_ADJUSTMENT_PERCENTAGE)
                .unwrap_or(Decimal::ONE / Decimal::from(10)),
        }
    }
}

impl Default for CapacityManagerConfig {
    fn default() -> Self {
        Self {
            default_capacity: MmCapacityConfig::default(),
            increase_threshold: DEFAULT_INCREASE_THRESHOLD,
            decrease_threshold: DEFAULT_DECREASE_THRESHOLD,
            adjustment_percentage: Decimal::from_str_exact(DEFAULT_ADJUSTMENT_PERCENTAGE)
                .unwrap_or(Decimal::ONE / Decimal::from(10)),
        }
    }
}

/// Repository trait for MM capacity data.
///
/// Implementations may use in-memory storage, Redis, PostgreSQL, or any
/// other backend suitable for capacity state management.
#[async_trait]
pub trait MmCapacityRepository: Send + Sync + fmt::Debug {
    /// Gets the capacity configuration for a market maker.
    ///
    /// Returns `None` if no configuration exists for this MM.
    ///
    /// # Errors
    ///
    /// Returns an error if the repository operation fails.
    async fn get_config(&self, mm_id: &CounterpartyId) -> DomainResult<Option<MmCapacityConfig>>;

    /// Saves the capacity configuration for a market maker.
    ///
    /// # Errors
    ///
    /// Returns an error if the repository operation fails.
    async fn save_config(
        &self,
        mm_id: &CounterpartyId,
        config: &MmCapacityConfig,
    ) -> DomainResult<()>;

    /// Gets the current capacity state for a market maker.
    ///
    /// Returns a new empty state if no state exists.
    ///
    /// # Errors
    ///
    /// Returns an error if the repository operation fails.
    async fn get_state(&self, mm_id: &CounterpartyId) -> DomainResult<MmCapacityState>;

    /// Saves the capacity state for a market maker.
    ///
    /// # Errors
    ///
    /// Returns an error if the repository operation fails.
    async fn save_state(&self, state: &MmCapacityState) -> DomainResult<()>;

    /// Returns all distinct market maker IDs that have capacity state.
    ///
    /// # Errors
    ///
    /// Returns an error if the repository operation fails.
    async fn get_all_mm_ids(&self) -> DomainResult<Vec<CounterpartyId>>;
}

/// Service for managing market maker capacity.
///
/// Provides methods to check, reserve, and release capacity for market makers,
/// as well as dynamic adjustment based on performance metrics.
///
/// # Concurrency
///
/// The manager uses per-MM locks to ensure atomic check-and-reserve operations
/// **within a single process/instance**. This prevents race conditions where
/// concurrent callers could both pass capacity checks and then both reserve,
/// exceeding limits.
///
/// **Note:** Cross-process atomicity requires external coordination (e.g., database
/// transactions or distributed locks) which is the responsibility of the repository
/// implementation.
pub struct CapacityManager<R: MmCapacityRepository> {
    /// Configuration for the capacity manager.
    config: CapacityManagerConfig,
    /// Repository for capacity data.
    repository: Arc<R>,
    /// Per-MM locks to serialize capacity operations.
    mm_locks: DashMap<CounterpartyId, Arc<Mutex<()>>>,
}

impl<R: MmCapacityRepository> CapacityManager<R> {
    /// Creates a new capacity manager.
    #[must_use]
    pub fn new(config: CapacityManagerConfig, repository: Arc<R>) -> Self {
        Self {
            config,
            repository,
            mm_locks: DashMap::new(),
        }
    }

    /// Gets or creates a lock for the given market maker.
    ///
    /// This ensures that capacity operations for the same MM are serialized.
    fn get_mm_lock(&self, mm_id: &CounterpartyId) -> Arc<Mutex<()>> {
        self.mm_locks
            .entry(mm_id.clone())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }

    /// Returns the configuration.
    #[inline]
    #[must_use]
    pub fn config(&self) -> &CapacityManagerConfig {
        &self.config
    }

    /// Checks if a market maker has capacity for a new RFQ.
    ///
    /// # Arguments
    ///
    /// * `mm_id` - Market maker identifier
    /// * `instrument` - Instrument symbol
    /// * `notional` - Notional amount to check
    ///
    /// # Returns
    ///
    /// A [`CapacityCheckResult`] indicating whether capacity is available
    /// or which limit would be exceeded.
    ///
    /// # Errors
    ///
    /// Returns an error if the repository operation fails.
    pub async fn check_capacity(
        &self,
        mm_id: &CounterpartyId,
        instrument: &Symbol,
        notional: Decimal,
    ) -> DomainResult<CapacityCheckResult> {
        if notional <= Decimal::ZERO {
            return Err(DomainError::ValidationError(
                "notional must be positive".to_string(),
            ));
        }

        let config = self.get_or_default_config(mm_id).await?;
        let state = self.repository.get_state(mm_id).await?;

        // Check concurrent quotes limit
        if state.active_quotes() >= config.max_concurrent_quotes() {
            return Ok(CapacityCheckResult::AtMaxQuotes {
                current: state.active_quotes(),
                max: config.max_concurrent_quotes(),
            });
        }

        // Check total notional limit
        let new_total = state.current_notional() + notional;
        if new_total > config.max_notional() {
            return Ok(CapacityCheckResult::AtMaxNotional {
                current: state.current_notional(),
                max: config.max_notional(),
            });
        }

        // Check per-instrument limit
        if let Some(instrument_limit) = config.instrument_limit(instrument) {
            let current_instrument_usage = state.instrument_usage(instrument);
            let new_instrument_total = current_instrument_usage + notional;
            if new_instrument_total > instrument_limit {
                return Ok(CapacityCheckResult::AtInstrumentLimit {
                    instrument: instrument.clone(),
                    current: current_instrument_usage,
                    max: instrument_limit,
                });
            }
        }

        Ok(CapacityCheckResult::Available)
    }

    /// Reserves capacity for an RFQ broadcast.
    ///
    /// This operation is atomic per-MM: the capacity check and reservation are
    /// performed under a lock to prevent race conditions where concurrent callers
    /// could both pass the check and then both reserve, exceeding limits.
    ///
    /// # Arguments
    ///
    /// * `mm_id` - Market maker identifier
    /// * `rfq_id` - RFQ identifier
    /// * `instrument` - Instrument symbol
    /// * `notional` - Notional amount to reserve
    ///
    /// # Returns
    ///
    /// A [`CapacityReserved`] event on success.
    ///
    /// # Errors
    ///
    /// Returns `CapacityExceeded` if the MM is at capacity.
    pub async fn reserve_capacity(
        &self,
        mm_id: &CounterpartyId,
        rfq_id: RfqId,
        instrument: &Symbol,
        notional: Decimal,
    ) -> DomainResult<CapacityReserved> {
        // Validate arguments before acquiring lock to reduce contention
        if notional <= Decimal::ZERO {
            return Err(DomainError::ValidationError(
                "notional must be positive".to_string(),
            ));
        }

        // Acquire per-MM lock to ensure atomic check-and-reserve
        let lock = self.get_mm_lock(mm_id);
        let _guard = lock.lock().await;

        // Check capacity under lock (validation is duplicated but harmless)
        let check_result = self.check_capacity(mm_id, instrument, notional).await?;
        if check_result.is_exceeded() {
            return Err(DomainError::CapacityExceeded {
                mm_id: mm_id.to_string(),
                reason: check_result.reason(),
            });
        }

        // Get current state and add reservation (still under lock)
        let mut state = self.repository.get_state(mm_id).await?;
        let reservation = CapacityReservation::new(rfq_id, instrument.clone(), notional);
        state.add_reservation(reservation)?;

        // Save updated state
        self.repository.save_state(&state).await?;

        // Create and return event
        Ok(CapacityReserved::new(
            mm_id.clone(),
            rfq_id,
            instrument.clone(),
            notional,
        ))
    }

    /// Releases capacity when an RFQ expires or completes.
    ///
    /// This operation is serialized per-MM to ensure consistency with
    /// concurrent reserve operations.
    ///
    /// # Arguments
    ///
    /// * `mm_id` - Market maker identifier
    /// * `rfq_id` - RFQ identifier
    ///
    /// # Returns
    ///
    /// A [`CapacityReleased`] event on success, or `None` if no reservation existed.
    ///
    /// # Errors
    ///
    /// Returns an error if the repository operation fails.
    pub async fn release_capacity(
        &self,
        mm_id: &CounterpartyId,
        rfq_id: &RfqId,
        reason: &str,
    ) -> DomainResult<Option<CapacityReleased>> {
        // Acquire per-MM lock for consistency with reserve operations
        let lock = self.get_mm_lock(mm_id);
        let _guard = lock.lock().await;

        let mut state = self.repository.get_state(mm_id).await?;

        if let Some(reservation) = state.remove_reservation(rfq_id)? {
            self.repository.save_state(&state).await?;

            Ok(Some(CapacityReleased::new(
                mm_id.clone(),
                *rfq_id,
                reservation.notional(),
                reason,
            )))
        } else {
            Ok(None)
        }
    }

    /// Adjusts capacity limits based on performance metrics.
    ///
    /// # Arguments
    ///
    /// * `mm_id` - Market maker identifier
    /// * `metrics` - Performance metrics for the MM
    ///
    /// # Returns
    ///
    /// A [`CapacityAdjusted`] event if adjustment was made, `None` otherwise.
    ///
    /// # Errors
    ///
    /// Returns an error if the repository operation fails.
    pub async fn adjust_capacity(
        &self,
        mm_id: &CounterpartyId,
        metrics: &MmPerformanceMetrics,
    ) -> DomainResult<Option<CapacityAdjusted>> {
        let mut config = self.get_or_default_config(mm_id).await?;

        if !config.dynamic_adjustment_enabled() {
            return Ok(None);
        }

        // Only adjust if there's sufficient data (response_rate_pct returns None when no RFQs sent)
        let Some(response_rate_pct) = metrics.response_rate_pct() else {
            return Ok(None);
        };

        let response_rate = response_rate_pct / 100.0;
        let previous_max_quotes = config.max_concurrent_quotes();
        let previous_max_notional = config.max_notional();

        let adjustment_made = if response_rate >= self.config.increase_threshold {
            // High performance - increase capacity
            let _ = config.adjust_max_quotes(self.config.adjustment_percentage);
            let _ = config.adjust_max_notional(self.config.adjustment_percentage);
            true
        } else if response_rate <= self.config.decrease_threshold {
            // Low performance - decrease capacity
            let neg_adjustment = -self.config.adjustment_percentage;
            let _ = config.adjust_max_quotes(neg_adjustment);
            let _ = config.adjust_max_notional(neg_adjustment);
            true
        } else {
            false
        };

        if adjustment_made {
            self.repository.save_config(mm_id, &config).await?;

            let reason = if response_rate >= self.config.increase_threshold {
                format!(
                    "high performance (response rate: {:.1}%)",
                    response_rate * 100.0
                )
            } else {
                format!(
                    "low performance (response rate: {:.1}%)",
                    response_rate * 100.0
                )
            };

            let adjustment = CapacityAdjustment::new(
                previous_max_quotes,
                config.max_concurrent_quotes(),
                previous_max_notional,
                config.max_notional(),
                reason,
            );

            Ok(Some(CapacityAdjusted::new(mm_id.clone(), adjustment)))
        } else {
            Ok(None)
        }
    }

    /// Gets the current capacity state for a market maker.
    ///
    /// # Errors
    ///
    /// Returns an error if the repository operation fails.
    pub async fn get_state(&self, mm_id: &CounterpartyId) -> DomainResult<MmCapacityState> {
        self.repository.get_state(mm_id).await
    }

    /// Gets the capacity config for a market maker.
    ///
    /// Returns the default config if none is set.
    ///
    /// # Errors
    ///
    /// Returns an error if the repository operation fails.
    pub async fn get_config(&self, mm_id: &CounterpartyId) -> DomainResult<MmCapacityConfig> {
        self.get_or_default_config(mm_id).await
    }

    /// Updates the capacity config for a market maker.
    ///
    /// # Errors
    ///
    /// Returns an error if the repository operation fails.
    pub async fn update_config(
        &self,
        mm_id: &CounterpartyId,
        config: MmCapacityConfig,
    ) -> DomainResult<()> {
        self.repository.save_config(mm_id, &config).await
    }

    /// Creates an exclusion event for an MM that was excluded due to capacity.
    #[must_use]
    pub fn create_exclusion_event(
        &self,
        mm_id: &CounterpartyId,
        rfq_id: RfqId,
        check_result: &CapacityCheckResult,
    ) -> MmExcludedForCapacity {
        MmExcludedForCapacity::new(mm_id.clone(), rfq_id, check_result)
    }

    /// Gets the config for an MM, or returns the default if none exists.
    async fn get_or_default_config(
        &self,
        mm_id: &CounterpartyId,
    ) -> DomainResult<MmCapacityConfig> {
        match self.repository.get_config(mm_id).await? {
            Some(config) => Ok(config),
            None => Ok(self.config.default_capacity.clone()),
        }
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::useless_vec,
    clippy::panic,
    clippy::needless_borrows_for_generic_args
)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tokio::sync::RwLock;

    /// In-memory implementation of MmCapacityRepository for testing.
    #[derive(Debug, Default)]
    struct InMemoryCapacityRepository {
        configs: RwLock<HashMap<CounterpartyId, MmCapacityConfig>>,
        states: RwLock<HashMap<CounterpartyId, MmCapacityState>>,
    }

    impl InMemoryCapacityRepository {
        fn new() -> Self {
            Self::default()
        }
    }

    #[async_trait]
    impl MmCapacityRepository for InMemoryCapacityRepository {
        async fn get_config(
            &self,
            mm_id: &CounterpartyId,
        ) -> DomainResult<Option<MmCapacityConfig>> {
            let configs = self.configs.read().await;
            Ok(configs.get(mm_id).cloned())
        }

        async fn save_config(
            &self,
            mm_id: &CounterpartyId,
            config: &MmCapacityConfig,
        ) -> DomainResult<()> {
            let mut configs = self.configs.write().await;
            configs.insert(mm_id.clone(), config.clone());
            Ok(())
        }

        async fn get_state(&self, mm_id: &CounterpartyId) -> DomainResult<MmCapacityState> {
            let states = self.states.read().await;
            Ok(states
                .get(mm_id)
                .cloned()
                .unwrap_or_else(|| MmCapacityState::new(mm_id.clone())))
        }

        async fn save_state(&self, state: &MmCapacityState) -> DomainResult<()> {
            let mut states = self.states.write().await;
            states.insert(state.mm_id().clone(), state.clone());
            Ok(())
        }

        async fn get_all_mm_ids(&self) -> DomainResult<Vec<CounterpartyId>> {
            let states = self.states.read().await;
            Ok(states.keys().cloned().collect())
        }
    }

    fn create_test_manager() -> CapacityManager<InMemoryCapacityRepository> {
        let config = CapacityManagerConfig::for_testing();
        let repository = Arc::new(InMemoryCapacityRepository::new());
        CapacityManager::new(config, repository)
    }

    fn create_test_symbol() -> Symbol {
        Symbol::new("BTC/USD").unwrap()
    }

    mod check_capacity {
        use super::*;

        #[tokio::test]
        async fn returns_available_when_under_limits() {
            let manager = create_test_manager();
            let mm_id = CounterpartyId::new("mm-test");
            let symbol = create_test_symbol();

            let result = manager
                .check_capacity(&mm_id, &symbol, Decimal::from(100_000))
                .await
                .unwrap();

            assert!(result.is_available());
        }

        #[tokio::test]
        async fn returns_at_max_quotes_when_limit_reached() {
            let manager = create_test_manager();
            let mm_id = CounterpartyId::new("mm-test");
            let symbol = create_test_symbol();

            // Fill up to max quotes (10 in test config)
            for _ in 0..10 {
                manager
                    .reserve_capacity(&mm_id, RfqId::new_v4(), &symbol, Decimal::from(1_000))
                    .await
                    .unwrap();
            }

            let result = manager
                .check_capacity(&mm_id, &symbol, Decimal::from(1_000))
                .await
                .unwrap();

            assert!(matches!(result, CapacityCheckResult::AtMaxQuotes { .. }));
        }

        #[tokio::test]
        async fn returns_at_max_notional_when_limit_reached() {
            let manager = create_test_manager();
            let mm_id = CounterpartyId::new("mm-test");
            let symbol = create_test_symbol();

            // Reserve most of the notional (1M in test config)
            manager
                .reserve_capacity(&mm_id, RfqId::new_v4(), &symbol, Decimal::from(900_000))
                .await
                .unwrap();

            let result = manager
                .check_capacity(&mm_id, &symbol, Decimal::from(200_000))
                .await
                .unwrap();

            assert!(matches!(result, CapacityCheckResult::AtMaxNotional { .. }));
        }

        #[tokio::test]
        async fn returns_at_instrument_limit_when_limit_reached() {
            let config = CapacityManagerConfig {
                default_capacity: MmCapacityConfig::builder()
                    .max_concurrent_quotes(100)
                    .max_notional(Decimal::from(10_000_000))
                    .instrument_limit(create_test_symbol(), Decimal::from(100_000))
                    .build(),
                ..CapacityManagerConfig::for_testing()
            };
            let repository = Arc::new(InMemoryCapacityRepository::new());
            let manager = CapacityManager::new(config, repository);

            let mm_id = CounterpartyId::new("mm-test");
            let symbol = create_test_symbol();

            // Reserve up to instrument limit
            manager
                .reserve_capacity(&mm_id, RfqId::new_v4(), &symbol, Decimal::from(90_000))
                .await
                .unwrap();

            let result = manager
                .check_capacity(&mm_id, &symbol, Decimal::from(20_000))
                .await
                .unwrap();

            assert!(matches!(
                result,
                CapacityCheckResult::AtInstrumentLimit { .. }
            ));
        }
    }

    mod reserve_capacity {
        use super::*;

        #[tokio::test]
        async fn reserves_capacity_successfully() {
            let manager = create_test_manager();
            let mm_id = CounterpartyId::new("mm-test");
            let symbol = create_test_symbol();
            let rfq_id = RfqId::new_v4();

            let event = manager
                .reserve_capacity(&mm_id, rfq_id, &symbol, Decimal::from(100_000))
                .await
                .unwrap();

            assert_eq!(event.mm_id(), &mm_id);
            assert_eq!(event.rfq_id_value(), rfq_id);
            assert_eq!(event.notional(), Decimal::from(100_000));

            // Verify state was updated
            let state = manager.get_state(&mm_id).await.unwrap();
            assert_eq!(state.active_quotes(), 1);
            assert_eq!(state.current_notional(), Decimal::from(100_000));
        }

        #[tokio::test]
        async fn fails_when_at_capacity() {
            let manager = create_test_manager();
            let mm_id = CounterpartyId::new("mm-test");
            let symbol = create_test_symbol();

            // Fill up to max quotes
            for _ in 0..10 {
                manager
                    .reserve_capacity(&mm_id, RfqId::new_v4(), &symbol, Decimal::from(1_000))
                    .await
                    .unwrap();
            }

            let result = manager
                .reserve_capacity(&mm_id, RfqId::new_v4(), &symbol, Decimal::from(1_000))
                .await;

            assert!(matches!(result, Err(DomainError::CapacityExceeded { .. })));
        }
    }

    mod release_capacity {
        use super::*;

        #[tokio::test]
        async fn releases_existing_reservation() {
            let manager = create_test_manager();
            let mm_id = CounterpartyId::new("mm-test");
            let symbol = create_test_symbol();
            let rfq_id = RfqId::new_v4();

            manager
                .reserve_capacity(&mm_id, rfq_id, &symbol, Decimal::from(100_000))
                .await
                .unwrap();

            let event = manager
                .release_capacity(&mm_id, &rfq_id, "completed")
                .await
                .unwrap();

            assert!(event.is_some());
            let event = event.unwrap();
            assert_eq!(event.notional(), Decimal::from(100_000));
            assert_eq!(event.reason(), "completed");

            // Verify state was updated
            let state = manager.get_state(&mm_id).await.unwrap();
            assert_eq!(state.active_quotes(), 0);
            assert_eq!(state.current_notional(), Decimal::ZERO);
        }

        #[tokio::test]
        async fn returns_none_for_nonexistent_reservation() {
            let manager = create_test_manager();
            let mm_id = CounterpartyId::new("mm-test");
            let rfq_id = RfqId::new_v4();

            let event = manager
                .release_capacity(&mm_id, &rfq_id, "completed")
                .await
                .unwrap();

            assert!(event.is_none());
        }
    }

    mod adjust_capacity {
        use super::*;
        use crate::domain::entities::mm_performance::{MmPerformanceEvent, MmPerformanceEventKind};
        use crate::domain::value_objects::timestamp::Timestamp;

        fn create_metrics(response_rate: f64) -> MmPerformanceMetrics {
            let mm_id = CounterpartyId::new("mm-test");
            let now = Timestamp::now();
            let week_ago = now.sub_secs(86400 * 7);

            // Create events to achieve the desired response rate
            // response_rate = quotes_received / rfqs_sent
            let rfqs_sent = 100usize;
            let quotes_received = (rfqs_sent as f64 * response_rate) as usize;

            let mut events = Vec::new();

            // Add RFQ sent events
            for _ in 0..rfqs_sent {
                events.push(MmPerformanceEvent::new(
                    mm_id.clone(),
                    MmPerformanceEventKind::RfqSent,
                    now,
                ));
            }

            // Add quote received events
            for _ in 0..quotes_received {
                events.push(MmPerformanceEvent::new(
                    mm_id.clone(),
                    MmPerformanceEventKind::QuoteReceived {
                        response_time_ms: 100,
                        rank: 1,
                    },
                    now,
                ));
            }

            MmPerformanceMetrics::compute(&mm_id, &events, week_ago, now)
        }

        #[tokio::test]
        async fn increases_capacity_for_high_performance() {
            let manager = create_test_manager();
            let mm_id = CounterpartyId::new("mm-test");
            let metrics = create_metrics(0.95); // 95% response rate

            let event = manager.adjust_capacity(&mm_id, &metrics).await.unwrap();

            assert!(event.is_some());
            let event = event.unwrap();
            assert!(event.adjustment().is_increase());
        }

        #[tokio::test]
        async fn decreases_capacity_for_low_performance() {
            let manager = create_test_manager();
            let mm_id = CounterpartyId::new("mm-test");
            let metrics = create_metrics(0.60); // 60% response rate

            let event = manager.adjust_capacity(&mm_id, &metrics).await.unwrap();

            assert!(event.is_some());
            let event = event.unwrap();
            assert!(event.adjustment().is_decrease());
        }

        #[tokio::test]
        async fn no_adjustment_for_normal_performance() {
            let manager = create_test_manager();
            let mm_id = CounterpartyId::new("mm-test");
            let metrics = create_metrics(0.80); // 80% response rate (between thresholds)

            let event = manager.adjust_capacity(&mm_id, &metrics).await.unwrap();

            assert!(event.is_none());
        }

        #[tokio::test]
        async fn no_adjustment_when_disabled() {
            let config = CapacityManagerConfig {
                default_capacity: MmCapacityConfig::builder()
                    .dynamic_adjustment_enabled(false)
                    .build(),
                ..CapacityManagerConfig::for_testing()
            };
            let repository = Arc::new(InMemoryCapacityRepository::new());
            let manager = CapacityManager::new(config, repository);

            let mm_id = CounterpartyId::new("mm-test");
            let metrics = create_metrics(0.95);

            let event = manager.adjust_capacity(&mm_id, &metrics).await.unwrap();

            assert!(event.is_none());
        }
    }

    mod config_management {
        use super::*;

        #[tokio::test]
        async fn get_config_returns_default_when_not_set() {
            let manager = create_test_manager();
            let mm_id = CounterpartyId::new("mm-test");

            let config = manager.get_config(&mm_id).await.unwrap();

            assert_eq!(
                config.max_concurrent_quotes(),
                manager.config().default_capacity.max_concurrent_quotes()
            );
        }

        #[tokio::test]
        async fn update_config_persists() {
            let manager = create_test_manager();
            let mm_id = CounterpartyId::new("mm-test");

            let custom_config = MmCapacityConfig::builder()
                .max_concurrent_quotes(200)
                .max_notional(Decimal::from(50_000_000))
                .build();

            manager
                .update_config(&mm_id, custom_config.clone())
                .await
                .unwrap();

            let retrieved = manager.get_config(&mm_id).await.unwrap();
            assert_eq!(retrieved.max_concurrent_quotes(), 200);
        }
    }

    mod concurrency {
        use super::*;
        use std::sync::atomic::{AtomicU32, Ordering};

        /// Creates a manager with a small max_concurrent_quotes limit for testing.
        fn create_limited_manager() -> Arc<CapacityManager<InMemoryCapacityRepository>> {
            let config = CapacityManagerConfig {
                default_capacity: MmCapacityConfig::builder()
                    .max_concurrent_quotes(5) // Small limit to trigger contention
                    .max_notional(Decimal::from(10_000_000))
                    .build(),
                ..CapacityManagerConfig::for_testing()
            };
            let repository = Arc::new(InMemoryCapacityRepository::new());
            Arc::new(CapacityManager::new(config, repository))
        }

        #[tokio::test]
        async fn concurrent_reservations_respect_limit() {
            let manager = create_limited_manager();
            let mm_id = CounterpartyId::new("mm-test");
            let symbol = create_test_symbol();

            // Spawn 20 concurrent tasks trying to reserve capacity
            // Only 5 should succeed (max_concurrent_quotes = 5)
            let success_count = Arc::new(AtomicU32::new(0));
            let failure_count = Arc::new(AtomicU32::new(0));

            let mut handles = Vec::new();
            for i in 0..20 {
                let manager = Arc::clone(&manager);
                let mm_id = mm_id.clone();
                let symbol = symbol.clone();
                let success_count = Arc::clone(&success_count);
                let failure_count = Arc::clone(&failure_count);

                handles.push(tokio::spawn(async move {
                    let rfq_id = RfqId::new_v4();
                    let result = manager
                        .reserve_capacity(&mm_id, rfq_id, &symbol, Decimal::from(1_000 * (i + 1)))
                        .await;

                    match result {
                        Ok(_) => {
                            success_count.fetch_add(1, Ordering::SeqCst);
                        }
                        Err(DomainError::CapacityExceeded { .. }) => {
                            failure_count.fetch_add(1, Ordering::SeqCst);
                        }
                        Err(e) => panic!("Unexpected error: {e:?}"),
                    }
                }));
            }

            // Wait for all tasks to complete
            for handle in handles {
                handle.await.unwrap();
            }

            // Exactly 5 should succeed, 15 should fail
            assert_eq!(success_count.load(Ordering::SeqCst), 5);
            assert_eq!(failure_count.load(Ordering::SeqCst), 15);

            // Verify final state
            let state = manager.get_state(&mm_id).await.unwrap();
            assert_eq!(state.active_quotes(), 5);
        }

        #[tokio::test]
        async fn different_mms_can_reserve_concurrently() {
            let manager = create_limited_manager();
            let symbol = create_test_symbol();

            // Spawn tasks for different MMs - all should succeed
            let mut handles = Vec::new();
            for i in 0..10 {
                let manager = Arc::clone(&manager);
                let symbol = symbol.clone();
                let mm_id = CounterpartyId::new(&format!("mm-{i}"));

                handles.push(tokio::spawn(async move {
                    let rfq_id = RfqId::new_v4();
                    manager
                        .reserve_capacity(&mm_id, rfq_id, &symbol, Decimal::from(100_000))
                        .await
                }));
            }

            // All should succeed since they're different MMs
            for handle in handles {
                let result = handle.await.unwrap();
                assert!(result.is_ok());
            }
        }
    }
}
