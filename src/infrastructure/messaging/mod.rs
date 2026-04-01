//! # Messaging Infrastructure
//!
//! Adapters for event publishing and message brokering.

#[cfg(feature = "nats")]
pub mod dispatcher;

#[cfg(feature = "nats")]
pub mod nats_worker;
