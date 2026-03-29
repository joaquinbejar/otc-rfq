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
//! The `generated` module contains IronSBE-generated encoders and decoders
//! from the XML schema. These provide zero-copy access to SBE messages.

pub mod codecs;
pub mod error;
#[cfg(test)]
mod proptest_roundtrip;
pub mod traits;
pub mod types;

// NOTE: IronSBE v0.2.0 codegen generates code with compilation errors when
// multiple messages use the same repeating group name (e.g., "quotes" group
// in CreateRfqResponse, GetRfqResponse, CancelRfqResponse). This causes
// duplicate type definitions for QuotesGroupDecoder/QuotesEntryDecoder,
// resulting in 65 compilation errors.
//
// Issue reported: https://github.com/joaquinbejar/IronSBE/issues/5
//
// The generated module is disabled until IronSBE fixes repeating group
// namespacing. For now, we use custom codec implementations in the codecs module.
//
// To enable when fixed:
// #[allow(unsafe_code, clippy::transmute_int_to_bool, clippy::all)]
// #[allow(dead_code, unused_imports, non_camel_case_types)]
// pub mod generated {
//     include!(concat!(env!("OUT_DIR"), "/sbe_generated.rs"));
// }

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
