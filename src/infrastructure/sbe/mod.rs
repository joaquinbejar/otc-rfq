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

// NOTE: IronSBE codegen is available but currently generates code with issues
// (empty enums). The generated module is disabled until IronSBE codegen is fixed.
// For now, we use ironsbe-core types directly in our custom implementation.
//
// To enable generated code in the future, uncomment:
// #[allow(unsafe_code, clippy::transmute_int_to_bool)]
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
