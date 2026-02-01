//! # OTC RFQ Engine
//!
//! High-performance OTC Request-for-Quote engine supporting DeFi protocols
//! (0x, 1inch, Uniswap, Hashflow) and TradFi venues via FIX 4.4.
//!
//! ## Architecture
//!
//! This crate follows Domain-Driven Design with a layered architecture:
//!
//! - **Domain Layer** (`domain`): Core business logic, entities, value objects, and domain events
//! - **Application Layer** (`application`): Use cases, services, and orchestration
//! - **Infrastructure Layer** (`infrastructure`): External adapters, repositories, and integrations
//! - **API Layer** (`api`): gRPC, REST, and WebSocket interfaces
//!
//! ## Example
//!
//! ```rust,ignore
//! use otc_rfq::application::use_cases::CreateRFQUseCase;
//! use otc_rfq::domain::value_objects::{Quantity, Symbol};
//!
//! // Create an RFQ request
//! let rfq = CreateRFQUseCase::new(/* dependencies */)
//!     .execute(request)
//!     .await?;
//! ```

#![warn(missing_docs)]
#![deny(unsafe_code)]

pub mod api;
pub mod application;
pub mod domain;
pub mod infrastructure;
