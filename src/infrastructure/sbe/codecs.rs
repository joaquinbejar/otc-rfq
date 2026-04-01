//! # SBE Message Codecs
//!
//! Encoding and decoding implementations for SBE messages.
//!
//! This module provides codecs for the three main SBE message types:
//! - `RfqCreated` (template ID: 1)
//! - `QuoteReceived` (template ID: 2)
//! - `TradeExecuted` (template ID: 3)

// Buffer bounds are validated before indexing in all functions
#![allow(clippy::indexing_slicing)]

use super::error::{SbeError, SbeResult};
use super::traits::{SbeDecode, SbeEncode};
use super::types::{SbeDecimal, SbeUuid, decode_var_string, encode_var_string};
use crate::domain::events::rfq_events::{QuoteReceived, RfqCreated};
use crate::domain::events::trade_events::TradeExecuted;
use crate::domain::value_objects::enums::{Blockchain, SettlementMethod};
use crate::domain::value_objects::timestamp::Timestamp;
use crate::domain::value_objects::{
    CounterpartyId, EventId, OrderSide, Price, Quantity, QuoteId, RfqId, TradeId, VenueId,
};

/// Message header size in bytes.
pub const MESSAGE_HEADER_SIZE: usize = 8;

/// Schema ID for OTC RFQ messages.
pub const SCHEMA_ID: u16 = 1;

/// Schema version.
/// Version 2: added fee fields (taker_fee, maker_fee, net_fee) and settlement_method
/// to the TradeExecuted message fixed block.
pub const SCHEMA_VERSION: u16 = 2;

/// Template ID for RfqCreated message.
pub const RFQ_CREATED_TEMPLATE_ID: u16 = 1;

/// Template ID for QuoteReceived message.
pub const QUOTE_RECEIVED_TEMPLATE_ID: u16 = 2;

/// Template ID for TradeExecuted message.
pub const TRADE_EXECUTED_TEMPLATE_ID: u16 = 3;

/// Encodes the SBE message header.
fn encode_header(buffer: &mut [u8], block_length: u16, template_id: u16) -> SbeResult<()> {
    if buffer.len() < MESSAGE_HEADER_SIZE {
        return Err(SbeError::BufferTooSmall {
            needed: MESSAGE_HEADER_SIZE,
            available: buffer.len(),
        });
    }
    buffer[0..2].copy_from_slice(&block_length.to_le_bytes());
    buffer[2..4].copy_from_slice(&template_id.to_le_bytes());
    buffer[4..6].copy_from_slice(&SCHEMA_ID.to_le_bytes());
    buffer[6..8].copy_from_slice(&SCHEMA_VERSION.to_le_bytes());
    Ok(())
}

/// Decodes the SBE message header.
fn decode_header(buffer: &[u8]) -> SbeResult<(u16, u16, u16, u16)> {
    if buffer.len() < MESSAGE_HEADER_SIZE {
        return Err(SbeError::BufferTooSmall {
            needed: MESSAGE_HEADER_SIZE,
            available: buffer.len(),
        });
    }
    let block_length = u16::from_le_bytes([buffer[0], buffer[1]]);
    let template_id = u16::from_le_bytes([buffer[2], buffer[3]]);
    let schema_id = u16::from_le_bytes([buffer[4], buffer[5]]);
    let version = u16::from_le_bytes([buffer[6], buffer[7]]);
    Ok((block_length, template_id, schema_id, version))
}

/// Encodes OrderSide to SBE enum value.
#[inline]
#[must_use]
fn encode_order_side(side: OrderSide) -> u8 {
    match side {
        OrderSide::Buy => 0,
        OrderSide::Sell => 1,
    }
}

/// Decodes OrderSide from SBE enum value.
#[inline]
fn decode_order_side(value: u8) -> SbeResult<OrderSide> {
    match value {
        0 => Ok(OrderSide::Buy),
        1 => Ok(OrderSide::Sell),
        _ => Err(SbeError::InvalidEnumValue(value)),
    }
}

/// Encodes [`SettlementMethod`] to two SBE bytes: `method_tag` + `blockchain_tag`.
///
/// Wire layout: `[method(1B)][blockchain(1B)]`
/// - `method = 0` → `OffChain`, blockchain byte is `0` (ignored).
/// - `method = 1` → `OnChain(chain)`, blockchain byte encodes the chain.
#[inline]
#[must_use]
fn encode_settlement_method(method: SettlementMethod) -> [u8; 2] {
    match method {
        SettlementMethod::OffChain => [0, 0],
        SettlementMethod::OnChain(chain) => [1, chain.as_u8()],
    }
}

/// Decodes [`SettlementMethod`] from two SBE bytes: `method_tag` + `blockchain_tag`.
///
/// # Errors
///
/// Returns [`SbeError::InvalidEnumValue`] if `method_tag` is unrecognized,
/// or if it's `OnChain` and `blockchain_tag` is unrecognized.
#[inline]
fn decode_settlement_method(buffer: &[u8; 2]) -> SbeResult<SettlementMethod> {
    match buffer[0] {
        0 => Ok(SettlementMethod::OffChain),
        1 => Blockchain::from_u8(buffer[1])
            .map(SettlementMethod::OnChain)
            .ok_or(SbeError::InvalidEnumValue(buffer[1])),
        _ => Err(SbeError::InvalidEnumValue(buffer[0])),
    }
}

/// Encodes an `Option<Decimal>` fee as a presence flag + [`SbeDecimal`] (10 bytes total).
///
/// Wire layout: `[present(1B)][mantissa(8B)][exponent(1B)]`
///
/// # Errors
///
/// Returns an error if the buffer is too small.
#[inline]
fn encode_optional_fee(fee: Option<rust_decimal::Decimal>, buffer: &mut [u8]) -> SbeResult<()> {
    const SIZE: usize = 1 + SbeDecimal::SIZE; // 10 bytes
    if buffer.len() < SIZE {
        return Err(SbeError::BufferTooSmall {
            needed: SIZE,
            available: buffer.len(),
        });
    }
    match fee {
        None => {
            buffer[0] = 0;
            buffer[1..SIZE].fill(0);
        }
        Some(dec) => {
            buffer[0] = 1;
            SbeDecimal::from_decimal(dec).encode(&mut buffer[1..])?;
        }
    }
    Ok(())
}

/// Decodes an `Option<Decimal>` fee from a presence flag + [`SbeDecimal`] (10 bytes).
///
/// # Errors
///
/// Returns an error if the buffer is too small or the decimal is invalid.
#[inline]
fn decode_optional_fee(buffer: &[u8]) -> SbeResult<Option<rust_decimal::Decimal>> {
    const SIZE: usize = 1 + SbeDecimal::SIZE; // 10 bytes
    if buffer.len() < SIZE {
        return Err(SbeError::BufferTooSmall {
            needed: SIZE,
            available: buffer.len(),
        });
    }
    match buffer[0] {
        0 => Ok(None),
        1 => Ok(Some(SbeDecimal::decode(&buffer[1..])?.to_decimal()?)),
        _ => Err(SbeError::InvalidEnumValue(buffer[0])),
    }
}

// ============================================================================
// RfqCreated Codec
// ============================================================================

/// Codec for RfqCreated messages.
///
/// Wire format:
/// - Header (8 bytes)
/// - eventId: UUID (16 bytes)
/// - rfqId: UUID (16 bytes)
/// - side: u8 (1 byte)
/// - quantity: Decimal (9 bytes)
/// - expiresAt: u64 (8 bytes)
/// - createdAt: u64 (8 bytes)
/// - clientId: varString
/// - symbol: varString
pub struct RfqCreatedCodec;

impl RfqCreatedCodec {
    /// Fixed block length (before variable fields).
    const BLOCK_LENGTH: u16 = 16 + 16 + 1 + 9 + 8 + 8; // 58 bytes
}

impl SbeEncode for RfqCreated {
    fn encoded_size(&self) -> usize {
        MESSAGE_HEADER_SIZE
            + RfqCreatedCodec::BLOCK_LENGTH as usize
            + 4
            + self.client_id.as_str().len()
            + 4
            + self.instrument.symbol().as_str().len()
    }

    fn encode(&self, buffer: &mut [u8]) -> SbeResult<usize> {
        let size = self.encoded_size();
        if buffer.len() < size {
            return Err(SbeError::BufferTooSmall {
                needed: size,
                available: buffer.len(),
            });
        }

        let mut offset = 0;

        // Header
        encode_header(
            &mut buffer[offset..],
            RfqCreatedCodec::BLOCK_LENGTH,
            RFQ_CREATED_TEMPLATE_ID,
        )?;
        offset += MESSAGE_HEADER_SIZE;

        // eventId
        let event_uuid = SbeUuid::from_uuid(self.metadata.event_id.get());
        event_uuid.encode(&mut buffer[offset..])?;
        offset += SbeUuid::SIZE;

        // rfqId
        if let Some(rfq_id) = self.metadata.rfq_id {
            let rfq_uuid = SbeUuid::from_uuid(rfq_id.get());
            rfq_uuid.encode(&mut buffer[offset..])?;
        } else {
            buffer[offset..offset + SbeUuid::SIZE].fill(0);
        }
        offset += SbeUuid::SIZE;

        // side
        buffer[offset] = encode_order_side(self.side);
        offset += 1;

        // quantity
        let qty_decimal = SbeDecimal::from_decimal(self.quantity.get());
        qty_decimal.encode(&mut buffer[offset..])?;
        offset += SbeDecimal::SIZE;

        // expiresAt (nanoseconds)
        let expires_nanos = self.expires_at.timestamp_nanos().unwrap_or(0) as u64;
        buffer[offset..offset + 8].copy_from_slice(&expires_nanos.to_le_bytes());
        offset += 8;

        // createdAt (nanoseconds)
        let created_nanos = self.metadata.timestamp.timestamp_nanos().unwrap_or(0) as u64;
        buffer[offset..offset + 8].copy_from_slice(&created_nanos.to_le_bytes());
        offset += 8;

        // clientId (variable)
        let client_size = encode_var_string(self.client_id.as_str(), &mut buffer[offset..])?;
        offset += client_size;

        // symbol (variable)
        let symbol_size =
            encode_var_string(self.instrument.symbol().as_str(), &mut buffer[offset..])?;
        offset += symbol_size;

        Ok(offset)
    }
}

impl SbeDecode for RfqCreated {
    fn decode(buffer: &[u8]) -> SbeResult<Self> {
        let (block_length, template_id, _schema_id, _version) = decode_header(buffer)?;

        if template_id != RFQ_CREATED_TEMPLATE_ID {
            return Err(SbeError::UnknownTemplateId(template_id));
        }

        let mut offset = MESSAGE_HEADER_SIZE;

        // eventId
        let event_uuid = SbeUuid::decode(&buffer[offset..])?;
        let event_id = EventId::new(event_uuid.to_uuid());
        offset += SbeUuid::SIZE;

        // rfqId
        let rfq_uuid = SbeUuid::decode(&buffer[offset..])?;
        let rfq_id = RfqId::new(rfq_uuid.to_uuid());
        offset += SbeUuid::SIZE;

        // side
        let side = decode_order_side(buffer[offset])?;
        offset += 1;

        // quantity
        let qty_sbe = SbeDecimal::decode(&buffer[offset..])?;
        let quantity = Quantity::from_decimal(qty_sbe.to_decimal()?)
            .map_err(|e| SbeError::InvalidDecimal(e.to_string()))?;
        offset += SbeDecimal::SIZE;

        // expiresAt
        let expires_nanos = u64::from_le_bytes([
            buffer[offset],
            buffer[offset + 1],
            buffer[offset + 2],
            buffer[offset + 3],
            buffer[offset + 4],
            buffer[offset + 5],
            buffer[offset + 6],
            buffer[offset + 7],
        ]);
        let expires_at = Timestamp::from_nanos(expires_nanos as i64)
            .ok_or_else(|| SbeError::InvalidTimestamp("invalid expires_at".to_string()))?;
        offset += 8;

        // createdAt
        let created_nanos = u64::from_le_bytes([
            buffer[offset],
            buffer[offset + 1],
            buffer[offset + 2],
            buffer[offset + 3],
            buffer[offset + 4],
            buffer[offset + 5],
            buffer[offset + 6],
            buffer[offset + 7],
        ]);
        let created_at = Timestamp::from_nanos(created_nanos as i64)
            .ok_or_else(|| SbeError::InvalidTimestamp("invalid created_at".to_string()))?;

        // Skip to variable fields (after block_length from header end)
        let mut offset = MESSAGE_HEADER_SIZE + block_length as usize;

        // clientId
        let (client_id_str, client_size) = decode_var_string(&buffer[offset..])?;
        let client_id = CounterpartyId::new(client_id_str);
        offset += client_size;

        // symbol
        let (symbol_str, _symbol_size) = decode_var_string(&buffer[offset..])?;

        // Reconstruct the event
        use crate::domain::events::domain_event::EventMetadata;
        use crate::domain::value_objects::{AssetClass, Instrument, Symbol};

        let symbol = Symbol::new(&symbol_str)
            .map_err(|e| SbeError::InvalidFieldValue(format!("invalid symbol: {e}")))?;
        let instrument = Instrument::builder(symbol, AssetClass::CryptoSpot).build();

        let mut metadata = EventMetadata::for_rfq(rfq_id);
        metadata.event_id = event_id;
        metadata.timestamp = created_at;

        Ok(RfqCreated {
            metadata,
            client_id,
            instrument,
            side,
            quantity,
            expires_at,
        })
    }
}

// ============================================================================
// QuoteReceived Codec
// ============================================================================

/// Codec for QuoteReceived messages.
pub struct QuoteReceivedCodec;

impl QuoteReceivedCodec {
    /// Fixed block length.
    /// eventId(16) + rfqId(16) + quoteId(16) + price(9) + quantity(9) + validUntil(8) + receivedAt(8)
    const BLOCK_LENGTH: u16 = 16 + 16 + 16 + 9 + 9 + 8 + 8; // 82 bytes
}

impl SbeEncode for QuoteReceived {
    fn encoded_size(&self) -> usize {
        MESSAGE_HEADER_SIZE
            + QuoteReceivedCodec::BLOCK_LENGTH as usize
            + 4
            + self.venue_id.as_str().len()
    }

    fn encode(&self, buffer: &mut [u8]) -> SbeResult<usize> {
        let size = self.encoded_size();
        if buffer.len() < size {
            return Err(SbeError::BufferTooSmall {
                needed: size,
                available: buffer.len(),
            });
        }

        let mut offset = 0;

        // Header
        encode_header(
            &mut buffer[offset..],
            QuoteReceivedCodec::BLOCK_LENGTH,
            QUOTE_RECEIVED_TEMPLATE_ID,
        )?;
        offset += MESSAGE_HEADER_SIZE;

        // eventId
        let event_uuid = SbeUuid::from_uuid(self.metadata.event_id.get());
        event_uuid.encode(&mut buffer[offset..])?;
        offset += SbeUuid::SIZE;

        // rfqId
        if let Some(rfq_id) = self.metadata.rfq_id {
            let rfq_uuid = SbeUuid::from_uuid(rfq_id.get());
            rfq_uuid.encode(&mut buffer[offset..])?;
        } else {
            buffer[offset..offset + SbeUuid::SIZE].fill(0);
        }
        offset += SbeUuid::SIZE;

        // quoteId
        let quote_uuid = SbeUuid::from_uuid(self.quote_id.get());
        quote_uuid.encode(&mut buffer[offset..])?;
        offset += SbeUuid::SIZE;

        // price
        let price_decimal = SbeDecimal::from_decimal(self.price.get());
        price_decimal.encode(&mut buffer[offset..])?;
        offset += SbeDecimal::SIZE;

        // quantity
        let qty_decimal = SbeDecimal::from_decimal(self.quantity.get());
        qty_decimal.encode(&mut buffer[offset..])?;
        offset += SbeDecimal::SIZE;

        // validUntil
        let valid_nanos = self.valid_until.timestamp_nanos().unwrap_or(0) as u64;
        buffer[offset..offset + 8].copy_from_slice(&valid_nanos.to_le_bytes());
        offset += 8;

        // receivedAt (use metadata timestamp)
        let received_nanos = self.metadata.timestamp.timestamp_nanos().unwrap_or(0) as u64;
        buffer[offset..offset + 8].copy_from_slice(&received_nanos.to_le_bytes());
        offset += 8;

        // venueId (variable)
        let venue_size = encode_var_string(self.venue_id.as_str(), &mut buffer[offset..])?;
        offset += venue_size;

        Ok(offset)
    }
}

impl SbeDecode for QuoteReceived {
    fn decode(buffer: &[u8]) -> SbeResult<Self> {
        let (block_length, template_id, _schema_id, _version) = decode_header(buffer)?;

        if template_id != QUOTE_RECEIVED_TEMPLATE_ID {
            return Err(SbeError::UnknownTemplateId(template_id));
        }

        let mut offset = MESSAGE_HEADER_SIZE;

        // eventId
        let event_uuid = SbeUuid::decode(&buffer[offset..])?;
        let event_id = EventId::new(event_uuid.to_uuid());
        offset += SbeUuid::SIZE;

        // rfqId
        let rfq_uuid = SbeUuid::decode(&buffer[offset..])?;
        let rfq_id = RfqId::new(rfq_uuid.to_uuid());
        offset += SbeUuid::SIZE;

        // quoteId
        let quote_uuid = SbeUuid::decode(&buffer[offset..])?;
        let quote_id = QuoteId::new(quote_uuid.to_uuid());
        offset += SbeUuid::SIZE;

        // price
        let price_sbe = SbeDecimal::decode(&buffer[offset..])?;
        let price = Price::from_decimal(price_sbe.to_decimal()?)
            .map_err(|e| SbeError::InvalidDecimal(e.to_string()))?;
        offset += SbeDecimal::SIZE;

        // quantity
        let qty_sbe = SbeDecimal::decode(&buffer[offset..])?;
        let quantity = Quantity::from_decimal(qty_sbe.to_decimal()?)
            .map_err(|e| SbeError::InvalidDecimal(e.to_string()))?;
        offset += SbeDecimal::SIZE;

        // validUntil
        let valid_nanos = u64::from_le_bytes([
            buffer[offset],
            buffer[offset + 1],
            buffer[offset + 2],
            buffer[offset + 3],
            buffer[offset + 4],
            buffer[offset + 5],
            buffer[offset + 6],
            buffer[offset + 7],
        ]);
        let valid_until = Timestamp::from_nanos(valid_nanos as i64)
            .ok_or_else(|| SbeError::InvalidTimestamp("invalid valid_until".to_string()))?;
        offset += 8;

        // receivedAt
        let received_nanos = u64::from_le_bytes([
            buffer[offset],
            buffer[offset + 1],
            buffer[offset + 2],
            buffer[offset + 3],
            buffer[offset + 4],
            buffer[offset + 5],
            buffer[offset + 6],
            buffer[offset + 7],
        ]);
        let received_at = Timestamp::from_nanos(received_nanos as i64)
            .ok_or_else(|| SbeError::InvalidTimestamp("invalid received_at".to_string()))?;

        // Skip to variable fields
        let offset = MESSAGE_HEADER_SIZE + block_length as usize;

        // venueId
        let (venue_id_str, _venue_size) = decode_var_string(&buffer[offset..])?;
        let venue_id = VenueId::new(venue_id_str);

        // Reconstruct
        use crate::domain::events::domain_event::EventMetadata;

        let mut metadata = EventMetadata::for_rfq(rfq_id);
        metadata.event_id = event_id;
        metadata.timestamp = received_at;

        Ok(QuoteReceived {
            metadata,
            quote_id,
            venue_id,
            price,
            quantity,
            valid_until,
        })
    }
}

// ============================================================================
// TradeExecuted Codec
// ============================================================================

/// Codec for TradeExecuted messages.
pub struct TradeExecutedCodec;

impl TradeExecutedCodec {
    /// Fixed block length (bytes).
    ///
    /// Layout:
    /// - eventId(16) + rfqId(16) + tradeId(16) + quoteId(16) = 64
    /// - price(9) + quantity(9) + executedAt(8)               = 26
    /// - hasTakerFee(1) + takerFee(9)                         = 10
    /// - hasMakerFee(1) + makerFee(9)                         = 10
    /// - hasNetFee(1) + netFee(9)                             = 10
    /// - settlementMethod(1) + blockchain(1)                  =  2
    ///
    /// Total: 122 bytes
    const BLOCK_LENGTH: u16 = 64 + 26 + 10 + 10 + 10 + 2; // 122 bytes

    /// Byte size of an optional fee field on the wire: presence flag + `SbeDecimal`.
    const OPTIONAL_FEE_SIZE: usize = 1 + SbeDecimal::SIZE; // 10 bytes

    /// Byte size of the settlement method field on the wire: method tag + blockchain tag.
    const SETTLEMENT_SIZE: usize = 2;
}

impl SbeEncode for TradeExecuted {
    fn encoded_size(&self) -> usize {
        MESSAGE_HEADER_SIZE
            + TradeExecutedCodec::BLOCK_LENGTH as usize
            + 4
            + self.venue_id.as_str().len()
            + 4
            + self.counterparty_id.as_str().len()
    }

    fn encode(&self, buffer: &mut [u8]) -> SbeResult<usize> {
        let size = self.encoded_size();
        if buffer.len() < size {
            return Err(SbeError::BufferTooSmall {
                needed: size,
                available: buffer.len(),
            });
        }

        let mut offset = 0;

        // Header
        encode_header(
            &mut buffer[offset..],
            TradeExecutedCodec::BLOCK_LENGTH,
            TRADE_EXECUTED_TEMPLATE_ID,
        )?;
        offset += MESSAGE_HEADER_SIZE;

        // eventId
        let event_uuid = SbeUuid::from_uuid(self.metadata.event_id.get());
        event_uuid.encode(&mut buffer[offset..])?;
        offset += SbeUuid::SIZE;

        // rfqId
        if let Some(rfq_id) = self.metadata.rfq_id {
            let rfq_uuid = SbeUuid::from_uuid(rfq_id.get());
            rfq_uuid.encode(&mut buffer[offset..])?;
        } else {
            buffer[offset..offset + SbeUuid::SIZE].fill(0);
        }
        offset += SbeUuid::SIZE;

        // tradeId
        let trade_uuid = SbeUuid::from_uuid(self.trade_id.get());
        trade_uuid.encode(&mut buffer[offset..])?;
        offset += SbeUuid::SIZE;

        // quoteId
        let quote_uuid = SbeUuid::from_uuid(self.quote_id.get());
        quote_uuid.encode(&mut buffer[offset..])?;
        offset += SbeUuid::SIZE;

        // price
        let price_decimal = SbeDecimal::from_decimal(self.price.get());
        price_decimal.encode(&mut buffer[offset..])?;
        offset += SbeDecimal::SIZE;

        // quantity
        let qty_decimal = SbeDecimal::from_decimal(self.quantity.get());
        qty_decimal.encode(&mut buffer[offset..])?;
        offset += SbeDecimal::SIZE;

        // executedAt
        let executed_nanos = self.metadata.timestamp.timestamp_nanos().unwrap_or(0) as u64;
        buffer[offset..offset + 8].copy_from_slice(&executed_nanos.to_le_bytes());
        offset += 8;

        // takerFee (presence flag + SbeDecimal)
        encode_optional_fee(self.taker_fee, &mut buffer[offset..])?;
        offset += TradeExecutedCodec::OPTIONAL_FEE_SIZE;

        // makerFee (presence flag + SbeDecimal)
        encode_optional_fee(self.maker_fee, &mut buffer[offset..])?;
        offset += TradeExecutedCodec::OPTIONAL_FEE_SIZE;

        // netFee (presence flag + SbeDecimal)
        encode_optional_fee(self.net_fee, &mut buffer[offset..])?;
        offset += TradeExecutedCodec::OPTIONAL_FEE_SIZE;

        // settlementMethod (method_tag + blockchain_tag)
        let settlement_bytes = encode_settlement_method(self.settlement_method);
        buffer[offset..offset + TradeExecutedCodec::SETTLEMENT_SIZE]
            .copy_from_slice(&settlement_bytes);
        offset += TradeExecutedCodec::SETTLEMENT_SIZE;

        // venueId (variable)
        let venue_size = encode_var_string(self.venue_id.as_str(), &mut buffer[offset..])?;
        offset += venue_size;

        // counterpartyId (variable)
        let cp_size = encode_var_string(self.counterparty_id.as_str(), &mut buffer[offset..])?;
        offset += cp_size;

        Ok(offset)
    }
}

impl SbeDecode for TradeExecuted {
    fn decode(buffer: &[u8]) -> SbeResult<Self> {
        let (block_length, template_id, _schema_id, _version) = decode_header(buffer)?;

        if template_id != TRADE_EXECUTED_TEMPLATE_ID {
            return Err(SbeError::UnknownTemplateId(template_id));
        }

        let mut offset = MESSAGE_HEADER_SIZE;

        // eventId
        let event_uuid = SbeUuid::decode(&buffer[offset..])?;
        let event_id = EventId::new(event_uuid.to_uuid());
        offset += SbeUuid::SIZE;

        // rfqId
        let rfq_uuid = SbeUuid::decode(&buffer[offset..])?;
        let rfq_id = RfqId::new(rfq_uuid.to_uuid());
        offset += SbeUuid::SIZE;

        // tradeId
        let trade_uuid = SbeUuid::decode(&buffer[offset..])?;
        let trade_id = TradeId::new(trade_uuid.to_uuid());
        offset += SbeUuid::SIZE;

        // quoteId
        let quote_uuid = SbeUuid::decode(&buffer[offset..])?;
        let quote_id = QuoteId::new(quote_uuid.to_uuid());
        offset += SbeUuid::SIZE;

        // price
        let price_sbe = SbeDecimal::decode(&buffer[offset..])?;
        let price = Price::from_decimal(price_sbe.to_decimal()?)
            .map_err(|e| SbeError::InvalidDecimal(e.to_string()))?;
        offset += SbeDecimal::SIZE;

        // quantity
        let qty_sbe = SbeDecimal::decode(&buffer[offset..])?;
        let quantity = Quantity::from_decimal(qty_sbe.to_decimal()?)
            .map_err(|e| SbeError::InvalidDecimal(e.to_string()))?;
        offset += SbeDecimal::SIZE;

        // executedAt
        let executed_nanos = u64::from_le_bytes([
            buffer[offset],
            buffer[offset + 1],
            buffer[offset + 2],
            buffer[offset + 3],
            buffer[offset + 4],
            buffer[offset + 5],
            buffer[offset + 6],
            buffer[offset + 7],
        ]);
        let executed_at = Timestamp::from_nanos(executed_nanos as i64)
            .ok_or_else(|| SbeError::InvalidTimestamp("invalid executed_at".to_string()))?;
        offset += 8;

        // takerFee, makerFee, netFee, settlementMethod
        // These fields were introduced in version 2 of the TradeExecuted schema.
        // For older versions, default fees to None and settlement_method to OffChain.
        let (taker_fee, maker_fee, net_fee, settlement_method) = if _version >= 2 {
            // takerFee
            let taker_fee = decode_optional_fee(&buffer[offset..])?;
            offset += TradeExecutedCodec::OPTIONAL_FEE_SIZE;

            // makerFee
            let maker_fee = decode_optional_fee(&buffer[offset..])?;
            offset += TradeExecutedCodec::OPTIONAL_FEE_SIZE;

            // netFee
            let net_fee = decode_optional_fee(&buffer[offset..])?;
            offset += TradeExecutedCodec::OPTIONAL_FEE_SIZE;

            // settlementMethod (method_tag + blockchain_tag)
            if offset + 2 > buffer.len() {
                return Err(SbeError::InvalidTimestamp(
                    "buffer too short for settlement_method".to_string(),
                ));
            }
            let settlement_method =
                decode_settlement_method(&[buffer[offset], buffer[offset + 1]])?;

            (taker_fee, maker_fee, net_fee, settlement_method)
        } else {
            (None, None, None, SettlementMethod::OffChain)
        };

        // Skip to variable fields (using block_length from header for forward compatibility)
        let mut offset = MESSAGE_HEADER_SIZE + block_length as usize;

        // venueId
        let (venue_id_str, venue_size) = decode_var_string(&buffer[offset..])?;
        let venue_id = VenueId::new(venue_id_str);
        offset += venue_size;

        // counterpartyId
        let (cp_id_str, _cp_size) = decode_var_string(&buffer[offset..])?;
        let counterparty_id = CounterpartyId::new(cp_id_str);

        // Reconstruct
        use crate::domain::events::domain_event::EventMetadata;

        let mut metadata = EventMetadata::for_rfq(rfq_id);
        metadata.event_id = event_id;
        metadata.timestamp = executed_at;

        Ok(TradeExecuted {
            metadata,
            trade_id,
            quote_id,
            venue_id,
            counterparty_id,
            price,
            quantity,
            settlement_method,
            taker_fee,
            maker_fee,
            net_fee,
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::domain::value_objects::{AssetClass, Instrument, Symbol};

    fn test_rfq_id() -> RfqId {
        RfqId::new_v4()
    }

    fn test_venue_id() -> VenueId {
        VenueId::new("test-venue")
    }

    fn test_client_id() -> CounterpartyId {
        CounterpartyId::new("test-client")
    }

    fn test_instrument() -> Instrument {
        Instrument::builder(Symbol::new("BTC/USD").unwrap(), AssetClass::CryptoSpot).build()
    }

    mod rfq_created {
        use super::*;

        #[test]
        fn roundtrip() {
            let event = RfqCreated::new(
                test_rfq_id(),
                test_client_id(),
                test_instrument(),
                OrderSide::Buy,
                Quantity::new(100.0).unwrap(),
                Timestamp::now().add_secs(300),
            );

            let encoded = event.encode_to_vec().unwrap();
            let decoded = RfqCreated::decode(&encoded).unwrap();

            assert_eq!(event.metadata.event_id, decoded.metadata.event_id);
            assert_eq!(event.metadata.rfq_id, decoded.metadata.rfq_id);
            assert_eq!(event.client_id, decoded.client_id);
            assert_eq!(event.side, decoded.side);
            assert_eq!(event.quantity, decoded.quantity);
        }
    }

    mod quote_received {
        use super::*;

        #[test]
        fn roundtrip() {
            let event = QuoteReceived::new(
                test_rfq_id(),
                QuoteId::new_v4(),
                test_venue_id(),
                Price::new(50000.0).unwrap(),
                Quantity::new(1.0).unwrap(),
                Timestamp::now().add_secs(60),
            );

            let encoded = event.encode_to_vec().unwrap();
            let decoded = QuoteReceived::decode(&encoded).unwrap();

            assert_eq!(event.metadata.event_id, decoded.metadata.event_id);
            assert_eq!(event.quote_id, decoded.quote_id);
            assert_eq!(event.venue_id, decoded.venue_id);
            assert_eq!(event.price, decoded.price);
            assert_eq!(event.quantity, decoded.quantity);
        }
    }

    mod trade_executed {
        use super::*;
        use crate::domain::value_objects::{Blockchain, SettlementMethod};
        use rust_decimal::Decimal;

        #[test]
        #[allow(deprecated)]
        fn roundtrip_no_fees() {
            let event = TradeExecuted::new(
                test_rfq_id(),
                TradeId::new_v4(),
                QuoteId::new_v4(),
                test_venue_id(),
                test_client_id(),
                Price::new(50000.0).unwrap(),
                Quantity::new(1.0).unwrap(),
                SettlementMethod::OffChain,
            );

            let encoded = event.encode_to_vec().unwrap();
            let decoded = TradeExecuted::decode(&encoded).unwrap();

            assert_eq!(event.metadata.event_id, decoded.metadata.event_id);
            assert_eq!(event.trade_id, decoded.trade_id);
            assert_eq!(event.quote_id, decoded.quote_id);
            assert_eq!(event.venue_id, decoded.venue_id);
            assert_eq!(event.counterparty_id, decoded.counterparty_id);
            assert_eq!(event.price, decoded.price);
            assert_eq!(event.quantity, decoded.quantity);
            assert_eq!(event.settlement_method, decoded.settlement_method);
            assert_eq!(decoded.taker_fee, None);
            assert_eq!(decoded.maker_fee, None);
            assert_eq!(decoded.net_fee, None);
        }

        #[test]
        fn roundtrip_with_all_fees() {
            let event = TradeExecuted::builder()
                .rfq_id(test_rfq_id())
                .trade_id(TradeId::new_v4())
                .quote_id(QuoteId::new_v4())
                .venue_id(test_venue_id())
                .counterparty_id(test_client_id())
                .price(Price::new(50000.0).unwrap())
                .quantity(Quantity::new(1.0).unwrap())
                .settlement_method(SettlementMethod::OnChain(Blockchain::Ethereum))
                .taker_fee(Decimal::new(50, 2)) // 0.50
                .maker_fee(Decimal::new(25, 2)) // 0.25
                .net_fee(Decimal::new(75, 2)) // 0.75
                .build();

            let encoded = event.encode_to_vec().unwrap();
            let decoded = TradeExecuted::decode(&encoded).unwrap();

            assert_eq!(event.settlement_method, decoded.settlement_method);
            assert_eq!(event.taker_fee, decoded.taker_fee);
            assert_eq!(event.maker_fee, decoded.maker_fee);
            assert_eq!(event.net_fee, decoded.net_fee);
        }

        #[test]
        fn roundtrip_partial_fees() {
            let event = TradeExecuted::builder()
                .rfq_id(test_rfq_id())
                .trade_id(TradeId::new_v4())
                .quote_id(QuoteId::new_v4())
                .venue_id(test_venue_id())
                .counterparty_id(test_client_id())
                .price(Price::new(50000.0).unwrap())
                .quantity(Quantity::new(1.0).unwrap())
                .settlement_method(SettlementMethod::OffChain)
                .taker_fee(Decimal::new(100, 2)) // only taker_fee set
                .build();

            let encoded = event.encode_to_vec().unwrap();
            let decoded = TradeExecuted::decode(&encoded).unwrap();

            assert_eq!(event.taker_fee, decoded.taker_fee);
            assert_eq!(decoded.maker_fee, None);
            assert_eq!(decoded.net_fee, None);
            assert_eq!(decoded.settlement_method, SettlementMethod::OffChain);
        }

        #[test]
        fn roundtrip_all_blockchain_variants() {
            let chains = [
                Blockchain::Ethereum,
                Blockchain::Polygon,
                Blockchain::Arbitrum,
                Blockchain::Optimism,
                Blockchain::Base,
            ];
            for chain in chains {
                let event = TradeExecuted::builder()
                    .rfq_id(test_rfq_id())
                    .trade_id(TradeId::new_v4())
                    .quote_id(QuoteId::new_v4())
                    .venue_id(test_venue_id())
                    .counterparty_id(test_client_id())
                    .price(Price::new(50000.0).unwrap())
                    .quantity(Quantity::new(1.0).unwrap())
                    .settlement_method(SettlementMethod::OnChain(chain))
                    .build();

                let encoded = event.encode_to_vec().unwrap();
                let decoded = TradeExecuted::decode(&encoded).unwrap();
                assert_eq!(
                    decoded.settlement_method,
                    SettlementMethod::OnChain(chain),
                    "failed for blockchain {chain:?}"
                );
            }
        }
    }
}
