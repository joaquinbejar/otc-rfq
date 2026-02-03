//! # REST Routes
//!
//! Route definitions for REST API.
//!
//! This module defines the axum router with all REST API endpoints
//! organized by resource type.
//!
//! # Route Structure
//!
//! ```text
//! /api/v1
//! ├── /health              GET  - Health check
//! ├── /rfqs                GET  - List RFQs
//! │   ├── /                POST - Create RFQ
//! │   └── /{id}            GET  - Get RFQ by ID
//! │       └── /            DELETE - Cancel RFQ
//! ├── /venues              GET  - List venues
//! │   └── /{id}            PUT  - Update venue config
//! └── /trades              GET  - List trades
//!     └── /{id}            GET  - Get trade by ID
//! ```
//!
//! # Examples
//!
//! ```ignore
//! use otc_rfq::api::rest::routes::create_router;
//! use otc_rfq::api::rest::handlers::AppState;
//!
//! let state = AppState { /* ... */ };
//! let router = create_router(state);
//!
//! axum::Server::bind(&addr)
//!     .serve(router.into_make_service())
//!     .await?;
//! ```

use crate::api::rest::handlers::{
    AppState, cancel_rfq, create_rfq, get_rfq, get_trade, health_check, list_rfqs, list_trades,
    list_venues, update_venue,
};
use axum::{Router, routing::get, routing::put};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

/// Creates the REST API router with all endpoints.
///
/// # Arguments
///
/// * `state` - Shared application state containing repositories
///
/// # Returns
///
/// An axum Router configured with all REST endpoints and middleware.
///
/// # Examples
///
/// ```ignore
/// use otc_rfq::api::rest::routes::create_router;
/// use otc_rfq::api::rest::handlers::AppState;
///
/// let state = Arc::new(AppState { /* ... */ });
/// let router = create_router(state);
/// ```
pub fn create_router(state: Arc<AppState>) -> Router {
    // RFQ routes
    let rfq_routes = Router::new()
        .route("/", get(list_rfqs).post(create_rfq))
        .route("/{id}", get(get_rfq).delete(cancel_rfq));

    // Venue routes
    let venue_routes = Router::new()
        .route("/", get(list_venues))
        .route("/{id}", put(update_venue));

    // Trade routes
    let trade_routes = Router::new()
        .route("/", get(list_trades))
        .route("/{id}", get(get_trade));

    // API v1 routes
    let api_v1 = Router::new()
        .route("/health", get(health_check))
        .nest("/rfqs", rfq_routes)
        .nest("/venues", venue_routes)
        .nest("/trades", trade_routes);

    // Main router with middleware
    Router::new()
        .nest("/api/v1", api_v1)
        .layer(TraceLayer::new_for_http())
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .with_state(state)
}

/// Creates a minimal router for testing without middleware.
///
/// This is useful for unit tests where you don't need tracing or CORS.
#[cfg(test)]
pub fn create_test_router(state: Arc<AppState>) -> Router {
    let rfq_routes = Router::new()
        .route("/", get(list_rfqs).post(create_rfq))
        .route("/{id}", get(get_rfq).delete(cancel_rfq));

    let venue_routes = Router::new()
        .route("/", get(list_venues))
        .route("/{id}", put(update_venue));

    let trade_routes = Router::new()
        .route("/", get(list_trades))
        .route("/{id}", get(get_trade));

    let api_v1 = Router::new()
        .route("/health", get(health_check))
        .nest("/rfqs", rfq_routes)
        .nest("/venues", venue_routes)
        .nest("/trades", trade_routes);

    Router::new().nest("/api/v1", api_v1).with_state(state)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::api::rest::handlers::{TradeFilter, TradeRepository, VenueRepository};
    use crate::application::use_cases::create_rfq::RfqRepository;
    use crate::domain::entities::rfq::Rfq;
    use crate::domain::entities::trade::Trade;
    use crate::domain::entities::venue::Venue;
    use crate::domain::value_objects::{RfqId, TradeId, VenueId};
    use async_trait::async_trait;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use std::collections::HashMap;
    use std::sync::RwLock;
    use tower::ServiceExt;

    #[derive(Debug, Default)]
    struct MockRfqRepository {
        rfqs: RwLock<HashMap<RfqId, Rfq>>,
    }

    #[async_trait]
    impl RfqRepository for MockRfqRepository {
        async fn save(&self, rfq: &Rfq) -> Result<(), String> {
            self.rfqs.write().unwrap().insert(rfq.id(), rfq.clone());
            Ok(())
        }

        async fn find_by_id(&self, id: RfqId) -> Result<Option<Rfq>, String> {
            Ok(self.rfqs.read().unwrap().get(&id).cloned())
        }
    }

    #[derive(Debug, Default)]
    struct MockVenueRepository {
        venues: RwLock<HashMap<String, Venue>>,
    }

    #[async_trait]
    impl VenueRepository for MockVenueRepository {
        async fn find_all(&self) -> Result<Vec<Venue>, String> {
            Ok(self.venues.read().unwrap().values().cloned().collect())
        }

        async fn find_by_id(&self, id: &VenueId) -> Result<Option<Venue>, String> {
            Ok(self.venues.read().unwrap().get(id.as_str()).cloned())
        }

        async fn save(&self, venue: &Venue) -> Result<(), String> {
            self.venues
                .write()
                .unwrap()
                .insert(venue.id().to_string(), venue.clone());
            Ok(())
        }
    }

    #[derive(Debug, Default)]
    struct MockTradeRepository {
        trades: RwLock<HashMap<TradeId, Trade>>,
    }

    #[async_trait]
    impl TradeRepository for MockTradeRepository {
        async fn find_all(&self, _filter: &TradeFilter) -> Result<Vec<Trade>, String> {
            Ok(self.trades.read().unwrap().values().cloned().collect())
        }

        async fn find_by_id(&self, id: TradeId) -> Result<Option<Trade>, String> {
            Ok(self.trades.read().unwrap().get(&id).cloned())
        }
    }

    fn create_test_state() -> Arc<AppState> {
        Arc::new(AppState {
            rfq_repository: Arc::new(MockRfqRepository::default()),
            venue_repository: Arc::new(MockVenueRepository::default()),
            trade_repository: Arc::new(MockTradeRepository::default()),
        })
    }

    #[tokio::test]
    async fn health_check_endpoint() {
        let state = create_test_state();
        let router = create_test_router(state);

        let response = router
            .oneshot(
                Request::builder()
                    .uri("/api/v1/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn list_rfqs_endpoint() {
        let state = create_test_state();
        let router = create_test_router(state);

        let response = router
            .oneshot(
                Request::builder()
                    .uri("/api/v1/rfqs")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn get_rfq_not_found() {
        let state = create_test_state();
        let router = create_test_router(state);

        let response = router
            .oneshot(
                Request::builder()
                    .uri("/api/v1/rfqs/550e8400-e29b-41d4-a716-446655440000")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn list_venues_endpoint() {
        let state = create_test_state();
        let router = create_test_router(state);

        let response = router
            .oneshot(
                Request::builder()
                    .uri("/api/v1/venues")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn list_trades_endpoint() {
        let state = create_test_state();
        let router = create_test_router(state);

        let response = router
            .oneshot(
                Request::builder()
                    .uri("/api/v1/trades")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn get_trade_not_found() {
        let state = create_test_state();
        let router = create_test_router(state);

        let response = router
            .oneshot(
                Request::builder()
                    .uri("/api/v1/trades/550e8400-e29b-41d4-a716-446655440000")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn create_rfq_endpoint() {
        let state = create_test_state();
        let router = create_test_router(state);

        let body = serde_json::json!({
            "client_id": "client-123",
            "base_asset": "BTC",
            "quote_asset": "USD",
            "side": "BUY",
            "quantity": 1.5,
            "expiry_seconds": 300
        });

        let response = router
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/rfqs")
                    .header("Content-Type", "application/json")
                    .body(Body::from(serde_json::to_string(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);
    }

    #[tokio::test]
    async fn create_rfq_validation_error() {
        let state = create_test_state();
        let router = create_test_router(state);

        let body = serde_json::json!({
            "client_id": "",
            "base_asset": "BTC",
            "quote_asset": "USD",
            "side": "BUY",
            "quantity": 1.5,
            "expiry_seconds": 300
        });

        let response = router
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/rfqs")
                    .header("Content-Type", "application/json")
                    .body(Body::from(serde_json::to_string(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}
