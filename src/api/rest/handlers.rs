//! # REST Handlers
//!
//! Request handlers for REST endpoints.
//!
//! This module provides axum handlers for all REST API endpoints,
//! including RFQ management, venue configuration, and trade queries.
//!
//! # Endpoints
//!
//! ## RFQs
//! - `GET /api/v1/rfqs` - List RFQs with filtering
//! - `GET /api/v1/rfqs/{id}` - Get RFQ by ID
//! - `POST /api/v1/rfqs` - Create RFQ
//! - `DELETE /api/v1/rfqs/{id}` - Cancel RFQ
//!
//! ## Venues
//! - `GET /api/v1/venues` - List venues
//! - `PUT /api/v1/venues/{id}` - Update venue config
//!
//! ## Trades
//! - `GET /api/v1/trades` - List trades
//! - `GET /api/v1/trades/{id}` - Get trade by ID

use crate::application::error::ApplicationError;
use crate::application::use_cases::create_rfq::RfqRepository;
use crate::domain::entities::rfq::Rfq;
use crate::domain::entities::trade::Trade;
use crate::domain::entities::venue::{Venue, VenueHealth};
use crate::domain::value_objects::timestamp::Timestamp;
use crate::domain::value_objects::{
    CounterpartyId, Instrument, OrderSide, Quantity, RfqId, RfqState, TradeId, VenueId, VenueType,
};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument, warn};

// ============================================================================
// Application State
// ============================================================================

/// Shared application state for REST handlers.
#[derive(Debug, Clone)]
pub struct AppState {
    /// RFQ repository.
    pub rfq_repository: Arc<dyn RfqRepository>,
    /// Venue repository.
    pub venue_repository: Arc<dyn VenueRepository>,
    /// Trade repository.
    pub trade_repository: Arc<dyn TradeRepository>,
}

/// Repository for venue persistence.
#[async_trait::async_trait]
pub trait VenueRepository: Send + Sync + std::fmt::Debug {
    /// Returns all venues.
    async fn find_all(&self) -> Result<Vec<Venue>, String>;

    /// Finds a venue by ID.
    async fn find_by_id(&self, id: &VenueId) -> Result<Option<Venue>, String>;

    /// Saves a venue.
    async fn save(&self, venue: &Venue) -> Result<(), String>;
}

/// Repository for trade persistence.
#[async_trait::async_trait]
pub trait TradeRepository: Send + Sync + std::fmt::Debug {
    /// Returns all trades with optional filtering.
    async fn find_all(&self, filter: &TradeFilter) -> Result<Vec<Trade>, String>;

    /// Finds a trade by ID.
    async fn find_by_id(&self, id: TradeId) -> Result<Option<Trade>, String>;
}

// ============================================================================
// Error Response
// ============================================================================

/// Standard error response format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    /// Error code.
    pub code: String,
    /// Human-readable error message.
    pub message: String,
    /// Additional error details.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl ErrorResponse {
    /// Creates a new error response.
    #[must_use]
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details: None,
        }
    }

    /// Creates an error response with details.
    #[must_use]
    pub fn with_details(
        code: impl Into<String>,
        message: impl Into<String>,
        details: serde_json::Value,
    ) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details: Some(details),
        }
    }
}

impl From<ApplicationError> for (StatusCode, Json<ErrorResponse>) {
    fn from(err: ApplicationError) -> Self {
        let (status, code) = match &err {
            ApplicationError::Validation(_) => (StatusCode::BAD_REQUEST, "VALIDATION_ERROR"),
            ApplicationError::NotFound { .. }
            | ApplicationError::ClientNotFound(_)
            | ApplicationError::RfqNotFound(_)
            | ApplicationError::QuoteNotFound(_) => (StatusCode::NOT_FOUND, "NOT_FOUND"),
            ApplicationError::Unauthorized => (StatusCode::UNAUTHORIZED, "UNAUTHORIZED"),
            ApplicationError::QuoteExpired(_) | ApplicationError::InvalidState(_) => {
                (StatusCode::CONFLICT, "CONFLICT")
            }
            ApplicationError::ComplianceFailed(_) => (StatusCode::FORBIDDEN, "FORBIDDEN"),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR"),
        };

        (status, Json(ErrorResponse::new(code, err.to_string())))
    }
}

// ============================================================================
// Pagination
// ============================================================================

/// Pagination parameters.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct PaginationParams {
    /// Page number (1-indexed).
    #[serde(default = "default_page")]
    pub page: u32,
    /// Items per page.
    #[serde(default = "default_page_size")]
    pub page_size: u32,
}

fn default_page() -> u32 {
    1
}

fn default_page_size() -> u32 {
    20
}

impl PaginationParams {
    /// Returns the offset for database queries.
    #[must_use]
    pub fn offset(&self) -> u32 {
        (self.page.saturating_sub(1)) * self.page_size
    }

    /// Returns the limit for database queries.
    #[must_use]
    pub fn limit(&self) -> u32 {
        self.page_size.min(100)
    }
}

/// Paginated response wrapper.
#[derive(Debug, Clone, Serialize)]
pub struct PaginatedResponse<T> {
    /// The data items.
    pub data: Vec<T>,
    /// Pagination metadata.
    pub pagination: PaginationMeta,
}

/// Pagination metadata.
#[derive(Debug, Clone, Serialize)]
pub struct PaginationMeta {
    /// Current page number.
    pub page: u32,
    /// Items per page.
    pub page_size: u32,
    /// Total number of items.
    pub total_items: u64,
    /// Total number of pages.
    pub total_pages: u32,
}

impl PaginationMeta {
    /// Creates pagination metadata.
    #[must_use]
    pub fn new(page: u32, page_size: u32, total_items: u64) -> Self {
        let total_pages = if total_items == 0 {
            1
        } else {
            ((total_items as f64) / (page_size as f64)).ceil() as u32
        };

        Self {
            page,
            page_size,
            total_items,
            total_pages,
        }
    }
}

// ============================================================================
// RFQ DTOs
// ============================================================================

/// Request to create a new RFQ.
#[derive(Debug, Clone, Deserialize)]
pub struct CreateRfqRequest {
    /// The client ID requesting the quote.
    pub client_id: String,
    /// Base asset symbol (e.g., "BTC").
    pub base_asset: String,
    /// Quote asset symbol (e.g., "USD").
    pub quote_asset: String,
    /// Buy or sell side.
    pub side: OrderSide,
    /// Requested quantity.
    pub quantity: f64,
    /// Expiry duration in seconds from now.
    pub expiry_seconds: u64,
}

/// RFQ filter parameters.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct RfqFilter {
    /// Filter by client ID.
    pub client_id: Option<String>,
    /// Filter by state.
    pub state: Option<RfqState>,
    /// Filter by base asset.
    pub base_asset: Option<String>,
    /// Filter by quote asset.
    pub quote_asset: Option<String>,
}

/// RFQ response DTO.
#[derive(Debug, Clone, Serialize)]
pub struct RfqResponse {
    /// RFQ ID.
    pub id: String,
    /// Client ID.
    pub client_id: String,
    /// Instrument symbol.
    pub symbol: String,
    /// Order side.
    pub side: OrderSide,
    /// Quantity.
    pub quantity: String,
    /// Current state.
    pub state: RfqState,
    /// Expiry timestamp (ISO 8601).
    pub expires_at: String,
    /// Number of quotes received.
    pub quote_count: usize,
    /// Created timestamp (ISO 8601).
    pub created_at: String,
    /// Updated timestamp (ISO 8601).
    pub updated_at: String,
}

impl From<&Rfq> for RfqResponse {
    fn from(rfq: &Rfq) -> Self {
        Self {
            id: rfq.id().to_string(),
            client_id: rfq.client_id().to_string(),
            symbol: rfq.instrument().symbol().to_string(),
            side: rfq.side(),
            quantity: rfq.quantity().to_string(),
            state: rfq.state(),
            expires_at: rfq.expires_at().to_string(),
            quote_count: rfq.quotes().len(),
            created_at: rfq.created_at().to_string(),
            updated_at: rfq.updated_at().to_string(),
        }
    }
}

// ============================================================================
// Venue DTOs
// ============================================================================

/// Venue response DTO.
#[derive(Debug, Clone, Serialize)]
pub struct VenueResponse {
    /// Venue ID.
    pub id: String,
    /// Venue name.
    pub name: String,
    /// Venue type.
    pub venue_type: VenueType,
    /// Whether the venue is enabled.
    pub enabled: bool,
    /// Health status.
    pub health: VenueHealth,
    /// Supported instruments count.
    pub supported_instruments: usize,
}

impl From<&Venue> for VenueResponse {
    fn from(venue: &Venue) -> Self {
        Self {
            id: venue.id().to_string(),
            name: venue.name().to_string(),
            venue_type: venue.venue_type(),
            enabled: venue.is_enabled(),
            health: venue.health(),
            supported_instruments: venue.supported_instruments().len(),
        }
    }
}

/// Request to update venue configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateVenueRequest {
    /// Whether the venue is enabled.
    pub enabled: Option<bool>,
    /// Priority (lower is higher priority).
    pub priority: Option<u32>,
}

// ============================================================================
// Trade DTOs
// ============================================================================

/// Trade filter parameters.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct TradeFilter {
    /// Filter by RFQ ID.
    pub rfq_id: Option<String>,
    /// Filter by venue ID.
    pub venue_id: Option<String>,
}

/// Trade response DTO.
#[derive(Debug, Clone, Serialize)]
pub struct TradeResponse {
    /// Trade ID.
    pub id: String,
    /// RFQ ID.
    pub rfq_id: String,
    /// Quote ID.
    pub quote_id: String,
    /// Venue ID.
    pub venue_id: String,
    /// Execution price.
    pub price: String,
    /// Executed quantity.
    pub quantity: String,
    /// Settlement state.
    pub settlement_state: String,
    /// Created timestamp (ISO 8601).
    pub created_at: String,
}

impl From<&Trade> for TradeResponse {
    fn from(trade: &Trade) -> Self {
        Self {
            id: trade.id().to_string(),
            rfq_id: trade.rfq_id().to_string(),
            quote_id: trade.quote_id().to_string(),
            venue_id: trade.venue_id().to_string(),
            price: trade.price().to_string(),
            quantity: trade.quantity().to_string(),
            settlement_state: trade.settlement_state().to_string(),
            created_at: trade.created_at().to_string(),
        }
    }
}

// ============================================================================
// RFQ Handlers
// ============================================================================

/// List RFQs with filtering and pagination.
///
/// # Errors
///
/// Returns an error response if the repository query fails.
#[instrument(skip(state))]
pub async fn list_rfqs(
    State(state): State<Arc<AppState>>,
    Query(pagination): Query<PaginationParams>,
    Query(filter): Query<RfqFilter>,
) -> Result<Json<PaginatedResponse<RfqResponse>>, (StatusCode, Json<ErrorResponse>)> {
    info!("Listing RFQs with filter: {:?}", filter);

    // For now, we'll fetch all and filter in memory
    // In production, this would be done at the database level
    let all_rfqs = fetch_all_rfqs(&state.rfq_repository).await?;

    let filtered: Vec<_> = all_rfqs
        .iter()
        .filter(|rfq| {
            filter
                .client_id
                .as_ref()
                .is_none_or(|id| rfq.client_id().as_str() == id)
        })
        .filter(|rfq| filter.state.is_none_or(|s| rfq.state() == s))
        .filter(|rfq| {
            filter
                .base_asset
                .as_ref()
                .is_none_or(|a| rfq.instrument().symbol().base_asset() == a)
        })
        .filter(|rfq| {
            filter
                .quote_asset
                .as_ref()
                .is_none_or(|a| rfq.instrument().symbol().quote_asset() == a)
        })
        .collect();

    let total_items = filtered.len() as u64;
    let offset = pagination.offset() as usize;
    let limit = pagination.limit() as usize;

    let page_data: Vec<RfqResponse> = filtered
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(RfqResponse::from)
        .collect();

    Ok(Json(PaginatedResponse {
        data: page_data,
        pagination: PaginationMeta::new(pagination.page, pagination.limit(), total_items),
    }))
}

/// Get RFQ by ID.
///
/// # Errors
///
/// Returns `NOT_FOUND` if the RFQ does not exist.
/// Returns `VALIDATION_ERROR` if the ID is not a valid UUID.
#[instrument(skip(state))]
pub async fn get_rfq(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<RfqResponse>, (StatusCode, Json<ErrorResponse>)> {
    info!("Getting RFQ: {}", id);

    let rfq_id = parse_rfq_id(&id)?;

    let rfq = state
        .rfq_repository
        .find_by_id(rfq_id)
        .await
        .map_err(|e| {
            error!("Failed to find RFQ: {}", e);
            internal_error(&e)
        })?
        .ok_or_else(|| not_found("RFQ", &id))?;

    Ok(Json(RfqResponse::from(&rfq)))
}

/// Create a new RFQ.
///
/// # Errors
///
/// Returns `VALIDATION_ERROR` if the request is invalid.
/// Returns `INTERNAL_ERROR` if the repository save fails.
#[instrument(skip(state, request))]
pub async fn create_rfq(
    State(state): State<Arc<AppState>>,
    Json(request): Json<CreateRfqRequest>,
) -> Result<(StatusCode, Json<RfqResponse>), (StatusCode, Json<ErrorResponse>)> {
    info!("Creating RFQ for client: {}", request.client_id);

    // Validate request
    validate_create_rfq_request(&request)?;

    // Build instrument
    let symbol_str = format!("{}/{}", request.base_asset, request.quote_asset);
    let symbol = crate::domain::value_objects::Symbol::new(&symbol_str)
        .map_err(|e| validation_error(&format!("invalid symbol: {e}")))?;

    let instrument = Instrument::builder(
        symbol,
        crate::domain::value_objects::enums::AssetClass::CryptoSpot,
    )
    .build();

    // Build quantity
    let quantity = Quantity::new(request.quantity)
        .map_err(|e| validation_error(&format!("invalid quantity: {e}")))?;

    // Build expiry
    let expires_at = Timestamp::now().add_secs(request.expiry_seconds as i64);

    // Create RFQ
    let rfq = crate::domain::entities::rfq::RfqBuilder::new(
        CounterpartyId::new(&request.client_id),
        instrument,
        request.side,
        quantity,
        expires_at,
    )
    .build();

    // Save to repository
    state.rfq_repository.save(&rfq).await.map_err(|e| {
        error!("Failed to save RFQ: {}", e);
        internal_error(&e)
    })?;

    info!("Created RFQ: {}", rfq.id());

    Ok((StatusCode::CREATED, Json(RfqResponse::from(&rfq))))
}

/// Cancel an RFQ.
///
/// # Errors
///
/// Returns `NOT_FOUND` if the RFQ does not exist.
/// Returns `CONFLICT` if the RFQ cannot be cancelled.
#[instrument(skip(state))]
pub async fn cancel_rfq(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<RfqResponse>, (StatusCode, Json<ErrorResponse>)> {
    info!("Cancelling RFQ: {}", id);

    let rfq_id = parse_rfq_id(&id)?;

    let mut rfq = state
        .rfq_repository
        .find_by_id(rfq_id)
        .await
        .map_err(|e| {
            error!("Failed to find RFQ: {}", e);
            internal_error(&e)
        })?
        .ok_or_else(|| not_found("RFQ", &id))?;

    rfq.cancel().map_err(|e| {
        warn!("Cannot cancel RFQ: {}", e);
        conflict_error(&format!("cannot cancel RFQ: {e}"))
    })?;

    state.rfq_repository.save(&rfq).await.map_err(|e| {
        error!("Failed to save cancelled RFQ: {}", e);
        internal_error(&e)
    })?;

    info!("Cancelled RFQ: {}", id);

    Ok(Json(RfqResponse::from(&rfq)))
}

// ============================================================================
// Venue Handlers
// ============================================================================

/// List all venues.
///
/// # Errors
///
/// Returns an error response if the repository query fails.
#[instrument(skip(state))]
pub async fn list_venues(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<VenueResponse>>, (StatusCode, Json<ErrorResponse>)> {
    info!("Listing venues");

    let venues = state.venue_repository.find_all().await.map_err(|e| {
        error!("Failed to list venues: {}", e);
        internal_error(&e)
    })?;

    let responses: Vec<VenueResponse> = venues.iter().map(VenueResponse::from).collect();

    Ok(Json(responses))
}

/// Update venue configuration.
///
/// # Errors
///
/// Returns `NOT_FOUND` if the venue does not exist.
/// Returns `INTERNAL_ERROR` if the repository save fails.
#[instrument(skip(state, request))]
pub async fn update_venue(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(request): Json<UpdateVenueRequest>,
) -> Result<Json<VenueResponse>, (StatusCode, Json<ErrorResponse>)> {
    info!("Updating venue: {}", id);

    let venue_id = VenueId::new(&id);

    let mut venue = state
        .venue_repository
        .find_by_id(&venue_id)
        .await
        .map_err(|e| {
            error!("Failed to find venue: {}", e);
            internal_error(&e)
        })?
        .ok_or_else(|| not_found("Venue", &id))?;

    // Apply updates
    if let Some(enabled) = request.enabled {
        venue.set_enabled(enabled);
    }

    // Note: priority update would require adding set_priority to Venue
    // For now, we ignore the priority field
    let _ = request.priority;

    // Save updated venue
    state.venue_repository.save(&venue).await.map_err(|e| {
        error!("Failed to save venue: {}", e);
        internal_error(&e)
    })?;

    info!("Updated venue: {}", id);

    Ok(Json(VenueResponse::from(&venue)))
}

// ============================================================================
// Trade Handlers
// ============================================================================

/// List trades with filtering and pagination.
///
/// # Errors
///
/// Returns an error response if the repository query fails.
#[instrument(skip(state))]
pub async fn list_trades(
    State(state): State<Arc<AppState>>,
    Query(pagination): Query<PaginationParams>,
    Query(filter): Query<TradeFilter>,
) -> Result<Json<PaginatedResponse<TradeResponse>>, (StatusCode, Json<ErrorResponse>)> {
    info!("Listing trades with filter: {:?}", filter);

    let trades = state
        .trade_repository
        .find_all(&filter)
        .await
        .map_err(|e| {
            error!("Failed to list trades: {}", e);
            internal_error(&e)
        })?;

    let total_items = trades.len() as u64;
    let offset = pagination.offset() as usize;
    let limit = pagination.limit() as usize;

    let page_data: Vec<TradeResponse> = trades
        .iter()
        .skip(offset)
        .take(limit)
        .map(TradeResponse::from)
        .collect();

    Ok(Json(PaginatedResponse {
        data: page_data,
        pagination: PaginationMeta::new(pagination.page, pagination.limit(), total_items),
    }))
}

/// Get trade by ID.
///
/// # Errors
///
/// Returns `NOT_FOUND` if the trade does not exist.
/// Returns `VALIDATION_ERROR` if the ID is not a valid UUID.
#[instrument(skip(state))]
pub async fn get_trade(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<TradeResponse>, (StatusCode, Json<ErrorResponse>)> {
    info!("Getting trade: {}", id);

    let trade_id = parse_trade_id(&id)?;

    let trade = state
        .trade_repository
        .find_by_id(trade_id)
        .await
        .map_err(|e| {
            error!("Failed to find trade: {}", e);
            internal_error(&e)
        })?
        .ok_or_else(|| not_found("Trade", &id))?;

    Ok(Json(TradeResponse::from(&trade)))
}

// ============================================================================
// Health Check
// ============================================================================

/// Health check response.
#[derive(Debug, Clone, Serialize)]
pub struct HealthResponse {
    /// Service status.
    pub status: String,
    /// Service version.
    pub version: String,
}

/// Health check endpoint.
pub async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

// ============================================================================
// Helper Functions
// ============================================================================

fn parse_rfq_id(id: &str) -> Result<RfqId, (StatusCode, Json<ErrorResponse>)> {
    uuid::Uuid::parse_str(id)
        .map(RfqId::from)
        .map_err(|_| validation_error(&format!("invalid RFQ ID: {id}")))
}

fn parse_trade_id(id: &str) -> Result<TradeId, (StatusCode, Json<ErrorResponse>)> {
    uuid::Uuid::parse_str(id)
        .map(TradeId::from)
        .map_err(|_| validation_error(&format!("invalid Trade ID: {id}")))
}

fn validate_create_rfq_request(
    request: &CreateRfqRequest,
) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
    if request.client_id.is_empty() {
        return Err(validation_error("client_id cannot be empty"));
    }
    if request.base_asset.is_empty() {
        return Err(validation_error("base_asset cannot be empty"));
    }
    if request.quote_asset.is_empty() {
        return Err(validation_error("quote_asset cannot be empty"));
    }
    if request.quantity <= 0.0 {
        return Err(validation_error("quantity must be positive"));
    }
    if request.expiry_seconds == 0 {
        return Err(validation_error("expiry_seconds must be greater than 0"));
    }
    Ok(())
}

async fn fetch_all_rfqs(
    _repo: &Arc<dyn RfqRepository>,
) -> Result<Vec<Rfq>, (StatusCode, Json<ErrorResponse>)> {
    // This is a placeholder - in production, the repository would have a find_all method
    // For now, we return an empty list since RfqRepository only has find_by_id
    Ok(vec![])
}

fn validation_error(message: &str) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse::new("VALIDATION_ERROR", message)),
    )
}

fn not_found(resource: &str, id: &str) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::NOT_FOUND,
        Json(ErrorResponse::new(
            "NOT_FOUND",
            format!("{resource} not found: {id}"),
        )),
    )
}

fn conflict_error(message: &str) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::CONFLICT,
        Json(ErrorResponse::new("CONFLICT", message)),
    )
}

fn internal_error(message: &str) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse::new("INTERNAL_ERROR", message)),
    )
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn error_response_new() {
        let err = ErrorResponse::new("TEST_ERROR", "test message");
        assert_eq!(err.code, "TEST_ERROR");
        assert_eq!(err.message, "test message");
        assert!(err.details.is_none());
    }

    #[test]
    fn error_response_with_details() {
        let details = serde_json::json!({"field": "quantity"});
        let err = ErrorResponse::with_details("VALIDATION_ERROR", "invalid field", details.clone());
        assert_eq!(err.code, "VALIDATION_ERROR");
        assert_eq!(err.details, Some(details));
    }

    #[test]
    fn pagination_params_defaults() {
        // Default derive sets fields to 0, but serde defaults use default_page/default_page_size
        let params = PaginationParams::default();
        assert_eq!(params.page, 0);
        assert_eq!(params.page_size, 0);
    }

    #[test]
    fn pagination_params_serde_defaults() {
        // When deserializing with missing fields, serde uses the default functions
        let params: PaginationParams = serde_json::from_str("{}").unwrap();
        assert_eq!(params.page, 1);
        assert_eq!(params.page_size, 20);
    }

    #[test]
    fn pagination_params_offset() {
        let params = PaginationParams {
            page: 3,
            page_size: 10,
        };
        assert_eq!(params.offset(), 20);
    }

    #[test]
    fn pagination_params_offset_page_one() {
        let params = PaginationParams {
            page: 1,
            page_size: 10,
        };
        assert_eq!(params.offset(), 0);
    }

    #[test]
    fn pagination_params_limit_capped() {
        let params = PaginationParams {
            page: 1,
            page_size: 500,
        };
        assert_eq!(params.limit(), 100);
    }

    #[test]
    fn pagination_meta_new() {
        let meta = PaginationMeta::new(2, 10, 45);
        assert_eq!(meta.page, 2);
        assert_eq!(meta.page_size, 10);
        assert_eq!(meta.total_items, 45);
        assert_eq!(meta.total_pages, 5);
    }

    #[test]
    fn pagination_meta_empty() {
        let meta = PaginationMeta::new(1, 10, 0);
        assert_eq!(meta.total_pages, 1);
    }

    #[test]
    fn parse_rfq_id_valid() {
        let id = "550e8400-e29b-41d4-a716-446655440000";
        let result = parse_rfq_id(id);
        assert!(result.is_ok());
    }

    #[test]
    fn parse_rfq_id_invalid() {
        let id = "not-a-uuid";
        let result = parse_rfq_id(id);
        assert!(result.is_err());
    }

    #[test]
    fn parse_trade_id_valid() {
        let id = "550e8400-e29b-41d4-a716-446655440000";
        let result = parse_trade_id(id);
        assert!(result.is_ok());
    }

    #[test]
    fn validate_create_rfq_request_valid() {
        let request = CreateRfqRequest {
            client_id: "client-1".to_string(),
            base_asset: "BTC".to_string(),
            quote_asset: "USD".to_string(),
            side: OrderSide::Buy,
            quantity: 1.0,
            expiry_seconds: 300,
        };
        assert!(validate_create_rfq_request(&request).is_ok());
    }

    #[test]
    fn validate_create_rfq_request_empty_client_id() {
        let request = CreateRfqRequest {
            client_id: String::new(),
            base_asset: "BTC".to_string(),
            quote_asset: "USD".to_string(),
            side: OrderSide::Buy,
            quantity: 1.0,
            expiry_seconds: 300,
        };
        assert!(validate_create_rfq_request(&request).is_err());
    }

    #[test]
    fn validate_create_rfq_request_zero_quantity() {
        let request = CreateRfqRequest {
            client_id: "client-1".to_string(),
            base_asset: "BTC".to_string(),
            quote_asset: "USD".to_string(),
            side: OrderSide::Buy,
            quantity: 0.0,
            expiry_seconds: 300,
        };
        assert!(validate_create_rfq_request(&request).is_err());
    }

    #[test]
    fn validate_create_rfq_request_zero_expiry() {
        let request = CreateRfqRequest {
            client_id: "client-1".to_string(),
            base_asset: "BTC".to_string(),
            quote_asset: "USD".to_string(),
            side: OrderSide::Buy,
            quantity: 1.0,
            expiry_seconds: 0,
        };
        assert!(validate_create_rfq_request(&request).is_err());
    }

    #[tokio::test]
    async fn health_check_returns_healthy() {
        let response = health_check().await;
        assert_eq!(response.status, "healthy");
    }
}
