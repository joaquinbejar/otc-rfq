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
//! - `Price`: Decimal price with checked arithmetic
//! - `Quantity`: Decimal quantity with checked arithmetic
//!
//! ## Domain Enums
//!
//! - `OrderSide`: Buy or Sell
//! - `RfqState`: RFQ lifecycle states
//! - `VenueType`: Types of liquidity venues

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

pub use ids::{CounterpartyId, EventId, QuoteId, RfqId, TradeId, VenueId};
