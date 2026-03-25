//! # Value Objects
//!
//! Immutable types with validation and domain semantics.
//!
//! ## Identity Types
//!
//! - [`RfqId`], [`QuoteId`], [`TradeId`], [`BlockTradeId`]: UUID-based identifiers
//! - [`VenueId`], [`CounterpartyId`]: String-based identifiers
//! - [`EventId`]: Domain event identifier
//!
//! ## Numeric Types
//!
//! - [`Price`]: Decimal price with checked arithmetic
//! - [`Quantity`]: Decimal quantity with checked arithmetic
//!
//! ## Arithmetic
//!
//! - [`ArithmeticError`]: Error type for arithmetic failures
//! - [`CheckedArithmetic`]: Trait for safe arithmetic operations
//! - [`Rounding`]: Enum for explicit rounding direction
//!
//! ## Domain Enums
//!
//! - [`OrderSide`]: Buy or Sell
//! - [`AssetClass`]: Asset classification
//! - [`Blockchain`]: Supported blockchain networks
//! - [`VenueType`]: Types of liquidity venues
//! - [`SettlementMethod`]: On-chain or off-chain settlement
//!
//! ## Trading Types
//!
//! - [`Symbol`]: Trading pair representation (e.g., BTC/USD)
//! - [`Instrument`]: Tradeable instrument with metadata
//!
//! ## State Types
//!
//! - [`RfqState`]: RFQ lifecycle state machine
//!
//! ## Compliance Types
//!
//! - [`ComplianceCheckResults`]: Results of KYC/AML checks
//! - [`RegulatoryFlag`]: Regulatory flags raised during compliance checks

pub mod arithmetic;
pub mod compliance;
pub mod confirmation;
pub mod enums;
pub mod ids;
pub mod instrument;
pub mod liquidity_classification;
pub mod negotiation_state;
pub mod notification_preferences;
pub mod price;
pub mod price_discovery;
pub mod price_improvement;
pub mod quantity;
pub mod reference_price;
pub mod rfq_state;
pub mod size_negotiation_mode;
pub mod spread_metrics;
pub mod strategy;
pub mod symbol;
pub mod timestamp;
pub mod trade_type;

#[cfg(test)]
mod tests;

pub use arithmetic::{ArithmeticError, ArithmeticResult, CheckedArithmetic, Rounding, div_round};
pub use compliance::{ComplianceCheckResults, ComplianceCheckResultsBuilder, RegulatoryFlag};
pub use confirmation::{
    ChannelDeliveryStatus, ConfirmationChannel, ConfirmationStatus, NotificationDestination,
    TradeConfirmation, TradeParticipant,
};
pub use enums::{AssetClass, Blockchain, OrderSide, ParseEnumError, SettlementMethod, VenueType};
pub use ids::{
    BlockTradeId, CounterpartyId, EventId, NegotiationId, PackageQuoteId, QuoteId, RfqId, TradeId,
    VenueId,
};
pub use instrument::{Instrument, InstrumentBuilder};
pub use liquidity_classification::LiquidityClassification;
pub use negotiation_state::{InvalidNegotiationStateError, NegotiationState};
pub use notification_preferences::NotificationPreferences;
pub use price::Price;
pub use price_discovery::{PriceDiscoveryMethod, TheoreticalPrice};
pub use price_improvement::{ImprovementSource, PriceImprovement};
pub use quantity::Quantity;
pub use reference_price::{PriceBoundsConfig, PriceBoundsResult, ReferencePriceSource};
pub use rfq_state::{InvalidRfqStateError, RfqState};
pub use size_negotiation_mode::SizeNegotiationMode;
pub use spread_metrics::{EffectiveSpread, RealizedSpread, SpreadMetrics};
pub use strategy::{Strategy, StrategyBuilder, StrategyLeg, StrategyType};
pub use symbol::{Symbol, SymbolError};
pub use timestamp::Timestamp;
pub use trade_type::TradeType;
