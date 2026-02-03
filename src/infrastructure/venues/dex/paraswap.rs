//! # Paraswap Adapter
//!
//! Adapter for Paraswap DEX aggregator.
//!
//! This module provides the [`ParaswapAdapter`] which implements the
//! [`VenueAdapter`] trait for the Paraswap DEX aggregator.
//!
//! # Features
//!
//! - HTTP client for Paraswap API (api.paraswap.io)
//! - Quote endpoint integration (/prices)
//! - Transaction builder endpoint (/transactions)
//! - Multi-chain support (Ethereum, Polygon, Arbitrum, etc.)
//! - Gas cost estimation
//! - Slippage configuration
//!
//! # Examples
//!
//! ```ignore
//! use otc_rfq::infrastructure::venues::dex::paraswap::{ParaswapAdapter, ParaswapConfig};
//!
//! let config = ParaswapConfig::new()
//!     .with_chain(ParaswapChain::Ethereum);
//!
//! let adapter = ParaswapAdapter::new(config);
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
const DEFAULT_TIMEOUT_MS: u64 = 5000;

/// Default quote validity in seconds.
const DEFAULT_QUOTE_VALIDITY_SECS: u64 = 60;

/// Base URL for Paraswap API.
const BASE_URL: &str = "https://api.paraswap.io";

/// Supported blockchain chains for Paraswap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ParaswapChain {
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
    /// BNB Smart Chain (chain ID 56).
    Bsc,
    /// Avalanche C-Chain (chain ID 43114).
    Avalanche,
    /// Fantom (chain ID 250).
    Fantom,
}

impl ParaswapChain {
    /// Returns the chain ID.
    #[must_use]
    pub fn chain_id(&self) -> u64 {
        match self {
            Self::Ethereum => 1,
            Self::Polygon => 137,
            Self::Arbitrum => 42161,
            Self::Optimism => 10,
            Self::Base => 8453,
            Self::Bsc => 56,
            Self::Avalanche => 43114,
            Self::Fantom => 250,
        }
    }

    /// Returns the network name for API calls.
    #[must_use]
    pub fn network_name(&self) -> &'static str {
        match self {
            Self::Ethereum => "ethereum",
            Self::Polygon => "polygon",
            Self::Arbitrum => "arbitrum",
            Self::Optimism => "optimism",
            Self::Base => "base",
            Self::Bsc => "bsc",
            Self::Avalanche => "avalanche",
            Self::Fantom => "fantom",
        }
    }

    /// Converts to domain Blockchain type.
    ///
    /// Returns `None` for chains not supported in the domain model.
    #[must_use]
    pub fn to_blockchain(&self) -> Option<Blockchain> {
        match self {
            Self::Ethereum => Some(Blockchain::Ethereum),
            Self::Polygon => Some(Blockchain::Polygon),
            Self::Arbitrum => Some(Blockchain::Arbitrum),
            Self::Optimism => Some(Blockchain::Optimism),
            Self::Base => Some(Blockchain::Base),
            // BSC, Avalanche, Fantom not in domain model yet
            Self::Bsc | Self::Avalanche | Self::Fantom => None,
        }
    }
}

impl fmt::Display for ParaswapChain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.network_name())
    }
}

/// Token information in a Paraswap response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParaswapToken {
    /// Token address.
    pub address: String,
    /// Token decimals.
    pub decimals: u8,
    /// Token symbol.
    pub symbol: Option<String>,
}

/// Price route from Paraswap.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParaswapPriceRoute {
    /// Block number at which the quote was generated.
    pub block_number: Option<u64>,
    /// Network ID.
    pub network: u64,
    /// Source token.
    pub src_token: String,
    /// Source token decimals.
    pub src_decimals: u8,
    /// Source amount.
    pub src_amount: String,
    /// Destination token.
    pub dest_token: String,
    /// Destination token decimals.
    pub dest_decimals: u8,
    /// Destination amount.
    pub dest_amount: String,
    /// Best route details.
    pub best_route: Option<Vec<ParaswapRoute>>,
    /// Gas cost in USD.
    pub gas_cost_usd: Option<String>,
    /// Gas cost in native token.
    pub gas_cost: Option<String>,
    /// Price impact percentage.
    pub price_impact: Option<String>,
    /// Partner fee percentage.
    pub partner_fee: Option<f64>,
    /// Maximum impact reached.
    pub max_impact_reached: Option<bool>,
    /// Contract address for approval.
    pub token_transfer_proxy: Option<String>,
    /// Contract method to call.
    pub contract_method: Option<String>,
    /// Contract address to call.
    pub contract_address: Option<String>,
}

/// Route information from Paraswap.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParaswapRoute {
    /// Percentage of the swap going through this route.
    pub percent: f64,
    /// Swaps in this route.
    pub swaps: Vec<ParaswapSwap>,
}

/// Swap information from Paraswap.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParaswapSwap {
    /// Source token address.
    pub src_token: String,
    /// Destination token address.
    pub dest_token: String,
    /// Exchanges used.
    pub swaps: Vec<ParaswapExchange>,
}

/// Exchange information from Paraswap.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParaswapExchange {
    /// Exchange name.
    pub exchange: String,
    /// Source amount.
    pub src_amount: String,
    /// Destination amount.
    pub dest_amount: String,
    /// Percentage of the swap.
    pub percent: f64,
}

/// Response from the Paraswap prices endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParaswapQuoteResponse {
    /// Price route information.
    pub price_route: ParaswapPriceRoute,
}

/// Response from the Paraswap transactions endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParaswapTransactionResponse {
    /// Sender address.
    pub from: String,
    /// Contract address.
    pub to: String,
    /// Value to send (in wei).
    pub value: String,
    /// Transaction data.
    pub data: String,
    /// Gas limit.
    pub gas: Option<String>,
    /// Gas price.
    pub gas_price: Option<String>,
    /// Chain ID.
    pub chain_id: u64,
}

/// Error response from Paraswap API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParaswapErrorResponse {
    /// Error message.
    pub error: String,
}

/// Configuration for the Paraswap adapter.
///
/// # Examples
///
/// ```
/// use otc_rfq::infrastructure::venues::dex::paraswap::{ParaswapConfig, ParaswapChain};
///
/// let config = ParaswapConfig::new()
///     .with_chain(ParaswapChain::Polygon)
///     .with_timeout_ms(3000);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParaswapConfig {
    /// Venue ID for this adapter.
    venue_id: VenueId,
    /// Target blockchain.
    chain: ParaswapChain,
    /// Base URL override (optional).
    base_url: Option<String>,
    /// Timeout in milliseconds.
    timeout_ms: u64,
    /// Quote validity in seconds.
    quote_validity_secs: u64,
    /// Slippage tolerance in basis points.
    slippage_bps: u32,
    /// Whether the adapter is enabled.
    enabled: bool,
    /// Token address mappings (symbol -> address).
    token_addresses: HashMap<String, String>,
    /// Partner name for referral tracking.
    partner: Option<String>,
}

impl ParaswapConfig {
    /// Creates a new Paraswap configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            venue_id: VenueId::new("paraswap"),
            chain: ParaswapChain::default(),
            base_url: None,
            timeout_ms: DEFAULT_TIMEOUT_MS,
            quote_validity_secs: DEFAULT_QUOTE_VALIDITY_SECS,
            slippage_bps: 50, // 0.5% default slippage
            enabled: true,
            token_addresses: Self::default_token_addresses(),
            partner: None,
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
    pub fn with_chain(mut self, chain: ParaswapChain) -> Self {
        self.chain = chain;
        self
    }

    /// Sets the base URL override.
    #[must_use]
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = Some(base_url.into());
        self
    }

    /// Sets the timeout in milliseconds.
    #[must_use]
    pub fn with_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }

    /// Sets the quote validity in seconds.
    #[must_use]
    pub fn with_quote_validity_secs(mut self, secs: u64) -> Self {
        self.quote_validity_secs = secs;
        self
    }

    /// Sets the slippage tolerance in basis points.
    #[must_use]
    pub fn with_slippage_bps(mut self, slippage_bps: u32) -> Self {
        self.slippage_bps = slippage_bps;
        self
    }

    /// Sets whether the adapter is enabled.
    #[must_use]
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Sets the partner name for referral tracking.
    #[must_use]
    pub fn with_partner(mut self, partner: impl Into<String>) -> Self {
        self.partner = Some(partner.into());
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

    /// Returns the venue ID.
    #[inline]
    #[must_use]
    pub fn venue_id(&self) -> &VenueId {
        &self.venue_id
    }

    /// Returns the target chain.
    #[inline]
    #[must_use]
    pub fn chain(&self) -> ParaswapChain {
        self.chain
    }

    /// Returns the base URL.
    #[must_use]
    pub fn base_url(&self) -> String {
        self.base_url
            .clone()
            .unwrap_or_else(|| BASE_URL.to_string())
    }

    /// Returns the timeout in milliseconds.
    #[inline]
    #[must_use]
    pub fn timeout_ms(&self) -> u64 {
        self.timeout_ms
    }

    /// Returns the quote validity in seconds.
    #[inline]
    #[must_use]
    pub fn quote_validity_secs(&self) -> u64 {
        self.quote_validity_secs
    }

    /// Returns the slippage tolerance in basis points.
    #[inline]
    #[must_use]
    pub fn slippage_bps(&self) -> u32 {
        self.slippage_bps
    }

    /// Returns whether the adapter is enabled.
    #[inline]
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Returns the partner name.
    #[inline]
    #[must_use]
    pub fn partner(&self) -> Option<&str> {
        self.partner.as_deref()
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
}

impl Default for ParaswapConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Paraswap DEX aggregator adapter.
///
/// Implements the [`VenueAdapter`] trait for the Paraswap Protocol.
///
/// # Note
///
/// This is a stub implementation. Full HTTP client integration
/// will be completed with reqwest.
pub struct ParaswapAdapter {
    /// Configuration.
    config: ParaswapConfig,
}

impl ParaswapAdapter {
    /// Creates a new Paraswap adapter.
    #[must_use]
    pub fn new(config: ParaswapConfig) -> Self {
        Self { config }
    }

    /// Returns the configuration.
    #[inline]
    #[must_use]
    pub fn config(&self) -> &ParaswapConfig {
        &self.config
    }

    /// Builds the prices URL.
    #[must_use]
    pub fn build_prices_url(&self) -> String {
        format!("{}/prices", self.config.base_url())
    }

    /// Builds the transactions URL.
    #[must_use]
    pub fn build_transactions_url(&self) -> String {
        format!(
            "{}/transactions/{}",
            self.config.base_url(),
            self.config.chain().chain_id()
        )
    }

    /// Resolves token addresses from an RFQ.
    ///
    /// Returns (src_token_address, dest_token_address).
    ///
    /// # Errors
    ///
    /// Returns `VenueError::InvalidRequest` if a token symbol cannot be resolved.
    pub fn resolve_tokens(&self, rfq: &Rfq) -> VenueResult<(String, String)> {
        let symbol = rfq.instrument().symbol();
        let base = symbol.base_asset();
        let quote = symbol.quote_asset();

        let (src_symbol, dest_symbol) = match rfq.side() {
            OrderSide::Buy => (quote, base),
            OrderSide::Sell => (base, quote),
        };

        let src_address = self
            .config
            .resolve_token_address(src_symbol)
            .ok_or_else(|| VenueError::invalid_request(format!("Unknown token: {}", src_symbol)))?
            .clone();

        let dest_address = self
            .config
            .resolve_token_address(dest_symbol)
            .ok_or_else(|| VenueError::invalid_request(format!("Unknown token: {}", dest_symbol)))?
            .clone();

        Ok((src_address, dest_address))
    }

    /// Converts a quantity to the smallest unit based on decimals.
    ///
    /// Note: In a real implementation, this would use the token's actual decimals.
    #[must_use]
    pub fn to_smallest_unit(&self, quantity: Decimal, decimals: u8) -> String {
        let multiplier = Decimal::from(10u64.pow(decimals as u32));
        let amount = quantity * multiplier;
        amount.trunc().to_string()
    }

    /// Calculates the price from a quote response.
    ///
    /// # Errors
    ///
    /// Returns `VenueError::ProtocolError` if the price cannot be calculated.
    pub fn calculate_price(&self, response: &ParaswapQuoteResponse) -> VenueResult<Price> {
        let src_amount: f64 = response
            .price_route
            .src_amount
            .parse()
            .map_err(|_| VenueError::protocol_error("Invalid src_amount in response"))?;

        let dest_amount: f64 = response
            .price_route
            .dest_amount
            .parse()
            .map_err(|_| VenueError::protocol_error("Invalid dest_amount in response"))?;

        if src_amount == 0.0 {
            return Err(VenueError::protocol_error("Source amount is zero"));
        }

        // Adjust for decimals
        let src_decimals = response.price_route.src_decimals;
        let dest_decimals = response.price_route.dest_decimals;

        let src_adjusted = src_amount / 10f64.powi(src_decimals as i32);
        let dest_adjusted = dest_amount / 10f64.powi(dest_decimals as i32);

        let price = dest_adjusted / src_adjusted;

        Price::new(price).map_err(|_| VenueError::protocol_error("Invalid price value"))
    }

    /// Estimates gas cost in USD from the response.
    pub fn estimate_gas_cost(&self, response: &ParaswapQuoteResponse) -> Option<Price> {
        let gas_cost_usd = response.price_route.gas_cost_usd.as_ref()?;
        let cost: f64 = gas_cost_usd.parse().ok()?;
        Price::new(cost).ok()
    }

    /// Formats the route information into a string.
    #[must_use]
    pub fn format_route(&self, routes: &[ParaswapRoute]) -> String {
        routes
            .iter()
            .flat_map(|route| {
                route.swaps.iter().flat_map(|swap| {
                    swap.swaps
                        .iter()
                        .map(|exchange| format!("{}:{:.1}%", exchange.exchange, exchange.percent))
                })
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Parses a quote response into a domain Quote.
    ///
    /// # Errors
    ///
    /// Returns `VenueError::ProtocolError` if the response cannot be parsed.
    pub fn parse_quote_response(
        &self,
        response: ParaswapQuoteResponse,
        rfq: &Rfq,
    ) -> VenueResult<Quote> {
        let price = self.calculate_price(&response)?;
        let gas_cost = self.estimate_gas_cost(&response);
        let valid_until = Timestamp::now().add_secs(self.config.quote_validity_secs() as i64);

        let mut builder = QuoteBuilder::new(
            rfq.id(),
            self.config.venue_id().clone(),
            price,
            rfq.quantity(),
            valid_until,
        );

        if let Some(cost) = gas_cost {
            builder = builder.commission(cost);
        }

        // Add metadata
        let route_info = response
            .price_route
            .best_route
            .as_ref()
            .map(|r| self.format_route(r));

        let mut metadata = QuoteMetadata::new();

        if let Some(route) = route_info {
            metadata.set("route_info", route);
        }

        if let Some(impact) = &response.price_route.price_impact {
            metadata.set("price_impact", impact.clone());
        }

        if let Some(contract) = &response.price_route.contract_address {
            metadata.set("contract_address", contract.clone());
        }

        if let Some(method) = &response.price_route.contract_method {
            metadata.set("contract_method", method.clone());
        }

        builder = builder.metadata(metadata);

        Ok(builder.build())
    }
}

impl fmt::Debug for ParaswapAdapter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ParaswapAdapter")
            .field("venue_id", self.config.venue_id())
            .field("chain", &self.config.chain())
            .field("enabled", &self.config.is_enabled())
            .finish()
    }
}

#[async_trait]
impl VenueAdapter for ParaswapAdapter {
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
                "Paraswap adapter is disabled",
            ));
        }

        // Resolve token addresses
        let (_src_token, _dest_token) = self.resolve_tokens(rfq)?;

        // Convert quantity to smallest unit (assuming 18 decimals)
        let _amount = self.to_smallest_unit(rfq.quantity().get(), 18);

        // Build prices URL
        let _url = self.build_prices_url();

        // TODO: Make HTTP request to Paraswap API
        // For now, return a stub error indicating not implemented
        Err(VenueError::internal_error(
            "Paraswap API integration not yet implemented - requires HTTP client",
        ))
    }

    async fn execute_trade(&self, quote: &Quote) -> VenueResult<ExecutionResult> {
        // Check if enabled
        if !self.config.is_enabled() {
            return Err(VenueError::venue_unavailable(
                self.config.venue_id().clone(),
                "Paraswap adapter is disabled",
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

        // TODO: Build transaction via Paraswap API and execute on-chain
        // For now, return a stub error
        Err(VenueError::internal_error(
            "On-chain execution not yet implemented - requires web3 provider",
        ))
    }

    async fn health_check(&self) -> VenueResult<VenueHealth> {
        // TODO: Make health check request to Paraswap API
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
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn test_config() -> ParaswapConfig {
        ParaswapConfig::new()
            .with_chain(ParaswapChain::Ethereum)
            .with_timeout_ms(3000)
    }

    mod chain {
        use super::*;

        #[test]
        fn chain_ids() {
            assert_eq!(ParaswapChain::Ethereum.chain_id(), 1);
            assert_eq!(ParaswapChain::Polygon.chain_id(), 137);
            assert_eq!(ParaswapChain::Arbitrum.chain_id(), 42161);
            assert_eq!(ParaswapChain::Optimism.chain_id(), 10);
            assert_eq!(ParaswapChain::Base.chain_id(), 8453);
            assert_eq!(ParaswapChain::Bsc.chain_id(), 56);
            assert_eq!(ParaswapChain::Avalanche.chain_id(), 43114);
            assert_eq!(ParaswapChain::Fantom.chain_id(), 250);
        }

        #[test]
        fn network_names() {
            assert_eq!(ParaswapChain::Ethereum.network_name(), "ethereum");
            assert_eq!(ParaswapChain::Polygon.network_name(), "polygon");
            assert_eq!(ParaswapChain::Arbitrum.network_name(), "arbitrum");
        }

        #[test]
        fn display() {
            assert_eq!(ParaswapChain::Ethereum.to_string(), "ethereum");
            assert_eq!(ParaswapChain::Polygon.to_string(), "polygon");
        }

        #[test]
        fn to_blockchain() {
            assert_eq!(
                ParaswapChain::Ethereum.to_blockchain(),
                Some(Blockchain::Ethereum)
            );
            assert_eq!(
                ParaswapChain::Polygon.to_blockchain(),
                Some(Blockchain::Polygon)
            );
            assert_eq!(ParaswapChain::Bsc.to_blockchain(), None);
            assert_eq!(ParaswapChain::Fantom.to_blockchain(), None);
        }
    }

    mod config {
        use super::*;

        #[test]
        fn new() {
            let config = ParaswapConfig::new();
            assert_eq!(config.chain(), ParaswapChain::Ethereum);
            assert!(config.is_enabled());
            assert_eq!(config.timeout_ms(), DEFAULT_TIMEOUT_MS);
        }

        #[test]
        fn default() {
            let config = ParaswapConfig::default();
            assert_eq!(config.chain(), ParaswapChain::Ethereum);
        }

        #[test]
        fn with_chain() {
            let config = ParaswapConfig::new().with_chain(ParaswapChain::Polygon);
            assert_eq!(config.chain(), ParaswapChain::Polygon);
        }

        #[test]
        fn with_timeout() {
            let config = ParaswapConfig::new().with_timeout_ms(10000);
            assert_eq!(config.timeout_ms(), 10000);
        }

        #[test]
        fn with_slippage() {
            let config = ParaswapConfig::new().with_slippage_bps(100);
            assert_eq!(config.slippage_bps(), 100);
        }

        #[test]
        fn with_partner() {
            let config = ParaswapConfig::new().with_partner("my-partner");
            assert_eq!(config.partner(), Some("my-partner"));
        }

        #[test]
        fn default_token_addresses() {
            let config = ParaswapConfig::new();
            assert!(config.resolve_token_address("WETH").is_some());
            assert!(config.resolve_token_address("USDC").is_some());
        }

        #[test]
        fn custom_token_address() {
            let config = ParaswapConfig::new().with_token_address("CUSTOM", "0x1234567890abcdef");
            assert_eq!(
                config.resolve_token_address("CUSTOM"),
                Some(&"0x1234567890abcdef".to_string())
            );
        }

        #[test]
        fn base_url_default() {
            let config = ParaswapConfig::new();
            assert_eq!(config.base_url(), BASE_URL);
        }

        #[test]
        fn base_url_override() {
            let config = ParaswapConfig::new().with_base_url("https://custom.api.com");
            assert_eq!(config.base_url(), "https://custom.api.com");
        }
    }

    mod adapter {
        use super::*;

        #[test]
        fn new() {
            let adapter = ParaswapAdapter::new(test_config());
            assert_eq!(adapter.venue_id(), &VenueId::new("paraswap"));
        }

        #[test]
        fn debug_format() {
            let adapter = ParaswapAdapter::new(test_config());
            let debug = format!("{:?}", adapter);
            assert!(debug.contains("ParaswapAdapter"));
            assert!(debug.contains("paraswap"));
        }

        #[test]
        fn build_prices_url() {
            let adapter = ParaswapAdapter::new(test_config());
            assert_eq!(adapter.build_prices_url(), "https://api.paraswap.io/prices");
        }

        #[test]
        fn build_transactions_url() {
            let adapter = ParaswapAdapter::new(test_config());
            assert_eq!(
                adapter.build_transactions_url(),
                "https://api.paraswap.io/transactions/1"
            );
        }

        #[test]
        fn build_transactions_url_polygon() {
            let config = ParaswapConfig::new().with_chain(ParaswapChain::Polygon);
            let adapter = ParaswapAdapter::new(config);
            assert_eq!(
                adapter.build_transactions_url(),
                "https://api.paraswap.io/transactions/137"
            );
        }

        #[test]
        fn to_smallest_unit() {
            let adapter = ParaswapAdapter::new(test_config());
            let amount = adapter.to_smallest_unit(Decimal::from(1), 18);
            assert_eq!(amount, "1000000000000000000");
        }

        #[test]
        fn to_smallest_unit_6_decimals() {
            let adapter = ParaswapAdapter::new(test_config());
            let amount = adapter.to_smallest_unit(Decimal::from(100), 6);
            assert_eq!(amount, "100000000");
        }

        #[tokio::test]
        async fn is_available_when_enabled() {
            let adapter = ParaswapAdapter::new(test_config());
            assert!(adapter.is_available().await);
        }

        #[tokio::test]
        async fn is_not_available_when_disabled() {
            let config = test_config().with_enabled(false);
            let adapter = ParaswapAdapter::new(config);
            assert!(!adapter.is_available().await);
        }

        #[tokio::test]
        async fn health_check_healthy_when_enabled() {
            let adapter = ParaswapAdapter::new(test_config());
            let health = adapter.health_check().await.unwrap();
            assert!(health.is_healthy());
        }

        #[tokio::test]
        async fn health_check_unhealthy_when_disabled() {
            let config = test_config().with_enabled(false);
            let adapter = ParaswapAdapter::new(config);
            let health = adapter.health_check().await.unwrap();
            assert!(!health.is_healthy());
        }
    }

    mod quote_parsing {
        use super::*;

        fn test_price_route() -> ParaswapPriceRoute {
            ParaswapPriceRoute {
                block_number: Some(18000000),
                network: 1,
                src_token: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string(),
                src_decimals: 18,
                src_amount: "1000000000000000000".to_string(), // 1 ETH
                dest_token: "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".to_string(),
                dest_decimals: 6,
                dest_amount: "1850500000".to_string(), // 1850.5 USDC
                best_route: Some(vec![ParaswapRoute {
                    percent: 100.0,
                    swaps: vec![ParaswapSwap {
                        src_token: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string(),
                        dest_token: "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".to_string(),
                        swaps: vec![
                            ParaswapExchange {
                                exchange: "UniswapV3".to_string(),
                                src_amount: "600000000000000000".to_string(),
                                dest_amount: "1110300000".to_string(),
                                percent: 60.0,
                            },
                            ParaswapExchange {
                                exchange: "SushiSwap".to_string(),
                                src_amount: "400000000000000000".to_string(),
                                dest_amount: "740200000".to_string(),
                                percent: 40.0,
                            },
                        ],
                    }],
                }]),
                gas_cost_usd: Some("5.25".to_string()),
                gas_cost: Some("150000".to_string()),
                price_impact: Some("0.05".to_string()),
                partner_fee: None,
                max_impact_reached: Some(false),
                token_transfer_proxy: Some(
                    "0x216b4b4ba9f3e719726886d34a177484278bfcae".to_string(),
                ),
                contract_method: Some("simpleSwap".to_string()),
                contract_address: Some("0xdef171fe48cf0115b1d80b88dc8eab59176fee57".to_string()),
            }
        }

        fn test_quote_response() -> ParaswapQuoteResponse {
            ParaswapQuoteResponse {
                price_route: test_price_route(),
            }
        }

        #[test]
        fn calculate_price() {
            let adapter = ParaswapAdapter::new(test_config());
            let response = test_quote_response();
            let price = adapter.calculate_price(&response).unwrap();
            // 1850.5 USDC / 1 ETH = 1850.5
            assert!((price.get().to_f64().unwrap() - 1850.5).abs() < 0.01);
        }

        #[test]
        fn estimate_gas_cost() {
            let adapter = ParaswapAdapter::new(test_config());
            let response = test_quote_response();
            let gas_cost = adapter.estimate_gas_cost(&response).unwrap();
            assert!((gas_cost.get().to_f64().unwrap() - 5.25).abs() < 0.01);
        }

        #[test]
        fn format_route() {
            let adapter = ParaswapAdapter::new(test_config());
            let routes = vec![ParaswapRoute {
                percent: 100.0,
                swaps: vec![ParaswapSwap {
                    src_token: "0x...".to_string(),
                    dest_token: "0x...".to_string(),
                    swaps: vec![
                        ParaswapExchange {
                            exchange: "UniswapV3".to_string(),
                            src_amount: "600".to_string(),
                            dest_amount: "1110".to_string(),
                            percent: 60.0,
                        },
                        ParaswapExchange {
                            exchange: "SushiSwap".to_string(),
                            src_amount: "400".to_string(),
                            dest_amount: "740".to_string(),
                            percent: 40.0,
                        },
                    ],
                }],
            }];
            let formatted = adapter.format_route(&routes);
            assert!(formatted.contains("UniswapV3:60.0%"));
            assert!(formatted.contains("SushiSwap:40.0%"));
        }
    }
}
