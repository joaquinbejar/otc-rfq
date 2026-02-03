//! # Application Layer
//!
//! Use case orchestration and application services.
//!
//! This layer coordinates domain objects to perform business operations,
//! handling transactions, authorization, and cross-cutting concerns.
//!
//! ## Use Cases
//!
//! - [`CreateRfqUseCase`]: Create a new RFQ request
//! - `CollectQuotesUseCase`: Gather quotes from venues
//! - `ExecuteTradeUseCase`: Execute a trade against a quote
//!
//! ## Services
//!
//! - `QuoteAggregationEngine`: Orchestrates quote collection and ranking
//! - `ComplianceService`: KYC/AML and limit checks

pub mod dto;
pub mod error;
pub mod services;
pub mod use_cases;

pub use dto::{CreateRfqRequest, CreateRfqResponse};
pub use error::{ApplicationError, ApplicationResult, InfrastructureError};
pub use services::{
    AggregationConfig, AggregationError, AggregationResult, AlwaysRetryable, AmlProvider,
    AmlResult, BestPriceStrategy, CircuitBreaker, CircuitBreakerConfig, CircuitBreakerError,
    CircuitBreakerResult, CircuitState, ComplianceCheckResult, ComplianceConfig, ComplianceFlag,
    ComplianceFlagType, ComplianceServiceImpl, ComplianceSeverity, CompositeStrategy,
    CompositeStrategyBuilder, CostConfig, KycProvider, KycStatus, LimitsProvider, LimitsResult,
    LowestCostStrategy, LowestSlippageStrategy, NeverRetryable, QuoteAggregationEngine,
    RankedQuote, RankingStrategy, RetryError, RetryPolicy, RetryResult, Retryable,
    SanctionsProvider, SanctionsResult, WeightedScoreStrategy, execute_with_retry,
};
pub use use_cases::{
    ClientRepository, CollectQuotesConfig, CollectQuotesResponse, CollectQuotesUseCase,
    ComplianceService, CreateRfqUseCase, EventPublisher, ExecuteTradeRequest, ExecuteTradeResponse,
    ExecuteTradeUseCase, InstrumentRegistry, QuoteEventPublisher, RfqRepository,
    TradeEventPublisher, TradeRepository, VenueQuoteResult, VenueRegistry,
};
