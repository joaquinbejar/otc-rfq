//! # gRPC Confirmation Adapter
//!
//! Delivers trade confirmations via gRPC streaming to connected clients.

use crate::domain::errors::{DomainError, DomainResult};
use crate::domain::services::confirmation_service::ConfirmationChannelAdapter;
use crate::domain::value_objects::CounterpartyId;
use crate::domain::value_objects::confirmation::{
    ConfirmationChannel, NotificationDestination, TradeConfirmation,
};
use async_trait::async_trait;
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use tokio::sync::RwLock;

/// gRPC stream handle for sending messages to clients.
pub type StreamHandle = Arc<dyn GrpcStream>;

/// Trait for gRPC streaming.
#[async_trait]
pub trait GrpcStream: Send + Sync + fmt::Debug {
    /// Sends a confirmation message to the gRPC client.
    ///
    /// # Errors
    ///
    /// Returns an error if the message cannot be sent.
    async fn send_confirmation(&self, confirmation: &TradeConfirmation) -> DomainResult<()>;

    /// Returns true if the stream is still active.
    fn is_active(&self) -> bool;
}

/// gRPC client registry.
#[async_trait]
pub trait GrpcClientRegistry: Send + Sync + fmt::Debug {
    /// Gets active gRPC streams for a counterparty.
    async fn get_streams(&self, counterparty_id: &CounterpartyId) -> Vec<StreamHandle>;
}

/// In-memory gRPC client registry implementation.
#[derive(Debug, Default)]
pub struct InMemoryGrpcClientRegistry {
    streams: Arc<RwLock<HashMap<CounterpartyId, Vec<StreamHandle>>>>,
}

impl InMemoryGrpcClientRegistry {
    /// Creates a new in-memory gRPC client registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            streams: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Registers a gRPC stream for a counterparty.
    pub async fn register_stream(&self, counterparty_id: CounterpartyId, stream: StreamHandle) {
        let mut streams = self.streams.write().await;
        streams
            .entry(counterparty_id)
            .or_insert_with(Vec::new)
            .push(stream);
    }

    /// Removes a gRPC stream for a counterparty.
    pub async fn remove_stream(&self, counterparty_id: &CounterpartyId, stream_id: &str) {
        let mut streams = self.streams.write().await;
        if let Some(stream_list) = streams.get_mut(counterparty_id) {
            stream_list.retain(|s| format!("{:?}", s) != stream_id);
            if stream_list.is_empty() {
                streams.remove(counterparty_id);
            }
        }
    }

    /// Cleans up inactive streams.
    pub async fn cleanup_inactive(&self) {
        let mut streams = self.streams.write().await;
        for stream_list in streams.values_mut() {
            stream_list.retain(|s| s.is_active());
        }
        streams.retain(|_, v| !v.is_empty());
    }
}

#[async_trait]
impl GrpcClientRegistry for InMemoryGrpcClientRegistry {
    async fn get_streams(&self, counterparty_id: &CounterpartyId) -> Vec<StreamHandle> {
        self.streams
            .read()
            .await
            .get(counterparty_id)
            .cloned()
            .unwrap_or_default()
    }
}

/// gRPC confirmation adapter.
#[derive(Debug)]
pub struct GrpcConfirmationAdapter {
    client_registry: Arc<dyn GrpcClientRegistry>,
}

impl GrpcConfirmationAdapter {
    /// Creates a new gRPC confirmation adapter.
    #[must_use]
    pub fn new(client_registry: Arc<dyn GrpcClientRegistry>) -> Self {
        Self { client_registry }
    }
}

#[async_trait]
impl ConfirmationChannelAdapter for GrpcConfirmationAdapter {
    async fn send(
        &self,
        confirmation: &TradeConfirmation,
        destination: NotificationDestination<'_>,
    ) -> DomainResult<()> {
        // Ensure destination is Grpc
        if !matches!(destination, NotificationDestination::Grpc(_)) {
            return Err(DomainError::InvalidNotificationPreferences {
                reason: "Expected Grpc destination".to_string(),
            });
        }
        // Extract counterparty IDs from participants (only counterparties get notifications)
        let mut counterparty_ids = Vec::new();
        if let Some(id) = confirmation.buyer().as_counterparty() {
            counterparty_ids.push(id);
        }
        if let Some(id) = confirmation.seller().as_counterparty() {
            counterparty_ids.push(id);
        }

        if counterparty_ids.is_empty() {
            return Err(DomainError::ConfirmationFailed {
                channel: "GRPC".to_string(),
                reason: "No counterparties found in trade participants".to_string(),
            });
        }

        let mut errors = Vec::new();
        let mut sent_count = 0;
        let mut total_streams = 0;

        // Send to all counterparties (venues don't receive gRPC notifications)
        for counterparty_id in counterparty_ids {
            let streams = self.client_registry.get_streams(counterparty_id).await;
            total_streams += streams.len();

            for stream in &streams {
                if stream.is_active() {
                    match stream.send_confirmation(confirmation).await {
                        Ok(()) => {
                            sent_count += 1;
                            tracing::debug!(
                                trade_id = %confirmation.trade_id(),
                                counterparty = %counterparty_id,
                                "gRPC confirmation sent to counterparty"
                            );
                        }
                        Err(e) => {
                            errors.push(format!("Stream error for {}: {}", counterparty_id, e));
                        }
                    }
                }
            }
        }

        if sent_count == 0 {
            let reason = if total_streams == 0 {
                "No active gRPC streams found for counterparties".to_string()
            } else if errors.is_empty() {
                "All gRPC streams are inactive".to_string()
            } else {
                format!("Failed to send to any stream: {}", errors.join(", "))
            };
            Err(DomainError::ConfirmationFailed {
                channel: "GRPC".to_string(),
                reason,
            })
        } else {
            Ok(())
        }
    }

    fn channel_type(&self) -> ConfirmationChannel {
        ConfirmationChannel::Grpc
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::clone_on_ref_ptr)]
mod tests {
    use super::*;
    use crate::domain::value_objects::{
        Blockchain, Price, Quantity, RfqId, SettlementMethod, TradeId, TradeParticipant,
    };
    use rust_decimal::Decimal;
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

    #[derive(Debug)]
    struct MockStream {
        active: Arc<AtomicBool>,
        send_count: Arc<AtomicU32>,
        should_fail: bool,
    }

    impl MockStream {
        fn new(active: bool, should_fail: bool) -> Self {
            Self {
                active: Arc::new(AtomicBool::new(active)),
                send_count: Arc::new(AtomicU32::new(0)),
                should_fail,
            }
        }

        fn send_count(&self) -> u32 {
            self.send_count.load(Ordering::SeqCst)
        }
    }

    #[async_trait]
    impl GrpcStream for MockStream {
        async fn send_confirmation(&self, _confirmation: &TradeConfirmation) -> DomainResult<()> {
            self.send_count.fetch_add(1, Ordering::SeqCst);
            if self.should_fail {
                Err(DomainError::ConfirmationFailed {
                    channel: "GRPC".to_string(),
                    reason: "Mock send failure".to_string(),
                })
            } else {
                Ok(())
            }
        }

        fn is_active(&self) -> bool {
            self.active.load(Ordering::SeqCst)
        }
    }

    fn create_test_confirmation() -> TradeConfirmation {
        let trade_id = TradeId::new_v4();
        let rfq_id = RfqId::new_v4();
        TradeConfirmation::new(
            trade_id,
            rfq_id,
            Price::new(50000.0).unwrap(),
            Quantity::new(1.0).unwrap(),
            Decimal::new(10, 0),
            Decimal::new(5, 0),
            Decimal::new(15, 0),
            SettlementMethod::OnChain(Blockchain::Ethereum),
            TradeParticipant::Counterparty(CounterpartyId::new("buyer-1")),
            TradeParticipant::Counterparty(CounterpartyId::new("seller-1")),
        )
    }

    #[tokio::test]
    async fn send_no_streams() {
        let registry = Arc::new(InMemoryGrpcClientRegistry::new());
        let adapter = GrpcConfirmationAdapter::new(registry);
        let confirmation = create_test_confirmation();

        let result = adapter
            .send(&confirmation, NotificationDestination::Grpc(None))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn send_to_single_counterparty() {
        let registry = Arc::new(InMemoryGrpcClientRegistry::new());
        let stream = Arc::new(MockStream::new(true, false));

        registry
            .register_stream(CounterpartyId::new("buyer-1"), stream.clone())
            .await;

        let adapter = GrpcConfirmationAdapter::new(registry);
        let confirmation = create_test_confirmation();

        let result = adapter
            .send(&confirmation, NotificationDestination::Grpc(None))
            .await;
        assert!(result.is_ok());
        assert_eq!(stream.send_count(), 1);
    }

    #[tokio::test]
    async fn send_to_both_counterparties() {
        let registry = Arc::new(InMemoryGrpcClientRegistry::new());
        let buyer_stream = Arc::new(MockStream::new(true, false));
        let seller_stream = Arc::new(MockStream::new(true, false));

        registry
            .register_stream(CounterpartyId::new("buyer-1"), buyer_stream.clone())
            .await;
        registry
            .register_stream(CounterpartyId::new("seller-1"), seller_stream.clone())
            .await;

        let adapter = GrpcConfirmationAdapter::new(registry);
        let confirmation = create_test_confirmation();

        let result = adapter
            .send(&confirmation, NotificationDestination::Grpc(None))
            .await;
        assert!(result.is_ok());
        assert_eq!(buyer_stream.send_count(), 1);
        assert_eq!(seller_stream.send_count(), 1);
    }

    #[tokio::test]
    async fn send_to_inactive_stream() {
        let registry = Arc::new(InMemoryGrpcClientRegistry::new());
        let stream = Arc::new(MockStream::new(false, false));

        registry
            .register_stream(CounterpartyId::new("buyer-1"), stream.clone())
            .await;

        let adapter = GrpcConfirmationAdapter::new(registry);
        let confirmation = create_test_confirmation();

        let result = adapter
            .send(&confirmation, NotificationDestination::Grpc(None))
            .await;
        assert!(result.is_err());
        assert_eq!(stream.send_count(), 0);
    }

    #[tokio::test]
    async fn send_partial_success() {
        let registry = Arc::new(InMemoryGrpcClientRegistry::new());
        let good_stream = Arc::new(MockStream::new(true, false));
        let bad_stream = Arc::new(MockStream::new(true, true));

        registry
            .register_stream(
                CounterpartyId::new("buyer-1"),
                Arc::clone(&good_stream) as Arc<dyn GrpcStream>,
            )
            .await;
        registry
            .register_stream(
                CounterpartyId::new("seller-1"),
                Arc::clone(&bad_stream) as Arc<dyn GrpcStream>,
            )
            .await;

        let adapter = GrpcConfirmationAdapter::new(registry);
        let confirmation = create_test_confirmation();

        let result = adapter
            .send(&confirmation, NotificationDestination::Grpc(None))
            .await;
        assert!(result.is_ok()); // At least one succeeded
        assert_eq!(good_stream.send_count(), 1);
        assert_eq!(bad_stream.send_count(), 1);
    }

    #[tokio::test]
    async fn cleanup_inactive_streams() {
        let registry = InMemoryGrpcClientRegistry::new();
        let active = Arc::new(MockStream::new(true, false));
        let inactive = Arc::new(MockStream::new(false, false));

        registry
            .register_stream(
                CounterpartyId::new("buyer-1"),
                Arc::clone(&active) as Arc<dyn GrpcStream>,
            )
            .await;
        registry
            .register_stream(
                CounterpartyId::new("buyer-1"),
                Arc::clone(&inactive) as Arc<dyn GrpcStream>,
            )
            .await;

        registry.cleanup_inactive().await;

        let streams = registry.get_streams(&CounterpartyId::new("buyer-1")).await;
        assert_eq!(streams.len(), 1);
    }

    #[test]
    fn channel_returns_grpc() {
        let registry = Arc::new(InMemoryGrpcClientRegistry::new());
        let adapter = GrpcConfirmationAdapter::new(registry);
        assert_eq!(adapter.channel_type(), ConfirmationChannel::Grpc);
    }
}
