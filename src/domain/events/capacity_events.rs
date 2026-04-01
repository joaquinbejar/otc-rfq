//! # Capacity Domain Events
//!
//! Domain events related to market maker capacity management.
//!
//! These events track capacity reservations, releases, adjustments, and
//! exclusions for audit trail and event sourcing purposes.

use crate::domain::entities::mm_capacity::{CapacityAdjustment, CapacityCheckResult};
use crate::domain::events::domain_event::{DomainEvent, EventMetadata, EventType};
use crate::domain::value_objects::CounterpartyId;
use crate::domain::value_objects::EventId;
use crate::domain::value_objects::ids::RfqId;
use crate::domain::value_objects::symbol::Symbol;
use crate::domain::value_objects::timestamp::Timestamp;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Event emitted when capacity is reserved for an RFQ.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapacityReserved {
    /// Event metadata.
    metadata: EventMetadata,
    /// Market maker identifier.
    mm_id: CounterpartyId,
    /// RFQ identifier.
    rfq_id_value: RfqId,
    /// Instrument symbol.
    instrument: Symbol,
    /// Reserved notional amount.
    notional: Decimal,
}

impl CapacityReserved {
    /// Creates a new capacity reserved event.
    #[must_use]
    pub fn new(
        mm_id: CounterpartyId,
        rfq_id: RfqId,
        instrument: Symbol,
        notional: Decimal,
    ) -> Self {
        Self {
            metadata: EventMetadata::for_rfq(rfq_id),
            mm_id,
            rfq_id_value: rfq_id,
            instrument,
            notional,
        }
    }

    /// Returns the market maker identifier.
    #[inline]
    #[must_use]
    pub fn mm_id(&self) -> &CounterpartyId {
        &self.mm_id
    }

    /// Returns the RFQ identifier.
    #[inline]
    #[must_use]
    pub fn rfq_id_value(&self) -> RfqId {
        self.rfq_id_value
    }

    /// Returns the instrument symbol.
    #[inline]
    #[must_use]
    pub fn instrument(&self) -> &Symbol {
        &self.instrument
    }

    /// Returns the reserved notional amount.
    #[inline]
    #[must_use]
    pub fn notional(&self) -> Decimal {
        self.notional
    }
}

impl DomainEvent for CapacityReserved {
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
        EventType::Capacity
    }

    fn event_name(&self) -> &'static str {
        "CapacityReserved"
    }
}

impl fmt::Display for CapacityReserved {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "CapacityReserved: MM {} reserved {} for RFQ {} on {}",
            self.mm_id, self.notional, self.rfq_id_value, self.instrument
        )
    }
}

/// Event emitted when capacity is released.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapacityReleased {
    /// Event metadata.
    metadata: EventMetadata,
    /// Market maker identifier.
    mm_id: CounterpartyId,
    /// RFQ identifier.
    rfq_id_value: RfqId,
    /// Released notional amount.
    notional: Decimal,
    /// Reason for release (e.g., "expired", "completed", "cancelled").
    reason: String,
}

impl CapacityReleased {
    /// Creates a new capacity released event.
    #[must_use]
    pub fn new(
        mm_id: CounterpartyId,
        rfq_id: RfqId,
        notional: Decimal,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            metadata: EventMetadata::for_rfq(rfq_id),
            mm_id,
            rfq_id_value: rfq_id,
            notional,
            reason: reason.into(),
        }
    }

    /// Returns the market maker identifier.
    #[inline]
    #[must_use]
    pub fn mm_id(&self) -> &CounterpartyId {
        &self.mm_id
    }

    /// Returns the RFQ identifier.
    #[inline]
    #[must_use]
    pub fn rfq_id_value(&self) -> RfqId {
        self.rfq_id_value
    }

    /// Returns the released notional amount.
    #[inline]
    #[must_use]
    pub fn notional(&self) -> Decimal {
        self.notional
    }

    /// Returns the reason for release.
    #[inline]
    #[must_use]
    pub fn reason(&self) -> &str {
        &self.reason
    }
}

impl DomainEvent for CapacityReleased {
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
        EventType::Capacity
    }

    fn event_name(&self) -> &'static str {
        "CapacityReleased"
    }
}

impl fmt::Display for CapacityReleased {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "CapacityReleased: MM {} released {} for RFQ {} ({})",
            self.mm_id, self.notional, self.rfq_id_value, self.reason
        )
    }
}

/// Event emitted when capacity limits are adjusted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapacityAdjusted {
    /// Event metadata.
    metadata: EventMetadata,
    /// Market maker identifier.
    mm_id: CounterpartyId,
    /// The adjustment details.
    adjustment: CapacityAdjustment,
}

impl CapacityAdjusted {
    /// Creates a new capacity adjusted event.
    #[must_use]
    pub fn new(mm_id: CounterpartyId, adjustment: CapacityAdjustment) -> Self {
        Self {
            metadata: EventMetadata::new(None),
            mm_id,
            adjustment,
        }
    }

    /// Returns the market maker identifier.
    #[inline]
    #[must_use]
    pub fn mm_id(&self) -> &CounterpartyId {
        &self.mm_id
    }

    /// Returns the adjustment details.
    #[inline]
    #[must_use]
    pub fn adjustment(&self) -> &CapacityAdjustment {
        &self.adjustment
    }
}

impl DomainEvent for CapacityAdjusted {
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
        EventType::Capacity
    }

    fn event_name(&self) -> &'static str {
        "CapacityAdjusted"
    }
}

impl fmt::Display for CapacityAdjusted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "CapacityAdjusted: MM {} - {}",
            self.mm_id, self.adjustment
        )
    }
}

/// Event emitted when an MM is excluded from RFQ broadcast due to capacity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MmExcludedForCapacity {
    /// Event metadata.
    metadata: EventMetadata,
    /// Market maker identifier.
    mm_id: CounterpartyId,
    /// RFQ identifier.
    rfq_id_value: RfqId,
    /// The capacity check result that caused exclusion.
    reason: String,
}

impl MmExcludedForCapacity {
    /// Creates a new MM excluded event.
    #[must_use]
    pub fn new(mm_id: CounterpartyId, rfq_id: RfqId, check_result: &CapacityCheckResult) -> Self {
        Self {
            metadata: EventMetadata::for_rfq(rfq_id),
            mm_id,
            rfq_id_value: rfq_id,
            reason: check_result.reason(),
        }
    }

    /// Returns the market maker identifier.
    #[inline]
    #[must_use]
    pub fn mm_id(&self) -> &CounterpartyId {
        &self.mm_id
    }

    /// Returns the RFQ identifier.
    #[inline]
    #[must_use]
    pub fn rfq_id_value(&self) -> RfqId {
        self.rfq_id_value
    }

    /// Returns the reason for exclusion.
    #[inline]
    #[must_use]
    pub fn reason(&self) -> &str {
        &self.reason
    }
}

impl DomainEvent for MmExcludedForCapacity {
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
        EventType::Capacity
    }

    fn event_name(&self) -> &'static str {
        "MmExcludedForCapacity"
    }
}

impl fmt::Display for MmExcludedForCapacity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "MmExcludedForCapacity: MM {} excluded from RFQ {} - {}",
            self.mm_id, self.rfq_id_value, self.reason
        )
    }
}

/// Enum wrapper for all capacity-related events.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum CapacityEvent {
    /// Capacity was reserved.
    Reserved(CapacityReserved),
    /// Capacity was released.
    Released(CapacityReleased),
    /// Capacity was adjusted.
    Adjusted(CapacityAdjusted),
    /// MM was excluded due to capacity.
    MmExcluded(MmExcludedForCapacity),
}

impl CapacityEvent {
    /// Returns the market maker identifier for this event.
    #[must_use]
    pub fn mm_id(&self) -> &CounterpartyId {
        match self {
            Self::Reserved(e) => e.mm_id(),
            Self::Released(e) => e.mm_id(),
            Self::Adjusted(e) => e.mm_id(),
            Self::MmExcluded(e) => e.mm_id(),
        }
    }
}

impl DomainEvent for CapacityEvent {
    fn event_id(&self) -> EventId {
        match self {
            Self::Reserved(e) => e.event_id(),
            Self::Released(e) => e.event_id(),
            Self::Adjusted(e) => e.event_id(),
            Self::MmExcluded(e) => e.event_id(),
        }
    }

    fn rfq_id(&self) -> Option<RfqId> {
        match self {
            Self::Reserved(e) => e.rfq_id(),
            Self::Released(e) => e.rfq_id(),
            Self::Adjusted(e) => e.rfq_id(),
            Self::MmExcluded(e) => e.rfq_id(),
        }
    }

    fn timestamp(&self) -> Timestamp {
        match self {
            Self::Reserved(e) => e.timestamp(),
            Self::Released(e) => e.timestamp(),
            Self::Adjusted(e) => e.timestamp(),
            Self::MmExcluded(e) => e.timestamp(),
        }
    }

    fn event_type(&self) -> EventType {
        EventType::Capacity
    }

    fn event_name(&self) -> &'static str {
        match self {
            Self::Reserved(_) => "CapacityReserved",
            Self::Released(_) => "CapacityReleased",
            Self::Adjusted(_) => "CapacityAdjusted",
            Self::MmExcluded(_) => "MmExcludedForCapacity",
        }
    }
}

impl fmt::Display for CapacityEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reserved(e) => write!(f, "{}", e),
            Self::Released(e) => write!(f, "{}", e),
            Self::Adjusted(e) => write!(f, "{}", e),
            Self::MmExcluded(e) => write!(f, "{}", e),
        }
    }
}

impl From<CapacityReserved> for CapacityEvent {
    fn from(event: CapacityReserved) -> Self {
        Self::Reserved(event)
    }
}

impl From<CapacityReleased> for CapacityEvent {
    fn from(event: CapacityReleased) -> Self {
        Self::Released(event)
    }
}

impl From<CapacityAdjusted> for CapacityEvent {
    fn from(event: CapacityAdjusted) -> Self {
        Self::Adjusted(event)
    }
}

impl From<MmExcludedForCapacity> for CapacityEvent {
    fn from(event: MmExcludedForCapacity) -> Self {
        Self::MmExcluded(event)
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::useless_vec
)]
mod tests {
    use super::*;

    fn create_test_symbol() -> Symbol {
        Symbol::new("BTC/USD").unwrap()
    }

    #[test]
    fn capacity_reserved_creates_event() {
        let mm_id = CounterpartyId::new("mm-test");
        let rfq_id = RfqId::new_v4();
        let symbol = create_test_symbol();
        let notional = Decimal::from(100_000);

        let event = CapacityReserved::new(mm_id.clone(), rfq_id, symbol.clone(), notional);

        assert_eq!(event.mm_id(), &mm_id);
        assert_eq!(event.rfq_id_value(), rfq_id);
        assert_eq!(event.instrument(), &symbol);
        assert_eq!(event.notional(), notional);
    }

    #[test]
    fn capacity_reserved_implements_domain_event() {
        let mm_id = CounterpartyId::new("mm-test");
        let rfq_id = RfqId::new_v4();
        let symbol = create_test_symbol();
        let notional = Decimal::from(100_000);

        let event = CapacityReserved::new(mm_id, rfq_id, symbol, notional);

        assert_eq!(event.rfq_id(), Some(rfq_id));
        assert_eq!(event.event_type(), EventType::Capacity);
        assert_eq!(event.event_name(), "CapacityReserved");
    }

    #[test]
    fn capacity_released_creates_event() {
        let mm_id = CounterpartyId::new("mm-test");
        let rfq_id = RfqId::new_v4();
        let notional = Decimal::from(100_000);

        let event = CapacityReleased::new(mm_id.clone(), rfq_id, notional, "expired");

        assert_eq!(event.mm_id(), &mm_id);
        assert_eq!(event.rfq_id_value(), rfq_id);
        assert_eq!(event.reason(), "expired");
    }

    #[test]
    fn capacity_released_implements_domain_event() {
        let mm_id = CounterpartyId::new("mm-test");
        let rfq_id = RfqId::new_v4();
        let notional = Decimal::from(100_000);

        let event = CapacityReleased::new(mm_id, rfq_id, notional, "expired");

        assert_eq!(event.rfq_id(), Some(rfq_id));
        assert_eq!(event.event_type(), EventType::Capacity);
        assert_eq!(event.event_name(), "CapacityReleased");
    }

    #[test]
    fn capacity_adjusted_implements_domain_event() {
        let mm_id = CounterpartyId::new("mm-test");
        let adjustment = CapacityAdjustment::new(
            50,
            60,
            Decimal::from(1_000_000),
            Decimal::from(1_000_000),
            "performance improvement",
        );

        let event = CapacityAdjusted::new(mm_id, adjustment);

        assert_eq!(event.rfq_id(), None);
        assert_eq!(event.event_type(), EventType::Capacity);
        assert_eq!(event.event_name(), "CapacityAdjusted");
    }

    #[test]
    fn mm_excluded_creates_event() {
        let mm_id = CounterpartyId::new("mm-test");
        let rfq_id = RfqId::new_v4();
        let check_result = CapacityCheckResult::AtMaxQuotes {
            current: 50,
            max: 50,
        };

        let event = MmExcludedForCapacity::new(mm_id.clone(), rfq_id, &check_result);

        assert_eq!(event.mm_id(), &mm_id);
        assert_eq!(event.rfq_id_value(), rfq_id);
        assert!(event.reason().contains("at max quotes"));
    }

    #[test]
    fn mm_excluded_implements_domain_event() {
        let mm_id = CounterpartyId::new("mm-test");
        let rfq_id = RfqId::new_v4();
        let check_result = CapacityCheckResult::AtMaxQuotes {
            current: 50,
            max: 50,
        };

        let event = MmExcludedForCapacity::new(mm_id, rfq_id, &check_result);

        assert_eq!(event.rfq_id(), Some(rfq_id));
        assert_eq!(event.event_type(), EventType::Capacity);
        assert_eq!(event.event_name(), "MmExcludedForCapacity");
    }

    #[test]
    fn capacity_event_enum_conversions() {
        let mm_id = CounterpartyId::new("mm-test");
        let rfq_id = RfqId::new_v4();
        let symbol = create_test_symbol();

        let reserved = CapacityReserved::new(mm_id.clone(), rfq_id, symbol, Decimal::from(100_000));
        let event: CapacityEvent = reserved.into();

        assert!(matches!(event, CapacityEvent::Reserved(_)));
        assert_eq!(event.mm_id(), &mm_id);
    }

    #[test]
    fn capacity_event_enum_implements_domain_event() {
        let mm_id = CounterpartyId::new("mm-test");
        let rfq_id = RfqId::new_v4();
        let symbol = create_test_symbol();

        let reserved = CapacityReserved::new(mm_id, rfq_id, symbol, Decimal::from(100_000));
        let event: CapacityEvent = reserved.into();

        assert_eq!(event.rfq_id(), Some(rfq_id));
        assert_eq!(event.event_type(), EventType::Capacity);
        assert_eq!(event.event_name(), "CapacityReserved");
    }
}
