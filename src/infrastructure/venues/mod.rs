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
//!
//! ## Implementations
//!
//! - `dex`: DEX aggregator adapters
//! - `rfq_protocols`: RFQ protocol adapters (Hashflow, Bebop)
//! - `internal_mm`: Internal market maker adapter
//! - `fix_adapter`: FIX protocol adapter

pub mod dex;
pub mod error;
pub mod fix_adapter;
pub mod fix_config;
pub mod internal_mm;
pub mod registry;
pub mod rfq_protocols;
pub mod traits;

pub use error::{VenueError, VenueResult};
pub use internal_mm::{InternalMMAdapter, InternalMMConfig};
pub use registry::{VenueConfig, VenueRegistry};
pub use traits::{ExecutionResult, VenueAdapter, VenueHealth, VenueHealthStatus};
