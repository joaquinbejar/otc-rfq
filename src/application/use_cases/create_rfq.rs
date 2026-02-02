//! # Create RFQ Use Case
//!
//! Use case for creating new RFQ requests.
//!
//! This use case orchestrates the creation of a new RFQ, including:
//! - Request validation
//! - Client verification
//! - Instrument validation
//! - Compliance pre-checks
//! - RFQ persistence
//! - Domain event publishing

use crate::application::dto::rfq_dto::{CreateRfqRequest, CreateRfqResponse};
use crate::application::error::{ApplicationError, ApplicationResult};
use crate::domain::entities::rfq::{ComplianceResult, Rfq};
use crate::domain::events::rfq_events::RfqCreated;
use crate::domain::value_objects::{CounterpartyId, RfqId};
use async_trait::async_trait;
use std::fmt;
use std::sync::Arc;

/// Repository for RFQ persistence.
#[async_trait]
pub trait RfqRepository: Send + Sync + fmt::Debug {
    /// Saves an RFQ to the repository.
    ///
    /// # Errors
    ///
    /// Returns an error if persistence fails.
    async fn save(&self, rfq: &Rfq) -> Result<(), String>;

    /// Finds an RFQ by ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    async fn find_by_id(&self, id: RfqId) -> Result<Option<Rfq>, String>;
}

/// Publisher for domain events.
#[async_trait]
pub trait EventPublisher: Send + Sync + fmt::Debug {
    /// Publishes an RFQ created event.
    ///
    /// # Errors
    ///
    /// Returns an error if publishing fails.
    async fn publish_rfq_created(&self, event: RfqCreated) -> Result<(), String>;
}

/// Service for compliance checks.
#[async_trait]
pub trait ComplianceService: Send + Sync + fmt::Debug {
    /// Performs a pre-check for RFQ creation.
    ///
    /// # Errors
    ///
    /// Returns an error if the check fails.
    async fn pre_check(
        &self,
        client_id: &CounterpartyId,
        base_asset: &str,
        quote_asset: &str,
        quantity: f64,
    ) -> Result<ComplianceResult, String>;
}

/// Repository for client information.
#[async_trait]
pub trait ClientRepository: Send + Sync + fmt::Debug {
    /// Checks if a client exists.
    async fn exists(&self, client_id: &str) -> Result<bool, String>;

    /// Checks if a client is active.
    async fn is_active(&self, client_id: &str) -> Result<bool, String>;
}

/// Registry for supported instruments.
#[async_trait]
pub trait InstrumentRegistry: Send + Sync + fmt::Debug {
    /// Checks if an instrument is supported.
    async fn is_supported(&self, base_asset: &str, quote_asset: &str) -> Result<bool, String>;
}

/// Use case for creating a new RFQ.
#[derive(Debug)]
pub struct CreateRfqUseCase {
    rfq_repository: Arc<dyn RfqRepository>,
    event_publisher: Arc<dyn EventPublisher>,
    compliance_service: Arc<dyn ComplianceService>,
    client_repository: Arc<dyn ClientRepository>,
    instrument_registry: Arc<dyn InstrumentRegistry>,
}

impl CreateRfqUseCase {
    /// Creates a new CreateRfqUseCase with all dependencies.
    #[must_use]
    pub fn new(
        rfq_repository: Arc<dyn RfqRepository>,
        event_publisher: Arc<dyn EventPublisher>,
        compliance_service: Arc<dyn ComplianceService>,
        client_repository: Arc<dyn ClientRepository>,
        instrument_registry: Arc<dyn InstrumentRegistry>,
    ) -> Self {
        Self {
            rfq_repository,
            event_publisher,
            compliance_service,
            client_repository,
            instrument_registry,
        }
    }

    /// Executes the create RFQ use case.
    ///
    /// # Arguments
    ///
    /// * `request` - The create RFQ request
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Request validation fails
    /// - Client does not exist or is not active
    /// - Instrument is not supported
    /// - Compliance check fails
    /// - Persistence fails
    /// - Event publishing fails
    pub async fn execute(&self, request: CreateRfqRequest) -> ApplicationResult<CreateRfqResponse> {
        // 1. Validate request
        request.validate().map_err(ApplicationError::validation)?;

        // 2. Check client exists
        let client_exists = self
            .client_repository
            .exists(&request.client_id)
            .await
            .map_err(ApplicationError::repository)?;

        if !client_exists {
            return Err(ApplicationError::client_not_found(&request.client_id));
        }

        // 3. Check client is active
        let client_active = self
            .client_repository
            .is_active(&request.client_id)
            .await
            .map_err(ApplicationError::repository)?;

        if !client_active {
            return Err(ApplicationError::client_not_active(&request.client_id));
        }

        // 4. Check instrument is supported
        let instrument_supported = self
            .instrument_registry
            .is_supported(&request.base_asset, &request.quote_asset)
            .await
            .map_err(ApplicationError::repository)?;

        if !instrument_supported {
            return Err(ApplicationError::instrument_not_supported(format!(
                "{}/{}",
                request.base_asset, request.quote_asset
            )));
        }

        // 5. Convert to domain types
        let (instrument, quantity, expires_at) = request
            .to_domain_types()
            .map_err(ApplicationError::validation)?;

        let client_id = CounterpartyId::new(&request.client_id);

        // 6. Run compliance pre-check
        let compliance_result = self
            .compliance_service
            .pre_check(
                &client_id,
                &request.base_asset,
                &request.quote_asset,
                request.quantity,
            )
            .await
            .map_err(ApplicationError::repository)?;

        if !compliance_result.passed {
            return Err(ApplicationError::compliance_failed(
                compliance_result
                    .reason
                    .unwrap_or_else(|| "unknown reason".to_string()),
            ));
        }

        // 7. Create RFQ aggregate
        let rfq = Rfq::new(client_id, instrument, request.side, quantity, expires_at)?;

        // 8. Persist RFQ
        self.rfq_repository
            .save(&rfq)
            .await
            .map_err(ApplicationError::repository)?;

        // 9. Publish domain event
        let event = RfqCreated::new(
            rfq.id(),
            rfq.client_id().clone(),
            rfq.instrument().clone(),
            rfq.side(),
            rfq.quantity(),
            rfq.expires_at(),
        );

        self.event_publisher
            .publish_rfq_created(event)
            .await
            .map_err(ApplicationError::event_publish)?;

        // 10. Return response
        Ok(CreateRfqResponse::new(
            rfq.id(),
            rfq.state(),
            rfq.created_at(),
            rfq.expires_at(),
        ))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::domain::value_objects::OrderSide;
    use std::collections::HashMap;
    use std::sync::Mutex;

    #[derive(Debug, Default)]
    struct MockRfqRepository {
        rfqs: Mutex<HashMap<RfqId, Rfq>>,
    }

    #[async_trait]
    impl RfqRepository for MockRfqRepository {
        async fn save(&self, rfq: &Rfq) -> Result<(), String> {
            self.rfqs.lock().unwrap().insert(rfq.id(), rfq.clone());
            Ok(())
        }

        async fn find_by_id(&self, id: RfqId) -> Result<Option<Rfq>, String> {
            Ok(self.rfqs.lock().unwrap().get(&id).cloned())
        }
    }

    #[derive(Debug, Default)]
    struct MockEventPublisher {
        events: Mutex<Vec<RfqCreated>>,
    }

    #[async_trait]
    impl EventPublisher for MockEventPublisher {
        async fn publish_rfq_created(&self, event: RfqCreated) -> Result<(), String> {
            self.events.lock().unwrap().push(event);
            Ok(())
        }
    }

    #[derive(Debug)]
    struct MockComplianceService {
        should_pass: bool,
    }

    impl MockComplianceService {
        fn passing() -> Self {
            Self { should_pass: true }
        }

        fn failing() -> Self {
            Self { should_pass: false }
        }
    }

    #[async_trait]
    impl ComplianceService for MockComplianceService {
        async fn pre_check(
            &self,
            _client_id: &CounterpartyId,
            _base_asset: &str,
            _quote_asset: &str,
            _quantity: f64,
        ) -> Result<ComplianceResult, String> {
            if self.should_pass {
                Ok(ComplianceResult::passed())
            } else {
                Ok(ComplianceResult::failed("compliance check failed"))
            }
        }
    }

    #[derive(Debug)]
    struct MockClientRepository {
        existing_clients: Vec<String>,
        active_clients: Vec<String>,
    }

    impl MockClientRepository {
        fn with_client(client_id: &str) -> Self {
            Self {
                existing_clients: vec![client_id.to_string()],
                active_clients: vec![client_id.to_string()],
            }
        }

        fn empty() -> Self {
            Self {
                existing_clients: vec![],
                active_clients: vec![],
            }
        }
    }

    #[async_trait]
    impl ClientRepository for MockClientRepository {
        async fn exists(&self, client_id: &str) -> Result<bool, String> {
            Ok(self.existing_clients.contains(&client_id.to_string()))
        }

        async fn is_active(&self, client_id: &str) -> Result<bool, String> {
            Ok(self.active_clients.contains(&client_id.to_string()))
        }
    }

    #[derive(Debug)]
    struct MockInstrumentRegistry {
        supported: Vec<(String, String)>,
    }

    impl MockInstrumentRegistry {
        fn with_instrument(base: &str, quote: &str) -> Self {
            Self {
                supported: vec![(base.to_string(), quote.to_string())],
            }
        }
    }

    #[async_trait]
    impl InstrumentRegistry for MockInstrumentRegistry {
        async fn is_supported(&self, base_asset: &str, quote_asset: &str) -> Result<bool, String> {
            Ok(self
                .supported
                .iter()
                .any(|(b, q)| b == base_asset && q == quote_asset))
        }
    }

    fn create_use_case(
        client_repo: impl ClientRepository + 'static,
        instrument_registry: impl InstrumentRegistry + 'static,
        compliance_service: impl ComplianceService + 'static,
    ) -> CreateRfqUseCase {
        CreateRfqUseCase::new(
            Arc::new(MockRfqRepository::default()),
            Arc::new(MockEventPublisher::default()),
            Arc::new(compliance_service),
            Arc::new(client_repo),
            Arc::new(instrument_registry),
        )
    }

    #[tokio::test]
    async fn execute_success() {
        let use_case = create_use_case(
            MockClientRepository::with_client("client-1"),
            MockInstrumentRegistry::with_instrument("BTC", "USD"),
            MockComplianceService::passing(),
        );

        let request = CreateRfqRequest::new("client-1", "BTC", "USD", OrderSide::Buy, 1.5, 300);
        let result = use_case.execute(request).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(
            response.state,
            crate::domain::value_objects::RfqState::Created
        );
    }

    #[tokio::test]
    async fn execute_client_not_found() {
        let use_case = create_use_case(
            MockClientRepository::empty(),
            MockInstrumentRegistry::with_instrument("BTC", "USD"),
            MockComplianceService::passing(),
        );

        let request =
            CreateRfqRequest::new("unknown-client", "BTC", "USD", OrderSide::Buy, 1.5, 300);
        let result = use_case.execute(request).await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ApplicationError::ClientNotFound(_)
        ));
    }

    #[tokio::test]
    async fn execute_instrument_not_supported() {
        let use_case = create_use_case(
            MockClientRepository::with_client("client-1"),
            MockInstrumentRegistry::with_instrument("ETH", "USD"),
            MockComplianceService::passing(),
        );

        let request = CreateRfqRequest::new("client-1", "BTC", "USD", OrderSide::Buy, 1.5, 300);
        let result = use_case.execute(request).await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ApplicationError::InstrumentNotSupported(_)
        ));
    }

    #[tokio::test]
    async fn execute_compliance_failed() {
        let use_case = create_use_case(
            MockClientRepository::with_client("client-1"),
            MockInstrumentRegistry::with_instrument("BTC", "USD"),
            MockComplianceService::failing(),
        );

        let request = CreateRfqRequest::new("client-1", "BTC", "USD", OrderSide::Buy, 1.5, 300);
        let result = use_case.execute(request).await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ApplicationError::ComplianceFailed(_)
        ));
    }

    #[tokio::test]
    async fn execute_validation_error() {
        let use_case = create_use_case(
            MockClientRepository::with_client("client-1"),
            MockInstrumentRegistry::with_instrument("BTC", "USD"),
            MockComplianceService::passing(),
        );

        let request = CreateRfqRequest::new("", "BTC", "USD", OrderSide::Buy, 1.5, 300);
        let result = use_case.execute(request).await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ApplicationError::ValidationError(_)
        ));
    }
}
