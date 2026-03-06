//! # Domain Services
//!
//! Domain services encapsulating complex business logic that doesn't
//! naturally belong to a single entity or value object.
//!
//! ## Services
//!
//! - [`mm_performance::MmPerformanceTracker`]: Market maker performance tracking
//! - [`block_trade`]: Block trade size thresholds and reporting tiers
//! - [`acceptance_flow`]: Atomic quote acceptance workflow
//! - [`quote_lock`]: Distributed locking for quote acceptance
//! - [`risk_check`]: Pre-trade risk validation
//! - [`last_look`]: Market maker confirmation protocol
//! - [`conflict_resolver`]: Race condition handling with first-commit-wins

pub mod acceptance_flow;
pub mod block_trade;
pub mod conflict_resolver;
pub mod last_look;
pub mod mm_performance;
pub mod quote_lock;
pub mod risk_check;

pub use crate::domain::events::conflict_events::{ConflictType, Resolution};
pub use acceptance_flow::{
    AcceptanceFailureReason, AcceptanceFlow, AcceptanceFlowConfig, AcceptanceResult, TradeExecutor,
};
pub use block_trade::{BlockTradeConfig, ReportingTier, TierMultiplierError, TierMultipliers};
pub use conflict_resolver::{ConflictContext, ConflictResolver, FirstCommitWinsResolver};
pub use last_look::{LastLookConfig, LastLookResult, LastLookService, LastLookStats};
pub use quote_lock::{LockHolderId, QuoteLock, QuoteLockConfig, QuoteLockService};
pub use risk_check::{RiskCheckConfig, RiskCheckService, RiskResult};
