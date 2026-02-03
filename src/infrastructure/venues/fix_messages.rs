//! # FIX Message Type Definitions
//!
//! Type-safe message builders and parsers for FIX 4.4 RFQ workflow messages.
//!
//! This module provides structured representations of FIX messages used in the
//! OTC RFQ workflow:
//!
//! - [`QuoteRequestBuilder`] - Build QuoteRequest (MsgType=R) messages
//! - [`QuoteMessage`] - Parse Quote (MsgType=S) messages
//! - [`NewOrderSingleBuilder`] - Build NewOrderSingle (MsgType=D) messages
//! - [`ExecutionReportMessage`] - Parse ExecutionReport (MsgType=8) messages
//!
//! # FIX 4.4 Reference
//!
//! See <https://www.fixtrading.org/standards/fix-4-4/> for the complete specification.
//!
//! # Example
//!
//! ```ignore
//! use otc_rfq::infrastructure::venues::fix_messages::QuoteRequestBuilder;
//!
//! let fields = QuoteRequestBuilder::new("QR-001", "BTC/USD")
//!     .side(Side::Buy)
//!     .quantity(100.0)
//!     .build();
//! ```

use crate::domain::value_objects::{OrderSide, Price, Quantity};
use rust_decimal::Decimal;
use std::collections::HashMap;

/// FIX message type constants (tag 35).
pub mod msg_type {
    /// QuoteRequest message type.
    pub const QUOTE_REQUEST: &str = "R";
    /// Quote message type.
    pub const QUOTE: &str = "S";
    /// NewOrderSingle message type.
    pub const NEW_ORDER_SINGLE: &str = "D";
    /// ExecutionReport message type.
    pub const EXECUTION_REPORT: &str = "8";
    /// Reject message type.
    pub const REJECT: &str = "3";
    /// QuoteRequestReject message type.
    pub const QUOTE_REQUEST_REJECT: &str = "AG";
    /// Heartbeat message type.
    pub const HEARTBEAT: &str = "0";
    /// Logon message type.
    pub const LOGON: &str = "A";
    /// Logout message type.
    pub const LOGOUT: &str = "5";
}

/// FIX field tag constants.
///
/// Reference: FIX 4.4 specification.
pub mod tags {
    /// MsgType (35) - Message type.
    pub const MSG_TYPE: u32 = 35;
    /// SenderCompID (49) - Sender identifier.
    pub const SENDER_COMP_ID: u32 = 49;
    /// TargetCompID (56) - Target identifier.
    pub const TARGET_COMP_ID: u32 = 56;
    /// MsgSeqNum (34) - Message sequence number.
    pub const MSG_SEQ_NUM: u32 = 34;
    /// SendingTime (52) - Time of message transmission.
    pub const SENDING_TIME: u32 = 52;

    /// QuoteReqID (131) - Quote request identifier.
    pub const QUOTE_REQ_ID: u32 = 131;
    /// QuoteID (117) - Quote identifier.
    pub const QUOTE_ID: u32 = 117;
    /// Symbol (55) - Instrument symbol.
    pub const SYMBOL: u32 = 55;
    /// Side (54) - Order side.
    pub const SIDE: u32 = 54;
    /// OrderQty (38) - Order quantity.
    pub const ORDER_QTY: u32 = 38;
    /// TransactTime (60) - Transaction time.
    pub const TRANSACT_TIME: u32 = 60;
    /// BidPx (132) - Bid price.
    pub const BID_PX: u32 = 132;
    /// OfferPx (133) - Offer price.
    pub const OFFER_PX: u32 = 133;
    /// BidSize (134) - Bid size.
    pub const BID_SIZE: u32 = 134;
    /// OfferSize (135) - Offer size.
    pub const OFFER_SIZE: u32 = 135;
    /// ValidUntilTime (62) - Quote validity time.
    pub const VALID_UNTIL_TIME: u32 = 62;
    /// ClOrdID (11) - Client order identifier.
    pub const CL_ORD_ID: u32 = 11;
    /// OrderID (37) - Order identifier.
    pub const ORDER_ID: u32 = 37;
    /// OrdType (40) - Order type.
    pub const ORD_TYPE: u32 = 40;
    /// Price (44) - Order price.
    pub const PRICE: u32 = 44;
    /// TimeInForce (59) - Time in force.
    pub const TIME_IN_FORCE: u32 = 59;
    /// ExecID (17) - Execution identifier.
    pub const EXEC_ID: u32 = 17;
    /// ExecType (150) - Execution type.
    pub const EXEC_TYPE: u32 = 150;
    /// OrdStatus (39) - Order status.
    pub const ORD_STATUS: u32 = 39;
    /// LastPx (31) - Last execution price.
    pub const LAST_PX: u32 = 31;
    /// LastQty (32) - Last execution quantity.
    pub const LAST_QTY: u32 = 32;
    /// LeavesQty (151) - Remaining quantity.
    pub const LEAVES_QTY: u32 = 151;
    /// CumQty (14) - Cumulative quantity.
    pub const CUM_QTY: u32 = 14;
    /// AvgPx (6) - Average price.
    pub const AVG_PX: u32 = 6;
    /// Text (58) - Free text field.
    pub const TEXT: u32 = 58;
    /// QuoteRejectReason (300) - Quote reject reason.
    pub const QUOTE_REJECT_REASON: u32 = 300;
    /// Account (1) - Account identifier.
    pub const ACCOUNT: u32 = 1;
    /// Currency (15) - Currency.
    pub const CURRENCY: u32 = 15;
    /// SettlDate (64) - Settlement date.
    pub const SETTL_DATE: u32 = 64;
}

/// FIX Side values (tag 54).
pub mod side_values {
    /// Buy side.
    pub const BUY: &str = "1";
    /// Sell side.
    pub const SELL: &str = "2";
}

/// FIX OrdType values (tag 40).
pub mod ord_type_values {
    /// Market order.
    pub const MARKET: &str = "1";
    /// Limit order.
    pub const LIMIT: &str = "2";
    /// Previously quoted order.
    pub const PREVIOUSLY_QUOTED: &str = "D";
}

/// FIX TimeInForce values (tag 59).
pub mod time_in_force_values {
    /// Day order.
    pub const DAY: &str = "0";
    /// Good Till Cancel.
    pub const GTC: &str = "1";
    /// Immediate or Cancel.
    pub const IOC: &str = "3";
    /// Fill or Kill.
    pub const FOK: &str = "4";
}

/// FIX ExecType values (tag 150).
pub mod exec_type_values {
    /// New execution.
    pub const NEW: &str = "0";
    /// Partial fill.
    pub const PARTIAL_FILL: &str = "1";
    /// Fill.
    pub const FILL: &str = "2";
    /// Done for day.
    pub const DONE_FOR_DAY: &str = "3";
    /// Canceled.
    pub const CANCELED: &str = "4";
    /// Replaced.
    pub const REPLACED: &str = "5";
    /// Pending cancel.
    pub const PENDING_CANCEL: &str = "6";
    /// Rejected.
    pub const REJECTED: &str = "8";
    /// Trade.
    pub const TRADE: &str = "F";
}

/// FIX OrdStatus values (tag 39).
pub mod ord_status_values {
    /// New order.
    pub const NEW: &str = "0";
    /// Partially filled.
    pub const PARTIALLY_FILLED: &str = "1";
    /// Filled.
    pub const FILLED: &str = "2";
    /// Done for day.
    pub const DONE_FOR_DAY: &str = "3";
    /// Canceled.
    pub const CANCELED: &str = "4";
    /// Replaced.
    pub const REPLACED: &str = "5";
    /// Pending cancel.
    pub const PENDING_CANCEL: &str = "6";
    /// Rejected.
    pub const REJECTED: &str = "8";
}

/// Converts domain OrderSide to FIX side value.
#[must_use]
pub fn order_side_to_fix(side: OrderSide) -> &'static str {
    match side {
        OrderSide::Buy => side_values::BUY,
        OrderSide::Sell => side_values::SELL,
    }
}

/// Converts FIX side value to domain OrderSide.
///
/// # Errors
///
/// Returns `None` if the side value is not recognized.
#[must_use]
pub fn fix_to_order_side(side: &str) -> Option<OrderSide> {
    match side {
        side_values::BUY => Some(OrderSide::Buy),
        side_values::SELL => Some(OrderSide::Sell),
        _ => None,
    }
}

/// A FIX field represented as a tag-value pair.
pub type FixField = (u32, String);

/// Builder for QuoteRequest (MsgType=R) messages.
///
/// # FIX 4.4 Reference
///
/// The QuoteRequest message is used to request a quote from a market maker.
///
/// # Required Fields
///
/// - QuoteReqID (131)
/// - Symbol (55)
/// - Side (54)
/// - OrderQty (38)
/// - TransactTime (60)
///
/// # Example
///
/// ```
/// use otc_rfq::infrastructure::venues::fix_messages::QuoteRequestBuilder;
/// use otc_rfq::domain::value_objects::OrderSide;
///
/// let fields = QuoteRequestBuilder::new("QR-001", "BTC/USD")
///     .side(OrderSide::Buy)
///     .quantity(100.0)
///     .build();
/// ```
#[derive(Debug, Clone)]
pub struct QuoteRequestBuilder {
    quote_req_id: String,
    symbol: String,
    side: Option<OrderSide>,
    quantity: Option<Decimal>,
    transact_time: Option<String>,
    account: Option<String>,
    currency: Option<String>,
}

impl QuoteRequestBuilder {
    /// Creates a new QuoteRequest builder.
    ///
    /// # Arguments
    ///
    /// * `quote_req_id` - Unique identifier for this quote request
    /// * `symbol` - Trading symbol (e.g., "BTC/USD")
    #[must_use]
    pub fn new(quote_req_id: impl Into<String>, symbol: impl Into<String>) -> Self {
        Self {
            quote_req_id: quote_req_id.into(),
            symbol: symbol.into(),
            side: None,
            quantity: None,
            transact_time: None,
            account: None,
            currency: None,
        }
    }

    /// Sets the order side.
    #[must_use]
    pub fn side(mut self, side: OrderSide) -> Self {
        self.side = Some(side);
        self
    }

    /// Sets the order quantity.
    #[must_use]
    pub fn quantity(mut self, quantity: impl Into<Decimal>) -> Self {
        self.quantity = Some(quantity.into());
        self
    }

    /// Sets the transaction time.
    ///
    /// Format: YYYYMMDD-HH:MM:SS.sss
    #[must_use]
    pub fn transact_time(mut self, time: impl Into<String>) -> Self {
        self.transact_time = Some(time.into());
        self
    }

    /// Sets the account.
    #[must_use]
    pub fn account(mut self, account: impl Into<String>) -> Self {
        self.account = Some(account.into());
        self
    }

    /// Sets the currency.
    #[must_use]
    pub fn currency(mut self, currency: impl Into<String>) -> Self {
        self.currency = Some(currency.into());
        self
    }

    /// Builds the QuoteRequest fields.
    ///
    /// Returns a vector of (tag, value) pairs ready for FIX encoding.
    #[must_use]
    pub fn build(self) -> Vec<FixField> {
        let mut fields = Vec::with_capacity(8);

        // MsgType
        fields.push((tags::MSG_TYPE, msg_type::QUOTE_REQUEST.to_string()));

        // Required fields
        fields.push((tags::QUOTE_REQ_ID, self.quote_req_id));
        fields.push((tags::SYMBOL, self.symbol));

        if let Some(side) = self.side {
            fields.push((tags::SIDE, order_side_to_fix(side).to_string()));
        }

        if let Some(qty) = self.quantity {
            fields.push((tags::ORDER_QTY, qty.to_string()));
        }

        // TransactTime - use current time if not provided
        let transact_time = self
            .transact_time
            .unwrap_or_else(|| chrono::Utc::now().format("%Y%m%d-%H:%M:%S%.3f").to_string());
        fields.push((tags::TRANSACT_TIME, transact_time));

        // Optional fields
        if let Some(account) = self.account {
            fields.push((tags::ACCOUNT, account));
        }

        if let Some(currency) = self.currency {
            fields.push((tags::CURRENCY, currency));
        }

        fields
    }
}

/// Parsed Quote (MsgType=S) message.
///
/// # FIX 4.4 Reference
///
/// The Quote message is sent by a market maker in response to a QuoteRequest.
#[derive(Debug, Clone)]
pub struct QuoteMessage {
    /// Quote request identifier (131).
    pub quote_req_id: String,
    /// Quote identifier (117).
    pub quote_id: String,
    /// Trading symbol (55).
    pub symbol: String,
    /// Bid price (132).
    pub bid_px: Option<Price>,
    /// Offer price (133).
    pub offer_px: Option<Price>,
    /// Bid size (134).
    pub bid_size: Option<Quantity>,
    /// Offer size (135).
    pub offer_size: Option<Quantity>,
    /// Quote validity time (62).
    pub valid_until_time: Option<String>,
    /// Free text (58).
    pub text: Option<String>,
}

impl QuoteMessage {
    /// Parses a Quote message from FIX fields.
    ///
    /// # Arguments
    ///
    /// * `fields` - Map of tag to value
    ///
    /// # Errors
    ///
    /// Returns `None` if required fields are missing.
    #[must_use]
    pub fn from_fields(fields: &HashMap<u32, String>) -> Option<Self> {
        let quote_req_id = fields.get(&tags::QUOTE_REQ_ID)?.clone();
        let quote_id = fields.get(&tags::QUOTE_ID)?.clone();
        let symbol = fields.get(&tags::SYMBOL)?.clone();

        let bid_px = fields
            .get(&tags::BID_PX)
            .and_then(|s| s.parse::<f64>().ok())
            .and_then(|v| Price::new(v).ok());

        let offer_px = fields
            .get(&tags::OFFER_PX)
            .and_then(|s| s.parse::<f64>().ok())
            .and_then(|v| Price::new(v).ok());

        let bid_size = fields
            .get(&tags::BID_SIZE)
            .and_then(|s| s.parse::<f64>().ok())
            .and_then(|v| Quantity::new(v).ok());

        let offer_size = fields
            .get(&tags::OFFER_SIZE)
            .and_then(|s| s.parse::<f64>().ok())
            .and_then(|v| Quantity::new(v).ok());

        let valid_until_time = fields.get(&tags::VALID_UNTIL_TIME).cloned();
        let text = fields.get(&tags::TEXT).cloned();

        Some(Self {
            quote_req_id,
            quote_id,
            symbol,
            bid_px,
            offer_px,
            bid_size,
            offer_size,
            valid_until_time,
            text,
        })
    }

    /// Returns the appropriate price based on the order side.
    ///
    /// - For Buy orders, returns the offer price (what you pay)
    /// - For Sell orders, returns the bid price (what you receive)
    #[must_use]
    pub fn price_for_side(&self, side: OrderSide) -> Option<Price> {
        match side {
            OrderSide::Buy => self.offer_px,
            OrderSide::Sell => self.bid_px,
        }
    }

    /// Returns the appropriate size based on the order side.
    #[must_use]
    pub fn size_for_side(&self, side: OrderSide) -> Option<Quantity> {
        match side {
            OrderSide::Buy => self.offer_size,
            OrderSide::Sell => self.bid_size,
        }
    }
}

/// Builder for NewOrderSingle (MsgType=D) messages.
///
/// # FIX 4.4 Reference
///
/// The NewOrderSingle message is used to submit a new order.
///
/// # Required Fields
///
/// - ClOrdID (11)
/// - Symbol (55)
/// - Side (54)
/// - OrderQty (38)
/// - OrdType (40)
/// - TransactTime (60)
///
/// # Example
///
/// ```
/// use otc_rfq::infrastructure::venues::fix_messages::NewOrderSingleBuilder;
/// use otc_rfq::domain::value_objects::OrderSide;
///
/// let fields = NewOrderSingleBuilder::new("ORD-001", "BTC/USD")
///     .side(OrderSide::Buy)
///     .quantity(100.0)
///     .previously_quoted("Q-123")
///     .price(50000.0)
///     .build();
/// ```
#[derive(Debug, Clone)]
pub struct NewOrderSingleBuilder {
    cl_ord_id: String,
    symbol: String,
    side: Option<OrderSide>,
    quantity: Option<Decimal>,
    ord_type: String,
    price: Option<Decimal>,
    time_in_force: String,
    quote_id: Option<String>,
    transact_time: Option<String>,
    account: Option<String>,
}

impl NewOrderSingleBuilder {
    /// Creates a new NewOrderSingle builder.
    ///
    /// # Arguments
    ///
    /// * `cl_ord_id` - Client order identifier
    /// * `symbol` - Trading symbol
    #[must_use]
    pub fn new(cl_ord_id: impl Into<String>, symbol: impl Into<String>) -> Self {
        Self {
            cl_ord_id: cl_ord_id.into(),
            symbol: symbol.into(),
            side: None,
            quantity: None,
            ord_type: ord_type_values::PREVIOUSLY_QUOTED.to_string(),
            price: None,
            time_in_force: time_in_force_values::IOC.to_string(),
            quote_id: None,
            transact_time: None,
            account: None,
        }
    }

    /// Sets the order side.
    #[must_use]
    pub fn side(mut self, side: OrderSide) -> Self {
        self.side = Some(side);
        self
    }

    /// Sets the order quantity.
    #[must_use]
    pub fn quantity(mut self, quantity: impl Into<Decimal>) -> Self {
        self.quantity = Some(quantity.into());
        self
    }

    /// Sets the order type to Previously Quoted and references the quote.
    #[must_use]
    pub fn previously_quoted(mut self, quote_id: impl Into<String>) -> Self {
        self.ord_type = ord_type_values::PREVIOUSLY_QUOTED.to_string();
        self.quote_id = Some(quote_id.into());
        self
    }

    /// Sets the order type to Limit.
    #[must_use]
    pub fn limit_order(mut self) -> Self {
        self.ord_type = ord_type_values::LIMIT.to_string();
        self
    }

    /// Sets the order type to Market.
    #[must_use]
    pub fn market_order(mut self) -> Self {
        self.ord_type = ord_type_values::MARKET.to_string();
        self
    }

    /// Sets the order price.
    #[must_use]
    pub fn price(mut self, price: impl Into<Decimal>) -> Self {
        self.price = Some(price.into());
        self
    }

    /// Sets the time in force to IOC (Immediate or Cancel).
    #[must_use]
    pub fn ioc(mut self) -> Self {
        self.time_in_force = time_in_force_values::IOC.to_string();
        self
    }

    /// Sets the time in force to FOK (Fill or Kill).
    #[must_use]
    pub fn fok(mut self) -> Self {
        self.time_in_force = time_in_force_values::FOK.to_string();
        self
    }

    /// Sets the transaction time.
    #[must_use]
    pub fn transact_time(mut self, time: impl Into<String>) -> Self {
        self.transact_time = Some(time.into());
        self
    }

    /// Sets the account.
    #[must_use]
    pub fn account(mut self, account: impl Into<String>) -> Self {
        self.account = Some(account.into());
        self
    }

    /// Builds the NewOrderSingle fields.
    #[must_use]
    pub fn build(self) -> Vec<FixField> {
        let mut fields = Vec::with_capacity(12);

        // MsgType
        fields.push((tags::MSG_TYPE, msg_type::NEW_ORDER_SINGLE.to_string()));

        // Required fields
        fields.push((tags::CL_ORD_ID, self.cl_ord_id));
        fields.push((tags::SYMBOL, self.symbol));

        if let Some(side) = self.side {
            fields.push((tags::SIDE, order_side_to_fix(side).to_string()));
        }

        if let Some(qty) = self.quantity {
            fields.push((tags::ORDER_QTY, qty.to_string()));
        }

        fields.push((tags::ORD_TYPE, self.ord_type));
        fields.push((tags::TIME_IN_FORCE, self.time_in_force));

        // Conditional fields
        if let Some(price) = self.price {
            fields.push((tags::PRICE, price.to_string()));
        }

        if let Some(quote_id) = self.quote_id {
            fields.push((tags::QUOTE_ID, quote_id));
        }

        // TransactTime
        let transact_time = self
            .transact_time
            .unwrap_or_else(|| chrono::Utc::now().format("%Y%m%d-%H:%M:%S%.3f").to_string());
        fields.push((tags::TRANSACT_TIME, transact_time));

        // Optional fields
        if let Some(account) = self.account {
            fields.push((tags::ACCOUNT, account));
        }

        fields
    }
}

/// Parsed ExecutionReport (MsgType=8) message.
///
/// # FIX 4.4 Reference
///
/// The ExecutionReport message is used to confirm order status and fills.
#[derive(Debug, Clone)]
pub struct ExecutionReportMessage {
    /// Execution identifier (17).
    pub exec_id: String,
    /// Client order identifier (11).
    pub cl_ord_id: String,
    /// Order identifier (37).
    pub order_id: String,
    /// Execution type (150).
    pub exec_type: String,
    /// Order status (39).
    pub ord_status: String,
    /// Trading symbol (55).
    pub symbol: String,
    /// Order side (54).
    pub side: Option<OrderSide>,
    /// Last execution price (31).
    pub last_px: Option<Price>,
    /// Last execution quantity (32).
    pub last_qty: Option<Quantity>,
    /// Remaining quantity (151).
    pub leaves_qty: Option<Quantity>,
    /// Cumulative quantity (14).
    pub cum_qty: Option<Quantity>,
    /// Average price (6).
    pub avg_px: Option<Price>,
    /// Free text (58).
    pub text: Option<String>,
}

impl ExecutionReportMessage {
    /// Parses an ExecutionReport message from FIX fields.
    ///
    /// # Arguments
    ///
    /// * `fields` - Map of tag to value
    ///
    /// # Errors
    ///
    /// Returns `None` if required fields are missing.
    #[must_use]
    pub fn from_fields(fields: &HashMap<u32, String>) -> Option<Self> {
        let exec_id = fields.get(&tags::EXEC_ID)?.clone();
        let cl_ord_id = fields.get(&tags::CL_ORD_ID)?.clone();
        let order_id = fields.get(&tags::ORDER_ID)?.clone();
        let exec_type = fields.get(&tags::EXEC_TYPE)?.clone();
        let ord_status = fields.get(&tags::ORD_STATUS)?.clone();
        let symbol = fields.get(&tags::SYMBOL)?.clone();

        let side = fields.get(&tags::SIDE).and_then(|s| fix_to_order_side(s));

        let last_px = fields
            .get(&tags::LAST_PX)
            .and_then(|s| s.parse::<f64>().ok())
            .and_then(|v| Price::new(v).ok());

        let last_qty = fields
            .get(&tags::LAST_QTY)
            .and_then(|s| s.parse::<f64>().ok())
            .and_then(|v| Quantity::new(v).ok());

        let leaves_qty = fields
            .get(&tags::LEAVES_QTY)
            .and_then(|s| s.parse::<f64>().ok())
            .and_then(|v| Quantity::new(v).ok());

        let cum_qty = fields
            .get(&tags::CUM_QTY)
            .and_then(|s| s.parse::<f64>().ok())
            .and_then(|v| Quantity::new(v).ok());

        let avg_px = fields
            .get(&tags::AVG_PX)
            .and_then(|s| s.parse::<f64>().ok())
            .and_then(|v| Price::new(v).ok());

        let text = fields.get(&tags::TEXT).cloned();

        Some(Self {
            exec_id,
            cl_ord_id,
            order_id,
            exec_type,
            ord_status,
            symbol,
            side,
            last_px,
            last_qty,
            leaves_qty,
            cum_qty,
            avg_px,
            text,
        })
    }

    /// Returns true if this is a fill execution.
    #[must_use]
    pub fn is_fill(&self) -> bool {
        self.exec_type == exec_type_values::FILL || self.exec_type == exec_type_values::TRADE
    }

    /// Returns true if this is a partial fill.
    #[must_use]
    pub fn is_partial_fill(&self) -> bool {
        self.exec_type == exec_type_values::PARTIAL_FILL
    }

    /// Returns true if this is a rejection.
    #[must_use]
    pub fn is_rejected(&self) -> bool {
        self.exec_type == exec_type_values::REJECTED
            || self.ord_status == ord_status_values::REJECTED
    }

    /// Returns true if the order is fully filled.
    #[must_use]
    pub fn is_fully_filled(&self) -> bool {
        self.ord_status == ord_status_values::FILLED
    }
}

/// Builder for Quote (MsgType=S) messages (for testing/simulation).
///
/// This is primarily used by the FIX simulator to generate quote responses.
#[derive(Debug, Clone)]
pub struct QuoteBuilder {
    quote_req_id: String,
    quote_id: String,
    symbol: String,
    bid_px: Option<Decimal>,
    offer_px: Option<Decimal>,
    bid_size: Option<Decimal>,
    offer_size: Option<Decimal>,
    valid_until_time: Option<String>,
}

impl QuoteBuilder {
    /// Creates a new Quote builder.
    #[must_use]
    pub fn new(
        quote_req_id: impl Into<String>,
        quote_id: impl Into<String>,
        symbol: impl Into<String>,
    ) -> Self {
        Self {
            quote_req_id: quote_req_id.into(),
            quote_id: quote_id.into(),
            symbol: symbol.into(),
            bid_px: None,
            offer_px: None,
            bid_size: None,
            offer_size: None,
            valid_until_time: None,
        }
    }

    /// Sets the bid price.
    #[must_use]
    pub fn bid_px(mut self, price: impl Into<Decimal>) -> Self {
        self.bid_px = Some(price.into());
        self
    }

    /// Sets the offer price.
    #[must_use]
    pub fn offer_px(mut self, price: impl Into<Decimal>) -> Self {
        self.offer_px = Some(price.into());
        self
    }

    /// Sets the bid size.
    #[must_use]
    pub fn bid_size(mut self, size: impl Into<Decimal>) -> Self {
        self.bid_size = Some(size.into());
        self
    }

    /// Sets the offer size.
    #[must_use]
    pub fn offer_size(mut self, size: impl Into<Decimal>) -> Self {
        self.offer_size = Some(size.into());
        self
    }

    /// Sets the quote validity time.
    #[must_use]
    pub fn valid_until_time(mut self, time: impl Into<String>) -> Self {
        self.valid_until_time = Some(time.into());
        self
    }

    /// Builds the Quote fields.
    #[must_use]
    pub fn build(self) -> Vec<FixField> {
        let mut fields = Vec::with_capacity(10);

        // MsgType
        fields.push((tags::MSG_TYPE, msg_type::QUOTE.to_string()));

        // Required fields
        fields.push((tags::QUOTE_REQ_ID, self.quote_req_id));
        fields.push((tags::QUOTE_ID, self.quote_id));
        fields.push((tags::SYMBOL, self.symbol));

        // Conditional fields
        if let Some(bid_px) = self.bid_px {
            fields.push((tags::BID_PX, bid_px.to_string()));
        }

        if let Some(offer_px) = self.offer_px {
            fields.push((tags::OFFER_PX, offer_px.to_string()));
        }

        if let Some(bid_size) = self.bid_size {
            fields.push((tags::BID_SIZE, bid_size.to_string()));
        }

        if let Some(offer_size) = self.offer_size {
            fields.push((tags::OFFER_SIZE, offer_size.to_string()));
        }

        if let Some(valid_until) = self.valid_until_time {
            fields.push((tags::VALID_UNTIL_TIME, valid_until));
        }

        fields
    }
}

/// Builder for ExecutionReport (MsgType=8) messages (for testing/simulation).
#[derive(Debug, Clone)]
pub struct ExecutionReportBuilder {
    exec_id: String,
    cl_ord_id: String,
    order_id: String,
    exec_type: String,
    ord_status: String,
    symbol: String,
    side: Option<OrderSide>,
    last_px: Option<Decimal>,
    last_qty: Option<Decimal>,
    leaves_qty: Option<Decimal>,
    cum_qty: Option<Decimal>,
    avg_px: Option<Decimal>,
    text: Option<String>,
}

impl ExecutionReportBuilder {
    /// Creates a new ExecutionReport builder.
    #[must_use]
    pub fn new(
        exec_id: impl Into<String>,
        cl_ord_id: impl Into<String>,
        order_id: impl Into<String>,
        symbol: impl Into<String>,
    ) -> Self {
        Self {
            exec_id: exec_id.into(),
            cl_ord_id: cl_ord_id.into(),
            order_id: order_id.into(),
            exec_type: exec_type_values::NEW.to_string(),
            ord_status: ord_status_values::NEW.to_string(),
            symbol: symbol.into(),
            side: None,
            last_px: None,
            last_qty: None,
            leaves_qty: None,
            cum_qty: None,
            avg_px: None,
            text: None,
        }
    }

    /// Sets the execution as a fill.
    #[must_use]
    pub fn fill(mut self, price: impl Into<Decimal>, quantity: impl Into<Decimal>) -> Self {
        self.exec_type = exec_type_values::FILL.to_string();
        self.ord_status = ord_status_values::FILLED.to_string();
        self.last_px = Some(price.into());
        self.last_qty = Some(quantity.into());
        self.leaves_qty = Some(Decimal::ZERO);
        self
    }

    /// Sets the execution as rejected.
    #[must_use]
    pub fn rejected(mut self, reason: impl Into<String>) -> Self {
        self.exec_type = exec_type_values::REJECTED.to_string();
        self.ord_status = ord_status_values::REJECTED.to_string();
        self.text = Some(reason.into());
        self
    }

    /// Sets the order side.
    #[must_use]
    pub fn side(mut self, side: OrderSide) -> Self {
        self.side = Some(side);
        self
    }

    /// Sets cumulative quantity.
    #[must_use]
    pub fn cum_qty(mut self, qty: impl Into<Decimal>) -> Self {
        self.cum_qty = Some(qty.into());
        self
    }

    /// Sets average price.
    #[must_use]
    pub fn avg_px(mut self, price: impl Into<Decimal>) -> Self {
        self.avg_px = Some(price.into());
        self
    }

    /// Builds the ExecutionReport fields.
    #[must_use]
    pub fn build(self) -> Vec<FixField> {
        let mut fields = Vec::with_capacity(15);

        // MsgType
        fields.push((tags::MSG_TYPE, msg_type::EXECUTION_REPORT.to_string()));

        // Required fields
        fields.push((tags::EXEC_ID, self.exec_id));
        fields.push((tags::CL_ORD_ID, self.cl_ord_id));
        fields.push((tags::ORDER_ID, self.order_id));
        fields.push((tags::EXEC_TYPE, self.exec_type));
        fields.push((tags::ORD_STATUS, self.ord_status));
        fields.push((tags::SYMBOL, self.symbol));

        // Optional fields
        if let Some(side) = self.side {
            fields.push((tags::SIDE, order_side_to_fix(side).to_string()));
        }

        if let Some(last_px) = self.last_px {
            fields.push((tags::LAST_PX, last_px.to_string()));
        }

        if let Some(last_qty) = self.last_qty {
            fields.push((tags::LAST_QTY, last_qty.to_string()));
        }

        if let Some(leaves_qty) = self.leaves_qty {
            fields.push((tags::LEAVES_QTY, leaves_qty.to_string()));
        }

        if let Some(cum_qty) = self.cum_qty {
            fields.push((tags::CUM_QTY, cum_qty.to_string()));
        }

        if let Some(avg_px) = self.avg_px {
            fields.push((tags::AVG_PX, avg_px.to_string()));
        }

        if let Some(text) = self.text {
            fields.push((tags::TEXT, text));
        }

        fields
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    mod quote_request_builder {
        use super::*;

        #[test]
        fn builds_basic_quote_request() {
            let fields = QuoteRequestBuilder::new("QR-001", "BTC/USD")
                .side(OrderSide::Buy)
                .quantity(Decimal::from(100))
                .build();

            assert!(
                fields
                    .iter()
                    .any(|(t, v)| *t == tags::MSG_TYPE && v == msg_type::QUOTE_REQUEST)
            );
            assert!(
                fields
                    .iter()
                    .any(|(t, v)| *t == tags::QUOTE_REQ_ID && v == "QR-001")
            );
            assert!(
                fields
                    .iter()
                    .any(|(t, v)| *t == tags::SYMBOL && v == "BTC/USD")
            );
            assert!(
                fields
                    .iter()
                    .any(|(t, v)| *t == tags::SIDE && v == side_values::BUY)
            );
            assert!(
                fields
                    .iter()
                    .any(|(t, v)| *t == tags::ORDER_QTY && v == "100")
            );
        }

        #[test]
        fn includes_transact_time() {
            let fields = QuoteRequestBuilder::new("QR-001", "BTC/USD").build();

            assert!(fields.iter().any(|(t, _)| *t == tags::TRANSACT_TIME));
        }

        #[test]
        fn includes_optional_fields() {
            let fields = QuoteRequestBuilder::new("QR-001", "BTC/USD")
                .account("ACC-001")
                .currency("USD")
                .build();

            assert!(
                fields
                    .iter()
                    .any(|(t, v)| *t == tags::ACCOUNT && v == "ACC-001")
            );
            assert!(
                fields
                    .iter()
                    .any(|(t, v)| *t == tags::CURRENCY && v == "USD")
            );
        }
    }

    mod quote_message {
        use super::*;
        use rust_decimal::prelude::ToPrimitive;

        fn sample_quote_fields() -> HashMap<u32, String> {
            let mut fields = HashMap::new();
            fields.insert(tags::QUOTE_REQ_ID, "QR-001".to_string());
            fields.insert(tags::QUOTE_ID, "Q-001".to_string());
            fields.insert(tags::SYMBOL, "BTC/USD".to_string());
            fields.insert(tags::BID_PX, "49900.50".to_string());
            fields.insert(tags::OFFER_PX, "50100.25".to_string());
            fields.insert(tags::BID_SIZE, "10".to_string());
            fields.insert(tags::OFFER_SIZE, "10".to_string());
            fields
        }

        #[test]
        fn parses_quote_message() {
            let fields = sample_quote_fields();
            let quote = QuoteMessage::from_fields(&fields).unwrap();

            assert_eq!(quote.quote_req_id, "QR-001");
            assert_eq!(quote.quote_id, "Q-001");
            assert_eq!(quote.symbol, "BTC/USD");
            assert!(quote.bid_px.is_some());
            assert!(quote.offer_px.is_some());
        }

        #[test]
        fn price_for_side_buy() {
            let fields = sample_quote_fields();
            let quote = QuoteMessage::from_fields(&fields).unwrap();

            let price = quote.price_for_side(OrderSide::Buy);
            assert!(price.is_some());
            // For buy, should return offer price
            assert!((price.unwrap().get().to_f64().unwrap() - 50100.25).abs() < 0.01);
        }

        #[test]
        fn price_for_side_sell() {
            let fields = sample_quote_fields();
            let quote = QuoteMessage::from_fields(&fields).unwrap();

            let price = quote.price_for_side(OrderSide::Sell);
            assert!(price.is_some());
            // For sell, should return bid price
            assert!((price.unwrap().get().to_f64().unwrap() - 49900.50).abs() < 0.01);
        }

        #[test]
        fn returns_none_for_missing_required_fields() {
            let mut fields = HashMap::new();
            fields.insert(tags::QUOTE_REQ_ID, "QR-001".to_string());
            // Missing QUOTE_ID and SYMBOL

            let quote = QuoteMessage::from_fields(&fields);
            assert!(quote.is_none());
        }
    }

    mod new_order_single_builder {
        use super::*;

        #[test]
        fn builds_previously_quoted_order() {
            let fields = NewOrderSingleBuilder::new("ORD-001", "BTC/USD")
                .side(OrderSide::Buy)
                .quantity(Decimal::from(100))
                .previously_quoted("Q-001")
                .price(Decimal::from(50000))
                .build();

            assert!(
                fields
                    .iter()
                    .any(|(t, v)| *t == tags::MSG_TYPE && v == msg_type::NEW_ORDER_SINGLE)
            );
            assert!(
                fields
                    .iter()
                    .any(|(t, v)| *t == tags::CL_ORD_ID && v == "ORD-001")
            );
            assert!(
                fields
                    .iter()
                    .any(|(t, v)| *t == tags::ORD_TYPE && v == ord_type_values::PREVIOUSLY_QUOTED)
            );
            assert!(
                fields
                    .iter()
                    .any(|(t, v)| *t == tags::QUOTE_ID && v == "Q-001")
            );
            assert!(
                fields
                    .iter()
                    .any(|(t, v)| *t == tags::PRICE && v == "50000")
            );
        }

        #[test]
        fn builds_limit_order() {
            let fields = NewOrderSingleBuilder::new("ORD-001", "BTC/USD")
                .side(OrderSide::Sell)
                .quantity(Decimal::from(50))
                .limit_order()
                .price(Decimal::from(51000))
                .fok()
                .build();

            assert!(
                fields
                    .iter()
                    .any(|(t, v)| *t == tags::ORD_TYPE && v == ord_type_values::LIMIT)
            );
            assert!(
                fields
                    .iter()
                    .any(|(t, v)| *t == tags::TIME_IN_FORCE && v == time_in_force_values::FOK)
            );
            assert!(
                fields
                    .iter()
                    .any(|(t, v)| *t == tags::SIDE && v == side_values::SELL)
            );
        }
    }

    mod execution_report_message {
        use super::*;

        fn sample_fill_fields() -> HashMap<u32, String> {
            let mut fields = HashMap::new();
            fields.insert(tags::EXEC_ID, "E-001".to_string());
            fields.insert(tags::CL_ORD_ID, "ORD-001".to_string());
            fields.insert(tags::ORDER_ID, "O-001".to_string());
            fields.insert(tags::EXEC_TYPE, exec_type_values::FILL.to_string());
            fields.insert(tags::ORD_STATUS, ord_status_values::FILLED.to_string());
            fields.insert(tags::SYMBOL, "BTC/USD".to_string());
            fields.insert(tags::SIDE, side_values::BUY.to_string());
            fields.insert(tags::LAST_PX, "50000".to_string());
            fields.insert(tags::LAST_QTY, "100".to_string());
            fields
        }

        #[test]
        fn parses_execution_report() {
            let fields = sample_fill_fields();
            let report = ExecutionReportMessage::from_fields(&fields).unwrap();

            assert_eq!(report.exec_id, "E-001");
            assert_eq!(report.cl_ord_id, "ORD-001");
            assert!(report.is_fill());
            assert!(report.is_fully_filled());
            assert!(!report.is_rejected());
        }

        #[test]
        fn detects_rejection() {
            let mut fields = sample_fill_fields();
            fields.insert(tags::EXEC_TYPE, exec_type_values::REJECTED.to_string());
            fields.insert(tags::ORD_STATUS, ord_status_values::REJECTED.to_string());

            let report = ExecutionReportMessage::from_fields(&fields).unwrap();
            assert!(report.is_rejected());
            assert!(!report.is_fill());
        }
    }

    mod quote_builder {
        use super::*;

        #[test]
        fn builds_quote() {
            let fields = QuoteBuilder::new("QR-001", "Q-001", "BTC/USD")
                .bid_px(Decimal::from(49900))
                .offer_px(Decimal::from(50100))
                .bid_size(Decimal::from(10))
                .offer_size(Decimal::from(10))
                .build();

            assert!(
                fields
                    .iter()
                    .any(|(t, v)| *t == tags::MSG_TYPE && v == msg_type::QUOTE)
            );
            assert!(
                fields
                    .iter()
                    .any(|(t, v)| *t == tags::QUOTE_REQ_ID && v == "QR-001")
            );
            assert!(
                fields
                    .iter()
                    .any(|(t, v)| *t == tags::QUOTE_ID && v == "Q-001")
            );
            assert!(
                fields
                    .iter()
                    .any(|(t, v)| *t == tags::BID_PX && v == "49900")
            );
            assert!(
                fields
                    .iter()
                    .any(|(t, v)| *t == tags::OFFER_PX && v == "50100")
            );
        }
    }

    mod execution_report_builder {
        use super::*;

        #[test]
        fn builds_fill_report() {
            let fields = ExecutionReportBuilder::new("E-001", "ORD-001", "O-001", "BTC/USD")
                .fill(Decimal::from(50000), Decimal::from(100))
                .side(OrderSide::Buy)
                .cum_qty(Decimal::from(100))
                .avg_px(Decimal::from(50000))
                .build();

            assert!(
                fields
                    .iter()
                    .any(|(t, v)| *t == tags::MSG_TYPE && v == msg_type::EXECUTION_REPORT)
            );
            assert!(
                fields
                    .iter()
                    .any(|(t, v)| *t == tags::EXEC_TYPE && v == exec_type_values::FILL)
            );
            assert!(
                fields
                    .iter()
                    .any(|(t, v)| *t == tags::ORD_STATUS && v == ord_status_values::FILLED)
            );
            assert!(
                fields
                    .iter()
                    .any(|(t, v)| *t == tags::LAST_PX && v == "50000")
            );
        }

        #[test]
        fn builds_rejected_report() {
            let fields = ExecutionReportBuilder::new("E-001", "ORD-001", "O-001", "BTC/USD")
                .rejected("Insufficient funds")
                .build();

            assert!(
                fields
                    .iter()
                    .any(|(t, v)| *t == tags::EXEC_TYPE && v == exec_type_values::REJECTED)
            );
            assert!(
                fields
                    .iter()
                    .any(|(t, v)| *t == tags::TEXT && v == "Insufficient funds")
            );
        }
    }

    mod side_conversion {
        use super::*;

        #[test]
        fn converts_order_side_to_fix() {
            assert_eq!(order_side_to_fix(OrderSide::Buy), side_values::BUY);
            assert_eq!(order_side_to_fix(OrderSide::Sell), side_values::SELL);
        }

        #[test]
        fn converts_fix_to_order_side() {
            assert_eq!(fix_to_order_side(side_values::BUY), Some(OrderSide::Buy));
            assert_eq!(fix_to_order_side(side_values::SELL), Some(OrderSide::Sell));
            assert_eq!(fix_to_order_side("X"), None);
        }
    }
}
