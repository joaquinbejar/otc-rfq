//! # Rate Limiting Middleware
//!
//! Tier-based rate limiting for API endpoints.
//!
//! This module provides rate limiting middleware for axum-based APIs,
//! supporting tier-based limits with Redis-backed counter storage.
//!
//! # Tiers
//!
//! | Tier | RFQs/min | Quotes/min |
//! |------|----------|------------|
//! | Standard | 60 | 300 |
//! | Professional | 300 | 1500 |
//! | Enterprise | 1000 | 5000 |
//!
//! # Response Headers
//!
//! - `X-RateLimit-Limit` - Maximum requests allowed
//! - `X-RateLimit-Remaining` - Requests remaining in window
//! - `X-RateLimit-Reset` - Unix timestamp when window resets
//!
//! # Usage
//!
//! ```ignore
//! use otc_rfq::api::middleware::rate_limit::{RateLimitConfig, RateLimiter, ClientTier};
//!
//! let config = RateLimitConfig::default();
//! let limiter = RateLimiter::new(config);
//!
//! let app = Router::new()
//!     .route("/rfqs", post(create_rfq))
//!     .layer(rate_limit_layer(limiter));
//! ```

use axum::{
    Json,
    extract::Request,
    http::{HeaderMap, HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use thiserror::Error;
use tokio::sync::RwLock;
use tracing::{debug, instrument};

// ============================================================================
// Client Tier
// ============================================================================

/// Client tier for rate limiting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ClientTier {
    /// Standard tier: 60 RFQs/min, 300 quotes/min.
    #[default]
    Standard,
    /// Professional tier: 300 RFQs/min, 1500 quotes/min.
    Professional,
    /// Enterprise tier: 1000 RFQs/min, 5000 quotes/min.
    Enterprise,
}

impl ClientTier {
    /// Returns the RFQ rate limit per minute for this tier.
    #[must_use]
    pub const fn rfq_limit_per_minute(&self) -> u32 {
        match self {
            ClientTier::Standard => 60,
            ClientTier::Professional => 300,
            ClientTier::Enterprise => 1000,
        }
    }

    /// Returns the quote rate limit per minute for this tier.
    #[must_use]
    pub const fn quote_limit_per_minute(&self) -> u32 {
        match self {
            ClientTier::Standard => 300,
            ClientTier::Professional => 1500,
            ClientTier::Enterprise => 5000,
        }
    }

    /// Returns the general API rate limit per minute for this tier.
    #[must_use]
    pub const fn api_limit_per_minute(&self) -> u32 {
        match self {
            ClientTier::Standard => 600,
            ClientTier::Professional => 3000,
            ClientTier::Enterprise => 10000,
        }
    }
}

impl std::fmt::Display for ClientTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClientTier::Standard => write!(f, "STANDARD"),
            ClientTier::Professional => write!(f, "PROFESSIONAL"),
            ClientTier::Enterprise => write!(f, "ENTERPRISE"),
        }
    }
}

// ============================================================================
// Rate Limit Type
// ============================================================================

/// Type of rate limit to apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RateLimitType {
    /// RFQ creation rate limit.
    Rfq,
    /// Quote request rate limit.
    Quote,
    /// General API rate limit.
    Api,
}

impl RateLimitType {
    /// Returns the limit for this type based on the client tier.
    #[must_use]
    pub const fn limit_for_tier(&self, tier: ClientTier) -> u32 {
        match self {
            RateLimitType::Rfq => tier.rfq_limit_per_minute(),
            RateLimitType::Quote => tier.quote_limit_per_minute(),
            RateLimitType::Api => tier.api_limit_per_minute(),
        }
    }
}

// ============================================================================
// Configuration
// ============================================================================

/// Rate limiting configuration.
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Window duration for rate limiting.
    pub window_duration: Duration,
    /// Whether to include rate limit headers in responses.
    pub include_headers: bool,
    /// Whether rate limiting is enabled.
    pub enabled: bool,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            window_duration: Duration::from_secs(60),
            include_headers: true,
            enabled: true,
        }
    }
}

impl RateLimitConfig {
    /// Creates a new rate limit config with the given window duration.
    #[must_use]
    pub fn with_window(mut self, duration: Duration) -> Self {
        self.window_duration = duration;
        self
    }

    /// Disables rate limit headers.
    #[must_use]
    pub fn without_headers(mut self) -> Self {
        self.include_headers = false;
        self
    }

    /// Disables rate limiting.
    #[must_use]
    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }
}

// ============================================================================
// Rate Limit Error
// ============================================================================

/// Rate limiting error.
#[derive(Debug, Error)]
pub enum RateLimitError {
    /// Rate limit exceeded.
    #[error("rate limit exceeded")]
    LimitExceeded {
        /// Maximum allowed requests.
        limit: u32,
        /// Requests remaining (0).
        remaining: u32,
        /// Unix timestamp when the window resets.
        reset_at: u64,
    },

    /// Internal error.
    #[error("rate limiter internal error: {0}")]
    Internal(String),
}

impl IntoResponse for RateLimitError {
    fn into_response(self) -> Response {
        match self {
            RateLimitError::LimitExceeded {
                limit,
                remaining,
                reset_at,
            } => {
                let mut headers = HeaderMap::new();
                headers.insert(
                    "X-RateLimit-Limit",
                    HeaderValue::from_str(&limit.to_string())
                        .unwrap_or_else(|_| HeaderValue::from_static("0")),
                );
                headers.insert(
                    "X-RateLimit-Remaining",
                    HeaderValue::from_str(&remaining.to_string())
                        .unwrap_or_else(|_| HeaderValue::from_static("0")),
                );
                headers.insert(
                    "X-RateLimit-Reset",
                    HeaderValue::from_str(&reset_at.to_string())
                        .unwrap_or_else(|_| HeaderValue::from_static("0")),
                );

                let body = serde_json::json!({
                    "code": "RATE_LIMIT_EXCEEDED",
                    "message": "Too many requests. Please try again later.",
                    "limit": limit,
                    "reset_at": reset_at,
                });

                (StatusCode::TOO_MANY_REQUESTS, headers, Json(body)).into_response()
            }
            RateLimitError::Internal(msg) => {
                let body = serde_json::json!({
                    "code": "INTERNAL_ERROR",
                    "message": msg,
                });
                (StatusCode::INTERNAL_SERVER_ERROR, Json(body)).into_response()
            }
        }
    }
}

// ============================================================================
// Rate Limit Info
// ============================================================================

/// Rate limit information for a client.
#[derive(Debug, Clone)]
pub struct RateLimitInfo {
    /// Maximum allowed requests in the window.
    pub limit: u32,
    /// Remaining requests in the current window.
    pub remaining: u32,
    /// Unix timestamp when the window resets.
    pub reset_at: u64,
}

impl RateLimitInfo {
    /// Creates rate limit response headers.
    #[must_use]
    pub fn to_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        if let Ok(v) = HeaderValue::from_str(&self.limit.to_string()) {
            headers.insert("X-RateLimit-Limit", v);
        }
        if let Ok(v) = HeaderValue::from_str(&self.remaining.to_string()) {
            headers.insert("X-RateLimit-Remaining", v);
        }
        if let Ok(v) = HeaderValue::from_str(&self.reset_at.to_string()) {
            headers.insert("X-RateLimit-Reset", v);
        }
        headers
    }
}

// ============================================================================
// Counter Entry
// ============================================================================

/// A rate limit counter entry.
#[derive(Debug, Clone)]
struct CounterEntry {
    /// Number of requests in the current window.
    count: u32,
    /// When the window started.
    window_start: Instant,
    /// When the window resets (Unix timestamp).
    reset_at: u64,
}

impl CounterEntry {
    fn new(reset_at: u64) -> Self {
        Self {
            count: 0,
            window_start: Instant::now(),
            reset_at,
        }
    }
}

// ============================================================================
// In-Memory Rate Limiter
// ============================================================================

/// In-memory rate limiter implementation.
///
/// This is suitable for single-instance deployments. For distributed
/// deployments, use a Redis-backed implementation.
#[derive(Debug)]
pub struct InMemoryRateLimiter {
    /// Configuration.
    config: RateLimitConfig,
    /// Counters by client ID and rate limit type.
    counters: RwLock<HashMap<(String, RateLimitType), CounterEntry>>,
}

impl InMemoryRateLimiter {
    /// Creates a new in-memory rate limiter.
    #[must_use]
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            config,
            counters: RwLock::new(HashMap::new()),
        }
    }

    /// Checks and increments the rate limit counter.
    ///
    /// # Arguments
    ///
    /// * `client_id` - The client identifier
    /// * `tier` - The client's tier
    /// * `limit_type` - The type of rate limit to check
    ///
    /// # Returns
    ///
    /// Rate limit info if the request is allowed, or an error if exceeded.
    ///
    /// # Errors
    ///
    /// Returns `RateLimitError::LimitExceeded` if the rate limit is exceeded.
    pub async fn check_and_increment(
        &self,
        client_id: &str,
        tier: ClientTier,
        limit_type: RateLimitType,
    ) -> Result<RateLimitInfo, RateLimitError> {
        if !self.config.enabled {
            return Ok(RateLimitInfo {
                limit: u32::MAX,
                remaining: u32::MAX,
                reset_at: 0,
            });
        }

        let limit = limit_type.limit_for_tier(tier);
        let key = (client_id.to_string(), limit_type);
        let now = Instant::now();
        let reset_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() + self.config.window_duration.as_secs())
            .unwrap_or(0);

        let mut counters = self.counters.write().await;

        let entry = counters
            .entry(key)
            .or_insert_with(|| CounterEntry::new(reset_at));

        // Check if window has expired
        if now.duration_since(entry.window_start) >= self.config.window_duration {
            // Reset the window
            *entry = CounterEntry::new(reset_at);
        }

        // Check if limit exceeded
        if entry.count >= limit {
            return Err(RateLimitError::LimitExceeded {
                limit,
                remaining: 0,
                reset_at: entry.reset_at,
            });
        }

        // Increment counter
        entry.count += 1;
        let remaining = limit.saturating_sub(entry.count);

        Ok(RateLimitInfo {
            limit,
            remaining,
            reset_at: entry.reset_at,
        })
    }

    /// Gets the current rate limit info without incrementing.
    pub async fn get_info(
        &self,
        client_id: &str,
        tier: ClientTier,
        limit_type: RateLimitType,
    ) -> RateLimitInfo {
        if !self.config.enabled {
            return RateLimitInfo {
                limit: u32::MAX,
                remaining: u32::MAX,
                reset_at: 0,
            };
        }

        let limit = limit_type.limit_for_tier(tier);
        let key = (client_id.to_string(), limit_type);

        let counters = self.counters.read().await;

        if let Some(entry) = counters.get(&key) {
            let now = Instant::now();
            if now.duration_since(entry.window_start) < self.config.window_duration {
                return RateLimitInfo {
                    limit,
                    remaining: limit.saturating_sub(entry.count),
                    reset_at: entry.reset_at,
                };
            }
        }

        // No entry or expired window
        let reset_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() + self.config.window_duration.as_secs())
            .unwrap_or(0);

        RateLimitInfo {
            limit,
            remaining: limit,
            reset_at,
        }
    }

    /// Cleans up expired entries.
    pub async fn cleanup_expired(&self) {
        let now = Instant::now();
        let window = self.config.window_duration;

        let mut counters = self.counters.write().await;
        counters.retain(|_, entry| now.duration_since(entry.window_start) < window);
    }
}

impl Default for InMemoryRateLimiter {
    fn default() -> Self {
        Self::new(RateLimitConfig::default())
    }
}

// ============================================================================
// Rate Limiter Trait
// ============================================================================

/// Trait for rate limiter implementations.
#[async_trait::async_trait]
pub trait RateLimiter: Send + Sync + std::fmt::Debug {
    /// Checks and increments the rate limit counter.
    async fn check_and_increment(
        &self,
        client_id: &str,
        tier: ClientTier,
        limit_type: RateLimitType,
    ) -> Result<RateLimitInfo, RateLimitError>;

    /// Gets the current rate limit info without incrementing.
    async fn get_info(
        &self,
        client_id: &str,
        tier: ClientTier,
        limit_type: RateLimitType,
    ) -> RateLimitInfo;
}

#[async_trait::async_trait]
impl RateLimiter for InMemoryRateLimiter {
    async fn check_and_increment(
        &self,
        client_id: &str,
        tier: ClientTier,
        limit_type: RateLimitType,
    ) -> Result<RateLimitInfo, RateLimitError> {
        self.check_and_increment(client_id, tier, limit_type).await
    }

    async fn get_info(
        &self,
        client_id: &str,
        tier: ClientTier,
        limit_type: RateLimitType,
    ) -> RateLimitInfo {
        self.get_info(client_id, tier, limit_type).await
    }
}

// ============================================================================
// Rate Limit State
// ============================================================================

/// Shared state for rate limiting middleware.
#[derive(Debug, Clone)]
pub struct RateLimitState {
    /// The rate limiter implementation.
    pub limiter: Arc<dyn RateLimiter>,
    /// Configuration.
    pub config: RateLimitConfig,
    /// Default tier for unauthenticated requests.
    pub default_tier: ClientTier,
    /// Default rate limit type.
    pub default_limit_type: RateLimitType,
}

impl RateLimitState {
    /// Creates a new rate limit state with an in-memory limiter.
    #[must_use]
    pub fn new() -> Self {
        let config = RateLimitConfig::default();
        Self {
            limiter: Arc::new(InMemoryRateLimiter::new(config.clone())),
            config,
            default_tier: ClientTier::Standard,
            default_limit_type: RateLimitType::Api,
        }
    }

    /// Creates a new rate limit state with a custom limiter.
    #[must_use]
    pub fn with_limiter(limiter: Arc<dyn RateLimiter>, config: RateLimitConfig) -> Self {
        Self {
            limiter,
            config,
            default_tier: ClientTier::Standard,
            default_limit_type: RateLimitType::Api,
        }
    }

    /// Sets the default tier for unauthenticated requests.
    #[must_use]
    pub fn with_default_tier(mut self, tier: ClientTier) -> Self {
        self.default_tier = tier;
        self
    }

    /// Sets the default rate limit type.
    #[must_use]
    pub fn with_limit_type(mut self, limit_type: RateLimitType) -> Self {
        self.default_limit_type = limit_type;
        self
    }
}

impl Default for RateLimitState {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Middleware
// ============================================================================

/// Rate limiting middleware function.
///
/// # Errors
///
/// Returns a 429 Too Many Requests response if the rate limit is exceeded.
#[instrument(skip(state, request, next))]
pub async fn rate_limit_middleware(
    state: Arc<RateLimitState>,
    request: Request,
    next: Next,
) -> Result<Response, RateLimitError> {
    // Extract client ID from request (from auth claims or IP)
    let client_id = extract_client_id(&request);

    // Extract tier from request (from auth claims)
    let tier = extract_tier(&request).unwrap_or(state.default_tier);

    // Check rate limit
    let info = state
        .limiter
        .check_and_increment(&client_id, tier, state.default_limit_type)
        .await?;

    debug!(
        client_id = %client_id,
        tier = %tier,
        limit = info.limit,
        remaining = info.remaining,
        "Rate limit check passed"
    );

    // Run the next handler
    let mut response = next.run(request).await;

    // Add rate limit headers if configured
    if state.config.include_headers {
        let headers = info.to_headers();
        for (name, value) in headers.iter() {
            response.headers_mut().insert(name.clone(), value.clone());
        }
    }

    Ok(response)
}

/// Extracts the client ID from the request.
fn extract_client_id(request: &Request) -> String {
    // Try to get client ID from auth claims extension
    if let Some(claims) = request
        .extensions()
        .get::<crate::api::middleware::auth::Claims>()
    {
        if let Some(ref client_id) = claims.client_id {
            return client_id.clone();
        }
        return claims.sub.clone();
    }

    // Fall back to IP address or a default
    request
        .headers()
        .get("X-Forwarded-For")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or("unknown").trim().to_string())
        .or_else(|| {
            request
                .headers()
                .get("X-Real-IP")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| "anonymous".to_string())
}

/// Extracts the client tier from the request.
fn extract_tier(request: &Request) -> Option<ClientTier> {
    // Try to get tier from auth claims extension
    if let Some(claims) = request
        .extensions()
        .get::<crate::api::middleware::auth::Claims>()
    {
        // Check for tier in roles
        for role in &claims.roles {
            match role.to_lowercase().as_str() {
                "enterprise" => return Some(ClientTier::Enterprise),
                "professional" => return Some(ClientTier::Professional),
                "standard" => return Some(ClientTier::Standard),
                _ => {}
            }
        }
    }

    None
}

/// Creates a rate limit state for use with middleware.
#[must_use]
pub fn create_rate_limit_state() -> Arc<RateLimitState> {
    Arc::new(RateLimitState::new())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

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
    fn rate_limit_type_limit_for_tier() {
        assert_eq!(RateLimitType::Rfq.limit_for_tier(ClientTier::Standard), 60);
        assert_eq!(
            RateLimitType::Quote.limit_for_tier(ClientTier::Professional),
            1500
        );
        assert_eq!(
            RateLimitType::Api.limit_for_tier(ClientTier::Enterprise),
            10000
        );
    }

    #[test]
    fn rate_limit_config_default() {
        let config = RateLimitConfig::default();
        assert_eq!(config.window_duration, Duration::from_secs(60));
        assert!(config.include_headers);
        assert!(config.enabled);
    }

    #[test]
    fn rate_limit_config_with_window() {
        let config = RateLimitConfig::default().with_window(Duration::from_secs(30));
        assert_eq!(config.window_duration, Duration::from_secs(30));
    }

    #[test]
    fn rate_limit_config_without_headers() {
        let config = RateLimitConfig::default().without_headers();
        assert!(!config.include_headers);
    }

    #[test]
    fn rate_limit_config_disabled() {
        let config = RateLimitConfig::default().disabled();
        assert!(!config.enabled);
    }

    #[test]
    fn rate_limit_info_to_headers() {
        let info = RateLimitInfo {
            limit: 100,
            remaining: 50,
            reset_at: 1234567890,
        };

        let headers = info.to_headers();
        assert_eq!(headers.get("X-RateLimit-Limit").unwrap(), "100");
        assert_eq!(headers.get("X-RateLimit-Remaining").unwrap(), "50");
        assert_eq!(headers.get("X-RateLimit-Reset").unwrap(), "1234567890");
    }

    #[tokio::test]
    async fn in_memory_rate_limiter_allows_requests() {
        let limiter = InMemoryRateLimiter::default();

        let result = limiter
            .check_and_increment("client-1", ClientTier::Standard, RateLimitType::Api)
            .await;

        assert!(result.is_ok());
        let info = result.unwrap();
        assert_eq!(info.limit, 600);
        assert_eq!(info.remaining, 599);
    }

    #[tokio::test]
    async fn in_memory_rate_limiter_increments_counter() {
        let limiter = InMemoryRateLimiter::default();

        // First request
        let info1 = limiter
            .check_and_increment("client-1", ClientTier::Standard, RateLimitType::Api)
            .await
            .unwrap();

        // Second request
        let info2 = limiter
            .check_and_increment("client-1", ClientTier::Standard, RateLimitType::Api)
            .await
            .unwrap();

        assert_eq!(info1.remaining, 599);
        assert_eq!(info2.remaining, 598);
    }

    #[tokio::test]
    async fn in_memory_rate_limiter_exceeds_limit() {
        let config = RateLimitConfig::default();
        let limiter = InMemoryRateLimiter::new(config);

        // Use RFQ limit (60 for Standard tier)
        for _ in 0..60 {
            let result = limiter
                .check_and_increment("client-1", ClientTier::Standard, RateLimitType::Rfq)
                .await;
            assert!(result.is_ok());
        }

        // 61st request should fail
        let result = limiter
            .check_and_increment("client-1", ClientTier::Standard, RateLimitType::Rfq)
            .await;

        assert!(matches!(result, Err(RateLimitError::LimitExceeded { .. })));
    }

    #[tokio::test]
    async fn in_memory_rate_limiter_separate_clients() {
        let limiter = InMemoryRateLimiter::default();

        let info1 = limiter
            .check_and_increment("client-1", ClientTier::Standard, RateLimitType::Api)
            .await
            .unwrap();

        let info2 = limiter
            .check_and_increment("client-2", ClientTier::Standard, RateLimitType::Api)
            .await
            .unwrap();

        // Both should have full remaining (minus 1)
        assert_eq!(info1.remaining, 599);
        assert_eq!(info2.remaining, 599);
    }

    #[tokio::test]
    async fn in_memory_rate_limiter_separate_types() {
        let limiter = InMemoryRateLimiter::default();

        let info1 = limiter
            .check_and_increment("client-1", ClientTier::Standard, RateLimitType::Rfq)
            .await
            .unwrap();

        let info2 = limiter
            .check_and_increment("client-1", ClientTier::Standard, RateLimitType::Quote)
            .await
            .unwrap();

        // Different limits for different types
        assert_eq!(info1.limit, 60);
        assert_eq!(info2.limit, 300);
    }

    #[tokio::test]
    async fn in_memory_rate_limiter_disabled() {
        let config = RateLimitConfig::default().disabled();
        let limiter = InMemoryRateLimiter::new(config);

        let info = limiter
            .check_and_increment("client-1", ClientTier::Standard, RateLimitType::Api)
            .await
            .unwrap();

        assert_eq!(info.limit, u32::MAX);
        assert_eq!(info.remaining, u32::MAX);
    }

    #[tokio::test]
    async fn in_memory_rate_limiter_get_info() {
        let limiter = InMemoryRateLimiter::default();

        // Get info before any requests
        let info1 = limiter
            .get_info("client-1", ClientTier::Standard, RateLimitType::Api)
            .await;
        assert_eq!(info1.remaining, 600);

        // Make a request
        limiter
            .check_and_increment("client-1", ClientTier::Standard, RateLimitType::Api)
            .await
            .unwrap();

        // Get info after request
        let info2 = limiter
            .get_info("client-1", ClientTier::Standard, RateLimitType::Api)
            .await;
        assert_eq!(info2.remaining, 599);
    }

    #[test]
    fn rate_limit_state_default() {
        let state = RateLimitState::default();
        assert_eq!(state.default_tier, ClientTier::Standard);
        assert_eq!(state.default_limit_type, RateLimitType::Api);
    }

    #[test]
    fn rate_limit_state_with_default_tier() {
        let state = RateLimitState::new().with_default_tier(ClientTier::Enterprise);
        assert_eq!(state.default_tier, ClientTier::Enterprise);
    }

    #[test]
    fn rate_limit_state_with_limit_type() {
        let state = RateLimitState::new().with_limit_type(RateLimitType::Rfq);
        assert_eq!(state.default_limit_type, RateLimitType::Rfq);
    }

    #[test]
    fn client_tier_serialization() {
        let tier = ClientTier::Professional;
        let json = serde_json::to_string(&tier).unwrap();
        assert_eq!(json, "\"PROFESSIONAL\"");
    }

    #[test]
    fn client_tier_deserialization() {
        let tier: ClientTier = serde_json::from_str("\"ENTERPRISE\"").unwrap();
        assert_eq!(tier, ClientTier::Enterprise);
    }
}
