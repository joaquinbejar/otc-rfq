//! # Domain Events
//!
//! Events emitted during domain operations for event sourcing and audit trail.
//!
//! ## RFQ Events
//!
//! - [`RfqCreated`]: New RFQ initiated
//! - [`QuoteCollectionStarted`]: Quote collection begins
//! - [`QuoteRequested`]: Quote requested from venue
//! - [`QuoteReceived`]: Quote received from venue
//! - [`QuoteRequestFailed`]: Quote request failed
//! - [`QuoteCollectionCompleted`]: Quote collection finished
//! - [`QuoteSelected`]: Client selected a quote
//! - [`ExecutionStarted`]: Trade execution begins
//! - [`ExecutionFailed`]: Trade execution failed
//! - [`RfqCancelled`]: RFQ was cancelled
//! - [`RfqExpired`]: RFQ expired
//!
//! ## Trade Events
//!
//! - [`TradeExecuted`]: Trade successfully executed
//! - [`SettlementInitiated`]: Settlement process started
//! - [`SettlementConfirmed`]: Settlement completed successfully
//! - [`SettlementFailed`]: Settlement failed
//!
//! ## Block Trade Events
//!
//! - [`BlockTradeSubmitted`]: Block trade submitted for validation
//! - [`BlockTradeValidated`]: Block trade validation completed
//! - [`BlockTradeConfirmed`]: Counterparty confirmed block trade
//! - [`BlockTradeApproved`]: Block trade approved (both parties confirmed)
//! - [`BlockTradeRejected`]: Block trade rejected
//! - [`BlockTradeExecuted`]: Block trade executed
//! - [`BlockTradeFailed`]: Block trade execution failed
//!
//! ## Acceptance Events
//!
//! - [`QuoteLocked`]: Quote locked for acceptance
//! - [`RiskCheckPassed`]: Risk check passed
//! - [`RiskCheckFailed`]: Risk check failed
//! - [`LastLookSent`]: Last-look request sent
//! - [`LastLookConfirmed`]: Last-look confirmed by MM
//! - [`LastLookRejected`]: Last-look rejected by MM
//! - [`LastLookTimeout`]: Last-look timed out
//! - [`AcceptanceCompleted`]: Acceptance flow completed
//! - [`AcceptanceFailed`]: Acceptance flow failed
//!
//! ## Compliance Events
//!
//! - [`ComplianceCheckPassed`]: Compliance check passed
//! - [`ComplianceCheckFailed`]: Compliance check failed

pub mod acceptance_events;
pub mod allocation_events;
pub mod block_trade_events;
pub mod compliance_events;
pub mod conflict_events;
pub mod domain_event;
pub mod negotiation_events;
pub mod off_book_events;
pub mod reporting_events;
pub mod rfq_events;
pub mod trade_events;

pub use acceptance_events::{
    AcceptanceCompleted, AcceptanceEvent, AcceptanceFailed, LastLookConfirmed, LastLookRejected,
    LastLookSent, LastLookTimeout, QuoteLocked, RiskCheckFailed, RiskCheckPassed,
};
pub use allocation_events::{
    AllocationEvent, AllocationExecuted, AllocationRolledBack, MultiMmFillAllocated,
};
pub use block_trade_events::{
    BlockTradeApproved, BlockTradeConfirmed, BlockTradeExecuted, BlockTradeFailed,
    BlockTradeRejected, BlockTradeRole, BlockTradeSubmitted, BlockTradeValidated,
};
pub use compliance_events::{
    ComplianceCheckFailed, ComplianceCheckPassed, ComplianceCheckType, ComplianceEvent,
};
pub use conflict_events::{ConflictDetectedEvent, ConflictEvent, ConflictResolvedEvent};
pub use domain_event::{DomainEvent, EventMetadata, EventType};
pub use negotiation_events::{
    CounterQuoteReceived as NegotiationCounterQuoteReceived, CounterQuoteSent,
    NegotiationCompleted, NegotiationEvent, NegotiationOutcome,
};
pub use off_book_events::{
    CollateralLocked, CollateralReleased, ExecutionStep, OffBookExecutionStarted, OffBookFailed,
    OffBookSettled, TradeHash,
};
pub use reporting_events::{BlockTradeReported, ReportScheduled};
pub use rfq_events::{
    ExecutionFailed, ExecutionStarted, QuoteCollectionCompleted, QuoteCollectionStarted,
    QuoteReceived, QuoteRequestFailed, QuoteRequested, QuoteSelected, RfqCancelled, RfqCreated,
    RfqEvent, RfqExpired,
};
pub use trade_events::{
    SettlementConfirmed, SettlementFailed, SettlementInitiated, TradeEvent, TradeExecuted,
};
