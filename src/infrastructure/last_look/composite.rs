//! # Composite Last-Look Service
//!
//! Routes last-look requests to the appropriate channel per venue.

use crate::domain::entities::quote::Quote;
use crate::domain::services::last_look::{
    LastLookRejectReason, LastLookResult, LastLookService, LastLookStats,
};
use crate::domain::value_objects::VenueId;
use async_trait::async_trait;
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

/// Channel type for last-look communication.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LastLookChannel {
    /// WebSocket channel.
    WebSocket,
    /// gRPC channel.
    Grpc,
    /// FIX protocol channel.
    Fix,
}

impl fmt::Display for LastLookChannel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WebSocket => write!(f, "websocket"),
            Self::Grpc => write!(f, "grpc"),
            Self::Fix => write!(f, "fix"),
        }
    }
}

/// Venue routing configuration.
#[derive(Debug, Clone)]
pub struct VenueRouting {
    /// The channel to use for this venue.
    pub channel: LastLookChannel,
    /// Whether last-look is required for this venue.
    pub requires_last_look: bool,
}

impl VenueRouting {
    /// Creates a new venue routing.
    #[must_use]
    pub fn new(channel: LastLookChannel, requires_last_look: bool) -> Self {
        Self {
            channel,
            requires_last_look,
        }
    }
}

/// Composite last-look service that routes to appropriate channel per venue.
pub struct CompositeLastLookService {
    /// WebSocket client.
    websocket: Option<Arc<dyn LastLookService>>,
    /// gRPC client.
    grpc: Option<Arc<dyn LastLookService>>,
    /// FIX client.
    fix: Option<Arc<dyn LastLookService>>,
    /// Venue routing configuration.
    routing: RwLock<HashMap<String, VenueRouting>>,
    /// Aggregated stats per venue.
    stats: RwLock<HashMap<String, LastLookStats>>,
}

impl fmt::Debug for CompositeLastLookService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CompositeLastLookService")
            .field("has_websocket", &self.websocket.is_some())
            .field("has_grpc", &self.grpc.is_some())
            .field("has_fix", &self.fix.is_some())
            .finish()
    }
}

impl CompositeLastLookService {
    /// Creates a new composite service with no channels configured.
    #[must_use]
    pub fn new() -> Self {
        Self {
            websocket: None,
            grpc: None,
            fix: None,
            routing: RwLock::new(HashMap::new()),
            stats: RwLock::new(HashMap::new()),
        }
    }

    /// Sets the WebSocket client.
    #[must_use]
    pub fn with_websocket(mut self, client: Arc<dyn LastLookService>) -> Self {
        self.websocket = Some(client);
        self
    }

    /// Sets the gRPC client.
    #[must_use]
    pub fn with_grpc(mut self, client: Arc<dyn LastLookService>) -> Self {
        self.grpc = Some(client);
        self
    }

    /// Sets the FIX client.
    #[must_use]
    pub fn with_fix(mut self, client: Arc<dyn LastLookService>) -> Self {
        self.fix = Some(client);
        self
    }

    /// Registers a venue with its routing configuration.
    pub async fn register_venue(&self, venue_id: &VenueId, routing: VenueRouting) {
        let mut guard = self.routing.write().await;
        guard.insert(venue_id.to_string(), routing);
    }

    /// Returns the client for a given channel.
    fn get_client(&self, channel: LastLookChannel) -> Option<&Arc<dyn LastLookService>> {
        match channel {
            LastLookChannel::WebSocket => self.websocket.as_ref(),
            LastLookChannel::Grpc => self.grpc.as_ref(),
            LastLookChannel::Fix => self.fix.as_ref(),
        }
    }

    /// Returns the routing for a venue (sync version using try_read).
    fn get_routing_sync(&self, venue_id: &VenueId) -> Option<VenueRouting> {
        self.routing
            .try_read()
            .ok()
            .and_then(|guard| guard.get(&venue_id.to_string()).cloned())
    }

    /// Returns the routing for a venue (async version).
    async fn get_routing(&self, venue_id: &VenueId) -> Option<VenueRouting> {
        let guard = self.routing.read().await;
        guard.get(&venue_id.to_string()).cloned()
    }
}

impl Default for CompositeLastLookService {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LastLookService for CompositeLastLookService {
    async fn request(&self, quote: &Quote, timeout: Duration) -> LastLookResult {
        let venue_id = quote.venue_id();

        // Get routing for this venue
        let routing = match self.get_routing(venue_id).await {
            Some(r) => r,
            None => {
                // No routing configured, auto-confirm
                return LastLookResult::confirmed(quote.id());
            }
        };

        // If last-look not required, auto-confirm
        if !routing.requires_last_look {
            return LastLookResult::confirmed(quote.id());
        }

        // Get the appropriate client
        let client = match self.get_client(routing.channel) {
            Some(c) => c,
            None => {
                // Channel not configured, reject with error
                return LastLookResult::rejected(
                    quote.id(),
                    LastLookRejectReason::other(format!(
                        "channel {} not configured",
                        routing.channel
                    )),
                );
            }
        };

        // Delegate to the channel client
        client.request(quote, timeout).await
    }

    fn requires_last_look(&self, venue_id: &VenueId) -> bool {
        // Use sync version with try_read for sync method
        self.get_routing_sync(venue_id)
            .map(|r| r.requires_last_look)
            .unwrap_or(false)
    }

    async fn get_stats(&self, venue_id: &VenueId) -> Option<LastLookStats> {
        let guard = self.stats.read().await;
        guard.get(&venue_id.to_string()).cloned()
    }

    async fn record_result(&self, venue_id: &VenueId, result: &LastLookResult) {
        let mut guard = self.stats.write().await;
        let stats = guard
            .entry(venue_id.to_string())
            .or_insert_with(LastLookStats::new);
        match result {
            LastLookResult::Confirmed { .. } => stats.record_confirmation(),
            LastLookResult::Rejected { .. } => stats.record_rejection(),
            LastLookResult::Timeout { .. } => stats.record_timeout(),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::domain::entities::quote::QuoteBuilder;
    use crate::domain::value_objects::{Price, Quantity, RfqId, Timestamp};

    fn create_test_quote(venue_id: &VenueId) -> Quote {
        QuoteBuilder::new(
            RfqId::new_v4(),
            venue_id.clone(),
            Price::new(100.0).unwrap(),
            Quantity::new(1.0).unwrap(),
            Timestamp::now().add_secs(300),
        )
        .build()
    }

    #[test]
    fn channel_display() {
        assert_eq!(LastLookChannel::WebSocket.to_string(), "websocket");
        assert_eq!(LastLookChannel::Grpc.to_string(), "grpc");
        assert_eq!(LastLookChannel::Fix.to_string(), "fix");
    }

    #[test]
    fn composite_debug() {
        let service = CompositeLastLookService::new();
        let debug = format!("{:?}", service);
        assert!(debug.contains("CompositeLastLookService"));
    }

    #[tokio::test]
    async fn no_routing_auto_confirms() {
        let service = CompositeLastLookService::new();
        let venue = VenueId::new("unknown-venue");
        let quote = create_test_quote(&venue);

        let result = service.request(&quote, Duration::from_millis(200)).await;
        assert!(result.is_confirmed());
    }

    #[tokio::test]
    async fn last_look_not_required_auto_confirms() {
        let service = CompositeLastLookService::new();
        let venue = VenueId::new("test-venue");
        service
            .register_venue(&venue, VenueRouting::new(LastLookChannel::WebSocket, false))
            .await;

        let quote = create_test_quote(&venue);
        let result = service.request(&quote, Duration::from_millis(200)).await;
        assert!(result.is_confirmed());
    }

    #[tokio::test]
    async fn missing_channel_rejects() {
        let service = CompositeLastLookService::new();
        let venue = VenueId::new("test-venue");
        service
            .register_venue(&venue, VenueRouting::new(LastLookChannel::WebSocket, true))
            .await;

        let quote = create_test_quote(&venue);
        let result = service.request(&quote, Duration::from_millis(200)).await;
        assert!(result.is_rejected());
    }

    #[tokio::test]
    async fn requires_last_look() {
        let service = CompositeLastLookService::new();
        let venue = VenueId::new("test-venue");

        assert!(!service.requires_last_look(&venue));

        service
            .register_venue(&venue, VenueRouting::new(LastLookChannel::Grpc, true))
            .await;
        assert!(service.requires_last_look(&venue));
    }
}
