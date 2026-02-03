//! # Quantity Value Object
//!
//! Decimal quantity with checked arithmetic.
//!
//! This module provides the [`Quantity`] type, a type-safe wrapper around
//! [`Decimal`] for representing trade quantities with validation and
//! checked arithmetic operations.
//!
//! # Examples
//!
//! ```
//! use otc_rfq::domain::value_objects::quantity::Quantity;
//!
//! let qty = Quantity::new(100.0).unwrap();
//! let other = Quantity::new(50.0).unwrap();
//!
//! let sum = qty.safe_add(other).unwrap();
//! assert_eq!(sum.get().to_string(), "150");
//! ```

use super::arithmetic::{ArithmeticError, ArithmeticResult, CheckedArithmetic};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fmt;
use std::str::FromStr;

/// A validated quantity value.
///
/// Represents a non-negative decimal quantity with checked arithmetic operations.
/// Quantities cannot be negative.
///
/// # Invariants
///
/// - Quantity is always >= 0
///
/// # Examples
///
/// ```
/// use otc_rfq::domain::value_objects::quantity::Quantity;
///
/// // Create from f64
/// let qty = Quantity::new(100.0).unwrap();
///
/// // Create from Decimal
/// use rust_decimal::Decimal;
/// let qty = Quantity::from_decimal(Decimal::new(100, 0)).unwrap();
///
/// // Zero quantity
/// let zero = Quantity::zero();
/// assert!(zero.is_zero());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "Decimal", into = "Decimal")]
pub struct Quantity(Decimal);

impl Quantity {
    /// Zero quantity constant.
    pub const ZERO: Self = Self(Decimal::ZERO);

    /// Creates a new quantity from an f64 value.
    ///
    /// # Arguments
    ///
    /// * `value` - The quantity value (must be non-negative)
    ///
    /// # Errors
    ///
    /// Returns `ArithmeticError::InvalidValue` if the value is negative.
    ///
    /// # Examples
    ///
    /// ```
    /// use otc_rfq::domain::value_objects::quantity::Quantity;
    ///
    /// let qty = Quantity::new(100.0).unwrap();
    /// assert!(!qty.is_zero());
    ///
    /// let invalid = Quantity::new(-10.0);
    /// assert!(invalid.is_err());
    /// ```
    #[must_use = "this returns a Result that should be handled"]
    pub fn new(value: f64) -> ArithmeticResult<Self> {
        let decimal =
            Decimal::try_from(value).map_err(|_| ArithmeticError::InvalidValue("invalid float"))?;
        Self::from_decimal(decimal)
    }

    /// Creates a new quantity from a Decimal value.
    ///
    /// # Arguments
    ///
    /// * `value` - The decimal quantity value (must be non-negative)
    ///
    /// # Errors
    ///
    /// Returns `ArithmeticError::InvalidValue` if the value is negative.
    #[must_use = "this returns a Result that should be handled"]
    pub fn from_decimal(value: Decimal) -> ArithmeticResult<Self> {
        if value.is_sign_negative() {
            return Err(ArithmeticError::InvalidValue("quantity cannot be negative"));
        }
        Ok(Self(value))
    }

    /// Creates a zero quantity.
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

    /// Returns true if the quantity is zero.
    #[inline]
    #[must_use]
    pub fn is_zero(self) -> bool {
        self.0.is_zero()
    }

    /// Returns true if the quantity is positive (non-zero).
    #[inline]
    #[must_use]
    pub fn is_positive(self) -> bool {
        self.0.is_sign_positive() && !self.0.is_zero()
    }

    /// Safely adds another quantity.
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

    /// Safely subtracts another quantity.
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

    /// Returns the minimum of two quantities.
    #[inline]
    #[must_use]
    pub fn min(self, other: Self) -> Self {
        if self.0 <= other.0 { self } else { other }
    }

    /// Returns the maximum of two quantities.
    #[inline]
    #[must_use]
    pub fn max(self, other: Self) -> Self {
        if self.0 >= other.0 { self } else { other }
    }

    /// Returns the remaining quantity after filling.
    ///
    /// Calculates `self - filled`, returning zero if filled >= self.
    ///
    /// # Examples
    ///
    /// ```
    /// use otc_rfq::domain::value_objects::quantity::Quantity;
    ///
    /// let total = Quantity::new(100.0).unwrap();
    /// let filled = Quantity::new(30.0).unwrap();
    /// let remaining = total.remaining_after(filled);
    /// assert_eq!(remaining.get().to_string(), "70");
    /// ```
    #[inline]
    #[must_use]
    pub fn remaining_after(self, filled: Self) -> Self {
        if filled.0 >= self.0 {
            Self::ZERO
        } else {
            Self(self.0 - filled.0)
        }
    }

    /// Returns the fill ratio as a Decimal between 0 and 1.
    ///
    /// # Arguments
    ///
    /// * `filled` - The filled quantity
    ///
    /// # Returns
    ///
    /// The ratio of filled to total, capped at 1.0.
    /// Returns 1.0 if self is zero (fully filled by definition).
    #[inline]
    #[must_use]
    pub fn fill_ratio(self, filled: Self) -> Decimal {
        if self.is_zero() {
            return Decimal::ONE;
        }
        let ratio = filled.0 / self.0;
        if ratio > Decimal::ONE {
            Decimal::ONE
        } else {
            ratio
        }
    }
}

impl fmt::Display for Quantity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl PartialOrd for Quantity {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Quantity {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }
}

impl TryFrom<Decimal> for Quantity {
    type Error = ArithmeticError;

    fn try_from(value: Decimal) -> Result<Self, Self::Error> {
        Self::from_decimal(value)
    }
}

impl From<Quantity> for Decimal {
    fn from(quantity: Quantity) -> Self {
        quantity.0
    }
}

impl FromStr for Quantity {
    type Err = ArithmeticError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let decimal =
            Decimal::from_str(s).map_err(|_| ArithmeticError::InvalidValue("invalid decimal"))?;
        Self::from_decimal(decimal)
    }
}

impl Default for Quantity {
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
            let qty = Quantity::new(100.0).unwrap();
            assert!(!qty.is_zero());
        }

        #[test]
        fn new_zero_succeeds() {
            let qty = Quantity::new(0.0).unwrap();
            assert!(qty.is_zero());
        }

        #[test]
        fn new_negative_fails() {
            let result = Quantity::new(-10.0);
            assert!(matches!(result, Err(ArithmeticError::InvalidValue(_))));
        }

        #[test]
        fn from_decimal_positive_succeeds() {
            let decimal = Decimal::new(100, 0);
            let qty = Quantity::from_decimal(decimal).unwrap();
            assert_eq!(qty.get(), decimal);
        }

        #[test]
        fn from_decimal_negative_fails() {
            let decimal = Decimal::new(-100, 0);
            let result = Quantity::from_decimal(decimal);
            assert!(matches!(result, Err(ArithmeticError::InvalidValue(_))));
        }

        #[test]
        fn zero_constant() {
            assert!(Quantity::ZERO.is_zero());
            assert_eq!(Quantity::zero(), Quantity::ZERO);
        }

        #[test]
        fn from_str_works() {
            let qty: Quantity = "100.50".parse().unwrap();
            assert_eq!(qty.get(), Decimal::new(10050, 2));
        }

        #[test]
        fn from_str_negative_fails() {
            let result: Result<Quantity, _> = "-100".parse();
            assert!(result.is_err());
        }
    }

    mod arithmetic {
        use super::*;

        #[test]
        fn safe_add_works() {
            let a = Quantity::new(100.0).unwrap();
            let b = Quantity::new(50.0).unwrap();
            let result = a.safe_add(b).unwrap();
            assert_eq!(result.get(), Decimal::new(150, 0));
        }

        #[test]
        fn safe_sub_works() {
            let a = Quantity::new(100.0).unwrap();
            let b = Quantity::new(50.0).unwrap();
            let result = a.safe_sub(b).unwrap();
            assert_eq!(result.get(), Decimal::new(50, 0));
        }

        #[test]
        fn safe_sub_underflow_fails() {
            let a = Quantity::new(50.0).unwrap();
            let b = Quantity::new(100.0).unwrap();
            let result = a.safe_sub(b);
            assert_eq!(result, Err(ArithmeticError::Underflow));
        }

        #[test]
        fn safe_mul_works() {
            let qty = Quantity::new(100.0).unwrap();
            let factor = Decimal::new(2, 0);
            let result = qty.safe_mul(factor).unwrap();
            assert_eq!(result.get(), Decimal::new(200, 0));
        }

        #[test]
        fn safe_mul_negative_factor_fails() {
            let qty = Quantity::new(100.0).unwrap();
            let factor = Decimal::new(-2, 0);
            let result = qty.safe_mul(factor);
            assert!(matches!(result, Err(ArithmeticError::InvalidValue(_))));
        }

        #[test]
        fn safe_div_works() {
            let qty = Quantity::new(100.0).unwrap();
            let divisor = Decimal::new(2, 0);
            let result = qty.safe_div(divisor).unwrap();
            assert_eq!(result.get(), Decimal::new(50, 0));
        }

        #[test]
        fn safe_div_by_zero_fails() {
            let qty = Quantity::new(100.0).unwrap();
            let result = qty.safe_div(Decimal::ZERO);
            assert_eq!(result, Err(ArithmeticError::DivisionByZero));
        }
    }

    mod comparison {
        use super::*;

        #[test]
        fn ordering_works() {
            let low = Quantity::new(50.0).unwrap();
            let high = Quantity::new(100.0).unwrap();
            assert!(low < high);
            assert!(high > low);
        }

        #[test]
        fn min_works() {
            let a = Quantity::new(50.0).unwrap();
            let b = Quantity::new(100.0).unwrap();
            assert_eq!(a.min(b), a);
            assert_eq!(b.min(a), a);
        }

        #[test]
        fn max_works() {
            let a = Quantity::new(50.0).unwrap();
            let b = Quantity::new(100.0).unwrap();
            assert_eq!(a.max(b), b);
            assert_eq!(b.max(a), b);
        }
    }

    mod fill_operations {
        use super::*;

        #[test]
        fn remaining_after_partial_fill() {
            let total = Quantity::new(100.0).unwrap();
            let filled = Quantity::new(30.0).unwrap();
            let remaining = total.remaining_after(filled);
            assert_eq!(remaining.get(), Decimal::new(70, 0));
        }

        #[test]
        fn remaining_after_full_fill() {
            let total = Quantity::new(100.0).unwrap();
            let filled = Quantity::new(100.0).unwrap();
            let remaining = total.remaining_after(filled);
            assert!(remaining.is_zero());
        }

        #[test]
        fn remaining_after_overfill() {
            let total = Quantity::new(100.0).unwrap();
            let filled = Quantity::new(150.0).unwrap();
            let remaining = total.remaining_after(filled);
            assert!(remaining.is_zero());
        }

        #[test]
        fn fill_ratio_partial() {
            let total = Quantity::new(100.0).unwrap();
            let filled = Quantity::new(50.0).unwrap();
            let ratio = total.fill_ratio(filled);
            assert_eq!(ratio, Decimal::new(5, 1)); // 0.5
        }

        #[test]
        fn fill_ratio_full() {
            let total = Quantity::new(100.0).unwrap();
            let filled = Quantity::new(100.0).unwrap();
            let ratio = total.fill_ratio(filled);
            assert_eq!(ratio, Decimal::ONE);
        }

        #[test]
        fn fill_ratio_overfill_capped() {
            let total = Quantity::new(100.0).unwrap();
            let filled = Quantity::new(150.0).unwrap();
            let ratio = total.fill_ratio(filled);
            assert_eq!(ratio, Decimal::ONE);
        }

        #[test]
        fn fill_ratio_zero_total() {
            let total = Quantity::ZERO;
            let filled = Quantity::new(50.0).unwrap();
            let ratio = total.fill_ratio(filled);
            assert_eq!(ratio, Decimal::ONE);
        }
    }

    mod display {
        use super::*;

        #[test]
        fn display_formats_correctly() {
            let qty = Quantity::new(100.50).unwrap();
            // Decimal may not preserve trailing zeros
            assert!(qty.to_string().starts_with("100.5"));
        }
    }

    mod serde {
        use super::*;

        #[test]
        fn serde_roundtrip() {
            let qty = Quantity::new(100.50).unwrap();
            let json = serde_json::to_string(&qty).unwrap();
            let deserialized: Quantity = serde_json::from_str(&json).unwrap();
            assert_eq!(qty, deserialized);
        }

        #[test]
        fn deserialize_negative_fails() {
            let json = "-100";
            let result: Result<Quantity, _> = serde_json::from_str(json);
            assert!(result.is_err());
        }
    }

    mod properties {
        use super::*;

        #[test]
        fn is_positive_works() {
            assert!(!Quantity::ZERO.is_positive());
            assert!(Quantity::new(100.0).unwrap().is_positive());
        }

        #[test]
        fn default_is_zero() {
            assert_eq!(Quantity::default(), Quantity::ZERO);
        }
    }
}
