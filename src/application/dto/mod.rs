//! # Data Transfer Objects
//!
//! DTOs for use case input/output, decoupling API from domain.
//!
//! These objects provide a clean interface between the API layer and
//! the application layer, handling validation and serialization.

pub mod rfq_dto;

pub use rfq_dto::{CreateRfqRequest, CreateRfqResponse};
