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

pub mod blockchain;
pub mod http_clients;
pub mod persistence;
pub mod sbe;
pub mod venues;

pub use persistence as repos;
pub use venues as venue_adapters;
