//! # Blockchain Clients
//!
//! Clients for on-chain execution on Ethereum and L2 networks.
//!
//! ## Available Components
//!
//! - [`BlockchainClient`]: Trait for blockchain interactions
//! - [`EthereumClient`]: Ethereum and L2 client implementation
//! - [`GasPrice`]: Gas pricing (legacy and EIP-1559)
//! - [`GasEstimator`]: Gas estimation with buffer
//! - [`ChainId`]: Supported blockchain networks
//! - [`ChainConfig`]: Chain-specific configuration
//! - [`TokenRegistry`]: Token address mapping across chains
//!
//! ## Supported Chains
//!
//! - Ethereum mainnet
//! - Polygon
//! - Arbitrum
//! - Optimism
//! - Base

pub mod client;
pub mod config;
pub mod ethereum;
pub mod gas;
pub mod tokens;

pub use client::{
    BlockchainClient, BlockchainError, BlockchainResult, ChainId, TxHash, TxPriority, TxReceipt,
};
pub use config::{
    ChainConfig, ChainConfigBuilder, ChainsConfig, ConfigError, ConfigResult, GasPriceStrategy,
    parse_chains_config, substitute_env_vars,
};
pub use ethereum::EthereumClient;
pub use gas::{FeeHistory, GasEstimator, GasPrice};
pub use tokens::{
    TokenError, TokenInfo, TokenRegistry, TokenResult, is_valid_address, normalize_address,
};
