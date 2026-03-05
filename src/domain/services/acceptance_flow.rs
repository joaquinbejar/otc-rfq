//! # Acceptance Flow Service
//!
//! Orchestrates the atomic quote acceptance workflow.
//!
//! This module provides the [`AcceptanceFlow`] struct that coordinates
//! the 5-step acceptance process: Lock → Risk Check → Last-Look → Execute → Release.

use crate::domain::entities::quote::Quote;
use crate::domain::entities::rfq::Rfq;
use crate::domain::entities::trade::Trade;
use crate::domain::errors::{DomainError, DomainResult};
use crate::domain::services::last_look::{LastLookConfig, LastLookResult, LastLookService};
use crate::domain::services::quote_lock::{
    LockHolderId, QuoteLock, QuoteLockConfig, QuoteLockService,
};
use crate::domain::services::risk_check::{RiskCheckConfig, RiskCheckService, RiskResult};
use crate::domain::value_objects::{QuoteId, RfqId};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Result of the acceptance flow.
#[derive(Debug, Clone)]
pub struct AcceptanceResult {
    /// The RFQ that was accepted.
    pub rfq_id: RfqId,
    /// The quote that was accepted.
    pub quote_id: QuoteId,
    /// The risk check result.
    pub risk_result: RiskResult,
    /// The last-look result (if applicable).
    pub last_look_result: Option<LastLookResult>,
    /// Total time taken for the acceptance flow.
    pub duration: Duration,
}

impl AcceptanceResult {
    /// Creates a new acceptance result.
    #[must_use]
    pub fn new(
        rfq_id: RfqId,
        quote_id: QuoteId,
        risk_result: RiskResult,
        last_look_result: Option<LastLookResult>,
        duration: Duration,
    ) -> Self {
        Self {
            rfq_id,
            quote_id,
            risk_result,
            last_look_result,
            duration,
        }
    }
}

/// Reason for acceptance failure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AcceptanceFailureReason {
    /// Quote is already locked.
    QuoteLocked,
    /// Failed to acquire lock.
    LockAcquisitionFailed(String),
    /// Risk check failed.
    RiskCheckFailed(String),
    /// Last-look was rejected.
    LastLookRejected(String),
    /// Last-look timed out.
    LastLookTimeout,
    /// Total flow timed out.
    AcceptanceTimeout,
    /// Execution failed.
    ExecutionFailed(String),
}

impl AcceptanceFailureReason {
    /// Converts to a DomainError.
    #[must_use]
    pub fn to_error(&self) -> DomainError {
        match self {
            Self::QuoteLocked => DomainError::QuoteLocked("Quote is already locked".to_string()),
            Self::LockAcquisitionFailed(msg) => DomainError::LockAcquisitionFailed(msg.clone()),
            Self::RiskCheckFailed(msg) => DomainError::RiskCheckFailed(msg.clone()),
            Self::LastLookRejected(msg) => DomainError::LastLookRejected(msg.clone()),
            Self::LastLookTimeout => DomainError::LastLookTimeout("Timed out".to_string()),
            Self::AcceptanceTimeout => DomainError::AcceptanceTimeout("Timed out".to_string()),
            Self::ExecutionFailed(msg) => DomainError::InvalidState(msg.clone()),
        }
    }
}

impl fmt::Display for AcceptanceFailureReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::QuoteLocked => write!(f, "Quote is already locked"),
            Self::LockAcquisitionFailed(msg) => write!(f, "Lock failed: {}", msg),
            Self::RiskCheckFailed(msg) => write!(f, "Risk check failed: {}", msg),
            Self::LastLookRejected(msg) => write!(f, "Last-look rejected: {}", msg),
            Self::LastLookTimeout => write!(f, "Last-look timed out"),
            Self::AcceptanceTimeout => write!(f, "Acceptance timed out"),
            Self::ExecutionFailed(msg) => write!(f, "Execution failed: {}", msg),
        }
    }
}

/// Configuration for the acceptance flow.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AcceptanceFlowConfig {
    /// Total timeout for the acceptance flow.
    pub total_timeout: Duration,
    /// Configuration for quote locking.
    pub lock_config: QuoteLockConfig,
    /// Configuration for risk checks.
    pub risk_config: RiskCheckConfig,
    /// Configuration for last-look.
    pub last_look_config: LastLookConfig,
}

impl Default for AcceptanceFlowConfig {
    fn default() -> Self {
        Self {
            total_timeout: Duration::from_millis(500),
            lock_config: QuoteLockConfig::default(),
            risk_config: RiskCheckConfig::default(),
            last_look_config: LastLookConfig::default(),
        }
    }
}

impl AcceptanceFlowConfig {
    /// Creates a new configuration.
    #[must_use]
    pub fn new(total_timeout: Duration) -> Self {
        Self {
            total_timeout,
            ..Default::default()
        }
    }

    /// Returns remaining time until deadline.
    #[must_use]
    pub fn remaining_time(&self, start: Instant) -> Duration {
        self.total_timeout.saturating_sub(start.elapsed())
    }

    /// Checks if deadline exceeded.
    #[must_use]
    pub fn is_timed_out(&self, start: Instant) -> bool {
        start.elapsed() >= self.total_timeout
    }
}

/// Trait for executing trades after acceptance.
#[async_trait]
pub trait TradeExecutor: Send + Sync + fmt::Debug {
    /// Executes a trade for the given RFQ and quote.
    async fn execute(
        &self,
        rfq: &Rfq,
        quote: &Quote,
        risk_result: &RiskResult,
    ) -> DomainResult<Trade>;
}

/// Orchestrates the atomic quote acceptance workflow.
#[derive(Debug)]
pub struct AcceptanceFlow {
    lock_service: Arc<dyn QuoteLockService>,
    risk_service: Arc<dyn RiskCheckService>,
    last_look_service: Arc<dyn LastLookService>,
    executor: Arc<dyn TradeExecutor>,
    config: AcceptanceFlowConfig,
    holder_id: LockHolderId,
}

impl AcceptanceFlow {
    /// Creates a new acceptance flow.
    #[must_use]
    pub fn new(
        lock_service: Arc<dyn QuoteLockService>,
        risk_service: Arc<dyn RiskCheckService>,
        last_look_service: Arc<dyn LastLookService>,
        executor: Arc<dyn TradeExecutor>,
        config: AcceptanceFlowConfig,
    ) -> Self {
        Self {
            lock_service,
            risk_service,
            last_look_service,
            executor,
            config,
            holder_id: LockHolderId::new(),
        }
    }

    /// Returns the configuration.
    #[must_use]
    pub fn config(&self) -> &AcceptanceFlowConfig {
        &self.config
    }

    /// Returns the holder ID.
    #[must_use]
    pub fn holder_id(&self) -> LockHolderId {
        self.holder_id
    }

    /// Executes the acceptance flow.
    ///
    /// # Errors
    ///
    /// Returns an error if any step fails:
    /// - `DomainError::QuoteLocked` if the quote is already locked
    /// - `DomainError::LockAcquisitionFailed` if lock acquisition fails
    /// - `DomainError::RiskCheckFailed` if risk validation fails
    /// - `DomainError::LastLookRejected` if MM rejects the quote
    /// - `DomainError::LastLookTimeout` if last-look times out
    /// - `DomainError::AcceptanceTimeout` if total timeout is exceeded
    pub async fn accept(
        &self,
        rfq: &Rfq,
        quote: &Quote,
    ) -> DomainResult<(Trade, AcceptanceResult)> {
        let start = Instant::now();
        let quote_id = quote.id();

        // Step 1: Acquire lock
        let lock = self.acquire_lock(quote_id, start).await?;

        // Execute remaining steps
        let result = self.execute_with_lock(rfq, quote, &lock, start).await;

        // Always release lock
        let _ = self.lock_service.release(&lock).await;

        result
    }

    async fn acquire_lock(&self, quote_id: QuoteId, start: Instant) -> DomainResult<QuoteLock> {
        if self.config.is_timed_out(start) {
            return Err(DomainError::AcceptanceTimeout(
                "Timeout before lock".to_string(),
            ));
        }
        let ttl = self.config.lock_config.effective_ttl(None);
        self.lock_service.lock(quote_id, self.holder_id, ttl).await
    }

    async fn execute_with_lock(
        &self,
        rfq: &Rfq,
        quote: &Quote,
        _lock: &QuoteLock,
        start: Instant,
    ) -> DomainResult<(Trade, AcceptanceResult)> {
        // Step 2: Risk check
        if self.config.is_timed_out(start) {
            return Err(DomainError::AcceptanceTimeout(
                "Timeout before risk check".to_string(),
            ));
        }
        let risk_result = self.risk_service.check(rfq, quote).await;
        if !risk_result.is_passed() {
            return Err(risk_result.to_error());
        }

        // Step 3: Last-look
        let last_look_result = self.perform_last_look(quote, start).await?;

        // Step 4: Execute
        if self.config.is_timed_out(start) {
            return Err(DomainError::AcceptanceTimeout(
                "Timeout before execution".to_string(),
            ));
        }
        let trade = self.executor.execute(rfq, quote, &risk_result).await?;

        Ok((
            trade,
            AcceptanceResult::new(
                rfq.id(),
                quote.id(),
                risk_result,
                last_look_result,
                start.elapsed(),
            ),
        ))
    }

    async fn perform_last_look(
        &self,
        quote: &Quote,
        start: Instant,
    ) -> DomainResult<Option<LastLookResult>> {
        if self.config.last_look_config.skip_if_not_required
            && !self.last_look_service.requires_last_look(quote.venue_id())
        {
            return Ok(None);
        }

        if self.config.is_timed_out(start) {
            return Err(DomainError::AcceptanceTimeout(
                "Timeout before last-look".to_string(),
            ));
        }

        let remaining = self.config.remaining_time(start);
        let timeout = self
            .config
            .last_look_config
            .effective_timeout(Some(remaining));
        let result = self.last_look_service.request(quote, timeout).await;
        self.last_look_service
            .record_result(quote.venue_id(), &result)
            .await;

        match &result {
            LastLookResult::Confirmed { .. } => Ok(Some(result)),
            LastLookResult::Rejected { reason, .. } => {
                Err(DomainError::LastLookRejected(reason.clone()))
            }
            LastLookResult::Timeout { timeout, .. } => Err(DomainError::LastLookTimeout(format!(
                "Timed out after {}ms",
                timeout.as_millis()
            ))),
        }
    }
}
