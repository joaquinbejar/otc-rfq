//! # Last-Look Service
//!
//! Market maker confirmation protocol for quote acceptance.
//!
//! This module provides the [`LastLookService`] trait and related types
//! for implementing the last-look confirmation window during acceptance.
//!
//! # Architecture
//!
//! Last-look allows market makers to confirm or reject a quote acceptance
//! within a short time window (typically 200ms), providing protection
//! against adverse price movements.
//!
//! ```text
//! Client accepts quote
//!        |
//!        v
//! Send last-look request to MM
//!        |
//!        +---> Confirmed (proceed to execution)
//!        |
//!        +---> Rejected (abort, notify client)
//!        |
//!        +---> Timeout (abort, notify client)
//! ```

use crate::domain::entities::quote::Quote;
use crate::domain::errors::DomainError;
use crate::domain::value_objects::{CounterpartyId, QuoteId, Timestamp, VenueId};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::Duration;

/// Reasons for last-look rejection by a market maker.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LastLookRejectReason {
    /// Price moved unfavorably since quote was issued.
    PriceMoved,
    /// MM capacity limits exceeded.
    CapacityExceeded,
    /// Risk limits would be breached.
    RiskLimit,
    /// Request timed out before MM responded.
    Timeout,
    /// Other reason with description.
    Other(String),
}

impl LastLookRejectReason {
    /// Creates a price moved rejection.
    #[must_use]
    pub fn price_moved() -> Self {
        Self::PriceMoved
    }

    /// Creates a capacity exceeded rejection.
    #[must_use]
    pub fn capacity_exceeded() -> Self {
        Self::CapacityExceeded
    }

    /// Creates a risk limit rejection.
    #[must_use]
    pub fn risk_limit() -> Self {
        Self::RiskLimit
    }

    /// Creates a timeout rejection.
    #[must_use]
    pub fn timeout() -> Self {
        Self::Timeout
    }

    /// Creates an other rejection with custom reason.
    #[must_use]
    pub fn other(reason: impl Into<String>) -> Self {
        Self::Other(reason.into())
    }

    /// Returns true if this is a timeout rejection.
    #[must_use]
    pub fn is_timeout(&self) -> bool {
        matches!(self, Self::Timeout)
    }
}

impl fmt::Display for LastLookRejectReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PriceMoved => write!(f, "price_moved"),
            Self::CapacityExceeded => write!(f, "capacity_exceeded"),
            Self::RiskLimit => write!(f, "risk_limit"),
            Self::Timeout => write!(f, "timeout"),
            Self::Other(reason) => write!(f, "other: {reason}"),
        }
    }
}

/// Per-market-maker last-look configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MmLastLookConfig {
    /// The market maker this config applies to.
    pub mm_id: CounterpartyId,
    /// Whether last-look is enabled for this MM.
    pub enabled: bool,
    /// Maximum window in milliseconds (default 200ms).
    pub max_window_ms: u64,
}

impl MmLastLookConfig {
    /// Default window in milliseconds.
    pub const DEFAULT_WINDOW_MS: u64 = 200;

    /// Creates a new MM last-look config.
    #[must_use]
    pub fn new(mm_id: CounterpartyId) -> Self {
        Self {
            mm_id,
            enabled: true,
            max_window_ms: Self::DEFAULT_WINDOW_MS,
        }
    }

    /// Creates a disabled config.
    #[must_use]
    pub fn disabled(mm_id: CounterpartyId) -> Self {
        Self {
            mm_id,
            enabled: false,
            max_window_ms: Self::DEFAULT_WINDOW_MS,
        }
    }

    /// Sets the maximum window.
    #[must_use]
    pub fn with_max_window_ms(mut self, ms: u64) -> Self {
        self.max_window_ms = ms;
        self
    }

    /// Returns the timeout as a Duration.
    #[must_use]
    pub fn timeout(&self) -> Duration {
        Duration::from_millis(self.max_window_ms)
    }
}

/// Result of a last-look request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LastLookResult {
    /// MM confirmed the quote acceptance.
    Confirmed {
        /// The quote that was confirmed.
        quote_id: QuoteId,
        /// When the confirmation was received.
        confirmed_at: Timestamp,
    },
    /// MM rejected the quote acceptance.
    Rejected {
        /// The quote that was rejected.
        quote_id: QuoteId,
        /// Reason for rejection.
        reason: LastLookRejectReason,
        /// When the rejection was received.
        rejected_at: Timestamp,
    },
    /// Last-look request timed out.
    Timeout {
        /// The quote that timed out.
        quote_id: QuoteId,
        /// The timeout duration.
        timeout: Duration,
        /// When the timeout occurred.
        timed_out_at: Timestamp,
    },
}

impl LastLookResult {
    /// Creates a confirmed result.
    #[must_use]
    pub fn confirmed(quote_id: QuoteId) -> Self {
        Self::Confirmed {
            quote_id,
            confirmed_at: Timestamp::now(),
        }
    }

    /// Creates a rejected result with typed reason.
    #[must_use]
    pub fn rejected(quote_id: QuoteId, reason: LastLookRejectReason) -> Self {
        Self::Rejected {
            quote_id,
            reason,
            rejected_at: Timestamp::now(),
        }
    }

    /// Creates a rejected result from a string reason (for backwards compatibility).
    #[must_use]
    pub fn rejected_other(quote_id: QuoteId, reason: impl Into<String>) -> Self {
        Self::Rejected {
            quote_id,
            reason: LastLookRejectReason::Other(reason.into()),
            rejected_at: Timestamp::now(),
        }
    }

    /// Creates a timeout result.
    #[must_use]
    pub fn timeout(quote_id: QuoteId, timeout: Duration) -> Self {
        Self::Timeout {
            quote_id,
            timeout,
            timed_out_at: Timestamp::now(),
        }
    }

    /// Returns the quote ID.
    #[must_use]
    pub fn quote_id(&self) -> QuoteId {
        match self {
            Self::Confirmed { quote_id, .. } => *quote_id,
            Self::Rejected { quote_id, .. } => *quote_id,
            Self::Timeout { quote_id, .. } => *quote_id,
        }
    }

    /// Returns true if the result is a confirmation.
    #[must_use]
    pub fn is_confirmed(&self) -> bool {
        matches!(self, Self::Confirmed { .. })
    }

    /// Returns true if the result is a rejection.
    #[must_use]
    pub fn is_rejected(&self) -> bool {
        matches!(self, Self::Rejected { .. })
    }

    /// Returns true if the result is a timeout.
    #[must_use]
    pub fn is_timeout(&self) -> bool {
        matches!(self, Self::Timeout { .. })
    }

    /// Returns the rejection reason if this is a rejection.
    #[must_use]
    pub fn rejection_reason(&self) -> Option<&LastLookRejectReason> {
        match self {
            Self::Rejected { reason, .. } => Some(reason),
            _ => None,
        }
    }

    /// Converts a non-confirmed result to a DomainError.
    ///
    /// Returns None if the result is confirmed.
    #[must_use]
    pub fn to_error(&self) -> Option<DomainError> {
        match self {
            Self::Confirmed { .. } => None,
            Self::Rejected { reason, .. } => {
                Some(DomainError::LastLookRejected(reason.to_string()))
            }
            Self::Timeout { timeout, .. } => Some(DomainError::LastLookTimeout(format!(
                "Last-look timed out after {}ms",
                timeout.as_millis()
            ))),
        }
    }
}

/// Statistics for last-look rejections per market maker.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LastLookStats {
    /// Total number of last-look requests.
    pub total_requests: u64,
    /// Number of confirmations.
    pub confirmations: u64,
    /// Number of rejections.
    pub rejections: u64,
    /// Number of timeouts.
    pub timeouts: u64,
}

impl LastLookStats {
    /// Creates new empty stats.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a confirmation.
    pub fn record_confirmation(&mut self) {
        self.total_requests = self.total_requests.saturating_add(1);
        self.confirmations = self.confirmations.saturating_add(1);
    }

    /// Records a rejection.
    pub fn record_rejection(&mut self) {
        self.total_requests = self.total_requests.saturating_add(1);
        self.rejections = self.rejections.saturating_add(1);
    }

    /// Records a timeout.
    pub fn record_timeout(&mut self) {
        self.total_requests = self.total_requests.saturating_add(1);
        self.timeouts = self.timeouts.saturating_add(1);
    }

    /// Returns the confirmation rate (0.0 to 1.0).
    #[must_use]
    pub fn confirmation_rate(&self) -> f64 {
        if self.total_requests == 0 {
            1.0
        } else {
            self.confirmations as f64 / self.total_requests as f64
        }
    }

    /// Returns the rejection rate (0.0 to 1.0).
    #[must_use]
    pub fn rejection_rate(&self) -> f64 {
        if self.total_requests == 0 {
            0.0
        } else {
            self.rejections as f64 / self.total_requests as f64
        }
    }

    /// Returns the timeout rate (0.0 to 1.0).
    #[must_use]
    pub fn timeout_rate(&self) -> f64 {
        if self.total_requests == 0 {
            0.0
        } else {
            self.timeouts as f64 / self.total_requests as f64
        }
    }
}

/// Service for handling last-look confirmations with market makers.
///
/// Implementations communicate with market makers to confirm or reject
/// quote acceptances within the configured timeout window.
#[async_trait]
pub trait LastLookService: Send + Sync + fmt::Debug {
    /// Sends a last-look request to the market maker.
    ///
    /// # Arguments
    ///
    /// * `quote` - The quote being accepted
    /// * `timeout` - Maximum time to wait for a response
    ///
    /// # Returns
    ///
    /// The result of the last-look request.
    async fn request(&self, quote: &Quote, timeout: Duration) -> LastLookResult;

    /// Checks if a venue requires last-look confirmation.
    ///
    /// # Arguments
    ///
    /// * `venue_id` - The venue to check
    ///
    /// # Returns
    ///
    /// True if the venue requires last-look, false otherwise.
    fn requires_last_look(&self, venue_id: &VenueId) -> bool;

    /// Returns last-look statistics for a venue.
    ///
    /// # Arguments
    ///
    /// * `venue_id` - The venue to get stats for
    ///
    /// # Returns
    ///
    /// Statistics for the venue, or None if no stats exist.
    async fn get_stats(&self, venue_id: &VenueId) -> Option<LastLookStats>;

    /// Records a last-look result for metrics tracking.
    ///
    /// # Arguments
    ///
    /// * `venue_id` - The venue that responded
    /// * `result` - The result to record
    async fn record_result(&self, venue_id: &VenueId, result: &LastLookResult);
}

/// Configuration for last-look handling.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LastLookConfig {
    /// Default timeout for last-look requests.
    pub default_timeout: Duration,
    /// Maximum timeout allowed.
    pub max_timeout: Duration,
    /// Whether to skip last-look for venues that don't require it.
    pub skip_if_not_required: bool,
    /// Rejection rate threshold for alerting (0.0 to 1.0).
    pub rejection_alert_threshold: f64,
}

impl Default for LastLookConfig {
    fn default() -> Self {
        Self {
            default_timeout: Duration::from_millis(200),
            max_timeout: Duration::from_millis(500),
            skip_if_not_required: true,
            rejection_alert_threshold: 0.1, // 10%
        }
    }
}

impl LastLookConfig {
    /// Creates a new last-look configuration.
    #[must_use]
    pub fn new(default_timeout: Duration) -> Self {
        Self {
            default_timeout,
            ..Default::default()
        }
    }

    /// Returns the effective timeout, clamped to max_timeout.
    #[must_use]
    pub fn effective_timeout(&self, requested: Option<Duration>) -> Duration {
        requested
            .unwrap_or(self.default_timeout)
            .min(self.max_timeout)
    }

    /// Sets the maximum timeout.
    #[must_use]
    pub fn with_max_timeout(mut self, max: Duration) -> Self {
        self.max_timeout = max;
        self
    }

    /// Sets whether to skip last-look for non-requiring venues.
    #[must_use]
    pub fn with_skip_if_not_required(mut self, skip: bool) -> Self {
        self.skip_if_not_required = skip;
        self
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn test_quote_id() -> QuoteId {
        QuoteId::new_v4()
    }

    #[test]
    fn last_look_result_confirmed() {
        let quote_id = test_quote_id();
        let result = LastLookResult::confirmed(quote_id);

        assert!(result.is_confirmed());
        assert!(!result.is_rejected());
        assert!(!result.is_timeout());
        assert_eq!(result.quote_id(), quote_id);
        assert!(result.to_error().is_none());
    }

    #[test]
    fn last_look_result_rejected() {
        let quote_id = test_quote_id();
        let result = LastLookResult::rejected(quote_id, LastLookRejectReason::PriceMoved);

        assert!(!result.is_confirmed());
        assert!(result.is_rejected());
        assert!(!result.is_timeout());
        assert_eq!(
            result.rejection_reason(),
            Some(&LastLookRejectReason::PriceMoved)
        );
        assert!(result.to_error().is_some());
    }

    #[test]
    fn last_look_result_rejected_other() {
        let quote_id = test_quote_id();
        let result = LastLookResult::rejected_other(quote_id, "Custom reason");

        assert!(result.is_rejected());
        assert!(matches!(
            result.rejection_reason(),
            Some(&LastLookRejectReason::Other(_))
        ));
    }

    #[test]
    fn last_look_reject_reason_display() {
        assert_eq!(LastLookRejectReason::PriceMoved.to_string(), "price_moved");
        assert_eq!(
            LastLookRejectReason::CapacityExceeded.to_string(),
            "capacity_exceeded"
        );
        assert_eq!(LastLookRejectReason::RiskLimit.to_string(), "risk_limit");
        assert_eq!(LastLookRejectReason::Timeout.to_string(), "timeout");
        assert_eq!(
            LastLookRejectReason::Other("test".to_string()).to_string(),
            "other: test"
        );
    }

    #[test]
    fn mm_last_look_config_new() {
        let mm_id = CounterpartyId::new("mm-1");
        let config = MmLastLookConfig::new(mm_id.clone());

        assert_eq!(config.mm_id, mm_id);
        assert!(config.enabled);
        assert_eq!(config.max_window_ms, 200);
        assert_eq!(config.timeout(), Duration::from_millis(200));
    }

    #[test]
    fn mm_last_look_config_disabled() {
        let mm_id = CounterpartyId::new("mm-1");
        let config = MmLastLookConfig::disabled(mm_id);

        assert!(!config.enabled);
    }

    #[test]
    fn mm_last_look_config_with_window() {
        let mm_id = CounterpartyId::new("mm-1");
        let config = MmLastLookConfig::new(mm_id).with_max_window_ms(300);

        assert_eq!(config.max_window_ms, 300);
        assert_eq!(config.timeout(), Duration::from_millis(300));
    }

    #[test]
    fn last_look_result_timeout() {
        let quote_id = test_quote_id();
        let timeout = Duration::from_millis(200);
        let result = LastLookResult::timeout(quote_id, timeout);

        assert!(!result.is_confirmed());
        assert!(!result.is_rejected());
        assert!(result.is_timeout());
        assert!(result.to_error().is_some());
    }

    #[test]
    fn last_look_stats_new() {
        let stats = LastLookStats::new();
        assert_eq!(stats.total_requests, 0);
        assert_eq!(stats.confirmations, 0);
        assert_eq!(stats.rejections, 0);
        assert_eq!(stats.timeouts, 0);
        assert_eq!(stats.confirmation_rate(), 1.0); // No requests = 100% success
    }

    #[test]
    fn last_look_stats_record() {
        let mut stats = LastLookStats::new();

        stats.record_confirmation();
        stats.record_confirmation();
        stats.record_rejection();

        assert_eq!(stats.total_requests, 3);
        assert_eq!(stats.confirmations, 2);
        assert_eq!(stats.rejections, 1);
        assert!((stats.confirmation_rate() - 0.666).abs() < 0.01);
        assert!((stats.rejection_rate() - 0.333).abs() < 0.01);
    }

    #[test]
    fn last_look_stats_timeout() {
        let mut stats = LastLookStats::new();

        stats.record_timeout();
        stats.record_timeout();

        assert_eq!(stats.total_requests, 2);
        assert_eq!(stats.timeouts, 2);
        assert_eq!(stats.timeout_rate(), 1.0);
    }

    #[test]
    fn last_look_config_default() {
        let config = LastLookConfig::default();
        assert_eq!(config.default_timeout, Duration::from_millis(200));
        assert_eq!(config.max_timeout, Duration::from_millis(500));
        assert!(config.skip_if_not_required);
    }

    #[test]
    fn last_look_config_effective_timeout() {
        let config = LastLookConfig::new(Duration::from_millis(200))
            .with_max_timeout(Duration::from_millis(300));

        // Uses default when None
        assert_eq!(config.effective_timeout(None), Duration::from_millis(200));

        // Uses requested when within max
        assert_eq!(
            config.effective_timeout(Some(Duration::from_millis(250))),
            Duration::from_millis(250)
        );

        // Clamps to max when exceeds
        assert_eq!(
            config.effective_timeout(Some(Duration::from_millis(500))),
            Duration::from_millis(300)
        );
    }
}
