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
//!
//! ## Generated Code
//!
//! The `generated` module contains IronSBE-generated encoders and decoders from
//! `schemas/sbe/otc-rfq.xml`, providing zero-copy access to all 18 SBE message types.
//!
//! Generated with IronSBE v0.2.1 which fixes repeating group namespacing
//! (issue <https://github.com/joaquinbejar/IronSBE/issues/5>).
//!
//! Generated types:
//! - 18 message encoders/decoders (IDs 1-4, 10-11, 20-27, 30-33, 40, 50)
//! - 5 enums: RfqState, OrderSide, VenueType, SettlementMethod, AssetClass
//! - Composite types: Uuid, Decimal, Timestamp
//! - Repeating group support for quotes arrays

pub mod codecs;
pub mod error;
#[cfg(test)]
mod proptest_roundtrip;
pub mod traits;
pub mod types;

// Generated SBE types from schemas/sbe/otc-rfq.xml
// IronSBE v0.2.1 fixes repeating group namespacing (issue #5)
#[allow(unsafe_code, clippy::transmute_int_to_bool, clippy::all)]
#[allow(dead_code, unused_imports, non_camel_case_types, missing_docs)]
pub mod generated {
    include!(concat!(env!("OUT_DIR"), "/sbe_generated.rs"));
}

pub use codecs::{
    MESSAGE_HEADER_SIZE, QUOTE_RECEIVED_TEMPLATE_ID, QuoteReceivedCodec, RFQ_CREATED_TEMPLATE_ID,
    RfqCreatedCodec, TRADE_EXECUTED_TEMPLATE_ID, TradeExecutedCodec,
};
pub use error::SbeError;
pub use traits::{SbeDecode, SbeEncode};
pub use types::{SbeDecimal, SbeUuid};

// Re-export IronSBE core types for convenience
pub use ironsbe_core::{
    decoder::DecodeError as IronSbeDecodeError, encoder::SbeEncoder as IronSbeEncoder,
    header::MessageHeader as IronSbeMessageHeader,
};
