//! # SBE Encoding/Decoding Traits
//!
//! Traits for SBE binary serialization.
//!
//! This module provides domain-specific encoding traits that complement
//! the IronSBE core traits (`ironsbe_core::encoder::SbeEncoder` and
//! `ironsbe_core::decoder::SbeDecoder`).
//!
//! ## Relationship with IronSBE
//!
//! - [`SbeEncode`] - Domain trait for encoding domain events to SBE format
//! - [`SbeDecode`] - Domain trait for decoding SBE messages to domain events
//! - `ironsbe_core::SbeEncoder` - Low-level encoder trait for generated code
//! - `ironsbe_core::SbeDecoder` - Low-level decoder trait for generated code

use super::error::SbeResult;

/// Trait for types that can be encoded to SBE binary format.
///
/// This is a domain-level trait for encoding domain events. It differs from
/// `ironsbe_core::SbeEncoder` which is designed for generated encoder wrappers.
pub trait SbeEncode {
    /// Returns the encoded size in bytes (including header).
    #[must_use]
    fn encoded_size(&self) -> usize;

    /// Encodes the message to a byte buffer.
    ///
    /// # Errors
    ///
    /// Returns `SbeError::BufferTooSmall` if the buffer is too small.
    fn encode(&self, buffer: &mut [u8]) -> SbeResult<usize>;

    /// Encodes the message to a new Vec.
    ///
    /// # Errors
    ///
    /// Returns an error if encoding fails.
    fn encode_to_vec(&self) -> SbeResult<Vec<u8>> {
        let size = self.encoded_size();
        let mut buffer = vec![0u8; size];
        self.encode(&mut buffer)?;
        Ok(buffer)
    }
}

/// Trait for types that can be decoded from SBE binary format.
///
/// This is a domain-level trait for decoding SBE messages to domain events.
/// It differs from `ironsbe_core::SbeDecoder` which is designed for generated
/// zero-copy decoder wrappers.
pub trait SbeDecode: Sized {
    /// Decodes a message from a byte buffer.
    ///
    /// # Errors
    ///
    /// Returns an error if the buffer is invalid or too small.
    fn decode(buffer: &[u8]) -> SbeResult<Self>;
}
