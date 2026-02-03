//! # WebSocket API
//!
//! WebSocket handlers for real-time streaming updates.
//!
//! This module provides WebSocket endpoints for streaming real-time updates
//! including quote updates, RFQ status changes, and trade notifications.
//!
//! # Endpoint
//!
//! - `GET /ws/v1` - WebSocket connection endpoint
//!
//! # Channels
//!
//! - `rfq.{id}.quotes` - Quote updates for a specific RFQ
//! - `rfq.{id}.status` - RFQ state changes
//! - `trades.{id}` - Trade status updates
//!
//! # Message Format
//!
//! All messages use JSON format:
//!
//! ```json
//! {
//!   "type": "subscribe|unsubscribe|event|error|ping|pong",
//!   "channel": "rfq.{id}.quotes",
//!   "payload": { ... }
//! }
//! ```
//!
//! # Usage
//!
//! ```ignore
//! use otc_rfq::api::websocket::{create_ws_router, WebSocketState};
//! use std::sync::Arc;
//!
//! let ws_state = Arc::new(WebSocketState::new());
//! let ws_router = create_ws_router(ws_state.clone());
//!
//! // Publish events
//! ws_state.publish_quote(quote_event);
//! ws_state.publish_rfq_status(status_event);
//! ws_state.publish_trade(trade_event);
//! ```

pub mod handlers;

pub use handlers::{
    Channel, ConnectionParams, IncomingMessage, MessageType, OutgoingMessage, QuoteEvent,
    RfqStatusEvent, TradeEvent, WebSocketConfig, WebSocketState, create_ws_router,
};
