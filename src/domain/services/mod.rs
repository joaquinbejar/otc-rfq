//! # Domain Services
//!
//! Domain services encapsulating complex business logic that doesn't
//! naturally belong to a single entity or value object.
//!
//! ## Services
//!
//! - [`mm_performance::MmPerformanceTracker`]: Market maker performance tracking
//! - [`block_trade`]: Block trade size thresholds and reporting tiers
//! - [`block_trade_service`]: Bilateral block trade validation
//! - [`acceptance_flow`]: Atomic quote acceptance workflow
//! - [`quote_lock`]: Distributed locking for quote acceptance
//! - [`risk_check`]: Pre-trade risk validation
//! - [`last_look`]: Market maker confirmation protocol
//! - [`conflict_resolver`]: Race condition handling with first-commit-wins

pub mod acceptance_flow;
pub mod block_trade;
pub mod block_trade_service;
pub mod collateral_lock;
pub mod conflict_resolver;
pub mod last_look;
pub mod market_calendar;
pub mod mm_performance;
pub mod off_book_executor;
pub mod position_service;
pub mod quote_lock;
pub mod report_publisher;
pub mod report_scheduler;
pub mod risk_check;
pub mod settlement;

pub use crate::domain::events::conflict_events::{ConflictType, Resolution};
pub use acceptance_flow::{
    AcceptanceFailureReason, AcceptanceFlow, AcceptanceFlowConfig, AcceptanceResult, TradeExecutor,
};
pub use block_trade::{BlockTradeConfig, ReportingTier, TierMultiplierError, TierMultipliers};
pub use block_trade_service::{
    BlockTradeValidationContext, BlockTradeValidationFailure, BlockTradeValidator,
    DefaultBlockTradeValidator,
};
pub use conflict_resolver::{
    ConflictContext, ConflictResolver, CounterPriceInfo, FirstCommitWinsResolver,
};
pub use last_look::{
    LastLookConfig, LastLookRejectReason, LastLookResult, LastLookService, LastLookStats,
    MmLastLookConfig,
};
pub use quote_lock::{LockHolderId, QuoteLock, QuoteLockConfig, QuoteLockService};
pub use risk_check::{RiskCheckConfig, RiskCheckService, RiskResult};

pub use collateral_lock::{CollateralLockHandle, CollateralLockService};
pub use market_calendar::{
    MarketCalendarConfig, delay_until_market_close, next_market_close, next_market_close_default,
};
pub use off_book_executor::{ExecutedBlockTrade, OffBookExecutor, OffBookExecutorConfig};
pub use position_service::{Position, PositionUpdateService};
pub use report_publisher::{PublishResult, ReportPublisher};
pub use report_scheduler::{ReportScheduler, ReportSchedulerConfig, ScheduledReport};
pub use settlement::{Fees, SettlementResult, SettlementService};
