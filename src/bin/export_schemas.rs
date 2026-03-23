//! # Schema Export CLI
//!
//! Command-line tool to export event schema metadata.
//!
//! This tool generates a JSON file listing all event types with their
//! current schema versions. Useful for documentation and consumer reference.
//!
//! # TODO: Future Automation
//!
//! Currently, event types and versions are manually defined in a JSON literal.
//! This creates a maintenance burden where developers must remember to update
//! this file when adding new events. Future improvements should consider:
//! - Macro-based registration of events at compile time
//! - Trait-based collector that discovers events automatically
//! - Code generation from event definitions

use clap::Parser;
use std::fs;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "export-schemas")]
#[command(about = "Export event schema metadata to JSON", long_about = None)]
struct Args {
    /// Output directory for schema files
    #[arg(short, long, default_value = "./schemas")]
    output: PathBuf,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // Create output directory if it doesn't exist
    fs::create_dir_all(&args.output)?;

    // Define all event types with their current schema versions
    let event_schemas = serde_json::json!({
        "schema_registry_version": "1.0.0",
        "generated_at": chrono::Utc::now().to_rfc3339(),
        "events": {
            "RfqCreated": {
                "version": "1.0.0",
                "description": "Event emitted when a new RFQ is created",
                "category": "rfq"
            },
            "QuoteCollectionStarted": {
                "version": "1.0.0",
                "description": "Event emitted when quote collection starts",
                "category": "quote"
            },
            "QuoteRequested": {
                "version": "1.0.0",
                "description": "Event emitted when a quote is requested from a venue",
                "category": "quote"
            },
            "QuoteReceived": {
                "version": "1.0.0",
                "description": "Event emitted when a quote is received from a venue",
                "category": "quote"
            },
            "QuoteRequestFailed": {
                "version": "1.0.0",
                "description": "Event emitted when a quote request fails",
                "category": "quote"
            },
            "QuoteCollectionCompleted": {
                "version": "1.0.0",
                "description": "Event emitted when quote collection is complete",
                "category": "quote"
            },
            "QuoteSelected": {
                "version": "1.0.0",
                "description": "Event emitted when a quote is selected",
                "category": "quote"
            },
            "ExecutionStarted": {
                "version": "1.0.0",
                "description": "Event emitted when execution starts",
                "category": "trade"
            },
            "ExecutionFailed": {
                "version": "1.0.0",
                "description": "Event emitted when execution fails",
                "category": "trade"
            },
            "RfqCancelled": {
                "version": "1.0.0",
                "description": "Event emitted when an RFQ is cancelled",
                "category": "rfq"
            },
            "RfqExpired": {
                "version": "1.0.0",
                "description": "Event emitted when an RFQ expires",
                "category": "rfq"
            },
            "TradeExecuted": {
                "version": "1.0.0",
                "description": "Event emitted when a trade is executed",
                "category": "trade"
            },
            "SettlementInitiated": {
                "version": "1.0.0",
                "description": "Event emitted when settlement is initiated",
                "category": "settlement"
            },
            "SettlementConfirmed": {
                "version": "1.0.0",
                "description": "Event emitted when settlement is confirmed",
                "category": "settlement"
            },
            "SettlementFailed": {
                "version": "1.0.0",
                "description": "Event emitted when settlement fails",
                "category": "settlement"
            },
            "QuoteLocked": {
                "version": "1.0.0",
                "description": "Event emitted when a quote is locked for acceptance",
                "category": "acceptance"
            },
            "RiskCheckPassed": {
                "version": "1.0.0",
                "description": "Event emitted when risk check passes",
                "category": "acceptance"
            },
            "RiskCheckFailed": {
                "version": "1.0.0",
                "description": "Event emitted when risk check fails",
                "category": "acceptance"
            },
            "LastLookSent": {
                "version": "1.0.0",
                "description": "Event emitted when last-look request is sent",
                "category": "acceptance"
            },
            "LastLookConfirmed": {
                "version": "1.0.0",
                "description": "Event emitted when last-look is confirmed",
                "category": "acceptance"
            },
            "LastLookRejected": {
                "version": "1.0.0",
                "description": "Event emitted when last-look is rejected",
                "category": "acceptance"
            },
            "LastLookTimeout": {
                "version": "1.0.0",
                "description": "Event emitted when last-look times out",
                "category": "acceptance"
            },
            "AcceptanceCompleted": {
                "version": "1.0.0",
                "description": "Event emitted when acceptance flow completes",
                "category": "acceptance"
            },
            "AcceptanceFailed": {
                "version": "1.0.0",
                "description": "Event emitted when acceptance flow fails",
                "category": "acceptance"
            },
            "ComplianceCheckPassed": {
                "version": "1.0.0",
                "description": "Event emitted when compliance check passes",
                "category": "compliance"
            },
            "ComplianceCheckFailed": {
                "version": "1.0.0",
                "description": "Event emitted when compliance check fails",
                "category": "compliance"
            },
            "BlockTradeSubmitted": {
                "version": "1.0.0",
                "description": "Event emitted when block trade is submitted",
                "category": "block_trade"
            },
            "BlockTradeValidated": {
                "version": "1.0.0",
                "description": "Event emitted when block trade validation completes",
                "category": "block_trade"
            },
            "BlockTradeConfirmed": {
                "version": "1.0.0",
                "description": "Event emitted when counterparty confirms block trade",
                "category": "block_trade"
            },
            "BlockTradeApproved": {
                "version": "1.0.0",
                "description": "Event emitted when block trade is approved",
                "category": "block_trade"
            },
            "BlockTradeRejected": {
                "version": "1.0.0",
                "description": "Event emitted when block trade is rejected",
                "category": "block_trade"
            },
            "BlockTradeExecuted": {
                "version": "1.0.0",
                "description": "Event emitted when block trade is executed",
                "category": "block_trade"
            },
            "BlockTradeFailed": {
                "version": "1.0.0",
                "description": "Event emitted when block trade execution fails",
                "category": "block_trade"
            }
        }
    });

    // Write to file
    let output_path = args.output.join("event_schemas.json");
    let json_string = serde_json::to_string_pretty(&event_schemas)?;
    fs::write(&output_path, json_string)?;

    println!("✅ Event schemas exported to: {}", output_path.display());

    let event_count = event_schemas
        .get("events")
        .and_then(|e| e.as_object())
        .map_or(0, |o| o.len());
    println!("📊 Total events: {}", event_count);

    Ok(())
}
