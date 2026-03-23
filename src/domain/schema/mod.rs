//! # Event Schema Management
//!
//! This module provides schema versioning and registry for domain events.
//!
//! ## Purpose
//!
//! - Track event schema versions for backward compatibility
//! - Generate JSON Schema documentation for event consumers
//! - Enable schema evolution without breaking existing consumers
//!
//! ## Design
//!
//! - **Zero runtime overhead**: Schemas are for documentation only
//! - **Compile-time safety**: Rust's type system validates events
//! - **Auto-generation**: JSON Schemas derived from Rust types via `schemars`

pub mod generator;
pub mod registry;
pub mod version;

pub use generator::generate_schema;
pub use registry::EventSchemaRegistry;
pub use version::SchemaVersion;
