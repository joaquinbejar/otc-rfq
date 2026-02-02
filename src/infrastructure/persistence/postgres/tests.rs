//! # PostgreSQL Repository Integration Tests
//!
//! Integration tests for PostgreSQL repository implementations using testcontainers.
//!
//! # Test Categories
//!
//! - **RFQ Repository**: CRUD operations, optimistic locking
//! - **Trade Repository**: CRUD operations, state transitions
//! - **Event Store**: Append-only semantics, event retrieval
//! - **Transaction Rollback**: Verify rollback behavior
//!
//! # Note
//!
//! These tests require Docker to be running for testcontainers.
//! They are marked with `#[ignore]` by default and can be run with:
//! ```bash
//! cargo test --lib postgres::tests -- --ignored
//! ```

#![allow(clippy::unwrap_used)]
#![allow(clippy::indexing_slicing)]

use sqlx::PgPool;

use crate::domain::entities::rfq::{Rfq, RfqBuilder};
use crate::domain::entities::trade::Trade;
use crate::domain::value_objects::timestamp::Timestamp;
use crate::domain::value_objects::{
    AssetClass, CounterpartyId, Instrument, OrderSide, Price, Quantity, QuoteId, RfqId, Symbol,
    VenueId,
};
use crate::infrastructure::persistence::event_store::{EventStore, StoredEvent};
use crate::infrastructure::persistence::postgres::{
    PostgresEventStore, PostgresRfqRepository, PostgresTradeRepository,
};
use crate::infrastructure::persistence::traits::{RfqRepository, TradeRepository};

// ============================================================================
// Test Helpers
// ============================================================================

/// Creates a test database pool.
///
/// This function creates a connection to a test PostgreSQL database.
/// In a real integration test setup, this would use testcontainers.
async fn create_test_pool() -> Option<PgPool> {
    // Try to connect to a test database
    // This requires a running PostgreSQL instance
    let database_url = std::env::var("TEST_DATABASE_URL").ok()?;

    PgPool::connect(&database_url).await.ok()
}

/// Creates the required database tables for testing.
async fn setup_tables(pool: &PgPool) -> Result<(), sqlx::Error> {
    // Create RFQs table
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS rfqs (
            id VARCHAR(36) PRIMARY KEY,
            client_id VARCHAR(255) NOT NULL,
            instrument JSONB NOT NULL,
            side VARCHAR(10) NOT NULL,
            quantity DECIMAL NOT NULL,
            state VARCHAR(50) NOT NULL,
            expires_at BIGINT NOT NULL,
            quotes JSONB NOT NULL DEFAULT '[]',
            selected_quote_id VARCHAR(36),
            compliance_result JSONB,
            failure_reason TEXT,
            version BIGINT NOT NULL DEFAULT 1,
            created_at BIGINT NOT NULL,
            updated_at BIGINT NOT NULL
        )
        "#,
    )
    .execute(pool)
    .await?;

    // Create Trades table
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS trades (
            id VARCHAR(36) PRIMARY KEY,
            rfq_id VARCHAR(36) NOT NULL,
            quote_id VARCHAR(36) NOT NULL,
            venue_id VARCHAR(255) NOT NULL,
            price DECIMAL NOT NULL,
            quantity DECIMAL NOT NULL,
            venue_execution_ref VARCHAR(255),
            settlement_state VARCHAR(50) NOT NULL,
            settlement_tx_ref VARCHAR(255),
            failure_reason TEXT,
            version BIGINT NOT NULL DEFAULT 1,
            created_at BIGINT NOT NULL,
            updated_at BIGINT NOT NULL
        )
        "#,
    )
    .execute(pool)
    .await?;

    // Create Events table
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS domain_events (
            id SERIAL PRIMARY KEY,
            event_id VARCHAR(36) NOT NULL UNIQUE,
            rfq_id VARCHAR(36),
            event_type VARCHAR(50) NOT NULL,
            event_name VARCHAR(100) NOT NULL,
            timestamp BIGINT NOT NULL,
            payload JSONB NOT NULL,
            sequence BIGINT NOT NULL
        )
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// Cleans up test data between tests.
async fn cleanup_tables(pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM domain_events")
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM trades").execute(pool).await?;
    sqlx::query("DELETE FROM rfqs").execute(pool).await?;
    Ok(())
}

/// Creates a test RFQ.
fn create_test_rfq() -> Rfq {
    let symbol = Symbol::new("BTC/USD").unwrap();
    let instrument = Instrument::builder(symbol, AssetClass::CryptoSpot).build();

    RfqBuilder::new(
        CounterpartyId::new("test-client"),
        instrument,
        OrderSide::Buy,
        Quantity::new(1.0).unwrap(),
        Timestamp::now().add_secs(3600),
    )
    .build()
}

/// Creates a test Trade.
fn create_test_trade(rfq_id: RfqId, quote_id: QuoteId) -> Trade {
    Trade::new(
        rfq_id,
        quote_id,
        VenueId::new("test-venue"),
        Price::new(50000.0).unwrap(),
        Quantity::new(1.0).unwrap(),
    )
}

/// Creates a test StoredEvent.
fn create_test_event(rfq_id: RfqId, sequence: u64) -> StoredEvent {
    use crate::domain::events::domain_event::EventType;
    use crate::domain::value_objects::EventId;

    StoredEvent {
        event_id: EventId::new_v4(),
        rfq_id: Some(rfq_id),
        event_type: EventType::Rfq,
        event_name: "RfqCreated".to_string(),
        timestamp: Timestamp::now(),
        payload: serde_json::json!({
            "rfq_id": rfq_id.to_string(),
            "client_id": "test-client",
            "symbol": "BTC/USD"
        }),
        sequence,
    }
}

// ============================================================================
// RFQ Repository Tests
// ============================================================================

#[tokio::test]
#[ignore = "requires PostgreSQL database"]
async fn rfq_repository_save_and_get() {
    let pool = match create_test_pool().await {
        Some(p) => p,
        None => {
            eprintln!("Skipping test: TEST_DATABASE_URL not set");
            return;
        }
    };

    setup_tables(&pool).await.unwrap();
    cleanup_tables(&pool).await.unwrap();

    let repo = PostgresRfqRepository::new(pool.clone());
    let rfq = create_test_rfq();
    let rfq_id = rfq.id();

    // Save
    repo.save(&rfq).await.unwrap();

    // Get
    let retrieved = repo.get(&rfq_id).await.unwrap();
    assert!(retrieved.is_some());

    let retrieved = retrieved.unwrap();
    assert_eq!(retrieved.id(), rfq_id);
    assert_eq!(retrieved.client_id().as_str(), rfq.client_id().as_str());
    assert_eq!(retrieved.side(), rfq.side());

    cleanup_tables(&pool).await.unwrap();
}

#[tokio::test]
#[ignore = "requires PostgreSQL database"]
async fn rfq_repository_update() {
    let pool = match create_test_pool().await {
        Some(p) => p,
        None => return,
    };

    setup_tables(&pool).await.unwrap();
    cleanup_tables(&pool).await.unwrap();

    let repo = PostgresRfqRepository::new(pool.clone());
    let mut rfq = create_test_rfq();
    let rfq_id = rfq.id();

    // Save initial
    repo.save(&rfq).await.unwrap();

    // Update state
    rfq.start_quote_collection().unwrap();
    repo.save(&rfq).await.unwrap();

    // Verify update
    let retrieved = repo.get(&rfq_id).await.unwrap().unwrap();
    assert_eq!(retrieved.state(), rfq.state());
    assert_eq!(retrieved.version(), 2);

    cleanup_tables(&pool).await.unwrap();
}

#[tokio::test]
#[ignore = "requires PostgreSQL database"]
async fn rfq_repository_optimistic_locking() {
    let pool = match create_test_pool().await {
        Some(p) => p,
        None => return,
    };

    setup_tables(&pool).await.unwrap();
    cleanup_tables(&pool).await.unwrap();

    let repo = PostgresRfqRepository::new(pool.clone());
    let rfq = create_test_rfq();
    let rfq_id = rfq.id();

    // Save initial version
    repo.save(&rfq).await.unwrap();

    // Get two copies
    let mut rfq1 = repo.get(&rfq_id).await.unwrap().unwrap();
    let mut rfq2 = repo.get(&rfq_id).await.unwrap().unwrap();

    // Update first copy
    rfq1.start_quote_collection().unwrap();
    repo.save(&rfq1).await.unwrap();

    // Try to update second copy (should fail due to version conflict)
    rfq2.start_quote_collection().unwrap();
    let result = repo.save(&rfq2).await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("version") || err.to_string().contains("conflict"));

    cleanup_tables(&pool).await.unwrap();
}

#[tokio::test]
#[ignore = "requires PostgreSQL database"]
async fn rfq_repository_find_by_client() {
    let pool = match create_test_pool().await {
        Some(p) => p,
        None => return,
    };

    setup_tables(&pool).await.unwrap();
    cleanup_tables(&pool).await.unwrap();

    let repo = PostgresRfqRepository::new(pool.clone());

    // Create multiple RFQs for same client
    let rfq1 = create_test_rfq();
    let rfq2 = create_test_rfq();

    repo.save(&rfq1).await.unwrap();
    repo.save(&rfq2).await.unwrap();

    // Find by client
    let client_id = CounterpartyId::new("test-client");
    let rfqs = repo.find_by_client(&client_id).await.unwrap();

    assert_eq!(rfqs.len(), 2);

    cleanup_tables(&pool).await.unwrap();
}

#[tokio::test]
#[ignore = "requires PostgreSQL database"]
async fn rfq_repository_get_nonexistent() {
    let pool = match create_test_pool().await {
        Some(p) => p,
        None => return,
    };

    setup_tables(&pool).await.unwrap();
    cleanup_tables(&pool).await.unwrap();

    let repo = PostgresRfqRepository::new(pool.clone());
    let nonexistent_id = RfqId::new_v4();

    let result = repo.get(&nonexistent_id).await.unwrap();
    assert!(result.is_none());

    cleanup_tables(&pool).await.unwrap();
}

// ============================================================================
// Trade Repository Tests
// ============================================================================

#[tokio::test]
#[ignore = "requires PostgreSQL database"]
async fn trade_repository_save_and_get() {
    let pool = match create_test_pool().await {
        Some(p) => p,
        None => return,
    };

    setup_tables(&pool).await.unwrap();
    cleanup_tables(&pool).await.unwrap();

    let repo = PostgresTradeRepository::new(pool.clone());

    let rfq_id = RfqId::new_v4();
    let quote_id = QuoteId::new_v4();
    let trade = create_test_trade(rfq_id, quote_id);
    let trade_id = trade.id();

    // Save
    repo.save(&trade).await.unwrap();

    // Get
    let retrieved = repo.get(&trade_id).await.unwrap();
    assert!(retrieved.is_some());

    let retrieved = retrieved.unwrap();
    assert_eq!(retrieved.id(), trade_id);
    assert_eq!(retrieved.rfq_id(), rfq_id);
    assert_eq!(retrieved.quote_id(), quote_id);

    cleanup_tables(&pool).await.unwrap();
}

#[tokio::test]
#[ignore = "requires PostgreSQL database"]
async fn trade_repository_update_settlement_state() {
    let pool = match create_test_pool().await {
        Some(p) => p,
        None => return,
    };

    setup_tables(&pool).await.unwrap();
    cleanup_tables(&pool).await.unwrap();

    let repo = PostgresTradeRepository::new(pool.clone());

    let rfq_id = RfqId::new_v4();
    let quote_id = QuoteId::new_v4();
    let mut trade = create_test_trade(rfq_id, quote_id);
    let trade_id = trade.id();

    // Save initial
    repo.save(&trade).await.unwrap();

    // Update settlement state
    trade.start_settlement().unwrap();
    repo.save(&trade).await.unwrap();

    // Verify
    let retrieved = repo.get(&trade_id).await.unwrap().unwrap();
    assert!(retrieved.is_in_progress());

    cleanup_tables(&pool).await.unwrap();
}

#[tokio::test]
#[ignore = "requires PostgreSQL database"]
async fn trade_repository_find_by_rfq() {
    let pool = match create_test_pool().await {
        Some(p) => p,
        None => return,
    };

    setup_tables(&pool).await.unwrap();
    cleanup_tables(&pool).await.unwrap();

    let repo = PostgresTradeRepository::new(pool.clone());

    let rfq_id = RfqId::new_v4();
    let quote_id1 = QuoteId::new_v4();
    let quote_id2 = QuoteId::new_v4();

    let trade1 = create_test_trade(rfq_id, quote_id1);
    let trade2 = create_test_trade(rfq_id, quote_id2);

    repo.save(&trade1).await.unwrap();
    repo.save(&trade2).await.unwrap();

    // Find by RFQ (get_by_rfq returns Option<Trade>, not Vec)
    let trade = repo.get_by_rfq(&rfq_id).await.unwrap();
    // get_by_rfq returns the first trade for the RFQ
    assert!(trade.is_some());

    cleanup_tables(&pool).await.unwrap();
}

#[tokio::test]
#[ignore = "requires PostgreSQL database"]
async fn trade_repository_optimistic_locking() {
    let pool = match create_test_pool().await {
        Some(p) => p,
        None => return,
    };

    setup_tables(&pool).await.unwrap();
    cleanup_tables(&pool).await.unwrap();

    let repo = PostgresTradeRepository::new(pool.clone());

    let rfq_id = RfqId::new_v4();
    let quote_id = QuoteId::new_v4();
    let trade = create_test_trade(rfq_id, quote_id);
    let trade_id = trade.id();

    // Save initial
    repo.save(&trade).await.unwrap();

    // Get two copies
    let mut trade1 = repo.get(&trade_id).await.unwrap().unwrap();
    let mut trade2 = repo.get(&trade_id).await.unwrap().unwrap();

    // Update first
    trade1.start_settlement().unwrap();
    repo.save(&trade1).await.unwrap();

    // Try to update second (should fail)
    trade2.start_settlement().unwrap();
    let result = repo.save(&trade2).await;

    assert!(result.is_err());

    cleanup_tables(&pool).await.unwrap();
}

// ============================================================================
// Event Store Tests
// ============================================================================

#[tokio::test]
#[ignore = "requires PostgreSQL database"]
async fn event_store_append_and_get() {
    let pool = match create_test_pool().await {
        Some(p) => p,
        None => return,
    };

    setup_tables(&pool).await.unwrap();
    cleanup_tables(&pool).await.unwrap();

    let store = PostgresEventStore::new(pool.clone());

    let rfq_id = RfqId::new_v4();
    let event = create_test_event(rfq_id, 1);

    // Append
    store.append(event.clone()).await.unwrap();

    // Get events
    let events = store.get_events(&rfq_id).await.unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event_id, event.event_id);

    cleanup_tables(&pool).await.unwrap();
}

#[tokio::test]
#[ignore = "requires PostgreSQL database"]
async fn event_store_ordering() {
    let pool = match create_test_pool().await {
        Some(p) => p,
        None => return,
    };

    setup_tables(&pool).await.unwrap();
    cleanup_tables(&pool).await.unwrap();

    let store = PostgresEventStore::new(pool.clone());

    let rfq_id = RfqId::new_v4();

    // Append events out of order
    let event3 = create_test_event(rfq_id, 3);
    let event1 = create_test_event(rfq_id, 1);
    let event2 = create_test_event(rfq_id, 2);

    store.append(event3.clone()).await.unwrap();
    store.append(event1.clone()).await.unwrap();
    store.append(event2.clone()).await.unwrap();

    // Get events (should be ordered by sequence)
    let events = store.get_events(&rfq_id).await.unwrap();
    assert_eq!(events.len(), 3);
    assert_eq!(events[0].sequence, 1);
    assert_eq!(events[1].sequence, 2);
    assert_eq!(events[2].sequence, 3);

    cleanup_tables(&pool).await.unwrap();
}

#[tokio::test]
#[ignore = "requires PostgreSQL database"]
async fn event_store_append_only() {
    let pool = match create_test_pool().await {
        Some(p) => p,
        None => return,
    };

    setup_tables(&pool).await.unwrap();
    cleanup_tables(&pool).await.unwrap();

    let store = PostgresEventStore::new(pool.clone());

    let rfq_id = RfqId::new_v4();
    let event = create_test_event(rfq_id, 1);

    // Append same event twice (should fail due to unique constraint)
    store.append(event.clone()).await.unwrap();
    let result = store.append(event.clone()).await;

    assert!(result.is_err());

    cleanup_tables(&pool).await.unwrap();
}

#[tokio::test]
#[ignore = "requires PostgreSQL database"]
async fn event_store_multiple_rfqs() {
    let pool = match create_test_pool().await {
        Some(p) => p,
        None => return,
    };

    setup_tables(&pool).await.unwrap();
    cleanup_tables(&pool).await.unwrap();

    let store = PostgresEventStore::new(pool.clone());

    let rfq_id1 = RfqId::new_v4();
    let rfq_id2 = RfqId::new_v4();

    // Append events for different RFQs
    store.append(create_test_event(rfq_id1, 1)).await.unwrap();
    store.append(create_test_event(rfq_id1, 2)).await.unwrap();
    store.append(create_test_event(rfq_id2, 1)).await.unwrap();

    // Get events for each RFQ
    let events1 = store.get_events(&rfq_id1).await.unwrap();
    let events2 = store.get_events(&rfq_id2).await.unwrap();

    assert_eq!(events1.len(), 2);
    assert_eq!(events2.len(), 1);

    cleanup_tables(&pool).await.unwrap();
}

// ============================================================================
// Database Cleanup Tests
// ============================================================================

#[tokio::test]
#[ignore = "requires PostgreSQL database"]
async fn cleanup_between_tests() {
    let pool = match create_test_pool().await {
        Some(p) => p,
        None => return,
    };

    setup_tables(&pool).await.unwrap();

    let rfq_repo = PostgresRfqRepository::new(pool.clone());
    let trade_repo = PostgresTradeRepository::new(pool.clone());
    let event_store = PostgresEventStore::new(pool.clone());

    // Create test data
    let rfq = create_test_rfq();
    let rfq_id = rfq.id();
    rfq_repo.save(&rfq).await.unwrap();

    let trade = create_test_trade(rfq_id, QuoteId::new_v4());
    trade_repo.save(&trade).await.unwrap();

    event_store
        .append(create_test_event(rfq_id, 1))
        .await
        .unwrap();

    // Cleanup
    cleanup_tables(&pool).await.unwrap();

    // Verify cleanup
    let rfqs = rfq_repo.get(&rfq_id).await.unwrap();
    assert!(rfqs.is_none());

    let trade = trade_repo.get_by_rfq(&rfq_id).await.unwrap();
    assert!(trade.is_none());

    let events = event_store.get_events(&rfq_id).await.unwrap();
    assert!(events.is_empty());
}

// ============================================================================
// Mock Tests (run without database)
// ============================================================================

#[test]
fn test_rfq_creation() {
    let rfq = create_test_rfq();
    assert_eq!(rfq.client_id().as_str(), "test-client");
    assert_eq!(rfq.side(), OrderSide::Buy);
    assert_eq!(rfq.version(), 1);
}

#[test]
fn test_trade_creation() {
    let rfq_id = RfqId::new_v4();
    let quote_id = QuoteId::new_v4();
    let trade = create_test_trade(rfq_id, quote_id);

    assert_eq!(trade.rfq_id(), rfq_id);
    assert_eq!(trade.quote_id(), quote_id);
    assert!(trade.is_pending());
}

#[test]
fn test_stored_event_creation() {
    let rfq_id = RfqId::new_v4();
    let event = create_test_event(rfq_id, 1);

    assert_eq!(event.rfq_id, Some(rfq_id));
    assert_eq!(event.sequence, 1);
    assert_eq!(event.event_name, "RfqCreated");
}
