//! # Circuit Breaker
//!
//! Circuit breaker pattern for venue failures.
//!
//! This module provides the [`CircuitBreaker`] which implements the circuit breaker
//! pattern to handle venue failures gracefully and prevent cascading failures.
//!
//! # State Machine
//!
//! ```text
//! Closed ──(failures >= threshold)──> Open
//!    ↑                                  │
//!    │                                  │
//!    │                          (timeout elapsed)
//!    │                                  │
//!    │                                  ↓
//!    └──(successes >= threshold)── HalfOpen
//!                                       │
//!                                  (any failure)
//!                                       │
//!                                       ↓
//!                                     Open
//! ```

use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::time::{Duration, Instant};

/// Circuit breaker state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CircuitState {
    /// Circuit is closed, requests pass through normally.
    #[default]
    Closed,
    /// Circuit is open, requests fail fast.
    Open,
    /// Circuit is half-open, testing if service recovered.
    HalfOpen,
}

impl fmt::Display for CircuitState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Closed => "CLOSED",
            Self::Open => "OPEN",
            Self::HalfOpen => "HALF_OPEN",
        };
        write!(f, "{}", s)
    }
}

/// Error returned when circuit breaker rejects a request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CircuitBreakerError {
    /// Circuit is open, request rejected.
    CircuitOpen {
        /// Time until circuit transitions to half-open.
        time_until_half_open_ms: Option<u64>,
    },
    /// Circuit is half-open, request limit reached.
    HalfOpenLimitReached {
        /// Maximum requests allowed in half-open state.
        max_requests: u32,
    },
}

impl fmt::Display for CircuitBreakerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CircuitOpen {
                time_until_half_open_ms,
            } => {
                if let Some(ms) = time_until_half_open_ms {
                    write!(f, "circuit is open, retry in {}ms", ms)
                } else {
                    write!(f, "circuit is open")
                }
            }
            Self::HalfOpenLimitReached { max_requests } => {
                write!(
                    f,
                    "circuit is half-open, max {} requests allowed",
                    max_requests
                )
            }
        }
    }
}

impl std::error::Error for CircuitBreakerError {}

/// Configuration for circuit breaker.
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Number of consecutive failures to trip the circuit.
    pub failure_threshold: u32,
    /// Number of consecutive successes in half-open to close the circuit.
    pub success_threshold: u32,
    /// Time in milliseconds before transitioning from open to half-open.
    pub reset_timeout_ms: u64,
    /// Maximum number of requests allowed in half-open state.
    pub half_open_max_requests: u32,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            success_threshold: 3,
            reset_timeout_ms: 30_000,
            half_open_max_requests: 3,
        }
    }
}

impl CircuitBreakerConfig {
    /// Creates a new configuration with custom values.
    #[must_use]
    pub fn new(
        failure_threshold: u32,
        success_threshold: u32,
        reset_timeout_ms: u64,
        half_open_max_requests: u32,
    ) -> Self {
        Self {
            failure_threshold,
            success_threshold,
            reset_timeout_ms,
            half_open_max_requests,
        }
    }

    /// Creates a configuration for aggressive failure detection.
    #[must_use]
    pub fn aggressive() -> Self {
        Self {
            failure_threshold: 3,
            success_threshold: 2,
            reset_timeout_ms: 10_000,
            half_open_max_requests: 1,
        }
    }

    /// Creates a configuration for lenient failure detection.
    #[must_use]
    pub fn lenient() -> Self {
        Self {
            failure_threshold: 10,
            success_threshold: 5,
            reset_timeout_ms: 60_000,
            half_open_max_requests: 5,
        }
    }
}

/// Circuit breaker for handling venue failures.
///
/// Implements the circuit breaker pattern to prevent cascading failures
/// when a venue becomes unavailable.
///
/// # Thread Safety
///
/// This implementation is thread-safe and can be shared across threads.
///
/// # Examples
///
/// ```
/// use otc_rfq::application::services::circuit_breaker::{
///     CircuitBreaker, CircuitBreakerConfig, CircuitState,
/// };
///
/// let config = CircuitBreakerConfig::default();
/// let breaker = CircuitBreaker::new("venue-1", config);
///
/// // Check if request can proceed
/// if breaker.can_execute().is_ok() {
///     // Make request...
///     // On success:
///     breaker.record_success();
///     // On failure:
///     // breaker.record_failure();
/// }
/// ```
#[derive(Debug)]
pub struct CircuitBreaker {
    /// Name/identifier for this circuit breaker.
    name: String,
    /// Current state of the circuit.
    state: RwLock<CircuitState>,
    /// Number of consecutive failures.
    failure_count: AtomicU32,
    /// Number of consecutive successes (in half-open state).
    success_count: AtomicU32,
    /// Number of requests in half-open state.
    half_open_requests: AtomicU32,
    /// Timestamp of last failure (as millis since UNIX epoch).
    last_failure_time: AtomicU64,
    /// Instant when circuit was opened (for timeout calculation).
    opened_at: RwLock<Option<Instant>>,
    /// Configuration.
    config: CircuitBreakerConfig,
}

impl CircuitBreaker {
    /// Creates a new circuit breaker with the given configuration.
    #[must_use]
    pub fn new(name: impl Into<String>, config: CircuitBreakerConfig) -> Self {
        Self {
            name: name.into(),
            state: RwLock::new(CircuitState::Closed),
            failure_count: AtomicU32::new(0),
            success_count: AtomicU32::new(0),
            half_open_requests: AtomicU32::new(0),
            last_failure_time: AtomicU64::new(0),
            opened_at: RwLock::new(None),
            config,
        }
    }

    /// Creates a new circuit breaker with default configuration.
    #[must_use]
    pub fn with_defaults(name: impl Into<String>) -> Self {
        Self::new(name, CircuitBreakerConfig::default())
    }

    /// Returns the name of this circuit breaker.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the current state of the circuit.
    #[must_use]
    pub fn state(&self) -> CircuitState {
        *self.read_state()
    }

    /// Reads the state lock, recovering from poison if needed.
    fn read_state(&self) -> RwLockReadGuard<'_, CircuitState> {
        self.state.read().unwrap_or_else(PoisonError::into_inner)
    }

    /// Writes the state lock, recovering from poison if needed.
    fn write_state(&self) -> RwLockWriteGuard<'_, CircuitState> {
        self.state.write().unwrap_or_else(PoisonError::into_inner)
    }

    /// Reads the opened_at lock, recovering from poison if needed.
    fn read_opened_at(&self) -> RwLockReadGuard<'_, Option<Instant>> {
        self.opened_at
            .read()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// Writes the opened_at lock, recovering from poison if needed.
    fn write_opened_at(&self) -> RwLockWriteGuard<'_, Option<Instant>> {
        self.opened_at
            .write()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// Returns the current failure count.
    #[must_use]
    pub fn failure_count(&self) -> u32 {
        self.failure_count.load(Ordering::SeqCst)
    }

    /// Returns the current success count.
    #[must_use]
    pub fn success_count(&self) -> u32 {
        self.success_count.load(Ordering::SeqCst)
    }

    /// Returns the configuration.
    #[must_use]
    pub fn config(&self) -> &CircuitBreakerConfig {
        &self.config
    }

    /// Checks if a request can be executed.
    ///
    /// # Returns
    ///
    /// - `Ok(())` if the request can proceed
    /// - `Err(CircuitBreakerError)` if the circuit is open or half-open limit reached
    ///
    /// # Errors
    ///
    /// Returns `CircuitBreakerError::CircuitOpen` if the circuit is open.
    /// Returns `CircuitBreakerError::HalfOpenLimitReached` if half-open request limit is reached.
    pub fn can_execute(&self) -> Result<(), CircuitBreakerError> {
        let current_state = self.state();

        match current_state {
            CircuitState::Closed => Ok(()),
            CircuitState::Open => {
                // Check if timeout has elapsed
                if self.should_transition_to_half_open() {
                    self.transition_to_half_open();
                    // Allow this request as the first half-open request
                    self.half_open_requests.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                } else {
                    Err(CircuitBreakerError::CircuitOpen {
                        time_until_half_open_ms: self.time_until_half_open(),
                    })
                }
            }
            CircuitState::HalfOpen => {
                let current_requests = self.half_open_requests.load(Ordering::SeqCst);
                if current_requests < self.config.half_open_max_requests {
                    self.half_open_requests.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                } else {
                    Err(CircuitBreakerError::HalfOpenLimitReached {
                        max_requests: self.config.half_open_max_requests,
                    })
                }
            }
        }
    }

    /// Records a successful request.
    ///
    /// In closed state, resets failure count.
    /// In half-open state, increments success count and may close the circuit.
    pub fn record_success(&self) {
        let current_state = self.state();

        match current_state {
            CircuitState::Closed => {
                // Reset failure count on success
                self.failure_count.store(0, Ordering::SeqCst);
            }
            CircuitState::HalfOpen => {
                let new_count = self.success_count.fetch_add(1, Ordering::SeqCst) + 1;
                if new_count >= self.config.success_threshold {
                    self.transition_to_closed();
                }
            }
            CircuitState::Open => {
                // Ignore success in open state (shouldn't happen normally)
            }
        }
    }

    /// Records a failed request.
    ///
    /// In closed state, increments failure count and may open the circuit.
    /// In half-open state, immediately opens the circuit.
    pub fn record_failure(&self) {
        let current_state = self.state();

        match current_state {
            CircuitState::Closed => {
                let new_count = self.failure_count.fetch_add(1, Ordering::SeqCst) + 1;
                self.update_last_failure_time();
                if new_count >= self.config.failure_threshold {
                    self.transition_to_open();
                }
            }
            CircuitState::HalfOpen => {
                // Any failure in half-open immediately opens the circuit
                self.update_last_failure_time();
                self.transition_to_open();
            }
            CircuitState::Open => {
                // Update failure time but don't increment count
                self.update_last_failure_time();
            }
        }
    }

    /// Resets the circuit breaker to closed state.
    pub fn reset(&self) {
        let mut state = self.write_state();
        *state = CircuitState::Closed;
        self.failure_count.store(0, Ordering::SeqCst);
        self.success_count.store(0, Ordering::SeqCst);
        self.half_open_requests.store(0, Ordering::SeqCst);
        self.last_failure_time.store(0, Ordering::SeqCst);
        *self.write_opened_at() = None;
    }

    /// Returns true if the circuit is closed.
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.state() == CircuitState::Closed
    }

    /// Returns true if the circuit is open.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.state() == CircuitState::Open
    }

    /// Returns true if the circuit is half-open.
    #[must_use]
    pub fn is_half_open(&self) -> bool {
        self.state() == CircuitState::HalfOpen
    }

    fn should_transition_to_half_open(&self) -> bool {
        let opened_at = self.read_opened_at();
        if let Some(instant) = *opened_at {
            let elapsed = instant.elapsed();
            elapsed >= Duration::from_millis(self.config.reset_timeout_ms)
        } else {
            false
        }
    }

    fn time_until_half_open(&self) -> Option<u64> {
        let opened_at = self.read_opened_at();
        if let Some(instant) = *opened_at {
            let elapsed = instant.elapsed();
            let timeout = Duration::from_millis(self.config.reset_timeout_ms);
            if elapsed < timeout {
                Some((timeout - elapsed).as_millis() as u64)
            } else {
                Some(0)
            }
        } else {
            None
        }
    }

    fn transition_to_open(&self) {
        let mut state = self.write_state();
        *state = CircuitState::Open;
        self.success_count.store(0, Ordering::SeqCst);
        self.half_open_requests.store(0, Ordering::SeqCst);
        *self.write_opened_at() = Some(Instant::now());
    }

    fn transition_to_half_open(&self) {
        let mut state = self.write_state();
        *state = CircuitState::HalfOpen;
        self.success_count.store(0, Ordering::SeqCst);
        self.half_open_requests.store(0, Ordering::SeqCst);
    }

    fn transition_to_closed(&self) {
        let mut state = self.write_state();
        *state = CircuitState::Closed;
        self.failure_count.store(0, Ordering::SeqCst);
        self.success_count.store(0, Ordering::SeqCst);
        self.half_open_requests.store(0, Ordering::SeqCst);
        *self.write_opened_at() = None;
    }

    fn update_last_failure_time(&self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        self.last_failure_time.store(now, Ordering::SeqCst);
    }
}

impl fmt::Display for CircuitBreaker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "CircuitBreaker({}: {} failures={} successes={})",
            self.name,
            self.state(),
            self.failure_count(),
            self.success_count()
        )
    }
}

/// Result type for circuit breaker operations.
pub type CircuitBreakerResult<T> = Result<T, CircuitBreakerError>;

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    fn create_test_config() -> CircuitBreakerConfig {
        CircuitBreakerConfig {
            failure_threshold: 3,
            success_threshold: 2,
            reset_timeout_ms: 100, // Short timeout for tests
            half_open_max_requests: 2,
        }
    }

    #[test]
    fn circuit_breaker_starts_closed() {
        let breaker = CircuitBreaker::new("test", create_test_config());

        assert!(breaker.is_closed());
        assert_eq!(breaker.state(), CircuitState::Closed);
        assert_eq!(breaker.failure_count(), 0);
        assert_eq!(breaker.success_count(), 0);
    }

    #[test]
    fn circuit_breaker_allows_requests_when_closed() {
        let breaker = CircuitBreaker::new("test", create_test_config());

        assert!(breaker.can_execute().is_ok());
        assert!(breaker.can_execute().is_ok());
        assert!(breaker.can_execute().is_ok());
    }

    #[test]
    fn circuit_breaker_trips_on_failures() {
        let breaker = CircuitBreaker::new("test", create_test_config());

        // Record failures up to threshold
        breaker.record_failure();
        assert!(breaker.is_closed());
        assert_eq!(breaker.failure_count(), 1);

        breaker.record_failure();
        assert!(breaker.is_closed());
        assert_eq!(breaker.failure_count(), 2);

        breaker.record_failure();
        assert!(breaker.is_open());
        assert_eq!(breaker.failure_count(), 3);
    }

    #[test]
    fn circuit_breaker_rejects_when_open() {
        let breaker = CircuitBreaker::new("test", create_test_config());

        // Trip the circuit
        for _ in 0..3 {
            breaker.record_failure();
        }

        assert!(breaker.is_open());

        // Requests should be rejected
        let result = breaker.can_execute();
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            CircuitBreakerError::CircuitOpen { .. }
        ));
    }

    #[test]
    fn circuit_breaker_transitions_to_half_open() {
        let breaker = CircuitBreaker::new("test", create_test_config());

        // Trip the circuit
        for _ in 0..3 {
            breaker.record_failure();
        }
        assert!(breaker.is_open());

        // Wait for timeout
        thread::sleep(Duration::from_millis(150));

        // Next request should transition to half-open
        assert!(breaker.can_execute().is_ok());
        assert!(breaker.is_half_open());
    }

    #[test]
    fn circuit_breaker_closes_on_success_in_half_open() {
        let breaker = CircuitBreaker::new("test", create_test_config());

        // Trip the circuit
        for _ in 0..3 {
            breaker.record_failure();
        }

        // Wait for timeout
        thread::sleep(Duration::from_millis(150));

        // Transition to half-open
        assert!(breaker.can_execute().is_ok());
        assert!(breaker.is_half_open());

        // Record successes
        breaker.record_success();
        assert!(breaker.is_half_open());
        assert_eq!(breaker.success_count(), 1);

        breaker.record_success();
        assert!(breaker.is_closed());
        assert_eq!(breaker.failure_count(), 0);
    }

    #[test]
    fn circuit_breaker_reopens_on_failure_in_half_open() {
        let breaker = CircuitBreaker::new("test", create_test_config());

        // Trip the circuit
        for _ in 0..3 {
            breaker.record_failure();
        }

        // Wait for timeout
        thread::sleep(Duration::from_millis(150));

        // Transition to half-open
        assert!(breaker.can_execute().is_ok());
        assert!(breaker.is_half_open());

        // Any failure should reopen
        breaker.record_failure();
        assert!(breaker.is_open());
    }

    #[test]
    fn circuit_breaker_limits_half_open_requests() {
        let breaker = CircuitBreaker::new("test", create_test_config());

        // Trip the circuit
        for _ in 0..3 {
            breaker.record_failure();
        }

        // Wait for timeout
        thread::sleep(Duration::from_millis(150));

        // First request transitions to half-open
        assert!(breaker.can_execute().is_ok());
        assert!(breaker.is_half_open());

        // Second request allowed
        assert!(breaker.can_execute().is_ok());

        // Third request should be rejected (max is 2)
        let result = breaker.can_execute();
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            CircuitBreakerError::HalfOpenLimitReached { max_requests: 2 }
        ));
    }

    #[test]
    fn circuit_breaker_reset() {
        let breaker = CircuitBreaker::new("test", create_test_config());

        // Trip the circuit
        for _ in 0..3 {
            breaker.record_failure();
        }
        assert!(breaker.is_open());

        // Reset
        breaker.reset();

        assert!(breaker.is_closed());
        assert_eq!(breaker.failure_count(), 0);
        assert_eq!(breaker.success_count(), 0);
        assert!(breaker.can_execute().is_ok());
    }

    #[test]
    fn circuit_breaker_success_resets_failure_count() {
        let breaker = CircuitBreaker::new("test", create_test_config());

        // Record some failures
        breaker.record_failure();
        breaker.record_failure();
        assert_eq!(breaker.failure_count(), 2);

        // Success should reset
        breaker.record_success();
        assert_eq!(breaker.failure_count(), 0);
        assert!(breaker.is_closed());
    }

    #[test]
    fn circuit_breaker_config_default() {
        let config = CircuitBreakerConfig::default();

        assert_eq!(config.failure_threshold, 5);
        assert_eq!(config.success_threshold, 3);
        assert_eq!(config.reset_timeout_ms, 30_000);
        assert_eq!(config.half_open_max_requests, 3);
    }

    #[test]
    fn circuit_breaker_config_aggressive() {
        let config = CircuitBreakerConfig::aggressive();

        assert_eq!(config.failure_threshold, 3);
        assert_eq!(config.success_threshold, 2);
        assert_eq!(config.reset_timeout_ms, 10_000);
        assert_eq!(config.half_open_max_requests, 1);
    }

    #[test]
    fn circuit_breaker_config_lenient() {
        let config = CircuitBreakerConfig::lenient();

        assert_eq!(config.failure_threshold, 10);
        assert_eq!(config.success_threshold, 5);
        assert_eq!(config.reset_timeout_ms, 60_000);
        assert_eq!(config.half_open_max_requests, 5);
    }

    #[test]
    fn circuit_state_display() {
        assert_eq!(CircuitState::Closed.to_string(), "CLOSED");
        assert_eq!(CircuitState::Open.to_string(), "OPEN");
        assert_eq!(CircuitState::HalfOpen.to_string(), "HALF_OPEN");
    }

    #[test]
    fn circuit_breaker_error_display() {
        let open_err = CircuitBreakerError::CircuitOpen {
            time_until_half_open_ms: Some(5000),
        };
        assert!(open_err.to_string().contains("5000ms"));

        let open_err_no_time = CircuitBreakerError::CircuitOpen {
            time_until_half_open_ms: None,
        };
        assert!(open_err_no_time.to_string().contains("circuit is open"));

        let half_open_err = CircuitBreakerError::HalfOpenLimitReached { max_requests: 3 };
        assert!(half_open_err.to_string().contains("3 requests"));
    }

    #[test]
    fn circuit_breaker_display() {
        let breaker = CircuitBreaker::new("venue-1", create_test_config());
        let display = breaker.to_string();

        assert!(display.contains("venue-1"));
        assert!(display.contains("CLOSED"));
    }

    #[test]
    fn circuit_breaker_name() {
        let breaker = CircuitBreaker::new("my-venue", create_test_config());
        assert_eq!(breaker.name(), "my-venue");
    }

    #[test]
    fn circuit_breaker_with_defaults() {
        let breaker = CircuitBreaker::with_defaults("test");

        assert!(breaker.is_closed());
        assert_eq!(breaker.config().failure_threshold, 5);
    }
}
