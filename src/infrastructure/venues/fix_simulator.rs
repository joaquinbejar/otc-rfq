//! # FIX Protocol Simulator
//!
//! A FIX protocol simulator for testing the FIX MM adapter using IronFix.
//!
//! This module provides a configurable FIX acceptor that can simulate
//! market maker behavior for integration testing.
//!
//! # Features
//!
//! - FIX acceptor that responds to QuoteRequest messages
//! - Configurable quote prices and latencies
//! - ExecutionReport generation for NewOrderSingle
//! - Error injection (rejects, timeouts, sequence gaps)
//! - Message recording for replay testing
//! - IronFix encoding for wire-ready messages with valid checksums
//! - Session-level messages (Logon, Heartbeat, Logout)
//!
//! # IronFix Integration
//!
//! The simulator uses `ironfix_tagvalue::Encoder` to produce properly formatted
//! FIX messages with BeginString, BodyLength, and Checksum fields.
//!
//! # Examples
//!
//! ```ignore
//! use otc_rfq::infrastructure::venues::fix_simulator::{FixSimulator, SimulatorConfig};
//!
//! let config = SimulatorConfig::default()
//!     .with_bid_price(49950.0)
//!     .with_ask_price(50050.0)
//!     .with_latency_ms(10);
//!
//! let simulator = FixSimulator::new(config);
//! simulator.start().await?;
//!
//! // Get wire-ready encoded message
//! let encoded = simulator.encode_logon();
//! ```

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use bytes::BytesMut;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::domain::value_objects::timestamp::Timestamp;
use crate::infrastructure::venues::fix_adapter::{
    exec_type, msg_type, ord_status, ord_type, side, tags, time_in_force,
};

/// FIX message type constants for session-level messages.
pub mod session_msg_type {
    /// Logon message type.
    pub const LOGON: &str = "A";
    /// Logout message type.
    pub const LOGOUT: &str = "5";
    /// Heartbeat message type.
    pub const HEARTBEAT: &str = "0";
    /// TestRequest message type.
    pub const TEST_REQUEST: &str = "1";
    /// ResendRequest message type.
    pub const RESEND_REQUEST: &str = "2";
    /// Reject message type.
    pub const REJECT: &str = "3";
    /// SequenceReset message type.
    pub const SEQUENCE_RESET: &str = "4";
}

/// FIX tag constants for session-level fields.
pub mod session_tags {
    /// EncryptMethod (98).
    pub const ENCRYPT_METHOD: u32 = 98;
    /// HeartBtInt (108).
    pub const HEART_BT_INT: u32 = 108;
    /// ResetSeqNumFlag (141).
    pub const RESET_SEQ_NUM_FLAG: u32 = 141;
    /// TestReqID (112).
    pub const TEST_REQ_ID: u32 = 112;
    /// BeginSeqNo (7).
    pub const BEGIN_SEQ_NO: u32 = 7;
    /// EndSeqNo (16).
    pub const END_SEQ_NO: u32 = 16;
    /// GapFillFlag (123).
    pub const GAP_FILL_FLAG: u32 = 123;
    /// NewSeqNo (36).
    pub const NEW_SEQ_NO: u32 = 36;
}

// ============================================================================
// Simulator Configuration
// ============================================================================

/// Configuration for the FIX simulator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulatorConfig {
    /// Sender CompID for the simulator.
    sender_comp_id: String,
    /// Target CompID (the client).
    target_comp_id: String,
    /// Bid price to quote.
    bid_price: f64,
    /// Ask price to quote.
    ask_price: f64,
    /// Bid size to quote.
    bid_size: f64,
    /// Ask size to quote.
    ask_size: f64,
    /// Quote validity in seconds.
    quote_validity_secs: u64,
    /// Simulated latency in milliseconds.
    latency_ms: u64,
    /// Whether to reject quote requests.
    reject_quotes: bool,
    /// Rejection reason for quote requests.
    quote_reject_reason: Option<String>,
    /// Whether to reject orders.
    reject_orders: bool,
    /// Rejection reason for orders.
    order_reject_reason: Option<String>,
    /// Whether to simulate timeouts (no response).
    simulate_timeout: bool,
    /// Whether to simulate sequence gaps.
    simulate_sequence_gap: bool,
    /// Maximum number of messages to record.
    max_recorded_messages: usize,
    /// Whether recording is enabled.
    recording_enabled: bool,
}

impl Default for SimulatorConfig {
    fn default() -> Self {
        Self {
            sender_comp_id: "SIMULATOR".to_string(),
            target_comp_id: "CLIENT".to_string(),
            bid_price: 49950.0,
            ask_price: 50050.0,
            bid_size: 10.0,
            ask_size: 10.0,
            quote_validity_secs: 60,
            latency_ms: 0,
            reject_quotes: false,
            quote_reject_reason: None,
            reject_orders: false,
            order_reject_reason: None,
            simulate_timeout: false,
            simulate_sequence_gap: false,
            max_recorded_messages: 1000,
            recording_enabled: true,
        }
    }
}

impl SimulatorConfig {
    /// Creates a new simulator configuration.
    #[must_use]
    pub fn new(sender_comp_id: impl Into<String>, target_comp_id: impl Into<String>) -> Self {
        Self {
            sender_comp_id: sender_comp_id.into(),
            target_comp_id: target_comp_id.into(),
            ..Default::default()
        }
    }

    /// Sets the bid price.
    #[must_use]
    pub fn with_bid_price(mut self, price: f64) -> Self {
        self.bid_price = price;
        self
    }

    /// Sets the ask price.
    #[must_use]
    pub fn with_ask_price(mut self, price: f64) -> Self {
        self.ask_price = price;
        self
    }

    /// Sets the bid size.
    #[must_use]
    pub fn with_bid_size(mut self, size: f64) -> Self {
        self.bid_size = size;
        self
    }

    /// Sets the ask size.
    #[must_use]
    pub fn with_ask_size(mut self, size: f64) -> Self {
        self.ask_size = size;
        self
    }

    /// Sets the quote validity in seconds.
    #[must_use]
    pub fn with_quote_validity_secs(mut self, secs: u64) -> Self {
        self.quote_validity_secs = secs;
        self
    }

    /// Sets the simulated latency in milliseconds.
    #[must_use]
    pub fn with_latency_ms(mut self, ms: u64) -> Self {
        self.latency_ms = ms;
        self
    }

    /// Enables quote rejection.
    #[must_use]
    pub fn with_quote_rejection(mut self, reason: impl Into<String>) -> Self {
        self.reject_quotes = true;
        self.quote_reject_reason = Some(reason.into());
        self
    }

    /// Enables order rejection.
    #[must_use]
    pub fn with_order_rejection(mut self, reason: impl Into<String>) -> Self {
        self.reject_orders = true;
        self.order_reject_reason = Some(reason.into());
        self
    }

    /// Enables timeout simulation.
    #[must_use]
    pub fn with_timeout_simulation(mut self) -> Self {
        self.simulate_timeout = true;
        self
    }

    /// Enables sequence gap simulation.
    #[must_use]
    pub fn with_sequence_gap_simulation(mut self) -> Self {
        self.simulate_sequence_gap = true;
        self
    }

    /// Disables message recording.
    #[must_use]
    pub fn without_recording(mut self) -> Self {
        self.recording_enabled = false;
        self
    }

    /// Returns the sender CompID.
    #[inline]
    #[must_use]
    pub fn sender_comp_id(&self) -> &str {
        &self.sender_comp_id
    }

    /// Returns the target CompID.
    #[inline]
    #[must_use]
    pub fn target_comp_id(&self) -> &str {
        &self.target_comp_id
    }

    /// Returns the bid price.
    #[inline]
    #[must_use]
    pub fn bid_price(&self) -> f64 {
        self.bid_price
    }

    /// Returns the ask price.
    #[inline]
    #[must_use]
    pub fn ask_price(&self) -> f64 {
        self.ask_price
    }

    /// Returns the latency in milliseconds.
    #[inline]
    #[must_use]
    pub fn latency_ms(&self) -> u64 {
        self.latency_ms
    }

    /// Returns whether quote rejection is enabled.
    #[inline]
    #[must_use]
    pub fn should_reject_quotes(&self) -> bool {
        self.reject_quotes
    }

    /// Returns whether order rejection is enabled.
    #[inline]
    #[must_use]
    pub fn should_reject_orders(&self) -> bool {
        self.reject_orders
    }

    /// Returns whether timeout simulation is enabled.
    #[inline]
    #[must_use]
    pub fn should_simulate_timeout(&self) -> bool {
        self.simulate_timeout
    }
}

// ============================================================================
// Recorded Message
// ============================================================================

/// Direction of a recorded message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageDirection {
    /// Message received from client.
    Inbound,
    /// Message sent to client.
    Outbound,
}

/// A recorded FIX message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordedMessage {
    /// Timestamp when the message was recorded.
    timestamp: Timestamp,
    /// Direction of the message.
    direction: MessageDirection,
    /// Message type (MsgType field).
    msg_type: String,
    /// Message fields as (tag, value) pairs.
    fields: Vec<(u32, String)>,
    /// Sequence number.
    seq_num: u64,
}

impl RecordedMessage {
    /// Creates a new recorded message.
    #[must_use]
    pub fn new(
        direction: MessageDirection,
        msg_type: impl Into<String>,
        fields: Vec<(u32, String)>,
        seq_num: u64,
    ) -> Self {
        Self {
            timestamp: Timestamp::now(),
            direction,
            msg_type: msg_type.into(),
            fields,
            seq_num,
        }
    }

    /// Returns the timestamp.
    #[inline]
    #[must_use]
    pub fn timestamp(&self) -> Timestamp {
        self.timestamp
    }

    /// Returns the direction.
    #[inline]
    #[must_use]
    pub fn direction(&self) -> MessageDirection {
        self.direction
    }

    /// Returns the message type.
    #[inline]
    #[must_use]
    pub fn msg_type(&self) -> &str {
        &self.msg_type
    }

    /// Returns the fields.
    #[inline]
    #[must_use]
    pub fn fields(&self) -> &[(u32, String)] {
        &self.fields
    }

    /// Returns the sequence number.
    #[inline]
    #[must_use]
    pub fn seq_num(&self) -> u64 {
        self.seq_num
    }

    /// Gets a field value by tag.
    #[must_use]
    pub fn get_field(&self, tag: u32) -> Option<&str> {
        self.fields
            .iter()
            .find(|(t, _)| *t == tag)
            .map(|(_, v)| v.as_str())
    }
}

// ============================================================================
// FIX Simulator
// ============================================================================

/// FIX protocol simulator for testing.
///
/// Simulates a FIX market maker acceptor that responds to QuoteRequest
/// and NewOrderSingle messages with configurable behavior.
pub struct FixSimulator {
    /// Configuration.
    config: SimulatorConfig,
    /// Whether the simulator is running.
    running: AtomicBool,
    /// Outbound sequence number.
    outbound_seq_num: AtomicU64,
    /// Inbound sequence number.
    inbound_seq_num: AtomicU64,
    /// Recorded messages.
    recorded_messages: Arc<RwLock<VecDeque<RecordedMessage>>>,
    /// Quote counter for generating unique IDs.
    quote_counter: AtomicU64,
    /// Execution counter for generating unique IDs.
    exec_counter: AtomicU64,
}

impl FixSimulator {
    /// Creates a new FIX simulator.
    #[must_use]
    pub fn new(config: SimulatorConfig) -> Self {
        Self {
            config,
            running: AtomicBool::new(false),
            outbound_seq_num: AtomicU64::new(1),
            inbound_seq_num: AtomicU64::new(1),
            recorded_messages: Arc::new(RwLock::new(VecDeque::new())),
            quote_counter: AtomicU64::new(1),
            exec_counter: AtomicU64::new(1),
        }
    }

    /// Returns the configuration.
    #[inline]
    #[must_use]
    pub fn config(&self) -> &SimulatorConfig {
        &self.config
    }

    /// Returns whether the simulator is running.
    #[inline]
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Starts the simulator.
    pub fn start(&self) {
        self.running.store(true, Ordering::SeqCst);
    }

    /// Stops the simulator.
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    /// Resets the simulator state.
    pub async fn reset(&self) {
        self.outbound_seq_num.store(1, Ordering::SeqCst);
        self.inbound_seq_num.store(1, Ordering::SeqCst);
        self.quote_counter.store(1, Ordering::SeqCst);
        self.exec_counter.store(1, Ordering::SeqCst);
        self.recorded_messages.write().await.clear();
    }

    /// Gets the next outbound sequence number.
    fn next_outbound_seq_num(&self) -> u64 {
        if self.config.simulate_sequence_gap {
            // Skip a sequence number to simulate a gap
            self.outbound_seq_num.fetch_add(2, Ordering::SeqCst)
        } else {
            self.outbound_seq_num.fetch_add(1, Ordering::SeqCst)
        }
    }

    /// Gets the next inbound sequence number.
    fn next_inbound_seq_num(&self) -> u64 {
        self.inbound_seq_num.fetch_add(1, Ordering::SeqCst)
    }

    /// Generates a unique quote ID.
    fn generate_quote_id(&self) -> String {
        format!(
            "SIM-Q-{}",
            self.quote_counter.fetch_add(1, Ordering::SeqCst)
        )
    }

    /// Generates a unique execution ID.
    fn generate_exec_id(&self) -> String {
        format!("SIM-E-{}", self.exec_counter.fetch_add(1, Ordering::SeqCst))
    }

    /// Records a message.
    async fn record_message(&self, message: RecordedMessage) {
        if !self.config.recording_enabled {
            return;
        }

        let mut messages = self.recorded_messages.write().await;
        if messages.len() >= self.config.max_recorded_messages {
            messages.pop_front();
        }
        messages.push_back(message);
    }

    /// Returns all recorded messages.
    pub async fn get_recorded_messages(&self) -> Vec<RecordedMessage> {
        self.recorded_messages
            .read()
            .await
            .iter()
            .cloned()
            .collect()
    }

    /// Returns recorded messages of a specific type.
    pub async fn get_messages_by_type(&self, msg_type: &str) -> Vec<RecordedMessage> {
        self.recorded_messages
            .read()
            .await
            .iter()
            .filter(|m| m.msg_type == msg_type)
            .cloned()
            .collect()
    }

    /// Clears recorded messages.
    pub async fn clear_recorded_messages(&self) {
        self.recorded_messages.write().await.clear();
    }

    /// Processes an incoming QuoteRequest message.
    ///
    /// Returns a Quote or QuoteRequestReject message.
    pub async fn process_quote_request(
        &self,
        fields: Vec<(u32, String)>,
    ) -> Option<(String, Vec<(u32, String)>)> {
        // Record inbound message
        let in_seq = self.next_inbound_seq_num();
        self.record_message(RecordedMessage::new(
            MessageDirection::Inbound,
            msg_type::QUOTE_REQUEST,
            fields.clone(),
            in_seq,
        ))
        .await;

        // Simulate timeout
        if self.config.simulate_timeout {
            return None;
        }

        // Simulate latency
        if self.config.latency_ms > 0 {
            tokio::time::sleep(Duration::from_millis(self.config.latency_ms)).await;
        }

        // Extract fields
        let quote_req_id = fields
            .iter()
            .find(|(t, _)| *t == tags::QUOTE_REQ_ID)
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| "UNKNOWN".to_string());

        let symbol = fields
            .iter()
            .find(|(t, _)| *t == tags::SYMBOL)
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| "BTC/USD".to_string());

        let req_side = fields
            .iter()
            .find(|(t, _)| *t == tags::SIDE)
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| side::BUY.to_string());

        let order_qty = fields
            .iter()
            .find(|(t, _)| *t == tags::ORDER_QTY)
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| "1.0".to_string());

        let out_seq = self.next_outbound_seq_num();

        // Check if we should reject
        if self.config.reject_quotes {
            let reject_reason = self
                .config
                .quote_reject_reason
                .clone()
                .unwrap_or_else(|| "Quote request rejected".to_string());

            let reject_fields = vec![
                (tags::QUOTE_REQ_ID, quote_req_id),
                (tags::TEXT, reject_reason),
            ];

            self.record_message(RecordedMessage::new(
                MessageDirection::Outbound,
                msg_type::QUOTE_REQUEST_REJECT,
                reject_fields.clone(),
                out_seq,
            ))
            .await;

            return Some((msg_type::QUOTE_REQUEST_REJECT.to_string(), reject_fields));
        }

        // Generate quote response
        let quote_id = self.generate_quote_id();
        let valid_until = Timestamp::now().add_secs(self.config.quote_validity_secs as i64);

        // Determine price based on side
        let (price, _size) = if req_side == side::BUY {
            (self.config.ask_price, self.config.ask_size)
        } else {
            (self.config.bid_price, self.config.bid_size)
        };

        let quote_fields = vec![
            (tags::QUOTE_REQ_ID, quote_req_id),
            (tags::QUOTE_ID, quote_id),
            (tags::SYMBOL, symbol),
            (tags::SIDE, req_side),
            (tags::BID_PX, self.config.bid_price.to_string()),
            (tags::OFFER_PX, self.config.ask_price.to_string()),
            (tags::BID_SIZE, self.config.bid_size.to_string()),
            (tags::OFFER_SIZE, self.config.ask_size.to_string()),
            (tags::ORDER_QTY, order_qty),
            (tags::PRICE, price.to_string()),
            (tags::VALID_UNTIL_TIME, valid_until.to_fix_format()),
            (tags::TRANSACT_TIME, Timestamp::now().to_fix_format()),
        ];

        self.record_message(RecordedMessage::new(
            MessageDirection::Outbound,
            msg_type::QUOTE,
            quote_fields.clone(),
            out_seq,
        ))
        .await;

        Some((msg_type::QUOTE.to_string(), quote_fields))
    }

    /// Processes an incoming NewOrderSingle message.
    ///
    /// Returns an ExecutionReport message.
    pub async fn process_new_order_single(
        &self,
        fields: Vec<(u32, String)>,
    ) -> Option<(String, Vec<(u32, String)>)> {
        // Record inbound message
        let in_seq = self.next_inbound_seq_num();
        self.record_message(RecordedMessage::new(
            MessageDirection::Inbound,
            msg_type::NEW_ORDER_SINGLE,
            fields.clone(),
            in_seq,
        ))
        .await;

        // Simulate timeout
        if self.config.simulate_timeout {
            return None;
        }

        // Simulate latency
        if self.config.latency_ms > 0 {
            tokio::time::sleep(Duration::from_millis(self.config.latency_ms)).await;
        }

        // Extract fields
        let cl_ord_id = fields
            .iter()
            .find(|(t, _)| *t == tags::CL_ORD_ID)
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| "UNKNOWN".to_string());

        let quote_id = fields
            .iter()
            .find(|(t, _)| *t == tags::QUOTE_ID)
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| "UNKNOWN".to_string());

        let symbol = fields
            .iter()
            .find(|(t, _)| *t == tags::SYMBOL)
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| "BTC/USD".to_string());

        let order_side = fields
            .iter()
            .find(|(t, _)| *t == tags::SIDE)
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| side::BUY.to_string());

        let order_qty = fields
            .iter()
            .find(|(t, _)| *t == tags::ORDER_QTY)
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| "1.0".to_string());

        let price = fields
            .iter()
            .find(|(t, _)| *t == tags::PRICE)
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| self.config.ask_price.to_string());

        let out_seq = self.next_outbound_seq_num();
        let exec_id = self.generate_exec_id();

        // Check if we should reject
        if self.config.reject_orders {
            let reject_reason = self
                .config
                .order_reject_reason
                .clone()
                .unwrap_or_else(|| "Order rejected".to_string());

            let reject_fields = vec![
                (tags::CL_ORD_ID, cl_ord_id),
                (tags::EXEC_ID, exec_id),
                (tags::EXEC_TYPE, exec_type::REJECTED.to_string()),
                (tags::ORD_STATUS, ord_status::REJECTED.to_string()),
                (tags::SYMBOL, symbol),
                (tags::SIDE, order_side),
                (tags::ORDER_QTY, order_qty),
                (tags::TEXT, reject_reason),
                (tags::TRANSACT_TIME, Timestamp::now().to_fix_format()),
            ];

            self.record_message(RecordedMessage::new(
                MessageDirection::Outbound,
                msg_type::EXECUTION_REPORT,
                reject_fields.clone(),
                out_seq,
            ))
            .await;

            return Some((msg_type::EXECUTION_REPORT.to_string(), reject_fields));
        }

        // Generate fill execution report
        let fill_fields = vec![
            (tags::CL_ORD_ID, cl_ord_id),
            (tags::QUOTE_ID, quote_id),
            (tags::EXEC_ID, exec_id),
            (tags::EXEC_TYPE, exec_type::FILL.to_string()),
            (tags::ORD_STATUS, ord_status::FILLED.to_string()),
            (tags::SYMBOL, symbol),
            (tags::SIDE, order_side),
            (tags::ORDER_QTY, order_qty.clone()),
            (tags::LAST_PX, price),
            (tags::LAST_QTY, order_qty),
            (tags::TRANSACT_TIME, Timestamp::now().to_fix_format()),
        ];

        self.record_message(RecordedMessage::new(
            MessageDirection::Outbound,
            msg_type::EXECUTION_REPORT,
            fill_fields.clone(),
            out_seq,
        ))
        .await;

        Some((msg_type::EXECUTION_REPORT.to_string(), fill_fields))
    }

    /// Processes an incoming FIX message based on its type.
    ///
    /// Returns the response message type and fields, or None for timeout.
    pub async fn process_message(
        &self,
        msg_type: &str,
        fields: Vec<(u32, String)>,
    ) -> Option<(String, Vec<(u32, String)>)> {
        match msg_type {
            msg_type::QUOTE_REQUEST => self.process_quote_request(fields).await,
            msg_type::NEW_ORDER_SINGLE => self.process_new_order_single(fields).await,
            _ => {
                // Unknown message type - record and ignore
                let in_seq = self.next_inbound_seq_num();
                self.record_message(RecordedMessage::new(
                    MessageDirection::Inbound,
                    msg_type,
                    fields,
                    in_seq,
                ))
                .await;
                None
            }
        }
    }

    /// Creates a QuoteRequest message for testing.
    #[must_use]
    pub fn create_quote_request(
        quote_req_id: &str,
        symbol: &str,
        side: &str,
        quantity: f64,
    ) -> Vec<(u32, String)> {
        vec![
            (tags::QUOTE_REQ_ID, quote_req_id.to_string()),
            (tags::SYMBOL, symbol.to_string()),
            (tags::SIDE, side.to_string()),
            (tags::ORDER_QTY, quantity.to_string()),
            (tags::TRANSACT_TIME, Timestamp::now().to_fix_format()),
        ]
    }

    /// Creates a NewOrderSingle message for testing.
    #[must_use]
    pub fn create_new_order_single(
        cl_ord_id: &str,
        quote_id: &str,
        symbol: &str,
        side: &str,
        quantity: f64,
        price: f64,
    ) -> Vec<(u32, String)> {
        vec![
            (tags::CL_ORD_ID, cl_ord_id.to_string()),
            (tags::QUOTE_ID, quote_id.to_string()),
            (tags::SYMBOL, symbol.to_string()),
            (tags::SIDE, side.to_string()),
            (tags::ORDER_QTY, quantity.to_string()),
            (tags::ORD_TYPE, ord_type::PREVIOUSLY_QUOTED.to_string()),
            (tags::PRICE, price.to_string()),
            (tags::TIME_IN_FORCE, time_in_force::FOK.to_string()),
            (tags::TRANSACT_TIME, Timestamp::now().to_fix_format()),
        ]
    }

    // ========================================================================
    // IronFix Encoding Methods
    // ========================================================================

    /// Encodes a Logon message using IronFix.
    ///
    /// Returns a wire-ready FIX message with proper header and checksum.
    #[must_use]
    pub fn encode_logon(&self) -> BytesMut {
        let mut encoder = ironfix_tagvalue::Encoder::new("FIX.4.4");

        // MsgType = A (Logon)
        encoder.put_str(35, session_msg_type::LOGON);

        // Session fields
        encoder.put_str(49, self.config.sender_comp_id());
        encoder.put_str(56, self.config.target_comp_id());
        encoder.put_uint(34, self.next_outbound_seq_num());
        encoder.put_str(52, &Timestamp::now().to_fix_format());

        // Logon-specific fields
        encoder.put_uint(session_tags::ENCRYPT_METHOD, 0); // No encryption
        encoder.put_uint(session_tags::HEART_BT_INT, 30); // 30 second heartbeat
        encoder.put_str(session_tags::RESET_SEQ_NUM_FLAG, "Y");

        encoder.finish()
    }

    /// Encodes a Logout message using IronFix.
    ///
    /// Returns a wire-ready FIX message with proper header and checksum.
    #[must_use]
    pub fn encode_logout(&self, text: Option<&str>) -> BytesMut {
        let mut encoder = ironfix_tagvalue::Encoder::new("FIX.4.4");

        // MsgType = 5 (Logout)
        encoder.put_str(35, session_msg_type::LOGOUT);

        // Session fields
        encoder.put_str(49, self.config.sender_comp_id());
        encoder.put_str(56, self.config.target_comp_id());
        encoder.put_uint(34, self.next_outbound_seq_num());
        encoder.put_str(52, &Timestamp::now().to_fix_format());

        // Optional text
        if let Some(t) = text {
            encoder.put_str(tags::TEXT, t);
        }

        encoder.finish()
    }

    /// Encodes a Heartbeat message using IronFix.
    ///
    /// Returns a wire-ready FIX message with proper header and checksum.
    #[must_use]
    pub fn encode_heartbeat(&self, test_req_id: Option<&str>) -> BytesMut {
        let mut encoder = ironfix_tagvalue::Encoder::new("FIX.4.4");

        // MsgType = 0 (Heartbeat)
        encoder.put_str(35, session_msg_type::HEARTBEAT);

        // Session fields
        encoder.put_str(49, self.config.sender_comp_id());
        encoder.put_str(56, self.config.target_comp_id());
        encoder.put_uint(34, self.next_outbound_seq_num());
        encoder.put_str(52, &Timestamp::now().to_fix_format());

        // TestReqID if responding to TestRequest
        if let Some(id) = test_req_id {
            encoder.put_str(session_tags::TEST_REQ_ID, id);
        }

        encoder.finish()
    }

    /// Encodes a TestRequest message using IronFix.
    ///
    /// Returns a wire-ready FIX message with proper header and checksum.
    #[must_use]
    pub fn encode_test_request(&self, test_req_id: &str) -> BytesMut {
        let mut encoder = ironfix_tagvalue::Encoder::new("FIX.4.4");

        // MsgType = 1 (TestRequest)
        encoder.put_str(35, session_msg_type::TEST_REQUEST);

        // Session fields
        encoder.put_str(49, self.config.sender_comp_id());
        encoder.put_str(56, self.config.target_comp_id());
        encoder.put_uint(34, self.next_outbound_seq_num());
        encoder.put_str(52, &Timestamp::now().to_fix_format());

        // TestReqID
        encoder.put_str(session_tags::TEST_REQ_ID, test_req_id);

        encoder.finish()
    }

    /// Encodes a Quote message using IronFix.
    ///
    /// Returns a wire-ready FIX message with proper header and checksum.
    #[must_use]
    pub fn encode_quote(
        &self,
        quote_req_id: &str,
        quote_id: &str,
        symbol: &str,
        req_side: &str,
        order_qty: &str,
    ) -> BytesMut {
        let mut encoder = ironfix_tagvalue::Encoder::new("FIX.4.4");

        // MsgType = S (Quote)
        encoder.put_str(35, msg_type::QUOTE);

        // Session fields
        encoder.put_str(49, self.config.sender_comp_id());
        encoder.put_str(56, self.config.target_comp_id());
        encoder.put_uint(34, self.next_outbound_seq_num());
        encoder.put_str(52, &Timestamp::now().to_fix_format());

        // Quote fields
        encoder.put_str(tags::QUOTE_REQ_ID, quote_req_id);
        encoder.put_str(tags::QUOTE_ID, quote_id);
        encoder.put_str(tags::SYMBOL, symbol);
        encoder.put_str(tags::SIDE, req_side);
        encoder.put_str(tags::BID_PX, &self.config.bid_price.to_string());
        encoder.put_str(tags::OFFER_PX, &self.config.ask_price.to_string());
        encoder.put_str(tags::BID_SIZE, &self.config.bid_size.to_string());
        encoder.put_str(tags::OFFER_SIZE, &self.config.ask_size.to_string());
        encoder.put_str(tags::ORDER_QTY, order_qty);

        let valid_until = Timestamp::now().add_secs(self.config.quote_validity_secs as i64);
        encoder.put_str(tags::VALID_UNTIL_TIME, &valid_until.to_fix_format());
        encoder.put_str(tags::TRANSACT_TIME, &Timestamp::now().to_fix_format());

        encoder.finish()
    }

    /// Encodes an ExecutionReport (Fill) message using IronFix.
    ///
    /// Returns a wire-ready FIX message with proper header and checksum.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn encode_execution_report_fill(
        &self,
        cl_ord_id: &str,
        quote_id: &str,
        exec_id: &str,
        symbol: &str,
        order_side: &str,
        order_qty: &str,
        price: &str,
    ) -> BytesMut {
        let mut encoder = ironfix_tagvalue::Encoder::new("FIX.4.4");

        // MsgType = 8 (ExecutionReport)
        encoder.put_str(35, msg_type::EXECUTION_REPORT);

        // Session fields
        encoder.put_str(49, self.config.sender_comp_id());
        encoder.put_str(56, self.config.target_comp_id());
        encoder.put_uint(34, self.next_outbound_seq_num());
        encoder.put_str(52, &Timestamp::now().to_fix_format());

        // Execution report fields
        encoder.put_str(tags::CL_ORD_ID, cl_ord_id);
        encoder.put_str(tags::QUOTE_ID, quote_id);
        encoder.put_str(tags::EXEC_ID, exec_id);
        encoder.put_str(tags::EXEC_TYPE, exec_type::FILL);
        encoder.put_str(tags::ORD_STATUS, ord_status::FILLED);
        encoder.put_str(tags::SYMBOL, symbol);
        encoder.put_str(tags::SIDE, order_side);
        encoder.put_str(tags::ORDER_QTY, order_qty);
        encoder.put_str(tags::LAST_PX, price);
        encoder.put_str(tags::LAST_QTY, order_qty);
        encoder.put_str(tags::TRANSACT_TIME, &Timestamp::now().to_fix_format());

        encoder.finish()
    }

    /// Encodes an ExecutionReport (Reject) message using IronFix.
    ///
    /// Returns a wire-ready FIX message with proper header and checksum.
    #[must_use]
    pub fn encode_execution_report_reject(
        &self,
        cl_ord_id: &str,
        exec_id: &str,
        symbol: &str,
        order_side: &str,
        order_qty: &str,
        reject_reason: &str,
    ) -> BytesMut {
        let mut encoder = ironfix_tagvalue::Encoder::new("FIX.4.4");

        // MsgType = 8 (ExecutionReport)
        encoder.put_str(35, msg_type::EXECUTION_REPORT);

        // Session fields
        encoder.put_str(49, self.config.sender_comp_id());
        encoder.put_str(56, self.config.target_comp_id());
        encoder.put_uint(34, self.next_outbound_seq_num());
        encoder.put_str(52, &Timestamp::now().to_fix_format());

        // Execution report fields
        encoder.put_str(tags::CL_ORD_ID, cl_ord_id);
        encoder.put_str(tags::EXEC_ID, exec_id);
        encoder.put_str(tags::EXEC_TYPE, exec_type::REJECTED);
        encoder.put_str(tags::ORD_STATUS, ord_status::REJECTED);
        encoder.put_str(tags::SYMBOL, symbol);
        encoder.put_str(tags::SIDE, order_side);
        encoder.put_str(tags::ORDER_QTY, order_qty);
        encoder.put_str(tags::TEXT, reject_reason);
        encoder.put_str(tags::TRANSACT_TIME, &Timestamp::now().to_fix_format());

        encoder.finish()
    }
}

impl std::fmt::Debug for FixSimulator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FixSimulator")
            .field("config", &self.config)
            .field("running", &self.running.load(Ordering::SeqCst))
            .field(
                "outbound_seq_num",
                &self.outbound_seq_num.load(Ordering::SeqCst),
            )
            .field(
                "inbound_seq_num",
                &self.inbound_seq_num.load(Ordering::SeqCst),
            )
            .finish()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn default_simulator() -> FixSimulator {
        FixSimulator::new(SimulatorConfig::default())
    }

    mod config {
        use super::*;

        #[test]
        fn default_config() {
            let config = SimulatorConfig::default();
            assert_eq!(config.sender_comp_id(), "SIMULATOR");
            assert_eq!(config.target_comp_id(), "CLIENT");
            assert_eq!(config.bid_price(), 49950.0);
            assert_eq!(config.ask_price(), 50050.0);
            assert_eq!(config.latency_ms(), 0);
            assert!(!config.should_reject_quotes());
            assert!(!config.should_reject_orders());
            assert!(!config.should_simulate_timeout());
        }

        #[test]
        fn builder_pattern() {
            let config = SimulatorConfig::new("MM", "CLIENT")
                .with_bid_price(100.0)
                .with_ask_price(101.0)
                .with_latency_ms(50)
                .with_quote_rejection("No liquidity");

            assert_eq!(config.sender_comp_id(), "MM");
            assert_eq!(config.bid_price(), 100.0);
            assert_eq!(config.ask_price(), 101.0);
            assert_eq!(config.latency_ms(), 50);
            assert!(config.should_reject_quotes());
        }

        #[test]
        fn timeout_simulation() {
            let config = SimulatorConfig::default().with_timeout_simulation();
            assert!(config.should_simulate_timeout());
        }

        #[test]
        fn order_rejection() {
            let config = SimulatorConfig::default().with_order_rejection("Insufficient funds");
            assert!(config.should_reject_orders());
        }
    }

    mod simulator {
        use super::*;

        #[test]
        fn new_simulator() {
            let sim = default_simulator();
            assert!(!sim.is_running());
        }

        #[test]
        fn start_stop() {
            let sim = default_simulator();
            assert!(!sim.is_running());

            sim.start();
            assert!(sim.is_running());

            sim.stop();
            assert!(!sim.is_running());
        }

        #[tokio::test]
        async fn reset_clears_state() {
            let sim = default_simulator();
            sim.start();

            // Process a message to increment counters
            let fields = FixSimulator::create_quote_request("QR-1", "BTC/USD", side::BUY, 1.0);
            sim.process_quote_request(fields).await;

            // Verify messages were recorded
            let messages = sim.get_recorded_messages().await;
            assert!(!messages.is_empty());

            // Reset
            sim.reset().await;

            // Verify state is cleared
            let messages = sim.get_recorded_messages().await;
            assert!(messages.is_empty());
        }
    }

    mod quote_request {
        use super::*;

        #[tokio::test]
        async fn successful_quote_response() {
            let sim = default_simulator();
            sim.start();

            let fields = FixSimulator::create_quote_request("QR-1", "BTC/USD", side::BUY, 1.0);
            let response = sim.process_quote_request(fields).await;

            assert!(response.is_some());
            let (msg_type, fields) = response.unwrap();
            assert_eq!(msg_type, msg_type::QUOTE);

            // Verify quote fields
            let quote_id = fields.iter().find(|(t, _)| *t == tags::QUOTE_ID);
            assert!(quote_id.is_some());

            let bid_px = fields.iter().find(|(t, _)| *t == tags::BID_PX);
            assert!(bid_px.is_some());
            assert_eq!(bid_px.unwrap().1, "49950");

            let offer_px = fields.iter().find(|(t, _)| *t == tags::OFFER_PX);
            assert!(offer_px.is_some());
            assert_eq!(offer_px.unwrap().1, "50050");
        }

        #[tokio::test]
        async fn quote_rejection() {
            let config = SimulatorConfig::default().with_quote_rejection("No liquidity");
            let sim = FixSimulator::new(config);
            sim.start();

            let fields = FixSimulator::create_quote_request("QR-1", "BTC/USD", side::BUY, 1.0);
            let response = sim.process_quote_request(fields).await;

            assert!(response.is_some());
            let (msg_type, fields) = response.unwrap();
            assert_eq!(msg_type, msg_type::QUOTE_REQUEST_REJECT);

            let text = fields.iter().find(|(t, _)| *t == tags::TEXT);
            assert!(text.is_some());
            assert_eq!(text.unwrap().1, "No liquidity");
        }

        #[tokio::test]
        async fn timeout_returns_none() {
            let config = SimulatorConfig::default().with_timeout_simulation();
            let sim = FixSimulator::new(config);
            sim.start();

            let fields = FixSimulator::create_quote_request("QR-1", "BTC/USD", side::BUY, 1.0);
            let response = sim.process_quote_request(fields).await;

            assert!(response.is_none());
        }

        #[tokio::test]
        async fn messages_recorded() {
            let sim = default_simulator();
            sim.start();

            let fields = FixSimulator::create_quote_request("QR-1", "BTC/USD", side::BUY, 1.0);
            sim.process_quote_request(fields).await;

            let messages = sim.get_recorded_messages().await;
            assert_eq!(messages.len(), 2); // Inbound + Outbound

            assert_eq!(messages[0].direction(), MessageDirection::Inbound);
            assert_eq!(messages[0].msg_type(), msg_type::QUOTE_REQUEST);

            assert_eq!(messages[1].direction(), MessageDirection::Outbound);
            assert_eq!(messages[1].msg_type(), msg_type::QUOTE);
        }
    }

    mod new_order_single {
        use super::*;

        #[tokio::test]
        async fn successful_fill() {
            let sim = default_simulator();
            sim.start();

            let fields = FixSimulator::create_new_order_single(
                "ORD-1",
                "Q-1",
                "BTC/USD",
                side::BUY,
                1.0,
                50050.0,
            );
            let response = sim.process_new_order_single(fields).await;

            assert!(response.is_some());
            let (msg_type, fields) = response.unwrap();
            assert_eq!(msg_type, msg_type::EXECUTION_REPORT);

            let exec_type_field = fields.iter().find(|(t, _)| *t == tags::EXEC_TYPE);
            assert!(exec_type_field.is_some());
            assert_eq!(exec_type_field.unwrap().1, exec_type::FILL);

            let ord_status_field = fields.iter().find(|(t, _)| *t == tags::ORD_STATUS);
            assert!(ord_status_field.is_some());
            assert_eq!(ord_status_field.unwrap().1, ord_status::FILLED);
        }

        #[tokio::test]
        async fn order_rejection() {
            let config = SimulatorConfig::default().with_order_rejection("Insufficient funds");
            let sim = FixSimulator::new(config);
            sim.start();

            let fields = FixSimulator::create_new_order_single(
                "ORD-1",
                "Q-1",
                "BTC/USD",
                side::BUY,
                1.0,
                50050.0,
            );
            let response = sim.process_new_order_single(fields).await;

            assert!(response.is_some());
            let (msg_type, fields) = response.unwrap();
            assert_eq!(msg_type, msg_type::EXECUTION_REPORT);

            let exec_type_field = fields.iter().find(|(t, _)| *t == tags::EXEC_TYPE);
            assert!(exec_type_field.is_some());
            assert_eq!(exec_type_field.unwrap().1, exec_type::REJECTED);

            let text = fields.iter().find(|(t, _)| *t == tags::TEXT);
            assert!(text.is_some());
            assert_eq!(text.unwrap().1, "Insufficient funds");
        }

        #[tokio::test]
        async fn timeout_returns_none() {
            let config = SimulatorConfig::default().with_timeout_simulation();
            let sim = FixSimulator::new(config);
            sim.start();

            let fields = FixSimulator::create_new_order_single(
                "ORD-1",
                "Q-1",
                "BTC/USD",
                side::BUY,
                1.0,
                50050.0,
            );
            let response = sim.process_new_order_single(fields).await;

            assert!(response.is_none());
        }
    }

    mod message_recording {
        use super::*;

        #[tokio::test]
        async fn filter_by_type() {
            let sim = default_simulator();
            sim.start();

            // Send quote request
            let qr_fields = FixSimulator::create_quote_request("QR-1", "BTC/USD", side::BUY, 1.0);
            sim.process_quote_request(qr_fields).await;

            // Send order
            let ord_fields = FixSimulator::create_new_order_single(
                "ORD-1",
                "Q-1",
                "BTC/USD",
                side::BUY,
                1.0,
                50050.0,
            );
            sim.process_new_order_single(ord_fields).await;

            // Filter by type
            let quote_requests = sim.get_messages_by_type(msg_type::QUOTE_REQUEST).await;
            assert_eq!(quote_requests.len(), 1);

            let quotes = sim.get_messages_by_type(msg_type::QUOTE).await;
            assert_eq!(quotes.len(), 1);

            let orders = sim.get_messages_by_type(msg_type::NEW_ORDER_SINGLE).await;
            assert_eq!(orders.len(), 1);

            let exec_reports = sim.get_messages_by_type(msg_type::EXECUTION_REPORT).await;
            assert_eq!(exec_reports.len(), 1);
        }

        #[tokio::test]
        async fn clear_messages() {
            let sim = default_simulator();
            sim.start();

            let fields = FixSimulator::create_quote_request("QR-1", "BTC/USD", side::BUY, 1.0);
            sim.process_quote_request(fields).await;

            assert!(!sim.get_recorded_messages().await.is_empty());

            sim.clear_recorded_messages().await;

            assert!(sim.get_recorded_messages().await.is_empty());
        }

        #[tokio::test]
        async fn recording_disabled() {
            let config = SimulatorConfig::default().without_recording();
            let sim = FixSimulator::new(config);
            sim.start();

            let fields = FixSimulator::create_quote_request("QR-1", "BTC/USD", side::BUY, 1.0);
            sim.process_quote_request(fields).await;

            assert!(sim.get_recorded_messages().await.is_empty());
        }

        #[tokio::test]
        async fn get_field_from_recorded() {
            let sim = default_simulator();
            sim.start();

            let fields = FixSimulator::create_quote_request("QR-1", "BTC/USD", side::BUY, 1.0);
            sim.process_quote_request(fields).await;

            let messages = sim.get_recorded_messages().await;
            let inbound = &messages[0];

            assert_eq!(inbound.get_field(tags::QUOTE_REQ_ID), Some("QR-1"));
            assert_eq!(inbound.get_field(tags::SYMBOL), Some("BTC/USD"));
            assert_eq!(inbound.get_field(tags::SIDE), Some(side::BUY));
        }
    }

    mod sequence_numbers {
        use super::*;

        #[tokio::test]
        async fn sequence_gap_simulation() {
            let config = SimulatorConfig::default().with_sequence_gap_simulation();
            let sim = FixSimulator::new(config);
            sim.start();

            // First message
            let fields1 = FixSimulator::create_quote_request("QR-1", "BTC/USD", side::BUY, 1.0);
            sim.process_quote_request(fields1).await;

            // Second message
            let fields2 = FixSimulator::create_quote_request("QR-2", "BTC/USD", side::BUY, 1.0);
            sim.process_quote_request(fields2).await;

            let messages = sim.get_recorded_messages().await;
            let outbound_messages: Vec<_> = messages
                .iter()
                .filter(|m| m.direction() == MessageDirection::Outbound)
                .collect();

            // With sequence gap, seq nums should skip (1, 3, 5, ...)
            assert_eq!(outbound_messages.len(), 2);
            let seq1 = outbound_messages[0].seq_num();
            let seq2 = outbound_messages[1].seq_num();
            assert_eq!(seq2 - seq1, 2); // Gap of 2
        }
    }

    mod latency {
        use super::*;
        use std::time::Instant;

        #[tokio::test]
        async fn simulated_latency() {
            let config = SimulatorConfig::default().with_latency_ms(100);
            let sim = FixSimulator::new(config);
            sim.start();

            let fields = FixSimulator::create_quote_request("QR-1", "BTC/USD", side::BUY, 1.0);

            let start = Instant::now();
            sim.process_quote_request(fields).await;
            let elapsed = start.elapsed();

            assert!(elapsed >= Duration::from_millis(100));
        }
    }

    mod ironfix_encoding {
        use super::*;

        #[test]
        fn encode_logon_has_valid_structure() {
            let sim = default_simulator();
            let encoded = sim.encode_logon();

            // Convert to string for inspection
            let msg = String::from_utf8_lossy(&encoded);

            // Verify message structure
            assert!(msg.starts_with("8=FIX.4.4\x01"));
            assert!(msg.contains("35=A\x01")); // MsgType = Logon
            assert!(msg.contains("49=SIMULATOR\x01")); // SenderCompID
            assert!(msg.contains("56=CLIENT\x01")); // TargetCompID
            assert!(msg.contains("98=0\x01")); // EncryptMethod
            assert!(msg.contains("108=30\x01")); // HeartBtInt
            assert!(msg.contains("10=")); // Checksum present
        }

        #[test]
        fn encode_logout_has_valid_structure() {
            let sim = default_simulator();
            let encoded = sim.encode_logout(Some("Session ended"));

            let msg = String::from_utf8_lossy(&encoded);

            assert!(msg.starts_with("8=FIX.4.4\x01"));
            assert!(msg.contains("35=5\x01")); // MsgType = Logout
            assert!(msg.contains("58=Session ended\x01")); // Text
            assert!(msg.contains("10=")); // Checksum present
        }

        #[test]
        fn encode_heartbeat_has_valid_structure() {
            let sim = default_simulator();
            let encoded = sim.encode_heartbeat(None);

            let msg = String::from_utf8_lossy(&encoded);

            assert!(msg.starts_with("8=FIX.4.4\x01"));
            assert!(msg.contains("35=0\x01")); // MsgType = Heartbeat
            assert!(msg.contains("10=")); // Checksum present
        }

        #[test]
        fn encode_heartbeat_with_test_req_id() {
            let sim = default_simulator();
            let encoded = sim.encode_heartbeat(Some("TEST-123"));

            let msg = String::from_utf8_lossy(&encoded);

            assert!(msg.contains("35=0\x01")); // MsgType = Heartbeat
            assert!(msg.contains("112=TEST-123\x01")); // TestReqID
        }

        #[test]
        fn encode_test_request_has_valid_structure() {
            let sim = default_simulator();
            let encoded = sim.encode_test_request("TR-001");

            let msg = String::from_utf8_lossy(&encoded);

            assert!(msg.starts_with("8=FIX.4.4\x01"));
            assert!(msg.contains("35=1\x01")); // MsgType = TestRequest
            assert!(msg.contains("112=TR-001\x01")); // TestReqID
            assert!(msg.contains("10=")); // Checksum present
        }

        #[test]
        fn encode_quote_has_valid_structure() {
            let sim = default_simulator();
            let encoded = sim.encode_quote("QR-001", "Q-001", "BTC/USD", side::BUY, "1.5");

            let msg = String::from_utf8_lossy(&encoded);

            assert!(msg.starts_with("8=FIX.4.4\x01"));
            assert!(msg.contains("35=S\x01")); // MsgType = Quote
            assert!(msg.contains("131=QR-001\x01")); // QuoteReqID
            assert!(msg.contains("117=Q-001\x01")); // QuoteID
            assert!(msg.contains("55=BTC/USD\x01")); // Symbol
            assert!(msg.contains("132=49950\x01")); // BidPx
            assert!(msg.contains("133=50050\x01")); // OfferPx
            assert!(msg.contains("10=")); // Checksum present
        }

        #[test]
        fn encode_execution_report_fill_has_valid_structure() {
            let sim = default_simulator();
            let encoded = sim.encode_execution_report_fill(
                "ORD-001",
                "Q-001",
                "EXEC-001",
                "BTC/USD",
                side::BUY,
                "1.0",
                "50050.0",
            );

            let msg = String::from_utf8_lossy(&encoded);

            assert!(msg.starts_with("8=FIX.4.4\x01"));
            assert!(msg.contains("35=8\x01")); // MsgType = ExecutionReport
            assert!(msg.contains("11=ORD-001\x01")); // ClOrdID
            assert!(msg.contains("17=EXEC-001\x01")); // ExecID
            assert!(msg.contains("150=F\x01")); // ExecType = Fill
            assert!(msg.contains("39=2\x01")); // OrdStatus = Filled
            assert!(msg.contains("31=50050.0\x01")); // LastPx
            assert!(msg.contains("10=")); // Checksum present
        }

        #[test]
        fn encode_execution_report_reject_has_valid_structure() {
            let sim = default_simulator();
            let encoded = sim.encode_execution_report_reject(
                "ORD-001",
                "EXEC-001",
                "BTC/USD",
                side::BUY,
                "1.0",
                "Insufficient funds",
            );

            let msg = String::from_utf8_lossy(&encoded);

            assert!(msg.starts_with("8=FIX.4.4\x01"));
            assert!(msg.contains("35=8\x01")); // MsgType = ExecutionReport
            assert!(msg.contains("150=8\x01")); // ExecType = Rejected
            assert!(msg.contains("39=8\x01")); // OrdStatus = Rejected
            assert!(msg.contains("58=Insufficient funds\x01")); // Text
            assert!(msg.contains("10=")); // Checksum present
        }

        #[test]
        fn sequence_numbers_increment() {
            let sim = default_simulator();

            // Encode multiple messages
            let msg1 = sim.encode_heartbeat(None);
            let msg2 = sim.encode_heartbeat(None);
            let msg3 = sim.encode_heartbeat(None);

            let s1 = String::from_utf8_lossy(&msg1);
            let s2 = String::from_utf8_lossy(&msg2);
            let s3 = String::from_utf8_lossy(&msg3);

            // Each message should have incrementing sequence numbers
            assert!(s1.contains("34=1\x01"));
            assert!(s2.contains("34=2\x01"));
            assert!(s3.contains("34=3\x01"));
        }
    }
}
