//! # Bebop Adapter
//!
//! Adapter for Bebop RFQ protocol with gasless trading and MEV protection.
//!
//! This module provides the [`BebopAdapter`] which implements the
//! [`VenueAdapter`] trait for the Bebop RFQ protocol.
//!
//! # Features
//!
//! - HTTP client for Bebop API (api.bebop.xyz)
//! - RFQ endpoint integration
//! - Signed quote handling with EIP-712 signatures
//! - Gasless execution support
//! - Batch swap support (multiple tokens in single tx)
//! - Quote expiry tracking
//! - MEV protection
//! - Multi-chain support (Ethereum, Polygon, Arbitrum, Optimism, BSC)
//!
//! # Examples
//!
//! ```ignore
//! use otc_rfq::infrastructure::venues::rfq_protocols::bebop::{BebopAdapter, BebopConfig};
//!
//! let config = BebopConfig::new("my-api-key")
//!     .with_chain(BebopChain::Ethereum);
//!
//! let adapter = BebopAdapter::new(config);
//! ```

use crate::domain::entities::quote::{Quote, QuoteBuilder, QuoteMetadata};
use crate::domain::entities::rfq::Rfq;
use crate::domain::value_objects::timestamp::Timestamp;
use crate::domain::value_objects::{Blockchain, OrderSide, Price, VenueId};
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

/// Base URL for Bebop API.
const BASE_URL: &str = "https://api.bebop.xyz";

/// Supported blockchain chains for Bebop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BebopChain {
    /// Ethereum mainnet (chain ID 1).
    #[default]
    Ethereum,
    /// Polygon (chain ID 137).
    Polygon,
    /// Arbitrum One (chain ID 42161).
    Arbitrum,
    /// Optimism (chain ID 10).
    Optimism,
    /// BNB Smart Chain (chain ID 56).
    Bsc,
    /// Base (chain ID 8453).
    Base,
    /// Blast (chain ID 81457).
    Blast,
}

impl BebopChain {
    /// Returns the chain ID.
    #[must_use]
    pub fn chain_id(&self) -> u64 {
        match self {
            Self::Ethereum => 1,
            Self::Polygon => 137,
            Self::Arbitrum => 42161,
            Self::Optimism => 10,
            Self::Bsc => 56,
            Self::Base => 8453,
            Self::Blast => 81457,
        }
    }

    /// Returns the chain name as used in Bebop API.
    #[must_use]
    pub fn api_name(&self) -> &'static str {
        match self {
            Self::Ethereum => "ethereum",
            Self::Polygon => "polygon",
            Self::Arbitrum => "arbitrum",
            Self::Optimism => "optimism",
            Self::Bsc => "bsc",
            Self::Base => "base",
            Self::Blast => "blast",
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
            // BSC, Base, and Blast not in domain model yet
            Self::Bsc | Self::Base | Self::Blast => None,
        }
    }

    /// Returns all supported chains.
    #[must_use]
    pub fn all() -> &'static [BebopChain] {
        &[
            BebopChain::Ethereum,
            BebopChain::Polygon,
            BebopChain::Arbitrum,
            BebopChain::Optimism,
            BebopChain::Bsc,
            BebopChain::Base,
            BebopChain::Blast,
        ]
    }
}

impl fmt::Display for BebopChain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.api_name())
    }
}

/// Token information in Bebop quote.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BebopTokenInfo {
    /// Token address.
    pub address: String,
    /// Token decimals.
    pub decimals: u8,
    /// Token symbol.
    pub symbol: Option<String>,
}

/// Quote data from Bebop RFQ response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BebopQuoteData {
    /// Quote ID.
    pub quote_id: String,
    /// Chain ID.
    pub chain_id: u64,
    /// Sell token address.
    pub sell_token: String,
    /// Buy token address.
    pub buy_token: String,
    /// Sell amount (in smallest unit).
    pub sell_amount: String,
    /// Buy amount (in smallest unit).
    pub buy_amount: String,
    /// Quote expiry timestamp (Unix milliseconds).
    pub expiry: u64,
    /// Taker address.
    pub taker: String,
    /// Receiver address (can differ from taker).
    pub receiver: Option<String>,
    /// Bebop settlement contract address.
    pub settlement_address: String,
    /// Approval target address.
    pub approval_target: String,
    /// EIP-712 signature.
    pub signature: String,
    /// Transaction data for execution.
    pub tx_data: Option<String>,
    /// Gas estimate.
    pub gas_estimate: Option<String>,
    /// Whether this is a gasless quote.
    pub gasless: bool,
}

/// Response from the Bebop quote endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BebopQuoteResponse {
    /// Status of the response.
    pub status: String,
    /// Quote data.
    pub quote: Option<BebopQuoteData>,
    /// Error message if status is not "success".
    pub error: Option<String>,
    /// Error code.
    pub error_code: Option<String>,
}

/// Request body for Bebop quote endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BebopQuoteRequest {
    /// Sell token address.
    pub sell_token: String,
    /// Buy token address.
    pub buy_token: String,
    /// Sell amount (in smallest unit).
    pub sell_amount: String,
    /// Taker wallet address.
    pub taker_address: String,
    /// Receiver address (optional, defaults to taker).
    pub receiver_address: Option<String>,
    /// Whether to request gasless execution.
    pub gasless: Option<bool>,
    /// Slippage tolerance in basis points.
    pub slippage_bps: Option<u32>,
}

/// Batch quote request for multiple token swaps.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BebopBatchQuoteRequest {
    /// List of sell tokens with amounts.
    pub sell_tokens: Vec<BebopTokenAmount>,
    /// List of buy tokens with amounts.
    pub buy_tokens: Vec<BebopTokenAmount>,
    /// Taker wallet address.
    pub taker_address: String,
    /// Whether to request gasless execution.
    pub gasless: Option<bool>,
}

/// Token with amount for batch requests.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BebopTokenAmount {
    /// Token address.
    pub address: String,
    /// Amount (in smallest unit).
    pub amount: String,
}

/// Error response from Bebop API.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BebopErrorResponse {
    /// Error status.
    pub status: String,
    /// Error message.
    pub error: Option<String>,
    /// Error code.
    pub error_code: Option<String>,
}

/// Configuration for the Bebop adapter.
///
/// # Examples
///
/// ```
/// use otc_rfq::infrastructure::venues::rfq_protocols::bebop::{BebopConfig, BebopChain};
///
/// let config = BebopConfig::new("my-api-key")
///     .with_chain(BebopChain::Polygon)
///     .with_timeout_ms(3000);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BebopConfig {
    /// Venue ID for this adapter.
    venue_id: VenueId,
    /// API key for Bebop.
    api_key: String,
    /// Target blockchain.
    chain: BebopChain,
    /// Timeout in milliseconds.
    timeout_ms: u64,
    /// Whether the adapter is enabled.
    enabled: bool,
    /// Taker wallet address.
    wallet_address: Option<String>,
    /// Token address mappings (symbol -> address).
    token_addresses: HashMap<String, String>,
    /// Whether to use gasless execution.
    gasless: bool,
    /// Default slippage in basis points.
    slippage_bps: u32,
}

impl BebopConfig {
    /// Creates a new Bebop configuration.
    #[must_use]
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            venue_id: VenueId::new("bebop"),
            api_key: api_key.into(),
            chain: BebopChain::default(),
            timeout_ms: DEFAULT_TIMEOUT_MS,
            enabled: true,
            wallet_address: None,
            token_addresses: Self::default_token_addresses(),
            gasless: true,
            slippage_bps: 50, // 0.5% default slippage
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
    pub fn with_chain(mut self, chain: BebopChain) -> Self {
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

    /// Sets the taker wallet address.
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

    /// Sets whether to use gasless execution.
    #[must_use]
    pub fn with_gasless(mut self, gasless: bool) -> Self {
        self.gasless = gasless;
        self
    }

    /// Sets the default slippage in basis points.
    #[must_use]
    pub fn with_slippage_bps(mut self, slippage_bps: u32) -> Self {
        self.slippage_bps = slippage_bps;
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
    pub fn chain(&self) -> BebopChain {
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

    /// Returns the taker wallet address.
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

    /// Returns whether gasless execution is enabled.
    #[inline]
    #[must_use]
    pub fn is_gasless(&self) -> bool {
        self.gasless
    }

    /// Returns the default slippage in basis points.
    #[inline]
    #[must_use]
    pub fn slippage_bps(&self) -> u32 {
        self.slippage_bps
    }

    /// Builds the quote URL for the configured chain.
    #[must_use]
    pub fn quote_url(&self) -> String {
        format!("{}/{}/v2/quote", BASE_URL, self.chain.api_name())
    }

    /// Builds the batch quote URL for the configured chain.
    #[must_use]
    pub fn batch_quote_url(&self) -> String {
        format!("{}/{}/v2/quote/batch", BASE_URL, self.chain.api_name())
    }

    /// Builds the order URL for execution.
    #[must_use]
    pub fn order_url(&self) -> String {
        format!("{}/{}/v2/order", BASE_URL, self.chain.api_name())
    }
}

/// Bebop RFQ protocol adapter.
///
/// Implements the [`VenueAdapter`] trait for the Bebop Protocol.
///
/// # Features
///
/// - Gasless trading with MEV protection
/// - Signed quotes with EIP-712 signatures
/// - Batch swap support
/// - Quote expiry tracking
/// - Multi-chain support
pub struct BebopAdapter {
    /// Configuration.
    config: BebopConfig,
    /// HTTP client for API requests.
    http_client: HttpClient,
}

impl BebopAdapter {
    /// Creates a new Bebop adapter.
    ///
    /// # Errors
    ///
    /// Returns `VenueError::InternalError` if the HTTP client cannot be created.
    pub fn new(config: BebopConfig) -> VenueResult<Self> {
        let headers = Self::build_headers(&config)?;
        let http_client = HttpClient::with_headers(config.timeout_ms(), headers)?;
        Ok(Self {
            config,
            http_client,
        })
    }

    /// Builds the default headers for API requests.
    fn build_headers(config: &BebopConfig) -> VenueResult<HeaderMap> {
        let mut headers = HeaderMap::new();
        let api_key = HeaderValue::from_str(config.api_key())
            .map_err(|_| VenueError::internal_error("Invalid API key format"))?;
        headers.insert("x-api-key", api_key);
        Ok(headers)
    }

    /// Returns the configuration.
    #[inline]
    #[must_use]
    pub fn config(&self) -> &BebopConfig {
        &self.config
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

        // For Buy side: sell quote token, buy base token
        // For Sell side: sell base token, buy quote token
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

    /// Builds a quote request.
    ///
    /// # Errors
    ///
    /// Returns `VenueError::InvalidRequest` if required configuration is missing.
    pub fn build_quote_request(&self, rfq: &Rfq) -> VenueResult<BebopQuoteRequest> {
        let (sell_token, buy_token) = self.resolve_tokens(rfq)?;

        let taker_address = self
            .config
            .wallet_address()
            .ok_or_else(|| VenueError::invalid_request("Wallet address not configured"))?
            .to_string();

        // Determine sell amount based on side
        let sell_amount = self.to_smallest_unit(rfq.quantity().get(), 18);

        Ok(BebopQuoteRequest {
            sell_token,
            buy_token,
            sell_amount,
            taker_address,
            receiver_address: None,
            gasless: Some(self.config.is_gasless()),
            slippage_bps: Some(self.config.slippage_bps()),
        })
    }

    /// Calculates the price from a quote response.
    ///
    /// # Errors
    ///
    /// Returns `VenueError::ProtocolError` if the price cannot be calculated.
    pub fn calculate_price(&self, quote: &BebopQuoteData) -> VenueResult<Price> {
        let sell_amount: f64 = quote
            .sell_amount
            .parse()
            .map_err(|_| VenueError::protocol_error("Invalid sell_amount"))?;

        let buy_amount: f64 = quote
            .buy_amount
            .parse()
            .map_err(|_| VenueError::protocol_error("Invalid buy_amount"))?;

        if sell_amount == 0.0 {
            return Err(VenueError::protocol_error("sell_amount is zero"));
        }

        // Price = buy_amount / sell_amount (assuming same decimals, simplified)
        let price = buy_amount / sell_amount;

        Price::new(price).map_err(|_| VenueError::protocol_error("Invalid price value"))
    }

    /// Checks if a quote has expired.
    ///
    /// Bebop expiry is in milliseconds.
    #[must_use]
    pub fn is_quote_expired(&self, quote: &BebopQuoteData) -> bool {
        let now_ms = Timestamp::now().timestamp_millis() as u64;
        now_ms >= quote.expiry
    }

    /// Returns the time until quote expiry in seconds.
    #[must_use]
    pub fn time_to_expiry(&self, quote: &BebopQuoteData) -> i64 {
        let now_ms = Timestamp::now().timestamp_millis() as u64;
        ((quote.expiry as i64) - (now_ms as i64)) / 1000
    }

    /// Parses a quote response into a domain Quote.
    ///
    /// # Errors
    ///
    /// Returns `VenueError::ProtocolError` if the response cannot be parsed.
    pub fn parse_quote_response(
        &self,
        response: BebopQuoteResponse,
        rfq: &Rfq,
    ) -> VenueResult<Quote> {
        // Check response status
        if response.status != "success" {
            let error_msg = response
                .error
                .unwrap_or_else(|| "Unknown error".to_string());
            return Err(VenueError::protocol_error(format!(
                "Bebop API error: {}",
                error_msg
            )));
        }

        let quote_data = response
            .quote
            .ok_or_else(|| VenueError::protocol_error("No quote in response"))?;

        // Check if quote is already expired
        if self.is_quote_expired(&quote_data) {
            return Err(VenueError::quote_expired("Quote has already expired"));
        }

        let price = self.calculate_price(&quote_data)?;

        // Convert expiry from milliseconds to seconds
        let expiry_secs = (quote_data.expiry / 1000) as i64;
        let valid_until = Timestamp::from_secs(expiry_secs)
            .ok_or_else(|| VenueError::protocol_error("Invalid quote expiry timestamp"))?;

        let mut builder = QuoteBuilder::new(
            rfq.id(),
            self.config.venue_id().clone(),
            price,
            rfq.quantity(),
            valid_until,
        );

        // Add metadata including signature for execution
        let mut metadata = QuoteMetadata::new();
        metadata.set("quote_id", quote_data.quote_id.clone());
        metadata.set("signature", quote_data.signature.clone());
        metadata.set("settlement_address", quote_data.settlement_address.clone());
        metadata.set("approval_target", quote_data.approval_target.clone());
        metadata.set("sell_token", quote_data.sell_token.clone());
        metadata.set("buy_token", quote_data.buy_token.clone());
        metadata.set("sell_amount", quote_data.sell_amount.clone());
        metadata.set("buy_amount", quote_data.buy_amount.clone());
        metadata.set("chain_id", quote_data.chain_id.to_string());
        metadata.set("gasless", quote_data.gasless.to_string());

        if let Some(tx_data) = &quote_data.tx_data {
            metadata.set("tx_data", tx_data.clone());
        }

        if let Some(gas_estimate) = &quote_data.gas_estimate {
            metadata.set("gas_estimate", gas_estimate.clone());
        }

        if let Some(receiver) = &quote_data.receiver {
            metadata.set("receiver", receiver.clone());
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
            "quote_id",
            "signature",
            "settlement_address",
            "approval_target",
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

impl fmt::Debug for BebopAdapter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BebopAdapter")
            .field("venue_id", self.config.venue_id())
            .field("chain", &self.config.chain())
            .field("gasless", &self.config.is_gasless())
            .field("enabled", &self.config.is_enabled())
            .finish()
    }
}

#[async_trait]
impl VenueAdapter for BebopAdapter {
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
                "Bebop adapter is disabled",
            ));
        }

        // Build quote request
        let request = self.build_quote_request(rfq)?;

        // Build quote URL
        let url = self.config.quote_url();

        // Make HTTP POST request to Bebop API
        let response: BebopQuoteResponse = self.http_client.post(&url, &request).await?;

        // Parse response into Quote
        self.parse_quote_response(response, rfq)
    }

    async fn execute_trade(&self, quote: &Quote) -> VenueResult<ExecutionResult> {
        // Check if enabled
        if !self.config.is_enabled() {
            return Err(VenueError::venue_unavailable(
                self.config.venue_id().clone(),
                "Bebop adapter is disabled",
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

        // Get execution URL
        let _url = self.config.order_url();

        // TODO: Execute via Bebop API (gasless) or on-chain
        Err(VenueError::internal_error(
            "Bebop execution not yet implemented - requires HTTP client",
        ))
    }

    async fn health_check(&self) -> VenueResult<VenueHealth> {
        if !self.config.is_enabled() {
            return Ok(VenueHealth::unhealthy(
                self.config.venue_id().clone(),
                "Adapter is disabled",
            ));
        }

        // Check API availability
        let url = format!("{}/health", BASE_URL);
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

    fn test_config() -> BebopConfig {
        BebopConfig::new("test-api-key")
            .with_chain(BebopChain::Ethereum)
            .with_wallet_address("0x1234567890abcdef1234567890abcdef12345678")
            .with_timeout_ms(3000)
    }

    mod chain {
        use super::*;

        #[test]
        fn chain_ids() {
            assert_eq!(BebopChain::Ethereum.chain_id(), 1);
            assert_eq!(BebopChain::Polygon.chain_id(), 137);
            assert_eq!(BebopChain::Arbitrum.chain_id(), 42161);
            assert_eq!(BebopChain::Optimism.chain_id(), 10);
            assert_eq!(BebopChain::Bsc.chain_id(), 56);
            assert_eq!(BebopChain::Base.chain_id(), 8453);
            assert_eq!(BebopChain::Blast.chain_id(), 81457);
        }

        #[test]
        fn api_names() {
            assert_eq!(BebopChain::Ethereum.api_name(), "ethereum");
            assert_eq!(BebopChain::Polygon.api_name(), "polygon");
            assert_eq!(BebopChain::Arbitrum.api_name(), "arbitrum");
            assert_eq!(BebopChain::Optimism.api_name(), "optimism");
            assert_eq!(BebopChain::Bsc.api_name(), "bsc");
            assert_eq!(BebopChain::Base.api_name(), "base");
            assert_eq!(BebopChain::Blast.api_name(), "blast");
        }

        #[test]
        fn to_blockchain() {
            assert_eq!(
                BebopChain::Ethereum.to_blockchain(),
                Some(Blockchain::Ethereum)
            );
            assert_eq!(
                BebopChain::Polygon.to_blockchain(),
                Some(Blockchain::Polygon)
            );
            assert_eq!(
                BebopChain::Arbitrum.to_blockchain(),
                Some(Blockchain::Arbitrum)
            );
            assert_eq!(
                BebopChain::Optimism.to_blockchain(),
                Some(Blockchain::Optimism)
            );
            assert_eq!(BebopChain::Bsc.to_blockchain(), None);
            assert_eq!(BebopChain::Base.to_blockchain(), None);
            assert_eq!(BebopChain::Blast.to_blockchain(), None);
        }

        #[test]
        fn display() {
            assert_eq!(BebopChain::Ethereum.to_string(), "ethereum");
            assert_eq!(BebopChain::Polygon.to_string(), "polygon");
        }

        #[test]
        fn default_is_ethereum() {
            assert_eq!(BebopChain::default(), BebopChain::Ethereum);
        }

        #[test]
        fn all_chains() {
            let chains = BebopChain::all();
            assert_eq!(chains.len(), 7);
            assert!(chains.contains(&BebopChain::Ethereum));
            assert!(chains.contains(&BebopChain::Blast));
        }
    }

    mod config {
        use super::*;

        #[test]
        fn new_config() {
            let config = BebopConfig::new("my-api-key");
            assert_eq!(config.api_key(), "my-api-key");
            assert_eq!(config.chain(), BebopChain::Ethereum);
            assert_eq!(config.timeout_ms(), DEFAULT_TIMEOUT_MS);
            assert!(config.is_enabled());
            assert!(config.is_gasless());
            assert_eq!(config.slippage_bps(), 50);
        }

        #[test]
        fn builder_pattern() {
            let config = BebopConfig::new("my-api-key")
                .with_chain(BebopChain::Polygon)
                .with_timeout_ms(3000)
                .with_enabled(false)
                .with_gasless(false)
                .with_slippage_bps(100)
                .with_wallet_address("0xabc");

            assert_eq!(config.chain(), BebopChain::Polygon);
            assert_eq!(config.timeout_ms(), 3000);
            assert!(!config.is_enabled());
            assert!(!config.is_gasless());
            assert_eq!(config.slippage_bps(), 100);
            assert_eq!(config.wallet_address(), Some("0xabc"));
        }

        #[test]
        fn venue_id() {
            let config = BebopConfig::new("key").with_venue_id("custom-bebop");
            assert_eq!(config.venue_id(), &VenueId::new("custom-bebop"));
        }

        #[test]
        fn token_addresses() {
            let config = BebopConfig::new("key").with_token_address("TEST", "0x123");
            assert_eq!(
                config.resolve_token_address("TEST"),
                Some(&"0x123".to_string())
            );
            assert!(config.resolve_token_address("UNKNOWN").is_none());
        }

        #[test]
        fn default_token_addresses() {
            let config = BebopConfig::new("key");
            assert!(config.resolve_token_address("WETH").is_some());
            assert!(config.resolve_token_address("USDC").is_some());
            assert!(config.resolve_token_address("USDT").is_some());
        }

        #[test]
        fn urls() {
            let config = BebopConfig::new("key").with_chain(BebopChain::Polygon);
            assert_eq!(config.quote_url(), "https://api.bebop.xyz/polygon/v2/quote");
            assert_eq!(
                config.batch_quote_url(),
                "https://api.bebop.xyz/polygon/v2/quote/batch"
            );
            assert_eq!(config.order_url(), "https://api.bebop.xyz/polygon/v2/order");
        }
    }

    mod adapter {
        use super::*;

        #[test]
        fn new_adapter() {
            let adapter = BebopAdapter::new(test_config()).unwrap();
            assert_eq!(adapter.config().api_key(), "test-api-key");
        }

        #[test]
        fn debug_impl() {
            let adapter = BebopAdapter::new(test_config()).unwrap();
            let debug = format!("{:?}", adapter);
            assert!(debug.contains("BebopAdapter"));
            assert!(debug.contains("bebop"));
        }

        #[test]
        fn to_smallest_unit() {
            let adapter = BebopAdapter::new(test_config()).unwrap();
            let amount = Decimal::from_str("1.5").unwrap();
            let result = adapter.to_smallest_unit(amount, 18);
            assert_eq!(result, "1500000000000000000");
        }

        #[test]
        fn to_smallest_unit_6_decimals() {
            let adapter = BebopAdapter::new(test_config()).unwrap();
            let amount = Decimal::from_str("100.0").unwrap();
            let result = adapter.to_smallest_unit(amount, 6);
            assert_eq!(result, "100000000");
        }
    }

    mod quote_data {
        use super::*;

        fn test_quote_data() -> BebopQuoteData {
            BebopQuoteData {
                quote_id: "test-quote-123".to_string(),
                chain_id: 1,
                sell_token: "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".to_string(),
                buy_token: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string(),
                sell_amount: "1000000000".to_string(),
                buy_amount: "500000000000000000".to_string(),
                expiry: (Timestamp::now().timestamp_millis() as u64) + 60000, // 60 seconds from now
                taker: "0x1234567890abcdef1234567890abcdef12345678".to_string(),
                receiver: None,
                settlement_address: "0xsettlement".to_string(),
                approval_target: "0xapproval".to_string(),
                signature: "0xsignature".to_string(),
                tx_data: Some("0xtxdata".to_string()),
                gas_estimate: Some("100000".to_string()),
                gasless: true,
            }
        }

        #[test]
        fn calculate_price() {
            let adapter = BebopAdapter::new(test_config()).unwrap();
            let quote = test_quote_data();
            let price = adapter.calculate_price(&quote).unwrap();
            // 500000000000000000 / 1000000000 = 500000000.0
            // This is the raw ratio - in practice would need decimal adjustment
            let expected = 500_000_000.0_f64;
            assert!((price.get().to_f64().unwrap() - expected).abs() < 1.0);
        }

        #[test]
        fn is_quote_expired_false() {
            let adapter = BebopAdapter::new(test_config()).unwrap();
            let quote = test_quote_data();
            assert!(!adapter.is_quote_expired(&quote));
        }

        #[test]
        fn is_quote_expired_true() {
            let adapter = BebopAdapter::new(test_config()).unwrap();
            let mut quote = test_quote_data();
            quote.expiry = 1000; // Very old timestamp
            assert!(adapter.is_quote_expired(&quote));
        }

        #[test]
        fn time_to_expiry() {
            let adapter = BebopAdapter::new(test_config()).unwrap();
            let quote = test_quote_data();
            let ttl = adapter.time_to_expiry(&quote);
            // Should be around 60 seconds
            assert!(ttl > 50 && ttl <= 60);
        }
    }

    mod venue_adapter {
        use super::*;

        #[tokio::test]
        async fn health_check_disabled() {
            let config = test_config().with_enabled(false);
            let adapter = BebopAdapter::new(config).unwrap();
            let health = adapter.health_check().await.unwrap();
            assert!(!health.is_healthy());
        }

        #[tokio::test]
        async fn is_available() {
            let adapter = BebopAdapter::new(test_config()).unwrap();
            assert!(adapter.is_available().await);

            let disabled_adapter = BebopAdapter::new(test_config().with_enabled(false)).unwrap();
            assert!(!disabled_adapter.is_available().await);
        }

        #[tokio::test]
        async fn venue_id() {
            let adapter = BebopAdapter::new(test_config()).unwrap();
            assert_eq!(adapter.venue_id(), &VenueId::new("bebop"));
        }

        #[tokio::test]
        async fn timeout_ms() {
            let adapter = BebopAdapter::new(test_config()).unwrap();
            assert_eq!(adapter.timeout_ms(), 3000);
        }
    }
}
