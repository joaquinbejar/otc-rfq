//! # DEX Aggregator Adapters
//!
//! Adapters for DEX aggregators like 0x, 1inch, and Paraswap.
//!
//! ## Available Adapters
//!
//! - [`ZeroXAdapter`]: 0x Protocol DEX aggregator
//! - [`OneInchAdapter`]: 1inch DEX aggregator
//! - [`ParaswapAdapter`]: Paraswap DEX aggregator
//!
//! ## Multi-Chain Support
//!
//! All adapters support multiple chains:
//! - Ethereum mainnet
//! - Polygon
//! - Arbitrum
//! - Optimism
//! - Base
//! - BSC
//! - Avalanche
//! - Gnosis (1inch only)
//! - Fantom (1inch, Paraswap)

pub mod one_inch;
pub mod paraswap;
pub mod zero_x;

pub use one_inch::{
    OneInchAdapter, OneInchChain, OneInchConfig, OneInchQuoteResponse, OneInchSwapResponse,
};
pub use paraswap::{
    ParaswapAdapter, ParaswapChain, ParaswapConfig, ParaswapQuoteResponse,
    ParaswapTransactionResponse,
};
pub use zero_x::{ZeroXAdapter, ZeroXChain, ZeroXConfig, ZeroXQuoteResponse, ZeroXSource};
