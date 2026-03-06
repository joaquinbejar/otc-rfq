//! # Quote Entity
//!
//! Represents a price quote from a venue.
//!
//! This module provides the [`Quote`] entity representing a price quote
//! received from a liquidity venue in response to an RFQ.
//!
//! # Examples
//!
//! ```
//! use otc_rfq::domain::entities::quote::{Quote, QuoteBuilder};
//! use otc_rfq::domain::value_objects::{QuoteId, RfqId, VenueId, Price, Quantity, Timestamp};
//!
//! let quote = QuoteBuilder::new(
//!     RfqId::new_v4(),
//!     VenueId::new("venue-1"),
//!     Price::new(50000.0).unwrap(),
//!     Quantity::new(1.0).unwrap(),
//!     Timestamp::now().add_secs(300),
//! ).build();
//!
//! assert!(!quote.is_expired());
//! ```

use crate::domain::errors::{DomainError, DomainResult};
use crate::domain::value_objects::timestamp::Timestamp;
use crate::domain::value_objects::{Price, Quantity, QuoteId, RfqId, VenueId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

/// Metadata associated with a quote.
///
/// Contains venue-specific data that may vary between venues.
///
/// # Examples
///
/// ```
/// use otc_rfq::domain::entities::quote::QuoteMetadata;
///
/// let mut metadata = QuoteMetadata::new();
/// metadata.set("execution_type", "FILL_OR_KILL");
/// assert_eq!(metadata.get("execution_type"), Some(&"FILL_OR_KILL".to_string()));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct QuoteMetadata {
    /// Key-value pairs for venue-specific data.
    data: HashMap<String, String>,
}

impl QuoteMetadata {
    /// Creates empty metadata.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates metadata from a HashMap.
    #[must_use]
    pub fn from_map(data: HashMap<String, String>) -> Self {
        Self { data }
    }

    /// Sets a metadata value.
    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.data.insert(key.into(), value.into());
    }

    /// Gets a metadata value.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&String> {
        self.data.get(key)
    }

    /// Returns true if the metadata is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Returns the number of metadata entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Returns an iterator over the metadata entries.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &String)> {
        self.data.iter()
    }
}

/// A price quote from a liquidity venue.
///
/// Represents a quote received in response to an RFQ, including
/// the price, quantity, validity period, and optional commission.
///
/// # Invariants
///
/// - Price must be positive
/// - Quantity must be positive
/// - `valid_until` must be in the future when created
///
/// # Examples
///
/// ```
/// use otc_rfq::domain::entities::quote::{Quote, QuoteBuilder};
/// use otc_rfq::domain::value_objects::{RfqId, VenueId, Price, Quantity, Timestamp};
///
/// let quote = QuoteBuilder::new(
///     RfqId::new_v4(),
///     VenueId::new("binance"),
///     Price::new(50000.0).unwrap(),
///     Quantity::new(1.5).unwrap(),
///     Timestamp::now().add_secs(60),
/// )
/// .commission(Price::new(10.0).unwrap())
/// .build();
///
/// assert!(quote.commission().is_some());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Quote {
    /// Unique identifier for this quote.
    id: QuoteId,
    /// The RFQ this quote is responding to.
    rfq_id: RfqId,
    /// The venue that provided this quote.
    venue_id: VenueId,
    /// The quoted price.
    price: Price,
    /// The quoted quantity.
    quantity: Quantity,
    /// Optional commission/fee.
    commission: Option<Price>,
    /// When this quote expires.
    valid_until: Timestamp,
    /// Venue-specific metadata.
    metadata: Option<QuoteMetadata>,
    /// When this quote was created.
    created_at: Timestamp,
    /// Whether this quote requires last-look confirmation from the MM.
    last_look_required: bool,
}

impl Quote {
    /// Creates a new quote with validation.
    ///
    /// # Arguments
    ///
    /// * `rfq_id` - The RFQ this quote responds to
    /// * `venue_id` - The venue providing the quote
    /// * `price` - The quoted price (must be positive)
    /// * `quantity` - The quoted quantity (must be positive)
    /// * `valid_until` - When the quote expires (must be in the future)
    ///
    /// # Errors
    ///
    /// Returns `DomainError::InvalidPrice` if price is not positive.
    /// Returns `DomainError::InvalidQuantity` if quantity is not positive.
    /// Returns `DomainError::QuoteExpired` if valid_until is in the past.
    ///
    /// # Examples
    ///
    /// ```
    /// use otc_rfq::domain::entities::quote::Quote;
    /// use otc_rfq::domain::value_objects::{RfqId, VenueId, Price, Quantity, Timestamp};
    ///
    /// let quote = Quote::new(
    ///     RfqId::new_v4(),
    ///     VenueId::new("venue"),
    ///     Price::new(100.0).unwrap(),
    ///     Quantity::new(10.0).unwrap(),
    ///     Timestamp::now().add_secs(300),
    /// ).unwrap();
    /// ```
    pub fn new(
        rfq_id: RfqId,
        venue_id: VenueId,
        price: Price,
        quantity: Quantity,
        valid_until: Timestamp,
    ) -> DomainResult<Self> {
        Self::validate_price(&price)?;
        Self::validate_quantity(&quantity)?;
        Self::validate_expiry(&valid_until)?;

        Ok(Self {
            id: QuoteId::new_v4(),
            rfq_id,
            venue_id,
            price,
            quantity,
            commission: None,
            valid_until,
            metadata: None,
            created_at: Timestamp::now(),
            last_look_required: false,
        })
    }

    /// Creates a quote with a specific ID (for reconstruction from storage).
    ///
    /// # Safety
    ///
    /// This method bypasses validation and should only be used when
    /// reconstructing from trusted storage.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn from_parts(
        id: QuoteId,
        rfq_id: RfqId,
        venue_id: VenueId,
        price: Price,
        quantity: Quantity,
        commission: Option<Price>,
        valid_until: Timestamp,
        metadata: Option<QuoteMetadata>,
        created_at: Timestamp,
        last_look_required: bool,
    ) -> Self {
        Self {
            id,
            rfq_id,
            venue_id,
            price,
            quantity,
            commission,
            valid_until,
            metadata,
            created_at,
            last_look_required,
        }
    }

    /// Returns a builder for constructing a quote.
    #[must_use]
    pub fn builder(
        rfq_id: RfqId,
        venue_id: VenueId,
        price: Price,
        quantity: Quantity,
        valid_until: Timestamp,
    ) -> QuoteBuilder {
        QuoteBuilder::new(rfq_id, venue_id, price, quantity, valid_until)
    }

    fn validate_price(price: &Price) -> DomainResult<()> {
        if !price.is_positive() {
            return Err(DomainError::InvalidPrice(
                "price must be positive".to_string(),
            ));
        }
        Ok(())
    }

    fn validate_quantity(quantity: &Quantity) -> DomainResult<()> {
        if !quantity.is_positive() {
            return Err(DomainError::InvalidQuantity(
                "quantity must be positive".to_string(),
            ));
        }
        Ok(())
    }

    fn validate_expiry(valid_until: &Timestamp) -> DomainResult<()> {
        if valid_until.is_expired() {
            return Err(DomainError::QuoteExpired(
                "valid_until must be in the future".to_string(),
            ));
        }
        Ok(())
    }

    /// Returns the quote ID.
    #[inline]
    #[must_use]
    pub fn id(&self) -> QuoteId {
        self.id
    }

    /// Returns the RFQ ID this quote responds to.
    #[inline]
    #[must_use]
    pub fn rfq_id(&self) -> RfqId {
        self.rfq_id
    }

    /// Returns the venue ID.
    #[inline]
    #[must_use]
    pub fn venue_id(&self) -> &VenueId {
        &self.venue_id
    }

    /// Returns the quoted price.
    #[inline]
    #[must_use]
    pub fn price(&self) -> Price {
        self.price
    }

    /// Returns the quoted quantity.
    #[inline]
    #[must_use]
    pub fn quantity(&self) -> Quantity {
        self.quantity
    }

    /// Returns the commission, if any.
    #[inline]
    #[must_use]
    pub fn commission(&self) -> Option<Price> {
        self.commission
    }

    /// Returns when this quote expires.
    #[inline]
    #[must_use]
    pub fn valid_until(&self) -> Timestamp {
        self.valid_until
    }

    /// Returns the metadata, if any.
    #[inline]
    #[must_use]
    pub fn metadata(&self) -> Option<&QuoteMetadata> {
        self.metadata.as_ref()
    }

    /// Returns when this quote was created.
    #[inline]
    #[must_use]
    pub fn created_at(&self) -> Timestamp {
        self.created_at
    }

    /// Returns whether this quote requires last-look confirmation from the MM.
    #[inline]
    #[must_use]
    pub fn last_look_required(&self) -> bool {
        self.last_look_required
    }

    /// Sets whether this quote requires last-look confirmation.
    #[must_use]
    pub fn with_last_look_required(mut self, required: bool) -> Self {
        self.last_look_required = required;
        self
    }

    /// Returns true if this quote has expired.
    ///
    /// # Examples
    ///
    /// ```
    /// use otc_rfq::domain::entities::quote::QuoteBuilder;
    /// use otc_rfq::domain::value_objects::{RfqId, VenueId, Price, Quantity, Timestamp};
    ///
    /// let quote = QuoteBuilder::new(
    ///     RfqId::new_v4(),
    ///     VenueId::new("venue"),
    ///     Price::new(100.0).unwrap(),
    ///     Quantity::new(10.0).unwrap(),
    ///     Timestamp::now().add_secs(300),
    /// ).build();
    ///
    /// assert!(!quote.is_expired());
    /// ```
    #[must_use]
    pub fn is_expired(&self) -> bool {
        self.valid_until.is_expired()
    }

    /// Returns the time remaining until expiry.
    ///
    /// Returns `Duration::ZERO` if already expired.
    #[must_use]
    pub fn time_to_expiry(&self) -> std::time::Duration {
        Timestamp::now().duration_until(&self.valid_until)
    }

    /// Calculates the total cost including commission.
    ///
    /// Returns `price * quantity + commission`.
    #[must_use]
    pub fn total_cost(&self) -> Option<Price> {
        let base_cost = self.price.safe_mul(self.quantity.get()).ok()?;
        match self.commission {
            Some(comm) => base_cost.safe_add(comm).ok(),
            None => Some(base_cost),
        }
    }
}

impl fmt::Display for Quote {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Quote({} @ {} from {})",
            self.quantity, self.price, self.venue_id
        )
    }
}

/// Builder for constructing [`Quote`] instances.
///
/// Provides a fluent API for setting optional fields.
///
/// # Examples
///
/// ```
/// use otc_rfq::domain::entities::quote::QuoteBuilder;
/// use otc_rfq::domain::value_objects::{RfqId, VenueId, Price, Quantity, Timestamp};
///
/// let quote = QuoteBuilder::new(
///     RfqId::new_v4(),
///     VenueId::new("venue"),
///     Price::new(100.0).unwrap(),
///     Quantity::new(10.0).unwrap(),
///     Timestamp::now().add_secs(300),
/// )
/// .commission(Price::new(5.0).unwrap())
/// .build();
/// ```
#[derive(Debug, Clone)]
pub struct QuoteBuilder {
    rfq_id: RfqId,
    venue_id: VenueId,
    price: Price,
    quantity: Quantity,
    valid_until: Timestamp,
    commission: Option<Price>,
    metadata: Option<QuoteMetadata>,
}

impl QuoteBuilder {
    /// Creates a new builder with required fields.
    #[must_use]
    pub fn new(
        rfq_id: RfqId,
        venue_id: VenueId,
        price: Price,
        quantity: Quantity,
        valid_until: Timestamp,
    ) -> Self {
        Self {
            rfq_id,
            venue_id,
            price,
            quantity,
            valid_until,
            commission: None,
            metadata: None,
        }
    }

    /// Sets the commission.
    #[must_use]
    pub fn commission(mut self, commission: Price) -> Self {
        self.commission = Some(commission);
        self
    }

    /// Sets the metadata.
    #[must_use]
    pub fn metadata(mut self, metadata: QuoteMetadata) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Builds the quote without validation.
    ///
    /// Use [`try_build`](Self::try_build) for validated construction.
    #[must_use]
    pub fn build(self) -> Quote {
        Quote {
            id: QuoteId::new_v4(),
            rfq_id: self.rfq_id,
            venue_id: self.venue_id,
            price: self.price,
            quantity: self.quantity,
            commission: self.commission,
            valid_until: self.valid_until,
            metadata: self.metadata,
            created_at: Timestamp::now(),
            last_look_required: false,
        }
    }

    /// Builds the quote with validation.
    ///
    /// # Errors
    ///
    /// Returns `DomainError` if validation fails.
    pub fn try_build(self) -> DomainResult<Quote> {
        Quote::validate_price(&self.price)?;
        Quote::validate_quantity(&self.quantity)?;
        Quote::validate_expiry(&self.valid_until)?;

        Ok(Quote {
            id: QuoteId::new_v4(),
            rfq_id: self.rfq_id,
            venue_id: self.venue_id,
            price: self.price,
            quantity: self.quantity,
            commission: self.commission,
            valid_until: self.valid_until,
            metadata: self.metadata,
            created_at: Timestamp::now(),
            last_look_required: false,
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn valid_price() -> Price {
        Price::new(100.0).unwrap()
    }

    fn valid_quantity() -> Quantity {
        Quantity::new(10.0).unwrap()
    }

    fn future_timestamp() -> Timestamp {
        Timestamp::now().add_secs(300)
    }

    fn past_timestamp() -> Timestamp {
        Timestamp::now().sub_secs(300)
    }

    mod construction {
        use super::*;

        #[test]
        fn new_creates_valid_quote() {
            let rfq_id = RfqId::new_v4();
            let venue_id = VenueId::new("venue-1");

            let quote = Quote::new(
                rfq_id,
                venue_id.clone(),
                valid_price(),
                valid_quantity(),
                future_timestamp(),
            )
            .unwrap();

            assert_eq!(quote.rfq_id(), rfq_id);
            assert_eq!(quote.venue_id(), &venue_id);
            assert_eq!(quote.price(), valid_price());
            assert_eq!(quote.quantity(), valid_quantity());
            assert!(quote.commission().is_none());
            assert!(quote.metadata().is_none());
            assert!(!quote.is_expired());
        }

        #[test]
        fn new_fails_with_zero_price() {
            let result = Quote::new(
                RfqId::new_v4(),
                VenueId::new("venue"),
                Price::zero(),
                valid_quantity(),
                future_timestamp(),
            );

            assert!(matches!(result, Err(DomainError::InvalidPrice(_))));
        }

        #[test]
        fn new_fails_with_zero_quantity() {
            let result = Quote::new(
                RfqId::new_v4(),
                VenueId::new("venue"),
                valid_price(),
                Quantity::zero(),
                future_timestamp(),
            );

            assert!(matches!(result, Err(DomainError::InvalidQuantity(_))));
        }

        #[test]
        fn new_fails_with_expired_valid_until() {
            let result = Quote::new(
                RfqId::new_v4(),
                VenueId::new("venue"),
                valid_price(),
                valid_quantity(),
                past_timestamp(),
            );

            assert!(matches!(result, Err(DomainError::QuoteExpired(_))));
        }

        #[test]
        fn builder_creates_quote() {
            let quote = QuoteBuilder::new(
                RfqId::new_v4(),
                VenueId::new("venue"),
                valid_price(),
                valid_quantity(),
                future_timestamp(),
            )
            .build();

            assert!(!quote.is_expired());
        }

        #[test]
        fn builder_with_commission() {
            let commission = Price::new(5.0).unwrap();
            let quote = QuoteBuilder::new(
                RfqId::new_v4(),
                VenueId::new("venue"),
                valid_price(),
                valid_quantity(),
                future_timestamp(),
            )
            .commission(commission)
            .build();

            assert_eq!(quote.commission(), Some(commission));
        }

        #[test]
        fn builder_with_metadata() {
            let mut metadata = QuoteMetadata::new();
            metadata.set("key", "value");

            let quote = QuoteBuilder::new(
                RfqId::new_v4(),
                VenueId::new("venue"),
                valid_price(),
                valid_quantity(),
                future_timestamp(),
            )
            .metadata(metadata.clone())
            .build();

            assert_eq!(quote.metadata(), Some(&metadata));
        }

        #[test]
        fn try_build_validates() {
            let result = QuoteBuilder::new(
                RfqId::new_v4(),
                VenueId::new("venue"),
                Price::zero(),
                valid_quantity(),
                future_timestamp(),
            )
            .try_build();

            assert!(matches!(result, Err(DomainError::InvalidPrice(_))));
        }
    }

    mod expiry {
        use super::*;

        #[test]
        fn is_expired_returns_false_for_future() {
            let quote = QuoteBuilder::new(
                RfqId::new_v4(),
                VenueId::new("venue"),
                valid_price(),
                valid_quantity(),
                future_timestamp(),
            )
            .build();

            assert!(!quote.is_expired());
        }

        #[test]
        fn time_to_expiry_positive_for_future() {
            let quote = QuoteBuilder::new(
                RfqId::new_v4(),
                VenueId::new("venue"),
                valid_price(),
                valid_quantity(),
                Timestamp::now().add_secs(60),
            )
            .build();

            let ttl = quote.time_to_expiry();
            assert!(ttl.as_secs() > 0);
        }
    }

    mod total_cost {
        use super::*;

        #[test]
        fn total_cost_without_commission() {
            let quote = QuoteBuilder::new(
                RfqId::new_v4(),
                VenueId::new("venue"),
                Price::new(100.0).unwrap(),
                Quantity::new(2.0).unwrap(),
                future_timestamp(),
            )
            .build();

            let total = quote.total_cost().unwrap();
            assert_eq!(total, Price::new(200.0).unwrap());
        }

        #[test]
        fn total_cost_with_commission() {
            let quote = QuoteBuilder::new(
                RfqId::new_v4(),
                VenueId::new("venue"),
                Price::new(100.0).unwrap(),
                Quantity::new(2.0).unwrap(),
                future_timestamp(),
            )
            .commission(Price::new(10.0).unwrap())
            .build();

            let total = quote.total_cost().unwrap();
            assert_eq!(total, Price::new(210.0).unwrap());
        }
    }

    mod metadata {
        use super::*;

        #[test]
        fn metadata_set_and_get() {
            let mut metadata = QuoteMetadata::new();
            metadata.set("execution_type", "FOK");
            metadata.set("venue_quote_id", "12345");

            assert_eq!(metadata.get("execution_type"), Some(&"FOK".to_string()));
            assert_eq!(metadata.get("venue_quote_id"), Some(&"12345".to_string()));
            assert_eq!(metadata.get("nonexistent"), None);
        }

        #[test]
        fn metadata_is_empty() {
            let metadata = QuoteMetadata::new();
            assert!(metadata.is_empty());
            assert_eq!(metadata.len(), 0);
        }

        #[test]
        fn metadata_from_map() {
            let mut map = HashMap::new();
            map.insert("key".to_string(), "value".to_string());

            let metadata = QuoteMetadata::from_map(map);
            assert_eq!(metadata.get("key"), Some(&"value".to_string()));
        }
    }

    mod display {
        use super::*;

        #[test]
        fn display_format() {
            let quote = QuoteBuilder::new(
                RfqId::new_v4(),
                VenueId::new("binance"),
                Price::new(50000.0).unwrap(),
                Quantity::new(1.5).unwrap(),
                future_timestamp(),
            )
            .build();

            let display = quote.to_string();
            assert!(display.contains("1.5"));
            assert!(display.contains("50000"));
            assert!(display.contains("binance"));
        }
    }

    mod serde {
        use super::*;

        #[test]
        fn serde_roundtrip() {
            let quote = QuoteBuilder::new(
                RfqId::new_v4(),
                VenueId::new("venue"),
                valid_price(),
                valid_quantity(),
                future_timestamp(),
            )
            .commission(Price::new(5.0).unwrap())
            .build();

            let json = serde_json::to_string(&quote).unwrap();
            let deserialized: Quote = serde_json::from_str(&json).unwrap();

            assert_eq!(quote.id(), deserialized.id());
            assert_eq!(quote.rfq_id(), deserialized.rfq_id());
            assert_eq!(quote.price(), deserialized.price());
            assert_eq!(quote.commission(), deserialized.commission());
        }

        #[test]
        fn metadata_serde_roundtrip() {
            let mut metadata = QuoteMetadata::new();
            metadata.set("key", "value");

            let json = serde_json::to_string(&metadata).unwrap();
            let deserialized: QuoteMetadata = serde_json::from_str(&json).unwrap();

            assert_eq!(metadata, deserialized);
        }
    }
}
