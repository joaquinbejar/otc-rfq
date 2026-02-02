//! # RFQ Protocol Adapters
//!
//! Adapters for RFQ-native protocols like Hashflow and Bebop.
//!
//! ## Available Adapters
//!
//! - [`HashflowAdapter`]: Hashflow RFQ protocol with gasless trading
//!
//! ## Features
//!
//! - Gasless trading with MEV protection
//! - Signed quotes with EIP-712 signatures
//! - Quote expiry tracking
//! - Multi-chain support

pub mod hashflow;

pub use hashflow::{
    HashflowAdapter, HashflowChain, HashflowConfig, HashflowQuoteData, HashflowRfqRequest,
    HashflowRfqResponse,
};
