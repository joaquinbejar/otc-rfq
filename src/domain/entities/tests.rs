//! # Property-Based Tests for Domain Entities
//!
//! This module contains property-based tests using proptest for comprehensive
//! testing of domain entities, focusing on state machine invariants and
//! aggregate behavior.
//!
//! # Test Categories
//!
//! - **RFQ State Machine**: Valid/invalid transitions, invariant enforcement
//! - **Quote Validation**: Price/quantity constraints, expiry behavior
//! - **Trade Settlement**: State machine, immutability after terminal states
//! - **Venue/Counterparty**: Configuration and lifecycle tests

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use proptest::prelude::*;

use crate::domain::entities::counterparty::{Counterparty, CounterpartyType, KycStatus};
use crate::domain::entities::quote::{Quote, QuoteBuilder};
use crate::domain::entities::rfq::{ComplianceResult, Rfq, RfqBuilder};
use crate::domain::entities::trade::{SettlementState, Trade};
use crate::domain::entities::venue::{Venue, VenueHealth};
use crate::domain::value_objects::timestamp::Timestamp;
use crate::domain::value_objects::{
    CounterpartyId, Instrument, OrderSide, Price, Quantity, QuoteId, RfqId, RfqState, VenueId,
    VenueType,
};

// ============================================================================
// Test Helpers
// ============================================================================

fn test_instrument() -> Instrument {
    use crate::domain::value_objects::{AssetClass, Symbol};
    let symbol = Symbol::new("BTC/USD").unwrap();
    Instrument::builder(symbol, AssetClass::CryptoSpot).build()
}

fn test_rfq() -> Rfq {
    RfqBuilder::new(
        CounterpartyId::new("client-1"),
        test_instrument(),
        OrderSide::Buy,
        Quantity::new(1.0).unwrap(),
        Timestamp::now().add_secs(300),
    )
    .build()
}

fn test_quote(rfq_id: RfqId) -> Quote {
    QuoteBuilder::new(
        rfq_id,
        VenueId::new("venue-1"),
        Price::new(50000.0).unwrap(),
        Quantity::new(1.0).unwrap(),
        Timestamp::now().add_secs(60),
    )
    .build()
}

fn test_trade() -> Trade {
    Trade::new(
        RfqId::new_v4(),
        QuoteId::new_v4(),
        VenueId::new("venue-1"),
        Price::new(50000.0).unwrap(),
        Quantity::new(1.0).unwrap(),
    )
}

// ============================================================================
// RFQ State Machine Property Tests
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// RFQ version always increases on state transitions
    #[test]
    fn rfq_version_monotonically_increases(transitions in 1usize..5) {
        let mut rfq = test_rfq();
        let initial_version = rfq.version();

        // Perform valid transitions
        rfq.start_quote_collection().unwrap();
        prop_assert!(rfq.version() > initial_version);

        let quote = test_quote(rfq.id());
        let quote_id = quote.id();
        rfq.receive_quote(quote).unwrap();
        prop_assert!(rfq.version() > initial_version + 1);

        // Continue if more transitions requested
        if transitions > 2 {
            rfq.select_quote(quote_id).unwrap();
            prop_assert!(rfq.version() > initial_version + 2);
        }

        if transitions > 3 {
            rfq.start_execution().unwrap();
            prop_assert!(rfq.version() > initial_version + 3);
        }

        if transitions > 4 {
            rfq.mark_executed().unwrap();
            prop_assert!(rfq.version() > initial_version + 4);
        }
    }

    /// RFQ cannot transition from terminal states
    #[test]
    fn rfq_terminal_states_are_final(terminal_idx in 0usize..4) {
        let mut rfq = test_rfq();

        // Get to a terminal state
        let terminal_states = [
            RfqState::Executed,
            RfqState::Failed,
            RfqState::Cancelled,
            RfqState::Expired,
        ];

        let target = *terminal_states.get(terminal_idx).unwrap();

        // Navigate to terminal state
        match target {
            RfqState::Executed => {
                rfq.start_quote_collection().unwrap();
                let quote = test_quote(rfq.id());
                let quote_id = quote.id();
                rfq.receive_quote(quote).unwrap();
                rfq.select_quote(quote_id).unwrap();
                rfq.start_execution().unwrap();
                rfq.mark_executed().unwrap();
            }
            RfqState::Failed => {
                rfq.start_quote_collection().unwrap();
                rfq.mark_failed("test").unwrap();
            }
            RfqState::Cancelled => {
                rfq.cancel().unwrap();
            }
            RfqState::Expired => {
                rfq.expire().unwrap();
            }
            _ => unreachable!(),
        }

        prop_assert!(rfq.state().is_terminal());
        prop_assert!(!rfq.is_active());

        // Try all possible transitions - all should fail
        prop_assert!(rfq.start_quote_collection().is_err());
        prop_assert!(rfq.cancel().is_err());
        prop_assert!(rfq.expire().is_err());
        prop_assert!(rfq.mark_failed("test").is_err());
    }
}

// ============================================================================
// RFQ Quote Handling Tests
// ============================================================================

#[cfg(test)]
mod rfq_quote_handling {
    use super::*;

    #[test]
    fn cannot_receive_quote_for_different_rfq() {
        let mut rfq = test_rfq();
        rfq.start_quote_collection().unwrap();

        // Create quote for a different RFQ
        let other_rfq_id = RfqId::new_v4();
        let quote = test_quote(other_rfq_id);

        let result = rfq.receive_quote(quote);
        assert!(result.is_err());
    }

    #[test]
    fn cannot_select_nonexistent_quote() {
        let mut rfq = test_rfq();
        rfq.start_quote_collection().unwrap();

        let quote = test_quote(rfq.id());
        rfq.receive_quote(quote).unwrap();

        // Try to select a quote that doesn't exist
        let fake_quote_id = QuoteId::new_v4();
        let result = rfq.select_quote(fake_quote_id);
        assert!(result.is_err());
    }

    #[test]
    fn multiple_quotes_can_be_received() {
        let mut rfq = test_rfq();
        rfq.start_quote_collection().unwrap();

        // Receive multiple quotes
        for i in 0..5 {
            let quote = QuoteBuilder::new(
                rfq.id(),
                VenueId::new(format!("venue-{}", i)),
                Price::new(50000.0 + i as f64 * 100.0).unwrap(),
                Quantity::new(1.0).unwrap(),
                Timestamp::now().add_secs(60),
            )
            .build();
            rfq.receive_quote(quote).unwrap();
        }

        assert_eq!(rfq.quote_count(), 5);
    }

    #[test]
    fn selected_quote_is_accessible() {
        let mut rfq = test_rfq();
        rfq.start_quote_collection().unwrap();

        let quote = test_quote(rfq.id());
        let quote_id = quote.id();
        let venue_id = quote.venue_id().clone();
        rfq.receive_quote(quote).unwrap();
        rfq.select_quote(quote_id).unwrap();

        let selected = rfq.selected_quote().unwrap();
        assert_eq!(selected.id(), quote_id);
        assert_eq!(selected.venue_id(), &venue_id);
    }
}

// ============================================================================
// Trade Settlement State Machine Tests
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// Trade settlement follows valid state machine
    #[test]
    fn trade_settlement_valid_paths(path_idx in 0usize..2) {
        let mut trade = test_trade();

        prop_assert!(trade.is_pending());
        prop_assert!(!trade.is_terminal());

        trade.start_settlement().unwrap();
        prop_assert!(trade.is_in_progress());

        match path_idx {
            0 => {
                // Success path
                trade.confirm_settlement("tx-123").unwrap();
                prop_assert!(trade.is_settled());
                prop_assert!(trade.is_terminal());
                prop_assert_eq!(trade.settlement_tx_ref(), Some("tx-123"));
            }
            1 => {
                // Failure path
                trade.fail_settlement("network error").unwrap();
                prop_assert!(trade.is_failed());
                prop_assert!(trade.is_terminal());
                prop_assert_eq!(trade.failure_reason(), Some("network error"));
            }
            _ => unreachable!(),
        }
    }

    /// Trade cannot skip settlement states
    #[test]
    fn trade_cannot_skip_states(_dummy in 0..1) {
        let mut trade = test_trade();

        // Cannot confirm from Pending
        prop_assert!(trade.confirm_settlement("tx-123").is_err());

        // Cannot fail from Pending
        prop_assert!(trade.fail_settlement("error").is_err());
    }
}

#[cfg(test)]
mod trade_immutability {
    use super::*;

    #[test]
    fn settled_trade_is_immutable() {
        let mut trade = test_trade();
        trade.start_settlement().unwrap();
        trade.confirm_settlement("tx-123").unwrap();

        // All state changes should fail
        assert!(trade.start_settlement().is_err());
        assert!(trade.confirm_settlement("tx-456").is_err());
        assert!(trade.fail_settlement("error").is_err());
    }

    #[test]
    fn failed_trade_is_immutable() {
        let mut trade = test_trade();
        trade.start_settlement().unwrap();
        trade.fail_settlement("error").unwrap();

        // All state changes should fail
        assert!(trade.start_settlement().is_err());
        assert!(trade.confirm_settlement("tx-123").is_err());
        assert!(trade.fail_settlement("another error").is_err());
    }

    #[test]
    fn trade_version_tracks_changes() {
        let mut trade = test_trade();
        assert_eq!(trade.version(), 1);

        trade.start_settlement().unwrap();
        assert_eq!(trade.version(), 2);

        trade.confirm_settlement("tx-123").unwrap();
        assert_eq!(trade.version(), 3);
    }
}

// ============================================================================
// Quote Validation Tests
// ============================================================================

#[cfg(test)]
mod quote_validation {
    use super::*;
    use crate::domain::errors::DomainError;

    #[test]
    fn quote_requires_positive_price() {
        let result = QuoteBuilder::new(
            RfqId::new_v4(),
            VenueId::new("venue"),
            Price::zero(),
            Quantity::new(1.0).unwrap(),
            Timestamp::now().add_secs(60),
        )
        .try_build();

        assert!(matches!(result, Err(DomainError::InvalidPrice(_))));
    }

    #[test]
    fn quote_requires_positive_quantity() {
        let result = QuoteBuilder::new(
            RfqId::new_v4(),
            VenueId::new("venue"),
            Price::new(100.0).unwrap(),
            Quantity::zero(),
            Timestamp::now().add_secs(60),
        )
        .try_build();

        assert!(matches!(result, Err(DomainError::InvalidQuantity(_))));
    }

    #[test]
    fn quote_total_cost_calculation() {
        let quote = QuoteBuilder::new(
            RfqId::new_v4(),
            VenueId::new("venue"),
            Price::new(100.0).unwrap(),
            Quantity::new(2.5).unwrap(),
            Timestamp::now().add_secs(60),
        )
        .commission(Price::new(5.0).unwrap())
        .build();

        let total = quote.total_cost().unwrap();
        // 100 * 2.5 + 5 = 255
        assert_eq!(total, Price::new(255.0).unwrap());
    }

    #[test]
    fn quote_time_to_expiry() {
        let quote = QuoteBuilder::new(
            RfqId::new_v4(),
            VenueId::new("venue"),
            Price::new(100.0).unwrap(),
            Quantity::new(1.0).unwrap(),
            Timestamp::now().add_secs(120),
        )
        .build();

        let ttl = quote.time_to_expiry();
        // Should be approximately 120 seconds (allow some margin)
        assert!(ttl.as_secs() >= 115 && ttl.as_secs() <= 125);
    }
}

// ============================================================================
// Settlement State Tests
// ============================================================================

#[cfg(test)]
mod settlement_state_tests {
    use super::*;

    #[test]
    fn all_valid_transitions() {
        // Pending -> InProgress
        assert!(SettlementState::Pending.can_transition_to(SettlementState::InProgress));

        // InProgress -> Settled
        assert!(SettlementState::InProgress.can_transition_to(SettlementState::Settled));

        // InProgress -> Failed
        assert!(SettlementState::InProgress.can_transition_to(SettlementState::Failed));
    }

    #[test]
    fn all_invalid_transitions() {
        // Pending cannot go directly to Settled or Failed
        assert!(!SettlementState::Pending.can_transition_to(SettlementState::Settled));
        assert!(!SettlementState::Pending.can_transition_to(SettlementState::Failed));

        // Terminal states cannot transition
        assert!(!SettlementState::Settled.can_transition_to(SettlementState::Pending));
        assert!(!SettlementState::Settled.can_transition_to(SettlementState::InProgress));
        assert!(!SettlementState::Settled.can_transition_to(SettlementState::Failed));

        assert!(!SettlementState::Failed.can_transition_to(SettlementState::Pending));
        assert!(!SettlementState::Failed.can_transition_to(SettlementState::InProgress));
        assert!(!SettlementState::Failed.can_transition_to(SettlementState::Settled));
    }

    #[test]
    fn terminal_state_identification() {
        assert!(!SettlementState::Pending.is_terminal());
        assert!(!SettlementState::InProgress.is_terminal());
        assert!(SettlementState::Settled.is_terminal());
        assert!(SettlementState::Failed.is_terminal());
    }
}

// ============================================================================
// Venue Tests
// ============================================================================

#[cfg(test)]
mod venue_tests {
    use super::*;

    #[test]
    fn venue_availability_matrix() {
        let mut venue = Venue::new(VenueId::new("test"), "Test", VenueType::ExternalMM);

        // Enabled + Healthy = Available
        venue.set_enabled(true);
        venue.set_health(VenueHealth::Healthy);
        assert!(venue.is_available());

        // Enabled + Degraded = Available
        venue.set_health(VenueHealth::Degraded);
        assert!(venue.is_available());

        // Enabled + Unhealthy = Unavailable
        venue.set_health(VenueHealth::Unhealthy);
        assert!(!venue.is_available());

        // Enabled + Unknown = Unavailable
        venue.set_health(VenueHealth::Unknown);
        assert!(!venue.is_available());

        // Disabled + Healthy = Unavailable
        venue.set_enabled(false);
        venue.set_health(VenueHealth::Healthy);
        assert!(!venue.is_available());
    }

    #[test]
    fn venue_health_transitions() {
        let mut venue = Venue::new(VenueId::new("test"), "Test", VenueType::ExternalMM);

        assert!(venue.is_healthy());

        venue.set_health(VenueHealth::Degraded);
        assert!(venue.health().is_degraded());
        assert!(venue.health().is_operational());

        venue.set_health(VenueHealth::Unhealthy);
        assert!(venue.health().is_unhealthy());
        assert!(!venue.health().is_operational());
    }

    #[test]
    fn venue_metrics_tracking() {
        let mut venue = Venue::new(VenueId::new("test"), "Test", VenueType::ExternalMM);

        venue.metrics_mut().record_request(100, true);
        venue.metrics_mut().record_request(200, true);
        venue.metrics_mut().record_request(150, false);

        assert_eq!(venue.metrics().total_requests(), 3);
        assert_eq!(venue.metrics().successful_requests(), 2);
        assert_eq!(venue.metrics().failed_requests(), 1);
        assert_eq!(venue.metrics().average_latency_ms(), Some(150));
    }
}

// ============================================================================
// Counterparty Tests
// ============================================================================

#[cfg(test)]
mod counterparty_tests {
    use super::*;

    #[test]
    fn kyc_requirements_by_type() {
        // Client requires KYC
        let client = Counterparty::new(
            CounterpartyId::new("client"),
            "Client",
            CounterpartyType::Client,
        );
        assert!(!client.can_trade());

        // Market Maker requires KYC
        let mm = Counterparty::new(
            CounterpartyId::new("mm"),
            "MM",
            CounterpartyType::MarketMaker,
        );
        assert!(!mm.can_trade());

        // DEX doesn't require KYC
        let dex = Counterparty::new(CounterpartyId::new("dex"), "DEX", CounterpartyType::Dex);
        assert!(dex.can_trade());

        // Internal doesn't require KYC
        let internal = Counterparty::new(
            CounterpartyId::new("internal"),
            "Internal",
            CounterpartyType::Internal,
        );
        assert!(internal.can_trade());
    }

    #[test]
    fn kyc_status_progression() {
        let mut cp = Counterparty::new(
            CounterpartyId::new("client"),
            "Client",
            CounterpartyType::Client,
        );

        assert_eq!(cp.kyc_status(), KycStatus::NotStarted);
        assert!(!cp.can_trade());

        cp.set_kyc_status(KycStatus::Pending);
        assert!(!cp.can_trade());

        cp.set_kyc_status(KycStatus::Approved);
        assert!(cp.can_trade());

        cp.set_kyc_status(KycStatus::Rejected);
        assert!(!cp.can_trade());
    }

    #[test]
    fn inactive_counterparty_cannot_trade() {
        let mut cp = Counterparty::new(CounterpartyId::new("dex"), "DEX", CounterpartyType::Dex);

        assert!(cp.can_trade());

        cp.set_active(false);
        assert!(!cp.can_trade());
    }

    #[test]
    fn trading_limits_enforcement() {
        use crate::domain::entities::counterparty::CounterpartyLimits;

        let limits = CounterpartyLimits::new(
            Price::new(10_000.0).unwrap(),  // max per trade
            Price::new(100_000.0).unwrap(), // daily limit
        );

        // Within limits
        assert!(limits.check_trade_amount(Price::new(5_000.0).unwrap()));

        // Exceeds per-trade limit
        assert!(!limits.check_trade_amount(Price::new(15_000.0).unwrap()));
    }
}

// ============================================================================
// Compliance Result Tests
// ============================================================================

#[cfg(test)]
mod compliance_tests {
    use super::*;

    #[test]
    fn compliance_result_passed() {
        let result = ComplianceResult::passed();
        assert!(result.passed);
        assert!(result.reason.is_none());
    }

    #[test]
    fn compliance_result_failed() {
        let result = ComplianceResult::failed("KYC verification failed");
        assert!(!result.passed);
        assert_eq!(result.reason, Some("KYC verification failed".to_string()));
    }

    #[test]
    fn rfq_compliance_integration() {
        let mut rfq = test_rfq();

        assert!(rfq.compliance_result().is_none());

        rfq.set_compliance_result(ComplianceResult::passed());
        assert!(rfq.compliance_result().is_some());
        assert!(rfq.compliance_result().unwrap().passed);

        rfq.set_compliance_result(ComplianceResult::failed("AML check failed"));
        assert!(!rfq.compliance_result().unwrap().passed);
    }
}

// ============================================================================
// RFQ Builder Tests
// ============================================================================

#[cfg(test)]
mod rfq_builder_tests {
    use super::*;
    use crate::domain::errors::DomainError;

    #[test]
    fn builder_creates_valid_rfq() {
        let rfq = RfqBuilder::new(
            CounterpartyId::new("client"),
            test_instrument(),
            OrderSide::Buy,
            Quantity::new(10.0).unwrap(),
            Timestamp::now().add_secs(300),
        )
        .build();

        assert_eq!(rfq.state(), RfqState::Created);
        assert!(rfq.is_active());
        assert!(!rfq.has_quotes());
    }

    #[test]
    fn builder_rejects_zero_quantity() {
        let result = RfqBuilder::new(
            CounterpartyId::new("client"),
            test_instrument(),
            OrderSide::Buy,
            Quantity::zero(),
            Timestamp::now().add_secs(300),
        )
        .try_build();

        assert!(matches!(result, Err(DomainError::InvalidQuantity(_))));
    }

    #[test]
    fn rfq_accessors() {
        let client_id = CounterpartyId::new("client-123");
        let instrument = test_instrument();
        let quantity = Quantity::new(5.0).unwrap();

        let rfq = RfqBuilder::new(
            client_id.clone(),
            instrument.clone(),
            OrderSide::Sell,
            quantity,
            Timestamp::now().add_secs(300),
        )
        .build();

        assert_eq!(rfq.client_id(), &client_id);
        assert_eq!(rfq.instrument(), &instrument);
        assert_eq!(rfq.side(), OrderSide::Sell);
        assert_eq!(rfq.quantity(), quantity);
    }
}
