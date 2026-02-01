//! # Counterparty Entity
//!
//! Represents a client or market maker.
//!
//! This module provides the [`Counterparty`] entity representing clients,
//! market makers, and other trading participants, including KYC status
//! and trading limits.
//!
//! # Examples
//!
//! ```
//! use otc_rfq::domain::entities::counterparty::{Counterparty, CounterpartyType, KycStatus};
//! use otc_rfq::domain::value_objects::CounterpartyId;
//!
//! let counterparty = Counterparty::new(
//!     CounterpartyId::new("acme-trading"),
//!     "Acme Trading",
//!     CounterpartyType::Client,
//! );
//!
//! assert!(counterparty.is_active());
//! assert_eq!(counterparty.kyc_status(), KycStatus::NotStarted);
//! ```

use crate::domain::value_objects::timestamp::Timestamp;
use crate::domain::value_objects::{Blockchain, CounterpartyId, Price};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Type of counterparty.
///
/// Represents the classification of a trading participant.
///
/// # Examples
///
/// ```
/// use otc_rfq::domain::entities::counterparty::CounterpartyType;
///
/// let client = CounterpartyType::Client;
/// assert!(client.requires_kyc());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[repr(u8)]
pub enum CounterpartyType {
    /// External client (requires KYC).
    #[default]
    Client = 0,

    /// Market maker providing liquidity.
    MarketMaker = 1,

    /// Decentralized exchange.
    Dex = 2,

    /// Internal account.
    Internal = 3,
}

impl CounterpartyType {
    /// Returns true if this counterparty type requires KYC.
    #[inline]
    #[must_use]
    pub const fn requires_kyc(&self) -> bool {
        matches!(self, Self::Client | Self::MarketMaker)
    }

    /// Returns true if this is a client.
    #[inline]
    #[must_use]
    pub const fn is_client(&self) -> bool {
        matches!(self, Self::Client)
    }

    /// Returns true if this is a market maker.
    #[inline]
    #[must_use]
    pub const fn is_market_maker(&self) -> bool {
        matches!(self, Self::MarketMaker)
    }

    /// Returns true if this is a DEX.
    #[inline]
    #[must_use]
    pub const fn is_dex(&self) -> bool {
        matches!(self, Self::Dex)
    }

    /// Returns true if this is an internal account.
    #[inline]
    #[must_use]
    pub const fn is_internal(&self) -> bool {
        matches!(self, Self::Internal)
    }

    /// Returns the numeric value of this type.
    #[inline]
    #[must_use]
    pub const fn as_u8(&self) -> u8 {
        *self as u8
    }
}

impl fmt::Display for CounterpartyType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Client => "CLIENT",
            Self::MarketMaker => "MARKET_MAKER",
            Self::Dex => "DEX",
            Self::Internal => "INTERNAL",
        };
        write!(f, "{}", s)
    }
}

impl TryFrom<u8> for CounterpartyType {
    type Error = InvalidCounterpartyTypeError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Client),
            1 => Ok(Self::MarketMaker),
            2 => Ok(Self::Dex),
            3 => Ok(Self::Internal),
            _ => Err(InvalidCounterpartyTypeError(value)),
        }
    }
}

/// Error returned when converting an invalid u8 to CounterpartyType.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidCounterpartyTypeError(pub u8);

impl fmt::Display for InvalidCounterpartyTypeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid counterparty type value: {}", self.0)
    }
}

impl std::error::Error for InvalidCounterpartyTypeError {}

/// KYC (Know Your Customer) status.
///
/// Represents the current state of KYC verification for a counterparty.
///
/// # Examples
///
/// ```
/// use otc_rfq::domain::entities::counterparty::KycStatus;
///
/// let status = KycStatus::Approved;
/// assert!(status.is_approved());
/// assert!(status.can_trade());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[repr(u8)]
pub enum KycStatus {
    /// KYC process has not been started.
    #[default]
    NotStarted = 0,

    /// KYC verification is in progress.
    Pending = 1,

    /// KYC verification approved.
    Approved = 2,

    /// KYC verification rejected.
    Rejected = 3,
}

impl KycStatus {
    /// Returns true if KYC is approved.
    #[inline]
    #[must_use]
    pub const fn is_approved(&self) -> bool {
        matches!(self, Self::Approved)
    }

    /// Returns true if KYC is pending.
    #[inline]
    #[must_use]
    pub const fn is_pending(&self) -> bool {
        matches!(self, Self::Pending)
    }

    /// Returns true if KYC is rejected.
    #[inline]
    #[must_use]
    pub const fn is_rejected(&self) -> bool {
        matches!(self, Self::Rejected)
    }

    /// Returns true if the counterparty can trade (approved or not started for non-KYC types).
    #[inline]
    #[must_use]
    pub const fn can_trade(&self) -> bool {
        matches!(self, Self::Approved)
    }

    /// Returns the numeric value of this status.
    #[inline]
    #[must_use]
    pub const fn as_u8(&self) -> u8 {
        *self as u8
    }
}

impl fmt::Display for KycStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::NotStarted => "NOT_STARTED",
            Self::Pending => "PENDING",
            Self::Approved => "APPROVED",
            Self::Rejected => "REJECTED",
        };
        write!(f, "{}", s)
    }
}

impl TryFrom<u8> for KycStatus {
    type Error = InvalidKycStatusError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::NotStarted),
            1 => Ok(Self::Pending),
            2 => Ok(Self::Approved),
            3 => Ok(Self::Rejected),
            _ => Err(InvalidKycStatusError(value)),
        }
    }
}

/// Error returned when converting an invalid u8 to KycStatus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidKycStatusError(pub u8);

impl fmt::Display for InvalidKycStatusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid KYC status value: {}", self.0)
    }
}

impl std::error::Error for InvalidKycStatusError {}

/// Trading limits for a counterparty.
///
/// Defines the maximum trading amounts allowed per trade and per day.
///
/// # Examples
///
/// ```
/// use otc_rfq::domain::entities::counterparty::CounterpartyLimits;
/// use otc_rfq::domain::value_objects::Price;
///
/// let limits = CounterpartyLimits::new(
///     Price::new(100_000.0).unwrap(),  // max per trade
///     Price::new(1_000_000.0).unwrap(), // daily limit
/// );
///
/// assert!(limits.check_trade_amount(Price::new(50_000.0).unwrap()));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CounterpartyLimits {
    /// Maximum amount per single trade.
    max_trade_amount: Price,
    /// Maximum total amount per day.
    daily_limit: Price,
    /// Current daily usage (tracked externally, stored for reference).
    daily_used: Price,
}

impl CounterpartyLimits {
    /// Creates new trading limits.
    ///
    /// # Arguments
    ///
    /// * `max_trade_amount` - Maximum amount per single trade
    /// * `daily_limit` - Maximum total amount per day
    #[must_use]
    pub fn new(max_trade_amount: Price, daily_limit: Price) -> Self {
        Self {
            max_trade_amount,
            daily_limit,
            daily_used: Price::ZERO,
        }
    }

    /// Creates unlimited trading limits.
    ///
    /// Uses a very large value (1 trillion) as the effective unlimited amount.
    #[must_use]
    pub fn unlimited() -> Self {
        // Use a very large but safe value for "unlimited"
        let max_value = Price::new(1_000_000_000_000.0).unwrap_or(Price::ZERO);
        Self {
            max_trade_amount: max_value,
            daily_limit: max_value,
            daily_used: Price::ZERO,
        }
    }

    /// Returns the maximum trade amount.
    #[inline]
    #[must_use]
    pub fn max_trade_amount(&self) -> Price {
        self.max_trade_amount
    }

    /// Returns the daily limit.
    #[inline]
    #[must_use]
    pub fn daily_limit(&self) -> Price {
        self.daily_limit
    }

    /// Returns the current daily usage.
    #[inline]
    #[must_use]
    pub fn daily_used(&self) -> Price {
        self.daily_used
    }

    /// Returns the remaining daily allowance.
    #[must_use]
    pub fn daily_remaining(&self) -> Price {
        self.daily_limit
            .safe_sub(self.daily_used)
            .unwrap_or(Price::ZERO)
    }

    /// Checks if a trade amount is within the per-trade limit.
    #[must_use]
    pub fn check_trade_amount(&self, amount: Price) -> bool {
        amount <= self.max_trade_amount
    }

    /// Checks if a trade amount is within the remaining daily limit.
    #[must_use]
    pub fn check_daily_limit(&self, amount: Price) -> bool {
        amount <= self.daily_remaining()
    }

    /// Checks if a trade amount is within both limits.
    #[must_use]
    pub fn check_limits(&self, amount: Price) -> bool {
        self.check_trade_amount(amount) && self.check_daily_limit(amount)
    }

    /// Records a trade amount against the daily limit.
    pub fn record_trade(&mut self, amount: Price) {
        self.daily_used = self.daily_used.safe_add(amount).unwrap_or(self.daily_limit);
    }

    /// Resets the daily usage (typically called at day boundary).
    pub fn reset_daily(&mut self) {
        self.daily_used = Price::ZERO;
    }
}

impl Default for CounterpartyLimits {
    fn default() -> Self {
        Self::unlimited()
    }
}

/// A wallet address on a specific blockchain.
///
/// Represents a counterparty's wallet address for on-chain settlement.
///
/// # Examples
///
/// ```
/// use otc_rfq::domain::entities::counterparty::WalletAddress;
/// use otc_rfq::domain::value_objects::Blockchain;
///
/// let wallet = WalletAddress::new(
///     Blockchain::Ethereum,
///     "0x742d35Cc6634C0532925a3b844Bc9e7595f1Db38",
/// );
///
/// assert_eq!(wallet.chain(), Blockchain::Ethereum);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WalletAddress {
    /// The blockchain network.
    chain: Blockchain,
    /// The wallet address (hex string for EVM chains).
    address: String,
    /// Optional label for the wallet.
    label: Option<String>,
    /// Whether this is the primary wallet for this chain.
    is_primary: bool,
}

impl WalletAddress {
    /// Creates a new wallet address.
    ///
    /// # Arguments
    ///
    /// * `chain` - The blockchain network
    /// * `address` - The wallet address
    #[must_use]
    pub fn new(chain: Blockchain, address: impl Into<String>) -> Self {
        Self {
            chain,
            address: address.into(),
            label: None,
            is_primary: false,
        }
    }

    /// Creates a primary wallet address.
    #[must_use]
    pub fn primary(chain: Blockchain, address: impl Into<String>) -> Self {
        Self {
            chain,
            address: address.into(),
            label: None,
            is_primary: true,
        }
    }

    /// Creates a wallet address with a label.
    #[must_use]
    pub fn with_label(
        chain: Blockchain,
        address: impl Into<String>,
        label: impl Into<String>,
    ) -> Self {
        Self {
            chain,
            address: address.into(),
            label: Some(label.into()),
            is_primary: false,
        }
    }

    /// Returns the blockchain.
    #[inline]
    #[must_use]
    pub fn chain(&self) -> Blockchain {
        self.chain
    }

    /// Returns the wallet address.
    #[inline]
    #[must_use]
    pub fn address(&self) -> &str {
        &self.address
    }

    /// Returns the label, if any.
    #[inline]
    #[must_use]
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    /// Returns true if this is the primary wallet.
    #[inline]
    #[must_use]
    pub fn is_primary(&self) -> bool {
        self.is_primary
    }

    /// Sets this wallet as primary.
    pub fn set_primary(&mut self, is_primary: bool) {
        self.is_primary = is_primary;
    }

    /// Sets the label.
    pub fn set_label(&mut self, label: impl Into<String>) {
        self.label = Some(label.into());
    }
}

impl fmt::Display for WalletAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(label) = &self.label {
            write!(f, "{} ({}: {})", label, self.chain, self.address)
        } else {
            write!(f, "{}: {}", self.chain, self.address)
        }
    }
}

/// A trading counterparty.
///
/// Represents a client, market maker, or other trading participant
/// with KYC status, trading limits, and wallet addresses.
///
/// # Examples
///
/// ```
/// use otc_rfq::domain::entities::counterparty::{Counterparty, CounterpartyType, KycStatus};
/// use otc_rfq::domain::value_objects::CounterpartyId;
///
/// let mut counterparty = Counterparty::new(
///     CounterpartyId::new("acme-trading"),
///     "Acme Trading",
///     CounterpartyType::Client,
/// );
///
/// // Approve KYC
/// counterparty.set_kyc_status(KycStatus::Approved);
/// assert!(counterparty.can_trade());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Counterparty {
    /// Unique identifier.
    id: CounterpartyId,
    /// Human-readable name.
    name: String,
    /// Type of counterparty.
    counterparty_type: CounterpartyType,
    /// KYC verification status.
    kyc_status: KycStatus,
    /// Trading limits.
    limits: CounterpartyLimits,
    /// Wallet addresses for on-chain settlement.
    wallet_addresses: Vec<WalletAddress>,
    /// Whether the counterparty is active.
    active: bool,
    /// When this counterparty was created.
    created_at: Timestamp,
    /// When this counterparty was last updated.
    updated_at: Timestamp,
}

impl Counterparty {
    /// Creates a new counterparty.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique identifier
    /// * `name` - Human-readable name
    /// * `counterparty_type` - Type of counterparty
    #[must_use]
    pub fn new(
        id: CounterpartyId,
        name: impl Into<String>,
        counterparty_type: CounterpartyType,
    ) -> Self {
        let now = Timestamp::now();
        Self {
            id,
            name: name.into(),
            counterparty_type,
            kyc_status: KycStatus::NotStarted,
            limits: CounterpartyLimits::default(),
            wallet_addresses: Vec::new(),
            active: true,
            created_at: now,
            updated_at: now,
        }
    }

    /// Creates a counterparty with specific configuration (for reconstruction from storage).
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn from_parts(
        id: CounterpartyId,
        name: String,
        counterparty_type: CounterpartyType,
        kyc_status: KycStatus,
        limits: CounterpartyLimits,
        wallet_addresses: Vec<WalletAddress>,
        active: bool,
        created_at: Timestamp,
        updated_at: Timestamp,
    ) -> Self {
        Self {
            id,
            name,
            counterparty_type,
            kyc_status,
            limits,
            wallet_addresses,
            active,
            created_at,
            updated_at,
        }
    }

    // ========== Accessors ==========

    /// Returns the counterparty ID.
    #[inline]
    #[must_use]
    pub fn id(&self) -> &CounterpartyId {
        &self.id
    }

    /// Returns the counterparty name.
    #[inline]
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the counterparty type.
    #[inline]
    #[must_use]
    pub fn counterparty_type(&self) -> CounterpartyType {
        self.counterparty_type
    }

    /// Returns the KYC status.
    #[inline]
    #[must_use]
    pub fn kyc_status(&self) -> KycStatus {
        self.kyc_status
    }

    /// Returns the trading limits.
    #[inline]
    #[must_use]
    pub fn limits(&self) -> &CounterpartyLimits {
        &self.limits
    }

    /// Returns a mutable reference to the trading limits.
    #[inline]
    pub fn limits_mut(&mut self) -> &mut CounterpartyLimits {
        self.updated_at = Timestamp::now();
        &mut self.limits
    }

    /// Returns the wallet addresses.
    #[inline]
    #[must_use]
    pub fn wallet_addresses(&self) -> &[WalletAddress] {
        &self.wallet_addresses
    }

    /// Returns whether the counterparty is active.
    #[inline]
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Returns when this counterparty was created.
    #[inline]
    #[must_use]
    pub fn created_at(&self) -> Timestamp {
        self.created_at
    }

    /// Returns when this counterparty was last updated.
    #[inline]
    #[must_use]
    pub fn updated_at(&self) -> Timestamp {
        self.updated_at
    }

    // ========== State Helpers ==========

    /// Returns true if the counterparty can trade.
    ///
    /// A counterparty can trade if:
    /// - They are active
    /// - KYC is approved (for types that require KYC) or not required
    #[must_use]
    pub fn can_trade(&self) -> bool {
        if !self.active {
            return false;
        }

        if self.counterparty_type.requires_kyc() {
            self.kyc_status.is_approved()
        } else {
            true
        }
    }

    /// Returns true if this is a client.
    #[inline]
    #[must_use]
    pub fn is_client(&self) -> bool {
        self.counterparty_type.is_client()
    }

    /// Returns true if this is a market maker.
    #[inline]
    #[must_use]
    pub fn is_market_maker(&self) -> bool {
        self.counterparty_type.is_market_maker()
    }

    /// Returns the primary wallet for a specific chain.
    #[must_use]
    pub fn primary_wallet(&self, chain: Blockchain) -> Option<&WalletAddress> {
        self.wallet_addresses
            .iter()
            .find(|w| w.chain() == chain && w.is_primary())
            .or_else(|| self.wallet_addresses.iter().find(|w| w.chain() == chain))
    }

    // ========== Mutators ==========

    /// Sets the counterparty name.
    pub fn set_name(&mut self, name: impl Into<String>) {
        self.name = name.into();
        self.updated_at = Timestamp::now();
    }

    /// Sets the KYC status.
    pub fn set_kyc_status(&mut self, status: KycStatus) {
        self.kyc_status = status;
        self.updated_at = Timestamp::now();
    }

    /// Sets whether the counterparty is active.
    pub fn set_active(&mut self, active: bool) {
        self.active = active;
        self.updated_at = Timestamp::now();
    }

    /// Adds a wallet address.
    pub fn add_wallet(&mut self, wallet: WalletAddress) {
        if !self
            .wallet_addresses
            .iter()
            .any(|w| w.chain() == wallet.chain() && w.address() == wallet.address())
        {
            self.wallet_addresses.push(wallet);
            self.updated_at = Timestamp::now();
        }
    }

    /// Removes a wallet address.
    pub fn remove_wallet(&mut self, chain: Blockchain, address: &str) {
        if let Some(pos) = self
            .wallet_addresses
            .iter()
            .position(|w| w.chain() == chain && w.address() == address)
        {
            self.wallet_addresses.remove(pos);
            self.updated_at = Timestamp::now();
        }
    }

    /// Clears all wallet addresses.
    pub fn clear_wallets(&mut self) {
        if !self.wallet_addresses.is_empty() {
            self.wallet_addresses.clear();
            self.updated_at = Timestamp::now();
        }
    }
}

impl fmt::Display for Counterparty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Counterparty({} {} [{}] {})",
            self.id,
            self.name,
            self.counterparty_type,
            if self.can_trade() {
                "tradeable"
            } else {
                "restricted"
            }
        )
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn test_counterparty_id() -> CounterpartyId {
        CounterpartyId::new("test-counterparty")
    }

    fn create_test_counterparty() -> Counterparty {
        Counterparty::new(
            test_counterparty_id(),
            "Test Client",
            CounterpartyType::Client,
        )
    }

    mod counterparty_type {
        use super::*;

        #[test]
        fn client_requires_kyc() {
            assert!(CounterpartyType::Client.requires_kyc());
            assert!(CounterpartyType::Client.is_client());
        }

        #[test]
        fn market_maker_requires_kyc() {
            assert!(CounterpartyType::MarketMaker.requires_kyc());
            assert!(CounterpartyType::MarketMaker.is_market_maker());
        }

        #[test]
        fn dex_does_not_require_kyc() {
            assert!(!CounterpartyType::Dex.requires_kyc());
            assert!(CounterpartyType::Dex.is_dex());
        }

        #[test]
        fn internal_does_not_require_kyc() {
            assert!(!CounterpartyType::Internal.requires_kyc());
            assert!(CounterpartyType::Internal.is_internal());
        }

        #[test]
        fn as_u8() {
            assert_eq!(CounterpartyType::Client.as_u8(), 0);
            assert_eq!(CounterpartyType::MarketMaker.as_u8(), 1);
            assert_eq!(CounterpartyType::Dex.as_u8(), 2);
            assert_eq!(CounterpartyType::Internal.as_u8(), 3);
        }

        #[test]
        fn try_from_u8() {
            assert_eq!(
                CounterpartyType::try_from(0).unwrap(),
                CounterpartyType::Client
            );
            assert_eq!(
                CounterpartyType::try_from(2).unwrap(),
                CounterpartyType::Dex
            );
            assert!(CounterpartyType::try_from(99).is_err());
        }

        #[test]
        fn display() {
            assert_eq!(CounterpartyType::Client.to_string(), "CLIENT");
            assert_eq!(CounterpartyType::MarketMaker.to_string(), "MARKET_MAKER");
            assert_eq!(CounterpartyType::Dex.to_string(), "DEX");
            assert_eq!(CounterpartyType::Internal.to_string(), "INTERNAL");
        }
    }

    mod kyc_status {
        use super::*;

        #[test]
        fn not_started_cannot_trade() {
            assert!(!KycStatus::NotStarted.can_trade());
        }

        #[test]
        fn pending_cannot_trade() {
            assert!(KycStatus::Pending.is_pending());
            assert!(!KycStatus::Pending.can_trade());
        }

        #[test]
        fn approved_can_trade() {
            assert!(KycStatus::Approved.is_approved());
            assert!(KycStatus::Approved.can_trade());
        }

        #[test]
        fn rejected_cannot_trade() {
            assert!(KycStatus::Rejected.is_rejected());
            assert!(!KycStatus::Rejected.can_trade());
        }

        #[test]
        fn as_u8() {
            assert_eq!(KycStatus::NotStarted.as_u8(), 0);
            assert_eq!(KycStatus::Pending.as_u8(), 1);
            assert_eq!(KycStatus::Approved.as_u8(), 2);
            assert_eq!(KycStatus::Rejected.as_u8(), 3);
        }

        #[test]
        fn try_from_u8() {
            assert_eq!(KycStatus::try_from(0).unwrap(), KycStatus::NotStarted);
            assert_eq!(KycStatus::try_from(2).unwrap(), KycStatus::Approved);
            assert!(KycStatus::try_from(99).is_err());
        }

        #[test]
        fn display() {
            assert_eq!(KycStatus::NotStarted.to_string(), "NOT_STARTED");
            assert_eq!(KycStatus::Pending.to_string(), "PENDING");
            assert_eq!(KycStatus::Approved.to_string(), "APPROVED");
            assert_eq!(KycStatus::Rejected.to_string(), "REJECTED");
        }
    }

    mod counterparty_limits {
        use super::*;

        #[test]
        fn new_creates_limits() {
            let limits = CounterpartyLimits::new(
                Price::new(100_000.0).unwrap(),
                Price::new(1_000_000.0).unwrap(),
            );

            assert_eq!(limits.max_trade_amount(), Price::new(100_000.0).unwrap());
            assert_eq!(limits.daily_limit(), Price::new(1_000_000.0).unwrap());
            assert_eq!(limits.daily_used(), Price::ZERO);
        }

        #[test]
        fn check_trade_amount() {
            let limits = CounterpartyLimits::new(
                Price::new(100_000.0).unwrap(),
                Price::new(1_000_000.0).unwrap(),
            );

            assert!(limits.check_trade_amount(Price::new(50_000.0).unwrap()));
            assert!(limits.check_trade_amount(Price::new(100_000.0).unwrap()));
            assert!(!limits.check_trade_amount(Price::new(150_000.0).unwrap()));
        }

        #[test]
        fn check_daily_limit() {
            let mut limits = CounterpartyLimits::new(
                Price::new(100_000.0).unwrap(),
                Price::new(200_000.0).unwrap(),
            );

            assert!(limits.check_daily_limit(Price::new(200_000.0).unwrap()));

            limits.record_trade(Price::new(150_000.0).unwrap());

            assert!(limits.check_daily_limit(Price::new(50_000.0).unwrap()));
            assert!(!limits.check_daily_limit(Price::new(100_000.0).unwrap()));
        }

        #[test]
        fn daily_remaining() {
            let mut limits = CounterpartyLimits::new(
                Price::new(100_000.0).unwrap(),
                Price::new(200_000.0).unwrap(),
            );

            assert_eq!(limits.daily_remaining(), Price::new(200_000.0).unwrap());

            limits.record_trade(Price::new(75_000.0).unwrap());

            assert_eq!(limits.daily_remaining(), Price::new(125_000.0).unwrap());
        }

        #[test]
        fn reset_daily() {
            let mut limits = CounterpartyLimits::new(
                Price::new(100_000.0).unwrap(),
                Price::new(200_000.0).unwrap(),
            );

            limits.record_trade(Price::new(100_000.0).unwrap());
            limits.reset_daily();

            assert_eq!(limits.daily_used(), Price::ZERO);
        }

        #[test]
        fn unlimited() {
            let limits = CounterpartyLimits::unlimited();

            assert!(limits.check_trade_amount(Price::new(1_000_000_000.0).unwrap()));
            assert!(limits.check_daily_limit(Price::new(1_000_000_000.0).unwrap()));
        }
    }

    mod wallet_address {
        use super::*;

        #[test]
        fn new_creates_wallet() {
            let wallet = WalletAddress::new(
                Blockchain::Ethereum,
                "0x742d35Cc6634C0532925a3b844Bc9e7595f1Db38",
            );

            assert_eq!(wallet.chain(), Blockchain::Ethereum);
            assert_eq!(
                wallet.address(),
                "0x742d35Cc6634C0532925a3b844Bc9e7595f1Db38"
            );
            assert!(!wallet.is_primary());
            assert!(wallet.label().is_none());
        }

        #[test]
        fn primary_creates_primary_wallet() {
            let wallet = WalletAddress::primary(Blockchain::Polygon, "0xabc123");

            assert!(wallet.is_primary());
        }

        #[test]
        fn with_label_creates_labeled_wallet() {
            let wallet =
                WalletAddress::with_label(Blockchain::Arbitrum, "0xdef456", "Trading Wallet");

            assert_eq!(wallet.label(), Some("Trading Wallet"));
        }

        #[test]
        fn display_with_label() {
            let wallet = WalletAddress::with_label(Blockchain::Ethereum, "0xabc", "Main");

            assert!(wallet.to_string().contains("Main"));
        }

        #[test]
        fn display_without_label() {
            let wallet = WalletAddress::new(Blockchain::Ethereum, "0xabc");

            assert!(wallet.to_string().contains("ETHEREUM"));
            assert!(wallet.to_string().contains("0xabc"));
        }
    }

    mod counterparty_construction {
        use super::*;

        #[test]
        fn new_creates_active_counterparty() {
            let cp = create_test_counterparty();

            assert!(cp.is_active());
            assert_eq!(cp.kyc_status(), KycStatus::NotStarted);
            assert!(cp.wallet_addresses().is_empty());
        }

        #[test]
        fn client_cannot_trade_without_kyc() {
            let cp = create_test_counterparty();

            assert!(!cp.can_trade());
        }

        #[test]
        fn client_can_trade_with_approved_kyc() {
            let mut cp = create_test_counterparty();
            cp.set_kyc_status(KycStatus::Approved);

            assert!(cp.can_trade());
        }

        #[test]
        fn dex_can_trade_without_kyc() {
            let cp = Counterparty::new(test_counterparty_id(), "Uniswap", CounterpartyType::Dex);

            assert!(cp.can_trade());
        }

        #[test]
        fn internal_can_trade_without_kyc() {
            let cp = Counterparty::new(
                test_counterparty_id(),
                "Treasury",
                CounterpartyType::Internal,
            );

            assert!(cp.can_trade());
        }

        #[test]
        fn inactive_cannot_trade() {
            let mut cp = create_test_counterparty();
            cp.set_kyc_status(KycStatus::Approved);
            cp.set_active(false);

            assert!(!cp.can_trade());
        }
    }

    mod counterparty_wallets {
        use super::*;

        #[test]
        fn add_wallet() {
            let mut cp = create_test_counterparty();
            let wallet = WalletAddress::new(Blockchain::Ethereum, "0xabc");

            cp.add_wallet(wallet);

            assert_eq!(cp.wallet_addresses().len(), 1);
        }

        #[test]
        fn add_duplicate_wallet_is_noop() {
            let mut cp = create_test_counterparty();
            let wallet1 = WalletAddress::new(Blockchain::Ethereum, "0xabc");
            let wallet2 = WalletAddress::new(Blockchain::Ethereum, "0xabc");

            cp.add_wallet(wallet1);
            cp.add_wallet(wallet2);

            assert_eq!(cp.wallet_addresses().len(), 1);
        }

        #[test]
        fn remove_wallet() {
            let mut cp = create_test_counterparty();
            cp.add_wallet(WalletAddress::new(Blockchain::Ethereum, "0xabc"));

            cp.remove_wallet(Blockchain::Ethereum, "0xabc");

            assert!(cp.wallet_addresses().is_empty());
        }

        #[test]
        fn primary_wallet() {
            let mut cp = create_test_counterparty();
            cp.add_wallet(WalletAddress::new(Blockchain::Ethereum, "0xsecondary"));
            cp.add_wallet(WalletAddress::primary(Blockchain::Ethereum, "0xprimary"));

            let primary = cp.primary_wallet(Blockchain::Ethereum).unwrap();
            assert_eq!(primary.address(), "0xprimary");
        }

        #[test]
        fn primary_wallet_fallback() {
            let mut cp = create_test_counterparty();
            cp.add_wallet(WalletAddress::new(Blockchain::Ethereum, "0xonly"));

            let wallet = cp.primary_wallet(Blockchain::Ethereum).unwrap();
            assert_eq!(wallet.address(), "0xonly");
        }

        #[test]
        fn clear_wallets() {
            let mut cp = create_test_counterparty();
            cp.add_wallet(WalletAddress::new(Blockchain::Ethereum, "0xabc"));
            cp.add_wallet(WalletAddress::new(Blockchain::Polygon, "0xdef"));

            cp.clear_wallets();

            assert!(cp.wallet_addresses().is_empty());
        }
    }

    mod display {
        use super::*;

        #[test]
        fn display_tradeable() {
            let mut cp = Counterparty::new(test_counterparty_id(), "DEX", CounterpartyType::Dex);
            cp.set_active(true);

            let display = cp.to_string();
            assert!(display.contains("tradeable"));
        }

        #[test]
        fn display_restricted() {
            let cp = create_test_counterparty();
            let display = cp.to_string();

            assert!(display.contains("restricted"));
        }
    }

    mod serde {
        use super::*;

        #[test]
        fn counterparty_serde_roundtrip() {
            let mut cp = create_test_counterparty();
            cp.set_kyc_status(KycStatus::Approved);
            cp.add_wallet(WalletAddress::primary(Blockchain::Ethereum, "0xabc"));

            let json = serde_json::to_string(&cp).unwrap();
            let deserialized: Counterparty = serde_json::from_str(&json).unwrap();

            assert_eq!(cp.id(), deserialized.id());
            assert_eq!(cp.name(), deserialized.name());
            assert_eq!(cp.kyc_status(), deserialized.kyc_status());
            assert_eq!(
                cp.wallet_addresses().len(),
                deserialized.wallet_addresses().len()
            );
        }

        #[test]
        fn counterparty_type_serde_roundtrip() {
            for ct in [
                CounterpartyType::Client,
                CounterpartyType::MarketMaker,
                CounterpartyType::Dex,
                CounterpartyType::Internal,
            ] {
                let json = serde_json::to_string(&ct).unwrap();
                let deserialized: CounterpartyType = serde_json::from_str(&json).unwrap();
                assert_eq!(ct, deserialized);
            }
        }

        #[test]
        fn kyc_status_serde_roundtrip() {
            for status in [
                KycStatus::NotStarted,
                KycStatus::Pending,
                KycStatus::Approved,
                KycStatus::Rejected,
            ] {
                let json = serde_json::to_string(&status).unwrap();
                let deserialized: KycStatus = serde_json::from_str(&json).unwrap();
                assert_eq!(status, deserialized);
            }
        }

        #[test]
        fn counterparty_type_serde_screaming_snake_case() {
            let json = serde_json::to_string(&CounterpartyType::MarketMaker).unwrap();
            assert_eq!(json, "\"MARKET_MAKER\"");
        }
    }
}
