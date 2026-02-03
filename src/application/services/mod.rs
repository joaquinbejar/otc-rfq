//! # Application Services
//!
//! Services that orchestrate domain logic and infrastructure.
//!
//! This module provides application-level services including:
//! - [`QuoteAggregationEngine`]: Concurrent quote collection and ranking
//! - [`RankingStrategy`]: Strategies for ranking quotes

pub mod circuit_breaker;
pub mod compliance;
pub mod quote_aggregation;
pub mod ranking_strategy;
pub mod retry;

pub use circuit_breaker::{
    CircuitBreaker, CircuitBreakerConfig, CircuitBreakerError, CircuitBreakerResult, CircuitState,
};
pub use compliance::{
    AmlProvider, AmlResult, ComplianceCheckResult, ComplianceConfig, ComplianceFlag,
    ComplianceFlagType, ComplianceServiceImpl, ComplianceSeverity, KycProvider, KycStatus,
    LimitsProvider, LimitsResult, SanctionsProvider, SanctionsResult,
};
pub use quote_aggregation::{
    AggregationConfig, AggregationError, AggregationResult, QuoteAggregationEngine,
};
pub use ranking_strategy::{
    BestPriceStrategy, CompositeStrategy, CompositeStrategyBuilder, CostConfig, LowestCostStrategy,
    LowestSlippageStrategy, RankedQuote, RankingStrategy, WeightedScoreStrategy,
};
pub use retry::{
    AlwaysRetryable, NeverRetryable, RetryError, RetryPolicy, RetryResult, Retryable,
    execute_with_retry,
};
