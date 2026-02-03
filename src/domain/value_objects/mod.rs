//! # Value Objects
//!
//! Immutable types with validation and domain semantics.
//!
//! ## Identity Types
//!
//! - [`RfqId`], [`QuoteId`], [`TradeId`]: UUID-based identifiers
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
pub mod enums;
pub mod ids;
pub mod instrument;
pub mod price;
pub mod quantity;
pub mod rfq_state;
pub mod symbol;
pub mod timestamp;

#[cfg(test)]
mod tests;

pub use arithmetic::{ArithmeticError, ArithmeticResult, CheckedArithmetic, Rounding, div_round};
pub use compliance::{ComplianceCheckResults, ComplianceCheckResultsBuilder, RegulatoryFlag};
pub use enums::{AssetClass, Blockchain, OrderSide, ParseEnumError, SettlementMethod, VenueType};
pub use ids::{CounterpartyId, EventId, QuoteId, RfqId, TradeId, VenueId};
pub use instrument::{Instrument, InstrumentBuilder};
pub use price::Price;
pub use quantity::Quantity;
pub use rfq_state::{InvalidRfqStateError, RfqState};
pub use symbol::{Symbol, SymbolError};
