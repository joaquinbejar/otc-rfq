//! # Authentication Middleware
//!
//! JWT and API key authentication for API endpoints.
//!
//! This module provides authentication middleware for axum-based APIs,
//! supporting both JWT tokens and API keys.
//!
//! # Token Structure
//!
//! JWT tokens must contain the following claims:
//! - `sub` - Subject (user ID)
//! - `iss` - Issuer
//! - `aud` - Audience
//! - `exp` - Expiration time
//! - `iat` - Issued at time
//! - `roles` - User roles
//! - `permissions` - User permissions
//! - `client_id` - Client identifier
//! - `org_id` - Organization identifier
//!
//! # Usage
//!
//! ```ignore
//! use otc_rfq::api::middleware::auth::{AuthConfig, AuthLayer, Claims};
//!
//! let config = AuthConfig::new("secret-key");
//! let auth_layer = AuthLayer::new(config);
//!
//! let app = Router::new()
//!     .route("/protected", get(handler))
//!     .layer(auth_layer);
//! ```

use axum::{
    Json,
    extract::{FromRequestParts, Request},
    http::{StatusCode, header::AUTHORIZATION, request::Parts},
    middleware::Next,
    response::{IntoResponse, Response},
};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;
use thiserror::Error;
use tracing::{debug, instrument};

// ============================================================================
// Configuration
// ============================================================================

/// Authentication configuration.
#[derive(Debug, Clone)]
pub struct AuthConfig {
    /// Secret key for HMAC-based JWT validation.
    pub secret: String,
    /// Expected issuer claim.
    pub issuer: Option<String>,
    /// Expected audience claim.
    pub audience: Option<String>,
    /// Whether to allow API key authentication.
    pub allow_api_keys: bool,
    /// Valid API keys (for simple API key auth).
    pub api_keys: HashSet<String>,
    /// Whether authentication is required (false = optional).
    pub required: bool,
}

impl AuthConfig {
    /// Creates a new auth config with the given secret.
    #[must_use]
    pub fn new(secret: impl Into<String>) -> Self {
        Self {
            secret: secret.into(),
            issuer: None,
            audience: None,
            allow_api_keys: false,
            api_keys: HashSet::new(),
            required: true,
        }
    }

    /// Sets the expected issuer.
    #[must_use]
    pub fn with_issuer(mut self, issuer: impl Into<String>) -> Self {
        self.issuer = Some(issuer.into());
        self
    }

    /// Sets the expected audience.
    #[must_use]
    pub fn with_audience(mut self, audience: impl Into<String>) -> Self {
        self.audience = Some(audience.into());
        self
    }

    /// Enables API key authentication.
    #[must_use]
    pub fn with_api_keys(mut self, keys: impl IntoIterator<Item = String>) -> Self {
        self.allow_api_keys = true;
        self.api_keys = keys.into_iter().collect();
        self
    }

    /// Sets whether authentication is required.
    #[must_use]
    pub fn optional(mut self) -> Self {
        self.required = false;
        self
    }
}

// ============================================================================
// JWT Claims
// ============================================================================

/// JWT claims structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// Subject (user ID).
    pub sub: String,
    /// Issuer.
    #[serde(default)]
    pub iss: Option<String>,
    /// Audience.
    #[serde(default)]
    pub aud: Option<String>,
    /// Expiration time (Unix timestamp).
    pub exp: u64,
    /// Issued at time (Unix timestamp).
    pub iat: u64,
    /// User roles.
    #[serde(default)]
    pub roles: Vec<String>,
    /// User permissions.
    #[serde(default)]
    pub permissions: Vec<String>,
    /// Client identifier.
    #[serde(default)]
    pub client_id: Option<String>,
    /// Organization identifier.
    #[serde(default)]
    pub org_id: Option<String>,
}

impl Claims {
    /// Creates new claims for a subject.
    #[must_use]
    pub fn new(sub: impl Into<String>, exp: u64, iat: u64) -> Self {
        Self {
            sub: sub.into(),
            iss: None,
            aud: None,
            exp,
            iat,
            roles: Vec::new(),
            permissions: Vec::new(),
            client_id: None,
            org_id: None,
        }
    }

    /// Sets the issuer.
    #[must_use]
    pub fn with_issuer(mut self, issuer: impl Into<String>) -> Self {
        self.iss = Some(issuer.into());
        self
    }

    /// Sets the audience.
    #[must_use]
    pub fn with_audience(mut self, audience: impl Into<String>) -> Self {
        self.aud = Some(audience.into());
        self
    }

    /// Sets the roles.
    #[must_use]
    pub fn with_roles(mut self, roles: Vec<String>) -> Self {
        self.roles = roles;
        self
    }

    /// Sets the permissions.
    #[must_use]
    pub fn with_permissions(mut self, permissions: Vec<String>) -> Self {
        self.permissions = permissions;
        self
    }

    /// Sets the client ID.
    #[must_use]
    pub fn with_client_id(mut self, client_id: impl Into<String>) -> Self {
        self.client_id = Some(client_id.into());
        self
    }

    /// Sets the organization ID.
    #[must_use]
    pub fn with_org_id(mut self, org_id: impl Into<String>) -> Self {
        self.org_id = Some(org_id.into());
        self
    }

    /// Checks if the user has a specific role.
    #[must_use]
    pub fn has_role(&self, role: &str) -> bool {
        self.roles.iter().any(|r| r == role)
    }

    /// Checks if the user has a specific permission.
    #[must_use]
    pub fn has_permission(&self, permission: &str) -> bool {
        self.permissions.iter().any(|p| p == permission)
    }

    /// Checks if the token is expired.
    #[must_use]
    pub fn is_expired(&self) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        self.exp < now
    }
}

// ============================================================================
// Authentication Error
// ============================================================================

/// Authentication error types.
#[derive(Debug, Error)]
pub enum AuthError {
    /// Missing authentication credentials.
    #[error("missing authentication credentials")]
    MissingCredentials,

    /// Invalid token format.
    #[error("invalid token format")]
    InvalidTokenFormat,

    /// Token validation failed.
    #[error("token validation failed: {0}")]
    ValidationFailed(String),

    /// Token expired.
    #[error("token expired")]
    TokenExpired,

    /// Invalid API key.
    #[error("invalid API key")]
    InvalidApiKey,

    /// Insufficient permissions.
    #[error("insufficient permissions")]
    InsufficientPermissions,
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, code, message) = match &self {
            AuthError::MissingCredentials => (
                StatusCode::UNAUTHORIZED,
                "MISSING_CREDENTIALS",
                self.to_string(),
            ),
            AuthError::InvalidTokenFormat => {
                (StatusCode::UNAUTHORIZED, "INVALID_TOKEN", self.to_string())
            }
            AuthError::ValidationFailed(_) => (
                StatusCode::UNAUTHORIZED,
                "VALIDATION_FAILED",
                self.to_string(),
            ),
            AuthError::TokenExpired => {
                (StatusCode::UNAUTHORIZED, "TOKEN_EXPIRED", self.to_string())
            }
            AuthError::InvalidApiKey => (
                StatusCode::UNAUTHORIZED,
                "INVALID_API_KEY",
                self.to_string(),
            ),
            AuthError::InsufficientPermissions => {
                (StatusCode::FORBIDDEN, "FORBIDDEN", self.to_string())
            }
        };

        let body = serde_json::json!({
            "code": code,
            "message": message,
        });

        (status, Json(body)).into_response()
    }
}

// ============================================================================
// Token Extraction
// ============================================================================

/// Query parameters for token extraction.
#[derive(Debug, Deserialize)]
pub struct TokenQuery {
    /// Token passed as query parameter.
    pub token: Option<String>,
    /// API key passed as query parameter.
    pub api_key: Option<String>,
}

/// Extracts the bearer token from the Authorization header.
fn extract_bearer_token(auth_header: &str) -> Option<&str> {
    auth_header
        .strip_prefix("Bearer ")
        .or_else(|| auth_header.strip_prefix("bearer "))
}

/// Extracts the API key from the Authorization header.
fn extract_api_key_from_header(auth_header: &str) -> Option<&str> {
    auth_header
        .strip_prefix("ApiKey ")
        .or_else(|| auth_header.strip_prefix("apikey "))
        .or_else(|| auth_header.strip_prefix("X-API-Key "))
}

// ============================================================================
// JWT Utilities
// ============================================================================

/// Validates a JWT token and returns the claims.
///
/// # Errors
///
/// Returns an error if the token is invalid or validation fails.
pub fn validate_jwt(token: &str, config: &AuthConfig) -> Result<Claims, AuthError> {
    let mut validation = Validation::default();

    if let Some(ref issuer) = config.issuer {
        validation.set_issuer(&[issuer]);
    }

    if let Some(ref audience) = config.audience {
        validation.set_audience(&[audience]);
    }

    let key = DecodingKey::from_secret(config.secret.as_bytes());

    decode::<Claims>(token, &key, &validation)
        .map(|data| data.claims)
        .map_err(|e| {
            if e.to_string().contains("ExpiredSignature") {
                AuthError::TokenExpired
            } else {
                AuthError::ValidationFailed(e.to_string())
            }
        })
}

/// Creates a JWT token from claims.
///
/// # Errors
///
/// Returns an error if token encoding fails.
pub fn create_jwt(claims: &Claims, secret: &str) -> Result<String, AuthError> {
    let key = EncodingKey::from_secret(secret.as_bytes());

    encode(&Header::default(), claims, &key).map_err(|e| AuthError::ValidationFailed(e.to_string()))
}

// ============================================================================
// Middleware
// ============================================================================

/// Authentication middleware function.
///
/// # Errors
///
/// Returns an error response if authentication fails.
#[instrument(skip(config, request, next))]
pub async fn auth_middleware(
    config: Arc<AuthConfig>,
    mut request: Request,
    next: Next,
) -> Result<Response, AuthError> {
    // Try to extract token from Authorization header
    let auth_header = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok());

    // Try to extract from query parameters
    let query_params: Option<TokenQuery> = request.uri().query().and_then(|q| {
        let pairs: Vec<(&str, &str)> = q
            .split('&')
            .filter_map(|pair| {
                let mut parts = pair.splitn(2, '=');
                Some((parts.next()?, parts.next()?))
            })
            .collect();

        let token = pairs
            .iter()
            .find(|(k, _)| *k == "token")
            .map(|(_, v)| v.to_string());
        let api_key = pairs
            .iter()
            .find(|(k, _)| *k == "api_key")
            .map(|(_, v)| v.to_string());

        if token.is_some() || api_key.is_some() {
            Some(TokenQuery { token, api_key })
        } else {
            None
        }
    });

    // Try bearer token first
    let token = auth_header
        .and_then(extract_bearer_token)
        .or_else(|| query_params.as_ref().and_then(|q| q.token.as_deref()));

    if let Some(token) = token {
        debug!("Validating JWT token");
        let claims = validate_jwt(token, &config)?;
        request.extensions_mut().insert(claims);
        return Ok(next.run(request).await);
    }

    // Try API key if enabled
    if config.allow_api_keys {
        let api_key = auth_header
            .and_then(extract_api_key_from_header)
            .or_else(|| query_params.as_ref().and_then(|q| q.api_key.as_deref()));

        if let Some(api_key) = api_key {
            debug!("Validating API key");
            if config.api_keys.contains(api_key) {
                // Create minimal claims for API key auth
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);

                let claims = Claims::new("api-key-user", now + 3600, now)
                    .with_roles(vec!["api-key".to_string()]);

                request.extensions_mut().insert(claims);
                return Ok(next.run(request).await);
            } else {
                return Err(AuthError::InvalidApiKey);
            }
        }
    }

    // No credentials found
    if config.required {
        Err(AuthError::MissingCredentials)
    } else {
        // Optional auth - proceed without claims
        Ok(next.run(request).await)
    }
}

/// Creates an authentication middleware layer.
///
/// # Arguments
///
/// * `config` - Authentication configuration
///
/// # Returns
///
/// A closure that can be used with `axum::middleware::from_fn_with_state`.
///
/// # Examples
///
/// ```ignore
/// use axum::middleware::from_fn_with_state;
///
/// let config = Arc::new(AuthConfig::new("secret"));
/// let app = Router::new()
///     .route("/", get(handler))
///     .layer(from_fn_with_state(config, auth_middleware));
/// ```
pub fn create_auth_config(config: AuthConfig) -> Arc<AuthConfig> {
    Arc::new(config)
}

// ============================================================================
// Request Extension Extractor
// ============================================================================

/// Extractor for authenticated claims from request extensions.
#[derive(Debug, Clone)]
pub struct AuthenticatedUser(pub Claims);

impl<S> FromRequestParts<S> for AuthenticatedUser
where
    S: Send + Sync,
{
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<Claims>()
            .cloned()
            .map(AuthenticatedUser)
            .ok_or(AuthError::MissingCredentials)
    }
}

/// Extractor for optional authenticated claims.
#[derive(Debug, Clone)]
pub struct OptionalUser(pub Option<Claims>);

impl<S> FromRequestParts<S> for OptionalUser
where
    S: Send + Sync,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(OptionalUser(parts.extensions.get::<Claims>().cloned()))
    }
}

// ============================================================================
// Role/Permission Guards
// ============================================================================

/// Checks if the user has the required role.
///
/// # Errors
///
/// Returns `AuthError::InsufficientPermissions` if the user lacks the role.
pub fn require_role(claims: &Claims, role: &str) -> Result<(), AuthError> {
    if claims.has_role(role) {
        Ok(())
    } else {
        Err(AuthError::InsufficientPermissions)
    }
}

/// Checks if the user has the required permission.
///
/// # Errors
///
/// Returns `AuthError::InsufficientPermissions` if the user lacks the permission.
pub fn require_permission(claims: &Claims, permission: &str) -> Result<(), AuthError> {
    if claims.has_permission(permission) {
        Ok(())
    } else {
        Err(AuthError::InsufficientPermissions)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn create_test_config() -> AuthConfig {
        AuthConfig::new("test-secret-key-for-jwt-validation")
    }

    fn create_test_claims() -> Claims {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Claims::new("user-123", now + 3600, now)
            .with_issuer("test-issuer")
            .with_audience("test-audience")
            .with_roles(vec!["admin".to_string(), "user".to_string()])
            .with_permissions(vec!["read".to_string(), "write".to_string()])
            .with_client_id("client-456")
            .with_org_id("org-789")
    }

    #[test]
    fn auth_config_new() {
        let config = AuthConfig::new("secret");
        assert_eq!(config.secret, "secret");
        assert!(config.issuer.is_none());
        assert!(config.audience.is_none());
        assert!(!config.allow_api_keys);
        assert!(config.required);
    }

    #[test]
    fn auth_config_with_issuer() {
        let config = AuthConfig::new("secret").with_issuer("my-issuer");
        assert_eq!(config.issuer, Some("my-issuer".to_string()));
    }

    #[test]
    fn auth_config_with_audience() {
        let config = AuthConfig::new("secret").with_audience("my-audience");
        assert_eq!(config.audience, Some("my-audience".to_string()));
    }

    #[test]
    fn auth_config_with_api_keys() {
        let config =
            AuthConfig::new("secret").with_api_keys(vec!["key1".to_string(), "key2".to_string()]);
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
    fn claims_new() {
        let claims = Claims::new("user-1", 1000, 900);
        assert_eq!(claims.sub, "user-1");
        assert_eq!(claims.exp, 1000);
        assert_eq!(claims.iat, 900);
    }

    #[test]
    fn claims_has_role() {
        let claims = create_test_claims();
        assert!(claims.has_role("admin"));
        assert!(claims.has_role("user"));
        assert!(!claims.has_role("superadmin"));
    }

    #[test]
    fn claims_has_permission() {
        let claims = create_test_claims();
        assert!(claims.has_permission("read"));
        assert!(claims.has_permission("write"));
        assert!(!claims.has_permission("delete"));
    }

    #[test]
    fn claims_is_expired() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let expired_claims = Claims::new("user", now - 100, now - 200);
        assert!(expired_claims.is_expired());

        let valid_claims = Claims::new("user", now + 100, now);
        assert!(!valid_claims.is_expired());
    }

    #[test]
    fn extract_bearer_token_valid() {
        assert_eq!(extract_bearer_token("Bearer abc123"), Some("abc123"));
        assert_eq!(extract_bearer_token("bearer xyz789"), Some("xyz789"));
    }

    #[test]
    fn extract_bearer_token_invalid() {
        assert_eq!(extract_bearer_token("Basic abc123"), None);
        assert_eq!(extract_bearer_token("abc123"), None);
    }

    #[test]
    fn extract_api_key_from_header_valid() {
        assert_eq!(extract_api_key_from_header("ApiKey abc123"), Some("abc123"));
        assert_eq!(extract_api_key_from_header("apikey xyz789"), Some("xyz789"));
        assert_eq!(
            extract_api_key_from_header("X-API-Key key123"),
            Some("key123")
        );
    }

    #[test]
    fn extract_api_key_from_header_invalid() {
        assert_eq!(extract_api_key_from_header("Bearer abc123"), None);
    }

    #[test]
    fn create_and_validate_jwt() {
        let config = create_test_config();
        // Create claims without issuer/audience to avoid validation issues
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let claims = Claims::new("user-123", now + 3600, now)
            .with_roles(vec!["admin".to_string()])
            .with_client_id("client-456")
            .with_org_id("org-789");

        let token = create_jwt(&claims, &config.secret).unwrap();
        assert!(!token.is_empty());

        let validated = validate_jwt(&token, &config).unwrap();
        assert_eq!(validated.sub, claims.sub);
        assert_eq!(validated.client_id, claims.client_id);
        assert_eq!(validated.org_id, claims.org_id);
    }

    #[test]
    fn validate_jwt_invalid_secret() {
        let config = create_test_config();
        let claims = create_test_claims();

        let token = create_jwt(&claims, &config.secret).unwrap();

        let wrong_config = AuthConfig::new("wrong-secret");
        let result = validate_jwt(&token, &wrong_config);
        assert!(result.is_err());
    }

    #[test]
    fn validate_jwt_expired() {
        let config = create_test_config();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let expired_claims = Claims::new("user", now - 100, now - 200);
        let token = create_jwt(&expired_claims, &config.secret).unwrap();

        let result = validate_jwt(&token, &config);
        assert!(matches!(result, Err(AuthError::TokenExpired)));
    }

    #[test]
    fn require_role_success() {
        let claims = create_test_claims();
        assert!(require_role(&claims, "admin").is_ok());
    }

    #[test]
    fn require_role_failure() {
        let claims = create_test_claims();
        assert!(matches!(
            require_role(&claims, "superadmin"),
            Err(AuthError::InsufficientPermissions)
        ));
    }

    #[test]
    fn require_permission_success() {
        let claims = create_test_claims();
        assert!(require_permission(&claims, "read").is_ok());
    }

    #[test]
    fn require_permission_failure() {
        let claims = create_test_claims();
        assert!(matches!(
            require_permission(&claims, "delete"),
            Err(AuthError::InsufficientPermissions)
        ));
    }

    #[test]
    fn auth_error_display() {
        assert_eq!(
            AuthError::MissingCredentials.to_string(),
            "missing authentication credentials"
        );
        assert_eq!(
            AuthError::InvalidTokenFormat.to_string(),
            "invalid token format"
        );
        assert_eq!(AuthError::TokenExpired.to_string(), "token expired");
        assert_eq!(AuthError::InvalidApiKey.to_string(), "invalid API key");
        assert_eq!(
            AuthError::InsufficientPermissions.to_string(),
            "insufficient permissions"
        );
    }

    #[test]
    fn claims_serialization() {
        let claims = create_test_claims();
        let json = serde_json::to_string(&claims).unwrap();
        assert!(json.contains("\"sub\":\"user-123\""));
        assert!(json.contains("\"client_id\":\"client-456\""));
    }

    #[test]
    fn claims_deserialization() {
        let json = r#"{
            "sub": "user-1",
            "exp": 1000,
            "iat": 900,
            "roles": ["admin"],
            "permissions": ["read"]
        }"#;

        let claims: Claims = serde_json::from_str(json).unwrap();
        assert_eq!(claims.sub, "user-1");
        assert_eq!(claims.roles, vec!["admin"]);
        assert_eq!(claims.permissions, vec!["read"]);
    }
}
