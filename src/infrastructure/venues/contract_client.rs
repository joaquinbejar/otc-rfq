//! # Contract Client
//!
//! Shared utilities for ethers-rs contract interactions.
//!
//! This module provides a wrapper around ethers-rs for making
//! smart contract calls to on-chain DEX protocols.
//!
//! # Features
//!
//! - Provider management with configurable RPC endpoints
//! - Contract call wrappers with error handling
//! - Gas estimation utilities
//! - Transaction building and signing
//!
//! # Examples
//!
//! ```ignore
//! use otc_rfq::infrastructure::venues::ContractClient;
//!
//! let client = ContractClient::new("https://eth-mainnet.g.alchemy.com/v2/...", 10000)?;
//! let block = client.get_block_number().await?;
//! ```

use crate::infrastructure::venues::error::{VenueError, VenueResult};
use ethers::prelude::*;
use std::sync::Arc;
use std::time::Duration;

/// HTTP provider type alias.
pub type HttpProvider = Provider<Http>;

/// Contract client for on-chain interactions.
///
/// Wraps an ethers-rs provider for making contract calls.
#[derive(Clone)]
pub struct ContractClient {
    /// The ethers provider.
    provider: Arc<HttpProvider>,
    /// RPC URL for reference.
    rpc_url: String,
    /// Timeout in milliseconds.
    timeout_ms: u64,
}

impl ContractClient {
    /// Creates a new contract client.
    ///
    /// # Arguments
    ///
    /// * `rpc_url` - The RPC endpoint URL.
    /// * `timeout_ms` - Request timeout in milliseconds.
    ///
    /// # Errors
    ///
    /// Returns `VenueError::Connection` if the provider cannot be created.
    pub fn new(rpc_url: impl Into<String>, timeout_ms: u64) -> VenueResult<Self> {
        let rpc_url = rpc_url.into();
        let provider = Provider::<Http>::try_from(&rpc_url)
            .map_err(|e| VenueError::connection(format!("Failed to create provider: {}", e)))?
            .interval(Duration::from_millis(100));

        Ok(Self {
            provider: Arc::new(provider),
            rpc_url,
            timeout_ms,
        })
    }

    /// Returns the provider.
    #[inline]
    #[must_use]
    pub fn provider(&self) -> Arc<HttpProvider> {
        Arc::clone(&self.provider)
    }

    /// Returns the RPC URL.
    #[inline]
    #[must_use]
    pub fn rpc_url(&self) -> &str {
        &self.rpc_url
    }

    /// Returns the timeout in milliseconds.
    #[inline]
    #[must_use]
    pub fn timeout_ms(&self) -> u64 {
        self.timeout_ms
    }

    /// Gets the current block number.
    ///
    /// # Errors
    ///
    /// Returns `VenueError::Connection` if the RPC call fails.
    pub async fn get_block_number(&self) -> VenueResult<u64> {
        let block =
            self.provider.get_block_number().await.map_err(|e| {
                VenueError::connection(format!("Failed to get block number: {}", e))
            })?;

        Ok(block.as_u64())
    }

    /// Gets the chain ID.
    ///
    /// # Errors
    ///
    /// Returns `VenueError::Connection` if the RPC call fails.
    pub async fn get_chain_id(&self) -> VenueResult<u64> {
        let chain_id = self
            .provider
            .get_chainid()
            .await
            .map_err(|e| VenueError::connection(format!("Failed to get chain ID: {}", e)))?;

        Ok(chain_id.as_u64())
    }

    /// Checks if the RPC connection is healthy.
    ///
    /// Returns true if a block number can be fetched.
    pub async fn is_healthy(&self) -> bool {
        self.get_block_number().await.is_ok()
    }

    /// Performs a health check and returns latency.
    ///
    /// # Errors
    ///
    /// Returns `VenueError::Connection` if the RPC call fails.
    pub async fn health_check_with_latency(&self) -> VenueResult<u64> {
        let start = std::time::Instant::now();
        self.get_block_number().await?;
        Ok(start.elapsed().as_millis() as u64)
    }

    /// Calls a contract view function.
    ///
    /// # Arguments
    ///
    /// * `contract_address` - The contract address.
    /// * `calldata` - The encoded function call data.
    ///
    /// # Errors
    ///
    /// Returns `VenueError::ProtocolError` if the call fails.
    pub async fn call(&self, contract_address: Address, calldata: Bytes) -> VenueResult<Bytes> {
        let tx = TransactionRequest::new()
            .to(contract_address)
            .data(calldata);

        let result = self
            .provider
            .call(&tx.into(), None)
            .await
            .map_err(|e| VenueError::protocol_error(format!("Contract call failed: {}", e)))?;

        Ok(result)
    }

    /// Estimates gas for a transaction.
    ///
    /// # Arguments
    ///
    /// * `from` - The sender address.
    /// * `to` - The recipient/contract address.
    /// * `calldata` - The encoded function call data.
    /// * `value` - The ETH value to send.
    ///
    /// # Errors
    ///
    /// Returns `VenueError::ProtocolError` if gas estimation fails.
    pub async fn estimate_gas(
        &self,
        from: Address,
        to: Address,
        calldata: Bytes,
        value: U256,
    ) -> VenueResult<U256> {
        let tx = TransactionRequest::new()
            .from(from)
            .to(to)
            .data(calldata)
            .value(value);

        let gas = self
            .provider
            .estimate_gas(&tx.into(), None)
            .await
            .map_err(|e| VenueError::protocol_error(format!("Gas estimation failed: {}", e)))?;

        Ok(gas)
    }

    /// Gets the current gas price.
    ///
    /// # Errors
    ///
    /// Returns `VenueError::Connection` if the RPC call fails.
    pub async fn get_gas_price(&self) -> VenueResult<U256> {
        let gas_price = self
            .provider
            .get_gas_price()
            .await
            .map_err(|e| VenueError::connection(format!("Failed to get gas price: {}", e)))?;

        Ok(gas_price)
    }

    /// Parses an address string.
    ///
    /// # Errors
    ///
    /// Returns `VenueError::InvalidRequest` if the address is invalid.
    pub fn parse_address(address: &str) -> VenueResult<Address> {
        address
            .parse()
            .map_err(|_| VenueError::invalid_request(format!("Invalid address: {}", address)))
    }

    /// Parses a U256 from a decimal string.
    ///
    /// # Errors
    ///
    /// Returns `VenueError::InvalidRequest` if the value is invalid.
    pub fn parse_u256(value: &str) -> VenueResult<U256> {
        U256::from_dec_str(value)
            .map_err(|_| VenueError::invalid_request(format!("Invalid U256 value: {}", value)))
    }
}

impl std::fmt::Debug for ContractClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContractClient")
            .field("rpc_url", &self.rpc_url)
            .field("timeout_ms", &self.timeout_ms)
            .finish()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn parse_address_valid() {
        let addr = ContractClient::parse_address("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");
        assert!(addr.is_ok());
    }

    #[test]
    fn parse_address_invalid() {
        let addr = ContractClient::parse_address("invalid");
        assert!(addr.is_err());
    }

    #[test]
    fn parse_u256_valid() {
        let value = ContractClient::parse_u256("1000000000000000000");
        assert!(value.is_ok());
        assert_eq!(value.unwrap(), U256::from(1_000_000_000_000_000_000u64));
    }

    #[test]
    fn parse_u256_invalid() {
        let value = ContractClient::parse_u256("not_a_number");
        assert!(value.is_err());
    }

    #[test]
    fn debug_format() {
        let client = ContractClient::new("https://eth-mainnet.example.com", 5000).unwrap();
        let debug = format!("{:?}", client);
        assert!(debug.contains("ContractClient"));
        assert!(debug.contains("eth-mainnet"));
    }
}
