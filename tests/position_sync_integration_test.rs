#![cfg(feature = "nats")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs)]

use async_nats::jetstream;
use std::time::Duration;
use testcontainers::runners::AsyncRunner;
use testcontainers::{ContainerAsync, GenericImage, ImageExt};
use tokio::sync::mpsc;

use otc_rfq::application::use_cases::execute_trade::TradeEventPublisher;
use otc_rfq::domain::events::{PositionUpdated, TradeExecuted};
use otc_rfq::domain::value_objects::enums::{AssetClass, SettlementMethod};
use otc_rfq::domain::value_objects::symbol::Symbol;
use otc_rfq::domain::value_objects::{
    CounterpartyId, Instrument, OrderSide, Price, Quantity, RfqId, TradeId, VenueId,
};
use otc_rfq::infrastructure::messaging::dispatcher::DomainEventDispatcher;
use otc_rfq::infrastructure::messaging::nats_worker::NatsPublisherWorker;

#[tokio::test]
#[ignore = "Requires local Docker daemon for testcontainers"]
async fn test_position_update_published_to_nats() {
    // 1. Start NATS server with JetStream enabled
    let nats_image = GenericImage::new("nats", "2.10.18")
        .with_exposed_port(testcontainers::core::ContainerPort::Tcp(4222))
        .with_cmd(["-js".to_string()]);
    let nats_container: ContainerAsync<GenericImage> = nats_image
        .start()
        .await
        .expect("Failed to start NATS container");
    let nats_port = nats_container.get_host_port_ipv4(4222).await.unwrap();
    let nats_url = format!("nats://127.0.0.1:{}", nats_port);

    // 2. Setup Dispatcher and Worker
    let (tx, rx) = mpsc::channel(100);
    let dispatcher = DomainEventDispatcher::new(tx, "test_position".to_string());

    let worker =
        NatsPublisherWorker::connect(&nats_url, "TEST_POSITION_STREAM", "test_position", rx)
            .await
            .expect("Failed to connect worker");

    // Ensure health check passes
    assert!(worker.health_check().is_ok());

    // Connect a separate client to verify the stream
    let verify_client = async_nats::connect(&nats_url).await.unwrap();
    let jetstream = jetstream::new(verify_client);
    let stream = jetstream.get_stream("TEST_POSITION_STREAM").await.unwrap();

    // Spawn the worker in the background
    tokio::spawn(async move {
        worker.run().await;
    });

    // 3. Create test data
    let rfq_id = RfqId::new_v4();
    let trade_id = TradeId::new_v4();
    let requester_id = CounterpartyId::new("client-1");
    let mm_id = VenueId::new("mm-1");
    let symbol = Symbol::new("BTC/USD").unwrap();
    let instrument = Instrument::new(symbol, AssetClass::CryptoSpot, SettlementMethod::default());

    // 4. Create and publish PositionUpdated event
    let position_event = PositionUpdated::new(
        rfq_id,
        trade_id,
        requester_id.clone(),
        OrderSide::Buy,
        mm_id.clone(),
        OrderSide::Sell,
        instrument.clone(),
        Quantity::new(1.0).unwrap(),
        Price::new(50000.0).unwrap(),
    );

    dispatcher
        .publish_position_updated(position_event.clone())
        .await
        .expect("Failed to publish position update");

    // 5. Wait for the events to hit JetStream
    tokio::time::sleep(Duration::from_millis(500)).await;

    // 6. Verify the stream contains our messages
    let consumer = stream
        .create_consumer(jetstream::consumer::pull::Config {
            durable_name: Some(format!("test_consumer_{}", uuid::Uuid::new_v4())),
            ..Default::default()
        })
        .await
        .unwrap();

    let mut messages = consumer.messages().await.unwrap();

    // Should receive 2 messages: one for requester, one for MM
    let mut received_requester = false;
    let mut received_mm = false;

    for _ in 0..2 {
        if let Some(msg) = tokio::time::timeout(
            Duration::from_secs(2),
            tokio_stream::StreamExt::next(&mut messages),
        )
        .await
        .expect("Timeout waiting for message")
        {
            let msg = msg.unwrap();
            let payload = std::str::from_utf8(&msg.payload).unwrap();
            let deserialized: PositionUpdated = serde_json::from_str(payload).unwrap();

            // Verify event data
            assert_eq!(deserialized.trade_id, trade_id);
            assert_eq!(deserialized.instrument, instrument);

            // Check which subject it was published to
            if msg.subject.as_str() == format!("test_position.position.{}.updated", &requester_id) {
                received_requester = true;
                assert_eq!(deserialized.requester_id, requester_id);
                assert_eq!(deserialized.requester_side, OrderSide::Buy);
            } else if msg.subject.as_str() == format!("test_position.position.{}.updated", &mm_id) {
                received_mm = true;
                assert_eq!(deserialized.mm_id, mm_id);
                assert_eq!(deserialized.mm_side, OrderSide::Sell);
            }

            msg.ack().await.unwrap();
        }
    }

    assert!(
        received_requester,
        "Did not receive requester position update"
    );
    assert!(received_mm, "Did not receive MM position update");
}

#[tokio::test]
#[ignore = "Requires local Docker daemon for testcontainers"]
async fn test_trade_executed_and_position_updated_both_published() {
    // 1. Start NATS server with JetStream enabled
    let nats_image = GenericImage::new("nats", "2.10.18")
        .with_exposed_port(testcontainers::core::ContainerPort::Tcp(4222))
        .with_cmd(["-js".to_string()]);
    let nats_container: ContainerAsync<GenericImage> = nats_image
        .start()
        .await
        .expect("Failed to start NATS container");
    let nats_port = nats_container.get_host_port_ipv4(4222).await.unwrap();
    let nats_url = format!("nats://127.0.0.1:{}", nats_port);

    // 2. Setup Dispatcher and Worker
    let (tx, rx) = mpsc::channel(100);
    let dispatcher = DomainEventDispatcher::new(tx, "test_both".to_string());

    let worker = NatsPublisherWorker::connect(&nats_url, "TEST_BOTH_STREAM", "test_both", rx)
        .await
        .expect("Failed to connect worker");

    let verify_client = async_nats::connect(&nats_url).await.unwrap();
    let jetstream = jetstream::new(verify_client);
    let stream = jetstream.get_stream("TEST_BOTH_STREAM").await.unwrap();

    tokio::spawn(async move {
        worker.run().await;
    });

    // 3. Create test data
    let rfq_id = RfqId::new_v4();
    let trade_id = TradeId::new_v4();
    let requester_id = CounterpartyId::new("client-1");
    let mm_id = VenueId::new("mm-1");
    let symbol = Symbol::new("ETH/USD").unwrap();
    let instrument = Instrument::new(symbol, AssetClass::CryptoSpot, SettlementMethod::default());

    // 4. Publish both TradeExecuted and PositionUpdated
    let trade_event = TradeExecuted::builder()
        .rfq_id(rfq_id)
        .trade_id(trade_id)
        .quote_id(otc_rfq::domain::value_objects::QuoteId::new_v4())
        .venue_id(mm_id.clone())
        .counterparty_id(requester_id.clone())
        .price(Price::new(3000.0).unwrap())
        .quantity(Quantity::new(10.0).unwrap())
        .settlement_method(SettlementMethod::OffChain)
        .build();

    dispatcher
        .publish_trade_executed(trade_event)
        .await
        .expect("Failed to publish trade executed");

    let position_event = PositionUpdated::new(
        rfq_id,
        trade_id,
        requester_id.clone(),
        OrderSide::Buy,
        mm_id.clone(),
        OrderSide::Sell,
        instrument,
        Quantity::new(10.0).unwrap(),
        Price::new(3000.0).unwrap(),
    );

    dispatcher
        .publish_position_updated(position_event)
        .await
        .expect("Failed to publish position update");

    // 5. Wait for events
    tokio::time::sleep(Duration::from_millis(500)).await;

    // 6. Verify we received all 3 events: 1 TradeExecuted + 2 PositionUpdated
    let consumer = stream
        .create_consumer(jetstream::consumer::pull::Config {
            durable_name: Some(format!("test_consumer_{}", uuid::Uuid::new_v4())),
            ..Default::default()
        })
        .await
        .unwrap();

    let mut messages = consumer.messages().await.unwrap();
    let mut event_count = 0;

    for _ in 0..3 {
        if let Some(msg) = tokio::time::timeout(
            Duration::from_secs(2),
            tokio_stream::StreamExt::next(&mut messages),
        )
        .await
        .ok()
        .flatten()
        {
            let msg = msg.unwrap();
            event_count += 1;
            msg.ack().await.unwrap();
        }
    }

    assert_eq!(
        event_count, 3,
        "Expected 3 events (1 TradeExecuted + 2 PositionUpdated)"
    );
}
