//! # DEX Aggregator Adapters
//!
//! Adapters for DEX aggregators like 0x and 1inch.
//!
//! ## Available Adapters
//!
//! - [`ZeroXAdapter`]: 0x Protocol DEX aggregator
//! - `OneInchAdapter`: 1inch DEX aggregator (TODO)
//!
//! ## Multi-Chain Support
//!
//! Both adapters support multiple chains:
//! - Ethereum mainnet
//! - Polygon
//! - Arbitrum
//! - Optimism
//! - Base
//! - BSC
//! - Avalanche

pub mod one_inch;
pub mod zero_x;

pub use zero_x::{ZeroXAdapter, ZeroXChain, ZeroXConfig, ZeroXQuoteResponse, ZeroXSource};
