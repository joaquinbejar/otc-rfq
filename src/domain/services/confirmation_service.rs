//! # Confirmation Service
//!
//! Multi-channel trade confirmation delivery service.
//!
//! This module provides the [`ConfirmationService`] trait and
//! [`MultiChannelConfirmationService`] implementation for delivering
//! trade confirmations to counterparties via multiple channels
//! (WebSocket, Email, API callbacks, gRPC).

use crate::domain::errors::{DomainError, DomainResult};
use crate::domain::value_objects::NotificationPreferences;
use crate::domain::value_objects::confirmation::{
    ChannelDeliveryStatus, ConfirmationChannel, ConfirmationStatus, NotificationDestination,
    TradeConfirmation,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::Arc;
use std::time::Duration;

/// Configuration for confirmation delivery.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfirmationConfig {
    /// Maximum retry attempts per channel.
    pub max_retry_attempts: u32,
    /// Initial retry delay.
    pub initial_retry_delay: Duration,
    /// Maximum retry delay (for exponential backoff).
    pub max_retry_delay: Duration,
    /// Timeout per channel delivery attempt.
    pub timeout_per_channel: Duration,
}

impl Default for ConfirmationConfig {
    fn default() -> Self {
        Self {
            max_retry_attempts: 3,
            initial_retry_delay: Duration::from_secs(1),
            max_retry_delay: Duration::from_secs(30),
            timeout_per_channel: Duration::from_secs(10),
        }
    }
}

impl ConfirmationConfig {
    /// Creates a new confirmation configuration.
    #[must_use]
    pub fn new(
        max_retry_attempts: u32,
        initial_retry_delay: Duration,
        max_retry_delay: Duration,
        timeout_per_channel: Duration,
    ) -> Self {
        Self {
            max_retry_attempts,
            initial_retry_delay,
            max_retry_delay,
            timeout_per_channel,
        }
    }

    /// Creates a configuration for testing (no retries, short timeout).
    #[must_use]
    pub fn for_testing() -> Self {
        Self {
            max_retry_attempts: 0,
            initial_retry_delay: Duration::from_millis(100),
            max_retry_delay: Duration::from_millis(500),
            timeout_per_channel: Duration::from_secs(1),
        }
    }

    /// Calculates the retry delay for a given attempt using exponential backoff.
    #[must_use]
    pub fn retry_delay(&self, attempt: u32) -> Duration {
        // Cap the exponent to prevent overflow (2^10 = 1024 is more than enough)
        let capped_attempt = attempt.min(10);
        let delay_ms = self
            .initial_retry_delay
            .as_millis()
            .saturating_mul(2_u128.saturating_pow(capped_attempt));
        let delay = Duration::from_millis(delay_ms.min(u64::MAX as u128) as u64);
        delay.min(self.max_retry_delay)
    }
}

/// Trait for confirmation channel adapters.
///
/// Adapters are responsible for the actual delivery of trade
/// confirmations via a specific channel (Email, Webhook, etc.).
#[async_trait]
pub trait ConfirmationChannelAdapter: Send + Sync + fmt::Debug {
    /// Sends a trade confirmation via the adapter's channel.
    ///
    /// # Arguments
    ///
    /// * `confirmation` - The trade confirmation data to send.
    /// * `destination` - The targeted destination for this delivery.
    ///
    /// # Errors
    ///
    /// Returns a `DomainError` if the delivery fails.
    async fn send(
        &self,
        confirmation: &TradeConfirmation,
        destination: NotificationDestination<'_>,
    ) -> DomainResult<()>;

    /// Returns the type of channel this adapter supports.
    #[must_use]
    fn channel_type(&self) -> ConfirmationChannel;
}

/// Trait for multi-channel confirmation service.
#[async_trait]
pub trait ConfirmationService: Send + Sync + fmt::Debug {
    /// Sends a trade confirmation via all enabled channels.
    ///
    /// # Arguments
    ///
    /// * `confirmation` - The trade confirmation to send
    /// * `preferences` - The notification preferences for the counterparty
    ///
    /// # Returns
    ///
    /// The overall confirmation status indicating which channels succeeded/failed.
    ///
    /// # Errors
    ///
    /// This method does not return an error. Individual channel failures are
    /// captured in the returned `ConfirmationStatus`.
    async fn send_confirmation(
        &self,
        confirmation: &TradeConfirmation,
        preferences: &NotificationPreferences,
    ) -> ConfirmationStatus;
}

/// Multi-channel confirmation service implementation.
///
/// Delivers trade confirmations to counterparties via multiple channels
/// with retry logic and exponential backoff.
#[derive(Debug)]
pub struct MultiChannelConfirmationService {
    /// Channel adapters.
    adapters: Vec<Arc<dyn ConfirmationChannelAdapter>>,
    /// Configuration.
    config: ConfirmationConfig,
}

impl MultiChannelConfirmationService {
    /// Creates a new multi-channel confirmation service.
    #[must_use]
    pub fn new(
        adapters: Vec<Arc<dyn ConfirmationChannelAdapter>>,
        config: ConfirmationConfig,
    ) -> Self {
        Self { adapters, config }
    }

    /// Sends confirmation with timeout (static version).
    async fn send_with_timeout_static(
        config: &ConfirmationConfig,
        adapter: &Arc<dyn ConfirmationChannelAdapter>,
        confirmation: &TradeConfirmation,
        destination: NotificationDestination<'_>,
    ) -> DomainResult<()> {
        tokio::time::timeout(
            config.timeout_per_channel,
            adapter.send(confirmation, destination),
        )
        .await
        .map_err(|_| DomainError::ConfirmationFailed {
            channel: adapter.channel_type().to_string(),
            reason: "Timeout".to_string(),
        })?
    }

    /// Static helper for sending with retry logic to avoid 'self' lifetime issues in spawned tasks.
    async fn send_with_retry_static(
        config: &ConfirmationConfig,
        adapter: &Arc<dyn ConfirmationChannelAdapter>,
        confirmation: &TradeConfirmation,
        destination: NotificationDestination<'_>,
    ) -> ChannelDeliveryStatus {
        let channel = adapter.channel_type();
        let mut attempts = 0;

        loop {
            // Pass destination by copy for each attempt in the retry loop
            match Self::send_with_timeout_static(config, adapter, confirmation, destination).await {
                Ok(()) => {
                    tracing::info!(
                        channel = %channel,
                        trade_id = %confirmation.trade_id(),
                        attempts = attempts,
                        "Confirmation delivered successfully"
                    );
                    return ChannelDeliveryStatus::success(channel, attempts);
                }
                Err(e) => {
                    attempts = attempts.saturating_add(1);

                    if attempts > config.max_retry_attempts {
                        tracing::error!(
                            channel = %channel,
                            trade_id = %confirmation.trade_id(),
                            attempts = attempts,
                            error = %e,
                            "Confirmation delivery failed after max retries"
                        );
                        return ChannelDeliveryStatus::failed(
                            channel,
                            format!("Failed after {} attempts: {}", attempts, e),
                            attempts.saturating_sub(1),
                        );
                    }

                    let delay = config.retry_delay(attempts.saturating_sub(1));
                    tracing::warn!(
                        channel = %channel,
                        trade_id = %confirmation.trade_id(),
                        attempt = attempts,
                        delay_ms = delay.as_millis(),
                        error = %e,
                        "Confirmation delivery failed, retrying"
                    );
                    tokio::time::sleep(delay).await;
                }
            }
        }
    }
}

#[async_trait]
impl ConfirmationService for MultiChannelConfirmationService {
    async fn send_confirmation(
        &self,
        confirmation: &TradeConfirmation,
        preferences: &NotificationPreferences,
    ) -> ConfirmationStatus {
        if !preferences.has_any_enabled() {
            tracing::debug!(
                trade_id = %confirmation.trade_id(),
                "No notification channels enabled, skipping confirmation"
            );
            return ConfirmationStatus::AllSent;
        }

        let enabled_channels = preferences.enabled_channels();

        // Find adapters for enabled channels
        let active_adapters: Vec<_> = self
            .adapters
            .iter()
            .filter(|adapter| enabled_channels.contains(&adapter.channel_type()))
            .collect();

        if active_adapters.is_empty() {
            tracing::warn!(
                trade_id = %confirmation.trade_id(),
                enabled_channels = ?enabled_channels,
                "No adapters found for enabled channels"
            );
            return ConfirmationStatus::AllSent;
        }

        tracing::info!(
            trade_id = %confirmation.trade_id(),
            channels = ?enabled_channels,
            "Sending confirmation via {} channels",
            active_adapters.len()
        );

        // Send to all channels in parallel
        let mut tasks = Vec::with_capacity(active_adapters.len());

        for adapter in active_adapters {
            let adapter = Arc::clone(adapter);
            let confirmation = confirmation.clone();
            let config = self.config.clone();
            let channel = adapter.channel_type();

            // Extract destinations as owned strings to move into the task
            let email_str = preferences.email_address().map(|s| s.to_string());
            let webhook_str = preferences.webhook_url().map(|s| s.to_string());
            let grpc_endpoint = preferences.grpc_endpoint().map(|s| s.to_string());

            tasks.push(tokio::spawn(async move {
                // Validate destination before sending
                let destination = match channel {
                    ConfirmationChannel::Email => {
                        email_str.as_deref().map(NotificationDestination::Email)
                    }
                    ConfirmationChannel::ApiCallback => {
                        webhook_str.as_deref().map(NotificationDestination::Webhook)
                    }
                    ConfirmationChannel::WebSocket => Some(NotificationDestination::WebSocket),
                    ConfirmationChannel::Grpc => {
                        Some(NotificationDestination::Grpc(grpc_endpoint.as_deref()))
                    }
                };

                if let Some(dest) = destination {
                    Self::send_with_retry_static(&config, &adapter, &confirmation, dest).await
                } else {
                    ChannelDeliveryStatus::failed(
                        channel,
                        format!("Missing required destination for channel {}", channel),
                        0,
                    )
                }
            }));
        }

        // Wait for all tasks to complete
        let mut results = Vec::new();
        for task in tasks {
            match task.await {
                Ok(status) => results.push(status),
                Err(e) => {
                    tracing::error!(
                        trade_id = %confirmation.trade_id(),
                        error = %e,
                        "Task panicked during confirmation delivery"
                    );
                }
            }
        }

        let status = ConfirmationStatus::from_results(results);

        tracing::info!(
            trade_id = %confirmation.trade_id(),
            status = %status,
            "Confirmation delivery completed"
        );

        status
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::clone_on_ref_ptr)]
mod tests {
    use super::*;
    use crate::domain::value_objects::{
        Blockchain, CounterpartyId, Price, Quantity, RfqId, SettlementMethod, TradeId,
        TradeParticipant,
    };
    use rust_decimal::Decimal;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[derive(Debug)]
    struct MockAdapter {
        channel_type: ConfirmationChannel,
        should_fail: bool,
        call_count: Arc<AtomicU32>,
    }

    impl MockAdapter {
        fn new(channel_type: ConfirmationChannel, should_fail: bool) -> Self {
            Self {
                channel_type,
                should_fail,
                call_count: Arc::new(AtomicU32::new(0)),
            }
        }

        fn call_count(&self) -> u32 {
            self.call_count.load(Ordering::SeqCst)
        }
    }

    #[async_trait]
    impl ConfirmationChannelAdapter for MockAdapter {
        async fn send(
            &self,
            _confirmation: &TradeConfirmation,
            _destination: NotificationDestination<'_>,
        ) -> DomainResult<()> {
            self.call_count.fetch_add(1, Ordering::SeqCst);

            if self.should_fail {
                Err(DomainError::ConfirmationFailed {
                    channel: self.channel_type.to_string(),
                    reason: "Mock failure".to_string(),
                })
            } else {
                Ok(())
            }
        }

        fn channel_type(&self) -> ConfirmationChannel {
            self.channel_type
        }
    }

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
    fn confirmation_config_default() {
        let config = ConfirmationConfig::default();
        assert_eq!(config.max_retry_attempts, 3);
        assert_eq!(config.initial_retry_delay, Duration::from_secs(1));
        assert_eq!(config.max_retry_delay, Duration::from_secs(30));
        assert_eq!(config.timeout_per_channel, Duration::from_secs(10));
    }

    #[test]
    fn confirmation_config_retry_delay_exponential_backoff() {
        let config = ConfirmationConfig::default();

        assert_eq!(config.retry_delay(0), Duration::from_secs(1));
        assert_eq!(config.retry_delay(1), Duration::from_secs(2));
        assert_eq!(config.retry_delay(2), Duration::from_secs(4));
        assert_eq!(config.retry_delay(3), Duration::from_secs(8));
        assert_eq!(config.retry_delay(4), Duration::from_secs(16));
        assert_eq!(config.retry_delay(5), Duration::from_secs(30)); // capped at max
    }

    #[tokio::test]
    async fn send_confirmation_no_channels_enabled() {
        let service =
            MultiChannelConfirmationService::new(vec![], ConfirmationConfig::for_testing());
        let confirmation = create_test_confirmation();
        let preferences = NotificationPreferences::none();

        let status = service.send_confirmation(&confirmation, &preferences).await;
        assert!(status.is_all_sent());
    }

    #[tokio::test]
    async fn send_confirmation_single_channel_success() {
        let adapter = Arc::new(MockAdapter::new(ConfirmationChannel::Email, false));
        let service = MultiChannelConfirmationService::new(
            vec![Arc::clone(&adapter) as Arc<dyn ConfirmationChannelAdapter>],
            ConfirmationConfig::for_testing(),
        );

        let confirmation = create_test_confirmation();
        let preferences =
            NotificationPreferences::email_only("test@example.com".to_string()).unwrap();

        let status = service.send_confirmation(&confirmation, &preferences).await;
        assert!(status.is_all_sent());
        assert_eq!(adapter.call_count(), 1);
    }

    #[tokio::test]
    async fn send_confirmation_single_channel_failure() {
        let adapter = Arc::new(MockAdapter::new(ConfirmationChannel::Email, true));
        let service = MultiChannelConfirmationService::new(
            vec![Arc::clone(&adapter) as Arc<dyn ConfirmationChannelAdapter>],
            ConfirmationConfig::for_testing(),
        );

        let confirmation = create_test_confirmation();
        let preferences =
            NotificationPreferences::email_only("test@example.com".to_string()).unwrap();

        let status = service.send_confirmation(&confirmation, &preferences).await;
        assert!(status.is_all_failed());
        assert_eq!(adapter.call_count(), 1); // No retries in test config
    }

    #[tokio::test]
    async fn send_confirmation_multi_channel_all_success() {
        let email_adapter = Arc::new(MockAdapter::new(ConfirmationChannel::Email, false));
        let ws_adapter = Arc::new(MockAdapter::new(ConfirmationChannel::WebSocket, false));

        let service = MultiChannelConfirmationService::new(
            vec![
                Arc::clone(&email_adapter) as Arc<dyn ConfirmationChannelAdapter>,
                Arc::clone(&ws_adapter) as Arc<dyn ConfirmationChannelAdapter>,
            ],
            ConfirmationConfig::for_testing(),
        );

        let confirmation = create_test_confirmation();
        let preferences = NotificationPreferences::new(
            vec![ConfirmationChannel::Email, ConfirmationChannel::WebSocket],
            Some("test@example.com".to_string()),
            None,
            None,
        )
        .unwrap();

        let status = service.send_confirmation(&confirmation, &preferences).await;
        assert!(status.is_all_sent());
        assert_eq!(email_adapter.call_count(), 1);
        assert_eq!(ws_adapter.call_count(), 1);
    }

    #[tokio::test]
    async fn send_confirmation_multi_channel_partial_success() {
        let email_adapter = Arc::new(MockAdapter::new(ConfirmationChannel::Email, false));
        let ws_adapter = Arc::new(MockAdapter::new(ConfirmationChannel::WebSocket, true));

        let service = MultiChannelConfirmationService::new(
            vec![
                Arc::clone(&email_adapter) as Arc<dyn ConfirmationChannelAdapter>,
                Arc::clone(&ws_adapter) as Arc<dyn ConfirmationChannelAdapter>,
            ],
            ConfirmationConfig::for_testing(),
        );

        let confirmation = create_test_confirmation();
        let preferences = NotificationPreferences::new(
            vec![ConfirmationChannel::Email, ConfirmationChannel::WebSocket],
            Some("test@example.com".to_string()),
            None,
            None,
        )
        .unwrap();

        let status = service.send_confirmation(&confirmation, &preferences).await;
        assert!(status.is_partial_success());
        assert_eq!(email_adapter.call_count(), 1);
        assert_eq!(ws_adapter.call_count(), 1);
    }

    #[tokio::test]
    async fn send_confirmation_with_retry() {
        let adapter = Arc::new(MockAdapter::new(ConfirmationChannel::Email, true));
        let config = ConfirmationConfig::new(
            2, // 2 retries
            Duration::from_millis(10),
            Duration::from_millis(100),
            Duration::from_secs(1),
        );

        let service = MultiChannelConfirmationService::new(
            vec![Arc::clone(&adapter) as Arc<dyn ConfirmationChannelAdapter>],
            config,
        );
        let confirmation = create_test_confirmation();
        let preferences =
            NotificationPreferences::email_only("test@example.com".to_string()).unwrap();

        let status = service.send_confirmation(&confirmation, &preferences).await;
        assert!(status.is_all_failed());
        assert_eq!(adapter.call_count(), 3); // Initial + 2 retries
    }
}
