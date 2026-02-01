//! # RFQ State
//!
//! RFQ lifecycle state machine.
//!
//! This module provides the [`RfqState`] enum representing the lifecycle
//! of a Request for Quote with enforced state transitions.
//!
//! # State Machine
//!
//! ```text
//! Created → QuoteRequesting → QuotesReceived → ClientSelecting → Executing → Executed
//!     ↓           ↓                 ↓                ↓              ↓
//!     └───────────┴─────────────────┴────────────────┴──────────────┴→ Failed/Cancelled/Expired
//! ```
//!
//! # Examples
//!
//! ```
//! use otc_rfq::domain::value_objects::rfq_state::RfqState;
//!
//! let state = RfqState::Created;
//! assert!(state.can_transition_to(RfqState::QuoteRequesting));
//! assert!(!state.can_transition_to(RfqState::Executed));
//! ```

use serde::{Deserialize, Serialize};
use std::fmt;

/// RFQ lifecycle state.
///
/// Represents the current state of a Request for Quote in the system.
/// State transitions are enforced via [`can_transition_to`](RfqState::can_transition_to).
///
/// # Terminal States
///
/// The following states are terminal (no further transitions allowed):
/// - [`Executed`](RfqState::Executed) - Trade completed successfully
/// - [`Failed`](RfqState::Failed) - Trade execution failed
/// - [`Cancelled`](RfqState::Cancelled) - RFQ cancelled by user
/// - [`Expired`](RfqState::Expired) - RFQ expired without execution
///
/// # Examples
///
/// ```
/// use otc_rfq::domain::value_objects::rfq_state::RfqState;
///
/// let state = RfqState::QuotesReceived;
/// assert!(!state.is_terminal());
///
/// let terminal = RfqState::Executed;
/// assert!(terminal.is_terminal());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[repr(u8)]
pub enum RfqState {
    /// RFQ has been created but quotes not yet requested.
    #[default]
    Created = 0,

    /// Quotes are being requested from venues.
    QuoteRequesting = 1,

    /// Quotes have been received from one or more venues.
    QuotesReceived = 2,

    /// Client is selecting a quote to execute.
    ClientSelecting = 3,

    /// Trade execution is in progress.
    Executing = 4,

    /// Trade executed successfully (terminal).
    Executed = 5,

    /// Trade execution failed (terminal).
    Failed = 6,

    /// RFQ was cancelled by user (terminal).
    Cancelled = 7,

    /// RFQ expired without execution (terminal).
    Expired = 8,
}

impl RfqState {
    /// Returns true if this is a terminal state.
    ///
    /// Terminal states cannot transition to any other state.
    ///
    /// # Examples
    ///
    /// ```
    /// use otc_rfq::domain::value_objects::rfq_state::RfqState;
    ///
    /// assert!(!RfqState::Created.is_terminal());
    /// assert!(RfqState::Executed.is_terminal());
    /// assert!(RfqState::Failed.is_terminal());
    /// assert!(RfqState::Cancelled.is_terminal());
    /// assert!(RfqState::Expired.is_terminal());
    /// ```
    #[inline]
    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Executed | Self::Failed | Self::Cancelled | Self::Expired
        )
    }

    /// Returns true if this state can transition to the target state.
    ///
    /// Enforces the RFQ state machine rules:
    /// - Created → QuoteRequesting, Cancelled, Expired
    /// - QuoteRequesting → QuotesReceived, Failed, Cancelled, Expired
    /// - QuotesReceived → ClientSelecting, Failed, Cancelled, Expired
    /// - ClientSelecting → Executing, Failed, Cancelled, Expired
    /// - Executing → Executed, Failed
    /// - Terminal states → (none)
    ///
    /// # Arguments
    ///
    /// * `target` - The target state to transition to
    ///
    /// # Examples
    ///
    /// ```
    /// use otc_rfq::domain::value_objects::rfq_state::RfqState;
    ///
    /// // Valid transition
    /// assert!(RfqState::Created.can_transition_to(RfqState::QuoteRequesting));
    ///
    /// // Invalid transition (skipping states)
    /// assert!(!RfqState::Created.can_transition_to(RfqState::Executed));
    ///
    /// // Terminal states cannot transition
    /// assert!(!RfqState::Executed.can_transition_to(RfqState::Created));
    /// ```
    #[must_use]
    pub const fn can_transition_to(&self, target: Self) -> bool {
        matches!(
            (self, target),
            // From Created
            (Self::Created, Self::QuoteRequesting)
                | (Self::Created, Self::Cancelled)
                | (Self::Created, Self::Expired)
                // From QuoteRequesting
                | (Self::QuoteRequesting, Self::QuotesReceived)
                | (Self::QuoteRequesting, Self::Failed)
                | (Self::QuoteRequesting, Self::Cancelled)
                | (Self::QuoteRequesting, Self::Expired)
                // From QuotesReceived
                | (Self::QuotesReceived, Self::ClientSelecting)
                | (Self::QuotesReceived, Self::Failed)
                | (Self::QuotesReceived, Self::Cancelled)
                | (Self::QuotesReceived, Self::Expired)
                // From ClientSelecting
                | (Self::ClientSelecting, Self::Executing)
                | (Self::ClientSelecting, Self::Failed)
                | (Self::ClientSelecting, Self::Cancelled)
                | (Self::ClientSelecting, Self::Expired)
                // From Executing
                | (Self::Executing, Self::Executed)
                | (Self::Executing, Self::Failed)
        )
    }

    /// Returns the valid next states from this state.
    ///
    /// # Examples
    ///
    /// ```
    /// use otc_rfq::domain::value_objects::rfq_state::RfqState;
    ///
    /// let transitions = RfqState::Created.valid_transitions();
    /// assert!(transitions.contains(&RfqState::QuoteRequesting));
    /// assert!(transitions.contains(&RfqState::Cancelled));
    /// ```
    #[must_use]
    pub fn valid_transitions(&self) -> Vec<Self> {
        match self {
            Self::Created => vec![Self::QuoteRequesting, Self::Cancelled, Self::Expired],
            Self::QuoteRequesting => {
                vec![
                    Self::QuotesReceived,
                    Self::Failed,
                    Self::Cancelled,
                    Self::Expired,
                ]
            }
            Self::QuotesReceived => {
                vec![
                    Self::ClientSelecting,
                    Self::Failed,
                    Self::Cancelled,
                    Self::Expired,
                ]
            }
            Self::ClientSelecting => {
                vec![
                    Self::Executing,
                    Self::Failed,
                    Self::Cancelled,
                    Self::Expired,
                ]
            }
            Self::Executing => vec![Self::Executed, Self::Failed],
            // Terminal states have no valid transitions
            Self::Executed | Self::Failed | Self::Cancelled | Self::Expired => vec![],
        }
    }

    /// Returns true if this is an active (non-terminal) state.
    ///
    /// Convenience method, equivalent to `!is_terminal()`.
    #[inline]
    #[must_use]
    pub const fn is_active(&self) -> bool {
        !self.is_terminal()
    }

    /// Returns true if this state indicates the RFQ is awaiting quotes.
    #[inline]
    #[must_use]
    pub const fn is_awaiting_quotes(&self) -> bool {
        matches!(self, Self::QuoteRequesting)
    }

    /// Returns true if this state indicates the RFQ has quotes available.
    #[inline]
    #[must_use]
    pub const fn has_quotes(&self) -> bool {
        matches!(
            self,
            Self::QuotesReceived | Self::ClientSelecting | Self::Executing | Self::Executed
        )
    }

    /// Returns true if this state indicates a successful outcome.
    #[inline]
    #[must_use]
    pub const fn is_success(&self) -> bool {
        matches!(self, Self::Executed)
    }

    /// Returns true if this state indicates a failure outcome.
    #[inline]
    #[must_use]
    pub const fn is_failure(&self) -> bool {
        matches!(self, Self::Failed | Self::Cancelled | Self::Expired)
    }

    /// Returns the numeric value of this state.
    #[inline]
    #[must_use]
    pub const fn as_u8(&self) -> u8 {
        *self as u8
    }
}

impl fmt::Display for RfqState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Created => "CREATED",
            Self::QuoteRequesting => "QUOTE_REQUESTING",
            Self::QuotesReceived => "QUOTES_RECEIVED",
            Self::ClientSelecting => "CLIENT_SELECTING",
            Self::Executing => "EXECUTING",
            Self::Executed => "EXECUTED",
            Self::Failed => "FAILED",
            Self::Cancelled => "CANCELLED",
            Self::Expired => "EXPIRED",
        };
        write!(f, "{}", s)
    }
}

impl TryFrom<u8> for RfqState {
    type Error = InvalidRfqStateError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Created),
            1 => Ok(Self::QuoteRequesting),
            2 => Ok(Self::QuotesReceived),
            3 => Ok(Self::ClientSelecting),
            4 => Ok(Self::Executing),
            5 => Ok(Self::Executed),
            6 => Ok(Self::Failed),
            7 => Ok(Self::Cancelled),
            8 => Ok(Self::Expired),
            _ => Err(InvalidRfqStateError(value)),
        }
    }
}

/// Error returned when converting an invalid u8 to RfqState.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidRfqStateError(pub u8);

impl fmt::Display for InvalidRfqStateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid RFQ state value: {}", self.0)
    }
}

impl std::error::Error for InvalidRfqStateError {}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    mod terminal_states {
        use super::*;

        #[test]
        fn executed_is_terminal() {
            assert!(RfqState::Executed.is_terminal());
        }

        #[test]
        fn failed_is_terminal() {
            assert!(RfqState::Failed.is_terminal());
        }

        #[test]
        fn cancelled_is_terminal() {
            assert!(RfqState::Cancelled.is_terminal());
        }

        #[test]
        fn expired_is_terminal() {
            assert!(RfqState::Expired.is_terminal());
        }

        #[test]
        fn non_terminal_states() {
            assert!(!RfqState::Created.is_terminal());
            assert!(!RfqState::QuoteRequesting.is_terminal());
            assert!(!RfqState::QuotesReceived.is_terminal());
            assert!(!RfqState::ClientSelecting.is_terminal());
            assert!(!RfqState::Executing.is_terminal());
        }
    }

    mod transitions {
        use super::*;

        #[test]
        fn created_transitions() {
            let state = RfqState::Created;
            assert!(state.can_transition_to(RfqState::QuoteRequesting));
            assert!(state.can_transition_to(RfqState::Cancelled));
            assert!(state.can_transition_to(RfqState::Expired));
            assert!(!state.can_transition_to(RfqState::QuotesReceived));
            assert!(!state.can_transition_to(RfqState::Executed));
        }

        #[test]
        fn quote_requesting_transitions() {
            let state = RfqState::QuoteRequesting;
            assert!(state.can_transition_to(RfqState::QuotesReceived));
            assert!(state.can_transition_to(RfqState::Failed));
            assert!(state.can_transition_to(RfqState::Cancelled));
            assert!(state.can_transition_to(RfqState::Expired));
            assert!(!state.can_transition_to(RfqState::Created));
            assert!(!state.can_transition_to(RfqState::Executed));
        }

        #[test]
        fn quotes_received_transitions() {
            let state = RfqState::QuotesReceived;
            assert!(state.can_transition_to(RfqState::ClientSelecting));
            assert!(state.can_transition_to(RfqState::Failed));
            assert!(state.can_transition_to(RfqState::Cancelled));
            assert!(state.can_transition_to(RfqState::Expired));
            assert!(!state.can_transition_to(RfqState::Created));
        }

        #[test]
        fn client_selecting_transitions() {
            let state = RfqState::ClientSelecting;
            assert!(state.can_transition_to(RfqState::Executing));
            assert!(state.can_transition_to(RfqState::Failed));
            assert!(state.can_transition_to(RfqState::Cancelled));
            assert!(state.can_transition_to(RfqState::Expired));
            assert!(!state.can_transition_to(RfqState::Created));
        }

        #[test]
        fn executing_transitions() {
            let state = RfqState::Executing;
            assert!(state.can_transition_to(RfqState::Executed));
            assert!(state.can_transition_to(RfqState::Failed));
            // Cannot cancel or expire during execution
            assert!(!state.can_transition_to(RfqState::Cancelled));
            assert!(!state.can_transition_to(RfqState::Expired));
        }

        #[test]
        fn terminal_states_cannot_transition() {
            for terminal in [
                RfqState::Executed,
                RfqState::Failed,
                RfqState::Cancelled,
                RfqState::Expired,
            ] {
                for target in [
                    RfqState::Created,
                    RfqState::QuoteRequesting,
                    RfqState::QuotesReceived,
                    RfqState::ClientSelecting,
                    RfqState::Executing,
                    RfqState::Executed,
                    RfqState::Failed,
                    RfqState::Cancelled,
                    RfqState::Expired,
                ] {
                    assert!(
                        !terminal.can_transition_to(target),
                        "{:?} should not transition to {:?}",
                        terminal,
                        target
                    );
                }
            }
        }
    }

    mod valid_transitions {
        use super::*;

        #[test]
        fn created_valid_transitions() {
            let transitions = RfqState::Created.valid_transitions();
            assert_eq!(transitions.len(), 3);
            assert!(transitions.contains(&RfqState::QuoteRequesting));
            assert!(transitions.contains(&RfqState::Cancelled));
            assert!(transitions.contains(&RfqState::Expired));
        }

        #[test]
        fn terminal_has_no_transitions() {
            assert!(RfqState::Executed.valid_transitions().is_empty());
            assert!(RfqState::Failed.valid_transitions().is_empty());
            assert!(RfqState::Cancelled.valid_transitions().is_empty());
            assert!(RfqState::Expired.valid_transitions().is_empty());
        }
    }

    mod helpers {
        use super::*;

        #[test]
        fn is_active() {
            assert!(RfqState::Created.is_active());
            assert!(RfqState::QuoteRequesting.is_active());
            assert!(!RfqState::Executed.is_active());
        }

        #[test]
        fn is_awaiting_quotes() {
            assert!(RfqState::QuoteRequesting.is_awaiting_quotes());
            assert!(!RfqState::Created.is_awaiting_quotes());
            assert!(!RfqState::QuotesReceived.is_awaiting_quotes());
        }

        #[test]
        fn has_quotes() {
            assert!(!RfqState::Created.has_quotes());
            assert!(!RfqState::QuoteRequesting.has_quotes());
            assert!(RfqState::QuotesReceived.has_quotes());
            assert!(RfqState::ClientSelecting.has_quotes());
            assert!(RfqState::Executing.has_quotes());
            assert!(RfqState::Executed.has_quotes());
        }

        #[test]
        fn is_success() {
            assert!(RfqState::Executed.is_success());
            assert!(!RfqState::Failed.is_success());
            assert!(!RfqState::Created.is_success());
        }

        #[test]
        fn is_failure() {
            assert!(RfqState::Failed.is_failure());
            assert!(RfqState::Cancelled.is_failure());
            assert!(RfqState::Expired.is_failure());
            assert!(!RfqState::Executed.is_failure());
            assert!(!RfqState::Created.is_failure());
        }
    }

    mod conversion {
        use super::*;

        #[test]
        fn as_u8() {
            assert_eq!(RfqState::Created.as_u8(), 0);
            assert_eq!(RfqState::QuoteRequesting.as_u8(), 1);
            assert_eq!(RfqState::Expired.as_u8(), 8);
        }

        #[test]
        fn try_from_u8_valid() {
            assert_eq!(RfqState::try_from(0).unwrap(), RfqState::Created);
            assert_eq!(RfqState::try_from(5).unwrap(), RfqState::Executed);
            assert_eq!(RfqState::try_from(8).unwrap(), RfqState::Expired);
        }

        #[test]
        fn try_from_u8_invalid() {
            assert!(RfqState::try_from(9).is_err());
            assert!(RfqState::try_from(255).is_err());
        }

        #[test]
        fn roundtrip_u8() {
            for i in 0..=8 {
                let state = RfqState::try_from(i).unwrap();
                assert_eq!(state.as_u8(), i);
            }
        }
    }

    mod display {
        use super::*;

        #[test]
        fn display_format() {
            assert_eq!(RfqState::Created.to_string(), "CREATED");
            assert_eq!(RfqState::QuoteRequesting.to_string(), "QUOTE_REQUESTING");
            assert_eq!(RfqState::QuotesReceived.to_string(), "QUOTES_RECEIVED");
            assert_eq!(RfqState::ClientSelecting.to_string(), "CLIENT_SELECTING");
            assert_eq!(RfqState::Executing.to_string(), "EXECUTING");
            assert_eq!(RfqState::Executed.to_string(), "EXECUTED");
            assert_eq!(RfqState::Failed.to_string(), "FAILED");
            assert_eq!(RfqState::Cancelled.to_string(), "CANCELLED");
            assert_eq!(RfqState::Expired.to_string(), "EXPIRED");
        }
    }

    mod serde {
        use super::*;

        #[test]
        fn serde_roundtrip() {
            for state in [
                RfqState::Created,
                RfqState::QuoteRequesting,
                RfqState::QuotesReceived,
                RfqState::ClientSelecting,
                RfqState::Executing,
                RfqState::Executed,
                RfqState::Failed,
                RfqState::Cancelled,
                RfqState::Expired,
            ] {
                let json = serde_json::to_string(&state).unwrap();
                let deserialized: RfqState = serde_json::from_str(&json).unwrap();
                assert_eq!(state, deserialized);
            }
        }

        #[test]
        fn serde_screaming_snake_case() {
            let json = serde_json::to_string(&RfqState::QuoteRequesting).unwrap();
            assert_eq!(json, "\"QUOTE_REQUESTING\"");

            let json = serde_json::to_string(&RfqState::ClientSelecting).unwrap();
            assert_eq!(json, "\"CLIENT_SELECTING\"");
        }
    }

    mod default {
        use super::*;

        #[test]
        fn default_is_created() {
            assert_eq!(RfqState::default(), RfqState::Created);
        }
    }
}
