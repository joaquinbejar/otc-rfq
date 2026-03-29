//! # SBE API Conversions
//!
//! Conversions between domain types and SBE API types.
//!
//! This module provides bidirectional conversions following the same pattern
//! as gRPC conversions in `src/api/grpc/conversions.rs`.

use super::types::*;
use crate::domain::value_objects::{QuoteId, RfqId};

// ============================================================================
// Request Conversions
// ============================================================================

impl GetRfqRequest {
    /// Creates a GetRfqRequest from domain types.
    #[must_use]
    pub fn from_domain(request_id: uuid::Uuid, rfq_id: RfqId) -> Self {
        Self {
            request_id,
            rfq_id: rfq_id.get(),
        }
    }

    /// Converts to domain RfqId.
    #[must_use]
    pub fn to_domain_rfq_id(&self) -> RfqId {
        RfqId::new(self.rfq_id)
    }
}

impl ExecuteTradeRequest {
    /// Creates an ExecuteTradeRequest from domain types.
    #[must_use]
    pub fn from_domain(rfq_id: RfqId, quote_id: QuoteId) -> Self {
        Self {
            rfq_id: rfq_id.get(),
            quote_id: quote_id.get(),
        }
    }

    /// Converts to domain RfqId.
    #[must_use]
    pub fn to_domain_rfq_id(&self) -> RfqId {
        RfqId::new(self.rfq_id)
    }

    /// Converts to domain QuoteId.
    #[must_use]
    pub fn to_domain_quote_id(&self) -> QuoteId {
        QuoteId::new(self.quote_id)
    }
}

impl SubscribeQuotesRequest {
    /// Creates a SubscribeQuotesRequest from domain RfqId.
    #[must_use]
    pub fn from_domain(rfq_id: RfqId) -> Self {
        Self {
            rfq_id: rfq_id.get(),
        }
    }

    /// Converts to domain RfqId.
    #[must_use]
    pub fn to_domain_rfq_id(&self) -> RfqId {
        RfqId::new(self.rfq_id)
    }
}

impl SubscribeRfqStatusRequest {
    /// Creates a SubscribeRfqStatusRequest from domain RfqId.
    #[must_use]
    pub fn from_domain(rfq_id: RfqId) -> Self {
        Self {
            rfq_id: rfq_id.get(),
        }
    }

    /// Converts to domain RfqId.
    #[must_use]
    pub fn to_domain_rfq_id(&self) -> RfqId {
        RfqId::new(self.rfq_id)
    }
}

impl UnsubscribeRequest {
    /// Creates an UnsubscribeRequest from domain RfqId.
    #[must_use]
    pub fn from_domain(rfq_id: RfqId) -> Self {
        Self {
            rfq_id: rfq_id.get(),
        }
    }

    /// Converts to domain RfqId.
    #[must_use]
    pub fn to_domain_rfq_id(&self) -> RfqId {
        RfqId::new(self.rfq_id)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn get_rfq_request_domain_conversion() {
        let rfq_id = RfqId::new_v4();
        let request_id = uuid::Uuid::new_v4();

        let request = GetRfqRequest::from_domain(request_id, rfq_id);
        assert_eq!(request.to_domain_rfq_id(), rfq_id);
    }

    #[test]
    fn execute_trade_request_domain_conversion() {
        let rfq_id = RfqId::new_v4();
        let quote_id = QuoteId::new_v4();

        let request = ExecuteTradeRequest::from_domain(rfq_id, quote_id);
        assert_eq!(request.to_domain_rfq_id(), rfq_id);
        assert_eq!(request.to_domain_quote_id(), quote_id);
    }
}
