//! # Retry Policy
//!
//! Retry policy with exponential backoff for handling transient failures.
//!
//! This module provides [`RetryPolicy`] for configuring retry behavior and
//! [`execute_with_retry`] for executing operations with automatic retries.
//!
//! # Features
//!
//! - Configurable retry parameters (max retries, delays, backoff multiplier)
//! - Exponential backoff with optional jitter to prevent thundering herd
//! - Support for distinguishing retryable vs non-retryable errors
//!
//! # Example
//!
//! ```
//! use otc_rfq::application::services::retry::{RetryPolicy, execute_with_retry, Retryable};
//!
//! #[derive(Debug)]
//! struct MyError(bool);
//!
//! impl Retryable for MyError {
//!     fn is_retryable(&self) -> bool {
//!         self.0
//!     }
//! }
//!
//! async fn fallible_operation() -> Result<String, MyError> {
//!     // ... operation that might fail
//!     Ok("success".to_string())
//! }
//!
//! # async fn example() {
//! let policy = RetryPolicy::default();
//! let result = execute_with_retry(&policy, || fallible_operation()).await;
//! # }
//! ```

use rand::Rng;
use std::fmt;
use std::future::Future;
use std::time::Duration;
use tokio::time::sleep;

/// Trait for errors that can indicate whether they are retryable.
pub trait Retryable {
    /// Returns true if the error is transient and the operation should be retried.
    fn is_retryable(&self) -> bool;
}

/// Configuration for retry behavior.
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// Maximum number of retry attempts (0 means no retries, just the initial attempt).
    pub max_retries: u32,
    /// Initial delay before the first retry, in milliseconds.
    pub initial_delay_ms: u64,
    /// Maximum delay cap, in milliseconds.
    pub max_delay_ms: u64,
    /// Multiplier for exponential backoff.
    pub backoff_multiplier: f64,
    /// Jitter factor (0.0-1.0) to randomize delays and prevent thundering herd.
    pub jitter_factor: f64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay_ms: 100,
            max_delay_ms: 10_000,
            backoff_multiplier: 2.0,
            jitter_factor: 0.1,
        }
    }
}

impl RetryPolicy {
    /// Creates a new retry policy with custom parameters.
    #[must_use]
    pub fn new(
        max_retries: u32,
        initial_delay_ms: u64,
        max_delay_ms: u64,
        backoff_multiplier: f64,
        jitter_factor: f64,
    ) -> Self {
        Self {
            max_retries,
            initial_delay_ms,
            max_delay_ms,
            backoff_multiplier,
            jitter_factor: jitter_factor.clamp(0.0, 1.0),
        }
    }

    /// Creates a policy with no retries (fail fast).
    #[must_use]
    pub fn no_retry() -> Self {
        Self {
            max_retries: 0,
            ..Default::default()
        }
    }

    /// Creates an aggressive retry policy with more attempts and shorter delays.
    #[must_use]
    pub fn aggressive() -> Self {
        Self {
            max_retries: 5,
            initial_delay_ms: 50,
            max_delay_ms: 5_000,
            backoff_multiplier: 1.5,
            jitter_factor: 0.2,
        }
    }

    /// Creates a conservative retry policy with fewer attempts and longer delays.
    #[must_use]
    pub fn conservative() -> Self {
        Self {
            max_retries: 2,
            initial_delay_ms: 500,
            max_delay_ms: 30_000,
            backoff_multiplier: 3.0,
            jitter_factor: 0.1,
        }
    }

    /// Calculates the delay for a given attempt number (0-indexed).
    ///
    /// The delay is calculated as:
    /// `min(initial_delay * (multiplier ^ attempt), max_delay)`
    ///
    /// # Arguments
    ///
    /// * `attempt` - The attempt number (0 for first retry, 1 for second, etc.)
    #[must_use]
    pub fn calculate_delay(&self, attempt: u32) -> Duration {
        let base_delay =
            self.initial_delay_ms as f64 * self.backoff_multiplier.powi(attempt as i32);
        let capped_delay = base_delay.min(self.max_delay_ms as f64);
        Duration::from_millis(capped_delay as u64)
    }

    /// Calculates the delay with jitter applied.
    ///
    /// Jitter is applied as: `delay * (1 - jitter_factor * random())`
    ///
    /// # Arguments
    ///
    /// * `attempt` - The attempt number (0 for first retry, 1 for second, etc.)
    #[must_use]
    pub fn calculate_delay_with_jitter(&self, attempt: u32) -> Duration {
        let base_delay = self.calculate_delay(attempt);
        if self.jitter_factor <= 0.0 {
            return base_delay;
        }

        let mut rng = rand::rng();
        let jitter: f64 = rng.random();
        let jitter_multiplier = 1.0 - (self.jitter_factor * jitter);
        let jittered_ms = base_delay.as_millis() as f64 * jitter_multiplier;
        Duration::from_millis(jittered_ms.max(1.0) as u64)
    }

    /// Returns true if more retries are allowed for the given attempt count.
    #[must_use]
    pub fn should_retry(&self, attempts_made: u32) -> bool {
        attempts_made < self.max_retries
    }
}

/// Error returned when retry execution fails.
#[derive(Debug)]
pub enum RetryError<E> {
    /// All retry attempts were exhausted.
    MaxRetriesExceeded {
        /// The last error encountered.
        last_error: E,
        /// Total number of attempts made.
        attempts: u32,
    },
    /// The error was marked as non-retryable.
    NonRetryable {
        /// The non-retryable error.
        error: E,
        /// Number of attempts made before encountering non-retryable error.
        attempts: u32,
    },
}

impl<E: fmt::Display> fmt::Display for RetryError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MaxRetriesExceeded {
                last_error,
                attempts,
            } => {
                write!(
                    f,
                    "max retries exceeded after {} attempts: {}",
                    attempts, last_error
                )
            }
            Self::NonRetryable { error, attempts } => {
                write!(
                    f,
                    "non-retryable error after {} attempts: {}",
                    attempts, error
                )
            }
        }
    }
}

impl<E: fmt::Debug + fmt::Display> std::error::Error for RetryError<E> {}

impl<E> RetryError<E> {
    /// Returns the underlying error.
    #[must_use]
    pub fn into_inner(self) -> E {
        match self {
            Self::MaxRetriesExceeded { last_error, .. } => last_error,
            Self::NonRetryable { error, .. } => error,
        }
    }

    /// Returns a reference to the underlying error.
    #[must_use]
    pub fn inner(&self) -> &E {
        match self {
            Self::MaxRetriesExceeded { last_error, .. } => last_error,
            Self::NonRetryable { error, .. } => error,
        }
    }

    /// Returns the number of attempts made.
    #[must_use]
    pub fn attempts(&self) -> u32 {
        match self {
            Self::MaxRetriesExceeded { attempts, .. } | Self::NonRetryable { attempts, .. } => {
                *attempts
            }
        }
    }

    /// Returns true if this was a max retries exceeded error.
    #[must_use]
    pub fn is_max_retries_exceeded(&self) -> bool {
        matches!(self, Self::MaxRetriesExceeded { .. })
    }

    /// Returns true if this was a non-retryable error.
    #[must_use]
    pub fn is_non_retryable(&self) -> bool {
        matches!(self, Self::NonRetryable { .. })
    }
}

/// Executes an async operation with retry logic.
///
/// # Arguments
///
/// * `policy` - The retry policy to use
/// * `operation` - A closure that returns a future producing the result
///
/// # Returns
///
/// * `Ok(T)` if the operation succeeds
/// * `Err(RetryError<E>)` if all retries are exhausted or a non-retryable error occurs
///
/// # Errors
///
/// Returns `RetryError::MaxRetriesExceeded` if all retry attempts are exhausted.
/// Returns `RetryError::NonRetryable` if a non-retryable error is encountered.
///
/// # Example
///
/// ```
/// use otc_rfq::application::services::retry::{RetryPolicy, execute_with_retry, Retryable};
///
/// #[derive(Debug)]
/// struct TransientError;
///
/// impl Retryable for TransientError {
///     fn is_retryable(&self) -> bool {
///         true
///     }
/// }
///
/// # async fn example() {
/// let policy = RetryPolicy::default();
/// let mut attempt = 0;
///
/// let result = execute_with_retry(&policy, || {
///     attempt += 1;
///     async move {
///         if attempt < 3 {
///             Err(TransientError)
///         } else {
///             Ok("success")
///         }
///     }
/// }).await;
/// # }
/// ```
pub async fn execute_with_retry<F, Fut, T, E>(
    policy: &RetryPolicy,
    mut operation: F,
) -> Result<T, RetryError<E>>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
    E: Retryable,
{
    let mut attempts = 0u32;

    loop {
        attempts = attempts.saturating_add(1);

        match operation().await {
            Ok(result) => return Ok(result),
            Err(error) => {
                if !error.is_retryable() {
                    return Err(RetryError::NonRetryable { error, attempts });
                }

                if !policy.should_retry(attempts) {
                    return Err(RetryError::MaxRetriesExceeded {
                        last_error: error,
                        attempts,
                    });
                }

                // Wait before retrying (attempt - 1 because attempt is 1-indexed here)
                let delay = policy.calculate_delay_with_jitter(attempts.saturating_sub(1));
                sleep(delay).await;
            }
        }
    }
}

/// Result type for retry operations.
pub type RetryResult<T, E> = Result<T, RetryError<E>>;

/// A simple wrapper to make any error retryable.
#[derive(Debug, Clone)]
pub struct AlwaysRetryable<E>(pub E);

impl<E> Retryable for AlwaysRetryable<E> {
    fn is_retryable(&self) -> bool {
        true
    }
}

impl<E: fmt::Display> fmt::Display for AlwaysRetryable<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<E: fmt::Debug + fmt::Display> std::error::Error for AlwaysRetryable<E> {}

/// A simple wrapper to make any error non-retryable.
#[derive(Debug, Clone)]
pub struct NeverRetryable<E>(pub E);

impl<E> Retryable for NeverRetryable<E> {
    fn is_retryable(&self) -> bool {
        false
    }
}

impl<E: fmt::Display> fmt::Display for NeverRetryable<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<E: fmt::Debug + fmt::Display> std::error::Error for NeverRetryable<E> {}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[derive(Debug, Clone)]
    struct TestError {
        retryable: bool,
        message: String,
    }

    impl TestError {
        fn retryable(msg: &str) -> Self {
            Self {
                retryable: true,
                message: msg.to_string(),
            }
        }

        fn non_retryable(msg: &str) -> Self {
            Self {
                retryable: false,
                message: msg.to_string(),
            }
        }
    }

    impl fmt::Display for TestError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{}", self.message)
        }
    }

    impl Retryable for TestError {
        fn is_retryable(&self) -> bool {
            self.retryable
        }
    }

    #[test]
    fn retry_policy_default() {
        let policy = RetryPolicy::default();

        assert_eq!(policy.max_retries, 3);
        assert_eq!(policy.initial_delay_ms, 100);
        assert_eq!(policy.max_delay_ms, 10_000);
        assert!((policy.backoff_multiplier - 2.0).abs() < f64::EPSILON);
        assert!((policy.jitter_factor - 0.1).abs() < f64::EPSILON);
    }

    #[test]
    fn retry_policy_no_retry() {
        let policy = RetryPolicy::no_retry();

        assert_eq!(policy.max_retries, 0);
        assert!(!policy.should_retry(0));
    }

    #[test]
    fn retry_policy_aggressive() {
        let policy = RetryPolicy::aggressive();

        assert_eq!(policy.max_retries, 5);
        assert_eq!(policy.initial_delay_ms, 50);
    }

    #[test]
    fn retry_policy_conservative() {
        let policy = RetryPolicy::conservative();

        assert_eq!(policy.max_retries, 2);
        assert_eq!(policy.initial_delay_ms, 500);
    }

    #[test]
    fn retry_policy_calculates_delay() {
        let policy = RetryPolicy {
            max_retries: 5,
            initial_delay_ms: 100,
            max_delay_ms: 10_000,
            backoff_multiplier: 2.0,
            jitter_factor: 0.0, // No jitter for deterministic test
        };

        assert_eq!(policy.calculate_delay(0), Duration::from_millis(100));
        assert_eq!(policy.calculate_delay(1), Duration::from_millis(200));
        assert_eq!(policy.calculate_delay(2), Duration::from_millis(400));
        assert_eq!(policy.calculate_delay(3), Duration::from_millis(800));
    }

    #[test]
    fn retry_policy_respects_max_delay() {
        let policy = RetryPolicy {
            max_retries: 10,
            initial_delay_ms: 1000,
            max_delay_ms: 5000,
            backoff_multiplier: 2.0,
            jitter_factor: 0.0,
        };

        // 1000 * 2^3 = 8000, but capped at 5000
        assert_eq!(policy.calculate_delay(3), Duration::from_millis(5000));
        assert_eq!(policy.calculate_delay(10), Duration::from_millis(5000));
    }

    #[test]
    fn retry_policy_applies_jitter() {
        let policy = RetryPolicy {
            max_retries: 3,
            initial_delay_ms: 1000,
            max_delay_ms: 10_000,
            backoff_multiplier: 2.0,
            jitter_factor: 0.5,
        };

        // With 50% jitter, delay should be between 500-1000ms for attempt 0
        let mut delays = Vec::new();
        for _ in 0..10 {
            delays.push(policy.calculate_delay_with_jitter(0));
        }

        // All delays should be <= base delay
        let base_delay = Duration::from_millis(1000);
        for delay in &delays {
            assert!(*delay <= base_delay);
            assert!(*delay >= Duration::from_millis(500));
        }

        // With randomness, we should see some variation
        let unique_delays: std::collections::HashSet<_> =
            delays.iter().map(|d| d.as_millis()).collect();
        // It's very unlikely all 10 random values are identical
        assert!(unique_delays.len() > 1 || delays.iter().all(|d| *d == base_delay));
    }

    #[test]
    fn retry_policy_should_retry() {
        let policy = RetryPolicy {
            max_retries: 3,
            ..Default::default()
        };

        assert!(policy.should_retry(0));
        assert!(policy.should_retry(1));
        assert!(policy.should_retry(2));
        assert!(!policy.should_retry(3));
        assert!(!policy.should_retry(4));
    }

    #[tokio::test]
    async fn execute_with_retry_succeeds_first_try() {
        let policy = RetryPolicy::default();
        let attempts = Arc::new(AtomicU32::new(0));
        let attempts_clone = Arc::clone(&attempts);

        let result: Result<&str, RetryError<TestError>> = execute_with_retry(&policy, || {
            attempts_clone.fetch_add(1, Ordering::SeqCst);
            async { Ok("success") }
        })
        .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn execute_with_retry_succeeds_after_retries() {
        let policy = RetryPolicy {
            max_retries: 5,
            initial_delay_ms: 10, // Short delay for tests
            max_delay_ms: 100,
            backoff_multiplier: 2.0,
            jitter_factor: 0.0,
        };

        let attempts = Arc::new(AtomicU32::new(0));
        let attempts_clone = Arc::clone(&attempts);

        let result: Result<&str, RetryError<TestError>> = execute_with_retry(&policy, || {
            let current = attempts_clone.fetch_add(1, Ordering::SeqCst) + 1;
            async move {
                if current < 3 {
                    Err(TestError::retryable("transient failure"))
                } else {
                    Ok("success")
                }
            }
        })
        .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn execute_with_retry_exhausts_retries() {
        let policy = RetryPolicy {
            max_retries: 3,
            initial_delay_ms: 10,
            max_delay_ms: 100,
            backoff_multiplier: 2.0,
            jitter_factor: 0.0,
        };

        let attempts = Arc::new(AtomicU32::new(0));
        let attempts_clone = Arc::clone(&attempts);

        let result: Result<&str, RetryError<TestError>> = execute_with_retry(&policy, || {
            attempts_clone.fetch_add(1, Ordering::SeqCst);
            async { Err(TestError::retryable("always fails")) }
        })
        .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.is_max_retries_exceeded());
        // With max_retries=3: attempt 1 (fail, should_retry(1)=true, retry),
        // attempt 2 (fail, should_retry(2)=true, retry),
        // attempt 3 (fail, should_retry(3)=false, return error)
        // Total: 1 initial + 2 retries = 3 attempts
        assert_eq!(err.attempts(), 3);
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn execute_with_retry_stops_on_non_retryable() {
        let policy = RetryPolicy {
            max_retries: 5,
            initial_delay_ms: 10,
            max_delay_ms: 100,
            backoff_multiplier: 2.0,
            jitter_factor: 0.0,
        };

        let attempts = Arc::new(AtomicU32::new(0));
        let attempts_clone = Arc::clone(&attempts);

        let result: Result<&str, RetryError<TestError>> = execute_with_retry(&policy, || {
            let current = attempts_clone.fetch_add(1, Ordering::SeqCst) + 1;
            async move {
                if current < 2 {
                    Err(TestError::retryable("transient"))
                } else {
                    Err(TestError::non_retryable("permanent failure"))
                }
            }
        })
        .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.is_non_retryable());
        assert_eq!(err.attempts(), 2);
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn execute_with_retry_no_retry_policy() {
        let policy = RetryPolicy::no_retry();
        let attempts = Arc::new(AtomicU32::new(0));
        let attempts_clone = Arc::clone(&attempts);

        let result: Result<&str, RetryError<TestError>> = execute_with_retry(&policy, || {
            attempts_clone.fetch_add(1, Ordering::SeqCst);
            async { Err(TestError::retryable("fails")) }
        })
        .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.is_max_retries_exceeded());
        assert_eq!(err.attempts(), 1); // Only initial attempt
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn retry_error_display() {
        let err: RetryError<TestError> = RetryError::MaxRetriesExceeded {
            last_error: TestError::retryable("timeout"),
            attempts: 4,
        };
        assert!(err.to_string().contains("max retries exceeded"));
        assert!(err.to_string().contains("4 attempts"));

        let err: RetryError<TestError> = RetryError::NonRetryable {
            error: TestError::non_retryable("auth failed"),
            attempts: 2,
        };
        assert!(err.to_string().contains("non-retryable"));
        assert!(err.to_string().contains("2 attempts"));
    }

    #[test]
    fn retry_error_into_inner() {
        let err: RetryError<TestError> = RetryError::MaxRetriesExceeded {
            last_error: TestError::retryable("test"),
            attempts: 3,
        };
        let inner = err.into_inner();
        assert_eq!(inner.message, "test");
    }

    #[test]
    fn always_retryable_wrapper() {
        let err = AlwaysRetryable("some error");
        assert!(err.is_retryable());
    }

    #[test]
    fn never_retryable_wrapper() {
        let err = NeverRetryable("some error");
        assert!(!err.is_retryable());
    }

    #[test]
    fn retry_policy_new_clamps_jitter() {
        let policy = RetryPolicy::new(3, 100, 1000, 2.0, 1.5);
        assert!((policy.jitter_factor - 1.0).abs() < f64::EPSILON);

        let policy = RetryPolicy::new(3, 100, 1000, 2.0, -0.5);
        assert!(policy.jitter_factor.abs() < f64::EPSILON);
    }
}
