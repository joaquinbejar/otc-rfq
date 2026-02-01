//! # Venue Registry
//!
//! Registry for managing venue adapters.
//!
//! This module provides the [`VenueRegistry`] for registering, discovering,
//! and managing venue adapters. It supports filtering by availability and
//! instrument support.
//!
//! # Thread Safety
//!
//! The registry is thread-safe and can be shared across async tasks using
//! `Arc<VenueRegistry>`.
//!
//! # Examples
//!
//! ```ignore
//! use otc_rfq::infrastructure::venues::registry::VenueRegistry;
//!
//! let registry = VenueRegistry::new();
//! registry.register(my_adapter);
//!
//! // Get a specific adapter
//! if let Some(adapter) = registry.get(&venue_id) {
//!     // Use adapter
//! }
//!
//! // Get all available adapters
//! let available = registry.get_available().await;
//! ```

use crate::domain::value_objects::{Instrument, VenueId};
use crate::infrastructure::venues::traits::VenueAdapter;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Configuration for a registered venue.
#[derive(Debug, Clone)]
pub struct VenueConfig {
    /// Whether this venue is enabled.
    enabled: bool,
    /// Priority for venue selection (lower is higher priority).
    priority: u32,
    /// Supported instruments (empty means all instruments).
    supported_instruments: Vec<Instrument>,
}

impl VenueConfig {
    /// Creates a new venue configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            enabled: true,
            priority: 100,
            supported_instruments: Vec::new(),
        }
    }

    /// Creates a disabled configuration.
    #[must_use]
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            priority: 100,
            supported_instruments: Vec::new(),
        }
    }

    /// Sets whether the venue is enabled.
    #[must_use]
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Sets the priority.
    #[must_use]
    pub fn with_priority(mut self, priority: u32) -> Self {
        self.priority = priority;
        self
    }

    /// Sets the supported instruments.
    #[must_use]
    pub fn with_instruments(mut self, instruments: Vec<Instrument>) -> Self {
        self.supported_instruments = instruments;
        self
    }

    /// Returns whether the venue is enabled.
    #[inline]
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Returns the priority.
    #[inline]
    #[must_use]
    pub fn priority(&self) -> u32 {
        self.priority
    }

    /// Returns the supported instruments.
    #[inline]
    #[must_use]
    pub fn supported_instruments(&self) -> &[Instrument] {
        &self.supported_instruments
    }

    /// Returns true if the venue supports the given instrument.
    ///
    /// If no instruments are configured, returns true (supports all).
    #[must_use]
    pub fn supports_instrument(&self, instrument: &Instrument) -> bool {
        self.supported_instruments.is_empty()
            || self.supported_instruments.iter().any(|i| i == instrument)
    }
}

impl Default for VenueConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Entry in the venue registry.
struct RegistryEntry {
    adapter: Arc<dyn VenueAdapter>,
    config: VenueConfig,
}

/// Registry for managing venue adapters.
///
/// Provides thread-safe registration and discovery of venue adapters.
/// Supports filtering by availability and instrument support.
///
/// # Examples
///
/// ```ignore
/// use otc_rfq::infrastructure::venues::registry::VenueRegistry;
///
/// let registry = VenueRegistry::new();
///
/// // Register adapters
/// registry.register(adapter1);
/// registry.register_with_config(adapter2, VenueConfig::new().with_priority(50));
///
/// // Get available adapters
/// let available = registry.get_available().await;
/// ```
#[derive(Debug)]
pub struct VenueRegistry {
    /// Registered adapters by venue ID.
    adapters: RwLock<HashMap<VenueId, RegistryEntry>>,
}

impl VenueRegistry {
    /// Creates a new empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            adapters: RwLock::new(HashMap::new()),
        }
    }

    /// Registers a venue adapter with default configuration.
    ///
    /// If an adapter with the same venue ID is already registered,
    /// it will be replaced.
    pub async fn register(&self, adapter: Arc<dyn VenueAdapter>) {
        self.register_with_config(adapter, VenueConfig::default())
            .await;
    }

    /// Registers a venue adapter with custom configuration.
    ///
    /// If an adapter with the same venue ID is already registered,
    /// it will be replaced.
    pub async fn register_with_config(&self, adapter: Arc<dyn VenueAdapter>, config: VenueConfig) {
        let venue_id = adapter.venue_id().clone();
        let entry = RegistryEntry { adapter, config };

        let mut adapters = self.adapters.write().await;
        adapters.insert(venue_id, entry);
    }

    /// Unregisters a venue adapter.
    ///
    /// Returns true if the adapter was found and removed.
    pub async fn unregister(&self, venue_id: &VenueId) -> bool {
        let mut adapters = self.adapters.write().await;
        adapters.remove(venue_id).is_some()
    }

    /// Gets a venue adapter by ID.
    ///
    /// Returns None if the adapter is not registered.
    pub async fn get(&self, venue_id: &VenueId) -> Option<Arc<dyn VenueAdapter>> {
        let adapters = self.adapters.read().await;
        adapters.get(venue_id).map(|e| Arc::clone(&e.adapter))
    }

    /// Gets the configuration for a venue.
    ///
    /// Returns None if the venue is not registered.
    pub async fn get_config(&self, venue_id: &VenueId) -> Option<VenueConfig> {
        let adapters = self.adapters.read().await;
        adapters.get(venue_id).map(|e| e.config.clone())
    }

    /// Updates the configuration for a venue.
    ///
    /// Returns true if the venue was found and updated.
    pub async fn update_config(&self, venue_id: &VenueId, config: VenueConfig) -> bool {
        let mut adapters = self.adapters.write().await;
        if let Some(entry) = adapters.get_mut(venue_id) {
            entry.config = config;
            true
        } else {
            false
        }
    }

    /// Enables or disables a venue.
    ///
    /// Returns true if the venue was found and updated.
    pub async fn set_enabled(&self, venue_id: &VenueId, enabled: bool) -> bool {
        let mut adapters = self.adapters.write().await;
        if let Some(entry) = adapters.get_mut(venue_id) {
            entry.config.enabled = enabled;
            true
        } else {
            false
        }
    }

    /// Gets all registered adapters.
    pub async fn get_all(&self) -> Vec<Arc<dyn VenueAdapter>> {
        let adapters = self.adapters.read().await;
        adapters.values().map(|e| Arc::clone(&e.adapter)).collect()
    }

    /// Gets all enabled adapters.
    pub async fn get_enabled(&self) -> Vec<Arc<dyn VenueAdapter>> {
        let adapters = self.adapters.read().await;
        adapters
            .values()
            .filter(|e| e.config.enabled)
            .map(|e| Arc::clone(&e.adapter))
            .collect()
    }

    /// Gets all available adapters (enabled and healthy).
    ///
    /// Performs health checks on all enabled adapters and returns
    /// only those that are operational.
    pub async fn get_available(&self) -> Vec<Arc<dyn VenueAdapter>> {
        let enabled = self.get_enabled().await;
        let mut available = Vec::with_capacity(enabled.len());

        for adapter in enabled {
            if adapter.is_available().await {
                available.push(adapter);
            }
        }

        available
    }

    /// Gets adapters that support the given instrument.
    ///
    /// Returns enabled adapters that either:
    /// - Have no instrument restrictions (support all)
    /// - Explicitly support the given instrument
    pub async fn get_for_instrument(&self, instrument: &Instrument) -> Vec<Arc<dyn VenueAdapter>> {
        let adapters = self.adapters.read().await;
        adapters
            .values()
            .filter(|e| e.config.enabled && e.config.supports_instrument(instrument))
            .map(|e| Arc::clone(&e.adapter))
            .collect()
    }

    /// Gets available adapters that support the given instrument.
    ///
    /// Combines instrument filtering with health checks.
    pub async fn get_available_for_instrument(
        &self,
        instrument: &Instrument,
    ) -> Vec<Arc<dyn VenueAdapter>> {
        let candidates = self.get_for_instrument(instrument).await;
        let mut available = Vec::with_capacity(candidates.len());

        for adapter in candidates {
            if adapter.is_available().await {
                available.push(adapter);
            }
        }

        available
    }

    /// Gets adapters sorted by priority.
    ///
    /// Lower priority values come first.
    pub async fn get_by_priority(&self) -> Vec<Arc<dyn VenueAdapter>> {
        let adapters = self.adapters.read().await;
        let mut entries: Vec<_> = adapters.values().filter(|e| e.config.enabled).collect();

        entries.sort_by_key(|e| e.config.priority);
        entries.iter().map(|e| Arc::clone(&e.adapter)).collect()
    }

    /// Returns the number of registered adapters.
    pub async fn len(&self) -> usize {
        let adapters = self.adapters.read().await;
        adapters.len()
    }

    /// Returns true if no adapters are registered.
    pub async fn is_empty(&self) -> bool {
        let adapters = self.adapters.read().await;
        adapters.is_empty()
    }

    /// Returns the IDs of all registered venues.
    pub async fn venue_ids(&self) -> Vec<VenueId> {
        let adapters = self.adapters.read().await;
        adapters.keys().cloned().collect()
    }

    /// Clears all registered adapters.
    pub async fn clear(&self) {
        let mut adapters = self.adapters.write().await;
        adapters.clear();
    }
}

impl Default for VenueRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// Manual Debug implementation since RegistryEntry contains dyn VenueAdapter
impl std::fmt::Debug for RegistryEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RegistryEntry")
            .field("adapter", &self.adapter.venue_id())
            .field("config", &self.config)
            .finish()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::domain::entities::quote::Quote;
    use crate::domain::entities::rfq::Rfq;
    use crate::domain::value_objects::{AssetClass, Symbol};
    use crate::infrastructure::venues::error::VenueResult;
    use crate::infrastructure::venues::traits::{ExecutionResult, VenueHealth};
    use async_trait::async_trait;

    /// Mock adapter for testing.
    #[derive(Debug)]
    struct MockAdapter {
        venue_id: VenueId,
        available: bool,
    }

    impl MockAdapter {
        fn new(id: &str) -> Self {
            Self {
                venue_id: VenueId::new(id),
                available: true,
            }
        }

        fn unavailable(id: &str) -> Self {
            Self {
                venue_id: VenueId::new(id),
                available: false,
            }
        }
    }

    #[async_trait]
    impl VenueAdapter for MockAdapter {
        fn venue_id(&self) -> &VenueId {
            &self.venue_id
        }

        fn timeout_ms(&self) -> u64 {
            5000
        }

        async fn request_quote(&self, _rfq: &Rfq) -> VenueResult<Quote> {
            unimplemented!("mock")
        }

        async fn execute_trade(&self, _quote: &Quote) -> VenueResult<ExecutionResult> {
            unimplemented!("mock")
        }

        async fn health_check(&self) -> VenueResult<VenueHealth> {
            Ok(VenueHealth::healthy(self.venue_id.clone()))
        }

        async fn is_available(&self) -> bool {
            self.available
        }
    }

    fn test_instrument() -> Instrument {
        let symbol = Symbol::new("BTC/USD").unwrap();
        Instrument::builder(symbol, AssetClass::CryptoSpot).build()
    }

    mod venue_config {
        use super::*;

        #[test]
        fn default_config() {
            let config = VenueConfig::new();
            assert!(config.is_enabled());
            assert_eq!(config.priority(), 100);
            assert!(config.supported_instruments().is_empty());
        }

        #[test]
        fn disabled_config() {
            let config = VenueConfig::disabled();
            assert!(!config.is_enabled());
        }

        #[test]
        fn with_priority() {
            let config = VenueConfig::new().with_priority(50);
            assert_eq!(config.priority(), 50);
        }

        #[test]
        fn supports_instrument_empty() {
            let config = VenueConfig::new();
            let instrument = test_instrument();
            assert!(config.supports_instrument(&instrument));
        }

        #[test]
        fn supports_instrument_specific() {
            let instrument = test_instrument();
            let config = VenueConfig::new().with_instruments(vec![instrument.clone()]);
            assert!(config.supports_instrument(&instrument));
        }
    }

    mod registry {
        use super::*;

        #[tokio::test]
        async fn new_is_empty() {
            let registry = VenueRegistry::new();
            assert!(registry.is_empty().await);
            assert_eq!(registry.len().await, 0);
        }

        #[tokio::test]
        async fn register_and_get() {
            let registry = VenueRegistry::new();
            let adapter = Arc::new(MockAdapter::new("test-venue"));

            registry.register(adapter).await;

            assert_eq!(registry.len().await, 1);
            let retrieved = registry.get(&VenueId::new("test-venue")).await;
            assert!(retrieved.is_some());
            assert_eq!(retrieved.unwrap().venue_id(), &VenueId::new("test-venue"));
        }

        #[tokio::test]
        async fn get_nonexistent() {
            let registry = VenueRegistry::new();
            let result = registry.get(&VenueId::new("nonexistent")).await;
            assert!(result.is_none());
        }

        #[tokio::test]
        async fn unregister() {
            let registry = VenueRegistry::new();
            let adapter = Arc::new(MockAdapter::new("test-venue"));

            registry.register(adapter).await;
            assert_eq!(registry.len().await, 1);

            let removed = registry.unregister(&VenueId::new("test-venue")).await;
            assert!(removed);
            assert!(registry.is_empty().await);
        }

        #[tokio::test]
        async fn unregister_nonexistent() {
            let registry = VenueRegistry::new();
            let removed = registry.unregister(&VenueId::new("nonexistent")).await;
            assert!(!removed);
        }

        #[tokio::test]
        async fn get_all() {
            let registry = VenueRegistry::new();
            registry
                .register(Arc::new(MockAdapter::new("venue-1")))
                .await;
            registry
                .register(Arc::new(MockAdapter::new("venue-2")))
                .await;

            let all = registry.get_all().await;
            assert_eq!(all.len(), 2);
        }

        #[tokio::test]
        async fn get_enabled() {
            let registry = VenueRegistry::new();
            registry
                .register(Arc::new(MockAdapter::new("venue-1")))
                .await;
            registry
                .register_with_config(
                    Arc::new(MockAdapter::new("venue-2")),
                    VenueConfig::disabled(),
                )
                .await;

            let enabled = registry.get_enabled().await;
            assert_eq!(enabled.len(), 1);
            assert_eq!(enabled[0].venue_id(), &VenueId::new("venue-1"));
        }

        #[tokio::test]
        async fn get_available() {
            let registry = VenueRegistry::new();
            registry
                .register(Arc::new(MockAdapter::new("venue-1")))
                .await;
            registry
                .register(Arc::new(MockAdapter::unavailable("venue-2")))
                .await;

            let available = registry.get_available().await;
            assert_eq!(available.len(), 1);
            assert_eq!(available[0].venue_id(), &VenueId::new("venue-1"));
        }

        #[tokio::test]
        async fn get_for_instrument() {
            let registry = VenueRegistry::new();
            let instrument = test_instrument();

            // Adapter with no restrictions
            registry
                .register(Arc::new(MockAdapter::new("venue-1")))
                .await;

            // Adapter with specific instrument
            registry
                .register_with_config(
                    Arc::new(MockAdapter::new("venue-2")),
                    VenueConfig::new().with_instruments(vec![instrument.clone()]),
                )
                .await;

            let adapters = registry.get_for_instrument(&instrument).await;
            assert_eq!(adapters.len(), 2);
        }

        #[tokio::test]
        async fn set_enabled() {
            let registry = VenueRegistry::new();
            registry
                .register(Arc::new(MockAdapter::new("venue-1")))
                .await;

            let updated = registry.set_enabled(&VenueId::new("venue-1"), false).await;
            assert!(updated);

            let enabled = registry.get_enabled().await;
            assert!(enabled.is_empty());
        }

        #[tokio::test]
        async fn get_by_priority() {
            let registry = VenueRegistry::new();

            registry
                .register_with_config(
                    Arc::new(MockAdapter::new("venue-low")),
                    VenueConfig::new().with_priority(200),
                )
                .await;
            registry
                .register_with_config(
                    Arc::new(MockAdapter::new("venue-high")),
                    VenueConfig::new().with_priority(50),
                )
                .await;

            let sorted = registry.get_by_priority().await;
            assert_eq!(sorted.len(), 2);
            assert_eq!(sorted[0].venue_id(), &VenueId::new("venue-high"));
            assert_eq!(sorted[1].venue_id(), &VenueId::new("venue-low"));
        }

        #[tokio::test]
        async fn venue_ids() {
            let registry = VenueRegistry::new();
            registry
                .register(Arc::new(MockAdapter::new("venue-1")))
                .await;
            registry
                .register(Arc::new(MockAdapter::new("venue-2")))
                .await;

            let ids = registry.venue_ids().await;
            assert_eq!(ids.len(), 2);
        }

        #[tokio::test]
        async fn clear() {
            let registry = VenueRegistry::new();
            registry
                .register(Arc::new(MockAdapter::new("venue-1")))
                .await;
            registry
                .register(Arc::new(MockAdapter::new("venue-2")))
                .await;

            registry.clear().await;
            assert!(registry.is_empty().await);
        }

        #[tokio::test]
        async fn update_config() {
            let registry = VenueRegistry::new();
            registry
                .register(Arc::new(MockAdapter::new("venue-1")))
                .await;

            let updated = registry
                .update_config(
                    &VenueId::new("venue-1"),
                    VenueConfig::new().with_priority(10),
                )
                .await;
            assert!(updated);

            let config = registry.get_config(&VenueId::new("venue-1")).await;
            assert_eq!(config.unwrap().priority(), 10);
        }
    }
}
