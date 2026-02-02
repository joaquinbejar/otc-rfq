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
pub use error::{ApplicationError, ApplicationResult};
pub use services::{
    AggregationConfig, AggregationError, AggregationResult, BestPriceStrategy, CompositeStrategy,
    CompositeStrategyBuilder, CostConfig, LowestCostStrategy, LowestSlippageStrategy,
    QuoteAggregationEngine, RankedQuote, RankingStrategy, WeightedScoreStrategy,
};
pub use use_cases::{
    ClientRepository, CollectQuotesConfig, CollectQuotesResponse, CollectQuotesUseCase,
    ComplianceService, CreateRfqUseCase, EventPublisher, ExecuteTradeRequest, ExecuteTradeResponse,
    ExecuteTradeUseCase, InstrumentRegistry, QuoteEventPublisher, RfqRepository,
    TradeEventPublisher, TradeRepository, VenueQuoteResult, VenueRegistry,
};
