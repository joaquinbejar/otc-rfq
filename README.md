# OTC RFQ Engine

[![License](https://img.shields.io/badge/license-MIT-blue)](./LICENSE)
[![Crates.io](https://img.shields.io/crates/v/otc-rfq.svg)](https://crates.io/crates/otc-rfq)
[![Build Status](https://img.shields.io/github/actions/workflow/status/joaquinbejar/otc-rfq/ci.yml?branch=main)](https://github.com/joaquinbejar/otc-rfq/actions)
[![Coverage](https://img.shields.io/codecov/c/github/joaquinbejar/otc-rfq)](https://codecov.io/gh/joaquinbejar/otc-rfq)
[![Documentation](https://img.shields.io/badge/docs-latest-blue.svg)](https://docs.rs/otc-rfq)

**High-performance OTC Request-for-Quote engine supporting DeFi protocols (0x, 1inch, Uniswap, Hashflow) and TradFi venues via FIX 4.4. Built in Rust with sub-millisecond latency.**

---

## Overview

OTC RFQ Engine is an enterprise-grade liquidity aggregation system that enables institutional-quality OTC trading across multiple venues. The system implements a reverse auction pattern where quote requests are broadcast to multiple liquidity providers, and the best quotes are presented to clients for execution.

### Key Capabilities

- **Multi-Venue Aggregation**: Simultaneously query DeFi protocols and TradFi market makers
- **Sub-Millisecond Latency**: Optimized for high-frequency trading workflows
- **FIX 4.4 Protocol**: Native support for traditional finance connectivity via [IronFix](https://github.com/joaquinbejar/IronFix)
- **DeFi Integration**: 0x, 1inch, Paraswap, Hashflow, Bebop, Uniswap V3, Curve
- **Event Sourcing**: Complete audit trail with domain event persistence
- **Multi-Protocol API**: gRPC (trading), REST (management), WebSocket (streaming)

## Architecture

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
        GRPC["gRPC<br/>(Trading)"]
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

## Project Structure

```
otc-rfq/
├── src/
│   ├── domain/              # Core business logic (DDD)
│   │   ├── entities/        # Aggregates: RFQ, Quote, Trade, Venue
│   │   ├── value_objects/   # Price, Quantity, identifiers
│   │   ├── events/          # Domain events for event sourcing
│   │   └── errors/          # Domain-specific errors
│   ├── application/         # Use cases and services
│   │   ├── use_cases/       # CreateRFQ, CollectQuotes, ExecuteTrade
│   │   ├── services/        # QuoteAggregation, Compliance
│   │   └── dto/             # Data transfer objects
│   ├── infrastructure/      # External adapters
│   │   ├── venues/          # FIX, DEX, RFQ protocol adapters
│   │   ├── persistence/     # PostgreSQL, Redis, Event Store
│   │   └── blockchain/      # Ethereum/L2 clients
│   └── api/                 # External interfaces
│       ├── grpc/            # gRPC services (tonic)
│       ├── rest/            # REST endpoints (axum)
│       └── websocket/       # Real-time streaming
├── proto/                   # Protocol Buffer definitions
├── schemas/sbe/             # SBE message schemas (IronSBE)
├── migrations/              # Database migrations
└── doc/                     # Design documentation
```

## Prerequisites

- **Rust**: 1.75+ (stable)
- **PostgreSQL**: 14+
- **Redis**: 7+
- **Kafka**: 3+ (optional, for event streaming)

## Installation

```bash
# Clone repository
git clone https://github.com/joaquinbejar/otc-rfq.git
cd otc-rfq

# Build
cargo build --release

# Run tests
cargo test --all-features

# Run lints
cargo clippy --all-targets --all-features -- -D warnings
```

## Configuration

Create a `.env` file or set environment variables:

```bash
# Database
DATABASE_URL=postgres://user:pass@localhost:5432/otc_rfq

# Redis
REDIS_URL=redis://localhost:6379

# API Ports
GRPC_PORT=50051
REST_PORT=8080

# Venue Configuration
FIX_SENDER_COMP_ID=OTC_RFQ
FIX_TARGET_COMP_ID=MARKET_MAKER

# DeFi
ZERO_X_API_KEY=your_api_key
ONE_INCH_API_KEY=your_api_key
```

## Usage

### Starting the Server

```bash
cargo run --release
```

### gRPC Client Example

```rust
use otc_rfq::api::grpc::RfqServiceClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = RfqServiceClient::connect("http://localhost:50051").await?;
    
    // Create RFQ
    let response = client.create_rfq(CreateRfqRequest {
        client_id: "DESK_001".to_string(),
        instrument: Some(Instrument {
            symbol: "BTC/USD".to_string(),
            asset_class: AssetClass::CryptoSpot as i32,
            ..Default::default()
        }),
        side: OrderSide::Buy as i32,
        quantity: Some(Decimal { value: "10.0".to_string() }),
        timeout_seconds: 30,
    }).await?;
    
    println!("RFQ created: {:?}", response.into_inner().rfq);
    Ok(())
}
```

### REST API

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

# Get RFQ
curl http://localhost:8080/api/v1/rfqs/{rfq_id}

# List venues
curl http://localhost:8080/api/v1/venues
```

## Documentation

Detailed documentation is available in the `doc/` directory:

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

## Performance Targets

| Metric | Target |
|--------|--------|
| Quote collection latency | < 100ms (p99) |
| Trade execution latency | < 50ms (p99) |
| Concurrent RFQs | 10,000+ |
| Quotes per second | 100,000+ |
| API response time | < 10ms (p99) |

## Development

```bash
# Format code
make fmt

# Run lints
make lint

# Run tests
make test

# Pre-push checks
make pre-push
```

## Roadmap

- [x] M0: Project Setup
- [ ] M1: Domain Layer
- [ ] M2: Infrastructure Layer
- [ ] M3: Application Layer
- [ ] M4: API Layer
- [ ] M5: Testing & Integration

See [GitHub Issues](https://github.com/joaquinbejar/otc-rfq/issues) for detailed progress.

## Contributing

1. Fork the repository
2. Create a feature branch: `git checkout -b M[N]/issue-[NUM]-description`
3. Follow the [Rust coding guidelines](.internalDoc/09-rust-guidelines.md)
4. Run `make pre-push` before committing
5. Submit a pull request

## Contact

- **Author**: Joaquín Béjar García
- **Email**: jb@taunais.com
- **Repository**: https://github.com/joaquinbejar/otc-rfq

## License

This project is licensed under the MIT License. See [LICENSE](./LICENSE) for details.
