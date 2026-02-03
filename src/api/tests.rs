//! # API Integration Tests
//!
//! Integration tests for REST API endpoints, authentication middleware,
//! rate limiting, and WebSocket connections.
//!
//! # Test Categories
//!
//! - **Authentication**: JWT validation, API key auth, claims handling
//! - **Rate Limiting**: Tier-based limits, header verification
//! - **WebSocket**: Configuration, state management
//! - **Error Responses**: Format verification for all error types
//! - **Pagination**: Offset/limit calculations

#![allow(clippy::unwrap_used)]

use std::time::{SystemTime, UNIX_EPOCH};

use jsonwebtoken::{EncodingKey, Header, encode};
use serde_json::json;

use crate::api::middleware::auth::{AuthConfig, Claims};
use crate::api::middleware::rate_limit::{
    ClientTier, InMemoryRateLimiter, RateLimitConfig, RateLimitType,
};
use crate::api::rest::handlers::{ErrorResponse, PaginationMeta, PaginationParams};
use crate::api::websocket::handlers::{WebSocketConfig, WebSocketState};

// ============================================================================
// Test Helpers
// ============================================================================

/// Creates a valid JWT token for testing.
fn create_test_token(secret: &str, exp_offset_secs: i64) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let exp = if exp_offset_secs >= 0 {
        now + exp_offset_secs as u64
    } else {
        now.saturating_sub((-exp_offset_secs) as u64)
    };

    let claims = Claims::new("test-user", exp, now)
        .with_client_id("test-client")
        .with_roles(vec!["trader".to_string()]);

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .unwrap()
}

// ============================================================================
// Authentication Middleware Tests
// ============================================================================

#[test]
fn auth_config_builder() {
    let config = AuthConfig::new("test-secret")
        .with_issuer("test-issuer")
        .with_audience("test-audience")
        .with_api_keys(vec!["key1".to_string(), "key2".to_string()]);

    assert_eq!(config.secret, "test-secret");
    assert_eq!(config.issuer, Some("test-issuer".to_string()));
    assert_eq!(config.audience, Some("test-audience".to_string()));
    assert!(config.allow_api_keys);
    assert!(config.api_keys.contains("key1"));
    assert!(config.api_keys.contains("key2"));
}

#[test]
fn auth_config_optional() {
    let config = AuthConfig::new("secret").optional();
    assert!(!config.required);
}

#[test]
fn claims_creation_and_validation() {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let claims = Claims::new("user-123", now + 3600, now)
        .with_issuer("test-issuer")
        .with_audience("test-audience")
        .with_roles(vec!["admin".to_string(), "trader".to_string()])
        .with_permissions(vec!["read".to_string(), "write".to_string()])
        .with_client_id("client-456")
        .with_org_id("org-789");

    assert_eq!(claims.sub, "user-123");
    assert_eq!(claims.iss, Some("test-issuer".to_string()));
    assert_eq!(claims.aud, Some("test-audience".to_string()));
    assert!(claims.has_role("admin"));
    assert!(claims.has_role("trader"));
    assert!(!claims.has_role("viewer"));
    assert!(claims.has_permission("read"));
    assert!(claims.has_permission("write"));
    assert!(!claims.has_permission("delete"));
    assert_eq!(claims.client_id, Some("client-456".to_string()));
    assert_eq!(claims.org_id, Some("org-789".to_string()));
    assert!(!claims.is_expired());
}

#[test]
fn claims_expired_token() {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let claims = Claims::new("user-123", now - 3600, now - 7200);
    assert!(claims.is_expired());
}

#[test]
fn claims_not_expired() {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let claims = Claims::new("user-123", now + 3600, now);
    assert!(!claims.is_expired());
}

#[test]
fn jwt_token_encoding() {
    let secret = "test-secret-key";
    let token = create_test_token(secret, 3600);

    // Token should be a valid JWT format (header.payload.signature)
    assert_eq!(token.split('.').count(), 3);
}

#[test]
fn jwt_token_expired() {
    let secret = "test-secret-key";
    let token = create_test_token(secret, -3600); // Expired 1 hour ago

    // Token should still be valid format
    assert_eq!(token.split('.').count(), 3);
}

// ============================================================================
// Rate Limiting Tests
// ============================================================================

#[test]
fn client_tier_rfq_limits() {
    assert_eq!(ClientTier::Standard.rfq_limit_per_minute(), 60);
    assert_eq!(ClientTier::Professional.rfq_limit_per_minute(), 300);
    assert_eq!(ClientTier::Enterprise.rfq_limit_per_minute(), 1000);
}

#[test]
fn client_tier_quote_limits() {
    assert_eq!(ClientTier::Standard.quote_limit_per_minute(), 300);
    assert_eq!(ClientTier::Professional.quote_limit_per_minute(), 1500);
    assert_eq!(ClientTier::Enterprise.quote_limit_per_minute(), 5000);
}

#[test]
fn client_tier_api_limits() {
    assert_eq!(ClientTier::Standard.api_limit_per_minute(), 600);
    assert_eq!(ClientTier::Professional.api_limit_per_minute(), 3000);
    assert_eq!(ClientTier::Enterprise.api_limit_per_minute(), 10000);
}

#[test]
fn client_tier_display() {
    assert_eq!(ClientTier::Standard.to_string(), "STANDARD");
    assert_eq!(ClientTier::Professional.to_string(), "PROFESSIONAL");
    assert_eq!(ClientTier::Enterprise.to_string(), "ENTERPRISE");
}

#[test]
fn client_tier_default() {
    let tier = ClientTier::default();
    assert_eq!(tier, ClientTier::Standard);
}

#[test]
fn rate_limit_type_limits() {
    assert_eq!(RateLimitType::Rfq.limit_for_tier(ClientTier::Standard), 60);
    assert_eq!(
        RateLimitType::Quote.limit_for_tier(ClientTier::Standard),
        300
    );
    assert_eq!(RateLimitType::Api.limit_for_tier(ClientTier::Standard), 600);
}

#[test]
fn rate_limit_config_default() {
    let config = RateLimitConfig::default();
    assert_eq!(config.window_duration.as_secs(), 60);
    assert!(config.include_headers);
    assert!(config.enabled);
}

#[test]
fn rate_limit_config_builder() {
    use std::time::Duration;

    let config = RateLimitConfig::default()
        .with_window(Duration::from_secs(120))
        .without_headers();

    assert_eq!(config.window_duration.as_secs(), 120);
    assert!(!config.include_headers);
    assert!(config.enabled);
}

#[test]
fn rate_limit_config_disabled() {
    let config = RateLimitConfig::default().disabled();
    assert!(!config.enabled);
}

#[tokio::test]
async fn rate_limiter_allows_requests_within_limit() {
    let config = RateLimitConfig::default();
    let limiter = InMemoryRateLimiter::new(config);

    // First request should be allowed
    let result = limiter
        .check_and_increment("test-client", ClientTier::Standard, RateLimitType::Api)
        .await;
    assert!(result.is_ok());

    let info = result.unwrap();
    assert_eq!(info.limit, 600); // Standard API limit
    assert_eq!(info.remaining, 599); // One request used
}

#[tokio::test]
async fn rate_limiter_tracks_multiple_requests() {
    let config = RateLimitConfig::default();
    let limiter = InMemoryRateLimiter::new(config);

    // Make several requests
    for i in 0..5 {
        let result = limiter
            .check_and_increment("test-client", ClientTier::Standard, RateLimitType::Api)
            .await;
        assert!(result.is_ok());
        let info = result.unwrap();
        assert_eq!(info.remaining, 600 - (i + 1));
    }
}

#[tokio::test]
async fn rate_limiter_separate_clients() {
    let config = RateLimitConfig::default();
    let limiter = InMemoryRateLimiter::new(config);

    // Client 1 makes a request
    let result1 = limiter
        .check_and_increment("client-1", ClientTier::Standard, RateLimitType::Api)
        .await;
    assert!(result1.is_ok());
    assert_eq!(result1.unwrap().remaining, 599);

    // Client 2 makes a request (should have full quota)
    let result2 = limiter
        .check_and_increment("client-2", ClientTier::Standard, RateLimitType::Api)
        .await;
    assert!(result2.is_ok());
    assert_eq!(result2.unwrap().remaining, 599);
}

#[tokio::test]
async fn rate_limiter_separate_limit_types() {
    let config = RateLimitConfig::default();
    let limiter = InMemoryRateLimiter::new(config);

    // API request
    let api_result = limiter
        .check_and_increment("test-client", ClientTier::Standard, RateLimitType::Api)
        .await;
    assert!(api_result.is_ok());

    // RFQ request (separate counter)
    let rfq_result = limiter
        .check_and_increment("test-client", ClientTier::Standard, RateLimitType::Rfq)
        .await;
    assert!(rfq_result.is_ok());
    assert_eq!(rfq_result.unwrap().remaining, 59); // RFQ limit is 60
}

#[tokio::test]
async fn rate_limiter_disabled_allows_all() {
    let config = RateLimitConfig::default().disabled();
    let limiter = InMemoryRateLimiter::new(config);

    // Should allow unlimited requests when disabled
    let result = limiter
        .check_and_increment("test-client", ClientTier::Standard, RateLimitType::Api)
        .await;
    assert!(result.is_ok());
    let info = result.unwrap();
    assert_eq!(info.limit, u32::MAX);
    assert_eq!(info.remaining, u32::MAX);
}

#[tokio::test]
async fn rate_limiter_tier_affects_limit() {
    let config = RateLimitConfig::default();
    let limiter = InMemoryRateLimiter::new(config);

    // Standard tier
    let standard = limiter
        .check_and_increment("standard-client", ClientTier::Standard, RateLimitType::Rfq)
        .await
        .unwrap();
    assert_eq!(standard.limit, 60);

    // Professional tier
    let professional = limiter
        .check_and_increment("pro-client", ClientTier::Professional, RateLimitType::Rfq)
        .await
        .unwrap();
    assert_eq!(professional.limit, 300);

    // Enterprise tier
    let enterprise = limiter
        .check_and_increment(
            "enterprise-client",
            ClientTier::Enterprise,
            RateLimitType::Rfq,
        )
        .await
        .unwrap();
    assert_eq!(enterprise.limit, 1000);
}

// ============================================================================
// Error Response Format Tests
// ============================================================================

#[test]
fn error_response_creation() {
    let error = ErrorResponse::new("TEST_ERROR", "Test error message");
    assert_eq!(error.code, "TEST_ERROR");
    assert_eq!(error.message, "Test error message");
    assert!(error.details.is_none());
}

#[test]
fn error_response_with_details() {
    let details = json!({"field": "quantity", "reason": "must be positive"});
    let error = ErrorResponse::with_details("VALIDATION_ERROR", "Invalid input", details.clone());

    assert_eq!(error.code, "VALIDATION_ERROR");
    assert_eq!(error.message, "Invalid input");
    assert_eq!(error.details, Some(details));
}

#[test]
fn error_response_serialization() {
    let error = ErrorResponse::new("NOT_FOUND", "Resource not found");
    let json = serde_json::to_string(&error).unwrap();

    assert!(json.contains("NOT_FOUND"));
    assert!(json.contains("Resource not found"));
    assert!(!json.contains("details")); // None should be skipped
}

#[test]
fn error_response_with_details_serialization() {
    let details = json!({"id": "123"});
    let error = ErrorResponse::with_details("NOT_FOUND", "Resource not found", details);
    let json = serde_json::to_string(&error).unwrap();

    assert!(json.contains("details"));
    assert!(json.contains("123"));
}

#[test]
fn error_response_deserialization() {
    let json = r#"{"code":"TEST","message":"Test message"}"#;
    let error: ErrorResponse = serde_json::from_str(json).unwrap();

    assert_eq!(error.code, "TEST");
    assert_eq!(error.message, "Test message");
    assert!(error.details.is_none());
}

// ============================================================================
// WebSocket Tests
// ============================================================================

#[test]
fn websocket_config_default() {
    let config = WebSocketConfig::default();
    assert_eq!(config.heartbeat_interval_secs, 30);
    assert_eq!(config.connection_timeout_secs, 60);
    assert_eq!(config.max_message_size, 64 * 1024);
    assert_eq!(config.max_subscriptions, 100);
}

#[test]
fn websocket_config_custom() {
    let config = WebSocketConfig {
        heartbeat_interval_secs: 15,
        connection_timeout_secs: 30,
        max_message_size: 32 * 1024,
        max_subscriptions: 50,
    };

    assert_eq!(config.heartbeat_interval_secs, 15);
    assert_eq!(config.connection_timeout_secs, 30);
    assert_eq!(config.max_message_size, 32 * 1024);
    assert_eq!(config.max_subscriptions, 50);
}

#[test]
fn websocket_state_creation() {
    let state = WebSocketState::new();
    assert_eq!(state.config.heartbeat_interval_secs, 30);
}

#[test]
fn websocket_state_with_custom_config() {
    let config = WebSocketConfig {
        heartbeat_interval_secs: 15,
        connection_timeout_secs: 30,
        max_message_size: 32 * 1024,
        max_subscriptions: 50,
    };

    let state = WebSocketState::with_config(config);
    assert_eq!(state.config.heartbeat_interval_secs, 15);
    assert_eq!(state.config.connection_timeout_secs, 30);
    assert_eq!(state.config.max_message_size, 32 * 1024);
    assert_eq!(state.config.max_subscriptions, 50);
}

// ============================================================================
// Pagination Tests
// ============================================================================

#[test]
fn pagination_params_defaults() {
    // Note: Default derive uses 0 for u32, serde defaults are only for deserialization
    let params = PaginationParams::default();
    assert_eq!(params.page, 0);
    assert_eq!(params.page_size, 0);
}

#[test]
fn pagination_offset_first_page() {
    let params = PaginationParams {
        page: 1,
        page_size: 20,
    };
    assert_eq!(params.offset(), 0);
}

#[test]
fn pagination_offset_second_page() {
    let params = PaginationParams {
        page: 2,
        page_size: 20,
    };
    assert_eq!(params.offset(), 20);
}

#[test]
fn pagination_offset_third_page() {
    let params = PaginationParams {
        page: 3,
        page_size: 10,
    };
    assert_eq!(params.offset(), 20);
}

#[test]
fn pagination_offset_zero_page() {
    let params = PaginationParams {
        page: 0,
        page_size: 20,
    };
    // Should handle gracefully (saturating_sub)
    assert_eq!(params.offset(), 0);
}

#[test]
fn pagination_limit_normal() {
    let params = PaginationParams {
        page: 1,
        page_size: 50,
    };
    assert_eq!(params.limit(), 50);
}

#[test]
fn pagination_limit_capped() {
    let params = PaginationParams {
        page: 1,
        page_size: 200,
    };
    assert_eq!(params.limit(), 100); // Capped at 100
}

#[test]
fn pagination_meta_calculation() {
    let meta = PaginationMeta::new(1, 20, 100);
    assert_eq!(meta.page, 1);
    assert_eq!(meta.page_size, 20);
    assert_eq!(meta.total_items, 100);
    assert_eq!(meta.total_pages, 5);
}

#[test]
fn pagination_meta_empty() {
    let meta = PaginationMeta::new(1, 20, 0);
    assert_eq!(meta.total_pages, 1); // At least 1 page
}

#[test]
fn pagination_meta_partial_page() {
    let meta = PaginationMeta::new(1, 20, 21);
    assert_eq!(meta.total_pages, 2); // Ceiling division
}

#[test]
fn pagination_meta_exact_pages() {
    let meta = PaginationMeta::new(1, 10, 50);
    assert_eq!(meta.total_pages, 5);
}

#[test]
fn pagination_meta_single_item() {
    let meta = PaginationMeta::new(1, 20, 1);
    assert_eq!(meta.total_pages, 1);
}

// ============================================================================
// Rate Limit Info Tests
// ============================================================================

#[test]
fn rate_limit_info_to_headers() {
    use crate::api::middleware::rate_limit::RateLimitInfo;

    let info = RateLimitInfo {
        limit: 100,
        remaining: 50,
        reset_at: 1234567890,
    };

    let headers = info.to_headers();
    assert_eq!(
        headers.get("X-RateLimit-Limit").unwrap().to_str().unwrap(),
        "100"
    );
    assert_eq!(
        headers
            .get("X-RateLimit-Remaining")
            .unwrap()
            .to_str()
            .unwrap(),
        "50"
    );
    assert_eq!(
        headers.get("X-RateLimit-Reset").unwrap().to_str().unwrap(),
        "1234567890"
    );
}
