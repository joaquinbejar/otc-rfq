//! # Identity Value Objects
//!
//! Type-safe identity wrappers for domain identifiers.
//!
//! This module provides newtype wrappers for all domain identifiers,
//! ensuring type safety and preventing accidental mixing of different ID types.
//!
//! ## UUID-based Identifiers
//!
//! - [`RfqId`] - Request-for-Quote identifier
//! - [`QuoteId`] - Quote identifier
//! - [`TradeId`] - Trade identifier
//! - [`EventId`] - Domain event identifier
//!
//! ## String-based Identifiers
//!
//! - [`VenueId`] - Venue identifier
//! - [`CounterpartyId`] - Counterparty identifier

use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// Request-for-Quote identifier.
///
/// A UUID-based identifier uniquely identifying an RFQ within the system.
///
/// # Examples
///
/// ```
/// use otc_rfq::domain::value_objects::ids::RfqId;
///
/// // Generate a new random RFQ ID
/// let rfq_id = RfqId::new_v4();
///
/// // Display as hyphenated UUID
/// println!("RFQ: {}", rfq_id);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RfqId(Uuid);

impl RfqId {
    /// Creates a new RFQ ID from an existing UUID.
    #[inline]
    #[must_use]
    pub const fn new(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Generates a new random RFQ ID using UUID v4.
    #[must_use]
    pub fn new_v4() -> Self {
        Self(Uuid::new_v4())
    }

    /// Returns the inner UUID value.
    #[inline]
    #[must_use]
    pub const fn get(self) -> Uuid {
        self.0
    }

    /// Creates an RFQ ID from a UUID reference.
    #[inline]
    #[must_use]
    pub const fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl fmt::Display for RfqId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.hyphenated())
    }
}

impl From<Uuid> for RfqId {
    #[inline]
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

/// Quote identifier.
///
/// A UUID-based identifier uniquely identifying a quote from a venue.
///
/// # Examples
///
/// ```
/// use otc_rfq::domain::value_objects::ids::QuoteId;
///
/// let quote_id = QuoteId::new_v4();
/// println!("Quote: {}", quote_id);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct QuoteId(Uuid);

impl QuoteId {
    /// Creates a new Quote ID from an existing UUID.
    #[inline]
    #[must_use]
    pub const fn new(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Generates a new random Quote ID using UUID v4.
    #[must_use]
    pub fn new_v4() -> Self {
        Self(Uuid::new_v4())
    }

    /// Returns the inner UUID value.
    #[inline]
    #[must_use]
    pub const fn get(self) -> Uuid {
        self.0
    }

    /// Creates a Quote ID from a UUID reference.
    #[inline]
    #[must_use]
    pub const fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl fmt::Display for QuoteId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.hyphenated())
    }
}

impl From<Uuid> for QuoteId {
    #[inline]
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

/// Trade identifier.
///
/// A UUID-based identifier uniquely identifying an executed trade.
///
/// # Examples
///
/// ```
/// use otc_rfq::domain::value_objects::ids::TradeId;
///
/// let trade_id = TradeId::new_v4();
/// println!("Trade: {}", trade_id);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TradeId(Uuid);

impl TradeId {
    /// Creates a new Trade ID from an existing UUID.
    #[inline]
    #[must_use]
    pub const fn new(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Generates a new random Trade ID using UUID v4.
    #[must_use]
    pub fn new_v4() -> Self {
        Self(Uuid::new_v4())
    }

    /// Returns the inner UUID value.
    #[inline]
    #[must_use]
    pub const fn get(self) -> Uuid {
        self.0
    }

    /// Creates a Trade ID from a UUID reference.
    #[inline]
    #[must_use]
    pub const fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl fmt::Display for TradeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.hyphenated())
    }
}

impl From<Uuid> for TradeId {
    #[inline]
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

/// Domain event identifier.
///
/// A UUID-based identifier uniquely identifying a domain event for event sourcing.
///
/// # Examples
///
/// ```
/// use otc_rfq::domain::value_objects::ids::EventId;
///
/// let event_id = EventId::new_v4();
/// println!("Event: {}", event_id);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EventId(Uuid);

impl EventId {
    /// Creates a new Event ID from an existing UUID.
    #[inline]
    #[must_use]
    pub const fn new(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Generates a new random Event ID using UUID v4.
    #[must_use]
    pub fn new_v4() -> Self {
        Self(Uuid::new_v4())
    }

    /// Returns the inner UUID value.
    #[inline]
    #[must_use]
    pub const fn get(self) -> Uuid {
        self.0
    }

    /// Creates an Event ID from a UUID reference.
    #[inline]
    #[must_use]
    pub const fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl fmt::Display for EventId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.hyphenated())
    }
}

impl From<Uuid> for EventId {
    #[inline]
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

/// Venue identifier.
///
/// A string-based identifier for liquidity venues (market makers, DEX aggregators, etc.).
///
/// # Examples
///
/// ```
/// use otc_rfq::domain::value_objects::ids::VenueId;
///
/// let venue_id = VenueId::new("0x-api");
/// assert_eq!(venue_id.as_str(), "0x-api");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct VenueId(String);

impl VenueId {
    /// Creates a new Venue ID from a string.
    #[inline]
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Returns the venue ID as a string slice.
    #[inline]
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consumes the VenueId and returns the inner String.
    #[inline]
    #[must_use]
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl fmt::Display for VenueId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for VenueId {
    #[inline]
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for VenueId {
    #[inline]
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}

impl AsRef<str> for VenueId {
    #[inline]
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Counterparty identifier.
///
/// A string-based identifier for counterparties (clients, market makers, etc.).
///
/// # Examples
///
/// ```
/// use otc_rfq::domain::value_objects::ids::CounterpartyId;
///
/// let cp_id = CounterpartyId::new("DESK_001");
/// assert_eq!(cp_id.as_str(), "DESK_001");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CounterpartyId(String);

impl CounterpartyId {
    /// Creates a new Counterparty ID from a string.
    #[inline]
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Returns the counterparty ID as a string slice.
    #[inline]
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consumes the CounterpartyId and returns the inner String.
    #[inline]
    #[must_use]
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl fmt::Display for CounterpartyId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for CounterpartyId {
    #[inline]
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for CounterpartyId {
    #[inline]
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}

impl AsRef<str> for CounterpartyId {
    #[inline]
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    mod rfq_id {
        use super::*;

        #[test]
        fn new_v4_generates_unique_ids() {
            let id1 = RfqId::new_v4();
            let id2 = RfqId::new_v4();
            assert_ne!(id1, id2);
        }

        #[test]
        fn from_uuid_roundtrip() {
            let uuid = Uuid::new_v4();
            let rfq_id = RfqId::new(uuid);
            assert_eq!(rfq_id.get(), uuid);
        }

        #[test]
        fn display_formats_as_hyphenated() {
            let uuid = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
            let rfq_id = RfqId::new(uuid);
            assert_eq!(rfq_id.to_string(), "550e8400-e29b-41d4-a716-446655440000");
        }

        #[test]
        fn serde_roundtrip() {
            let rfq_id = RfqId::new_v4();
            let json = serde_json::to_string(&rfq_id).unwrap();
            let deserialized: RfqId = serde_json::from_str(&json).unwrap();
            assert_eq!(rfq_id, deserialized);
        }

        #[test]
        fn hash_equality() {
            use std::collections::HashSet;
            let uuid = Uuid::new_v4();
            let id1 = RfqId::new(uuid);
            let id2 = RfqId::new(uuid);

            let mut set = HashSet::new();
            set.insert(id1);
            assert!(set.contains(&id2));
        }
    }

    mod quote_id {
        use super::*;

        #[test]
        fn new_v4_generates_unique_ids() {
            let id1 = QuoteId::new_v4();
            let id2 = QuoteId::new_v4();
            assert_ne!(id1, id2);
        }

        #[test]
        fn from_uuid_roundtrip() {
            let uuid = Uuid::new_v4();
            let quote_id = QuoteId::new(uuid);
            assert_eq!(quote_id.get(), uuid);
        }

        #[test]
        fn serde_roundtrip() {
            let quote_id = QuoteId::new_v4();
            let json = serde_json::to_string(&quote_id).unwrap();
            let deserialized: QuoteId = serde_json::from_str(&json).unwrap();
            assert_eq!(quote_id, deserialized);
        }
    }

    mod trade_id {
        use super::*;

        #[test]
        fn new_v4_generates_unique_ids() {
            let id1 = TradeId::new_v4();
            let id2 = TradeId::new_v4();
            assert_ne!(id1, id2);
        }

        #[test]
        fn from_uuid_roundtrip() {
            let uuid = Uuid::new_v4();
            let trade_id = TradeId::new(uuid);
            assert_eq!(trade_id.get(), uuid);
        }

        #[test]
        fn serde_roundtrip() {
            let trade_id = TradeId::new_v4();
            let json = serde_json::to_string(&trade_id).unwrap();
            let deserialized: TradeId = serde_json::from_str(&json).unwrap();
            assert_eq!(trade_id, deserialized);
        }
    }

    mod event_id {
        use super::*;

        #[test]
        fn new_v4_generates_unique_ids() {
            let id1 = EventId::new_v4();
            let id2 = EventId::new_v4();
            assert_ne!(id1, id2);
        }

        #[test]
        fn from_uuid_roundtrip() {
            let uuid = Uuid::new_v4();
            let event_id = EventId::new(uuid);
            assert_eq!(event_id.get(), uuid);
        }

        #[test]
        fn serde_roundtrip() {
            let event_id = EventId::new_v4();
            let json = serde_json::to_string(&event_id).unwrap();
            let deserialized: EventId = serde_json::from_str(&json).unwrap();
            assert_eq!(event_id, deserialized);
        }
    }

    mod venue_id {
        use super::*;

        #[test]
        fn new_from_str() {
            let venue_id = VenueId::new("0x-api");
            assert_eq!(venue_id.as_str(), "0x-api");
        }

        #[test]
        fn new_from_string() {
            let venue_id = VenueId::new(String::from("1inch"));
            assert_eq!(venue_id.as_str(), "1inch");
        }

        #[test]
        fn display_formats_correctly() {
            let venue_id = VenueId::new("hashflow");
            assert_eq!(venue_id.to_string(), "hashflow");
        }

        #[test]
        fn serde_roundtrip() {
            let venue_id = VenueId::new("bebop");
            let json = serde_json::to_string(&venue_id).unwrap();
            let deserialized: VenueId = serde_json::from_str(&json).unwrap();
            assert_eq!(venue_id, deserialized);
        }

        #[test]
        fn from_str_impl() {
            let venue_id: VenueId = "uniswap".into();
            assert_eq!(venue_id.as_str(), "uniswap");
        }

        #[test]
        fn hash_equality() {
            use std::collections::HashSet;
            let id1 = VenueId::new("curve");
            let id2 = VenueId::new("curve");

            let mut set = HashSet::new();
            set.insert(id1);
            assert!(set.contains(&id2));
        }

        #[test]
        fn into_inner() {
            let venue_id = VenueId::new("paraswap");
            let inner = venue_id.into_inner();
            assert_eq!(inner, "paraswap");
        }
    }

    mod counterparty_id {
        use super::*;

        #[test]
        fn new_from_str() {
            let cp_id = CounterpartyId::new("DESK_001");
            assert_eq!(cp_id.as_str(), "DESK_001");
        }

        #[test]
        fn new_from_string() {
            let cp_id = CounterpartyId::new(String::from("MM_ALPHA"));
            assert_eq!(cp_id.as_str(), "MM_ALPHA");
        }

        #[test]
        fn display_formats_correctly() {
            let cp_id = CounterpartyId::new("CLIENT_XYZ");
            assert_eq!(cp_id.to_string(), "CLIENT_XYZ");
        }

        #[test]
        fn serde_roundtrip() {
            let cp_id = CounterpartyId::new("INTERNAL_MM");
            let json = serde_json::to_string(&cp_id).unwrap();
            let deserialized: CounterpartyId = serde_json::from_str(&json).unwrap();
            assert_eq!(cp_id, deserialized);
        }

        #[test]
        fn from_str_impl() {
            let cp_id: CounterpartyId = "HEDGE_FUND_A".into();
            assert_eq!(cp_id.as_str(), "HEDGE_FUND_A");
        }

        #[test]
        fn into_inner() {
            let cp_id = CounterpartyId::new("PROP_DESK");
            let inner = cp_id.into_inner();
            assert_eq!(inner, "PROP_DESK");
        }
    }
}
