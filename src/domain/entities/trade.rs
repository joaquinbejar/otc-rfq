//! # Trade Aggregate
//!
//! Represents an executed trade.
//!
//! This module provides the [`Trade`] aggregate representing an executed trade
//! resulting from an RFQ, including settlement lifecycle management.
//!
//! # Settlement State Machine
//!
//! ```text
//! Pending → InProgress → Settled
//!              ↓
//!           Failed
//! ```
//!
//! # Examples
//!
//! ```
//! use otc_rfq::domain::entities::trade::Trade;
//! use otc_rfq::domain::value_objects::{
//!     RfqId, QuoteId, VenueId, Price, Quantity,
//! };
//!
//! let trade = Trade::new(
//!     RfqId::new_v4(),
//!     QuoteId::new_v4(),
//!     VenueId::new("binance"),
//!     Price::new(50000.0).unwrap(),
//!     Quantity::new(1.0).unwrap(),
//! );
//!
//! assert!(trade.is_pending());
//! ```

use crate::domain::errors::{DomainError, DomainResult};
use crate::domain::value_objects::timestamp::Timestamp;
use crate::domain::value_objects::{Price, Quantity, QuoteId, RfqId, TradeId, VenueId};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Settlement lifecycle state.
///
/// Represents the current state of trade settlement.
///
/// # State Machine
///
/// - `Pending` → `InProgress` → `Settled`
/// - `InProgress` → `Failed`
///
/// # Examples
///
/// ```
/// use otc_rfq::domain::entities::trade::SettlementState;
///
/// let state = SettlementState::Pending;
/// assert!(state.can_transition_to(SettlementState::InProgress));
/// assert!(!state.can_transition_to(SettlementState::Settled));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[repr(u8)]
pub enum SettlementState {
    /// Trade executed, awaiting settlement initiation.
    #[default]
    Pending = 0,

    /// Settlement is in progress.
    InProgress = 1,

    /// Settlement completed successfully (terminal).
    Settled = 2,

    /// Settlement failed (terminal).
    Failed = 3,
}

impl SettlementState {
    /// Returns true if this is a terminal state.
    ///
    /// Terminal states cannot transition to any other state.
    #[inline]
    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        matches!(self, Self::Settled | Self::Failed)
    }

    /// Returns true if this state can transition to the target state.
    ///
    /// # Arguments
    ///
    /// * `target` - The target state to transition to
    #[must_use]
    pub const fn can_transition_to(&self, target: Self) -> bool {
        matches!(
            (self, target),
            (Self::Pending, Self::InProgress)
                | (Self::InProgress, Self::Settled)
                | (Self::InProgress, Self::Failed)
        )
    }

    /// Returns the numeric value of this state.
    #[inline]
    #[must_use]
    pub const fn as_u8(&self) -> u8 {
        *self as u8
    }
}

impl fmt::Display for SettlementState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Pending => "PENDING",
            Self::InProgress => "IN_PROGRESS",
            Self::Settled => "SETTLED",
            Self::Failed => "FAILED",
        };
        write!(f, "{}", s)
    }
}

impl TryFrom<u8> for SettlementState {
    type Error = InvalidSettlementStateError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Pending),
            1 => Ok(Self::InProgress),
            2 => Ok(Self::Settled),
            3 => Ok(Self::Failed),
            _ => Err(InvalidSettlementStateError(value)),
        }
    }
}

/// Error returned when converting an invalid u8 to SettlementState.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidSettlementStateError(pub u8);

impl fmt::Display for InvalidSettlementStateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid settlement state value: {}", self.0)
    }
}

impl std::error::Error for InvalidSettlementStateError {}

/// An executed trade.
///
/// Represents a trade resulting from an RFQ execution, including
/// settlement lifecycle management.
///
/// # Invariants
///
/// - Settlement lifecycle: Pending → InProgress → Settled/Failed
/// - Cannot modify after Settled state
/// - Must reference valid RFQ and Quote
///
/// # Examples
///
/// ```
/// use otc_rfq::domain::entities::trade::Trade;
/// use otc_rfq::domain::value_objects::{
///     RfqId, QuoteId, VenueId, Price, Quantity,
/// };
///
/// let mut trade = Trade::new(
///     RfqId::new_v4(),
///     QuoteId::new_v4(),
///     VenueId::new("venue"),
///     Price::new(100.0).unwrap(),
///     Quantity::new(10.0).unwrap(),
/// );
///
/// // Start settlement
/// trade.start_settlement().unwrap();
/// assert!(trade.is_in_progress());
///
/// // Confirm settlement
/// trade.confirm_settlement("tx-123").unwrap();
/// assert!(trade.is_settled());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Trade {
    /// Unique identifier for this trade.
    id: TradeId,
    /// The RFQ this trade originated from.
    rfq_id: RfqId,
    /// The quote that was executed.
    quote_id: QuoteId,
    /// The venue where the trade was executed.
    venue_id: VenueId,
    /// The execution price.
    price: Price,
    /// The executed quantity.
    quantity: Quantity,
    /// Venue-specific execution reference.
    venue_execution_ref: Option<String>,
    /// Current settlement state.
    settlement_state: SettlementState,
    /// Settlement transaction reference.
    settlement_tx_ref: Option<String>,
    /// Reason for settlement failure, if any.
    failure_reason: Option<String>,
    /// Version for optimistic locking.
    version: u64,
    /// When this trade was created.
    created_at: Timestamp,
    /// When this trade was last updated.
    updated_at: Timestamp,
}

impl Trade {
    /// Creates a new trade.
    ///
    /// # Arguments
    ///
    /// * `rfq_id` - The RFQ this trade originated from
    /// * `quote_id` - The quote that was executed
    /// * `venue_id` - The venue where the trade was executed
    /// * `price` - The execution price
    /// * `quantity` - The executed quantity
    ///
    /// # Examples
    ///
    /// ```
    /// use otc_rfq::domain::entities::trade::Trade;
    /// use otc_rfq::domain::value_objects::{
    ///     RfqId, QuoteId, VenueId, Price, Quantity,
    /// };
    ///
    /// let trade = Trade::new(
    ///     RfqId::new_v4(),
    ///     QuoteId::new_v4(),
    ///     VenueId::new("venue"),
    ///     Price::new(100.0).unwrap(),
    ///     Quantity::new(10.0).unwrap(),
    /// );
    /// ```
    #[must_use]
    pub fn new(
        rfq_id: RfqId,
        quote_id: QuoteId,
        venue_id: VenueId,
        price: Price,
        quantity: Quantity,
    ) -> Self {
        let now = Timestamp::now();
        Self {
            id: TradeId::new_v4(),
            rfq_id,
            quote_id,
            venue_id,
            price,
            quantity,
            venue_execution_ref: None,
            settlement_state: SettlementState::Pending,
            settlement_tx_ref: None,
            failure_reason: None,
            version: 1,
            created_at: now,
            updated_at: now,
        }
    }

    /// Creates a trade with a venue execution reference.
    #[must_use]
    pub fn with_venue_ref(
        rfq_id: RfqId,
        quote_id: QuoteId,
        venue_id: VenueId,
        price: Price,
        quantity: Quantity,
        venue_execution_ref: impl Into<String>,
    ) -> Self {
        let mut trade = Self::new(rfq_id, quote_id, venue_id, price, quantity);
        trade.venue_execution_ref = Some(venue_execution_ref.into());
        trade
    }

    /// Creates a trade with a specific ID (for reconstruction from storage).
    ///
    /// # Safety
    ///
    /// This method bypasses validation and should only be used when
    /// reconstructing from trusted storage.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn from_parts(
        id: TradeId,
        rfq_id: RfqId,
        quote_id: QuoteId,
        venue_id: VenueId,
        price: Price,
        quantity: Quantity,
        venue_execution_ref: Option<String>,
        settlement_state: SettlementState,
        settlement_tx_ref: Option<String>,
        failure_reason: Option<String>,
        version: u64,
        created_at: Timestamp,
        updated_at: Timestamp,
    ) -> Self {
        Self {
            id,
            rfq_id,
            quote_id,
            venue_id,
            price,
            quantity,
            venue_execution_ref,
            settlement_state,
            settlement_tx_ref,
            failure_reason,
            version,
            created_at,
            updated_at,
        }
    }

    fn transition_to(&mut self, target: SettlementState) -> DomainResult<()> {
        if !self.settlement_state.can_transition_to(target) {
            return Err(DomainError::InvalidState(format!(
                "cannot transition from {} to {}",
                self.settlement_state, target
            )));
        }
        self.settlement_state = target;
        self.updated_at = Timestamp::now();
        self.version = self.version.saturating_add(1);
        Ok(())
    }

    // ========== Accessors ==========

    /// Returns the trade ID.
    #[inline]
    #[must_use]
    pub fn id(&self) -> TradeId {
        self.id
    }

    /// Returns the RFQ ID.
    #[inline]
    #[must_use]
    pub fn rfq_id(&self) -> RfqId {
        self.rfq_id
    }

    /// Returns the quote ID.
    #[inline]
    #[must_use]
    pub fn quote_id(&self) -> QuoteId {
        self.quote_id
    }

    /// Returns the venue ID.
    #[inline]
    #[must_use]
    pub fn venue_id(&self) -> &VenueId {
        &self.venue_id
    }

    /// Returns the execution price.
    #[inline]
    #[must_use]
    pub fn price(&self) -> Price {
        self.price
    }

    /// Returns the executed quantity.
    #[inline]
    #[must_use]
    pub fn quantity(&self) -> Quantity {
        self.quantity
    }

    /// Returns the venue execution reference, if any.
    #[inline]
    #[must_use]
    pub fn venue_execution_ref(&self) -> Option<&str> {
        self.venue_execution_ref.as_deref()
    }

    /// Returns the current settlement state.
    #[inline]
    #[must_use]
    pub fn settlement_state(&self) -> SettlementState {
        self.settlement_state
    }

    /// Returns the settlement transaction reference, if any.
    #[inline]
    #[must_use]
    pub fn settlement_tx_ref(&self) -> Option<&str> {
        self.settlement_tx_ref.as_deref()
    }

    /// Returns the failure reason, if any.
    #[inline]
    #[must_use]
    pub fn failure_reason(&self) -> Option<&str> {
        self.failure_reason.as_deref()
    }

    /// Returns the version for optimistic locking.
    #[inline]
    #[must_use]
    pub fn version(&self) -> u64 {
        self.version
    }

    /// Returns when this trade was created.
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

    // ========== State Helpers ==========

    /// Returns true if settlement is pending.
    #[inline]
    #[must_use]
    pub fn is_pending(&self) -> bool {
        self.settlement_state == SettlementState::Pending
    }

    /// Returns true if settlement is in progress.
    #[inline]
    #[must_use]
    pub fn is_in_progress(&self) -> bool {
        self.settlement_state == SettlementState::InProgress
    }

    /// Returns true if the trade is settled.
    #[inline]
    #[must_use]
    pub fn is_settled(&self) -> bool {
        self.settlement_state == SettlementState::Settled
    }

    /// Returns true if settlement failed.
    #[inline]
    #[must_use]
    pub fn is_failed(&self) -> bool {
        self.settlement_state == SettlementState::Failed
    }

    /// Returns true if this trade is in a terminal state.
    #[inline]
    #[must_use]
    pub fn is_terminal(&self) -> bool {
        self.settlement_state.is_terminal()
    }

    /// Calculates the total trade value.
    ///
    /// Returns `price * quantity`.
    #[must_use]
    pub fn total_value(&self) -> Option<Price> {
        self.price.safe_mul(self.quantity.get()).ok()
    }

    // ========== State Transitions ==========

    /// Starts the settlement process.
    ///
    /// Transitions: Pending → InProgress
    ///
    /// # Errors
    ///
    /// Returns `DomainError::InvalidState` if not in Pending state.
    pub fn start_settlement(&mut self) -> DomainResult<()> {
        self.transition_to(SettlementState::InProgress)
    }

    /// Confirms successful settlement.
    ///
    /// Transitions: InProgress → Settled
    ///
    /// # Arguments
    ///
    /// * `tx_ref` - The settlement transaction reference
    ///
    /// # Errors
    ///
    /// Returns `DomainError::InvalidState` if not in InProgress state.
    pub fn confirm_settlement(&mut self, tx_ref: impl Into<String>) -> DomainResult<()> {
        self.settlement_tx_ref = Some(tx_ref.into());
        self.transition_to(SettlementState::Settled)
    }

    /// Marks settlement as failed.
    ///
    /// Transitions: InProgress → Failed
    ///
    /// # Arguments
    ///
    /// * `reason` - The reason for failure
    ///
    /// # Errors
    ///
    /// Returns `DomainError::InvalidState` if not in InProgress state.
    pub fn fail_settlement(&mut self, reason: impl Into<String>) -> DomainResult<()> {
        self.failure_reason = Some(reason.into());
        self.transition_to(SettlementState::Failed)
    }
}

impl fmt::Display for Trade {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Trade({} {} @ {} [{}])",
            self.id, self.quantity, self.price, self.settlement_state
        )
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

    fn test_venue_id() -> VenueId {
        VenueId::new("test-venue")
    }

    fn test_price() -> Price {
        Price::new(50000.0).unwrap()
    }

    fn test_quantity() -> Quantity {
        Quantity::new(1.5).unwrap()
    }

    fn create_test_trade() -> Trade {
        Trade::new(
            test_rfq_id(),
            test_quote_id(),
            test_venue_id(),
            test_price(),
            test_quantity(),
        )
    }

    mod settlement_state {
        use super::*;

        #[test]
        fn pending_can_transition_to_in_progress() {
            assert!(SettlementState::Pending.can_transition_to(SettlementState::InProgress));
        }

        #[test]
        fn pending_cannot_transition_to_settled() {
            assert!(!SettlementState::Pending.can_transition_to(SettlementState::Settled));
        }

        #[test]
        fn in_progress_can_transition_to_settled() {
            assert!(SettlementState::InProgress.can_transition_to(SettlementState::Settled));
        }

        #[test]
        fn in_progress_can_transition_to_failed() {
            assert!(SettlementState::InProgress.can_transition_to(SettlementState::Failed));
        }

        #[test]
        fn settled_is_terminal() {
            assert!(SettlementState::Settled.is_terminal());
            assert!(!SettlementState::Settled.can_transition_to(SettlementState::Pending));
        }

        #[test]
        fn failed_is_terminal() {
            assert!(SettlementState::Failed.is_terminal());
        }

        #[test]
        fn as_u8() {
            assert_eq!(SettlementState::Pending.as_u8(), 0);
            assert_eq!(SettlementState::InProgress.as_u8(), 1);
            assert_eq!(SettlementState::Settled.as_u8(), 2);
            assert_eq!(SettlementState::Failed.as_u8(), 3);
        }

        #[test]
        fn try_from_u8() {
            assert_eq!(
                SettlementState::try_from(0).unwrap(),
                SettlementState::Pending
            );
            assert_eq!(
                SettlementState::try_from(2).unwrap(),
                SettlementState::Settled
            );
            assert!(SettlementState::try_from(99).is_err());
        }

        #[test]
        fn display() {
            assert_eq!(SettlementState::Pending.to_string(), "PENDING");
            assert_eq!(SettlementState::InProgress.to_string(), "IN_PROGRESS");
            assert_eq!(SettlementState::Settled.to_string(), "SETTLED");
            assert_eq!(SettlementState::Failed.to_string(), "FAILED");
        }
    }

    mod construction {
        use super::*;

        #[test]
        fn new_creates_pending_trade() {
            let trade = create_test_trade();

            assert!(trade.is_pending());
            assert_eq!(trade.version(), 1);
            assert!(trade.venue_execution_ref().is_none());
            assert!(trade.settlement_tx_ref().is_none());
            assert!(trade.failure_reason().is_none());
        }

        #[test]
        fn with_venue_ref_sets_reference() {
            let trade = Trade::with_venue_ref(
                test_rfq_id(),
                test_quote_id(),
                test_venue_id(),
                test_price(),
                test_quantity(),
                "venue-ref-123",
            );

            assert_eq!(trade.venue_execution_ref(), Some("venue-ref-123"));
        }
    }

    mod state_transitions {
        use super::*;

        #[test]
        fn start_settlement_from_pending() {
            let mut trade = create_test_trade();

            assert!(trade.start_settlement().is_ok());
            assert!(trade.is_in_progress());
            assert_eq!(trade.version(), 2);
        }

        #[test]
        fn start_settlement_fails_from_in_progress() {
            let mut trade = create_test_trade();
            trade.start_settlement().unwrap();

            let result = trade.start_settlement();
            assert!(matches!(result, Err(DomainError::InvalidState(_))));
        }

        #[test]
        fn confirm_settlement_from_in_progress() {
            let mut trade = create_test_trade();
            trade.start_settlement().unwrap();

            assert!(trade.confirm_settlement("tx-abc-123").is_ok());
            assert!(trade.is_settled());
            assert_eq!(trade.settlement_tx_ref(), Some("tx-abc-123"));
            assert_eq!(trade.version(), 3);
        }

        #[test]
        fn confirm_settlement_fails_from_pending() {
            let mut trade = create_test_trade();

            let result = trade.confirm_settlement("tx-123");
            assert!(matches!(result, Err(DomainError::InvalidState(_))));
        }

        #[test]
        fn fail_settlement_from_in_progress() {
            let mut trade = create_test_trade();
            trade.start_settlement().unwrap();

            assert!(trade.fail_settlement("network error").is_ok());
            assert!(trade.is_failed());
            assert_eq!(trade.failure_reason(), Some("network error"));
        }

        #[test]
        fn fail_settlement_fails_from_pending() {
            let mut trade = create_test_trade();

            let result = trade.fail_settlement("error");
            assert!(matches!(result, Err(DomainError::InvalidState(_))));
        }

        #[test]
        fn cannot_transition_from_settled() {
            let mut trade = create_test_trade();
            trade.start_settlement().unwrap();
            trade.confirm_settlement("tx-123").unwrap();

            // Try various transitions
            assert!(trade.start_settlement().is_err());
            assert!(trade.confirm_settlement("tx-456").is_err());
            assert!(trade.fail_settlement("error").is_err());
        }

        #[test]
        fn cannot_transition_from_failed() {
            let mut trade = create_test_trade();
            trade.start_settlement().unwrap();
            trade.fail_settlement("error").unwrap();

            assert!(trade.start_settlement().is_err());
            assert!(trade.confirm_settlement("tx-123").is_err());
        }
    }

    mod helpers {
        use super::*;

        #[test]
        fn is_terminal() {
            let mut trade = create_test_trade();
            assert!(!trade.is_terminal());

            trade.start_settlement().unwrap();
            assert!(!trade.is_terminal());

            trade.confirm_settlement("tx-123").unwrap();
            assert!(trade.is_terminal());
        }

        #[test]
        fn total_value() {
            let trade = Trade::new(
                test_rfq_id(),
                test_quote_id(),
                test_venue_id(),
                Price::new(100.0).unwrap(),
                Quantity::new(2.5).unwrap(),
            );

            let value = trade.total_value().unwrap();
            assert_eq!(value, Price::new(250.0).unwrap());
        }
    }

    mod display {
        use super::*;

        #[test]
        fn display_format() {
            let trade = create_test_trade();
            let display = trade.to_string();

            assert!(display.contains("Trade"));
            assert!(display.contains("PENDING"));
        }
    }

    mod serde {
        use super::*;

        #[test]
        fn trade_serde_roundtrip() {
            let mut trade = create_test_trade();
            trade.start_settlement().unwrap();
            trade.confirm_settlement("tx-123").unwrap();

            let json = serde_json::to_string(&trade).unwrap();
            let deserialized: Trade = serde_json::from_str(&json).unwrap();

            assert_eq!(trade.id(), deserialized.id());
            assert_eq!(trade.settlement_state(), deserialized.settlement_state());
            assert_eq!(trade.settlement_tx_ref(), deserialized.settlement_tx_ref());
        }

        #[test]
        fn settlement_state_serde_roundtrip() {
            for state in [
                SettlementState::Pending,
                SettlementState::InProgress,
                SettlementState::Settled,
                SettlementState::Failed,
            ] {
                let json = serde_json::to_string(&state).unwrap();
                let deserialized: SettlementState = serde_json::from_str(&json).unwrap();
                assert_eq!(state, deserialized);
            }
        }

        #[test]
        fn settlement_state_serde_screaming_snake_case() {
            let json = serde_json::to_string(&SettlementState::InProgress).unwrap();
            assert_eq!(json, "\"IN_PROGRESS\"");
        }
    }
}
