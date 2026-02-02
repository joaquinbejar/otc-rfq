//! # API Middleware
//!
//! Cross-cutting concerns for API requests.
//!
//! This module provides middleware components for the API layer,
//! including authentication, logging, rate limiting, and tracing.
//!
//! # Authentication
//!
//! The `auth` module provides JWT and API key authentication:
//!
//! ```ignore
//! use otc_rfq::api::middleware::auth::{AuthConfig, auth_layer, Claims};
//!
//! let config = AuthConfig::new("secret-key")
//!     .with_issuer("my-service")
//!     .with_audience("my-api");
//!
//! let app = Router::new()
//!     .route("/protected", get(handler))
//!     .layer(auth_layer(config));
//! ```

pub mod auth;
pub mod logging;
pub mod rate_limit;
pub mod tracing_mw;

pub use auth::{
    auth_middleware, create_auth_config, create_jwt, require_permission, require_role,
    validate_jwt, AuthConfig, AuthError, AuthenticatedUser, Claims, OptionalUser, TokenQuery,
};

pub use rate_limit::{
    create_rate_limit_state, rate_limit_middleware, ClientTier, InMemoryRateLimiter,
    RateLimitConfig, RateLimitError, RateLimitInfo, RateLimitState, RateLimitType, RateLimiter,
};

pub use logging::{
    create_logging_state, create_logging_state_with_config, logging_middleware, redact_headers,
    redact_sensitive, LogEntry, LoggingConfig, LoggingState, RequestId,
};

pub use tracing_mw::{
    create_tracing_state, create_tracing_state_with_config, extract_trace_context,
    generate_span_id, generate_trace_id, headers, inject_trace_context, tracing_middleware,
    TraceContext, TracingConfig, TracingState,
};
