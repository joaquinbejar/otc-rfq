//! # API Callback Confirmation Adapter
//!
//! Delivers trade confirmations via HTTP POST to webhook URLs.

use crate::domain::errors::{DomainError, DomainResult};
use crate::domain::services::confirmation_service::ConfirmationChannelAdapter;
use crate::domain::value_objects::confirmation::{
    ConfirmationChannel, NotificationDestination, TradeConfirmation,
};
use async_trait::async_trait;
use reqwest;
use serde_json;
use std::time::Duration;

/// API callback confirmation adapter.
#[derive(Debug)]
pub struct ApiCallbackConfirmationAdapter {
    client: reqwest::Client,
}

impl ApiCallbackConfirmationAdapter {
    /// Creates a new API callback confirmation adapter.
    ///
    /// # Arguments
    ///
    /// * `timeout` - HTTP request timeout
    ///
    /// # Errors
    ///
    /// Returns an error if the HTTP client cannot be created.
    pub fn new(timeout: Duration) -> DomainResult<Self> {
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|e| DomainError::ConfirmationFailed {
                channel: "API_CALLBACK".to_string(),
                reason: format!("Failed to create HTTP client: {}", e),
            })?;

        Ok(Self { client })
    }

    /// Creates a new adapter with default timeout (10 seconds).
    ///
    /// # Errors
    ///
    /// Returns an error if the HTTP client cannot be created.
    pub fn with_default_timeout() -> DomainResult<Self> {
        Self::new(Duration::from_secs(10))
    }
}

#[async_trait]
impl ConfirmationChannelAdapter for ApiCallbackConfirmationAdapter {
    async fn send(
        &self,
        confirmation: &TradeConfirmation,
        destination: NotificationDestination<'_>,
    ) -> DomainResult<()> {
        // Extract webhook URL from destination
        let webhook_url = match destination {
            NotificationDestination::Webhook(url) => url,
            _ => {
                return Err(DomainError::InvalidNotificationPreferences {
                    reason: "Expected Webhook destination".to_string(),
                });
            }
        };

        // Serialize confirmation to JSON
        let json_body =
            serde_json::to_string(confirmation).map_err(|e| DomainError::ConfirmationFailed {
                channel: "API_CALLBACK".to_string(),
                reason: format!("JSON serialization failed: {}", e),
            })?;

        // Send POST request
        let response = self
            .client
            .post(webhook_url)
            .header("Content-Type", "application/json")
            .header("User-Agent", "OTC-RFQ-Platform/1.0")
            .body(json_body)
            .send()
            .await
            .map_err(|e| DomainError::ConfirmationFailed {
                channel: "API_CALLBACK".to_string(),
                reason: format!("HTTP request failed: {}", e),
            })?;

        // Check response status
        let status = response.status();
        if status.is_success() {
            tracing::info!(
                trade_id = %confirmation.trade_id(),
                webhook_url = %webhook_url,
                status = %status,
                "API callback confirmation sent successfully"
            );
            Ok(())
        } else if status.is_server_error() {
            // 5xx errors should be retried by the confirmation service
            let reason = match status.canonical_reason() {
                Some(reason) => format!("Server error {}: {}", status.as_u16(), reason),
                None => format!("Server error {}", status.as_u16()),
            };
            Err(DomainError::ConfirmationFailed {
                channel: "API_CALLBACK".to_string(),
                reason,
            })
        } else {
            // 4xx errors should not be retried (client errors are permanent)
            let reason = match status.canonical_reason() {
                Some(reason) => format!("Client error {}: {}", status.as_u16(), reason),
                None => format!("Client error {}", status.as_u16()),
            };
            Err(DomainError::ConfirmationFailed {
                channel: "API_CALLBACK".to_string(),
                reason,
            })
        }
    }

    fn channel_type(&self) -> ConfirmationChannel {
        ConfirmationChannel::ApiCallback
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::domain::value_objects::{
        Blockchain, CounterpartyId, Price, Quantity, RfqId, SettlementMethod, TradeId,
        TradeParticipant,
    };
    use rust_decimal::Decimal;

    fn create_test_confirmation() -> TradeConfirmation {
        TradeConfirmation::new(
            TradeId::new_v4(),
            RfqId::new_v4(),
            Price::new(50000.0).unwrap(),
            Quantity::new(1.0).unwrap(),
            Decimal::new(10, 0),
            Decimal::new(5, 0),
            Decimal::new(15, 0),
            SettlementMethod::OnChain(Blockchain::Ethereum),
            TradeParticipant::Counterparty(CounterpartyId::new("buyer-1")),
            TradeParticipant::Counterparty(CounterpartyId::new("seller-1")),
        )
    }

    #[test]
    fn new_creates_adapter() {
        let result = ApiCallbackConfirmationAdapter::new(Duration::from_secs(5));
        assert!(result.is_ok());
    }

    #[test]
    fn with_default_timeout_creates_adapter() {
        let result = ApiCallbackConfirmationAdapter::with_default_timeout();
        assert!(result.is_ok());
    }

    #[test]
    fn channel_returns_api_callback() {
        let adapter = ApiCallbackConfirmationAdapter::with_default_timeout().unwrap();
        assert_eq!(adapter.channel_type(), ConfirmationChannel::ApiCallback);
    }

    #[tokio::test]
    async fn send_error_on_invalid_destination() {
        let adapter = ApiCallbackConfirmationAdapter::with_default_timeout().unwrap();
        let confirmation = create_test_confirmation();
        let destination = NotificationDestination::Email("test@example.com");

        let result = adapter.send(&confirmation, destination).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            DomainError::InvalidNotificationPreferences { .. }
        ));
    }

    #[tokio::test]
    async fn send_invalid_url_fails() {
        let adapter = ApiCallbackConfirmationAdapter::with_default_timeout().unwrap();
        let confirmation = create_test_confirmation();
        let destination = NotificationDestination::Webhook("not-a-valid-url");

        let result = adapter.send(&confirmation, destination).await;
        assert!(result.is_err());
    }

    // Note: Integration tests with actual HTTP server would go in tests/ directory
}
