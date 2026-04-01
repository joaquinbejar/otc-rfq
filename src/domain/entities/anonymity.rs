//! # Anonymity Entities
//!
//! Domain entities for managing RFQ anonymity.
//!
//! This module provides types for hiding requester identity from market makers
//! during the quoting phase, with controlled reveal post-execution for settlement.
//!
//! # Privacy Model
//!
//! ```text
//! RFQ Created (anonymous=true)
//!     ↓
//! AnonymousRfqView broadcast to MMs (no client_id)
//!     ↓
//! Quotes received (MMs quote blind)
//!     ↓
//! Trade executed
//!     ↓
//! IdentityRevealed event (for settlement)
//! ```
//!
//! # Examples
//!
//! ```
//! use otc_rfq::domain::entities::anonymity::{AnonymityLevel, AnonymousRfqView};
//! use otc_rfq::domain::value_objects::{
//!     Instrument, OrderSide, Quantity, RfqId, Symbol,
//! };
//! use otc_rfq::domain::value_objects::enums::AssetClass;
//! use otc_rfq::domain::value_objects::timestamp::Timestamp;
//!
//! let symbol = Symbol::new("BTC/USD").unwrap();
//! let instrument = Instrument::builder(symbol, AssetClass::CryptoSpot).build();
//! let view = AnonymousRfqView::new(
//!     RfqId::new_v4(),
//!     instrument,
//!     OrderSide::Buy,
//!     Quantity::new(10.0).unwrap(),
//!     Timestamp::now().add_secs(300),
//!     AnonymityLevel::FullAnonymous,
//! );
//!
//! assert!(view.is_anonymous());
//! ```

use crate::domain::value_objects::timestamp::Timestamp;
use crate::domain::value_objects::{CounterpartyId, Instrument, OrderSide, Quantity, RfqId};
use serde::{Deserialize, Serialize};
use std::fmt;

// ============================================================================
// AnonymityLevel
// ============================================================================

/// Level of anonymity for an RFQ.
///
/// Controls how much identity information is visible to market makers
/// during the quoting phase.
///
/// # Variants
///
/// - `Transparent`: Full identity visible to all parties
/// - `SemiAnonymous`: Identity visible to selected counterparties only
/// - `FullAnonymous`: Complete identity hiding until post-trade reveal
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AnonymityLevel {
    /// Full identity visible to all parties.
    /// This is the default behavior for non-anonymous RFQs.
    #[default]
    Transparent,

    /// Identity visible only to pre-approved counterparties.
    /// Useful for established trading relationships.
    SemiAnonymous,

    /// Complete identity hiding until post-trade reveal.
    /// Market makers quote without knowing the requester.
    FullAnonymous,
}

impl AnonymityLevel {
    /// Returns true if this level provides any anonymity.
    #[inline]
    #[must_use]
    pub const fn is_anonymous(&self) -> bool {
        !matches!(self, Self::Transparent)
    }

    /// Returns true if this is full anonymity mode.
    #[inline]
    #[must_use]
    pub const fn is_full_anonymous(&self) -> bool {
        matches!(self, Self::FullAnonymous)
    }

    /// Returns true if identity should be hidden from market makers.
    #[inline]
    #[must_use]
    pub const fn hides_identity(&self) -> bool {
        matches!(self, Self::FullAnonymous | Self::SemiAnonymous)
    }
}

impl fmt::Display for AnonymityLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Transparent => write!(f, "Transparent"),
            Self::SemiAnonymous => write!(f, "SemiAnonymous"),
            Self::FullAnonymous => write!(f, "FullAnonymous"),
        }
    }
}

// ============================================================================
// AnonymousRfqView
// ============================================================================

/// Anonymized view of an RFQ for broadcasting to market makers.
///
/// This struct contains only the information that market makers need
/// to provide quotes, without revealing the requester's identity.
///
/// # Fields Exposed
///
/// - `rfq_id`: Unique identifier for the RFQ
/// - `instrument`: The instrument being traded
/// - `side`: Buy or sell direction
/// - `quantity`: Requested quantity
/// - `expires_at`: When the RFQ expires
/// - `anonymity_level`: The level of anonymity applied
///
/// # Fields Hidden
///
/// - `client_id`: The requester's identity
/// - Any other identifying metadata
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnonymousRfqView {
    /// Unique identifier for this RFQ.
    rfq_id: RfqId,
    /// The instrument being traded.
    instrument: Instrument,
    /// Buy or sell side.
    side: OrderSide,
    /// Requested quantity.
    quantity: Quantity,
    /// When this RFQ expires.
    expires_at: Timestamp,
    /// The anonymity level applied.
    anonymity_level: AnonymityLevel,
    /// When this view was created.
    created_at: Timestamp,
}

impl AnonymousRfqView {
    /// Creates a new anonymous RFQ view.
    #[must_use]
    pub fn new(
        rfq_id: RfqId,
        instrument: Instrument,
        side: OrderSide,
        quantity: Quantity,
        expires_at: Timestamp,
        anonymity_level: AnonymityLevel,
    ) -> Self {
        Self {
            rfq_id,
            instrument,
            side,
            quantity,
            expires_at,
            anonymity_level,
            created_at: Timestamp::now(),
        }
    }

    /// Returns the RFQ ID.
    #[inline]
    #[must_use]
    pub fn rfq_id(&self) -> RfqId {
        self.rfq_id
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

    /// Returns when this RFQ expires.
    #[inline]
    #[must_use]
    pub fn expires_at(&self) -> Timestamp {
        self.expires_at
    }

    /// Returns the anonymity level.
    #[inline]
    #[must_use]
    pub fn anonymity_level(&self) -> AnonymityLevel {
        self.anonymity_level
    }

    /// Returns when this view was created.
    #[inline]
    #[must_use]
    pub fn created_at(&self) -> Timestamp {
        self.created_at
    }

    /// Returns true if this RFQ is anonymous.
    #[inline]
    #[must_use]
    pub fn is_anonymous(&self) -> bool {
        self.anonymity_level.is_anonymous()
    }

    /// Returns true if this RFQ has expired.
    #[inline]
    #[must_use]
    pub fn is_expired(&self) -> bool {
        self.expires_at.is_expired()
    }
}

impl fmt::Display for AnonymousRfqView {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "AnonymousRfqView({}, {}, {:?}, {}, {})",
            self.rfq_id,
            self.instrument.symbol(),
            self.side,
            self.quantity,
            self.anonymity_level
        )
    }
}

// ============================================================================
// IdentityMapping
// ============================================================================

/// Mapping of RFQ to real identities for compliance and settlement.
///
/// This entity maintains the link between an anonymous RFQ and the
/// actual requester identity. It is used for:
///
/// - **Compliance**: Audit trail of all anonymous RFQs
/// - **Settlement**: Revealing identity post-trade for on-chain settlement
/// - **Dispute resolution**: Identifying parties if issues arise
///
/// # Invariants
///
/// - Once created, the mapping is immutable (append-only reveals)
/// - Only authorized parties can access the mapping
/// - All reveals are logged for audit purposes
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentityMapping {
    /// The RFQ this mapping belongs to.
    rfq_id: RfqId,
    /// The actual requester identity.
    requester_id: CounterpartyId,
    /// The anonymity level used.
    anonymity_level: AnonymityLevel,
    /// When this mapping was created.
    created_at: Timestamp,
    /// When identity was revealed (if revealed).
    revealed_at: Option<Timestamp>,
    /// Parties to whom identity has been revealed.
    revealed_to: Vec<CounterpartyId>,
}

impl IdentityMapping {
    /// Creates a new identity mapping.
    #[must_use]
    pub fn new(
        rfq_id: RfqId,
        requester_id: CounterpartyId,
        anonymity_level: AnonymityLevel,
    ) -> Self {
        Self {
            rfq_id,
            requester_id,
            anonymity_level,
            created_at: Timestamp::now(),
            revealed_at: None,
            revealed_to: Vec::new(),
        }
    }

    /// Returns the RFQ ID.
    #[inline]
    #[must_use]
    pub fn rfq_id(&self) -> RfqId {
        self.rfq_id
    }

    /// Returns the requester ID.
    ///
    /// # Security
    ///
    /// This method provides direct access to the requester identity.
    /// Callers must ensure they have authorization to access this data.
    #[inline]
    #[must_use]
    pub fn requester_id(&self) -> &CounterpartyId {
        &self.requester_id
    }

    /// Returns the anonymity level.
    #[inline]
    #[must_use]
    pub fn anonymity_level(&self) -> AnonymityLevel {
        self.anonymity_level
    }

    /// Returns when this mapping was created.
    #[inline]
    #[must_use]
    pub fn created_at(&self) -> Timestamp {
        self.created_at
    }

    /// Returns when identity was revealed, if at all.
    #[inline]
    #[must_use]
    pub fn revealed_at(&self) -> Option<Timestamp> {
        self.revealed_at
    }

    /// Returns the list of parties to whom identity has been revealed.
    #[inline]
    #[must_use]
    pub fn revealed_to(&self) -> &[CounterpartyId] {
        &self.revealed_to
    }

    /// Returns true if identity has been revealed to anyone.
    #[inline]
    #[must_use]
    pub fn is_revealed(&self) -> bool {
        self.revealed_at.is_some()
    }

    /// Returns true if identity has been revealed to a specific party.
    #[must_use]
    pub fn is_revealed_to(&self, party: &CounterpartyId) -> bool {
        self.revealed_to.iter().any(|p| p == party)
    }

    /// Records that identity was revealed to a party.
    ///
    /// This is an append-only operation for audit purposes.
    /// If this is the first reveal, `revealed_at` is set.
    pub fn reveal_to(&mut self, party: CounterpartyId) {
        if self.revealed_at.is_none() {
            self.revealed_at = Some(Timestamp::now());
        }
        if !self.is_revealed_to(&party) {
            self.revealed_to.push(party);
        }
    }

    /// Creates an identity mapping from stored parts (for repository reconstruction).
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn from_parts(
        rfq_id: RfqId,
        requester_id: CounterpartyId,
        anonymity_level: AnonymityLevel,
        created_at: Timestamp,
        revealed_at: Option<Timestamp>,
        revealed_to: Vec<CounterpartyId>,
    ) -> Self {
        Self {
            rfq_id,
            requester_id,
            anonymity_level,
            created_at,
            revealed_at,
            revealed_to,
        }
    }
}

impl fmt::Display for IdentityMapping {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_revealed() {
            let count = self.revealed_to.len();
            let noun = if count == 1 { "party" } else { "parties" };
            write!(
                f,
                "IdentityMapping({}, revealed to {} {})",
                self.rfq_id, count, noun
            )
        } else {
            write!(f, "IdentityMapping({}, not revealed)", self.rfq_id)
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::useless_vec
)]
mod tests {
    use super::*;
    use crate::domain::value_objects::Symbol;
    use crate::domain::value_objects::enums::AssetClass;

    fn create_test_instrument() -> Instrument {
        let symbol = Symbol::new("BTC/USD").unwrap();
        Instrument::builder(symbol, AssetClass::CryptoSpot).build()
    }

    // AnonymityLevel tests

    #[test]
    fn anonymity_level_default_is_transparent() {
        let level = AnonymityLevel::default();
        assert_eq!(level, AnonymityLevel::Transparent);
        assert!(!level.is_anonymous());
    }

    #[test]
    fn anonymity_level_transparent_does_not_hide() {
        let level = AnonymityLevel::Transparent;
        assert!(!level.is_anonymous());
        assert!(!level.is_full_anonymous());
        assert!(!level.hides_identity());
    }

    #[test]
    fn anonymity_level_semi_anonymous_hides() {
        let level = AnonymityLevel::SemiAnonymous;
        assert!(level.is_anonymous());
        assert!(!level.is_full_anonymous());
        assert!(level.hides_identity());
    }

    #[test]
    fn anonymity_level_full_anonymous_hides() {
        let level = AnonymityLevel::FullAnonymous;
        assert!(level.is_anonymous());
        assert!(level.is_full_anonymous());
        assert!(level.hides_identity());
    }

    #[test]
    fn anonymity_level_display() {
        assert_eq!(AnonymityLevel::Transparent.to_string(), "Transparent");
        assert_eq!(AnonymityLevel::SemiAnonymous.to_string(), "SemiAnonymous");
        assert_eq!(AnonymityLevel::FullAnonymous.to_string(), "FullAnonymous");
    }

    // AnonymousRfqView tests

    #[test]
    fn anonymous_rfq_view_creation() {
        let rfq_id = RfqId::new_v4();
        let instrument = create_test_instrument();
        let quantity = Quantity::new(10.0).unwrap();
        let expires_at = Timestamp::now().add_secs(300);

        let view = AnonymousRfqView::new(
            rfq_id,
            instrument.clone(),
            OrderSide::Buy,
            quantity,
            expires_at,
            AnonymityLevel::FullAnonymous,
        );

        assert_eq!(view.rfq_id(), rfq_id);
        assert_eq!(view.instrument(), &instrument);
        assert_eq!(view.side(), OrderSide::Buy);
        assert_eq!(view.quantity(), quantity);
        assert_eq!(view.expires_at(), expires_at);
        assert_eq!(view.anonymity_level(), AnonymityLevel::FullAnonymous);
        assert!(view.is_anonymous());
        assert!(!view.is_expired());
    }

    #[test]
    fn anonymous_rfq_view_transparent_not_anonymous() {
        let view = AnonymousRfqView::new(
            RfqId::new_v4(),
            create_test_instrument(),
            OrderSide::Sell,
            Quantity::new(5.0).unwrap(),
            Timestamp::now().add_secs(300),
            AnonymityLevel::Transparent,
        );

        assert!(!view.is_anonymous());
    }

    #[test]
    fn anonymous_rfq_view_display() {
        let view = AnonymousRfqView::new(
            RfqId::new_v4(),
            create_test_instrument(),
            OrderSide::Buy,
            Quantity::new(10.0).unwrap(),
            Timestamp::now().add_secs(300),
            AnonymityLevel::FullAnonymous,
        );

        let display = view.to_string();
        assert!(display.contains("AnonymousRfqView"));
        assert!(display.contains("BTC/USD"));
        assert!(display.contains("FullAnonymous"));
    }

    // IdentityMapping tests

    #[test]
    fn identity_mapping_creation() {
        let rfq_id = RfqId::new_v4();
        let requester_id = CounterpartyId::new("client-1");

        let mapping =
            IdentityMapping::new(rfq_id, requester_id.clone(), AnonymityLevel::FullAnonymous);

        assert_eq!(mapping.rfq_id(), rfq_id);
        assert_eq!(mapping.requester_id(), &requester_id);
        assert_eq!(mapping.anonymity_level(), AnonymityLevel::FullAnonymous);
        assert!(!mapping.is_revealed());
        assert!(mapping.revealed_to().is_empty());
    }

    #[test]
    fn identity_mapping_reveal_to_party() {
        let rfq_id = RfqId::new_v4();
        let requester_id = CounterpartyId::new("client-1");
        let mm_id = CounterpartyId::new("mm-1");

        let mut mapping = IdentityMapping::new(rfq_id, requester_id, AnonymityLevel::FullAnonymous);

        assert!(!mapping.is_revealed());
        assert!(!mapping.is_revealed_to(&mm_id));

        mapping.reveal_to(mm_id.clone());

        assert!(mapping.is_revealed());
        assert!(mapping.revealed_at().is_some());
        assert!(mapping.is_revealed_to(&mm_id));
        assert_eq!(mapping.revealed_to().len(), 1);
    }

    #[test]
    fn identity_mapping_reveal_to_multiple_parties() {
        let rfq_id = RfqId::new_v4();
        let requester_id = CounterpartyId::new("client-1");
        let mm1 = CounterpartyId::new("mm-1");
        let mm2 = CounterpartyId::new("mm-2");

        let mut mapping = IdentityMapping::new(rfq_id, requester_id, AnonymityLevel::FullAnonymous);

        mapping.reveal_to(mm1.clone());
        let first_reveal_time = mapping.revealed_at();

        mapping.reveal_to(mm2.clone());

        // revealed_at should not change on subsequent reveals
        assert_eq!(mapping.revealed_at(), first_reveal_time);
        assert!(mapping.is_revealed_to(&mm1));
        assert!(mapping.is_revealed_to(&mm2));
        assert_eq!(mapping.revealed_to().len(), 2);
    }

    #[test]
    fn identity_mapping_reveal_same_party_twice_is_idempotent() {
        let rfq_id = RfqId::new_v4();
        let requester_id = CounterpartyId::new("client-1");
        let mm_id = CounterpartyId::new("mm-1");

        let mut mapping = IdentityMapping::new(rfq_id, requester_id, AnonymityLevel::FullAnonymous);

        mapping.reveal_to(mm_id.clone());
        mapping.reveal_to(mm_id.clone());

        assert_eq!(mapping.revealed_to().len(), 1);
    }

    #[test]
    fn identity_mapping_from_parts() {
        let rfq_id = RfqId::new_v4();
        let requester_id = CounterpartyId::new("client-1");
        let mm_id = CounterpartyId::new("mm-1");
        let created_at = Timestamp::now();
        let revealed_at = Some(Timestamp::now());

        let mapping = IdentityMapping::from_parts(
            rfq_id,
            requester_id.clone(),
            AnonymityLevel::FullAnonymous,
            created_at,
            revealed_at,
            vec![mm_id.clone()],
        );

        assert_eq!(mapping.rfq_id(), rfq_id);
        assert_eq!(mapping.requester_id(), &requester_id);
        assert!(mapping.is_revealed());
        assert!(mapping.is_revealed_to(&mm_id));
    }

    #[test]
    fn identity_mapping_display() {
        let mut mapping = IdentityMapping::new(
            RfqId::new_v4(),
            CounterpartyId::new("client-1"),
            AnonymityLevel::FullAnonymous,
        );

        let display_before = mapping.to_string();
        assert!(display_before.contains("not revealed"));

        mapping.reveal_to(CounterpartyId::new("mm-1"));

        let display_after = mapping.to_string();
        assert!(display_after.contains("revealed to 1 party"));
    }
}
