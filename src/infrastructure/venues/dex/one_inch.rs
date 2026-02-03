//! # 1inch Adapter
//!
//! Adapter for 1inch DEX aggregator.
//!
//! This module provides the [`OneInchAdapter`] which implements the
//! [`VenueAdapter`] trait for the 1inch DEX aggregator.
//!
//! # Features
//!
//! - HTTP client for 1inch API
//! - Quote endpoint integration (/v5.2/{chainId}/quote)
//! - Swap endpoint for execution
//! - Multi-chain support (Ethereum, Polygon, Arbitrum, etc.)
//! - Token address resolution
//!
//! # Examples
//!
//! ```ignore
//! use otc_rfq::infrastructure::venues::dex::one_inch::{OneInchAdapter, OneInchConfig};
//!
//! let config = OneInchConfig::new("my-api-key")
//!     .with_chain(OneInchChain::Ethereum);
//!
//! let adapter = OneInchAdapter::new(config);
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

/// Base URL for 1inch API.
const BASE_URL: &str = "https://api.1inch.dev";

/// Supported blockchain chains for 1inch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OneInchChain {
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
    /// Gnosis Chain (chain ID 100).
    Gnosis,
    /// Fantom (chain ID 250).
    Fantom,
}

impl OneInchChain {
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
            Self::Gnosis => 100,
            Self::Fantom => 250,
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
            // BSC, Avalanche, Gnosis, Fantom not in domain model yet
            Self::Bsc | Self::Avalanche | Self::Gnosis | Self::Fantom => None,
        }
    }
}

impl fmt::Display for OneInchChain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ethereum => write!(f, "ethereum"),
            Self::Polygon => write!(f, "polygon"),
            Self::Arbitrum => write!(f, "arbitrum"),
            Self::Optimism => write!(f, "optimism"),
            Self::Base => write!(f, "base"),
            Self::Bsc => write!(f, "bsc"),
            Self::Avalanche => write!(f, "avalanche"),
            Self::Gnosis => write!(f, "gnosis"),
            Self::Fantom => write!(f, "fantom"),
        }
    }
}

/// Protocol used in a 1inch swap.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OneInchProtocol {
    /// Protocol name.
    pub name: String,
    /// Part of the swap routed through this protocol (0-100).
    pub part: f64,
    /// From token index.
    pub from_token_index: Option<i32>,
    /// To token index.
    pub to_token_index: Option<i32>,
}

/// Token information from 1inch API.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OneInchToken {
    /// Token symbol.
    pub symbol: String,
    /// Token name.
    pub name: String,
    /// Token address.
    pub address: String,
    /// Token decimals.
    pub decimals: u8,
    /// Logo URI.
    pub logo_uri: Option<String>,
}

/// Response from the 1inch quote endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OneInchQuoteResponse {
    /// Source token.
    pub from_token: OneInchToken,
    /// Destination token.
    pub to_token: OneInchToken,
    /// Amount of source token.
    pub from_token_amount: String,
    /// Amount of destination token.
    pub to_token_amount: String,
    /// Protocols used in the swap.
    pub protocols: Option<Vec<Vec<Vec<OneInchProtocol>>>>,
    /// Estimated gas.
    pub estimated_gas: Option<u64>,
}

/// Response from the 1inch swap endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OneInchSwapResponse {
    /// Source token.
    pub from_token: OneInchToken,
    /// Destination token.
    pub to_token: OneInchToken,
    /// Amount of source token.
    pub from_token_amount: String,
    /// Amount of destination token.
    pub to_token_amount: String,
    /// Protocols used in the swap.
    pub protocols: Option<Vec<Vec<Vec<OneInchProtocol>>>>,
    /// Transaction data.
    pub tx: OneInchTxData,
}

/// Transaction data from 1inch swap response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OneInchTxData {
    /// Sender address.
    pub from: String,
    /// Contract address to call.
    pub to: String,
    /// Calldata.
    pub data: String,
    /// ETH value to send.
    pub value: String,
    /// Gas limit.
    pub gas: Option<u64>,
    /// Gas price.
    pub gas_price: Option<String>,
}

/// Error response from 1inch API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OneInchErrorResponse {
    /// Error code.
    pub status_code: Option<i32>,
    /// Error message.
    pub error: Option<String>,
    /// Error description.
    pub description: Option<String>,
    /// Request ID.
    pub request_id: Option<String>,
}

/// Configuration for the 1inch adapter.
///
/// # Examples
///
/// ```
/// use otc_rfq::infrastructure::venues::dex::one_inch::{OneInchConfig, OneInchChain};
///
/// let config = OneInchConfig::new("my-api-key")
///     .with_chain(OneInchChain::Polygon)
///     .with_timeout_ms(3000);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OneInchConfig {
    /// Venue ID for this adapter.
    venue_id: VenueId,
    /// API key for 1inch.
    api_key: String,
    /// Target blockchain.
    chain: OneInchChain,
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

impl OneInchConfig {
    /// Creates a new 1inch configuration.
    #[must_use]
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            venue_id: VenueId::new("1inch-aggregator"),
            api_key: api_key.into(),
            chain: OneInchChain::default(),
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
        // Native ETH represented as 0xEeee...
        map.insert(
            "ETH".to_string(),
            "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE".to_string(),
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
    pub fn with_chain(mut self, chain: OneInchChain) -> Self {
        self.chain = chain;
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
    pub fn chain(&self) -> OneInchChain {
        self.chain
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

    /// Builds the quote URL for the configured chain.
    #[must_use]
    pub fn quote_url(&self) -> String {
        format!("{}/swap/v5.2/{}/quote", BASE_URL, self.chain.chain_id())
    }

    /// Builds the swap URL for the configured chain.
    #[must_use]
    pub fn swap_url(&self) -> String {
        format!("{}/swap/v5.2/{}/swap", BASE_URL, self.chain.chain_id())
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

/// 1inch DEX aggregator adapter.
///
/// Implements the [`VenueAdapter`] trait for the 1inch Protocol.
///
/// # Features
///
/// - HTTP client for 1inch API
/// - Quote endpoint integration
/// - Multi-chain support
pub struct OneInchAdapter {
    /// Configuration.
    config: OneInchConfig,
    /// HTTP client for API requests.
    http_client: HttpClient,
}

impl OneInchAdapter {
    /// Creates a new 1inch adapter.
    ///
    /// # Errors
    ///
    /// Returns `VenueError::InternalError` if the HTTP client cannot be created.
    pub fn new(config: OneInchConfig) -> VenueResult<Self> {
        let headers = Self::build_headers(&config)?;
        let http_client = HttpClient::with_headers(config.timeout_ms(), headers)?;
        Ok(Self {
            config,
            http_client,
        })
    }

    /// Builds the default headers for API requests.
    fn build_headers(config: &OneInchConfig) -> VenueResult<HeaderMap> {
        let mut headers = HeaderMap::new();
        let auth_value = HeaderValue::from_str(&format!("Bearer {}", config.api_key()))
            .map_err(|_| VenueError::internal_error("Invalid API key format"))?;
        headers.insert("Authorization", auth_value);
        Ok(headers)
    }

    /// Returns the configuration.
    #[inline]
    #[must_use]
    pub fn config(&self) -> &OneInchConfig {
        &self.config
    }

    /// Resolves token addresses from an RFQ.
    ///
    /// Returns (src_token_address, dst_token_address).
    ///
    /// # Errors
    ///
    /// Returns `VenueError::InvalidRequest` if a token symbol cannot be resolved.
    pub fn resolve_tokens(&self, rfq: &Rfq) -> VenueResult<(String, String)> {
        let symbol = rfq.instrument().symbol();
        let base = symbol.base_asset();
        let quote = symbol.quote_asset();

        let (src_symbol, dst_symbol) = match rfq.side() {
            OrderSide::Buy => (quote, base),
            OrderSide::Sell => (base, quote),
        };

        let src_address = self
            .config
            .resolve_token_address(src_symbol)
            .ok_or_else(|| VenueError::invalid_request(format!("Unknown token: {}", src_symbol)))?
            .clone();

        let dst_address = self
            .config
            .resolve_token_address(dst_symbol)
            .ok_or_else(|| VenueError::invalid_request(format!("Unknown token: {}", dst_symbol)))?
            .clone();

        Ok((src_address, dst_address))
    }

    /// Converts a quantity to the smallest unit based on decimals.
    #[must_use]
    pub fn to_smallest_unit(&self, quantity: Decimal, decimals: u8) -> String {
        let multiplier = Decimal::from(10u64.pow(u32::from(decimals)));
        let amount = quantity * multiplier;
        amount.trunc().to_string()
    }

    /// Calculates the price from a quote response.
    ///
    /// # Errors
    ///
    /// Returns `VenueError::ProtocolError` if the price cannot be calculated.
    pub fn calculate_price(&self, response: &OneInchQuoteResponse) -> VenueResult<Price> {
        let from_amount: f64 = response
            .from_token_amount
            .parse()
            .map_err(|_| VenueError::protocol_error("Invalid from_token_amount"))?;

        let to_amount: f64 = response
            .to_token_amount
            .parse()
            .map_err(|_| VenueError::protocol_error("Invalid to_token_amount"))?;

        if from_amount == 0.0 {
            return Err(VenueError::protocol_error("from_token_amount is zero"));
        }

        // Adjust for decimals
        let from_decimals = response.from_token.decimals;
        let to_decimals = response.to_token.decimals;

        let from_normalized = from_amount / 10f64.powi(i32::from(from_decimals));
        let to_normalized = to_amount / 10f64.powi(i32::from(to_decimals));

        let price = to_normalized / from_normalized;

        Price::new(price).map_err(|_| VenueError::protocol_error("Invalid price value"))
    }

    /// Formats the protocols into a route string.
    #[must_use]
    pub fn format_protocols(&self, protocols: &[Vec<Vec<OneInchProtocol>>]) -> String {
        protocols
            .iter()
            .flatten()
            .flatten()
            .filter(|p| p.part > 0.0)
            .map(|p| format!("{}:{:.0}%", p.name, p.part))
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
        response: OneInchQuoteResponse,
        rfq: &Rfq,
    ) -> VenueResult<Quote> {
        let price = self.calculate_price(&response)?;
        let valid_until = Timestamp::now().add_secs(self.config.quote_validity_secs() as i64);

        let mut builder = QuoteBuilder::new(
            rfq.id(),
            self.config.venue_id().clone(),
            price,
            rfq.quantity(),
            valid_until,
        );

        // Add metadata
        let route_info = response
            .protocols
            .as_ref()
            .map(|p| self.format_protocols(p));

        let mut metadata = QuoteMetadata::new();
        metadata.set("from_token", response.from_token.address.clone());
        metadata.set("to_token", response.to_token.address.clone());
        metadata.set("from_amount", response.from_token_amount.clone());
        metadata.set("to_amount", response.to_token_amount.clone());
        if let Some(route) = route_info {
            metadata.set("route_info", route);
        }
        if let Some(gas) = response.estimated_gas {
            metadata.set("gas_estimate", gas.to_string());
        }

        builder = builder.metadata(metadata);

        Ok(builder.build())
    }

    /// Parses a swap response into a domain Quote with transaction data.
    ///
    /// # Errors
    ///
    /// Returns `VenueError::ProtocolError` if the response cannot be parsed.
    pub fn parse_swap_response(
        &self,
        response: OneInchSwapResponse,
        rfq: &Rfq,
    ) -> VenueResult<Quote> {
        let price = self.calculate_price_from_swap(&response)?;
        let valid_until = Timestamp::now().add_secs(self.config.quote_validity_secs() as i64);

        let mut builder = QuoteBuilder::new(
            rfq.id(),
            self.config.venue_id().clone(),
            price,
            rfq.quantity(),
            valid_until,
        );

        // Add metadata including transaction data
        let mut metadata = QuoteMetadata::new();
        metadata.set("calldata", response.tx.data.clone());
        metadata.set("to_contract", response.tx.to.clone());
        metadata.set("value", response.tx.value.clone());
        metadata.set("from_token", response.from_token.address.clone());
        metadata.set("to_token", response.to_token.address.clone());
        if let Some(gas) = response.tx.gas {
            metadata.set("gas_estimate", gas.to_string());
        }

        builder = builder.metadata(metadata);

        Ok(builder.build())
    }

    fn calculate_price_from_swap(&self, response: &OneInchSwapResponse) -> VenueResult<Price> {
        let from_amount: f64 = response
            .from_token_amount
            .parse()
            .map_err(|_| VenueError::protocol_error("Invalid from_token_amount"))?;

        let to_amount: f64 = response
            .to_token_amount
            .parse()
            .map_err(|_| VenueError::protocol_error("Invalid to_token_amount"))?;

        if from_amount == 0.0 {
            return Err(VenueError::protocol_error("from_token_amount is zero"));
        }

        let from_decimals = response.from_token.decimals;
        let to_decimals = response.to_token.decimals;

        let from_normalized = from_amount / 10f64.powi(i32::from(from_decimals));
        let to_normalized = to_amount / 10f64.powi(i32::from(to_decimals));

        let price = to_normalized / from_normalized;

        Price::new(price).map_err(|_| VenueError::protocol_error("Invalid price value"))
    }
}

impl fmt::Debug for OneInchAdapter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OneInchAdapter")
            .field("venue_id", self.config.venue_id())
            .field("chain", &self.config.chain())
            .field("enabled", &self.config.is_enabled())
            .finish()
    }
}

#[async_trait]
impl VenueAdapter for OneInchAdapter {
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
                "1inch adapter is disabled",
            ));
        }

        // Resolve token addresses
        let (src_token, dst_token) = self.resolve_tokens(rfq)?;

        // Convert quantity (assuming 18 decimals for now)
        let amount = self.to_smallest_unit(rfq.quantity().get(), 18);

        // Build query parameters
        let params = [
            ("src", src_token.as_str()),
            ("dst", dst_token.as_str()),
            ("amount", amount.as_str()),
        ];

        // Build quote URL
        let url = self.config.quote_url();

        // Make HTTP request to 1inch API
        let response: OneInchQuoteResponse =
            self.http_client.get_with_params(&url, &params).await?;

        // Parse response into Quote
        self.parse_quote_response(response, rfq)
    }

    async fn execute_trade(&self, quote: &Quote) -> VenueResult<ExecutionResult> {
        // Check if enabled
        if !self.config.is_enabled() {
            return Err(VenueError::venue_unavailable(
                self.config.venue_id().clone(),
                "1inch adapter is disabled",
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

        Ok(execution)
    }

    async fn health_check(&self) -> VenueResult<VenueHealth> {
        if !self.config.is_enabled() {
            return Ok(VenueHealth::unhealthy(
                self.config.venue_id().clone(),
                "Adapter is disabled",
            ));
        }

        // Check API availability
        let url = format!("{}/healthcheck", BASE_URL);
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

    fn test_config() -> OneInchConfig {
        OneInchConfig::new("test-api-key")
            .with_chain(OneInchChain::Ethereum)
            .with_timeout_ms(3000)
    }

    mod chain {
        use super::*;

        #[test]
        fn chain_ids() {
            assert_eq!(OneInchChain::Ethereum.chain_id(), 1);
            assert_eq!(OneInchChain::Polygon.chain_id(), 137);
            assert_eq!(OneInchChain::Arbitrum.chain_id(), 42161);
            assert_eq!(OneInchChain::Optimism.chain_id(), 10);
            assert_eq!(OneInchChain::Base.chain_id(), 8453);
            assert_eq!(OneInchChain::Bsc.chain_id(), 56);
            assert_eq!(OneInchChain::Gnosis.chain_id(), 100);
            assert_eq!(OneInchChain::Fantom.chain_id(), 250);
        }

        #[test]
        fn display() {
            assert_eq!(OneInchChain::Ethereum.to_string(), "ethereum");
            assert_eq!(OneInchChain::Polygon.to_string(), "polygon");
            assert_eq!(OneInchChain::Gnosis.to_string(), "gnosis");
        }

        #[test]
        fn to_blockchain() {
            assert_eq!(
                OneInchChain::Ethereum.to_blockchain(),
                Some(Blockchain::Ethereum)
            );
            assert_eq!(
                OneInchChain::Polygon.to_blockchain(),
                Some(Blockchain::Polygon)
            );
            assert_eq!(OneInchChain::Bsc.to_blockchain(), None);
            assert_eq!(OneInchChain::Gnosis.to_blockchain(), None);
        }
    }

    mod config {
        use super::*;

        #[test]
        fn new() {
            let config = OneInchConfig::new("my-key");
            assert_eq!(config.api_key(), "my-key");
            assert_eq!(config.chain(), OneInchChain::Ethereum);
            assert!(config.is_enabled());
        }

        #[test]
        fn with_chain() {
            let config = OneInchConfig::new("key").with_chain(OneInchChain::Polygon);
            assert_eq!(config.chain(), OneInchChain::Polygon);
        }

        #[test]
        fn with_timeout() {
            let config = OneInchConfig::new("key").with_timeout_ms(10000);
            assert_eq!(config.timeout_ms(), 10000);
        }

        #[test]
        fn with_slippage() {
            let config = OneInchConfig::new("key").with_slippage_bps(100);
            assert_eq!(config.slippage_bps(), 100);
        }

        #[test]
        fn default_token_addresses() {
            let config = OneInchConfig::new("key");
            assert!(config.resolve_token_address("WETH").is_some());
            assert!(config.resolve_token_address("USDC").is_some());
            assert!(config.resolve_token_address("ETH").is_some());
        }

        #[test]
        fn custom_token_address() {
            let config =
                OneInchConfig::new("key").with_token_address("CUSTOM", "0x1234567890abcdef");
            assert_eq!(
                config.resolve_token_address("CUSTOM"),
                Some(&"0x1234567890abcdef".to_string())
            );
        }

        #[test]
        fn quote_url() {
            let config = OneInchConfig::new("key").with_chain(OneInchChain::Ethereum);
            assert_eq!(
                config.quote_url(),
                "https://api.1inch.dev/swap/v5.2/1/quote"
            );
        }

        #[test]
        fn swap_url() {
            let config = OneInchConfig::new("key").with_chain(OneInchChain::Polygon);
            assert_eq!(
                config.swap_url(),
                "https://api.1inch.dev/swap/v5.2/137/swap"
            );
        }
    }

    mod adapter {
        use super::*;

        #[test]
        fn new() {
            let adapter = OneInchAdapter::new(test_config()).unwrap();
            assert_eq!(adapter.venue_id(), &VenueId::new("1inch-aggregator"));
        }

        #[test]
        fn debug_format() {
            let adapter = OneInchAdapter::new(test_config()).unwrap();
            let debug = format!("{:?}", adapter);
            assert!(debug.contains("OneInchAdapter"));
            assert!(debug.contains("1inch-aggregator"));
        }

        #[test]
        fn to_smallest_unit() {
            let adapter = OneInchAdapter::new(test_config()).unwrap();
            let amount = adapter.to_smallest_unit(Decimal::from(1), 18);
            assert_eq!(amount, "1000000000000000000");

            let amount_6 = adapter.to_smallest_unit(Decimal::from(1), 6);
            assert_eq!(amount_6, "1000000");
        }

        #[tokio::test]
        async fn is_available_when_enabled() {
            let adapter = OneInchAdapter::new(test_config()).unwrap();
            assert!(adapter.is_available().await);
        }

        #[tokio::test]
        async fn is_not_available_when_disabled() {
            let config = test_config().with_enabled(false);
            let adapter = OneInchAdapter::new(config).unwrap();
            assert!(!adapter.is_available().await);
        }

        #[tokio::test]
        async fn health_check_unhealthy_when_disabled() {
            let config = test_config().with_enabled(false);
            let adapter = OneInchAdapter::new(config).unwrap();
            let health = adapter.health_check().await.unwrap();
            assert!(!health.is_healthy());
        }
    }

    mod quote_parsing {
        use super::*;

        fn test_quote_response() -> OneInchQuoteResponse {
            OneInchQuoteResponse {
                from_token: OneInchToken {
                    symbol: "WETH".to_string(),
                    name: "Wrapped Ether".to_string(),
                    address: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string(),
                    decimals: 18,
                    logo_uri: None,
                },
                to_token: OneInchToken {
                    symbol: "USDC".to_string(),
                    name: "USD Coin".to_string(),
                    address: "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".to_string(),
                    decimals: 6,
                    logo_uri: None,
                },
                from_token_amount: "1000000000000000000".to_string(), // 1 WETH
                to_token_amount: "1850000000".to_string(),            // 1850 USDC
                protocols: Some(vec![vec![vec![
                    OneInchProtocol {
                        name: "UNISWAP_V3".to_string(),
                        part: 60.0,
                        from_token_index: Some(0),
                        to_token_index: Some(1),
                    },
                    OneInchProtocol {
                        name: "SUSHISWAP".to_string(),
                        part: 40.0,
                        from_token_index: Some(0),
                        to_token_index: Some(1),
                    },
                ]]]),
                estimated_gas: Some(165000),
            }
        }

        #[test]
        fn calculate_price() {
            let adapter = OneInchAdapter::new(test_config()).unwrap();
            let response = test_quote_response();
            let price = adapter.calculate_price(&response).unwrap();
            // 1850 USDC / 1 WETH = 1850
            assert!((price.get().to_f64().unwrap() - 1850.0).abs() < 0.01);
        }

        #[test]
        fn format_protocols() {
            let adapter = OneInchAdapter::new(test_config()).unwrap();
            let protocols = vec![vec![vec![
                OneInchProtocol {
                    name: "UNISWAP_V3".to_string(),
                    part: 60.0,
                    from_token_index: Some(0),
                    to_token_index: Some(1),
                },
                OneInchProtocol {
                    name: "SUSHISWAP".to_string(),
                    part: 40.0,
                    from_token_index: Some(0),
                    to_token_index: Some(1),
                },
                OneInchProtocol {
                    name: "CURVE".to_string(),
                    part: 0.0,
                    from_token_index: Some(0),
                    to_token_index: Some(1),
                },
            ]]];
            let formatted = adapter.format_protocols(&protocols);
            assert!(formatted.contains("UNISWAP_V3:60%"));
            assert!(formatted.contains("SUSHISWAP:40%"));
            assert!(!formatted.contains("CURVE"));
        }
    }
}
