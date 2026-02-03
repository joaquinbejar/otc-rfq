//! # DEX Adapters
//!
//! Adapters for DEX aggregators and protocols like 0x, 1inch, Paraswap, Uniswap V3, Curve, and Balancer.
//!
//! ## Available Adapters
//!
//! - [`ZeroXAdapter`]: 0x Protocol DEX aggregator
//! - [`OneInchAdapter`]: 1inch DEX aggregator
//! - [`ParaswapAdapter`]: Paraswap DEX aggregator
//! - [`UniswapV3Adapter`]: Uniswap V3 concentrated liquidity DEX
//! - [`CurveAdapter`]: Curve Finance StableSwap and CryptoSwap DEX
//! - [`BalancerAdapter`]: Balancer V2 weighted and stable pools
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

pub mod balancer;
pub mod curve;
pub mod one_inch;
pub mod paraswap;
pub mod uniswap_v3;
pub mod zero_x;

pub use balancer::{
    BalancerAdapter, BalancerChain, BalancerConfig, BalancerPoolInfo, BalancerPoolType,
    BalancerQuoteResult, BalancerSwapRoute, SwapKind,
};
pub use curve::{
    CurveAdapter, CurveChain, CurveConfig, CurvePoolInfo, CurvePoolType, CurveQuoteResult,
    CurveSwapRoute,
};
pub use one_inch::{
    OneInchAdapter, OneInchChain, OneInchConfig, OneInchQuoteResponse, OneInchSwapResponse,
};
pub use paraswap::{
    ParaswapAdapter, ParaswapChain, ParaswapConfig, ParaswapQuoteResponse,
    ParaswapTransactionResponse,
};
pub use uniswap_v3::{
    FeeTier, PoolInfo, QuoteResult, SwapPath, UniswapV3Adapter, UniswapV3Chain, UniswapV3Config,
};
pub use zero_x::{ZeroXAdapter, ZeroXChain, ZeroXConfig, ZeroXQuoteResponse, ZeroXSource};
