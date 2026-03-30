//! # API Layer
//!
//! External interfaces for the OTC RFQ system.
//!
//! ## Protocols
//!
//! - **gRPC**: High-performance trading operations
//! - **REST**: Management and administrative operations
//! - **WebSocket**: Real-time streaming updates
//! - **SBE**: Ultra-low-latency binary protocol
//!
//! ## Middleware
//!
//! - Authentication (JWT, API keys)
//! - Rate limiting
//! - Request logging and tracing

pub mod grpc;
pub mod middleware;
pub mod rest;
pub mod sbe;
#[cfg(test)]
mod tests;
pub mod websocket;

pub use grpc as grpc_api;
pub use rest as rest_api;
pub use sbe as sbe_api;
pub use websocket as ws_api;
