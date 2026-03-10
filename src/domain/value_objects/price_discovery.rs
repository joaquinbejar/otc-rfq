//! # Price Discovery Value Objects
//!
//! Value objects for price discovery mechanisms in illiquid markets.

use crate::domain::value_objects::Price;
use serde::{Deserialize, Serialize};

/// Method used for price discovery in illiquid markets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PriceDiscoveryMethod {
    /// Use existing CLOB quotes (standard flow).
    Clob,
    /// Non-binding indicative quotes to gauge MM interest.
    Indicative,
    /// Broadcast interest check before formal RFQ.
    InterestGathering,
    /// Theoretical price using Black-Scholes + IV interpolation.
    Theoretical,
}

impl PriceDiscoveryMethod {
    /// Returns the priority of this method (lower = higher priority).
    #[must_use]
    pub fn priority(&self) -> u8 {
        match self {
            Self::Clob => 0,
            Self::Indicative => 1,
            Self::InterestGathering => 2,
            Self::Theoretical => 3,
        }
    }

    /// Returns whether this method provides firm quotes.
    #[must_use]
    pub fn is_firm(&self) -> bool {
        matches!(self, Self::Clob)
    }
}

impl std::fmt::Display for PriceDiscoveryMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Clob => write!(f, "CLOB"),
            Self::Indicative => write!(f, "Indicative"),
            Self::InterestGathering => write!(f, "InterestGathering"),
            Self::Theoretical => write!(f, "Theoretical"),
        }
    }
}

/// Theoretical price computed using Black-Scholes and IV interpolation.
///
/// Contains the computed price, implied volatility used, and confidence score.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TheoreticalPrice {
    /// Computed theoretical price.
    price: Price,
    /// Implied volatility used in calculation.
    implied_volatility: f64,
    /// Discovery method used.
    method: PriceDiscoveryMethod,
    /// Confidence score (0.0 = no confidence, 1.0 = high confidence).
    confidence: f64,
}

impl TheoreticalPrice {
    /// Creates a new theoretical price.
    ///
    /// # Arguments
    ///
    /// * `price` - Computed theoretical price
    /// * `implied_volatility` - IV used in calculation (must be > 0)
    /// * `method` - Discovery method used
    /// * `confidence` - Confidence score (clamped to 0.0-1.0)
    #[must_use]
    pub fn new(
        price: Price,
        implied_volatility: f64,
        method: PriceDiscoveryMethod,
        confidence: f64,
    ) -> Self {
        Self {
            price,
            implied_volatility: implied_volatility.max(0.0),
            method,
            confidence: confidence.clamp(0.0, 1.0),
        }
    }

    /// Returns the theoretical price.
    #[must_use]
    pub fn price(&self) -> Price {
        self.price
    }

    /// Returns the implied volatility used.
    #[must_use]
    pub fn implied_volatility(&self) -> f64 {
        self.implied_volatility
    }

    /// Returns the discovery method.
    #[must_use]
    pub fn method(&self) -> PriceDiscoveryMethod {
        self.method
    }

    /// Returns the confidence score.
    #[must_use]
    pub fn confidence(&self) -> f64 {
        self.confidence
    }

    /// Returns whether this price is reliable enough for trading.
    ///
    /// Prices with confidence < 0.5 should be reviewed manually.
    #[must_use]
    pub fn is_reliable(&self) -> bool {
        self.confidence >= 0.5
    }
}

impl std::fmt::Display for TheoreticalPrice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} (IV: {:.2}%, confidence: {:.1}%, method: {})",
            self.price,
            self.implied_volatility * 100.0,
            self.confidence * 100.0,
            self.method
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn price_discovery_method_priority() {
        assert_eq!(PriceDiscoveryMethod::Clob.priority(), 0);
        assert_eq!(PriceDiscoveryMethod::Indicative.priority(), 1);
        assert_eq!(PriceDiscoveryMethod::InterestGathering.priority(), 2);
        assert_eq!(PriceDiscoveryMethod::Theoretical.priority(), 3);
    }

    #[test]
    fn price_discovery_method_is_firm() {
        assert!(PriceDiscoveryMethod::Clob.is_firm());
        assert!(!PriceDiscoveryMethod::Indicative.is_firm());
        assert!(!PriceDiscoveryMethod::InterestGathering.is_firm());
        assert!(!PriceDiscoveryMethod::Theoretical.is_firm());
    }

    #[test]
    fn theoretical_price_creation() {
        let price = Price::new(100.0).unwrap();
        let theo = TheoreticalPrice::new(
            price,
            0.25,
            PriceDiscoveryMethod::Theoretical,
            0.8,
        );

        assert_eq!(theo.price(), price);
        assert_eq!(theo.implied_volatility(), 0.25);
        assert_eq!(theo.method(), PriceDiscoveryMethod::Theoretical);
        assert_eq!(theo.confidence(), 0.8);
        assert!(theo.is_reliable());
    }

    #[test]
    fn theoretical_price_clamps_confidence() {
        let price = Price::new(100.0).unwrap();
        
        let theo_high = TheoreticalPrice::new(
            price,
            0.25,
            PriceDiscoveryMethod::Theoretical,
            1.5,
        );
        assert_eq!(theo_high.confidence(), 1.0);

        let theo_low = TheoreticalPrice::new(
            price,
            0.25,
            PriceDiscoveryMethod::Theoretical,
            -0.5,
        );
        assert_eq!(theo_low.confidence(), 0.0);
    }

    #[test]
    fn theoretical_price_clamps_iv() {
        let price = Price::new(100.0).unwrap();
        let theo = TheoreticalPrice::new(
            price,
            -0.1,
            PriceDiscoveryMethod::Theoretical,
            0.8,
        );
        assert_eq!(theo.implied_volatility(), 0.0);
    }

    #[test]
    fn theoretical_price_reliability() {
        let price = Price::new(100.0).unwrap();
        
        let reliable = TheoreticalPrice::new(
            price,
            0.25,
            PriceDiscoveryMethod::Theoretical,
            0.5,
        );
        assert!(reliable.is_reliable());

        let unreliable = TheoreticalPrice::new(
            price,
            0.25,
            PriceDiscoveryMethod::Theoretical,
            0.49,
        );
        assert!(!unreliable.is_reliable());
    }
}
