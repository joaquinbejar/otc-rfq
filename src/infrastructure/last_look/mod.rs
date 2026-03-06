//! # Last-Look Infrastructure
//!
//! Channel-specific implementations of the
//! [`LastLookService`](crate::domain::services::LastLookService) trait.
//!
//! This module provides concrete implementations for different communication
//! channels used to send last-look requests to market makers:
//!
//! - [`WebSocketLastLookClient`]: WebSocket-based last-look
//! - [`GrpcLastLookClient`]: gRPC-based last-look
//! - [`FixLastLookClient`]: FIX protocol-based last-look
//! - [`CompositeLastLookService`]: Routes to appropriate channel per venue

mod composite;
mod fix;
mod grpc;
mod websocket;

pub use composite::CompositeLastLookService;
pub use fix::FixLastLookClient;
pub use grpc::GrpcLastLookClient;
pub use websocket::WebSocketLastLookClient;
