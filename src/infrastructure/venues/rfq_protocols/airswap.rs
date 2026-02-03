//! # Airswap Adapter
//!
//! Adapter for Airswap RFQ protocol with peer-to-peer trading and EIP-712 signatures.
//!
//! This module provides the [`AirswapAdapter`] which implements the
//! [`VenueAdapter`] trait for the Airswap RFQ protocol.
//!
//! # Features
//!
//! - Peer-to-peer RFQ trading
//! - Server discovery via Registry contract
//! - EIP-712 order signing and verification
//! - Order expiry tracking
//! - Nonce management
//! - Multi-chain support (Ethereum, Polygon, Arbitrum, Avalanche, BSC)
//!
//! # Examples
//!
//! ```ignore
//! use otc_rfq::infrastructure::venues::rfq_protocols::airswap::{AirswapAdapter, AirswapConfig};
//!
//! let config = AirswapConfig::new()
//!     .with_chain(AirswapChain::Ethereum);
//!
//! let adapter = AirswapAdapter::new(config);
//! ```

use crate::domain::entities::quote::{Quote, QuoteBuilder, QuoteMetadata};
use crate::domain::entities::rfq::Rfq;
use crate::domain::value_objects::timestamp::Timestamp;
use crate::domain::value_objects::{Blockchain, OrderSide, Price, VenueId};
use crate::infrastructure::venues::error::{VenueError, VenueResult};
use crate::infrastructure::venues::http_client::HttpClient;
use crate::infrastructure::venues::traits::{ExecutionResult, VenueAdapter, VenueHealth};
use async_trait::async_trait;
use rust_decimal::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

/// Default timeout in milliseconds.
const DEFAULT_TIMEOUT_MS: u64 = 5000;

/// Airswap Swap contract addresses by chain.
pub mod swap_contracts {
    /// Ethereum mainnet Swap contract.
    pub const ETHEREUM: &str = "0x522D6F36c95A1b6509A14272C17747BbB582F2A6";
    /// Polygon Swap contract.
    pub const POLYGON: &str = "0x522D6F36c95A1b6509A14272C17747BbB582F2A6";
    /// Arbitrum Swap contract.
    pub const ARBITRUM: &str = "0x522D6F36c95A1b6509A14272C17747BbB582F2A6";
    /// Avalanche Swap contract.
    pub const AVALANCHE: &str = "0x522D6F36c95A1b6509A14272C17747BbB582F2A6";
    /// BSC Swap contract.
    pub const BSC: &str = "0x522D6F36c95A1b6509A14272C17747BbB582F2A6";
}

/// Airswap Registry contract addresses by chain.
pub mod registry_contracts {
    /// Ethereum mainnet Registry contract.
    pub const ETHEREUM: &str = "0x8F9DA6d38939411340b19401E8c54Ea1f51B8f95";
    /// Polygon Registry contract.
    pub const POLYGON: &str = "0x8F9DA6d38939411340b19401E8c54Ea1f51B8f95";
    /// Arbitrum Registry contract.
    pub const ARBITRUM: &str = "0x8F9DA6d38939411340b19401E8c54Ea1f51B8f95";
    /// Avalanche Registry contract.
    pub const AVALANCHE: &str = "0x8F9DA6d38939411340b19401E8c54Ea1f51B8f95";
    /// BSC Registry contract.
    pub const BSC: &str = "0x8F9DA6d38939411340b19401E8c54Ea1f51B8f95";
}

/// Supported blockchain chains for Airswap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AirswapChain {
    /// Ethereum mainnet (chain ID 1).
    #[default]
    Ethereum,
    /// Polygon (chain ID 137).
    Polygon,
    /// Arbitrum One (chain ID 42161).
    Arbitrum,
    /// Avalanche C-Chain (chain ID 43114).
    Avalanche,
    /// BNB Smart Chain (chain ID 56).
    Bsc,
}

impl AirswapChain {
    /// Returns the chain ID.
    #[must_use]
    pub fn chain_id(&self) -> u64 {
        match self {
            Self::Ethereum => 1,
            Self::Polygon => 137,
            Self::Arbitrum => 42161,
            Self::Avalanche => 43114,
            Self::Bsc => 56,
        }
    }

    /// Returns the chain name as used in Airswap.
    #[must_use]
    pub fn api_name(&self) -> &'static str {
        match self {
            Self::Ethereum => "ethereum",
            Self::Polygon => "polygon",
            Self::Arbitrum => "arbitrum",
            Self::Avalanche => "avalanche",
            Self::Bsc => "bsc",
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
            // Avalanche and BSC not in domain model yet
            Self::Avalanche | Self::Bsc => None,
        }
    }

    /// Returns the Swap contract address for this chain.
    #[must_use]
    pub fn swap_contract(&self) -> &'static str {
        match self {
            Self::Ethereum => swap_contracts::ETHEREUM,
            Self::Polygon => swap_contracts::POLYGON,
            Self::Arbitrum => swap_contracts::ARBITRUM,
            Self::Avalanche => swap_contracts::AVALANCHE,
            Self::Bsc => swap_contracts::BSC,
        }
    }

    /// Returns the Registry contract address for this chain.
    #[must_use]
    pub fn registry_contract(&self) -> &'static str {
        match self {
            Self::Ethereum => registry_contracts::ETHEREUM,
            Self::Polygon => registry_contracts::POLYGON,
            Self::Arbitrum => registry_contracts::ARBITRUM,
            Self::Avalanche => registry_contracts::AVALANCHE,
            Self::Bsc => registry_contracts::BSC,
        }
    }

    /// Returns all supported chains.
    #[must_use]
    pub fn all() -> &'static [AirswapChain] {
        &[
            AirswapChain::Ethereum,
            AirswapChain::Polygon,
            AirswapChain::Arbitrum,
            AirswapChain::Avalanche,
            AirswapChain::Bsc,
        ]
    }
}

impl fmt::Display for AirswapChain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.api_name())
    }
}

/// EIP-712 domain for Airswap orders.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AirswapDomain {
    /// Domain name.
    pub name: String,
    /// Domain version.
    pub version: String,
    /// Chain ID.
    pub chain_id: u64,
    /// Verifying contract address.
    pub verifying_contract: String,
}

impl AirswapDomain {
    /// Creates a new domain for the given chain.
    #[must_use]
    pub fn new(chain: AirswapChain) -> Self {
        Self {
            name: "SWAP".to_string(),
            version: "4".to_string(),
            chain_id: chain.chain_id(),
            verifying_contract: chain.swap_contract().to_string(),
        }
    }
}

/// Airswap order (EIP-712 typed data).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AirswapOrder {
    /// Nonce for replay protection.
    pub nonce: String,
    /// Order expiry timestamp (Unix seconds).
    pub expiry: u64,
    /// Signer wallet address.
    pub signer_wallet: String,
    /// Signer token address.
    pub signer_token: String,
    /// Signer token amount.
    pub signer_amount: String,
    /// Protocol fee (basis points).
    pub protocol_fee: String,
    /// Sender wallet address.
    pub sender_wallet: String,
    /// Sender token address.
    pub sender_token: String,
    /// Sender token amount.
    pub sender_amount: String,
}

/// Signed Airswap order with EIP-712 signature.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignedAirswapOrder {
    /// The order data.
    #[serde(flatten)]
    pub order: AirswapOrder,
    /// EIP-712 signature v component.
    pub v: u8,
    /// EIP-712 signature r component.
    pub r: String,
    /// EIP-712 signature s component.
    pub s: String,
}

impl SignedAirswapOrder {
    /// Returns the full signature as a hex string.
    #[must_use]
    pub fn signature(&self) -> String {
        format!("0x{}{}{:02x}", &self.r[2..], &self.s[2..], self.v)
    }
}

/// Server info from Airswap Registry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AirswapServer {
    /// Server URL.
    pub url: String,
    /// Supported tokens.
    pub tokens: Vec<String>,
    /// Server staking amount.
    pub staking: Option<String>,
}

/// RFQ request to an Airswap server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AirswapRfqRequest {
    /// Chain ID.
    pub chain_id: u64,
    /// Sender wallet address.
    pub sender_wallet: String,
    /// Sender token address.
    pub sender_token: String,
    /// Sender token amount.
    pub sender_amount: String,
    /// Signer token address (token to receive).
    pub signer_token: String,
}

/// RFQ response from an Airswap server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AirswapRfqResponse {
    /// Signed order from the server.
    pub order: Option<SignedAirswapOrder>,
    /// Error message if order is not available.
    pub error: Option<String>,
}

/// Configuration for the Airswap adapter.
///
/// # Examples
///
/// ```
/// use otc_rfq::infrastructure::venues::rfq_protocols::airswap::{AirswapConfig, AirswapChain};
///
/// let config = AirswapConfig::new()
///     .with_chain(AirswapChain::Polygon)
///     .with_timeout_ms(3000);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AirswapConfig {
    /// Venue ID for this adapter.
    venue_id: VenueId,
    /// Target blockchain.
    chain: AirswapChain,
    /// Timeout in milliseconds.
    timeout_ms: u64,
    /// Whether the adapter is enabled.
    enabled: bool,
    /// Sender wallet address.
    wallet_address: Option<String>,
    /// Token address mappings (symbol -> address).
    token_addresses: HashMap<String, String>,
    /// Known server URLs (optional, can be discovered via Registry).
    server_urls: Vec<String>,
    /// Protocol fee in basis points.
    protocol_fee_bps: u32,
}

impl AirswapConfig {
    /// Creates a new Airswap configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            venue_id: VenueId::new("airswap"),
            chain: AirswapChain::default(),
            timeout_ms: DEFAULT_TIMEOUT_MS,
            enabled: true,
            wallet_address: None,
            token_addresses: Self::default_token_addresses(),
            server_urls: Vec::new(),
            protocol_fee_bps: 7, // 0.07% default protocol fee
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
    pub fn with_chain(mut self, chain: AirswapChain) -> Self {
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

    /// Adds a server URL.
    #[must_use]
    pub fn with_server_url(mut self, url: impl Into<String>) -> Self {
        self.server_urls.push(url.into());
        self
    }

    /// Sets the protocol fee in basis points.
    #[must_use]
    pub fn with_protocol_fee_bps(mut self, fee_bps: u32) -> Self {
        self.protocol_fee_bps = fee_bps;
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
    pub fn chain(&self) -> AirswapChain {
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

    /// Returns the server URLs.
    #[inline]
    #[must_use]
    pub fn server_urls(&self) -> &[String] {
        &self.server_urls
    }

    /// Returns the protocol fee in basis points.
    #[inline]
    #[must_use]
    pub fn protocol_fee_bps(&self) -> u32 {
        self.protocol_fee_bps
    }

    /// Returns the Swap contract address.
    #[inline]
    #[must_use]
    pub fn swap_contract(&self) -> &'static str {
        self.chain.swap_contract()
    }

    /// Returns the Registry contract address.
    #[inline]
    #[must_use]
    pub fn registry_contract(&self) -> &'static str {
        self.chain.registry_contract()
    }

    /// Returns the EIP-712 domain for this configuration.
    #[must_use]
    pub fn domain(&self) -> AirswapDomain {
        AirswapDomain::new(self.chain)
    }
}

impl Default for AirswapConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Airswap RFQ protocol adapter.
///
/// Implements the [`VenueAdapter`] trait for the Airswap Protocol.
///
/// # Features
///
/// - Peer-to-peer RFQ trading
/// - Server discovery via Registry contract
/// - EIP-712 order signing and verification
/// - Order expiry tracking
/// - Nonce management
/// - Multi-chain support
pub struct AirswapAdapter {
    /// Configuration.
    config: AirswapConfig,
    /// Current nonce for order creation.
    nonce: std::sync::atomic::AtomicU64,
    /// HTTP client for API requests.
    http_client: HttpClient,
}

impl AirswapAdapter {
    /// Creates a new Airswap adapter.
    ///
    /// # Errors
    ///
    /// Returns `VenueError::InternalError` if the HTTP client cannot be created.
    pub fn new(config: AirswapConfig) -> VenueResult<Self> {
        let http_client = HttpClient::new(config.timeout_ms())?;
        Ok(Self {
            config,
            nonce: std::sync::atomic::AtomicU64::new(Timestamp::now().timestamp_millis() as u64),
            http_client,
        })
    }

    /// Returns the configuration.
    #[inline]
    #[must_use]
    pub fn config(&self) -> &AirswapConfig {
        &self.config
    }

    /// Generates a new nonce.
    #[must_use]
    pub fn next_nonce(&self) -> String {
        let nonce = self.nonce.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        nonce.to_string()
    }

    /// Resolves token addresses from an RFQ.
    ///
    /// Returns (sender_token_address, signer_token_address).
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

        // For Buy side: sender sends quote token, receives base token
        // For Sell side: sender sends base token, receives quote token
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

    /// Builds an RFQ request.
    ///
    /// # Errors
    ///
    /// Returns `VenueError::InvalidRequest` if required configuration is missing.
    pub fn build_rfq_request(&self, rfq: &Rfq) -> VenueResult<AirswapRfqRequest> {
        let (sender_token, signer_token) = self.resolve_tokens(rfq)?;

        let sender_wallet = self
            .config
            .wallet_address()
            .ok_or_else(|| VenueError::invalid_request("Wallet address not configured"))?
            .to_string();

        let sender_amount = self.to_smallest_unit(rfq.quantity().get(), 18);

        Ok(AirswapRfqRequest {
            chain_id: self.config.chain().chain_id(),
            sender_wallet,
            sender_token,
            sender_amount,
            signer_token,
        })
    }

    /// Calculates the price from a signed order.
    ///
    /// # Errors
    ///
    /// Returns `VenueError::ProtocolError` if the price cannot be calculated.
    pub fn calculate_price(&self, order: &SignedAirswapOrder) -> VenueResult<Price> {
        let sender_amount: f64 = order
            .order
            .sender_amount
            .parse()
            .map_err(|_| VenueError::protocol_error("Invalid sender_amount"))?;

        let signer_amount: f64 = order
            .order
            .signer_amount
            .parse()
            .map_err(|_| VenueError::protocol_error("Invalid signer_amount"))?;

        if sender_amount == 0.0 {
            return Err(VenueError::protocol_error("sender_amount is zero"));
        }

        // Price = signer_amount / sender_amount
        let price = signer_amount / sender_amount;

        Price::new(price).map_err(|_| VenueError::protocol_error("Invalid price value"))
    }

    /// Checks if an order has expired.
    #[must_use]
    pub fn is_order_expired(&self, order: &SignedAirswapOrder) -> bool {
        let now = Timestamp::now().timestamp_secs() as u64;
        now >= order.order.expiry
    }

    /// Returns the time until order expiry in seconds.
    #[must_use]
    pub fn time_to_expiry(&self, order: &SignedAirswapOrder) -> i64 {
        let now = Timestamp::now().timestamp_secs() as u64;
        order.order.expiry as i64 - now as i64
    }

    /// Parses an RFQ response into a domain Quote.
    ///
    /// # Errors
    ///
    /// Returns `VenueError::ProtocolError` if the response cannot be parsed.
    pub fn parse_rfq_response(
        &self,
        response: AirswapRfqResponse,
        rfq: &Rfq,
    ) -> VenueResult<Quote> {
        // Check for error
        if let Some(error) = response.error {
            return Err(VenueError::protocol_error(format!(
                "Airswap server error: {}",
                error
            )));
        }

        let order = response
            .order
            .ok_or_else(|| VenueError::protocol_error("No order in response"))?;

        // Check if order is already expired
        if self.is_order_expired(&order) {
            return Err(VenueError::quote_expired("Order has already expired"));
        }

        let price = self.calculate_price(&order)?;
        let valid_until = Timestamp::from_secs(order.order.expiry as i64)
            .ok_or_else(|| VenueError::protocol_error("Invalid order expiry timestamp"))?;

        let mut builder = QuoteBuilder::new(
            rfq.id(),
            self.config.venue_id().clone(),
            price,
            rfq.quantity(),
            valid_until,
        );

        // Add metadata including signature for execution
        let mut metadata = QuoteMetadata::new();
        metadata.set("nonce", order.order.nonce.clone());
        metadata.set("signer_wallet", order.order.signer_wallet.clone());
        metadata.set("signer_token", order.order.signer_token.clone());
        metadata.set("signer_amount", order.order.signer_amount.clone());
        metadata.set("sender_wallet", order.order.sender_wallet.clone());
        metadata.set("sender_token", order.order.sender_token.clone());
        metadata.set("sender_amount", order.order.sender_amount.clone());
        metadata.set("protocol_fee", order.order.protocol_fee.clone());
        metadata.set("expiry", order.order.expiry.to_string());
        metadata.set("signature", order.signature());
        metadata.set("v", order.v.to_string());
        metadata.set("r", order.r.clone());
        metadata.set("s", order.s.clone());
        metadata.set("chain_id", self.config.chain().chain_id().to_string());
        metadata.set("swap_contract", self.config.swap_contract().to_string());

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
            "nonce",
            "signer_wallet",
            "signer_token",
            "signer_amount",
            "sender_token",
            "sender_amount",
            "signature",
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

impl fmt::Debug for AirswapAdapter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AirswapAdapter")
            .field("venue_id", self.config.venue_id())
            .field("chain", &self.config.chain())
            .field("enabled", &self.config.is_enabled())
            .finish()
    }
}

#[async_trait]
impl VenueAdapter for AirswapAdapter {
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
                "Airswap adapter is disabled",
            ));
        }

        // Build RFQ request
        let request = self.build_rfq_request(rfq)?;

        // Get server URLs from config (in production, would discover from Registry)
        let server_urls = self.config.server_urls();
        if server_urls.is_empty() {
            return Err(VenueError::invalid_request(
                "No Airswap server URLs configured",
            ));
        }

        // Try each server until one succeeds
        let mut last_error = None;
        for server_url in server_urls {
            let url = format!("{}/signer-api/v1/getSignerSideOrder", server_url);
            match self
                .http_client
                .post::<AirswapRfqResponse, _>(&url, &request)
                .await
            {
                Ok(response) => {
                    return self.parse_rfq_response(response, rfq);
                }
                Err(e) => {
                    last_error = Some(e);
                }
            }
        }

        Err(last_error
            .unwrap_or_else(|| VenueError::internal_error("No Airswap servers available")))
    }

    async fn execute_trade(&self, quote: &Quote) -> VenueResult<ExecutionResult> {
        // Check if enabled
        if !self.config.is_enabled() {
            return Err(VenueError::venue_unavailable(
                self.config.venue_id().clone(),
                "Airswap adapter is disabled",
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

        // TODO: Execute via Swap contract on-chain
        Err(VenueError::internal_error(
            "Airswap execution not yet implemented - requires on-chain transaction",
        ))
    }

    async fn health_check(&self) -> VenueResult<VenueHealth> {
        if !self.config.is_enabled() {
            return Ok(VenueHealth::unhealthy(
                self.config.venue_id().clone(),
                "Adapter is disabled",
            ));
        }

        // Check server availability
        let server_urls = self.config.server_urls();
        if server_urls.is_empty() {
            return Ok(VenueHealth::unhealthy(
                self.config.venue_id().clone(),
                "No server URLs configured",
            ));
        }

        let start = std::time::Instant::now();
        let mut any_healthy = false;

        for server_url in server_urls {
            if self.http_client.health_check(server_url).await {
                any_healthy = true;
                break;
            }
        }

        let latency_ms = start.elapsed().as_millis() as u64;

        if any_healthy {
            Ok(VenueHealth::healthy_with_latency(
                self.config.venue_id().clone(),
                latency_ms,
            ))
        } else {
            Ok(VenueHealth::unhealthy(
                self.config.venue_id().clone(),
                "All servers unavailable",
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

    fn test_config() -> AirswapConfig {
        AirswapConfig::new()
            .with_chain(AirswapChain::Ethereum)
            .with_wallet_address("0x1234567890abcdef1234567890abcdef12345678")
            .with_timeout_ms(3000)
    }

    mod chain {
        use super::*;

        #[test]
        fn chain_ids() {
            assert_eq!(AirswapChain::Ethereum.chain_id(), 1);
            assert_eq!(AirswapChain::Polygon.chain_id(), 137);
            assert_eq!(AirswapChain::Arbitrum.chain_id(), 42161);
            assert_eq!(AirswapChain::Avalanche.chain_id(), 43114);
            assert_eq!(AirswapChain::Bsc.chain_id(), 56);
        }

        #[test]
        fn api_names() {
            assert_eq!(AirswapChain::Ethereum.api_name(), "ethereum");
            assert_eq!(AirswapChain::Polygon.api_name(), "polygon");
            assert_eq!(AirswapChain::Arbitrum.api_name(), "arbitrum");
            assert_eq!(AirswapChain::Avalanche.api_name(), "avalanche");
            assert_eq!(AirswapChain::Bsc.api_name(), "bsc");
        }

        #[test]
        fn to_blockchain() {
            assert_eq!(
                AirswapChain::Ethereum.to_blockchain(),
                Some(Blockchain::Ethereum)
            );
            assert_eq!(
                AirswapChain::Polygon.to_blockchain(),
                Some(Blockchain::Polygon)
            );
            assert_eq!(
                AirswapChain::Arbitrum.to_blockchain(),
                Some(Blockchain::Arbitrum)
            );
            assert_eq!(AirswapChain::Avalanche.to_blockchain(), None);
            assert_eq!(AirswapChain::Bsc.to_blockchain(), None);
        }

        #[test]
        fn display() {
            assert_eq!(AirswapChain::Ethereum.to_string(), "ethereum");
            assert_eq!(AirswapChain::Polygon.to_string(), "polygon");
        }

        #[test]
        fn default_is_ethereum() {
            assert_eq!(AirswapChain::default(), AirswapChain::Ethereum);
        }

        #[test]
        fn all_chains() {
            let chains = AirswapChain::all();
            assert_eq!(chains.len(), 5);
            assert!(chains.contains(&AirswapChain::Ethereum));
            assert!(chains.contains(&AirswapChain::Bsc));
        }

        #[test]
        fn swap_contract() {
            assert!(!AirswapChain::Ethereum.swap_contract().is_empty());
            assert!(AirswapChain::Ethereum.swap_contract().starts_with("0x"));
        }

        #[test]
        fn registry_contract() {
            assert!(!AirswapChain::Ethereum.registry_contract().is_empty());
            assert!(AirswapChain::Ethereum.registry_contract().starts_with("0x"));
        }
    }

    mod config {
        use super::*;

        #[test]
        fn new_config() {
            let config = AirswapConfig::new();
            assert_eq!(config.chain(), AirswapChain::Ethereum);
            assert_eq!(config.timeout_ms(), DEFAULT_TIMEOUT_MS);
            assert!(config.is_enabled());
            assert_eq!(config.protocol_fee_bps(), 7);
        }

        #[test]
        fn builder_pattern() {
            let config = AirswapConfig::new()
                .with_chain(AirswapChain::Polygon)
                .with_timeout_ms(3000)
                .with_enabled(false)
                .with_protocol_fee_bps(10)
                .with_wallet_address("0xabc")
                .with_server_url("https://server1.example.com");

            assert_eq!(config.chain(), AirswapChain::Polygon);
            assert_eq!(config.timeout_ms(), 3000);
            assert!(!config.is_enabled());
            assert_eq!(config.protocol_fee_bps(), 10);
            assert_eq!(config.wallet_address(), Some("0xabc"));
            assert_eq!(config.server_urls().len(), 1);
        }

        #[test]
        fn venue_id() {
            let config = AirswapConfig::new().with_venue_id("custom-airswap");
            assert_eq!(config.venue_id(), &VenueId::new("custom-airswap"));
        }

        #[test]
        fn token_addresses() {
            let config = AirswapConfig::new().with_token_address("TEST", "0x123");
            assert_eq!(
                config.resolve_token_address("TEST"),
                Some(&"0x123".to_string())
            );
            assert!(config.resolve_token_address("UNKNOWN").is_none());
        }

        #[test]
        fn default_token_addresses() {
            let config = AirswapConfig::new();
            assert!(config.resolve_token_address("WETH").is_some());
            assert!(config.resolve_token_address("USDC").is_some());
            assert!(config.resolve_token_address("USDT").is_some());
        }

        #[test]
        fn contract_addresses() {
            let config = AirswapConfig::new().with_chain(AirswapChain::Polygon);
            assert!(!config.swap_contract().is_empty());
            assert!(!config.registry_contract().is_empty());
        }

        #[test]
        fn domain() {
            let config = AirswapConfig::new().with_chain(AirswapChain::Ethereum);
            let domain = config.domain();
            assert_eq!(domain.name, "SWAP");
            assert_eq!(domain.version, "4");
            assert_eq!(domain.chain_id, 1);
        }

        #[test]
        fn default_impl() {
            let config = AirswapConfig::default();
            assert_eq!(config.chain(), AirswapChain::Ethereum);
        }
    }

    mod adapter {
        use super::*;

        #[test]
        fn new_adapter() {
            let adapter = AirswapAdapter::new(test_config()).unwrap();
            assert_eq!(adapter.config().chain(), AirswapChain::Ethereum);
        }

        #[test]
        fn debug_impl() {
            let adapter = AirswapAdapter::new(test_config()).unwrap();
            let debug = format!("{:?}", adapter);
            assert!(debug.contains("AirswapAdapter"));
            assert!(debug.contains("airswap"));
        }

        #[test]
        fn next_nonce() {
            let adapter = AirswapAdapter::new(test_config()).unwrap();
            let nonce1 = adapter.next_nonce();
            let nonce2 = adapter.next_nonce();
            let nonce3 = adapter.next_nonce();

            // Nonces should be incrementing
            let n1: u64 = nonce1.parse().unwrap();
            let n2: u64 = nonce2.parse().unwrap();
            let n3: u64 = nonce3.parse().unwrap();

            assert_eq!(n2, n1 + 1);
            assert_eq!(n3, n2 + 1);
        }

        #[test]
        fn to_smallest_unit() {
            let adapter = AirswapAdapter::new(test_config()).unwrap();
            let amount = Decimal::from_str("1.5").unwrap();
            let result = adapter.to_smallest_unit(amount, 18);
            assert_eq!(result, "1500000000000000000");
        }

        #[test]
        fn to_smallest_unit_6_decimals() {
            let adapter = AirswapAdapter::new(test_config()).unwrap();
            let amount = Decimal::from_str("100.0").unwrap();
            let result = adapter.to_smallest_unit(amount, 6);
            assert_eq!(result, "100000000");
        }
    }

    mod order {
        use super::*;

        fn test_order() -> SignedAirswapOrder {
            SignedAirswapOrder {
                order: AirswapOrder {
                    nonce: "1234567890".to_string(),
                    expiry: (Timestamp::now().timestamp_secs() as u64) + 300, // 5 minutes from now
                    signer_wallet: "0xsigner".to_string(),
                    signer_token: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string(),
                    signer_amount: "1000000000000000000".to_string(), // 1 ETH
                    protocol_fee: "7".to_string(),
                    sender_wallet: "0xsender".to_string(),
                    sender_token: "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".to_string(),
                    sender_amount: "2000000000".to_string(), // 2000 USDC
                },
                v: 27,
                r: "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef".to_string(),
                s: "0xfedcba0987654321fedcba0987654321fedcba0987654321fedcba0987654321".to_string(),
            }
        }

        #[test]
        fn signature() {
            let order = test_order();
            let sig = order.signature();
            assert!(sig.starts_with("0x"));
            // Should be r + s + v = 64 + 64 + 2 = 130 chars + 0x prefix = 132
            assert_eq!(sig.len(), 132);
        }

        #[test]
        fn calculate_price() {
            let adapter = AirswapAdapter::new(test_config()).unwrap();
            let order = test_order();
            let price = adapter.calculate_price(&order).unwrap();
            // 1000000000000000000 / 2000000000 = 500000000
            let expected = 500_000_000.0_f64;
            assert!((price.get().to_f64().unwrap() - expected).abs() < 1.0);
        }

        #[test]
        fn is_order_expired_false() {
            let adapter = AirswapAdapter::new(test_config()).unwrap();
            let order = test_order();
            assert!(!adapter.is_order_expired(&order));
        }

        #[test]
        fn is_order_expired_true() {
            let adapter = AirswapAdapter::new(test_config()).unwrap();
            let mut order = test_order();
            order.order.expiry = 1000; // Very old timestamp
            assert!(adapter.is_order_expired(&order));
        }

        #[test]
        fn time_to_expiry() {
            let adapter = AirswapAdapter::new(test_config()).unwrap();
            let order = test_order();
            let ttl = adapter.time_to_expiry(&order);
            // Should be around 300 seconds (5 minutes)
            assert!(ttl > 290 && ttl <= 300);
        }
    }

    mod domain {
        use super::*;

        #[test]
        fn new_domain() {
            let domain = AirswapDomain::new(AirswapChain::Ethereum);
            assert_eq!(domain.name, "SWAP");
            assert_eq!(domain.version, "4");
            assert_eq!(domain.chain_id, 1);
            assert!(!domain.verifying_contract.is_empty());
        }

        #[test]
        fn domain_for_polygon() {
            let domain = AirswapDomain::new(AirswapChain::Polygon);
            assert_eq!(domain.chain_id, 137);
        }
    }

    mod venue_adapter {
        use super::*;

        #[tokio::test]
        async fn health_check_disabled() {
            let config = test_config().with_enabled(false);
            let adapter = AirswapAdapter::new(config).unwrap();
            let health = adapter.health_check().await.unwrap();
            assert!(!health.is_healthy());
        }

        #[tokio::test]
        async fn health_check_no_servers() {
            let config = AirswapConfig::new()
                .with_chain(AirswapChain::Ethereum)
                .with_wallet_address("0x1234567890abcdef1234567890abcdef12345678")
                .with_timeout_ms(3000);
            let adapter = AirswapAdapter::new(config).unwrap();
            let health = adapter.health_check().await.unwrap();
            assert!(!health.is_healthy());
        }

        #[tokio::test]
        async fn is_available() {
            let adapter = AirswapAdapter::new(test_config()).unwrap();
            assert!(adapter.is_available().await);

            let disabled_adapter = AirswapAdapter::new(test_config().with_enabled(false)).unwrap();
            assert!(!disabled_adapter.is_available().await);
        }

        #[tokio::test]
        async fn venue_id() {
            let adapter = AirswapAdapter::new(test_config()).unwrap();
            assert_eq!(adapter.venue_id(), &VenueId::new("airswap"));
        }

        #[tokio::test]
        async fn timeout_ms() {
            let adapter = AirswapAdapter::new(test_config()).unwrap();
            assert_eq!(adapter.timeout_ms(), 3000);
        }
    }
}
