//! # Price Discovery Domain Events
//!
//! Events related to price discovery mechanisms for illiquid instruments.

use crate::domain::events::domain_event::{DomainEvent, EventMetadata, EventType};
use crate::domain::value_objects::{
    EventId, Instrument, PriceDiscoveryMethod, RfqId, TheoreticalPrice, Timestamp,
};
use serde::{Deserialize, Serialize};

/// Event emitted when a theoretical price is computed for an illiquid instrument.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TheoreticalPriceComputed {
    /// Event metadata.
    metadata: EventMetadata,
    /// RFQ ID this price is for.
    rfq_id: RfqId,
    /// Instrument being priced.
    instrument: Instrument,
    /// Computed theoretical price.
    theoretical_price: TheoreticalPrice,
    /// Number of reference IVs used.
    reference_count: usize,
}

impl TheoreticalPriceComputed {
    /// Creates a new `TheoreticalPriceComputed` event.
    #[must_use]
    pub fn new(
        rfq_id: RfqId,
        instrument: Instrument,
        theoretical_price: TheoreticalPrice,
        reference_count: usize,
    ) -> Self {
        Self {
            metadata: EventMetadata::new(Some(rfq_id)),
            rfq_id,
            instrument,
            theoretical_price,
            reference_count,
        }
    }

    /// Returns the RFQ ID.
    #[must_use]
    pub fn rfq_id_value(&self) -> RfqId {
        self.rfq_id
    }

    /// Returns the instrument.
    #[must_use]
    pub fn instrument(&self) -> &Instrument {
        &self.instrument
    }

    /// Returns the theoretical price.
    #[must_use]
    pub fn theoretical_price(&self) -> &TheoreticalPrice {
        &self.theoretical_price
    }

    /// Returns the number of reference IVs used.
    #[must_use]
    pub fn reference_count(&self) -> usize {
        self.reference_count
    }
}

impl DomainEvent for TheoreticalPriceComputed {
    fn event_id(&self) -> EventId {
        self.metadata.event_id
    }

    fn rfq_id(&self) -> Option<RfqId> {
        self.metadata.rfq_id
    }

    fn timestamp(&self) -> Timestamp {
        self.metadata.timestamp
    }

    fn event_type(&self) -> EventType {
        EventType::Quote
    }

    fn event_name(&self) -> &'static str {
        "TheoreticalPriceComputed"
    }
}

/// Event emitted when an indicative quote is requested.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndicativeQuoteRequested {
    /// Event metadata.
    metadata: EventMetadata,
    /// RFQ ID this request is for.
    rfq_id: RfqId,
    /// Instrument for which indicative quote is requested.
    instrument: Instrument,
    /// Number of market makers contacted.
    mm_count: usize,
}

impl IndicativeQuoteRequested {
    /// Creates a new `IndicativeQuoteRequested` event.
    #[must_use]
    pub fn new(rfq_id: RfqId, instrument: Instrument, mm_count: usize) -> Self {
        Self {
            metadata: EventMetadata::new(Some(rfq_id)),
            rfq_id,
            instrument,
            mm_count,
        }
    }

    /// Returns the RFQ ID.
    #[must_use]
    pub fn rfq_id_value(&self) -> RfqId {
        self.rfq_id
    }

    /// Returns the instrument.
    #[must_use]
    pub fn instrument(&self) -> &Instrument {
        &self.instrument
    }

    /// Returns the number of market makers contacted.
    #[must_use]
    pub fn mm_count(&self) -> usize {
        self.mm_count
    }
}

impl DomainEvent for IndicativeQuoteRequested {
    fn event_id(&self) -> EventId {
        self.metadata.event_id
    }

    fn rfq_id(&self) -> Option<RfqId> {
        self.metadata.rfq_id
    }

    fn timestamp(&self) -> Timestamp {
        self.metadata.timestamp
    }

    fn event_type(&self) -> EventType {
        EventType::Quote
    }

    fn event_name(&self) -> &'static str {
        "IndicativeQuoteRequested"
    }
}

/// Event emitted when interest gathering is initiated for an illiquid instrument.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterestGathered {
    /// Event metadata.
    metadata: EventMetadata,
    /// RFQ ID this gathering is for.
    rfq_id: RfqId,
    /// Instrument for which interest is gathered.
    instrument: Instrument,
    /// Number of interested market makers.
    interested_count: usize,
    /// Total market makers contacted.
    total_contacted: usize,
}

impl InterestGathered {
    /// Creates a new `InterestGathered` event.
    #[must_use]
    pub fn new(
        rfq_id: RfqId,
        instrument: Instrument,
        interested_count: usize,
        total_contacted: usize,
    ) -> Self {
        Self {
            metadata: EventMetadata::new(Some(rfq_id)),
            rfq_id,
            instrument,
            interested_count,
            total_contacted,
        }
    }

    /// Returns the RFQ ID.
    #[must_use]
    pub fn rfq_id_value(&self) -> RfqId {
        self.rfq_id
    }

    /// Returns the instrument.
    #[must_use]
    pub fn instrument(&self) -> &Instrument {
        &self.instrument
    }

    /// Returns the number of interested market makers.
    #[must_use]
    pub fn interested_count(&self) -> usize {
        self.interested_count
    }

    /// Returns the total number of market makers contacted.
    #[must_use]
    pub fn total_contacted(&self) -> usize {
        self.total_contacted
    }

    /// Returns the interest rate (percentage of MMs interested).
    #[must_use]
    pub fn interest_rate(&self) -> f64 {
        if self.total_contacted == 0 {
            0.0
        } else {
            self.interested_count as f64 / self.total_contacted as f64
        }
    }
}

impl DomainEvent for InterestGathered {
    fn event_id(&self) -> EventId {
        self.metadata.event_id
    }

    fn rfq_id(&self) -> Option<RfqId> {
        self.metadata.rfq_id
    }

    fn timestamp(&self) -> Timestamp {
        self.metadata.timestamp
    }

    fn event_type(&self) -> EventType {
        EventType::Quote
    }

    fn event_name(&self) -> &'static str {
        "InterestGathered"
    }
}

/// Event emitted when price discovery method is selected.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PriceDiscoveryMethodSelected {
    /// Event metadata.
    metadata: EventMetadata,
    /// RFQ ID.
    rfq_id: RfqId,
    /// Instrument.
    instrument: Instrument,
    /// Selected method.
    method: PriceDiscoveryMethod,
    /// Reason for selection.
    reason: String,
}

impl PriceDiscoveryMethodSelected {
    /// Creates a new `PriceDiscoveryMethodSelected` event.
    #[must_use]
    pub fn new(
        rfq_id: RfqId,
        instrument: Instrument,
        method: PriceDiscoveryMethod,
        reason: String,
    ) -> Self {
        Self {
            metadata: EventMetadata::new(Some(rfq_id)),
            rfq_id,
            instrument,
            method,
            reason,
        }
    }

    /// Returns the RFQ ID.
    #[must_use]
    pub fn rfq_id_value(&self) -> RfqId {
        self.rfq_id
    }

    /// Returns the instrument.
    #[must_use]
    pub fn instrument(&self) -> &Instrument {
        &self.instrument
    }

    /// Returns the selected method.
    #[must_use]
    pub fn method(&self) -> PriceDiscoveryMethod {
        self.method
    }

    /// Returns the reason for selection.
    #[must_use]
    pub fn reason(&self) -> &str {
        &self.reason
    }
}

impl DomainEvent for PriceDiscoveryMethodSelected {
    fn event_id(&self) -> EventId {
        self.metadata.event_id
    }

    fn rfq_id(&self) -> Option<RfqId> {
        self.metadata.rfq_id
    }

    fn timestamp(&self) -> Timestamp {
        self.metadata.timestamp
    }

    fn event_type(&self) -> EventType {
        EventType::Quote
    }

    fn event_name(&self) -> &'static str {
        "PriceDiscoveryMethodSelected"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::value_objects::{AssetClass, Price, SettlementMethod, Symbol};
    use uuid::Uuid;

    fn create_test_instrument() -> Instrument {
        let symbol = Symbol::new("BTC/USD").unwrap();
        Instrument::new(symbol, AssetClass::CryptoDerivs, SettlementMethod::OffChain)
    }

    #[test]
    fn theoretical_price_computed_event() {
        let rfq_id = RfqId::new(Uuid::new_v4());
        let instrument = create_test_instrument();
        let price = Price::new(50000.0).unwrap();
        let theo_price = TheoreticalPrice::new(
            price,
            0.25,
            PriceDiscoveryMethod::Theoretical,
            0.8,
        );

        let event = TheoreticalPriceComputed::new(rfq_id, instrument.clone(), theo_price.clone(), 5);

        assert_eq!(event.rfq_id_value(), rfq_id);
        assert_eq!(event.instrument(), &instrument);
        assert_eq!(event.theoretical_price(), &theo_price);
        assert_eq!(event.reference_count(), 5);
        assert_eq!(event.event_name(), "TheoreticalPriceComputed");
    }

    #[test]
    fn indicative_quote_requested_event() {
        let rfq_id = RfqId::new(Uuid::new_v4());
        let instrument = create_test_instrument();

        let event = IndicativeQuoteRequested::new(rfq_id, instrument.clone(), 3);

        assert_eq!(event.rfq_id_value(), rfq_id);
        assert_eq!(event.instrument(), &instrument);
        assert_eq!(event.mm_count(), 3);
        assert_eq!(event.event_name(), "IndicativeQuoteRequested");
    }

    #[test]
    fn interest_gathered_event() {
        let rfq_id = RfqId::new(Uuid::new_v4());
        let instrument = create_test_instrument();

        let event = InterestGathered::new(rfq_id, instrument.clone(), 2, 5);

        assert_eq!(event.rfq_id_value(), rfq_id);
        assert_eq!(event.instrument(), &instrument);
        assert_eq!(event.interested_count(), 2);
        assert_eq!(event.total_contacted(), 5);
        assert_eq!(event.interest_rate(), 0.4);
        assert_eq!(event.event_name(), "InterestGathered");
    }

    #[test]
    fn interest_gathered_zero_contacted() {
        let rfq_id = RfqId::new(Uuid::new_v4());
        let instrument = create_test_instrument();

        let event = InterestGathered::new(rfq_id, instrument, 0, 0);
        assert_eq!(event.interest_rate(), 0.0);
    }

    #[test]
    fn price_discovery_method_selected_event() {
        let rfq_id = RfqId::new(Uuid::new_v4());
        let instrument = create_test_instrument();

        let event = PriceDiscoveryMethodSelected::new(
            rfq_id,
            instrument.clone(),
            PriceDiscoveryMethod::Theoretical,
            "No CLOB quotes available".to_string(),
        );

        assert_eq!(event.rfq_id_value(), rfq_id);
        assert_eq!(event.instrument(), &instrument);
        assert_eq!(event.method(), PriceDiscoveryMethod::Theoretical);
        assert_eq!(event.reason(), "No CLOB quotes available");
        assert_eq!(event.event_name(), "PriceDiscoveryMethodSelected");
    }
}
