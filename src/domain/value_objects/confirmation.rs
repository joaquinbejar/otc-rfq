//! # Trade Confirmation Value Objects
//!
//! Value objects for multi-channel trade confirmation.
//!
//! This module provides types for representing trade confirmations
//! that are sent to counterparties via multiple channels (WebSocket,
//! Email, API callbacks, gRPC).

use crate::domain::value_objects::enums::ParseEnumError;
use crate::domain::value_objects::timestamp::Timestamp;
use crate::domain::value_objects::{
    CounterpartyId, Price, Quantity, RfqId, SettlementMethod, TradeId, VenueId,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Confirmation channel type.
///
/// Represents the different channels through which trade confirmations
/// can be delivered to counterparties.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ConfirmationChannel {
    /// WebSocket push notification.
    WebSocket,
    /// Email notification.
    Email,
    /// HTTP API callback (webhook).
    ApiCallback,
    /// gRPC streaming notification.
    Grpc,
}

/// Targeted destination for a trade confirmation delivery.
///
/// This enum allows decoupling the adapters from the full
/// `NotificationPreferences` struct, ensuring they only receive
/// the specific routing information they need.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationDestination<'a> {
    /// Email address for email notifications.
    Email(&'a str),
    /// Webhook URL for API callback notifications.
    Webhook(&'a str),
    /// WebSocket destination (registry-based).
    WebSocket,
    /// gRPC destination (registry-based with optional endpoint).
    Grpc(Option<&'a str>),
}

impl ConfirmationChannel {
    /// Returns all available confirmation channels.
    #[must_use]
    pub fn all() -> Vec<Self> {
        vec![Self::WebSocket, Self::Email, Self::ApiCallback, Self::Grpc]
    }

    /// Returns the channel name as a string.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::WebSocket => "WEBSOCKET",
            Self::Email => "EMAIL",
            Self::ApiCallback => "API_CALLBACK",
            Self::Grpc => "GRPC",
        }
    }
}

impl fmt::Display for ConfirmationChannel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for ConfirmationChannel {
    type Err = ParseEnumError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.eq_ignore_ascii_case("WEBSOCKET") {
            Ok(Self::WebSocket)
        } else if s.eq_ignore_ascii_case("EMAIL") {
            Ok(Self::Email)
        } else if s.eq_ignore_ascii_case("API_CALLBACK") {
            Ok(Self::ApiCallback)
        } else if s.eq_ignore_ascii_case("GRPC") {
            Ok(Self::Grpc)
        } else {
            Err(ParseEnumError::InvalidValue(
                "ConfirmationChannel",
                s.to_string(),
            ))
        }
    }
}

/// A participant in a trade.
///
/// Represents either a counterparty (client) or a venue (market maker)
/// involved in a trade execution.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", content = "id")]
pub enum TradeParticipant {
    /// A counterparty (client or institutional trader).
    Counterparty(CounterpartyId),
    /// A venue (market maker or liquidity provider).
    Venue(VenueId),
}

impl TradeParticipant {
    /// Returns the counterparty ID if this is a counterparty participant.
    ///
    /// # Examples
    ///
    /// ```
    /// use otc_rfq::domain::value_objects::{TradeParticipant, CounterpartyId};
    ///
    /// let participant = TradeParticipant::Counterparty(CounterpartyId::new("client-1"));
    /// assert!(participant.as_counterparty().is_some());
    /// ```
    #[inline]
    #[must_use]
    pub fn as_counterparty(&self) -> Option<&CounterpartyId> {
        match self {
            Self::Counterparty(id) => Some(id),
            Self::Venue(_) => None,
        }
    }

    /// Returns the venue ID if this is a venue participant.
    ///
    /// # Examples
    ///
    /// ```
    /// use otc_rfq::domain::value_objects::{TradeParticipant, VenueId};
    ///
    /// let participant = TradeParticipant::Venue(VenueId::new("venue-1"));
    /// assert!(participant.as_venue().is_some());
    /// ```
    #[inline]
    #[must_use]
    pub fn as_venue(&self) -> Option<&VenueId> {
        match self {
            Self::Counterparty(_) => None,
            Self::Venue(id) => Some(id),
        }
    }
}

/// Trade confirmation data.
///
/// Contains all information needed to notify counterparties about
/// a completed trade execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TradeConfirmation {
    /// The trade ID.
    pub trade_id: TradeId,
    /// The RFQ ID.
    pub rfq_id: RfqId,
    /// The execution price.
    pub price: Price,
    /// The executed quantity.
    pub quantity: Quantity,
    /// Taker fee.
    pub taker_fee: Decimal,
    /// Maker fee.
    pub maker_fee: Decimal,
    /// Net fee (taker + maker).
    pub net_fee: Decimal,
    /// Settlement method.
    pub settlement_method: SettlementMethod,
    /// Buyer participant (counterparty or venue).
    pub buyer: TradeParticipant,
    /// Seller participant (counterparty or venue).
    pub seller: TradeParticipant,
    /// Confirmation timestamp.
    pub timestamp: Timestamp,
}

impl TradeConfirmation {
    /// Creates a new trade confirmation.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        trade_id: TradeId,
        rfq_id: RfqId,
        price: Price,
        quantity: Quantity,
        taker_fee: Decimal,
        maker_fee: Decimal,
        net_fee: Decimal,
        settlement_method: SettlementMethod,
        buyer: TradeParticipant,
        seller: TradeParticipant,
    ) -> Self {
        Self {
            trade_id,
            rfq_id,
            price,
            quantity,
            taker_fee,
            maker_fee,
            net_fee,
            settlement_method,
            buyer,
            seller,
            timestamp: Timestamp::now(),
        }
    }

    /// Returns the trade ID.
    #[inline]
    #[must_use]
    pub fn trade_id(&self) -> TradeId {
        self.trade_id
    }

    /// Returns the RFQ ID.
    #[inline]
    #[must_use]
    pub fn rfq_id(&self) -> RfqId {
        self.rfq_id
    }

    /// Returns the price.
    #[inline]
    #[must_use]
    pub fn price(&self) -> Price {
        self.price
    }

    /// Returns the quantity.
    #[inline]
    #[must_use]
    pub fn quantity(&self) -> Quantity {
        self.quantity
    }

    /// Returns the taker fee.
    #[inline]
    #[must_use]
    pub fn taker_fee(&self) -> Decimal {
        self.taker_fee
    }

    /// Returns the maker fee.
    #[inline]
    #[must_use]
    pub fn maker_fee(&self) -> Decimal {
        self.maker_fee
    }

    /// Returns the net fee.
    #[inline]
    #[must_use]
    pub fn net_fee(&self) -> Decimal {
        self.net_fee
    }

    /// Returns the settlement method.
    #[inline]
    #[must_use]
    pub fn settlement_method(&self) -> SettlementMethod {
        self.settlement_method
    }

    /// Returns the buyer participant.
    #[inline]
    #[must_use]
    pub fn buyer(&self) -> &TradeParticipant {
        &self.buyer
    }

    /// Returns the seller participant.
    #[inline]
    #[must_use]
    pub fn seller(&self) -> &TradeParticipant {
        &self.seller
    }

    /// Returns the timestamp.
    #[inline]
    #[must_use]
    pub fn timestamp(&self) -> Timestamp {
        self.timestamp
    }
}

/// Delivery status for a single channel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelDeliveryStatus {
    /// The channel.
    pub channel: ConfirmationChannel,
    /// Whether delivery succeeded.
    pub success: bool,
    /// Error message if delivery failed.
    pub error: Option<String>,
    /// Number of retry attempts made.
    pub retry_attempts: u32,
}

impl ChannelDeliveryStatus {
    /// Creates a successful delivery status.
    #[must_use]
    pub fn success(channel: ConfirmationChannel, retry_attempts: u32) -> Self {
        Self {
            channel,
            success: true,
            error: None,
            retry_attempts,
        }
    }

    /// Creates a failed delivery status.
    #[must_use]
    pub fn failed(channel: ConfirmationChannel, error: String, retry_attempts: u32) -> Self {
        Self {
            channel,
            success: false,
            error: Some(error),
            retry_attempts,
        }
    }
}

/// Overall confirmation status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConfirmationStatus {
    /// All channels delivered successfully.
    AllSent,
    /// Some channels failed.
    PartialSuccess {
        /// Channels that failed.
        failed_channels: Vec<ChannelDeliveryStatus>,
    },
    /// All channels failed.
    AllFailed {
        /// All channel statuses.
        channel_statuses: Vec<ChannelDeliveryStatus>,
    },
}

impl ConfirmationStatus {
    /// Creates a confirmation status from channel delivery results.
    #[must_use]
    pub fn from_results(results: Vec<ChannelDeliveryStatus>) -> Self {
        let failed: Vec<_> = results.iter().filter(|r| !r.success).cloned().collect();

        if failed.is_empty() {
            Self::AllSent
        } else if failed.len() == results.len() {
            Self::AllFailed {
                channel_statuses: results,
            }
        } else {
            Self::PartialSuccess {
                failed_channels: failed,
            }
        }
    }

    /// Returns true if all channels succeeded.
    #[must_use]
    pub fn is_all_sent(&self) -> bool {
        matches!(self, Self::AllSent)
    }

    /// Returns true if all channels failed.
    #[must_use]
    pub fn is_all_failed(&self) -> bool {
        matches!(self, Self::AllFailed { .. })
    }

    /// Returns true if some channels succeeded and some failed.
    #[must_use]
    pub fn is_partial_success(&self) -> bool {
        matches!(self, Self::PartialSuccess { .. })
    }
}

impl fmt::Display for ConfirmationStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AllSent => write!(f, "All confirmations sent successfully"),
            Self::PartialSuccess { failed_channels } => {
                write!(
                    f,
                    "Partial success ({} channels failed)",
                    failed_channels.len()
                )
            }
            Self::AllFailed { channel_statuses } => {
                write!(
                    f,
                    "All confirmations failed ({} channels)",
                    channel_statuses.len()
                )
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn confirmation_channel_all_returns_all_variants() {
        let channels = ConfirmationChannel::all();
        assert_eq!(channels.len(), 4);
        assert!(channels.contains(&ConfirmationChannel::WebSocket));
        assert!(channels.contains(&ConfirmationChannel::Email));
        assert!(channels.contains(&ConfirmationChannel::ApiCallback));
        assert!(channels.contains(&ConfirmationChannel::Grpc));
    }

    #[test]
    fn confirmation_channel_display() {
        assert_eq!(ConfirmationChannel::WebSocket.to_string(), "WEBSOCKET");
        assert_eq!(ConfirmationChannel::Email.to_string(), "EMAIL");
        assert_eq!(ConfirmationChannel::ApiCallback.to_string(), "API_CALLBACK");
        assert_eq!(ConfirmationChannel::Grpc.to_string(), "GRPC");
    }

    #[test]
    fn confirmation_channel_from_str_canonical_values() {
        assert_eq!(
            "WEBSOCKET".parse::<ConfirmationChannel>(),
            Ok(ConfirmationChannel::WebSocket)
        );
        assert_eq!(
            "EMAIL".parse::<ConfirmationChannel>(),
            Ok(ConfirmationChannel::Email)
        );
        assert_eq!(
            "API_CALLBACK".parse::<ConfirmationChannel>(),
            Ok(ConfirmationChannel::ApiCallback)
        );
        assert_eq!(
            "GRPC".parse::<ConfirmationChannel>(),
            Ok(ConfirmationChannel::Grpc)
        );
    }

    #[test]
    fn confirmation_channel_from_str_case_insensitive() {
        assert_eq!(
            "websocket".parse::<ConfirmationChannel>(),
            Ok(ConfirmationChannel::WebSocket)
        );
        assert_eq!(
            "Email".parse::<ConfirmationChannel>(),
            Ok(ConfirmationChannel::Email)
        );
        assert_eq!(
            "api_callback".parse::<ConfirmationChannel>(),
            Ok(ConfirmationChannel::ApiCallback)
        );
        assert_eq!(
            "gRpC".parse::<ConfirmationChannel>(),
            Ok(ConfirmationChannel::Grpc)
        );
    }

    #[test]
    fn confirmation_channel_from_str_invalid_value() {
        let result = "SMTP".parse::<ConfirmationChannel>();

        assert!(matches!(
            result,
            Err(ParseEnumError::InvalidValue("ConfirmationChannel", ref value)) if value == "SMTP"
        ));
    }

    #[test]
    fn trade_confirmation_new() {
        use crate::domain::value_objects::Blockchain;

        let trade_id = TradeId::new_v4();
        let rfq_id = RfqId::new_v4();
        let buyer_id = CounterpartyId::new("buyer-1");
        let seller_id = CounterpartyId::new("seller-1");

        let confirmation = TradeConfirmation::new(
            trade_id,
            rfq_id,
            Price::new(50000.0).unwrap(),
            Quantity::new(1.5).unwrap(),
            Decimal::new(10, 0),
            Decimal::new(5, 0),
            Decimal::new(15, 0),
            SettlementMethod::OnChain(Blockchain::Ethereum),
            TradeParticipant::Counterparty(buyer_id.clone()),
            TradeParticipant::Counterparty(seller_id.clone()),
        );

        assert_eq!(confirmation.trade_id(), trade_id);
        assert_eq!(confirmation.rfq_id(), rfq_id);
        assert_eq!(confirmation.price(), Price::new(50000.0).unwrap());
        assert_eq!(confirmation.quantity(), Quantity::new(1.5).unwrap());
        assert_eq!(confirmation.taker_fee(), Decimal::new(10, 0));
        assert_eq!(confirmation.maker_fee(), Decimal::new(5, 0));
        assert_eq!(confirmation.net_fee(), Decimal::new(15, 0));
        assert_eq!(confirmation.buyer().as_counterparty(), Some(&buyer_id));
        assert_eq!(confirmation.seller().as_counterparty(), Some(&seller_id));
    }

    #[test]
    fn channel_delivery_status_success() {
        let status = ChannelDeliveryStatus::success(ConfirmationChannel::Email, 2);
        assert!(status.success);
        assert!(status.error.is_none());
        assert_eq!(status.retry_attempts, 2);
    }

    #[test]
    fn channel_delivery_status_failed() {
        let status = ChannelDeliveryStatus::failed(
            ConfirmationChannel::WebSocket,
            "Connection timeout".to_string(),
            3,
        );
        assert!(!status.success);
        assert_eq!(status.error, Some("Connection timeout".to_string()));
        assert_eq!(status.retry_attempts, 3);
    }

    #[test]
    fn confirmation_status_all_sent() {
        let results = vec![
            ChannelDeliveryStatus::success(ConfirmationChannel::Email, 0),
            ChannelDeliveryStatus::success(ConfirmationChannel::WebSocket, 1),
        ];
        let status = ConfirmationStatus::from_results(results);
        assert!(status.is_all_sent());
        assert!(!status.is_all_failed());
        assert!(!status.is_partial_success());
    }

    #[test]
    fn confirmation_status_all_failed() {
        let results = vec![
            ChannelDeliveryStatus::failed(ConfirmationChannel::Email, "SMTP error".to_string(), 3),
            ChannelDeliveryStatus::failed(
                ConfirmationChannel::WebSocket,
                "No session".to_string(),
                0,
            ),
        ];
        let status = ConfirmationStatus::from_results(results);
        assert!(!status.is_all_sent());
        assert!(status.is_all_failed());
        assert!(!status.is_partial_success());
    }

    #[test]
    fn confirmation_status_partial_success() {
        let results = vec![
            ChannelDeliveryStatus::success(ConfirmationChannel::Email, 0),
            ChannelDeliveryStatus::failed(
                ConfirmationChannel::WebSocket,
                "No session".to_string(),
                0,
            ),
        ];
        let status = ConfirmationStatus::from_results(results);
        assert!(!status.is_all_sent());
        assert!(!status.is_all_failed());
        assert!(status.is_partial_success());
    }

    #[test]
    fn confirmation_status_display() {
        let all_sent = ConfirmationStatus::AllSent;
        assert!(all_sent.to_string().contains("successfully"));

        let partial = ConfirmationStatus::PartialSuccess {
            failed_channels: vec![ChannelDeliveryStatus::failed(
                ConfirmationChannel::Email,
                "error".to_string(),
                1,
            )],
        };
        assert!(partial.to_string().contains("Partial"));

        let all_failed = ConfirmationStatus::AllFailed {
            channel_statuses: vec![],
        };
        assert!(all_failed.to_string().contains("All confirmations failed"));
    }
}
