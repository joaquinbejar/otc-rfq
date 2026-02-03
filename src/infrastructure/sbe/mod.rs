//! # SBE (Simple Binary Encoding) Module
//!
//! Binary encoding/decoding for high-performance message serialization.
//!
//! This module provides SBE encoding and decoding for domain events,
//! following the schema defined in `schemas/sbe/otc-rfq.xml`.
//!
//! ## Message Types
//!
//! - `RfqCreated` (ID: 1) - RFQ creation event
//! - `QuoteReceived` (ID: 2) - Quote received from venue
//! - `TradeExecuted` (ID: 3) - Trade execution event
//!
//! ## Wire Format
//!
//! All messages use little-endian byte order and follow the SBE specification:
//! - Message header (8 bytes): blockLength, templateId, schemaId, version
//! - Fixed fields in declaration order
//! - Variable-length fields at the end

pub mod codecs;
pub mod error;
#[cfg(test)]
mod proptest_roundtrip;
pub mod traits;
pub mod types;

pub use codecs::{
    MESSAGE_HEADER_SIZE, QUOTE_RECEIVED_TEMPLATE_ID, QuoteReceivedCodec, RFQ_CREATED_TEMPLATE_ID,
    RfqCreatedCodec, TRADE_EXECUTED_TEMPLATE_ID, TradeExecutedCodec,
};
pub use error::SbeError;
pub use traits::{SbeDecode, SbeEncode};
pub use types::{SbeDecimal, SbeUuid};
