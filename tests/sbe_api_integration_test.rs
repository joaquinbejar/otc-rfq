//! # SBE API Integration Tests
//!
//! End-to-end integration tests for the SBE API covering full RFQ lifecycle,
//! subscription streaming, concurrent clients, error handling, and large messages.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::useless_vec,
    clippy::clone_on_ref_ptr
)]

mod common;

use common::MockRfqRepository;
use otc_rfq::api::sbe::server::{AppState, SbeConfig, SbeServer};
use otc_rfq::api::sbe::types::*;
use otc_rfq::domain::value_objects::enums::{AssetClass, VenueType};
use otc_rfq::domain::value_objects::timestamp::Timestamp;
use otc_rfq::domain::value_objects::{OrderSide, Price, Quantity};
use otc_rfq::infrastructure::sbe::{SbeDecode, SbeEncode};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::watch;
use uuid::Uuid;

// ============================================================================
// Test Harness
// ============================================================================

/// Test harness for SBE server integration tests.
struct SbeTestHarness {
    addr: SocketAddr,
    state: Arc<AppState>,
    shutdown: watch::Sender<bool>,
}

impl SbeTestHarness {
    /// Starts a test server on a random port.
    async fn start() -> Self {
        Self::start_with_config(SbeConfig::default()).await
    }

    /// Starts a test server with custom configuration.
    async fn start_with_config(config: SbeConfig) -> Self {
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

        let server = SbeServer::new(listener, Arc::clone(&state), shutdown_rx, config);
        tokio::spawn(async move {
            let _ = server.run().await;
        });

        Self {
            addr,
            state,
            shutdown: shutdown_tx,
        }
    }

    /// Connects a new test client to the server.
    async fn connect(&self) -> SbeTestClient {
        // Retry loop to wait for server readiness
        let stream = loop {
            match TcpStream::connect(self.addr).await {
                Ok(s) => break s,
                Err(_) => {
                    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                }
            }
        };
        SbeTestClient { stream }
    }

    /// Publishes a quote update to all subscribers.
    fn publish_quote(&self, update: QuoteUpdate) {
        self.state.publish_quote_update(update);
    }

    /// Publishes an RFQ status update to all subscribers.
    fn publish_status(&self, update: RfqStatusUpdate) {
        self.state.publish_status_update(update);
    }
}

impl Drop for SbeTestHarness {
    fn drop(&mut self) {
        let _ = self.shutdown.send(true);
    }
}

/// Test client for SBE integration tests.
struct SbeTestClient {
    stream: TcpStream,
}

impl SbeTestClient {
    /// Sends a typed SBE message.
    async fn send<T: SbeEncode>(&mut self, msg: &T) {
        let encoded = msg.encode_to_vec().expect("Failed to encode");
        self.send_raw(&encoded).await;
    }

    /// Sends a raw frame (for malformed message tests).
    async fn send_raw(&mut self, data: &[u8]) {
        let length = data.len() as u32;
        self.stream
            .write_all(&length.to_be_bytes())
            .await
            .expect("Failed to write length");
        self.stream
            .write_all(data)
            .await
            .expect("Failed to write data");
    }

    /// Receives a raw frame.
    async fn recv_raw(&mut self) -> Vec<u8> {
        let mut len_buf = [0u8; 4];
        self.stream
            .read_exact(&mut len_buf)
            .await
            .expect("Failed to read length");
        let length = u32::from_be_bytes(len_buf) as usize;

        let mut buffer = vec![0u8; length];
        self.stream
            .read_exact(&mut buffer)
            .await
            .expect("Failed to read data");
        buffer
    }

    /// Receives and decodes a typed SBE message.
    async fn recv_decode<T: SbeDecode>(&mut self) -> T {
        let data = self.recv_raw().await;
        T::decode(&data).expect("Failed to decode")
    }

    /// Tries to receive a frame with timeout, returns None if timeout or connection closed.
    async fn try_recv_timeout(&mut self, ms: u64) -> Option<Vec<u8>> {
        let result = tokio::time::timeout(tokio::time::Duration::from_millis(ms), async {
            let mut len_buf = [0u8; 4];
            match self.stream.read_exact(&mut len_buf).await {
                Ok(_) => {
                    let length = u32::from_be_bytes(len_buf) as usize;
                    let mut buffer = vec![0u8; length];
                    self.stream.read_exact(&mut buffer).await.ok()?;
                    Some(buffer)
                }
                Err(_) => None, // Connection closed or error
            }
        })
        .await;

        match result {
            Ok(Some(data)) => Some(data),
            _ => None, // Timeout or connection closed
        }
    }
}

// ============================================================================
// Integration Tests
// ============================================================================

#[tokio::test]
async fn lifecycle_create_subscribe_quotes_execute() {
    let harness = SbeTestHarness::start().await;
    let mut client = harness.connect().await;

    // Create RFQ
    let create_req = CreateRfqRequest {
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
    client.send(&create_req).await;
    let create_resp: CreateRfqResponse = client.recv_decode().await;
    let rfq_id = create_resp.rfq_id;

    // Subscribe to quotes
    let sub_req = SubscribeQuotesRequest::new(rfq_id);
    client.send(&sub_req).await;
    let ack: ErrorResponse = client.recv_decode().await;
    assert_eq!(ack.code, 0); // ACK
    assert_eq!(ack.rfq_id, rfq_id);

    // Publish multiple quote updates
    let quote_update1 = QuoteUpdate {
        quote_id: Uuid::new_v4(),
        rfq_id,
        price: Price::new(50000.0).unwrap(),
        quantity: Quantity::new(1.0).unwrap(),
        commission: rust_decimal::Decimal::ZERO,
        valid_until: Timestamp::now(),
        created_at: Timestamp::now(),
        venue_type: VenueType::InternalMM,
        is_final: false,
        venue_id: "venue-1".to_string(),
    };
    harness.publish_quote(quote_update1.clone());

    // Receive first quote update
    let received1: QuoteUpdate = client.recv_decode().await;
    assert_eq!(received1.quote_id, quote_update1.quote_id);
    assert_eq!(received1.rfq_id, rfq_id);
    assert_eq!(received1.price, quote_update1.price);

    // Publish second quote update
    let quote_update2 = QuoteUpdate {
        quote_id: Uuid::new_v4(),
        rfq_id,
        price: Price::new(49500.0).unwrap(),
        quantity: Quantity::new(1.0).unwrap(),
        commission: rust_decimal::Decimal::ZERO,
        valid_until: Timestamp::now(),
        created_at: Timestamp::now(),
        venue_type: VenueType::InternalMM,
        is_final: true,
        venue_id: "venue-2".to_string(),
    };
    harness.publish_quote(quote_update2.clone());

    // Receive second quote update
    let received2: QuoteUpdate = client.recv_decode().await;
    assert_eq!(received2.quote_id, quote_update2.quote_id);
    assert_eq!(received2.price, quote_update2.price);
    assert!(received2.is_final);
}

#[tokio::test]
async fn lifecycle_create_subscribe_status_cancel() {
    let harness = SbeTestHarness::start().await;
    let mut client = harness.connect().await;

    // Create RFQ
    let create_req = CreateRfqRequest {
        request_id: Uuid::new_v4(),
        client_id: "test-client".to_string(),
        symbol: "ETH/USD".to_string(),
        base_asset: "ETH".to_string(),
        quote_asset: "USD".to_string(),
        side: OrderSide::Sell,
        quantity: Quantity::from_decimal(rust_decimal::Decimal::new(10, 0)).unwrap(),
        timeout_seconds: 600,
        asset_class: AssetClass::CryptoSpot,
    };
    client.send(&create_req).await;
    let create_resp: CreateRfqResponse = client.recv_decode().await;
    let rfq_id = create_resp.rfq_id;

    // Subscribe to status updates
    let sub_req = SubscribeRfqStatusRequest::new(rfq_id);
    client.send(&sub_req).await;
    let ack: ErrorResponse = client.recv_decode().await;
    assert_eq!(ack.code, 0);

    // Cancel RFQ
    let cancel_req = CancelRfqRequest {
        request_id: Uuid::new_v4(),
        rfq_id,
        reason: "Test cancellation".to_string(),
    };
    client.send(&cancel_req).await;
    let cancel_resp: CancelRfqResponse = client.recv_decode().await;
    assert_eq!(cancel_resp.rfq_id, rfq_id);

    // Receive status update (published by cancel handler)
    let status_update: RfqStatusUpdate = client.recv_decode().await;
    assert_eq!(status_update.rfq_id, rfq_id);
    assert_eq!(
        status_update.current_state,
        otc_rfq::domain::value_objects::rfq_state::RfqState::Cancelled
    );
}

#[tokio::test]
async fn error_execute_trade_rfq_not_found() {
    let harness = SbeTestHarness::start().await;
    let mut client = harness.connect().await;

    let exec_req = ExecuteTradeRequest {
        rfq_id: Uuid::new_v4(),
        quote_id: Uuid::new_v4(),
    };
    client.send(&exec_req).await;
    let error_resp: ErrorResponse = client.recv_decode().await;
    assert_eq!(error_resp.code, 400);
    assert!(error_resp.message.contains("RFQ not found"));
}

#[tokio::test]
async fn error_unknown_template_id() {
    let harness = SbeTestHarness::start().await;
    let mut client = harness.connect().await;

    // Create a frame with invalid template ID
    let mut frame = vec![0u8; 8];
    frame[2..4].copy_from_slice(&999u16.to_le_bytes()); // Invalid template ID
    client.send_raw(&frame).await;

    let error_resp: ErrorResponse = client.recv_decode().await;
    assert_eq!(error_resp.code, 400);
    assert!(error_resp.message.contains("unknown template id"));
}

#[tokio::test]
async fn error_oversized_frame() {
    let config = SbeConfig {
        max_message_size: 1024,
        ..Default::default()
    };
    let harness = SbeTestHarness::start_with_config(config).await;
    let mut client = harness.connect().await;

    // Try to send a frame claiming to be larger than max
    let oversized_length = 2048u32;
    client
        .stream
        .write_all(&oversized_length.to_be_bytes())
        .await
        .expect("Failed to write");

    // Server should close connection immediately (no response sent)
    // Attempting to read should result in EOF or timeout
    let result = client.try_recv_timeout(500).await;
    // Connection should be dropped (None) or we get EOF
    assert!(
        result.is_none(),
        "Expected connection to be dropped for oversized frame"
    );
}

#[tokio::test]
async fn subscribe_quotes_ack() {
    let harness = SbeTestHarness::start().await;
    let mut client = harness.connect().await;

    let rfq_id = Uuid::new_v4();
    let sub_req = SubscribeQuotesRequest::new(rfq_id);
    client.send(&sub_req).await;

    let ack: ErrorResponse = client.recv_decode().await;
    assert_eq!(ack.code, 0);
    assert_eq!(ack.rfq_id, rfq_id);
    assert!(ack.message.contains("subscribed to quotes"));
}

#[tokio::test]
async fn subscribe_status_ack_and_delivery() {
    let harness = SbeTestHarness::start().await;
    let mut client = harness.connect().await;

    let rfq_id = Uuid::new_v4();
    let sub_req = SubscribeRfqStatusRequest::new(rfq_id);
    client.send(&sub_req).await;

    let ack: ErrorResponse = client.recv_decode().await;
    assert_eq!(ack.code, 0);
    assert!(ack.message.contains("subscribed to status"));

    // Publish status update
    let status_update = RfqStatusUpdate {
        rfq_id,
        previous_state: otc_rfq::domain::value_objects::rfq_state::RfqState::Created,
        current_state: otc_rfq::domain::value_objects::rfq_state::RfqState::Cancelled,
        timestamp: Timestamp::now(),
        message: "Test status update".to_string(),
    };
    harness.publish_status(status_update.clone());

    // Receive the update
    let received: RfqStatusUpdate = client.recv_decode().await;
    assert_eq!(received.rfq_id, rfq_id);
    assert_eq!(received.current_state, status_update.current_state);
}

#[tokio::test]
async fn unsubscribe_stops_delivery() {
    let harness = SbeTestHarness::start().await;
    let mut client = harness.connect().await;

    let rfq_id = Uuid::new_v4();

    // Subscribe
    let sub_req = SubscribeQuotesRequest::new(rfq_id);
    client.send(&sub_req).await;
    let _ack: ErrorResponse = client.recv_decode().await;

    // Unsubscribe
    let unsub_req = UnsubscribeRequest::new(rfq_id);
    client.send(&unsub_req).await;
    let unsub_ack: ErrorResponse = client.recv_decode().await;
    assert_eq!(unsub_ack.code, 0);
    assert!(unsub_ack.message.contains("unsubscribed"));

    // Publish quote update
    let quote_update = QuoteUpdate {
        quote_id: Uuid::new_v4(),
        rfq_id,
        price: Price::new(50000.0).unwrap(),
        quantity: Quantity::new(1.0).unwrap(),
        commission: rust_decimal::Decimal::ZERO,
        valid_until: Timestamp::now(),
        created_at: Timestamp::now(),
        venue_type: VenueType::InternalMM,
        is_final: true,
        venue_id: "test-venue".to_string(),
    };
    harness.publish_quote(quote_update);

    // Should NOT receive the update (timeout)
    let result = client.try_recv_timeout(200).await;
    assert!(
        result.is_none(),
        "Should not receive update after unsubscribe"
    );
}

#[tokio::test]
async fn disconnect_cleanup_server_survives() {
    let harness = SbeTestHarness::start().await;

    // First client subscribes and disconnects
    {
        let mut client = harness.connect().await;
        let rfq_id = Uuid::new_v4();
        let sub_req = SubscribeQuotesRequest::new(rfq_id);
        client.send(&sub_req).await;
        let _ack: ErrorResponse = client.recv_decode().await;
        // Client drops here, connection closes
    }

    // Give server time to clean up
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    // Second client should connect successfully
    let mut client2 = harness.connect().await;
    let create_req = CreateRfqRequest {
        request_id: Uuid::new_v4(),
        client_id: "client2".to_string(),
        symbol: "BTC/USD".to_string(),
        base_asset: "BTC".to_string(),
        quote_asset: "USD".to_string(),
        side: OrderSide::Buy,
        quantity: Quantity::from_decimal(rust_decimal::Decimal::new(1, 0)).unwrap(),
        timeout_seconds: 300,
        asset_class: AssetClass::CryptoSpot,
    };
    client2.send(&create_req).await;
    let _resp: CreateRfqResponse = client2.recv_decode().await;
    // If we get here, server survived the disconnect
}

#[tokio::test]
async fn concurrent_clients_independent_subscriptions() {
    let harness = SbeTestHarness::start().await;

    let rfq_id1 = Uuid::new_v4();
    let rfq_id2 = Uuid::new_v4();
    let rfq_id3 = Uuid::new_v4();

    // Connect 3 clients
    let mut client1 = harness.connect().await;
    let mut client2 = harness.connect().await;
    let mut client3 = harness.connect().await;

    // Each subscribes to a different RFQ
    client1.send(&SubscribeQuotesRequest::new(rfq_id1)).await;
    let _: ErrorResponse = client1.recv_decode().await;

    client2.send(&SubscribeQuotesRequest::new(rfq_id2)).await;
    let _: ErrorResponse = client2.recv_decode().await;

    client3.send(&SubscribeQuotesRequest::new(rfq_id3)).await;
    let _: ErrorResponse = client3.recv_decode().await;

    // Publish update for rfq_id2
    let quote_update = QuoteUpdate {
        quote_id: Uuid::new_v4(),
        rfq_id: rfq_id2,
        price: Price::new(50000.0).unwrap(),
        quantity: Quantity::new(1.0).unwrap(),
        commission: rust_decimal::Decimal::ZERO,
        valid_until: Timestamp::now(),
        created_at: Timestamp::now(),
        venue_type: VenueType::InternalMM,
        is_final: true,
        venue_id: "test-venue".to_string(),
    };
    harness.publish_quote(quote_update);

    // Only client2 should receive it
    assert!(client1.try_recv_timeout(200).await.is_none());
    let received2: QuoteUpdate = client2.recv_decode().await;
    assert_eq!(received2.rfq_id, rfq_id2);
    assert!(client3.try_recv_timeout(200).await.is_none());
}

#[tokio::test]
async fn concurrent_clients_shared_rfq() {
    let harness = SbeTestHarness::start().await;
    let rfq_id = Uuid::new_v4();

    // Connect 2 clients
    let mut client1 = harness.connect().await;
    let mut client2 = harness.connect().await;

    // Both subscribe to same RFQ
    client1.send(&SubscribeQuotesRequest::new(rfq_id)).await;
    let _: ErrorResponse = client1.recv_decode().await;

    client2.send(&SubscribeQuotesRequest::new(rfq_id)).await;
    let _: ErrorResponse = client2.recv_decode().await;

    // Publish update
    let quote_update = QuoteUpdate {
        quote_id: Uuid::new_v4(),
        rfq_id,
        price: Price::new(50000.0).unwrap(),
        quantity: Quantity::new(1.0).unwrap(),
        commission: rust_decimal::Decimal::ZERO,
        valid_until: Timestamp::now(),
        created_at: Timestamp::now(),
        venue_type: VenueType::InternalMM,
        is_final: true,
        venue_id: "test-venue".to_string(),
    };
    harness.publish_quote(quote_update.clone());

    // Both clients should receive it
    let received1: QuoteUpdate = client1.recv_decode().await;
    assert_eq!(received1.rfq_id, rfq_id);
    assert_eq!(received1.quote_id, quote_update.quote_id);

    let received2: QuoteUpdate = client2.recv_decode().await;
    assert_eq!(received2.rfq_id, rfq_id);
    assert_eq!(received2.quote_id, quote_update.quote_id);
}

#[tokio::test]
async fn large_cancel_reason_roundtrip() {
    let harness = SbeTestHarness::start().await;
    let mut client = harness.connect().await;

    // Create RFQ
    let create_req = CreateRfqRequest {
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
    client.send(&create_req).await;
    let create_resp: CreateRfqResponse = client.recv_decode().await;

    // Cancel with large reason string (~10KB)
    let large_reason = "A".repeat(10_000);
    let cancel_req = CancelRfqRequest {
        request_id: Uuid::new_v4(),
        rfq_id: create_resp.rfq_id,
        reason: large_reason.clone(),
    };
    client.send(&cancel_req).await;
    let cancel_resp: CancelRfqResponse = client.recv_decode().await;
    assert_eq!(cancel_resp.rfq_id, create_resp.rfq_id);
    assert_eq!(
        cancel_resp.state,
        otc_rfq::domain::value_objects::rfq_state::RfqState::Cancelled
    );
}
