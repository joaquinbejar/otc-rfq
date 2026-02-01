//! # Instrument Value Object
//!
//! Trading instrument with asset class and settlement method.
//!
//! This module provides the [`Instrument`] type for representing tradeable
//! instruments with their associated metadata.
//!
//! # Examples
//!
//! ```
//! use otc_rfq::domain::value_objects::instrument::Instrument;
//! use otc_rfq::domain::value_objects::symbol::Symbol;
//! use otc_rfq::domain::value_objects::enums::{AssetClass, SettlementMethod, Blockchain};
//!
//! let symbol = Symbol::new("BTC/USD").unwrap();
//! let instrument = Instrument::new(
//!     symbol,
//!     AssetClass::CryptoSpot,
//!     SettlementMethod::OnChain(Blockchain::Ethereum),
//! );
//!
//! assert_eq!(instrument.symbol().to_string(), "BTC/USD");
//! assert!(instrument.asset_class().is_crypto());
//! ```

use super::enums::{AssetClass, SettlementMethod};
use super::quantity::Quantity;
use super::symbol::Symbol;
use serde::{Deserialize, Serialize};
use std::fmt;

/// A tradeable instrument with associated metadata.
///
/// Combines a trading symbol with asset classification, settlement method,
/// and optional trading constraints.
///
/// # Examples
///
/// ```
/// use otc_rfq::domain::value_objects::instrument::Instrument;
/// use otc_rfq::domain::value_objects::symbol::Symbol;
/// use otc_rfq::domain::value_objects::enums::{AssetClass, SettlementMethod, Blockchain};
/// use otc_rfq::domain::value_objects::quantity::Quantity;
///
/// let symbol = Symbol::new("ETH/USDC").unwrap();
/// let instrument = Instrument::builder(symbol, AssetClass::CryptoSpot)
///     .settlement_method(SettlementMethod::OnChain(Blockchain::Ethereum))
///     .min_quantity(Quantity::new(0.01).unwrap())
///     .price_decimals(2)
///     .build();
///
/// assert_eq!(instrument.price_decimals(), Some(2));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Instrument {
    /// The trading symbol (e.g., BTC/USD).
    symbol: Symbol,
    /// The asset class classification.
    asset_class: AssetClass,
    /// The settlement method for trades.
    settlement_method: SettlementMethod,
    /// Minimum tradeable quantity (optional).
    min_quantity: Option<Quantity>,
    /// Number of decimal places for price (optional).
    price_decimals: Option<u8>,
}

impl Instrument {
    /// Creates a new instrument with required fields.
    ///
    /// Use [`Instrument::builder`] for more control over optional fields.
    ///
    /// # Arguments
    ///
    /// * `symbol` - The trading symbol
    /// * `asset_class` - The asset class classification
    /// * `settlement_method` - The settlement method
    ///
    /// # Examples
    ///
    /// ```
    /// use otc_rfq::domain::value_objects::instrument::Instrument;
    /// use otc_rfq::domain::value_objects::symbol::Symbol;
    /// use otc_rfq::domain::value_objects::enums::{AssetClass, SettlementMethod};
    ///
    /// let symbol = Symbol::new("BTC/USD").unwrap();
    /// let instrument = Instrument::new(
    ///     symbol,
    ///     AssetClass::CryptoSpot,
    ///     SettlementMethod::default(),
    /// );
    /// ```
    #[must_use]
    pub fn new(
        symbol: Symbol,
        asset_class: AssetClass,
        settlement_method: SettlementMethod,
    ) -> Self {
        Self {
            symbol,
            asset_class,
            settlement_method,
            min_quantity: None,
            price_decimals: None,
        }
    }

    /// Creates a builder for constructing an instrument with optional fields.
    ///
    /// # Arguments
    ///
    /// * `symbol` - The trading symbol
    /// * `asset_class` - The asset class classification
    ///
    /// # Examples
    ///
    /// ```
    /// use otc_rfq::domain::value_objects::instrument::Instrument;
    /// use otc_rfq::domain::value_objects::symbol::Symbol;
    /// use otc_rfq::domain::value_objects::enums::{AssetClass, SettlementMethod, Blockchain};
    ///
    /// let symbol = Symbol::new("ETH/USDC").unwrap();
    /// let instrument = Instrument::builder(symbol, AssetClass::CryptoSpot)
    ///     .settlement_method(SettlementMethod::OnChain(Blockchain::Polygon))
    ///     .price_decimals(6)
    ///     .build();
    /// ```
    #[must_use]
    pub fn builder(symbol: Symbol, asset_class: AssetClass) -> InstrumentBuilder {
        InstrumentBuilder::new(symbol, asset_class)
    }

    /// Returns the trading symbol.
    #[inline]
    #[must_use]
    pub fn symbol(&self) -> &Symbol {
        &self.symbol
    }

    /// Returns the base asset of the trading pair.
    #[inline]
    #[must_use]
    pub fn base_asset(&self) -> &str {
        self.symbol.base_asset()
    }

    /// Returns the quote asset of the trading pair.
    #[inline]
    #[must_use]
    pub fn quote_asset(&self) -> &str {
        self.symbol.quote_asset()
    }

    /// Returns the asset class.
    #[inline]
    #[must_use]
    pub fn asset_class(&self) -> AssetClass {
        self.asset_class
    }

    /// Returns the settlement method.
    #[inline]
    #[must_use]
    pub fn settlement_method(&self) -> SettlementMethod {
        self.settlement_method
    }

    /// Returns the minimum tradeable quantity, if set.
    #[inline]
    #[must_use]
    pub fn min_quantity(&self) -> Option<Quantity> {
        self.min_quantity
    }

    /// Returns the number of price decimal places, if set.
    #[inline]
    #[must_use]
    pub fn price_decimals(&self) -> Option<u8> {
        self.price_decimals
    }

    /// Returns true if this is a cryptocurrency instrument.
    #[inline]
    #[must_use]
    pub fn is_crypto(&self) -> bool {
        self.asset_class.is_crypto()
    }

    /// Returns true if this instrument settles on-chain.
    #[inline]
    #[must_use]
    pub fn is_onchain(&self) -> bool {
        self.settlement_method.is_onchain()
    }
}

impl fmt::Display for Instrument {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.symbol, self.asset_class)
    }
}

/// Builder for constructing [`Instrument`] instances.
///
/// Provides a fluent API for setting optional fields.
#[derive(Debug, Clone)]
pub struct InstrumentBuilder {
    symbol: Symbol,
    asset_class: AssetClass,
    settlement_method: SettlementMethod,
    min_quantity: Option<Quantity>,
    price_decimals: Option<u8>,
}

impl InstrumentBuilder {
    /// Creates a new builder with required fields.
    fn new(symbol: Symbol, asset_class: AssetClass) -> Self {
        Self {
            symbol,
            asset_class,
            settlement_method: SettlementMethod::default(),
            min_quantity: None,
            price_decimals: None,
        }
    }

    /// Sets the settlement method.
    #[must_use]
    pub fn settlement_method(mut self, method: SettlementMethod) -> Self {
        self.settlement_method = method;
        self
    }

    /// Sets the minimum tradeable quantity.
    #[must_use]
    pub fn min_quantity(mut self, quantity: Quantity) -> Self {
        self.min_quantity = Some(quantity);
        self
    }

    /// Sets the number of price decimal places.
    #[must_use]
    pub fn price_decimals(mut self, decimals: u8) -> Self {
        self.price_decimals = Some(decimals);
        self
    }

    /// Builds the instrument.
    #[must_use]
    pub fn build(self) -> Instrument {
        Instrument {
            symbol: self.symbol,
            asset_class: self.asset_class,
            settlement_method: self.settlement_method,
            min_quantity: self.min_quantity,
            price_decimals: self.price_decimals,
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::domain::value_objects::enums::Blockchain;

    mod construction {
        use super::*;

        #[test]
        fn new_creates_instrument() {
            let symbol = Symbol::new("BTC/USD").unwrap();
            let instrument = Instrument::new(
                symbol.clone(),
                AssetClass::CryptoSpot,
                SettlementMethod::OnChain(Blockchain::Ethereum),
            );

            assert_eq!(instrument.symbol(), &symbol);
            assert_eq!(instrument.asset_class(), AssetClass::CryptoSpot);
            assert!(instrument.settlement_method().is_onchain());
            assert!(instrument.min_quantity().is_none());
            assert!(instrument.price_decimals().is_none());
        }

        #[test]
        fn builder_with_all_fields() {
            let symbol = Symbol::new("ETH/USDC").unwrap();
            let min_qty = Quantity::new(0.01).unwrap();

            let instrument = Instrument::builder(symbol.clone(), AssetClass::CryptoSpot)
                .settlement_method(SettlementMethod::OnChain(Blockchain::Polygon))
                .min_quantity(min_qty)
                .price_decimals(6)
                .build();

            assert_eq!(instrument.symbol(), &symbol);
            assert_eq!(instrument.asset_class(), AssetClass::CryptoSpot);
            assert_eq!(
                instrument.settlement_method(),
                SettlementMethod::OnChain(Blockchain::Polygon)
            );
            assert_eq!(instrument.min_quantity(), Some(min_qty));
            assert_eq!(instrument.price_decimals(), Some(6));
        }

        #[test]
        fn builder_with_defaults() {
            let symbol = Symbol::new("SOL/USDT").unwrap();
            let instrument = Instrument::builder(symbol, AssetClass::CryptoDerivs).build();

            assert_eq!(instrument.settlement_method(), SettlementMethod::default());
            assert!(instrument.min_quantity().is_none());
            assert!(instrument.price_decimals().is_none());
        }
    }

    mod accessors {
        use super::*;

        #[test]
        fn base_and_quote_asset() {
            let symbol = Symbol::new("BTC/USD").unwrap();
            let instrument =
                Instrument::new(symbol, AssetClass::CryptoSpot, SettlementMethod::default());

            assert_eq!(instrument.base_asset(), "BTC");
            assert_eq!(instrument.quote_asset(), "USD");
        }

        #[test]
        fn is_crypto() {
            let symbol = Symbol::new("BTC/USD").unwrap();

            let crypto = Instrument::new(
                symbol.clone(),
                AssetClass::CryptoSpot,
                SettlementMethod::default(),
            );
            assert!(crypto.is_crypto());

            let stock = Instrument::new(symbol, AssetClass::Stock, SettlementMethod::OffChain);
            assert!(!stock.is_crypto());
        }

        #[test]
        fn is_onchain() {
            let symbol = Symbol::new("ETH/USDC").unwrap();

            let onchain = Instrument::new(
                symbol.clone(),
                AssetClass::CryptoSpot,
                SettlementMethod::OnChain(Blockchain::Ethereum),
            );
            assert!(onchain.is_onchain());

            let offchain =
                Instrument::new(symbol, AssetClass::CryptoSpot, SettlementMethod::OffChain);
            assert!(!offchain.is_onchain());
        }
    }

    mod display {
        use super::*;

        #[test]
        fn display_format() {
            let symbol = Symbol::new("BTC/USD").unwrap();
            let instrument =
                Instrument::new(symbol, AssetClass::CryptoSpot, SettlementMethod::default());

            assert_eq!(instrument.to_string(), "BTC/USD (CRYPTO_SPOT)");
        }
    }

    mod serde {
        use super::*;

        #[test]
        fn serde_roundtrip() {
            let symbol = Symbol::new("BTC/USD").unwrap();
            let instrument = Instrument::builder(symbol, AssetClass::CryptoSpot)
                .settlement_method(SettlementMethod::OnChain(Blockchain::Ethereum))
                .price_decimals(2)
                .build();

            let json = serde_json::to_string(&instrument).unwrap();
            let deserialized: Instrument = serde_json::from_str(&json).unwrap();

            assert_eq!(instrument, deserialized);
        }

        #[test]
        fn serde_with_optional_fields() {
            let symbol = Symbol::new("ETH/USDC").unwrap();
            let min_qty = Quantity::new(0.1).unwrap();

            let instrument = Instrument::builder(symbol, AssetClass::CryptoSpot)
                .min_quantity(min_qty)
                .build();

            let json = serde_json::to_string(&instrument).unwrap();
            let deserialized: Instrument = serde_json::from_str(&json).unwrap();

            assert_eq!(instrument.min_quantity(), deserialized.min_quantity());
        }
    }
}
