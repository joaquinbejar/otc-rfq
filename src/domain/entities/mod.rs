//! # Domain Entities
//!
//! Aggregate roots and entities representing core business concepts.
//!
//! ## Aggregates
//!
//! - [`Rfq`]: Request-for-Quote aggregate with state machine
//! - [`Trade`]: Executed trade aggregate
//! - [`BlockTrade`]: Pre-arranged bilateral block trade
//! - `Venue`: Liquidity venue configuration
//!
//! ## Entities
//!
//! - [`Quote`]: Price quote from a venue
//! - `Counterparty`: Client or market maker
//! - `MmPerformanceMetrics`: Market maker performance tracking

pub mod allocation;
pub mod block_trade;
pub mod counter_quote;
pub mod counterparty;
pub mod delayed_report;
pub mod mm_performance;
pub mod negotiation;
pub mod quote;
pub mod rfq;
pub mod trade;
pub mod venue;

#[cfg(test)]
mod tests;

pub use allocation::Allocation;
pub use block_trade::{
    BlockTrade, BlockTradeId, BlockTradeState, BlockTradeValidation, InvalidBlockTradeStateError,
};
pub use counter_quote::{CounterQuote, CounterQuoteBuilder};
pub use counterparty::{
    Counterparty, CounterpartyLimits, CounterpartyType, InvalidCounterpartyTypeError,
    InvalidKycStatusError, KycStatus, WalletAddress,
};
pub use delayed_report::{DelayedReport, TradeSummary};
pub use mm_performance::{
    DEFAULT_MIN_RESPONSE_RATE_PCT, DEFAULT_WINDOW_DAYS, MmPerformanceEvent, MmPerformanceEventKind,
    MmPerformanceMetrics,
};
pub use negotiation::{DEFAULT_MAX_ROUNDS, MAX_ALLOWED_ROUNDS, Negotiation, NegotiationRound};
pub use quote::{Quote, QuoteBuilder, QuoteMetadata};
pub use rfq::{ComplianceResult, Rfq, RfqBuilder};
pub use trade::{InvalidSettlementStateError, SettlementState, Trade};
pub use venue::{InvalidVenueHealthError, Venue, VenueConfig, VenueHealth, VenueMetrics};
