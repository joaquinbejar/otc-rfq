# OTC RFQ Engine

[![MIT License](https://img.shields.io/badge/license-MIT-blue)](./LICENSE)
[![Crates.io](https://img.shields.io/crates/v/otc-rfq.svg)](https://crates.io/crates/otc-rfq)
[![Downloads](https://img.shields.io/crates/d/otc-rfq.svg)](https://crates.io/crates/otc-rfq)
[![Stars](https://img.shields.io/github/stars/joaquinbejar/otc-rfq.svg)](https://github.com/joaquinbejar/otc-rfq/stargazers)
[![Issues](https://img.shields.io/github/issues/joaquinbejar/otc-rfq.svg)](https://github.com/joaquinbejar/otc-rfq/issues)
[![PRs](https://img.shields.io/github/issues-pr/joaquinbejar/otc-rfq.svg)](https://github.com/joaquinbejar/otc-rfq/pulls)

[![Build Status](https://img.shields.io/github/actions/workflow/status/joaquinbejar/otc-rfq/ci.yml?branch=main)](https://github.com/joaquinbejar/otc-rfq/actions)
[![Coverage](https://img.shields.io/codecov/c/github/joaquinbejar/otc-rfq)](https://codecov.io/gh/joaquinbejar/otc-rfq)
[![Dependencies](https://img.shields.io/librariesio/github/joaquinbejar/otc-rfq)](https://libraries.io/github/joaquinbejar/otc-rfq)
[![Documentation](https://img.shields.io/badge/docs-latest-blue.svg)](https://docs.rs/otc-rfq)

**High-performance OTC Request-for-Quote engine supporting DeFi protocols (0x, 1inch, Uniswap, Hashflow) and TradFi venues via FIX 4.4. Built in Rust with sub-millisecond latency.**

---

## Table of Contents

- [Overview](#overview)
- [Key Features](#key-features)
- [Architecture](#architecture)
- [Domain Model](#domain-model)
- [Project Structure](#project-structure)
- [Technology Stack](#technology-stack)
- [Prerequisites](#prerequisites)
- [Installation](#installation)
- [Configuration](#configuration)
- [Usage](#usage)
- [API Reference](#api-reference)
- [Testing](#testing)
- [Performance](#performance)
- [Documentation](#documentation)
- [Roadmap](#roadmap)
- [Contribution and Contact](#contribution-and-contact)

---

## Overview

OTC RFQ Engine is an enterprise-grade liquidity aggregation system that enables institutional-quality Over-The-Counter (OTC) trading across multiple venues. The system implements a **reverse auction pattern** where quote requests are broadcast to multiple liquidity providers simultaneously, and the best quotes are presented to clients for execution.

### Problem Statement

Institutional traders face several challenges when executing large OTC trades:

- **Fragmented Liquidity**: Liquidity is spread across multiple venues (banks, market makers, DEXs)
- **Manual Processes**: Traditional RFQ workflows involve phone calls and chat messages
- **Limited Transparency**: Difficult to compare prices across venues in real-time
- **Operational Risk**: Manual execution increases error rates
- **DeFi Integration Gap**: Traditional systems don't connect to on-chain liquidity

### Solution

The OTC RFQ System addresses these challenges by:

- **Aggregating liquidity** from TradFi and DeFi sources
- **Automating the RFQ workflow** from request to settlement
- **Providing real-time price discovery** across all venues
- **Ensuring compliance** with regulatory requirements
- **Supporting multiple asset classes** (crypto, stocks, forex, commodities)

---

## Key Features

### Core Capabilities

| Feature | Description |
|---------|-------------|
| **Multi-Venue Aggregation** | Simultaneously query DeFi protocols and TradFi market makers |
| **Sub-Millisecond Latency** | Optimized for high-frequency trading workflows |
| **FIX 4.4 Protocol** | Native support for traditional finance connectivity via [IronFix](https://github.com/joaquinbejar/IronFix) |
| **DeFi Integration** | 0x, 1inch, Paraswap, Hashflow, Bebop, Uniswap V3, Curve |
| **Event Sourcing** | Complete audit trail with domain event persistence |
| **Multi-Protocol API** | SBE (trading), REST (management), WebSocket (streaming) |
| **Binary Encoding** | High-performance SBE encoding via [IronSBE](https://github.com/joaquinbejar/IronSBE) |

### Supported Venue Types

| Category | Venues |
|----------|--------|
| **TradFi Market Makers** | FIX 4.4, Internal Market Makers |
| **DEX Aggregators** | 0x, 1inch, Paraswap |
| **RFQ Protocols** | Hashflow, Bebop, Airswap |
| **On-Chain Protocols** | Uniswap V3, Curve, Balancer |
| **Settlement Options** | On-chain, Off-chain, Hybrid |

### Supported Asset Classes

- **Crypto Spot**: BTC, ETH, and major tokens
- **Crypto Derivatives**: Perpetuals, options (future)
- **Forex**: Major currency pairs
- **Equities**: Stocks and ETFs
- **Commodities**: Precious metals, energy

---

## Architecture

The system follows a **layered architecture** based on Domain-Driven Design (DDD) principles with clear separation between business logic and infrastructure concerns.

### High-Level Architecture

```mermaid
flowchart TB
    subgraph CLIENTS["CLIENTS"]
        direction LR
        C1["Trading Desks"]
        C2["Algo Traders"]
        C3["DeFi Apps"]
    end

    subgraph API["API LAYER"]
        direction TB
        SBE["SBE<br/>(Trading)"]
        REST["REST<br/>(Management)"]
        WS["WebSocket<br/>(Streaming)"]
    end

    subgraph APPLICATION["APPLICATION LAYER"]
        direction TB
        RFQ_SVC["RFQ Service"]
        QUOTE_AGG["Quote Aggregation<br/>Engine"]
        EXEC_SVC["Execution Service"]
        COMPLIANCE["Compliance<br/>Service"]
    end

    subgraph DOMAIN["DOMAIN LAYER"]
        direction TB
        RFQ["RFQ Aggregate"]
        QUOTE["Quote Entity"]
        TRADE["Trade Aggregate"]
    end

    subgraph INFRASTRUCTURE["INFRASTRUCTURE LAYER"]
        direction TB
        
        subgraph VENUES["Venue Adapters"]
            FIX["FIX MM<br/>(IronFix)"]
            DEX["DEX Aggregators<br/>(0x, 1inch)"]
            RFQ_PROTO["RFQ Protocols<br/>(Hashflow)"]
        end
        
        subgraph PERSISTENCE["Persistence"]
            PG["PostgreSQL"]
            REDIS["Redis Cache"]
            KAFKA["Kafka Events"]
        end
    end

    CLIENTS --> API
    API --> APPLICATION
    APPLICATION --> DOMAIN
    APPLICATION --> INFRASTRUCTURE
    DOMAIN --> INFRASTRUCTURE
```

### Architecture Principles

| Principle | Description |
|-----------|-------------|
| **Layered Architecture** | Clear separation between domain, application, and infrastructure |
| **Domain-Driven Design** | Business logic encapsulated in domain layer |
| **Hexagonal Architecture** | Ports and adapters for external integrations |
| **Event Sourcing** | Immutable event log for audit and replay |
| **CQRS** | Separate read and write models where beneficial |
| **Microservices Ready** | Monolith that can be split into services |

### Layer Responsibilities

```mermaid
block-beta
    columns 1
    block:API["🌐 API LAYER"]
        A1["Handles external communication (SBE, REST, WebSocket)"]
        A2["Request/response serialization, authentication, rate limiting"]
    end
    block:APP["⚙️ APPLICATION LAYER"]
        B1["Use cases, orchestration, transaction management"]
        B2["No business logic, only coordination"]
    end
    block:DOM["🎯 DOMAIN LAYER"]
        C1["Core business logic, entities, value objects, domain events"]
        C2["Pure Rust, no I/O, no external dependencies"]
    end
    block:INF["🔧 INFRASTRUCTURE LAYER"]
        D1["Adapters for external systems (venues, databases, messaging)"]
        D2["Implements domain interfaces"]
    end
    
    API --> APP --> DOM --> INF
```

---

## Domain Model

### RFQ Workflow (Reverse Auction)

```mermaid
sequenceDiagram
    participant Client as 👤 Client (Taker)
    participant Engine as 🔄 RFQ Engine
    participant V1 as 🏦 Venue 1
    participant V2 as 🏦 Venue 2
    participant V3 as 🏦 Venue 3
    participant Settlement as 💰 Settlement

    Note over Client,V3: 1. CLIENT REQUESTS QUOTES
    Client->>Engine: Create RFQ
    Engine->>V1: Request Quote
    Engine->>V2: Request Quote
    Engine->>V3: Request Quote

    Note over Client,V3: 2. VENUES RESPOND WITH PRICES
    V1-->>Engine: Quote $100.05
    V2-->>Engine: Quote $100.02
    V3-->>Engine: Quote $100.08

    Note over Client,V3: 3. CLIENT SELECTS BEST QUOTE
    Engine-->>Client: Ranked Quotes
    Client->>Engine: Select Best (Venue 2)

    Note over Client,Settlement: 4. EXECUTION & 5. SETTLEMENT
    Engine->>V2: Execute Trade
    V2-->>Engine: Trade Confirmed
    Engine->>Settlement: Initiate Settlement
    Settlement-->>Client: Assets Exchanged ✅
```

### RFQ State Machine

```mermaid
stateDiagram-v2
    [*] --> Created
    
    Created --> QuoteRequesting: start_quote_collection()
    
    QuoteRequesting --> QuotesReceived: receive_quote()
    QuoteRequesting --> Expired: timeout/no quotes
    QuoteRequesting --> Cancelled: cancel()
    
    QuotesReceived --> Executing: select_quote()
    QuotesReceived --> Expired: timeout
    QuotesReceived --> Cancelled: cancel()
    
    Executing --> Executed: mark_executed()
    Executing --> Failed: mark_failed()
    Executing --> Cancelled: cancel()
    
    Expired --> [*]
    Executed --> [*]
    Failed --> [*]
    Cancelled --> [*]
```

### Core Aggregates

| Aggregate | Description |
|-----------|-------------|
| **RFQ** | Central aggregate managing the complete trading workflow |
| **Quote** | Price quote from a venue with validity period |
| **Trade** | Executed trade with settlement tracking |
| **Venue** | Liquidity source configuration and health |
| **Counterparty** | Client or market maker with compliance status |

### Domain Events

```mermaid
graph LR
    subgraph RFQ_EVENTS["📋 RFQ Lifecycle Events"]
        E1[RFQCreated]
        E2[QuoteCollectionStarted]
        E3[QuoteReceived]
        E4[QuoteSelected]
        E5[TradeExecuted]
        E6[RFQCancelled]
        E7[RFQExpired]
    end
    
    subgraph SETTLEMENT_EVENTS["💰 Settlement Events"]
        S1[SettlementInitiated]
        S2[SettlementConfirmed]
        S3[SettlementFailed]
    end
    
    subgraph COMPLIANCE_EVENTS["✅ Compliance Events"]
        C1[ComplianceCheckPassed]
        C2[ComplianceCheckFailed]
    end
```

---

## Project Structure

```
otc-rfq/
├── src/
│   ├── domain/                  # Core business logic (DDD)
│   │   ├── entities/            # Aggregates: RFQ, Quote, Trade, Venue
│   │   │   ├── rfq.rs           # RFQ aggregate with state machine
│   │   │   ├── quote.rs         # Quote entity
│   │   │   ├── trade.rs         # Trade aggregate
│   │   │   └── venue.rs         # Venue aggregate
│   │   ├── value_objects/       # Immutable value types
│   │   │   ├── ids.rs           # RFQId, QuoteId, TradeId, VenueId
│   │   │   ├── price.rs         # Price with precision
│   │   │   ├── quantity.rs      # Quantity with validation
│   │   │   ├── timestamp.rs     # Timestamp wrapper
│   │   │   └── enums.rs         # OrderSide, AssetClass, etc.
│   │   ├── events/              # Domain events for event sourcing
│   │   │   ├── rfq_events.rs    # RFQ lifecycle events
│   │   │   └── trade_events.rs  # Trade execution events
│   │   └── errors/              # Domain-specific errors
│   │       └── domain_error.rs
│   │
│   ├── application/             # Use cases and services
│   │   ├── use_cases/           # Business workflows
│   │   │   ├── create_rfq.rs    # CreateRFQUseCase
│   │   │   ├── collect_quotes.rs # CollectQuotesUseCase
│   │   │   └── execute_trade.rs # ExecuteTradeUseCase
│   │   ├── services/            # Application services
│   │   │   ├── quote_aggregation.rs  # Quote ranking and selection
│   │   │   └── circuit_breaker.rs    # Fault tolerance
│   │   ├── dto/                 # Data transfer objects
│   │   └── error.rs             # Application errors
│   │
│   ├── infrastructure/          # External adapters
│   │   ├── venues/              # Venue adapters (ports & adapters)
│   │   │   ├── traits.rs        # VenueAdapter trait
│   │   │   ├── internal_mm.rs   # Internal market maker
│   │   │   ├── fix_adapter.rs   # FIX protocol adapter
│   │   │   ├── dex_aggregator.rs # DEX aggregator adapter
│   │   │   └── registry.rs      # Venue registry
│   │   ├── persistence/         # Data persistence
│   │   │   ├── postgres/        # PostgreSQL repositories
│   │   │   ├── redis/           # Redis cache
│   │   │   └── event_store.rs   # Event sourcing store
│   │   ├── blockchain/          # Blockchain clients
│   │   │   └── ethereum.rs      # Ethereum/L2 client
│   │   └── sbe/                 # SBE binary encoding
│   │
│   ├── api/                     # External interfaces
│   │   ├── sbe/                 # SBE binary protocol (IronSBE)
│   │   │   ├── server.rs        # SBE TCP server
│   │   │   └── handlers.rs      # Request handlers
│   │   ├── rest/                # REST endpoints (axum)
│   │   │   ├── handlers.rs      # Request handlers
│   │   │   └── routes.rs        # Route configuration
│   │   ├── websocket/           # Real-time streaming
│   │   │   └── handlers.rs      # WebSocket handlers
│   │   └── middleware/          # Cross-cutting concerns
│   │       ├── auth.rs          # JWT authentication
│   │       └── rate_limit.rs    # Rate limiting
│   │
│   ├── config.rs                # Configuration management
│   ├── lib.rs                   # Library entry point
│   └── main.rs                  # Application entry point
│
├── schemas/sbe/                 # SBE message schemas
├── migrations/                  # Database migrations
├── benches/                     # Performance benchmarks
└── tests/                       # Integration tests
```

---

## Technology Stack

| Layer | Technology | Rationale |
|-------|------------|-----------|
| **Language** | Rust | Performance, safety, async |
| **Async Runtime** | Tokio | Industry standard for Rust async |
| **Web Framework** | Axum | Modern, tower-based |
| **Binary Protocol** | IronSBE | Ultra-low-latency SBE |
| **Database** | PostgreSQL | Reliability, JSON support |
| **Cache** | Redis | Speed, pub/sub |
| **Message Queue** | Kafka | Throughput, durability |
| **Blockchain** | ethers-rs | Ethereum interaction |
| **Serialization** | serde, prost | Rust standard |
| **Binary Encoding** | IronSBE | High-performance SBE |
| **FIX Protocol** | IronFix | FIX 4.4 support |
| **Observability** | tracing | Structured logging |

### Dependencies Highlights

```toml
[dependencies]
tokio = { version = "1.49", features = ["full"] }
axum = { version = "0.8", features = ["ws", "macros"] }
sqlx = { version = "0.8", features = ["postgres", "uuid", "chrono"] }
ironsbe-core = "0.2.0"
redis = { version = "1.0", features = ["tokio-comp"] }
ethers = { version = "2.0", features = ["rustls"] }
serde = { version = "1.0", features = ["derive"] }
uuid = { version = "1.20", features = ["v4", "serde"] }
rust_decimal = { version = "1.40", features = ["serde"] }
```

---

## Prerequisites

- **Rust**: 1.75+ (stable)
- **PostgreSQL**: 14+
- **Redis**: 7+
- **Kafka**: 3+ (optional, for event streaming)
- **Docker**: 20+ (optional, for containerized development)

---

## Installation

### From Source

```bash
# Clone repository
git clone https://github.com/joaquinbejar/otc-rfq.git
cd otc-rfq

# Build release
cargo build --release

# Run tests
cargo test --all-features

# Run lints
cargo clippy --all-targets --all-features -- -D warnings
```

### From Crates.io

```bash
cargo add otc-rfq
```

### Docker

```bash
# Build image
docker build -t otc-rfq:latest .

# Run container
docker run -p 50052:50052 -p 8080:8080 otc-rfq:latest
```

---

## Configuration

### Environment Variables

Create a `.env` file or set environment variables:

```bash
# =============================================================================
# Core Configuration (match apply_env_overrides in src/config.rs)
# =============================================================================
OTC_RFQ_REST_HOST=0.0.0.0
OTC_RFQ_REST_PORT=8080
OTC_RFQ_SBE_HOST=0.0.0.0
OTC_RFQ_SBE_PORT=50052
OTC_RFQ_LOG_LEVEL=info
OTC_RFQ_LOG_FORMAT=json
OTC_RFQ_DATABASE_URL=postgres://user:pass@localhost:5432/otc_rfq
OTC_RFQ_SERVICE_NAME=otc-rfq
OTC_RFQ_ENVIRONMENT=production
OTC_RFQ_CONFIG_FILE=config.toml
```

### Configuration File (TOML)

```toml
[rest]
host = "0.0.0.0"
port = 8080

[sbe]
host = "0.0.0.0"
port = 50052

[log]
level = "info"
format = "json"

[database]
url = "postgres://user:pass@localhost:5432/otc_rfq"
pool_size = 20

[venues]
quote_timeout_ms = 500
max_concurrent_requests = 100
```

---

## Usage

### Starting the Server

```bash
# Development
cargo run

# Production
cargo run --release

# With custom config
CONFIG_PATH=./config.toml cargo run --release
```

### SBE Client Example

The SBE API uses binary-encoded messages over a raw TCP connection.
See the `tests/sbe_api_integration_test.rs` file for full working examples.

```rust,ignore
use tokio::net::TcpStream;
use otc_rfq::api::sbe::codec;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect to the SBE TCP server
    let stream = TcpStream::connect("127.0.0.1:50052").await?;

    // Encode a CreateRfqRequest using SBE binary framing
    let request = codec::encode_create_rfq("DESK_001", "BTC/USD", /* ... */);
    // Send the framed message and read the response
    // (see integration tests for the full send/receive protocol)

    Ok(())
}
```

### REST API Examples

```bash
# Create RFQ
curl -X POST http://localhost:8080/api/v1/rfqs \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $TOKEN" \
  -d '{
    "client_id": "DESK_001",
    "symbol": "BTC/USD",
    "side": "BUY",
    "quantity": "10.0",
    "timeout_seconds": 30
  }'

# Get RFQ by ID
curl http://localhost:8080/api/v1/rfqs/{rfq_id} \
  -H "Authorization: Bearer $TOKEN"

# List RFQs with pagination
curl "http://localhost:8080/api/v1/rfqs?page=1&page_size=20&status=QUOTES_RECEIVED" \
  -H "Authorization: Bearer $TOKEN"

# Execute trade
curl -X POST http://localhost:8080/api/v1/rfqs/{rfq_id}/execute \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $TOKEN" \
  -d '{
    "quote_id": "quote-uuid-here"
  }'

# List venues
curl http://localhost:8080/api/v1/venues \
  -H "Authorization: Bearer $TOKEN"

# Get venue health
curl http://localhost:8080/api/v1/venues/{venue_id}/health \
  -H "Authorization: Bearer $TOKEN"
```

### WebSocket Streaming

```javascript
// Connect to WebSocket
const ws = new WebSocket('ws://localhost:8081/ws');

// Subscribe to quote updates
ws.send(JSON.stringify({
  type: 'subscribe',
  channel: 'quotes',
  rfq_id: 'rfq-uuid-here'
}));

// Handle messages
ws.onmessage = (event) => {
  const data = JSON.parse(event.data);
  console.log('Quote update:', data);
};
```

---

## API Reference

### SBE Messages

| Message | Template ID | Description |
|---------|-------------|-------------|
| `CreateRfqRequest` | 1 | Create a new RFQ |
| `GetRfqRequest` | 2 | Get RFQ by ID |
| `CancelRfqRequest` | 3 | Cancel an active RFQ |
| `ExecuteTradeRequest` | 4 | Execute trade on selected quote |
| `SubscribeQuotesRequest` | 5 | Subscribe to quote updates |
| `SubscribeRfqStatusRequest` | 6 | Subscribe to RFQ status updates |
| `UnsubscribeRequest` | 7 | Unsubscribe from updates |

### REST Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| `POST` | `/api/v1/rfqs` | Create RFQ |
| `GET` | `/api/v1/rfqs/{id}` | Get RFQ |
| `GET` | `/api/v1/rfqs` | List RFQs |
| `DELETE` | `/api/v1/rfqs/{id}` | Cancel RFQ |
| `POST` | `/api/v1/rfqs/{id}/execute` | Execute trade |
| `GET` | `/api/v1/venues` | List venues |
| `GET` | `/api/v1/venues/{id}/health` | Venue health |
| `GET` | `/api/v1/trades` | List trades |
| `GET` | `/api/v1/trades/{id}` | Get trade |

### WebSocket Channels

| Channel | Description |
|---------|-------------|
| `quotes` | Real-time quote updates |
| `rfq_status` | RFQ state changes |
| `trades` | Trade execution updates |
| `venue_health` | Venue health changes |

---

## Testing

### Running Tests

```bash
# Run all tests
cargo test --all-features

# Run specific test module
cargo test domain::entities::rfq

# Run with output
cargo test -- --nocapture

# Run integration tests (requires database)
TEST_DATABASE_URL=postgres://... cargo test --test integration

# Run benchmarks
cargo bench
```

### Test Categories

| Category | Description | Count |
|----------|-------------|-------|
| **Unit Tests** | Domain logic, value objects | 800+ |
| **Integration Tests** | Repository, API handlers | 200+ |
| **E2E Tests** | Complete RFQ workflow | 12 |
| **Property Tests** | Proptest-based invariants | 50+ |
| **Benchmarks** | Performance measurements | 10+ |

### Makefile Commands

The project includes a comprehensive Makefile with all development commands. Run `make help` for the full list.

#### 🔧 Build & Run

```bash
make build         # Compile the project
make release       # Build in release mode
make run           # Run the main binary
make run-release   # Run in release mode
make clean         # Clean build artifacts
```

#### 🧪 Test & Quality

```bash
make test          # Run all tests
make test-lib      # Run library tests only
make test-integration # Run integration tests
make fmt           # Format code
make fmt-check     # Check formatting without applying
make lint          # Run clippy with warnings as errors
make lint-fix      # Auto-fix lint issues
make fix           # Auto-fix Rust compiler suggestions
make check         # Run fmt-check + lint + test
make pre-push      # Run all pre-push checks
```

#### 📦 Packaging & Docs

```bash
make doc           # Generate documentation
make doc-open      # Build and open Rust documentation
make doc-check     # Check for missing docs via clippy
make create-doc    # Generate internal docs
make readme        # Regenerate README using cargo-readme
make publish       # Prepare and publish crate to crates.io
```

#### 📈 Coverage & Benchmarks

```bash
make coverage            # Generate code coverage report (XML)
make coverage-html       # Generate HTML coverage report
make open-coverage       # Open HTML report
make bench               # Run benchmarks using Criterion
make bench-show          # Open benchmark report
make bench-save          # Save benchmark history snapshot
make bench-compare       # Compare benchmark runs
make bench-json          # Output benchmarks in JSON
make bench-clean         # Remove benchmark data
```

#### 🗄️ Database

```bash
make migrate         # Run database migrations
make migrate-new     # Create a new migration
make migrate-revert  # Revert last migration
make db-reset        # Reset database (drop, create, migrate)
```

#### 🐳 Docker

```bash
make docker-up       # Start Docker services
make docker-down     # Stop Docker services
make docker-logs     # Show Docker logs
make docker-build    # Build Docker image
make docker-run      # Run Docker container
```

#### 🧹 Git & Workflow Helpers

```bash
make git-log             # Show commits on current branch vs main
make check-spanish       # Check for Spanish words in code
make zip                 # Create zip without target/ and temp files
make tree                # Visualize project tree
make loc                 # Count lines of code
make deps                # Show dependency tree
make outdated            # Check for outdated dependencies
make audit               # Run security audit
```

#### 🤖 GitHub Actions (via act)

```bash
make workflow-build      # Simulate build workflow
make workflow-lint       # Simulate lint workflow
make workflow-test       # Simulate test workflow
make workflow-coverage   # Simulate coverage workflow
make workflow            # Run all workflows
```

#### 🚀 Release

```bash
make version         # Show current version
make tag             # Create git tag from Cargo.toml version
```

---

## Performance

### Targets

| Metric | Target | Achieved |
|--------|--------|----------|
| Quote collection latency | < 100ms (p99) | ✅ |
| Trade execution latency | < 50ms (p99) | ✅ |
| Concurrent RFQs | 10,000+ | ✅ |
| Quotes per second | 100,000+ | ✅ |
| API response time | < 10ms (p99) | ✅ |

### Benchmarks

```bash
# Run benchmarks
make bench

# View benchmark report
make bench-show

# Save baseline and compare
make bench-save
make bench-compare
```

### Optimization Features

- **Zero-copy deserialization** with SBE encoding
- **Connection pooling** for database and Redis
- **Concurrent venue requests** with tokio
- **Circuit breakers** for fault tolerance
- **Response caching** with Redis

---

## Documentation

### Internal Documentation

| Document | Description |
|----------|-------------|
| [System Overview](.internalDoc/architecture/01-SYSTEM-OVERVIEW.md) | Business context and goals |
| [Architecture](.internalDoc/architecture/02-ARCHITECTURE.md) | Layered architecture design |
| [Domain Model](.internalDoc/architecture/03-DOMAIN-MODEL.md) | Entities, aggregates, events |
| [Data Model](.internalDoc/architecture/04-DATA-MODEL.md) | Database schema and caching |
| [API Overview](.internalDoc/api/01-API-OVERVIEW.md) | Multi-protocol API design |
| [FIX Protocol](.internalDoc/protocols/02-FIX-PROTOCOL.md) | FIX 4.4 integration |
| [DeFi Protocols](.internalDoc/protocols/03-DEFI-PROTOCOLS.md) | DEX and RFQ protocol adapters |
| [SBE Encoding](.internalDoc/protocols/04-SBE-ENCODING.md) | Binary message encoding |
| [Rust Guidelines](.internalDoc/09-rust-guidelines.md) | Coding standards |

### API Documentation

```bash
# Generate and open docs
cargo doc --open --no-deps
```

---

## Roadmap

### Completed Milestones

- [x] **M0**: Project Setup - Repository, CI/CD, documentation structure
- [x] **M1**: Domain Layer - Entities, value objects, domain events
- [x] **M2**: Infrastructure Layer - Venues, persistence, blockchain
- [x] **M3**: Application Layer - Use cases, services, DTOs
- [x] **M4**: API Layer - SBE, REST, WebSocket
- [x] **M5**: Testing & Integration - Unit, integration, E2E tests

### Future Milestones

- [ ] **M6**: Production Hardening - Monitoring, alerting, deployment
- [ ] **M7**: Advanced Features - Algorithmic execution, portfolio management
- [ ] **M8**: Multi-Region - Geographic distribution, failover

See [GitHub Issues](https://github.com/joaquinbejar/otc-rfq/issues) for detailed progress.

---

## Contribution and Contact

We welcome contributions to this project! If you would like to contribute, please follow these steps:

1. Fork the repository.
2. Create a new branch for your feature or bug fix.
3. Make your changes and ensure that the project still builds and all tests pass.
4. Commit your changes and push your branch to your forked repository.
5. Submit a pull request to the main repository.

### Development Workflow

```bash
# Create feature branch
git checkout -b M[N]/issue-[NUM]-description

# Make changes and test
cargo test --all-features
make pre-push

# Commit and push
git commit -m "feat: description"
git push origin your-branch

# Create PR
gh pr create
```

If you have any questions, issues, or would like to provide feedback, please feel free to contact the project maintainer:

### **Contact Information**

- **Author**: Joaquín Béjar García
- **Email**: jb@taunais.com
- **Telegram**: [@joaquin_bejar](https://t.me/joaquin_bejar)
- **Repository**: <https://github.com/joaquinbejar/otc-rfq>
- **Documentation**: <https://docs.rs/otc-rfq>

We appreciate your interest and look forward to your contributions!

---

**License**: MIT
