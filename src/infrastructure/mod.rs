//! # Infrastructure Layer
//!
//! External adapters and implementations of domain ports.
//!
//! ## Venues
//!
//! Adapters for liquidity venues:
//! - FIX protocol market makers
//! - DEX aggregators (0x, 1inch)
//! - RFQ protocols (Hashflow, Bebop)
//!
//! ## Persistence
//!
//! Repository implementations:
//! - PostgreSQL repositories
//! - In-memory repositories for testing
//! - Event store
//!
//! ## Blockchain
//!
//! On-chain execution clients for DeFi protocols.
//!
//! ## Last-Look
//!
//! Channel-specific implementations for MM confirmation protocol.

pub mod blockchain;
pub mod http_clients;
pub mod last_look;
pub mod messaging;
pub mod notifications;
pub mod persistence;
pub mod sbe;
pub mod streaming;
pub mod venues;

pub use last_look::{
    CompositeLastLookService, FixLastLookClient, GrpcLastLookClient, WebSocketLastLookClient,
};
pub use persistence as repos;
pub use streaming::{
    CompositeStreamingQuoteService, FixStreamingClient, GrpcStreamingClient,
    WebSocketStreamingClient,
};
pub use venues as venue_adapters;
