//! # Chain Configuration
//!
//! Configuration for blockchain networks and multi-chain support.
//!
//! Provides chain-specific configuration including RPC URLs, gas strategies,
//! and network parameters for Ethereum and L2 networks.

use super::client::ChainId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use thiserror::Error;

/// Gas price strategy for a chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum GasPriceStrategy {
    /// Legacy gas pricing (single gas price).
    Legacy,
    /// EIP-1559 dynamic fee pricing.
    #[default]
    Eip1559,
}

impl fmt::Display for GasPriceStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Legacy => write!(f, "legacy"),
            Self::Eip1559 => write!(f, "eip1559"),
        }
    }
}

/// Configuration error.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// Chain not found in configuration.
    #[error("chain not found: {0}")]
    ChainNotFound(String),

    /// Invalid configuration value.
    #[error("invalid configuration: {0}")]
    Invalid(String),

    /// Missing required field.
    #[error("missing required field: {0}")]
    MissingField(String),

    /// Environment variable not set.
    #[error("environment variable not set: {0}")]
    EnvVarNotSet(String),

    /// TOML parsing error.
    #[error("TOML parse error: {0}")]
    TomlParse(String),
}

/// Result type for configuration operations.
pub type ConfigResult<T> = Result<T, ConfigError>;

/// Configuration for a single blockchain network.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainConfig {
    /// Chain identifier.
    pub chain_id: ChainId,
    /// RPC endpoint URLs (primary first, then backups).
    pub rpc_urls: Vec<String>,
    /// Average block time in milliseconds.
    pub block_time_ms: u64,
    /// Gas price strategy for this chain.
    pub gas_price_strategy: GasPriceStrategy,
    /// Required confirmations for transaction finality.
    #[serde(default = "default_confirmations")]
    pub confirmations: u64,
    /// Gas buffer percentage for estimation.
    #[serde(default = "default_gas_buffer")]
    pub gas_buffer_percent: u64,
}

fn default_confirmations() -> u64 {
    1
}

fn default_gas_buffer() -> u64 {
    20
}

impl ChainConfig {
    /// Creates a new chain configuration.
    ///
    /// # Arguments
    ///
    /// * `chain_id` - The chain identifier
    /// * `rpc_urls` - RPC endpoint URLs
    #[must_use]
    pub fn new(chain_id: ChainId, rpc_urls: Vec<String>) -> Self {
        Self {
            chain_id,
            rpc_urls,
            block_time_ms: chain_id.block_time_ms(),
            gas_price_strategy: if chain_id.supports_eip1559() {
                GasPriceStrategy::Eip1559
            } else {
                GasPriceStrategy::Legacy
            },
            confirmations: default_confirmations(),
            gas_buffer_percent: default_gas_buffer(),
        }
    }

    /// Returns the primary RPC URL.
    #[must_use]
    pub fn primary_rpc(&self) -> Option<&str> {
        self.rpc_urls.first().map(String::as_str)
    }

    /// Returns backup RPC URLs.
    #[must_use]
    pub fn backup_rpcs(&self) -> Vec<String> {
        self.rpc_urls.iter().skip(1).cloned().collect()
    }

    /// Validates the configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration is invalid.
    pub fn validate(&self) -> ConfigResult<()> {
        if self.rpc_urls.is_empty() {
            return Err(ConfigError::MissingField("rpc_urls".to_string()));
        }

        if self.block_time_ms == 0 {
            return Err(ConfigError::Invalid(
                "block_time_ms must be greater than 0".to_string(),
            ));
        }

        Ok(())
    }
}

/// Builder for chain configuration.
#[derive(Debug, Default)]
pub struct ChainConfigBuilder {
    chain_id: Option<ChainId>,
    rpc_urls: Vec<String>,
    block_time_ms: Option<u64>,
    gas_price_strategy: Option<GasPriceStrategy>,
    confirmations: Option<u64>,
    gas_buffer_percent: Option<u64>,
}

impl ChainConfigBuilder {
    /// Creates a new builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the chain ID.
    #[must_use]
    pub fn chain_id(mut self, chain_id: ChainId) -> Self {
        self.chain_id = Some(chain_id);
        self
    }

    /// Adds an RPC URL.
    #[must_use]
    pub fn rpc_url(mut self, url: impl Into<String>) -> Self {
        self.rpc_urls.push(url.into());
        self
    }

    /// Sets multiple RPC URLs.
    #[must_use]
    pub fn rpc_urls(mut self, urls: Vec<String>) -> Self {
        self.rpc_urls = urls;
        self
    }

    /// Sets the block time in milliseconds.
    #[must_use]
    pub fn block_time_ms(mut self, ms: u64) -> Self {
        self.block_time_ms = Some(ms);
        self
    }

    /// Sets the gas price strategy.
    #[must_use]
    pub fn gas_price_strategy(mut self, strategy: GasPriceStrategy) -> Self {
        self.gas_price_strategy = Some(strategy);
        self
    }

    /// Sets the required confirmations.
    #[must_use]
    pub fn confirmations(mut self, confirmations: u64) -> Self {
        self.confirmations = Some(confirmations);
        self
    }

    /// Sets the gas buffer percentage.
    #[must_use]
    pub fn gas_buffer_percent(mut self, percent: u64) -> Self {
        self.gas_buffer_percent = Some(percent);
        self
    }

    /// Builds the chain configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if required fields are missing.
    pub fn build(self) -> ConfigResult<ChainConfig> {
        let chain_id = self
            .chain_id
            .ok_or_else(|| ConfigError::MissingField("chain_id".to_string()))?;

        if self.rpc_urls.is_empty() {
            return Err(ConfigError::MissingField("rpc_urls".to_string()));
        }

        Ok(ChainConfig {
            chain_id,
            rpc_urls: self.rpc_urls,
            block_time_ms: self
                .block_time_ms
                .unwrap_or_else(|| chain_id.block_time_ms()),
            gas_price_strategy: self.gas_price_strategy.unwrap_or_else(|| {
                if chain_id.supports_eip1559() {
                    GasPriceStrategy::Eip1559
                } else {
                    GasPriceStrategy::Legacy
                }
            }),
            confirmations: self.confirmations.unwrap_or_else(default_confirmations),
            gas_buffer_percent: self.gas_buffer_percent.unwrap_or_else(default_gas_buffer),
        })
    }
}

/// Multi-chain configuration container.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChainsConfig {
    /// Chain configurations indexed by chain name.
    #[serde(flatten)]
    pub chains: HashMap<String, ChainConfig>,
}

impl ChainsConfig {
    /// Creates a new empty chains configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a chain configuration.
    pub fn add_chain(&mut self, name: impl Into<String>, config: ChainConfig) {
        self.chains.insert(name.into(), config);
    }

    /// Gets a chain configuration by name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&ChainConfig> {
        self.chains.get(name)
    }

    /// Gets a chain configuration by chain ID.
    #[must_use]
    pub fn get_by_chain_id(&self, chain_id: ChainId) -> Option<&ChainConfig> {
        self.chains.values().find(|c| c.chain_id == chain_id)
    }

    /// Returns all chain names.
    #[must_use]
    pub fn chain_names(&self) -> Vec<&str> {
        self.chains.keys().map(String::as_str).collect()
    }

    /// Returns the number of configured chains.
    #[must_use]
    pub fn len(&self) -> usize {
        self.chains.len()
    }

    /// Returns true if no chains are configured.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.chains.is_empty()
    }

    /// Validates all chain configurations.
    ///
    /// # Errors
    ///
    /// Returns an error if any configuration is invalid.
    pub fn validate(&self) -> ConfigResult<()> {
        for (name, config) in &self.chains {
            config
                .validate()
                .map_err(|e| ConfigError::Invalid(format!("chain '{}': {}", name, e)))?;
        }
        Ok(())
    }
}

/// Substitutes environment variables in a string.
///
/// Replaces `${VAR_NAME}` patterns with the corresponding environment variable value.
///
/// # Arguments
///
/// * `input` - The input string with potential variable references
///
/// # Errors
///
/// Returns an error if a referenced environment variable is not set.
pub fn substitute_env_vars(input: &str) -> ConfigResult<String> {
    let mut result = input.to_string();
    let mut start = 0;

    while let Some(var_start) = result[start..].find("${") {
        let abs_start = start + var_start;
        if let Some(var_end) = result[abs_start..].find('}') {
            let abs_end = abs_start + var_end;
            let var_name = &result[abs_start + 2..abs_end];

            let var_value = std::env::var(var_name)
                .map_err(|_| ConfigError::EnvVarNotSet(var_name.to_string()))?;

            result.replace_range(abs_start..abs_end + 1, &var_value);
            start = abs_start + var_value.len();
        } else {
            break;
        }
    }

    Ok(result)
}

/// Parses a TOML configuration string into chains configuration.
///
/// # Arguments
///
/// * `toml_str` - The TOML configuration string
///
/// # Errors
///
/// Returns an error if parsing fails.
pub fn parse_chains_config(toml_str: &str) -> ConfigResult<ChainsConfig> {
    toml::from_str(toml_str).map_err(|e| ConfigError::TomlParse(e.to_string()))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn gas_price_strategy_display() {
        assert_eq!(GasPriceStrategy::Legacy.to_string(), "legacy");
        assert_eq!(GasPriceStrategy::Eip1559.to_string(), "eip1559");
    }

    #[test]
    fn chain_config_new() {
        let config = ChainConfig::new(
            ChainId::Ethereum,
            vec!["https://eth.example.com".to_string()],
        );

        assert_eq!(config.chain_id, ChainId::Ethereum);
        assert_eq!(config.rpc_urls.len(), 1);
        assert_eq!(config.block_time_ms, 12000);
        assert_eq!(config.gas_price_strategy, GasPriceStrategy::Eip1559);
        assert_eq!(config.confirmations, 1);
        assert_eq!(config.gas_buffer_percent, 20);
    }

    #[test]
    fn chain_config_arbitrum_uses_legacy() {
        let config = ChainConfig::new(
            ChainId::Arbitrum,
            vec!["https://arb.example.com".to_string()],
        );

        assert_eq!(config.gas_price_strategy, GasPriceStrategy::Legacy);
    }

    #[test]
    fn chain_config_primary_and_backup_rpcs() {
        let config = ChainConfig::new(
            ChainId::Ethereum,
            vec![
                "https://primary.example.com".to_string(),
                "https://backup1.example.com".to_string(),
                "https://backup2.example.com".to_string(),
            ],
        );

        assert_eq!(config.primary_rpc(), Some("https://primary.example.com"));
        assert_eq!(config.backup_rpcs().len(), 2);
    }

    #[test]
    fn chain_config_validate_success() {
        let config = ChainConfig::new(
            ChainId::Ethereum,
            vec!["https://eth.example.com".to_string()],
        );

        assert!(config.validate().is_ok());
    }

    #[test]
    fn chain_config_validate_empty_rpc() {
        let config = ChainConfig {
            chain_id: ChainId::Ethereum,
            rpc_urls: vec![],
            block_time_ms: 12000,
            gas_price_strategy: GasPriceStrategy::Eip1559,
            confirmations: 1,
            gas_buffer_percent: 20,
        };

        assert!(config.validate().is_err());
    }

    #[test]
    fn chain_config_builder() {
        let config = ChainConfigBuilder::new()
            .chain_id(ChainId::Polygon)
            .rpc_url("https://polygon.example.com")
            .confirmations(5)
            .gas_buffer_percent(25)
            .build()
            .unwrap();

        assert_eq!(config.chain_id, ChainId::Polygon);
        assert_eq!(config.confirmations, 5);
        assert_eq!(config.gas_buffer_percent, 25);
    }

    #[test]
    fn chain_config_builder_missing_chain_id() {
        let result = ChainConfigBuilder::new()
            .rpc_url("https://example.com")
            .build();

        assert!(result.is_err());
    }

    #[test]
    fn chains_config_add_and_get() {
        let mut config = ChainsConfig::new();
        config.add_chain(
            "ethereum",
            ChainConfig::new(
                ChainId::Ethereum,
                vec!["https://eth.example.com".to_string()],
            ),
        );

        assert_eq!(config.len(), 1);
        assert!(config.get("ethereum").is_some());
        assert!(config.get("polygon").is_none());
    }

    #[test]
    fn chains_config_get_by_chain_id() {
        let mut config = ChainsConfig::new();
        config.add_chain(
            "ethereum",
            ChainConfig::new(
                ChainId::Ethereum,
                vec!["https://eth.example.com".to_string()],
            ),
        );

        assert!(config.get_by_chain_id(ChainId::Ethereum).is_some());
        assert!(config.get_by_chain_id(ChainId::Polygon).is_none());
    }

    #[test]
    fn substitute_env_vars_no_vars() {
        let result = substitute_env_vars("https://example.com").unwrap();
        assert_eq!(result, "https://example.com");
    }

    #[test]
    fn substitute_env_vars_with_var() {
        // Use HOME which is always set on Unix systems
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        let result = substitute_env_vars("prefix/${HOME}/suffix").unwrap();
        assert_eq!(result, format!("prefix/{}/suffix", home));
    }

    #[test]
    fn substitute_env_vars_missing_var() {
        // Use a var name that is extremely unlikely to exist
        let result = substitute_env_vars("https://api.example.com/${__NONEXISTENT_VAR_12345678__}");
        assert!(result.is_err());
    }

    #[test]
    fn config_error_display() {
        let err = ConfigError::ChainNotFound("unknown".to_string());
        assert_eq!(err.to_string(), "chain not found: unknown");
    }
}
