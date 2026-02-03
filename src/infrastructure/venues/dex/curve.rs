//! # Curve Finance Adapter
//!
//! Adapter for Curve Finance DEX with StableSwap and CryptoSwap pools.
//!
//! This module provides the [`CurveAdapter`] which implements the
//! [`VenueAdapter`] trait for Curve Finance protocol.
//!
//! # Features
//!
//! - Multi-chain support (Ethereum, Polygon, Arbitrum, Optimism, Avalanche)
//! - Pool discovery via registry
//! - StableSwap pool support (optimized for stablecoins)
//! - CryptoSwap pool support (volatile assets)
//! - Meta-pool support (pools containing LP tokens)
//! - Gas estimation
//! - Slippage protection
//!
//! # Examples
//!
//! ```ignore
//! use otc_rfq::infrastructure::venues::dex::curve::{CurveAdapter, CurveConfig};
//!
//! let config = CurveConfig::new("https://eth-mainnet.g.alchemy.com/v2/...")
//!     .with_chain(CurveChain::Ethereum);
//!
//! let adapter = CurveAdapter::new(config);
//! ```

use crate::domain::entities::quote::{Quote, QuoteBuilder, QuoteMetadata};
use crate::domain::entities::rfq::Rfq;
use crate::domain::value_objects::timestamp::Timestamp;
use crate::domain::value_objects::{Blockchain, OrderSide, Price, VenueId};
use crate::infrastructure::venues::contract_client::ContractClient;
use crate::infrastructure::venues::error::{VenueError, VenueResult};
use crate::infrastructure::venues::traits::{ExecutionResult, VenueAdapter, VenueHealth};
use async_trait::async_trait;
use ethers::prelude::*;
use rust_decimal::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

/// Default timeout in milliseconds.
const DEFAULT_TIMEOUT_MS: u64 = 10000;

/// Default slippage in basis points (0.5%).
const DEFAULT_SLIPPAGE_BPS: u32 = 50;

/// Default quote validity in seconds.
const DEFAULT_QUOTE_VALIDITY_SECS: u64 = 60;

/// Curve pool types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CurvePoolType {
    /// Plain pool with same-decimal tokens.
    #[default]
    Plain,
    /// Lending pool with wrapped tokens (aTokens, cTokens).
    Lending,
    /// Meta pool containing an LP token from another pool.
    Meta,
    /// CryptoSwap pool for volatile assets.
    Crypto,
    /// Tricrypto pool (3 volatile assets).
    Tricrypto,
    /// Factory pool created via factory contract.
    Factory,
}

impl CurvePoolType {
    /// Returns the pool type name.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::Plain => "plain",
            Self::Lending => "lending",
            Self::Meta => "meta",
            Self::Crypto => "crypto",
            Self::Tricrypto => "tricrypto",
            Self::Factory => "factory",
        }
    }

    /// Returns true if this is a stable pool (uses StableSwap invariant).
    #[must_use]
    pub fn is_stable(&self) -> bool {
        matches!(
            self,
            Self::Plain | Self::Lending | Self::Meta | Self::Factory
        )
    }

    /// Returns true if this is a crypto pool (uses CryptoSwap invariant).
    #[must_use]
    pub fn is_crypto(&self) -> bool {
        matches!(self, Self::Crypto | Self::Tricrypto)
    }

    /// Returns all pool types.
    #[must_use]
    pub fn all() -> &'static [CurvePoolType] {
        &[
            CurvePoolType::Plain,
            CurvePoolType::Lending,
            CurvePoolType::Meta,
            CurvePoolType::Crypto,
            CurvePoolType::Tricrypto,
            CurvePoolType::Factory,
        ]
    }
}

impl fmt::Display for CurvePoolType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Curve Address Provider contract addresses by chain.
pub mod address_provider_contracts {
    /// Ethereum mainnet Address Provider.
    pub const ETHEREUM: &str = "0x0000000022D53366457F9d5E68Ec105046FC4383";
    /// Polygon Address Provider.
    pub const POLYGON: &str = "0x0000000022D53366457F9d5E68Ec105046FC4383";
    /// Arbitrum Address Provider.
    pub const ARBITRUM: &str = "0x0000000022D53366457F9d5E68Ec105046FC4383";
    /// Optimism Address Provider.
    pub const OPTIMISM: &str = "0x0000000022D53366457F9d5E68Ec105046FC4383";
    /// Avalanche Address Provider.
    pub const AVALANCHE: &str = "0x0000000022D53366457F9d5E68Ec105046FC4383";
}

/// Curve Router contract addresses by chain.
pub mod router_contracts {
    /// Ethereum mainnet Router.
    pub const ETHEREUM: &str = "0xF0d4c12A5768D806021F80a262B4d39d26C58b8D";
    /// Polygon Router.
    pub const POLYGON: &str = "0xF0d4c12A5768D806021F80a262B4d39d26C58b8D";
    /// Arbitrum Router.
    pub const ARBITRUM: &str = "0xF0d4c12A5768D806021F80a262B4d39d26C58b8D";
    /// Optimism Router.
    pub const OPTIMISM: &str = "0xF0d4c12A5768D806021F80a262B4d39d26C58b8D";
    /// Avalanche Router.
    pub const AVALANCHE: &str = "0xF0d4c12A5768D806021F80a262B4d39d26C58b8D";
}

/// Curve Registry contract addresses by chain.
pub mod registry_contracts {
    /// Ethereum mainnet Registry.
    pub const ETHEREUM: &str = "0x90E00ACe148ca3b23Ac1bC8C240C2a7Dd9c2d7f5";
    /// Polygon Registry.
    pub const POLYGON: &str = "0x094d12e5b541784701FD8d65F11fc0598FBC6332";
    /// Arbitrum Registry.
    pub const ARBITRUM: &str = "0x445FE580eF8d70FF569aB36e80c647af338db351";
    /// Optimism Registry.
    pub const OPTIMISM: &str = "0x7DA64233Fefb352f8F501B357c018158ED0aA8F8";
    /// Avalanche Registry.
    pub const AVALANCHE: &str = "0x8474DdbE98F5aA3179B3B3F5942D724aFcdec9f6";
}

/// Supported blockchain chains for Curve.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CurveChain {
    /// Ethereum mainnet (chain ID 1).
    #[default]
    Ethereum,
    /// Polygon (chain ID 137).
    Polygon,
    /// Arbitrum One (chain ID 42161).
    Arbitrum,
    /// Optimism (chain ID 10).
    Optimism,
    /// Avalanche C-Chain (chain ID 43114).
    Avalanche,
}

impl CurveChain {
    /// Returns the chain ID.
    #[must_use]
    pub fn chain_id(&self) -> u64 {
        match self {
            Self::Ethereum => 1,
            Self::Polygon => 137,
            Self::Arbitrum => 42161,
            Self::Optimism => 10,
            Self::Avalanche => 43114,
        }
    }

    /// Returns the chain name.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::Ethereum => "ethereum",
            Self::Polygon => "polygon",
            Self::Arbitrum => "arbitrum",
            Self::Optimism => "optimism",
            Self::Avalanche => "avalanche",
        }
    }

    /// Converts to domain Blockchain type.
    #[must_use]
    pub fn to_blockchain(&self) -> Option<Blockchain> {
        match self {
            Self::Ethereum => Some(Blockchain::Ethereum),
            Self::Polygon => Some(Blockchain::Polygon),
            Self::Arbitrum => Some(Blockchain::Arbitrum),
            Self::Optimism => Some(Blockchain::Optimism),
            Self::Avalanche => None, // Avalanche not in domain model yet
        }
    }

    /// Returns the Address Provider contract address for this chain.
    #[must_use]
    pub fn address_provider_contract(&self) -> &'static str {
        match self {
            Self::Ethereum => address_provider_contracts::ETHEREUM,
            Self::Polygon => address_provider_contracts::POLYGON,
            Self::Arbitrum => address_provider_contracts::ARBITRUM,
            Self::Optimism => address_provider_contracts::OPTIMISM,
            Self::Avalanche => address_provider_contracts::AVALANCHE,
        }
    }

    /// Returns the Router contract address for this chain.
    #[must_use]
    pub fn router_contract(&self) -> &'static str {
        match self {
            Self::Ethereum => router_contracts::ETHEREUM,
            Self::Polygon => router_contracts::POLYGON,
            Self::Arbitrum => router_contracts::ARBITRUM,
            Self::Optimism => router_contracts::OPTIMISM,
            Self::Avalanche => router_contracts::AVALANCHE,
        }
    }

    /// Returns the Registry contract address for this chain.
    #[must_use]
    pub fn registry_contract(&self) -> &'static str {
        match self {
            Self::Ethereum => registry_contracts::ETHEREUM,
            Self::Polygon => registry_contracts::POLYGON,
            Self::Arbitrum => registry_contracts::ARBITRUM,
            Self::Optimism => registry_contracts::OPTIMISM,
            Self::Avalanche => registry_contracts::AVALANCHE,
        }
    }

    /// Returns all supported chains.
    #[must_use]
    pub fn all() -> &'static [CurveChain] {
        &[
            CurveChain::Ethereum,
            CurveChain::Polygon,
            CurveChain::Arbitrum,
            CurveChain::Optimism,
            CurveChain::Avalanche,
        ]
    }
}

impl fmt::Display for CurveChain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Pool information from Curve registry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurvePoolInfo {
    /// Pool address.
    pub address: String,
    /// Pool name.
    pub name: String,
    /// Pool type.
    pub pool_type: CurvePoolType,
    /// Token addresses in the pool.
    pub coins: Vec<String>,
    /// Underlying token addresses (for lending pools).
    pub underlying_coins: Option<Vec<String>>,
    /// LP token address.
    pub lp_token: String,
    /// Amplification coefficient (A parameter).
    pub amplification: Option<String>,
    /// Pool fee in basis points.
    pub fee_bps: u32,
    /// Admin fee percentage.
    pub admin_fee_pct: u32,
}

/// Quote result from Curve get_dy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurveQuoteResult {
    /// Amount out.
    pub amount_out: String,
    /// Pool address used.
    pub pool: String,
    /// Input token index in pool.
    pub i: u32,
    /// Output token index in pool.
    pub j: u32,
    /// Whether to use underlying tokens.
    pub use_underlying: bool,
    /// Gas estimate.
    pub gas_estimate: Option<String>,
}

/// Swap route for multi-pool routing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CurveSwapRoute {
    /// Pool addresses in order.
    pub pools: Vec<String>,
    /// Token indices for each swap.
    pub indices: Vec<(u32, u32)>,
    /// Whether to use underlying for each swap.
    pub use_underlying: Vec<bool>,
}

impl CurveSwapRoute {
    /// Creates a direct swap route (single pool).
    #[must_use]
    pub fn direct(pool: impl Into<String>, i: u32, j: u32, use_underlying: bool) -> Self {
        Self {
            pools: vec![pool.into()],
            indices: vec![(i, j)],
            use_underlying: vec![use_underlying],
        }
    }

    /// Returns true if this is a direct swap (single pool).
    #[must_use]
    pub fn is_direct(&self) -> bool {
        self.pools.len() == 1
    }

    /// Returns the number of hops.
    #[must_use]
    pub fn hops(&self) -> usize {
        self.pools.len()
    }
}

/// Configuration for the Curve adapter.
///
/// # Examples
///
/// ```
/// use otc_rfq::infrastructure::venues::dex::curve::{CurveConfig, CurveChain};
///
/// let config = CurveConfig::new("https://eth-mainnet.g.alchemy.com/v2/...")
///     .with_chain(CurveChain::Polygon)
///     .with_slippage_bps(100);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurveConfig {
    /// Venue ID for this adapter.
    venue_id: VenueId,
    /// RPC URL for the chain.
    rpc_url: String,
    /// Target blockchain.
    chain: CurveChain,
    /// Timeout in milliseconds.
    timeout_ms: u64,
    /// Whether the adapter is enabled.
    enabled: bool,
    /// Sender wallet address.
    wallet_address: Option<String>,
    /// Token address mappings (symbol -> address).
    token_addresses: HashMap<String, String>,
    /// Slippage tolerance in basis points.
    slippage_bps: u32,
    /// Quote validity in seconds.
    quote_validity_secs: u64,
    /// Whether to use underlying tokens for lending pools.
    use_underlying: bool,
}

impl CurveConfig {
    /// Creates a new Curve configuration.
    #[must_use]
    pub fn new(rpc_url: impl Into<String>) -> Self {
        Self {
            venue_id: VenueId::new("curve"),
            rpc_url: rpc_url.into(),
            chain: CurveChain::default(),
            timeout_ms: DEFAULT_TIMEOUT_MS,
            enabled: true,
            wallet_address: None,
            token_addresses: Self::default_token_addresses(),
            slippage_bps: DEFAULT_SLIPPAGE_BPS,
            quote_validity_secs: DEFAULT_QUOTE_VALIDITY_SECS,
            use_underlying: true,
        }
    }

    /// Creates default token address mappings for common tokens.
    fn default_token_addresses() -> HashMap<String, String> {
        let mut map = HashMap::new();
        // Ethereum mainnet stablecoin addresses
        map.insert(
            "USDC".to_string(),
            "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".to_string(),
        );
        map.insert(
            "USDT".to_string(),
            "0xdAC17F958D2ee523a2206206994597C13D831ec7".to_string(),
        );
        map.insert(
            "DAI".to_string(),
            "0x6B175474E89094C44Da98b954EeddeBC35e4D1".to_string(),
        );
        map.insert(
            "FRAX".to_string(),
            "0x853d955aCEf822Db058eb8505911ED77F175b99e".to_string(),
        );
        map.insert(
            "LUSD".to_string(),
            "0x5f98805A4E8be255a32880FDeC7F6728C6568bA0".to_string(),
        );
        // Wrapped ETH
        map.insert(
            "WETH".to_string(),
            "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string(),
        );
        // Wrapped BTC
        map.insert(
            "WBTC".to_string(),
            "0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599".to_string(),
        );
        map
    }

    /// Sets the venue ID.
    #[must_use]
    pub fn with_venue_id(mut self, venue_id: impl Into<String>) -> Self {
        self.venue_id = VenueId::new(venue_id);
        self
    }

    /// Sets the target chain.
    #[must_use]
    pub fn with_chain(mut self, chain: CurveChain) -> Self {
        self.chain = chain;
        self
    }

    /// Sets the timeout in milliseconds.
    #[must_use]
    pub fn with_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }

    /// Sets whether the adapter is enabled.
    #[must_use]
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Sets the sender wallet address.
    #[must_use]
    pub fn with_wallet_address(mut self, address: impl Into<String>) -> Self {
        self.wallet_address = Some(address.into());
        self
    }

    /// Adds a token address mapping.
    #[must_use]
    pub fn with_token_address(
        mut self,
        symbol: impl Into<String>,
        address: impl Into<String>,
    ) -> Self {
        self.token_addresses.insert(symbol.into(), address.into());
        self
    }

    /// Sets the slippage tolerance in basis points.
    #[must_use]
    pub fn with_slippage_bps(mut self, slippage_bps: u32) -> Self {
        self.slippage_bps = slippage_bps;
        self
    }

    /// Sets the quote validity in seconds.
    #[must_use]
    pub fn with_quote_validity_secs(mut self, secs: u64) -> Self {
        self.quote_validity_secs = secs;
        self
    }

    /// Sets whether to use underlying tokens for lending pools.
    #[must_use]
    pub fn with_use_underlying(mut self, use_underlying: bool) -> Self {
        self.use_underlying = use_underlying;
        self
    }

    /// Returns the venue ID.
    #[inline]
    #[must_use]
    pub fn venue_id(&self) -> &VenueId {
        &self.venue_id
    }

    /// Returns the RPC URL.
    #[inline]
    #[must_use]
    pub fn rpc_url(&self) -> &str {
        &self.rpc_url
    }

    /// Returns the target chain.
    #[inline]
    #[must_use]
    pub fn chain(&self) -> CurveChain {
        self.chain
    }

    /// Returns the timeout in milliseconds.
    #[inline]
    #[must_use]
    pub fn timeout_ms(&self) -> u64 {
        self.timeout_ms
    }

    /// Returns whether the adapter is enabled.
    #[inline]
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Returns the sender wallet address.
    #[inline]
    #[must_use]
    pub fn wallet_address(&self) -> Option<&str> {
        self.wallet_address.as_deref()
    }

    /// Returns the token addresses.
    #[inline]
    #[must_use]
    pub fn token_addresses(&self) -> &HashMap<String, String> {
        &self.token_addresses
    }

    /// Resolves a token symbol to an address.
    #[must_use]
    pub fn resolve_token_address(&self, symbol: &str) -> Option<&String> {
        self.token_addresses.get(symbol)
    }

    /// Returns the slippage tolerance in basis points.
    #[inline]
    #[must_use]
    pub fn slippage_bps(&self) -> u32 {
        self.slippage_bps
    }

    /// Returns the quote validity in seconds.
    #[inline]
    #[must_use]
    pub fn quote_validity_secs(&self) -> u64 {
        self.quote_validity_secs
    }

    /// Returns whether to use underlying tokens.
    #[inline]
    #[must_use]
    pub fn use_underlying(&self) -> bool {
        self.use_underlying
    }

    /// Returns the Address Provider contract address.
    #[inline]
    #[must_use]
    pub fn address_provider_contract(&self) -> &'static str {
        self.chain.address_provider_contract()
    }

    /// Returns the Router contract address.
    #[inline]
    #[must_use]
    pub fn router_contract(&self) -> &'static str {
        self.chain.router_contract()
    }

    /// Returns the Registry contract address.
    #[inline]
    #[must_use]
    pub fn registry_contract(&self) -> &'static str {
        self.chain.registry_contract()
    }
}

/// Curve Finance DEX adapter.
///
/// Implements the [`VenueAdapter`] trait for Curve Finance Protocol.
///
/// # Features
///
/// - Multi-chain support
/// - Pool discovery via registry
/// - StableSwap and CryptoSwap pools
/// - Meta-pool support
/// - Gas estimation
/// - Slippage protection
pub struct CurveAdapter {
    /// Configuration.
    config: CurveConfig,
    /// Contract client for RPC calls.
    contract_client: ContractClient,
}

impl CurveAdapter {
    /// Creates a new Curve adapter.
    ///
    /// # Errors
    ///
    /// Returns `VenueError::Connection` if the RPC provider cannot be created.
    pub fn new(config: CurveConfig) -> VenueResult<Self> {
        let contract_client = ContractClient::new(config.rpc_url(), config.timeout_ms())?;
        Ok(Self {
            config,
            contract_client,
        })
    }

    /// Returns the configuration.
    #[inline]
    #[must_use]
    pub fn config(&self) -> &CurveConfig {
        &self.config
    }

    /// Resolves token addresses from an RFQ.
    ///
    /// Returns (token_in_address, token_out_address).
    ///
    /// # Errors
    ///
    /// Returns `VenueError::InvalidRequest` if a token symbol cannot be resolved.
    pub fn resolve_tokens(&self, rfq: &Rfq) -> VenueResult<(String, String)> {
        let symbol = rfq.instrument().symbol();
        let base = symbol.base_asset();
        let quote = symbol.quote_asset();

        let base_address = self
            .config
            .resolve_token_address(base)
            .ok_or_else(|| VenueError::invalid_request(format!("Unknown token: {}", base)))?
            .clone();

        let quote_address = self
            .config
            .resolve_token_address(quote)
            .ok_or_else(|| VenueError::invalid_request(format!("Unknown token: {}", quote)))?
            .clone();

        // For Buy side: swap quote token for base token
        // For Sell side: swap base token for quote token
        match rfq.side() {
            OrderSide::Buy => Ok((quote_address, base_address)),
            OrderSide::Sell => Ok((base_address, quote_address)),
        }
    }

    /// Converts a quantity to the smallest unit based on decimals.
    #[must_use]
    pub fn to_smallest_unit(&self, quantity: Decimal, decimals: u8) -> String {
        let multiplier = Decimal::from(10u64.pow(u32::from(decimals)));
        let amount = quantity * multiplier;
        amount.trunc().to_string()
    }

    /// Calculates the minimum amount out with slippage.
    ///
    /// # Errors
    ///
    /// Returns `VenueError::ProtocolError` if the amount cannot be parsed.
    pub fn calculate_min_amount_out(&self, amount_out: &str) -> VenueResult<String> {
        let amount: Decimal = amount_out
            .parse()
            .map_err(|_| VenueError::protocol_error("Invalid amount_out"))?;

        let slippage_factor =
            Decimal::ONE - Decimal::from(self.config.slippage_bps()) / Decimal::from(10000);
        let min_amount = amount * slippage_factor;

        Ok(min_amount.trunc().to_string())
    }

    /// Calculates the price from a quote result.
    ///
    /// # Errors
    ///
    /// Returns `VenueError::ProtocolError` if the price cannot be calculated.
    pub fn calculate_price(&self, amount_in: &str, amount_out: &str) -> VenueResult<Price> {
        let in_amount: f64 = amount_in
            .parse()
            .map_err(|_| VenueError::protocol_error("Invalid amount_in"))?;

        let out_amount: f64 = amount_out
            .parse()
            .map_err(|_| VenueError::protocol_error("Invalid amount_out"))?;

        if in_amount == 0.0 {
            return Err(VenueError::protocol_error("amount_in is zero"));
        }

        let price = out_amount / in_amount;

        Price::new(price).map_err(|_| VenueError::protocol_error("Invalid price value"))
    }

    /// Parses a quote result into a domain Quote.
    ///
    /// # Errors
    ///
    /// Returns `VenueError::ProtocolError` if the quote cannot be created.
    pub fn create_quote(
        &self,
        rfq: &Rfq,
        amount_in: &str,
        quote_result: &CurveQuoteResult,
        route: &CurveSwapRoute,
    ) -> VenueResult<Quote> {
        let price = self.calculate_price(amount_in, &quote_result.amount_out)?;

        let valid_until = Timestamp::now().add_secs(self.config.quote_validity_secs() as i64);

        let mut builder = QuoteBuilder::new(
            rfq.id(),
            self.config.venue_id().clone(),
            price,
            rfq.quantity(),
            valid_until,
        );

        // Add metadata for execution
        let mut metadata = QuoteMetadata::new();
        metadata.set("amount_in", amount_in.to_string());
        metadata.set("amount_out", quote_result.amount_out.clone());
        metadata.set(
            "min_amount_out",
            self.calculate_min_amount_out(&quote_result.amount_out)?,
        );
        metadata.set("pool", quote_result.pool.clone());
        metadata.set("i", quote_result.i.to_string());
        metadata.set("j", quote_result.j.to_string());
        metadata.set("use_underlying", quote_result.use_underlying.to_string());
        metadata.set("chain_id", self.config.chain().chain_id().to_string());
        metadata.set("router", self.config.router_contract().to_string());
        metadata.set("slippage_bps", self.config.slippage_bps().to_string());
        metadata.set("hops", route.hops().to_string());

        if let Some(gas) = &quote_result.gas_estimate {
            metadata.set("gas_estimate", gas.clone());
        }

        builder = builder.metadata(metadata);

        Ok(builder.build())
    }

    /// Validates a quote has all required fields for execution.
    ///
    /// # Errors
    ///
    /// Returns `VenueError::InvalidRequest` if required fields are missing.
    pub fn validate_quote_for_execution(&self, quote: &Quote) -> VenueResult<()> {
        let metadata = quote
            .metadata()
            .ok_or_else(|| VenueError::invalid_request("Quote missing metadata"))?;

        let required_fields = [
            "amount_in",
            "amount_out",
            "min_amount_out",
            "pool",
            "router",
        ];

        for field in required_fields {
            if metadata.get(field).is_none() {
                return Err(VenueError::invalid_request(format!(
                    "Quote missing required field: {}",
                    field
                )));
            }
        }

        Ok(())
    }

    /// Encodes the get_best_rate call for Curve Registry.
    ///
    /// Function signature: get_best_rate(address from, address to, uint256 amount) returns (address, uint256)
    fn encode_get_best_rate(&self, from: Address, to: Address, amount: U256) -> Bytes {
        // Function selector for get_best_rate(address,address,uint256)
        // keccak256("get_best_rate(address,address,uint256)")[0:4]
        let selector: [u8; 4] = [0x4e, 0x21, 0xdf, 0x75];

        let encoded = ethers::abi::encode(&[
            ethers::abi::Token::Address(from),
            ethers::abi::Token::Address(to),
            ethers::abi::Token::Uint(amount),
        ]);

        let mut calldata = Vec::with_capacity(4 + encoded.len());
        calldata.extend_from_slice(&selector);
        calldata.extend_from_slice(&encoded);

        Bytes::from(calldata)
    }

    /// Decodes the get_best_rate result.
    ///
    /// Returns (pool address, amount out).
    ///
    /// # Errors
    ///
    /// Returns `VenueError::ProtocolError` if decoding fails.
    fn decode_best_rate_result(&self, result: &Bytes) -> VenueResult<(Address, U256)> {
        let pool_bytes = result
            .get(12..32)
            .ok_or_else(|| VenueError::protocol_error("Invalid best rate result length"))?;
        let amount_bytes = result
            .get(32..64)
            .ok_or_else(|| VenueError::protocol_error("Invalid best rate result length"))?;

        // First 32 bytes is pool address (padded)
        let pool = Address::from_slice(pool_bytes);
        // Next 32 bytes is amount out
        let amount_out = U256::from_big_endian(amount_bytes);

        Ok((pool, amount_out))
    }
}

impl fmt::Debug for CurveAdapter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CurveAdapter")
            .field("venue_id", self.config.venue_id())
            .field("chain", &self.config.chain())
            .field("enabled", &self.config.is_enabled())
            .finish()
    }
}

#[async_trait]
impl VenueAdapter for CurveAdapter {
    fn venue_id(&self) -> &VenueId {
        self.config.venue_id()
    }

    fn timeout_ms(&self) -> u64 {
        self.config.timeout_ms()
    }

    async fn request_quote(&self, rfq: &Rfq) -> VenueResult<Quote> {
        // Check if enabled
        if !self.config.is_enabled() {
            return Err(VenueError::venue_unavailable(
                self.config.venue_id().clone(),
                "Curve adapter is disabled",
            ));
        }

        // Resolve tokens
        let (token_in, token_out) = self.resolve_tokens(rfq)?;

        // Parse addresses
        let token_in_addr = ContractClient::parse_address(&token_in)?;
        let token_out_addr = ContractClient::parse_address(&token_out)?;
        let registry_addr = ContractClient::parse_address(self.config.registry_contract())?;

        // Calculate amount in
        let amount_in = self.to_smallest_unit(rfq.quantity().get(), 18);
        let amount_in_u256 = ContractClient::parse_u256(&amount_in)?;

        // Encode get_best_rate call on Registry
        // Function: get_best_rate(address from, address to, uint256 amount) returns (address pool, uint256 amountOut)
        let calldata = self.encode_get_best_rate(token_in_addr, token_out_addr, amount_in_u256);

        // Call the registry contract
        let result = self.contract_client.call(registry_addr, calldata).await?;

        // Decode the result (pool address, amount out)
        let (pool_addr, amount_out) = self.decode_best_rate_result(&result)?;

        if pool_addr == Address::zero() {
            return Err(VenueError::protocol_error("No pool found for token pair"));
        }

        // Calculate min amount out with slippage
        let min_amount_out = self.calculate_min_amount_out(&amount_out.to_string())?;

        // Calculate price
        let price = self.calculate_price(&amount_in, &amount_out.to_string())?;

        // Build quote
        let valid_until = Timestamp::now().add_secs(self.config.quote_validity_secs() as i64);

        let mut builder = QuoteBuilder::new(
            rfq.id(),
            self.config.venue_id().clone(),
            price,
            rfq.quantity(),
            valid_until,
        );

        // Add metadata
        let mut metadata = QuoteMetadata::new();
        metadata.set("amount_in", amount_in.clone());
        metadata.set("amount_out", amount_out.to_string());
        metadata.set("min_amount_out", min_amount_out);
        metadata.set("pool", format!("{:?}", pool_addr));
        metadata.set("router", self.config.router_contract().to_string());
        metadata.set("i", "0");
        metadata.set("j", "1");
        metadata.set("use_underlying", self.config.use_underlying().to_string());

        builder = builder.metadata(metadata);

        Ok(builder.build())
    }

    async fn execute_trade(&self, quote: &Quote) -> VenueResult<ExecutionResult> {
        // Check if enabled
        if !self.config.is_enabled() {
            return Err(VenueError::venue_unavailable(
                self.config.venue_id().clone(),
                "Curve adapter is disabled",
            ));
        }

        // Check if quote is expired
        if quote.is_expired() {
            return Err(VenueError::quote_expired("Quote has expired"));
        }

        // Verify quote is from this venue
        if quote.venue_id() != self.config.venue_id() {
            return Err(VenueError::invalid_request("Quote is not from this venue"));
        }

        // Validate quote has required fields
        self.validate_quote_for_execution(quote)?;

        // TODO: Execute via Router contract
        Err(VenueError::internal_error(
            "Curve execution not yet implemented - requires on-chain transaction",
        ))
    }

    async fn health_check(&self) -> VenueResult<VenueHealth> {
        if !self.config.is_enabled() {
            return Ok(VenueHealth::unhealthy(
                self.config.venue_id().clone(),
                "Adapter is disabled",
            ));
        }

        // Check RPC connection
        match self.contract_client.health_check_with_latency().await {
            Ok(latency_ms) => Ok(VenueHealth::healthy_with_latency(
                self.config.venue_id().clone(),
                latency_ms,
            )),
            Err(_) => Ok(VenueHealth::unhealthy(
                self.config.venue_id().clone(),
                "RPC connection failed",
            )),
        }
    }

    async fn is_available(&self) -> bool {
        self.config.is_enabled()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn test_config() -> CurveConfig {
        CurveConfig::new("https://eth-mainnet.example.com")
            .with_chain(CurveChain::Ethereum)
            .with_wallet_address("0x1234567890abcdef1234567890abcdef12345678")
            .with_timeout_ms(5000)
    }

    mod pool_type {
        use super::*;

        #[test]
        fn names() {
            assert_eq!(CurvePoolType::Plain.name(), "plain");
            assert_eq!(CurvePoolType::Lending.name(), "lending");
            assert_eq!(CurvePoolType::Meta.name(), "meta");
            assert_eq!(CurvePoolType::Crypto.name(), "crypto");
            assert_eq!(CurvePoolType::Tricrypto.name(), "tricrypto");
            assert_eq!(CurvePoolType::Factory.name(), "factory");
        }

        #[test]
        fn is_stable() {
            assert!(CurvePoolType::Plain.is_stable());
            assert!(CurvePoolType::Lending.is_stable());
            assert!(CurvePoolType::Meta.is_stable());
            assert!(CurvePoolType::Factory.is_stable());
            assert!(!CurvePoolType::Crypto.is_stable());
            assert!(!CurvePoolType::Tricrypto.is_stable());
        }

        #[test]
        fn is_crypto() {
            assert!(!CurvePoolType::Plain.is_crypto());
            assert!(!CurvePoolType::Lending.is_crypto());
            assert!(CurvePoolType::Crypto.is_crypto());
            assert!(CurvePoolType::Tricrypto.is_crypto());
        }

        #[test]
        fn display() {
            assert_eq!(CurvePoolType::Plain.to_string(), "plain");
            assert_eq!(CurvePoolType::Crypto.to_string(), "crypto");
        }

        #[test]
        fn default_is_plain() {
            assert_eq!(CurvePoolType::default(), CurvePoolType::Plain);
        }

        #[test]
        fn all_types() {
            let types = CurvePoolType::all();
            assert_eq!(types.len(), 6);
        }
    }

    mod chain {
        use super::*;

        #[test]
        fn chain_ids() {
            assert_eq!(CurveChain::Ethereum.chain_id(), 1);
            assert_eq!(CurveChain::Polygon.chain_id(), 137);
            assert_eq!(CurveChain::Arbitrum.chain_id(), 42161);
            assert_eq!(CurveChain::Optimism.chain_id(), 10);
            assert_eq!(CurveChain::Avalanche.chain_id(), 43114);
        }

        #[test]
        fn names() {
            assert_eq!(CurveChain::Ethereum.name(), "ethereum");
            assert_eq!(CurveChain::Polygon.name(), "polygon");
            assert_eq!(CurveChain::Arbitrum.name(), "arbitrum");
            assert_eq!(CurveChain::Optimism.name(), "optimism");
            assert_eq!(CurveChain::Avalanche.name(), "avalanche");
        }

        #[test]
        fn to_blockchain() {
            assert_eq!(
                CurveChain::Ethereum.to_blockchain(),
                Some(Blockchain::Ethereum)
            );
            assert_eq!(
                CurveChain::Polygon.to_blockchain(),
                Some(Blockchain::Polygon)
            );
            assert_eq!(CurveChain::Avalanche.to_blockchain(), None);
        }

        #[test]
        fn display() {
            assert_eq!(CurveChain::Ethereum.to_string(), "ethereum");
        }

        #[test]
        fn default_is_ethereum() {
            assert_eq!(CurveChain::default(), CurveChain::Ethereum);
        }

        #[test]
        fn all_chains() {
            let chains = CurveChain::all();
            assert_eq!(chains.len(), 5);
        }

        #[test]
        fn contract_addresses() {
            assert!(!CurveChain::Ethereum.address_provider_contract().is_empty());
            assert!(!CurveChain::Ethereum.router_contract().is_empty());
            assert!(!CurveChain::Ethereum.registry_contract().is_empty());
            assert!(CurveChain::Ethereum.router_contract().starts_with("0x"));
        }
    }

    mod config {
        use super::*;

        #[test]
        fn new_config() {
            let config = CurveConfig::new("https://rpc.example.com");
            assert_eq!(config.rpc_url(), "https://rpc.example.com");
            assert_eq!(config.chain(), CurveChain::Ethereum);
            assert_eq!(config.timeout_ms(), DEFAULT_TIMEOUT_MS);
            assert!(config.is_enabled());
            assert_eq!(config.slippage_bps(), DEFAULT_SLIPPAGE_BPS);
            assert!(config.use_underlying());
        }

        #[test]
        fn builder_pattern() {
            let config = CurveConfig::new("https://rpc.example.com")
                .with_chain(CurveChain::Polygon)
                .with_timeout_ms(3000)
                .with_enabled(false)
                .with_slippage_bps(100)
                .with_quote_validity_secs(120)
                .with_use_underlying(false)
                .with_wallet_address("0xabc");

            assert_eq!(config.chain(), CurveChain::Polygon);
            assert_eq!(config.timeout_ms(), 3000);
            assert!(!config.is_enabled());
            assert_eq!(config.slippage_bps(), 100);
            assert_eq!(config.quote_validity_secs(), 120);
            assert!(!config.use_underlying());
            assert_eq!(config.wallet_address(), Some("0xabc"));
        }

        #[test]
        fn venue_id() {
            let config = CurveConfig::new("https://rpc.example.com").with_venue_id("custom-curve");
            assert_eq!(config.venue_id(), &VenueId::new("custom-curve"));
        }

        #[test]
        fn token_addresses() {
            let config =
                CurveConfig::new("https://rpc.example.com").with_token_address("TEST", "0x123");
            assert_eq!(
                config.resolve_token_address("TEST"),
                Some(&"0x123".to_string())
            );
            assert!(config.resolve_token_address("UNKNOWN").is_none());
        }

        #[test]
        fn default_token_addresses() {
            let config = CurveConfig::new("https://rpc.example.com");
            assert!(config.resolve_token_address("USDC").is_some());
            assert!(config.resolve_token_address("USDT").is_some());
            assert!(config.resolve_token_address("DAI").is_some());
        }

        #[test]
        fn contract_addresses() {
            let config =
                CurveConfig::new("https://rpc.example.com").with_chain(CurveChain::Polygon);
            assert!(!config.address_provider_contract().is_empty());
            assert!(!config.router_contract().is_empty());
            assert!(!config.registry_contract().is_empty());
        }
    }

    mod swap_route {
        use super::*;

        #[test]
        fn direct_route() {
            let route = CurveSwapRoute::direct("0xpool", 0, 1, false);
            assert!(route.is_direct());
            assert_eq!(route.hops(), 1);
            assert_eq!(route.pools.len(), 1);
            assert_eq!(route.indices[0], (0, 1));
            assert!(!route.use_underlying[0]);
        }

        #[test]
        fn direct_route_with_underlying() {
            let route = CurveSwapRoute::direct("0xpool", 0, 2, true);
            assert!(route.use_underlying[0]);
        }
    }

    mod adapter {
        use super::*;

        #[test]
        fn new_adapter() {
            let adapter = CurveAdapter::new(test_config()).unwrap();
            assert_eq!(adapter.config().chain(), CurveChain::Ethereum);
        }

        #[test]
        fn debug_impl() {
            let adapter = CurveAdapter::new(test_config()).unwrap();
            let debug = format!("{:?}", adapter);
            assert!(debug.contains("CurveAdapter"));
            assert!(debug.contains("curve"));
        }

        #[test]
        fn to_smallest_unit() {
            let adapter = CurveAdapter::new(test_config()).unwrap();
            let amount = Decimal::from_str("1.5").unwrap();
            let result = adapter.to_smallest_unit(amount, 18);
            assert_eq!(result, "1500000000000000000");
        }

        #[test]
        fn to_smallest_unit_6_decimals() {
            let adapter = CurveAdapter::new(test_config()).unwrap();
            let amount = Decimal::from_str("100.0").unwrap();
            let result = adapter.to_smallest_unit(amount, 6);
            assert_eq!(result, "100000000");
        }

        #[test]
        fn calculate_min_amount_out() {
            let adapter = CurveAdapter::new(test_config()).unwrap();
            // 50 bps slippage = 0.5%
            let min = adapter
                .calculate_min_amount_out("1000000000000000000")
                .unwrap();
            // 1e18 * 0.995 = 995000000000000000
            assert_eq!(min, "995000000000000000");
        }

        #[test]
        fn calculate_price() {
            let adapter = CurveAdapter::new(test_config()).unwrap();
            let price = adapter.calculate_price("1000000000", "2000000000").unwrap();
            assert!((price.get().to_f64().unwrap() - 2.0).abs() < 0.0001);
        }
    }

    mod venue_adapter {
        use super::*;

        #[tokio::test]
        async fn health_check_disabled() {
            let config = test_config().with_enabled(false);
            let adapter = CurveAdapter::new(config).unwrap();
            let health = adapter.health_check().await.unwrap();
            assert!(!health.is_healthy());
        }

        #[tokio::test]
        async fn is_available() {
            let adapter = CurveAdapter::new(test_config()).unwrap();
            assert!(adapter.is_available().await);

            let disabled_adapter = CurveAdapter::new(test_config().with_enabled(false)).unwrap();
            assert!(!disabled_adapter.is_available().await);
        }

        #[tokio::test]
        async fn venue_id() {
            let adapter = CurveAdapter::new(test_config()).unwrap();
            assert_eq!(adapter.venue_id(), &VenueId::new("curve"));
        }

        #[tokio::test]
        async fn timeout_ms() {
            let adapter = CurveAdapter::new(test_config()).unwrap();
            assert_eq!(adapter.timeout_ms(), 5000);
        }
    }
}
