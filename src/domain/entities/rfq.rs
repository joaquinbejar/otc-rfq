//! # RFQ Aggregate Root
//!
//! The RFQ (Request-for-Quote) aggregate manages the lifecycle of a quote request.
//!
//! This module provides the [`Rfq`] aggregate root, which is the central entity
//! managing the RFQ lifecycle including state transitions, quote collection,
//! and execution tracking.
//!
//! # State Machine
//!
//! The RFQ follows a strict state machine:
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
//! use otc_rfq::domain::entities::rfq::{Rfq, RfqBuilder};
//! use otc_rfq::domain::value_objects::{
//!     CounterpartyId, Instrument, OrderSide, Quantity, Symbol,
//! };
//! use otc_rfq::domain::value_objects::enums::AssetClass;
//! use otc_rfq::domain::value_objects::timestamp::Timestamp;
//!
//! let symbol = Symbol::new("BTC/USD").unwrap();
//! let instrument = Instrument::builder(symbol, AssetClass::CryptoSpot).build();
//! let rfq = RfqBuilder::new(
//!     CounterpartyId::new("client-1"),
//!     instrument,
//!     OrderSide::Buy,
//!     Quantity::new(1.0).unwrap(),
//!     Timestamp::now().add_secs(300),
//! ).build();
//!
//! assert!(rfq.is_active());
//! ```

use crate::domain::entities::anonymity::{AnonymityLevel, AnonymousRfqView};
use crate::domain::entities::quote::Quote;
use crate::domain::errors::{DomainError, DomainResult};
use crate::domain::value_objects::timestamp::Timestamp;
use crate::domain::value_objects::{
    CounterpartyId, Instrument, OrderSide, Quantity, QuoteId, RfqId, RfqState,
};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Result of a compliance check.
///
/// Contains the outcome of KYC/AML compliance verification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComplianceResult {
    /// Whether the compliance check passed.
    pub passed: bool,
    /// Reason for failure, if any.
    pub reason: Option<String>,
    /// When the check was performed.
    pub checked_at: Timestamp,
}

impl ComplianceResult {
    /// Creates a passing compliance result.
    #[must_use]
    pub fn passed() -> Self {
        Self {
            passed: true,
            reason: None,
            checked_at: Timestamp::now(),
        }
    }

    /// Creates a failing compliance result with a reason.
    #[must_use]
    pub fn failed(reason: impl Into<String>) -> Self {
        Self {
            passed: false,
            reason: Some(reason.into()),
            checked_at: Timestamp::now(),
        }
    }
}

/// RFQ (Request-for-Quote) aggregate root.
///
/// The central entity managing the RFQ lifecycle, including state transitions,
/// quote collection, quote selection, and execution tracking.
///
/// # Invariants
///
/// - Valid state transitions only (FSM enforced)
/// - Quote must belong to this RFQ
/// - Cannot select expired quote
/// - Single active execution at a time
///
/// # Examples
///
/// ```
/// use otc_rfq::domain::entities::rfq::{Rfq, RfqBuilder};
/// use otc_rfq::domain::value_objects::{
///     CounterpartyId, Instrument, OrderSide, Quantity, RfqState, Symbol,
/// };
/// use otc_rfq::domain::value_objects::enums::AssetClass;
/// use otc_rfq::domain::value_objects::timestamp::Timestamp;
///
/// let symbol = Symbol::new("BTC/USD").unwrap();
/// let instrument = Instrument::builder(symbol, AssetClass::CryptoSpot).build();
/// let mut rfq = RfqBuilder::new(
///     CounterpartyId::new("client-1"),
///     instrument,
///     OrderSide::Buy,
///     Quantity::new(1.0).unwrap(),
///     Timestamp::now().add_secs(300),
/// ).build();
///
/// // Start quote collection
/// rfq.start_quote_collection().unwrap();
/// assert_eq!(rfq.state(), RfqState::QuoteRequesting);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rfq {
    /// Unique identifier for this RFQ.
    id: RfqId,
    /// The client requesting the quote.
    client_id: CounterpartyId,
    /// The instrument being quoted.
    instrument: Instrument,
    /// Buy or sell side.
    side: OrderSide,
    /// Requested quantity.
    quantity: Quantity,
    /// Optional minimum acceptable quantity for partial fills.
    min_quantity: Option<Quantity>,
    /// Anonymity level for this RFQ.
    anonymity_level: AnonymityLevel,
    /// Current state in the lifecycle.
    state: RfqState,
    /// When this RFQ expires.
    expires_at: Timestamp,
    /// Quotes received from venues.
    quotes: Vec<Quote>,
    /// The selected quote for execution.
    selected_quote_id: Option<QuoteId>,
    /// Compliance check results.
    compliance_result: Option<ComplianceResult>,
    /// Reason for failure, if failed.
    failure_reason: Option<String>,
    /// Version for optimistic locking.
    version: u64,
    /// When this RFQ was created.
    created_at: Timestamp,
    /// When this RFQ was last updated.
    updated_at: Timestamp,
}

impl Rfq {
    /// Creates a new RFQ with validation.
    ///
    /// # Arguments
    ///
    /// * `client_id` - The client requesting the quote
    /// * `instrument` - The instrument to quote
    /// * `side` - Buy or sell
    /// * `quantity` - The quantity to quote (must be positive)
    /// * `expires_at` - When this RFQ expires (must be in the future)
    ///
    /// # Errors
    ///
    /// Returns `DomainError::InvalidQuantity` if quantity is not positive.
    /// Returns `DomainError::ValidationError` if expires_at is in the past.
    pub fn new(
        client_id: CounterpartyId,
        instrument: Instrument,
        side: OrderSide,
        quantity: Quantity,
        expires_at: Timestamp,
    ) -> DomainResult<Self> {
        Self::validate_quantity(&quantity)?;
        Self::validate_expiry(&expires_at)?;

        let now = Timestamp::now();
        Ok(Self {
            id: RfqId::new_v4(),
            client_id,
            instrument,
            side,
            quantity,
            min_quantity: None,
            anonymity_level: AnonymityLevel::default(),
            state: RfqState::Created,
            expires_at,
            quotes: Vec::new(),
            selected_quote_id: None,
            compliance_result: None,
            failure_reason: None,
            version: 1,
            created_at: now,
            updated_at: now,
        })
    }

    /// Creates an RFQ with a specific ID (for reconstruction from storage).
    ///
    /// # Safety
    ///
    /// This method bypasses validation and should only be used when
    /// reconstructing from trusted storage.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn from_parts(
        id: RfqId,
        client_id: CounterpartyId,
        instrument: Instrument,
        side: OrderSide,
        quantity: Quantity,
        min_quantity: Option<Quantity>,
        anonymity_level: AnonymityLevel,
        state: RfqState,
        expires_at: Timestamp,
        quotes: Vec<Quote>,
        selected_quote_id: Option<QuoteId>,
        compliance_result: Option<ComplianceResult>,
        failure_reason: Option<String>,
        version: u64,
        created_at: Timestamp,
        updated_at: Timestamp,
    ) -> Self {
        Self {
            id,
            client_id,
            instrument,
            side,
            quantity,
            min_quantity,
            anonymity_level,
            state,
            expires_at,
            quotes,
            selected_quote_id,
            compliance_result,
            failure_reason,
            version,
            created_at,
            updated_at,
        }
    }

    /// Returns a builder for constructing an RFQ.
    #[must_use]
    pub fn builder(
        client_id: CounterpartyId,
        instrument: Instrument,
        side: OrderSide,
        quantity: Quantity,
        expires_at: Timestamp,
    ) -> RfqBuilder {
        RfqBuilder::new(client_id, instrument, side, quantity, expires_at)
    }

    fn validate_quantity(quantity: &Quantity) -> DomainResult<()> {
        if !quantity.is_positive() {
            return Err(DomainError::InvalidQuantity(
                "quantity must be positive".to_string(),
            ));
        }
        Ok(())
    }

    fn validate_expiry(expires_at: &Timestamp) -> DomainResult<()> {
        if expires_at.is_expired() {
            return Err(DomainError::ValidationError(
                "expires_at must be in the future".to_string(),
            ));
        }
        Ok(())
    }

    fn transition_to(&mut self, target: RfqState) -> DomainResult<()> {
        if !self.state.can_transition_to(target) {
            return Err(DomainError::InvalidStateTransition {
                from: self.state,
                to: target,
            });
        }
        self.state = target;
        self.updated_at = Timestamp::now();
        self.version = self.version.saturating_add(1);
        Ok(())
    }

    // ========== Accessors ==========

    /// Returns the RFQ ID.
    #[inline]
    #[must_use]
    pub fn id(&self) -> RfqId {
        self.id
    }

    /// Returns the client ID.
    #[inline]
    #[must_use]
    pub fn client_id(&self) -> &CounterpartyId {
        &self.client_id
    }

    /// Returns the instrument.
    #[inline]
    #[must_use]
    pub fn instrument(&self) -> &Instrument {
        &self.instrument
    }

    /// Returns the order side.
    #[inline]
    #[must_use]
    pub fn side(&self) -> OrderSide {
        self.side
    }

    /// Returns the quantity.
    #[inline]
    #[must_use]
    pub fn quantity(&self) -> Quantity {
        self.quantity
    }

    /// Returns the minimum acceptable quantity for partial fills, if set.
    #[inline]
    #[must_use]
    pub fn min_quantity(&self) -> Option<Quantity> {
        self.min_quantity
    }

    /// Returns the anonymity level.
    #[inline]
    #[must_use]
    pub fn anonymity_level(&self) -> AnonymityLevel {
        self.anonymity_level
    }

    /// Returns true if this RFQ is anonymous.
    #[inline]
    #[must_use]
    pub fn is_anonymous(&self) -> bool {
        self.anonymity_level.is_anonymous()
    }

    /// Creates an anonymized view of this RFQ for broadcasting to market makers.
    ///
    /// The view contains only the information needed for quoting,
    /// without revealing the requester's identity.
    #[must_use]
    pub fn to_anonymous_view(&self) -> AnonymousRfqView {
        AnonymousRfqView::new(
            self.id,
            self.instrument.clone(),
            self.side,
            self.quantity,
            self.expires_at,
            self.anonymity_level,
        )
    }

    /// Returns the current state.
    #[inline]
    #[must_use]
    pub fn state(&self) -> RfqState {
        self.state
    }

    /// Returns when this RFQ expires.
    #[inline]
    #[must_use]
    pub fn expires_at(&self) -> Timestamp {
        self.expires_at
    }

    /// Returns the quotes received.
    #[inline]
    #[must_use]
    pub fn quotes(&self) -> &[Quote] {
        &self.quotes
    }

    /// Returns the selected quote ID, if any.
    #[inline]
    #[must_use]
    pub fn selected_quote_id(&self) -> Option<QuoteId> {
        self.selected_quote_id
    }

    /// Returns the selected quote, if any.
    #[must_use]
    pub fn selected_quote(&self) -> Option<&Quote> {
        self.selected_quote_id
            .and_then(|id| self.quotes.iter().find(|q| q.id() == id))
    }

    /// Returns the compliance result, if any.
    #[inline]
    #[must_use]
    pub fn compliance_result(&self) -> Option<&ComplianceResult> {
        self.compliance_result.as_ref()
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

    /// Returns when this RFQ was created.
    #[inline]
    #[must_use]
    pub fn created_at(&self) -> Timestamp {
        self.created_at
    }

    /// Returns when this RFQ was last updated.
    #[inline]
    #[must_use]
    pub fn updated_at(&self) -> Timestamp {
        self.updated_at
    }

    // ========== State Helpers ==========

    /// Returns true if this RFQ is in an active (non-terminal) state.
    #[inline]
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.state.is_active()
    }

    /// Returns true if this RFQ has expired.
    #[must_use]
    pub fn is_expired(&self) -> bool {
        self.expires_at.is_expired()
    }

    /// Returns the number of quotes received.
    #[inline]
    #[must_use]
    pub fn quote_count(&self) -> usize {
        self.quotes.len()
    }

    /// Returns true if any quotes have been received.
    #[inline]
    #[must_use]
    pub fn has_quotes(&self) -> bool {
        !self.quotes.is_empty()
    }

    // ========== State Transitions ==========

    /// Starts quote collection from venues.
    ///
    /// Transitions: Created → QuoteRequesting
    ///
    /// # Errors
    ///
    /// Returns `DomainError::InvalidStateTransition` if not in Created state.
    pub fn start_quote_collection(&mut self) -> DomainResult<()> {
        self.transition_to(RfqState::QuoteRequesting)
    }

    /// Receives a quote from a venue.
    ///
    /// Transitions: QuoteRequesting → QuotesReceived (on first quote)
    ///
    /// # Arguments
    ///
    /// * `quote` - The quote to add
    ///
    /// # Errors
    ///
    /// Returns `DomainError::InvalidStateTransition` if not in QuoteRequesting or QuotesReceived state.
    /// Returns `DomainError::ValidationError` if quote doesn't belong to this RFQ.
    /// Returns `DomainError::QuoteExpired` if the quote has expired.
    pub fn receive_quote(&mut self, quote: Quote) -> DomainResult<()> {
        // Validate quote belongs to this RFQ
        if quote.rfq_id() != self.id {
            return Err(DomainError::ValidationError(
                "quote does not belong to this RFQ".to_string(),
            ));
        }

        // Validate quote is not expired
        if quote.is_expired() {
            return Err(DomainError::QuoteExpired(
                "cannot receive expired quote".to_string(),
            ));
        }

        // Check we're in a valid state to receive quotes
        match self.state {
            RfqState::QuoteRequesting => {
                self.quotes.push(quote);
                self.transition_to(RfqState::QuotesReceived)?;
            }
            RfqState::QuotesReceived => {
                self.quotes.push(quote);
                self.updated_at = Timestamp::now();
                self.version = self.version.saturating_add(1);
            }
            _ => {
                return Err(DomainError::InvalidStateTransition {
                    from: self.state,
                    to: RfqState::QuotesReceived,
                });
            }
        }

        Ok(())
    }

    /// Selects a quote for execution.
    ///
    /// Transitions: QuotesReceived → ClientSelecting
    ///
    /// # Arguments
    ///
    /// * `quote_id` - The ID of the quote to select
    ///
    /// # Errors
    ///
    /// Returns `DomainError::InvalidStateTransition` if not in QuotesReceived state.
    /// Returns `DomainError::QuoteNotFound` if quote doesn't exist.
    /// Returns `DomainError::QuoteExpired` if the selected quote has expired.
    pub fn select_quote(&mut self, quote_id: QuoteId) -> DomainResult<()> {
        // Find the quote
        let quote = self
            .quotes
            .iter()
            .find(|q| q.id() == quote_id)
            .ok_or_else(|| DomainError::QuoteNotFound(quote_id.to_string()))?;

        // Validate quote is not expired
        if quote.is_expired() {
            return Err(DomainError::QuoteExpired(
                "cannot select expired quote".to_string(),
            ));
        }

        // Transition state
        self.transition_to(RfqState::ClientSelecting)?;
        self.selected_quote_id = Some(quote_id);

        Ok(())
    }

    /// Starts execution of the selected quote.
    ///
    /// Transitions: ClientSelecting → Executing
    ///
    /// # Errors
    ///
    /// Returns `DomainError::InvalidStateTransition` if not in ClientSelecting state.
    /// Returns `DomainError::ValidationError` if no quote is selected.
    /// Returns `DomainError::QuoteExpired` if the selected quote has expired.
    pub fn start_execution(&mut self) -> DomainResult<()> {
        // Validate a quote is selected
        let quote_id = self.selected_quote_id.ok_or_else(|| {
            DomainError::ValidationError("no quote selected for execution".to_string())
        })?;

        // Validate selected quote is not expired
        let quote = self
            .quotes
            .iter()
            .find(|q| q.id() == quote_id)
            .ok_or_else(|| DomainError::QuoteNotFound(quote_id.to_string()))?;

        if quote.is_expired() {
            return Err(DomainError::QuoteExpired(
                "selected quote has expired".to_string(),
            ));
        }

        self.transition_to(RfqState::Executing)
    }

    /// Marks the RFQ as successfully executed.
    ///
    /// Transitions: Executing → Executed
    ///
    /// # Errors
    ///
    /// Returns `DomainError::InvalidStateTransition` if not in Executing state.
    pub fn mark_executed(&mut self) -> DomainResult<()> {
        self.transition_to(RfqState::Executed)
    }

    /// Marks the RFQ as failed.
    ///
    /// Transitions: QuoteRequesting/QuotesReceived/ClientSelecting/Executing → Failed
    ///
    /// # Arguments
    ///
    /// * `reason` - The reason for failure
    ///
    /// # Errors
    ///
    /// Returns `DomainError::InvalidStateTransition` if in a terminal state.
    pub fn mark_failed(&mut self, reason: impl Into<String>) -> DomainResult<()> {
        self.failure_reason = Some(reason.into());
        self.transition_to(RfqState::Failed)
    }

    /// Cancels the RFQ.
    ///
    /// Transitions: Created/QuoteRequesting/QuotesReceived/ClientSelecting → Cancelled
    ///
    /// # Errors
    ///
    /// Returns `DomainError::InvalidStateTransition` if in Executing or terminal state.
    pub fn cancel(&mut self) -> DomainResult<()> {
        self.transition_to(RfqState::Cancelled)
    }

    /// Expires the RFQ.
    ///
    /// Transitions: Created/QuoteRequesting/QuotesReceived/ClientSelecting → Expired
    ///
    /// # Errors
    ///
    /// Returns `DomainError::InvalidStateTransition` if in Executing or terminal state.
    pub fn expire(&mut self) -> DomainResult<()> {
        self.transition_to(RfqState::Expired)
    }

    /// Sets the compliance result.
    pub fn set_compliance_result(&mut self, result: ComplianceResult) {
        self.compliance_result = Some(result);
        self.updated_at = Timestamp::now();
        self.version = self.version.saturating_add(1);
    }
}

impl fmt::Display for Rfq {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "RFQ({} {} {} {} [{}])",
            self.id, self.side, self.quantity, self.instrument, self.state
        )
    }
}

/// Builder for constructing [`Rfq`] instances.
///
/// Provides a fluent API for setting optional fields.
///
/// # Examples
///
/// ```
/// use otc_rfq::domain::entities::rfq::RfqBuilder;
/// use otc_rfq::domain::value_objects::{
///     CounterpartyId, Instrument, OrderSide, Quantity, Symbol,
/// };
/// use otc_rfq::domain::value_objects::enums::AssetClass;
/// use otc_rfq::domain::value_objects::timestamp::Timestamp;
///
/// let symbol = Symbol::new("BTC/USD").unwrap();
/// let instrument = Instrument::builder(symbol, AssetClass::CryptoSpot).build();
/// let rfq = RfqBuilder::new(
///     CounterpartyId::new("client-1"),
///     instrument,
///     OrderSide::Buy,
///     Quantity::new(1.0).unwrap(),
///     Timestamp::now().add_secs(300),
/// ).build();
/// ```
#[derive(Debug, Clone)]
pub struct RfqBuilder {
    client_id: CounterpartyId,
    instrument: Instrument,
    side: OrderSide,
    quantity: Quantity,
    min_quantity: Option<Quantity>,
    anonymity_level: AnonymityLevel,
    expires_at: Timestamp,
}

impl RfqBuilder {
    /// Creates a new builder with required fields.
    #[must_use]
    pub fn new(
        client_id: CounterpartyId,
        instrument: Instrument,
        side: OrderSide,
        quantity: Quantity,
        expires_at: Timestamp,
    ) -> Self {
        Self {
            client_id,
            instrument,
            side,
            quantity,
            min_quantity: None,
            anonymity_level: AnonymityLevel::default(),
            expires_at,
        }
    }

    /// Sets the minimum acceptable quantity for partial fills.
    #[must_use]
    pub fn min_quantity(mut self, min_quantity: Quantity) -> Self {
        self.min_quantity = Some(min_quantity);
        self
    }

    /// Sets the anonymity level for this RFQ.
    #[must_use]
    pub fn anonymity_level(mut self, level: AnonymityLevel) -> Self {
        self.anonymity_level = level;
        self
    }

    /// Sets the RFQ to full anonymous mode.
    #[must_use]
    pub fn anonymous(mut self) -> Self {
        self.anonymity_level = AnonymityLevel::FullAnonymous;
        self
    }

    /// Builds the RFQ without validation.
    ///
    /// Use [`try_build`](Self::try_build) for validated construction.
    #[must_use]
    pub fn build(self) -> Rfq {
        let now = Timestamp::now();
        Rfq {
            id: RfqId::new_v4(),
            client_id: self.client_id,
            instrument: self.instrument,
            side: self.side,
            quantity: self.quantity,
            min_quantity: self.min_quantity,
            anonymity_level: self.anonymity_level,
            state: RfqState::Created,
            expires_at: self.expires_at,
            quotes: Vec::new(),
            selected_quote_id: None,
            compliance_result: None,
            failure_reason: None,
            version: 1,
            created_at: now,
            updated_at: now,
        }
    }

    /// Builds the RFQ with validation.
    ///
    /// # Errors
    ///
    /// Returns `DomainError` if validation fails.
    pub fn try_build(self) -> DomainResult<Rfq> {
        Rfq::new(
            self.client_id,
            self.instrument,
            self.side,
            self.quantity,
            self.expires_at,
        )
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::domain::entities::quote::QuoteBuilder;
    use crate::domain::value_objects::{Price, VenueId};

    fn test_client_id() -> CounterpartyId {
        CounterpartyId::new("test-client")
    }

    fn test_instrument() -> Instrument {
        use crate::domain::value_objects::{AssetClass, Symbol};
        let symbol = Symbol::new("BTC/USD").unwrap();
        Instrument::builder(symbol, AssetClass::CryptoSpot).build()
    }

    fn test_quantity() -> Quantity {
        Quantity::new(1.0).unwrap()
    }

    fn future_timestamp() -> Timestamp {
        Timestamp::now().add_secs(300)
    }

    fn past_timestamp() -> Timestamp {
        Timestamp::now().sub_secs(300)
    }

    fn create_test_rfq() -> Rfq {
        RfqBuilder::new(
            test_client_id(),
            test_instrument(),
            OrderSide::Buy,
            test_quantity(),
            future_timestamp(),
        )
        .build()
    }

    fn create_test_quote(rfq_id: RfqId) -> Quote {
        QuoteBuilder::new(
            rfq_id,
            VenueId::new("test-venue"),
            Price::new(50000.0).unwrap(),
            Quantity::new(1.0).unwrap(),
            future_timestamp(),
        )
        .build()
    }

    mod construction {
        use super::*;

        #[test]
        fn new_creates_valid_rfq() {
            let rfq = Rfq::new(
                test_client_id(),
                test_instrument(),
                OrderSide::Buy,
                test_quantity(),
                future_timestamp(),
            )
            .unwrap();

            assert_eq!(rfq.state(), RfqState::Created);
            assert_eq!(rfq.version(), 1);
            assert!(rfq.quotes().is_empty());
            assert!(rfq.selected_quote_id().is_none());
        }

        #[test]
        fn new_fails_with_zero_quantity() {
            let result = Rfq::new(
                test_client_id(),
                test_instrument(),
                OrderSide::Buy,
                Quantity::zero(),
                future_timestamp(),
            );

            assert!(matches!(result, Err(DomainError::InvalidQuantity(_))));
        }

        #[test]
        fn new_fails_with_expired_timestamp() {
            let result = Rfq::new(
                test_client_id(),
                test_instrument(),
                OrderSide::Buy,
                test_quantity(),
                past_timestamp(),
            );

            assert!(matches!(result, Err(DomainError::ValidationError(_))));
        }

        #[test]
        fn builder_creates_rfq() {
            let rfq = create_test_rfq();
            assert_eq!(rfq.state(), RfqState::Created);
            assert!(rfq.is_active());
        }
    }

    mod state_transitions {
        use super::*;

        #[test]
        fn start_quote_collection_from_created() {
            let mut rfq = create_test_rfq();
            assert!(rfq.start_quote_collection().is_ok());
            assert_eq!(rfq.state(), RfqState::QuoteRequesting);
            assert_eq!(rfq.version(), 2);
        }

        #[test]
        fn start_quote_collection_fails_from_wrong_state() {
            let mut rfq = create_test_rfq();
            rfq.start_quote_collection().unwrap();

            let result = rfq.start_quote_collection();
            assert!(matches!(
                result,
                Err(DomainError::InvalidStateTransition { .. })
            ));
        }

        #[test]
        fn receive_quote_transitions_to_quotes_received() {
            let mut rfq = create_test_rfq();
            rfq.start_quote_collection().unwrap();

            let quote = create_test_quote(rfq.id());
            assert!(rfq.receive_quote(quote).is_ok());
            assert_eq!(rfq.state(), RfqState::QuotesReceived);
            assert_eq!(rfq.quote_count(), 1);
        }

        #[test]
        fn receive_multiple_quotes() {
            let mut rfq = create_test_rfq();
            rfq.start_quote_collection().unwrap();

            let quote1 = create_test_quote(rfq.id());
            let quote2 = create_test_quote(rfq.id());

            rfq.receive_quote(quote1).unwrap();
            rfq.receive_quote(quote2).unwrap();

            assert_eq!(rfq.quote_count(), 2);
            assert_eq!(rfq.state(), RfqState::QuotesReceived);
        }

        #[test]
        fn receive_quote_fails_for_wrong_rfq() {
            let mut rfq = create_test_rfq();
            rfq.start_quote_collection().unwrap();

            let wrong_quote = create_test_quote(RfqId::new_v4());
            let result = rfq.receive_quote(wrong_quote);

            assert!(matches!(result, Err(DomainError::ValidationError(_))));
        }

        #[test]
        fn select_quote_transitions_to_client_selecting() {
            let mut rfq = create_test_rfq();
            rfq.start_quote_collection().unwrap();

            let quote = create_test_quote(rfq.id());
            let quote_id = quote.id();
            rfq.receive_quote(quote).unwrap();

            assert!(rfq.select_quote(quote_id).is_ok());
            assert_eq!(rfq.state(), RfqState::ClientSelecting);
            assert_eq!(rfq.selected_quote_id(), Some(quote_id));
        }

        #[test]
        fn select_quote_fails_for_nonexistent_quote() {
            let mut rfq = create_test_rfq();
            rfq.start_quote_collection().unwrap();

            let quote = create_test_quote(rfq.id());
            rfq.receive_quote(quote).unwrap();

            let result = rfq.select_quote(QuoteId::new_v4());
            assert!(matches!(result, Err(DomainError::QuoteNotFound(_))));
        }

        #[test]
        fn start_execution_transitions_to_executing() {
            let mut rfq = create_test_rfq();
            rfq.start_quote_collection().unwrap();

            let quote = create_test_quote(rfq.id());
            let quote_id = quote.id();
            rfq.receive_quote(quote).unwrap();
            rfq.select_quote(quote_id).unwrap();

            assert!(rfq.start_execution().is_ok());
            assert_eq!(rfq.state(), RfqState::Executing);
        }

        #[test]
        fn start_execution_fails_without_selected_quote() {
            let mut rfq = create_test_rfq();
            rfq.start_quote_collection().unwrap();

            let quote = create_test_quote(rfq.id());
            rfq.receive_quote(quote).unwrap();

            // Skip select_quote, try to execute directly
            // Need to manually set state for this test
            rfq.state = RfqState::ClientSelecting;

            let result = rfq.start_execution();
            assert!(matches!(result, Err(DomainError::ValidationError(_))));
        }

        #[test]
        fn mark_executed_transitions_to_executed() {
            let mut rfq = create_test_rfq();
            rfq.start_quote_collection().unwrap();

            let quote = create_test_quote(rfq.id());
            let quote_id = quote.id();
            rfq.receive_quote(quote).unwrap();
            rfq.select_quote(quote_id).unwrap();
            rfq.start_execution().unwrap();

            assert!(rfq.mark_executed().is_ok());
            assert_eq!(rfq.state(), RfqState::Executed);
            assert!(!rfq.is_active());
        }

        #[test]
        fn mark_failed_transitions_to_failed() {
            let mut rfq = create_test_rfq();
            rfq.start_quote_collection().unwrap();

            assert!(rfq.mark_failed("test failure").is_ok());
            assert_eq!(rfq.state(), RfqState::Failed);
            assert_eq!(rfq.failure_reason(), Some("test failure"));
        }

        #[test]
        fn cancel_transitions_to_cancelled() {
            let mut rfq = create_test_rfq();
            rfq.start_quote_collection().unwrap();

            assert!(rfq.cancel().is_ok());
            assert_eq!(rfq.state(), RfqState::Cancelled);
        }

        #[test]
        fn cancel_fails_during_execution() {
            let mut rfq = create_test_rfq();
            rfq.start_quote_collection().unwrap();

            let quote = create_test_quote(rfq.id());
            let quote_id = quote.id();
            rfq.receive_quote(quote).unwrap();
            rfq.select_quote(quote_id).unwrap();
            rfq.start_execution().unwrap();

            let result = rfq.cancel();
            assert!(matches!(
                result,
                Err(DomainError::InvalidStateTransition { .. })
            ));
        }

        #[test]
        fn expire_transitions_to_expired() {
            let mut rfq = create_test_rfq();
            assert!(rfq.expire().is_ok());
            assert_eq!(rfq.state(), RfqState::Expired);
        }
    }

    mod version {
        use super::*;

        #[test]
        fn version_increments_on_transition() {
            let mut rfq = create_test_rfq();
            assert_eq!(rfq.version(), 1);

            rfq.start_quote_collection().unwrap();
            assert_eq!(rfq.version(), 2);

            let quote = create_test_quote(rfq.id());
            rfq.receive_quote(quote).unwrap();
            assert_eq!(rfq.version(), 3);
        }
    }

    mod compliance {
        use super::*;

        #[test]
        fn set_compliance_result() {
            let mut rfq = create_test_rfq();
            let result = ComplianceResult::passed();

            rfq.set_compliance_result(result.clone());

            assert!(rfq.compliance_result().is_some());
            assert!(rfq.compliance_result().unwrap().passed);
        }

        #[test]
        fn compliance_result_failed() {
            let result = ComplianceResult::failed("KYC not verified");
            assert!(!result.passed);
            assert_eq!(result.reason, Some("KYC not verified".to_string()));
        }
    }

    mod helpers {
        use super::*;

        #[test]
        fn is_active() {
            let rfq = create_test_rfq();
            assert!(rfq.is_active());
        }

        #[test]
        fn has_quotes() {
            let mut rfq = create_test_rfq();
            assert!(!rfq.has_quotes());

            rfq.start_quote_collection().unwrap();
            let quote = create_test_quote(rfq.id());
            rfq.receive_quote(quote).unwrap();

            assert!(rfq.has_quotes());
        }

        #[test]
        fn selected_quote() {
            let mut rfq = create_test_rfq();
            rfq.start_quote_collection().unwrap();

            let quote = create_test_quote(rfq.id());
            let quote_id = quote.id();
            rfq.receive_quote(quote).unwrap();
            rfq.select_quote(quote_id).unwrap();

            let selected = rfq.selected_quote();
            assert!(selected.is_some());
            assert_eq!(selected.unwrap().id(), quote_id);
        }
    }

    mod display {
        use super::*;

        #[test]
        fn display_format() {
            let rfq = create_test_rfq();
            let display = rfq.to_string();

            assert!(display.contains("RFQ"));
            assert!(display.contains("BUY"));
            assert!(display.contains("CREATED"));
        }
    }

    mod serde {
        use super::*;

        #[test]
        fn serde_roundtrip() {
            let rfq = create_test_rfq();

            let json = serde_json::to_string(&rfq).unwrap();
            let deserialized: Rfq = serde_json::from_str(&json).unwrap();

            assert_eq!(rfq.id(), deserialized.id());
            assert_eq!(rfq.state(), deserialized.state());
            assert_eq!(rfq.version(), deserialized.version());
        }
    }
}
