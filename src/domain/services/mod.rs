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
//! - [`atomic_matcher`]: Atomic all-or-nothing trade execution
//! - [`lock_manager`]: Distributed lock management
//! - [`resource_lock`]: Resource lock types for atomic execution

pub mod acceptance_flow;
pub mod atomic_matcher;
pub mod block_trade;
pub mod block_trade_service;
pub mod collateral_lock;
pub mod compensating_trade;
pub mod conflict_resolver;
pub mod last_look;
pub mod lock_manager;
pub mod market_calendar;
pub mod mm_performance;
pub mod multi_leg_executor;
pub mod off_book_executor;
pub mod package_quote_validator;
pub mod position_service;
pub mod price_discovery_service;
pub mod quote_lock;
pub mod quote_normalizer;
pub mod resource_lock;
// pub mod quote_aggregator; // TODO: Module not yet implemented
pub mod report_publisher;
pub mod report_scheduler;
pub mod risk_check;
pub mod settlement;
pub mod theoretical_pricer;

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
pub use price_discovery_service::{LiquidityMetrics, PriceDiscoveryConfig, PriceDiscoveryService};
pub use report_publisher::{PublishResult, ReportPublisher};
pub use report_scheduler::{ReportScheduler, ReportSchedulerConfig, ScheduledReport};
pub use settlement::{Fees, SettlementResult, SettlementService};
pub use theoretical_pricer::TheoreticalPricer;

pub use atomic_matcher::{AtomicExecutionResult, AtomicMatcher, AtomicMatcherConfig};
pub use compensating_trade::{CompensatingTrade, CompensatingTradeGenerator, LegExecutionResult};
pub use lock_manager::{LockGuard, LockInfo, LockManager, LockManagerConfig, SharedLockManager};
pub use multi_leg_executor::{
    LegExecutor, MultiLegExecutionResult, MultiLegExecutionState, MultiLegExecutor,
    MultiLegExecutorConfig,
};
pub use package_quote_validator::{DEFAULT_TOLERANCE_BPS, PackageQuoteValidator};
pub use quote_normalizer::QuoteNormalizer;
pub use resource_lock::{ResourceLock, sort_locks};
