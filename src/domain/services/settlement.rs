//! # Settlement Service
//!
//! Service for on-chain settlement of block trades.
//!
//! This module provides the [`SettlementService`] trait for executing
//! settlements on-chain with CRE (Chainlink) price verification.

use crate::domain::entities::block_trade::BlockTrade;
use crate::domain::errors::DomainResult;
use crate::domain::events::TradeHash;
use crate::domain::value_objects::Price;
use crate::domain::value_objects::timestamp::Timestamp;
use async_trait::async_trait;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Fees charged for the settlement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fees {
    /// Platform fee.
    pub platform_fee: Decimal,
    /// Network/gas fee.
    pub network_fee: Decimal,
}

impl Fees {
    /// Creates a new fees structure.
    #[must_use]
    pub fn new(platform_fee: Decimal, network_fee: Decimal) -> Self {
        Self {
            platform_fee,
            network_fee,
        }
    }

    /// Creates zero fees.
    #[must_use]
    pub fn zero() -> Self {
        Self {
            platform_fee: Decimal::ZERO,
            network_fee: Decimal::ZERO,
        }
    }

    /// Returns the total fees.
    #[must_use]
    pub fn total(&self) -> Decimal {
        self.platform_fee.saturating_add(self.network_fee)
    }
}

impl Default for Fees {
    fn default() -> Self {
        Self::zero()
    }
}

/// Result of a successful settlement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettlementResult {
    /// On-chain trade hash.
    trade_hash: TradeHash,
    /// When settlement completed.
    settlement_timestamp: Timestamp,
    /// Position change for buyer (positive = long).
    buyer_position_delta: Decimal,
    /// Position change for seller (negative = short).
    seller_position_delta: Decimal,
    /// Fees charged.
    fees: Fees,
}

impl SettlementResult {
    /// Creates a new settlement result.
    #[must_use]
    pub fn new(
        trade_hash: TradeHash,
        settlement_timestamp: Timestamp,
        buyer_position_delta: Decimal,
        seller_position_delta: Decimal,
        fees: Fees,
    ) -> Self {
        Self {
            trade_hash,
            settlement_timestamp,
            buyer_position_delta,
            seller_position_delta,
            fees,
        }
    }

    /// Returns the trade hash.
    #[must_use]
    pub fn trade_hash(&self) -> &TradeHash {
        &self.trade_hash
    }

    /// Returns the settlement timestamp.
    #[must_use]
    pub fn settlement_timestamp(&self) -> Timestamp {
        self.settlement_timestamp
    }

    /// Returns the buyer's position delta.
    #[must_use]
    pub fn buyer_position_delta(&self) -> Decimal {
        self.buyer_position_delta
    }

    /// Returns the seller's position delta.
    #[must_use]
    pub fn seller_position_delta(&self) -> Decimal {
        self.seller_position_delta
    }

    /// Returns the fees.
    #[must_use]
    pub fn fees(&self) -> &Fees {
        &self.fees
    }
}

/// Service for on-chain settlement.
///
/// Implementations handle the actual settlement of block trades
/// on the blockchain, including CRE price verification.
#[async_trait]
pub trait SettlementService: Send + Sync + fmt::Debug {
    /// Settles a block trade on-chain.
    ///
    /// This executes the settlement transaction, updating balances
    /// and positions for both counterparties.
    ///
    /// # Arguments
    ///
    /// * `trade` - The block trade to settle
    ///
    /// # Returns
    ///
    /// A `SettlementResult` containing the trade hash and settlement details.
    ///
    /// # Errors
    ///
    /// Returns `DomainError::SettlementFailed` if:
    /// - The on-chain transaction fails
    /// - Price bounds verification fails
    /// - Any other settlement error occurs
    async fn settle(&self, trade: &BlockTrade) -> DomainResult<SettlementResult>;

    /// Verifies that the trade price is within CRE (Chainlink) bounds.
    ///
    /// This check ensures the agreed price is within acceptable
    /// deviation from the oracle price.
    ///
    /// # Arguments
    ///
    /// * `trade` - The block trade to verify
    ///
    /// # Returns
    ///
    /// `true` if the price is within bounds, `false` otherwise.
    ///
    /// # Errors
    ///
    /// Returns `DomainError::PriceBoundsVerificationFailed` if the
    /// oracle cannot be reached or returns invalid data.
    async fn verify_price_bounds(&self, trade: &BlockTrade) -> DomainResult<bool>;

    /// Gets the current oracle price for an instrument.
    ///
    /// # Arguments
    ///
    /// * `instrument_symbol` - The instrument symbol (e.g., "BTC/USD")
    ///
    /// # Returns
    ///
    /// The current oracle price.
    async fn get_oracle_price(&self, instrument_symbol: &str) -> DomainResult<Price>;
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn fees_creation() {
        let fees = Fees::new(Decimal::new(100, 2), Decimal::new(50, 2));
        assert_eq!(fees.platform_fee, Decimal::new(100, 2));
        assert_eq!(fees.network_fee, Decimal::new(50, 2));
        assert_eq!(fees.total(), Decimal::new(150, 2));
    }

    #[test]
    fn fees_zero() {
        let fees = Fees::zero();
        assert_eq!(fees.total(), Decimal::ZERO);
    }

    #[test]
    fn settlement_result_creation() {
        let result = SettlementResult::new(
            TradeHash::new("0xabc123"),
            Timestamp::now(),
            Decimal::new(100, 0),
            Decimal::new(-100, 0),
            Fees::new(Decimal::new(10, 2), Decimal::new(5, 2)),
        );

        assert_eq!(result.trade_hash().as_str(), "0xabc123");
        assert_eq!(result.buyer_position_delta(), Decimal::new(100, 0));
        assert_eq!(result.seller_position_delta(), Decimal::new(-100, 0));
        assert_eq!(result.fees().total(), Decimal::new(15, 2));
    }
}
