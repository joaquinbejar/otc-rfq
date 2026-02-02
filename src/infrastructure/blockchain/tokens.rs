//! # Token Registry
//!
//! Token address mapping across chains.
//!
//! Provides a registry for looking up token addresses by symbol and chain,
//! supporting multi-chain deployments of the same token.

use super::client::ChainId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// Token registry error.
#[derive(Debug, Error)]
pub enum TokenError {
    /// Token not found in registry.
    #[error("token not found: {0}")]
    TokenNotFound(String),

    /// Token not available on specified chain.
    #[error("token '{0}' not available on chain {1}")]
    NotOnChain(String, ChainId),

    /// Invalid token address format.
    #[error("invalid address format: {0}")]
    InvalidAddress(String),
}

/// Result type for token operations.
pub type TokenResult<T> = Result<T, TokenError>;

/// Information about a token.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenInfo {
    /// Token symbol (e.g., "WETH", "USDC").
    pub symbol: String,
    /// Token name (e.g., "Wrapped Ether").
    pub name: String,
    /// Token decimals.
    pub decimals: u8,
    /// Token addresses per chain.
    #[serde(default)]
    pub addresses: HashMap<ChainId, String>,
}

impl TokenInfo {
    /// Creates a new token info.
    #[must_use]
    pub fn new(symbol: impl Into<String>, name: impl Into<String>, decimals: u8) -> Self {
        Self {
            symbol: symbol.into(),
            name: name.into(),
            decimals,
            addresses: HashMap::new(),
        }
    }

    /// Adds an address for a chain.
    #[must_use]
    pub fn with_address(mut self, chain: ChainId, address: impl Into<String>) -> Self {
        self.addresses.insert(chain, address.into());
        self
    }

    /// Gets the address for a specific chain.
    #[must_use]
    pub fn address(&self, chain: ChainId) -> Option<&str> {
        self.addresses.get(&chain).map(String::as_str)
    }

    /// Returns all chains where this token is available.
    #[must_use]
    pub fn available_chains(&self) -> Vec<ChainId> {
        self.addresses.keys().copied().collect()
    }

    /// Returns true if the token is available on the specified chain.
    #[must_use]
    pub fn is_available_on(&self, chain: ChainId) -> bool {
        self.addresses.contains_key(&chain)
    }
}

/// Registry for token address mappings across chains.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenRegistry {
    /// Tokens indexed by symbol.
    #[serde(flatten)]
    tokens: HashMap<String, TokenInfo>,
}

impl TokenRegistry {
    /// Creates a new empty token registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a registry with common tokens pre-populated.
    #[must_use]
    pub fn with_common_tokens() -> Self {
        let mut registry = Self::new();

        // WETH
        registry.register(
            TokenInfo::new("WETH", "Wrapped Ether", 18)
                .with_address(
                    ChainId::Ethereum,
                    "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
                )
                .with_address(
                    ChainId::Arbitrum,
                    "0x82aF49447D8a07e3bd95BD0d56f35241523fBab1",
                )
                .with_address(
                    ChainId::Polygon,
                    "0x7ceB23fD6bC0adD59E62ac25578270cFf1b9f619",
                )
                .with_address(
                    ChainId::Optimism,
                    "0x4200000000000000000000000000000000000006",
                )
                .with_address(ChainId::Base, "0x4200000000000000000000000000000000000006"),
        );

        // USDC
        registry.register(
            TokenInfo::new("USDC", "USD Coin", 6)
                .with_address(
                    ChainId::Ethereum,
                    "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
                )
                .with_address(
                    ChainId::Arbitrum,
                    "0xaf88d065e77c8cC2239327C5EDb3A432268e5831",
                )
                .with_address(
                    ChainId::Polygon,
                    "0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359",
                )
                .with_address(
                    ChainId::Optimism,
                    "0x0b2C639c533813f4Aa9D7837CAf62653d097Ff85",
                )
                .with_address(ChainId::Base, "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913"),
        );

        // USDT
        registry.register(
            TokenInfo::new("USDT", "Tether USD", 6)
                .with_address(
                    ChainId::Ethereum,
                    "0xdAC17F958D2ee523a2206206994597C13D831ec7",
                )
                .with_address(
                    ChainId::Arbitrum,
                    "0xFd086bC7CD5C481DCC9C85ebE478A1C0b69FCbb9",
                )
                .with_address(
                    ChainId::Polygon,
                    "0xc2132D05D31c914a87C6611C10748AEb04B58e8F",
                )
                .with_address(
                    ChainId::Optimism,
                    "0x94b008aA00579c1307B0EF2c499aD98a8ce58e58",
                ),
        );

        // DAI
        registry.register(
            TokenInfo::new("DAI", "Dai Stablecoin", 18)
                .with_address(
                    ChainId::Ethereum,
                    "0x6B175474E89094C44Da98b954EeddeBC35e4D1",
                )
                .with_address(
                    ChainId::Arbitrum,
                    "0xDA10009cBd5D07dd0CeCc66161FC93D7c9000da1",
                )
                .with_address(
                    ChainId::Polygon,
                    "0x8f3Cf7ad23Cd3CaDbD9735AFf958023239c6A063",
                )
                .with_address(
                    ChainId::Optimism,
                    "0xDA10009cBd5D07dd0CeCc66161FC93D7c9000da1",
                )
                .with_address(ChainId::Base, "0x50c5725949A6F0c72E6C4a641F24049A917DB0Cb"),
        );

        // WBTC
        registry.register(
            TokenInfo::new("WBTC", "Wrapped Bitcoin", 8)
                .with_address(
                    ChainId::Ethereum,
                    "0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599",
                )
                .with_address(
                    ChainId::Arbitrum,
                    "0x2f2a2543B76A4166549F7aaB2e75Bef0aefC5B0f",
                )
                .with_address(
                    ChainId::Polygon,
                    "0x1BFD67037B42Cf73acF2047067bd4F2C47D9BfD6",
                )
                .with_address(
                    ChainId::Optimism,
                    "0x68f180fcCe6836688e9084f035309E29Bf0A2095",
                ),
        );

        registry
    }

    /// Registers a token in the registry.
    pub fn register(&mut self, token: TokenInfo) {
        self.tokens.insert(token.symbol.clone(), token);
    }

    /// Gets token info by symbol.
    #[must_use]
    pub fn get(&self, symbol: &str) -> Option<&TokenInfo> {
        self.tokens.get(symbol)
    }

    /// Gets the address of a token on a specific chain.
    ///
    /// # Errors
    ///
    /// Returns an error if the token is not found or not available on the chain.
    pub fn get_address(&self, symbol: &str, chain: ChainId) -> TokenResult<&str> {
        let token = self
            .tokens
            .get(symbol)
            .ok_or_else(|| TokenError::TokenNotFound(symbol.to_string()))?;

        token
            .address(chain)
            .ok_or_else(|| TokenError::NotOnChain(symbol.to_string(), chain))
    }

    /// Gets token info by address on a specific chain.
    #[must_use]
    pub fn get_by_address(&self, address: &str, chain: ChainId) -> Option<&TokenInfo> {
        let normalized = address.to_lowercase();
        self.tokens.values().find(|t| {
            t.address(chain)
                .map(|a| a.to_lowercase() == normalized)
                .unwrap_or(false)
        })
    }

    /// Returns all registered token symbols.
    #[must_use]
    pub fn symbols(&self) -> Vec<&str> {
        self.tokens.keys().map(String::as_str).collect()
    }

    /// Returns the number of registered tokens.
    #[must_use]
    pub fn len(&self) -> usize {
        self.tokens.len()
    }

    /// Returns true if no tokens are registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tokens.is_empty()
    }

    /// Returns all tokens available on a specific chain.
    #[must_use]
    pub fn tokens_on_chain(&self, chain: ChainId) -> Vec<&TokenInfo> {
        self.tokens
            .values()
            .filter(|t| t.is_available_on(chain))
            .collect()
    }
}

/// Validates an Ethereum address format.
///
/// # Arguments
///
/// * `address` - The address to validate
///
/// # Returns
///
/// True if the address is a valid Ethereum address format.
#[must_use]
pub fn is_valid_address(address: &str) -> bool {
    if !address.starts_with("0x") {
        return false;
    }

    let hex_part = &address[2..];
    hex_part.len() == 40 && hex_part.chars().all(|c| c.is_ascii_hexdigit())
}

/// Normalizes an Ethereum address to lowercase with 0x prefix.
///
/// # Arguments
///
/// * `address` - The address to normalize
///
/// # Errors
///
/// Returns an error if the address format is invalid.
pub fn normalize_address(address: &str) -> TokenResult<String> {
    if !is_valid_address(address) {
        return Err(TokenError::InvalidAddress(address.to_string()));
    }

    Ok(address.to_lowercase())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn token_info_new() {
        let token = TokenInfo::new("WETH", "Wrapped Ether", 18);
        assert_eq!(token.symbol, "WETH");
        assert_eq!(token.name, "Wrapped Ether");
        assert_eq!(token.decimals, 18);
        assert!(token.addresses.is_empty());
    }

    #[test]
    fn token_info_with_address() {
        let token = TokenInfo::new("WETH", "Wrapped Ether", 18).with_address(
            ChainId::Ethereum,
            "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
        );

        assert_eq!(
            token.address(ChainId::Ethereum),
            Some("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2")
        );
        assert!(token.address(ChainId::Polygon).is_none());
    }

    #[test]
    fn token_info_available_chains() {
        let token = TokenInfo::new("WETH", "Wrapped Ether", 18)
            .with_address(ChainId::Ethereum, "0x1")
            .with_address(ChainId::Polygon, "0x2");

        let chains = token.available_chains();
        assert_eq!(chains.len(), 2);
        assert!(token.is_available_on(ChainId::Ethereum));
        assert!(token.is_available_on(ChainId::Polygon));
        assert!(!token.is_available_on(ChainId::Arbitrum));
    }

    #[test]
    fn token_registry_new() {
        let registry = TokenRegistry::new();
        assert!(registry.is_empty());
    }

    #[test]
    fn token_registry_with_common_tokens() {
        let registry = TokenRegistry::with_common_tokens();
        assert!(!registry.is_empty());
        assert!(registry.get("WETH").is_some());
        assert!(registry.get("USDC").is_some());
        assert!(registry.get("USDT").is_some());
        assert!(registry.get("DAI").is_some());
        assert!(registry.get("WBTC").is_some());
    }

    #[test]
    fn token_registry_get_address() {
        let registry = TokenRegistry::with_common_tokens();

        let address = registry.get_address("WETH", ChainId::Ethereum).unwrap();
        assert_eq!(address, "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");
    }

    #[test]
    fn token_registry_get_address_not_found() {
        let registry = TokenRegistry::with_common_tokens();

        let result = registry.get_address("UNKNOWN", ChainId::Ethereum);
        assert!(matches!(result, Err(TokenError::TokenNotFound(_))));
    }

    #[test]
    fn token_registry_get_address_not_on_chain() {
        let mut registry = TokenRegistry::new();
        registry.register(TokenInfo::new("TEST", "Test Token", 18).with_address(
            ChainId::Ethereum,
            "0x1234567890123456789012345678901234567890",
        ));

        let result = registry.get_address("TEST", ChainId::Polygon);
        assert!(matches!(result, Err(TokenError::NotOnChain(_, _))));
    }

    #[test]
    fn token_registry_get_by_address() {
        let registry = TokenRegistry::with_common_tokens();

        let token = registry
            .get_by_address(
                "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
                ChainId::Ethereum,
            )
            .unwrap();
        assert_eq!(token.symbol, "WETH");
    }

    #[test]
    fn token_registry_get_by_address_case_insensitive() {
        let registry = TokenRegistry::with_common_tokens();

        let token = registry
            .get_by_address(
                "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                ChainId::Ethereum,
            )
            .unwrap();
        assert_eq!(token.symbol, "WETH");
    }

    #[test]
    fn token_registry_tokens_on_chain() {
        let registry = TokenRegistry::with_common_tokens();

        let tokens = registry.tokens_on_chain(ChainId::Ethereum);
        assert!(!tokens.is_empty());
        assert!(tokens.iter().any(|t| t.symbol == "WETH"));
    }

    #[test]
    fn is_valid_address_valid() {
        assert!(is_valid_address(
            "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"
        ));
        assert!(is_valid_address(
            "0x0000000000000000000000000000000000000000"
        ));
    }

    #[test]
    fn is_valid_address_invalid() {
        assert!(!is_valid_address(
            "C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"
        )); // no 0x
        assert!(!is_valid_address(
            "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc"
        )); // too short
        assert!(!is_valid_address(
            "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2X"
        )); // too long
        assert!(!is_valid_address(
            "0xGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGG"
        )); // invalid chars
    }

    #[test]
    fn normalize_address_valid() {
        let result = normalize_address("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2").unwrap();
        assert_eq!(result, "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2");
    }

    #[test]
    fn normalize_address_invalid() {
        let result = normalize_address("invalid");
        assert!(result.is_err());
    }

    #[test]
    fn token_error_display() {
        let err = TokenError::TokenNotFound("UNKNOWN".to_string());
        assert_eq!(err.to_string(), "token not found: UNKNOWN");

        let err = TokenError::NotOnChain("WETH".to_string(), ChainId::Polygon);
        assert!(err.to_string().contains("WETH"));
    }
}
