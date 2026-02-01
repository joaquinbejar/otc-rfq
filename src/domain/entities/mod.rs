//! # Domain Entities
//!
//! Aggregate roots and entities representing core business concepts.
//!
//! ## Aggregates
//!
//! - [`Rfq`]: Request-for-Quote aggregate with state machine
//! - `Trade`: Executed trade aggregate
//! - `Venue`: Liquidity venue configuration
//!
//! ## Entities
//!
//! - [`Quote`]: Price quote from a venue
//! - `Counterparty`: Client or market maker

pub mod counterparty;
pub mod quote;
pub mod rfq;
pub mod trade;
pub mod venue;

pub use counterparty::{
    Counterparty, CounterpartyLimits, CounterpartyType, InvalidCounterpartyTypeError,
    InvalidKycStatusError, KycStatus, WalletAddress,
};
pub use quote::{Quote, QuoteBuilder, QuoteMetadata};
pub use rfq::{ComplianceResult, Rfq, RfqBuilder};
pub use trade::{InvalidSettlementStateError, SettlementState, Trade};
pub use venue::{InvalidVenueHealthError, Venue, VenueConfig, VenueHealth, VenueMetrics};
