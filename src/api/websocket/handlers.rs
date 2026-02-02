//! # WebSocket Handlers
//!
//! WebSocket connection and message handlers for real-time streaming.
//!
//! This module provides WebSocket handlers for streaming real-time updates
//! including quote updates, RFQ status changes, and trade notifications.
//!
//! # Channels
//!
//! - `rfq.{id}.quotes` - Quote updates for a specific RFQ
//! - `rfq.{id}.status` - RFQ state changes
//! - `trades.{id}` - Trade status updates
//!
//! # Message Format
//!
//! All messages use JSON format with the following structure:
//!
//! ```json
//! {
//!   "type": "subscribe|unsubscribe|event|error|ping|pong",
//!   "channel": "rfq.{id}.quotes",
//!   "payload": { ... }
//! }
//! ```

use crate::domain::entities::quote::Quote;
use crate::domain::entities::trade::Trade;
use crate::domain::value_objects::RfqState;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Query, State,
    },
    response::IntoResponse,
};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, RwLock};
use tracing::{debug, error, info, instrument, warn};

// ============================================================================
// Configuration
// ============================================================================

/// WebSocket configuration.
#[derive(Debug, Clone)]
pub struct WebSocketConfig {
    /// Heartbeat interval in seconds.
    pub heartbeat_interval_secs: u64,
    /// Connection timeout in seconds.
    pub connection_timeout_secs: u64,
    /// Maximum message size in bytes.
    pub max_message_size: usize,
    /// Maximum subscriptions per connection.
    pub max_subscriptions: usize,
}

impl Default for WebSocketConfig {
    fn default() -> Self {
        Self {
            heartbeat_interval_secs: 30,
            connection_timeout_secs: 60,
            max_message_size: 64 * 1024, // 64KB
            max_subscriptions: 100,
        }
    }
}

// ============================================================================
// Application State
// ============================================================================

/// Shared state for WebSocket connections.
#[derive(Debug)]
pub struct WebSocketState {
    /// Configuration.
    pub config: WebSocketConfig,
    /// Event broadcaster for RFQ quote updates.
    pub quote_events: broadcast::Sender<QuoteEvent>,
    /// Event broadcaster for RFQ status updates.
    pub rfq_status_events: broadcast::Sender<RfqStatusEvent>,
    /// Event broadcaster for trade updates.
    pub trade_events: broadcast::Sender<TradeEvent>,
    /// Active connections (for metrics/management).
    pub active_connections: RwLock<usize>,
}

impl WebSocketState {
    /// Creates a new WebSocket state with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(WebSocketConfig::default())
    }

    /// Creates a new WebSocket state with custom configuration.
    #[must_use]
    pub fn with_config(config: WebSocketConfig) -> Self {
        let (quote_events, _) = broadcast::channel(1024);
        let (rfq_status_events, _) = broadcast::channel(1024);
        let (trade_events, _) = broadcast::channel(1024);

        Self {
            config,
            quote_events,
            rfq_status_events,
            trade_events,
            active_connections: RwLock::new(0),
        }
    }

    /// Publishes a quote event.
    pub fn publish_quote(&self, event: QuoteEvent) {
        let _ = self.quote_events.send(event);
    }

    /// Publishes an RFQ status event.
    pub fn publish_rfq_status(&self, event: RfqStatusEvent) {
        let _ = self.rfq_status_events.send(event);
    }

    /// Publishes a trade event.
    pub fn publish_trade(&self, event: TradeEvent) {
        let _ = self.trade_events.send(event);
    }
}

impl Default for WebSocketState {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Events
// ============================================================================

/// Quote update event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuoteEvent {
    /// RFQ ID this quote belongs to.
    pub rfq_id: String,
    /// Quote ID.
    pub quote_id: String,
    /// Venue ID.
    pub venue_id: String,
    /// Price.
    pub price: String,
    /// Quantity.
    pub quantity: String,
    /// Valid until timestamp (ISO 8601).
    pub valid_until: String,
    /// Event timestamp (ISO 8601).
    pub timestamp: String,
}

impl From<&Quote> for QuoteEvent {
    fn from(quote: &Quote) -> Self {
        Self {
            rfq_id: quote.rfq_id().to_string(),
            quote_id: quote.id().to_string(),
            venue_id: quote.venue_id().to_string(),
            price: quote.price().to_string(),
            quantity: quote.quantity().to_string(),
            valid_until: quote.valid_until().to_string(),
            timestamp: crate::domain::value_objects::timestamp::Timestamp::now().to_string(),
        }
    }
}

/// RFQ status change event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RfqStatusEvent {
    /// RFQ ID.
    pub rfq_id: String,
    /// Previous state.
    pub previous_state: Option<RfqState>,
    /// Current state.
    pub current_state: RfqState,
    /// Event timestamp (ISO 8601).
    pub timestamp: String,
}

/// Trade update event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeEvent {
    /// Trade ID.
    pub trade_id: String,
    /// RFQ ID.
    pub rfq_id: String,
    /// Settlement state.
    pub settlement_state: String,
    /// Event timestamp (ISO 8601).
    pub timestamp: String,
}

impl From<&Trade> for TradeEvent {
    fn from(trade: &Trade) -> Self {
        Self {
            trade_id: trade.id().to_string(),
            rfq_id: trade.rfq_id().to_string(),
            settlement_state: trade.settlement_state().to_string(),
            timestamp: crate::domain::value_objects::timestamp::Timestamp::now().to_string(),
        }
    }
}

// ============================================================================
// WebSocket Messages
// ============================================================================

/// Message type for WebSocket communication.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageType {
    /// Subscribe to a channel.
    Subscribe,
    /// Unsubscribe from a channel.
    Unsubscribe,
    /// Event notification.
    Event,
    /// Error message.
    Error,
    /// Ping request.
    Ping,
    /// Pong response.
    Pong,
    /// Acknowledgment.
    Ack,
}

/// Incoming WebSocket message.
#[derive(Debug, Clone, Deserialize)]
pub struct IncomingMessage {
    /// Message type.
    #[serde(rename = "type")]
    pub msg_type: MessageType,
    /// Channel to subscribe/unsubscribe.
    #[serde(default)]
    pub channel: Option<String>,
    /// Optional payload.
    #[serde(default)]
    pub payload: Option<serde_json::Value>,
}

/// Outgoing WebSocket message.
#[derive(Debug, Clone, Serialize)]
pub struct OutgoingMessage {
    /// Message type.
    #[serde(rename = "type")]
    pub msg_type: MessageType,
    /// Channel (for events).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
    /// Payload.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<serde_json::Value>,
    /// Error message (for error type).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl OutgoingMessage {
    /// Creates an acknowledgment message.
    #[must_use]
    pub fn ack(channel: &str) -> Self {
        Self {
            msg_type: MessageType::Ack,
            channel: Some(channel.to_string()),
            payload: None,
            error: None,
        }
    }

    /// Creates an event message.
    #[must_use]
    pub fn event(channel: &str, payload: serde_json::Value) -> Self {
        Self {
            msg_type: MessageType::Event,
            channel: Some(channel.to_string()),
            payload: Some(payload),
            error: None,
        }
    }

    /// Creates an error message.
    #[must_use]
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            msg_type: MessageType::Error,
            channel: None,
            payload: None,
            error: Some(message.into()),
        }
    }

    /// Creates a pong message.
    #[must_use]
    pub fn pong() -> Self {
        Self {
            msg_type: MessageType::Pong,
            channel: None,
            payload: None,
            error: None,
        }
    }
}

// ============================================================================
// Channel Types
// ============================================================================

/// Parsed channel subscription.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Channel {
    /// Quote updates for a specific RFQ.
    RfqQuotes(String),
    /// Status updates for a specific RFQ.
    RfqStatus(String),
    /// Updates for a specific trade.
    Trade(String),
}

impl Channel {
    /// Parses a channel string into a Channel enum.
    ///
    /// # Errors
    ///
    /// Returns an error message if the channel format is invalid.
    pub fn parse(channel: &str) -> Result<Self, String> {
        let parts: Vec<&str> = channel.split('.').collect();

        match parts.as_slice() {
            ["rfq", id, "quotes"] => Ok(Channel::RfqQuotes(id.to_string())),
            ["rfq", id, "status"] => Ok(Channel::RfqStatus(id.to_string())),
            ["trades", id] => Ok(Channel::Trade(id.to_string())),
            _ => Err(format!("invalid channel format: {channel}")),
        }
    }

    /// Returns the channel string representation.
    #[must_use]
    pub fn to_channel_string(&self) -> String {
        match self {
            Channel::RfqQuotes(id) => format!("rfq.{id}.quotes"),
            Channel::RfqStatus(id) => format!("rfq.{id}.status"),
            Channel::Trade(id) => format!("trades.{id}"),
        }
    }
}

// ============================================================================
// Connection Query Parameters
// ============================================================================

/// Query parameters for WebSocket connection.
#[derive(Debug, Deserialize)]
pub struct ConnectionParams {
    /// Authentication token.
    pub token: Option<String>,
}

// ============================================================================
// WebSocket Handler
// ============================================================================

/// WebSocket upgrade handler.
///
/// # Errors
///
/// Returns an error response if authentication fails.
#[instrument(skip(state, ws))]
pub async fn ws_handler(
    State(state): State<Arc<WebSocketState>>,
    Query(params): Query<ConnectionParams>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    info!("WebSocket connection request");

    // Validate token if provided (placeholder for actual auth)
    if let Some(ref token) = params.token {
        if !validate_token(token) {
            warn!("Invalid WebSocket token");
            // In production, return an error response
        }
    }

    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

/// Validates an authentication token.
///
/// This is a placeholder implementation. In production, this would
/// validate against a JWT or API key store.
fn validate_token(token: &str) -> bool {
    // Placeholder: accept any non-empty token
    !token.is_empty()
}

/// Handles an established WebSocket connection.
async fn handle_socket(socket: WebSocket, state: Arc<WebSocketState>) {
    // Track connection
    {
        let mut count = state.active_connections.write().await;
        *count += 1;
        info!("WebSocket connected. Active connections: {}", *count);
    }

    let (mut sender, mut receiver) = socket.split();

    // Channel for sending messages to the client
    let (tx, mut rx) = mpsc::channel::<OutgoingMessage>(100);

    // Subscriptions for this connection
    let subscriptions: Arc<RwLock<HashSet<Channel>>> = Arc::new(RwLock::new(HashSet::new()));

    // Subscribe to broadcast channels
    let mut quote_rx = state.quote_events.subscribe();
    let mut rfq_status_rx = state.rfq_status_events.subscribe();
    let mut trade_rx = state.trade_events.subscribe();

    let subs_clone = Arc::clone(&subscriptions);
    let tx_clone = tx.clone();

    // Spawn task to forward broadcast events to subscribed clients
    let event_forwarder = tokio::spawn(async move {
        loop {
            tokio::select! {
                Ok(event) = quote_rx.recv() => {
                    let channel = Channel::RfqQuotes(event.rfq_id.clone());
                    let subs = subs_clone.read().await;
                    if subs.contains(&channel) {
                        let msg = OutgoingMessage::event(
                            &channel.to_channel_string(),
                            serde_json::to_value(&event).unwrap_or_default(),
                        );
                        let _ = tx_clone.send(msg).await;
                    }
                }
                Ok(event) = rfq_status_rx.recv() => {
                    let channel = Channel::RfqStatus(event.rfq_id.clone());
                    let subs = subs_clone.read().await;
                    if subs.contains(&channel) {
                        let msg = OutgoingMessage::event(
                            &channel.to_channel_string(),
                            serde_json::to_value(&event).unwrap_or_default(),
                        );
                        let _ = tx_clone.send(msg).await;
                    }
                }
                Ok(event) = trade_rx.recv() => {
                    let channel = Channel::Trade(event.trade_id.clone());
                    let subs = subs_clone.read().await;
                    if subs.contains(&channel) {
                        let msg = OutgoingMessage::event(
                            &channel.to_channel_string(),
                            serde_json::to_value(&event).unwrap_or_default(),
                        );
                        let _ = tx_clone.send(msg).await;
                    }
                }
                else => break,
            }
        }
    });

    // Spawn task to send messages to client
    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if let Ok(json) = serde_json::to_string(&msg) {
                if sender.send(Message::Text(json.into())).await.is_err() {
                    break;
                }
            }
        }
    });

    // Handle incoming messages
    let subs_for_recv = Arc::clone(&subscriptions);
    let tx_for_recv = tx.clone();
    let max_subs = state.config.max_subscriptions;

    while let Some(result) = receiver.next().await {
        match result {
            Ok(Message::Text(text)) => {
                if let Err(e) =
                    handle_text_message(&text, &subs_for_recv, &tx_for_recv, max_subs).await
                {
                    let _ = tx_for_recv.send(OutgoingMessage::error(e)).await;
                }
            }
            Ok(Message::Ping(_)) => {
                // Axum handles pong automatically, but we can also respond
                debug!("Received ping");
            }
            Ok(Message::Pong(_)) => {
                debug!("Received pong");
            }
            Ok(Message::Close(_)) => {
                info!("Client requested close");
                break;
            }
            Ok(Message::Binary(_)) => {
                let _ = tx_for_recv
                    .send(OutgoingMessage::error("binary messages not supported"))
                    .await;
            }
            Err(e) => {
                error!("WebSocket error: {}", e);
                break;
            }
        }
    }

    // Cleanup
    event_forwarder.abort();
    send_task.abort();

    // Track disconnection
    {
        let mut count = state.active_connections.write().await;
        *count = count.saturating_sub(1);
        info!("WebSocket disconnected. Active connections: {}", *count);
    }
}

/// Handles a text message from the client.
async fn handle_text_message(
    text: &str,
    subscriptions: &Arc<RwLock<HashSet<Channel>>>,
    tx: &mpsc::Sender<OutgoingMessage>,
    max_subscriptions: usize,
) -> Result<(), String> {
    let msg: IncomingMessage =
        serde_json::from_str(text).map_err(|e| format!("invalid JSON: {e}"))?;

    match msg.msg_type {
        MessageType::Subscribe => {
            let channel_str = msg.channel.ok_or("channel required for subscribe")?;
            let channel = Channel::parse(&channel_str)?;

            let mut subs = subscriptions.write().await;
            if subs.len() >= max_subscriptions {
                return Err(format!(
                    "maximum subscriptions ({max_subscriptions}) reached"
                ));
            }

            subs.insert(channel);
            let _ = tx.send(OutgoingMessage::ack(&channel_str)).await;
            info!("Subscribed to channel: {}", channel_str);
        }
        MessageType::Unsubscribe => {
            let channel_str = msg.channel.ok_or("channel required for unsubscribe")?;
            let channel = Channel::parse(&channel_str)?;

            let mut subs = subscriptions.write().await;
            subs.remove(&channel);
            let _ = tx.send(OutgoingMessage::ack(&channel_str)).await;
            info!("Unsubscribed from channel: {}", channel_str);
        }
        MessageType::Ping => {
            let _ = tx.send(OutgoingMessage::pong()).await;
        }
        _ => {
            return Err(format!("unsupported message type: {:?}", msg.msg_type));
        }
    }

    Ok(())
}

// ============================================================================
// Router
// ============================================================================

/// Creates the WebSocket router.
///
/// # Arguments
///
/// * `state` - Shared WebSocket state
///
/// # Returns
///
/// An axum Router configured with WebSocket endpoints.
pub fn create_ws_router(state: Arc<WebSocketState>) -> axum::Router {
    use axum::routing::get;

    axum::Router::new()
        .route("/", get(ws_handler))
        .with_state(state)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn channel_parse_rfq_quotes() {
        let channel = Channel::parse("rfq.abc123.quotes").unwrap();
        assert_eq!(channel, Channel::RfqQuotes("abc123".to_string()));
    }

    #[test]
    fn channel_parse_rfq_status() {
        let channel = Channel::parse("rfq.abc123.status").unwrap();
        assert_eq!(channel, Channel::RfqStatus("abc123".to_string()));
    }

    #[test]
    fn channel_parse_trade() {
        let channel = Channel::parse("trades.xyz789").unwrap();
        assert_eq!(channel, Channel::Trade("xyz789".to_string()));
    }

    #[test]
    fn channel_parse_invalid() {
        let result = Channel::parse("invalid.channel");
        assert!(result.is_err());
    }

    #[test]
    fn channel_to_string() {
        let channel = Channel::RfqQuotes("abc123".to_string());
        assert_eq!(channel.to_channel_string(), "rfq.abc123.quotes");

        let channel = Channel::RfqStatus("abc123".to_string());
        assert_eq!(channel.to_channel_string(), "rfq.abc123.status");

        let channel = Channel::Trade("xyz789".to_string());
        assert_eq!(channel.to_channel_string(), "trades.xyz789");
    }

    #[test]
    fn outgoing_message_ack() {
        let msg = OutgoingMessage::ack("rfq.123.quotes");
        assert_eq!(msg.msg_type, MessageType::Ack);
        assert_eq!(msg.channel, Some("rfq.123.quotes".to_string()));
    }

    #[test]
    fn outgoing_message_error() {
        let msg = OutgoingMessage::error("test error");
        assert_eq!(msg.msg_type, MessageType::Error);
        assert_eq!(msg.error, Some("test error".to_string()));
    }

    #[test]
    fn outgoing_message_pong() {
        let msg = OutgoingMessage::pong();
        assert_eq!(msg.msg_type, MessageType::Pong);
    }

    #[test]
    fn outgoing_message_event() {
        let payload = serde_json::json!({"key": "value"});
        let msg = OutgoingMessage::event("rfq.123.quotes", payload.clone());
        assert_eq!(msg.msg_type, MessageType::Event);
        assert_eq!(msg.payload, Some(payload));
    }

    #[test]
    fn websocket_config_default() {
        let config = WebSocketConfig::default();
        assert_eq!(config.heartbeat_interval_secs, 30);
        assert_eq!(config.connection_timeout_secs, 60);
        assert_eq!(config.max_message_size, 64 * 1024);
        assert_eq!(config.max_subscriptions, 100);
    }

    #[test]
    fn websocket_state_new() {
        let state = WebSocketState::new();
        assert_eq!(state.config.heartbeat_interval_secs, 30);
    }

    #[test]
    fn message_type_serialize() {
        let json = serde_json::to_string(&MessageType::Subscribe).unwrap();
        assert_eq!(json, "\"subscribe\"");
    }

    #[test]
    fn message_type_deserialize() {
        let msg_type: MessageType = serde_json::from_str("\"subscribe\"").unwrap();
        assert_eq!(msg_type, MessageType::Subscribe);
    }

    #[test]
    fn incoming_message_deserialize() {
        let json = r#"{"type": "subscribe", "channel": "rfq.123.quotes"}"#;
        let msg: IncomingMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.msg_type, MessageType::Subscribe);
        assert_eq!(msg.channel, Some("rfq.123.quotes".to_string()));
    }

    #[test]
    fn outgoing_message_serialize() {
        let msg = OutgoingMessage::ack("rfq.123.quotes");
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"ack\""));
        assert!(json.contains("\"channel\":\"rfq.123.quotes\""));
    }

    #[test]
    fn validate_token_non_empty() {
        assert!(validate_token("valid-token"));
    }

    #[test]
    fn validate_token_empty() {
        assert!(!validate_token(""));
    }

    #[tokio::test]
    async fn websocket_state_publish_quote() {
        let state = WebSocketState::new();
        let mut rx = state.quote_events.subscribe();

        let event = QuoteEvent {
            rfq_id: "rfq-1".to_string(),
            quote_id: "quote-1".to_string(),
            venue_id: "venue-1".to_string(),
            price: "100.00".to_string(),
            quantity: "10.0".to_string(),
            valid_until: "2024-01-01T00:00:00Z".to_string(),
            timestamp: "2024-01-01T00:00:00Z".to_string(),
        };

        state.publish_quote(event.clone());

        let received = rx.recv().await.unwrap();
        assert_eq!(received.rfq_id, "rfq-1");
        assert_eq!(received.quote_id, "quote-1");
    }

    #[tokio::test]
    async fn websocket_state_publish_rfq_status() {
        let state = WebSocketState::new();
        let mut rx = state.rfq_status_events.subscribe();

        let event = RfqStatusEvent {
            rfq_id: "rfq-1".to_string(),
            previous_state: Some(RfqState::Created),
            current_state: RfqState::QuoteRequesting,
            timestamp: "2024-01-01T00:00:00Z".to_string(),
        };

        state.publish_rfq_status(event.clone());

        let received = rx.recv().await.unwrap();
        assert_eq!(received.rfq_id, "rfq-1");
        assert_eq!(received.current_state, RfqState::QuoteRequesting);
    }

    #[tokio::test]
    async fn websocket_state_publish_trade() {
        let state = WebSocketState::new();
        let mut rx = state.trade_events.subscribe();

        let event = TradeEvent {
            trade_id: "trade-1".to_string(),
            rfq_id: "rfq-1".to_string(),
            settlement_state: "Pending".to_string(),
            timestamp: "2024-01-01T00:00:00Z".to_string(),
        };

        state.publish_trade(event.clone());

        let received = rx.recv().await.unwrap();
        assert_eq!(received.trade_id, "trade-1");
        assert_eq!(received.settlement_state, "Pending");
    }
}
