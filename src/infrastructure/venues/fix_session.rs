//! # FIX Session Management
//!
//! Session management for FIX protocol connections using IronFix.
//!
//! This module provides session-level functionality including:
//! - Sequence number management with [`SequenceManager`]
//! - Heartbeat timing with [`HeartbeatManager`]
//! - Session state tracking
//!
//! # IronFix Integration
//!
//! This module wraps and re-exports types from `ironfix-session`:
//! - [`ironfix_session::SequenceManager`] for atomic sequence number handling
//! - [`ironfix_session::HeartbeatManager`] for heartbeat/TestRequest logic
//! - [`ironfix_session::SessionConfig`] for session configuration
//!
//! # Example
//!
//! ```ignore
//! use otc_rfq::infrastructure::venues::fix_session::FixSession;
//! use otc_rfq::infrastructure::venues::fix_config::FixSessionConfig;
//!
//! let config = FixSessionConfig::new("SENDER", "TARGET")
//!     .with_host("fix.example.com")
//!     .with_port(9876);
//!
//! let session = FixSession::new(config);
//! let seq = session.allocate_sender_seq();
//! ```

use crate::infrastructure::venues::fix_config::FixSessionConfig;
use std::time::Duration;

// Re-export IronFix session types
pub use ironfix_session::{HeartbeatManager, SequenceManager};

/// FIX session state enumeration.
///
/// Represents the current state of a FIX session connection.
/// This mirrors the states from `ironfix_session` but as a simple enum
/// for easier runtime state tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum FixSessionState {
    /// Not connected to the counterparty.
    #[default]
    Disconnected,
    /// TCP connection in progress.
    Connecting,
    /// Logon message sent, awaiting response.
    LogonSent,
    /// Session is fully established and active.
    Active,
    /// Processing a resend request.
    Resending,
    /// Logout sent, awaiting confirmation.
    LogoutPending,
}

impl std::fmt::Display for FixSessionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Disconnected => write!(f, "DISCONNECTED"),
            Self::Connecting => write!(f, "CONNECTING"),
            Self::LogonSent => write!(f, "LOGON_SENT"),
            Self::Active => write!(f, "ACTIVE"),
            Self::Resending => write!(f, "RESENDING"),
            Self::LogoutPending => write!(f, "LOGOUT_PENDING"),
        }
    }
}

/// FIX session manager.
///
/// Combines sequence management, heartbeat handling, and state tracking
/// for a FIX protocol session.
///
/// # Thread Safety
///
/// The sequence manager uses atomic operations for thread-safe access.
/// The heartbeat manager requires mutable access and should be protected
/// by appropriate synchronization primitives in multi-threaded contexts.
#[derive(Debug)]
pub struct FixSession {
    /// Session configuration.
    config: FixSessionConfig,
    /// Sequence number manager.
    sequence_manager: SequenceManager,
    /// Heartbeat manager.
    heartbeat_manager: HeartbeatManager,
    /// Current session state.
    state: FixSessionState,
}

impl FixSession {
    /// Creates a new FIX session.
    ///
    /// # Arguments
    ///
    /// * `config` - Session configuration
    #[must_use]
    pub fn new(config: FixSessionConfig) -> Self {
        let heartbeat_interval = Duration::from_secs(u64::from(config.heartbeat_interval()));
        Self {
            config,
            sequence_manager: SequenceManager::new(),
            heartbeat_manager: HeartbeatManager::new(heartbeat_interval),
            state: FixSessionState::Disconnected,
        }
    }

    /// Creates a new FIX session with initial sequence numbers.
    ///
    /// Use this for session recovery when sequence numbers need to be restored.
    ///
    /// # Arguments
    ///
    /// * `config` - Session configuration
    /// * `sender_seq` - Initial sender sequence number
    /// * `target_seq` - Initial target sequence number
    #[must_use]
    pub fn with_initial_sequences(
        config: FixSessionConfig,
        sender_seq: u64,
        target_seq: u64,
    ) -> Self {
        let heartbeat_interval = Duration::from_secs(u64::from(config.heartbeat_interval()));
        Self {
            config,
            sequence_manager: SequenceManager::with_initial(sender_seq, target_seq),
            heartbeat_manager: HeartbeatManager::new(heartbeat_interval),
            state: FixSessionState::Disconnected,
        }
    }

    /// Returns the session configuration.
    #[inline]
    #[must_use]
    pub fn config(&self) -> &FixSessionConfig {
        &self.config
    }

    /// Returns the current session state.
    #[inline]
    #[must_use]
    pub fn state(&self) -> FixSessionState {
        self.state
    }

    /// Sets the session state.
    #[inline]
    pub fn set_state(&mut self, state: FixSessionState) {
        self.state = state;
    }

    /// Returns true if the session is active.
    #[inline]
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.state == FixSessionState::Active
    }

    /// Returns true if the session is disconnected.
    #[inline]
    #[must_use]
    pub fn is_disconnected(&self) -> bool {
        self.state == FixSessionState::Disconnected
    }

    /// Returns the next sender sequence number without incrementing.
    #[inline]
    #[must_use]
    pub fn next_sender_seq(&self) -> u64 {
        self.sequence_manager.next_sender_seq().value()
    }

    /// Returns the next target sequence number without incrementing.
    #[inline]
    #[must_use]
    pub fn next_target_seq(&self) -> u64 {
        self.sequence_manager.next_target_seq().value()
    }

    /// Allocates and returns the next sender sequence number.
    ///
    /// This atomically increments the sequence number.
    #[inline]
    pub fn allocate_sender_seq(&self) -> u64 {
        self.sequence_manager.allocate_sender_seq().value()
    }

    /// Increments the target sequence number.
    ///
    /// Call this after successfully processing an incoming message.
    #[inline]
    pub fn increment_target_seq(&self) {
        self.sequence_manager.increment_target_seq();
    }

    /// Resets both sequence numbers to 1.
    #[inline]
    pub fn reset_sequences(&self) {
        self.sequence_manager.reset();
    }

    /// Sets the sender sequence number.
    #[inline]
    pub fn set_sender_seq(&self, seq: u64) {
        self.sequence_manager.set_sender_seq(seq);
    }

    /// Sets the target sequence number.
    #[inline]
    pub fn set_target_seq(&self, seq: u64) {
        self.sequence_manager.set_target_seq(seq);
    }

    /// Records that a message was sent.
    #[inline]
    pub fn on_message_sent(&mut self) {
        self.heartbeat_manager.on_message_sent();
    }

    /// Records that a message was received.
    ///
    /// # Arguments
    ///
    /// * `is_heartbeat` - Whether the received message is a Heartbeat
    /// * `test_req_id` - The TestReqID from the Heartbeat, if present
    #[inline]
    pub fn on_message_received(&mut self, is_heartbeat: bool, test_req_id: Option<&str>) {
        self.heartbeat_manager
            .on_message_received(is_heartbeat, test_req_id);
    }

    /// Checks if a heartbeat should be sent.
    #[inline]
    #[must_use]
    pub fn should_send_heartbeat(&self) -> bool {
        self.heartbeat_manager.should_send_heartbeat()
    }

    /// Checks if a TestRequest should be sent.
    #[inline]
    #[must_use]
    pub fn should_send_test_request(&self) -> bool {
        self.heartbeat_manager.should_send_test_request()
    }

    /// Records that a TestRequest was sent.
    ///
    /// # Arguments
    ///
    /// * `test_req_id` - The TestReqID that was sent
    #[inline]
    pub fn on_test_request_sent(&mut self, test_req_id: impl Into<String>) {
        self.heartbeat_manager
            .on_test_request_sent(test_req_id.into());
    }

    /// Checks if the session has timed out due to no heartbeat response.
    #[inline]
    #[must_use]
    pub fn is_timed_out(&self) -> bool {
        self.heartbeat_manager.is_timed_out()
    }

    /// Transitions to connecting state.
    pub fn connect(&mut self) {
        self.state = FixSessionState::Connecting;
    }

    /// Transitions to logon sent state.
    pub fn send_logon(&mut self) {
        self.state = FixSessionState::LogonSent;
    }

    /// Transitions to active state after successful logon.
    pub fn logon_confirmed(&mut self) {
        self.state = FixSessionState::Active;
        if self.config.reset_on_logon() {
            self.reset_sequences();
        }
    }

    /// Transitions to logout pending state.
    pub fn send_logout(&mut self) {
        self.state = FixSessionState::LogoutPending;
    }

    /// Transitions to disconnected state.
    pub fn disconnect(&mut self) {
        if self.config.reset_on_disconnect() {
            self.reset_sequences();
        }
        self.state = FixSessionState::Disconnected;
    }

    /// Generates a unique session identifier.
    #[must_use]
    pub fn session_id(&self) -> String {
        format!(
            "{}->{}",
            self.config.sender_comp_id(),
            self.config.target_comp_id()
        )
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn test_config() -> FixSessionConfig {
        FixSessionConfig::new("SENDER", "TARGET")
            .with_host("localhost")
            .with_port(9876)
            .with_heartbeat_interval(30)
    }

    mod fix_session_state {
        use super::*;

        #[test]
        fn default_is_disconnected() {
            assert_eq!(FixSessionState::default(), FixSessionState::Disconnected);
        }

        #[test]
        fn display() {
            assert_eq!(FixSessionState::Active.to_string(), "ACTIVE");
            assert_eq!(FixSessionState::Disconnected.to_string(), "DISCONNECTED");
            assert_eq!(FixSessionState::LogonSent.to_string(), "LOGON_SENT");
        }
    }

    mod fix_session {
        use super::*;

        #[test]
        fn new_session_is_disconnected() {
            let session = FixSession::new(test_config());
            assert!(session.is_disconnected());
            assert!(!session.is_active());
        }

        #[test]
        fn sequence_numbers_start_at_one() {
            let session = FixSession::new(test_config());
            assert_eq!(session.next_sender_seq(), 1);
            assert_eq!(session.next_target_seq(), 1);
        }

        #[test]
        fn allocate_sender_seq_increments() {
            let session = FixSession::new(test_config());
            assert_eq!(session.allocate_sender_seq(), 1);
            assert_eq!(session.allocate_sender_seq(), 2);
            assert_eq!(session.allocate_sender_seq(), 3);
            assert_eq!(session.next_sender_seq(), 4);
        }

        #[test]
        fn increment_target_seq() {
            let session = FixSession::new(test_config());
            assert_eq!(session.next_target_seq(), 1);
            session.increment_target_seq();
            assert_eq!(session.next_target_seq(), 2);
        }

        #[test]
        fn reset_sequences() {
            let session = FixSession::new(test_config());
            session.allocate_sender_seq();
            session.allocate_sender_seq();
            session.increment_target_seq();

            session.reset_sequences();

            assert_eq!(session.next_sender_seq(), 1);
            assert_eq!(session.next_target_seq(), 1);
        }

        #[test]
        fn with_initial_sequences() {
            let session = FixSession::with_initial_sequences(test_config(), 100, 200);
            assert_eq!(session.next_sender_seq(), 100);
            assert_eq!(session.next_target_seq(), 200);
        }

        #[test]
        fn state_transitions() {
            let mut session = FixSession::new(test_config());

            assert!(session.is_disconnected());

            session.connect();
            assert_eq!(session.state(), FixSessionState::Connecting);

            session.send_logon();
            assert_eq!(session.state(), FixSessionState::LogonSent);

            session.logon_confirmed();
            assert!(session.is_active());

            session.send_logout();
            assert_eq!(session.state(), FixSessionState::LogoutPending);

            session.disconnect();
            assert!(session.is_disconnected());
        }

        #[test]
        fn session_id() {
            let session = FixSession::new(test_config());
            assert_eq!(session.session_id(), "SENDER->TARGET");
        }

        #[test]
        fn config_access() {
            let session = FixSession::new(test_config());
            assert_eq!(session.config().sender_comp_id(), "SENDER");
            assert_eq!(session.config().target_comp_id(), "TARGET");
        }
    }

    mod heartbeat {
        use super::*;

        #[test]
        fn initial_heartbeat_not_needed() {
            let session = FixSession::new(test_config());
            // Just created, shouldn't need heartbeat yet
            assert!(!session.should_send_heartbeat());
        }

        #[test]
        fn message_sent_resets_heartbeat_timer() {
            let mut session = FixSession::new(test_config());
            session.on_message_sent();
            assert!(!session.should_send_heartbeat());
        }

        #[test]
        fn message_received_updates_timer() {
            let mut session = FixSession::new(test_config());
            session.on_message_received(false, None);
            // Should not need test request immediately after receiving
            assert!(!session.should_send_test_request());
        }
    }
}
