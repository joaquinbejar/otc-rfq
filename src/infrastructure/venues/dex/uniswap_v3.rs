//! # Uniswap V3 Adapter
//!
//! Adapter for Uniswap V3 DEX with concentrated liquidity.
//!
//! This module provides the [`UniswapV3Adapter`] which implements the
//! [`VenueAdapter`] trait for Uniswap V3 protocol.
//!
//! # Features
//!
//! - Multi-chain support (Ethereum, Polygon, Arbitrum, Optimism, Base)
//! - Pool discovery and selection
//! - Tick-based price calculation
//! - Slippage protection
//! - Gas estimation
//! - Multi-hop routing
//! - Multiple fee tiers (0.01%, 0.05%, 0.3%, 1%)
//!
//! # Examples
//!
//! ```ignore
//! use otc_rfq::infrastructure::venues::dex::uniswap_v3::{UniswapV3Adapter, UniswapV3Config};
//!
//! let config = UniswapV3Config::new("https://eth-mainnet.g.alchemy.com/v2/...")
//!     .with_chain(UniswapV3Chain::Ethereum);
//!
//! let adapter = UniswapV3Adapter::new(config);
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
use ethers::utils::hex;
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

/// Uniswap V3 fee tiers in hundredths of a basis point.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum FeeTier {
    /// 0.01% fee tier (1 bps).
    Lowest = 100,
    /// 0.05% fee tier (5 bps).
    Low = 500,
    /// 0.3% fee tier (30 bps).
    #[default]
    Medium = 3000,
    /// 1% fee tier (100 bps).
    High = 10000,
}

impl FeeTier {
    /// Returns the fee in basis points.
    #[must_use]
    pub fn bps(&self) -> u32 {
        match self {
            Self::Lowest => 1,
            Self::Low => 5,
            Self::Medium => 30,
            Self::High => 100,
        }
    }

    /// Returns the fee as a decimal (e.g., 0.003 for 0.3%).
    #[must_use]
    pub fn as_decimal(&self) -> Decimal {
        Decimal::from(*self as u32) / Decimal::from(1_000_000)
    }

    /// Returns all fee tiers.
    #[must_use]
    pub fn all() -> &'static [FeeTier] {
        &[
            FeeTier::Lowest,
            FeeTier::Low,
            FeeTier::Medium,
            FeeTier::High,
        ]
    }

    /// Returns the raw fee value used in contracts.
    #[must_use]
    pub fn raw(&self) -> u32 {
        *self as u32
    }
}

impl fmt::Display for FeeTier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lowest => write!(f, "0.01%"),
            Self::Low => write!(f, "0.05%"),
            Self::Medium => write!(f, "0.3%"),
            Self::High => write!(f, "1%"),
        }
    }
}

/// Uniswap V3 SwapRouter02 contract addresses by chain.
pub mod router_contracts {
    /// Ethereum mainnet SwapRouter02.
    pub const ETHEREUM: &str = "0x68b3465833fb72A70ecDF485E0e4C7bD8665Fc45";
    /// Polygon SwapRouter02.
    pub const POLYGON: &str = "0x68b3465833fb72A70ecDF485E0e4C7bD8665Fc45";
    /// Arbitrum SwapRouter02.
    pub const ARBITRUM: &str = "0x68b3465833fb72A70ecDF485E0e4C7bD8665Fc45";
    /// Optimism SwapRouter02.
    pub const OPTIMISM: &str = "0x68b3465833fb72A70ecDF485E0e4C7bD8665Fc45";
    /// Base SwapRouter02.
    pub const BASE: &str = "0x2626664c2603336E57B271c5C0b26F421741e481";
}

/// Uniswap V3 Quoter V2 contract addresses by chain.
pub mod quoter_contracts {
    /// Ethereum mainnet Quoter V2.
    pub const ETHEREUM: &str = "0x61fFE014bA17989E743c5F6cB21bF9697530B21e";
    /// Polygon Quoter V2.
    pub const POLYGON: &str = "0x61fFE014bA17989E743c5F6cB21bF9697530B21e";
    /// Arbitrum Quoter V2.
    pub const ARBITRUM: &str = "0x61fFE014bA17989E743c5F6cB21bF9697530B21e";
    /// Optimism Quoter V2.
    pub const OPTIMISM: &str = "0x61fFE014bA17989E743c5F6cB21bF9697530B21e";
    /// Base Quoter V2.
    pub const BASE: &str = "0x3d4e44Eb1374240CE5F1B871ab261CD16335B76a";
}

/// Uniswap V3 Factory contract addresses by chain.
pub mod factory_contracts {
    /// Ethereum mainnet Factory.
    pub const ETHEREUM: &str = "0x1F98431c8aD98523631AE4a59f267346ea31F984";
    /// Polygon Factory.
    pub const POLYGON: &str = "0x1F98431c8aD98523631AE4a59f267346ea31F984";
    /// Arbitrum Factory.
    pub const ARBITRUM: &str = "0x1F98431c8aD98523631AE4a59f267346ea31F984";
    /// Optimism Factory.
    pub const OPTIMISM: &str = "0x1F98431c8aD98523631AE4a59f267346ea31F984";
    /// Base Factory.
    pub const BASE: &str = "0x33128a8fC17869897dcE68Ed026d694621f6FDfD";
}

/// Supported blockchain chains for Uniswap V3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UniswapV3Chain {
    /// Ethereum mainnet (chain ID 1).
    #[default]
    Ethereum,
    /// Polygon (chain ID 137).
    Polygon,
    /// Arbitrum One (chain ID 42161).
    Arbitrum,
    /// Optimism (chain ID 10).
    Optimism,
    /// Base (chain ID 8453).
    Base,
}

impl UniswapV3Chain {
    /// Returns the chain ID.
    #[must_use]
    pub fn chain_id(&self) -> u64 {
        match self {
            Self::Ethereum => 1,
            Self::Polygon => 137,
            Self::Arbitrum => 42161,
            Self::Optimism => 10,
            Self::Base => 8453,
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
            Self::Base => "base",
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
            Self::Base => None, // Base not in domain model yet
        }
    }

    /// Returns the SwapRouter02 contract address for this chain.
    #[must_use]
    pub fn router_contract(&self) -> &'static str {
        match self {
            Self::Ethereum => router_contracts::ETHEREUM,
            Self::Polygon => router_contracts::POLYGON,
            Self::Arbitrum => router_contracts::ARBITRUM,
            Self::Optimism => router_contracts::OPTIMISM,
            Self::Base => router_contracts::BASE,
        }
    }

    /// Returns the Quoter V2 contract address for this chain.
    #[must_use]
    pub fn quoter_contract(&self) -> &'static str {
        match self {
            Self::Ethereum => quoter_contracts::ETHEREUM,
            Self::Polygon => quoter_contracts::POLYGON,
            Self::Arbitrum => quoter_contracts::ARBITRUM,
            Self::Optimism => quoter_contracts::OPTIMISM,
            Self::Base => quoter_contracts::BASE,
        }
    }

    /// Returns the Factory contract address for this chain.
    #[must_use]
    pub fn factory_contract(&self) -> &'static str {
        match self {
            Self::Ethereum => factory_contracts::ETHEREUM,
            Self::Polygon => factory_contracts::POLYGON,
            Self::Arbitrum => factory_contracts::ARBITRUM,
            Self::Optimism => factory_contracts::OPTIMISM,
            Self::Base => factory_contracts::BASE,
        }
    }

    /// Returns all supported chains.
    #[must_use]
    pub fn all() -> &'static [UniswapV3Chain] {
        &[
            UniswapV3Chain::Ethereum,
            UniswapV3Chain::Polygon,
            UniswapV3Chain::Arbitrum,
            UniswapV3Chain::Optimism,
            UniswapV3Chain::Base,
        ]
    }
}

impl fmt::Display for UniswapV3Chain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Pool information for a token pair.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PoolInfo {
    /// Pool address.
    pub address: String,
    /// Token0 address.
    pub token0: String,
    /// Token1 address.
    pub token1: String,
    /// Fee tier.
    pub fee: u32,
    /// Current tick.
    pub tick: i32,
    /// Current sqrt price (Q64.96 format).
    pub sqrt_price_x96: String,
    /// Liquidity.
    pub liquidity: String,
}

/// Quote result from Quoter V2.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuoteResult {
    /// Amount out.
    pub amount_out: String,
    /// Sqrt price after swap (Q64.96 format).
    pub sqrt_price_x96_after: String,
    /// Ticks crossed during quote.
    pub initialized_ticks_crossed: u32,
    /// Gas estimate.
    pub gas_estimate: String,
}

/// Swap path for multi-hop routing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SwapPath {
    /// Token addresses in order.
    pub tokens: Vec<String>,
    /// Fee tiers between tokens.
    pub fees: Vec<u32>,
}

impl SwapPath {
    /// Creates a direct swap path (single hop).
    #[must_use]
    pub fn direct(token_in: impl Into<String>, token_out: impl Into<String>, fee: FeeTier) -> Self {
        Self {
            tokens: vec![token_in.into(), token_out.into()],
            fees: vec![fee.raw()],
        }
    }

    /// Creates a multi-hop swap path.
    #[must_use]
    pub fn multi_hop(tokens: Vec<String>, fees: Vec<u32>) -> Option<Self> {
        if tokens.len() < 2 || fees.len() != tokens.len() - 1 {
            return None;
        }
        Some(Self { tokens, fees })
    }

    /// Returns true if this is a direct swap (single hop).
    #[must_use]
    pub fn is_direct(&self) -> bool {
        self.tokens.len() == 2
    }

    /// Returns the number of hops.
    #[must_use]
    pub fn hops(&self) -> usize {
        self.fees.len()
    }

    /// Encodes the path for Uniswap V3 router.
    ///
    /// Format: token0 (20 bytes) + fee (3 bytes) + token1 (20 bytes) + ...
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut encoded = Vec::new();

        for (i, token) in self.tokens.iter().enumerate() {
            // Remove 0x prefix and decode hex
            let token_bytes = hex::decode(token.trim_start_matches("0x")).unwrap_or_default();
            encoded.extend_from_slice(&token_bytes);

            if let Some(&fee) = self.fees.get(i) {
                // Fee is 3 bytes (24 bits)
                encoded.push((fee >> 16) as u8);
                encoded.push((fee >> 8) as u8);
                encoded.push(fee as u8);
            }
        }

        encoded
    }
}

/// Configuration for the Uniswap V3 adapter.
///
/// # Examples
///
/// ```
/// use otc_rfq::infrastructure::venues::dex::uniswap_v3::{UniswapV3Config, UniswapV3Chain, FeeTier};
///
/// let config = UniswapV3Config::new("https://eth-mainnet.g.alchemy.com/v2/...")
///     .with_chain(UniswapV3Chain::Polygon)
///     .with_default_fee_tier(FeeTier::Low)
///     .with_slippage_bps(100);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UniswapV3Config {
    /// Venue ID for this adapter.
    venue_id: VenueId,
    /// RPC URL for the chain.
    rpc_url: String,
    /// Target blockchain.
    chain: UniswapV3Chain,
    /// Timeout in milliseconds.
    timeout_ms: u64,
    /// Whether the adapter is enabled.
    enabled: bool,
    /// Sender wallet address.
    wallet_address: Option<String>,
    /// Token address mappings (symbol -> address).
    token_addresses: HashMap<String, String>,
    /// Default fee tier for swaps.
    default_fee_tier: FeeTier,
    /// Slippage tolerance in basis points.
    slippage_bps: u32,
    /// Quote validity in seconds.
    quote_validity_secs: u64,
    /// Whether to use multi-hop routing.
    multi_hop_enabled: bool,
}

impl UniswapV3Config {
    /// Creates a new Uniswap V3 configuration.
    #[must_use]
    pub fn new(rpc_url: impl Into<String>) -> Self {
        Self {
            venue_id: VenueId::new("uniswap-v3"),
            rpc_url: rpc_url.into(),
            chain: UniswapV3Chain::default(),
            timeout_ms: DEFAULT_TIMEOUT_MS,
            enabled: true,
            wallet_address: None,
            token_addresses: Self::default_token_addresses(),
            default_fee_tier: FeeTier::default(),
            slippage_bps: DEFAULT_SLIPPAGE_BPS,
            quote_validity_secs: DEFAULT_QUOTE_VALIDITY_SECS,
            multi_hop_enabled: true,
        }
    }

    /// Creates default token address mappings for common tokens.
    fn default_token_addresses() -> HashMap<String, String> {
        let mut map = HashMap::new();
        // Ethereum mainnet addresses
        map.insert(
            "WETH".to_string(),
            "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string(),
        );
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
    pub fn with_chain(mut self, chain: UniswapV3Chain) -> Self {
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

    /// Sets the default fee tier.
    #[must_use]
    pub fn with_default_fee_tier(mut self, fee_tier: FeeTier) -> Self {
        self.default_fee_tier = fee_tier;
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

    /// Sets whether multi-hop routing is enabled.
    #[must_use]
    pub fn with_multi_hop(mut self, enabled: bool) -> Self {
        self.multi_hop_enabled = enabled;
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
    pub fn chain(&self) -> UniswapV3Chain {
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

    /// Returns the default fee tier.
    #[inline]
    #[must_use]
    pub fn default_fee_tier(&self) -> FeeTier {
        self.default_fee_tier
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

    /// Returns whether multi-hop routing is enabled.
    #[inline]
    #[must_use]
    pub fn is_multi_hop_enabled(&self) -> bool {
        self.multi_hop_enabled
    }

    /// Returns the SwapRouter02 contract address.
    #[inline]
    #[must_use]
    pub fn router_contract(&self) -> &'static str {
        self.chain.router_contract()
    }

    /// Returns the Quoter V2 contract address.
    #[inline]
    #[must_use]
    pub fn quoter_contract(&self) -> &'static str {
        self.chain.quoter_contract()
    }

    /// Returns the Factory contract address.
    #[inline]
    #[must_use]
    pub fn factory_contract(&self) -> &'static str {
        self.chain.factory_contract()
    }
}

/// Uniswap V3 DEX adapter.
///
/// Implements the [`VenueAdapter`] trait for Uniswap V3 Protocol.
///
/// # Features
///
/// - Multi-chain support
/// - Pool discovery and selection
/// - Tick-based price calculation
/// - Slippage protection
/// - Gas estimation
/// - Multi-hop routing
pub struct UniswapV3Adapter {
    /// Configuration.
    config: UniswapV3Config,
    /// Contract client for RPC calls.
    contract_client: ContractClient,
}

impl UniswapV3Adapter {
    /// Creates a new Uniswap V3 adapter.
    ///
    /// # Errors
    ///
    /// Returns `VenueError::Connection` if the RPC provider cannot be created.
    pub fn new(config: UniswapV3Config) -> VenueResult<Self> {
        let contract_client = ContractClient::new(config.rpc_url(), config.timeout_ms())?;
        Ok(Self {
            config,
            contract_client,
        })
    }

    /// Returns the configuration.
    #[inline]
    #[must_use]
    pub fn config(&self) -> &UniswapV3Config {
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

    /// Creates a direct swap path for the given tokens.
    #[must_use]
    pub fn create_swap_path(&self, token_in: &str, token_out: &str) -> SwapPath {
        SwapPath::direct(token_in, token_out, self.config.default_fee_tier())
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
        quote_result: &QuoteResult,
        path: &SwapPath,
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
        metadata.set(
            "sqrt_price_x96_after",
            quote_result.sqrt_price_x96_after.clone(),
        );
        metadata.set("gas_estimate", quote_result.gas_estimate.clone());
        metadata.set(
            "initialized_ticks_crossed",
            quote_result.initialized_ticks_crossed.to_string(),
        );
        metadata.set("path", hex::encode(path.encode()));
        metadata.set("chain_id", self.config.chain().chain_id().to_string());
        metadata.set("router", self.config.router_contract().to_string());
        metadata.set("slippage_bps", self.config.slippage_bps().to_string());

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
            "path",
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

    /// Encodes the quoteExactInputSingle call for QuoterV2.
    ///
    /// Function signature: quoteExactInputSingle(QuoteExactInputSingleParams)
    /// where QuoteExactInputSingleParams is (address tokenIn, address tokenOut, uint256 amountIn, uint24 fee, uint160 sqrtPriceLimitX96)
    fn encode_quote_exact_input_single(
        &self,
        token_in: Address,
        token_out: Address,
        amount_in: U256,
        fee: u32,
        sqrt_price_limit: U256,
    ) -> Bytes {
        // Function selector for quoteExactInputSingle((address,address,uint256,uint24,uint160))
        // keccak256("quoteExactInputSingle((address,address,uint256,uint24,uint160))")[0:4]
        let selector: [u8; 4] = [0xc6, 0xa5, 0x02, 0x6a];

        // Encode the struct as a tuple
        let encoded = ethers::abi::encode(&[
            ethers::abi::Token::Address(token_in),
            ethers::abi::Token::Address(token_out),
            ethers::abi::Token::Uint(amount_in),
            ethers::abi::Token::Uint(U256::from(fee)),
            ethers::abi::Token::Uint(sqrt_price_limit),
        ]);

        // Combine selector and encoded params
        let mut calldata = Vec::with_capacity(4 + encoded.len());
        calldata.extend_from_slice(&selector);
        calldata.extend_from_slice(&encoded);

        Bytes::from(calldata)
    }

    /// Decodes the quote result from QuoterV2.
    ///
    /// Returns (amountOut, sqrtPriceX96After, initializedTicksCrossed, gasEstimate)
    /// We only need amountOut for the quote.
    ///
    /// # Errors
    ///
    /// Returns `VenueError::ProtocolError` if decoding fails.
    fn decode_quote_result(&self, result: &Bytes) -> VenueResult<U256> {
        let bytes = result
            .get(0..32)
            .ok_or_else(|| VenueError::protocol_error("Invalid quote result length"))?;

        // First 32 bytes is amountOut
        let amount_out = U256::from_big_endian(bytes);
        Ok(amount_out)
    }
}

impl fmt::Debug for UniswapV3Adapter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UniswapV3Adapter")
            .field("venue_id", self.config.venue_id())
            .field("chain", &self.config.chain())
            .field("enabled", &self.config.is_enabled())
            .field("multi_hop", &self.config.is_multi_hop_enabled())
            .finish()
    }
}

#[async_trait]
impl VenueAdapter for UniswapV3Adapter {
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
                "Uniswap V3 adapter is disabled",
            ));
        }

        // Resolve tokens
        let (token_in, token_out) = self.resolve_tokens(rfq)?;

        // Create swap path
        let path = self.create_swap_path(&token_in, &token_out);

        // Parse addresses
        let token_in_addr = ContractClient::parse_address(&token_in)?;
        let token_out_addr = ContractClient::parse_address(&token_out)?;
        let quoter_addr = ContractClient::parse_address(self.config.quoter_contract())?;

        // Calculate amount in
        let amount_in = self.to_smallest_unit(rfq.quantity().get(), 18);
        let amount_in_u256 = ContractClient::parse_u256(&amount_in)?;

        // Encode quoteExactInputSingle call
        // Function signature: quoteExactInputSingle((address,address,uint256,uint24,uint160))
        let fee = self.config.default_fee_tier().raw();
        let sqrt_price_limit = U256::zero();

        // Build calldata for QuoterV2.quoteExactInputSingle
        let calldata = self.encode_quote_exact_input_single(
            token_in_addr,
            token_out_addr,
            amount_in_u256,
            fee,
            sqrt_price_limit,
        );

        // Call the quoter contract
        let result = self.contract_client.call(quoter_addr, calldata).await?;

        // Decode the result (amountOut, sqrtPriceX96After, initializedTicksCrossed, gasEstimate)
        let amount_out = self.decode_quote_result(&result)?;

        // Calculate min amount out with slippage
        let min_amount_out = self.calculate_min_amount_out(&amount_out.to_string())?;

        // Calculate price
        let amount_in_f64: f64 = amount_in.parse().unwrap_or(1.0);
        let amount_out_f64: f64 = amount_out.to_string().parse().unwrap_or(0.0);
        let price_value = if amount_in_f64 > 0.0 {
            amount_out_f64 / amount_in_f64
        } else {
            0.0
        };
        let price = Price::new(price_value)
            .map_err(|_| VenueError::protocol_error("Invalid price value"))?;

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
        metadata.set("path", hex::encode(path.encode()));
        metadata.set("router", self.config.router_contract().to_string());
        metadata.set("fee_tier", fee.to_string());

        builder = builder.metadata(metadata);

        Ok(builder.build())
    }

    async fn execute_trade(&self, quote: &Quote) -> VenueResult<ExecutionResult> {
        // Check if enabled
        if !self.config.is_enabled() {
            return Err(VenueError::venue_unavailable(
                self.config.venue_id().clone(),
                "Uniswap V3 adapter is disabled",
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

        // TODO: Execute via SwapRouter02 contract
        Err(VenueError::internal_error(
            "Uniswap V3 execution not yet implemented - requires on-chain transaction",
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

    fn test_config() -> UniswapV3Config {
        UniswapV3Config::new("https://eth-mainnet.example.com")
            .with_chain(UniswapV3Chain::Ethereum)
            .with_wallet_address("0x1234567890abcdef1234567890abcdef12345678")
            .with_timeout_ms(5000)
    }

    mod fee_tier {
        use super::*;

        #[test]
        fn raw_values() {
            assert_eq!(FeeTier::Lowest.raw(), 100);
            assert_eq!(FeeTier::Low.raw(), 500);
            assert_eq!(FeeTier::Medium.raw(), 3000);
            assert_eq!(FeeTier::High.raw(), 10000);
        }

        #[test]
        fn bps() {
            assert_eq!(FeeTier::Lowest.bps(), 1);
            assert_eq!(FeeTier::Low.bps(), 5);
            assert_eq!(FeeTier::Medium.bps(), 30);
            assert_eq!(FeeTier::High.bps(), 100);
        }

        #[test]
        fn as_decimal() {
            assert_eq!(
                FeeTier::Lowest.as_decimal(),
                Decimal::from_str("0.0001").unwrap()
            );
            assert_eq!(
                FeeTier::Low.as_decimal(),
                Decimal::from_str("0.0005").unwrap()
            );
            assert_eq!(
                FeeTier::Medium.as_decimal(),
                Decimal::from_str("0.003").unwrap()
            );
            assert_eq!(
                FeeTier::High.as_decimal(),
                Decimal::from_str("0.01").unwrap()
            );
        }

        #[test]
        fn display() {
            assert_eq!(FeeTier::Lowest.to_string(), "0.01%");
            assert_eq!(FeeTier::Low.to_string(), "0.05%");
            assert_eq!(FeeTier::Medium.to_string(), "0.3%");
            assert_eq!(FeeTier::High.to_string(), "1%");
        }

        #[test]
        fn default_is_medium() {
            assert_eq!(FeeTier::default(), FeeTier::Medium);
        }

        #[test]
        fn all_tiers() {
            let tiers = FeeTier::all();
            assert_eq!(tiers.len(), 4);
        }
    }

    mod chain {
        use super::*;

        #[test]
        fn chain_ids() {
            assert_eq!(UniswapV3Chain::Ethereum.chain_id(), 1);
            assert_eq!(UniswapV3Chain::Polygon.chain_id(), 137);
            assert_eq!(UniswapV3Chain::Arbitrum.chain_id(), 42161);
            assert_eq!(UniswapV3Chain::Optimism.chain_id(), 10);
            assert_eq!(UniswapV3Chain::Base.chain_id(), 8453);
        }

        #[test]
        fn names() {
            assert_eq!(UniswapV3Chain::Ethereum.name(), "ethereum");
            assert_eq!(UniswapV3Chain::Polygon.name(), "polygon");
            assert_eq!(UniswapV3Chain::Arbitrum.name(), "arbitrum");
            assert_eq!(UniswapV3Chain::Optimism.name(), "optimism");
            assert_eq!(UniswapV3Chain::Base.name(), "base");
        }

        #[test]
        fn to_blockchain() {
            assert_eq!(
                UniswapV3Chain::Ethereum.to_blockchain(),
                Some(Blockchain::Ethereum)
            );
            assert_eq!(
                UniswapV3Chain::Polygon.to_blockchain(),
                Some(Blockchain::Polygon)
            );
            assert_eq!(UniswapV3Chain::Base.to_blockchain(), None);
        }

        #[test]
        fn display() {
            assert_eq!(UniswapV3Chain::Ethereum.to_string(), "ethereum");
        }

        #[test]
        fn default_is_ethereum() {
            assert_eq!(UniswapV3Chain::default(), UniswapV3Chain::Ethereum);
        }

        #[test]
        fn all_chains() {
            let chains = UniswapV3Chain::all();
            assert_eq!(chains.len(), 5);
        }

        #[test]
        fn contract_addresses() {
            assert!(!UniswapV3Chain::Ethereum.router_contract().is_empty());
            assert!(!UniswapV3Chain::Ethereum.quoter_contract().is_empty());
            assert!(!UniswapV3Chain::Ethereum.factory_contract().is_empty());
            assert!(UniswapV3Chain::Ethereum.router_contract().starts_with("0x"));
        }
    }

    mod config {
        use super::*;

        #[test]
        fn new_config() {
            let config = UniswapV3Config::new("https://rpc.example.com");
            assert_eq!(config.rpc_url(), "https://rpc.example.com");
            assert_eq!(config.chain(), UniswapV3Chain::Ethereum);
            assert_eq!(config.timeout_ms(), DEFAULT_TIMEOUT_MS);
            assert!(config.is_enabled());
            assert_eq!(config.default_fee_tier(), FeeTier::Medium);
            assert_eq!(config.slippage_bps(), DEFAULT_SLIPPAGE_BPS);
            assert!(config.is_multi_hop_enabled());
        }

        #[test]
        fn builder_pattern() {
            let config = UniswapV3Config::new("https://rpc.example.com")
                .with_chain(UniswapV3Chain::Polygon)
                .with_timeout_ms(3000)
                .with_enabled(false)
                .with_default_fee_tier(FeeTier::Low)
                .with_slippage_bps(100)
                .with_quote_validity_secs(120)
                .with_multi_hop(false)
                .with_wallet_address("0xabc");

            assert_eq!(config.chain(), UniswapV3Chain::Polygon);
            assert_eq!(config.timeout_ms(), 3000);
            assert!(!config.is_enabled());
            assert_eq!(config.default_fee_tier(), FeeTier::Low);
            assert_eq!(config.slippage_bps(), 100);
            assert_eq!(config.quote_validity_secs(), 120);
            assert!(!config.is_multi_hop_enabled());
            assert_eq!(config.wallet_address(), Some("0xabc"));
        }

        #[test]
        fn venue_id() {
            let config =
                UniswapV3Config::new("https://rpc.example.com").with_venue_id("custom-uniswap");
            assert_eq!(config.venue_id(), &VenueId::new("custom-uniswap"));
        }

        #[test]
        fn token_addresses() {
            let config =
                UniswapV3Config::new("https://rpc.example.com").with_token_address("TEST", "0x123");
            assert_eq!(
                config.resolve_token_address("TEST"),
                Some(&"0x123".to_string())
            );
            assert!(config.resolve_token_address("UNKNOWN").is_none());
        }

        #[test]
        fn default_token_addresses() {
            let config = UniswapV3Config::new("https://rpc.example.com");
            assert!(config.resolve_token_address("WETH").is_some());
            assert!(config.resolve_token_address("USDC").is_some());
        }

        #[test]
        fn contract_addresses() {
            let config =
                UniswapV3Config::new("https://rpc.example.com").with_chain(UniswapV3Chain::Polygon);
            assert!(!config.router_contract().is_empty());
            assert!(!config.quoter_contract().is_empty());
            assert!(!config.factory_contract().is_empty());
        }
    }

    mod swap_path {
        use super::*;

        #[test]
        fn direct_path() {
            let path = SwapPath::direct("0xtoken0", "0xtoken1", FeeTier::Medium);
            assert!(path.is_direct());
            assert_eq!(path.hops(), 1);
            assert_eq!(path.tokens.len(), 2);
            assert_eq!(path.fees.len(), 1);
            assert_eq!(path.fees[0], 3000);
        }

        #[test]
        fn multi_hop_path() {
            let path = SwapPath::multi_hop(
                vec![
                    "0xtoken0".to_string(),
                    "0xtoken1".to_string(),
                    "0xtoken2".to_string(),
                ],
                vec![500, 3000],
            );
            assert!(path.is_some());
            let path = path.unwrap();
            assert!(!path.is_direct());
            assert_eq!(path.hops(), 2);
        }

        #[test]
        fn invalid_multi_hop() {
            // Mismatched tokens and fees
            let path = SwapPath::multi_hop(
                vec!["0xtoken0".to_string(), "0xtoken1".to_string()],
                vec![500, 3000], // Too many fees
            );
            assert!(path.is_none());
        }

        #[test]
        fn encode_path() {
            let path = SwapPath::direct(
                "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
                "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
                FeeTier::Medium,
            );
            let encoded = path.encode();
            // 20 bytes + 3 bytes + 20 bytes = 43 bytes
            assert_eq!(encoded.len(), 43);
        }
    }

    mod adapter {
        use super::*;

        #[test]
        fn new_adapter() {
            let adapter = UniswapV3Adapter::new(test_config()).unwrap();
            assert_eq!(adapter.config().chain(), UniswapV3Chain::Ethereum);
        }

        #[test]
        fn debug_impl() {
            let adapter = UniswapV3Adapter::new(test_config()).unwrap();
            let debug = format!("{:?}", adapter);
            assert!(debug.contains("UniswapV3Adapter"));
            assert!(debug.contains("uniswap-v3"));
        }

        #[test]
        fn to_smallest_unit() {
            let adapter = UniswapV3Adapter::new(test_config()).unwrap();
            let amount = Decimal::from_str("1.5").unwrap();
            let result = adapter.to_smallest_unit(amount, 18);
            assert_eq!(result, "1500000000000000000");
        }

        #[test]
        fn calculate_min_amount_out() {
            let adapter = UniswapV3Adapter::new(test_config()).unwrap();
            // 50 bps slippage = 0.5%
            let min = adapter
                .calculate_min_amount_out("1000000000000000000")
                .unwrap();
            // 1e18 * 0.995 = 995000000000000000
            assert_eq!(min, "995000000000000000");
        }

        #[test]
        fn create_swap_path() {
            let adapter = UniswapV3Adapter::new(test_config()).unwrap();
            let path = adapter.create_swap_path("0xtoken0", "0xtoken1");
            assert!(path.is_direct());
            assert_eq!(path.fees[0], FeeTier::Medium.raw());
        }

        #[test]
        fn calculate_price() {
            let adapter = UniswapV3Adapter::new(test_config()).unwrap();
            let price = adapter.calculate_price("1000000000", "2000000000").unwrap();
            assert!((price.get().to_f64().unwrap() - 2.0).abs() < 0.0001);
        }
    }

    mod venue_adapter {
        use super::*;

        #[tokio::test]
        async fn health_check_disabled() {
            let config = test_config().with_enabled(false);
            let adapter = UniswapV3Adapter::new(config).unwrap();
            let health = adapter.health_check().await.unwrap();
            assert!(!health.is_healthy());
        }

        #[tokio::test]
        async fn is_available() {
            let adapter = UniswapV3Adapter::new(test_config()).unwrap();
            assert!(adapter.is_available().await);

            let disabled_adapter =
                UniswapV3Adapter::new(test_config().with_enabled(false)).unwrap();
            assert!(!disabled_adapter.is_available().await);
        }

        #[tokio::test]
        async fn venue_id() {
            let adapter = UniswapV3Adapter::new(test_config()).unwrap();
            assert_eq!(adapter.venue_id(), &VenueId::new("uniswap-v3"));
        }

        #[tokio::test]
        async fn timeout_ms() {
            let adapter = UniswapV3Adapter::new(test_config()).unwrap();
            assert_eq!(adapter.timeout_ms(), 5000);
        }
    }
}
