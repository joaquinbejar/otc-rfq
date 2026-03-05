//! # Domain Services
//!
//! Domain services encapsulating complex business logic that doesn't
//! naturally belong to a single entity or value object.
//!
//! ## Services
//!
//! - [`mm_performance::MmPerformanceTracker`]: Market maker performance tracking
//! - [`block_trade`]: Block trade size thresholds and reporting tiers

pub mod block_trade;
pub mod mm_performance;

pub use block_trade::{BlockTradeConfig, ReportingTier, TierMultiplierError, TierMultipliers};
