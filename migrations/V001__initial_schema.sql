-- V001__initial_schema.sql
-- Initial database schema for OTC-RFQ system
-- 
-- This migration creates all core tables required for the RFQ trading system:
-- - RFQs and quotes
-- - Trades
-- - Venues
-- - Counterparties and wallets
-- - Domain events (event store)
--
-- Note: Timestamps are stored as BIGINT (milliseconds since epoch) for
-- compatibility with Rust i64 and precise timestamp handling.

-- =============================================================================
-- RFQ Tables
-- =============================================================================

-- Request for Quote table
-- Stores RFQ lifecycle state with JSONB for complex fields
CREATE TABLE IF NOT EXISTS rfqs (
    id VARCHAR(36) PRIMARY KEY,
    client_id VARCHAR(255) NOT NULL,
    instrument JSONB NOT NULL,
    side VARCHAR(10) NOT NULL CHECK (side IN ('BUY', 'SELL', 'Buy', 'Sell')),
    quantity DECIMAL(38, 18) NOT NULL,
    state VARCHAR(50) NOT NULL,
    expires_at BIGINT NOT NULL,
    quotes JSONB NOT NULL DEFAULT '[]',
    selected_quote_id VARCHAR(36),
    compliance_result JSONB,
    failure_reason TEXT,
    version BIGINT NOT NULL DEFAULT 0,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL
);

-- Indexes for common RFQ queries
CREATE INDEX IF NOT EXISTS idx_rfqs_client_id ON rfqs(client_id);
CREATE INDEX IF NOT EXISTS idx_rfqs_state ON rfqs(state);
CREATE INDEX IF NOT EXISTS idx_rfqs_created_at ON rfqs(created_at);
CREATE INDEX IF NOT EXISTS idx_rfqs_expires_at ON rfqs(expires_at);

-- Quotes table (standalone, for normalized storage)
-- Note: The repository currently stores quotes as JSONB in rfqs table,
-- but this table is available for future normalization
CREATE TABLE IF NOT EXISTS quotes (
    id VARCHAR(36) PRIMARY KEY,
    rfq_id VARCHAR(36) NOT NULL REFERENCES rfqs(id) ON DELETE CASCADE,
    venue_id VARCHAR(255) NOT NULL,
    price DECIMAL(38, 18) NOT NULL,
    quantity DECIMAL(38, 18) NOT NULL,
    commission DECIMAL(38, 18),
    valid_until BIGINT NOT NULL,
    metadata JSONB,
    created_at BIGINT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_quotes_rfq_id ON quotes(rfq_id);
CREATE INDEX IF NOT EXISTS idx_quotes_venue_id ON quotes(venue_id);
CREATE INDEX IF NOT EXISTS idx_quotes_valid_until ON quotes(valid_until);

-- =============================================================================
-- Trade Tables
-- =============================================================================

-- Trade table
-- Stores executed trades with settlement tracking
CREATE TABLE IF NOT EXISTS trades (
    id VARCHAR(36) PRIMARY KEY,
    rfq_id VARCHAR(36) NOT NULL,
    quote_id VARCHAR(36) NOT NULL,
    venue_id VARCHAR(255) NOT NULL,
    price DECIMAL(38, 18) NOT NULL,
    quantity DECIMAL(38, 18) NOT NULL,
    venue_execution_ref VARCHAR(255),
    settlement_state VARCHAR(50) NOT NULL,
    settlement_tx_ref VARCHAR(255),
    failure_reason TEXT,
    version BIGINT NOT NULL DEFAULT 0,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_trades_rfq_id ON trades(rfq_id);
CREATE INDEX IF NOT EXISTS idx_trades_venue_id ON trades(venue_id);
CREATE INDEX IF NOT EXISTS idx_trades_settlement_state ON trades(settlement_state);
CREATE INDEX IF NOT EXISTS idx_trades_created_at ON trades(created_at);

-- =============================================================================
-- Venue Tables
-- =============================================================================

-- Venue configuration table
-- Stores venue settings and supported instruments
CREATE TABLE IF NOT EXISTS venues (
    id VARCHAR(255) PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    venue_type VARCHAR(50) NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT true,
    priority INTEGER NOT NULL DEFAULT 0,
    supported_instruments JSONB NOT NULL DEFAULT '[]',
    config JSONB NOT NULL DEFAULT '{}',
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_venues_enabled ON venues(enabled);
CREATE INDEX IF NOT EXISTS idx_venues_venue_type ON venues(venue_type);

-- =============================================================================
-- Counterparty Tables
-- =============================================================================

-- Counterparty table
-- Stores counterparty information with KYC status and limits
CREATE TABLE IF NOT EXISTS counterparties (
    id VARCHAR(255) PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    counterparty_type VARCHAR(50) NOT NULL,
    kyc_status VARCHAR(50) NOT NULL DEFAULT 'NotStarted',
    limits JSONB NOT NULL DEFAULT '{}',
    wallet_addresses JSONB NOT NULL DEFAULT '[]',
    active BOOLEAN NOT NULL DEFAULT true,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_counterparties_type ON counterparties(counterparty_type);
CREATE INDEX IF NOT EXISTS idx_counterparties_kyc_status ON counterparties(kyc_status);
CREATE INDEX IF NOT EXISTS idx_counterparties_active ON counterparties(active);
CREATE INDEX IF NOT EXISTS idx_counterparties_name ON counterparties(name);

-- Wallet addresses for counterparties (normalized table)
-- Note: The repository currently stores wallets as JSONB in counterparties table,
-- but this table is available for future normalization
CREATE TABLE IF NOT EXISTS counterparty_wallets (
    id SERIAL PRIMARY KEY,
    counterparty_id VARCHAR(255) NOT NULL REFERENCES counterparties(id) ON DELETE CASCADE,
    chain VARCHAR(50) NOT NULL,
    address VARCHAR(255) NOT NULL,
    label VARCHAR(255),
    verified BOOLEAN NOT NULL DEFAULT false,
    created_at BIGINT NOT NULL,
    UNIQUE(counterparty_id, chain, address)
);

CREATE INDEX IF NOT EXISTS idx_counterparty_wallets_counterparty_id ON counterparty_wallets(counterparty_id);

-- =============================================================================
-- Event Store Tables
-- =============================================================================

-- Domain events table for event sourcing and audit trail
-- Append-only: events should never be updated or deleted
CREATE TABLE IF NOT EXISTS domain_events (
    id SERIAL PRIMARY KEY,
    event_id VARCHAR(36) NOT NULL UNIQUE,
    rfq_id VARCHAR(36),
    event_type VARCHAR(100) NOT NULL,
    event_name VARCHAR(100) NOT NULL,
    timestamp BIGINT NOT NULL,
    payload JSONB NOT NULL,
    sequence BIGINT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_domain_events_event_id ON domain_events(event_id);
CREATE INDEX IF NOT EXISTS idx_domain_events_rfq_id ON domain_events(rfq_id);
CREATE INDEX IF NOT EXISTS idx_domain_events_event_type ON domain_events(event_type);
CREATE INDEX IF NOT EXISTS idx_domain_events_timestamp ON domain_events(timestamp);
CREATE INDEX IF NOT EXISTS idx_domain_events_rfq_sequence ON domain_events(rfq_id, sequence);

-- =============================================================================
-- Comments
-- =============================================================================

COMMENT ON TABLE rfqs IS 'Request for Quote - stores RFQ lifecycle state';
COMMENT ON TABLE quotes IS 'Quotes received from venues (normalized storage)';
COMMENT ON TABLE trades IS 'Executed trades with settlement tracking';
COMMENT ON TABLE venues IS 'Venue configuration and supported instruments';
COMMENT ON TABLE counterparties IS 'Counterparty information with KYC and limits';
COMMENT ON TABLE counterparty_wallets IS 'Wallet addresses for counterparties';
COMMENT ON TABLE domain_events IS 'Append-only event store for domain events';

COMMENT ON COLUMN rfqs.instrument IS 'JSONB containing symbol, asset_class, settlement_method';
COMMENT ON COLUMN rfqs.quotes IS 'JSONB array of quotes received for this RFQ';
COMMENT ON COLUMN rfqs.version IS 'Optimistic locking version number';
COMMENT ON COLUMN trades.version IS 'Optimistic locking version number';
COMMENT ON COLUMN domain_events.sequence IS 'Sequence number for ordering within an aggregate';
