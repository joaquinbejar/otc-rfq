//! # Compliance Value Objects
//!
//! KYC/AML compliance check results.
//!
//! This module provides value objects for tracking compliance verification
//! results including KYC, AML, sanctions screening, and regulatory flags.
//!
//! # Examples
//!
//! ```
//! use otc_rfq::domain::value_objects::compliance::{ComplianceCheckResults, RegulatoryFlag};
//!
//! let results = ComplianceCheckResults::builder()
//!     .kyc_passed(true)
//!     .aml_passed(true)
//!     .sanctions_passed(true)
//!     .limits_checked(true)
//!     .build();
//!
//! assert!(results.all_passed());
//! assert!(!results.has_blocking_flags());
//! ```

use crate::domain::value_objects::Price;
use crate::domain::value_objects::timestamp::Timestamp;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Regulatory flags that may be raised during compliance checks.
///
/// Some flags are blocking (prevent trade execution) while others
/// are informational (allow execution but require reporting).
///
/// # Examples
///
/// ```
/// use otc_rfq::domain::value_objects::compliance::RegulatoryFlag;
/// use otc_rfq::domain::value_objects::Price;
///
/// let flag = RegulatoryFlag::HighNotional {
///     threshold: Price::new(1_000_000.0).unwrap(),
///     actual: Price::new(1_500_000.0).unwrap(),
/// };
///
/// assert!(!flag.is_blocking());
///
/// let blocking = RegulatoryFlag::CounterpartyBlacklist {
///     reason: "Sanctioned entity".to_string(),
/// };
///
/// assert!(blocking.is_blocking());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RegulatoryFlag {
    /// Trade notional exceeds reporting threshold.
    HighNotional {
        /// The threshold that was exceeded.
        threshold: Price,
        /// The actual notional amount.
        actual: Price,
    },

    /// Large trade notification required.
    LargeTradeNotification,

    /// Suspicious trading pattern detected.
    SuspiciousPattern {
        /// Description of the pattern.
        pattern: String,
    },

    /// Counterparty is on a watchlist (non-blocking).
    CounterpartyWatchlist {
        /// Reason for being on the watchlist.
        reason: String,
    },

    /// Counterparty is blacklisted (BLOCKING).
    CounterpartyBlacklist {
        /// Reason for being blacklisted.
        reason: String,
    },

    /// Manual review is required (BLOCKING).
    ManualReviewRequired {
        /// Reason for requiring manual review.
        reason: String,
    },
}

impl RegulatoryFlag {
    /// Returns true if this flag blocks trade execution.
    ///
    /// Blocking flags prevent the trade from proceeding until resolved.
    ///
    /// # Blocking Flags
    ///
    /// - [`CounterpartyBlacklist`](RegulatoryFlag::CounterpartyBlacklist)
    /// - [`ManualReviewRequired`](RegulatoryFlag::ManualReviewRequired)
    #[must_use]
    pub const fn is_blocking(&self) -> bool {
        matches!(
            self,
            Self::CounterpartyBlacklist { .. } | Self::ManualReviewRequired { .. }
        )
    }

    /// Returns true if this flag is informational only.
    #[must_use]
    pub const fn is_informational(&self) -> bool {
        !self.is_blocking()
    }

    /// Returns the flag type as a string.
    #[must_use]
    pub const fn flag_type(&self) -> &'static str {
        match self {
            Self::HighNotional { .. } => "HIGH_NOTIONAL",
            Self::LargeTradeNotification => "LARGE_TRADE_NOTIFICATION",
            Self::SuspiciousPattern { .. } => "SUSPICIOUS_PATTERN",
            Self::CounterpartyWatchlist { .. } => "COUNTERPARTY_WATCHLIST",
            Self::CounterpartyBlacklist { .. } => "COUNTERPARTY_BLACKLIST",
            Self::ManualReviewRequired { .. } => "MANUAL_REVIEW_REQUIRED",
        }
    }
}

impl fmt::Display for RegulatoryFlag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HighNotional { threshold, actual } => {
                write!(
                    f,
                    "HIGH_NOTIONAL(threshold={}, actual={})",
                    threshold, actual
                )
            }
            Self::LargeTradeNotification => write!(f, "LARGE_TRADE_NOTIFICATION"),
            Self::SuspiciousPattern { pattern } => {
                write!(f, "SUSPICIOUS_PATTERN({})", pattern)
            }
            Self::CounterpartyWatchlist { reason } => {
                write!(f, "COUNTERPARTY_WATCHLIST({})", reason)
            }
            Self::CounterpartyBlacklist { reason } => {
                write!(f, "COUNTERPARTY_BLACKLIST[BLOCKING]({})", reason)
            }
            Self::ManualReviewRequired { reason } => {
                write!(f, "MANUAL_REVIEW_REQUIRED[BLOCKING]({})", reason)
            }
        }
    }
}

/// Results of compliance checks for a trade.
///
/// Contains the results of various compliance checks (KYC, AML, sanctions)
/// and any regulatory flags that were raised.
///
/// # Examples
///
/// ```
/// use otc_rfq::domain::value_objects::compliance::{ComplianceCheckResults, RegulatoryFlag};
///
/// // All checks passed
/// let passed = ComplianceCheckResults::all_checks_passed();
/// assert!(passed.all_passed());
///
/// // Some checks failed
/// let failed = ComplianceCheckResults::builder()
///     .kyc_passed(false)
///     .aml_passed(true)
///     .sanctions_passed(true)
///     .limits_checked(true)
///     .build();
/// assert!(!failed.all_passed());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComplianceCheckResults {
    /// Whether KYC verification passed.
    kyc_passed: bool,
    /// Whether AML check passed.
    aml_passed: bool,
    /// Whether trading limits were checked.
    limits_checked: bool,
    /// Whether sanctions screening passed.
    sanctions_passed: bool,
    /// Regulatory flags raised during checks.
    regulatory_flags: Vec<RegulatoryFlag>,
    /// When the checks were performed.
    checked_at: Timestamp,
}

impl ComplianceCheckResults {
    /// Creates a new builder for compliance check results.
    #[must_use]
    pub fn builder() -> ComplianceCheckResultsBuilder {
        ComplianceCheckResultsBuilder::new()
    }

    /// Creates results where all checks passed with no flags.
    #[must_use]
    pub fn all_checks_passed() -> Self {
        Self {
            kyc_passed: true,
            aml_passed: true,
            limits_checked: true,
            sanctions_passed: true,
            regulatory_flags: Vec::new(),
            checked_at: Timestamp::now(),
        }
    }

    /// Creates results from individual components (for reconstruction from storage).
    #[must_use]
    pub fn from_parts(
        kyc_passed: bool,
        aml_passed: bool,
        limits_checked: bool,
        sanctions_passed: bool,
        regulatory_flags: Vec<RegulatoryFlag>,
        checked_at: Timestamp,
    ) -> Self {
        Self {
            kyc_passed,
            aml_passed,
            limits_checked,
            sanctions_passed,
            regulatory_flags,
            checked_at,
        }
    }

    // ========== Accessors ==========

    /// Returns whether KYC verification passed.
    #[inline]
    #[must_use]
    pub fn kyc_passed(&self) -> bool {
        self.kyc_passed
    }

    /// Returns whether AML check passed.
    #[inline]
    #[must_use]
    pub fn aml_passed(&self) -> bool {
        self.aml_passed
    }

    /// Returns whether trading limits were checked.
    #[inline]
    #[must_use]
    pub fn limits_checked(&self) -> bool {
        self.limits_checked
    }

    /// Returns whether sanctions screening passed.
    #[inline]
    #[must_use]
    pub fn sanctions_passed(&self) -> bool {
        self.sanctions_passed
    }

    /// Returns the regulatory flags.
    #[inline]
    #[must_use]
    pub fn regulatory_flags(&self) -> &[RegulatoryFlag] {
        &self.regulatory_flags
    }

    /// Returns when the checks were performed.
    #[inline]
    #[must_use]
    pub fn checked_at(&self) -> Timestamp {
        self.checked_at
    }

    // ========== Status Helpers ==========

    /// Returns true if all compliance checks passed and there are no blocking flags.
    ///
    /// This is the primary method to determine if a trade can proceed.
    #[must_use]
    pub fn all_passed(&self) -> bool {
        self.kyc_passed
            && self.aml_passed
            && self.limits_checked
            && self.sanctions_passed
            && !self.has_blocking_flags()
    }

    /// Returns true if any regulatory flag is blocking.
    #[must_use]
    pub fn has_blocking_flags(&self) -> bool {
        self.regulatory_flags
            .iter()
            .any(RegulatoryFlag::is_blocking)
    }

    /// Returns true if there are any regulatory flags (blocking or informational).
    #[must_use]
    pub fn has_flags(&self) -> bool {
        !self.regulatory_flags.is_empty()
    }

    /// Returns only the blocking flags.
    #[must_use]
    pub fn blocking_flags(&self) -> Vec<&RegulatoryFlag> {
        self.regulatory_flags
            .iter()
            .filter(|f| f.is_blocking())
            .collect()
    }

    /// Returns only the informational (non-blocking) flags.
    #[must_use]
    pub fn informational_flags(&self) -> Vec<&RegulatoryFlag> {
        self.regulatory_flags
            .iter()
            .filter(|f| f.is_informational())
            .collect()
    }

    /// Returns the number of flags.
    #[must_use]
    pub fn flag_count(&self) -> usize {
        self.regulatory_flags.len()
    }
}

impl Default for ComplianceCheckResults {
    fn default() -> Self {
        Self::all_checks_passed()
    }
}

impl fmt::Display for ComplianceCheckResults {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ComplianceCheckResults(kyc={}, aml={}, limits={}, sanctions={}, flags={})",
            self.kyc_passed,
            self.aml_passed,
            self.limits_checked,
            self.sanctions_passed,
            self.regulatory_flags.len()
        )
    }
}

/// Builder for [`ComplianceCheckResults`].
#[derive(Debug, Default)]
pub struct ComplianceCheckResultsBuilder {
    kyc_passed: bool,
    aml_passed: bool,
    limits_checked: bool,
    sanctions_passed: bool,
    regulatory_flags: Vec<RegulatoryFlag>,
}

impl ComplianceCheckResultsBuilder {
    /// Creates a new builder with all checks defaulting to false.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the KYC verification result.
    #[must_use]
    pub fn kyc_passed(mut self, passed: bool) -> Self {
        self.kyc_passed = passed;
        self
    }

    /// Sets the AML check result.
    #[must_use]
    pub fn aml_passed(mut self, passed: bool) -> Self {
        self.aml_passed = passed;
        self
    }

    /// Sets the limits checked result.
    #[must_use]
    pub fn limits_checked(mut self, checked: bool) -> Self {
        self.limits_checked = checked;
        self
    }

    /// Sets the sanctions screening result.
    #[must_use]
    pub fn sanctions_passed(mut self, passed: bool) -> Self {
        self.sanctions_passed = passed;
        self
    }

    /// Adds a regulatory flag.
    #[must_use]
    pub fn add_flag(mut self, flag: RegulatoryFlag) -> Self {
        self.regulatory_flags.push(flag);
        self
    }

    /// Sets all regulatory flags.
    #[must_use]
    pub fn flags(mut self, flags: Vec<RegulatoryFlag>) -> Self {
        self.regulatory_flags = flags;
        self
    }

    /// Builds the compliance check results.
    #[must_use]
    pub fn build(self) -> ComplianceCheckResults {
        ComplianceCheckResults {
            kyc_passed: self.kyc_passed,
            aml_passed: self.aml_passed,
            limits_checked: self.limits_checked,
            sanctions_passed: self.sanctions_passed,
            regulatory_flags: self.regulatory_flags,
            checked_at: Timestamp::now(),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    mod regulatory_flag {
        use super::*;

        #[test]
        fn high_notional_is_not_blocking() {
            let flag = RegulatoryFlag::HighNotional {
                threshold: Price::new(1_000_000.0).unwrap(),
                actual: Price::new(1_500_000.0).unwrap(),
            };

            assert!(!flag.is_blocking());
            assert!(flag.is_informational());
        }

        #[test]
        fn large_trade_notification_is_not_blocking() {
            let flag = RegulatoryFlag::LargeTradeNotification;

            assert!(!flag.is_blocking());
            assert!(flag.is_informational());
        }

        #[test]
        fn suspicious_pattern_is_not_blocking() {
            let flag = RegulatoryFlag::SuspiciousPattern {
                pattern: "Unusual volume".to_string(),
            };

            assert!(!flag.is_blocking());
        }

        #[test]
        fn counterparty_watchlist_is_not_blocking() {
            let flag = RegulatoryFlag::CounterpartyWatchlist {
                reason: "PEP".to_string(),
            };

            assert!(!flag.is_blocking());
        }

        #[test]
        fn counterparty_blacklist_is_blocking() {
            let flag = RegulatoryFlag::CounterpartyBlacklist {
                reason: "Sanctioned entity".to_string(),
            };

            assert!(flag.is_blocking());
            assert!(!flag.is_informational());
        }

        #[test]
        fn manual_review_required_is_blocking() {
            let flag = RegulatoryFlag::ManualReviewRequired {
                reason: "First trade with counterparty".to_string(),
            };

            assert!(flag.is_blocking());
        }

        #[test]
        fn flag_type() {
            assert_eq!(
                RegulatoryFlag::HighNotional {
                    threshold: Price::ZERO,
                    actual: Price::ZERO
                }
                .flag_type(),
                "HIGH_NOTIONAL"
            );
            assert_eq!(
                RegulatoryFlag::LargeTradeNotification.flag_type(),
                "LARGE_TRADE_NOTIFICATION"
            );
            assert_eq!(
                RegulatoryFlag::CounterpartyBlacklist {
                    reason: String::new()
                }
                .flag_type(),
                "COUNTERPARTY_BLACKLIST"
            );
        }

        #[test]
        fn display() {
            let flag = RegulatoryFlag::CounterpartyBlacklist {
                reason: "Test".to_string(),
            };

            let display = flag.to_string();
            assert!(display.contains("BLOCKING"));
            assert!(display.contains("Test"));
        }

        #[test]
        fn serde_roundtrip() {
            let flag = RegulatoryFlag::HighNotional {
                threshold: Price::new(1_000_000.0).unwrap(),
                actual: Price::new(1_500_000.0).unwrap(),
            };

            let json = serde_json::to_string(&flag).unwrap();
            let deserialized: RegulatoryFlag = serde_json::from_str(&json).unwrap();

            assert_eq!(flag, deserialized);
        }
    }

    mod compliance_check_results {
        use super::*;

        #[test]
        fn all_checks_passed() {
            let results = ComplianceCheckResults::all_checks_passed();

            assert!(results.kyc_passed());
            assert!(results.aml_passed());
            assert!(results.limits_checked());
            assert!(results.sanctions_passed());
            assert!(results.all_passed());
            assert!(!results.has_flags());
        }

        #[test]
        fn builder_all_passed() {
            let results = ComplianceCheckResults::builder()
                .kyc_passed(true)
                .aml_passed(true)
                .limits_checked(true)
                .sanctions_passed(true)
                .build();

            assert!(results.all_passed());
        }

        #[test]
        fn builder_kyc_failed() {
            let results = ComplianceCheckResults::builder()
                .kyc_passed(false)
                .aml_passed(true)
                .limits_checked(true)
                .sanctions_passed(true)
                .build();

            assert!(!results.all_passed());
            assert!(!results.kyc_passed());
        }

        #[test]
        fn builder_with_informational_flag() {
            let results = ComplianceCheckResults::builder()
                .kyc_passed(true)
                .aml_passed(true)
                .limits_checked(true)
                .sanctions_passed(true)
                .add_flag(RegulatoryFlag::LargeTradeNotification)
                .build();

            assert!(results.all_passed());
            assert!(results.has_flags());
            assert!(!results.has_blocking_flags());
            assert_eq!(results.flag_count(), 1);
        }

        #[test]
        fn builder_with_blocking_flag() {
            let results = ComplianceCheckResults::builder()
                .kyc_passed(true)
                .aml_passed(true)
                .limits_checked(true)
                .sanctions_passed(true)
                .add_flag(RegulatoryFlag::CounterpartyBlacklist {
                    reason: "Sanctioned".to_string(),
                })
                .build();

            assert!(!results.all_passed());
            assert!(results.has_blocking_flags());
        }

        #[test]
        fn blocking_flags_filter() {
            let results = ComplianceCheckResults::builder()
                .kyc_passed(true)
                .aml_passed(true)
                .limits_checked(true)
                .sanctions_passed(true)
                .add_flag(RegulatoryFlag::LargeTradeNotification)
                .add_flag(RegulatoryFlag::CounterpartyBlacklist {
                    reason: "Test".to_string(),
                })
                .add_flag(RegulatoryFlag::HighNotional {
                    threshold: Price::new(1_000_000.0).unwrap(),
                    actual: Price::new(2_000_000.0).unwrap(),
                })
                .build();

            assert_eq!(results.blocking_flags().len(), 1);
            assert_eq!(results.informational_flags().len(), 2);
            assert_eq!(results.flag_count(), 3);
        }

        #[test]
        fn default_is_all_passed() {
            let results = ComplianceCheckResults::default();
            assert!(results.all_passed());
        }

        #[test]
        fn display() {
            let results = ComplianceCheckResults::builder()
                .kyc_passed(true)
                .aml_passed(false)
                .limits_checked(true)
                .sanctions_passed(true)
                .build();

            let display = results.to_string();
            assert!(display.contains("kyc=true"));
            assert!(display.contains("aml=false"));
        }

        #[test]
        fn serde_roundtrip() {
            let results = ComplianceCheckResults::builder()
                .kyc_passed(true)
                .aml_passed(true)
                .limits_checked(true)
                .sanctions_passed(true)
                .add_flag(RegulatoryFlag::LargeTradeNotification)
                .build();

            let json = serde_json::to_string(&results).unwrap();
            let deserialized: ComplianceCheckResults = serde_json::from_str(&json).unwrap();

            assert_eq!(results.kyc_passed(), deserialized.kyc_passed());
            assert_eq!(results.flag_count(), deserialized.flag_count());
        }

        #[test]
        fn from_parts() {
            let checked_at = Timestamp::now();
            let flags = vec![RegulatoryFlag::LargeTradeNotification];

            let results = ComplianceCheckResults::from_parts(
                true,
                true,
                true,
                false,
                flags.clone(),
                checked_at,
            );

            assert!(results.kyc_passed());
            assert!(!results.sanctions_passed());
            assert_eq!(results.checked_at(), checked_at);
            assert_eq!(results.flag_count(), 1);
        }
    }
}
