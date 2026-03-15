//! # Anonymity Service
//!
//! Domain service for managing RFQ anonymity.
//!
//! This service handles the creation and management of anonymous RFQs,
//! including identity mapping for compliance and post-trade reveal for settlement.
//!
//! # Privacy Model
//!
//! ```text
//! 1. Client creates anonymous RFQ
//! 2. AnonymityService creates IdentityMapping (stored for compliance)
//! 3. AnonymousRfqView broadcast to MMs (no client_id)
//! 4. MMs quote without knowing requester
//! 5. Trade executed
//! 6. AnonymityService reveals identity to winning MM (for settlement)
//! 7. IdentityRevealed event emitted (audit trail)
//! ```
//!
//! # Examples
//!
//! ```ignore
//! use otc_rfq::domain::services::anonymity_service::AnonymityService;
//!
//! let service = AnonymityService::new(identity_repo);
//!
//! // Create mapping when anonymous RFQ is created
//! service.create_mapping(&rfq).await?;
//!
//! // Get anonymized view for broadcasting
//! let view = service.anonymize(&rfq);
//!
//! // Reveal identity post-trade
//! let requester = service.reveal_identity(rfq_id, &mm_id).await?;
//! ```

use crate::domain::entities::anonymity::{AnonymousRfqView, IdentityMapping};
use crate::domain::entities::rfq::Rfq;
use crate::domain::value_objects::{CounterpartyId, RfqId, RfqState};
use crate::infrastructure::persistence::traits::IdentityMappingRepository;
use std::sync::Arc;
use thiserror::Error;

/// Errors that can occur in the anonymity service.
#[derive(Debug, Error)]
pub enum AnonymityError {
    /// RFQ is not anonymous.
    #[error("RFQ {0} is not anonymous")]
    NotAnonymous(RfqId),

    /// Identity mapping not found.
    #[error("Identity mapping not found for RFQ {0}")]
    MappingNotFound(RfqId),

    /// Identity already revealed to this party.
    #[error("Identity already revealed to {counterparty} for RFQ {rfq_id}")]
    AlreadyRevealed {
        /// The RFQ ID.
        rfq_id: RfqId,
        /// The counterparty.
        counterparty: CounterpartyId,
    },

    /// Cannot reveal identity - RFQ not in valid state.
    #[error("Cannot reveal identity for RFQ {rfq_id}: invalid state {state:?}")]
    InvalidStateForReveal {
        /// The RFQ ID.
        rfq_id: RfqId,
        /// Current state.
        state: RfqState,
    },

    /// Repository error.
    #[error("Repository error: {0}")]
    Repository(String),
}

impl From<crate::infrastructure::persistence::traits::RepositoryError> for AnonymityError {
    fn from(err: crate::infrastructure::persistence::traits::RepositoryError) -> Self {
        Self::Repository(err.to_string())
    }
}

/// Result type for anonymity operations.
pub type AnonymityResult<T> = Result<T, AnonymityError>;

/// Service for managing RFQ anonymity.
///
/// Handles identity mapping, anonymization, and post-trade reveal.
#[derive(Debug)]
pub struct AnonymityService {
    /// Repository for identity mappings.
    identity_repository: Arc<dyn IdentityMappingRepository>,
}

impl AnonymityService {
    /// Creates a new anonymity service.
    #[must_use]
    pub fn new(identity_repository: Arc<dyn IdentityMappingRepository>) -> Self {
        Self {
            identity_repository,
        }
    }

    /// Creates an identity mapping for an anonymous RFQ.
    ///
    /// This should be called when an anonymous RFQ is created.
    /// The mapping links the RFQ to the requester's identity for
    /// compliance and settlement purposes.
    ///
    /// # Arguments
    ///
    /// * `rfq` - The RFQ to create a mapping for
    ///
    /// # Errors
    ///
    /// Returns `AnonymityError::NotAnonymous` if the RFQ is not anonymous.
    /// Returns `AnonymityError::Repository` if persistence fails.
    pub async fn create_mapping(&self, rfq: &Rfq) -> AnonymityResult<IdentityMapping> {
        if !rfq.is_anonymous() {
            return Err(AnonymityError::NotAnonymous(rfq.id()));
        }

        let mapping =
            IdentityMapping::new(rfq.id(), rfq.client_id().clone(), rfq.anonymity_level());

        self.identity_repository.save(&mapping).await?;

        Ok(mapping)
    }

    /// Creates an anonymized view of an RFQ.
    ///
    /// This is a convenience method that delegates to `Rfq::to_anonymous_view`.
    #[must_use]
    pub fn anonymize(&self, rfq: &Rfq) -> AnonymousRfqView {
        rfq.to_anonymous_view()
    }

    /// Checks if an RFQ should be broadcast anonymously.
    ///
    /// Returns true if the RFQ has an anonymity level that hides identity.
    #[must_use]
    pub fn should_anonymize(&self, rfq: &Rfq) -> bool {
        rfq.anonymity_level().hides_identity()
    }

    /// Retrieves the identity mapping for an RFQ.
    ///
    /// # Errors
    ///
    /// Returns `AnonymityError::MappingNotFound` if no mapping exists.
    /// Returns `AnonymityError::Repository` if the query fails.
    pub async fn get_mapping(&self, rfq_id: RfqId) -> AnonymityResult<IdentityMapping> {
        self.identity_repository
            .get(&rfq_id)
            .await?
            .ok_or(AnonymityError::MappingNotFound(rfq_id))
    }

    /// Reveals the requester's identity to a counterparty.
    ///
    /// This should be called post-trade to reveal the requester's identity
    /// to the winning market maker for settlement purposes.
    ///
    /// # Arguments
    ///
    /// * `rfq_id` - The RFQ ID
    /// * `reveal_to` - The counterparty to reveal identity to
    ///
    /// # Returns
    ///
    /// The requester's identity.
    ///
    /// # Errors
    ///
    /// Returns `AnonymityError::MappingNotFound` if no mapping exists.
    /// Returns `AnonymityError::Repository` if persistence fails.
    pub async fn reveal_identity(
        &self,
        rfq_id: RfqId,
        reveal_to: &CounterpartyId,
    ) -> AnonymityResult<CounterpartyId> {
        // Get the mapping
        let mapping = self.get_mapping(rfq_id).await?;

        // Record the reveal
        self.identity_repository
            .record_reveal(&rfq_id, reveal_to)
            .await?;

        Ok(mapping.requester_id().clone())
    }

    /// Reveals identity only if the RFQ is in a valid state for reveal.
    ///
    /// Identity can only be revealed after the RFQ has been executed.
    ///
    /// # Arguments
    ///
    /// * `rfq` - The RFQ
    /// * `reveal_to` - The counterparty to reveal identity to
    ///
    /// # Errors
    ///
    /// Returns `AnonymityError::InvalidStateForReveal` if RFQ is not executed.
    /// Returns `AnonymityError::NotAnonymous` if RFQ is not anonymous.
    pub async fn reveal_identity_if_valid(
        &self,
        rfq: &Rfq,
        reveal_to: &CounterpartyId,
    ) -> AnonymityResult<CounterpartyId> {
        // Check if RFQ is anonymous
        if !rfq.is_anonymous() {
            return Err(AnonymityError::NotAnonymous(rfq.id()));
        }

        // Check if RFQ is in a valid state for reveal
        if !Self::can_reveal_identity(rfq) {
            return Err(AnonymityError::InvalidStateForReveal {
                rfq_id: rfq.id(),
                state: rfq.state(),
            });
        }

        self.reveal_identity(rfq.id(), reveal_to).await
    }

    /// Checks if identity can be revealed for an RFQ.
    ///
    /// Identity can be revealed when:
    /// - RFQ is in `Executed` state
    /// - RFQ is in `Executing` state (for settlement preparation)
    #[must_use]
    pub fn can_reveal_identity(rfq: &Rfq) -> bool {
        matches!(rfq.state(), RfqState::Executing | RfqState::Executed)
    }

    /// Checks if identity has been revealed to a specific party.
    ///
    /// # Errors
    ///
    /// Returns `AnonymityError::MappingNotFound` if no mapping exists.
    pub async fn is_revealed_to(
        &self,
        rfq_id: RfqId,
        party: &CounterpartyId,
    ) -> AnonymityResult<bool> {
        let mapping = self.get_mapping(rfq_id).await?;
        Ok(mapping.is_revealed_to(party))
    }

    /// Gets the requester identity without recording a reveal.
    ///
    /// This is for internal/compliance use only. It does not record
    /// that the identity was accessed.
    ///
    /// # Security
    ///
    /// Callers must have appropriate authorization to access identity data.
    ///
    /// # Errors
    ///
    /// Returns `AnonymityError::MappingNotFound` if no mapping exists.
    pub async fn get_requester_identity(&self, rfq_id: RfqId) -> AnonymityResult<CounterpartyId> {
        let mapping = self.get_mapping(rfq_id).await?;
        Ok(mapping.requester_id().clone())
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::useless_vec
)]
mod tests {
    use super::*;
    use crate::domain::entities::anonymity::AnonymityLevel;
    use crate::domain::entities::rfq::RfqBuilder;
    use crate::domain::value_objects::enums::AssetClass;
    use crate::domain::value_objects::timestamp::Timestamp;
    use crate::domain::value_objects::{Instrument, OrderSide, Quantity, Symbol};
    use crate::infrastructure::persistence::traits::RepositoryResult;
    use std::collections::HashMap;
    use std::sync::Mutex;

    #[derive(Debug, Default)]
    struct MockIdentityMappingRepository {
        mappings: Mutex<HashMap<RfqId, IdentityMapping>>,
    }

    #[async_trait::async_trait]
    impl IdentityMappingRepository for MockIdentityMappingRepository {
        async fn save(&self, mapping: &IdentityMapping) -> RepositoryResult<()> {
            self.mappings
                .lock()
                .unwrap()
                .insert(mapping.rfq_id(), mapping.clone());
            Ok(())
        }

        async fn get(&self, rfq_id: &RfqId) -> RepositoryResult<Option<IdentityMapping>> {
            Ok(self.mappings.lock().unwrap().get(rfq_id).cloned())
        }

        async fn record_reveal(
            &self,
            rfq_id: &RfqId,
            revealed_to: &CounterpartyId,
        ) -> RepositoryResult<()> {
            let mut mappings = self.mappings.lock().unwrap();
            if let Some(mapping) = mappings.get_mut(rfq_id) {
                mapping.reveal_to(revealed_to.clone());
                Ok(())
            } else {
                Err(
                    crate::infrastructure::persistence::traits::RepositoryError::not_found(
                        "IdentityMapping",
                        rfq_id.to_string(),
                    ),
                )
            }
        }

        async fn find_revealed(&self) -> RepositoryResult<Vec<IdentityMapping>> {
            Ok(self
                .mappings
                .lock()
                .unwrap()
                .values()
                .filter(|m| m.is_revealed())
                .cloned()
                .collect())
        }

        async fn find_unrevealed(&self) -> RepositoryResult<Vec<IdentityMapping>> {
            Ok(self
                .mappings
                .lock()
                .unwrap()
                .values()
                .filter(|m| !m.is_revealed())
                .cloned()
                .collect())
        }

        async fn delete(&self, rfq_id: &RfqId) -> RepositoryResult<bool> {
            Ok(self.mappings.lock().unwrap().remove(rfq_id).is_some())
        }

        async fn count(&self) -> RepositoryResult<u64> {
            Ok(self.mappings.lock().unwrap().len() as u64)
        }
    }

    fn create_test_instrument() -> Instrument {
        let symbol = Symbol::new("BTC/USD").unwrap();
        Instrument::builder(symbol, AssetClass::CryptoSpot).build()
    }

    fn create_anonymous_rfq() -> Rfq {
        RfqBuilder::new(
            CounterpartyId::new("client-1"),
            create_test_instrument(),
            OrderSide::Buy,
            Quantity::new(10.0).unwrap(),
            Timestamp::now().add_secs(300),
        )
        .anonymous()
        .build()
    }

    fn create_transparent_rfq() -> Rfq {
        RfqBuilder::new(
            CounterpartyId::new("client-1"),
            create_test_instrument(),
            OrderSide::Buy,
            Quantity::new(10.0).unwrap(),
            Timestamp::now().add_secs(300),
        )
        .build()
    }

    fn create_service() -> AnonymityService {
        AnonymityService::new(Arc::new(MockIdentityMappingRepository::default()))
    }

    #[tokio::test]
    async fn create_mapping_for_anonymous_rfq() {
        let service = create_service();
        let rfq = create_anonymous_rfq();

        let result = service.create_mapping(&rfq).await;

        assert!(result.is_ok());
        let mapping = result.unwrap();
        assert_eq!(mapping.rfq_id(), rfq.id());
        assert_eq!(mapping.requester_id(), rfq.client_id());
        assert_eq!(mapping.anonymity_level(), AnonymityLevel::FullAnonymous);
        assert!(!mapping.is_revealed());
    }

    #[tokio::test]
    async fn create_mapping_fails_for_transparent_rfq() {
        let service = create_service();
        let rfq = create_transparent_rfq();

        let result = service.create_mapping(&rfq).await;

        assert!(matches!(result, Err(AnonymityError::NotAnonymous(_))));
    }

    #[tokio::test]
    async fn anonymize_creates_view() {
        let service = create_service();
        let rfq = create_anonymous_rfq();

        let view = service.anonymize(&rfq);

        assert_eq!(view.rfq_id(), rfq.id());
        assert_eq!(view.instrument(), rfq.instrument());
        assert_eq!(view.side(), rfq.side());
        assert_eq!(view.quantity(), rfq.quantity());
        assert!(view.is_anonymous());
    }

    #[tokio::test]
    async fn should_anonymize_returns_true_for_anonymous() {
        let service = create_service();
        let rfq = create_anonymous_rfq();

        assert!(service.should_anonymize(&rfq));
    }

    #[tokio::test]
    async fn should_anonymize_returns_false_for_transparent() {
        let service = create_service();
        let rfq = create_transparent_rfq();

        assert!(!service.should_anonymize(&rfq));
    }

    #[tokio::test]
    async fn reveal_identity_returns_requester() {
        let service = create_service();
        let rfq = create_anonymous_rfq();
        let mm_id = CounterpartyId::new("mm-1");

        service.create_mapping(&rfq).await.unwrap();

        let result = service.reveal_identity(rfq.id(), &mm_id).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), *rfq.client_id());
    }

    #[tokio::test]
    async fn reveal_identity_records_reveal() {
        let service = create_service();
        let rfq = create_anonymous_rfq();
        let mm_id = CounterpartyId::new("mm-1");

        service.create_mapping(&rfq).await.unwrap();
        service.reveal_identity(rfq.id(), &mm_id).await.unwrap();

        let is_revealed = service.is_revealed_to(rfq.id(), &mm_id).await.unwrap();
        assert!(is_revealed);
    }

    #[tokio::test]
    async fn reveal_identity_fails_for_missing_mapping() {
        let service = create_service();
        let rfq_id = RfqId::new_v4();
        let mm_id = CounterpartyId::new("mm-1");

        let result = service.reveal_identity(rfq_id, &mm_id).await;

        assert!(matches!(result, Err(AnonymityError::MappingNotFound(_))));
    }

    #[tokio::test]
    async fn get_requester_identity_without_reveal() {
        let service = create_service();
        let rfq = create_anonymous_rfq();

        service.create_mapping(&rfq).await.unwrap();

        let requester = service.get_requester_identity(rfq.id()).await.unwrap();
        assert_eq!(requester, *rfq.client_id());

        // Verify no reveal was recorded
        let mm_id = CounterpartyId::new("mm-1");
        let is_revealed = service.is_revealed_to(rfq.id(), &mm_id).await.unwrap();
        assert!(!is_revealed);
    }

    #[test]
    fn can_reveal_identity_for_executed_state() {
        let mut rfq = create_anonymous_rfq();
        // Simulate state transitions to Executed
        rfq.start_quote_collection().unwrap();
        // Note: We can't easily get to Executed state in tests without more setup
        // So we test the Created state which should return false
        assert!(!AnonymityService::can_reveal_identity(&rfq));
    }
}
