//! # Compliance Service
//!
//! KYC/AML and limit checks.
//!
//! This module provides the [`ComplianceServiceImpl`] which orchestrates
//! compliance checks including KYC verification, AML screening, sanctions
//! checking, and trading limit validation.

use crate::application::error::ApplicationResult;
use crate::application::use_cases::create_rfq::ComplianceService;
use crate::domain::entities::rfq::ComplianceResult;
use crate::domain::value_objects::CounterpartyId;
use async_trait::async_trait;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::Arc;

/// KYC (Know Your Customer) status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum KycStatus {
    /// KYC verification passed.
    Verified,
    /// KYC verification pending.
    Pending,
    /// KYC verification expired.
    Expired,
    /// KYC verification failed.
    Failed,
    /// KYC not submitted.
    NotSubmitted,
}

impl KycStatus {
    /// Returns true if the KYC status allows trading.
    #[must_use]
    pub fn allows_trading(&self) -> bool {
        matches!(self, Self::Verified)
    }
}

impl fmt::Display for KycStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Verified => "VERIFIED",
            Self::Pending => "PENDING",
            Self::Expired => "EXPIRED",
            Self::Failed => "FAILED",
            Self::NotSubmitted => "NOT_SUBMITTED",
        };
        write!(f, "{}", s)
    }
}

/// AML (Anti-Money Laundering) screening result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AmlResult {
    /// Whether the screening passed.
    pub passed: bool,
    /// Risk score (0-100, higher = more risk).
    pub risk_score: u8,
    /// Alert details if any.
    pub alert: Option<String>,
}

impl AmlResult {
    /// Creates a passing AML result.
    #[must_use]
    pub fn passed(risk_score: u8) -> Self {
        Self {
            passed: true,
            risk_score,
            alert: None,
        }
    }

    /// Creates a failing AML result with an alert.
    #[must_use]
    pub fn failed(risk_score: u8, alert: impl Into<String>) -> Self {
        Self {
            passed: false,
            risk_score,
            alert: Some(alert.into()),
        }
    }
}

/// Sanctions screening result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SanctionsResult {
    /// Whether the screening passed (no hits).
    pub passed: bool,
    /// List names that matched, if any.
    pub matched_lists: Vec<String>,
}

impl SanctionsResult {
    /// Creates a passing sanctions result.
    #[must_use]
    pub fn passed() -> Self {
        Self {
            passed: true,
            matched_lists: Vec::new(),
        }
    }

    /// Creates a failing sanctions result with matched lists.
    #[must_use]
    pub fn failed(matched_lists: Vec<String>) -> Self {
        Self {
            passed: false,
            matched_lists,
        }
    }
}

/// Trading limits check result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LimitsResult {
    /// Whether all limits are within bounds.
    pub passed: bool,
    /// Daily volume used.
    pub daily_volume_used: Decimal,
    /// Daily volume limit.
    pub daily_volume_limit: Decimal,
    /// Per-trade notional limit.
    pub per_trade_limit: Decimal,
    /// Exceeded limits, if any.
    pub exceeded_limits: Vec<String>,
}

impl LimitsResult {
    /// Creates a passing limits result.
    #[must_use]
    pub fn passed(
        daily_volume_used: Decimal,
        daily_volume_limit: Decimal,
        per_trade_limit: Decimal,
    ) -> Self {
        Self {
            passed: true,
            daily_volume_used,
            daily_volume_limit,
            per_trade_limit,
            exceeded_limits: Vec::new(),
        }
    }

    /// Creates a failing limits result.
    #[must_use]
    pub fn failed(
        daily_volume_used: Decimal,
        daily_volume_limit: Decimal,
        per_trade_limit: Decimal,
        exceeded_limits: Vec<String>,
    ) -> Self {
        Self {
            passed: false,
            daily_volume_used,
            daily_volume_limit,
            per_trade_limit,
            exceeded_limits,
        }
    }
}

/// Severity level for compliance flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ComplianceSeverity {
    /// Informational flag.
    Info,
    /// Warning flag (non-blocking).
    Warning,
    /// Blocking flag (prevents trading).
    Blocking,
}

impl fmt::Display for ComplianceSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Info => "INFO",
            Self::Warning => "WARNING",
            Self::Blocking => "BLOCKING",
        };
        write!(f, "{}", s)
    }
}

/// Type of compliance flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ComplianceFlagType {
    /// KYC not verified.
    KycNotVerified,
    /// KYC expired.
    KycExpired,
    /// KYC pending.
    KycPending,
    /// AML alert.
    AmlAlert,
    /// High AML risk score.
    AmlHighRisk,
    /// Sanctions list hit.
    SanctionsHit,
    /// Daily volume limit exceeded.
    DailyLimitExceeded,
    /// Per-trade limit exceeded.
    PerTradeLimitExceeded,
    /// Instrument not allowed.
    InstrumentNotAllowed,
}

impl fmt::Display for ComplianceFlagType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::KycNotVerified => "KYC_NOT_VERIFIED",
            Self::KycExpired => "KYC_EXPIRED",
            Self::KycPending => "KYC_PENDING",
            Self::AmlAlert => "AML_ALERT",
            Self::AmlHighRisk => "AML_HIGH_RISK",
            Self::SanctionsHit => "SANCTIONS_HIT",
            Self::DailyLimitExceeded => "DAILY_LIMIT_EXCEEDED",
            Self::PerTradeLimitExceeded => "PER_TRADE_LIMIT_EXCEEDED",
            Self::InstrumentNotAllowed => "INSTRUMENT_NOT_ALLOWED",
        };
        write!(f, "{}", s)
    }
}

/// A compliance flag indicating an issue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComplianceFlag {
    /// Type of the flag.
    pub flag_type: ComplianceFlagType,
    /// Severity level.
    pub severity: ComplianceSeverity,
    /// Human-readable message.
    pub message: String,
}

impl ComplianceFlag {
    /// Creates a new compliance flag.
    #[must_use]
    pub fn new(
        flag_type: ComplianceFlagType,
        severity: ComplianceSeverity,
        message: impl Into<String>,
    ) -> Self {
        Self {
            flag_type,
            severity,
            message: message.into(),
        }
    }

    /// Creates a blocking flag.
    #[must_use]
    pub fn blocking(flag_type: ComplianceFlagType, message: impl Into<String>) -> Self {
        Self::new(flag_type, ComplianceSeverity::Blocking, message)
    }

    /// Creates a warning flag.
    #[must_use]
    pub fn warning(flag_type: ComplianceFlagType, message: impl Into<String>) -> Self {
        Self::new(flag_type, ComplianceSeverity::Warning, message)
    }

    /// Creates an info flag.
    #[must_use]
    pub fn info(flag_type: ComplianceFlagType, message: impl Into<String>) -> Self {
        Self::new(flag_type, ComplianceSeverity::Info, message)
    }

    /// Returns true if this is a blocking flag.
    #[must_use]
    pub fn is_blocking(&self) -> bool {
        self.severity == ComplianceSeverity::Blocking
    }
}

impl fmt::Display for ComplianceFlag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}] {}: {}",
            self.severity, self.flag_type, self.message
        )
    }
}

/// Comprehensive compliance check results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceCheckResult {
    /// Whether all checks passed.
    pub passed: bool,
    /// KYC status.
    pub kyc_status: KycStatus,
    /// AML screening result.
    pub aml_result: AmlResult,
    /// Sanctions screening result.
    pub sanctions_result: SanctionsResult,
    /// Trading limits result.
    pub limits_result: LimitsResult,
    /// All compliance flags.
    pub flags: Vec<ComplianceFlag>,
}

impl ComplianceCheckResult {
    /// Returns blocking flags only.
    #[must_use]
    pub fn blocking_flags(&self) -> Vec<&ComplianceFlag> {
        self.flags.iter().filter(|f| f.is_blocking()).collect()
    }

    /// Returns true if there are any blocking flags.
    #[must_use]
    pub fn has_blocking_flags(&self) -> bool {
        self.flags.iter().any(|f| f.is_blocking())
    }

    /// Converts to domain ComplianceResult.
    #[must_use]
    pub fn to_domain_result(&self) -> ComplianceResult {
        if self.passed {
            ComplianceResult::passed()
        } else {
            let reason = self
                .blocking_flags()
                .first()
                .map(|f| f.message.clone())
                .unwrap_or_else(|| "compliance check failed".to_string());
            ComplianceResult::failed(reason)
        }
    }
}

/// Provider for KYC status checks.
#[async_trait]
pub trait KycProvider: Send + Sync + fmt::Debug {
    /// Checks the KYC status for a client.
    async fn check_kyc_status(&self, client_id: &CounterpartyId) -> ApplicationResult<KycStatus>;
}

/// Provider for AML screening.
#[async_trait]
pub trait AmlProvider: Send + Sync + fmt::Debug {
    /// Screens a transaction for AML concerns.
    async fn screen_transaction(
        &self,
        client_id: &CounterpartyId,
        amount: Decimal,
    ) -> ApplicationResult<AmlResult>;
}

/// Provider for sanctions list checking.
#[async_trait]
pub trait SanctionsProvider: Send + Sync + fmt::Debug {
    /// Checks if a client is on any sanctions lists.
    async fn check_sanctions(
        &self,
        client_id: &CounterpartyId,
    ) -> ApplicationResult<SanctionsResult>;
}

/// Provider for trading limits.
#[async_trait]
pub trait LimitsProvider: Send + Sync + fmt::Debug {
    /// Checks trading limits for a client.
    async fn check_limits(
        &self,
        client_id: &CounterpartyId,
        trade_amount: Decimal,
    ) -> ApplicationResult<LimitsResult>;
}

/// Configuration for compliance service.
#[derive(Debug, Clone)]
pub struct ComplianceConfig {
    /// AML risk score threshold for warnings.
    pub aml_warning_threshold: u8,
    /// AML risk score threshold for blocking.
    pub aml_blocking_threshold: u8,
}

impl Default for ComplianceConfig {
    fn default() -> Self {
        Self {
            aml_warning_threshold: 50,
            aml_blocking_threshold: 80,
        }
    }
}

/// Implementation of the compliance service.
#[derive(Debug)]
pub struct ComplianceServiceImpl {
    kyc_provider: Arc<dyn KycProvider>,
    aml_provider: Arc<dyn AmlProvider>,
    sanctions_provider: Arc<dyn SanctionsProvider>,
    limits_provider: Arc<dyn LimitsProvider>,
    config: ComplianceConfig,
}

impl ComplianceServiceImpl {
    /// Creates a new compliance service.
    #[must_use]
    pub fn new(
        kyc_provider: Arc<dyn KycProvider>,
        aml_provider: Arc<dyn AmlProvider>,
        sanctions_provider: Arc<dyn SanctionsProvider>,
        limits_provider: Arc<dyn LimitsProvider>,
    ) -> Self {
        Self {
            kyc_provider,
            aml_provider,
            sanctions_provider,
            limits_provider,
            config: ComplianceConfig::default(),
        }
    }

    /// Creates a new compliance service with custom configuration.
    #[must_use]
    pub fn with_config(
        kyc_provider: Arc<dyn KycProvider>,
        aml_provider: Arc<dyn AmlProvider>,
        sanctions_provider: Arc<dyn SanctionsProvider>,
        limits_provider: Arc<dyn LimitsProvider>,
        config: ComplianceConfig,
    ) -> Self {
        Self {
            kyc_provider,
            aml_provider,
            sanctions_provider,
            limits_provider,
            config,
        }
    }

    /// Performs a comprehensive compliance check.
    ///
    /// # Errors
    ///
    /// Returns an error if any provider check fails.
    pub async fn check(
        &self,
        client_id: &CounterpartyId,
        trade_amount: Decimal,
    ) -> ApplicationResult<ComplianceCheckResult> {
        let mut flags = Vec::new();

        // Check KYC status
        let kyc_status = self.kyc_provider.check_kyc_status(client_id).await?;
        self.evaluate_kyc_status(kyc_status, &mut flags);

        // Run AML screening
        let aml_result = self
            .aml_provider
            .screen_transaction(client_id, trade_amount)
            .await?;
        self.evaluate_aml_result(&aml_result, &mut flags);

        // Check sanctions
        let sanctions_result = self.sanctions_provider.check_sanctions(client_id).await?;
        self.evaluate_sanctions_result(&sanctions_result, &mut flags);

        // Check trading limits
        let limits_result = self
            .limits_provider
            .check_limits(client_id, trade_amount)
            .await?;
        self.evaluate_limits_result(&limits_result, &mut flags);

        // Determine overall pass/fail
        let passed = !flags.iter().any(|f| f.is_blocking());

        Ok(ComplianceCheckResult {
            passed,
            kyc_status,
            aml_result,
            sanctions_result,
            limits_result,
            flags,
        })
    }

    fn evaluate_kyc_status(&self, status: KycStatus, flags: &mut Vec<ComplianceFlag>) {
        match status {
            KycStatus::Verified => {}
            KycStatus::Pending => {
                flags.push(ComplianceFlag::blocking(
                    ComplianceFlagType::KycPending,
                    "KYC verification is pending",
                ));
            }
            KycStatus::Expired => {
                flags.push(ComplianceFlag::blocking(
                    ComplianceFlagType::KycExpired,
                    "KYC verification has expired",
                ));
            }
            KycStatus::Failed | KycStatus::NotSubmitted => {
                flags.push(ComplianceFlag::blocking(
                    ComplianceFlagType::KycNotVerified,
                    "KYC verification not completed",
                ));
            }
        }
    }

    fn evaluate_aml_result(&self, result: &AmlResult, flags: &mut Vec<ComplianceFlag>) {
        if !result.passed {
            flags.push(ComplianceFlag::blocking(
                ComplianceFlagType::AmlAlert,
                result
                    .alert
                    .clone()
                    .unwrap_or_else(|| "AML screening failed".to_string()),
            ));
        } else if result.risk_score >= self.config.aml_blocking_threshold {
            flags.push(ComplianceFlag::blocking(
                ComplianceFlagType::AmlHighRisk,
                format!("AML risk score {} exceeds threshold", result.risk_score),
            ));
        } else if result.risk_score >= self.config.aml_warning_threshold {
            flags.push(ComplianceFlag::warning(
                ComplianceFlagType::AmlHighRisk,
                format!("Elevated AML risk score: {}", result.risk_score),
            ));
        }
    }

    fn evaluate_sanctions_result(&self, result: &SanctionsResult, flags: &mut Vec<ComplianceFlag>) {
        if !result.passed {
            let lists = result.matched_lists.join(", ");
            flags.push(ComplianceFlag::blocking(
                ComplianceFlagType::SanctionsHit,
                format!("Sanctions list match: {}", lists),
            ));
        }
    }

    fn evaluate_limits_result(&self, result: &LimitsResult, flags: &mut Vec<ComplianceFlag>) {
        for limit in &result.exceeded_limits {
            let flag_type = if limit.contains("daily") {
                ComplianceFlagType::DailyLimitExceeded
            } else {
                ComplianceFlagType::PerTradeLimitExceeded
            };
            flags.push(ComplianceFlag::blocking(flag_type, limit.clone()));
        }
    }
}

#[async_trait]
impl ComplianceService for ComplianceServiceImpl {
    async fn pre_check(
        &self,
        client_id: &CounterpartyId,
        _base_asset: &str,
        _quote_asset: &str,
        quantity: f64,
    ) -> Result<ComplianceResult, String> {
        let amount = Decimal::try_from(quantity).map_err(|e| e.to_string())?;
        let result = self
            .check(client_id, amount)
            .await
            .map_err(|e| e.to_string())?;
        Ok(result.to_domain_result())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use rust_decimal::prelude::FromPrimitive;

    #[derive(Debug)]
    struct MockKycProvider {
        status: KycStatus,
    }

    impl MockKycProvider {
        fn verified() -> Self {
            Self {
                status: KycStatus::Verified,
            }
        }

        fn with_status(status: KycStatus) -> Self {
            Self { status }
        }
    }

    #[async_trait]
    impl KycProvider for MockKycProvider {
        async fn check_kyc_status(
            &self,
            _client_id: &CounterpartyId,
        ) -> ApplicationResult<KycStatus> {
            Ok(self.status)
        }
    }

    #[derive(Debug)]
    struct MockAmlProvider {
        result: AmlResult,
    }

    impl MockAmlProvider {
        fn passing(risk_score: u8) -> Self {
            Self {
                result: AmlResult::passed(risk_score),
            }
        }

        fn failing(risk_score: u8, alert: &str) -> Self {
            Self {
                result: AmlResult::failed(risk_score, alert),
            }
        }
    }

    #[async_trait]
    impl AmlProvider for MockAmlProvider {
        async fn screen_transaction(
            &self,
            _client_id: &CounterpartyId,
            _amount: Decimal,
        ) -> ApplicationResult<AmlResult> {
            Ok(self.result.clone())
        }
    }

    #[derive(Debug)]
    struct MockSanctionsProvider {
        result: SanctionsResult,
    }

    impl MockSanctionsProvider {
        fn passing() -> Self {
            Self {
                result: SanctionsResult::passed(),
            }
        }

        fn failing(lists: Vec<&str>) -> Self {
            Self {
                result: SanctionsResult::failed(lists.into_iter().map(String::from).collect()),
            }
        }
    }

    #[async_trait]
    impl SanctionsProvider for MockSanctionsProvider {
        async fn check_sanctions(
            &self,
            _client_id: &CounterpartyId,
        ) -> ApplicationResult<SanctionsResult> {
            Ok(self.result.clone())
        }
    }

    #[derive(Debug)]
    struct MockLimitsProvider {
        result: LimitsResult,
    }

    impl MockLimitsProvider {
        fn passing() -> Self {
            Self {
                result: LimitsResult::passed(
                    Decimal::from_i64(50000).unwrap(),
                    Decimal::from_i64(100000).unwrap(),
                    Decimal::from_i64(25000).unwrap(),
                ),
            }
        }

        fn exceeding_daily() -> Self {
            Self {
                result: LimitsResult::failed(
                    Decimal::from_i64(95000).unwrap(),
                    Decimal::from_i64(100000).unwrap(),
                    Decimal::from_i64(25000).unwrap(),
                    vec!["daily volume limit exceeded".to_string()],
                ),
            }
        }

        fn exceeding_per_trade() -> Self {
            Self {
                result: LimitsResult::failed(
                    Decimal::from_i64(50000).unwrap(),
                    Decimal::from_i64(100000).unwrap(),
                    Decimal::from_i64(25000).unwrap(),
                    vec!["per-trade limit exceeded".to_string()],
                ),
            }
        }
    }

    #[async_trait]
    impl LimitsProvider for MockLimitsProvider {
        async fn check_limits(
            &self,
            _client_id: &CounterpartyId,
            _trade_amount: Decimal,
        ) -> ApplicationResult<LimitsResult> {
            Ok(self.result.clone())
        }
    }

    fn create_service(
        kyc: impl KycProvider + 'static,
        aml: impl AmlProvider + 'static,
        sanctions: impl SanctionsProvider + 'static,
        limits: impl LimitsProvider + 'static,
    ) -> ComplianceServiceImpl {
        ComplianceServiceImpl::new(
            Arc::new(kyc),
            Arc::new(aml),
            Arc::new(sanctions),
            Arc::new(limits),
        )
    }

    #[tokio::test]
    async fn check_compliance_all_pass() {
        let service = create_service(
            MockKycProvider::verified(),
            MockAmlProvider::passing(20),
            MockSanctionsProvider::passing(),
            MockLimitsProvider::passing(),
        );

        let client_id = CounterpartyId::new("client-1");
        let result = service
            .check(&client_id, Decimal::from_i64(10000).unwrap())
            .await
            .unwrap();

        assert!(result.passed);
        assert!(result.flags.is_empty());
        assert!(!result.has_blocking_flags());
    }

    #[tokio::test]
    async fn check_compliance_kyc_pending() {
        let service = create_service(
            MockKycProvider::with_status(KycStatus::Pending),
            MockAmlProvider::passing(20),
            MockSanctionsProvider::passing(),
            MockLimitsProvider::passing(),
        );

        let client_id = CounterpartyId::new("client-1");
        let result = service
            .check(&client_id, Decimal::from_i64(10000).unwrap())
            .await
            .unwrap();

        assert!(!result.passed);
        assert!(result.has_blocking_flags());
        assert_eq!(result.blocking_flags().len(), 1);
        assert_eq!(
            result.blocking_flags().first().unwrap().flag_type,
            ComplianceFlagType::KycPending
        );
    }

    #[tokio::test]
    async fn check_compliance_kyc_expired() {
        let service = create_service(
            MockKycProvider::with_status(KycStatus::Expired),
            MockAmlProvider::passing(20),
            MockSanctionsProvider::passing(),
            MockLimitsProvider::passing(),
        );

        let client_id = CounterpartyId::new("client-1");
        let result = service
            .check(&client_id, Decimal::from_i64(10000).unwrap())
            .await
            .unwrap();

        assert!(!result.passed);
        assert_eq!(
            result.blocking_flags().first().unwrap().flag_type,
            ComplianceFlagType::KycExpired
        );
    }

    #[tokio::test]
    async fn check_compliance_aml_alert() {
        let service = create_service(
            MockKycProvider::verified(),
            MockAmlProvider::failing(90, "Suspicious activity detected"),
            MockSanctionsProvider::passing(),
            MockLimitsProvider::passing(),
        );

        let client_id = CounterpartyId::new("client-1");
        let result = service
            .check(&client_id, Decimal::from_i64(10000).unwrap())
            .await
            .unwrap();

        assert!(!result.passed);
        assert_eq!(
            result.blocking_flags().first().unwrap().flag_type,
            ComplianceFlagType::AmlAlert
        );
    }

    #[tokio::test]
    async fn check_compliance_aml_high_risk() {
        let service = create_service(
            MockKycProvider::verified(),
            MockAmlProvider::passing(85), // Above blocking threshold
            MockSanctionsProvider::passing(),
            MockLimitsProvider::passing(),
        );

        let client_id = CounterpartyId::new("client-1");
        let result = service
            .check(&client_id, Decimal::from_i64(10000).unwrap())
            .await
            .unwrap();

        assert!(!result.passed);
        assert_eq!(
            result.blocking_flags().first().unwrap().flag_type,
            ComplianceFlagType::AmlHighRisk
        );
    }

    #[tokio::test]
    async fn check_compliance_aml_warning() {
        let service = create_service(
            MockKycProvider::verified(),
            MockAmlProvider::passing(60), // Above warning, below blocking
            MockSanctionsProvider::passing(),
            MockLimitsProvider::passing(),
        );

        let client_id = CounterpartyId::new("client-1");
        let result = service
            .check(&client_id, Decimal::from_i64(10000).unwrap())
            .await
            .unwrap();

        assert!(result.passed); // Warnings don't block
        assert!(!result.has_blocking_flags());
        assert_eq!(result.flags.len(), 1);
        assert_eq!(
            result.flags.first().unwrap().severity,
            ComplianceSeverity::Warning
        );
    }

    #[tokio::test]
    async fn check_compliance_sanctions_hit() {
        let service = create_service(
            MockKycProvider::verified(),
            MockAmlProvider::passing(20),
            MockSanctionsProvider::failing(vec!["OFAC", "EU"]),
            MockLimitsProvider::passing(),
        );

        let client_id = CounterpartyId::new("client-1");
        let result = service
            .check(&client_id, Decimal::from_i64(10000).unwrap())
            .await
            .unwrap();

        assert!(!result.passed);
        let blocking_flags = result.blocking_flags();
        let first_flag = blocking_flags.first().unwrap();
        assert_eq!(first_flag.flag_type, ComplianceFlagType::SanctionsHit);
        assert!(first_flag.message.contains("OFAC"));
    }

    #[tokio::test]
    async fn check_compliance_daily_limit_exceeded() {
        let service = create_service(
            MockKycProvider::verified(),
            MockAmlProvider::passing(20),
            MockSanctionsProvider::passing(),
            MockLimitsProvider::exceeding_daily(),
        );

        let client_id = CounterpartyId::new("client-1");
        let result = service
            .check(&client_id, Decimal::from_i64(10000).unwrap())
            .await
            .unwrap();

        assert!(!result.passed);
        assert_eq!(
            result.blocking_flags().first().unwrap().flag_type,
            ComplianceFlagType::DailyLimitExceeded
        );
    }

    #[tokio::test]
    async fn check_compliance_per_trade_limit_exceeded() {
        let service = create_service(
            MockKycProvider::verified(),
            MockAmlProvider::passing(20),
            MockSanctionsProvider::passing(),
            MockLimitsProvider::exceeding_per_trade(),
        );

        let client_id = CounterpartyId::new("client-1");
        let result = service
            .check(&client_id, Decimal::from_i64(30000).unwrap())
            .await
            .unwrap();

        assert!(!result.passed);
        assert_eq!(
            result.blocking_flags().first().unwrap().flag_type,
            ComplianceFlagType::PerTradeLimitExceeded
        );
    }

    #[tokio::test]
    async fn check_compliance_multiple_flags() {
        let service = create_service(
            MockKycProvider::with_status(KycStatus::Expired),
            MockAmlProvider::passing(60), // Warning level
            MockSanctionsProvider::passing(),
            MockLimitsProvider::exceeding_daily(),
        );

        let client_id = CounterpartyId::new("client-1");
        let result = service
            .check(&client_id, Decimal::from_i64(10000).unwrap())
            .await
            .unwrap();

        assert!(!result.passed);
        assert_eq!(result.flags.len(), 3); // KYC expired + AML warning + daily limit
        assert_eq!(result.blocking_flags().len(), 2); // KYC expired + daily limit
    }

    #[test]
    fn kyc_status_allows_trading() {
        assert!(KycStatus::Verified.allows_trading());
        assert!(!KycStatus::Pending.allows_trading());
        assert!(!KycStatus::Expired.allows_trading());
        assert!(!KycStatus::Failed.allows_trading());
        assert!(!KycStatus::NotSubmitted.allows_trading());
    }

    #[test]
    fn compliance_flag_is_blocking() {
        let blocking = ComplianceFlag::blocking(ComplianceFlagType::KycExpired, "test");
        let warning = ComplianceFlag::warning(ComplianceFlagType::AmlHighRisk, "test");
        let info = ComplianceFlag::info(ComplianceFlagType::KycPending, "test");

        assert!(blocking.is_blocking());
        assert!(!warning.is_blocking());
        assert!(!info.is_blocking());
    }

    #[test]
    fn compliance_check_result_to_domain() {
        let passed_result = ComplianceCheckResult {
            passed: true,
            kyc_status: KycStatus::Verified,
            aml_result: AmlResult::passed(20),
            sanctions_result: SanctionsResult::passed(),
            limits_result: LimitsResult::passed(
                Decimal::from_i64(50000).unwrap(),
                Decimal::from_i64(100000).unwrap(),
                Decimal::from_i64(25000).unwrap(),
            ),
            flags: Vec::new(),
        };

        let domain_result = passed_result.to_domain_result();
        assert!(domain_result.passed);

        let failed_result = ComplianceCheckResult {
            passed: false,
            kyc_status: KycStatus::Expired,
            aml_result: AmlResult::passed(20),
            sanctions_result: SanctionsResult::passed(),
            limits_result: LimitsResult::passed(
                Decimal::from_i64(50000).unwrap(),
                Decimal::from_i64(100000).unwrap(),
                Decimal::from_i64(25000).unwrap(),
            ),
            flags: vec![ComplianceFlag::blocking(
                ComplianceFlagType::KycExpired,
                "KYC expired",
            )],
        };

        let domain_result = failed_result.to_domain_result();
        assert!(!domain_result.passed);
        assert_eq!(domain_result.reason, Some("KYC expired".to_string()));
    }

    #[test]
    fn aml_result_constructors() {
        let passed = AmlResult::passed(30);
        assert!(passed.passed);
        assert_eq!(passed.risk_score, 30);
        assert!(passed.alert.is_none());

        let failed = AmlResult::failed(90, "Suspicious");
        assert!(!failed.passed);
        assert_eq!(failed.risk_score, 90);
        assert_eq!(failed.alert, Some("Suspicious".to_string()));
    }

    #[test]
    fn sanctions_result_constructors() {
        let passed = SanctionsResult::passed();
        assert!(passed.passed);
        assert!(passed.matched_lists.is_empty());

        let failed = SanctionsResult::failed(vec!["OFAC".to_string()]);
        assert!(!failed.passed);
        assert_eq!(failed.matched_lists, vec!["OFAC".to_string()]);
    }

    #[test]
    fn limits_result_constructors() {
        let passed = LimitsResult::passed(
            Decimal::from_i64(50000).unwrap(),
            Decimal::from_i64(100000).unwrap(),
            Decimal::from_i64(25000).unwrap(),
        );
        assert!(passed.passed);
        assert!(passed.exceeded_limits.is_empty());

        let failed = LimitsResult::failed(
            Decimal::from_i64(95000).unwrap(),
            Decimal::from_i64(100000).unwrap(),
            Decimal::from_i64(25000).unwrap(),
            vec!["daily limit".to_string()],
        );
        assert!(!failed.passed);
        assert_eq!(failed.exceeded_limits.len(), 1);
    }

    #[test]
    fn compliance_config_default() {
        let config = ComplianceConfig::default();
        assert_eq!(config.aml_warning_threshold, 50);
        assert_eq!(config.aml_blocking_threshold, 80);
    }
}
