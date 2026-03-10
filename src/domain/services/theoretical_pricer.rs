//! # Theoretical Pricer
//!
//! Computes theoretical prices for illiquid instruments using Black-Scholes
//! pricing with IV interpolation.

use crate::domain::errors::DomainError;
use crate::domain::value_objects::{Price, PriceDiscoveryMethod, TheoreticalPrice};
use rust_decimal::prelude::*;

/// Theoretical pricer for computing prices using Black-Scholes + IV interpolation.
#[derive(Debug, Clone)]
pub struct TheoreticalPricer;

impl TheoreticalPricer {
    /// Creates a new theoretical pricer.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Computes a theoretical price using Black-Scholes with IV interpolation.
    ///
    /// # Arguments
    ///
    /// * `underlying_price` - Current price of the underlying asset
    /// * `strike` - Strike price of the option
    /// * `time_to_expiry` - Time to expiry in years
    /// * `risk_free_rate` - Risk-free interest rate (annualized)
    /// * `nearby_ivs` - Reference IVs from nearby strikes [(strike, iv)]
    /// * `is_call` - True for call option, false for put
    ///
    /// # Returns
    ///
    /// Theoretical price with confidence score.
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Insufficient reference IVs (need at least 2)
    /// - Invalid input parameters (negative prices, etc.)
    pub fn compute(
        &self,
        underlying_price: Price,
        strike: Price,
        time_to_expiry: f64,
        risk_free_rate: f64,
        nearby_ivs: &[(Price, f64)],
        is_call: bool,
    ) -> Result<TheoreticalPrice, DomainError> {
        // Validate inputs
        if time_to_expiry <= 0.0 {
            return Err(DomainError::ValidationError(
                "Time to expiry must be positive".to_string(),
            ));
        }

        if nearby_ivs.len() < 2 {
            return Err(DomainError::ValidationError(
                "Need at least 2 reference IVs for interpolation".to_string(),
            ));
        }

        // Interpolate IV for the target strike
        let iv = Self::interpolate_iv(strike, nearby_ivs)?;

        // Convert Decimal to f64 for Black-Scholes calculation
        let underlying_f64 = underlying_price.get().to_f64().ok_or_else(|| {
            DomainError::ValidationError("Failed to convert underlying price to f64".to_string())
        })?;
        let strike_f64 = strike.get().to_f64().ok_or_else(|| {
            DomainError::ValidationError("Failed to convert strike to f64".to_string())
        })?;

        // Compute Black-Scholes price
        let price_value = Self::black_scholes(
            underlying_f64,
            strike_f64,
            time_to_expiry,
            risk_free_rate,
            iv,
            is_call,
        )?;

        let price = Price::new(price_value).map_err(|e| {
            DomainError::ValidationError(format!("Invalid theoretical price: {}", e))
        })?;

        // Compute confidence score
        let confidence = Self::compute_confidence(strike, nearby_ivs, time_to_expiry);

        Ok(TheoreticalPrice::new(
            price,
            iv,
            PriceDiscoveryMethod::Theoretical,
            confidence,
        ))
    }

    /// Interpolates implied volatility for a target strike.
    ///
    /// Uses linear interpolation between the two nearest strikes.
    /// For strikes outside the range, uses the nearest IV with a decay factor.
    fn interpolate_iv(strike: Price, nearby_ivs: &[(Price, f64)]) -> Result<f64, DomainError> {
        if nearby_ivs.is_empty() {
            return Err(DomainError::ValidationError(
                "No reference IVs provided".to_string(),
            ));
        }

        // Sort by distance from target strike
        let mut sorted: Vec<_> = nearby_ivs
            .iter()
            .map(|(s, iv)| {
                let s_f64 = s.get().to_f64().unwrap_or(0.0);
                let strike_f64 = strike.get().to_f64().unwrap_or(0.0);
                let distance = (s_f64 - strike_f64).abs();
                (distance, *s, *iv)
            })
            .collect();
        sorted.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

        if sorted.len() == 1 {
            // Only one reference point - use it directly
            return Ok(sorted[0].2.max(0.01)); // Minimum 1% IV
        }

        let (_, s1, iv1) = sorted[0];
        let (_, s2, iv2) = sorted[1];

        // Check if we're between the two nearest points
        let strike_val = strike.get().to_f64().unwrap_or(0.0);
        let s1_val = s1.get().to_f64().unwrap_or(0.0);
        let s2_val = s2.get().to_f64().unwrap_or(0.0);

        let (lower, upper, lower_iv, upper_iv) = if s1_val < s2_val {
            (s1_val, s2_val, iv1, iv2)
        } else {
            (s2_val, s1_val, iv2, iv1)
        };

        let iv = if strike_val >= lower && strike_val <= upper {
            // Linear interpolation
            let weight = (strike_val - lower) / (upper - lower);
            lower_iv + weight * (upper_iv - lower_iv)
        } else {
            // Extrapolation - use nearest IV with slight adjustment
            if strike_val < lower {
                // Below range - use lower IV
                lower_iv
            } else {
                // Above range - use upper IV
                upper_iv
            }
        };

        Ok(iv.max(0.01)) // Minimum 1% IV
    }

    /// Computes Black-Scholes option price.
    ///
    /// Simplified Black-Scholes formula for European options.
    #[allow(clippy::too_many_arguments)]
    fn black_scholes(
        spot: f64,
        strike: f64,
        time: f64,
        rate: f64,
        volatility: f64,
        is_call: bool,
    ) -> Result<f64, DomainError> {
        if spot <= 0.0 || strike <= 0.0 || time <= 0.0 || volatility <= 0.0 {
            return Err(DomainError::ValidationError(
                "All Black-Scholes parameters must be positive".to_string(),
            ));
        }

        let sqrt_time = time.sqrt();
        let vol_sqrt_time = volatility * sqrt_time;

        if vol_sqrt_time == 0.0 {
            // Zero volatility edge case
            return Ok(if is_call {
                (spot - strike * (-rate * time).exp()).max(0.0)
            } else {
                (strike * (-rate * time).exp() - spot).max(0.0)
            });
        }

        let d1 =
            ((spot / strike).ln() + (rate + 0.5 * volatility * volatility) * time) / vol_sqrt_time;
        let d2 = d1 - vol_sqrt_time;

        let price = if is_call {
            spot * Self::norm_cdf(d1) - strike * (-rate * time).exp() * Self::norm_cdf(d2)
        } else {
            strike * (-rate * time).exp() * Self::norm_cdf(-d2) - spot * Self::norm_cdf(-d1)
        };

        Ok(price.max(0.0))
    }

    /// Standard normal cumulative distribution function.
    ///
    /// Approximation using the error function.
    fn norm_cdf(x: f64) -> f64 {
        0.5 * (1.0 + Self::erf(x / std::f64::consts::SQRT_2))
    }

    /// Error function approximation.
    ///
    /// Uses Abramowitz and Stegun approximation (maximum error: 1.5e-7).
    fn erf(x: f64) -> f64 {
        let sign = if x >= 0.0 { 1.0 } else { -1.0 };
        let x = x.abs();

        let a1 = 0.254_829_592;
        let a2 = -0.284_496_736;
        let a3 = 1.421_413_741;
        let a4 = -1.453_152_027;
        let a5 = 1.061_405_429;
        let p = 0.327_591_1;

        let t = 1.0 / (1.0 + p * x);
        let t2 = t * t;
        let t3 = t2 * t;
        let t4 = t3 * t;
        let t5 = t4 * t;

        sign * (1.0 - (a1 * t + a2 * t2 + a3 * t3 + a4 * t4 + a5 * t5) * (-x * x).exp())
    }

    /// Computes confidence score for the theoretical price.
    ///
    /// Confidence is based on:
    /// - Distance from reference strikes (closer = higher confidence)
    /// - Time to expiry (shorter = higher confidence)
    /// - Number of reference points (more = higher confidence)
    fn compute_confidence(strike: Price, nearby_ivs: &[(Price, f64)], time_to_expiry: f64) -> f64 {
        // Distance penalty - how far is the strike from nearest reference?
        let strike_f64 = strike.get().to_f64().unwrap_or(0.0);
        let min_distance = nearby_ivs
            .iter()
            .map(|(s, _)| {
                let s_f64 = s.get().to_f64().unwrap_or(0.0);
                (s_f64 - strike_f64).abs()
            })
            .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(f64::MAX);

        let distance_score = if min_distance == 0.0 {
            1.0
        } else {
            1.0 / (1.0 + min_distance / strike_f64)
        };

        // Time penalty - longer expiries are less predictable
        // Decay with half-life of 1 year
        let time_score = (-time_to_expiry / 365.0).exp();

        // Data quality - more reference points = higher confidence
        let data_score = (nearby_ivs.len() as f64 / 10.0).min(1.0);

        // Combined score
        (distance_score * time_score * data_score).clamp(0.0, 1.0)
    }
}

impl Default for TheoreticalPricer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interpolate_iv_between_two_points() {
        let nearby = vec![
            (Price::new(100.0).unwrap(), 0.20),
            (Price::new(110.0).unwrap(), 0.25),
        ];

        // Strike exactly between the two points
        let strike = Price::new(105.0).unwrap();
        let iv = TheoreticalPricer::interpolate_iv(strike, &nearby).unwrap();

        // Should be midpoint: (0.20 + 0.25) / 2 = 0.225
        assert!((iv - 0.225).abs() < 0.001);
    }

    #[test]
    fn interpolate_iv_at_reference_point() {
        let nearby = vec![
            (Price::new(100.0).unwrap(), 0.20),
            (Price::new(110.0).unwrap(), 0.25),
        ];

        let strike = Price::new(100.0).unwrap();
        let iv = TheoreticalPricer::interpolate_iv(strike, &nearby).unwrap();

        assert!((iv - 0.20).abs() < 0.001);
    }

    #[test]
    fn interpolate_iv_extrapolation_below() {
        let nearby = vec![
            (Price::new(100.0).unwrap(), 0.20),
            (Price::new(110.0).unwrap(), 0.25),
        ];

        let strike = Price::new(90.0).unwrap();
        let iv = TheoreticalPricer::interpolate_iv(strike, &nearby).unwrap();

        // Should use nearest (lower) IV
        assert!((iv - 0.20).abs() < 0.001);
    }

    #[test]
    fn interpolate_iv_extrapolation_above() {
        let nearby = vec![
            (Price::new(100.0).unwrap(), 0.20),
            (Price::new(110.0).unwrap(), 0.25),
        ];

        let strike = Price::new(120.0).unwrap();
        let iv = TheoreticalPricer::interpolate_iv(strike, &nearby).unwrap();

        // Should use nearest (upper) IV
        assert!((iv - 0.25).abs() < 0.001);
    }

    #[test]
    fn interpolate_iv_minimum_floor() {
        let nearby = vec![(Price::new(100.0).unwrap(), -0.05)];

        let strike = Price::new(100.0).unwrap();
        let iv = TheoreticalPricer::interpolate_iv(strike, &nearby).unwrap();

        // Should be clamped to minimum 1%
        assert_eq!(iv, 0.01);
    }

    #[test]
    fn black_scholes_call_atm() {
        // ATM call: spot = strike = 100, 1 year, 5% rate, 20% vol
        let price = TheoreticalPricer::black_scholes(100.0, 100.0, 1.0, 0.05, 0.20, true).unwrap();

        // ATM call should be roughly 8-10 for these parameters
        assert!(price > 7.0 && price < 11.0);
    }

    #[test]
    fn black_scholes_put_atm() {
        // ATM put: spot = strike = 100, 1 year, 5% rate, 20% vol
        let price = TheoreticalPricer::black_scholes(100.0, 100.0, 1.0, 0.05, 0.20, false).unwrap();

        // ATM put should be similar to call but slightly less due to drift
        assert!(price > 5.0 && price < 9.0);
    }

    #[test]
    fn black_scholes_deep_itm_call() {
        // Deep ITM call: spot = 120, strike = 100
        let price = TheoreticalPricer::black_scholes(120.0, 100.0, 1.0, 0.05, 0.20, true).unwrap();

        // Should be at least intrinsic value (20)
        assert!(price >= 20.0);
    }

    #[test]
    fn black_scholes_deep_otm_call() {
        // Deep OTM call: spot = 80, strike = 100
        let price = TheoreticalPricer::black_scholes(80.0, 100.0, 1.0, 0.05, 0.20, true).unwrap();

        // Should be small but positive
        assert!(price > 0.0 && price < 5.0);
    }

    #[test]
    fn compute_full_theoretical_price() {
        let pricer = TheoreticalPricer::new();

        let underlying = Price::new(100.0).unwrap();
        let strike = Price::new(105.0).unwrap();
        let nearby_ivs = vec![
            (Price::new(100.0).unwrap(), 0.20),
            (Price::new(110.0).unwrap(), 0.25),
        ];

        let result = pricer
            .compute(underlying, strike, 1.0, 0.05, &nearby_ivs, true)
            .unwrap();

        assert!(result.price().get() > Decimal::ZERO);
        assert!(result.implied_volatility() > 0.20 && result.implied_volatility() < 0.25);
        assert!(result.confidence() > 0.0 && result.confidence() <= 1.0);
        assert_eq!(result.method(), PriceDiscoveryMethod::Theoretical);
    }

    #[test]
    fn compute_requires_positive_time() {
        let pricer = TheoreticalPricer::new();

        let underlying = Price::new(100.0).unwrap();
        let strike = Price::new(100.0).unwrap();
        let nearby_ivs = vec![(Price::new(100.0).unwrap(), 0.20)];

        let result = pricer.compute(underlying, strike, 0.0, 0.05, &nearby_ivs, true);
        assert!(result.is_err());
    }

    #[test]
    fn compute_requires_sufficient_ivs() {
        let pricer = TheoreticalPricer::new();

        let underlying = Price::new(100.0).unwrap();
        let strike = Price::new(100.0).unwrap();
        let nearby_ivs = vec![(Price::new(100.0).unwrap(), 0.20)];

        let result = pricer.compute(underlying, strike, 1.0, 0.05, &nearby_ivs, true);
        assert!(result.is_err());
    }

    #[test]
    fn confidence_score_at_reference_strike() {
        let strike = Price::new(100.0).unwrap();
        let nearby_ivs = vec![
            (Price::new(100.0).unwrap(), 0.20),
            (Price::new(110.0).unwrap(), 0.25),
        ];

        let confidence = TheoreticalPricer::compute_confidence(strike, &nearby_ivs, 30.0 / 365.0);

        // At reference strike with short expiry, should have reasonable confidence
        assert!(confidence > 0.1 && confidence <= 1.0);
    }

    #[test]
    fn confidence_score_decreases_with_distance() {
        let nearby_ivs = vec![
            (Price::new(100.0).unwrap(), 0.20),
            (Price::new(110.0).unwrap(), 0.25),
        ];

        let close_strike = Price::new(105.0).unwrap();
        let far_strike = Price::new(150.0).unwrap();

        let close_confidence =
            TheoreticalPricer::compute_confidence(close_strike, &nearby_ivs, 30.0 / 365.0);
        let far_confidence =
            TheoreticalPricer::compute_confidence(far_strike, &nearby_ivs, 30.0 / 365.0);

        assert!(close_confidence > far_confidence);
    }

    #[test]
    fn confidence_score_decreases_with_time() {
        let strike = Price::new(105.0).unwrap();
        let nearby_ivs = vec![
            (Price::new(100.0).unwrap(), 0.20),
            (Price::new(110.0).unwrap(), 0.25),
        ];

        let short_time_confidence =
            TheoreticalPricer::compute_confidence(strike, &nearby_ivs, 30.0 / 365.0);
        let long_time_confidence =
            TheoreticalPricer::compute_confidence(strike, &nearby_ivs, 365.0);

        assert!(short_time_confidence > long_time_confidence);
    }
}
