//! # Notification Preferences Value Object
//!
//! Value object for counterparty notification preferences.
//!
//! This module provides the `NotificationPreferences` value object that
//! encapsulates which notification channels are enabled for a counterparty
//! and the necessary configuration for each channel.

use crate::domain::errors::{DomainError, DomainResult};
use crate::domain::value_objects::confirmation::ConfirmationChannel;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Notification preferences for a counterparty.
///
/// Defines which notification channels are enabled and the configuration
/// needed for each channel (email address, webhook URL, gRPC endpoint).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotificationPreferences {
    /// Enabled notification channels.
    enabled_channels: Vec<ConfirmationChannel>,
    /// Email address for email notifications.
    email_address: Option<String>,
    /// Webhook URL for API callback notifications.
    webhook_url: Option<String>,
    /// gRPC endpoint for gRPC notifications.
    grpc_endpoint: Option<String>,
}

impl NotificationPreferences {
    /// Creates new notification preferences.
    ///
    /// # Arguments
    ///
    /// * `enabled_channels` - List of enabled notification channels
    /// * `email_address` - Email address (required if Email channel enabled)
    /// * `webhook_url` - Webhook URL (required if ApiCallback channel enabled)
    /// * `grpc_endpoint` - gRPC endpoint (required if Grpc channel enabled)
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Email channel is enabled but email_address is None or invalid
    /// - ApiCallback channel is enabled but webhook_url is None or invalid
    /// - Grpc channel is enabled but grpc_endpoint is None
    pub fn new(
        enabled_channels: Vec<ConfirmationChannel>,
        email_address: Option<String>,
        webhook_url: Option<String>,
        grpc_endpoint: Option<String>,
    ) -> DomainResult<Self> {
        let prefs = Self {
            enabled_channels,
            email_address,
            webhook_url,
            grpc_endpoint,
        };

        prefs.validate()?;
        Ok(prefs)
    }

    /// Creates notification preferences with all channels disabled.
    #[must_use]
    pub fn none() -> Self {
        Self {
            enabled_channels: Vec::new(),
            email_address: None,
            webhook_url: None,
            grpc_endpoint: None,
        }
    }

    /// Creates notification preferences with only email enabled.
    ///
    /// # Errors
    ///
    /// Returns an error if the email address is invalid.
    pub fn email_only(email_address: String) -> DomainResult<Self> {
        Self::new(
            vec![ConfirmationChannel::Email],
            Some(email_address),
            None,
            None,
        )
    }

    /// Creates notification preferences with only webhook enabled.
    ///
    /// # Errors
    ///
    /// Returns an error if the webhook URL is invalid.
    pub fn webhook_only(webhook_url: String) -> DomainResult<Self> {
        Self::new(
            vec![ConfirmationChannel::ApiCallback],
            None,
            Some(webhook_url),
            None,
        )
    }

    /// Returns the enabled channels.
    #[inline]
    #[must_use]
    pub fn enabled_channels(&self) -> &[ConfirmationChannel] {
        &self.enabled_channels
    }

    /// Returns the email address.
    #[inline]
    #[must_use]
    pub fn email_address(&self) -> Option<&str> {
        self.email_address.as_deref()
    }

    /// Returns the webhook URL.
    #[inline]
    #[must_use]
    pub fn webhook_url(&self) -> Option<&str> {
        self.webhook_url.as_deref()
    }

    /// Returns the gRPC endpoint.
    #[inline]
    #[must_use]
    pub fn grpc_endpoint(&self) -> Option<&str> {
        self.grpc_endpoint.as_deref()
    }

    /// Returns true if the specified channel is enabled.
    #[must_use]
    pub fn is_channel_enabled(&self, channel: ConfirmationChannel) -> bool {
        self.enabled_channels.contains(&channel)
    }

    /// Returns true if any channel is enabled.
    #[must_use]
    pub fn has_any_enabled(&self) -> bool {
        !self.enabled_channels.is_empty()
    }

    /// Validates the notification preferences.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Email channel is enabled but email_address is None or invalid
    /// - ApiCallback channel is enabled but webhook_url is None or invalid
    /// - Grpc channel is enabled but grpc_endpoint is None
    fn validate(&self) -> DomainResult<()> {
        for channel in &self.enabled_channels {
            match channel {
                ConfirmationChannel::Email => {
                    if let Some(email) = &self.email_address {
                        Self::validate_email(email)?;
                    } else {
                        return Err(DomainError::InvalidNotificationPreferences {
                            reason: "Email channel enabled but no email address provided"
                                .to_string(),
                        });
                    }
                }
                ConfirmationChannel::ApiCallback => {
                    if let Some(url) = &self.webhook_url {
                        Self::validate_url(url)?;
                    } else {
                        return Err(DomainError::InvalidNotificationPreferences {
                            reason: "ApiCallback channel enabled but no webhook URL provided"
                                .to_string(),
                        });
                    }
                }
                ConfirmationChannel::Grpc => {
                    if self.grpc_endpoint.is_none() {
                        return Err(DomainError::InvalidNotificationPreferences {
                            reason: "Grpc channel enabled but no gRPC endpoint provided"
                                .to_string(),
                        });
                    }
                }
                ConfirmationChannel::WebSocket => {
                    // WebSocket doesn't require additional configuration
                }
            }
        }
        Ok(())
    }

    /// Validates an email address.
    ///
    /// # Errors
    ///
    /// Returns an error if the email address is invalid.
    fn validate_email(email: &str) -> DomainResult<()> {
        if email.is_empty() {
            return Err(DomainError::InvalidNotificationPreferences {
                reason: "Email address cannot be empty".to_string(),
            });
        }

        if !email.contains('@') {
            return Err(DomainError::InvalidNotificationPreferences {
                reason: "Invalid email address format".to_string(),
            });
        }

        let parts: Vec<&str> = email.split('@').collect();
        if parts.len() != 2 {
            return Err(DomainError::InvalidNotificationPreferences {
                reason: "Invalid email address format".to_string(),
            });
        }

        let local = parts.first().copied().unwrap_or("");
        let domain = parts.get(1).copied().unwrap_or("");

        if local.is_empty() || domain.is_empty() {
            return Err(DomainError::InvalidNotificationPreferences {
                reason: "Invalid email address format".to_string(),
            });
        }

        // Check domain has at least one dot
        if !domain.contains('.') {
            return Err(DomainError::InvalidNotificationPreferences {
                reason: "Invalid email domain".to_string(),
            });
        }

        Ok(())
    }

    /// Validates a URL.
    ///
    /// # Errors
    ///
    /// Returns an error if the URL is invalid.
    fn validate_url(url: &str) -> DomainResult<()> {
        if url.is_empty() {
            return Err(DomainError::InvalidNotificationPreferences {
                reason: "URL cannot be empty".to_string(),
            });
        }

        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err(DomainError::InvalidNotificationPreferences {
                reason: "URL must start with http:// or https://".to_string(),
            });
        }

        Ok(())
    }
}

impl Default for NotificationPreferences {
    fn default() -> Self {
        Self::none()
    }
}

impl fmt::Display for NotificationPreferences {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.enabled_channels.is_empty() {
            write!(f, "No notifications enabled")
        } else {
            write!(
                f,
                "Enabled channels: {}",
                self.enabled_channels
                    .iter()
                    .map(|c| c.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn none_creates_empty_preferences() {
        let prefs = NotificationPreferences::none();
        assert!(prefs.enabled_channels().is_empty());
        assert!(!prefs.has_any_enabled());
    }

    #[test]
    fn email_only_creates_valid_preferences() {
        let prefs = NotificationPreferences::email_only("test@example.com".to_string()).unwrap();
        assert_eq!(prefs.enabled_channels().len(), 1);
        assert!(prefs.is_channel_enabled(ConfirmationChannel::Email));
        assert_eq!(prefs.email_address(), Some("test@example.com"));
    }

    #[test]
    fn email_only_rejects_invalid_email() {
        let result = NotificationPreferences::email_only("invalid-email".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn webhook_only_creates_valid_preferences() {
        let prefs =
            NotificationPreferences::webhook_only("https://example.com/webhook".to_string())
                .unwrap();
        assert_eq!(prefs.enabled_channels().len(), 1);
        assert!(prefs.is_channel_enabled(ConfirmationChannel::ApiCallback));
        assert_eq!(prefs.webhook_url(), Some("https://example.com/webhook"));
    }

    #[test]
    fn webhook_only_rejects_invalid_url() {
        let result = NotificationPreferences::webhook_only("not-a-url".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn new_validates_email_channel_requires_address() {
        let result =
            NotificationPreferences::new(vec![ConfirmationChannel::Email], None, None, None);
        assert!(result.is_err());
    }

    #[test]
    fn new_validates_api_callback_requires_url() {
        let result =
            NotificationPreferences::new(vec![ConfirmationChannel::ApiCallback], None, None, None);
        assert!(result.is_err());
    }

    #[test]
    fn new_validates_grpc_requires_endpoint() {
        let result =
            NotificationPreferences::new(vec![ConfirmationChannel::Grpc], None, None, None);
        assert!(result.is_err());
    }

    #[test]
    fn new_allows_websocket_without_config() {
        let result =
            NotificationPreferences::new(vec![ConfirmationChannel::WebSocket], None, None, None);
        assert!(result.is_ok());
    }

    #[test]
    fn new_creates_multi_channel_preferences() {
        let prefs = NotificationPreferences::new(
            vec![
                ConfirmationChannel::Email,
                ConfirmationChannel::WebSocket,
                ConfirmationChannel::ApiCallback,
            ],
            Some("test@example.com".to_string()),
            Some("https://example.com/webhook".to_string()),
            None,
        )
        .unwrap();

        assert_eq!(prefs.enabled_channels().len(), 3);
        assert!(prefs.is_channel_enabled(ConfirmationChannel::Email));
        assert!(prefs.is_channel_enabled(ConfirmationChannel::WebSocket));
        assert!(prefs.is_channel_enabled(ConfirmationChannel::ApiCallback));
        assert!(!prefs.is_channel_enabled(ConfirmationChannel::Grpc));
    }

    #[test]
    fn validate_email_rejects_empty() {
        let result = NotificationPreferences::validate_email("");
        assert!(result.is_err());
    }

    #[test]
    fn validate_email_rejects_no_at_sign() {
        let result = NotificationPreferences::validate_email("invalid");
        assert!(result.is_err());
    }

    #[test]
    fn validate_email_rejects_no_domain() {
        let result = NotificationPreferences::validate_email("user@");
        assert!(result.is_err());
    }

    #[test]
    fn validate_email_rejects_no_local_part() {
        let result = NotificationPreferences::validate_email("@example.com");
        assert!(result.is_err());
    }

    #[test]
    fn validate_email_rejects_no_tld() {
        let result = NotificationPreferences::validate_email("user@domain");
        assert!(result.is_err());
    }

    #[test]
    fn validate_email_accepts_valid() {
        let result = NotificationPreferences::validate_email("user@example.com");
        assert!(result.is_ok());
    }

    #[test]
    fn validate_url_rejects_empty() {
        let result = NotificationPreferences::validate_url("");
        assert!(result.is_err());
    }

    #[test]
    fn validate_url_rejects_no_scheme() {
        let result = NotificationPreferences::validate_url("example.com");
        assert!(result.is_err());
    }

    #[test]
    fn validate_url_accepts_http() {
        let result = NotificationPreferences::validate_url("http://example.com");
        assert!(result.is_ok());
    }

    #[test]
    fn validate_url_accepts_https() {
        let result = NotificationPreferences::validate_url("https://example.com/path");
        assert!(result.is_ok());
    }

    #[test]
    fn display_shows_no_notifications_when_empty() {
        let prefs = NotificationPreferences::none();
        assert!(prefs.to_string().contains("No notifications"));
    }

    #[test]
    fn display_shows_enabled_channels() {
        let prefs = NotificationPreferences::new(
            vec![ConfirmationChannel::Email, ConfirmationChannel::WebSocket],
            Some("test@example.com".to_string()),
            None,
            None,
        )
        .unwrap();
        let display = prefs.to_string();
        assert!(display.contains("EMAIL"));
        assert!(display.contains("WEBSOCKET"));
    }
}
