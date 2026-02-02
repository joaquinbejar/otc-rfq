//! # Tracing Middleware
//!
//! Distributed tracing support with OpenTelemetry context propagation.
//!
//! This module provides middleware for distributed tracing, including
//! trace context propagation and span creation for HTTP requests.
//!
//! # Features
//!
//! - OpenTelemetry trace context propagation
//! - W3C Trace Context header support
//! - Span creation with request attributes
//! - Trace ID and Span ID extraction/injection
//!
//! # Usage
//!
//! ```ignore
//! use otc_rfq::api::middleware::tracing_mw::{TracingConfig, tracing_middleware};
//!
//! let config = TracingConfig::default();
//! let app = Router::new()
//!     .route("/api", get(handler))
//!     .layer(tracing_layer(config));
//! ```

use axum::{
    extract::Request,
    http::{HeaderMap, HeaderValue},
    middleware::Next,
    response::Response,
};
use std::sync::Arc;
use tracing::{debug, info_span, instrument};

// ============================================================================
// Trace Context Headers
// ============================================================================

/// W3C Trace Context header names.
pub mod headers {
    /// The traceparent header name.
    pub const TRACEPARENT: &str = "traceparent";
    /// The tracestate header name.
    pub const TRACESTATE: &str = "tracestate";
    /// The B3 trace ID header (Zipkin).
    pub const B3_TRACE_ID: &str = "X-B3-TraceId";
    /// The B3 span ID header (Zipkin).
    pub const B3_SPAN_ID: &str = "X-B3-SpanId";
    /// The B3 sampled header (Zipkin).
    pub const B3_SAMPLED: &str = "X-B3-Sampled";
    /// The B3 single header (Zipkin).
    pub const B3_SINGLE: &str = "b3";
}

// ============================================================================
// Trace Context
// ============================================================================

/// Parsed W3C Trace Context.
#[derive(Debug, Clone, Default)]
pub struct TraceContext {
    /// Trace ID (32 hex characters).
    pub trace_id: Option<String>,
    /// Parent span ID (16 hex characters).
    pub parent_span_id: Option<String>,
    /// Trace flags.
    pub trace_flags: u8,
    /// Trace state (vendor-specific data).
    pub trace_state: Option<String>,
}

impl TraceContext {
    /// Creates a new empty trace context.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new trace context with a generated trace ID.
    #[must_use]
    pub fn generate() -> Self {
        Self {
            trace_id: Some(generate_trace_id()),
            parent_span_id: None,
            trace_flags: 1, // Sampled
            trace_state: None,
        }
    }

    /// Parses a trace context from the traceparent header.
    ///
    /// Format: `{version}-{trace_id}-{parent_id}-{flags}`
    /// Example: `00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01`
    #[must_use]
    pub fn from_traceparent(header: &str) -> Option<Self> {
        let parts: Vec<&str> = header.split('-').collect();
        if parts.len() != 4 {
            return None;
        }

        let version = parts.first()?;
        if *version != "00" {
            return None; // Only support version 00
        }

        let trace_id = parts.get(1)?;
        let parent_span_id = parts.get(2)?;
        let flags_str = parts.get(3)?;
        let flags = u8::from_str_radix(flags_str, 16).ok()?;

        // Validate lengths
        if trace_id.len() != 32 || parent_span_id.len() != 16 {
            return None;
        }

        Some(Self {
            trace_id: Some((*trace_id).to_string()),
            parent_span_id: Some((*parent_span_id).to_string()),
            trace_flags: flags,
            trace_state: None,
        })
    }

    /// Formats the trace context as a traceparent header value.
    #[must_use]
    pub fn to_traceparent(&self) -> Option<String> {
        let trace_id = self.trace_id.as_ref()?;
        let span_id = self.parent_span_id.as_deref().unwrap_or("0000000000000000");
        Some(format!(
            "00-{}-{}-{:02x}",
            trace_id, span_id, self.trace_flags
        ))
    }

    /// Returns whether this trace is sampled.
    #[must_use]
    pub fn is_sampled(&self) -> bool {
        self.trace_flags & 0x01 != 0
    }

    /// Sets the sampled flag.
    pub fn set_sampled(&mut self, sampled: bool) {
        if sampled {
            self.trace_flags |= 0x01;
        } else {
            self.trace_flags &= !0x01;
        }
    }
}

/// Generates a random trace ID (32 hex characters).
#[must_use]
pub fn generate_trace_id() -> String {
    use uuid::Uuid;
    let uuid1 = Uuid::new_v4();
    let uuid2 = Uuid::new_v4();
    format!("{:032x}", uuid1.as_u128() ^ (uuid2.as_u128() << 64 >> 64))
}

/// Generates a random span ID (16 hex characters).
#[must_use]
pub fn generate_span_id() -> String {
    use uuid::Uuid;
    let uuid = Uuid::new_v4();
    format!("{:016x}", uuid.as_u128() as u64)
}

// ============================================================================
// Configuration
// ============================================================================

/// Tracing configuration.
#[derive(Debug, Clone)]
pub struct TracingConfig {
    /// Whether to propagate trace context.
    pub propagate_context: bool,
    /// Whether to generate trace IDs if not present.
    pub generate_if_missing: bool,
    /// Whether to include trace headers in response.
    pub include_response_headers: bool,
    /// Service name for spans.
    pub service_name: String,
    /// Sampling rate (0.0 to 1.0).
    pub sampling_rate: f64,
}

impl Default for TracingConfig {
    fn default() -> Self {
        Self {
            propagate_context: true,
            generate_if_missing: true,
            include_response_headers: true,
            service_name: "otc-rfq".to_string(),
            sampling_rate: 1.0,
        }
    }
}

impl TracingConfig {
    /// Creates a new tracing config.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the service name.
    #[must_use]
    pub fn with_service_name(mut self, name: impl Into<String>) -> Self {
        self.service_name = name.into();
        self
    }

    /// Sets the sampling rate.
    #[must_use]
    pub fn with_sampling_rate(mut self, rate: f64) -> Self {
        self.sampling_rate = rate.clamp(0.0, 1.0);
        self
    }

    /// Disables context propagation.
    #[must_use]
    pub fn without_propagation(mut self) -> Self {
        self.propagate_context = false;
        self
    }

    /// Disables response headers.
    #[must_use]
    pub fn without_response_headers(mut self) -> Self {
        self.include_response_headers = false;
        self
    }
}

// ============================================================================
// Tracing State
// ============================================================================

/// Shared state for tracing middleware.
#[derive(Debug, Clone)]
pub struct TracingState {
    /// Configuration.
    pub config: TracingConfig,
}

impl TracingState {
    /// Creates a new tracing state.
    #[must_use]
    pub fn new(config: TracingConfig) -> Self {
        Self { config }
    }
}

impl Default for TracingState {
    fn default() -> Self {
        Self::new(TracingConfig::default())
    }
}

// ============================================================================
// Context Extraction
// ============================================================================

/// Extracts trace context from request headers.
#[must_use]
pub fn extract_trace_context(headers: &HeaderMap) -> Option<TraceContext> {
    // Try W3C Trace Context first
    if let Some(traceparent) = headers.get(headers::TRACEPARENT) {
        if let Ok(value) = traceparent.to_str() {
            if let Some(mut ctx) = TraceContext::from_traceparent(value) {
                // Also extract tracestate if present
                if let Some(tracestate) = headers.get(headers::TRACESTATE) {
                    if let Ok(state) = tracestate.to_str() {
                        ctx.trace_state = Some(state.to_string());
                    }
                }
                return Some(ctx);
            }
        }
    }

    // Try B3 headers (Zipkin)
    if let Some(trace_id) = headers.get(headers::B3_TRACE_ID) {
        if let Ok(trace_id_str) = trace_id.to_str() {
            let mut ctx = TraceContext::new();
            ctx.trace_id = Some(trace_id_str.to_string());

            if let Some(span_id) = headers.get(headers::B3_SPAN_ID) {
                if let Ok(span_id_str) = span_id.to_str() {
                    ctx.parent_span_id = Some(span_id_str.to_string());
                }
            }

            if let Some(sampled) = headers.get(headers::B3_SAMPLED) {
                if let Ok(sampled_str) = sampled.to_str() {
                    ctx.set_sampled(sampled_str == "1");
                }
            }

            return Some(ctx);
        }
    }

    // Try B3 single header
    if let Some(b3) = headers.get(headers::B3_SINGLE) {
        if let Ok(b3_str) = b3.to_str() {
            return parse_b3_single(b3_str);
        }
    }

    None
}

/// Parses B3 single header format.
fn parse_b3_single(header: &str) -> Option<TraceContext> {
    // Format: {TraceId}-{SpanId}-{SamplingState}-{ParentSpanId}
    // or: {TraceId}-{SpanId}-{SamplingState}
    // or: {TraceId}-{SpanId}
    let parts: Vec<&str> = header.split('-').collect();

    let trace_id = parts.first()?;
    let span_id = parts.get(1)?;

    let mut ctx = TraceContext::new();
    ctx.trace_id = Some((*trace_id).to_string());
    ctx.parent_span_id = Some((*span_id).to_string());

    if let Some(sampled) = parts.get(2) {
        ctx.set_sampled(*sampled == "1" || *sampled == "d");
    }

    Some(ctx)
}

/// Injects trace context into response headers.
pub fn inject_trace_context(headers: &mut HeaderMap, ctx: &TraceContext) {
    if let Some(traceparent) = ctx.to_traceparent() {
        if let Ok(value) = HeaderValue::from_str(&traceparent) {
            headers.insert(headers::TRACEPARENT, value);
        }
    }

    if let Some(ref tracestate) = ctx.trace_state {
        if let Ok(value) = HeaderValue::from_str(tracestate) {
            headers.insert(headers::TRACESTATE, value);
        }
    }
}

// ============================================================================
// Middleware
// ============================================================================

/// Tracing middleware function.
///
/// Creates spans for requests and propagates trace context.
#[instrument(skip_all)]
pub async fn tracing_middleware(
    state: Arc<TracingState>,
    request: Request,
    next: Next,
) -> Response {
    // Extract or generate trace context
    let mut trace_ctx = if state.config.propagate_context {
        extract_trace_context(request.headers())
    } else {
        None
    };

    if trace_ctx.is_none() && state.config.generate_if_missing {
        trace_ctx = Some(TraceContext::generate());
    }

    // Generate a new span ID for this request
    let span_id = generate_span_id();

    // Create span with trace context
    let span = if let Some(ref ctx) = trace_ctx {
        info_span!(
            "http_request",
            trace_id = ctx.trace_id.as_deref().unwrap_or("unknown"),
            span_id = %span_id,
            parent_span_id = ctx.parent_span_id.as_deref().unwrap_or("none"),
            service = %state.config.service_name,
            method = %request.method(),
            path = %request.uri().path(),
        )
    } else {
        info_span!(
            "http_request",
            span_id = %span_id,
            service = %state.config.service_name,
            method = %request.method(),
            path = %request.uri().path(),
        )
    };

    // Store trace context in extensions
    let mut request = request;
    if let Some(ctx) = trace_ctx.clone() {
        request.extensions_mut().insert(ctx);
    }

    // Execute request within span
    let mut response = {
        let _guard = span.enter();
        debug!("Processing request");
        next.run(request).await
    };

    // Add trace headers to response
    if state.config.include_response_headers {
        if let Some(mut ctx) = trace_ctx {
            // Update parent span ID to current span
            ctx.parent_span_id = Some(span_id);
            inject_trace_context(response.headers_mut(), &ctx);
        }
    }

    response
}

/// Creates a tracing state for use with middleware.
#[must_use]
pub fn create_tracing_state() -> Arc<TracingState> {
    Arc::new(TracingState::default())
}

/// Creates a tracing state with custom config.
#[must_use]
pub fn create_tracing_state_with_config(config: TracingConfig) -> Arc<TracingState> {
    Arc::new(TracingState::new(config))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn trace_context_new() {
        let ctx = TraceContext::new();
        assert!(ctx.trace_id.is_none());
        assert!(ctx.parent_span_id.is_none());
        assert_eq!(ctx.trace_flags, 0);
    }

    #[test]
    fn trace_context_generate() {
        let ctx = TraceContext::generate();
        assert!(ctx.trace_id.is_some());
        assert!(ctx.is_sampled());
    }

    #[test]
    fn trace_context_from_traceparent_valid() {
        let header = "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01";
        let ctx = TraceContext::from_traceparent(header).unwrap();

        assert_eq!(
            ctx.trace_id.as_deref(),
            Some("0af7651916cd43dd8448eb211c80319c")
        );
        assert_eq!(ctx.parent_span_id.as_deref(), Some("b7ad6b7169203331"));
        assert_eq!(ctx.trace_flags, 1);
        assert!(ctx.is_sampled());
    }

    #[test]
    fn trace_context_from_traceparent_not_sampled() {
        let header = "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-00";
        let ctx = TraceContext::from_traceparent(header).unwrap();

        assert!(!ctx.is_sampled());
    }

    #[test]
    fn trace_context_from_traceparent_invalid_version() {
        let header = "01-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01";
        assert!(TraceContext::from_traceparent(header).is_none());
    }

    #[test]
    fn trace_context_from_traceparent_invalid_format() {
        let header = "invalid-header";
        assert!(TraceContext::from_traceparent(header).is_none());
    }

    #[test]
    fn trace_context_to_traceparent() {
        let ctx = TraceContext {
            trace_id: Some("0af7651916cd43dd8448eb211c80319c".to_string()),
            parent_span_id: Some("b7ad6b7169203331".to_string()),
            trace_flags: 1,
            trace_state: None,
        };

        let header = ctx.to_traceparent().unwrap();
        assert_eq!(
            header,
            "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01"
        );
    }

    #[test]
    fn trace_context_set_sampled() {
        let mut ctx = TraceContext::new();
        assert!(!ctx.is_sampled());

        ctx.set_sampled(true);
        assert!(ctx.is_sampled());

        ctx.set_sampled(false);
        assert!(!ctx.is_sampled());
    }

    #[test]
    fn generate_trace_id_length() {
        let trace_id = generate_trace_id();
        assert_eq!(trace_id.len(), 32);
    }

    #[test]
    fn generate_span_id_length() {
        let span_id = generate_span_id();
        assert_eq!(span_id.len(), 16);
    }

    #[test]
    fn generate_trace_id_unique() {
        let id1 = generate_trace_id();
        let id2 = generate_trace_id();
        assert_ne!(id1, id2);
    }

    #[test]
    fn tracing_config_default() {
        let config = TracingConfig::default();
        assert!(config.propagate_context);
        assert!(config.generate_if_missing);
        assert!(config.include_response_headers);
        assert_eq!(config.service_name, "otc-rfq");
        assert!((config.sampling_rate - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn tracing_config_with_service_name() {
        let config = TracingConfig::default().with_service_name("my-service");
        assert_eq!(config.service_name, "my-service");
    }

    #[test]
    fn tracing_config_with_sampling_rate() {
        let config = TracingConfig::default().with_sampling_rate(0.5);
        assert!((config.sampling_rate - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn tracing_config_with_sampling_rate_clamped() {
        let config = TracingConfig::default().with_sampling_rate(2.0);
        assert!((config.sampling_rate - 1.0).abs() < f64::EPSILON);

        let config = TracingConfig::default().with_sampling_rate(-1.0);
        assert!((config.sampling_rate - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn tracing_config_without_propagation() {
        let config = TracingConfig::default().without_propagation();
        assert!(!config.propagate_context);
    }

    #[test]
    fn tracing_config_without_response_headers() {
        let config = TracingConfig::default().without_response_headers();
        assert!(!config.include_response_headers);
    }

    #[test]
    fn extract_trace_context_w3c() {
        let mut headers = HeaderMap::new();
        headers.insert(
            headers::TRACEPARENT,
            HeaderValue::from_static("00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01"),
        );

        let ctx = extract_trace_context(&headers).unwrap();
        assert_eq!(
            ctx.trace_id.as_deref(),
            Some("0af7651916cd43dd8448eb211c80319c")
        );
    }

    #[test]
    fn extract_trace_context_b3() {
        let mut headers = HeaderMap::new();
        headers.insert(
            headers::B3_TRACE_ID,
            HeaderValue::from_static("0af7651916cd43dd8448eb211c80319c"),
        );
        headers.insert(
            headers::B3_SPAN_ID,
            HeaderValue::from_static("b7ad6b7169203331"),
        );
        headers.insert(headers::B3_SAMPLED, HeaderValue::from_static("1"));

        let ctx = extract_trace_context(&headers).unwrap();
        assert_eq!(
            ctx.trace_id.as_deref(),
            Some("0af7651916cd43dd8448eb211c80319c")
        );
        assert!(ctx.is_sampled());
    }

    #[test]
    fn extract_trace_context_b3_single() {
        let mut headers = HeaderMap::new();
        headers.insert(
            headers::B3_SINGLE,
            HeaderValue::from_static("0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-1"),
        );

        let ctx = extract_trace_context(&headers).unwrap();
        assert_eq!(
            ctx.trace_id.as_deref(),
            Some("0af7651916cd43dd8448eb211c80319c")
        );
        assert!(ctx.is_sampled());
    }

    #[test]
    fn extract_trace_context_none() {
        let headers = HeaderMap::new();
        assert!(extract_trace_context(&headers).is_none());
    }

    #[test]
    fn inject_trace_context_headers() {
        let ctx = TraceContext {
            trace_id: Some("0af7651916cd43dd8448eb211c80319c".to_string()),
            parent_span_id: Some("b7ad6b7169203331".to_string()),
            trace_flags: 1,
            trace_state: Some("vendor=value".to_string()),
        };

        let mut headers = HeaderMap::new();
        inject_trace_context(&mut headers, &ctx);

        assert!(headers.contains_key(headers::TRACEPARENT));
        assert!(headers.contains_key(headers::TRACESTATE));
    }

    #[test]
    fn tracing_state_default() {
        let state = TracingState::default();
        assert!(state.config.propagate_context);
    }

    #[test]
    fn tracing_state_new() {
        let config = TracingConfig::default().with_service_name("test");
        let state = TracingState::new(config);
        assert_eq!(state.config.service_name, "test");
    }
}
