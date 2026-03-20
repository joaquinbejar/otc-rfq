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
pub mod anonymity;
pub mod block_trade;
pub mod counter_quote;
pub mod counterparty;
pub mod delayed_report;
pub mod mm_capacity;
pub mod mm_incentive;
pub mod mm_performance;
pub mod negotiation;
pub mod package_quote;
pub mod quote;
pub mod quote_normalizer;
pub mod rfq;
pub mod settlement;
pub mod streaming_quote;
pub mod trade;
pub mod venue;

#[cfg(test)]
mod tests;

pub use allocation::Allocation;
pub use anonymity::{AnonymityLevel, AnonymousRfqView, IdentityMapping};
pub use block_trade::{
    BlockTrade, BlockTradeId, BlockTradeState, BlockTradeValidation, InvalidBlockTradeStateError,
};
pub use counter_quote::{CounterQuote, CounterQuoteBuilder};
pub use counterparty::{
    Counterparty, CounterpartyLimits, CounterpartyType, InvalidCounterpartyTypeError,
    InvalidKycStatusError, KycStatus, WalletAddress,
};
pub use delayed_report::{DelayedReport, TradeSummary};
pub use mm_capacity::{
    CapacityAdjustment, CapacityCheckResult, CapacityReservation, DEFAULT_MAX_CONCURRENT_QUOTES,
    DEFAULT_MAX_NOTIONAL_USD, MmCapacityConfig, MmCapacityConfigBuilder, MmCapacityState,
};
pub use mm_incentive::{
    IncentiveConfig, IncentiveConfigBuilder, IncentiveResult, IncentiveTier, MmIncentiveStatus,
    PenaltyResult, compute_incentive, evaluate_penalties, volume_to_next_tier,
};
pub use mm_performance::{
    DEFAULT_MIN_RESPONSE_RATE_PCT, DEFAULT_WINDOW_DAYS, MmPerformanceEvent, MmPerformanceEventKind,
    MmPerformanceMetrics,
};
pub use negotiation::{DEFAULT_MAX_ROUNDS, MAX_ALLOWED_ROUNDS, Negotiation, NegotiationRound};
pub use package_quote::{LegPrice, PackageQuote, PackageQuoteBuilder};
pub use quote::{Quote, QuoteBuilder, QuoteMetadata};
pub use quote_normalizer::{
    FxRate, NormalizationConfig, NormalizationConfigBuilder, NormalizationConfigRegistry,
    NormalizedQuote, QuoteType,
};
pub use rfq::{ComplianceResult, Rfq, RfqBuilder};
pub use settlement::{
    IncentiveEvent, IncentiveSettlement, SettlementError, SettlementId, SettlementPeriod,
    SettlementStatus,
};
pub use streaming_quote::{
    BestQuote, DEFAULT_MAX_QUOTES_PER_SECOND, DEFAULT_STALENESS_CHECK_INTERVAL_MS, DEFAULT_TTL_MS,
    PriceLevel, StreamingQuote, StreamingQuoteConfig, StreamingQuoteConfigBuilder,
    StreamingQuoteId, StreamingQuoteStats,
};
pub use trade::{InvalidSettlementStateError, SettlementState, Trade};
pub use venue::{InvalidVenueHealthError, Venue, VenueConfig, VenueHealth, VenueMetrics};
