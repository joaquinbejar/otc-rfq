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
    let state = Arc::new(AppState {
        rfq_repository: Arc::new(MockRfqRepository::new()),
    });
    let config = SbeConfig::default();
    let server = SbeServer::new(listener, state, shutdown_rx, config);

    tokio::spawn(async move {
        let _ = server.run().await;
    });

    // Give server time to start
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Connect client
    let mut client = TcpStream::connect(addr).await.expect("Failed to connect");

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
async fn sbe_server_unsupported_template_returns_error() {
    // Start server
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("Failed to bind");
    let addr = listener.local_addr().expect("No local addr");

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let state = Arc::new(AppState {
        rfq_repository: Arc::new(MockRfqRepository::new()),
    });
    let config = SbeConfig::default();
    let server = SbeServer::new(listener, state, shutdown_rx, config);

    tokio::spawn(async move {
        let _ = server.run().await;
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let mut client = TcpStream::connect(addr).await.expect("Failed to connect");

    // Send CancelRfq request (not implemented yet)
    let request = CancelRfqRequest {
        request_id: Uuid::new_v4(),
        rfq_id: Uuid::new_v4(),
        reason: "test".to_string(),
    };

    let request_bytes = request.encode_to_vec().expect("Failed to encode");
    write_frame(&mut client, &request_bytes)
        .await
        .expect("Failed to write");

    // Read error response
    let response_bytes = read_frame(&mut client).await.expect("Failed to read");
    let response = ErrorResponse::decode(&response_bytes).expect("Failed to decode");

    assert_eq!(response.code, 501);
    assert_eq!(response.message, "not implemented");

    let _ = shutdown_tx.send(true);
}
