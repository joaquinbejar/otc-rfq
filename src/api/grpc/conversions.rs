//! # gRPC Conversions
//!
//! Conversions between Protocol Buffer messages and domain types.
//!
//! This module provides bidirectional conversions for all types used in the
//! gRPC API, including RFQs, Quotes, Trades, and supporting value objects.
//!
//! # Conversion Traits
//!
//! - `From<DomainType> for ProtoType` - Domain to proto (infallible)
//! - `TryFrom<ProtoType> for DomainType` - Proto to domain (fallible)
//!
//! # Examples
//!
//! ```ignore
//! use otc_rfq::api::grpc::conversions::*;
//! use otc_rfq::domain::value_objects::RfqId;
//!
//! let domain_id = RfqId::new_v4();
//! let proto_id: proto::Uuid = domain_id.into();
//! let back: RfqId = proto_id.try_into().unwrap();
//! ```

use crate::api::grpc::proto;
use crate::domain::entities::quote::Quote as DomainQuote;
use crate::domain::entities::rfq::Rfq as DomainRfq;
use crate::domain::entities::trade::Trade as DomainTrade;
use crate::domain::value_objects::enums::{
    AssetClass as DomainAssetClass, VenueType as DomainVenueType,
};
use crate::domain::value_objects::timestamp::Timestamp as DomainTimestamp;
use crate::domain::value_objects::{
    Instrument as DomainInstrument, OrderSide as DomainOrderSide, Price, Quantity, QuoteId, RfqId,
    RfqState as DomainRfqState, TradeId,
};
use rust_decimal::Decimal;
use std::str::FromStr;
use thiserror::Error;

/// Error type for conversion failures.
#[derive(Debug, Error)]
pub enum ConversionError {
    /// Missing required field.
    #[error("missing required field: {0}")]
    MissingField(&'static str),

    /// Invalid field value.
    #[error("invalid {field}: {message}")]
    InvalidValue {
        /// Field name.
        field: &'static str,
        /// Error message.
        message: String,
    },

    /// Invalid UUID format.
    #[error("invalid UUID: {0}")]
    InvalidUuid(String),

    /// Invalid decimal format.
    #[error("invalid decimal: {0}")]
    InvalidDecimal(String),

    /// Invalid enum value.
    #[error("invalid enum value for {enum_name}: {value}")]
    InvalidEnum {
        /// Enum name.
        enum_name: &'static str,
        /// Invalid value.
        value: i32,
    },
}

// ============================================================================
// UUID Conversions
// ============================================================================

impl From<RfqId> for proto::Uuid {
    fn from(id: RfqId) -> Self {
        Self {
            value: id.to_string(),
        }
    }
}

impl TryFrom<proto::Uuid> for RfqId {
    type Error = ConversionError;

    fn try_from(proto: proto::Uuid) -> Result<Self, Self::Error> {
        uuid::Uuid::parse_str(&proto.value)
            .map(RfqId::from)
            .map_err(|_| ConversionError::InvalidUuid(proto.value))
    }
}

impl From<QuoteId> for proto::Uuid {
    fn from(id: QuoteId) -> Self {
        Self {
            value: id.to_string(),
        }
    }
}

impl TryFrom<proto::Uuid> for QuoteId {
    type Error = ConversionError;

    fn try_from(proto: proto::Uuid) -> Result<Self, Self::Error> {
        uuid::Uuid::parse_str(&proto.value)
            .map(QuoteId::from)
            .map_err(|_| ConversionError::InvalidUuid(proto.value))
    }
}

impl From<TradeId> for proto::Uuid {
    fn from(id: TradeId) -> Self {
        Self {
            value: id.to_string(),
        }
    }
}

impl TryFrom<proto::Uuid> for TradeId {
    type Error = ConversionError;

    fn try_from(proto: proto::Uuid) -> Result<Self, Self::Error> {
        uuid::Uuid::parse_str(&proto.value)
            .map(TradeId::from)
            .map_err(|_| ConversionError::InvalidUuid(proto.value))
    }
}

// ============================================================================
// Decimal Conversions
// ============================================================================

impl From<Decimal> for proto::Decimal {
    fn from(d: Decimal) -> Self {
        Self {
            value: d.to_string(),
        }
    }
}

impl TryFrom<proto::Decimal> for Decimal {
    type Error = ConversionError;

    fn try_from(proto: proto::Decimal) -> Result<Self, Self::Error> {
        Decimal::from_str(&proto.value).map_err(|_| ConversionError::InvalidDecimal(proto.value))
    }
}

impl From<Price> for proto::Decimal {
    fn from(p: Price) -> Self {
        Self {
            value: p.to_string(),
        }
    }
}

impl From<Quantity> for proto::Decimal {
    fn from(q: Quantity) -> Self {
        Self {
            value: q.to_string(),
        }
    }
}

// ============================================================================
// Timestamp Conversions
// ============================================================================

impl From<DomainTimestamp> for proto::Timestamp {
    fn from(ts: DomainTimestamp) -> Self {
        let millis = ts.timestamp_millis();
        let seconds = millis / 1000;
        let nanos = ((millis % 1000) * 1_000_000) as i32;
        Self { seconds, nanos }
    }
}

impl From<proto::Timestamp> for DomainTimestamp {
    fn from(proto: proto::Timestamp) -> Self {
        let millis = proto.seconds * 1000 + i64::from(proto.nanos) / 1_000_000;
        DomainTimestamp::from_millis(millis).unwrap_or_else(DomainTimestamp::now)
    }
}

// ============================================================================
// Enum Conversions
// ============================================================================

impl From<DomainOrderSide> for proto::OrderSide {
    fn from(side: DomainOrderSide) -> Self {
        match side {
            DomainOrderSide::Buy => proto::OrderSide::Buy,
            DomainOrderSide::Sell => proto::OrderSide::Sell,
        }
    }
}

impl From<DomainOrderSide> for i32 {
    fn from(side: DomainOrderSide) -> Self {
        proto::OrderSide::from(side) as i32
    }
}

/// Converts a proto OrderSide i32 value to a domain OrderSide.
///
/// # Errors
///
/// Returns `ConversionError::InvalidEnum` if the value is invalid or unspecified.
pub fn proto_order_side_to_domain(value: i32) -> Result<DomainOrderSide, ConversionError> {
    match proto::OrderSide::try_from(value) {
        Ok(proto::OrderSide::Buy) => Ok(DomainOrderSide::Buy),
        Ok(proto::OrderSide::Sell) => Ok(DomainOrderSide::Sell),
        Ok(proto::OrderSide::Unspecified) | Err(_) => Err(ConversionError::InvalidEnum {
            enum_name: "OrderSide",
            value,
        }),
    }
}

impl From<DomainRfqState> for proto::RfqState {
    fn from(state: DomainRfqState) -> Self {
        match state {
            DomainRfqState::Created => proto::RfqState::Created,
            DomainRfqState::QuoteRequesting => proto::RfqState::QuoteRequesting,
            DomainRfqState::QuotesReceived => proto::RfqState::QuotesReceived,
            DomainRfqState::ClientSelecting => proto::RfqState::ClientSelecting,
            DomainRfqState::Executing => proto::RfqState::Executing,
            DomainRfqState::Executed => proto::RfqState::Executed,
            DomainRfqState::Failed => proto::RfqState::Failed,
            DomainRfqState::Cancelled => proto::RfqState::Cancelled,
            DomainRfqState::Expired => proto::RfqState::Expired,
        }
    }
}

impl From<DomainRfqState> for i32 {
    fn from(state: DomainRfqState) -> Self {
        proto::RfqState::from(state) as i32
    }
}

/// Converts a proto RfqState i32 value to a domain RfqState.
///
/// # Errors
///
/// Returns `ConversionError::InvalidEnum` if the value is invalid or unspecified.
pub fn proto_rfq_state_to_domain(value: i32) -> Result<DomainRfqState, ConversionError> {
    match proto::RfqState::try_from(value) {
        Ok(proto::RfqState::Created) => Ok(DomainRfqState::Created),
        Ok(proto::RfqState::QuoteRequesting) => Ok(DomainRfqState::QuoteRequesting),
        Ok(proto::RfqState::QuotesReceived) => Ok(DomainRfqState::QuotesReceived),
        Ok(proto::RfqState::ClientSelecting) => Ok(DomainRfqState::ClientSelecting),
        Ok(proto::RfqState::Executing) => Ok(DomainRfqState::Executing),
        Ok(proto::RfqState::Executed) => Ok(DomainRfqState::Executed),
        Ok(proto::RfqState::Failed) => Ok(DomainRfqState::Failed),
        Ok(proto::RfqState::Cancelled) => Ok(DomainRfqState::Cancelled),
        Ok(proto::RfqState::Expired) => Ok(DomainRfqState::Expired),
        Ok(proto::RfqState::Unspecified) | Err(_) => Err(ConversionError::InvalidEnum {
            enum_name: "RfqState",
            value,
        }),
    }
}

impl From<DomainAssetClass> for proto::AssetClass {
    fn from(ac: DomainAssetClass) -> Self {
        match ac {
            DomainAssetClass::CryptoSpot => proto::AssetClass::CryptoSpot,
            DomainAssetClass::CryptoDerivs => proto::AssetClass::CryptoDerivs,
            DomainAssetClass::Stock => proto::AssetClass::Stock,
            DomainAssetClass::Forex => proto::AssetClass::Forex,
            DomainAssetClass::Commodity => proto::AssetClass::Commodity,
        }
    }
}

impl From<DomainAssetClass> for i32 {
    fn from(ac: DomainAssetClass) -> Self {
        proto::AssetClass::from(ac) as i32
    }
}

impl From<DomainVenueType> for proto::VenueType {
    fn from(vt: DomainVenueType) -> Self {
        match vt {
            DomainVenueType::InternalMM => proto::VenueType::InternalMm,
            DomainVenueType::ExternalMM => proto::VenueType::ExternalMm,
            DomainVenueType::DexAggregator => proto::VenueType::DexAggregator,
            DomainVenueType::Protocol => proto::VenueType::Protocol,
            DomainVenueType::RfqProtocol => proto::VenueType::RfqProtocol,
        }
    }
}

impl From<DomainVenueType> for i32 {
    fn from(vt: DomainVenueType) -> Self {
        proto::VenueType::from(vt) as i32
    }
}

// ============================================================================
// Instrument Conversions
// ============================================================================

impl From<&DomainInstrument> for proto::Instrument {
    fn from(inst: &DomainInstrument) -> Self {
        Self {
            symbol: inst.symbol().to_string(),
            asset_class: i32::from(inst.asset_class()),
            base_asset: inst.symbol().base_asset().to_string(),
            quote_asset: inst.symbol().quote_asset().to_string(),
        }
    }
}

impl From<DomainInstrument> for proto::Instrument {
    fn from(inst: DomainInstrument) -> Self {
        proto::Instrument::from(&inst)
    }
}

// ============================================================================
// Quote Conversions
// ============================================================================

impl From<&DomainQuote> for proto::Quote {
    fn from(quote: &DomainQuote) -> Self {
        Self {
            id: Some(proto::Uuid::from(quote.id())),
            rfq_id: Some(proto::Uuid::from(quote.rfq_id())),
            venue_id: quote.venue_id().to_string(),
            venue_type: proto::VenueType::Unspecified as i32, // Venue type not stored in domain Quote
            price: Some(proto::Decimal::from(quote.price())),
            quantity: Some(proto::Decimal::from(quote.quantity())),
            commission: quote.commission().map(proto::Decimal::from),
            valid_until: Some(proto::Timestamp::from(quote.valid_until())),
            created_at: Some(proto::Timestamp::from(quote.created_at())),
        }
    }
}

impl From<DomainQuote> for proto::Quote {
    fn from(quote: DomainQuote) -> Self {
        proto::Quote::from(&quote)
    }
}

// ============================================================================
// Trade Conversions
// ============================================================================

impl From<&DomainTrade> for proto::Trade {
    fn from(trade: &DomainTrade) -> Self {
        Self {
            id: Some(proto::Uuid::from(trade.id())),
            rfq_id: Some(proto::Uuid::from(trade.rfq_id())),
            quote_id: Some(proto::Uuid::from(trade.quote_id())),
            venue_id: trade.venue_id().to_string(),
            price: Some(proto::Decimal::from(trade.price())),
            quantity: Some(proto::Decimal::from(trade.quantity())),
            venue_execution_ref: trade.venue_execution_ref().unwrap_or_default().to_string(),
            created_at: Some(proto::Timestamp::from(trade.created_at())),
        }
    }
}

impl From<DomainTrade> for proto::Trade {
    fn from(trade: DomainTrade) -> Self {
        proto::Trade::from(&trade)
    }
}

// ============================================================================
// RFQ Conversions
// ============================================================================

impl From<&DomainRfq> for proto::Rfq {
    fn from(rfq: &DomainRfq) -> Self {
        Self {
            id: Some(proto::Uuid::from(rfq.id())),
            client_id: rfq.client_id().to_string(),
            instrument: Some(proto::Instrument::from(rfq.instrument())),
            side: i32::from(rfq.side()),
            quantity: Some(proto::Decimal::from(rfq.quantity())),
            state: i32::from(rfq.state()),
            expires_at: Some(proto::Timestamp::from(rfq.expires_at())),
            quotes: rfq.quotes().iter().map(proto::Quote::from).collect(),
            selected_quote_id: rfq.selected_quote_id().map(proto::Uuid::from),
            created_at: Some(proto::Timestamp::from(rfq.created_at())),
            updated_at: Some(proto::Timestamp::from(rfq.updated_at())),
        }
    }
}

impl From<DomainRfq> for proto::Rfq {
    fn from(rfq: DomainRfq) -> Self {
        proto::Rfq::from(&rfq)
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Extracts a required field from an Option, returning a ConversionError if missing.
///
/// # Errors
///
/// Returns `ConversionError::MissingField` if the option is None.
pub fn require_field<T>(opt: Option<T>, field_name: &'static str) -> Result<T, ConversionError> {
    opt.ok_or(ConversionError::MissingField(field_name))
}

/// Converts a proto Decimal to a domain Price.
///
/// # Errors
///
/// Returns `ConversionError::MissingField` if the proto is None.
/// Returns `ConversionError::InvalidDecimal` if the decimal string is invalid.
/// Returns `ConversionError::InvalidValue` if the value cannot be converted to Price.
pub fn proto_decimal_to_price(
    proto: Option<proto::Decimal>,
    field_name: &'static str,
) -> Result<Price, ConversionError> {
    let decimal = require_field(proto, field_name)?;
    let value = Decimal::try_from(decimal)?;
    Price::try_from(value).map_err(|e| ConversionError::InvalidValue {
        field: field_name,
        message: e.to_string(),
    })
}

/// Converts a proto Decimal to a domain Quantity.
///
/// # Errors
///
/// Returns `ConversionError::MissingField` if the proto is None.
/// Returns `ConversionError::InvalidDecimal` if the decimal string is invalid.
/// Returns `ConversionError::InvalidValue` if the value cannot be converted to Quantity.
pub fn proto_decimal_to_quantity(
    proto: Option<proto::Decimal>,
    field_name: &'static str,
) -> Result<Quantity, ConversionError> {
    let decimal = require_field(proto, field_name)?;
    let value = Decimal::try_from(decimal)?;
    Quantity::try_from(value).map_err(|e| ConversionError::InvalidValue {
        field: field_name,
        message: e.to_string(),
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::domain::entities::quote::QuoteBuilder;
    use crate::domain::entities::rfq::RfqBuilder;
    use crate::domain::value_objects::{CounterpartyId, Instrument, VenueId};

    #[test]
    fn rfq_id_roundtrip() {
        let original = RfqId::new_v4();
        let proto_uuid: proto::Uuid = original.into();
        let back: RfqId = proto_uuid.try_into().unwrap();
        assert_eq!(original, back);
    }

    #[test]
    fn quote_id_roundtrip() {
        let original = QuoteId::new_v4();
        let proto_uuid: proto::Uuid = original.into();
        let back: QuoteId = proto_uuid.try_into().unwrap();
        assert_eq!(original, back);
    }

    #[test]
    fn trade_id_roundtrip() {
        let original = TradeId::new_v4();
        let proto_uuid: proto::Uuid = original.into();
        let back: TradeId = proto_uuid.try_into().unwrap();
        assert_eq!(original, back);
    }

    #[test]
    fn invalid_uuid_returns_error() {
        let proto_uuid = proto::Uuid {
            value: "not-a-uuid".to_string(),
        };
        let result: Result<RfqId, _> = proto_uuid.try_into();
        assert!(result.is_err());
    }

    #[test]
    fn decimal_roundtrip() {
        let original = Decimal::from_str("123.456789").unwrap();
        let proto_decimal: proto::Decimal = original.into();
        let back: Decimal = proto_decimal.try_into().unwrap();
        assert_eq!(original, back);
    }

    #[test]
    fn invalid_decimal_returns_error() {
        let proto_decimal = proto::Decimal {
            value: "not-a-number".to_string(),
        };
        let result: Result<Decimal, _> = proto_decimal.try_into();
        assert!(result.is_err());
    }

    #[test]
    fn timestamp_roundtrip() {
        let original = DomainTimestamp::now();
        let proto_ts: proto::Timestamp = original.into();
        let back: DomainTimestamp = proto_ts.into();
        // Allow 1ms difference due to precision loss
        let diff = (original.timestamp_millis() - back.timestamp_millis()).abs();
        assert!(diff <= 1);
    }

    #[test]
    fn order_side_buy_conversion() {
        let domain = DomainOrderSide::Buy;
        let proto_val: i32 = domain.into();
        assert_eq!(proto_val, proto::OrderSide::Buy as i32);

        let back = proto_order_side_to_domain(proto_val).unwrap();
        assert_eq!(back, DomainOrderSide::Buy);
    }

    #[test]
    fn order_side_sell_conversion() {
        let domain = DomainOrderSide::Sell;
        let proto_val: i32 = domain.into();
        assert_eq!(proto_val, proto::OrderSide::Sell as i32);

        let back = proto_order_side_to_domain(proto_val).unwrap();
        assert_eq!(back, DomainOrderSide::Sell);
    }

    #[test]
    fn order_side_unspecified_returns_error() {
        let result = proto_order_side_to_domain(0);
        assert!(result.is_err());
    }

    #[test]
    fn rfq_state_all_variants() {
        let states = [
            (DomainRfqState::Created, proto::RfqState::Created),
            (
                DomainRfqState::QuoteRequesting,
                proto::RfqState::QuoteRequesting,
            ),
            (
                DomainRfqState::QuotesReceived,
                proto::RfqState::QuotesReceived,
            ),
            (
                DomainRfqState::ClientSelecting,
                proto::RfqState::ClientSelecting,
            ),
            (DomainRfqState::Executing, proto::RfqState::Executing),
            (DomainRfqState::Executed, proto::RfqState::Executed),
            (DomainRfqState::Failed, proto::RfqState::Failed),
            (DomainRfqState::Cancelled, proto::RfqState::Cancelled),
            (DomainRfqState::Expired, proto::RfqState::Expired),
        ];

        for (domain, expected_proto) in states {
            let proto_val: i32 = domain.into();
            assert_eq!(proto_val, expected_proto as i32);

            let back = proto_rfq_state_to_domain(proto_val).unwrap();
            assert_eq!(back, domain);
        }
    }

    #[test]
    fn asset_class_conversion() {
        let domain = DomainAssetClass::CryptoSpot;
        let proto_val: i32 = domain.into();
        assert_eq!(proto_val, proto::AssetClass::CryptoSpot as i32);
    }

    #[test]
    fn venue_type_conversion() {
        let domain = DomainVenueType::ExternalMM;
        let proto_val: i32 = domain.into();
        assert_eq!(proto_val, proto::VenueType::ExternalMm as i32);
    }

    #[test]
    fn instrument_conversion() {
        use crate::domain::value_objects::Symbol;
        use crate::domain::value_objects::enums::AssetClass;

        let symbol = Symbol::new("BTC/USD").unwrap();
        let instrument = Instrument::builder(symbol, AssetClass::CryptoSpot).build();
        let proto_inst: proto::Instrument = instrument.clone().into();

        assert_eq!(proto_inst.symbol, "BTC/USD");
        assert_eq!(proto_inst.base_asset, "BTC");
        assert_eq!(proto_inst.quote_asset, "USD");
        assert_eq!(proto_inst.asset_class, proto::AssetClass::CryptoSpot as i32);
    }

    #[test]
    fn quote_conversion() {
        let quote = QuoteBuilder::new(
            RfqId::new_v4(),
            VenueId::new("venue-1"),
            Price::new(50000.0).unwrap(),
            Quantity::new(1.0).unwrap(),
            DomainTimestamp::now().add_secs(300),
        )
        .build();

        let proto_quote: proto::Quote = quote.clone().into();

        assert!(proto_quote.id.is_some());
        assert!(proto_quote.rfq_id.is_some());
        assert_eq!(proto_quote.venue_id, "venue-1");
        assert!(proto_quote.price.is_some());
        assert!(proto_quote.quantity.is_some());
        assert!(proto_quote.valid_until.is_some());
    }

    #[test]
    fn rfq_conversion() {
        use crate::domain::value_objects::Symbol;
        use crate::domain::value_objects::enums::AssetClass;

        let symbol = Symbol::new("ETH/USD").unwrap();
        let instrument = Instrument::builder(symbol, AssetClass::CryptoSpot).build();
        let rfq = RfqBuilder::new(
            CounterpartyId::new("client-1"),
            instrument,
            DomainOrderSide::Buy,
            Quantity::new(10.0).unwrap(),
            DomainTimestamp::now().add_secs(300),
        )
        .build();

        let proto_rfq: proto::Rfq = rfq.clone().into();

        assert!(proto_rfq.id.is_some());
        assert_eq!(proto_rfq.client_id, "client-1");
        assert!(proto_rfq.instrument.is_some());
        assert_eq!(proto_rfq.side, proto::OrderSide::Buy as i32);
        assert!(proto_rfq.quantity.is_some());
        assert_eq!(proto_rfq.state, proto::RfqState::Created as i32);
    }

    #[test]
    fn require_field_returns_value() {
        let result = require_field(Some(42), "test_field");
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn require_field_returns_error_on_none() {
        let result: Result<i32, _> = require_field(None, "test_field");
        assert!(matches!(
            result,
            Err(ConversionError::MissingField("test_field"))
        ));
    }

    #[test]
    fn proto_decimal_to_price_valid() {
        let proto_decimal = proto::Decimal {
            value: "100.50".to_string(),
        };
        let result = proto_decimal_to_price(Some(proto_decimal), "price");
        assert!(result.is_ok());
    }

    #[test]
    fn proto_decimal_to_price_missing() {
        let result = proto_decimal_to_price(None, "price");
        assert!(matches!(
            result,
            Err(ConversionError::MissingField("price"))
        ));
    }

    #[test]
    fn proto_decimal_to_quantity_valid() {
        let proto_decimal = proto::Decimal {
            value: "10.5".to_string(),
        };
        let result = proto_decimal_to_quantity(Some(proto_decimal), "quantity");
        assert!(result.is_ok());
    }
}
