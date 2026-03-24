//! # Composite Streaming Quote Service
//!
//! Core implementation that aggregates streaming quotes from all MMs.

use crate::domain::entities::streaming_quote::{
    BestQuote, PriceLevel, StreamingQuote, StreamingQuoteConfig, StreamingQuoteStats,
};
use crate::domain::services::streaming_quote::{
    StreamingQuoteRejectReason, StreamingQuoteResult, StreamingQuoteService,
};
use crate::domain::value_objects::{CounterpartyId, Instrument};
use async_trait::async_trait;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::RwLock;

/// Channel type for streaming quote communication.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StreamingChannel {
    /// WebSocket channel.
    WebSocket,
    /// gRPC channel.
    Grpc,
    /// FIX protocol channel.
    Fix,
}

impl fmt::Display for StreamingChannel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WebSocket => write!(f, "websocket"),
            Self::Grpc => write!(f, "grpc"),
            Self::Fix => write!(f, "fix"),
        }
    }
}

/// Per-venue streaming configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VenueStreamingConfig {
    /// The channel to use for this venue.
    pub channel: StreamingChannel,
    /// Whether streaming is enabled for this venue.
    pub enabled: bool,
}

impl VenueStreamingConfig {
    /// Creates a new venue streaming config.
    #[must_use]
    pub fn new(channel: StreamingChannel, enabled: bool) -> Self {
        Self { channel, enabled }
    }
}

/// Rate limiting state for a market maker.
#[derive(Debug)]
struct RateLimitState {
    count: AtomicU64,
    window_start_ms: AtomicU64,
}

impl RateLimitState {
    fn new() -> Self {
        Self {
            count: AtomicU64::new(0),
            window_start_ms: AtomicU64::new(0),
        }
    }

    fn check_and_increment(&self, max_per_second: u32) -> (bool, u32) {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        let window_start = self.window_start_ms.load(Ordering::Relaxed);

        if now_ms.saturating_sub(window_start) >= 1000 {
            self.window_start_ms.store(now_ms, Ordering::Relaxed);
            self.count.store(1, Ordering::Relaxed);
            return (false, 1);
        }

        let count = self.count.fetch_add(1, Ordering::Relaxed) + 1;
        let is_exceeded = count > max_per_second as u64;
        (is_exceeded, count as u32)
    }
}

/// Quote book for a single instrument.
#[derive(Debug)]
struct InstrumentBook {
    quotes: HashMap<CounterpartyId, StreamingQuote>,
    best_bid: Option<(CounterpartyId, PriceLevel)>,
    best_ask: Option<(CounterpartyId, PriceLevel)>,
}

impl InstrumentBook {
    fn new() -> Self {
        Self {
            quotes: HashMap::new(),
            best_bid: None,
            best_ask: None,
        }
    }

    fn update_quote(&mut self, quote: StreamingQuote) -> bool {
        let mm_id = quote.mm_id().clone();
        self.quotes.insert(mm_id.clone(), quote);
        self.recalculate_best();

        let is_best_bid = self
            .best_bid
            .as_ref()
            .map(|(id, _)| id == &mm_id)
            .unwrap_or(false);
        let is_best_ask = self
            .best_ask
            .as_ref()
            .map(|(id, _)| id == &mm_id)
            .unwrap_or(false);
        is_best_bid || is_best_ask
    }

    fn remove_stale(
        &mut self,
        registered_mms: &std::collections::HashSet<CounterpartyId>,
    ) -> usize {
        let stale_mms: Vec<_> = self
            .quotes
            .iter()
            .filter(|(id, q)| q.is_stale() || !registered_mms.contains(id))
            .map(|(id, _)| id.clone())
            .collect();

        let count = stale_mms.len();
        for mm_id in stale_mms {
            self.quotes.remove(&mm_id);
        }

        if count > 0 {
            self.recalculate_best();
        }
        count
    }

    fn recalculate_best(&mut self) {
        self.best_bid = None;
        self.best_ask = None;

        for (mm_id, quote) in &self.quotes {
            if quote.is_stale() {
                continue;
            }

            match &self.best_bid {
                None => self.best_bid = Some((mm_id.clone(), *quote.bid())),
                Some((_, current)) => {
                    if quote.bid().price().get() > current.price().get() {
                        self.best_bid = Some((mm_id.clone(), *quote.bid()));
                    }
                }
            }

            match &self.best_ask {
                None => self.best_ask = Some((mm_id.clone(), *quote.ask())),
                Some((_, current)) => {
                    if quote.ask().price().get() < current.price().get() {
                        self.best_ask = Some((mm_id.clone(), *quote.ask()));
                    }
                }
            }
        }
    }

    fn best_quote(&self) -> Option<BestQuote> {
        match (&self.best_bid, &self.best_ask) {
            (Some((bid_mm, bid)), Some((ask_mm, ask))) => {
                Some(BestQuote::new(*bid, bid_mm.clone(), *ask, ask_mm.clone()))
            }
            _ => None,
        }
    }

}

/// Composite streaming quote service that aggregates quotes from all MMs.
pub struct CompositeStreamingQuoteService {
    books: DashMap<Instrument, InstrumentBook>,
    rate_limits: DashMap<CounterpartyId, RateLimitState>,
    registered_mms: DashMap<CounterpartyId, bool>,
    config: StreamingQuoteConfig,
    stats: RwLock<StreamingQuoteStats>,
    mm_stats: DashMap<CounterpartyId, StreamingQuoteStats>,
}

impl fmt::Debug for CompositeStreamingQuoteService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CompositeStreamingQuoteService")
            .field("instruments", &self.books.len())
            .field("registered_mms", &self.registered_mms.len())
            .finish()
    }
}

impl CompositeStreamingQuoteService {
    /// Creates a new composite streaming quote service.
    #[must_use]
    pub fn new(config: StreamingQuoteConfig) -> Self {
        Self {
            books: DashMap::new(),
            rate_limits: DashMap::new(),
            registered_mms: DashMap::new(),
            config,
            stats: RwLock::new(StreamingQuoteStats::new()),
            mm_stats: DashMap::new(),
        }
    }

    /// Creates with default configuration.
    #[must_use]
    pub fn with_defaults() -> Self {
        Self::new(StreamingQuoteConfig::default())
    }

    fn validate_quote(&self, quote: &StreamingQuote) -> Result<(), StreamingQuoteRejectReason> {
        if !self.registered_mms.contains_key(quote.mm_id()) {
            return Err(StreamingQuoteRejectReason::unknown_mm());
        }

        if quote.is_stale() {
            return Err(StreamingQuoteRejectReason::stale());
        }

        match quote.spread_bps() {
            Some(spread_bps) if spread_bps > self.config.max_spread_bps() => {
                return Err(StreamingQuoteRejectReason::invalid_spread(
                    spread_bps,
                    self.config.max_spread_bps(),
                ));
            }
            None => {
                // Treat uncomputable spread (overflow) as invalid
                return Err(StreamingQuoteRejectReason::invalid_spread(
                    self.config.max_spread_bps(),
                    self.config.max_spread_bps(),
                ));
            }
            _ => {}
        }

        let min_size = self.config.min_size();
        if !min_size.is_zero() {
            if quote.bid().size().get() < min_size {
                return Err(StreamingQuoteRejectReason::invalid_size(
                    quote.bid().size().get(),
                    min_size,
                ));
            }
            if quote.ask().size().get() < min_size {
                return Err(StreamingQuoteRejectReason::invalid_size(
                    quote.ask().size().get(),
                    min_size,
                ));
            }
        }

        Ok(())
    }

    fn check_rate_limit(&self, mm_id: &CounterpartyId) -> Result<(), StreamingQuoteRejectReason> {
        let state = self
            .rate_limits
            .entry(mm_id.clone())
            .or_insert_with(RateLimitState::new);
        let max_rate = self.config.max_quotes_per_second();
        let (exceeded, current) = state.check_and_increment(max_rate);

        if exceeded {
            Err(StreamingQuoteRejectReason::rate_limited(current, max_rate))
        } else {
            Ok(())
        }
    }

    async fn record_stats(&self, mm_id: &CounterpartyId, result: &StreamingQuoteResult) {
        {
            let mut stats = self.stats.write().await;
            stats.record_received();
            match result {
                StreamingQuoteResult::Accepted { .. } => stats.record_accepted(),
                StreamingQuoteResult::Rejected { reason } => match reason {
                    StreamingQuoteRejectReason::RateLimitExceeded { .. } => {
                        stats.record_rate_limited()
                    }
                    _ => stats.record_rejected(),
                },
            }
        }

        let mut mm_stats = self.mm_stats.entry(mm_id.clone()).or_default();
        mm_stats.record_received();
        match result {
            StreamingQuoteResult::Accepted { .. } => mm_stats.record_accepted(),
            StreamingQuoteResult::Rejected { reason } => match reason {
                StreamingQuoteRejectReason::RateLimitExceeded { .. } => {
                    mm_stats.record_rate_limited()
                }
                _ => mm_stats.record_rejected(),
            },
        }
    }
}

#[async_trait]
impl StreamingQuoteService for CompositeStreamingQuoteService {
    async fn receive_quote(&self, quote: StreamingQuote) -> StreamingQuoteResult {
        let mm_id = quote.mm_id().clone();
        let quote_id = quote.id();

        if let Err(reason) = self.check_rate_limit(&mm_id) {
            let result = StreamingQuoteResult::rejected(reason);
            self.record_stats(&mm_id, &result).await;
            return result;
        }

        if let Err(reason) = self.validate_quote(&quote) {
            let result = StreamingQuoteResult::rejected(reason);
            self.record_stats(&mm_id, &result).await;
            return result;
        }

        let instrument = quote.instrument().clone();
        let is_best = {
            let mut book = self
                .books
                .entry(instrument)
                .or_insert_with(InstrumentBook::new);
            book.update_quote(quote)
        };

        let result = StreamingQuoteResult::accepted(quote_id, is_best);
        self.record_stats(&mm_id, &result).await;
        result
    }

    fn best_quote(&self, instrument: &Instrument) -> Option<BestQuote> {
        self.books.get(instrument).and_then(|book| {
            let best = book.best_quote()?;
            // Ensure both MMs are still registered to satisfy the immediate removal contract.
            if self.is_mm_registered(best.bid_mm()) && self.is_mm_registered(best.ask_mm()) {
                Some(best)
            } else {
                None
            }
        })
    }

    fn active_quotes(&self, instrument: &Instrument) -> Vec<StreamingQuote> {
        self.books
            .get(instrument)
            .map(|book| {
                book.quotes
                    .values()
                    .filter(|q| !q.is_stale() && self.is_mm_registered(q.mm_id()))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    fn mm_quote(&self, instrument: &Instrument, mm_id: &CounterpartyId) -> Option<StreamingQuote> {
        if !self.is_mm_registered(mm_id) {
            return None;
        }

        self.books.get(instrument).and_then(|book| {
            let quote = book.quotes.get(mm_id)?;
            if quote.is_stale() {
                None
            } else {
                Some(quote.clone())
            }
        })
    }

    fn is_mm_registered(&self, mm_id: &CounterpartyId) -> bool {
        self.registered_mms.contains_key(mm_id)
    }

    fn register_mm(&self, mm_id: CounterpartyId) {
        self.registered_mms.insert(mm_id.clone(), true);
        self.mm_stats.entry(mm_id).or_default();
    }

    fn unregister_mm(&self, mm_id: &CounterpartyId) {
        self.registered_mms.remove(mm_id);
        // Note: Surgical cleanup of the order books is deferred to the background
        // staleness checker to avoid O(N) iteration and global lock contention.
    }

    fn config(&self) -> &StreamingQuoteConfig {
        &self.config
    }

    async fn get_stats(&self) -> StreamingQuoteStats {
        self.stats.read().await.clone()
    }

    async fn get_mm_stats(&self, mm_id: &CounterpartyId) -> Option<StreamingQuoteStats> {
        self.mm_stats.get(mm_id).map(|s| s.clone())
    }

    async fn remove_stale_quotes(&self) -> usize {
        let mut total = 0;
        let registered: std::collections::HashSet<_> = {
            let mut set = std::collections::HashSet::with_capacity(self.registered_mms.len());
            for kv in self.registered_mms.iter() {
                set.insert(kv.key().clone());
            }
            set
        };

        for mut book in self.books.iter_mut() {
            total += book.remove_stale(&registered);
        }

        let mut stats = self.stats.write().await;
        for _ in 0..total {
            stats.record_expired();
        }

        total
    }

    fn active_instruments(&self) -> Vec<Instrument> {
        self.books
            .iter()
            .filter(|book| {
                book.quotes
                    .iter()
                    .any(|(id, q)| !q.is_stale() && self.is_mm_registered(id))
            })
            .map(|book| book.key().clone())
            .collect()
    }

    fn total_active_quotes(&self) -> usize {
        self.books
            .iter()
            .map(|book| {
                book.quotes
                    .iter()
                    .filter(|(id, q)| !q.is_stale() && self.is_mm_registered(id))
                    .count()
            })
            .sum()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::domain::value_objects::enums::{AssetClass, SettlementMethod};
    use crate::domain::value_objects::symbol::Symbol;
    use crate::domain::value_objects::{Price, Quantity};

    fn create_test_instrument() -> Instrument {
        let symbol = Symbol::new("BTC/USD").unwrap();
        Instrument::new(symbol, AssetClass::CryptoSpot, SettlementMethod::default())
    }

    fn create_test_quote(mm_id: &str, bid_price: f64, ask_price: f64) -> StreamingQuote {
        let instrument = create_test_instrument();
        let bid =
            PriceLevel::new(Price::new(bid_price).unwrap(), Quantity::new(1.0).unwrap()).unwrap();
        let ask =
            PriceLevel::new(Price::new(ask_price).unwrap(), Quantity::new(1.0).unwrap()).unwrap();

        StreamingQuote::new(CounterpartyId::new(mm_id), instrument, bid, ask, 5000).unwrap()
    }

    #[tokio::test]
    async fn composite_service_creation() {
        let service = CompositeStreamingQuoteService::with_defaults();
        assert_eq!(service.total_active_quotes(), 0);
    }

    #[tokio::test]
    async fn register_and_receive_quote() {
        let service = CompositeStreamingQuoteService::with_defaults();
        let mm_id = CounterpartyId::new("mm-1");

        service.register_mm(mm_id.clone());
        assert!(service.is_mm_registered(&mm_id));

        let quote = create_test_quote("mm-1", 50000.0, 50010.0);
        let result = service.receive_quote(quote).await;

        assert!(result.is_accepted());
        assert_eq!(service.total_active_quotes(), 1);
    }

    #[tokio::test]
    async fn reject_unregistered_mm() {
        let service = CompositeStreamingQuoteService::with_defaults();
        let quote = create_test_quote("unknown-mm", 50000.0, 50010.0);
        let result = service.receive_quote(quote).await;

        assert!(result.is_rejected());
        if let StreamingQuoteResult::Rejected { reason } = result {
            assert!(matches!(reason, StreamingQuoteRejectReason::UnknownMM));
        }
    }

    #[tokio::test]
    async fn best_quote_aggregation() {
        let service = CompositeStreamingQuoteService::with_defaults();

        service.register_mm(CounterpartyId::new("mm-1"));
        service.register_mm(CounterpartyId::new("mm-2"));

        let quote1 = create_test_quote("mm-1", 50000.0, 50020.0);
        let quote2 = create_test_quote("mm-2", 50005.0, 50015.0);

        service.receive_quote(quote1).await;
        service.receive_quote(quote2).await;

        let instrument = create_test_instrument();
        let best = service.best_quote(&instrument).unwrap();

        // Best bid = highest = mm-2 at 50005
        assert_eq!(best.bid_mm().to_string(), "mm-2");
        // Best ask = lowest = mm-2 at 50015
        assert_eq!(best.ask_mm().to_string(), "mm-2");
    }

    #[tokio::test]
    async fn unregister_removes_quotes() {
        let service = CompositeStreamingQuoteService::with_defaults();
        let mm_id = CounterpartyId::new("mm-1");

        service.register_mm(mm_id.clone());
        let quote = create_test_quote("mm-1", 50000.0, 50010.0);
        service.receive_quote(quote).await;

        assert_eq!(service.total_active_quotes(), 1);
        service.unregister_mm(&mm_id);
        assert!(!service.is_mm_registered(&mm_id));

        // Contract: Quotes must be removed immediately from public API
        assert_eq!(service.total_active_quotes(), 0);
        let instrument = create_test_instrument();
        assert!(service.best_quote(&instrument).is_none());

        // Background cleanup will eventually purge the internal book (implementation detail)
        service.remove_stale_quotes().await;
        assert_eq!(service.total_active_quotes(), 0);
    }

    #[tokio::test]
    async fn unregister_removes_multiple_instruments() {
        let service = CompositeStreamingQuoteService::with_defaults();
        let mm_id = CounterpartyId::new("mm-1");
        service.register_mm(mm_id.clone());

        let symbol1 = Symbol::new("BTC/USD").unwrap();
        let instr1 = Instrument::new(symbol1, AssetClass::CryptoSpot, SettlementMethod::default());

        let symbol2 = Symbol::new("ETH/USD").unwrap();
        let instr2 = Instrument::new(symbol2, AssetClass::CryptoSpot, SettlementMethod::default());

        let bid =
            PriceLevel::new(Price::new(50000.0).unwrap(), Quantity::new(1.0).unwrap()).unwrap();
        let ask =
            PriceLevel::new(Price::new(50010.0).unwrap(), Quantity::new(1.0).unwrap()).unwrap();

        let quote1 = StreamingQuote::new(mm_id.clone(), instr1, bid, ask, 5000).unwrap();
        let quote2 = StreamingQuote::new(mm_id.clone(), instr2, bid, ask, 5000).unwrap();

        service.receive_quote(quote1).await;
        service.receive_quote(quote2).await;

        assert_eq!(service.total_active_quotes(), 2);

        service.unregister_mm(&mm_id);
        assert_eq!(service.total_active_quotes(), 0);

        service.remove_stale_quotes().await;
        assert_eq!(service.total_active_quotes(), 0);
    }

    #[tokio::test]
    async fn stats_tracking() {
        let service = CompositeStreamingQuoteService::with_defaults();
        service.register_mm(CounterpartyId::new("mm-1"));

        let quote = create_test_quote("mm-1", 50000.0, 50010.0);
        service.receive_quote(quote).await;

        let stats = service.get_stats().await;
        assert_eq!(stats.quotes_received, 1);
        assert_eq!(stats.quotes_accepted, 1);
    }
}
