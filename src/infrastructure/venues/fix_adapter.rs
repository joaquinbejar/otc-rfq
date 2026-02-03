//! # FIX Market Maker Adapter
//!
//! Adapter for FIX protocol market makers using IronFix.
//!
//! This module provides the [`FixMMAdapter`] which implements the
//! [`VenueAdapter`] trait for FIX protocol market makers.
//!
//! # Features
//!
//! - FIX 4.4 protocol support
//! - QuoteRequest (R) message building
//! - Quote (S) message parsing
//! - NewOrderSingle (D) message for execution
//! - ExecutionReport (8) handling
//! - Pending quote tracking with response channels
//! - Configurable timeouts
//!
//! # Note
//!
//! This is a stub implementation. Full IronFix integration will be
//! completed when the IronFix library is ready.
//!
//! # Examples
//!
//! ```ignore
//! use otc_rfq::infrastructure::venues::fix_adapter::FixMMAdapter;
//! use otc_rfq::infrastructure::venues::fix_config::{FixMMConfig, FixSessionConfig};
//!
//! let session = FixSessionConfig::new("OTC_PLATFORM", "MARKET_MAKER")
//!     .with_host("fix.example.com")
//!     .with_port(9876);
//!
//! let config = FixMMConfig::new("fix-mm-1", session);
//! let adapter = FixMMAdapter::new(config);
//! ```

use crate::domain::entities::quote::{Quote, QuoteBuilder};
use crate::domain::entities::rfq::Rfq;
use crate::domain::value_objects::timestamp::Timestamp;
use crate::domain::value_objects::{OrderSide, Price, QuoteId, SettlementMethod, VenueId};
use crate::infrastructure::venues::error::{VenueError, VenueResult};
use crate::infrastructure::venues::fix_config::FixMMConfig;
use crate::infrastructure::venues::traits::{ExecutionResult, VenueAdapter, VenueHealth};
use async_trait::async_trait;
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::{RwLock, oneshot};

/// FIX message type constants.
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
}

/// FIX field tag constants.
pub mod tags {
    /// QuoteReqID (131).
    pub const QUOTE_REQ_ID: u32 = 131;
    /// QuoteID (117).
    pub const QUOTE_ID: u32 = 117;
    /// Symbol (55).
    pub const SYMBOL: u32 = 55;
    /// Side (54).
    pub const SIDE: u32 = 54;
    /// OrderQty (38).
    pub const ORDER_QTY: u32 = 38;
    /// TransactTime (60).
    pub const TRANSACT_TIME: u32 = 60;
    /// BidPx (132).
    pub const BID_PX: u32 = 132;
    /// OfferPx (133).
    pub const OFFER_PX: u32 = 133;
    /// BidSize (134).
    pub const BID_SIZE: u32 = 134;
    /// OfferSize (135).
    pub const OFFER_SIZE: u32 = 135;
    /// ValidUntilTime (62).
    pub const VALID_UNTIL_TIME: u32 = 62;
    /// ClOrdID (11).
    pub const CL_ORD_ID: u32 = 11;
    /// OrdType (40).
    pub const ORD_TYPE: u32 = 40;
    /// Price (44).
    pub const PRICE: u32 = 44;
    /// TimeInForce (59).
    pub const TIME_IN_FORCE: u32 = 59;
    /// ExecID (17).
    pub const EXEC_ID: u32 = 17;
    /// ExecType (150).
    pub const EXEC_TYPE: u32 = 150;
    /// OrdStatus (39).
    pub const ORD_STATUS: u32 = 39;
    /// LastPx (31).
    pub const LAST_PX: u32 = 31;
    /// LastQty (32).
    pub const LAST_QTY: u32 = 32;
    /// Text (58).
    pub const TEXT: u32 = 58;
}

/// FIX side values.
pub mod side {
    /// Buy side.
    pub const BUY: &str = "1";
    /// Sell side.
    pub const SELL: &str = "2";
}

/// FIX order type values.
pub mod ord_type {
    /// Previously quoted order type.
    pub const PREVIOUSLY_QUOTED: &str = "D";
}

/// FIX time in force values.
pub mod time_in_force {
    /// Immediate or Cancel.
    pub const IOC: &str = "3";
    /// Fill or Kill.
    pub const FOK: &str = "4";
}

/// FIX execution type values.
pub mod exec_type {
    /// New execution.
    pub const NEW: &str = "0";
    /// Fill execution.
    pub const FILL: &str = "F";
    /// Rejected execution.
    pub const REJECTED: &str = "8";
}

/// FIX order status values.
pub mod ord_status {
    /// New order.
    pub const NEW: &str = "0";
    /// Filled order.
    pub const FILLED: &str = "2";
    /// Rejected order.
    pub const REJECTED: &str = "8";
}

/// Session state for the FIX connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    /// Disconnected from the counterparty.
    Disconnected,
    /// Connecting to the counterparty.
    Connecting,
    /// Connected but not logged on.
    Connected,
    /// Logging on to the session.
    LoggingOn,
    /// Logged on and active.
    LoggedOn,
    /// Logging out from the session.
    LoggingOut,
}

impl fmt::Display for SessionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Disconnected => write!(f, "DISCONNECTED"),
            Self::Connecting => write!(f, "CONNECTING"),
            Self::Connected => write!(f, "CONNECTED"),
            Self::LoggingOn => write!(f, "LOGGING_ON"),
            Self::LoggedOn => write!(f, "LOGGED_ON"),
            Self::LoggingOut => write!(f, "LOGGING_OUT"),
        }
    }
}

/// Pending quote request awaiting response.
#[allow(dead_code)]
struct PendingQuoteRequest {
    /// Channel to send the response.
    response_tx: oneshot::Sender<VenueResult<Quote>>,
    /// The original RFQ.
    rfq_id: crate::domain::value_objects::RfqId,
    /// When the request was sent.
    sent_at: Timestamp,
}

/// Pending order awaiting execution report.
#[allow(dead_code)]
struct PendingOrder {
    /// Channel to send the response.
    response_tx: oneshot::Sender<VenueResult<ExecutionResult>>,
    /// The quote being executed.
    quote_id: QuoteId,
    /// When the order was sent.
    sent_at: Timestamp,
}

/// FIX Market Maker adapter.
///
/// Implements the [`VenueAdapter`] trait for FIX protocol market makers.
///
/// # Note
///
/// This is a stub implementation. The actual FIX engine integration
/// (IronFix) will be added when the library is ready.
pub struct FixMMAdapter {
    /// Configuration.
    config: FixMMConfig,
    /// Current session state.
    session_state: Arc<RwLock<SessionState>>,
    /// Sequence number counter.
    seq_num: AtomicU64,
    /// Pending quote requests.
    pending_quotes: Arc<RwLock<HashMap<String, PendingQuoteRequest>>>,
    /// Pending orders.
    pending_orders: Arc<RwLock<HashMap<String, PendingOrder>>>,
}

impl FixMMAdapter {
    /// Creates a new FIX MM adapter.
    #[must_use]
    pub fn new(config: FixMMConfig) -> Self {
        Self {
            config,
            session_state: Arc::new(RwLock::new(SessionState::Disconnected)),
            seq_num: AtomicU64::new(1),
            pending_quotes: Arc::new(RwLock::new(HashMap::new())),
            pending_orders: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Returns the configuration.
    #[inline]
    #[must_use]
    pub fn config(&self) -> &FixMMConfig {
        &self.config
    }

    /// Returns the current session state.
    pub async fn session_state(&self) -> SessionState {
        *self.session_state.read().await
    }

    /// Gets the next sequence number.
    fn next_seq_num(&self) -> u64 {
        self.seq_num.fetch_add(1, Ordering::SeqCst)
    }

    /// Generates a unique quote request ID.
    fn generate_quote_req_id(&self, rfq: &Rfq) -> String {
        format!("QR-{}-{}", rfq.id(), self.next_seq_num())
    }

    /// Generates a unique client order ID.
    fn generate_cl_ord_id(&self, quote: &Quote) -> String {
        format!("ORD-{}-{}", quote.id(), self.next_seq_num())
    }

    /// Converts order side to FIX side value.
    #[must_use]
    pub fn side_to_fix(side: OrderSide) -> &'static str {
        match side {
            OrderSide::Buy => side::BUY,
            OrderSide::Sell => side::SELL,
        }
    }

    /// Converts FIX side value to order side.
    #[must_use]
    pub fn fix_to_side(fix_side: &str) -> Option<OrderSide> {
        match fix_side {
            side::BUY => Some(OrderSide::Buy),
            side::SELL => Some(OrderSide::Sell),
            _ => None,
        }
    }

    /// Builds a QuoteRequest FIX message.
    ///
    /// Returns the message fields as a vector of (tag, value) pairs.
    #[must_use]
    pub fn build_quote_request(&self, rfq: &Rfq, quote_req_id: &str) -> Vec<(u32, String)> {
        vec![
            (tags::QUOTE_REQ_ID, quote_req_id.to_string()),
            (tags::SYMBOL, rfq.instrument().symbol().to_string()),
            (tags::SIDE, Self::side_to_fix(rfq.side()).to_string()),
            (tags::ORDER_QTY, rfq.quantity().get().to_string()),
            (tags::TRANSACT_TIME, Timestamp::now().to_fix_format()),
        ]
    }

    /// Builds a NewOrderSingle FIX message.
    ///
    /// Returns the message fields as a vector of (tag, value) pairs.
    #[must_use]
    pub fn build_new_order_single(
        &self,
        quote: &Quote,
        cl_ord_id: &str,
        venue_quote_id: &str,
    ) -> Vec<(u32, String)> {
        vec![
            (tags::CL_ORD_ID, cl_ord_id.to_string()),
            (tags::QUOTE_ID, venue_quote_id.to_string()),
            (tags::SYMBOL, "BTC/USD".to_string()), // Would come from quote metadata
            (tags::SIDE, side::BUY.to_string()),   // Would come from quote
            (tags::ORDER_QTY, quote.quantity().get().to_string()),
            (tags::ORD_TYPE, ord_type::PREVIOUSLY_QUOTED.to_string()),
            (tags::PRICE, quote.price().get().to_string()),
            (tags::TIME_IN_FORCE, time_in_force::FOK.to_string()),
            (tags::TRANSACT_TIME, Timestamp::now().to_fix_format()),
        ]
    }

    /// Parses a Quote FIX message response.
    ///
    /// This is a stub that would parse actual FIX message bytes.
    ///
    /// # Errors
    ///
    /// Returns `VenueError::ProtocolError` if the quote cannot be parsed.
    pub fn parse_quote_response(&self, _fields: &[(u32, String)], rfq: &Rfq) -> VenueResult<Quote> {
        // Stub implementation - would parse actual FIX fields
        // For now, return a mock quote for testing
        let price = Price::new(50_000.0)
            .map_err(|_| VenueError::protocol_error("Invalid price in quote"))?;

        let valid_until = Timestamp::now().add_secs(60);

        let quote = QuoteBuilder::new(
            rfq.id(),
            self.config.venue_id().clone(),
            price,
            rfq.quantity(),
            valid_until,
        )
        .build();

        Ok(quote)
    }

    /// Parses an ExecutionReport FIX message.
    ///
    /// This is a stub that would parse actual FIX message bytes.
    ///
    /// # Errors
    ///
    /// Returns `VenueError::ProtocolError` if the execution report cannot be parsed.
    pub fn parse_execution_report(
        &self,
        _fields: &[(u32, String)],
        quote: &Quote,
    ) -> VenueResult<ExecutionResult> {
        // Stub implementation - would parse actual FIX fields
        let result = ExecutionResult::new(
            quote.id(),
            self.config.venue_id().clone(),
            quote.price(),
            quote.quantity(),
            SettlementMethod::OffChain,
        )
        .with_venue_execution_id(format!("FIX-{}", uuid::Uuid::new_v4()));

        Ok(result)
    }

    /// Handles an incoming FIX message.
    ///
    /// This would be called by the FIX engine when a message is received.
    pub async fn handle_incoming_message(&self, msg_type: &str, fields: Vec<(u32, String)>) {
        match msg_type {
            msg_type::QUOTE => {
                self.handle_quote_message(fields).await;
            }
            msg_type::EXECUTION_REPORT => {
                self.handle_execution_report(fields).await;
            }
            msg_type::QUOTE_REQUEST_REJECT => {
                self.handle_quote_reject(fields).await;
            }
            msg_type::REJECT => {
                self.handle_reject(fields).await;
            }
            _ => {
                // Unknown message type
            }
        }
    }

    async fn handle_quote_message(&self, fields: Vec<(u32, String)>) {
        // Find QuoteReqID in fields
        let quote_req_id = fields
            .iter()
            .find(|(tag, _)| *tag == tags::QUOTE_REQ_ID)
            .map(|(_, v)| v.clone());

        if let Some(req_id) = quote_req_id {
            let mut pending = self.pending_quotes.write().await;
            if let Some(request) = pending.remove(&req_id) {
                // Parse quote and send response
                // This is simplified - would need the actual RFQ
                let _ = request.response_tx.send(Err(VenueError::internal_error(
                    "Quote parsing not implemented",
                )));
            }
        }
    }

    async fn handle_execution_report(&self, fields: Vec<(u32, String)>) {
        // Find ClOrdID in fields
        let cl_ord_id = fields
            .iter()
            .find(|(tag, _)| *tag == tags::CL_ORD_ID)
            .map(|(_, v)| v.clone());

        if let Some(ord_id) = cl_ord_id {
            let mut pending = self.pending_orders.write().await;
            if let Some(order) = pending.remove(&ord_id) {
                // Parse execution report and send response
                let _ = order.response_tx.send(Err(VenueError::internal_error(
                    "Execution report parsing not implemented",
                )));
            }
        }
    }

    async fn handle_quote_reject(&self, fields: Vec<(u32, String)>) {
        // Find QuoteReqID in fields
        let quote_req_id = fields
            .iter()
            .find(|(tag, _)| *tag == tags::QUOTE_REQ_ID)
            .map(|(_, v)| v.clone());

        let text = fields
            .iter()
            .find(|(tag, _)| *tag == tags::TEXT)
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| "Quote request rejected".to_string());

        if let Some(req_id) = quote_req_id {
            let mut pending = self.pending_quotes.write().await;
            if let Some(request) = pending.remove(&req_id) {
                let _ = request
                    .response_tx
                    .send(Err(VenueError::quote_unavailable(text)));
            }
        }
    }

    async fn handle_reject(&self, fields: Vec<(u32, String)>) {
        let text = fields
            .iter()
            .find(|(tag, _)| *tag == tags::TEXT)
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| "Message rejected".to_string());

        // Log the rejection
        tracing::warn!("FIX message rejected: {}", text);
    }
}

impl fmt::Debug for FixMMAdapter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FixMMAdapter")
            .field("venue_id", self.config.venue_id())
            .field("session", self.config.session())
            .field("enabled", &self.config.is_enabled())
            .finish()
    }
}

#[async_trait]
impl VenueAdapter for FixMMAdapter {
    fn venue_id(&self) -> &VenueId {
        self.config.venue_id()
    }

    fn timeout_ms(&self) -> u64 {
        self.config.quote_timeout_ms()
    }

    async fn request_quote(&self, rfq: &Rfq) -> VenueResult<Quote> {
        // Check if enabled
        if !self.config.is_enabled() {
            return Err(VenueError::venue_unavailable(
                self.config.venue_id().clone(),
                "FIX adapter is disabled",
            ));
        }

        // Check session state
        let state = self.session_state().await;
        if state != SessionState::LoggedOn {
            return Err(VenueError::venue_unavailable(
                self.config.venue_id().clone(),
                format!("FIX session not logged on (state: {})", state),
            ));
        }

        // Generate quote request ID
        let quote_req_id = self.generate_quote_req_id(rfq);

        // Create response channel
        let (tx, rx) = oneshot::channel();

        // Store pending request
        {
            let mut pending = self.pending_quotes.write().await;
            pending.insert(
                quote_req_id.clone(),
                PendingQuoteRequest {
                    response_tx: tx,
                    rfq_id: rfq.id(),
                    sent_at: Timestamp::now(),
                },
            );
        }

        // Build and send QuoteRequest message
        let _fields = self.build_quote_request(rfq, &quote_req_id);
        // TODO: Send via IronFix engine
        // self.engine.send(msg_type::QUOTE_REQUEST, fields).await?;

        // Wait for response with timeout
        let timeout = tokio::time::Duration::from_millis(self.config.quote_timeout_ms());
        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => {
                // Channel closed - remove from pending
                let mut pending = self.pending_quotes.write().await;
                pending.remove(&quote_req_id);
                Err(VenueError::internal_error("Response channel closed"))
            }
            Err(_) => {
                // Timeout - remove from pending
                let mut pending = self.pending_quotes.write().await;
                pending.remove(&quote_req_id);
                Err(VenueError::timeout_with_duration(
                    "Quote request timed out",
                    self.config.quote_timeout_ms(),
                ))
            }
        }
    }

    async fn execute_trade(&self, quote: &Quote) -> VenueResult<ExecutionResult> {
        // Check if enabled
        if !self.config.is_enabled() {
            return Err(VenueError::venue_unavailable(
                self.config.venue_id().clone(),
                "FIX adapter is disabled",
            ));
        }

        // Check session state
        let state = self.session_state().await;
        if state != SessionState::LoggedOn {
            return Err(VenueError::venue_unavailable(
                self.config.venue_id().clone(),
                format!("FIX session not logged on (state: {})", state),
            ));
        }

        // Check if quote is expired
        if quote.is_expired() {
            return Err(VenueError::quote_expired("Quote has expired"));
        }

        // Verify quote is from this venue
        if quote.venue_id() != self.config.venue_id() {
            return Err(VenueError::invalid_request("Quote is not from this venue"));
        }

        // Generate client order ID
        let cl_ord_id = self.generate_cl_ord_id(quote);

        // Get venue quote ID from metadata (would be stored when quote was received)
        // For now, use the quote ID as a fallback
        let venue_quote_id = quote.id().to_string();

        // Create response channel
        let (tx, rx) = oneshot::channel();

        // Store pending order
        {
            let mut pending = self.pending_orders.write().await;
            pending.insert(
                cl_ord_id.clone(),
                PendingOrder {
                    response_tx: tx,
                    quote_id: quote.id(),
                    sent_at: Timestamp::now(),
                },
            );
        }

        // Build and send NewOrderSingle message
        let _fields = self.build_new_order_single(quote, &cl_ord_id, &venue_quote_id);
        // TODO: Send via IronFix engine
        // self.engine.send(msg_type::NEW_ORDER_SINGLE, fields).await?;

        // Wait for response with timeout
        let timeout = tokio::time::Duration::from_millis(self.config.execution_timeout_ms());
        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => {
                // Channel closed - remove from pending
                let mut pending = self.pending_orders.write().await;
                pending.remove(&cl_ord_id);
                Err(VenueError::internal_error("Response channel closed"))
            }
            Err(_) => {
                // Timeout - remove from pending
                let mut pending = self.pending_orders.write().await;
                pending.remove(&cl_ord_id);
                Err(VenueError::timeout_with_duration(
                    "Order execution timed out",
                    self.config.execution_timeout_ms(),
                ))
            }
        }
    }

    async fn health_check(&self) -> VenueResult<VenueHealth> {
        let state = self.session_state().await;

        let health = match state {
            SessionState::LoggedOn => VenueHealth::healthy(self.config.venue_id().clone()),
            SessionState::Connecting | SessionState::LoggingOn => {
                VenueHealth::degraded(self.config.venue_id().clone(), "Session connecting")
            }
            _ => VenueHealth::unhealthy(
                self.config.venue_id().clone(),
                format!("Session state: {}", state),
            ),
        };

        Ok(health)
    }

    async fn is_available(&self) -> bool {
        if !self.config.is_enabled() {
            return false;
        }
        let state = self.session_state().await;
        state == SessionState::LoggedOn
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::infrastructure::venues::fix_config::FixSessionConfig;

    fn test_config() -> FixMMConfig {
        let session = FixSessionConfig::new("OTC_PLATFORM", "MARKET_MAKER")
            .with_host("fix.example.com")
            .with_port(9876);
        FixMMConfig::new("fix-mm-test", session)
    }

    mod session_state {
        use super::*;

        #[test]
        fn display() {
            assert_eq!(SessionState::Disconnected.to_string(), "DISCONNECTED");
            assert_eq!(SessionState::LoggedOn.to_string(), "LOGGED_ON");
        }
    }

    mod adapter {
        use super::*;

        #[test]
        fn new() {
            let adapter = FixMMAdapter::new(test_config());
            assert_eq!(adapter.venue_id(), &VenueId::new("fix-mm-test"));
        }

        #[test]
        fn debug_format() {
            let adapter = FixMMAdapter::new(test_config());
            let debug = format!("{:?}", adapter);
            assert!(debug.contains("FixMMAdapter"));
            assert!(debug.contains("fix-mm-test"));
        }

        #[tokio::test]
        async fn initial_state_is_disconnected() {
            let adapter = FixMMAdapter::new(test_config());
            assert_eq!(adapter.session_state().await, SessionState::Disconnected);
        }

        #[tokio::test]
        async fn is_not_available_when_disconnected() {
            let adapter = FixMMAdapter::new(test_config());
            assert!(!adapter.is_available().await);
        }

        #[tokio::test]
        async fn is_not_available_when_disabled() {
            let config = test_config().with_enabled(false);
            let adapter = FixMMAdapter::new(config);
            assert!(!adapter.is_available().await);
        }
    }

    mod side_conversion {
        use super::*;

        #[test]
        fn side_to_fix() {
            assert_eq!(FixMMAdapter::side_to_fix(OrderSide::Buy), "1");
            assert_eq!(FixMMAdapter::side_to_fix(OrderSide::Sell), "2");
        }

        #[test]
        fn fix_to_side() {
            assert_eq!(FixMMAdapter::fix_to_side("1"), Some(OrderSide::Buy));
            assert_eq!(FixMMAdapter::fix_to_side("2"), Some(OrderSide::Sell));
            assert_eq!(FixMMAdapter::fix_to_side("X"), None);
        }
    }

    mod message_building {
        use super::*;
        use crate::domain::entities::rfq::RfqBuilder;
        use crate::domain::value_objects::{
            AssetClass, CounterpartyId, Instrument, Quantity, Symbol,
        };

        fn test_rfq() -> Rfq {
            let symbol = Symbol::new("BTC/USD").unwrap();
            let instrument = Instrument::builder(symbol, AssetClass::CryptoSpot).build();
            RfqBuilder::new(
                CounterpartyId::new("test-client"),
                instrument,
                OrderSide::Buy,
                Quantity::new(1.0).unwrap(),
                Timestamp::now().add_secs(300),
            )
            .build()
        }

        #[test]
        fn build_quote_request() {
            let adapter = FixMMAdapter::new(test_config());
            let rfq = test_rfq();
            let fields = adapter.build_quote_request(&rfq, "QR-123");

            assert!(
                fields
                    .iter()
                    .any(|(t, v)| *t == tags::QUOTE_REQ_ID && v == "QR-123")
            );
            assert!(
                fields
                    .iter()
                    .any(|(t, v)| *t == tags::SYMBOL && v == "BTC/USD")
            );
            assert!(fields.iter().any(|(t, v)| *t == tags::SIDE && v == "1"));
        }
    }

    mod health_check {
        use super::*;

        #[tokio::test]
        async fn unhealthy_when_disconnected() {
            let adapter = FixMMAdapter::new(test_config());
            let health = adapter.health_check().await.unwrap();
            assert!(!health.is_healthy());
        }
    }
}
