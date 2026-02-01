//! # Domain Events
//!
//! Events emitted during domain operations for event sourcing and audit trail.
//!
//! ## RFQ Events
//!
//! - `RfqCreated`: New RFQ initiated
//! - `QuoteReceived`: Quote received from venue
//! - `QuoteSelected`: Client selected a quote
//! - `TradeExecuted`: Trade successfully executed
//!
//! ## Settlement Events
//!
//! - `SettlementInitiated`: Settlement process started
//! - `SettlementConfirmed`: Settlement completed successfully

pub mod compliance_events;
pub mod domain_event;
pub mod rfq_events;
pub mod trade_events;
