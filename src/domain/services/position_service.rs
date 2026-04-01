//! # Position Update Service
//!
//! Service for updating counterparty positions after settlement.
//!
//! This module provides the [`PositionUpdateService`] trait for
//! updating positions following successful block trade settlement.

use crate::domain::entities::block_trade::BlockTrade;
use crate::domain::errors::DomainResult;
use crate::domain::services::settlement::SettlementResult;
use crate::domain::value_objects::CounterpartyId;
use async_trait::async_trait;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Position information for a counterparty.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Position {
    /// Counterparty ID.
    counterparty_id: CounterpartyId,
    /// Instrument symbol.
    instrument_symbol: String,
    /// Current position size (positive = long, negative = short).
    size: Decimal,
    /// Average entry price.
    average_price: Decimal,
    /// Unrealized PnL.
    unrealized_pnl: Decimal,
}

impl Position {
    /// Creates a new position.
    #[must_use]
    pub fn new(
        counterparty_id: CounterpartyId,
        instrument_symbol: String,
        size: Decimal,
        average_price: Decimal,
    ) -> Self {
        Self {
            counterparty_id,
            instrument_symbol,
            size,
            average_price,
            unrealized_pnl: Decimal::ZERO,
        }
    }

    /// Returns the counterparty ID.
    #[must_use]
    pub fn counterparty_id(&self) -> &CounterpartyId {
        &self.counterparty_id
    }

    /// Returns the instrument symbol.
    #[must_use]
    pub fn instrument_symbol(&self) -> &str {
        &self.instrument_symbol
    }

    /// Returns the position size.
    #[must_use]
    pub fn size(&self) -> Decimal {
        self.size
    }

    /// Returns the average entry price.
    #[must_use]
    pub fn average_price(&self) -> Decimal {
        self.average_price
    }

    /// Returns the unrealized PnL.
    #[must_use]
    pub fn unrealized_pnl(&self) -> Decimal {
        self.unrealized_pnl
    }

    /// Returns true if this is a long position.
    #[must_use]
    pub fn is_long(&self) -> bool {
        self.size > Decimal::ZERO
    }

    /// Returns true if this is a short position.
    #[must_use]
    pub fn is_short(&self) -> bool {
        self.size < Decimal::ZERO
    }

    /// Returns true if this position is flat (zero).
    #[must_use]
    pub fn is_flat(&self) -> bool {
        self.size == Decimal::ZERO
    }
}

/// Service for updating positions.
///
/// Implementations handle updating counterparty positions
/// after successful settlement.
#[async_trait]
pub trait PositionUpdateService: Send + Sync + fmt::Debug {
    /// Updates positions for both parties after settlement.
    ///
    /// This updates the buyer's and seller's positions based on
    /// the settlement result.
    ///
    /// # Arguments
    ///
    /// * `trade` - The block trade that was settled
    /// * `settlement` - The settlement result
    ///
    /// # Errors
    ///
    /// Returns `DomainError::PositionUpdateFailed` if the positions
    /// cannot be updated.
    async fn update_both(
        &self,
        trade: &BlockTrade,
        settlement: &SettlementResult,
    ) -> DomainResult<()>;

    /// Gets the current position for a counterparty and instrument.
    ///
    /// # Arguments
    ///
    /// * `counterparty_id` - The counterparty
    /// * `instrument_symbol` - The instrument symbol
    ///
    /// # Returns
    ///
    /// The current position, or None if no position exists.
    async fn get_position(
        &self,
        counterparty_id: &CounterpartyId,
        instrument_symbol: &str,
    ) -> DomainResult<Option<Position>>;
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn position_creation() {
        let position = Position::new(
            CounterpartyId::new("trader-1"),
            "BTC/USD".to_string(),
            Decimal::new(10, 0),
            Decimal::new(50000, 0),
        );

        assert_eq!(position.counterparty_id().as_str(), "trader-1");
        assert_eq!(position.instrument_symbol(), "BTC/USD");
        assert_eq!(position.size(), Decimal::new(10, 0));
        assert!(position.is_long());
        assert!(!position.is_short());
        assert!(!position.is_flat());
    }

    #[test]
    fn position_short() {
        let position = Position::new(
            CounterpartyId::new("trader-1"),
            "BTC/USD".to_string(),
            Decimal::new(-5, 0),
            Decimal::new(50000, 0),
        );

        assert!(position.is_short());
        assert!(!position.is_long());
    }

    #[test]
    fn position_flat() {
        let position = Position::new(
            CounterpartyId::new("trader-1"),
            "BTC/USD".to_string(),
            Decimal::ZERO,
            Decimal::ZERO,
        );

        assert!(position.is_flat());
    }
}
