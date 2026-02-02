//! # Use Cases
//!
//! Application use cases implementing business workflows.
//!
//! Each use case orchestrates domain objects to perform a specific
//! business operation, handling validation, persistence, and events.

pub mod collect_quotes;
pub mod create_rfq;
pub mod execute_trade;

pub use collect_quotes::{
    CollectQuotesConfig, CollectQuotesResponse, CollectQuotesUseCase, QuoteEventPublisher,
    VenueQuoteResult, VenueRegistry,
};
pub use create_rfq::{
    ClientRepository, ComplianceService, CreateRfqUseCase, EventPublisher, InstrumentRegistry,
    RfqRepository,
};
