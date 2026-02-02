//! # gRPC Services
//!
//! gRPC service implementations using tonic.
//!
//! # Modules
//!
//! - [`proto`]: Generated protobuf types and gRPC service definitions
//! - [`conversions`]: Conversions between domain types and protobuf messages
//! - [`service`]: gRPC service implementation

pub mod conversions;
pub mod proto;
pub mod service;

pub use proto::otc_rfq_v1;
