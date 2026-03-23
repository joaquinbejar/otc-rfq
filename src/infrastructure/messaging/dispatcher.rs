//! # Domain Event Dispatcher
//!
//! An in-memory dispatcher that implements domain event publisher traits
//! and routes serialized events to a background worker to avoid blocking
//! the critical path with network I/O.

use crate::application::error::ApplicationResult;
use crate::application::use_cases::collect_quotes::QuoteEventPublisher;
use crate::application::use_cases::create_rfq::EventPublisher;
use crate::application::use_cases::execute_trade::TradeEventPublisher;
use crate::domain::events::rfq_events::{QuoteReceived, RfqCreated};
use crate::domain::events::trade_events::TradeExecuted;
use crate::domain::value_objects::{QuoteId, RfqId};
use async_trait::async_trait;
use serde::Serialize;
use std::fmt;
use tokio::sync::mpsc;

/// Tuple representing an event to be published: `(subject, json_payload)`
pub type PublishPayload = (String, String);

/// A fast, non-blocking event dispatcher.
///
/// Implements application-layer publisher traits, serializes events,
/// and delegates network publishing to a background worker via an async channel.
#[derive(Clone)]
pub struct DomainEventDispatcher {
    sender: mpsc::Sender<PublishPayload>,
    subject_prefix: String,
}

impl DomainEventDispatcher {
    /// Creates a new dispatcher.
    #[must_use]
    pub fn new(sender: mpsc::Sender<PublishPayload>, subject_prefix: String) -> Self {
        Self {
            sender,
            subject_prefix,
        }
    }

    /// Serializes and dispatches an event.
    #[allow(clippy::unused_async)]
    async fn dispatch<T: Serialize>(&self, subject: String, event: &T) -> Result<(), String> {
        let payload = serde_json::to_string(event).map_err(|e| format!("{e}"))?;
        let msg = (subject, payload);

        // Non-blocking try_send is preferred to avoid stalling the use-case
        // if the background worker falls behind or channel is full.
        if let Err(e) = self.sender.try_send(msg) {
            match e {
                mpsc::error::TrySendError::Full(returned_msg) => {
                    tracing::warn!(
                        "Event dispatcher channel full, dropping event: {}",
                        returned_msg.0
                    );
                }
                mpsc::error::TrySendError::Closed(returned_msg) => {
                    tracing::error!(
                        "Event dispatcher channel closed. Action: {}",
                        returned_msg.0
                    );
                }
            }
        }
        Ok(())
    }
}

impl fmt::Debug for DomainEventDispatcher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DomainEventDispatcher")
            .field("subject_prefix", &self.subject_prefix)
            .finish()
    }
}

#[async_trait]
impl EventPublisher for DomainEventDispatcher {
    async fn publish_rfq_created(&self, event: RfqCreated) -> Result<(), String> {
        let rfq_id = event
            .metadata
            .rfq_id
            .ok_or_else(|| "Missing RFQ ID in event metadata".to_string())?;
        let subject = format!("{}.rfq.{}.created", self.subject_prefix, rfq_id);
        self.dispatch(subject, &event).await
    }
}

#[async_trait]
impl QuoteEventPublisher for DomainEventDispatcher {
    async fn publish_quote_received(&self, event: QuoteReceived) -> Result<(), String> {
        let rfq_id = event
            .metadata
            .rfq_id
            .ok_or_else(|| "Missing RFQ ID in event metadata".to_string())?;
        let subject = format!("{}.rfq.{}.quote_received", self.subject_prefix, rfq_id);
        self.dispatch(subject, &event).await
    }
}

#[async_trait]
impl TradeEventPublisher for DomainEventDispatcher {
    async fn publish_trade_executed(&self, event: TradeExecuted) -> ApplicationResult<()> {
        let rfq_id = event.metadata.rfq_id.ok_or_else(|| {
            crate::application::error::ApplicationError::EventPublishError(
                "Missing RFQ ID in event metadata".to_string(),
            )
        })?;
        let subject = format!("{}.rfq.{}.trade_executed", self.subject_prefix, rfq_id);
        self.dispatch(subject, &event)
            .await
            .map_err(crate::application::error::ApplicationError::EventPublishError)
    }

    async fn publish_execution_failed(
        &self,
        rfq_id: RfqId,
        quote_id: QuoteId,
        reason: &str,
    ) -> ApplicationResult<()> {
        // We create an anonymous payload for the failure
        let payload = serde_json::json!({
            "rfq_id": rfq_id,
            "quote_id": quote_id,
            "reason": reason,
        });
        let subject = format!("{}.rfq.{}.execution_failed", self.subject_prefix, rfq_id);

        self.dispatch(subject, &payload)
            .await
            .map_err(crate::application::error::ApplicationError::EventPublishError)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::domain::value_objects::CounterpartyId;
    use crate::domain::value_objects::enums::{AssetClass, SettlementMethod};
    use crate::domain::value_objects::symbol::Symbol;
    use crate::domain::value_objects::timestamp::Timestamp;
    use crate::domain::value_objects::{Instrument, OrderSide, Quantity};

    fn create_test_rfq_created() -> RfqCreated {
        let symbol = Symbol::new("BTC/USD").unwrap();
        let instrument =
            Instrument::new(symbol, AssetClass::CryptoSpot, SettlementMethod::default());
        let quantity = Quantity::new(1.0).unwrap();
        let expires_at = Timestamp::now().add_secs(300);
        let rfq_id = RfqId::new_v4();

        let mut event = RfqCreated::new(
            rfq_id,
            CounterpartyId::new("client-1"),
            instrument,
            OrderSide::Buy,
            quantity,
            expires_at,
        );
        event.metadata.rfq_id = Some(rfq_id);
        event
    }

    #[tokio::test]
    async fn test_dispatcher_publishes_rfq_created() {
        let (tx, mut rx) = mpsc::channel(100);
        let dispatcher = DomainEventDispatcher::new(tx, "otc".to_string());

        let event = create_test_rfq_created();
        let rfq_id = event.metadata.rfq_id.unwrap();

        // Publish
        let result = dispatcher.publish_rfq_created(event.clone()).await;
        assert!(result.is_ok());

        // Verify channel received
        let (subject, payload) = rx.recv().await.expect("Channel closed");
        assert_eq!(subject, format!("otc.rfq.{}.created", rfq_id));

        // Payload should serialize to the event
        let deserialized: RfqCreated =
            serde_json::from_str(&payload).expect("Failed to parse JSON");
        assert_eq!(deserialized.metadata.rfq_id, Some(rfq_id));
    }
}
