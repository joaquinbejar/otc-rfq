//! # Email Confirmation Adapter
//!
//! Delivers trade confirmations via email using SMTP.

use crate::domain::errors::{DomainError, DomainResult};
use crate::domain::services::confirmation_service::ConfirmationChannelAdapter;
use crate::domain::value_objects::confirmation::{
    ConfirmationChannel, NotificationDestination, TradeConfirmation,
};
use async_trait::async_trait;
use lettre::message::{MultiPart, SinglePart, header};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use serde::{Deserialize, Serialize};

/// SMTP configuration for email delivery.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SmtpConfig {
    /// SMTP server host.
    pub host: String,
    /// SMTP server port.
    pub port: u16,
    /// SMTP username.
    pub username: String,
    /// SMTP password.
    pub password: String,
    /// From email address.
    pub from_address: String,
    /// From name.
    pub from_name: String,
}

impl Default for SmtpConfig {
    fn default() -> Self {
        Self {
            host: "localhost".to_string(),
            port: 587,
            username: "noreply".to_string(),
            password: String::new(),
            from_address: "noreply@example.com".to_string(),
            from_name: "OTC Trading Platform".to_string(),
        }
    }
}

/// Email confirmation adapter.
#[derive(Debug)]
pub struct EmailConfirmationAdapter {
    config: SmtpConfig,
}

impl EmailConfirmationAdapter {
    /// Creates a new email confirmation adapter.
    #[must_use]
    pub fn new(config: SmtpConfig) -> Self {
        Self { config }
    }

    /// Generates HTML email body for trade confirmation.
    fn generate_html_body(&self, confirmation: &TradeConfirmation) -> String {
        format!(
            r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <style>
        body {{ font-family: Arial, sans-serif; line-height: 1.6; color: #333; }}
        .container {{ max-width: 600px; margin: 0 auto; padding: 20px; }}
        .header {{ background-color: #2c3e50; color: white; padding: 20px; text-align: center; }}
        .content {{ background-color: #f9f9f9; padding: 20px; }}
        .detail-row {{ display: flex; justify-content: space-between; padding: 10px 0; border-bottom: 1px solid #ddd; }}
        .label {{ font-weight: bold; color: #555; }}
        .value {{ color: #2c3e50; }}
        .footer {{ text-align: center; padding: 20px; font-size: 12px; color: #777; }}
        .highlight {{ background-color: #3498db; color: white; padding: 15px; margin: 20px 0; text-align: center; font-size: 18px; }}
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>Trade Confirmation</h1>
        </div>
        <div class="content">
            <p>Dear Valued Client,</p>
            <p>This is to confirm the execution of your trade with the following details:</p>
            
            <div class="highlight">
                Trade ID: {trade_id}
            </div>
            
            <div class="detail-row">
                <span class="label">RFQ ID:</span>
                <span class="value">{rfq_id}</span>
            </div>
            <div class="detail-row">
                <span class="label">Price:</span>
                <span class="value">{price}</span>
            </div>
            <div class="detail-row">
                <span class="label">Quantity:</span>
                <span class="value">{quantity}</span>
            </div>
            <div class="detail-row">
                <span class="label">Taker Fee:</span>
                <span class="value">{taker_fee}</span>
            </div>
            <div class="detail-row">
                <span class="label">Maker Fee:</span>
                <span class="value">{maker_fee}</span>
            </div>
            <div class="detail-row">
                <span class="label">Net Fee:</span>
                <span class="value">{net_fee}</span>
            </div>
            <div class="detail-row">
                <span class="label">Settlement Method:</span>
                <span class="value">{settlement_method}</span>
            </div>
            <div class="detail-row">
                <span class="label">Buyer:</span>
                <span class="value">{buyer:?}</span>
            </div>
            <div class="detail-row">
                <span class="label">Seller:</span>
                <span class="value">{seller:?}</span>
            </div>
            <div class="detail-row">
                <span class="label">Timestamp:</span>
                <span class="value">{timestamp}</span>
            </div>
            
            <p style="margin-top: 20px;">If you have any questions regarding this trade, please contact our support team.</p>
        </div>
        <div class="footer">
            <p>This is an automated message. Please do not reply to this email.</p>
            <p>&copy; 2026 OTC Trading Platform. All rights reserved.</p>
        </div>
    </div>
</body>
</html>"#,
            trade_id = confirmation.trade_id(),
            rfq_id = confirmation.rfq_id(),
            price = confirmation.price(),
            quantity = confirmation.quantity(),
            taker_fee = confirmation.taker_fee(),
            maker_fee = confirmation.maker_fee(),
            net_fee = confirmation.net_fee(),
            settlement_method = confirmation.settlement_method(),
            buyer = confirmation.buyer(),
            seller = confirmation.seller(),
            timestamp = confirmation.timestamp(),
        )
    }

    /// Generates plain text email body for trade confirmation.
    fn generate_text_body(&self, confirmation: &TradeConfirmation) -> String {
        format!(
            r#"TRADE CONFIRMATION

Dear Valued Client,

This is to confirm the execution of your trade with the following details:

Trade ID: {}
RFQ ID: {}
Price: {}
Quantity: {}
Taker Fee: {}
Maker Fee: {}
Net Fee: {}
Settlement Method: {}
Buyer: {:?}
Seller: {:?}
Timestamp: {}

If you have any questions regarding this trade, please contact our support team.

---
This is an automated message. Please do not reply to this email.
© 2026 OTC Trading Platform. All rights reserved.
"#,
            confirmation.trade_id(),
            confirmation.rfq_id(),
            confirmation.price(),
            confirmation.quantity(),
            confirmation.taker_fee(),
            confirmation.maker_fee(),
            confirmation.net_fee(),
            confirmation.settlement_method(),
            confirmation.buyer(),
            confirmation.seller(),
            confirmation.timestamp(),
        )
    }
}

#[async_trait]
impl ConfirmationChannelAdapter for EmailConfirmationAdapter {
    async fn send(
        &self,
        confirmation: &TradeConfirmation,
        destination: NotificationDestination<'_>,
    ) -> DomainResult<()> {
        // Extract email address from destination
        let to_address = match destination {
            NotificationDestination::Email(addr) => addr,
            _ => {
                return Err(DomainError::InvalidNotificationPreferences {
                    reason: "Expected Email destination".to_string(),
                });
            }
        };

        // Build email message
        let email = Message::builder()
            .from(
                format!("{} <{}>", self.config.from_name, self.config.from_address)
                    .parse()
                    .map_err(|e| DomainError::ConfirmationFailed {
                        channel: "EMAIL".to_string(),
                        reason: format!("Invalid from address: {}", e),
                    })?,
            )
            .to(to_address
                .parse()
                .map_err(|e| DomainError::ConfirmationFailed {
                    channel: "EMAIL".to_string(),
                    reason: format!("Invalid to address: {}", e),
                })?)
            .subject(format!("Trade Confirmation - {}", confirmation.trade_id()))
            .multipart(
                MultiPart::alternative()
                    .singlepart(
                        SinglePart::builder()
                            .header(header::ContentType::TEXT_PLAIN)
                            .body(self.generate_text_body(confirmation)),
                    )
                    .singlepart(
                        SinglePart::builder()
                            .header(header::ContentType::TEXT_HTML)
                            .body(self.generate_html_body(confirmation)),
                    ),
            )
            .map_err(|e| DomainError::ConfirmationFailed {
                channel: "EMAIL".to_string(),
                reason: format!("Failed to build email: {}", e),
            })?;

        // Create SMTP transport
        let creds = Credentials::new(self.config.username.clone(), self.config.password.clone());

        let mailer: AsyncSmtpTransport<Tokio1Executor> =
            AsyncSmtpTransport::<Tokio1Executor>::relay(&self.config.host)
                .map_err(|e| DomainError::ConfirmationFailed {
                    channel: "EMAIL".to_string(),
                    reason: format!("Failed to create SMTP transport: {}", e),
                })?
                .port(self.config.port)
                .credentials(creds)
                .build();

        // Send email
        mailer
            .send(email)
            .await
            .map_err(|e| DomainError::ConfirmationFailed {
                channel: "EMAIL".to_string(),
                reason: format!("SMTP send failed: {}", e),
            })?;

        tracing::info!(
            trade_id = %confirmation.trade_id(),
            to = %to_address,
            "Email confirmation sent successfully"
        );

        Ok(())
    }

    fn channel_type(&self) -> ConfirmationChannel {
        ConfirmationChannel::Email
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
    fn generate_html_body_contains_trade_details() {
        let config = SmtpConfig::default();
        let adapter = EmailConfirmationAdapter::new(config);
        let confirmation = create_test_confirmation();

        let html = adapter.generate_html_body(&confirmation);

        assert!(html.contains(&confirmation.trade_id().to_string()));
        assert!(html.contains(&confirmation.rfq_id().to_string()));
        assert!(html.contains(&confirmation.price().to_string()));
        assert!(html.contains(&confirmation.quantity().to_string()));
        assert!(html.contains("Trade Confirmation"));
    }

    #[test]
    fn generate_text_body_contains_trade_details() {
        let config = SmtpConfig::default();
        let adapter = EmailConfirmationAdapter::new(config);
        let confirmation = create_test_confirmation();

        let text = adapter.generate_text_body(&confirmation);

        assert!(text.contains(&confirmation.trade_id().to_string()));
        assert!(text.contains(&confirmation.rfq_id().to_string()));
        assert!(text.contains(&confirmation.price().to_string()));
        assert!(text.contains(&confirmation.quantity().to_string()));
        assert!(text.contains("TRADE CONFIRMATION"));
    }

    #[test]
    fn channel_returns_email() {
        let config = SmtpConfig::default();
        let adapter = EmailConfirmationAdapter::new(config);
        assert_eq!(adapter.channel_type(), ConfirmationChannel::Email);
    }

    #[tokio::test]
    async fn send_error_on_invalid_destination() {
        let config = SmtpConfig::default();
        let adapter = EmailConfirmationAdapter::new(config);
        let confirmation = create_test_confirmation();
        let destination = NotificationDestination::WebSocket;

        let result = adapter.send(&confirmation, destination).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            DomainError::InvalidNotificationPreferences { .. }
        ));
    }
}
