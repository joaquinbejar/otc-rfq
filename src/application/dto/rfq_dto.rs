//! # RFQ DTOs
//!
//! Data transfer objects for RFQ operations.
//!
//! These DTOs decouple the API layer from the domain layer, providing
//! validation and serialization for RFQ-related requests and responses.

use crate::domain::value_objects::enums::{AssetClass, SettlementMethod};
use crate::domain::value_objects::symbol::Symbol;
use crate::domain::value_objects::timestamp::Timestamp;
use crate::domain::value_objects::{Instrument, OrderSide, Quantity, RfqId, RfqState};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Request to create a new RFQ.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRfqRequest {
    /// The client ID requesting the quote.
    pub client_id: String,
    /// Base asset symbol (e.g., "BTC").
    pub base_asset: String,
    /// Quote asset symbol (e.g., "USD").
    pub quote_asset: String,
    /// Buy or sell side.
    pub side: OrderSide,
    /// Requested quantity.
    pub quantity: f64,
    /// Expiry duration in seconds from now.
    pub expiry_seconds: u64,
}

impl CreateRfqRequest {
    /// Creates a new CreateRfqRequest.
    #[must_use]
    pub fn new(
        client_id: impl Into<String>,
        base_asset: impl Into<String>,
        quote_asset: impl Into<String>,
        side: OrderSide,
        quantity: f64,
        expiry_seconds: u64,
    ) -> Self {
        Self {
            client_id: client_id.into(),
            base_asset: base_asset.into(),
            quote_asset: quote_asset.into(),
            side,
            quantity,
            expiry_seconds,
        }
    }

    /// Validates the request fields.
    ///
    /// # Errors
    ///
    /// Returns an error message if validation fails.
    pub fn validate(&self) -> Result<(), String> {
        if self.client_id.is_empty() {
            return Err("client_id cannot be empty".to_string());
        }

        if self.base_asset.is_empty() {
            return Err("base_asset cannot be empty".to_string());
        }

        if self.quote_asset.is_empty() {
            return Err("quote_asset cannot be empty".to_string());
        }

        if self.quantity <= 0.0 {
            return Err("quantity must be positive".to_string());
        }

        if self.expiry_seconds == 0 {
            return Err("expiry_seconds must be greater than 0".to_string());
        }

        Ok(())
    }

    /// Converts the request to domain types.
    ///
    /// # Errors
    ///
    /// Returns an error if conversion fails.
    pub fn to_domain_types(&self) -> Result<(Instrument, Quantity, Timestamp), String> {
        let symbol_str = format!("{}/{}", self.base_asset, self.quote_asset);
        let symbol = Symbol::new(&symbol_str).map_err(|e| e.to_string())?;

        let instrument =
            Instrument::new(symbol, AssetClass::CryptoSpot, SettlementMethod::default());

        let quantity = Quantity::new(self.quantity).map_err(|e| e.to_string())?;

        let expires_at = Timestamp::now().add_secs(self.expiry_seconds as i64);

        Ok((instrument, quantity, expires_at))
    }
}

impl fmt::Display for CreateRfqRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "CreateRfqRequest {{ client: {}, {}/{} {} {} }}",
            self.client_id, self.base_asset, self.quote_asset, self.side, self.quantity
        )
    }
}

/// Response after creating an RFQ.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRfqResponse {
    /// The created RFQ ID.
    pub rfq_id: RfqId,
    /// Current state of the RFQ.
    pub state: RfqState,
    /// When the RFQ was created.
    pub created_at: Timestamp,
    /// When the RFQ expires.
    pub expires_at: Timestamp,
}

impl CreateRfqResponse {
    /// Creates a new CreateRfqResponse.
    #[must_use]
    pub fn new(
        rfq_id: RfqId,
        state: RfqState,
        created_at: Timestamp,
        expires_at: Timestamp,
    ) -> Self {
        Self {
            rfq_id,
            state,
            created_at,
            expires_at,
        }
    }
}

impl fmt::Display for CreateRfqResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "CreateRfqResponse {{ rfq_id: {}, state: {} }}",
            self.rfq_id, self.state
        )
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn create_rfq_request_new() {
        let request = CreateRfqRequest::new("client-1", "BTC", "USD", OrderSide::Buy, 1.5, 300);

        assert_eq!(request.client_id, "client-1");
        assert_eq!(request.base_asset, "BTC");
        assert_eq!(request.quote_asset, "USD");
        assert_eq!(request.side, OrderSide::Buy);
        assert!((request.quantity - 1.5).abs() < f64::EPSILON);
        assert_eq!(request.expiry_seconds, 300);
    }

    #[test]
    fn create_rfq_request_validate_success() {
        let request = CreateRfqRequest::new("client-1", "BTC", "USD", OrderSide::Buy, 1.5, 300);
        assert!(request.validate().is_ok());
    }

    #[test]
    fn create_rfq_request_validate_empty_client_id() {
        let request = CreateRfqRequest::new("", "BTC", "USD", OrderSide::Buy, 1.5, 300);
        assert!(request.validate().is_err());
    }

    #[test]
    fn create_rfq_request_validate_empty_base_asset() {
        let request = CreateRfqRequest::new("client-1", "", "USD", OrderSide::Buy, 1.5, 300);
        assert!(request.validate().is_err());
    }

    #[test]
    fn create_rfq_request_validate_zero_quantity() {
        let request = CreateRfqRequest::new("client-1", "BTC", "USD", OrderSide::Buy, 0.0, 300);
        assert!(request.validate().is_err());
    }

    #[test]
    fn create_rfq_request_validate_negative_quantity() {
        let request = CreateRfqRequest::new("client-1", "BTC", "USD", OrderSide::Buy, -1.0, 300);
        assert!(request.validate().is_err());
    }

    #[test]
    fn create_rfq_request_validate_zero_expiry() {
        let request = CreateRfqRequest::new("client-1", "BTC", "USD", OrderSide::Buy, 1.5, 0);
        assert!(request.validate().is_err());
    }

    #[test]
    fn create_rfq_request_to_domain_types() {
        let request = CreateRfqRequest::new("client-1", "BTC", "USD", OrderSide::Buy, 1.5, 300);
        let result = request.to_domain_types();
        assert!(result.is_ok());

        let (instrument, _quantity, expires_at) = result.unwrap();
        assert_eq!(instrument.symbol().base_asset(), "BTC");
        assert_eq!(instrument.symbol().quote_asset(), "USD");
        assert!(!expires_at.is_expired());
    }

    #[test]
    fn create_rfq_request_display() {
        let request = CreateRfqRequest::new("client-1", "BTC", "USD", OrderSide::Buy, 1.5, 300);
        let display = request.to_string();
        assert!(display.contains("client-1"));
        assert!(display.contains("BTC/USD"));
    }

    #[test]
    fn create_rfq_response_new() {
        let rfq_id = RfqId::new_v4();
        let now = Timestamp::now();
        let expires = now.add_secs(300);
        let response = CreateRfqResponse::new(rfq_id, RfqState::Created, now, expires);

        assert_eq!(response.rfq_id, rfq_id);
        assert_eq!(response.state, RfqState::Created);
    }

    #[test]
    fn create_rfq_response_display() {
        let rfq_id = RfqId::new_v4();
        let now = Timestamp::now();
        let expires = now.add_secs(300);
        let response = CreateRfqResponse::new(rfq_id, RfqState::Created, now, expires);
        let display = response.to_string();
        assert!(display.contains("rfq_id"));
    }
}
