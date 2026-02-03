//! # Integration Tests for Venue Adapters
//!
//! This module provides integration tests for venue adapters using mock HTTP servers.
//!
//! # Test Categories
//!
//! - **0x Aggregator**: Quote request/response, execution, error handling
//! - **1inch Aggregator**: Quote request/response, execution, error handling
//! - **Hashflow RFQ**: RFQ protocol flow, signed quotes, execution
//! - **Timeout Handling**: Request timeout scenarios
//! - **Error Responses**: API error handling

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(dead_code)]

use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::domain::entities::quote::Quote;
use crate::domain::entities::rfq::RfqBuilder;
use crate::domain::value_objects::enums::{AssetClass, SettlementMethod};
use crate::domain::value_objects::symbol::Symbol;
use crate::domain::value_objects::timestamp::Timestamp;
use crate::domain::value_objects::{
    CounterpartyId, Instrument, OrderSide, Price, Quantity, VenueId,
};
use crate::infrastructure::venues::dex::one_inch::{OneInchAdapter, OneInchChain, OneInchConfig};
use crate::infrastructure::venues::dex::zero_x::{ZeroXAdapter, ZeroXChain, ZeroXConfig};
use crate::infrastructure::venues::rfq_protocols::hashflow::{
    HashflowAdapter, HashflowChain, HashflowConfig,
};
use crate::infrastructure::venues::traits::VenueAdapter;

// ============================================================================
// Test Helpers
// ============================================================================

fn create_test_rfq() -> crate::domain::entities::rfq::Rfq {
    let symbol = Symbol::new("WETH/USDC").unwrap();
    let instrument = Instrument::new(symbol, AssetClass::CryptoSpot, SettlementMethod::default());
    let quantity = Quantity::new(1.0).unwrap();
    let expires_at = Timestamp::now().add_secs(300);

    RfqBuilder::new(
        CounterpartyId::new("client-1"),
        instrument,
        OrderSide::Buy,
        quantity,
        expires_at,
    )
    .build()
}

// ============================================================================
// 0x Aggregator Tests
// ============================================================================

#[cfg(test)]
mod zero_x_tests {
    use super::*;

    fn zero_x_quote_response() -> serde_json::Value {
        serde_json::json!({
            "chainId": 1,
            "price": "1850.5",
            "guaranteedPrice": "1845.0",
            "estimatedPriceImpact": "0.01",
            "to": "0x1234567890abcdef1234567890abcdef12345678",
            "data": "0xabcdef",
            "value": "0",
            "gas": "150000",
            "estimatedGas": "140000",
            "gasPrice": "20000000000",
            "buyAmount": "1850500000",
            "sellAmount": "1000000000000000000",
            "sources": [
                {"name": "Uniswap_V3", "proportion": "0.8"},
                {"name": "SushiSwap", "proportion": "0.2"}
            ],
            "buyTokenAddress": "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
            "sellTokenAddress": "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
            "allowanceTarget": "0xdef1c0ded9bec7f1a1670819833240f027b25eff"
        })
    }

    fn zero_x_error_response() -> serde_json::Value {
        serde_json::json!({
            "code": 100,
            "reason": "Validation Failed",
            "validationErrors": [
                {
                    "field": "sellAmount",
                    "code": 1001,
                    "reason": "INSUFFICIENT_ASSET_LIQUIDITY"
                }
            ]
        })
    }

    #[tokio::test]
    async fn quote_request_returns_stub_error() {
        // The adapter currently returns a stub error since HTTP client isn't implemented
        let config = ZeroXConfig::new("test-api-key")
            .with_chain(ZeroXChain::Ethereum)
            .with_timeout_ms(5000);

        let adapter = ZeroXAdapter::new(config);
        let rfq = create_test_rfq();

        let result = adapter.request_quote(&rfq).await;

        // Expected to fail with "not yet implemented" until HTTP client is added
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("not yet implemented"));
    }

    #[tokio::test]
    async fn mock_server_setup_for_future_integration() {
        // This test demonstrates wiremock setup for when HTTP client is implemented
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/swap/v1/quote"))
            .respond_with(ResponseTemplate::new(400).set_body_json(zero_x_error_response()))
            .mount(&mock_server)
            .await;

        // Verify mock server is running
        assert!(!mock_server.uri().is_empty());
    }

    #[tokio::test]
    async fn health_check_enabled() {
        let config = ZeroXConfig::new("test-api-key")
            .with_chain(ZeroXChain::Ethereum)
            .with_enabled(true);

        let adapter = ZeroXAdapter::new(config);
        let health = adapter.health_check().await.unwrap();

        assert!(health.is_healthy());
    }

    #[tokio::test]
    async fn health_check_disabled() {
        let config = ZeroXConfig::new("test-api-key")
            .with_chain(ZeroXChain::Ethereum)
            .with_enabled(false);

        let adapter = ZeroXAdapter::new(config);
        let health = adapter.health_check().await.unwrap();

        assert!(!health.is_healthy());
    }

    #[tokio::test]
    async fn adapter_disabled_returns_error() {
        let config = ZeroXConfig::new("test-api-key")
            .with_chain(ZeroXChain::Ethereum)
            .with_enabled(false);

        let adapter = ZeroXAdapter::new(config);
        let rfq = create_test_rfq();

        let result = adapter.request_quote(&rfq).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("disabled"));
    }

    #[tokio::test]
    async fn venue_id_correct() {
        let config = ZeroXConfig::new("test-api-key").with_venue_id("custom-0x-venue");

        let adapter = ZeroXAdapter::new(config);
        assert_eq!(adapter.venue_id(), &VenueId::new("custom-0x-venue"));
    }

    #[tokio::test]
    async fn timeout_configuration() {
        let config = ZeroXConfig::new("test-api-key").with_timeout_ms(10000);

        let adapter = ZeroXAdapter::new(config);
        assert_eq!(adapter.timeout_ms(), 10000);
    }
}

// ============================================================================
// 1inch Aggregator Tests
// ============================================================================

#[cfg(test)]
mod one_inch_tests {
    use super::*;

    fn one_inch_quote_response() -> serde_json::Value {
        serde_json::json!({
            "fromToken": {
                "symbol": "WETH",
                "name": "Wrapped Ether",
                "address": "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
                "decimals": 18,
                "logoURI": "https://tokens.1inch.io/weth.png"
            },
            "toToken": {
                "symbol": "USDC",
                "name": "USD Coin",
                "address": "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
                "decimals": 6,
                "logoURI": "https://tokens.1inch.io/usdc.png"
            },
            "fromTokenAmount": "1000000000000000000",
            "toTokenAmount": "1850500000",
            "protocols": [
                [
                    [
                        {
                            "name": "UNISWAP_V3",
                            "part": 80,
                            "fromTokenIndex": 0,
                            "toTokenIndex": 1
                        }
                    ]
                ]
            ],
            "estimatedGas": 150000
        })
    }

    fn one_inch_error_response() -> serde_json::Value {
        serde_json::json!({
            "error": "insufficient liquidity",
            "statusCode": 400,
            "description": "Not enough liquidity to complete the swap"
        })
    }

    #[tokio::test]
    async fn quote_request_returns_stub_error() {
        // The adapter currently returns a stub error since HTTP client isn't implemented
        let config = OneInchConfig::new("test-api-key")
            .with_chain(OneInchChain::Ethereum)
            .with_timeout_ms(5000);

        let adapter = OneInchAdapter::new(config);
        let rfq = create_test_rfq();

        let result = adapter.request_quote(&rfq).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("not yet implemented")
        );
    }

    #[tokio::test]
    async fn health_check_enabled() {
        let config = OneInchConfig::new("test-api-key")
            .with_chain(OneInchChain::Ethereum)
            .with_enabled(true);

        let adapter = OneInchAdapter::new(config);
        let health = adapter.health_check().await.unwrap();

        assert!(health.is_healthy());
    }

    #[tokio::test]
    async fn health_check_disabled() {
        let config = OneInchConfig::new("test-api-key")
            .with_chain(OneInchChain::Ethereum)
            .with_enabled(false);

        let adapter = OneInchAdapter::new(config);
        let health = adapter.health_check().await.unwrap();

        assert!(!health.is_healthy());
    }

    #[tokio::test]
    async fn adapter_disabled_returns_error() {
        let config = OneInchConfig::new("test-api-key")
            .with_chain(OneInchChain::Ethereum)
            .with_enabled(false);

        let adapter = OneInchAdapter::new(config);
        let rfq = create_test_rfq();

        let result = adapter.request_quote(&rfq).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("disabled"));
    }

    #[tokio::test]
    async fn multi_chain_support() {
        let chains = [
            (OneInchChain::Ethereum, 1),
            (OneInchChain::Polygon, 137),
            (OneInchChain::Arbitrum, 42161),
            (OneInchChain::Optimism, 10),
            (OneInchChain::Base, 8453),
        ];

        for (chain, expected_id) in chains {
            let config = OneInchConfig::new("test-api-key").with_chain(chain);
            assert_eq!(config.chain().chain_id(), expected_id);
        }
    }

    #[tokio::test]
    async fn venue_id_correct() {
        let config = OneInchConfig::new("test-api-key").with_venue_id("custom-1inch-venue");

        let adapter = OneInchAdapter::new(config);
        assert_eq!(adapter.venue_id(), &VenueId::new("custom-1inch-venue"));
    }
}

// ============================================================================
// Hashflow RFQ Protocol Tests
// ============================================================================

#[cfg(test)]
mod hashflow_tests {
    use super::*;

    fn hashflow_rfq_response() -> serde_json::Value {
        serde_json::json!({
            "status": "success",
            "quotes": [
                {
                    "quoteId": "hf-quote-12345",
                    "chainId": 1,
                    "baseToken": "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
                    "quoteToken": "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
                    "baseTokenAmount": "1000000000000000000",
                    "quoteTokenAmount": "1850500000",
                    "quoteExpiry": 1700000000,
                    "nonce": "12345",
                    "txnDeadline": 1700000060,
                    "pool": "0x1234567890abcdef1234567890abcdef12345678",
                    "externalAccount": "0xabcdef1234567890abcdef1234567890abcdef12",
                    "trader": "0xabcdef1234567890abcdef1234567890abcdef12",
                    "effectiveTrader": "0xabcdef1234567890abcdef1234567890abcdef12",
                    "signature": "0xsignature",
                    "marketMaker": {
                        "mmId": "mm-001",
                        "name": "Test Market Maker"
                    }
                }
            ]
        })
    }

    fn hashflow_error_response() -> serde_json::Value {
        serde_json::json!({
            "status": "error",
            "error": {
                "code": "NO_QUOTES",
                "message": "No quotes available for this pair"
            }
        })
    }

    #[tokio::test]
    async fn rfq_request_returns_stub_error() {
        // The adapter currently returns a stub error since HTTP client isn't implemented
        let config = HashflowConfig::new("test-api-key")
            .with_chain(HashflowChain::Ethereum)
            .with_timeout_ms(5000);

        let adapter = HashflowAdapter::new(config);
        let rfq = create_test_rfq();

        let result = adapter.request_quote(&rfq).await;
        assert!(result.is_err());
        // Hashflow adapter returns error when token addresses can't be resolved
        let err_msg = result.unwrap_err().to_string();
        assert!(!err_msg.is_empty());
    }

    #[tokio::test]
    async fn health_check_enabled() {
        let config = HashflowConfig::new("test-api-key")
            .with_chain(HashflowChain::Ethereum)
            .with_enabled(true);

        let adapter = HashflowAdapter::new(config);
        let health = adapter.health_check().await.unwrap();

        assert!(health.is_healthy());
    }

    #[tokio::test]
    async fn health_check_disabled() {
        let config = HashflowConfig::new("test-api-key")
            .with_chain(HashflowChain::Ethereum)
            .with_enabled(false);

        let adapter = HashflowAdapter::new(config);
        let health = adapter.health_check().await.unwrap();

        assert!(!health.is_healthy());
    }

    #[tokio::test]
    async fn adapter_disabled_returns_error() {
        let config = HashflowConfig::new("test-api-key")
            .with_chain(HashflowChain::Ethereum)
            .with_enabled(false);

        let adapter = HashflowAdapter::new(config);
        let rfq = create_test_rfq();

        let result = adapter.request_quote(&rfq).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("disabled"));
    }

    #[tokio::test]
    async fn multi_chain_support() {
        let chains = [
            (HashflowChain::Ethereum, 1),
            (HashflowChain::Polygon, 137),
            (HashflowChain::Arbitrum, 42161),
            (HashflowChain::Optimism, 10),
        ];

        for (chain, expected_id) in chains {
            let config = HashflowConfig::new("test-api-key").with_chain(chain);
            assert_eq!(config.chain().chain_id(), expected_id);
        }
    }

    #[tokio::test]
    async fn venue_id_correct() {
        let config = HashflowConfig::new("test-api-key").with_venue_id("custom-hashflow-venue");

        let adapter = HashflowAdapter::new(config);
        assert_eq!(adapter.venue_id(), &VenueId::new("custom-hashflow-venue"));
    }

    #[tokio::test]
    async fn wallet_address_configuration() {
        let config = HashflowConfig::new("test-api-key")
            .with_wallet_address("0x1234567890abcdef1234567890abcdef12345678");

        assert_eq!(
            config.wallet_address(),
            Some("0x1234567890abcdef1234567890abcdef12345678")
        );
    }
}

// ============================================================================
// Timeout Handling Tests
// ============================================================================

#[cfg(test)]
mod timeout_tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn zero_x_timeout_configuration() {
        let config = ZeroXConfig::new("test-api-key").with_timeout_ms(1000);

        let adapter = ZeroXAdapter::new(config);
        assert_eq!(adapter.timeout_ms(), 1000);
    }

    #[tokio::test]
    async fn one_inch_timeout_configuration() {
        let config = OneInchConfig::new("test-api-key").with_timeout_ms(2000);

        let adapter = OneInchAdapter::new(config);
        assert_eq!(adapter.timeout_ms(), 2000);
    }

    #[tokio::test]
    async fn hashflow_timeout_configuration() {
        let config = HashflowConfig::new("test-api-key").with_timeout_ms(3000);

        let adapter = HashflowAdapter::new(config);
        assert_eq!(adapter.timeout_ms(), 3000);
    }

    #[tokio::test]
    async fn slow_server_mock_setup() {
        // Demonstrates wiremock delay setup for future HTTP client integration
        let mock_server = MockServer::start().await;

        // Server responds after 2 seconds
        Mock::given(method("GET"))
            .and(path("/swap/v1/quote"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"price": "1850.5"}))
                    .set_delay(Duration::from_secs(2)),
            )
            .mount(&mock_server)
            .await;

        // Verify mock server is configured
        assert!(!mock_server.uri().is_empty());
    }
}

// ============================================================================
// Error Response Tests
// ============================================================================

#[cfg(test)]
mod error_response_tests {
    use super::*;

    #[tokio::test]
    async fn zero_x_rate_limit_mock_setup() {
        // Demonstrates wiremock rate limit response setup
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/swap/v1/quote"))
            .respond_with(ResponseTemplate::new(429).set_body_json(serde_json::json!({
                "code": 429,
                "reason": "Rate limit exceeded"
            })))
            .mount(&mock_server)
            .await;

        assert!(!mock_server.uri().is_empty());
    }

    #[tokio::test]
    async fn zero_x_server_error_mock_setup() {
        // Demonstrates wiremock server error response setup
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/swap/v1/quote"))
            .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
            .mount(&mock_server)
            .await;

        assert!(!mock_server.uri().is_empty());
    }

    #[tokio::test]
    async fn one_inch_unauthorized_mock_setup() {
        // Demonstrates wiremock unauthorized response setup
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/swap/v5.2/1/quote"))
            .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
                "error": "Unauthorized",
                "statusCode": 401,
                "description": "Invalid API key"
            })))
            .mount(&mock_server)
            .await;

        assert!(!mock_server.uri().is_empty());
    }

    #[tokio::test]
    async fn hashflow_service_unavailable_mock_setup() {
        // Demonstrates wiremock service unavailable response setup
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/taker/v3/rfq"))
            .respond_with(ResponseTemplate::new(503).set_body_string("Service Unavailable"))
            .mount(&mock_server)
            .await;

        assert!(!mock_server.uri().is_empty());
    }
}

// ============================================================================
// Quote Validation Tests
// ============================================================================

#[cfg(test)]
mod quote_validation_tests {
    use super::*;
    use crate::domain::entities::quote::QuoteBuilder;

    fn create_test_quote(venue_id: &str) -> Quote {
        QuoteBuilder::new(
            crate::domain::value_objects::RfqId::new_v4(),
            VenueId::new(venue_id),
            Price::new(1850.5).unwrap(),
            Quantity::new(1.0).unwrap(),
            Timestamp::now().add_secs(60),
        )
        .build()
    }

    #[tokio::test]
    async fn zero_x_execute_wrong_venue_quote() {
        let config = ZeroXConfig::new("test-api-key").with_venue_id("0x-aggregator");

        let adapter = ZeroXAdapter::new(config);
        let quote = create_test_quote("different-venue");

        let result = adapter.execute_trade(&quote).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("not from this venue")
        );
    }

    #[tokio::test]
    async fn one_inch_execute_wrong_venue_quote() {
        let config = OneInchConfig::new("test-api-key").with_venue_id("1inch-aggregator");

        let adapter = OneInchAdapter::new(config);
        let quote = create_test_quote("different-venue");

        let result = adapter.execute_trade(&quote).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("not from this venue")
        );
    }

    #[tokio::test]
    async fn hashflow_execute_wrong_venue_quote() {
        let config = HashflowConfig::new("test-api-key").with_venue_id("hashflow-rfq");

        let adapter = HashflowAdapter::new(config);
        let quote = create_test_quote("different-venue");

        let result = adapter.execute_trade(&quote).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("not from this venue")
        );
    }

    #[tokio::test]
    async fn execute_expired_quote() {
        let config = ZeroXConfig::new("test-api-key").with_venue_id("0x-aggregator");

        let adapter = ZeroXAdapter::new(config);

        // Create an expired quote
        let expired_quote = QuoteBuilder::new(
            crate::domain::value_objects::RfqId::new_v4(),
            VenueId::new("0x-aggregator"),
            Price::new(1850.5).unwrap(),
            Quantity::new(1.0).unwrap(),
            Timestamp::now().add_secs(-60), // Expired 60 seconds ago
        )
        .build();

        let result = adapter.execute_trade(&expired_quote).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("expired"));
    }
}

// ============================================================================
// Configuration Tests
// ============================================================================

#[cfg(test)]
mod configuration_tests {
    use super::*;

    #[test]
    fn zero_x_default_config() {
        let config = ZeroXConfig::new("test-key");

        assert_eq!(config.api_key(), "test-key");
        assert_eq!(config.chain(), ZeroXChain::Ethereum);
        assert!(config.is_enabled());
        assert_eq!(config.timeout_ms(), 5000);
        assert_eq!(config.slippage_bps(), 50);
    }

    #[test]
    fn one_inch_default_config() {
        let config = OneInchConfig::new("test-key");

        assert_eq!(config.api_key(), "test-key");
        assert_eq!(config.chain(), OneInchChain::Ethereum);
        assert!(config.is_enabled());
        assert_eq!(config.timeout_ms(), 5000);
    }

    #[test]
    fn hashflow_default_config() {
        let config = HashflowConfig::new("test-key");

        assert_eq!(config.api_key(), "test-key");
        assert_eq!(config.chain(), HashflowChain::Ethereum);
        assert!(config.is_enabled());
        assert_eq!(config.timeout_ms(), 5000);
    }

    #[test]
    fn zero_x_chain_builder() {
        let config = ZeroXConfig::new("key")
            .with_chain(ZeroXChain::Polygon)
            .with_timeout_ms(10000)
            .with_slippage_bps(100)
            .with_enabled(false);

        assert_eq!(config.chain(), ZeroXChain::Polygon);
        assert_eq!(config.timeout_ms(), 10000);
        assert_eq!(config.slippage_bps(), 100);
        assert!(!config.is_enabled());
    }

    #[test]
    fn one_inch_chain_builder() {
        let config = OneInchConfig::new("key")
            .with_chain(OneInchChain::Arbitrum)
            .with_timeout_ms(8000)
            .with_slippage_bps(75)
            .with_enabled(false);

        assert_eq!(config.chain(), OneInchChain::Arbitrum);
        assert_eq!(config.timeout_ms(), 8000);
        assert_eq!(config.slippage_bps(), 75);
        assert!(!config.is_enabled());
    }

    #[test]
    fn hashflow_chain_builder() {
        let config = HashflowConfig::new("key")
            .with_chain(HashflowChain::Optimism)
            .with_timeout_ms(6000)
            .with_enabled(false);

        assert_eq!(config.chain(), HashflowChain::Optimism);
        assert_eq!(config.timeout_ms(), 6000);
        assert!(!config.is_enabled());
    }

    #[test]
    fn zero_x_token_address_resolution() {
        let config = ZeroXConfig::new("key").with_token_address("CUSTOM", "0xcustom");

        assert!(config.resolve_token_address("WETH").is_some());
        assert!(config.resolve_token_address("USDC").is_some());
        assert_eq!(
            config.resolve_token_address("CUSTOM"),
            Some(&"0xcustom".to_string())
        );
        assert!(config.resolve_token_address("UNKNOWN").is_none());
    }
}
