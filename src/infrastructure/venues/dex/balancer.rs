//! # Balancer V2 Adapter
//!
//! Adapter for Balancer V2 DEX with weighted and stable pools.
//!
//! This module provides the [`BalancerAdapter`] which implements the
//! [`VenueAdapter`] trait for Balancer V2 protocol.
//!
//! # Features
//!
//! - Multi-chain support (Ethereum, Polygon, Arbitrum, Optimism, Avalanche, Base)
//! - Pool discovery via subgraph or registry
//! - Weighted pool support
//! - Stable pool support
//! - Composable stable pool support
//! - Batch swap support
//! - Gas estimation
//! - Slippage protection
//!
//! # Examples
//!
//! ```ignore
//! use otc_rfq::infrastructure::venues::dex::balancer::{BalancerAdapter, BalancerConfig};
//!
//! let config = BalancerConfig::new("https://eth-mainnet.g.alchemy.com/v2/...")
//!     .with_chain(BalancerChain::Ethereum);
//!
//! let adapter = BalancerAdapter::new(config);
//! ```

use crate::domain::entities::quote::{Quote, QuoteBuilder, QuoteMetadata};
use crate::domain::entities::rfq::Rfq;
use crate::domain::value_objects::timestamp::Timestamp;
use crate::domain::value_objects::{Blockchain, OrderSide, Price, VenueId};
use crate::infrastructure::venues::error::{VenueError, VenueResult};
use crate::infrastructure::venues::traits::{ExecutionResult, VenueAdapter, VenueHealth};
use async_trait::async_trait;
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

/// Balancer pool types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BalancerPoolType {
    /// Weighted pool with customizable token weights.
    #[default]
    Weighted,
    /// Stable pool optimized for pegged assets.
    Stable,
    /// Composable stable pool with nested pools.
    ComposableStable,
    /// Linear pool for wrapped tokens.
    Linear,
    /// Liquidity bootstrapping pool.
    LiquidityBootstrapping,
    /// Managed pool with dynamic weights.
    Managed,
}

impl BalancerPoolType {
    /// Returns the pool type name.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::Weighted => "weighted",
            Self::Stable => "stable",
            Self::ComposableStable => "composable_stable",
            Self::Linear => "linear",
            Self::LiquidityBootstrapping => "liquidity_bootstrapping",
            Self::Managed => "managed",
        }
    }

    /// Returns true if this is a stable pool type.
    #[must_use]
    pub fn is_stable(&self) -> bool {
        matches!(self, Self::Stable | Self::ComposableStable)
    }

    /// Returns true if this is a weighted pool type.
    #[must_use]
    pub fn is_weighted(&self) -> bool {
        matches!(
            self,
            Self::Weighted | Self::LiquidityBootstrapping | Self::Managed
        )
    }

    /// Returns all pool types.
    #[must_use]
    pub fn all() -> &'static [BalancerPoolType] {
        &[
            BalancerPoolType::Weighted,
            BalancerPoolType::Stable,
            BalancerPoolType::ComposableStable,
            BalancerPoolType::Linear,
            BalancerPoolType::LiquidityBootstrapping,
            BalancerPoolType::Managed,
        ]
    }
}

impl fmt::Display for BalancerPoolType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Swap kind for Balancer swaps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum SwapKind {
    /// Exact amount in, variable amount out.
    #[default]
    GivenIn,
    /// Variable amount in, exact amount out.
    GivenOut,
}

impl SwapKind {
    /// Returns the swap kind value for contract calls.
    #[must_use]
    pub fn value(&self) -> u8 {
        match self {
            Self::GivenIn => 0,
            Self::GivenOut => 1,
        }
    }
}

impl fmt::Display for SwapKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GivenIn => write!(f, "GIVEN_IN"),
            Self::GivenOut => write!(f, "GIVEN_OUT"),
        }
    }
}

/// Balancer Vault contract addresses by chain.
pub mod vault_contracts {
    /// Ethereum mainnet Vault.
    pub const ETHEREUM: &str = "0xBA12222222228d8Ba445958a75a0704d566BF2C8";
    /// Polygon Vault.
    pub const POLYGON: &str = "0xBA12222222228d8Ba445958a75a0704d566BF2C8";
    /// Arbitrum Vault.
    pub const ARBITRUM: &str = "0xBA12222222228d8Ba445958a75a0704d566BF2C8";
    /// Optimism Vault.
    pub const OPTIMISM: &str = "0xBA12222222228d8Ba445958a75a0704d566BF2C8";
    /// Avalanche Vault.
    pub const AVALANCHE: &str = "0xBA12222222228d8Ba445958a75a0704d566BF2C8";
    /// Base Vault.
    pub const BASE: &str = "0xBA12222222228d8Ba445958a75a0704d566BF2C8";
}

/// Balancer Query contract addresses by chain.
pub mod query_contracts {
    /// Ethereum mainnet Query.
    pub const ETHEREUM: &str = "0xE39B5e3B6D74016b2F6A9673D7d7493B6DF549d5";
    /// Polygon Query.
    pub const POLYGON: &str = "0xE39B5e3B6D74016b2F6A9673D7d7493B6DF549d5";
    /// Arbitrum Query.
    pub const ARBITRUM: &str = "0xE39B5e3B6D74016b2F6A9673D7d7493B6DF549d5";
    /// Optimism Query.
    pub const OPTIMISM: &str = "0xE39B5e3B6D74016b2F6A9673D7d7493B6DF549d5";
    /// Avalanche Query.
    pub const AVALANCHE: &str = "0xE39B5e3B6D74016b2F6A9673D7d7493B6DF549d5";
    /// Base Query.
    pub const BASE: &str = "0xE39B5e3B6D74016b2F6A9673D7d7493B6DF549d5";
}

/// Supported blockchain chains for Balancer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BalancerChain {
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
    /// Base (chain ID 8453).
    Base,
}

impl BalancerChain {
    /// Returns the chain ID.
    #[must_use]
    pub fn chain_id(&self) -> u64 {
        match self {
            Self::Ethereum => 1,
            Self::Polygon => 137,
            Self::Arbitrum => 42161,
            Self::Optimism => 10,
            Self::Avalanche => 43114,
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
            Self::Avalanche => "avalanche",
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
            // Avalanche and Base not in domain model yet
            Self::Avalanche | Self::Base => None,
        }
    }

    /// Returns the Vault contract address for this chain.
    #[must_use]
    pub fn vault_contract(&self) -> &'static str {
        match self {
            Self::Ethereum => vault_contracts::ETHEREUM,
            Self::Polygon => vault_contracts::POLYGON,
            Self::Arbitrum => vault_contracts::ARBITRUM,
            Self::Optimism => vault_contracts::OPTIMISM,
            Self::Avalanche => vault_contracts::AVALANCHE,
            Self::Base => vault_contracts::BASE,
        }
    }

    /// Returns the Query contract address for this chain.
    #[must_use]
    pub fn query_contract(&self) -> &'static str {
        match self {
            Self::Ethereum => query_contracts::ETHEREUM,
            Self::Polygon => query_contracts::POLYGON,
            Self::Arbitrum => query_contracts::ARBITRUM,
            Self::Optimism => query_contracts::OPTIMISM,
            Self::Avalanche => query_contracts::AVALANCHE,
            Self::Base => query_contracts::BASE,
        }
    }

    /// Returns the subgraph URL for this chain.
    #[must_use]
    pub fn subgraph_url(&self) -> &'static str {
        match self {
            Self::Ethereum => "https://api.thegraph.com/subgraphs/name/balancer-labs/balancer-v2",
            Self::Polygon => {
                "https://api.thegraph.com/subgraphs/name/balancer-labs/balancer-polygon-v2"
            }
            Self::Arbitrum => {
                "https://api.thegraph.com/subgraphs/name/balancer-labs/balancer-arbitrum-v2"
            }
            Self::Optimism => {
                "https://api.thegraph.com/subgraphs/name/balancer-labs/balancer-optimism-v2"
            }
            Self::Avalanche => {
                "https://api.thegraph.com/subgraphs/name/balancer-labs/balancer-avalanche-v2"
            }
            Self::Base => "https://api.thegraph.com/subgraphs/name/balancer-labs/balancer-base-v2",
        }
    }

    /// Returns all supported chains.
    #[must_use]
    pub fn all() -> &'static [BalancerChain] {
        &[
            BalancerChain::Ethereum,
            BalancerChain::Polygon,
            BalancerChain::Arbitrum,
            BalancerChain::Optimism,
            BalancerChain::Avalanche,
            BalancerChain::Base,
        ]
    }
}

impl fmt::Display for BalancerChain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Pool information from Balancer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BalancerPoolInfo {
    /// Pool ID (32 bytes).
    pub id: String,
    /// Pool address.
    pub address: String,
    /// Pool type.
    pub pool_type: BalancerPoolType,
    /// Token addresses in the pool.
    pub tokens: Vec<String>,
    /// Token weights (for weighted pools).
    pub weights: Option<Vec<String>>,
    /// Swap fee in basis points.
    pub swap_fee_bps: u32,
    /// Total liquidity in USD.
    pub total_liquidity: Option<String>,
}

/// Single swap step for batch swaps.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchSwapStep {
    /// Pool ID.
    pub pool_id: String,
    /// Index of asset in.
    pub asset_in_index: u32,
    /// Index of asset out.
    pub asset_out_index: u32,
    /// Amount (depends on swap kind).
    pub amount: String,
    /// User data for pool.
    pub user_data: String,
}

/// Quote result from Balancer query.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BalancerQuoteResult {
    /// Amount out (for GIVEN_IN) or amount in (for GIVEN_OUT).
    pub amount: String,
    /// Pool ID used.
    pub pool_id: String,
    /// Swap kind.
    pub swap_kind: SwapKind,
    /// Gas estimate.
    pub gas_estimate: Option<String>,
}

/// Swap route for multi-hop routing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BalancerSwapRoute {
    /// Batch swap steps.
    pub steps: Vec<BatchSwapStep>,
    /// Assets involved (addresses).
    pub assets: Vec<String>,
    /// Swap kind.
    pub swap_kind: SwapKind,
}

impl BalancerSwapRoute {
    /// Creates a direct swap route (single pool).
    #[must_use]
    pub fn direct(
        pool_id: impl Into<String>,
        token_in: impl Into<String>,
        token_out: impl Into<String>,
        amount: impl Into<String>,
        swap_kind: SwapKind,
    ) -> Self {
        let token_in = token_in.into();
        let token_out = token_out.into();
        let assets = vec![token_in.clone(), token_out.clone()];

        let step = BatchSwapStep {
            pool_id: pool_id.into(),
            asset_in_index: 0,
            asset_out_index: 1,
            amount: amount.into(),
            user_data: "0x".to_string(),
        };

        Self {
            steps: vec![step],
            assets,
            swap_kind,
        }
    }

    /// Returns true if this is a direct swap (single pool).
    #[must_use]
    pub fn is_direct(&self) -> bool {
        self.steps.len() == 1
    }

    /// Returns the number of hops.
    #[must_use]
    pub fn hops(&self) -> usize {
        self.steps.len()
    }
}

/// Configuration for the Balancer adapter.
///
/// # Examples
///
/// ```
/// use otc_rfq::infrastructure::venues::dex::balancer::{BalancerConfig, BalancerChain};
///
/// let config = BalancerConfig::new("https://eth-mainnet.g.alchemy.com/v2/...")
///     .with_chain(BalancerChain::Polygon)
///     .with_slippage_bps(100);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalancerConfig {
    /// Venue ID for this adapter.
    venue_id: VenueId,
    /// RPC URL for the chain.
    rpc_url: String,
    /// Target blockchain.
    chain: BalancerChain,
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
    /// Default swap kind.
    swap_kind: SwapKind,
}

impl BalancerConfig {
    /// Creates a new Balancer configuration.
    #[must_use]
    pub fn new(rpc_url: impl Into<String>) -> Self {
        Self {
            venue_id: VenueId::new("balancer"),
            rpc_url: rpc_url.into(),
            chain: BalancerChain::default(),
            timeout_ms: DEFAULT_TIMEOUT_MS,
            enabled: true,
            wallet_address: None,
            token_addresses: Self::default_token_addresses(),
            slippage_bps: DEFAULT_SLIPPAGE_BPS,
            quote_validity_secs: DEFAULT_QUOTE_VALIDITY_SECS,
            swap_kind: SwapKind::default(),
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
        map.insert(
            "BAL".to_string(),
            "0xba100000625a3754423978a60c9317c58a424e3D".to_string(),
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
    pub fn with_chain(mut self, chain: BalancerChain) -> Self {
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

    /// Sets the default swap kind.
    #[must_use]
    pub fn with_swap_kind(mut self, swap_kind: SwapKind) -> Self {
        self.swap_kind = swap_kind;
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
    pub fn chain(&self) -> BalancerChain {
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

    /// Returns the default swap kind.
    #[inline]
    #[must_use]
    pub fn swap_kind(&self) -> SwapKind {
        self.swap_kind
    }

    /// Returns the Vault contract address.
    #[inline]
    #[must_use]
    pub fn vault_contract(&self) -> &'static str {
        self.chain.vault_contract()
    }

    /// Returns the Query contract address.
    #[inline]
    #[must_use]
    pub fn query_contract(&self) -> &'static str {
        self.chain.query_contract()
    }

    /// Returns the subgraph URL.
    #[inline]
    #[must_use]
    pub fn subgraph_url(&self) -> &'static str {
        self.chain.subgraph_url()
    }
}

/// Balancer V2 DEX adapter.
///
/// Implements the [`VenueAdapter`] trait for Balancer V2 Protocol.
///
/// # Features
///
/// - Multi-chain support
/// - Pool discovery via subgraph
/// - Weighted and stable pools
/// - Batch swap support
/// - Gas estimation
/// - Slippage protection
///
/// # Note
///
/// This is a stub implementation. Full contract integration
/// will be completed with ethers-rs.
pub struct BalancerAdapter {
    /// Configuration.
    config: BalancerConfig,
}

impl BalancerAdapter {
    /// Creates a new Balancer adapter.
    #[must_use]
    pub fn new(config: BalancerConfig) -> Self {
        Self { config }
    }

    /// Returns the configuration.
    #[inline]
    #[must_use]
    pub fn config(&self) -> &BalancerConfig {
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
        quote_result: &BalancerQuoteResult,
        route: &BalancerSwapRoute,
    ) -> VenueResult<Quote> {
        let price = self.calculate_price(amount_in, &quote_result.amount)?;

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
        metadata.set("amount_out", quote_result.amount.clone());
        metadata.set(
            "min_amount_out",
            self.calculate_min_amount_out(&quote_result.amount)?,
        );
        metadata.set("pool_id", quote_result.pool_id.clone());
        metadata.set("swap_kind", quote_result.swap_kind.to_string());
        metadata.set("chain_id", self.config.chain().chain_id().to_string());
        metadata.set("vault", self.config.vault_contract().to_string());
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
            "pool_id",
            "vault",
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
}

impl fmt::Debug for BalancerAdapter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BalancerAdapter")
            .field("venue_id", self.config.venue_id())
            .field("chain", &self.config.chain())
            .field("enabled", &self.config.is_enabled())
            .finish()
    }
}

#[async_trait]
impl VenueAdapter for BalancerAdapter {
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
                "Balancer adapter is disabled",
            ));
        }

        // Resolve tokens
        let (_token_in, _token_out) = self.resolve_tokens(rfq)?;

        // TODO: Query subgraph for best pool
        // TODO: Call queryBatchSwap on Query contract
        // For now, return a stub error indicating not implemented
        Err(VenueError::internal_error(
            "Balancer integration not yet implemented - requires ethers-rs contract calls",
        ))
    }

    async fn execute_trade(&self, quote: &Quote) -> VenueResult<ExecutionResult> {
        // Check if enabled
        if !self.config.is_enabled() {
            return Err(VenueError::venue_unavailable(
                self.config.venue_id().clone(),
                "Balancer adapter is disabled",
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

        // TODO: Execute via Vault contract batchSwap
        Err(VenueError::internal_error(
            "Balancer execution not yet implemented - requires on-chain transaction",
        ))
    }

    async fn health_check(&self) -> VenueResult<VenueHealth> {
        // TODO: Check RPC connection and contract availability
        // For now, return healthy if enabled
        if self.config.is_enabled() {
            Ok(VenueHealth::healthy(self.config.venue_id().clone()))
        } else {
            Ok(VenueHealth::unhealthy(
                self.config.venue_id().clone(),
                "Adapter is disabled",
            ))
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

    fn test_config() -> BalancerConfig {
        BalancerConfig::new("https://eth-mainnet.example.com")
            .with_chain(BalancerChain::Ethereum)
            .with_wallet_address("0x1234567890abcdef1234567890abcdef12345678")
            .with_timeout_ms(5000)
    }

    mod pool_type {
        use super::*;

        #[test]
        fn names() {
            assert_eq!(BalancerPoolType::Weighted.name(), "weighted");
            assert_eq!(BalancerPoolType::Stable.name(), "stable");
            assert_eq!(
                BalancerPoolType::ComposableStable.name(),
                "composable_stable"
            );
            assert_eq!(BalancerPoolType::Linear.name(), "linear");
            assert_eq!(
                BalancerPoolType::LiquidityBootstrapping.name(),
                "liquidity_bootstrapping"
            );
            assert_eq!(BalancerPoolType::Managed.name(), "managed");
        }

        #[test]
        fn is_stable() {
            assert!(!BalancerPoolType::Weighted.is_stable());
            assert!(BalancerPoolType::Stable.is_stable());
            assert!(BalancerPoolType::ComposableStable.is_stable());
            assert!(!BalancerPoolType::Linear.is_stable());
        }

        #[test]
        fn is_weighted() {
            assert!(BalancerPoolType::Weighted.is_weighted());
            assert!(!BalancerPoolType::Stable.is_weighted());
            assert!(BalancerPoolType::LiquidityBootstrapping.is_weighted());
            assert!(BalancerPoolType::Managed.is_weighted());
        }

        #[test]
        fn display() {
            assert_eq!(BalancerPoolType::Weighted.to_string(), "weighted");
            assert_eq!(BalancerPoolType::Stable.to_string(), "stable");
        }

        #[test]
        fn default_is_weighted() {
            assert_eq!(BalancerPoolType::default(), BalancerPoolType::Weighted);
        }

        #[test]
        fn all_types() {
            let types = BalancerPoolType::all();
            assert_eq!(types.len(), 6);
        }
    }

    mod swap_kind {
        use super::*;

        #[test]
        fn values() {
            assert_eq!(SwapKind::GivenIn.value(), 0);
            assert_eq!(SwapKind::GivenOut.value(), 1);
        }

        #[test]
        fn display() {
            assert_eq!(SwapKind::GivenIn.to_string(), "GIVEN_IN");
            assert_eq!(SwapKind::GivenOut.to_string(), "GIVEN_OUT");
        }

        #[test]
        fn default_is_given_in() {
            assert_eq!(SwapKind::default(), SwapKind::GivenIn);
        }
    }

    mod chain {
        use super::*;

        #[test]
        fn chain_ids() {
            assert_eq!(BalancerChain::Ethereum.chain_id(), 1);
            assert_eq!(BalancerChain::Polygon.chain_id(), 137);
            assert_eq!(BalancerChain::Arbitrum.chain_id(), 42161);
            assert_eq!(BalancerChain::Optimism.chain_id(), 10);
            assert_eq!(BalancerChain::Avalanche.chain_id(), 43114);
            assert_eq!(BalancerChain::Base.chain_id(), 8453);
        }

        #[test]
        fn names() {
            assert_eq!(BalancerChain::Ethereum.name(), "ethereum");
            assert_eq!(BalancerChain::Polygon.name(), "polygon");
            assert_eq!(BalancerChain::Arbitrum.name(), "arbitrum");
            assert_eq!(BalancerChain::Optimism.name(), "optimism");
            assert_eq!(BalancerChain::Avalanche.name(), "avalanche");
            assert_eq!(BalancerChain::Base.name(), "base");
        }

        #[test]
        fn to_blockchain() {
            assert_eq!(
                BalancerChain::Ethereum.to_blockchain(),
                Some(Blockchain::Ethereum)
            );
            assert_eq!(
                BalancerChain::Polygon.to_blockchain(),
                Some(Blockchain::Polygon)
            );
            assert_eq!(BalancerChain::Avalanche.to_blockchain(), None);
            assert_eq!(BalancerChain::Base.to_blockchain(), None);
        }

        #[test]
        fn display() {
            assert_eq!(BalancerChain::Ethereum.to_string(), "ethereum");
        }

        #[test]
        fn default_is_ethereum() {
            assert_eq!(BalancerChain::default(), BalancerChain::Ethereum);
        }

        #[test]
        fn all_chains() {
            let chains = BalancerChain::all();
            assert_eq!(chains.len(), 6);
        }

        #[test]
        fn contract_addresses() {
            assert!(!BalancerChain::Ethereum.vault_contract().is_empty());
            assert!(!BalancerChain::Ethereum.query_contract().is_empty());
            assert!(BalancerChain::Ethereum.vault_contract().starts_with("0x"));
        }

        #[test]
        fn subgraph_urls() {
            assert!(BalancerChain::Ethereum.subgraph_url().contains("balancer"));
            assert!(BalancerChain::Polygon.subgraph_url().contains("polygon"));
        }
    }

    mod config {
        use super::*;

        #[test]
        fn new_config() {
            let config = BalancerConfig::new("https://rpc.example.com");
            assert_eq!(config.rpc_url(), "https://rpc.example.com");
            assert_eq!(config.chain(), BalancerChain::Ethereum);
            assert_eq!(config.timeout_ms(), DEFAULT_TIMEOUT_MS);
            assert!(config.is_enabled());
            assert_eq!(config.slippage_bps(), DEFAULT_SLIPPAGE_BPS);
            assert_eq!(config.swap_kind(), SwapKind::GivenIn);
        }

        #[test]
        fn builder_pattern() {
            let config = BalancerConfig::new("https://rpc.example.com")
                .with_chain(BalancerChain::Polygon)
                .with_timeout_ms(3000)
                .with_enabled(false)
                .with_slippage_bps(100)
                .with_quote_validity_secs(120)
                .with_swap_kind(SwapKind::GivenOut)
                .with_wallet_address("0xabc");

            assert_eq!(config.chain(), BalancerChain::Polygon);
            assert_eq!(config.timeout_ms(), 3000);
            assert!(!config.is_enabled());
            assert_eq!(config.slippage_bps(), 100);
            assert_eq!(config.quote_validity_secs(), 120);
            assert_eq!(config.swap_kind(), SwapKind::GivenOut);
            assert_eq!(config.wallet_address(), Some("0xabc"));
        }

        #[test]
        fn venue_id() {
            let config =
                BalancerConfig::new("https://rpc.example.com").with_venue_id("custom-balancer");
            assert_eq!(config.venue_id(), &VenueId::new("custom-balancer"));
        }

        #[test]
        fn token_addresses() {
            let config =
                BalancerConfig::new("https://rpc.example.com").with_token_address("TEST", "0x123");
            assert_eq!(
                config.resolve_token_address("TEST"),
                Some(&"0x123".to_string())
            );
            assert!(config.resolve_token_address("UNKNOWN").is_none());
        }

        #[test]
        fn default_token_addresses() {
            let config = BalancerConfig::new("https://rpc.example.com");
            assert!(config.resolve_token_address("WETH").is_some());
            assert!(config.resolve_token_address("USDC").is_some());
            assert!(config.resolve_token_address("BAL").is_some());
        }

        #[test]
        fn contract_addresses() {
            let config =
                BalancerConfig::new("https://rpc.example.com").with_chain(BalancerChain::Polygon);
            assert!(!config.vault_contract().is_empty());
            assert!(!config.query_contract().is_empty());
            assert!(!config.subgraph_url().is_empty());
        }
    }

    mod swap_route {
        use super::*;

        #[test]
        fn direct_route() {
            let route = BalancerSwapRoute::direct(
                "0xpoolid",
                "0xtoken_in",
                "0xtoken_out",
                "1000000000000000000",
                SwapKind::GivenIn,
            );
            assert!(route.is_direct());
            assert_eq!(route.hops(), 1);
            assert_eq!(route.steps.len(), 1);
            assert_eq!(route.assets.len(), 2);
            assert_eq!(route.swap_kind, SwapKind::GivenIn);
        }

        #[test]
        fn direct_route_given_out() {
            let route = BalancerSwapRoute::direct(
                "0xpoolid",
                "0xtoken_in",
                "0xtoken_out",
                "1000000000000000000",
                SwapKind::GivenOut,
            );
            assert_eq!(route.swap_kind, SwapKind::GivenOut);
        }
    }

    mod adapter {
        use super::*;

        #[test]
        fn new_adapter() {
            let adapter = BalancerAdapter::new(test_config());
            assert_eq!(adapter.config().chain(), BalancerChain::Ethereum);
        }

        #[test]
        fn debug_impl() {
            let adapter = BalancerAdapter::new(test_config());
            let debug = format!("{:?}", adapter);
            assert!(debug.contains("BalancerAdapter"));
            assert!(debug.contains("balancer"));
        }

        #[test]
        fn to_smallest_unit() {
            let adapter = BalancerAdapter::new(test_config());
            let amount = Decimal::from_str("1.5").unwrap();
            let result = adapter.to_smallest_unit(amount, 18);
            assert_eq!(result, "1500000000000000000");
        }

        #[test]
        fn to_smallest_unit_6_decimals() {
            let adapter = BalancerAdapter::new(test_config());
            let amount = Decimal::from_str("100.0").unwrap();
            let result = adapter.to_smallest_unit(amount, 6);
            assert_eq!(result, "100000000");
        }

        #[test]
        fn calculate_min_amount_out() {
            let adapter = BalancerAdapter::new(test_config());
            // 50 bps slippage = 0.5%
            let min = adapter
                .calculate_min_amount_out("1000000000000000000")
                .unwrap();
            // 1e18 * 0.995 = 995000000000000000
            assert_eq!(min, "995000000000000000");
        }

        #[test]
        fn calculate_price() {
            let adapter = BalancerAdapter::new(test_config());
            let price = adapter.calculate_price("1000000000", "2000000000").unwrap();
            assert!((price.get().to_f64().unwrap() - 2.0).abs() < 0.0001);
        }
    }

    mod venue_adapter {
        use super::*;

        #[tokio::test]
        async fn health_check_enabled() {
            let adapter = BalancerAdapter::new(test_config());
            let health = adapter.health_check().await.unwrap();
            assert!(health.is_healthy());
        }

        #[tokio::test]
        async fn health_check_disabled() {
            let config = test_config().with_enabled(false);
            let adapter = BalancerAdapter::new(config);
            let health = adapter.health_check().await.unwrap();
            assert!(!health.is_healthy());
        }

        #[tokio::test]
        async fn is_available() {
            let adapter = BalancerAdapter::new(test_config());
            assert!(adapter.is_available().await);

            let disabled_adapter = BalancerAdapter::new(test_config().with_enabled(false));
            assert!(!disabled_adapter.is_available().await);
        }

        #[tokio::test]
        async fn venue_id() {
            let adapter = BalancerAdapter::new(test_config());
            assert_eq!(adapter.venue_id(), &VenueId::new("balancer"));
        }

        #[tokio::test]
        async fn timeout_ms() {
            let adapter = BalancerAdapter::new(test_config());
            assert_eq!(adapter.timeout_ms(), 5000);
        }
    }
}
