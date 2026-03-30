//! # SBE Request Handlers
//!
//! Handler functions for SBE request messages.
//!
//! These handlers bridge the SBE API layer with the application use cases,
//! performing validation, domain conversion, and response encoding.

// String slicing is validated before use
#![allow(clippy::indexing_slicing)]

use crate::api::sbe::error::{SbeApiError, SbeApiResult};
use crate::api::sbe::server::AppState;
use crate::api::sbe::types::*;
use crate::domain::entities::rfq::RfqBuilder;
use crate::domain::value_objects::enums::{SettlementMethod, VenueType};
use crate::domain::value_objects::timestamp::Timestamp;
use crate::domain::value_objects::{CounterpartyId, Instrument, RfqId, Symbol};

/// Handles CreateRfq request.
///
/// # Errors
///
/// Returns error if validation fails or persistence fails.
pub async fn handle_create_rfq(
    request: CreateRfqRequest,
    state: &AppState,
) -> SbeApiResult<CreateRfqResponse> {
    // Validate client_id
    if request.client_id.is_empty() {
        return Err(SbeApiError::InvalidValue {
            field: "client_id",
            message: "cannot be empty".to_string(),
        });
    }

    // Validate symbol
    if request.symbol.is_empty() {
        return Err(SbeApiError::InvalidValue {
            field: "symbol",
            message: "cannot be empty".to_string(),
        });
    }

    // Create domain types
    let client_id = CounterpartyId::new(&request.client_id);
    let symbol = Symbol::new(&request.symbol).map_err(|e| SbeApiError::InvalidValue {
        field: "symbol",
        message: e.to_string(),
    })?;

    let instrument = Instrument::new(symbol, request.asset_class, SettlementMethod::default());

    let quantity = request.quantity;
    let side = request.side;

    // Calculate expiration
    let expires_at = Timestamp::now().add_secs(request.timeout_seconds);

    // Build RFQ
    let rfq = RfqBuilder::new(client_id, instrument, side, quantity, expires_at).build();

    // Persist
    state
        .rfq_repository
        .save(&rfq)
        .await
        .map_err(|e| SbeApiError::Domain(format!("failed to save RFQ: {}", e)))?;

    // Build response with empty quotes
    Ok(CreateRfqResponse {
        request_id: request.request_id,
        rfq_id: rfq.id().get(),
        side: rfq.side(),
        quantity: rfq.quantity(),
        state: rfq.state(),
        expires_at: rfq.expires_at(),
        created_at: rfq.created_at(),
        updated_at: rfq.updated_at(),
        asset_class: rfq.instrument().asset_class(),
        selected_quote_id: uuid::Uuid::nil(),
        quotes: Vec::new(),
        client_id: rfq.client_id().to_string(),
        symbol: rfq.instrument().symbol().to_string(),
        base_asset: request.base_asset,
        quote_asset: request.quote_asset,
    })
}

/// Handles GetRfq request.
///
/// # Errors
///
/// Returns error if RFQ not found or retrieval fails.
pub async fn handle_get_rfq(
    request: GetRfqRequest,
    state: &AppState,
) -> SbeApiResult<GetRfqResponse> {
    let rfq_id = RfqId::new(request.rfq_id);

    // Retrieve RFQ
    let rfq = state
        .rfq_repository
        .find_by_id(rfq_id)
        .await
        .map_err(|e| SbeApiError::Domain(format!("failed to find RFQ: {}", e)))?
        .ok_or_else(|| SbeApiError::InvalidValue {
            field: "rfq_id",
            message: format!("RFQ not found: {}", rfq_id),
        })?;

    // Extract symbol parts (assuming format "BASE/QUOTE")
    let symbol_str = rfq.instrument().symbol().to_string();
    let (base_asset, quote_asset) = match symbol_str.split_once('/') {
        Some((base, quote)) => (base.to_string(), quote.to_string()),
        None => (symbol_str.clone(), String::new()),
    };

    // Convert quotes to SBE format
    let quotes = rfq
        .quotes()
        .iter()
        .map(|q| RfqQuoteEntry {
            quote_id: q.id().get(),
            price: q.price(),
            quantity: q.quantity(),
            commission: rust_decimal::Decimal::ZERO,
            valid_until: q.valid_until(),
            created_at: q.created_at(),
            venue_type: VenueType::InternalMM,
            venue_id: q.venue_id().to_string(),
        })
        .collect();

    Ok(GetRfqResponse {
        request_id: request.request_id,
        rfq_id: rfq.id().get(),
        side: rfq.side(),
        quantity: rfq.quantity(),
        state: rfq.state(),
        expires_at: rfq.expires_at(),
        created_at: rfq.created_at(),
        updated_at: rfq.updated_at(),
        asset_class: rfq.instrument().asset_class(),
        selected_quote_id: rfq
            .selected_quote_id()
            .map(|id| id.get())
            .unwrap_or_else(uuid::Uuid::nil),
        quotes,
        client_id: rfq.client_id().to_string(),
        symbol: symbol_str,
        base_asset,
        quote_asset,
    })
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::useless_vec,
    clippy::clone_on_ref_ptr
)]
mod tests {
    use super::*;
    use crate::domain::entities::rfq::Rfq;
    use crate::domain::value_objects::enums::AssetClass;
    use crate::domain::value_objects::{OrderSide, Quantity};
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::RwLock;
    use uuid::Uuid;

    #[derive(Debug)]
    struct MockRfqRepository {
        rfqs: RwLock<HashMap<RfqId, Rfq>>,
    }

    impl MockRfqRepository {
        fn new() -> Self {
            Self {
                rfqs: RwLock::new(HashMap::new()),
            }
        }

        fn with_rfq(rfq: Rfq) -> Self {
            let mut map = HashMap::new();
            map.insert(rfq.id(), rfq);
            Self {
                rfqs: RwLock::new(map),
            }
        }
    }

    #[async_trait::async_trait]
    impl crate::application::use_cases::create_rfq::RfqRepository for MockRfqRepository {
        async fn save(&self, rfq: &Rfq) -> Result<(), String> {
            self.rfqs.write().await.insert(rfq.id(), rfq.clone());
            Ok(())
        }

        async fn find_by_id(&self, id: RfqId) -> Result<Option<Rfq>, String> {
            Ok(self.rfqs.read().await.get(&id).cloned())
        }
    }

    fn create_test_state() -> AppState {
        AppState {
            rfq_repository: Arc::new(MockRfqRepository::new()),
        }
    }

    #[tokio::test]
    async fn handle_create_rfq_success() {
        let state = create_test_state();

        let request = CreateRfqRequest {
            request_id: Uuid::new_v4(),
            client_id: "client-1".to_string(),
            symbol: "BTC/USD".to_string(),
            base_asset: "BTC".to_string(),
            quote_asset: "USD".to_string(),
            side: OrderSide::Buy,
            quantity: Quantity::from_decimal(rust_decimal::Decimal::new(1, 0)).unwrap(),
            timeout_seconds: 300,
            asset_class: AssetClass::CryptoSpot,
        };

        let result = handle_create_rfq(request.clone(), &state).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        assert_eq!(response.request_id, request.request_id);
        assert_eq!(response.client_id, "client-1");
        assert_eq!(response.symbol, "BTC/USD");
        assert_eq!(response.side, OrderSide::Buy);
        assert!(response.quotes.is_empty());
    }

    #[tokio::test]
    async fn handle_create_rfq_empty_client_id() {
        let state = create_test_state();

        let request = CreateRfqRequest {
            request_id: Uuid::new_v4(),
            client_id: String::new(),
            symbol: "BTC/USD".to_string(),
            base_asset: "BTC".to_string(),
            quote_asset: "USD".to_string(),
            side: OrderSide::Buy,
            quantity: Quantity::from_decimal(rust_decimal::Decimal::new(1, 0)).unwrap(),
            timeout_seconds: 300,
            asset_class: AssetClass::CryptoSpot,
        };

        let result = handle_create_rfq(request, &state).await;
        assert!(matches!(
            result,
            Err(SbeApiError::InvalidValue {
                field: "client_id",
                ..
            })
        ));
    }

    #[tokio::test]
    async fn handle_create_rfq_empty_symbol() {
        let state = create_test_state();

        let request = CreateRfqRequest {
            request_id: Uuid::new_v4(),
            client_id: "client-1".to_string(),
            symbol: String::new(),
            base_asset: "BTC".to_string(),
            quote_asset: "USD".to_string(),
            side: OrderSide::Buy,
            quantity: Quantity::from_decimal(rust_decimal::Decimal::new(1, 0)).unwrap(),
            timeout_seconds: 300,
            asset_class: AssetClass::CryptoSpot,
        };

        let result = handle_create_rfq(request, &state).await;
        assert!(matches!(
            result,
            Err(SbeApiError::InvalidValue {
                field: "symbol",
                ..
            })
        ));
    }

    #[tokio::test]
    async fn handle_get_rfq_not_found() {
        let state = create_test_state();

        let request = GetRfqRequest {
            request_id: Uuid::new_v4(),
            rfq_id: Uuid::new_v4(),
        };

        let result = handle_get_rfq(request, &state).await;
        assert!(matches!(
            result,
            Err(SbeApiError::InvalidValue {
                field: "rfq_id",
                ..
            })
        ));
    }

    #[tokio::test]
    async fn handle_get_rfq_success() {
        use crate::domain::entities::rfq::RfqBuilder;
        use crate::domain::value_objects::timestamp::Timestamp;

        let symbol = Symbol::new("ETH/USD").unwrap();
        let instrument =
            Instrument::new(symbol, AssetClass::CryptoSpot, SettlementMethod::default());
        let quantity = Quantity::from_decimal(rust_decimal::Decimal::new(5, 0)).unwrap();
        let expires_at = Timestamp::now().add_secs(600);

        let rfq = RfqBuilder::new(
            CounterpartyId::new("client-2"),
            instrument,
            OrderSide::Sell,
            quantity,
            expires_at,
        )
        .build();

        let rfq_id = rfq.id();
        let state = AppState {
            rfq_repository: Arc::new(MockRfqRepository::with_rfq(rfq)),
        };

        let request = GetRfqRequest {
            request_id: Uuid::new_v4(),
            rfq_id: rfq_id.get(),
        };

        let result = handle_get_rfq(request.clone(), &state).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        assert_eq!(response.request_id, request.request_id);
        assert_eq!(response.rfq_id, rfq_id.get());
        assert_eq!(response.client_id, "client-2");
        assert_eq!(response.symbol, "ETH/USD");
        assert_eq!(response.side, OrderSide::Sell);
    }
}
