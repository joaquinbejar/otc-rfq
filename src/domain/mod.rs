//! # Domain Layer
//!
//! Core business logic following Domain-Driven Design principles.
//!
//! This layer contains:
//! - **Entities**: Aggregate roots and domain entities (RFQ, Quote, Trade, Venue)
//! - **Value Objects**: Immutable types with validation (Price, Quantity, identifiers)
//! - **Events**: Domain events for event sourcing and audit trail
//! - **Errors**: Domain-specific error types
//! - **Services**: Domain services for complex business logic

pub mod entities;
pub mod errors;
pub mod events;
pub mod services;
pub mod value_objects;
