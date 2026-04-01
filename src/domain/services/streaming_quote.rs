//! # Streaming Quote Service
//!
//! Domain service trait for continuous streaming quote reception and aggregation.
//!
//! This module provides the [`StreamingQuoteService`] trait that defines the interface
//! for receiving, validating, and aggregating streaming quotes from market makers.
//!
//! # Architecture
//!
//! The streaming quote service follows the same pattern as `LastLookService`:
//! - A domain trait defines the protocol-agnostic interface
//! - Infrastructure implementations handle WebSocket, gRPC, and FIX protocols
//! - A composite service routes to the appropriate channel per venue
//!
//! ```text
//! MM pushes quote
//!        |
//!        v
//! StreamingQuoteService::receive_quote()
//!        |
//!        +---> Validate (spread, size, rate limit)
//!        |
//!        +---> Update instrument book
//!        |
//!        +---> Recalculate best bid/ask
//! ```
//!
//! # Examples
//!
//! ```ignore
//! use otc_rfq::domain::services::streaming_quote::StreamingQuoteService;
//!
//! async fn process_quote(service: &dyn StreamingQuoteService, quote: StreamingQuote) {
//!     match service.receive_quote(quote).await {
//!         StreamingQuoteResult::Accepted { quote_id } => {
//!             println!("Quote accepted: {}", quote_id);
//!         }
//!         StreamingQuoteResult::Rejected { reason } => {
//!             println!("Quote rejected: {}", reason);
//!         }
//!     }
//! }
//! ```

use crate::domain::entities::streaming_quote::{
    BestQuote, StreamingQuote, StreamingQuoteConfig, StreamingQuoteId, StreamingQuoteStats,
};
use crate::domain::value_objects::{CounterpartyId, Instrument};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt;

// ============================================================================
// StreamingQuoteResult
// ============================================================================

/// Result of processing a streaming quote.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StreamingQuoteResult {
    /// Quote was accepted and added to the book.
    Accepted {
        /// The assigned quote ID.
        quote_id: StreamingQuoteId,
        /// Whether this quote is now the best bid or ask.
        is_best: bool,
    },
    /// Quote was rejected.
    Rejected {
        /// Reason for rejection.
        reason: StreamingQuoteRejectReason,
    },
}

impl StreamingQuoteResult {
    /// Creates an accepted result.
    #[must_use]
    pub fn accepted(quote_id: StreamingQuoteId, is_best: bool) -> Self {
        Self::Accepted { quote_id, is_best }
    }

    /// Creates a rejected result.
    #[must_use]
    pub fn rejected(reason: StreamingQuoteRejectReason) -> Self {
        Self::Rejected { reason }
    }

    /// Returns true if the quote was accepted.
    #[inline]
    #[must_use]
    pub fn is_accepted(&self) -> bool {
        matches!(self, Self::Accepted { .. })
    }

    /// Returns true if the quote was rejected.
    #[inline]
    #[must_use]
    pub fn is_rejected(&self) -> bool {
        matches!(self, Self::Rejected { .. })
    }
}

// ============================================================================
// StreamingQuoteRejectReason
// ============================================================================

/// Reasons for rejecting a streaming quote.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StreamingQuoteRejectReason {
    /// Quote has already expired (stale).
    QuoteStale,
    /// MM is sending quotes too fast.
    RateLimitExceeded {
        /// Current rate (quotes per second).
        current_rate: u32,
        /// Maximum allowed rate.
        max_rate: u32,
    },
    /// Spread exceeds maximum allowed.
    InvalidSpread {
        /// Actual spread in basis points.
        spread_bps: rust_decimal::Decimal,
        /// Maximum allowed spread.
        max_spread_bps: rust_decimal::Decimal,
    },
    /// Quote size is below minimum.
    InvalidSize {
        /// Actual size.
        size: rust_decimal::Decimal,
        /// Minimum required size.
        min_size: rust_decimal::Decimal,
    },
    /// MM is not registered for streaming.
    UnknownMM,
    /// Instrument is not supported for streaming.
    InstrumentNotSupported,
    /// Internal error during processing.
    InternalError(String),
}

impl StreamingQuoteRejectReason {
    /// Creates a stale quote rejection.
    #[must_use]
    pub fn stale() -> Self {
        Self::QuoteStale
    }

    /// Creates a rate limit rejection.
    #[must_use]
    pub fn rate_limited(current_rate: u32, max_rate: u32) -> Self {
        Self::RateLimitExceeded {
            current_rate,
            max_rate,
        }
    }

    /// Creates an invalid spread rejection.
    #[must_use]
    pub fn invalid_spread(
        spread_bps: rust_decimal::Decimal,
        max_spread_bps: rust_decimal::Decimal,
    ) -> Self {
        Self::InvalidSpread {
            spread_bps,
            max_spread_bps,
        }
    }

    /// Creates an invalid size rejection.
    #[must_use]
    pub fn invalid_size(size: rust_decimal::Decimal, min_size: rust_decimal::Decimal) -> Self {
        Self::InvalidSize { size, min_size }
    }

    /// Creates an unknown MM rejection.
    #[must_use]
    pub fn unknown_mm() -> Self {
        Self::UnknownMM
    }

    /// Creates an instrument not supported rejection.
    #[must_use]
    pub fn instrument_not_supported() -> Self {
        Self::InstrumentNotSupported
    }

    /// Creates an internal error rejection.
    #[must_use]
    pub fn internal_error(msg: impl Into<String>) -> Self {
        Self::InternalError(msg.into())
    }
}

impl fmt::Display for StreamingQuoteRejectReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::QuoteStale => write!(f, "quote_stale"),
            Self::RateLimitExceeded {
                current_rate,
                max_rate,
            } => {
                write!(f, "rate_limit_exceeded: {current_rate}/{max_rate} qps")
            }
            Self::InvalidSpread {
                spread_bps,
                max_spread_bps,
            } => {
                write!(f, "invalid_spread: {spread_bps} bps > {max_spread_bps} bps")
            }
            Self::InvalidSize { size, min_size } => {
                write!(f, "invalid_size: {size} < {min_size}")
            }
            Self::UnknownMM => write!(f, "unknown_mm"),
            Self::InstrumentNotSupported => write!(f, "instrument_not_supported"),
            Self::InternalError(msg) => write!(f, "internal_error: {msg}"),
        }
    }
}

// ============================================================================
// StreamingQuoteService Trait
// ============================================================================

/// Domain service trait for streaming quote reception and aggregation.
///
/// This trait defines the protocol-agnostic interface for receiving streaming
/// quotes from market makers. Implementations handle validation, rate limiting,
/// and best bid/ask aggregation.
///
/// # Thread Safety
///
/// Implementations must be `Send + Sync` to support concurrent quote reception
/// from multiple MMs across different protocols.
///
/// # Examples
///
/// ```ignore
/// use otc_rfq::domain::services::streaming_quote::StreamingQuoteService;
///
/// struct MyStreamingService { /* ... */ }
///
/// #[async_trait]
/// impl StreamingQuoteService for MyStreamingService {
///     async fn receive_quote(&self, quote: StreamingQuote) -> StreamingQuoteResult {
///         // Validate and process quote
///         StreamingQuoteResult::accepted(quote.id(), false)
///     }
///     // ... other methods
/// }
/// ```
#[async_trait]
pub trait StreamingQuoteService: Send + Sync + fmt::Debug {
    /// Processes an incoming streaming quote.
    ///
    /// Validates the quote against configured limits (spread, size, rate),
    /// updates the instrument book, and recalculates best bid/ask.
    ///
    /// # Arguments
    ///
    /// * `quote` - The streaming quote to process
    ///
    /// # Returns
    ///
    /// `StreamingQuoteResult::Accepted` if the quote was valid and added,
    /// `StreamingQuoteResult::Rejected` with reason if validation failed.
    async fn receive_quote(&self, quote: StreamingQuote) -> StreamingQuoteResult;

    /// Returns the best bid/ask for an instrument.
    ///
    /// Aggregates across all active (non-stale) quotes from all MMs.
    ///
    /// # Arguments
    ///
    /// * `instrument` - The instrument to query
    ///
    /// # Returns
    ///
    /// `Some(BestQuote)` if there are active quotes, `None` otherwise.
    fn best_quote(&self, instrument: &Instrument) -> Option<BestQuote>;

    /// Returns all active (non-stale) quotes for an instrument.
    ///
    /// # Arguments
    ///
    /// * `instrument` - The instrument to query
    ///
    /// # Returns
    ///
    /// Vector of active streaming quotes, may be empty.
    fn active_quotes(&self, instrument: &Instrument) -> Vec<StreamingQuote>;

    /// Returns the quote from a specific MM for an instrument.
    ///
    /// # Arguments
    ///
    /// * `instrument` - The instrument to query
    /// * `mm_id` - The market maker ID
    ///
    /// # Returns
    ///
    /// `Some(StreamingQuote)` if the MM has an active quote, `None` otherwise.
    fn mm_quote(&self, instrument: &Instrument, mm_id: &CounterpartyId) -> Option<StreamingQuote>;

    /// Checks if a market maker is registered for streaming.
    ///
    /// # Arguments
    ///
    /// * `mm_id` - The market maker ID to check
    ///
    /// # Returns
    ///
    /// `true` if the MM is registered, `false` otherwise.
    fn is_mm_registered(&self, mm_id: &CounterpartyId) -> bool;

    /// Registers a market maker for streaming.
    ///
    /// # Arguments
    ///
    /// * `mm_id` - The market maker ID to register
    fn register_mm(&self, mm_id: CounterpartyId);

    /// Unregisters a market maker from streaming.
    ///
    /// Also removes all active quotes from this MM.
    ///
    /// # Arguments
    ///
    /// * `mm_id` - The market maker ID to unregister
    fn unregister_mm(&self, mm_id: &CounterpartyId);

    /// Returns the service configuration.
    fn config(&self) -> &StreamingQuoteConfig;

    /// Returns aggregated statistics.
    async fn get_stats(&self) -> StreamingQuoteStats;

    /// Returns statistics for a specific MM.
    ///
    /// # Arguments
    ///
    /// * `mm_id` - The market maker ID
    async fn get_mm_stats(&self, mm_id: &CounterpartyId) -> Option<StreamingQuoteStats>;

    /// Removes all stale quotes from all instruments.
    ///
    /// Called periodically by the staleness checker background task.
    ///
    /// # Returns
    ///
    /// Number of quotes removed.
    async fn remove_stale_quotes(&self) -> usize;

    /// Returns the list of instruments with active quotes.
    fn active_instruments(&self) -> Vec<Instrument>;

    /// Returns the number of active quotes across all instruments.
    fn total_active_quotes(&self) -> usize;
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn streaming_quote_result_accepted() {
        let id = StreamingQuoteId::new(uuid::Uuid::new_v4());
        let result = StreamingQuoteResult::accepted(id, true);
        assert!(result.is_accepted());
        assert!(!result.is_rejected());
    }

    #[test]
    fn streaming_quote_result_rejected() {
        let result = StreamingQuoteResult::rejected(StreamingQuoteRejectReason::stale());
        assert!(result.is_rejected());
        assert!(!result.is_accepted());
    }

    #[test]
    fn reject_reason_display() {
        let reason = StreamingQuoteRejectReason::stale();
        assert_eq!(format!("{}", reason), "quote_stale");

        let reason = StreamingQuoteRejectReason::rate_limited(150, 100);
        assert!(format!("{}", reason).contains("150/100"));

        let reason = StreamingQuoteRejectReason::unknown_mm();
        assert_eq!(format!("{}", reason), "unknown_mm");
    }

    #[test]
    fn reject_reason_constructors() {
        let _ = StreamingQuoteRejectReason::stale();
        let _ = StreamingQuoteRejectReason::rate_limited(10, 5);
        let _ = StreamingQuoteRejectReason::invalid_spread(
            rust_decimal::Decimal::new(100, 0),
            rust_decimal::Decimal::new(50, 0),
        );
        let _ = StreamingQuoteRejectReason::invalid_size(
            rust_decimal::Decimal::new(1, 0),
            rust_decimal::Decimal::new(10, 0),
        );
        let _ = StreamingQuoteRejectReason::unknown_mm();
        let _ = StreamingQuoteRejectReason::instrument_not_supported();
        let _ = StreamingQuoteRejectReason::internal_error("test error");
    }
}
