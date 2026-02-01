//! # Symbol Value Object
//!
//! Trading symbol representation.
//!
//! This module provides the [`Symbol`] type for representing trading pairs
//! in the format `BASE/QUOTE` (e.g., `BTC/USD`, `ETH/USDC`).
//!
//! # Examples
//!
//! ```
//! use otc_rfq::domain::value_objects::symbol::Symbol;
//!
//! let symbol = Symbol::new("BTC/USD").unwrap();
//! assert_eq!(symbol.base_asset(), "BTC");
//! assert_eq!(symbol.quote_asset(), "USD");
//! ```

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use thiserror::Error;

/// Error type for symbol parsing and validation.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SymbolError {
    /// Symbol string is empty.
    #[error("symbol cannot be empty")]
    Empty,

    /// Symbol format is invalid (missing separator).
    #[error("invalid symbol format: expected BASE/QUOTE, got '{0}'")]
    InvalidFormat(String),

    /// Base asset is empty.
    #[error("base asset cannot be empty")]
    EmptyBaseAsset,

    /// Quote asset is empty.
    #[error("quote asset cannot be empty")]
    EmptyQuoteAsset,

    /// Symbol contains invalid characters.
    #[error("symbol contains invalid characters: '{0}'")]
    InvalidCharacters(String),
}

/// A validated trading symbol.
///
/// Represents a trading pair in the format `BASE/QUOTE`.
/// Both base and quote assets are stored in uppercase.
///
/// # Invariants
///
/// - Symbol is non-empty
/// - Format is `BASE/QUOTE` with exactly one `/` separator
/// - Both base and quote assets are non-empty
/// - Only alphanumeric characters allowed in asset names
///
/// # Examples
///
/// ```
/// use otc_rfq::domain::value_objects::symbol::Symbol;
///
/// // Create from string
/// let symbol = Symbol::new("btc/usd").unwrap();
/// assert_eq!(symbol.to_string(), "BTC/USD");
///
/// // Parse from string
/// let symbol: Symbol = "ETH/USDC".parse().unwrap();
/// assert_eq!(symbol.base_asset(), "ETH");
/// assert_eq!(symbol.quote_asset(), "USDC");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Symbol {
    /// The full symbol string (e.g., "BTC/USD").
    value: String,
    /// Index of the separator for efficient base/quote extraction.
    separator_index: usize,
}

impl Symbol {
    /// The separator character between base and quote assets.
    pub const SEPARATOR: char = '/';

    /// Creates a new symbol from a string.
    ///
    /// The input is normalized to uppercase.
    ///
    /// # Arguments
    ///
    /// * `value` - The symbol string (e.g., "BTC/USD")
    ///
    /// # Errors
    ///
    /// Returns `SymbolError` if:
    /// - The string is empty
    /// - The format is invalid (missing or multiple separators)
    /// - Base or quote asset is empty
    /// - Contains invalid characters
    ///
    /// # Examples
    ///
    /// ```
    /// use otc_rfq::domain::value_objects::symbol::Symbol;
    ///
    /// let symbol = Symbol::new("BTC/USD").unwrap();
    /// assert_eq!(symbol.to_string(), "BTC/USD");
    ///
    /// // Lowercase is normalized to uppercase
    /// let symbol = Symbol::new("eth/usdc").unwrap();
    /// assert_eq!(symbol.to_string(), "ETH/USDC");
    /// ```
    pub fn new(value: impl AsRef<str>) -> Result<Self, SymbolError> {
        let value = value.as_ref().trim();

        if value.is_empty() {
            return Err(SymbolError::Empty);
        }

        // Normalize to uppercase
        let normalized = value.to_uppercase();

        // Find separator
        let separator_count = normalized.matches(Self::SEPARATOR).count();
        if separator_count != 1 {
            return Err(SymbolError::InvalidFormat(value.to_string()));
        }

        let separator_index = normalized
            .find(Self::SEPARATOR)
            .ok_or_else(|| SymbolError::InvalidFormat(value.to_string()))?;

        // Validate base asset
        let base = &normalized[..separator_index];
        if base.is_empty() {
            return Err(SymbolError::EmptyBaseAsset);
        }

        // Validate quote asset
        let quote = &normalized[separator_index + 1..];
        if quote.is_empty() {
            return Err(SymbolError::EmptyQuoteAsset);
        }

        // Validate characters (alphanumeric only)
        if !base.chars().all(|c| c.is_ascii_alphanumeric()) {
            return Err(SymbolError::InvalidCharacters(base.to_string()));
        }
        if !quote.chars().all(|c| c.is_ascii_alphanumeric()) {
            return Err(SymbolError::InvalidCharacters(quote.to_string()));
        }

        Ok(Self {
            value: normalized,
            separator_index,
        })
    }

    /// Returns the base asset (left side of the pair).
    ///
    /// # Examples
    ///
    /// ```
    /// use otc_rfq::domain::value_objects::symbol::Symbol;
    ///
    /// let symbol = Symbol::new("BTC/USD").unwrap();
    /// assert_eq!(symbol.base_asset(), "BTC");
    /// ```
    #[inline]
    #[must_use]
    pub fn base_asset(&self) -> &str {
        &self.value[..self.separator_index]
    }

    /// Returns the quote asset (right side of the pair).
    ///
    /// # Examples
    ///
    /// ```
    /// use otc_rfq::domain::value_objects::symbol::Symbol;
    ///
    /// let symbol = Symbol::new("BTC/USD").unwrap();
    /// assert_eq!(symbol.quote_asset(), "USD");
    /// ```
    #[inline]
    #[must_use]
    pub fn quote_asset(&self) -> &str {
        &self.value[self.separator_index + 1..]
    }

    /// Returns the full symbol string.
    #[inline]
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.value
    }

    /// Returns the inverted symbol (swap base and quote).
    ///
    /// # Examples
    ///
    /// ```
    /// use otc_rfq::domain::value_objects::symbol::Symbol;
    ///
    /// let symbol = Symbol::new("BTC/USD").unwrap();
    /// let inverted = symbol.invert();
    /// assert_eq!(inverted.to_string(), "USD/BTC");
    /// ```
    #[must_use]
    pub fn invert(&self) -> Self {
        let inverted = format!("{}/{}", self.quote_asset(), self.base_asset());
        // Safe because we're just swapping validated components
        Self {
            separator_index: self.value.len() - self.separator_index - 1,
            value: inverted,
        }
    }
}

impl fmt::Display for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.value)
    }
}

impl FromStr for Symbol {
    type Err = SymbolError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

impl TryFrom<String> for Symbol {
    type Error = SymbolError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<&str> for Symbol {
    type Error = SymbolError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<Symbol> for String {
    fn from(symbol: Symbol) -> Self {
        symbol.value
    }
}

impl AsRef<str> for Symbol {
    fn as_ref(&self) -> &str {
        &self.value
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    mod construction {
        use super::*;

        #[test]
        fn new_valid_symbol() {
            let symbol = Symbol::new("BTC/USD").unwrap();
            assert_eq!(symbol.to_string(), "BTC/USD");
        }

        #[test]
        fn new_normalizes_to_uppercase() {
            let symbol = Symbol::new("btc/usd").unwrap();
            assert_eq!(symbol.to_string(), "BTC/USD");
        }

        #[test]
        fn new_mixed_case() {
            let symbol = Symbol::new("Eth/Usdc").unwrap();
            assert_eq!(symbol.to_string(), "ETH/USDC");
        }

        #[test]
        fn new_trims_whitespace() {
            let symbol = Symbol::new("  BTC/USD  ").unwrap();
            assert_eq!(symbol.to_string(), "BTC/USD");
        }

        #[test]
        fn new_empty_fails() {
            let result = Symbol::new("");
            assert!(matches!(result, Err(SymbolError::Empty)));
        }

        #[test]
        fn new_no_separator_fails() {
            let result = Symbol::new("BTCUSD");
            assert!(matches!(result, Err(SymbolError::InvalidFormat(_))));
        }

        #[test]
        fn new_multiple_separators_fails() {
            let result = Symbol::new("BTC/USD/EUR");
            assert!(matches!(result, Err(SymbolError::InvalidFormat(_))));
        }

        #[test]
        fn new_empty_base_fails() {
            let result = Symbol::new("/USD");
            assert!(matches!(result, Err(SymbolError::EmptyBaseAsset)));
        }

        #[test]
        fn new_empty_quote_fails() {
            let result = Symbol::new("BTC/");
            assert!(matches!(result, Err(SymbolError::EmptyQuoteAsset)));
        }

        #[test]
        fn new_invalid_characters_fails() {
            let result = Symbol::new("BTC$/USD");
            assert!(matches!(result, Err(SymbolError::InvalidCharacters(_))));
        }
    }

    mod accessors {
        use super::*;

        #[test]
        fn base_asset() {
            let symbol = Symbol::new("BTC/USD").unwrap();
            assert_eq!(symbol.base_asset(), "BTC");
        }

        #[test]
        fn quote_asset() {
            let symbol = Symbol::new("BTC/USD").unwrap();
            assert_eq!(symbol.quote_asset(), "USD");
        }

        #[test]
        fn as_str() {
            let symbol = Symbol::new("ETH/USDC").unwrap();
            assert_eq!(symbol.as_str(), "ETH/USDC");
        }
    }

    mod invert {
        use super::*;

        #[test]
        fn invert_swaps_base_and_quote() {
            let symbol = Symbol::new("BTC/USD").unwrap();
            let inverted = symbol.invert();
            assert_eq!(inverted.to_string(), "USD/BTC");
            assert_eq!(inverted.base_asset(), "USD");
            assert_eq!(inverted.quote_asset(), "BTC");
        }

        #[test]
        fn double_invert_returns_original() {
            let symbol = Symbol::new("ETH/USDC").unwrap();
            let double_inverted = symbol.invert().invert();
            assert_eq!(symbol, double_inverted);
        }
    }

    mod from_str {
        use super::*;

        #[test]
        fn parse_works() {
            let symbol: Symbol = "BTC/USD".parse().unwrap();
            assert_eq!(symbol.to_string(), "BTC/USD");
        }

        #[test]
        fn parse_invalid_fails() {
            let result: Result<Symbol, _> = "INVALID".parse();
            assert!(result.is_err());
        }
    }

    mod serde {
        use super::*;

        #[test]
        fn serde_roundtrip() {
            let symbol = Symbol::new("BTC/USD").unwrap();
            let json = serde_json::to_string(&symbol).unwrap();
            assert_eq!(json, "\"BTC/USD\"");
            let deserialized: Symbol = serde_json::from_str(&json).unwrap();
            assert_eq!(symbol, deserialized);
        }

        #[test]
        fn deserialize_invalid_fails() {
            let json = "\"INVALID\"";
            let result: Result<Symbol, _> = serde_json::from_str(json);
            assert!(result.is_err());
        }
    }

    mod display {
        use super::*;

        #[test]
        fn display_format() {
            let symbol = Symbol::new("SOL/USDT").unwrap();
            assert_eq!(format!("{}", symbol), "SOL/USDT");
        }
    }

    mod error {
        use super::*;

        #[test]
        fn error_display() {
            assert_eq!(SymbolError::Empty.to_string(), "symbol cannot be empty");
            assert_eq!(
                SymbolError::InvalidFormat("test".to_string()).to_string(),
                "invalid symbol format: expected BASE/QUOTE, got 'test'"
            );
        }
    }
}
