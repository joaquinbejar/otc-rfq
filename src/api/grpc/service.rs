//! # gRPC RFQ Service
//!
//! gRPC service implementation using tonic.
//!
//! This module provides the [`RfqServiceImpl`] which implements the gRPC
//! `RfqService` trait generated from the protobuf definitions.
//!
//! # Features
//!
//! - Request validation and error mapping
//! - DTO conversion between proto and domain types
//! - Streaming support for `GetQuotes`
//! - Tracing integration for request logging
//! - Proper error responses with gRPC status codes
//!
//! # Examples
//!
//! ```ignore
//! use otc_rfq::api::grpc::service::RfqServiceImpl;
//! use tonic::transport::Server;
//!
//! let service = RfqServiceImpl::new(/* dependencies */);
//! Server::builder()
//!     .add_service(RfqServiceServer::new(service))
//!     .serve(addr)
//!     .await?;
//! ```

use crate::api::grpc::conversions::ConversionError;
use crate::api::grpc::proto::{
    self, CancelRfqRequest, CancelRfqResponse, CreateRfqRequest, CreateRfqResponse,
    ExecuteTradeRequest, ExecuteTradeResponse, GetQuotesRequest, GetQuotesResponse, GetRfqRequest,
    GetRfqResponse, StreamRfqStatusRequest, StreamRfqStatusResponse,
    rfq_service_server::RfqService,
};
use crate::application::error::ApplicationError;
use crate::application::use_cases::create_rfq::RfqRepository;
use crate::domain::value_objects::timestamp::Timestamp;
use crate::domain::value_objects::{CounterpartyId, Instrument, OrderSide, Quantity, RfqId};
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_stream::{Stream, wrappers::ReceiverStream};
use tonic::{Request, Response, Status};
use tracing::{error, info, instrument, warn};

/// gRPC RFQ Service implementation.
///
/// Implements the `RfqService` trait generated from protobuf definitions,
/// providing all RPC methods for RFQ lifecycle management.
#[derive(Debug)]
pub struct RfqServiceImpl {
    rfq_repository: Arc<dyn RfqRepository>,
}

impl RfqServiceImpl {
    /// Creates a new RFQ service with the given repository.
    #[must_use]
    pub fn new(rfq_repository: Arc<dyn RfqRepository>) -> Self {
        Self { rfq_repository }
    }

    /// Validates a CreateRfqRequest and returns domain types.
    fn validate_create_request(
        &self,
        request: &CreateRfqRequest,
    ) -> Result<(CounterpartyId, Instrument, OrderSide, Quantity, Timestamp), Status> {
        // Validate client_id
        if request.client_id.is_empty() {
            return Err(Status::invalid_argument("client_id cannot be empty"));
        }

        // Validate and convert instrument
        let instrument = request
            .instrument
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("instrument is required"))?;

        if instrument.base_asset.is_empty() {
            return Err(Status::invalid_argument("base_asset cannot be empty"));
        }
        if instrument.quote_asset.is_empty() {
            return Err(Status::invalid_argument("quote_asset cannot be empty"));
        }

        let symbol_str = format!("{}/{}", instrument.base_asset, instrument.quote_asset);
        let symbol = crate::domain::value_objects::Symbol::new(&symbol_str)
            .map_err(|e| Status::invalid_argument(format!("invalid symbol: {e}")))?;
        let domain_instrument = Instrument::builder(
            symbol,
            crate::domain::value_objects::enums::AssetClass::CryptoSpot,
        )
        .build();

        // Validate and convert side
        let side: OrderSide =
            crate::api::grpc::conversions::proto_order_side_to_domain(request.side)
                .map_err(|_| Status::invalid_argument("invalid order side"))?;

        // Validate and convert quantity
        let quantity_decimal = request
            .quantity
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("quantity is required"))?;

        let quantity_value: rust_decimal::Decimal = quantity_decimal
            .clone()
            .try_into()
            .map_err(|e: ConversionError| Status::invalid_argument(e.to_string()))?;

        let quantity = Quantity::try_from(quantity_value)
            .map_err(|e| Status::invalid_argument(format!("invalid quantity: {e}")))?;

        // Validate timeout
        if request.timeout_seconds <= 0 {
            return Err(Status::invalid_argument("timeout_seconds must be positive"));
        }

        let expires_at = Timestamp::now().add_secs(request.timeout_seconds);

        Ok((
            CounterpartyId::new(&request.client_id),
            domain_instrument,
            side,
            quantity,
            expires_at,
        ))
    }
}

#[tonic::async_trait]
impl RfqService for RfqServiceImpl {
    /// Creates a new RFQ.
    #[instrument(skip(self, request), fields(client_id))]
    async fn create_rfq(
        &self,
        request: Request<CreateRfqRequest>,
    ) -> Result<Response<CreateRfqResponse>, Status> {
        let req = request.into_inner();
        tracing::Span::current().record("client_id", &req.client_id);

        info!("Creating RFQ for client: {}", req.client_id);

        // Validate request
        let (client_id, instrument, side, quantity, expires_at) =
            self.validate_create_request(&req)?;

        // Build RFQ
        let rfq = crate::domain::entities::rfq::RfqBuilder::new(
            client_id, instrument, side, quantity, expires_at,
        )
        .build();

        // Save to repository
        self.rfq_repository.save(&rfq).await.map_err(|e| {
            error!("Failed to save RFQ: {}", e);
            Status::internal(format!("failed to save RFQ: {e}"))
        })?;

        info!("Created RFQ: {}", rfq.id());

        // Convert to proto response
        let response = CreateRfqResponse {
            rfq: Some(proto::Rfq::from(&rfq)),
        };

        Ok(Response::new(response))
    }

    /// Gets an RFQ by ID.
    #[instrument(skip(self, request), fields(rfq_id))]
    async fn get_rfq(
        &self,
        request: Request<GetRfqRequest>,
    ) -> Result<Response<GetRfqResponse>, Status> {
        let req = request.into_inner();

        let rfq_id: RfqId = req
            .rfq_id
            .ok_or_else(|| Status::invalid_argument("rfq_id is required"))?
            .try_into()
            .map_err(|e: ConversionError| Status::invalid_argument(e.to_string()))?;

        tracing::Span::current().record("rfq_id", rfq_id.to_string());

        info!("Getting RFQ: {}", rfq_id);

        let rfq = self
            .rfq_repository
            .find_by_id(rfq_id)
            .await
            .map_err(|e| {
                error!("Failed to find RFQ: {}", e);
                Status::internal(format!("failed to find RFQ: {e}"))
            })?
            .ok_or_else(|| {
                warn!("RFQ not found: {}", rfq_id);
                Status::not_found(format!("RFQ not found: {rfq_id}"))
            })?;

        let response = GetRfqResponse {
            rfq: Some(proto::Rfq::from(&rfq)),
        };

        Ok(Response::new(response))
    }

    type GetQuotesStream = Pin<Box<dyn Stream<Item = Result<GetQuotesResponse, Status>> + Send>>;

    /// Streams quotes for an RFQ.
    #[instrument(skip(self, request), fields(rfq_id))]
    async fn get_quotes(
        &self,
        request: Request<GetQuotesRequest>,
    ) -> Result<Response<Self::GetQuotesStream>, Status> {
        let req = request.into_inner();

        let rfq_id: RfqId = req
            .rfq_id
            .ok_or_else(|| Status::invalid_argument("rfq_id is required"))?
            .try_into()
            .map_err(|e: ConversionError| Status::invalid_argument(e.to_string()))?;

        tracing::Span::current().record("rfq_id", rfq_id.to_string());

        info!("Streaming quotes for RFQ: {}", rfq_id);

        // Get the RFQ to access its quotes
        let rfq = self
            .rfq_repository
            .find_by_id(rfq_id)
            .await
            .map_err(|e| {
                error!("Failed to find RFQ: {}", e);
                Status::internal(format!("failed to find RFQ: {e}"))
            })?
            .ok_or_else(|| {
                warn!("RFQ not found: {}", rfq_id);
                Status::not_found(format!("RFQ not found: {rfq_id}"))
            })?;

        // Create a channel for streaming
        let (tx, rx) = mpsc::channel(32);

        // Spawn a task to send quotes
        let quotes = rfq.quotes().to_vec();
        tokio::spawn(async move {
            for (i, quote) in quotes.iter().enumerate() {
                let is_final = i == quotes.len() - 1;
                let response = GetQuotesResponse {
                    quote: Some(proto::Quote::from(quote)),
                    is_final,
                };

                if tx.send(Ok(response)).await.is_err() {
                    // Receiver dropped
                    break;
                }
            }

            // If no quotes, send a final empty response
            if quotes.is_empty() {
                let _ = tx
                    .send(Ok(GetQuotesResponse {
                        quote: None,
                        is_final: true,
                    }))
                    .await;
            }
        });

        let stream = ReceiverStream::new(rx);
        Ok(Response::new(Box::pin(stream)))
    }

    /// Executes a trade for a selected quote.
    #[instrument(skip(self, request), fields(rfq_id, quote_id))]
    async fn execute_trade(
        &self,
        request: Request<ExecuteTradeRequest>,
    ) -> Result<Response<ExecuteTradeResponse>, Status> {
        let req = request.into_inner();

        let rfq_id: RfqId = req
            .rfq_id
            .ok_or_else(|| Status::invalid_argument("rfq_id is required"))?
            .try_into()
            .map_err(|e: ConversionError| Status::invalid_argument(e.to_string()))?;

        let quote_id: crate::domain::value_objects::QuoteId = req
            .quote_id
            .ok_or_else(|| Status::invalid_argument("quote_id is required"))?
            .try_into()
            .map_err(|e: ConversionError| Status::invalid_argument(e.to_string()))?;

        tracing::Span::current().record("rfq_id", rfq_id.to_string());
        tracing::Span::current().record("quote_id", quote_id.to_string());

        info!("Executing trade for RFQ: {}, Quote: {}", rfq_id, quote_id);

        // Get the RFQ
        let rfq = self
            .rfq_repository
            .find_by_id(rfq_id)
            .await
            .map_err(|e| {
                error!("Failed to find RFQ: {}", e);
                Status::internal(format!("failed to find RFQ: {e}"))
            })?
            .ok_or_else(|| {
                warn!("RFQ not found: {}", rfq_id);
                Status::not_found(format!("RFQ not found: {rfq_id}"))
            })?;

        // Find the quote
        let quote = rfq
            .quotes()
            .iter()
            .find(|q| q.id() == quote_id)
            .ok_or_else(|| {
                warn!("Quote not found: {}", quote_id);
                Status::not_found(format!("Quote not found: {quote_id}"))
            })?;

        // Check if quote is expired
        if quote.is_expired() {
            return Err(Status::failed_precondition("quote has expired"));
        }

        // Create a trade from the quote
        let trade = crate::domain::entities::trade::Trade::new(
            rfq_id,
            quote_id,
            quote.venue_id().clone(),
            quote.price(),
            quote.quantity(),
        );

        info!("Created trade: {}", trade.id());

        let response = ExecuteTradeResponse {
            trade: Some(proto::Trade::from(&trade)),
        };

        Ok(Response::new(response))
    }

    /// Cancels an RFQ.
    #[instrument(skip(self, request), fields(rfq_id))]
    async fn cancel_rfq(
        &self,
        request: Request<CancelRfqRequest>,
    ) -> Result<Response<CancelRfqResponse>, Status> {
        let req = request.into_inner();

        let rfq_id: RfqId = req
            .rfq_id
            .ok_or_else(|| Status::invalid_argument("rfq_id is required"))?
            .try_into()
            .map_err(|e: ConversionError| Status::invalid_argument(e.to_string()))?;

        tracing::Span::current().record("rfq_id", rfq_id.to_string());

        info!("Cancelling RFQ: {}, reason: {}", rfq_id, req.reason);

        // Get the RFQ
        let mut rfq = self
            .rfq_repository
            .find_by_id(rfq_id)
            .await
            .map_err(|e| {
                error!("Failed to find RFQ: {}", e);
                Status::internal(format!("failed to find RFQ: {e}"))
            })?
            .ok_or_else(|| {
                warn!("RFQ not found: {}", rfq_id);
                Status::not_found(format!("RFQ not found: {rfq_id}"))
            })?;

        // Cancel the RFQ
        rfq.cancel().map_err(|e| {
            warn!("Cannot cancel RFQ: {}", e);
            Status::failed_precondition(format!("cannot cancel RFQ: {e}"))
        })?;

        // Save the updated RFQ
        self.rfq_repository.save(&rfq).await.map_err(|e| {
            error!("Failed to save cancelled RFQ: {}", e);
            Status::internal(format!("failed to save RFQ: {e}"))
        })?;

        info!("Cancelled RFQ: {}", rfq_id);

        let response = CancelRfqResponse {
            rfq: Some(proto::Rfq::from(&rfq)),
        };

        Ok(Response::new(response))
    }

    type StreamRfqStatusStream =
        Pin<Box<dyn Stream<Item = Result<StreamRfqStatusResponse, Status>> + Send>>;

    /// Streams RFQ status updates.
    #[instrument(skip(self, request), fields(rfq_id))]
    async fn stream_rfq_status(
        &self,
        request: Request<StreamRfqStatusRequest>,
    ) -> Result<Response<Self::StreamRfqStatusStream>, Status> {
        let req = request.into_inner();

        let rfq_id: RfqId = req
            .rfq_id
            .ok_or_else(|| Status::invalid_argument("rfq_id is required"))?
            .try_into()
            .map_err(|e: ConversionError| Status::invalid_argument(e.to_string()))?;

        tracing::Span::current().record("rfq_id", rfq_id.to_string());

        info!("Streaming status for RFQ: {}", rfq_id);

        // Get the current RFQ state
        let rfq = self
            .rfq_repository
            .find_by_id(rfq_id)
            .await
            .map_err(|e| {
                error!("Failed to find RFQ: {}", e);
                Status::internal(format!("failed to find RFQ: {e}"))
            })?
            .ok_or_else(|| {
                warn!("RFQ not found: {}", rfq_id);
                Status::not_found(format!("RFQ not found: {rfq_id}"))
            })?;

        // Create a channel for streaming
        let (tx, rx) = mpsc::channel(32);

        // Send initial state
        let initial_response = StreamRfqStatusResponse {
            rfq_id: Some(proto::Uuid::from(rfq_id)),
            previous_state: proto::RfqState::Unspecified as i32,
            current_state: i32::from(rfq.state()),
            message: "Initial state".to_string(),
            timestamp: Some(proto::Timestamp::from(Timestamp::now())),
        };

        let _ = tx.send(Ok(initial_response)).await;

        // Note: In a real implementation, this would subscribe to state change events
        // For now, we just send the initial state and close the stream

        let stream = ReceiverStream::new(rx);
        Ok(Response::new(Box::pin(stream)))
    }
}

/// Converts an ApplicationError to a gRPC Status.
impl From<ApplicationError> for Status {
    fn from(err: ApplicationError) -> Self {
        match &err {
            ApplicationError::Validation(_) => Status::invalid_argument(err.to_string()),
            ApplicationError::NotFound { .. }
            | ApplicationError::ClientNotFound(_)
            | ApplicationError::RfqNotFound(_)
            | ApplicationError::QuoteNotFound(_) => Status::not_found(err.to_string()),
            ApplicationError::Unauthorized => Status::unauthenticated(err.to_string()),
            ApplicationError::QuoteExpired(_) | ApplicationError::InvalidState(_) => {
                Status::failed_precondition(err.to_string())
            }
            ApplicationError::ComplianceFailed(_) => Status::permission_denied(err.to_string()),
            _ => Status::internal(err.to_string()),
        }
    }
}

/// Converts a ConversionError to a gRPC Status.
impl From<ConversionError> for Status {
    fn from(err: ConversionError) -> Self {
        Status::invalid_argument(err.to_string())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::collections::HashMap;
    use std::sync::RwLock;

    use crate::domain::entities::rfq::Rfq;

    /// Mock RFQ repository for testing.
    #[derive(Debug, Default)]
    struct MockRfqRepository {
        rfqs: RwLock<HashMap<RfqId, Rfq>>,
    }

    #[async_trait]
    impl RfqRepository for MockRfqRepository {
        async fn save(&self, rfq: &Rfq) -> Result<(), String> {
            self.rfqs.write().unwrap().insert(rfq.id(), rfq.clone());
            Ok(())
        }

        async fn find_by_id(&self, id: RfqId) -> Result<Option<Rfq>, String> {
            Ok(self.rfqs.read().unwrap().get(&id).cloned())
        }
    }

    fn create_service() -> RfqServiceImpl {
        RfqServiceImpl::new(Arc::new(MockRfqRepository::default()))
    }

    fn create_valid_request() -> CreateRfqRequest {
        CreateRfqRequest {
            client_id: "client-123".to_string(),
            instrument: Some(proto::Instrument {
                symbol: "BTC/USD".to_string(),
                asset_class: proto::AssetClass::CryptoSpot as i32,
                base_asset: "BTC".to_string(),
                quote_asset: "USD".to_string(),
            }),
            side: proto::OrderSide::Buy as i32,
            quantity: Some(proto::Decimal {
                value: "1.5".to_string(),
            }),
            timeout_seconds: 300,
        }
    }

    #[tokio::test]
    async fn create_rfq_success() {
        let service = create_service();
        let request = Request::new(create_valid_request());

        let response = service.create_rfq(request).await;
        assert!(response.is_ok());

        let rfq = response.unwrap().into_inner().rfq.unwrap();
        assert_eq!(rfq.client_id, "client-123");
        assert_eq!(rfq.state, proto::RfqState::Created as i32);
    }

    #[tokio::test]
    async fn create_rfq_empty_client_id() {
        let service = create_service();
        let mut req = create_valid_request();
        req.client_id = String::new();

        let response = service.create_rfq(Request::new(req)).await;
        assert!(response.is_err());
        assert_eq!(response.unwrap_err().code(), tonic::Code::InvalidArgument);
    }

    #[tokio::test]
    async fn create_rfq_missing_instrument() {
        let service = create_service();
        let mut req = create_valid_request();
        req.instrument = None;

        let response = service.create_rfq(Request::new(req)).await;
        assert!(response.is_err());
        assert_eq!(response.unwrap_err().code(), tonic::Code::InvalidArgument);
    }

    #[tokio::test]
    async fn create_rfq_invalid_side() {
        let service = create_service();
        let mut req = create_valid_request();
        req.side = 0; // Unspecified

        let response = service.create_rfq(Request::new(req)).await;
        assert!(response.is_err());
        assert_eq!(response.unwrap_err().code(), tonic::Code::InvalidArgument);
    }

    #[tokio::test]
    async fn create_rfq_missing_quantity() {
        let service = create_service();
        let mut req = create_valid_request();
        req.quantity = None;

        let response = service.create_rfq(Request::new(req)).await;
        assert!(response.is_err());
        assert_eq!(response.unwrap_err().code(), tonic::Code::InvalidArgument);
    }

    #[tokio::test]
    async fn create_rfq_invalid_timeout() {
        let service = create_service();
        let mut req = create_valid_request();
        req.timeout_seconds = 0;

        let response = service.create_rfq(Request::new(req)).await;
        assert!(response.is_err());
        assert_eq!(response.unwrap_err().code(), tonic::Code::InvalidArgument);
    }

    #[tokio::test]
    async fn get_rfq_not_found() {
        let service = create_service();
        let request = Request::new(GetRfqRequest {
            rfq_id: Some(proto::Uuid {
                value: RfqId::new_v4().to_string(),
            }),
        });

        let response = service.get_rfq(request).await;
        assert!(response.is_err());
        assert_eq!(response.unwrap_err().code(), tonic::Code::NotFound);
    }

    #[tokio::test]
    async fn get_rfq_success() {
        let service = create_service();

        // First create an RFQ
        let create_response = service
            .create_rfq(Request::new(create_valid_request()))
            .await
            .unwrap();
        let rfq_id = create_response.into_inner().rfq.unwrap().id.unwrap();

        // Then get it
        let get_response = service
            .get_rfq(Request::new(GetRfqRequest {
                rfq_id: Some(rfq_id.clone()),
            }))
            .await;

        assert!(get_response.is_ok());
        let rfq = get_response.unwrap().into_inner().rfq.unwrap();
        assert_eq!(rfq.id.unwrap().value, rfq_id.value);
    }

    #[tokio::test]
    async fn get_rfq_missing_id() {
        let service = create_service();
        let request = Request::new(GetRfqRequest { rfq_id: None });

        let response = service.get_rfq(request).await;
        assert!(response.is_err());
        assert_eq!(response.unwrap_err().code(), tonic::Code::InvalidArgument);
    }

    #[tokio::test]
    async fn cancel_rfq_not_found() {
        let service = create_service();
        let request = Request::new(CancelRfqRequest {
            rfq_id: Some(proto::Uuid {
                value: RfqId::new_v4().to_string(),
            }),
            reason: "test".to_string(),
        });

        let response = service.cancel_rfq(request).await;
        assert!(response.is_err());
        assert_eq!(response.unwrap_err().code(), tonic::Code::NotFound);
    }

    #[tokio::test]
    async fn execute_trade_not_found() {
        let service = create_service();
        let request = Request::new(ExecuteTradeRequest {
            rfq_id: Some(proto::Uuid {
                value: RfqId::new_v4().to_string(),
            }),
            quote_id: Some(proto::Uuid {
                value: crate::domain::value_objects::QuoteId::new_v4().to_string(),
            }),
        });

        let response = service.execute_trade(request).await;
        assert!(response.is_err());
        assert_eq!(response.unwrap_err().code(), tonic::Code::NotFound);
    }

    #[test]
    fn application_error_to_status_validation() {
        let err = ApplicationError::validation("invalid input");
        let status: Status = err.into();
        assert_eq!(status.code(), tonic::Code::InvalidArgument);
    }

    #[test]
    fn application_error_to_status_not_found() {
        let err = ApplicationError::not_found("RFQ", "123");
        let status: Status = err.into();
        assert_eq!(status.code(), tonic::Code::NotFound);
    }

    #[test]
    fn application_error_to_status_unauthorized() {
        let err = ApplicationError::unauthorized();
        let status: Status = err.into();
        assert_eq!(status.code(), tonic::Code::Unauthenticated);
    }

    #[test]
    fn conversion_error_to_status() {
        let err = ConversionError::MissingField("test");
        let status: Status = err.into();
        assert_eq!(status.code(), tonic::Code::InvalidArgument);
    }
}
