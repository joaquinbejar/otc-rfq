//! # Market Calendar
//!
//! Utilities for calculating market times, including end-of-day (EOD)
//! for delayed trade reporting.

use crate::domain::value_objects::Timestamp;
use chrono::{Datelike, NaiveTime, TimeZone, Utc, Weekday};

/// Default market close time (UTC).
/// Assumes 21:00 UTC which corresponds to 4:00 PM EST / 5:00 PM EDT.
pub const DEFAULT_MARKET_CLOSE_HOUR: u32 = 21;
/// Default market close minute (0 = on the hour).
pub const DEFAULT_MARKET_CLOSE_MINUTE: u32 = 0;

/// Configuration for market calendar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketCalendarConfig {
    /// Market close hour (0-23, UTC).
    pub close_hour: u32,
    /// Market close minute (0-59).
    pub close_minute: u32,
}

impl Default for MarketCalendarConfig {
    fn default() -> Self {
        Self {
            close_hour: DEFAULT_MARKET_CLOSE_HOUR,
            close_minute: DEFAULT_MARKET_CLOSE_MINUTE,
        }
    }
}

impl MarketCalendarConfig {
    /// Creates a new market calendar configuration.
    #[must_use]
    pub fn new(close_hour: u32, close_minute: u32) -> Self {
        Self {
            close_hour: close_hour.min(23),
            close_minute: close_minute.min(59),
        }
    }
}

/// Calculates the next market close time from a given timestamp.
///
/// If the current time is before today's market close, returns today's close.
/// If the current time is after today's market close, returns the next
/// business day's close (skipping weekends).
///
/// # Arguments
///
/// * `from` - The timestamp to calculate from
/// * `config` - Market calendar configuration
///
/// # Returns
///
/// The next market close timestamp.
///
/// # Panics
///
/// This function will not panic under normal circumstances. Invalid timestamps
/// will fall back to the input timestamp.
#[must_use]
#[allow(clippy::expect_used)]
pub fn next_market_close(from: Timestamp, config: &MarketCalendarConfig) -> Timestamp {
    let dt = Utc.timestamp_millis_opt(from.timestamp_millis()).single();

    let datetime = match dt {
        Some(d) => d,
        None => return from, // Fallback if timestamp is invalid
    };

    let close_time = NaiveTime::from_hms_opt(config.close_hour, config.close_minute, 0)
        .unwrap_or_else(|| NaiveTime::from_hms_opt(21, 0, 0).expect("valid time"));

    // Build today's close datetime
    let today_close = datetime.date_naive().and_time(close_time).and_utc();

    // If before today's close and today is a business day, use today
    if datetime < today_close && is_business_day(datetime.weekday()) {
        if let Some(ts) = Timestamp::from_millis(today_close.timestamp_millis()) {
            return ts;
        }
        return from;
    }

    // Otherwise, find the next business day's close
    let mut next_day = match datetime.date_naive().succ_opt() {
        Some(d) => d,
        None => return from,
    };
    while !is_business_day(next_day.weekday()) {
        next_day = match next_day.succ_opt() {
            Some(d) => d,
            None => return from,
        };
    }

    let next_close = next_day.and_time(close_time).and_utc();
    Timestamp::from_millis(next_close.timestamp_millis()).unwrap_or(from)
}

/// Calculates the next market close using default configuration.
#[must_use]
pub fn next_market_close_default(from: Timestamp) -> Timestamp {
    next_market_close(from, &MarketCalendarConfig::default())
}

/// Checks if a weekday is a business day (Monday-Friday).
#[must_use]
fn is_business_day(weekday: Weekday) -> bool {
    !matches!(weekday, Weekday::Sat | Weekday::Sun)
}

/// Calculates the delay until the next market close.
///
/// # Returns
///
/// [`std::time::Duration`] until the next market close. Returns
/// [`std::time::Duration::ZERO`] if the market close is not in the future
/// (i.e., already past or exactly at `from`).
#[must_use]
pub fn delay_until_market_close(
    from: Timestamp,
    config: &MarketCalendarConfig,
) -> std::time::Duration {
    let close = next_market_close(from, config);
    let close_ms = close.timestamp_millis();
    let from_ms = from.timestamp_millis();

    if close_ms <= from_ms {
        std::time::Duration::ZERO
    } else {
        std::time::Duration::from_millis((close_ms - from_ms) as u64)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use chrono::Timelike;

    fn make_timestamp(year: i32, month: u32, day: u32, hour: u32, min: u32) -> Timestamp {
        Timestamp::from_millis(
            chrono::Utc
                .with_ymd_and_hms(year, month, day, hour, min, 0)
                .unwrap()
                .timestamp_millis(),
        )
        .unwrap()
    }

    #[test]
    fn default_config() {
        let config = MarketCalendarConfig::default();
        assert_eq!(config.close_hour, 21);
        assert_eq!(config.close_minute, 0);
    }

    #[test]
    fn next_market_close_before_close_today() {
        // Monday 10:00 UTC -> should return Monday 21:00 UTC
        let monday_morning = make_timestamp(2026, 3, 9, 10, 0);
        let config = MarketCalendarConfig::default();
        let close = next_market_close(monday_morning, &config);

        let close_dt = Utc.timestamp_millis_opt(close.timestamp_millis()).unwrap();
        assert_eq!(close_dt.hour(), 21);
        assert_eq!(close_dt.day(), 9);
    }

    #[test]
    fn next_market_close_after_close_today() {
        // Monday 22:00 UTC -> should return Tuesday 21:00 UTC
        let monday_evening = make_timestamp(2026, 3, 9, 22, 0);
        let config = MarketCalendarConfig::default();
        let close = next_market_close(monday_evening, &config);

        let close_dt = Utc.timestamp_millis_opt(close.timestamp_millis()).unwrap();
        assert_eq!(close_dt.hour(), 21);
        assert_eq!(close_dt.day(), 10);
    }

    #[test]
    fn next_market_close_friday_evening_skips_weekend() {
        // Friday 22:00 UTC -> should return Monday 21:00 UTC
        let friday_evening = make_timestamp(2026, 3, 13, 22, 0);
        let config = MarketCalendarConfig::default();
        let close = next_market_close(friday_evening, &config);

        let close_dt = Utc.timestamp_millis_opt(close.timestamp_millis()).unwrap();
        assert_eq!(close_dt.hour(), 21);
        assert_eq!(close_dt.day(), 16); // Monday
        assert_eq!(close_dt.weekday(), Weekday::Mon);
    }

    #[test]
    fn next_market_close_saturday_skips_to_monday() {
        // Saturday 10:00 UTC -> should return Monday 21:00 UTC
        let saturday = make_timestamp(2026, 3, 14, 10, 0);
        let config = MarketCalendarConfig::default();
        let close = next_market_close(saturday, &config);

        let close_dt = Utc.timestamp_millis_opt(close.timestamp_millis()).unwrap();
        assert_eq!(close_dt.weekday(), Weekday::Mon);
    }

    #[test]
    fn delay_until_market_close_calculation() {
        // Monday 20:00 UTC -> 1 hour until 21:00 close
        let monday_20 = make_timestamp(2026, 3, 9, 20, 0);
        let config = MarketCalendarConfig::default();
        let delay = delay_until_market_close(monday_20, &config);

        // Should be approximately 1 hour
        assert!(delay.as_secs() >= 3599 && delay.as_secs() <= 3601);
    }

    #[test]
    fn is_business_day_check() {
        assert!(is_business_day(Weekday::Mon));
        assert!(is_business_day(Weekday::Tue));
        assert!(is_business_day(Weekday::Wed));
        assert!(is_business_day(Weekday::Thu));
        assert!(is_business_day(Weekday::Fri));
        assert!(!is_business_day(Weekday::Sat));
        assert!(!is_business_day(Weekday::Sun));
    }
}
