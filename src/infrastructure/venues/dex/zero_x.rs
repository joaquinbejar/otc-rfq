//! # 0x Protocol Adapter
//!
//! Adapter for 0x DEX aggregator.
//!
//! This module provides the [`ZeroXAdapter`] which implements the
//! [`VenueAdapter`] trait for the 0x Protocol DEX aggregator.
//!
//! # Features
//!
//! - HTTP client for 0x API (api.0x.org)
//! - Quote endpoint integration (/swap/v1/quote)
//! - Multi-chain support (Ethereum, Polygon, Arbitrum, etc.)
//! - Gas cost estimation
//! - Route info extraction from sources
//!
//! # Examples
//!
//! ```ignore
//! use otc_rfq::infrastructure::venues::dex::zero_x::{ZeroXAdapter, ZeroXConfig};
//!
//! let config = ZeroXConfig::new("my-api-key")
//!     .with_chain(Chain::Ethereum);
//!
//! let adapter = ZeroXAdapter::new(config);
//! ```

use crate::domain::entities::quote::{Quote, QuoteBuilder, QuoteMetadata};
use crate::domain::entities::rfq::Rfq;
use crate::domain::value_objects::timestamp::Timestamp;
use crate::domain::value_objects::{Blockchain, OrderSide, Price, SettlementMethod, VenueId};
use crate::infrastructure::venues::error::{VenueError, VenueResult};
use crate::infrastructure::venues::http_client::HttpClient;
use crate::infrastructure::venues::traits::{ExecutionResult, VenueAdapter, VenueHealth};
use async_trait::async_trait;
use reqwest::header::{HeaderMap, HeaderValue};
use rust_decimal::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

/// Default timeout in milliseconds.
const DEFAULT_TIMEOUT_MS: u64 = 5000;

/// Default quote validity in seconds.
const DEFAULT_QUOTE_VALIDITY_SECS: u64 = 60;

/// Supported blockchain chains for 0x.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ZeroXChain {
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
}

impl ZeroXChain {
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
        }
    }

    /// Returns the API subdomain for this chain.
    #[must_use]
    pub fn api_subdomain(&self) -> &'static str {
        match self {
            Self::Ethereum => "api",
            Self::Polygon => "polygon",
            Self::Arbitrum => "arbitrum",
            Self::Optimism => "optimism",
            Self::Base => "base",
            Self::Bsc => "bsc",
            Self::Avalanche => "avalanche",
        }
    }

    /// Returns the base URL for this chain.
    #[must_use]
    pub fn base_url(&self) -> String {
        format!("https://{}.0x.org", self.api_subdomain())
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
            // BSC and Avalanche not in domain model yet
            Self::Bsc | Self::Avalanche => None,
        }
    }
}

impl fmt::Display for ZeroXChain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ethereum => write!(f, "ethereum"),
            Self::Polygon => write!(f, "polygon"),
            Self::Arbitrum => write!(f, "arbitrum"),
            Self::Optimism => write!(f, "optimism"),
            Self::Base => write!(f, "base"),
            Self::Bsc => write!(f, "bsc"),
            Self::Avalanche => write!(f, "avalanche"),
        }
    }
}

/// Source of liquidity in a 0x quote.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ZeroXSource {
    /// Name of the liquidity source (e.g., "Uniswap_V3").
    pub name: String,
    /// Proportion of the trade routed through this source (0.0 to 1.0).
    pub proportion: String,
}

/// Response from the 0x quote endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZeroXQuoteResponse {
    /// Chain ID.
    pub chain_id: Option<u64>,
    /// Price (buy amount / sell amount).
    pub price: String,
    /// Guaranteed price (worst case).
    pub guaranteed_price: Option<String>,
    /// Estimated price impact as a percentage.
    pub estimated_price_impact: Option<String>,
    /// Contract address to send the transaction to.
    pub to: String,
    /// Calldata for the transaction.
    pub data: String,
    /// ETH value to send with the transaction.
    pub value: String,
    /// Gas limit.
    pub gas: Option<String>,
    /// Estimated gas.
    pub estimated_gas: Option<String>,
    /// Gas price in wei.
    pub gas_price: Option<String>,
    /// Amount of buy token received.
    pub buy_amount: String,
    /// Amount of sell token sent.
    pub sell_amount: String,
    /// Liquidity sources used.
    pub sources: Option<Vec<ZeroXSource>>,
    /// Buy token address.
    pub buy_token_address: Option<String>,
    /// Sell token address.
    pub sell_token_address: Option<String>,
    /// Address to approve for token spending.
    pub allowance_target: Option<String>,
}

/// Error response from 0x API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZeroXErrorResponse {
    /// Error code.
    pub code: Option<i32>,
    /// Error reason.
    pub reason: Option<String>,
    /// Validation errors.
    pub validation_errors: Option<Vec<ZeroXValidationError>>,
}

/// Validation error from 0x API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZeroXValidationError {
    /// Field that failed validation.
    pub field: String,
    /// Error code.
    pub code: i32,
    /// Error reason.
    pub reason: String,
}

/// Configuration for the 0x adapter.
///
/// # Examples
///
/// ```
/// use otc_rfq::infrastructure::venues::dex::zero_x::{ZeroXConfig, ZeroXChain};
///
/// let config = ZeroXConfig::new("my-api-key")
///     .with_chain(ZeroXChain::Polygon)
///     .with_timeout_ms(3000);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZeroXConfig {
    /// Venue ID for this adapter.
    venue_id: VenueId,
    /// API key for 0x.
    api_key: String,
    /// Target blockchain.
    chain: ZeroXChain,
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
    /// Wallet address for transaction execution.
    wallet_address: Option<String>,
}

impl ZeroXConfig {
    /// Creates a new 0x configuration.
    #[must_use]
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            venue_id: VenueId::new("0x-aggregator"),
            api_key: api_key.into(),
            chain: ZeroXChain::default(),
            base_url: None,
            timeout_ms: DEFAULT_TIMEOUT_MS,
            quote_validity_secs: DEFAULT_QUOTE_VALIDITY_SECS,
            slippage_bps: 50, // 0.5% default slippage
            enabled: true,
            token_addresses: Self::default_token_addresses(),
            wallet_address: None,
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
    pub fn with_chain(mut self, chain: ZeroXChain) -> Self {
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

    /// Returns the API key.
    #[inline]
    #[must_use]
    pub fn api_key(&self) -> &str {
        &self.api_key
    }

    /// Returns the target chain.
    #[inline]
    #[must_use]
    pub fn chain(&self) -> ZeroXChain {
        self.chain
    }

    /// Returns the base URL.
    #[must_use]
    pub fn base_url(&self) -> String {
        self.base_url
            .clone()
            .unwrap_or_else(|| self.chain.base_url())
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

    /// Sets the wallet address for transaction execution.
    #[must_use]
    pub fn with_wallet_address(mut self, address: impl Into<String>) -> Self {
        self.wallet_address = Some(address.into());
        self
    }

    /// Returns the wallet address.
    #[inline]
    #[must_use]
    pub fn wallet_address(&self) -> Option<&str> {
        self.wallet_address.as_deref()
    }
}

/// 0x Protocol DEX aggregator adapter.
///
/// Implements the [`VenueAdapter`] trait for the 0x Protocol.
///
/// # Features
///
/// - HTTP client for 0x API
/// - Quote endpoint integration
/// - Multi-chain support
/// - Gas cost estimation
pub struct ZeroXAdapter {
    /// Configuration.
    config: ZeroXConfig,
    /// HTTP client for API requests.
    http_client: HttpClient,
}

impl ZeroXAdapter {
    /// Creates a new 0x adapter.
    ///
    /// # Errors
    ///
    /// Returns `VenueError::InternalError` if the HTTP client cannot be created.
    pub fn new(config: ZeroXConfig) -> VenueResult<Self> {
        let headers = Self::build_headers(&config)?;
        let http_client = HttpClient::with_headers(config.timeout_ms(), headers)?;
        Ok(Self {
            config,
            http_client,
        })
    }

    /// Builds the default headers for API requests.
    fn build_headers(config: &ZeroXConfig) -> VenueResult<HeaderMap> {
        let mut headers = HeaderMap::new();
        let api_key = HeaderValue::from_str(config.api_key())
            .map_err(|_| VenueError::internal_error("Invalid API key format"))?;
        headers.insert("0x-api-key", api_key);
        Ok(headers)
    }

    /// Returns the configuration.
    #[inline]
    #[must_use]
    pub fn config(&self) -> &ZeroXConfig {
        &self.config
    }

    /// Builds the quote URL.
    #[must_use]
    pub fn build_quote_url(&self) -> String {
        format!("{}/swap/v1/quote", self.config.base_url())
    }

    /// Resolves token addresses from an RFQ.
    ///
    /// Returns (sell_token_address, buy_token_address).
    ///
    /// # Errors
    ///
    /// Returns `VenueError::InvalidRequest` if a token symbol cannot be resolved.
    pub fn resolve_tokens(&self, rfq: &Rfq) -> VenueResult<(String, String)> {
        let symbol = rfq.instrument().symbol();
        let base = symbol.base_asset();
        let quote = symbol.quote_asset();

        let (sell_symbol, buy_symbol) = match rfq.side() {
            OrderSide::Buy => (quote, base),
            OrderSide::Sell => (base, quote),
        };

        let sell_address = self
            .config
            .resolve_token_address(sell_symbol)
            .ok_or_else(|| VenueError::invalid_request(format!("Unknown token: {}", sell_symbol)))?
            .clone();

        let buy_address = self
            .config
            .resolve_token_address(buy_symbol)
            .ok_or_else(|| VenueError::invalid_request(format!("Unknown token: {}", buy_symbol)))?
            .clone();

        Ok((sell_address, buy_address))
    }

    /// Converts a quantity to wei (18 decimals).
    ///
    /// Note: In a real implementation, this would use the token's actual decimals.
    #[must_use]
    pub fn to_wei(&self, quantity: Decimal) -> String {
        let wei = quantity * Decimal::from(1_000_000_000_000_000_000u64);
        wei.trunc().to_string()
    }

    /// Calculates the price from a quote response.
    ///
    /// # Errors
    ///
    /// Returns `VenueError::ProtocolError` if the price cannot be parsed.
    pub fn calculate_price(&self, response: &ZeroXQuoteResponse) -> VenueResult<Price> {
        let price_str = &response.price;
        let price_f64: f64 = price_str
            .parse()
            .map_err(|_| VenueError::protocol_error("Invalid price in response"))?;

        Price::new(price_f64).map_err(|_| VenueError::protocol_error("Invalid price value"))
    }

    /// Estimates gas cost in USD.
    pub fn estimate_gas_cost(&self, response: &ZeroXQuoteResponse) -> Option<Price> {
        let gas = response.estimated_gas.as_ref()?.parse::<u64>().ok()?;
        let gas_price = response.gas_price.as_ref()?.parse::<u64>().ok()?;

        // Gas cost in wei
        let gas_cost_wei = gas.checked_mul(gas_price)?;

        // Convert to ETH (rough estimate, assumes 18 decimals)
        let gas_cost_eth = gas_cost_wei as f64 / 1e18;

        // Rough ETH price estimate (in production, fetch from oracle)
        let eth_price_usd = 2000.0;
        let gas_cost_usd = gas_cost_eth * eth_price_usd;

        Price::new(gas_cost_usd).ok()
    }

    /// Formats the liquidity sources into a string.
    #[must_use]
    pub fn format_sources(&self, sources: &[ZeroXSource]) -> String {
        sources
            .iter()
            .filter(|s| {
                s.proportion
                    .parse::<f64>()
                    .map(|p| p > 0.0)
                    .unwrap_or(false)
            })
            .map(|s| format!("{}:{}", s.name, s.proportion))
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
        response: ZeroXQuoteResponse,
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
        let route_info = response.sources.as_ref().map(|s| self.format_sources(s));

        let gas_estimate = response
            .estimated_gas
            .as_ref()
            .and_then(|g| g.parse::<u64>().ok());

        let mut metadata = QuoteMetadata::new();
        metadata.set("calldata", response.data.clone());
        if let Some(route) = route_info {
            metadata.set("route_info", route);
        }
        if let Some(gas) = gas_estimate {
            metadata.set("gas_estimate", gas.to_string());
        }

        builder = builder.metadata(metadata);

        Ok(builder.build())
    }
}

impl fmt::Debug for ZeroXAdapter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ZeroXAdapter")
            .field("venue_id", self.config.venue_id())
            .field("chain", &self.config.chain())
            .field("enabled", &self.config.is_enabled())
            .finish()
    }
}

#[async_trait]
impl VenueAdapter for ZeroXAdapter {
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
                "0x adapter is disabled",
            ));
        }

        // Resolve token addresses
        let (sell_token, buy_token) = self.resolve_tokens(rfq)?;

        // Convert quantity to wei
        let sell_amount = self.to_wei(rfq.quantity().get());

        // Build query parameters
        let params = [
            ("sellToken", sell_token.as_str()),
            ("buyToken", buy_token.as_str()),
            ("sellAmount", sell_amount.as_str()),
            (
                "slippagePercentage",
                &format!("{:.4}", self.config.slippage_bps() as f64 / 10000.0),
            ),
        ];

        // Build quote URL
        let url = self.build_quote_url();

        // Make HTTP request to 0x API
        let response: ZeroXQuoteResponse = self.http_client.get_with_params(&url, &params).await?;

        // Parse response into Quote
        self.parse_quote_response(response, rfq)
    }

    async fn execute_trade(&self, quote: &Quote) -> VenueResult<ExecutionResult> {
        // Check if enabled
        if !self.config.is_enabled() {
            return Err(VenueError::venue_unavailable(
                self.config.venue_id().clone(),
                "0x adapter is disabled",
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

        // Get calldata from quote metadata
        let calldata = quote
            .metadata()
            .and_then(|m| m.get("calldata"))
            .ok_or_else(|| VenueError::invalid_request("Quote missing calldata"))?;

        // Validate calldata format (should be hex string starting with 0x)
        if !calldata.starts_with("0x") || calldata.len() < 10 {
            return Err(VenueError::invalid_request("Invalid calldata format"));
        }

        // Check if wallet is configured
        let wallet_address = self
            .config
            .wallet_address()
            .ok_or_else(|| VenueError::invalid_request("Wallet address not configured"))?;

        // Build execution result with transaction details
        // Note: Actual on-chain execution requires a signer/wallet integration
        // This implementation validates the quote and returns a pending execution
        let settlement_method = SettlementMethod::OnChain(
            self.config
                .chain()
                .to_blockchain()
                .unwrap_or(Blockchain::Ethereum),
        );
        let execution = ExecutionResult::new(
            quote.id(),
            self.config.venue_id().clone(),
            quote.price(),
            quote.quantity(),
            settlement_method,
        );

        // Log the transaction details for debugging
        tracing::info!(
            venue = %self.config.venue_id(),
            wallet = %wallet_address,
            calldata_len = calldata.len(),
            "Trade execution prepared - requires signer for on-chain submission"
        );

        // Return execution result
        // In production, this would submit the transaction and wait for confirmation
        Ok(execution)
    }

    async fn health_check(&self) -> VenueResult<VenueHealth> {
        if !self.config.is_enabled() {
            return Ok(VenueHealth::unhealthy(
                self.config.venue_id().clone(),
                "Adapter is disabled",
            ));
        }

        // Check API availability by making a simple request
        let url = format!("{}/swap/v1/sources", self.config.base_url());
        let start = std::time::Instant::now();
        let is_healthy = self.http_client.health_check(&url).await;
        let latency_ms = start.elapsed().as_millis() as u64;

        if is_healthy {
            Ok(VenueHealth::healthy_with_latency(
                self.config.venue_id().clone(),
                latency_ms,
            ))
        } else {
            Ok(VenueHealth::unhealthy(
                self.config.venue_id().clone(),
                "API health check failed",
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

    fn test_config() -> ZeroXConfig {
        ZeroXConfig::new("test-api-key")
            .with_chain(ZeroXChain::Ethereum)
            .with_timeout_ms(3000)
    }

    mod chain {
        use super::*;

        #[test]
        fn chain_ids() {
            assert_eq!(ZeroXChain::Ethereum.chain_id(), 1);
            assert_eq!(ZeroXChain::Polygon.chain_id(), 137);
            assert_eq!(ZeroXChain::Arbitrum.chain_id(), 42161);
            assert_eq!(ZeroXChain::Optimism.chain_id(), 10);
            assert_eq!(ZeroXChain::Base.chain_id(), 8453);
        }

        #[test]
        fn api_subdomains() {
            assert_eq!(ZeroXChain::Ethereum.api_subdomain(), "api");
            assert_eq!(ZeroXChain::Polygon.api_subdomain(), "polygon");
        }

        #[test]
        fn base_urls() {
            assert_eq!(ZeroXChain::Ethereum.base_url(), "https://api.0x.org");
            assert_eq!(ZeroXChain::Polygon.base_url(), "https://polygon.0x.org");
        }

        #[test]
        fn display() {
            assert_eq!(ZeroXChain::Ethereum.to_string(), "ethereum");
            assert_eq!(ZeroXChain::Polygon.to_string(), "polygon");
        }

        #[test]
        fn to_blockchain() {
            assert_eq!(
                ZeroXChain::Ethereum.to_blockchain(),
                Some(Blockchain::Ethereum)
            );
            assert_eq!(
                ZeroXChain::Polygon.to_blockchain(),
                Some(Blockchain::Polygon)
            );
            assert_eq!(ZeroXChain::Bsc.to_blockchain(), None);
        }
    }

    mod config {
        use super::*;

        #[test]
        fn new() {
            let config = ZeroXConfig::new("my-key");
            assert_eq!(config.api_key(), "my-key");
            assert_eq!(config.chain(), ZeroXChain::Ethereum);
            assert!(config.is_enabled());
        }

        #[test]
        fn with_chain() {
            let config = ZeroXConfig::new("key").with_chain(ZeroXChain::Polygon);
            assert_eq!(config.chain(), ZeroXChain::Polygon);
        }

        #[test]
        fn with_timeout() {
            let config = ZeroXConfig::new("key").with_timeout_ms(10000);
            assert_eq!(config.timeout_ms(), 10000);
        }

        #[test]
        fn with_slippage() {
            let config = ZeroXConfig::new("key").with_slippage_bps(100);
            assert_eq!(config.slippage_bps(), 100);
        }

        #[test]
        fn default_token_addresses() {
            let config = ZeroXConfig::new("key");
            assert!(config.resolve_token_address("WETH").is_some());
            assert!(config.resolve_token_address("USDC").is_some());
        }

        #[test]
        fn custom_token_address() {
            let config = ZeroXConfig::new("key").with_token_address("CUSTOM", "0x1234567890abcdef");
            assert_eq!(
                config.resolve_token_address("CUSTOM"),
                Some(&"0x1234567890abcdef".to_string())
            );
        }
    }

    mod adapter {
        use super::*;

        #[test]
        fn new() {
            let adapter = ZeroXAdapter::new(test_config()).unwrap();
            assert_eq!(adapter.venue_id(), &VenueId::new("0x-aggregator"));
        }

        #[test]
        fn debug_format() {
            let adapter = ZeroXAdapter::new(test_config()).unwrap();
            let debug = format!("{:?}", adapter);
            assert!(debug.contains("ZeroXAdapter"));
            assert!(debug.contains("0x-aggregator"));
        }

        #[test]
        fn build_quote_url() {
            let adapter = ZeroXAdapter::new(test_config()).unwrap();
            assert_eq!(
                adapter.build_quote_url(),
                "https://api.0x.org/swap/v1/quote"
            );
        }

        #[test]
        fn to_wei() {
            let adapter = ZeroXAdapter::new(test_config()).unwrap();
            let wei = adapter.to_wei(Decimal::from(1));
            assert_eq!(wei, "1000000000000000000");
        }

        #[tokio::test]
        async fn is_available_when_enabled() {
            let adapter = ZeroXAdapter::new(test_config()).unwrap();
            assert!(adapter.is_available().await);
        }

        #[tokio::test]
        async fn is_not_available_when_disabled() {
            let config = test_config().with_enabled(false);
            let adapter = ZeroXAdapter::new(config).unwrap();
            assert!(!adapter.is_available().await);
        }

        #[tokio::test]
        async fn health_check_unhealthy_when_disabled() {
            let config = test_config().with_enabled(false);
            let adapter = ZeroXAdapter::new(config).unwrap();
            let health = adapter.health_check().await.unwrap();
            assert!(!health.is_healthy());
        }
    }

    mod quote_parsing {
        use super::*;

        fn test_quote_response() -> ZeroXQuoteResponse {
            ZeroXQuoteResponse {
                chain_id: Some(1),
                price: "1850.5".to_string(),
                guaranteed_price: Some("1848.0".to_string()),
                estimated_price_impact: Some("0.05".to_string()),
                to: "0xdef1c0ded9bec7f1a1670819833240f027b25eff".to_string(),
                data: "0xabcdef".to_string(),
                value: "0".to_string(),
                gas: Some("180000".to_string()),
                estimated_gas: Some("165000".to_string()),
                gas_price: Some("25000000000".to_string()),
                buy_amount: "1850500000000000000000".to_string(),
                sell_amount: "1000000000000000000".to_string(),
                sources: Some(vec![
                    ZeroXSource {
                        name: "Uniswap_V3".to_string(),
                        proportion: "0.6".to_string(),
                    },
                    ZeroXSource {
                        name: "SushiSwap".to_string(),
                        proportion: "0.4".to_string(),
                    },
                ]),
                buy_token_address: Some("0x6B175474E89094C44Da98b954EeddeBC35e4D1".to_string()),
                sell_token_address: Some("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string()),
                allowance_target: Some("0xdef1c0ded9bec7f1a1670819833240f027b25eff".to_string()),
            }
        }

        #[test]
        fn calculate_price() {
            let adapter = ZeroXAdapter::new(test_config()).unwrap();
            let response = test_quote_response();
            let price = adapter.calculate_price(&response).unwrap();
            assert!((price.get().to_f64().unwrap() - 1850.5).abs() < 0.01);
        }

        #[test]
        fn format_sources() {
            let adapter = ZeroXAdapter::new(test_config()).unwrap();
            let sources = vec![
                ZeroXSource {
                    name: "Uniswap_V3".to_string(),
                    proportion: "0.6".to_string(),
                },
                ZeroXSource {
                    name: "SushiSwap".to_string(),
                    proportion: "0.4".to_string(),
                },
                ZeroXSource {
                    name: "Curve".to_string(),
                    proportion: "0".to_string(),
                },
            ];
            let formatted = adapter.format_sources(&sources);
            assert!(formatted.contains("Uniswap_V3:0.6"));
            assert!(formatted.contains("SushiSwap:0.4"));
            assert!(!formatted.contains("Curve"));
        }
    }
}
