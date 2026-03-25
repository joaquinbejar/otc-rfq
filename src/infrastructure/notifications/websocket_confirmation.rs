//! # WebSocket Confirmation Adapter
//!
//! Delivers trade confirmations via WebSocket to connected clients.

use crate::domain::errors::{DomainError, DomainResult};
use crate::domain::services::confirmation_service::ConfirmationChannelAdapter;
use crate::domain::value_objects::CounterpartyId;
use crate::domain::value_objects::confirmation::{
    ConfirmationChannel, NotificationDestination, TradeConfirmation,
};
use async_trait::async_trait;
use serde_json;
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use tokio::sync::RwLock;

/// WebSocket session handle.
pub type SessionHandle = Arc<dyn WebSocketSession>;

/// Trait for WebSocket session management.
#[async_trait]
pub trait WebSocketSession: Send + Sync + fmt::Debug {
    /// Sends a message to the WebSocket client.
    ///
    /// # Errors
    ///
    /// Returns an error if the message cannot be sent.
    async fn send_message(&self, message: String) -> DomainResult<()>;

    /// Returns true if the session is still connected.
    fn is_connected(&self) -> bool;
}

/// WebSocket session registry.
pub trait SessionRegistry: Send + Sync + fmt::Debug {
    /// Gets active sessions for a counterparty.
    fn get_sessions(&self, counterparty_id: &CounterpartyId) -> Vec<SessionHandle>;
}

/// In-memory session registry implementation.
#[derive(Debug, Default)]
pub struct InMemorySessionRegistry {
    sessions: Arc<RwLock<HashMap<CounterpartyId, Vec<SessionHandle>>>>,
}

impl InMemorySessionRegistry {
    /// Creates a new in-memory session registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Registers a session for a counterparty.
    pub async fn register_session(&self, counterparty_id: CounterpartyId, session: SessionHandle) {
        let mut sessions = self.sessions.write().await;
        sessions
            .entry(counterparty_id)
            .or_insert_with(Vec::new)
            .push(session);
    }

    /// Removes a session for a counterparty.
    pub async fn remove_session(&self, counterparty_id: &CounterpartyId, session_id: &str) {
        let mut sessions = self.sessions.write().await;
        if let Some(session_list) = sessions.get_mut(counterparty_id) {
            session_list.retain(|s| format!("{:?}", s) != session_id);
            if session_list.is_empty() {
                sessions.remove(counterparty_id);
            }
        }
    }

    /// Cleans up disconnected sessions.
    pub async fn cleanup_disconnected(&self) {
        let mut sessions = self.sessions.write().await;
        for session_list in sessions.values_mut() {
            session_list.retain(|s| s.is_connected());
        }
        sessions.retain(|_, v| !v.is_empty());
    }
}

impl SessionRegistry for InMemorySessionRegistry {
    fn get_sessions(&self, counterparty_id: &CounterpartyId) -> Vec<SessionHandle> {
        // LIMITATION: Using try_read() which can fail spuriously under contention
        // Reviewers suggested using read().await or DashMap for better reliability
        // However, this requires making the trait async (breaking change) or using DashMap
        // For now, we accept potential spurious failures under high contention
        // TODO: Refactor to async trait or DashMap in production
        self.sessions
            .try_read()
            .ok()
            .and_then(|sessions| sessions.get(counterparty_id).cloned())
            .unwrap_or_default()
    }
}

/// WebSocket confirmation adapter.
#[derive(Debug)]
pub struct WebSocketConfirmationAdapter {
    session_registry: Arc<dyn SessionRegistry>,
}

impl WebSocketConfirmationAdapter {
    /// Creates a new WebSocket confirmation adapter.
    #[must_use]
    pub fn new(session_registry: Arc<dyn SessionRegistry>) -> Self {
        Self { session_registry }
    }
}

#[async_trait]
impl ConfirmationChannelAdapter for WebSocketConfirmationAdapter {
    async fn send(
        &self,
        confirmation: &TradeConfirmation,
        destination: NotificationDestination<'_>,
    ) -> DomainResult<()> {
        // Ensure destination is WebSocket
        if !matches!(destination, NotificationDestination::WebSocket) {
            return Err(DomainError::InvalidNotificationPreferences {
                reason: "Expected WebSocket destination".to_string(),
            });
        }
        // Serialize confirmation to JSON
        let message =
            serde_json::to_string(confirmation).map_err(|e| DomainError::ConfirmationFailed {
                channel: "WEBSOCKET".to_string(),
                reason: format!("JSON serialization failed: {}", e),
            })?;

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
                channel: "WEBSOCKET".to_string(),
                reason: "No counterparties found in trade participants".to_string(),
            });
        }

        let mut errors = Vec::new();
        let mut sent_count = 0;

        // Send to all counterparties (venues don't receive WebSocket notifications)
        for counterparty_id in counterparty_ids {
            let sessions = self.session_registry.get_sessions(counterparty_id);
            
            for session in &sessions {
                if session.is_connected() {
                    match session.send_message(message.clone()).await {
                        Ok(()) => {
                            sent_count += 1;
                            tracing::debug!(
                                trade_id = %confirmation.trade_id(),
                                counterparty = %counterparty_id,
                                "WebSocket confirmation sent to counterparty"
                            );
                        }
                        Err(e) => {
                            errors.push(format!("Session error for {}: {}", counterparty_id, e));
                        }
                    }
                }
            }
        }

        if sent_count == 0 {
            Err(DomainError::ConfirmationFailed {
                channel: "WEBSOCKET".to_string(),
                reason: format!("Failed to send to any session: {}", errors.join(", ")),
            })
        } else {
            Ok(())
        }
    }

    fn channel_type(&self) -> ConfirmationChannel {
        ConfirmationChannel::WebSocket
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::clone_on_ref_ptr)]
mod tests {
    use super::*;
    use crate::domain::value_objects::{
        Blockchain, Price, Quantity, RfqId, SettlementMethod, TradeId,
    };
    use rust_decimal::Decimal;
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

    #[derive(Debug)]
    struct MockSession {
        connected: Arc<AtomicBool>,
        send_count: Arc<AtomicU32>,
        should_fail: bool,
    }

    impl MockSession {
        fn new(connected: bool, should_fail: bool) -> Self {
            Self {
                connected: Arc::new(AtomicBool::new(connected)),
                send_count: Arc::new(AtomicU32::new(0)),
                should_fail,
            }
        }

        fn send_count(&self) -> u32 {
            self.send_count.load(Ordering::SeqCst)
        }
    }

    #[async_trait]
    impl WebSocketSession for MockSession {
        async fn send_message(&self, _message: String) -> DomainResult<()> {
            self.send_count.fetch_add(1, Ordering::SeqCst);
            if self.should_fail {
                Err(DomainError::ConfirmationFailed {
                    channel: "WEBSOCKET".to_string(),
                    reason: "Mock send failure".to_string(),
                })
            } else {
                Ok(())
            }
        }

        fn is_connected(&self) -> bool {
            self.connected.load(Ordering::SeqCst)
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
    async fn send_no_sessions() {
        let registry = Arc::new(InMemorySessionRegistry::new());
        let adapter = WebSocketConfirmationAdapter::new(registry);
        let confirmation = create_test_confirmation();

        let result = adapter
            .send(&confirmation, NotificationDestination::WebSocket)
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn send_to_buyer_session() {
        let registry = Arc::new(InMemorySessionRegistry::new());
        let session = Arc::new(MockSession::new(true, false));

        registry
            .register_session(CounterpartyId::new("buyer-1"), session.clone())
            .await;

        let adapter = WebSocketConfirmationAdapter::new(registry);
        let confirmation = create_test_confirmation();

        let result = adapter
            .send(&confirmation, NotificationDestination::WebSocket)
            .await;
        assert!(result.is_ok());
        assert_eq!(session.send_count(), 1);
    }

    #[tokio::test]
    async fn send_to_both_counterparties() {
        let registry = Arc::new(InMemorySessionRegistry::new());
        let buyer_session = Arc::new(MockSession::new(true, false));
        let seller_session = Arc::new(MockSession::new(true, false));

        registry
            .register_session(
                CounterpartyId::new("buyer-1"),
                Arc::clone(&buyer_session) as Arc<dyn WebSocketSession>,
            )
            .await;
        registry
            .register_session(
                CounterpartyId::new("seller-1"),
                Arc::clone(&seller_session) as Arc<dyn WebSocketSession>,
            )
            .await;

        let adapter = WebSocketConfirmationAdapter::new(registry);
        let confirmation = create_test_confirmation();

        let result = adapter
            .send(&confirmation, NotificationDestination::WebSocket)
            .await;
        assert!(result.is_ok());
        assert_eq!(buyer_session.send_count(), 1);
        assert_eq!(seller_session.send_count(), 1);
    }

    #[tokio::test]
    async fn send_disconnected_session_fails() {
        let registry = Arc::new(InMemorySessionRegistry::new());
        let session = Arc::new(MockSession::new(false, false));

        registry
            .register_session(CounterpartyId::new("buyer-1"), session.clone())
            .await;

        let adapter = WebSocketConfirmationAdapter::new(registry);
        let confirmation = create_test_confirmation();

        let result = adapter
            .send(&confirmation, NotificationDestination::WebSocket)
            .await;
        assert!(result.is_err());
        assert_eq!(session.send_count(), 0);
    }

    #[tokio::test]
    async fn send_partial_success() {
        let registry = Arc::new(InMemorySessionRegistry::new());
        let good_session = Arc::new(MockSession::new(true, false));
        let bad_session = Arc::new(MockSession::new(true, true));

        registry
            .register_session(
                CounterpartyId::new("buyer-1"),
                Arc::clone(&good_session) as Arc<dyn WebSocketSession>,
            )
            .await;
        registry
            .register_session(
                CounterpartyId::new("seller-1"),
                Arc::clone(&bad_session) as Arc<dyn WebSocketSession>,
            )
            .await;

        let adapter = WebSocketConfirmationAdapter::new(registry);
        let confirmation = create_test_confirmation();

        let result = adapter
            .send(&confirmation, NotificationDestination::WebSocket)
            .await;
        assert!(result.is_ok()); // At least one succeeded
        assert_eq!(good_session.send_count(), 1);
        assert_eq!(bad_session.send_count(), 1);
    }

    #[tokio::test]
    async fn cleanup_disconnected_sessions() {
        let registry = InMemorySessionRegistry::new();
        let connected = Arc::new(MockSession::new(true, false));
        let disconnected = Arc::new(MockSession::new(false, false));

        registry
            .register_session(
                CounterpartyId::new("buyer-1"),
                Arc::clone(&connected) as Arc<dyn WebSocketSession>,
            )
            .await;
        registry
            .register_session(
                CounterpartyId::new("buyer-1"),
                Arc::clone(&disconnected) as Arc<dyn WebSocketSession>,
            )
            .await;

        registry.cleanup_disconnected().await;

        let sessions = registry.get_sessions(&CounterpartyId::new("buyer-1"));
        assert_eq!(sessions.len(), 1);
    }
}
