//! Integration tests for SBE TCP server.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::useless_vec,
    clippy::clone_on_ref_ptr
)]

use otc_rfq::api::sbe::server::{AppState, SbeConfig, SbeServer};
use otc_rfq::api::sbe::types::*;
use otc_rfq::application::use_cases::create_rfq::RfqRepository;
use otc_rfq::domain::entities::rfq::Rfq;
use otc_rfq::domain::value_objects::enums::AssetClass;
use otc_rfq::domain::value_objects::{OrderSide, Quantity, RfqId};
use otc_rfq::infrastructure::sbe::{SbeDecode, SbeEncode};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{RwLock, watch};
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
}

#[async_trait::async_trait]
impl RfqRepository for MockRfqRepository {
    async fn save(&self, rfq: &Rfq) -> Result<(), String> {
        self.rfqs.write().await.insert(rfq.id(), rfq.clone());
        Ok(())
    }

    async fn find_by_id(&self, id: RfqId) -> Result<Option<Rfq>, String> {
        Ok(self.rfqs.read().await.get(&id).cloned())
    }
}

async fn write_frame(stream: &mut TcpStream, data: &[u8]) -> std::io::Result<()> {
    let length = data.len() as u32;
    stream.write_all(&length.to_be_bytes()).await?;
    stream.write_all(data).await?;
    Ok(())
}

async fn read_frame(stream: &mut TcpStream) -> std::io::Result<Vec<u8>> {
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).await?;
    let length = u32::from_be_bytes(len_buf) as usize;

    let mut buffer = vec![0u8; length];
    stream.read_exact(&mut buffer).await?;
    Ok(buffer)
}

#[tokio::test]
async fn sbe_server_create_rfq_roundtrip() {
    // Start server
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("Failed to bind");
    let addr = listener.local_addr().expect("No local addr");

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let (quote_updates, _) = tokio::sync::broadcast::channel(16);
    let (status_updates, _) = tokio::sync::broadcast::channel(16);
    let state = Arc::new(AppState {
        rfq_repository: Arc::new(MockRfqRepository::new()),
        quote_updates,
        status_updates,
    });
    let config = SbeConfig::default();
    let server = SbeServer::new(listener, state, shutdown_rx, config);

    tokio::spawn(async move {
        let _ = server.run().await;
    });

    // Connect client with retry to wait for server readiness
    let mut client = loop {
        match TcpStream::connect(addr).await {
            Ok(stream) => break stream,
            Err(_) => {
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            }
        }
    };

    // Send CreateRfq request
    let request = CreateRfqRequest {
        request_id: Uuid::new_v4(),
        client_id: "test-client".to_string(),
        symbol: "BTC/USD".to_string(),
        base_asset: "BTC".to_string(),
        quote_asset: "USD".to_string(),
        side: OrderSide::Buy,
        quantity: Quantity::from_decimal(rust_decimal::Decimal::new(1, 0)).unwrap(),
        timeout_seconds: 300,
        asset_class: AssetClass::CryptoSpot,
    };

    let request_bytes = request.encode_to_vec().expect("Failed to encode request");
    write_frame(&mut client, &request_bytes)
        .await
        .expect("Failed to write frame");

    // Read response
    let response_bytes = read_frame(&mut client).await.expect("Failed to read frame");

    let response = CreateRfqResponse::decode(&response_bytes).expect("Failed to decode response");

    assert_eq!(response.request_id, request.request_id);
    assert_eq!(response.client_id, "test-client");
    assert_eq!(response.symbol, "BTC/USD");
    assert_eq!(response.side, OrderSide::Buy);

    // Cleanup
    let _ = shutdown_tx.send(true);
}

#[tokio::test]
async fn sbe_server_cancel_rfq_roundtrip() {
    // Start server
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("Failed to bind");
    let addr = listener.local_addr().expect("No local addr");

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let (quote_updates, _) = tokio::sync::broadcast::channel(16);
    let (status_updates, _) = tokio::sync::broadcast::channel(16);
    let state = Arc::new(AppState {
        rfq_repository: Arc::new(MockRfqRepository::new()),
        quote_updates,
        status_updates,
    });
    let config = SbeConfig::default();
    let server = SbeServer::new(listener, state, shutdown_rx, config);

    tokio::spawn(async move {
        let _ = server.run().await;
    });

    // Connect client with retry
    let mut client = loop {
        match TcpStream::connect(addr).await {
            Ok(stream) => break stream,
            Err(_) => {
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            }
        }
    };

    // First, create an RFQ
    let create_request = CreateRfqRequest {
        request_id: Uuid::new_v4(),
        client_id: "test-client".to_string(),
        symbol: "ETH/USD".to_string(),
        base_asset: "ETH".to_string(),
        quote_asset: "USD".to_string(),
        side: OrderSide::Sell,
        quantity: Quantity::from_decimal(rust_decimal::Decimal::new(5, 0)).unwrap(),
        timeout_seconds: 600,
        asset_class: AssetClass::CryptoSpot,
    };

    let request_bytes = create_request.encode_to_vec().expect("Failed to encode");
    write_frame(&mut client, &request_bytes)
        .await
        .expect("Failed to write");

    let response_bytes = read_frame(&mut client).await.expect("Failed to read");
    let create_response = CreateRfqResponse::decode(&response_bytes).expect("Failed to decode");
    let rfq_id = create_response.rfq_id;

    // Now cancel the RFQ
    let cancel_request = CancelRfqRequest {
        request_id: Uuid::new_v4(),
        rfq_id,
        reason: "Client requested cancellation".to_string(),
    };

    let cancel_bytes = cancel_request.encode_to_vec().expect("Failed to encode");
    write_frame(&mut client, &cancel_bytes)
        .await
        .expect("Failed to write");

    let cancel_response_bytes = read_frame(&mut client).await.expect("Failed to read");
    let cancel_response =
        CancelRfqResponse::decode(&cancel_response_bytes).expect("Failed to decode");

    assert_eq!(cancel_response.request_id, cancel_request.request_id);
    assert_eq!(cancel_response.rfq_id, rfq_id);
    assert_eq!(
        cancel_response.state,
        otc_rfq::domain::value_objects::rfq_state::RfqState::Cancelled
    );

    // Verify via GetRfq that it's cancelled
    let get_request = GetRfqRequest {
        request_id: Uuid::new_v4(),
        rfq_id,
    };

    let get_bytes = get_request.encode_to_vec().expect("Failed to encode");
    write_frame(&mut client, &get_bytes)
        .await
        .expect("Failed to write");

    let get_response_bytes = read_frame(&mut client).await.expect("Failed to read");
    let get_response = GetRfqResponse::decode(&get_response_bytes).expect("Failed to decode");

    assert_eq!(
        get_response.state,
        otc_rfq::domain::value_objects::rfq_state::RfqState::Cancelled
    );

    let _ = shutdown_tx.send(true);
}

#[tokio::test]
async fn sbe_server_execute_trade_not_found() {
    // Start server
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("Failed to bind");
    let addr = listener.local_addr().expect("No local addr");

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let (quote_updates, _) = tokio::sync::broadcast::channel(16);
    let (status_updates, _) = tokio::sync::broadcast::channel(16);
    let state = Arc::new(AppState {
        rfq_repository: Arc::new(MockRfqRepository::new()),
        quote_updates,
        status_updates,
    });
    let config = SbeConfig::default();
    let server = SbeServer::new(listener, state, shutdown_rx, config);

    tokio::spawn(async move {
        let _ = server.run().await;
    });

    // Connect client with retry
    let mut client = loop {
        match TcpStream::connect(addr).await {
            Ok(stream) => break stream,
            Err(_) => {
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            }
        }
    };

    // Send ExecuteTrade request with non-existent RFQ
    let request = ExecuteTradeRequest {
        rfq_id: Uuid::new_v4(),
        quote_id: Uuid::new_v4(),
    };

    let request_bytes = request.encode_to_vec().expect("Failed to encode");
    write_frame(&mut client, &request_bytes)
        .await
        .expect("Failed to write");

    // Read error response
    let response_bytes = read_frame(&mut client).await.expect("Failed to read");
    let response = ErrorResponse::decode(&response_bytes).expect("Failed to decode");

    assert_eq!(response.code, 400);
    assert!(response.message.contains("RFQ not found"));

    let _ = shutdown_tx.send(true);
}
