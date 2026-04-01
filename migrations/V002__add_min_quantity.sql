-- V002__add_min_quantity.sql
-- Add min_quantity column to rfqs table
--
-- This migration adds support for persisting the optional minimum quantity
-- field that was previously lost on database round-trips.
--
-- The column is nullable to maintain backward compatibility with existing
-- RFQs that don't have this field set.

ALTER TABLE rfqs ADD COLUMN min_quantity DECIMAL(38, 18);

COMMENT ON COLUMN rfqs.min_quantity IS 'Optional minimum fill quantity for size negotiation modes';
