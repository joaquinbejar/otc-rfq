//! # Venue Adapters
//!
//! Implementations of the VenueAdapter trait for different liquidity sources.
//!
//! ## Core Types
//!
//! - [`VenueAdapter`]: Trait defining the interface for venue integrations
//! - [`VenueError`]: Error type for venue operations
//! - [`VenueResult`]: Result type alias for venue operations
//! - [`ExecutionResult`]: Result of a trade execution
//! - [`VenueHealth`]: Health information for a venue
//! - [`VenueHealthStatus`]: Health status enum
//! - [`VenueRegistry`]: Registry for managing venue adapters
//! - [`VenueConfig`]: Configuration for registered venues
//! - [`InternalMMAdapter`]: Internal market maker adapter
//! - [`InternalMMConfig`]: Configuration for internal market maker
//! - [`FixMMAdapter`]: FIX protocol market maker adapter
//! - [`FixMMConfig`]: Configuration for FIX market maker
//! - [`FixSessionConfig`]: FIX session configuration
//!
//! ## IronFix Integration
//!
//! The FIX adapter uses IronFix for protocol encoding and session management:
//! - [`fix_adapter::FixEncoder`]: Re-exported `ironfix_tagvalue::Encoder`
//! - [`fix_messages`]: Type-safe FIX message builders
//! - [`fix_session`]: Session management with sequence and heartbeat handling
//!
//! ## Implementations
//!
//! - `dex`: DEX aggregator adapters
//! - `rfq_protocols`: RFQ protocol adapters (Hashflow, Bebop)
//! - `internal_mm`: Internal market maker adapter
//! - `fix_adapter`: FIX protocol adapter with IronFix encoding
//! - `fix_session`: FIX session management with IronFix

pub mod dex;
pub mod error;
pub mod fix_adapter;
pub mod fix_config;
pub mod fix_messages;
pub mod fix_session;
pub mod fix_simulator;
pub mod internal_mm;
pub mod registry;
pub mod rfq_protocols;
pub mod traits;

#[cfg(test)]
mod tests;

pub use error::{VenueError, VenueResult};
pub use fix_adapter::{FixMMAdapter, SessionState};
pub use fix_config::{FixMMConfig, FixSessionConfig, FixVersion, LogonCredentials, TlsConfig};
pub use fix_session::{FixSession, FixSessionState};
pub use internal_mm::{InternalMMAdapter, InternalMMConfig};
pub use registry::{VenueConfig, VenueRegistry};
pub use traits::{ExecutionResult, VenueAdapter, VenueHealth, VenueHealthStatus};
