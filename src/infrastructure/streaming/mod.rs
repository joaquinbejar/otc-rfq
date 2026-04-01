//! # Streaming Quote Infrastructure
//!
//! Channel-specific implementations of the
//! [`StreamingQuoteService`](crate::domain::services::streaming_quote::StreamingQuoteService) trait.
//!
//! This module provides concrete implementations for different communication
//! channels used to receive streaming quotes from market makers:
//!
//! - [`WebSocketStreamingClient`]: WebSocket-based streaming
//! - [`GrpcStreamingClient`]: gRPC-based streaming
//! - [`FixStreamingClient`]: FIX protocol-based streaming (MassQuote)
//! - [`CompositeStreamingQuoteService`]: Routes to appropriate channel per venue

mod composite;
mod fix;
mod grpc;
mod websocket;

pub use composite::{CompositeStreamingQuoteService, StreamingChannel, VenueStreamingConfig};
pub use fix::{FixStreamingClient, FixStreamingConfig};
pub use grpc::{GrpcStreamingClient, GrpcStreamingConfig};
pub use websocket::{WebSocketStreamingClient, WebSocketStreamingConfig};
