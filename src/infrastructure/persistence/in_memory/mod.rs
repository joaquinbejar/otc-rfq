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
//! - [`InMemoryNegotiationAuditLog`]: Negotiation audit log with μs precision
//! - [`InMemoryBlockTradeRepository`]: Block trade persistence
//!
//! ## Thread Safety
//!
//! All implementations use appropriate synchronization primitives (e.g. `Arc<RwLock<HashMap<_>>>`,
//! `DashMap`, or `Mutex<Vec<_>>`) for thread-safe access.

pub mod audit_log_repository;
pub mod block_trade_repository;
pub mod counterparty_repository;
pub mod delayed_report_repository;
pub mod mm_performance_repository;
pub mod mock_services;
pub mod quote_lock_repository;
pub mod rfq_repository;
pub mod trade_repository;
pub mod venue_repository;

pub use super::traits::BlockTradeRepository;
pub use audit_log_repository::InMemoryNegotiationAuditLog;
pub use block_trade_repository::InMemoryBlockTradeRepository;
pub use counterparty_repository::InMemoryCounterpartyRepository;
pub use delayed_report_repository::InMemoryDelayedReportRepository;
pub use mm_performance_repository::InMemoryMmPerformanceRepository;
pub use mock_services::{MockLastLookBehavior, MockLastLookService, MockRiskCheckService};
pub use quote_lock_repository::InMemoryQuoteLockRepository;
pub use rfq_repository::InMemoryRfqRepository;
pub use trade_repository::InMemoryTradeRepository;
pub use venue_repository::InMemoryVenueRepository;
