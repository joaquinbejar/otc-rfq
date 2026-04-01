//! # Domain Enums
//!
//! Enumeration types for domain concepts.
//!
//! This module provides core domain enumerations used throughout the OTC RFQ system:
//!
//! - [`OrderSide`] - Buy or Sell direction
//! - [`AssetClass`] - Asset classification (crypto, stocks, forex, etc.)
//! - [`Blockchain`] - Supported blockchain networks
//! - [`VenueType`] - Types of liquidity venues
//! - [`SettlementMethod`] - On-chain or off-chain settlement
//!
//! All enums implement `Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`, `Hash`,
//! `Display`, `FromStr`, and Serde traits.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Order side indicating buy or sell direction.
///
/// Uses `#[repr(u8)]` for compact binary representation.
///
/// # Examples
///
/// ```
/// use otc_rfq::domain::value_objects::enums::OrderSide;
///
/// let buy = OrderSide::Buy;
/// let sell = OrderSide::Sell;
///
/// assert_eq!(buy.opposite(), sell);
/// assert_eq!(buy.to_string(), "BUY");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
#[repr(u8)]
pub enum OrderSide {
    /// Buy order - acquiring the asset.
    Buy = 0,
    /// Sell order - disposing of the asset.
    Sell = 1,
}

impl OrderSide {
    /// Returns the opposite side.
    ///
    /// # Examples
    ///
    /// ```
    /// use otc_rfq::domain::value_objects::enums::OrderSide;
    ///
    /// assert_eq!(OrderSide::Buy.opposite(), OrderSide::Sell);
    /// assert_eq!(OrderSide::Sell.opposite(), OrderSide::Buy);
    /// ```
    #[inline]
    #[must_use]
    pub const fn opposite(self) -> Self {
        match self {
            Self::Buy => Self::Sell,
            Self::Sell => Self::Buy,
        }
    }

    /// Returns true if this is a buy order.
    #[inline]
    #[must_use]
    pub const fn is_buy(self) -> bool {
        matches!(self, Self::Buy)
    }

    /// Returns true if this is a sell order.
    #[inline]
    #[must_use]
    pub const fn is_sell(self) -> bool {
        matches!(self, Self::Sell)
    }
}

impl fmt::Display for OrderSide {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Buy => write!(f, "BUY"),
            Self::Sell => write!(f, "SELL"),
        }
    }
}

impl FromStr for OrderSide {
    type Err = ParseEnumError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "BUY" => Ok(Self::Buy),
            "SELL" => Ok(Self::Sell),
            _ => Err(ParseEnumError::InvalidValue("OrderSide", s.to_string())),
        }
    }
}

/// Asset class classification.
///
/// Categorizes financial instruments by their underlying asset type.
///
/// # Examples
///
/// ```
/// use otc_rfq::domain::value_objects::enums::AssetClass;
///
/// let crypto = AssetClass::CryptoSpot;
/// assert!(crypto.is_crypto());
/// assert_eq!(crypto.to_string(), "CRYPTO_SPOT");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[repr(u8)]
pub enum AssetClass {
    /// Cryptocurrency spot trading.
    CryptoSpot = 0,
    /// Cryptocurrency derivatives (futures, options, perpetuals).
    CryptoDerivs = 1,
    /// Equity/stock instruments.
    Stock = 2,
    /// Foreign exchange (currency pairs).
    Forex = 3,
    /// Commodity instruments.
    Commodity = 4,
}

impl AssetClass {
    /// Returns true if this is a cryptocurrency asset class.
    #[inline]
    #[must_use]
    pub const fn is_crypto(self) -> bool {
        matches!(self, Self::CryptoSpot | Self::CryptoDerivs)
    }

    /// Returns true if this is a traditional finance asset class.
    #[inline]
    #[must_use]
    pub const fn is_tradfi(self) -> bool {
        matches!(self, Self::Stock | Self::Forex | Self::Commodity)
    }
}

impl fmt::Display for AssetClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CryptoSpot => write!(f, "CRYPTO_SPOT"),
            Self::CryptoDerivs => write!(f, "CRYPTO_DERIVS"),
            Self::Stock => write!(f, "STOCK"),
            Self::Forex => write!(f, "FOREX"),
            Self::Commodity => write!(f, "COMMODITY"),
        }
    }
}

impl FromStr for AssetClass {
    type Err = ParseEnumError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().replace('-', "_").as_str() {
            "CRYPTO_SPOT" | "CRYPTOSPOT" => Ok(Self::CryptoSpot),
            "CRYPTO_DERIVS" | "CRYPTODERIVS" => Ok(Self::CryptoDerivs),
            "STOCK" => Ok(Self::Stock),
            "FOREX" => Ok(Self::Forex),
            "COMMODITY" => Ok(Self::Commodity),
            _ => Err(ParseEnumError::InvalidValue("AssetClass", s.to_string())),
        }
    }
}

/// Supported blockchain networks.
///
/// Represents the blockchain networks where on-chain settlement can occur.
///
/// # Examples
///
/// ```
/// use otc_rfq::domain::value_objects::enums::Blockchain;
///
/// let eth = Blockchain::Ethereum;
/// assert_eq!(eth.chain_id(), 1);
/// assert_eq!(eth.to_string(), "ETHEREUM");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[repr(u8)]
pub enum Blockchain {
    /// Ethereum mainnet (chain ID: 1).
    Ethereum = 0,
    /// Polygon PoS (chain ID: 137).
    Polygon = 1,
    /// Arbitrum One (chain ID: 42161).
    Arbitrum = 2,
    /// Optimism (chain ID: 10).
    Optimism = 3,
    /// Base (chain ID: 8453).
    Base = 4,
}

impl Blockchain {
    /// Returns the EVM chain ID for this blockchain.
    ///
    /// # Examples
    ///
    /// ```
    /// use otc_rfq::domain::value_objects::enums::Blockchain;
    ///
    /// assert_eq!(Blockchain::Ethereum.chain_id(), 1);
    /// assert_eq!(Blockchain::Polygon.chain_id(), 137);
    /// assert_eq!(Blockchain::Arbitrum.chain_id(), 42161);
    /// ```
    #[inline]
    #[must_use]
    pub const fn chain_id(self) -> u64 {
        match self {
            Self::Ethereum => 1,
            Self::Polygon => 137,
            Self::Arbitrum => 42161,
            Self::Optimism => 10,
            Self::Base => 8453,
        }
    }

    /// Returns true if this is a Layer 2 network.
    #[inline]
    #[must_use]
    pub const fn is_layer2(self) -> bool {
        matches!(
            self,
            Self::Polygon | Self::Arbitrum | Self::Optimism | Self::Base
        )
    }

    /// Creates a blockchain from a chain ID.
    ///
    /// # Returns
    ///
    /// `Some(Blockchain)` if the chain ID is recognized, `None` otherwise.
    #[must_use]
    pub const fn from_chain_id(chain_id: u64) -> Option<Self> {
        match chain_id {
            1 => Some(Self::Ethereum),
            137 => Some(Self::Polygon),
            42161 => Some(Self::Arbitrum),
            10 => Some(Self::Optimism),
            8453 => Some(Self::Base),
            _ => None,
        }
    }

    /// Returns the SBE wire encoding tag for this blockchain.
    ///
    /// # Examples
    ///
    /// ```
    /// use otc_rfq::domain::value_objects::enums::Blockchain;
    ///
    /// assert_eq!(Blockchain::Ethereum.as_u8(), 0);
    /// assert_eq!(Blockchain::Polygon.as_u8(), 1);
    /// ```
    #[inline]
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    /// Creates a blockchain from its SBE wire encoding tag.
    ///
    /// # Returns
    ///
    /// `Some(Blockchain)` if the tag is recognized, `None` otherwise.
    ///
    /// # Examples
    ///
    /// ```
    /// use otc_rfq::domain::value_objects::enums::Blockchain;
    ///
    /// assert_eq!(Blockchain::from_u8(0), Some(Blockchain::Ethereum));
    /// assert_eq!(Blockchain::from_u8(99), None);
    /// ```
    #[must_use]
    pub const fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Ethereum),
            1 => Some(Self::Polygon),
            2 => Some(Self::Arbitrum),
            3 => Some(Self::Optimism),
            4 => Some(Self::Base),
            _ => None,
        }
    }
}

impl fmt::Display for Blockchain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ethereum => write!(f, "ETHEREUM"),
            Self::Polygon => write!(f, "POLYGON"),
            Self::Arbitrum => write!(f, "ARBITRUM"),
            Self::Optimism => write!(f, "OPTIMISM"),
            Self::Base => write!(f, "BASE"),
        }
    }
}

impl FromStr for Blockchain {
    type Err = ParseEnumError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "ETHEREUM" | "ETH" => Ok(Self::Ethereum),
            "POLYGON" | "MATIC" => Ok(Self::Polygon),
            "ARBITRUM" | "ARB" => Ok(Self::Arbitrum),
            "OPTIMISM" | "OP" => Ok(Self::Optimism),
            "BASE" => Ok(Self::Base),
            _ => Err(ParseEnumError::InvalidValue("Blockchain", s.to_string())),
        }
    }
}

/// Types of liquidity venues.
///
/// Categorizes the different sources of liquidity in the OTC RFQ system.
///
/// # Examples
///
/// ```
/// use otc_rfq::domain::value_objects::enums::VenueType;
///
/// let venue = VenueType::InternalMM;
/// assert!(venue.is_market_maker());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[repr(u8)]
pub enum VenueType {
    /// Internal market maker (proprietary liquidity).
    InternalMM = 0,
    /// External market maker (third-party liquidity provider).
    ExternalMM = 1,
    /// DEX aggregator (0x, 1inch, Paraswap).
    DexAggregator = 2,
    /// Direct protocol integration (Uniswap, Curve).
    Protocol = 3,
    /// RFQ-specific protocol (Hashflow, Bebop).
    RfqProtocol = 4,
}

impl VenueType {
    /// Returns true if this is a market maker venue.
    #[inline]
    #[must_use]
    pub const fn is_market_maker(self) -> bool {
        matches!(self, Self::InternalMM | Self::ExternalMM)
    }

    /// Returns true if this is a DeFi venue.
    #[inline]
    #[must_use]
    pub const fn is_defi(self) -> bool {
        matches!(
            self,
            Self::DexAggregator | Self::Protocol | Self::RfqProtocol
        )
    }
}

impl fmt::Display for VenueType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InternalMM => write!(f, "INTERNAL_MM"),
            Self::ExternalMM => write!(f, "EXTERNAL_MM"),
            Self::DexAggregator => write!(f, "DEX_AGGREGATOR"),
            Self::Protocol => write!(f, "PROTOCOL"),
            Self::RfqProtocol => write!(f, "RFQ_PROTOCOL"),
        }
    }
}

impl FromStr for VenueType {
    type Err = ParseEnumError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().replace('-', "_").as_str() {
            "INTERNAL_MM" | "INTERNALMM" => Ok(Self::InternalMM),
            "EXTERNAL_MM" | "EXTERNALMM" => Ok(Self::ExternalMM),
            "DEX_AGGREGATOR" | "DEXAGGREGATOR" => Ok(Self::DexAggregator),
            "PROTOCOL" => Ok(Self::Protocol),
            "RFQ_PROTOCOL" | "RFQPROTOCOL" => Ok(Self::RfqProtocol),
            _ => Err(ParseEnumError::InvalidValue("VenueType", s.to_string())),
        }
    }
}

/// Settlement method for trades.
///
/// Specifies how a trade will be settled.
///
/// # Examples
///
/// ```
/// use otc_rfq::domain::value_objects::enums::{SettlementMethod, Blockchain};
///
/// let onchain = SettlementMethod::OnChain(Blockchain::Ethereum);
/// assert!(onchain.is_onchain());
///
/// let offchain = SettlementMethod::OffChain;
/// assert!(!offchain.is_onchain());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SettlementMethod {
    /// On-chain settlement on a specific blockchain.
    OnChain(Blockchain),
    /// Off-chain settlement (traditional finance).
    OffChain,
}

impl SettlementMethod {
    /// Returns true if this is on-chain settlement.
    #[inline]
    #[must_use]
    pub const fn is_onchain(self) -> bool {
        matches!(self, Self::OnChain(_))
    }

    /// Returns the blockchain if this is on-chain settlement.
    #[inline]
    #[must_use]
    pub const fn blockchain(self) -> Option<Blockchain> {
        match self {
            Self::OnChain(chain) => Some(chain),
            Self::OffChain => None,
        }
    }
}

impl fmt::Display for SettlementMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OnChain(chain) => write!(f, "ON_CHAIN({})", chain),
            Self::OffChain => write!(f, "OFF_CHAIN"),
        }
    }
}

impl Default for SettlementMethod {
    fn default() -> Self {
        Self::OnChain(Blockchain::Ethereum)
    }
}

/// Error type for parsing enum values from strings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseEnumError {
    /// The provided string value is not valid for the enum.
    InvalidValue(&'static str, String),
}

impl fmt::Display for ParseEnumError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidValue(enum_name, value) => {
                write!(f, "invalid {} value: '{}'", enum_name, value)
            }
        }
    }
}

impl std::error::Error for ParseEnumError {}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    mod order_side {
        use super::*;

        #[test]
        fn opposite_works() {
            assert_eq!(OrderSide::Buy.opposite(), OrderSide::Sell);
            assert_eq!(OrderSide::Sell.opposite(), OrderSide::Buy);
        }

        #[test]
        fn is_buy_sell() {
            assert!(OrderSide::Buy.is_buy());
            assert!(!OrderSide::Buy.is_sell());
            assert!(OrderSide::Sell.is_sell());
            assert!(!OrderSide::Sell.is_buy());
        }

        #[test]
        fn display_uppercase() {
            assert_eq!(OrderSide::Buy.to_string(), "BUY");
            assert_eq!(OrderSide::Sell.to_string(), "SELL");
        }

        #[test]
        fn from_str_works() {
            assert_eq!("BUY".parse::<OrderSide>().unwrap(), OrderSide::Buy);
            assert_eq!("buy".parse::<OrderSide>().unwrap(), OrderSide::Buy);
            assert_eq!("SELL".parse::<OrderSide>().unwrap(), OrderSide::Sell);
            assert_eq!("sell".parse::<OrderSide>().unwrap(), OrderSide::Sell);
        }

        #[test]
        fn from_str_invalid() {
            assert!("HOLD".parse::<OrderSide>().is_err());
        }

        #[test]
        fn serde_roundtrip() {
            let buy = OrderSide::Buy;
            let json = serde_json::to_string(&buy).unwrap();
            assert_eq!(json, "\"BUY\"");
            let deserialized: OrderSide = serde_json::from_str(&json).unwrap();
            assert_eq!(buy, deserialized);
        }

        #[test]
        fn repr_values() {
            assert_eq!(OrderSide::Buy as u8, 0);
            assert_eq!(OrderSide::Sell as u8, 1);
        }
    }

    mod asset_class {
        use super::*;

        #[test]
        fn is_crypto() {
            assert!(AssetClass::CryptoSpot.is_crypto());
            assert!(AssetClass::CryptoDerivs.is_crypto());
            assert!(!AssetClass::Stock.is_crypto());
        }

        #[test]
        fn is_tradfi() {
            assert!(AssetClass::Stock.is_tradfi());
            assert!(AssetClass::Forex.is_tradfi());
            assert!(AssetClass::Commodity.is_tradfi());
            assert!(!AssetClass::CryptoSpot.is_tradfi());
        }

        #[test]
        fn display_screaming_snake() {
            assert_eq!(AssetClass::CryptoSpot.to_string(), "CRYPTO_SPOT");
            assert_eq!(AssetClass::CryptoDerivs.to_string(), "CRYPTO_DERIVS");
        }

        #[test]
        fn from_str_works() {
            assert_eq!(
                "CRYPTO_SPOT".parse::<AssetClass>().unwrap(),
                AssetClass::CryptoSpot
            );
            assert_eq!("stock".parse::<AssetClass>().unwrap(), AssetClass::Stock);
        }

        #[test]
        fn serde_roundtrip() {
            let asset = AssetClass::CryptoDerivs;
            let json = serde_json::to_string(&asset).unwrap();
            assert_eq!(json, "\"CRYPTO_DERIVS\"");
            let deserialized: AssetClass = serde_json::from_str(&json).unwrap();
            assert_eq!(asset, deserialized);
        }
    }

    mod blockchain {
        use super::*;

        #[test]
        fn chain_ids() {
            assert_eq!(Blockchain::Ethereum.chain_id(), 1);
            assert_eq!(Blockchain::Polygon.chain_id(), 137);
            assert_eq!(Blockchain::Arbitrum.chain_id(), 42161);
            assert_eq!(Blockchain::Optimism.chain_id(), 10);
            assert_eq!(Blockchain::Base.chain_id(), 8453);
        }

        #[test]
        fn is_layer2() {
            assert!(!Blockchain::Ethereum.is_layer2());
            assert!(Blockchain::Polygon.is_layer2());
            assert!(Blockchain::Arbitrum.is_layer2());
            assert!(Blockchain::Optimism.is_layer2());
            assert!(Blockchain::Base.is_layer2());
        }

        #[test]
        fn from_chain_id() {
            assert_eq!(Blockchain::from_chain_id(1), Some(Blockchain::Ethereum));
            assert_eq!(Blockchain::from_chain_id(137), Some(Blockchain::Polygon));
            assert_eq!(Blockchain::from_chain_id(999), None);
        }

        #[test]
        fn from_str_aliases() {
            assert_eq!("ETH".parse::<Blockchain>().unwrap(), Blockchain::Ethereum);
            assert_eq!("MATIC".parse::<Blockchain>().unwrap(), Blockchain::Polygon);
            assert_eq!("ARB".parse::<Blockchain>().unwrap(), Blockchain::Arbitrum);
            assert_eq!("OP".parse::<Blockchain>().unwrap(), Blockchain::Optimism);
        }

        #[test]
        fn serde_roundtrip() {
            let chain = Blockchain::Arbitrum;
            let json = serde_json::to_string(&chain).unwrap();
            assert_eq!(json, "\"ARBITRUM\"");
            let deserialized: Blockchain = serde_json::from_str(&json).unwrap();
            assert_eq!(chain, deserialized);
        }
    }

    mod venue_type {
        use super::*;

        #[test]
        fn is_market_maker() {
            assert!(VenueType::InternalMM.is_market_maker());
            assert!(VenueType::ExternalMM.is_market_maker());
            assert!(!VenueType::DexAggregator.is_market_maker());
        }

        #[test]
        fn is_defi() {
            assert!(VenueType::DexAggregator.is_defi());
            assert!(VenueType::Protocol.is_defi());
            assert!(VenueType::RfqProtocol.is_defi());
            assert!(!VenueType::InternalMM.is_defi());
        }

        #[test]
        fn display_screaming_snake() {
            assert_eq!(VenueType::InternalMM.to_string(), "INTERNAL_MM");
            assert_eq!(VenueType::DexAggregator.to_string(), "DEX_AGGREGATOR");
        }

        #[test]
        fn serde_roundtrip() {
            let venue = VenueType::RfqProtocol;
            let json = serde_json::to_string(&venue).unwrap();
            assert_eq!(json, "\"RFQ_PROTOCOL\"");
            let deserialized: VenueType = serde_json::from_str(&json).unwrap();
            assert_eq!(venue, deserialized);
        }
    }

    mod settlement_method {
        use super::*;

        #[test]
        fn is_onchain() {
            let onchain = SettlementMethod::OnChain(Blockchain::Ethereum);
            let offchain = SettlementMethod::OffChain;

            assert!(onchain.is_onchain());
            assert!(!offchain.is_onchain());
        }

        #[test]
        fn blockchain_accessor() {
            let onchain = SettlementMethod::OnChain(Blockchain::Polygon);
            let offchain = SettlementMethod::OffChain;

            assert_eq!(onchain.blockchain(), Some(Blockchain::Polygon));
            assert_eq!(offchain.blockchain(), None);
        }

        #[test]
        fn display_format() {
            let onchain = SettlementMethod::OnChain(Blockchain::Ethereum);
            let offchain = SettlementMethod::OffChain;

            assert_eq!(onchain.to_string(), "ON_CHAIN(ETHEREUM)");
            assert_eq!(offchain.to_string(), "OFF_CHAIN");
        }

        #[test]
        fn default_is_ethereum() {
            let default = SettlementMethod::default();
            assert_eq!(default, SettlementMethod::OnChain(Blockchain::Ethereum));
        }

        #[test]
        fn serde_roundtrip() {
            let onchain = SettlementMethod::OnChain(Blockchain::Base);
            let json = serde_json::to_string(&onchain).unwrap();
            let deserialized: SettlementMethod = serde_json::from_str(&json).unwrap();
            assert_eq!(onchain, deserialized);

            let offchain = SettlementMethod::OffChain;
            let json = serde_json::to_string(&offchain).unwrap();
            assert_eq!(json, "\"OFF_CHAIN\"");
        }
    }

    mod parse_enum_error {
        use super::*;

        #[test]
        fn display_format() {
            let err = ParseEnumError::InvalidValue("OrderSide", "HOLD".to_string());
            assert_eq!(err.to_string(), "invalid OrderSide value: 'HOLD'");
        }
    }
}
