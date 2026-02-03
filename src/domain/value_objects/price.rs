//! # Price Value Object
//!
//! Decimal price with checked arithmetic.
//!
//! This module provides the [`Price`] type, a type-safe wrapper around
//! [`Decimal`] for representing monetary prices with validation and
//! checked arithmetic operations.
//!
//! # Examples
//!
//! ```
//! use otc_rfq::domain::value_objects::price::Price;
//!
//! let price = Price::new(100.50).unwrap();
//! let other = Price::new(50.25).unwrap();
//!
//! let sum = price.safe_add(other).unwrap();
//! assert_eq!(sum.get().to_string(), "150.75");
//! ```

use super::arithmetic::{ArithmeticError, ArithmeticResult, CheckedArithmetic};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fmt;
use std::str::FromStr;

/// A validated price value.
///
/// Represents a non-negative decimal price with checked arithmetic operations.
/// Prices cannot be negative.
///
/// # Invariants
///
/// - Price is always >= 0
///
/// # Examples
///
/// ```
/// use otc_rfq::domain::value_objects::price::Price;
///
/// // Create from f64
/// let price = Price::new(100.50).unwrap();
///
/// // Create from Decimal
/// use rust_decimal::Decimal;
/// let price = Price::from_decimal(Decimal::new(10050, 2)).unwrap();
///
/// // Zero price
/// let zero = Price::zero();
/// assert!(zero.is_zero());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "Decimal", into = "Decimal")]
pub struct Price(Decimal);

impl Price {
    /// Zero price constant.
    pub const ZERO: Self = Self(Decimal::ZERO);

    /// Creates a new price from an f64 value.
    ///
    /// # Arguments
    ///
    /// * `value` - The price value (must be non-negative)
    ///
    /// # Errors
    ///
    /// Returns `ArithmeticError::InvalidValue` if the value is negative.
    ///
    /// # Examples
    ///
    /// ```
    /// use otc_rfq::domain::value_objects::price::Price;
    ///
    /// let price = Price::new(100.50).unwrap();
    /// assert!(!price.is_zero());
    ///
    /// let invalid = Price::new(-10.0);
    /// assert!(invalid.is_err());
    /// ```
    #[must_use = "this returns a Result that should be handled"]
    pub fn new(value: f64) -> ArithmeticResult<Self> {
        let decimal =
            Decimal::try_from(value).map_err(|_| ArithmeticError::InvalidValue("invalid float"))?;
        Self::from_decimal(decimal)
    }

    /// Creates a new price from a Decimal value.
    ///
    /// # Arguments
    ///
    /// * `value` - The decimal price value (must be non-negative)
    ///
    /// # Errors
    ///
    /// Returns `ArithmeticError::InvalidValue` if the value is negative.
    #[must_use = "this returns a Result that should be handled"]
    pub fn from_decimal(value: Decimal) -> ArithmeticResult<Self> {
        if value.is_sign_negative() {
            return Err(ArithmeticError::InvalidValue("price cannot be negative"));
        }
        Ok(Self(value))
    }

    /// Creates a zero price.
    #[inline]
    #[must_use]
    pub const fn zero() -> Self {
        Self::ZERO
    }

    /// Returns the inner Decimal value.
    #[inline]
    #[must_use]
    pub const fn get(self) -> Decimal {
        self.0
    }

    /// Returns true if the price is zero.
    #[inline]
    #[must_use]
    pub fn is_zero(self) -> bool {
        self.0.is_zero()
    }

    /// Returns true if the price is positive (non-zero).
    #[inline]
    #[must_use]
    pub fn is_positive(self) -> bool {
        self.0.is_sign_positive() && !self.0.is_zero()
    }

    /// Safely adds another price.
    ///
    /// # Errors
    ///
    /// Returns `ArithmeticError::Overflow` if the result would overflow.
    #[inline]
    #[must_use = "this returns the result of the operation, without modifying the original"]
    pub fn safe_add(self, rhs: Self) -> ArithmeticResult<Self> {
        let result = self.0.safe_add(rhs.0)?;
        Ok(Self(result))
    }

    /// Safely subtracts another price.
    ///
    /// # Errors
    ///
    /// Returns `ArithmeticError::Underflow` if the result would be negative.
    #[inline]
    #[must_use = "this returns the result of the operation, without modifying the original"]
    pub fn safe_sub(self, rhs: Self) -> ArithmeticResult<Self> {
        let result = self.0.safe_sub(rhs.0)?;
        if result.is_sign_negative() {
            return Err(ArithmeticError::Underflow);
        }
        Ok(Self(result))
    }

    /// Safely multiplies by a Decimal factor.
    ///
    /// # Errors
    ///
    /// Returns `ArithmeticError::Overflow` if the result would overflow.
    /// Returns `ArithmeticError::InvalidValue` if the result would be negative.
    #[inline]
    #[must_use = "this returns the result of the operation, without modifying the original"]
    pub fn safe_mul(self, factor: Decimal) -> ArithmeticResult<Self> {
        let result = self.0.safe_mul(factor)?;
        if result.is_sign_negative() {
            return Err(ArithmeticError::InvalidValue(
                "multiplication result cannot be negative",
            ));
        }
        Ok(Self(result))
    }

    /// Safely divides by a Decimal divisor.
    ///
    /// # Errors
    ///
    /// Returns `ArithmeticError::DivisionByZero` if the divisor is zero.
    /// Returns `ArithmeticError::InvalidValue` if the result would be negative.
    #[inline]
    #[must_use = "this returns the result of the operation, without modifying the original"]
    pub fn safe_div(self, divisor: Decimal) -> ArithmeticResult<Self> {
        let result = self.0.safe_div(divisor)?;
        if result.is_sign_negative() {
            return Err(ArithmeticError::InvalidValue(
                "division result cannot be negative",
            ));
        }
        Ok(Self(result))
    }

    /// Returns the minimum of two prices.
    #[inline]
    #[must_use]
    pub fn min(self, other: Self) -> Self {
        if self.0 <= other.0 { self } else { other }
    }

    /// Returns the maximum of two prices.
    #[inline]
    #[must_use]
    pub fn max(self, other: Self) -> Self {
        if self.0 >= other.0 { self } else { other }
    }
}

impl fmt::Display for Price {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl PartialOrd for Price {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Price {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }
}

impl TryFrom<Decimal> for Price {
    type Error = ArithmeticError;

    fn try_from(value: Decimal) -> Result<Self, Self::Error> {
        Self::from_decimal(value)
    }
}

impl From<Price> for Decimal {
    fn from(price: Price) -> Self {
        price.0
    }
}

impl FromStr for Price {
    type Err = ArithmeticError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let decimal =
            Decimal::from_str(s).map_err(|_| ArithmeticError::InvalidValue("invalid decimal"))?;
        Self::from_decimal(decimal)
    }
}

impl Default for Price {
    fn default() -> Self {
        Self::ZERO
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    mod construction {
        use super::*;

        #[test]
        fn new_positive_succeeds() {
            let price = Price::new(100.50).unwrap();
            assert!(!price.is_zero());
        }

        #[test]
        fn new_zero_succeeds() {
            let price = Price::new(0.0).unwrap();
            assert!(price.is_zero());
        }

        #[test]
        fn new_negative_fails() {
            let result = Price::new(-10.0);
            assert!(matches!(result, Err(ArithmeticError::InvalidValue(_))));
        }

        #[test]
        fn from_decimal_positive_succeeds() {
            let decimal = Decimal::new(10050, 2);
            let price = Price::from_decimal(decimal).unwrap();
            assert_eq!(price.get(), decimal);
        }

        #[test]
        fn from_decimal_negative_fails() {
            let decimal = Decimal::new(-100, 0);
            let result = Price::from_decimal(decimal);
            assert!(matches!(result, Err(ArithmeticError::InvalidValue(_))));
        }

        #[test]
        fn zero_constant() {
            assert!(Price::ZERO.is_zero());
            assert_eq!(Price::zero(), Price::ZERO);
        }

        #[test]
        fn from_str_works() {
            let price: Price = "100.50".parse().unwrap();
            assert_eq!(price.get(), Decimal::new(10050, 2));
        }

        #[test]
        fn from_str_negative_fails() {
            let result: Result<Price, _> = "-100".parse();
            assert!(result.is_err());
        }
    }

    mod arithmetic {
        use super::*;

        #[test]
        fn safe_add_works() {
            let a = Price::new(100.0).unwrap();
            let b = Price::new(50.0).unwrap();
            let result = a.safe_add(b).unwrap();
            assert_eq!(result.get(), Decimal::new(150, 0));
        }

        #[test]
        fn safe_sub_works() {
            let a = Price::new(100.0).unwrap();
            let b = Price::new(50.0).unwrap();
            let result = a.safe_sub(b).unwrap();
            assert_eq!(result.get(), Decimal::new(50, 0));
        }

        #[test]
        fn safe_sub_underflow_fails() {
            let a = Price::new(50.0).unwrap();
            let b = Price::new(100.0).unwrap();
            let result = a.safe_sub(b);
            assert_eq!(result, Err(ArithmeticError::Underflow));
        }

        #[test]
        fn safe_mul_works() {
            let price = Price::new(100.0).unwrap();
            let factor = Decimal::new(2, 0);
            let result = price.safe_mul(factor).unwrap();
            assert_eq!(result.get(), Decimal::new(200, 0));
        }

        #[test]
        fn safe_mul_negative_factor_fails() {
            let price = Price::new(100.0).unwrap();
            let factor = Decimal::new(-2, 0);
            let result = price.safe_mul(factor);
            assert!(matches!(result, Err(ArithmeticError::InvalidValue(_))));
        }

        #[test]
        fn safe_div_works() {
            let price = Price::new(100.0).unwrap();
            let divisor = Decimal::new(2, 0);
            let result = price.safe_div(divisor).unwrap();
            assert_eq!(result.get(), Decimal::new(50, 0));
        }

        #[test]
        fn safe_div_by_zero_fails() {
            let price = Price::new(100.0).unwrap();
            let result = price.safe_div(Decimal::ZERO);
            assert_eq!(result, Err(ArithmeticError::DivisionByZero));
        }
    }

    mod comparison {
        use super::*;

        #[test]
        fn ordering_works() {
            let low = Price::new(50.0).unwrap();
            let high = Price::new(100.0).unwrap();
            assert!(low < high);
            assert!(high > low);
        }

        #[test]
        fn min_works() {
            let a = Price::new(50.0).unwrap();
            let b = Price::new(100.0).unwrap();
            assert_eq!(a.min(b), a);
            assert_eq!(b.min(a), a);
        }

        #[test]
        fn max_works() {
            let a = Price::new(50.0).unwrap();
            let b = Price::new(100.0).unwrap();
            assert_eq!(a.max(b), b);
            assert_eq!(b.max(a), b);
        }
    }

    mod display {
        use super::*;

        #[test]
        fn display_formats_correctly() {
            let price = Price::new(100.50).unwrap();
            // Decimal may not preserve trailing zeros
            assert!(price.to_string().starts_with("100.5"));
        }
    }

    mod serde {
        use super::*;

        #[test]
        fn serde_roundtrip() {
            let price = Price::new(100.50).unwrap();
            let json = serde_json::to_string(&price).unwrap();
            let deserialized: Price = serde_json::from_str(&json).unwrap();
            assert_eq!(price, deserialized);
        }

        #[test]
        fn deserialize_negative_fails() {
            let json = "-100";
            let result: Result<Price, _> = serde_json::from_str(json);
            assert!(result.is_err());
        }
    }

    mod properties {
        use super::*;

        #[test]
        fn is_positive_works() {
            assert!(!Price::ZERO.is_positive());
            assert!(Price::new(100.0).unwrap().is_positive());
        }

        #[test]
        fn default_is_zero() {
            assert_eq!(Price::default(), Price::ZERO);
        }
    }
}
