//! # PostgreSQL Repositories
//!
//! PostgreSQL implementations of repository traits using sqlx.
//!
//! ## Available Repositories
//!
//! - [`PostgresRfqRepository`]: RFQ persistence with JSONB
//! - [`PostgresTradeRepository`]: Trade persistence with optimistic locking
//! - [`PostgresVenueRepository`]: Venue configuration persistence
//! - [`PostgresCounterpartyRepository`]: Counterparty persistence
//! - [`PostgresEventStore`]: Append-only event storage
//!
//! ## Features
//!
//! - Connection pooling via `sqlx::PgPool`
//! - Optimistic locking with version fields
//! - JSONB serialization for complex fields
//! - Append-only event store for event sourcing

pub mod counterparty_repository;
pub mod event_store;
pub mod rfq_repository;
#[cfg(test)]
mod tests;
pub mod trade_repository;
pub mod venue_repository;

pub use counterparty_repository::PostgresCounterpartyRepository;
pub use event_store::PostgresEventStore;
pub use rfq_repository::PostgresRfqRepository;
pub use trade_repository::PostgresTradeRepository;
pub use venue_repository::PostgresVenueRepository;
