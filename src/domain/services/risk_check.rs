//! # Risk Check Service
//!
//! Pre-trade risk validation for quote acceptance.
//!
//! This module provides the [`RiskCheckService`] trait and related types
//! for validating trades against risk limits before execution.
//!
//! # Architecture
//!
//! Risk checks run after quote locking but before last-look confirmation,
//! ensuring that trades meet margin, collateral, and position limit requirements.
//!
//! ```text
//! Lock -> Risk Check -> Last-Look -> Execute
//!              |
//!              v
//!         Margin OK?
//!         Collateral OK?
//!         Position limits OK?
//! ```

use crate::domain::entities::quote::Quote;
use crate::domain::entities::rfq::Rfq;
use crate::domain::errors::{DomainError, DomainResult};
use crate::domain::value_objects::{QuoteId, RfqId};
use async_trait::async_trait;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Result of a risk check evaluation.
///
/// Contains detailed information about the risk assessment,
/// including whether the check passed and any warnings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RiskResult {
    /// Whether the risk check passed.
    passed: bool,
    /// The RFQ ID that was checked.
    rfq_id: RfqId,
    /// The quote ID that was checked.
    quote_id: QuoteId,
    /// Available margin for the trade.
    margin_available: Option<Decimal>,
    /// Required collateral for the trade.
    collateral_required: Option<Decimal>,
    /// Current position size in the instrument.
    current_position: Option<Decimal>,
    /// Position limit for the instrument.
    position_limit: Option<Decimal>,
    /// Warnings that don't block the trade but should be logged.
    warnings: Vec<String>,
    /// Reason for failure if the check didn't pass.
    failure_reason: Option<String>,
}

impl RiskResult {
    /// Creates a passing risk result.
    #[must_use]
    pub fn passed(rfq_id: RfqId, quote_id: QuoteId) -> Self {
        Self {
            passed: true,
            rfq_id,
            quote_id,
            margin_available: None,
            collateral_required: None,
            current_position: None,
            position_limit: None,
            warnings: Vec::new(),
            failure_reason: None,
        }
    }

    /// Creates a failing risk result.
    #[must_use]
    pub fn failed(rfq_id: RfqId, quote_id: QuoteId, reason: impl Into<String>) -> Self {
        Self {
            passed: false,
            rfq_id,
            quote_id,
            margin_available: None,
            collateral_required: None,
            current_position: None,
            position_limit: None,
            warnings: Vec::new(),
            failure_reason: Some(reason.into()),
        }
    }

    /// Returns whether the risk check passed.
    #[must_use]
    pub fn is_passed(&self) -> bool {
        self.passed
    }

    /// Returns the RFQ ID.
    #[must_use]
    pub fn rfq_id(&self) -> RfqId {
        self.rfq_id
    }

    /// Returns the quote ID.
    #[must_use]
    pub fn quote_id(&self) -> QuoteId {
        self.quote_id
    }

    /// Returns the available margin.
    #[must_use]
    pub fn margin_available(&self) -> Option<Decimal> {
        self.margin_available
    }

    /// Returns the required collateral.
    #[must_use]
    pub fn collateral_required(&self) -> Option<Decimal> {
        self.collateral_required
    }

    /// Returns the current position.
    #[must_use]
    pub fn current_position(&self) -> Option<Decimal> {
        self.current_position
    }

    /// Returns the position limit.
    #[must_use]
    pub fn position_limit(&self) -> Option<Decimal> {
        self.position_limit
    }

    /// Returns any warnings.
    #[must_use]
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    /// Returns the failure reason if the check failed.
    #[must_use]
    pub fn failure_reason(&self) -> Option<&str> {
        self.failure_reason.as_deref()
    }

    /// Sets the margin available.
    #[must_use]
    pub fn with_margin_available(mut self, margin: Decimal) -> Self {
        self.margin_available = Some(margin);
        self
    }

    /// Sets the collateral required.
    #[must_use]
    pub fn with_collateral_required(mut self, collateral: Decimal) -> Self {
        self.collateral_required = Some(collateral);
        self
    }

    /// Sets the current position.
    #[must_use]
    pub fn with_current_position(mut self, position: Decimal) -> Self {
        self.current_position = Some(position);
        self
    }

    /// Sets the position limit.
    #[must_use]
    pub fn with_position_limit(mut self, limit: Decimal) -> Self {
        self.position_limit = Some(limit);
        self
    }

    /// Adds a warning.
    #[must_use]
    pub fn with_warning(mut self, warning: impl Into<String>) -> Self {
        self.warnings.push(warning.into());
        self
    }

    /// Converts a failed result to a DomainError.
    ///
    /// Returns `None` if the risk check passed, `Some(error)` if it failed.
    #[must_use]
    pub fn to_error(&self) -> Option<DomainError> {
        if self.passed {
            None
        } else {
            Some(DomainError::RiskCheckFailed(
                self.failure_reason
                    .clone()
                    .unwrap_or_else(|| "Unknown risk check failure".to_string()),
            ))
        }
    }
}

/// Service for performing pre-trade risk checks.
///
/// Implementations validate trades against various risk limits
/// including margin, collateral, and position limits.
#[async_trait]
pub trait RiskCheckService: Send + Sync + fmt::Debug {
    /// Performs a risk check for the given RFQ and quote.
    ///
    /// # Arguments
    ///
    /// * `rfq` - The RFQ being executed
    /// * `quote` - The quote being accepted
    ///
    /// # Returns
    ///
    /// A `RiskResult` indicating whether the trade passes risk checks.
    async fn check(&self, rfq: &Rfq, quote: &Quote) -> RiskResult;

    /// Validates margin requirements for a trade.
    ///
    /// # Arguments
    ///
    /// * `counterparty_id` - The counterparty executing the trade
    /// * `notional` - The notional value of the trade
    ///
    /// # Returns
    ///
    /// Ok(available_margin) if sufficient margin exists.
    async fn check_margin(&self, counterparty_id: &str, notional: Decimal)
    -> DomainResult<Decimal>;

    /// Validates collateral requirements for a trade.
    ///
    /// # Arguments
    ///
    /// * `counterparty_id` - The counterparty executing the trade
    /// * `required` - The required collateral amount
    ///
    /// # Returns
    ///
    /// Ok(()) if sufficient collateral exists.
    async fn check_collateral(&self, counterparty_id: &str, required: Decimal) -> DomainResult<()>;

    /// Validates position limits for a trade.
    ///
    /// # Arguments
    ///
    /// * `counterparty_id` - The counterparty executing the trade
    /// * `instrument` - The instrument being traded
    /// * `additional_size` - The size being added to the position
    ///
    /// # Returns
    ///
    /// Ok(()) if the position limit is not exceeded.
    async fn check_position_limit(
        &self,
        counterparty_id: &str,
        instrument: &str,
        additional_size: Decimal,
    ) -> DomainResult<()>;
}

/// Configuration for risk checks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RiskCheckConfig {
    /// Whether risk checks are enabled.
    pub enabled: bool,
    /// Default margin requirement as a percentage (0.0 to 1.0).
    pub default_margin_requirement: Decimal,
    /// Default position limit per instrument.
    pub default_position_limit: Option<Decimal>,
    /// Whether to fail on warnings.
    pub fail_on_warnings: bool,
}

impl Default for RiskCheckConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            default_margin_requirement: Decimal::new(10, 2), // 10%
            default_position_limit: None,
            fail_on_warnings: false,
        }
    }
}

impl RiskCheckConfig {
    /// Creates a new risk check configuration.
    #[must_use]
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            ..Default::default()
        }
    }

    /// Creates a disabled risk check configuration.
    #[must_use]
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Default::default()
        }
    }

    /// Sets the default margin requirement.
    #[must_use]
    pub fn with_margin_requirement(mut self, requirement: Decimal) -> Self {
        self.default_margin_requirement = requirement;
        self
    }

    /// Sets the default position limit.
    #[must_use]
    pub fn with_position_limit(mut self, limit: Decimal) -> Self {
        self.default_position_limit = Some(limit);
        self
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn test_rfq_id() -> RfqId {
        RfqId::new_v4()
    }

    fn test_quote_id() -> QuoteId {
        QuoteId::new_v4()
    }

    #[test]
    fn risk_result_passed() {
        let rfq_id = test_rfq_id();
        let quote_id = test_quote_id();
        let result = RiskResult::passed(rfq_id, quote_id);

        assert!(result.is_passed());
        assert_eq!(result.rfq_id(), rfq_id);
        assert_eq!(result.quote_id(), quote_id);
        assert!(result.failure_reason().is_none());
    }

    #[test]
    fn risk_result_failed() {
        let rfq_id = test_rfq_id();
        let quote_id = test_quote_id();
        let result = RiskResult::failed(rfq_id, quote_id, "Insufficient margin");

        assert!(!result.is_passed());
        assert_eq!(result.failure_reason(), Some("Insufficient margin"));
    }

    #[test]
    fn risk_result_with_details() {
        let rfq_id = test_rfq_id();
        let quote_id = test_quote_id();
        let result = RiskResult::passed(rfq_id, quote_id)
            .with_margin_available(Decimal::new(10000, 2))
            .with_collateral_required(Decimal::new(1000, 2))
            .with_current_position(Decimal::new(5, 0))
            .with_position_limit(Decimal::new(100, 0))
            .with_warning("Approaching position limit");

        assert!(result.is_passed());
        assert_eq!(result.margin_available(), Some(Decimal::new(10000, 2)));
        assert_eq!(result.collateral_required(), Some(Decimal::new(1000, 2)));
        assert_eq!(result.current_position(), Some(Decimal::new(5, 0)));
        assert_eq!(result.position_limit(), Some(Decimal::new(100, 0)));
        assert_eq!(result.warnings().len(), 1);
    }

    #[test]
    fn risk_result_to_error() {
        let rfq_id = test_rfq_id();
        let quote_id = test_quote_id();
        let failed_result = RiskResult::failed(rfq_id, quote_id, "Position limit exceeded");
        let passed_result = RiskResult::passed(rfq_id, quote_id);

        // Failed result returns Some(error)
        let error = failed_result.to_error();
        assert!(matches!(error, Some(DomainError::RiskCheckFailed(_))));

        // Passed result returns None
        assert!(passed_result.to_error().is_none());
    }

    #[test]
    fn risk_check_config_default() {
        let config = RiskCheckConfig::default();
        assert!(config.enabled);
        assert_eq!(config.default_margin_requirement, Decimal::new(10, 2));
        assert!(config.default_position_limit.is_none());
        assert!(!config.fail_on_warnings);
    }

    #[test]
    fn risk_check_config_disabled() {
        let config = RiskCheckConfig::disabled();
        assert!(!config.enabled);
    }

    #[test]
    fn risk_check_config_builder() {
        let config = RiskCheckConfig::new(true)
            .with_margin_requirement(Decimal::new(20, 2))
            .with_position_limit(Decimal::new(1000, 0));

        assert!(config.enabled);
        assert_eq!(config.default_margin_requirement, Decimal::new(20, 2));
        assert_eq!(config.default_position_limit, Some(Decimal::new(1000, 0)));
    }
}
