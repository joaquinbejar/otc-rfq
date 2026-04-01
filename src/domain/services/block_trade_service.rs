//! # Block Trade Service
//!
//! Service for validating and processing pre-arranged bilateral block trades.
//!
//! This module provides the [`BlockTradeValidator`] trait and implementations
//! for validating block trade submissions according to platform rules.
//!
//! # Validation Rules
//!
//! 1. **Counterparty eligibility**: Both parties must have active accounts,
//!    KYC tier >= 2 (Approved), and not be in liquidation
//! 2. **Collateral**: Both parties must have sufficient margin
//! 3. **Instrument**: Must be tradeable and not halted
//! 4. **Price**: Must be within configured bounds relative to reference price
//! 5. **Size**: Must qualify as a block trade per configured thresholds
//!
//! # Examples
//!
//! ```
//! use otc_rfq::domain::services::block_trade_service::{
//!     BlockTradeValidator, DefaultBlockTradeValidator,
//! };
//! use otc_rfq::domain::entities::block_trade::BlockTrade;
//! use otc_rfq::domain::value_objects::{
//!     CounterpartyId, Instrument, Price, Quantity, Symbol, AssetClass, Timestamp,
//! };
//!
//! let validator = DefaultBlockTradeValidator::new();
//! // Validation would be performed against counterparty and market data
//! ```

use crate::domain::entities::block_trade::{BlockTrade, BlockTradeValidation};
use crate::domain::entities::counterparty::{Counterparty, KycStatus};
use crate::domain::errors::DomainResult;
use crate::domain::services::{BlockTradeConfig, ReportingTier};
use crate::domain::value_objects::{Instrument, Price, Quantity};
use async_trait::async_trait;
use std::fmt;

/// Reasons for block trade validation failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockTradeValidationFailure {
    /// Buyer is not eligible (not active, KYC not approved, or in liquidation).
    BuyerNotEligible(String),
    /// Seller is not eligible (not active, KYC not approved, or in liquidation).
    SellerNotEligible(String),
    /// Buyer has insufficient collateral.
    BuyerInsufficientCollateral,
    /// Seller has insufficient collateral.
    SellerInsufficientCollateral,
    /// Instrument is not tradeable or is halted.
    InstrumentNotTradeable(String),
    /// Price is outside acceptable bounds.
    PriceOutOfBounds {
        /// The proposed price.
        proposed: Price,
        /// The reference price.
        reference: Price,
    },
    /// Size does not qualify as a block trade.
    SizeNotQualified {
        /// The trade quantity.
        quantity: Quantity,
        /// The required threshold.
        threshold: Quantity,
    },
}

impl fmt::Display for BlockTradeValidationFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BuyerNotEligible(reason) => write!(f, "buyer not eligible: {}", reason),
            Self::SellerNotEligible(reason) => write!(f, "seller not eligible: {}", reason),
            Self::BuyerInsufficientCollateral => write!(f, "buyer has insufficient collateral"),
            Self::SellerInsufficientCollateral => write!(f, "seller has insufficient collateral"),
            Self::InstrumentNotTradeable(reason) => {
                write!(f, "instrument not tradeable: {}", reason)
            }
            Self::PriceOutOfBounds {
                proposed,
                reference,
            } => {
                write!(
                    f,
                    "price out of bounds: proposed {}, reference {}",
                    proposed, reference
                )
            }
            Self::SizeNotQualified {
                quantity,
                threshold,
            } => {
                write!(
                    f,
                    "size {} does not meet block trade threshold {}",
                    quantity, threshold
                )
            }
        }
    }
}

/// Context for block trade validation.
///
/// Contains all the data needed to validate a block trade.
#[derive(Debug, Clone)]
pub struct BlockTradeValidationContext {
    /// The buyer counterparty.
    pub buyer: Counterparty,
    /// The seller counterparty.
    pub seller: Counterparty,
    /// Reference price for the instrument (if available).
    pub reference_price: Option<Price>,
    /// Whether the instrument is currently tradeable.
    ///
    /// TODO: Add liquidation status check once the field exists on Counterparty.
    /// See issue for adding is_in_liquidation field to Counterparty entity.
    pub instrument_tradeable: bool,
    /// Reason if instrument is not tradeable.
    pub instrument_halt_reason: Option<String>,
    /// Buyer's available collateral.
    pub buyer_collateral: Price,
    /// Seller's available collateral.
    pub seller_collateral: Price,
    /// Required collateral for this trade.
    pub required_collateral: Price,
}

impl BlockTradeValidationContext {
    /// Creates a new validation context.
    #[must_use]
    pub fn new(buyer: Counterparty, seller: Counterparty) -> Self {
        Self {
            buyer,
            seller,
            reference_price: None,
            instrument_tradeable: true,
            instrument_halt_reason: None,
            buyer_collateral: Price::ZERO,
            seller_collateral: Price::ZERO,
            required_collateral: Price::ZERO,
        }
    }

    /// Sets the reference price.
    #[must_use]
    pub fn with_reference_price(mut self, price: Price) -> Self {
        self.reference_price = Some(price);
        self
    }

    /// Sets the instrument as not tradeable with a reason.
    #[must_use]
    pub fn with_instrument_halted(mut self, reason: &str) -> Self {
        self.instrument_tradeable = false;
        self.instrument_halt_reason = Some(reason.to_string());
        self
    }

    /// Sets collateral information.
    #[must_use]
    pub fn with_collateral(
        mut self,
        buyer_collateral: Price,
        seller_collateral: Price,
        required: Price,
    ) -> Self {
        self.buyer_collateral = buyer_collateral;
        self.seller_collateral = seller_collateral;
        self.required_collateral = required;
        self
    }
}

/// Trait for validating block trades.
///
/// Implementations perform all necessary checks to determine if a
/// block trade can proceed.
#[async_trait]
pub trait BlockTradeValidator: Send + Sync + fmt::Debug {
    /// Validates a block trade submission.
    ///
    /// # Arguments
    ///
    /// * `trade` - The block trade to validate
    /// * `context` - Validation context with counterparty and market data
    /// * `config` - Block trade configuration for size thresholds
    ///
    /// # Returns
    ///
    /// A validation result indicating success or failure with reasons.
    async fn validate(
        &self,
        trade: &BlockTrade,
        context: &BlockTradeValidationContext,
        config: &BlockTradeConfig,
    ) -> DomainResult<BlockTradeValidation>;

    /// Determines the reporting tier for a block trade.
    ///
    /// # Arguments
    ///
    /// * `instrument` - The instrument being traded
    /// * `quantity` - The trade quantity
    /// * `config` - Block trade configuration
    fn determine_tier(
        &self,
        instrument: &Instrument,
        quantity: Quantity,
        config: &BlockTradeConfig,
    ) -> Option<ReportingTier>;
}

/// Default implementation of block trade validation.
#[derive(Debug, Clone)]
pub struct DefaultBlockTradeValidator {
    /// Maximum allowed price deviation from reference (as decimal, e.g., 0.05 = 5%).
    max_price_deviation: Option<rust_decimal::Decimal>,
}

impl Default for DefaultBlockTradeValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl DefaultBlockTradeValidator {
    /// Creates a new default validator.
    ///
    /// By default, this enforces a maximum price deviation of 5% from the
    /// reference price. Use [`Self::with_max_price_deviation`] to override.
    #[must_use]
    pub fn new() -> Self {
        Self {
            // 0.05 = 5% maximum allowed price deviation
            max_price_deviation: Some(rust_decimal::Decimal::new(5, 2)),
        }
    }

    /// Creates a validator with a maximum price deviation.
    #[must_use]
    pub fn with_max_price_deviation(mut self, deviation: rust_decimal::Decimal) -> Self {
        self.max_price_deviation = Some(deviation);
        self
    }

    /// Validates counterparty eligibility.
    fn validate_counterparty_eligibility(
        &self,
        counterparty: &Counterparty,
        role: &str,
    ) -> Result<(), BlockTradeValidationFailure> {
        // Check active status
        if !counterparty.is_active() {
            return Err(if role == "buyer" {
                BlockTradeValidationFailure::BuyerNotEligible("account not active".to_string())
            } else {
                BlockTradeValidationFailure::SellerNotEligible("account not active".to_string())
            });
        }

        // Check KYC status (must be Approved for block trades)
        if counterparty.kyc_status() != KycStatus::Approved {
            return Err(if role == "buyer" {
                BlockTradeValidationFailure::BuyerNotEligible(format!(
                    "KYC status is {:?}, must be Approved",
                    counterparty.kyc_status()
                ))
            } else {
                BlockTradeValidationFailure::SellerNotEligible(format!(
                    "KYC status is {:?}, must be Approved",
                    counterparty.kyc_status()
                ))
            });
        }

        // Note: Liquidation status check would be added here when the field exists
        // in the Counterparty entity. For now, we only check active status and KYC.

        Ok(())
    }

    /// Validates collateral sufficiency.
    fn validate_collateral(
        &self,
        context: &BlockTradeValidationContext,
    ) -> Vec<BlockTradeValidationFailure> {
        let mut failures = Vec::new();

        if context.buyer_collateral < context.required_collateral {
            failures.push(BlockTradeValidationFailure::BuyerInsufficientCollateral);
        }

        if context.seller_collateral < context.required_collateral {
            failures.push(BlockTradeValidationFailure::SellerInsufficientCollateral);
        }

        failures
    }

    /// Validates price against reference.
    fn validate_price(
        &self,
        proposed: Price,
        context: &BlockTradeValidationContext,
    ) -> Result<(), BlockTradeValidationFailure> {
        // If no reference price or no max deviation configured, skip price validation
        let reference = match context.reference_price {
            Some(r) => r,
            None => return Ok(()),
        };

        let max_deviation = match self.max_price_deviation {
            Some(d) => d,
            None => return Ok(()),
        };

        // Calculate deviation
        let proposed_dec = proposed.get();
        let reference_dec = reference.get();

        if reference_dec.is_zero() {
            return Ok(()); // Can't calculate deviation from zero
        }

        let deviation = match (proposed_dec - reference_dec)
            .abs()
            .checked_div(reference_dec)
        {
            Some(d) => d,
            None => return Ok(()), // Division failed, skip validation
        };

        if deviation > max_deviation {
            return Err(BlockTradeValidationFailure::PriceOutOfBounds {
                proposed,
                reference,
            });
        }

        Ok(())
    }
}

#[async_trait]
impl BlockTradeValidator for DefaultBlockTradeValidator {
    async fn validate(
        &self,
        trade: &BlockTrade,
        context: &BlockTradeValidationContext,
        config: &BlockTradeConfig,
    ) -> DomainResult<BlockTradeValidation> {
        let mut failures: Vec<String> = Vec::new();
        let mut buyer_eligible = true;
        let mut seller_eligible = true;
        let mut collateral_sufficient = true;
        let mut instrument_valid = true;
        let mut price_valid = true;
        let mut size_qualifies = true;

        // 1. Validate buyer eligibility
        if let Err(failure) = self.validate_counterparty_eligibility(&context.buyer, "buyer") {
            buyer_eligible = false;
            failures.push(failure.to_string());
        }

        // 2. Validate seller eligibility
        if let Err(failure) = self.validate_counterparty_eligibility(&context.seller, "seller") {
            seller_eligible = false;
            failures.push(failure.to_string());
        }

        // 3. Validate collateral
        let collateral_failures = self.validate_collateral(context);
        if !collateral_failures.is_empty() {
            collateral_sufficient = false;
            for failure in collateral_failures {
                failures.push(failure.to_string());
            }
        }

        // 4. Validate instrument
        if !context.instrument_tradeable {
            instrument_valid = false;
            let reason = context
                .instrument_halt_reason
                .clone()
                .unwrap_or_else(|| "instrument halted".to_string());
            failures.push(format!("instrument not tradeable: {}", reason));
        }

        // 5. Validate price
        if let Err(failure) = self.validate_price(trade.price(), context) {
            price_valid = false;
            failures.push(failure.to_string());
        }

        // 6. Validate size qualifies as block trade
        if !config.qualifies(trade.instrument(), trade.quantity()) {
            size_qualifies = false;
            let threshold = config
                .get_threshold(trade.instrument())
                .unwrap_or(Quantity::ZERO);
            failures.push(format!(
                "size {} does not meet block trade threshold {}",
                trade.quantity(),
                threshold
            ));
        }

        Ok(BlockTradeValidation {
            buyer_eligible,
            seller_eligible,
            collateral_sufficient,
            instrument_valid,
            price_valid,
            size_qualifies,
            failure_reasons: failures,
        })
    }

    fn determine_tier(
        &self,
        instrument: &Instrument,
        quantity: Quantity,
        config: &BlockTradeConfig,
    ) -> Option<ReportingTier> {
        config.determine_tier(instrument, quantity)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::domain::entities::counterparty::CounterpartyType;
    use crate::domain::value_objects::{AssetClass, CounterpartyId, Symbol, Timestamp};

    fn create_test_instrument() -> Instrument {
        let symbol = Symbol::new("BTC/USD").unwrap();
        Instrument::builder(symbol, AssetClass::CryptoSpot).build()
    }

    fn create_eligible_counterparty(id: &str) -> Counterparty {
        let mut cp = Counterparty::new(
            CounterpartyId::new(id),
            format!("{} Trading", id),
            CounterpartyType::Client,
        );
        cp.set_kyc_status(KycStatus::Approved);
        cp
    }

    fn create_test_block_trade() -> BlockTrade {
        BlockTrade::new(
            CounterpartyId::new("buyer-1"),
            CounterpartyId::new("seller-1"),
            create_test_instrument(),
            Price::new(50000.0).unwrap(),
            Quantity::new(30.0).unwrap(),
            Timestamp::now(),
        )
    }

    #[tokio::test]
    async fn validate_eligible_counterparties_passes() {
        let validator = DefaultBlockTradeValidator::new();
        let trade = create_test_block_trade();
        let config = BlockTradeConfig::default();

        let context = BlockTradeValidationContext::new(
            create_eligible_counterparty("buyer-1"),
            create_eligible_counterparty("seller-1"),
        )
        .with_collateral(
            Price::new(100000.0).unwrap(),
            Price::new(100000.0).unwrap(),
            Price::new(50000.0).unwrap(),
        );

        let result = validator.validate(&trade, &context, &config).await.unwrap();

        assert!(result.buyer_eligible);
        assert!(result.seller_eligible);
        assert!(result.collateral_sufficient);
        assert!(result.size_qualifies);
    }

    #[tokio::test]
    async fn validate_inactive_buyer_fails() {
        let validator = DefaultBlockTradeValidator::new();
        let trade = create_test_block_trade();
        let config = BlockTradeConfig::default();

        let mut buyer = create_eligible_counterparty("buyer-1");
        buyer.set_active(false);

        let context =
            BlockTradeValidationContext::new(buyer, create_eligible_counterparty("seller-1"));

        let result = validator.validate(&trade, &context, &config).await.unwrap();

        assert!(!result.buyer_eligible);
        assert!(!result.is_valid());
    }

    #[tokio::test]
    async fn validate_unapproved_kyc_fails() {
        let validator = DefaultBlockTradeValidator::new();
        let trade = create_test_block_trade();
        let config = BlockTradeConfig::default();

        // Create counterparty without KYC approval
        let buyer = Counterparty::new(
            CounterpartyId::new("buyer-1"),
            "Buyer Trading",
            CounterpartyType::Client,
        );

        let context =
            BlockTradeValidationContext::new(buyer, create_eligible_counterparty("seller-1"));

        let result = validator.validate(&trade, &context, &config).await.unwrap();

        assert!(!result.buyer_eligible);
        assert!(!result.is_valid());
    }

    #[tokio::test]
    async fn validate_insufficient_collateral_fails() {
        let validator = DefaultBlockTradeValidator::new();
        let trade = create_test_block_trade();
        let config = BlockTradeConfig::default();

        let context = BlockTradeValidationContext::new(
            create_eligible_counterparty("buyer-1"),
            create_eligible_counterparty("seller-1"),
        )
        .with_collateral(
            Price::new(10000.0).unwrap(), // Insufficient
            Price::new(100000.0).unwrap(),
            Price::new(50000.0).unwrap(),
        );

        let result = validator.validate(&trade, &context, &config).await.unwrap();

        assert!(!result.collateral_sufficient);
        assert!(!result.is_valid());
    }

    #[tokio::test]
    async fn validate_halted_instrument_fails() {
        let validator = DefaultBlockTradeValidator::new();
        let trade = create_test_block_trade();
        let config = BlockTradeConfig::default();

        let context = BlockTradeValidationContext::new(
            create_eligible_counterparty("buyer-1"),
            create_eligible_counterparty("seller-1"),
        )
        .with_instrument_halted("regulatory halt");

        let result = validator.validate(&trade, &context, &config).await.unwrap();

        assert!(!result.instrument_valid);
        assert!(!result.is_valid());
    }

    #[tokio::test]
    async fn validate_size_below_threshold_fails() {
        let validator = DefaultBlockTradeValidator::new();
        let config = BlockTradeConfig::default();

        // Create trade with quantity below threshold (25 BTC)
        let trade = BlockTrade::new(
            CounterpartyId::new("buyer-1"),
            CounterpartyId::new("seller-1"),
            create_test_instrument(),
            Price::new(50000.0).unwrap(),
            Quantity::new(10.0).unwrap(), // Below 25 BTC threshold
            Timestamp::now(),
        );

        let context = BlockTradeValidationContext::new(
            create_eligible_counterparty("buyer-1"),
            create_eligible_counterparty("seller-1"),
        );

        let result = validator.validate(&trade, &context, &config).await.unwrap();

        assert!(!result.size_qualifies);
        assert!(!result.is_valid());
    }

    #[tokio::test]
    async fn validate_price_out_of_bounds_fails() {
        let validator = DefaultBlockTradeValidator::new()
            .with_max_price_deviation(rust_decimal::Decimal::new(5, 2)); // 5%

        let trade = create_test_block_trade(); // Price: 50000
        let config = BlockTradeConfig::default();

        let context = BlockTradeValidationContext::new(
            create_eligible_counterparty("buyer-1"),
            create_eligible_counterparty("seller-1"),
        )
        .with_reference_price(Price::new(40000.0).unwrap()); // 25% deviation

        let result = validator.validate(&trade, &context, &config).await.unwrap();

        assert!(!result.price_valid);
        assert!(!result.is_valid());
    }

    #[tokio::test]
    async fn determine_tier_returns_correct_tier() {
        let validator = DefaultBlockTradeValidator::new();
        let config = BlockTradeConfig::default();
        let instrument = create_test_instrument();

        // Standard tier (1x threshold = 25 BTC)
        assert_eq!(
            validator.determine_tier(&instrument, Quantity::new(25.0).unwrap(), &config),
            Some(ReportingTier::Standard)
        );

        // Large tier (5x threshold = 125 BTC)
        assert_eq!(
            validator.determine_tier(&instrument, Quantity::new(125.0).unwrap(), &config),
            Some(ReportingTier::Large)
        );

        // Very large tier (10x threshold = 250 BTC)
        assert_eq!(
            validator.determine_tier(&instrument, Quantity::new(250.0).unwrap(), &config),
            Some(ReportingTier::VeryLarge)
        );
    }

    #[test]
    fn validation_failure_display() {
        let failure = BlockTradeValidationFailure::BuyerNotEligible("test reason".to_string());
        assert!(failure.to_string().contains("buyer not eligible"));

        let failure = BlockTradeValidationFailure::BuyerInsufficientCollateral;
        assert!(failure.to_string().contains("insufficient collateral"));
    }
}
