//! # Trade Type
//!
//! Enumeration of trade types supported by the fee engine.
//!
//! This module provides the [`TradeType`] enum which distinguishes between
//! RFQ (Request for Quote) trades and Block trades for fee calculation purposes.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Type of trade for fee calculation purposes.
///
/// Different trade types have different fee structures:
/// - **RFQ**: Request for Quote trades with taker/maker differentiation
/// - **Block**: Pre-arranged bilateral block trades
///
/// # Examples
///
/// ```
/// use otc_rfq::domain::value_objects::TradeType;
///
/// let trade_type = TradeType::Rfq;
/// assert_eq!(trade_type.to_string(), "RFQ");
///
/// let block = TradeType::Block;
/// assert_eq!(block.to_string(), "BLOCK");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TradeType {
    /// Request for Quote trade.
    ///
    /// RFQ trades typically have asymmetric fees:
    /// - Taker pays a fee
    /// - Maker may receive a rebate (negative fee)
    #[default]
    Rfq,

    /// Block trade.
    ///
    /// Block trades are pre-arranged bilateral trades with
    /// symmetric fees for both parties.
    Block,
}

impl TradeType {
    /// Returns true if this is an RFQ trade.
    #[inline]
    #[must_use]
    pub const fn is_rfq(self) -> bool {
        matches!(self, Self::Rfq)
    }

    /// Returns true if this is a block trade.
    #[inline]
    #[must_use]
    pub const fn is_block(self) -> bool {
        matches!(self, Self::Block)
    }
}

impl fmt::Display for TradeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rfq => write!(f, "RFQ"),
            Self::Block => write!(f, "BLOCK"),
        }
    }
}
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn trade_type_display() {
        assert_eq!(TradeType::Rfq.to_string(), "RFQ");
        assert_eq!(TradeType::Block.to_string(), "BLOCK");
    }

    #[test]
    fn trade_type_is_rfq() {
        assert!(TradeType::Rfq.is_rfq());
        assert!(!TradeType::Block.is_rfq());
    }

    #[test]
    fn trade_type_is_block() {
        assert!(TradeType::Block.is_block());
        assert!(!TradeType::Rfq.is_block());
    }

    #[test]
    fn trade_type_default() {
        assert_eq!(TradeType::default(), TradeType::Rfq);
    }

    #[test]
    fn trade_type_serialization() {
        let rfq = TradeType::Rfq;
        let json = serde_json::to_string(&rfq).unwrap();
        assert_eq!(json, "\"RFQ\"");

        let block = TradeType::Block;
        let json = serde_json::to_string(&block).unwrap();
        assert_eq!(json, "\"BLOCK\"");
    }

    #[test]
    fn trade_type_deserialization() {
        let rfq: TradeType = serde_json::from_str("\"RFQ\"").unwrap();
        assert_eq!(rfq, TradeType::Rfq);

        let block: TradeType = serde_json::from_str("\"BLOCK\"").unwrap();
        assert_eq!(block, TradeType::Block);
    }
}
