//! # SBE API Module
//!
//! Simple Binary Encoding (SBE) API for high-performance client operations.
//!
//! This module provides SBE-based request/response types for client API operations,
//! offering an alternative to gRPC with lower latency and smaller message sizes.
//!
//! ## Architecture
//!
//! - **types**: Request/Response struct definitions
//! - **conversions**: Domain type conversions (From/TryFrom)
//! - **error**: SbeApiError type for conversion failures
//!
//! ## Message Types
//!
//! **Client API (IDs 20-27):**
//! - CreateRfq, GetRfq, CancelRfq, ExecuteTrade
//!
//! **Streaming (IDs 30-33):**
//! - SubscribeQuotes, QuoteUpdate, SubscribeRfqStatus, RfqStatusUpdate
//!
//! **Control/Error (IDs 40, 50):**
//! - Unsubscribe, ErrorResponse
//!
//! ## Usage
//!
//! ```ignore
//! use otc_rfq::api::sbe::{CreateRfqRequest, CreateRfqResponse};
//! use otc_rfq::infrastructure::sbe::{SbeEncode, SbeDecode};
//!
//! // Encode request
//! let request = CreateRfqRequest { /* ... */ };
//! let mut buffer = vec![0u8; request.encoded_size()];
//! request.encode(&mut buffer)?;
//!
//! // Decode response
//! let response = CreateRfqResponse::decode(&buffer)?;
//! ```

pub mod conversions;
pub mod error;
pub mod handlers;
pub mod server;
pub mod types;

pub use error::{SbeApiError, SbeApiResult};
pub use server::{AppState, SbeServer};
pub use types::*;
