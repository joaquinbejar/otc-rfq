#![cfg(feature = "nats")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs)]

use async_nats::jetstream;
use std::time::Duration;
use testcontainers::runners::AsyncRunner;
use testcontainers::{ContainerAsync, GenericImage, ImageExt};
use tokio::sync::mpsc;

use otc_rfq::application::use_cases::create_rfq::EventPublisher;
use otc_rfq::domain::events::rfq_events::RfqCreated;
use otc_rfq::domain::value_objects::enums::{AssetClass, SettlementMethod};
use otc_rfq::domain::value_objects::symbol::Symbol;
use otc_rfq::domain::value_objects::timestamp::Timestamp;
use otc_rfq::domain::value_objects::{CounterpartyId, Instrument, OrderSide, Quantity, RfqId};
use otc_rfq::infrastructure::messaging::dispatcher::DomainEventDispatcher;
use otc_rfq::infrastructure::messaging::nats_worker::NatsPublisherWorker;

#[tokio::test]
#[ignore = "Requires local Docker daemon for testcontainers"]
async fn test_nats_jetstream_publishing() {
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
    let dispatcher = DomainEventDispatcher::new(tx, "test_integration".to_string());

    let worker = NatsPublisherWorker::connect(&nats_url, "TEST_STREAM", "test_integration", rx)
        .await
        .expect("Failed to connect worker");

    // Ensure health check passes
    assert!(worker.health_check().is_ok());

    // Connect a separate client to verify the stream
    let verify_client = async_nats::connect(&nats_url).await.unwrap();
    let jetstream = jetstream::new(verify_client);
    let stream = jetstream.get_stream("TEST_STREAM").await.unwrap();

    // Spawn the worker in the background
    tokio::spawn(async move {
        worker.run().await;
    });

    // 3. Create a test event and publish via dispatcher
    let symbol = Symbol::new("BTC/USD").unwrap();
    let instrument = Instrument::new(symbol, AssetClass::CryptoSpot, SettlementMethod::default());
    let rfq_id = RfqId::new_v4();

    let mut event = RfqCreated::new(
        rfq_id,
        CounterpartyId::new("client-1"),
        instrument,
        OrderSide::Buy,
        Quantity::new(1.0).unwrap(),
        Timestamp::now().add_secs(300),
    );
    event.metadata.rfq_id = Some(rfq_id);

    dispatcher
        .publish_rfq_created(event.clone())
        .await
        .expect("Failed to dispatch event");

    // 4. Wait for the event to hit JetStream
    tokio::time::sleep(Duration::from_millis(500)).await;

    // 5. Verify the stream contains our message
    let consumer = stream
        .create_consumer(jetstream::consumer::pull::Config {
            durable_name: Some(format!("test_consumer_{}", uuid::Uuid::new_v4())),
            ..Default::default()
        })
        .await
        .unwrap();

    let mut messages = consumer.messages().await.unwrap();
    if let Some(msg) = tokio::time::timeout(
        Duration::from_secs(2),
        tokio_stream::StreamExt::next(&mut messages),
    )
    .await
    .expect("Timeout waiting for message")
    {
        let msg = msg.unwrap();
        let payload = std::str::from_utf8(&msg.payload).unwrap();
        let deserialized: RfqCreated = serde_json::from_str(payload).unwrap();
        assert_eq!(deserialized.metadata.rfq_id, Some(rfq_id));
        assert_eq!(
            msg.subject.as_str(),
            format!("test_integration.rfq.{}.created", rfq_id)
        );
        msg.ack().await.unwrap();
    } else {
        panic!("No message received from JetStream");
    }
}
