//! # SBE API Types
//!
//! Request and response types for SBE client API.
//!
//! These types represent the wire format for SBE messages and provide
//! encoding/decoding implementations using the infrastructure SBE helpers.

// Buffer bounds are validated before indexing in all functions
#![allow(clippy::indexing_slicing)]

use crate::domain::value_objects::enums::{AssetClass, VenueType};
use crate::domain::value_objects::timestamp::Timestamp;
use crate::domain::value_objects::{OrderSide, Price, Quantity, RfqState};
use crate::infrastructure::sbe::error::SbeResult;
use crate::infrastructure::sbe::types::{
    SbeDecimal, SbeUuid, decode_var_string, encode_var_string,
};
use crate::infrastructure::sbe::{SbeDecode, SbeEncode, SbeError};
use uuid::Uuid;

/// Message header size in bytes.
pub const MESSAGE_HEADER_SIZE: usize = 8;

/// Schema ID for OTC RFQ SBE messages.
pub const SCHEMA_ID: u16 = 1;

/// Schema version.
pub const SCHEMA_VERSION: u16 = 1;

// ============================================================================
// Template ID Constants
// ============================================================================

/// Template ID for CreateRfqRequest.
pub const CREATE_RFQ_REQUEST_TEMPLATE_ID: u16 = 20;

/// Template ID for CreateRfqResponse.
pub const CREATE_RFQ_RESPONSE_TEMPLATE_ID: u16 = 21;

/// Template ID for GetRfqRequest.
pub const GET_RFQ_REQUEST_TEMPLATE_ID: u16 = 22;

/// Template ID for GetRfqResponse.
pub const GET_RFQ_RESPONSE_TEMPLATE_ID: u16 = 23;

/// Template ID for CancelRfqRequest.
pub const CANCEL_RFQ_REQUEST_TEMPLATE_ID: u16 = 24;

/// Template ID for CancelRfqResponse.
pub const CANCEL_RFQ_RESPONSE_TEMPLATE_ID: u16 = 25;

/// Template ID for ExecuteTradeRequest.
pub const EXECUTE_TRADE_REQUEST_TEMPLATE_ID: u16 = 26;

/// Template ID for ExecuteTradeResponse.
pub const EXECUTE_TRADE_RESPONSE_TEMPLATE_ID: u16 = 27;

/// Template ID for SubscribeQuotesRequest.
pub const SUBSCRIBE_QUOTES_REQUEST_TEMPLATE_ID: u16 = 30;

/// Template ID for QuoteUpdate.
pub const QUOTE_UPDATE_TEMPLATE_ID: u16 = 31;

/// Template ID for SubscribeRfqStatusRequest.
pub const SUBSCRIBE_RFQ_STATUS_REQUEST_TEMPLATE_ID: u16 = 32;

/// Template ID for RfqStatusUpdate.
pub const RFQ_STATUS_UPDATE_TEMPLATE_ID: u16 = 33;

/// Template ID for UnsubscribeRequest.
pub const UNSUBSCRIBE_REQUEST_TEMPLATE_ID: u16 = 40;

/// Template ID for ErrorResponse.
pub const ERROR_RESPONSE_TEMPLATE_ID: u16 = 50;

// ============================================================================
// Helper Functions
// ============================================================================

/// Encodes the SBE message header.
#[inline]
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
#[inline]
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

/// Encodes AssetClass to SBE enum value.
#[inline]
#[must_use]
fn encode_asset_class(class: AssetClass) -> u8 {
    match class {
        AssetClass::CryptoSpot => 0,
        AssetClass::CryptoDerivs => 1,
        AssetClass::Stock => 2,
        AssetClass::Forex => 3,
        AssetClass::Commodity => 4,
    }
}

/// Decodes AssetClass from SBE enum value.
#[inline]
fn decode_asset_class(value: u8) -> SbeResult<AssetClass> {
    match value {
        0 => Ok(AssetClass::CryptoSpot),
        1 => Ok(AssetClass::CryptoDerivs),
        2 => Ok(AssetClass::Stock),
        3 => Ok(AssetClass::Forex),
        4 => Ok(AssetClass::Commodity),
        _ => Err(SbeError::InvalidEnumValue(value)),
    }
}

/// Encodes RfqState to SBE enum value.
#[inline]
#[must_use]
fn encode_rfq_state(state: RfqState) -> u8 {
    match state {
        RfqState::Created => 0,
        RfqState::QuoteRequesting => 1,
        RfqState::QuotesReceived => 2,
        RfqState::ClientSelecting => 3,
        RfqState::Executing => 4,
        RfqState::Executed => 5,
        RfqState::Failed => 6,
        RfqState::Cancelled => 7,
        RfqState::Expired => 8,
        RfqState::Negotiating => 9,
    }
}

/// Decodes RfqState from SBE enum value.
#[inline]
fn decode_rfq_state(value: u8) -> SbeResult<RfqState> {
    match value {
        0 => Ok(RfqState::Created),
        1 => Ok(RfqState::QuoteRequesting),
        2 => Ok(RfqState::QuotesReceived),
        3 => Ok(RfqState::ClientSelecting),
        4 => Ok(RfqState::Executing),
        5 => Ok(RfqState::Executed),
        6 => Ok(RfqState::Failed),
        7 => Ok(RfqState::Cancelled),
        8 => Ok(RfqState::Expired),
        9 => Ok(RfqState::Negotiating),
        _ => Err(SbeError::InvalidEnumValue(value)),
    }
}

/// Encodes VenueType to SBE enum value.
#[inline]
#[must_use]
fn encode_venue_type(vtype: VenueType) -> u8 {
    match vtype {
        VenueType::InternalMM => 0,
        VenueType::ExternalMM => 1,
        VenueType::DexAggregator => 2,
        VenueType::Protocol => 3,
        VenueType::RfqProtocol => 4,
    }
}

/// Decodes VenueType from SBE enum value.
#[inline]
fn decode_venue_type(value: u8) -> SbeResult<VenueType> {
    match value {
        0 => Ok(VenueType::InternalMM),
        1 => Ok(VenueType::ExternalMM),
        2 => Ok(VenueType::DexAggregator),
        3 => Ok(VenueType::Protocol),
        4 => Ok(VenueType::RfqProtocol),
        _ => Err(SbeError::InvalidEnumValue(value)),
    }
}

/// Calculates the size of a variable-length string field.
#[inline]
#[must_use]
fn var_string_size(s: &str) -> usize {
    4 + s.len() // 4-byte length prefix + UTF-8 data
}

// ============================================================================
// Request Types
// ============================================================================

/// CreateRfqRequest - Create a new RFQ.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateRfqRequest {
    /// Request correlation ID.
    pub request_id: Uuid,
    /// Client identifier.
    pub client_id: String,
    /// Trading symbol.
    pub symbol: String,
    /// Base asset.
    pub base_asset: String,
    /// Quote asset.
    pub quote_asset: String,
    /// Order side.
    pub side: OrderSide,
    /// Requested quantity.
    pub quantity: Quantity,
    /// RFQ timeout in seconds.
    pub timeout_seconds: i64,
    /// Asset class.
    pub asset_class: AssetClass,
}

impl CreateRfqRequest {
    /// Block length for CreateRfqRequest (fixed fields only).
    pub const BLOCK_LENGTH: u16 = 35;
}

impl SbeEncode for CreateRfqRequest {
    fn encoded_size(&self) -> usize {
        MESSAGE_HEADER_SIZE
            + Self::BLOCK_LENGTH as usize
            + var_string_size(&self.client_id)
            + var_string_size(&self.symbol)
            + var_string_size(&self.base_asset)
            + var_string_size(&self.quote_asset)
    }

    fn encode(&self, buffer: &mut [u8]) -> SbeResult<usize> {
        let size = self.encoded_size();
        if buffer.len() < size {
            return Err(SbeError::BufferTooSmall {
                needed: size,
                available: buffer.len(),
            });
        }

        let mut offset: usize = 0;

        // Header
        encode_header(buffer, Self::BLOCK_LENGTH, CREATE_RFQ_REQUEST_TEMPLATE_ID)?;
        offset =
            offset
                .checked_add(MESSAGE_HEADER_SIZE)
                .ok_or(SbeError::BufferTooSmall {
                    needed: size,
                    available: buffer.len(),
                })?;

        // requestId
        let request_uuid = SbeUuid::from_uuid(self.request_id);
        request_uuid.encode(&mut buffer[offset..])?;
        offset = offset
            .checked_add(SbeUuid::SIZE)
            .ok_or(SbeError::BufferTooSmall {
                needed: size,
                available: buffer.len(),
            })?;

        // side
        buffer[offset] = encode_order_side(self.side);
        offset = offset
            .checked_add(1)
            .ok_or(SbeError::BufferTooSmall {
                needed: size,
                available: buffer.len(),
            })?;

        // quantity
        let qty_decimal = SbeDecimal::from_decimal(self.quantity.get());
        qty_decimal.encode(&mut buffer[offset..])?;
        offset = offset
            .checked_add(SbeDecimal::SIZE)
            .ok_or(SbeError::BufferTooSmall {
                needed: size,
                available: buffer.len(),
            })?;

        // timeoutSeconds
        buffer[offset..offset.checked_add(8).ok_or(SbeError::Overflow)?]
            .copy_from_slice(&self.timeout_seconds.to_le_bytes());
        offset = offset
            .checked_add(8)
            .ok_or(SbeError::BufferTooSmall {
                needed: size,
                available: buffer.len(),
            })?;

        // assetClass
        buffer[offset] = encode_asset_class(self.asset_class);
        offset = offset
            .checked_add(1)
            .ok_or(SbeError::BufferTooSmall {
                needed: size,
                available: buffer.len(),
            })?;

        // Variable fields
        let client_size = encode_var_string(&self.client_id, &mut buffer[offset..])?;
        offset = offset
            .checked_add(client_size)
            .ok_or(SbeError::BufferTooSmall {
                needed: size,
                available: buffer.len(),
            })?;

        let symbol_size = encode_var_string(&self.symbol, &mut buffer[offset..])?;
        offset = offset
            .checked_add(symbol_size)
            .ok_or(SbeError::BufferTooSmall {
                needed: size,
                available: buffer.len(),
            })?;

        let base_size = encode_var_string(&self.base_asset, &mut buffer[offset..])?;
        offset = offset
            .checked_add(base_size)
            .ok_or(SbeError::BufferTooSmall {
                needed: size,
                available: buffer.len(),
            })?;

        let quote_size = encode_var_string(&self.quote_asset, &mut buffer[offset..])?;
        offset = offset
            .checked_add(quote_size)
            .ok_or(SbeError::BufferTooSmall {
                needed: size,
                available: buffer.len(),
            })?;

        Ok(offset)
    }
}

impl SbeDecode for CreateRfqRequest {
    fn decode(buffer: &[u8]) -> SbeResult<Self> {
        let (_block_length, template_id, _schema_id, _version) = decode_header(buffer)?;

        if template_id != CREATE_RFQ_REQUEST_TEMPLATE_ID {
            return Err(SbeError::UnknownTemplateId(template_id));
        }

        let mut offset: usize = MESSAGE_HEADER_SIZE;

        // requestId
        let request_uuid = SbeUuid::decode(&buffer[offset..])?;
        offset = offset
            .checked_add(SbeUuid::SIZE)
            .ok_or(SbeError::Overflow)?;

        // side
        let side = decode_order_side(buffer[offset])?;
        offset = offset.checked_add(1).ok_or(SbeError::Overflow)?;

        // quantity
        let qty_sbe = SbeDecimal::decode(&buffer[offset..])?;
        let qty_decimal = qty_sbe.to_decimal()?;
        let quantity = Quantity::from_decimal(qty_decimal)
            .map_err(|e| SbeError::InvalidFieldValue(format!("invalid quantity: {}", e)))?;
        offset = offset
            .checked_add(SbeDecimal::SIZE)
            .ok_or(SbeError::Overflow)?;

        // timeoutSeconds
        let timeout_bytes = buffer
            .get(offset..offset.checked_add(8).ok_or(SbeError::Overflow)?)
            .ok_or_else(|| SbeError::BufferTooSmall {
                needed: offset + 8,
                available: buffer.len(),
            })?;
        let timeout_seconds = i64::from_le_bytes(
            timeout_bytes
                .try_into()
                .map_err(|_| SbeError::InvalidFieldValue("invalid timeout".to_string()))?,
        );
        offset = offset.checked_add(8).ok_or(SbeError::Overflow)?;

        // assetClass
        let asset_class = decode_asset_class(buffer[offset])?;
        offset = offset.checked_add(1).ok_or(SbeError::Overflow)?;

        // Variable fields
        let (client_id, client_size) = decode_var_string(&buffer[offset..])?;
        offset = offset
            .checked_add(client_size)
            .ok_or(SbeError::Overflow)?;

        let (symbol, symbol_size) = decode_var_string(&buffer[offset..])?;
        offset = offset
            .checked_add(symbol_size)
            .ok_or(SbeError::Overflow)?;

        let (base_asset, base_size) = decode_var_string(&buffer[offset..])?;
        offset = offset
            .checked_add(base_size)
            .ok_or(SbeError::Overflow)?;

        let (quote_asset, _quote_size) = decode_var_string(&buffer[offset..])?;

        Ok(Self {
            request_id: request_uuid.to_uuid(),
            client_id,
            symbol,
            base_asset,
            quote_asset,
            side,
            quantity,
            timeout_seconds,
            asset_class,
        })
    }
}

/// GetRfqRequest - Retrieve an RFQ by ID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetRfqRequest {
    /// Request correlation ID.
    pub request_id: Uuid,
    /// RFQ identifier to retrieve.
    pub rfq_id: Uuid,
}

impl GetRfqRequest {
    /// Block length for GetRfqRequest (fixed fields only).
    pub const BLOCK_LENGTH: u16 = 32;

    /// Creates a new GetRfqRequest.
    #[must_use]
    pub fn new(request_id: Uuid, rfq_id: Uuid) -> Self {
        Self { request_id, rfq_id }
    }
}

impl SbeEncode for GetRfqRequest {
    fn encoded_size(&self) -> usize {
        MESSAGE_HEADER_SIZE + Self::BLOCK_LENGTH as usize
    }

    fn encode(&self, buffer: &mut [u8]) -> SbeResult<usize> {
        let size = self.encoded_size();
        if buffer.len() < size {
            return Err(SbeError::BufferTooSmall {
                needed: size,
                available: buffer.len(),
            });
        }

        let mut offset: usize = 0;

        // Header
        encode_header(buffer, Self::BLOCK_LENGTH, GET_RFQ_REQUEST_TEMPLATE_ID)?;
        offset =
            offset
                .checked_add(MESSAGE_HEADER_SIZE)
                .ok_or(SbeError::BufferTooSmall {
                    needed: MESSAGE_HEADER_SIZE,
                    available: 0,
                })?;

        // requestId
        let request_uuid = SbeUuid::from_uuid(self.request_id);
        request_uuid.encode(&mut buffer[offset..])?;
        offset = offset
            .checked_add(SbeUuid::SIZE)
            .ok_or(SbeError::BufferTooSmall {
                needed: size,
                available: buffer.len(),
            })?;

        // rfqId
        let rfq_uuid = SbeUuid::from_uuid(self.rfq_id);
        rfq_uuid.encode(&mut buffer[offset..])?;
        offset = offset
            .checked_add(SbeUuid::SIZE)
            .ok_or(SbeError::BufferTooSmall {
                needed: size,
                available: buffer.len(),
            })?;

        Ok(offset)
    }
}

impl SbeDecode for GetRfqRequest {
    fn decode(buffer: &[u8]) -> SbeResult<Self> {
        let (_block_length, template_id, _schema_id, _version) = decode_header(buffer)?;

        if template_id != GET_RFQ_REQUEST_TEMPLATE_ID {
            return Err(SbeError::UnknownTemplateId(template_id));
        }

        let mut offset: usize = MESSAGE_HEADER_SIZE;

        // requestId
        let request_uuid = SbeUuid::decode(&buffer[offset..])?;
        offset = offset
            .checked_add(SbeUuid::SIZE)
            .ok_or(SbeError::Overflow)?;

        // rfqId
        let rfq_uuid = SbeUuid::decode(&buffer[offset..])?;
        let _ = offset
            .checked_add(SbeUuid::SIZE)
            .ok_or(SbeError::Overflow)?;

        Ok(Self {
            request_id: request_uuid.to_uuid(),
            rfq_id: rfq_uuid.to_uuid(),
        })
    }
}

/// ExecuteTradeRequest - Execute a trade with a selected quote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecuteTradeRequest {
    /// RFQ identifier.
    pub rfq_id: Uuid,
    /// Selected quote identifier.
    pub quote_id: Uuid,
}

impl ExecuteTradeRequest {
    /// Block length for ExecuteTradeRequest.
    pub const BLOCK_LENGTH: u16 = 32;

    /// Creates a new ExecuteTradeRequest.
    #[must_use]
    pub fn new(rfq_id: Uuid, quote_id: Uuid) -> Self {
        Self { rfq_id, quote_id }
    }
}

impl SbeEncode for ExecuteTradeRequest {
    fn encoded_size(&self) -> usize {
        MESSAGE_HEADER_SIZE + Self::BLOCK_LENGTH as usize
    }

    fn encode(&self, buffer: &mut [u8]) -> SbeResult<usize> {
        let size = self.encoded_size();
        if buffer.len() < size {
            return Err(SbeError::BufferTooSmall {
                needed: size,
                available: buffer.len(),
            });
        }

        let mut offset: usize = 0;

        // Header
        encode_header(
            buffer,
            Self::BLOCK_LENGTH,
            EXECUTE_TRADE_REQUEST_TEMPLATE_ID,
        )?;
        offset =
            offset
                .checked_add(MESSAGE_HEADER_SIZE)
                .ok_or(SbeError::BufferTooSmall {
                    needed: size,
                    available: buffer.len(),
                })?;

        // rfqId
        let rfq_uuid = SbeUuid::from_uuid(self.rfq_id);
        rfq_uuid.encode(&mut buffer[offset..])?;
        offset = offset
            .checked_add(SbeUuid::SIZE)
            .ok_or(SbeError::BufferTooSmall {
                needed: size,
                available: buffer.len(),
            })?;

        // quoteId
        let quote_uuid = SbeUuid::from_uuid(self.quote_id);
        quote_uuid.encode(&mut buffer[offset..])?;
        offset = offset
            .checked_add(SbeUuid::SIZE)
            .ok_or(SbeError::BufferTooSmall {
                needed: size,
                available: buffer.len(),
            })?;

        Ok(offset)
    }
}

impl SbeDecode for ExecuteTradeRequest {
    fn decode(buffer: &[u8]) -> SbeResult<Self> {
        let (_block_length, template_id, _schema_id, _version) = decode_header(buffer)?;

        if template_id != EXECUTE_TRADE_REQUEST_TEMPLATE_ID {
            return Err(SbeError::UnknownTemplateId(template_id));
        }

        let mut offset: usize = MESSAGE_HEADER_SIZE;

        // rfqId
        let rfq_uuid = SbeUuid::decode(&buffer[offset..])?;
        offset = offset
            .checked_add(SbeUuid::SIZE)
            .ok_or(SbeError::Overflow)?;

        // quoteId
        let quote_uuid = SbeUuid::decode(&buffer[offset..])?;
        let _ = offset
            .checked_add(SbeUuid::SIZE)
            .ok_or(SbeError::Overflow)?;

        Ok(Self {
            rfq_id: rfq_uuid.to_uuid(),
            quote_id: quote_uuid.to_uuid(),
        })
    }
}

/// SubscribeQuotesRequest - Subscribe to quote updates for an RFQ.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubscribeQuotesRequest {
    /// RFQ identifier to subscribe to.
    pub rfq_id: Uuid,
}

impl SubscribeQuotesRequest {
    /// Block length for SubscribeQuotesRequest.
    pub const BLOCK_LENGTH: u16 = 16;

    /// Creates a new SubscribeQuotesRequest.
    #[must_use]
    pub fn new(rfq_id: Uuid) -> Self {
        Self { rfq_id }
    }
}

impl SbeEncode for SubscribeQuotesRequest {
    fn encoded_size(&self) -> usize {
        MESSAGE_HEADER_SIZE + Self::BLOCK_LENGTH as usize
    }

    fn encode(&self, buffer: &mut [u8]) -> SbeResult<usize> {
        let size = self.encoded_size();
        if buffer.len() < size {
            return Err(SbeError::BufferTooSmall {
                needed: size,
                available: buffer.len(),
            });
        }

        let mut offset: usize = 0;

        // Header
        encode_header(
            buffer,
            Self::BLOCK_LENGTH,
            SUBSCRIBE_QUOTES_REQUEST_TEMPLATE_ID,
        )?;
        offset =
            offset
                .checked_add(MESSAGE_HEADER_SIZE)
                .ok_or(SbeError::BufferTooSmall {
                    needed: size,
                    available: buffer.len(),
                })?;

        // rfqId
        let rfq_uuid = SbeUuid::from_uuid(self.rfq_id);
        rfq_uuid.encode(&mut buffer[offset..])?;
        offset = offset
            .checked_add(SbeUuid::SIZE)
            .ok_or(SbeError::BufferTooSmall {
                needed: size,
                available: buffer.len(),
            })?;

        Ok(offset)
    }
}

impl SbeDecode for SubscribeQuotesRequest {
    fn decode(buffer: &[u8]) -> SbeResult<Self> {
        let (_block_length, template_id, _schema_id, _version) = decode_header(buffer)?;

        if template_id != SUBSCRIBE_QUOTES_REQUEST_TEMPLATE_ID {
            return Err(SbeError::UnknownTemplateId(template_id));
        }

        let offset = MESSAGE_HEADER_SIZE;

        // rfqId
        let rfq_uuid = SbeUuid::decode(&buffer[offset..])?;

        Ok(Self {
            rfq_id: rfq_uuid.to_uuid(),
        })
    }
}

/// SubscribeRfqStatusRequest - Subscribe to RFQ status updates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubscribeRfqStatusRequest {
    /// RFQ identifier to subscribe to.
    pub rfq_id: Uuid,
}

impl SubscribeRfqStatusRequest {
    /// Block length for SubscribeRfqStatusRequest.
    pub const BLOCK_LENGTH: u16 = 16;

    /// Creates a new SubscribeRfqStatusRequest.
    #[must_use]
    pub fn new(rfq_id: Uuid) -> Self {
        Self { rfq_id }
    }
}

impl SbeEncode for SubscribeRfqStatusRequest {
    fn encoded_size(&self) -> usize {
        MESSAGE_HEADER_SIZE + Self::BLOCK_LENGTH as usize
    }

    fn encode(&self, buffer: &mut [u8]) -> SbeResult<usize> {
        let size = self.encoded_size();
        if buffer.len() < size {
            return Err(SbeError::BufferTooSmall {
                needed: size,
                available: buffer.len(),
            });
        }

        let mut offset: usize = 0;

        // Header
        encode_header(
            buffer,
            Self::BLOCK_LENGTH,
            SUBSCRIBE_RFQ_STATUS_REQUEST_TEMPLATE_ID,
        )?;
        offset =
            offset
                .checked_add(MESSAGE_HEADER_SIZE)
                .ok_or(SbeError::BufferTooSmall {
                    needed: size,
                    available: buffer.len(),
                })?;

        // rfqId
        let rfq_uuid = SbeUuid::from_uuid(self.rfq_id);
        rfq_uuid.encode(&mut buffer[offset..])?;
        offset = offset
            .checked_add(SbeUuid::SIZE)
            .ok_or(SbeError::BufferTooSmall {
                needed: size,
                available: buffer.len(),
            })?;

        Ok(offset)
    }
}

impl SbeDecode for SubscribeRfqStatusRequest {
    fn decode(buffer: &[u8]) -> SbeResult<Self> {
        let (_block_length, template_id, _schema_id, _version) = decode_header(buffer)?;

        if template_id != SUBSCRIBE_RFQ_STATUS_REQUEST_TEMPLATE_ID {
            return Err(SbeError::UnknownTemplateId(template_id));
        }

        let offset = MESSAGE_HEADER_SIZE;

        // rfqId
        let rfq_uuid = SbeUuid::decode(&buffer[offset..])?;

        Ok(Self {
            rfq_id: rfq_uuid.to_uuid(),
        })
    }
}

/// UnsubscribeRequest - Unsubscribe from updates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsubscribeRequest {
    /// RFQ identifier to unsubscribe from.
    pub rfq_id: Uuid,
}

impl UnsubscribeRequest {
    /// Block length for UnsubscribeRequest.
    pub const BLOCK_LENGTH: u16 = 16;

    /// Creates a new UnsubscribeRequest.
    #[must_use]
    pub fn new(rfq_id: Uuid) -> Self {
        Self { rfq_id }
    }
}

impl SbeEncode for UnsubscribeRequest {
    fn encoded_size(&self) -> usize {
        MESSAGE_HEADER_SIZE + Self::BLOCK_LENGTH as usize
    }

    fn encode(&self, buffer: &mut [u8]) -> SbeResult<usize> {
        let size = self.encoded_size();
        if buffer.len() < size {
            return Err(SbeError::BufferTooSmall {
                needed: size,
                available: buffer.len(),
            });
        }

        let mut offset: usize = 0;

        // Header
        encode_header(buffer, Self::BLOCK_LENGTH, UNSUBSCRIBE_REQUEST_TEMPLATE_ID)?;
        offset =
            offset
                .checked_add(MESSAGE_HEADER_SIZE)
                .ok_or(SbeError::BufferTooSmall {
                    needed: size,
                    available: buffer.len(),
                })?;

        // rfqId
        let rfq_uuid = SbeUuid::from_uuid(self.rfq_id);
        rfq_uuid.encode(&mut buffer[offset..])?;
        offset = offset
            .checked_add(SbeUuid::SIZE)
            .ok_or(SbeError::BufferTooSmall {
                needed: size,
                available: buffer.len(),
            })?;

        Ok(offset)
    }
}

impl SbeDecode for UnsubscribeRequest {
    fn decode(buffer: &[u8]) -> SbeResult<Self> {
        let (_block_length, template_id, _schema_id, _version) = decode_header(buffer)?;

        if template_id != UNSUBSCRIBE_REQUEST_TEMPLATE_ID {
            return Err(SbeError::UnknownTemplateId(template_id));
        }

        let offset = MESSAGE_HEADER_SIZE;

        // rfqId
        let rfq_uuid = SbeUuid::decode(&buffer[offset..])?;

        Ok(Self {
            rfq_id: rfq_uuid.to_uuid(),
        })
    }
}

/// CancelRfqRequest - Cancel an RFQ.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CancelRfqRequest {
    /// Request correlation ID.
    pub request_id: Uuid,
    /// RFQ identifier to cancel.
    pub rfq_id: Uuid,
    /// Cancellation reason.
    pub reason: String,
}

impl CancelRfqRequest {
    /// Block length for CancelRfqRequest (fixed fields only).
    pub const BLOCK_LENGTH: u16 = 32;
}

impl SbeEncode for CancelRfqRequest {
    fn encoded_size(&self) -> usize {
        MESSAGE_HEADER_SIZE + Self::BLOCK_LENGTH as usize + var_string_size(&self.reason)
    }

    fn encode(&self, buffer: &mut [u8]) -> SbeResult<usize> {
        let size = self.encoded_size();
        if buffer.len() < size {
            return Err(SbeError::BufferTooSmall {
                needed: size,
                available: buffer.len(),
            });
        }

        let mut offset: usize = 0;

        // Header
        encode_header(buffer, Self::BLOCK_LENGTH, CANCEL_RFQ_REQUEST_TEMPLATE_ID)?;
        offset =
            offset
                .checked_add(MESSAGE_HEADER_SIZE)
                .ok_or(SbeError::BufferTooSmall {
                    needed: size,
                    available: buffer.len(),
                })?;

        // requestId
        let request_uuid = SbeUuid::from_uuid(self.request_id);
        request_uuid.encode(&mut buffer[offset..])?;
        offset = offset
            .checked_add(SbeUuid::SIZE)
            .ok_or(SbeError::BufferTooSmall {
                needed: size,
                available: buffer.len(),
            })?;

        // rfqId
        let rfq_uuid = SbeUuid::from_uuid(self.rfq_id);
        rfq_uuid.encode(&mut buffer[offset..])?;
        offset = offset
            .checked_add(SbeUuid::SIZE)
            .ok_or(SbeError::BufferTooSmall {
                needed: size,
                available: buffer.len(),
            })?;

        // reason (variable)
        let reason_size = encode_var_string(&self.reason, &mut buffer[offset..])?;
        offset = offset
            .checked_add(reason_size)
            .ok_or(SbeError::BufferTooSmall {
                needed: size,
                available: buffer.len(),
            })?;

        Ok(offset)
    }
}

impl SbeDecode for CancelRfqRequest {
    fn decode(buffer: &[u8]) -> SbeResult<Self> {
        let (_block_length, template_id, _schema_id, _version) = decode_header(buffer)?;

        if template_id != CANCEL_RFQ_REQUEST_TEMPLATE_ID {
            return Err(SbeError::UnknownTemplateId(template_id));
        }

        let mut offset: usize = MESSAGE_HEADER_SIZE;

        // requestId
        let request_uuid = SbeUuid::decode(&buffer[offset..])?;
        offset = offset
            .checked_add(SbeUuid::SIZE)
            .ok_or(SbeError::Overflow)?;

        // rfqId
        let rfq_uuid = SbeUuid::decode(&buffer[offset..])?;
        offset = offset
            .checked_add(SbeUuid::SIZE)
            .ok_or(SbeError::Overflow)?;

        // reason (variable)
        let (reason, _reason_size) = decode_var_string(&buffer[offset..])?;

        Ok(Self {
            request_id: request_uuid.to_uuid(),
            rfq_id: rfq_uuid.to_uuid(),
            reason,
        })
    }
}

// ============================================================================
// Response Types
// ============================================================================

/// ExecuteTradeResponse - Response containing executed trade details.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecuteTradeResponse {
    /// Trade identifier.
    pub trade_id: Uuid,
    /// RFQ identifier.
    pub rfq_id: Uuid,
    /// Executed quote identifier.
    pub quote_id: Uuid,
    /// Execution price.
    pub price: Price,
    /// Executed quantity.
    pub quantity: Quantity,
    /// Trade execution time.
    pub created_at: Timestamp,
    /// Venue identifier.
    pub venue_id: String,
    /// Venue execution reference ID.
    pub venue_execution_ref: String,
}

impl ExecuteTradeResponse {
    /// Block length for ExecuteTradeResponse (fixed fields only).
    pub const BLOCK_LENGTH: u16 = 74;
}

impl SbeEncode for ExecuteTradeResponse {
    fn encoded_size(&self) -> usize {
        MESSAGE_HEADER_SIZE
            + Self::BLOCK_LENGTH as usize
            + var_string_size(&self.venue_id)
            + var_string_size(&self.venue_execution_ref)
    }

    fn encode(&self, buffer: &mut [u8]) -> SbeResult<usize> {
        let size = self.encoded_size();
        if buffer.len() < size {
            return Err(SbeError::BufferTooSmall {
                needed: size,
                available: buffer.len(),
            });
        }

        let mut offset: usize = 0;

        // Header
        encode_header(
            buffer,
            Self::BLOCK_LENGTH,
            EXECUTE_TRADE_RESPONSE_TEMPLATE_ID,
        )?;
        offset =
            offset
                .checked_add(MESSAGE_HEADER_SIZE)
                .ok_or(SbeError::BufferTooSmall {
                    needed: size,
                    available: buffer.len(),
                })?;

        // tradeId
        let trade_uuid = SbeUuid::from_uuid(self.trade_id);
        trade_uuid.encode(&mut buffer[offset..])?;
        offset = offset
            .checked_add(SbeUuid::SIZE)
            .ok_or(SbeError::BufferTooSmall {
                needed: size,
                available: buffer.len(),
            })?;

        // rfqId
        let rfq_uuid = SbeUuid::from_uuid(self.rfq_id);
        rfq_uuid.encode(&mut buffer[offset..])?;
        offset = offset
            .checked_add(SbeUuid::SIZE)
            .ok_or(SbeError::BufferTooSmall {
                needed: size,
                available: buffer.len(),
            })?;

        // quoteId
        let quote_uuid = SbeUuid::from_uuid(self.quote_id);
        quote_uuid.encode(&mut buffer[offset..])?;
        offset = offset
            .checked_add(SbeUuid::SIZE)
            .ok_or(SbeError::BufferTooSmall {
                needed: size,
                available: buffer.len(),
            })?;

        // price
        let price_decimal = SbeDecimal::from_decimal(self.price.get());
        price_decimal.encode(&mut buffer[offset..])?;
        offset = offset
            .checked_add(SbeDecimal::SIZE)
            .ok_or(SbeError::BufferTooSmall {
                needed: size,
                available: buffer.len(),
            })?;

        // quantity
        let qty_decimal = SbeDecimal::from_decimal(self.quantity.get());
        qty_decimal.encode(&mut buffer[offset..])?;
        offset = offset
            .checked_add(SbeDecimal::SIZE)
            .ok_or(SbeError::BufferTooSmall {
                needed: size,
                available: buffer.len(),
            })?;

        // createdAt (nanoseconds)
        let created_nanos = self.created_at.timestamp_nanos().unwrap_or(0) as u64;
        buffer[offset..offset.checked_add(8).ok_or(SbeError::Overflow)?]
            .copy_from_slice(&created_nanos.to_le_bytes());
        offset = offset
            .checked_add(8)
            .ok_or(SbeError::BufferTooSmall {
                needed: size,
                available: buffer.len(),
            })?;

        // Variable fields
        let venue_size = encode_var_string(&self.venue_id, &mut buffer[offset..])?;
        offset = offset
            .checked_add(venue_size)
            .ok_or(SbeError::BufferTooSmall {
                needed: size,
                available: buffer.len(),
            })?;

        let ref_size = encode_var_string(&self.venue_execution_ref, &mut buffer[offset..])?;
        offset = offset
            .checked_add(ref_size)
            .ok_or(SbeError::BufferTooSmall {
                needed: size,
                available: buffer.len(),
            })?;

        Ok(offset)
    }
}

impl SbeDecode for ExecuteTradeResponse {
    fn decode(buffer: &[u8]) -> SbeResult<Self> {
        let (_block_length, template_id, _schema_id, _version) = decode_header(buffer)?;

        if template_id != EXECUTE_TRADE_RESPONSE_TEMPLATE_ID {
            return Err(SbeError::UnknownTemplateId(template_id));
        }

        let mut offset: usize = MESSAGE_HEADER_SIZE;

        // tradeId
        let trade_uuid = SbeUuid::decode(&buffer[offset..])?;
        offset = offset
            .checked_add(SbeUuid::SIZE)
            .ok_or(SbeError::Overflow)?;

        // rfqId
        let rfq_uuid = SbeUuid::decode(&buffer[offset..])?;
        offset = offset
            .checked_add(SbeUuid::SIZE)
            .ok_or(SbeError::Overflow)?;

        // quoteId
        let quote_uuid = SbeUuid::decode(&buffer[offset..])?;
        offset = offset
            .checked_add(SbeUuid::SIZE)
            .ok_or(SbeError::Overflow)?;

        // price
        let price_sbe = SbeDecimal::decode(&buffer[offset..])?;
        let price_decimal = price_sbe.to_decimal()?;
        let price = Price::from_decimal(price_decimal)
            .map_err(|e| SbeError::InvalidFieldValue(format!("invalid price: {}", e)))?;
        offset = offset
            .checked_add(SbeDecimal::SIZE)
            .ok_or(SbeError::Overflow)?;

        // quantity
        let qty_sbe = SbeDecimal::decode(&buffer[offset..])?;
        let qty_decimal = qty_sbe.to_decimal()?;
        let quantity = Quantity::from_decimal(qty_decimal)
            .map_err(|e| SbeError::InvalidFieldValue(format!("invalid quantity: {}", e)))?;
        offset = offset
            .checked_add(SbeDecimal::SIZE)
            .ok_or(SbeError::Overflow)?;

        // createdAt
        let created_bytes = buffer
            .get(offset..offset.checked_add(8).ok_or(SbeError::Overflow)?)
            .ok_or_else(|| SbeError::BufferTooSmall {
                needed: offset + 8,
                available: buffer.len(),
            })?;
        let created_nanos = u64::from_le_bytes(
            created_bytes
                .try_into()
                .map_err(|_| SbeError::InvalidFieldValue("invalid timestamp".to_string()))?,
        );
        let created_at = Timestamp::from_nanos(created_nanos as i64)
            .ok_or_else(|| SbeError::InvalidTimestamp("timestamp out of range".to_string()))?;
        offset = offset.checked_add(8).ok_or(SbeError::Overflow)?;

        // Variable fields
        let (venue_id, venue_size) = decode_var_string(&buffer[offset..])?;
        offset = offset
            .checked_add(venue_size)
            .ok_or(SbeError::Overflow)?;

        let (venue_execution_ref, _ref_size) = decode_var_string(&buffer[offset..])?;

        Ok(Self {
            trade_id: trade_uuid.to_uuid(),
            rfq_id: rfq_uuid.to_uuid(),
            quote_id: quote_uuid.to_uuid(),
            price,
            quantity,
            created_at,
            venue_id,
            venue_execution_ref,
        })
    }
}

/// QuoteUpdate - Streaming quote update.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuoteUpdate {
    /// Quote identifier.
    pub quote_id: Uuid,
    /// RFQ identifier.
    pub rfq_id: Uuid,
    /// Quoted price.
    pub price: Price,
    /// Quoted quantity.
    pub quantity: Quantity,
    /// Venue commission.
    pub commission: rust_decimal::Decimal,
    /// Quote validity expiration.
    pub valid_until: Timestamp,
    /// Quote creation time.
    pub created_at: Timestamp,
    /// Type of venue.
    pub venue_type: VenueType,
    /// Whether this is the final quote.
    pub is_final: bool,
    /// Venue identifier.
    pub venue_id: String,
}

impl QuoteUpdate {
    /// Block length for QuoteUpdate (fixed fields only).
    pub const BLOCK_LENGTH: u16 = 77;
}

impl SbeEncode for QuoteUpdate {
    fn encoded_size(&self) -> usize {
        MESSAGE_HEADER_SIZE + Self::BLOCK_LENGTH as usize + var_string_size(&self.venue_id)
    }

    fn encode(&self, buffer: &mut [u8]) -> SbeResult<usize> {
        let size = self.encoded_size();
        if buffer.len() < size {
            return Err(SbeError::BufferTooSmall {
                needed: size,
                available: buffer.len(),
            });
        }

        let mut offset: usize = 0;

        // Header
        encode_header(buffer, Self::BLOCK_LENGTH, QUOTE_UPDATE_TEMPLATE_ID)?;
        offset =
            offset
                .checked_add(MESSAGE_HEADER_SIZE)
                .ok_or(SbeError::BufferTooSmall {
                    needed: size,
                    available: buffer.len(),
                })?;

        // quoteId
        let quote_uuid = SbeUuid::from_uuid(self.quote_id);
        quote_uuid.encode(&mut buffer[offset..])?;
        offset = offset
            .checked_add(SbeUuid::SIZE)
            .ok_or(SbeError::BufferTooSmall {
                needed: size,
                available: buffer.len(),
            })?;

        // rfqId
        let rfq_uuid = SbeUuid::from_uuid(self.rfq_id);
        rfq_uuid.encode(&mut buffer[offset..])?;
        offset = offset
            .checked_add(SbeUuid::SIZE)
            .ok_or(SbeError::BufferTooSmall {
                needed: size,
                available: buffer.len(),
            })?;

        // price
        let price_decimal = SbeDecimal::from_decimal(self.price.get());
        price_decimal.encode(&mut buffer[offset..])?;
        offset = offset
            .checked_add(SbeDecimal::SIZE)
            .ok_or(SbeError::BufferTooSmall {
                needed: size,
                available: buffer.len(),
            })?;

        // quantity
        let qty_decimal = SbeDecimal::from_decimal(self.quantity.get());
        qty_decimal.encode(&mut buffer[offset..])?;
        offset = offset
            .checked_add(SbeDecimal::SIZE)
            .ok_or(SbeError::BufferTooSmall {
                needed: size,
                available: buffer.len(),
            })?;

        // commission
        let commission_decimal = SbeDecimal::from_decimal(self.commission);
        commission_decimal.encode(&mut buffer[offset..])?;
        offset = offset
            .checked_add(SbeDecimal::SIZE)
            .ok_or(SbeError::BufferTooSmall {
                needed: size,
                available: buffer.len(),
            })?;

        // validUntil (nanoseconds)
        let valid_nanos = self.valid_until.timestamp_nanos().unwrap_or(0) as u64;
        buffer[offset..offset.checked_add(8).ok_or(SbeError::Overflow)?]
            .copy_from_slice(&valid_nanos.to_le_bytes());
        offset = offset
            .checked_add(8)
            .ok_or(SbeError::BufferTooSmall {
                needed: size,
                available: buffer.len(),
            })?;

        // createdAt (nanoseconds)
        let created_nanos = self.created_at.timestamp_nanos().unwrap_or(0) as u64;
        buffer[offset..offset.checked_add(8).ok_or(SbeError::Overflow)?]
            .copy_from_slice(&created_nanos.to_le_bytes());
        offset = offset
            .checked_add(8)
            .ok_or(SbeError::BufferTooSmall {
                needed: size,
                available: buffer.len(),
            })?;

        // venueType
        buffer[offset] = encode_venue_type(self.venue_type);
        offset = offset
            .checked_add(1)
            .ok_or(SbeError::BufferTooSmall {
                needed: size,
                available: buffer.len(),
            })?;

        // isFinal
        buffer[offset] = if self.is_final { 1 } else { 0 };
        offset = offset
            .checked_add(1)
            .ok_or(SbeError::BufferTooSmall {
                needed: size,
                available: buffer.len(),
            })?;

        // venueId (variable)
        let venue_size = encode_var_string(&self.venue_id, &mut buffer[offset..])?;
        offset = offset
            .checked_add(venue_size)
            .ok_or(SbeError::BufferTooSmall {
                needed: size,
                available: buffer.len(),
            })?;

        Ok(offset)
    }
}

impl SbeDecode for QuoteUpdate {
    fn decode(buffer: &[u8]) -> SbeResult<Self> {
        let (_block_length, template_id, _schema_id, _version) = decode_header(buffer)?;

        if template_id != QUOTE_UPDATE_TEMPLATE_ID {
            return Err(SbeError::UnknownTemplateId(template_id));
        }

        let mut offset: usize = MESSAGE_HEADER_SIZE;

        // quoteId
        let quote_uuid = SbeUuid::decode(&buffer[offset..])?;
        offset = offset
            .checked_add(SbeUuid::SIZE)
            .ok_or(SbeError::Overflow)?;

        // rfqId
        let rfq_uuid = SbeUuid::decode(&buffer[offset..])?;
        offset = offset
            .checked_add(SbeUuid::SIZE)
            .ok_or(SbeError::Overflow)?;

        // price
        let price_sbe = SbeDecimal::decode(&buffer[offset..])?;
        let price_decimal = price_sbe.to_decimal()?;
        let price = Price::from_decimal(price_decimal)
            .map_err(|e| SbeError::InvalidFieldValue(format!("invalid price: {}", e)))?;
        offset = offset
            .checked_add(SbeDecimal::SIZE)
            .ok_or(SbeError::Overflow)?;

        // quantity
        let qty_sbe = SbeDecimal::decode(&buffer[offset..])?;
        let qty_decimal = qty_sbe.to_decimal()?;
        let quantity = Quantity::from_decimal(qty_decimal)
            .map_err(|e| SbeError::InvalidFieldValue(format!("invalid quantity: {}", e)))?;
        offset = offset
            .checked_add(SbeDecimal::SIZE)
            .ok_or(SbeError::Overflow)?;

        // commission
        let commission_sbe = SbeDecimal::decode(&buffer[offset..])?;
        let commission = commission_sbe.to_decimal()?;
        offset = offset
            .checked_add(SbeDecimal::SIZE)
            .ok_or(SbeError::Overflow)?;

        // validUntil
        let valid_bytes = buffer
            .get(offset..offset.checked_add(8).ok_or(SbeError::Overflow)?)
            .ok_or_else(|| SbeError::BufferTooSmall {
                needed: offset + 8,
                available: buffer.len(),
            })?;
        let valid_nanos = u64::from_le_bytes(
            valid_bytes
                .try_into()
                .map_err(|_| SbeError::InvalidFieldValue("invalid timestamp".to_string()))?,
        );
        let valid_until = Timestamp::from_nanos(valid_nanos as i64)
            .ok_or_else(|| SbeError::InvalidTimestamp("timestamp out of range".to_string()))?;
        offset = offset.checked_add(8).ok_or(SbeError::Overflow)?;

        // createdAt
        let created_bytes = buffer
            .get(offset..offset.checked_add(8).ok_or(SbeError::Overflow)?)
            .ok_or_else(|| SbeError::BufferTooSmall {
                needed: offset + 8,
                available: buffer.len(),
            })?;
        let created_nanos = u64::from_le_bytes(
            created_bytes
                .try_into()
                .map_err(|_| SbeError::InvalidFieldValue("invalid timestamp".to_string()))?,
        );
        let created_at = Timestamp::from_nanos(created_nanos as i64)
            .ok_or_else(|| SbeError::InvalidTimestamp("timestamp out of range".to_string()))?;
        offset = offset.checked_add(8).ok_or(SbeError::Overflow)?;

        // venueType
        let venue_type = decode_venue_type(buffer[offset])?;
        offset = offset.checked_add(1).ok_or(SbeError::Overflow)?;

        // isFinal
        let is_final = buffer[offset] != 0;
        offset = offset.checked_add(1).ok_or(SbeError::Overflow)?;

        // venueId (variable)
        let (venue_id, _venue_size) = decode_var_string(&buffer[offset..])?;

        Ok(Self {
            quote_id: quote_uuid.to_uuid(),
            rfq_id: rfq_uuid.to_uuid(),
            price,
            quantity,
            commission,
            valid_until,
            created_at,
            venue_type,
            is_final,
            venue_id,
        })
    }
}

/// RfqStatusUpdate - Streaming RFQ status update.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RfqStatusUpdate {
    /// RFQ identifier.
    pub rfq_id: Uuid,
    /// Previous RFQ state.
    pub previous_state: RfqState,
    /// Current RFQ state.
    pub current_state: RfqState,
    /// State transition timestamp.
    pub timestamp: Timestamp,
    /// Status message.
    pub message: String,
}

impl RfqStatusUpdate {
    /// Block length for RfqStatusUpdate (fixed fields only).
    pub const BLOCK_LENGTH: u16 = 26;
}

impl SbeEncode for RfqStatusUpdate {
    fn encoded_size(&self) -> usize {
        MESSAGE_HEADER_SIZE + Self::BLOCK_LENGTH as usize + var_string_size(&self.message)
    }

    fn encode(&self, buffer: &mut [u8]) -> SbeResult<usize> {
        let size = self.encoded_size();
        if buffer.len() < size {
            return Err(SbeError::BufferTooSmall {
                needed: size,
                available: buffer.len(),
            });
        }

        let mut offset: usize = 0;

        // Header
        encode_header(buffer, Self::BLOCK_LENGTH, RFQ_STATUS_UPDATE_TEMPLATE_ID)?;
        offset =
            offset
                .checked_add(MESSAGE_HEADER_SIZE)
                .ok_or(SbeError::BufferTooSmall {
                    needed: size,
                    available: buffer.len(),
                })?;

        // rfqId
        let rfq_uuid = SbeUuid::from_uuid(self.rfq_id);
        rfq_uuid.encode(&mut buffer[offset..])?;
        offset = offset
            .checked_add(SbeUuid::SIZE)
            .ok_or(SbeError::BufferTooSmall {
                needed: size,
                available: buffer.len(),
            })?;

        // previousState
        buffer[offset] = encode_rfq_state(self.previous_state);
        offset = offset
            .checked_add(1)
            .ok_or(SbeError::BufferTooSmall {
                needed: size,
                available: buffer.len(),
            })?;

        // currentState
        buffer[offset] = encode_rfq_state(self.current_state);
        offset = offset
            .checked_add(1)
            .ok_or(SbeError::BufferTooSmall {
                needed: size,
                available: buffer.len(),
            })?;

        // timestamp (nanoseconds)
        let timestamp_nanos = self.timestamp.timestamp_nanos().unwrap_or(0) as u64;
        buffer[offset..offset.checked_add(8).ok_or(SbeError::Overflow)?]
            .copy_from_slice(&timestamp_nanos.to_le_bytes());
        offset = offset
            .checked_add(8)
            .ok_or(SbeError::BufferTooSmall {
                needed: size,
                available: buffer.len(),
            })?;

        // message (variable)
        let msg_size = encode_var_string(&self.message, &mut buffer[offset..])?;
        offset = offset
            .checked_add(msg_size)
            .ok_or(SbeError::BufferTooSmall {
                needed: size,
                available: buffer.len(),
            })?;

        Ok(offset)
    }
}

impl SbeDecode for RfqStatusUpdate {
    fn decode(buffer: &[u8]) -> SbeResult<Self> {
        let (_block_length, template_id, _schema_id, _version) = decode_header(buffer)?;

        if template_id != RFQ_STATUS_UPDATE_TEMPLATE_ID {
            return Err(SbeError::UnknownTemplateId(template_id));
        }

        let mut offset: usize = MESSAGE_HEADER_SIZE;

        // rfqId
        let rfq_uuid = SbeUuid::decode(&buffer[offset..])?;
        offset = offset
            .checked_add(SbeUuid::SIZE)
            .ok_or(SbeError::Overflow)?;

        // previousState
        let previous_state = decode_rfq_state(buffer[offset])?;
        offset = offset.checked_add(1).ok_or(SbeError::Overflow)?;

        // currentState
        let current_state = decode_rfq_state(buffer[offset])?;
        offset = offset.checked_add(1).ok_or(SbeError::Overflow)?;

        // timestamp
        let ts_bytes = buffer
            .get(offset..offset.checked_add(8).ok_or(SbeError::Overflow)?)
            .ok_or_else(|| SbeError::BufferTooSmall {
                needed: offset + 8,
                available: buffer.len(),
            })?;
        let ts_nanos = u64::from_le_bytes(
            ts_bytes
                .try_into()
                .map_err(|_| SbeError::InvalidFieldValue("invalid timestamp".to_string()))?,
        );
        let timestamp = Timestamp::from_nanos(ts_nanos as i64)
            .ok_or_else(|| SbeError::InvalidTimestamp("timestamp out of range".to_string()))?;
        offset = offset.checked_add(8).ok_or(SbeError::Overflow)?;

        // message (variable)
        let (message, _msg_size) = decode_var_string(&buffer[offset..])?;

        Ok(Self {
            rfq_id: rfq_uuid.to_uuid(),
            previous_state,
            current_state,
            timestamp,
            message,
        })
    }
}

/// ErrorResponse - Error response for failed operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorResponse {
    /// Request correlation ID.
    pub request_id: Uuid,
    /// RFQ identifier (zeroed if error not RFQ-specific).
    pub rfq_id: Uuid,
    /// Error code.
    pub code: i32,
    /// Human-readable error message.
    pub message: String,
    /// JSON-encoded error metadata.
    pub metadata: String,
}

impl ErrorResponse {
    /// Block length for ErrorResponse (fixed fields only).
    pub const BLOCK_LENGTH: u16 = 36;
}

impl SbeEncode for ErrorResponse {
    fn encoded_size(&self) -> usize {
        MESSAGE_HEADER_SIZE
            + Self::BLOCK_LENGTH as usize
            + var_string_size(&self.message)
            + var_string_size(&self.metadata)
    }

    fn encode(&self, buffer: &mut [u8]) -> SbeResult<usize> {
        let size = self.encoded_size();
        if buffer.len() < size {
            return Err(SbeError::BufferTooSmall {
                needed: size,
                available: buffer.len(),
            });
        }

        let mut offset: usize = 0;

        // Header
        encode_header(buffer, Self::BLOCK_LENGTH, ERROR_RESPONSE_TEMPLATE_ID)?;
        offset =
            offset
                .checked_add(MESSAGE_HEADER_SIZE)
                .ok_or(SbeError::BufferTooSmall {
                    needed: size,
                    available: buffer.len(),
                })?;

        // requestId
        let request_uuid = SbeUuid::from_uuid(self.request_id);
        request_uuid.encode(&mut buffer[offset..])?;
        offset = offset
            .checked_add(SbeUuid::SIZE)
            .ok_or(SbeError::BufferTooSmall {
                needed: size,
                available: buffer.len(),
            })?;

        // rfqId
        let rfq_uuid = SbeUuid::from_uuid(self.rfq_id);
        rfq_uuid.encode(&mut buffer[offset..])?;
        offset = offset
            .checked_add(SbeUuid::SIZE)
            .ok_or(SbeError::BufferTooSmall {
                needed: size,
                available: buffer.len(),
            })?;

        // code
        buffer[offset..offset.checked_add(4).ok_or(SbeError::Overflow)?]
            .copy_from_slice(&self.code.to_le_bytes());
        offset = offset
            .checked_add(4)
            .ok_or(SbeError::BufferTooSmall {
                needed: size,
                available: buffer.len(),
            })?;

        // message (variable)
        let msg_size = encode_var_string(&self.message, &mut buffer[offset..])?;
        offset = offset
            .checked_add(msg_size)
            .ok_or(SbeError::BufferTooSmall {
                needed: size,
                available: buffer.len(),
            })?;

        // metadata (variable)
        let meta_size = encode_var_string(&self.metadata, &mut buffer[offset..])?;
        offset = offset
            .checked_add(meta_size)
            .ok_or(SbeError::BufferTooSmall {
                needed: size,
                available: buffer.len(),
            })?;

        Ok(offset)
    }
}

impl SbeDecode for ErrorResponse {
    fn decode(buffer: &[u8]) -> SbeResult<Self> {
        let (_block_length, template_id, _schema_id, _version) = decode_header(buffer)?;

        if template_id != ERROR_RESPONSE_TEMPLATE_ID {
            return Err(SbeError::UnknownTemplateId(template_id));
        }

        let mut offset: usize = MESSAGE_HEADER_SIZE;

        // requestId
        let request_uuid = SbeUuid::decode(&buffer[offset..])?;
        offset = offset
            .checked_add(SbeUuid::SIZE)
            .ok_or(SbeError::Overflow)?;

        // rfqId
        let rfq_uuid = SbeUuid::decode(&buffer[offset..])?;
        offset = offset
            .checked_add(SbeUuid::SIZE)
            .ok_or(SbeError::Overflow)?;

        // code
        let code_bytes = buffer
            .get(offset..offset.checked_add(4).ok_or(SbeError::Overflow)?)
            .ok_or_else(|| SbeError::BufferTooSmall {
                needed: offset + 4,
                available: buffer.len(),
            })?;
        let code = i32::from_le_bytes(
            code_bytes
                .try_into()
                .map_err(|_| SbeError::InvalidFieldValue("invalid error code".to_string()))?,
        );
        offset = offset.checked_add(4).ok_or(SbeError::Overflow)?;

        // message (variable)
        let (message, msg_size) = decode_var_string(&buffer[offset..])?;
        offset = offset
            .checked_add(msg_size)
            .ok_or(SbeError::Overflow)?;

        // metadata (variable)
        let (metadata, _meta_size) = decode_var_string(&buffer[offset..])?;

        Ok(Self {
            request_id: request_uuid.to_uuid(),
            rfq_id: rfq_uuid.to_uuid(),
            code,
            message,
            metadata,
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn get_rfq_request_roundtrip() {
        let request = GetRfqRequest::new(Uuid::new_v4(), Uuid::new_v4());

        let mut buffer = vec![0u8; request.encoded_size()];
        let encoded_size = request.encode(&mut buffer).unwrap();

        assert_eq!(encoded_size, request.encoded_size());

        let decoded = GetRfqRequest::decode(&buffer).unwrap();
        assert_eq!(request, decoded);
    }

    #[test]
    fn execute_trade_request_roundtrip() {
        let request = ExecuteTradeRequest::new(Uuid::new_v4(), Uuid::new_v4());

        let mut buffer = vec![0u8; request.encoded_size()];
        let encoded_size = request.encode(&mut buffer).unwrap();

        assert_eq!(encoded_size, request.encoded_size());

        let decoded = ExecuteTradeRequest::decode(&buffer).unwrap();
        assert_eq!(request, decoded);
    }

    #[test]
    fn subscribe_quotes_request_roundtrip() {
        let request = SubscribeQuotesRequest::new(Uuid::new_v4());

        let mut buffer = vec![0u8; request.encoded_size()];
        let encoded_size = request.encode(&mut buffer).unwrap();

        assert_eq!(encoded_size, request.encoded_size());

        let decoded = SubscribeQuotesRequest::decode(&buffer).unwrap();
        assert_eq!(request, decoded);
    }
}
