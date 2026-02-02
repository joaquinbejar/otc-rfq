//! # Hashflow Adapter
//!
//! Adapter for Hashflow RFQ protocol with gasless trading and MEV protection.
//!
//! This module provides the [`HashflowAdapter`] which implements the
//! [`VenueAdapter`] trait for the Hashflow RFQ protocol.
//!
//! # Features
//!
//! - HTTP client for Hashflow API
//! - RFQ endpoint integration (/taker/v3/rfq)
//! - Signed quote handling with EIP-712 signatures
//! - Gasless execution support
//! - Quote expiry tracking
//! - MEV protection
//!
//! # Examples
//!
//! ```ignore
//! use otc_rfq::infrastructure::venues::rfq_protocols::hashflow::{HashflowAdapter, HashflowConfig};
//!
//! let config = HashflowConfig::new("my-api-key")
//!     .with_chain(HashflowChain::Ethereum);
//!
//! let adapter = HashflowAdapter::new(config);
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

/// Base URL for Hashflow API.
const BASE_URL: &str = "https://api.hashflow.com";

/// Supported blockchain chains for Hashflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HashflowChain {
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
    /// Avalanche C-Chain (chain ID 43114).
    Avalanche,
}

impl HashflowChain {
    /// Returns the chain ID.
    #[must_use]
    pub fn chain_id(&self) -> u64 {
        match self {
            Self::Ethereum => 1,
            Self::Polygon => 137,
            Self::Arbitrum => 42161,
            Self::Optimism => 10,
            Self::Bsc => 56,
            Self::Avalanche => 43114,
        }
    }

    /// Returns the chain name as used in Hashflow API.
    #[must_use]
    pub fn api_name(&self) -> &'static str {
        match self {
            Self::Ethereum => "ethereum",
            Self::Polygon => "polygon",
            Self::Arbitrum => "arbitrum",
            Self::Optimism => "optimism",
            Self::Bsc => "bsc",
            Self::Avalanche => "avalanche",
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
            // BSC and Avalanche not in domain model yet
            Self::Bsc | Self::Avalanche => None,
        }
    }
}

impl fmt::Display for HashflowChain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.api_name())
    }
}

/// Market maker information in Hashflow response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HashflowMarketMaker {
    /// Market maker identifier.
    pub mm_id: String,
    /// Market maker name.
    pub name: Option<String>,
}

/// Quote data from Hashflow RFQ response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HashflowQuoteData {
    /// Quote ID.
    pub quote_id: String,
    /// Chain ID.
    pub chain_id: u64,
    /// Base token address.
    pub base_token: String,
    /// Quote token address.
    pub quote_token: String,
    /// Base token amount.
    pub base_token_amount: String,
    /// Quote token amount.
    pub quote_token_amount: String,
    /// Quote expiry timestamp (Unix seconds).
    pub quote_expiry: u64,
    /// Nonce for the quote.
    pub nonce: String,
    /// Transaction deadline.
    pub txn_deadline: u64,
    /// Pool address.
    pub pool: String,
    /// External account (taker address).
    pub external_account: String,
    /// Trader address.
    pub trader: String,
    /// Effective base token amount after fees.
    pub effective_base_token_amount: Option<String>,
    /// EIP-712 signature.
    pub signature: String,
}

/// Response from the Hashflow RFQ endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HashflowRfqResponse {
    /// Status of the response.
    pub status: String,
    /// Quote data.
    pub quotes: Vec<HashflowQuoteData>,
    /// Market makers that provided quotes.
    pub market_makers: Option<Vec<HashflowMarketMaker>>,
}

/// Request body for Hashflow RFQ endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HashflowRfqRequest {
    /// Source chain ID.
    pub source_chain_id: u64,
    /// Destination chain ID (same as source for same-chain swaps).
    pub destination_chain_id: u64,
    /// Base token address.
    pub base_token: String,
    /// Quote token address.
    pub quote_token: String,
    /// Base token amount.
    pub base_token_amount: String,
    /// Taker wallet address.
    pub wallet: String,
    /// Whether to include fees in the quote.
    pub include_fees: Option<bool>,
}

/// Error response from Hashflow API.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HashflowErrorResponse {
    /// Error status.
    pub status: String,
    /// Error message.
    pub message: Option<String>,
    /// Error code.
    pub code: Option<String>,
}

/// Configuration for the Hashflow adapter.
///
/// # Examples
///
/// ```
/// use otc_rfq::infrastructure::venues::rfq_protocols::hashflow::{HashflowConfig, HashflowChain};
///
/// let config = HashflowConfig::new("my-api-key")
///     .with_chain(HashflowChain::Polygon)
///     .with_timeout_ms(3000);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HashflowConfig {
    /// Venue ID for this adapter.
    venue_id: VenueId,
    /// API key for Hashflow.
    api_key: String,
    /// Target blockchain.
    chain: HashflowChain,
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
}

impl HashflowConfig {
    /// Creates a new Hashflow configuration.
    #[must_use]
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            venue_id: VenueId::new("hashflow"),
            api_key: api_key.into(),
            chain: HashflowChain::default(),
            timeout_ms: DEFAULT_TIMEOUT_MS,
            enabled: true,
            wallet_address: None,
            token_addresses: Self::default_token_addresses(),
            gasless: true,
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
    pub fn with_chain(mut self, chain: HashflowChain) -> Self {
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
    pub fn chain(&self) -> HashflowChain {
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

    /// Builds the RFQ URL.
    #[must_use]
    pub fn rfq_url(&self) -> String {
        format!("{}/taker/v3/rfq", BASE_URL)
    }

    /// Builds the trade URL for gasless execution.
    #[must_use]
    pub fn trade_url(&self) -> String {
        format!("{}/taker/v3/trade", BASE_URL)
    }
}

/// Hashflow RFQ protocol adapter.
///
/// Implements the [`VenueAdapter`] trait for the Hashflow Protocol.
///
/// # Features
///
/// - Gasless trading with MEV protection
/// - Signed quotes with EIP-712 signatures
/// - Quote expiry tracking
/// - Multi-chain support
///
/// # Note
///
/// This is a stub implementation. Full HTTP client integration
/// will be completed with reqwest.
pub struct HashflowAdapter {
    /// Configuration.
    config: HashflowConfig,
}

impl HashflowAdapter {
    /// Creates a new Hashflow adapter.
    #[must_use]
    pub fn new(config: HashflowConfig) -> Self {
        Self { config }
    }

    /// Returns the configuration.
    #[inline]
    #[must_use]
    pub fn config(&self) -> &HashflowConfig {
        &self.config
    }

    /// Resolves token addresses from an RFQ.
    ///
    /// Returns (base_token_address, quote_token_address).
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

        Ok((base_address, quote_address))
    }

    /// Converts a quantity to the smallest unit based on decimals.
    #[must_use]
    pub fn to_smallest_unit(&self, quantity: Decimal, decimals: u8) -> String {
        let multiplier = Decimal::from(10u64.pow(u32::from(decimals)));
        let amount = quantity * multiplier;
        amount.trunc().to_string()
    }

    /// Builds an RFQ request.
    ///
    /// # Errors
    ///
    /// Returns `VenueError::InvalidRequest` if required configuration is missing.
    pub fn build_rfq_request(&self, rfq: &Rfq) -> VenueResult<HashflowRfqRequest> {
        let (base_token, quote_token) = self.resolve_tokens(rfq)?;

        let wallet = self
            .config
            .wallet_address()
            .ok_or_else(|| VenueError::invalid_request("Wallet address not configured"))?
            .to_string();

        // Determine base token amount based on side
        let base_token_amount = match rfq.side() {
            OrderSide::Buy => self.to_smallest_unit(rfq.quantity().get(), 18),
            OrderSide::Sell => self.to_smallest_unit(rfq.quantity().get(), 18),
        };

        let chain_id = self.config.chain().chain_id();

        Ok(HashflowRfqRequest {
            source_chain_id: chain_id,
            destination_chain_id: chain_id,
            base_token,
            quote_token,
            base_token_amount,
            wallet,
            include_fees: Some(true),
        })
    }

    /// Calculates the price from a quote response.
    ///
    /// # Errors
    ///
    /// Returns `VenueError::ProtocolError` if the price cannot be calculated.
    pub fn calculate_price(&self, quote: &HashflowQuoteData) -> VenueResult<Price> {
        let base_amount: f64 = quote
            .base_token_amount
            .parse()
            .map_err(|_| VenueError::protocol_error("Invalid base_token_amount"))?;

        let quote_amount: f64 = quote
            .quote_token_amount
            .parse()
            .map_err(|_| VenueError::protocol_error("Invalid quote_token_amount"))?;

        if base_amount == 0.0 {
            return Err(VenueError::protocol_error("base_token_amount is zero"));
        }

        // Assuming 18 decimals for both tokens (simplified)
        let price = quote_amount / base_amount;

        Price::new(price).map_err(|_| VenueError::protocol_error("Invalid price value"))
    }

    /// Checks if a quote has expired.
    #[must_use]
    pub fn is_quote_expired(&self, quote: &HashflowQuoteData) -> bool {
        let now = Timestamp::now().timestamp_secs() as u64;
        now >= quote.quote_expiry
    }

    /// Returns the time until quote expiry in seconds.
    #[must_use]
    pub fn time_to_expiry(&self, quote: &HashflowQuoteData) -> i64 {
        let now = Timestamp::now().timestamp_secs() as u64;
        quote.quote_expiry as i64 - now as i64
    }

    /// Parses an RFQ response into a domain Quote.
    ///
    /// # Errors
    ///
    /// Returns `VenueError::ProtocolError` if the response cannot be parsed.
    pub fn parse_rfq_response(
        &self,
        response: HashflowRfqResponse,
        rfq: &Rfq,
    ) -> VenueResult<Quote> {
        // Get the best quote (first one)
        let quote_data = response
            .quotes
            .first()
            .ok_or_else(|| VenueError::protocol_error("No quotes in response"))?;

        // Check if quote is already expired
        if self.is_quote_expired(quote_data) {
            return Err(VenueError::quote_expired("Quote has already expired"));
        }

        let price = self.calculate_price(quote_data)?;
        let valid_until = Timestamp::from_secs(quote_data.quote_expiry as i64)
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
        metadata.set("pool", quote_data.pool.clone());
        metadata.set("nonce", quote_data.nonce.clone());
        metadata.set("base_token", quote_data.base_token.clone());
        metadata.set("quote_token", quote_data.quote_token.clone());
        metadata.set("base_token_amount", quote_data.base_token_amount.clone());
        metadata.set("quote_token_amount", quote_data.quote_token_amount.clone());
        metadata.set("txn_deadline", quote_data.txn_deadline.to_string());
        metadata.set("chain_id", quote_data.chain_id.to_string());

        if let Some(effective) = &quote_data.effective_base_token_amount {
            metadata.set("effective_base_token_amount", effective.clone());
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

        let required_fields = ["quote_id", "signature", "pool", "nonce"];

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

impl fmt::Debug for HashflowAdapter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HashflowAdapter")
            .field("venue_id", self.config.venue_id())
            .field("chain", &self.config.chain())
            .field("gasless", &self.config.is_gasless())
            .field("enabled", &self.config.is_enabled())
            .finish()
    }
}

#[async_trait]
impl VenueAdapter for HashflowAdapter {
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
                "Hashflow adapter is disabled",
            ));
        }

        // Build RFQ request
        let _request = self.build_rfq_request(rfq)?;

        // Build RFQ URL
        let _url = self.config.rfq_url();

        // TODO: Make HTTP request to Hashflow API
        // For now, return a stub error indicating not implemented
        Err(VenueError::internal_error(
            "Hashflow API integration not yet implemented - requires HTTP client",
        ))
    }

    async fn execute_trade(&self, quote: &Quote) -> VenueResult<ExecutionResult> {
        // Check if enabled
        if !self.config.is_enabled() {
            return Err(VenueError::venue_unavailable(
                self.config.venue_id().clone(),
                "Hashflow adapter is disabled",
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

        // Get execution URL based on gasless setting
        let _url = if self.config.is_gasless() {
            self.config.trade_url()
        } else {
            // For non-gasless, would need on-chain execution
            return Err(VenueError::internal_error(
                "Non-gasless execution not yet implemented",
            ));
        };

        // TODO: Execute via Hashflow API (gasless) or on-chain
        Err(VenueError::internal_error(
            "Hashflow execution not yet implemented - requires HTTP client",
        ))
    }

    async fn health_check(&self) -> VenueResult<VenueHealth> {
        // TODO: Make health check request to Hashflow API
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

    fn test_config() -> HashflowConfig {
        HashflowConfig::new("test-api-key")
            .with_chain(HashflowChain::Ethereum)
            .with_wallet_address("0x1234567890abcdef1234567890abcdef12345678")
            .with_timeout_ms(3000)
    }

    mod chain {
        use super::*;

        #[test]
        fn chain_ids() {
            assert_eq!(HashflowChain::Ethereum.chain_id(), 1);
            assert_eq!(HashflowChain::Polygon.chain_id(), 137);
            assert_eq!(HashflowChain::Arbitrum.chain_id(), 42161);
            assert_eq!(HashflowChain::Optimism.chain_id(), 10);
            assert_eq!(HashflowChain::Bsc.chain_id(), 56);
            assert_eq!(HashflowChain::Avalanche.chain_id(), 43114);
        }

        #[test]
        fn api_names() {
            assert_eq!(HashflowChain::Ethereum.api_name(), "ethereum");
            assert_eq!(HashflowChain::Polygon.api_name(), "polygon");
            assert_eq!(HashflowChain::Bsc.api_name(), "bsc");
        }

        #[test]
        fn display() {
            assert_eq!(HashflowChain::Ethereum.to_string(), "ethereum");
            assert_eq!(HashflowChain::Polygon.to_string(), "polygon");
        }

        #[test]
        fn to_blockchain() {
            assert_eq!(
                HashflowChain::Ethereum.to_blockchain(),
                Some(Blockchain::Ethereum)
            );
            assert_eq!(
                HashflowChain::Polygon.to_blockchain(),
                Some(Blockchain::Polygon)
            );
            assert_eq!(HashflowChain::Bsc.to_blockchain(), None);
        }
    }

    mod config {
        use super::*;

        #[test]
        fn new() {
            let config = HashflowConfig::new("my-key");
            assert_eq!(config.api_key(), "my-key");
            assert_eq!(config.chain(), HashflowChain::Ethereum);
            assert!(config.is_enabled());
            assert!(config.is_gasless());
        }

        #[test]
        fn with_chain() {
            let config = HashflowConfig::new("key").with_chain(HashflowChain::Polygon);
            assert_eq!(config.chain(), HashflowChain::Polygon);
        }

        #[test]
        fn with_timeout() {
            let config = HashflowConfig::new("key").with_timeout_ms(10000);
            assert_eq!(config.timeout_ms(), 10000);
        }

        #[test]
        fn with_wallet_address() {
            let config = HashflowConfig::new("key").with_wallet_address("0x1234");
            assert_eq!(config.wallet_address(), Some("0x1234"));
        }

        #[test]
        fn with_gasless() {
            let config = HashflowConfig::new("key").with_gasless(false);
            assert!(!config.is_gasless());
        }

        #[test]
        fn default_token_addresses() {
            let config = HashflowConfig::new("key");
            assert!(config.resolve_token_address("WETH").is_some());
            assert!(config.resolve_token_address("USDC").is_some());
        }

        #[test]
        fn rfq_url() {
            let config = HashflowConfig::new("key");
            assert_eq!(config.rfq_url(), "https://api.hashflow.com/taker/v3/rfq");
        }

        #[test]
        fn trade_url() {
            let config = HashflowConfig::new("key");
            assert_eq!(
                config.trade_url(),
                "https://api.hashflow.com/taker/v3/trade"
            );
        }
    }

    mod adapter {
        use super::*;

        #[test]
        fn new() {
            let adapter = HashflowAdapter::new(test_config());
            assert_eq!(adapter.venue_id(), &VenueId::new("hashflow"));
        }

        #[test]
        fn debug_format() {
            let adapter = HashflowAdapter::new(test_config());
            let debug = format!("{:?}", adapter);
            assert!(debug.contains("HashflowAdapter"));
            assert!(debug.contains("hashflow"));
            assert!(debug.contains("gasless"));
        }

        #[test]
        fn to_smallest_unit() {
            let adapter = HashflowAdapter::new(test_config());
            let amount = adapter.to_smallest_unit(Decimal::from(1), 18);
            assert_eq!(amount, "1000000000000000000");

            let amount_6 = adapter.to_smallest_unit(Decimal::from(1), 6);
            assert_eq!(amount_6, "1000000");
        }

        #[tokio::test]
        async fn is_available_when_enabled() {
            let adapter = HashflowAdapter::new(test_config());
            assert!(adapter.is_available().await);
        }

        #[tokio::test]
        async fn is_not_available_when_disabled() {
            let config = test_config().with_enabled(false);
            let adapter = HashflowAdapter::new(config);
            assert!(!adapter.is_available().await);
        }

        #[tokio::test]
        async fn health_check_healthy_when_enabled() {
            let adapter = HashflowAdapter::new(test_config());
            let health = adapter.health_check().await.unwrap();
            assert!(health.is_healthy());
        }

        #[tokio::test]
        async fn health_check_unhealthy_when_disabled() {
            let config = test_config().with_enabled(false);
            let adapter = HashflowAdapter::new(config);
            let health = adapter.health_check().await.unwrap();
            assert!(!health.is_healthy());
        }
    }

    mod quote_handling {
        use super::*;

        fn test_quote_data() -> HashflowQuoteData {
            let now = Timestamp::now().timestamp_secs() as u64;
            HashflowQuoteData {
                quote_id: "quote-123".to_string(),
                chain_id: 1,
                base_token: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string(),
                quote_token: "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".to_string(),
                base_token_amount: "1000000000000000000".to_string(), // 1 WETH
                quote_token_amount: "1850000000000000000000".to_string(), // 1850 USDC (18 decimals)
                quote_expiry: now + 60,                               // 60 seconds from now
                nonce: "12345".to_string(),
                txn_deadline: now + 120,
                pool: "0xpool".to_string(),
                external_account: "0xtaker".to_string(),
                trader: "0xtrader".to_string(),
                effective_base_token_amount: None,
                signature: "0xsignature".to_string(),
            }
        }

        #[test]
        fn calculate_price() {
            let adapter = HashflowAdapter::new(test_config());
            let quote = test_quote_data();
            let price = adapter.calculate_price(&quote).unwrap();
            // 1850 / 1 = 1850
            assert!((price.get().to_f64().unwrap() - 1850.0).abs() < 0.01);
        }

        #[test]
        fn is_quote_expired_false() {
            let adapter = HashflowAdapter::new(test_config());
            let quote = test_quote_data();
            assert!(!adapter.is_quote_expired(&quote));
        }

        #[test]
        fn is_quote_expired_true() {
            let adapter = HashflowAdapter::new(test_config());
            let now = Timestamp::now().timestamp_secs() as u64;
            let mut quote = test_quote_data();
            quote.quote_expiry = now - 10; // 10 seconds ago
            assert!(adapter.is_quote_expired(&quote));
        }

        #[test]
        fn time_to_expiry() {
            let adapter = HashflowAdapter::new(test_config());
            let quote = test_quote_data();
            let ttl = adapter.time_to_expiry(&quote);
            // Should be around 60 seconds (give or take a few for test execution)
            assert!(ttl > 55 && ttl <= 60);
        }
    }
}
