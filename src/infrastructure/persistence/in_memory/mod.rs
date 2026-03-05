//! # In-Memory Repositories
//!
//! In-memory implementations for testing without database dependencies.
//!
//! ## Available Repositories
//!
//! - [`InMemoryRfqRepository`]: RFQ persistence
//! - [`InMemoryTradeRepository`]: Trade persistence
//! - [`InMemoryVenueRepository`]: Venue configuration persistence
//! - [`InMemoryCounterpartyRepository`]: Counterparty persistence
//! - [`InMemoryMmPerformanceRepository`]: MM performance event persistence
//! - [`InMemoryQuoteLockRepository`]: Quote locking for acceptance flow
//!
//! ## Thread Safety
//!
//! All implementations use `Arc<RwLock<HashMap>>` or `DashMap` for thread-safe access.

pub mod counterparty_repository;
pub mod mm_performance_repository;
pub mod mock_services;
pub mod quote_lock_repository;
pub mod rfq_repository;
pub mod trade_repository;
pub mod venue_repository;

pub use counterparty_repository::InMemoryCounterpartyRepository;
pub use mm_performance_repository::InMemoryMmPerformanceRepository;
pub use mock_services::{MockLastLookBehavior, MockLastLookService, MockRiskCheckService};
pub use quote_lock_repository::InMemoryQuoteLockRepository;
pub use rfq_repository::InMemoryRfqRepository;
pub use trade_repository::InMemoryTradeRepository;
pub use venue_repository::InMemoryVenueRepository;
