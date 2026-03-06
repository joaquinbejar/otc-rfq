//! # Block Trade Aggregate
//!
//! Represents a pre-arranged bilateral block trade.
//!
//! This module provides the [`BlockTrade`] aggregate representing a pre-negotiated
//! trade between two counterparties that requires platform validation before execution.
//!
//! # State Machine
//!
//! ```text
//! Submitted → Validating → Approved → Executing → Executed
//!                ↓            ↓           ↓
//!             Rejected     Rejected    Failed
//! ```
//!
//! # Examples
//!
//! ```
//! use otc_rfq::domain::entities::block_trade::{BlockTrade, BlockTradeState};
//! use otc_rfq::domain::value_objects::{
//!     CounterpartyId, Instrument, Price, Quantity, Symbol, AssetClass, Timestamp,
//! };
//!
//! let symbol = Symbol::new("BTC/USD").unwrap();
//! let instrument = Instrument::builder(symbol, AssetClass::CryptoSpot).build();
//!
//! let trade = BlockTrade::new(
//!     CounterpartyId::new("buyer-1"),
//!     CounterpartyId::new("seller-1"),
//!     instrument,
//!     Price::new(50000.0).unwrap(),
//!     Quantity::new(30.0).unwrap(),
//!     Timestamp::now(),
//! );
//!
//! assert_eq!(trade.state(), BlockTradeState::Submitted);
//! assert!(!trade.is_fully_confirmed());
//! ```

use crate::domain::errors::{DomainError, DomainResult};
use crate::domain::services::ReportingTier;
use crate::domain::value_objects::timestamp::Timestamp;
use crate::domain::value_objects::{CounterpartyId, Instrument, Price, Quantity};
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// Unique identifier for a block trade.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BlockTradeId(Uuid);

impl BlockTradeId {
    /// Creates a new random block trade ID.
    #[must_use]
    pub fn new_v4() -> Self {
        Self(Uuid::new_v4())
    }

    /// Creates a block trade ID from an existing UUID.
    #[must_use]
    pub const fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Returns the inner UUID.
    #[must_use]
    pub const fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl fmt::Display for BlockTradeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Default for BlockTradeId {
    fn default() -> Self {
        Self::new_v4()
    }
}

/// Block trade lifecycle state.
///
/// Represents the current state of a bilateral block trade.
///
/// # State Machine
///
/// - `Submitted` → `Validating` (when validation starts)
/// - `Validating` → `Approved` (when validation passes and both confirm)
/// - `Validating` → `Rejected` (when validation fails)
/// - `Approved` → `Executing` (when execution starts)
/// - `Approved` → `Rejected` (if approval is revoked)
/// - `Executing` → `Executed` (when execution completes)
/// - `Executing` → `Failed` (when execution fails)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[repr(u8)]
pub enum BlockTradeState {
    /// Trade submitted, awaiting validation.
    #[default]
    Submitted = 0,

    /// Validation in progress.
    Validating = 1,

    /// Validation passed, both parties confirmed.
    Approved = 2,

    /// Validation failed or trade rejected.
    Rejected = 3,

    /// Execution in progress.
    Executing = 4,

    /// Trade executed successfully (terminal).
    Executed = 5,

    /// Execution failed (terminal).
    Failed = 6,
}

impl BlockTradeState {
    /// Returns true if this is a terminal state.
    #[inline]
    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        matches!(self, Self::Executed | Self::Failed | Self::Rejected)
    }

    /// Returns true if this state can transition to the target state.
    #[must_use]
    pub const fn can_transition_to(&self, target: Self) -> bool {
        matches!(
            (self, target),
            (Self::Submitted, Self::Validating)
                | (Self::Validating, Self::Approved)
                | (Self::Validating, Self::Rejected)
                | (Self::Approved, Self::Executing)
                | (Self::Approved, Self::Rejected)
                | (Self::Executing, Self::Executed)
                | (Self::Executing, Self::Failed)
        )
    }

    /// Returns the numeric value of this state.
    #[inline]
    #[must_use]
    pub const fn as_u8(&self) -> u8 {
        *self as u8
    }
}

impl fmt::Display for BlockTradeState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Submitted => "SUBMITTED",
            Self::Validating => "VALIDATING",
            Self::Approved => "APPROVED",
            Self::Rejected => "REJECTED",
            Self::Executing => "EXECUTING",
            Self::Executed => "EXECUTED",
            Self::Failed => "FAILED",
        };
        write!(f, "{}", s)
    }
}

impl TryFrom<u8> for BlockTradeState {
    type Error = InvalidBlockTradeStateError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Submitted),
            1 => Ok(Self::Validating),
            2 => Ok(Self::Approved),
            3 => Ok(Self::Rejected),
            4 => Ok(Self::Executing),
            5 => Ok(Self::Executed),
            6 => Ok(Self::Failed),
            _ => Err(InvalidBlockTradeStateError(value)),
        }
    }
}

/// Error returned when converting an invalid u8 to BlockTradeState.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidBlockTradeStateError(pub u8);

impl fmt::Display for InvalidBlockTradeStateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid block trade state value: {}", self.0)
    }
}

impl std::error::Error for InvalidBlockTradeStateError {}

/// Validation result for a block trade.
///
/// Contains details about what was validated and any issues found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockTradeValidation {
    /// Whether the buyer passed eligibility checks.
    pub buyer_eligible: bool,
    /// Whether the seller passed eligibility checks.
    pub seller_eligible: bool,
    /// Whether both parties have sufficient collateral.
    pub collateral_sufficient: bool,
    /// Whether the instrument is tradeable.
    pub instrument_valid: bool,
    /// Whether the price is within bounds.
    pub price_valid: bool,
    /// Whether the size qualifies as a block trade.
    pub size_qualifies: bool,
    /// Detailed failure reasons, if any.
    pub failure_reasons: Vec<String>,
}

impl BlockTradeValidation {
    /// Creates a new validation result indicating all checks passed.
    #[must_use]
    pub fn passed() -> Self {
        Self {
            buyer_eligible: true,
            seller_eligible: true,
            collateral_sufficient: true,
            instrument_valid: true,
            price_valid: true,
            size_qualifies: true,
            failure_reasons: Vec::new(),
        }
    }

    /// Creates a new validation result with failures.
    #[must_use]
    pub fn failed(reasons: Vec<String>) -> Self {
        Self {
            buyer_eligible: false,
            seller_eligible: false,
            collateral_sufficient: false,
            instrument_valid: false,
            price_valid: false,
            size_qualifies: false,
            failure_reasons: reasons,
        }
    }

    /// Returns true if all validations passed.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.buyer_eligible
            && self.seller_eligible
            && self.collateral_sufficient
            && self.instrument_valid
            && self.price_valid
            && self.size_qualifies
    }
}

impl Default for BlockTradeValidation {
    fn default() -> Self {
        Self::passed()
    }
}

/// A pre-arranged bilateral block trade.
///
/// Represents a trade where both counterparties have agreed on terms
/// off-platform and submit the trade for platform validation.
///
/// # Confirmation Flow
///
/// Both buyer and seller must confirm the trade before it can be approved:
/// 1. Trade is submitted (by either party)
/// 2. Platform validates eligibility, collateral, price, instrument
/// 3. Both parties confirm
/// 4. Trade is approved and can be executed
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockTrade {
    /// Unique identifier.
    id: BlockTradeId,
    /// Buyer counterparty ID.
    buyer_id: CounterpartyId,
    /// Seller counterparty ID.
    seller_id: CounterpartyId,
    /// Instrument being traded.
    instrument: Instrument,
    /// Agreed price.
    price: Price,
    /// Agreed quantity.
    quantity: Quantity,
    /// Current state.
    state: BlockTradeState,
    /// Reporting tier based on size.
    reporting_tier: Option<ReportingTier>,
    /// Whether buyer has confirmed.
    buyer_confirmed: bool,
    /// Whether seller has confirmed.
    seller_confirmed: bool,
    /// Validation result, if validation has been performed.
    validation_result: Option<BlockTradeValidation>,
    /// When the parties agreed on terms (off-platform).
    agreed_at: Timestamp,
    /// When this trade was submitted to the platform.
    created_at: Timestamp,
    /// When this trade was last updated.
    updated_at: Timestamp,
    /// Rejection reason, if rejected.
    rejection_reason: Option<String>,
}

impl BlockTrade {
    /// Creates a new block trade submission.
    ///
    /// # Arguments
    ///
    /// * `buyer_id` - The buyer's counterparty ID
    /// * `seller_id` - The seller's counterparty ID
    /// * `instrument` - The instrument being traded
    /// * `price` - The agreed price
    /// * `quantity` - The agreed quantity
    /// * `agreed_at` - When the parties agreed on terms
    #[must_use]
    pub fn new(
        buyer_id: CounterpartyId,
        seller_id: CounterpartyId,
        instrument: Instrument,
        price: Price,
        quantity: Quantity,
        agreed_at: Timestamp,
    ) -> Self {
        let now = Timestamp::now();
        Self {
            id: BlockTradeId::new_v4(),
            buyer_id,
            seller_id,
            instrument,
            price,
            quantity,
            state: BlockTradeState::Submitted,
            reporting_tier: None,
            buyer_confirmed: false,
            seller_confirmed: false,
            validation_result: None,
            agreed_at,
            created_at: now,
            updated_at: now,
            rejection_reason: None,
        }
    }

    /// Creates a block trade with a specific ID (for reconstruction from storage).
    ///
    /// # Safety
    ///
    /// This method bypasses validation and should only be used when
    /// reconstructing from trusted storage.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn from_parts(
        id: BlockTradeId,
        buyer_id: CounterpartyId,
        seller_id: CounterpartyId,
        instrument: Instrument,
        price: Price,
        quantity: Quantity,
        state: BlockTradeState,
        reporting_tier: Option<ReportingTier>,
        buyer_confirmed: bool,
        seller_confirmed: bool,
        validation_result: Option<BlockTradeValidation>,
        agreed_at: Timestamp,
        created_at: Timestamp,
        updated_at: Timestamp,
        rejection_reason: Option<String>,
    ) -> Self {
        Self {
            id,
            buyer_id,
            seller_id,
            instrument,
            price,
            quantity,
            state,
            reporting_tier,
            buyer_confirmed,
            seller_confirmed,
            validation_result,
            agreed_at,
            created_at,
            updated_at,
            rejection_reason,
        }
    }

    // =========================================================================
    // Accessors
    // =========================================================================

    /// Returns the block trade ID.
    #[inline]
    #[must_use]
    pub fn id(&self) -> BlockTradeId {
        self.id
    }

    /// Returns the buyer's counterparty ID.
    #[inline]
    #[must_use]
    pub fn buyer_id(&self) -> &CounterpartyId {
        &self.buyer_id
    }

    /// Returns the seller's counterparty ID.
    #[inline]
    #[must_use]
    pub fn seller_id(&self) -> &CounterpartyId {
        &self.seller_id
    }

    /// Returns the instrument being traded.
    #[inline]
    #[must_use]
    pub fn instrument(&self) -> &Instrument {
        &self.instrument
    }

    /// Returns the agreed price.
    #[inline]
    #[must_use]
    pub fn price(&self) -> Price {
        self.price
    }

    /// Returns the agreed quantity.
    #[inline]
    #[must_use]
    pub fn quantity(&self) -> Quantity {
        self.quantity
    }

    /// Returns the current state.
    #[inline]
    #[must_use]
    pub fn state(&self) -> BlockTradeState {
        self.state
    }

    /// Returns the reporting tier, if determined.
    #[inline]
    #[must_use]
    pub fn reporting_tier(&self) -> Option<ReportingTier> {
        self.reporting_tier
    }

    /// Returns whether the buyer has confirmed.
    #[inline]
    #[must_use]
    pub fn buyer_confirmed(&self) -> bool {
        self.buyer_confirmed
    }

    /// Returns whether the seller has confirmed.
    #[inline]
    #[must_use]
    pub fn seller_confirmed(&self) -> bool {
        self.seller_confirmed
    }

    /// Returns true if both parties have confirmed.
    #[inline]
    #[must_use]
    pub fn is_fully_confirmed(&self) -> bool {
        self.buyer_confirmed && self.seller_confirmed
    }

    /// Returns the validation result, if available.
    #[inline]
    #[must_use]
    pub fn validation_result(&self) -> Option<&BlockTradeValidation> {
        self.validation_result.as_ref()
    }

    /// Returns when the parties agreed on terms.
    #[inline]
    #[must_use]
    pub fn agreed_at(&self) -> Timestamp {
        self.agreed_at
    }

    /// Returns when this trade was submitted.
    #[inline]
    #[must_use]
    pub fn created_at(&self) -> Timestamp {
        self.created_at
    }

    /// Returns when this trade was last updated.
    #[inline]
    #[must_use]
    pub fn updated_at(&self) -> Timestamp {
        self.updated_at
    }

    /// Returns the rejection reason, if rejected.
    #[inline]
    #[must_use]
    pub fn rejection_reason(&self) -> Option<&str> {
        self.rejection_reason.as_deref()
    }

    // =========================================================================
    // State Transitions
    // =========================================================================

    /// Transitions to the specified state.
    fn transition_to(&mut self, target: BlockTradeState) -> DomainResult<()> {
        if !self.state.can_transition_to(target) {
            return Err(DomainError::GenericStateTransitionError {
                from: self.state.to_string(),
                to: target.to_string(),
            });
        }
        self.state = target;
        self.updated_at = Timestamp::now();
        Ok(())
    }

    /// Starts validation of the block trade.
    ///
    /// Transitions from `Submitted` to `Validating`.
    ///
    /// # Errors
    ///
    /// Returns an error if the trade is not in `Submitted` state.
    pub fn start_validation(&mut self) -> DomainResult<()> {
        self.transition_to(BlockTradeState::Validating)
    }

    /// Records the validation result and sets the reporting tier.
    ///
    /// If validation passes, the trade remains in `Validating` state
    /// until both parties confirm.
    ///
    /// # Errors
    ///
    /// Returns an error if the trade is not in `Validating` state.
    pub fn record_validation(
        &mut self,
        result: BlockTradeValidation,
        reporting_tier: Option<ReportingTier>,
    ) -> DomainResult<()> {
        if self.state != BlockTradeState::Validating {
            return Err(DomainError::GenericStateTransitionError {
                from: self.state.to_string(),
                to: "recording validation".to_string(),
            });
        }

        self.reporting_tier = reporting_tier;

        if !result.is_valid() {
            let reason = result.failure_reasons.join("; ");
            self.rejection_reason = Some(reason);
            self.validation_result = Some(result);
            self.transition_to(BlockTradeState::Rejected)?;
        } else {
            self.validation_result = Some(result);
        }

        Ok(())
    }

    /// Confirms the trade from a counterparty's perspective.
    ///
    /// Both buyer and seller must confirm before the trade can be approved.
    ///
    /// # Arguments
    ///
    /// * `counterparty_id` - The ID of the confirming counterparty
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The counterparty is neither buyer nor seller
    /// - The trade is not in a state that accepts confirmations
    pub fn confirm(&mut self, counterparty_id: &CounterpartyId) -> DomainResult<()> {
        // Can only confirm during validation (after validation passes)
        if self.state != BlockTradeState::Validating {
            return Err(DomainError::GenericStateTransitionError {
                from: self.state.to_string(),
                to: "confirming".to_string(),
            });
        }

        // Check validation passed
        if let Some(ref validation) = self.validation_result {
            if !validation.is_valid() {
                return Err(DomainError::ValidationFailed(
                    "Cannot confirm a trade that failed validation".to_string(),
                ));
            }
        } else {
            return Err(DomainError::ValidationFailed(
                "Cannot confirm before validation is complete".to_string(),
            ));
        }

        // Record confirmation
        if counterparty_id == &self.buyer_id {
            self.buyer_confirmed = true;
        } else if counterparty_id == &self.seller_id {
            self.seller_confirmed = true;
        } else {
            return Err(DomainError::UnauthorizedCounterparty(
                counterparty_id.to_string(),
            ));
        }

        self.updated_at = Timestamp::now();

        // If both confirmed, transition to Approved
        if self.is_fully_confirmed() {
            self.transition_to(BlockTradeState::Approved)?;
        }

        Ok(())
    }

    /// Starts execution of the approved trade.
    ///
    /// Transitions from `Approved` to `Executing`.
    ///
    /// # Errors
    ///
    /// Returns an error if the trade is not in `Approved` state.
    pub fn start_execution(&mut self) -> DomainResult<()> {
        self.transition_to(BlockTradeState::Executing)
    }

    /// Marks the trade as successfully executed.
    ///
    /// Transitions from `Executing` to `Executed`.
    ///
    /// # Errors
    ///
    /// Returns an error if the trade is not in `Executing` state.
    pub fn mark_executed(&mut self) -> DomainResult<()> {
        self.transition_to(BlockTradeState::Executed)
    }

    /// Marks the trade as failed.
    ///
    /// Transitions from `Executing` to `Failed`.
    ///
    /// # Errors
    ///
    /// Returns an error if the trade is not in `Executing` state.
    pub fn mark_failed(&mut self, reason: &str) -> DomainResult<()> {
        self.rejection_reason = Some(reason.to_string());
        self.transition_to(BlockTradeState::Failed)
    }

    /// Rejects the trade.
    ///
    /// Can be called from `Validating` or `Approved` states.
    ///
    /// # Errors
    ///
    /// Returns an error if the trade is not in a rejectable state.
    pub fn reject(&mut self, reason: &str) -> DomainResult<()> {
        self.rejection_reason = Some(reason.to_string());
        self.transition_to(BlockTradeState::Rejected)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::domain::value_objects::{AssetClass, Symbol};

    fn create_test_instrument() -> Instrument {
        let symbol = Symbol::new("BTC/USD").unwrap();
        Instrument::builder(symbol, AssetClass::CryptoSpot).build()
    }

    fn create_test_block_trade() -> BlockTrade {
        BlockTrade::new(
            CounterpartyId::new("buyer-1"),
            CounterpartyId::new("seller-1"),
            create_test_instrument(),
            Price::new(50000.0).unwrap(),
            Quantity::new(30.0).unwrap(),
            Timestamp::now(),
        )
    }

    #[test]
    fn new_block_trade_has_submitted_state() {
        let trade = create_test_block_trade();
        assert_eq!(trade.state(), BlockTradeState::Submitted);
        assert!(!trade.buyer_confirmed());
        assert!(!trade.seller_confirmed());
        assert!(!trade.is_fully_confirmed());
    }

    #[test]
    fn block_trade_state_transitions() {
        assert!(BlockTradeState::Submitted.can_transition_to(BlockTradeState::Validating));
        assert!(BlockTradeState::Validating.can_transition_to(BlockTradeState::Approved));
        assert!(BlockTradeState::Validating.can_transition_to(BlockTradeState::Rejected));
        assert!(BlockTradeState::Approved.can_transition_to(BlockTradeState::Executing));
        assert!(BlockTradeState::Executing.can_transition_to(BlockTradeState::Executed));
        assert!(BlockTradeState::Executing.can_transition_to(BlockTradeState::Failed));

        // Invalid transitions
        assert!(!BlockTradeState::Submitted.can_transition_to(BlockTradeState::Approved));
        assert!(!BlockTradeState::Executed.can_transition_to(BlockTradeState::Submitted));
    }

    #[test]
    fn terminal_states_are_correct() {
        assert!(BlockTradeState::Executed.is_terminal());
        assert!(BlockTradeState::Failed.is_terminal());
        assert!(BlockTradeState::Rejected.is_terminal());
        assert!(!BlockTradeState::Submitted.is_terminal());
        assert!(!BlockTradeState::Validating.is_terminal());
        assert!(!BlockTradeState::Approved.is_terminal());
    }

    #[test]
    fn start_validation_transitions_state() {
        let mut trade = create_test_block_trade();
        assert!(trade.start_validation().is_ok());
        assert_eq!(trade.state(), BlockTradeState::Validating);
    }

    #[test]
    fn record_validation_with_failure_rejects() {
        let mut trade = create_test_block_trade();
        trade.start_validation().unwrap();

        let validation = BlockTradeValidation::failed(vec!["Buyer not eligible".to_string()]);
        assert!(trade.record_validation(validation, None).is_ok());
        assert_eq!(trade.state(), BlockTradeState::Rejected);
        assert!(trade.rejection_reason().is_some());
    }

    #[test]
    fn record_validation_with_success_stays_validating() {
        let mut trade = create_test_block_trade();
        trade.start_validation().unwrap();

        let validation = BlockTradeValidation::passed();
        assert!(
            trade
                .record_validation(validation, Some(ReportingTier::Standard))
                .is_ok()
        );
        assert_eq!(trade.state(), BlockTradeState::Validating);
        assert_eq!(trade.reporting_tier(), Some(ReportingTier::Standard));
    }

    #[test]
    fn confirm_requires_validation_first() {
        let mut trade = create_test_block_trade();
        let buyer_id = CounterpartyId::new("buyer-1");

        // Cannot confirm before validation starts
        assert!(trade.confirm(&buyer_id).is_err());

        trade.start_validation().unwrap();

        // Cannot confirm before validation completes
        assert!(trade.confirm(&buyer_id).is_err());
    }

    #[test]
    fn confirm_from_both_parties_approves_trade() {
        let mut trade = create_test_block_trade();
        let buyer_id = CounterpartyId::new("buyer-1");
        let seller_id = CounterpartyId::new("seller-1");

        trade.start_validation().unwrap();
        trade
            .record_validation(
                BlockTradeValidation::passed(),
                Some(ReportingTier::Standard),
            )
            .unwrap();

        // Buyer confirms
        assert!(trade.confirm(&buyer_id).is_ok());
        assert!(trade.buyer_confirmed());
        assert!(!trade.seller_confirmed());
        assert_eq!(trade.state(), BlockTradeState::Validating);

        // Seller confirms
        assert!(trade.confirm(&seller_id).is_ok());
        assert!(trade.seller_confirmed());
        assert!(trade.is_fully_confirmed());
        assert_eq!(trade.state(), BlockTradeState::Approved);
    }

    #[test]
    fn confirm_from_unauthorized_party_fails() {
        let mut trade = create_test_block_trade();
        let unknown_id = CounterpartyId::new("unknown");

        trade.start_validation().unwrap();
        trade
            .record_validation(
                BlockTradeValidation::passed(),
                Some(ReportingTier::Standard),
            )
            .unwrap();

        assert!(trade.confirm(&unknown_id).is_err());
    }

    #[test]
    fn full_execution_flow() {
        let mut trade = create_test_block_trade();
        let buyer_id = CounterpartyId::new("buyer-1");
        let seller_id = CounterpartyId::new("seller-1");

        // Submit → Validating
        assert!(trade.start_validation().is_ok());

        // Record validation
        trade
            .record_validation(BlockTradeValidation::passed(), Some(ReportingTier::Large))
            .unwrap();

        // Both confirm → Approved
        trade.confirm(&buyer_id).unwrap();
        trade.confirm(&seller_id).unwrap();
        assert_eq!(trade.state(), BlockTradeState::Approved);

        // Start execution
        assert!(trade.start_execution().is_ok());
        assert_eq!(trade.state(), BlockTradeState::Executing);

        // Mark executed
        assert!(trade.mark_executed().is_ok());
        assert_eq!(trade.state(), BlockTradeState::Executed);
        assert!(trade.state().is_terminal());
    }

    #[test]
    fn execution_failure_flow() {
        let mut trade = create_test_block_trade();
        let buyer_id = CounterpartyId::new("buyer-1");
        let seller_id = CounterpartyId::new("seller-1");

        trade.start_validation().unwrap();
        trade
            .record_validation(
                BlockTradeValidation::passed(),
                Some(ReportingTier::Standard),
            )
            .unwrap();
        trade.confirm(&buyer_id).unwrap();
        trade.confirm(&seller_id).unwrap();
        trade.start_execution().unwrap();

        // Mark failed
        assert!(trade.mark_failed("Settlement failed").is_ok());
        assert_eq!(trade.state(), BlockTradeState::Failed);
        assert_eq!(trade.rejection_reason(), Some("Settlement failed"));
    }

    #[test]
    fn block_trade_state_display() {
        assert_eq!(BlockTradeState::Submitted.to_string(), "SUBMITTED");
        assert_eq!(BlockTradeState::Validating.to_string(), "VALIDATING");
        assert_eq!(BlockTradeState::Approved.to_string(), "APPROVED");
        assert_eq!(BlockTradeState::Rejected.to_string(), "REJECTED");
        assert_eq!(BlockTradeState::Executing.to_string(), "EXECUTING");
        assert_eq!(BlockTradeState::Executed.to_string(), "EXECUTED");
        assert_eq!(BlockTradeState::Failed.to_string(), "FAILED");
    }

    #[test]
    fn block_trade_state_try_from() {
        assert_eq!(
            BlockTradeState::try_from(0u8),
            Ok(BlockTradeState::Submitted)
        );
        assert_eq!(
            BlockTradeState::try_from(1u8),
            Ok(BlockTradeState::Validating)
        );
        assert_eq!(
            BlockTradeState::try_from(2u8),
            Ok(BlockTradeState::Approved)
        );
        assert_eq!(
            BlockTradeState::try_from(3u8),
            Ok(BlockTradeState::Rejected)
        );
        assert_eq!(
            BlockTradeState::try_from(4u8),
            Ok(BlockTradeState::Executing)
        );
        assert_eq!(
            BlockTradeState::try_from(5u8),
            Ok(BlockTradeState::Executed)
        );
        assert_eq!(BlockTradeState::try_from(6u8), Ok(BlockTradeState::Failed));
        assert!(BlockTradeState::try_from(7u8).is_err());
    }

    #[test]
    fn validation_result_is_valid() {
        let passed = BlockTradeValidation::passed();
        assert!(passed.is_valid());

        let failed = BlockTradeValidation::failed(vec!["Error".to_string()]);
        assert!(!failed.is_valid());

        let partial = BlockTradeValidation {
            buyer_eligible: true,
            seller_eligible: false,
            collateral_sufficient: true,
            instrument_valid: true,
            price_valid: true,
            size_qualifies: true,
            failure_reasons: vec!["Seller not eligible".to_string()],
        };
        assert!(!partial.is_valid());
    }
}
