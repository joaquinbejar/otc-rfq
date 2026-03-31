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
use crate::domain::entities::rfq::{Rfq, RfqBuilder};
use crate::domain::entities::trade::Trade;
use crate::domain::value_objects::enums::{SettlementMethod, VenueType};
use crate::domain::value_objects::timestamp::Timestamp;
use crate::domain::value_objects::{CounterpartyId, Instrument, Symbol};

/// Builds an [`RfqResponse`] from a domain [`Rfq`].
///
/// Extracts all fields from the RFQ entity including quotes.
/// Used by CreateRfq, GetRfq, and CancelRfq handlers.
#[inline]
fn rfq_to_response<const T: u16>(rfq: &Rfq, request_id: uuid::Uuid) -> RfqResponse<T> {
    let symbol = rfq.instrument().symbol();
    let symbol_str = symbol.to_string();
    let base_asset = symbol.base_asset().to_string();
    let quote_asset = symbol.quote_asset().to_string();

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

    RfqResponse {
        request_id,
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
    }
}

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

    // Publish status update
    state.publish_status_update(RfqStatusUpdate {
        rfq_id: rfq.id().get(),
        previous_state: crate::domain::value_objects::rfq_state::RfqState::Created,
        current_state: rfq.state(),
        timestamp: crate::domain::value_objects::timestamp::Timestamp::now(),
        message: "RFQ created".to_string(),
    });

    // Build response using helper
    Ok(rfq_to_response(&rfq, request.request_id))
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
    let rfq_id = request.to_domain_rfq_id();

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

    // Build response using helper
    Ok(rfq_to_response(&rfq, request.request_id))
}

/// Handles CancelRfq request.
///
/// # Errors
///
/// Returns error if RFQ not found, state transition invalid, or persistence fails.
pub async fn handle_cancel_rfq(
    request: CancelRfqRequest,
    state: &AppState,
) -> SbeApiResult<CancelRfqResponse> {
    let rfq_id = request.to_domain_rfq_id();

    // Retrieve RFQ
    let mut rfq = state
        .rfq_repository
        .find_by_id(rfq_id)
        .await
        .map_err(|e| SbeApiError::Domain(format!("failed to find RFQ: {}", e)))?
        .ok_or_else(|| SbeApiError::InvalidValue {
            field: "rfq_id",
            message: format!("RFQ not found: {}", rfq_id),
        })?;

    // Capture previous state before cancellation
    let previous_state = rfq.state();

    // Cancel the RFQ
    rfq.cancel().map_err(|e| SbeApiError::InvalidValue {
        field: "state",
        message: format!("invalid RFQ state for cancellation: {}", e),
    })?;

    // Persist updated RFQ
    state
        .rfq_repository
        .save(&rfq)
        .await
        .map_err(|e| SbeApiError::Domain(format!("failed to save RFQ: {}", e)))?;

    // Publish status update
    state.publish_status_update(RfqStatusUpdate {
        rfq_id: rfq.id().get(),
        previous_state,
        current_state: rfq.state(),
        timestamp: crate::domain::value_objects::timestamp::Timestamp::now(),
        message: "RFQ cancelled".to_string(),
    });

    // Build response using helper
    Ok(rfq_to_response(&rfq, request.request_id))
}

/// Handles ExecuteTrade request.
///
/// # Errors
///
/// Returns error if RFQ not found, quote not found, quote expired, or trade creation fails.
pub async fn handle_execute_trade(
    request: ExecuteTradeRequest,
    state: &AppState,
) -> SbeApiResult<ExecuteTradeResponse> {
    let rfq_id = request.to_domain_rfq_id();
    let quote_id = request.to_domain_quote_id();

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

    // Find the quote
    let quote = rfq
        .quotes()
        .iter()
        .find(|q| q.id() == quote_id)
        .ok_or_else(|| SbeApiError::InvalidValue {
            field: "quote_id",
            message: format!("quote not found: {}", quote_id),
        })?;

    // Validate quote not expired
    if quote.is_expired() {
        return Err(SbeApiError::InvalidValue {
            field: "valid_until",
            message: format!("quote has expired: {}", quote_id),
        });
    }

    // Create trade
    let trade = Trade::new(
        rfq_id,
        quote_id,
        quote.venue_id().clone(),
        quote.price(),
        quote.quantity(),
    );

    // Publish status update with actual RFQ state (not hardcoded)
    // Note: Full execution flow (start_execution -> mark_executed) is handled
    // by the execution engine, not in this basic handler
    state.publish_status_update(RfqStatusUpdate {
        rfq_id: rfq.id().get(),
        previous_state: rfq.state(),
        current_state: rfq.state(),
        timestamp: crate::domain::value_objects::timestamp::Timestamp::now(),
        message: "Trade executed".to_string(),
    });

    // Build response
    Ok(ExecuteTradeResponse {
        trade_id: trade.id().get(),
        rfq_id: trade.rfq_id().get(),
        quote_id: trade.quote_id().get(),
        price: trade.price(),
        quantity: trade.quantity(),
        created_at: trade.created_at(),
        venue_id: trade.venue_id().to_string(),
        venue_execution_ref: trade.venue_execution_ref().unwrap_or_default().to_string(),
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
    use crate::domain::value_objects::{OrderSide, Quantity, RfqId};
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
        let (quote_updates, _) = tokio::sync::broadcast::channel(16);
        let (status_updates, _) = tokio::sync::broadcast::channel(16);
        AppState {
            rfq_repository: Arc::new(MockRfqRepository::new()),
            quote_updates,
            status_updates,
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
        let (quote_updates, _) = tokio::sync::broadcast::channel(16);
        let (status_updates, _) = tokio::sync::broadcast::channel(16);
        let state = AppState {
            rfq_repository: Arc::new(MockRfqRepository::with_rfq(rfq)),
            quote_updates,
            status_updates,
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

    #[tokio::test]
    async fn handle_cancel_rfq_success() {
        use crate::domain::entities::rfq::RfqBuilder;
        use crate::domain::value_objects::rfq_state::RfqState;
        use crate::domain::value_objects::timestamp::Timestamp;

        let symbol = Symbol::new("BTC/USD").unwrap();
        let instrument =
            Instrument::new(symbol, AssetClass::CryptoSpot, SettlementMethod::default());
        let quantity = Quantity::from_decimal(rust_decimal::Decimal::new(1, 0)).unwrap();
        let expires_at = Timestamp::now().add_secs(300);

        let rfq = RfqBuilder::new(
            CounterpartyId::new("client-1"),
            instrument,
            OrderSide::Buy,
            quantity,
            expires_at,
        )
        .build();
        let rfq_id = rfq.id();
        let (quote_updates, _) = tokio::sync::broadcast::channel(16);
        let (status_updates, _) = tokio::sync::broadcast::channel(16);
        let state = AppState {
            rfq_repository: Arc::new(MockRfqRepository::with_rfq(rfq)),
            quote_updates,
            status_updates,
        };

        let request = CancelRfqRequest {
            request_id: Uuid::new_v4(),
            rfq_id: rfq_id.get(),
            reason: "Client requested cancellation".to_string(),
        };
        let result = handle_cancel_rfq(request.clone(), &state).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        assert_eq!(response.request_id, request.request_id);
        assert_eq!(response.rfq_id, rfq_id.get());
        assert_eq!(response.state, RfqState::Cancelled);
    }

    #[tokio::test]
    async fn handle_cancel_rfq_not_found() {
        let state = create_test_state();
        let request = CancelRfqRequest {
            request_id: Uuid::new_v4(),
            rfq_id: Uuid::new_v4(),
            reason: "Test".to_string(),
        };
        let result = handle_cancel_rfq(request, &state).await;
        assert!(matches!(
            result,
            Err(SbeApiError::InvalidValue {
                field: "rfq_id",
                ..
            })
        ));
    }

    #[tokio::test]
    async fn handle_cancel_rfq_invalid_state() {
        use crate::domain::entities::rfq::RfqBuilder;
        use crate::domain::value_objects::timestamp::Timestamp;

        let symbol = Symbol::new("BTC/USD").unwrap();
        let instrument =
            Instrument::new(symbol, AssetClass::CryptoSpot, SettlementMethod::default());
        let quantity = Quantity::from_decimal(rust_decimal::Decimal::new(1, 0)).unwrap();
        let expires_at = Timestamp::now().add_secs(300);

        let mut rfq = RfqBuilder::new(
            CounterpartyId::new("client-1"),
            instrument,
            OrderSide::Buy,
            quantity,
            expires_at,
        )
        .build();
        rfq.cancel().unwrap();

        let rfq_id = rfq.id();
        let (quote_updates, _) = tokio::sync::broadcast::channel(16);
        let (status_updates, _) = tokio::sync::broadcast::channel(16);
        let state = AppState {
            rfq_repository: Arc::new(MockRfqRepository::with_rfq(rfq)),
            quote_updates,
            status_updates,
        };
        let request = CancelRfqRequest {
            request_id: Uuid::new_v4(),
            rfq_id: rfq_id.get(),
            reason: "Test".to_string(),
        };
        let result = handle_cancel_rfq(request, &state).await;
        assert!(matches!(
            result,
            Err(SbeApiError::InvalidValue { field: "state", .. })
        ));
    }

    #[tokio::test]
    async fn handle_execute_trade_success() {
        use crate::domain::entities::quote::Quote;
        use crate::domain::entities::rfq::RfqBuilder;
        use crate::domain::value_objects::timestamp::Timestamp;
        use crate::domain::value_objects::{Price, VenueId};

        let symbol = Symbol::new("ETH/USD").unwrap();
        let instrument =
            Instrument::new(symbol, AssetClass::CryptoSpot, SettlementMethod::default());
        let quantity = Quantity::from_decimal(rust_decimal::Decimal::new(2, 0)).unwrap();
        let expires_at = Timestamp::now().add_secs(600);

        let mut rfq = RfqBuilder::new(
            CounterpartyId::new("client-3"),
            instrument,
            OrderSide::Buy,
            quantity,
            expires_at,
        )
        .build();
        let rfq_id = rfq.id();

        rfq.start_quote_collection().unwrap();
        let quote = Quote::new(
            rfq_id,
            VenueId::new("venue-1"),
            Price::from_decimal(rust_decimal::Decimal::new(2000, 0)).unwrap(),
            quantity,
            Timestamp::now().add_secs(300),
        )
        .unwrap();
        let quote_id = quote.id();
        rfq.receive_quote(quote).unwrap();

        let (quote_updates, _) = tokio::sync::broadcast::channel(16);
        let (status_updates, _) = tokio::sync::broadcast::channel(16);
        let state = AppState {
            rfq_repository: Arc::new(MockRfqRepository::with_rfq(rfq)),
            quote_updates,
            status_updates,
        };
        let request = ExecuteTradeRequest {
            rfq_id: rfq_id.get(),
            quote_id: quote_id.get(),
        };
        let result = handle_execute_trade(request.clone(), &state).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        assert_eq!(response.rfq_id, rfq_id.get());
        assert_eq!(response.quote_id, quote_id.get());
        assert_eq!(response.venue_id, "venue-1");
        assert!(!response.trade_id.is_nil());
    }

    #[tokio::test]
    async fn handle_execute_trade_rfq_not_found() {
        let state = create_test_state();
        let request = ExecuteTradeRequest {
            rfq_id: Uuid::new_v4(),
            quote_id: Uuid::new_v4(),
        };
        let result = handle_execute_trade(request, &state).await;
        assert!(matches!(
            result,
            Err(SbeApiError::InvalidValue {
                field: "rfq_id",
                ..
            })
        ));
    }

    #[tokio::test]
    async fn handle_execute_trade_quote_not_found() {
        use crate::domain::entities::rfq::RfqBuilder;
        use crate::domain::value_objects::timestamp::Timestamp;

        let symbol = Symbol::new("BTC/USD").unwrap();
        let instrument =
            Instrument::new(symbol, AssetClass::CryptoSpot, SettlementMethod::default());
        let quantity = Quantity::from_decimal(rust_decimal::Decimal::new(1, 0)).unwrap();
        let expires_at = Timestamp::now().add_secs(300);

        let rfq = RfqBuilder::new(
            CounterpartyId::new("client-1"),
            instrument,
            OrderSide::Buy,
            quantity,
            expires_at,
        )
        .build();
        let rfq_id = rfq.id();
        let (quote_updates, _) = tokio::sync::broadcast::channel(16);
        let (status_updates, _) = tokio::sync::broadcast::channel(16);
        let state = AppState {
            rfq_repository: Arc::new(MockRfqRepository::with_rfq(rfq)),
            quote_updates,
            status_updates,
        };
        let request = ExecuteTradeRequest {
            rfq_id: rfq_id.get(),
            quote_id: Uuid::new_v4(),
        };
        let result = handle_execute_trade(request, &state).await;
        assert!(matches!(
            result,
            Err(SbeApiError::InvalidValue {
                field: "quote_id",
                ..
            })
        ));
    }

    #[tokio::test]
    async fn handle_execute_trade_quote_expired() {
        use crate::domain::entities::anonymity::AnonymityLevel;
        use crate::domain::entities::quote::Quote;
        use crate::domain::entities::rfq::Rfq;
        use crate::domain::value_objects::rfq_state::RfqState;
        use crate::domain::value_objects::timestamp::Timestamp;
        use crate::domain::value_objects::{Price, QuoteId, VenueId};

        let symbol = Symbol::new("BTC/USD").unwrap();
        let instrument =
            Instrument::new(symbol, AssetClass::CryptoSpot, SettlementMethod::default());
        let quantity = Quantity::from_decimal(rust_decimal::Decimal::new(1, 0)).unwrap();
        let expires_at = Timestamp::now().add_secs(300);

        let rfq_id = RfqId::new_v4();
        let quote_id = QuoteId::new_v4();

        let quote = Quote::from_parts(
            quote_id,
            rfq_id,
            VenueId::new("venue-1"),
            Price::from_decimal(rust_decimal::Decimal::new(50000, 0)).unwrap(),
            quantity,
            None,
            Timestamp::from_nanos(1).unwrap(),
            None,
            Timestamp::now(),
            false,
        );
        let rfq = Rfq::from_parts(
            rfq_id,
            CounterpartyId::new("client-1"),
            instrument,
            OrderSide::Buy,
            quantity,
            None,
            AnonymityLevel::default(),
            RfqState::QuotesReceived,
            expires_at,
            vec![quote],
            None,
            None,
            None,
            1,
            Timestamp::now(),
            Timestamp::now(),
        );

        let (quote_updates, _) = tokio::sync::broadcast::channel(16);
        let (status_updates, _) = tokio::sync::broadcast::channel(16);
        let state = AppState {
            rfq_repository: Arc::new(MockRfqRepository::with_rfq(rfq)),
            quote_updates,
            status_updates,
        };
        let request = ExecuteTradeRequest {
            rfq_id: rfq_id.get(),
            quote_id: quote_id.get(),
        };
        let result = handle_execute_trade(request, &state).await;
        assert!(matches!(
            result,
            Err(SbeApiError::InvalidValue {
                field: "valid_until",
                ..
            })
        ));
    }
}
