//! # Report Export
//!
//! Export incentive reports to various formats (JSON, PDF).

use crate::domain::entities::settlement::IncentiveReport;
use printpdf::{
    FontId, Mm, Op, ParsedFont, PdfDocument, PdfFontHandle, PdfPage, PdfSaveOptions, Point, Pt,
    TextItem,
};
use rust_decimal::Decimal;

/// Embedded Roboto Regular font.
const ROBOTO_REGULAR: &[u8] = include_bytes!("assets/fonts/Roboto-Regular.ttf");

/// Embedded Roboto Bold font.
const ROBOTO_BOLD: &[u8] = include_bytes!("assets/fonts/Roboto-Bold.ttf");

/// Errors that can occur during report export.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ExportError {
    /// JSON serialization failed.
    #[error("JSON serialization failed: {0}")]
    JsonSerialization(String),

    /// PDF generation failed.
    #[error("PDF generation failed: {0}")]
    PdfGeneration(String),
}

/// Trait for exporting incentive reports to different formats.
pub trait ReportExporter {
    /// Exports a report to JSON format.
    ///
    /// # Arguments
    ///
    /// * `report` - The report to export
    ///
    /// # Returns
    ///
    /// JSON string representation of the report.
    ///
    /// # Errors
    ///
    /// Returns `ExportError::JsonSerialization` if serialization fails.
    fn export_to_json(&self, report: &IncentiveReport) -> Result<String, ExportError>;

    /// Exports a report to PDF format.
    ///
    /// # Arguments
    ///
    /// * `report` - The report to export
    ///
    /// # Returns
    ///
    /// PDF document as bytes.
    ///
    /// # Errors
    ///
    /// Returns `ExportError::PdfGeneration` if PDF generation fails.
    fn export_to_pdf(&self, report: &IncentiveReport) -> Result<Vec<u8>, ExportError>;
}

/// Default implementation of report exporter.
#[derive(Debug, Clone, Copy)]
pub struct IncentiveReportExporter;

impl IncentiveReportExporter {
    /// Creates a new report exporter.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for IncentiveReportExporter {
    fn default() -> Self {
        Self::new()
    }
}

/// Formats a Decimal value for PDF display without precision loss.
///
/// # Arguments
///
/// * `value` - The decimal value to format
/// * `decimals` - Number of decimal places to display
///
/// # Returns
///
/// Formatted string representation of the decimal.
fn format_decimal(value: Decimal, decimals: u32) -> String {
    value.round_dp(decimals).to_string()
}

impl ReportExporter for IncentiveReportExporter {
    fn export_to_json(&self, report: &IncentiveReport) -> Result<String, ExportError> {
        serde_json::to_string_pretty(report)
            .map_err(|e| ExportError::JsonSerialization(e.to_string()))
    }

    fn export_to_pdf(&self, report: &IncentiveReport) -> Result<Vec<u8>, ExportError> {
        let mut doc = PdfDocument::new("Incentive Report");

        // Parse and add fonts
        let mut warnings = Vec::new();
        let font_regular = ParsedFont::from_bytes(ROBOTO_REGULAR, 0, &mut warnings)
            .ok_or_else(|| ExportError::PdfGeneration("Failed to parse regular font".into()))?;
        let font_bold_parsed = ParsedFont::from_bytes(ROBOTO_BOLD, 0, &mut warnings)
            .ok_or_else(|| ExportError::PdfGeneration("Failed to parse bold font".into()))?;

        let font: FontId = doc.add_font(&font_regular);
        let font_bold: FontId = doc.add_font(&font_bold_parsed);

        let mut ops: Vec<Op> = Vec::new();
        let mut y_position = 270.0;
        let left_margin = 20.0;

        // Helper to add text at position
        fn add_text(ops: &mut Vec<Op>, text: &str, size: f32, x: f32, y: f32, font_id: &FontId) {
            ops.push(Op::StartTextSection);
            ops.push(Op::SetTextCursor {
                pos: Point {
                    x: Mm(x).into(),
                    y: Mm(y).into(),
                },
            });
            ops.push(Op::SetFont {
                font: PdfFontHandle::External(font_id.clone()),
                size: Pt(size),
            });
            ops.push(Op::ShowText {
                items: vec![TextItem::Text(text.to_string())],
            });
            ops.push(Op::EndTextSection);
        }

        // Title
        add_text(
            &mut ops,
            "Market Maker Incentive Report",
            24.0,
            left_margin,
            y_position,
            &font_bold,
        );
        y_position -= 15.0;

        // Header section
        add_text(
            &mut ops,
            &format!("MM ID: {}", report.mm_id()),
            12.0,
            left_margin,
            y_position,
            &font,
        );
        y_position -= 6.0;

        add_text(
            &mut ops,
            &format!("Period: {}", report.period()),
            12.0,
            left_margin,
            y_position,
            &font,
        );
        y_position -= 6.0;

        add_text(
            &mut ops,
            &format!("Generated: {}", report.generated_at().to_iso8601()),
            12.0,
            left_margin,
            y_position,
            &font,
        );
        y_position -= 6.0;

        add_text(
            &mut ops,
            &format!("Tier: {}", report.current_tier()),
            12.0,
            left_margin,
            y_position,
            &font,
        );
        y_position -= 15.0;

        // Summary section
        add_text(
            &mut ops,
            "Summary",
            16.0,
            left_margin,
            y_position,
            &font_bold,
        );
        y_position -= 10.0;

        let summary = report.summary();
        add_text(
            &mut ops,
            &format!("Total Trades: {}", summary.total_trades()),
            11.0,
            left_margin,
            y_position,
            &font,
        );
        y_position -= 5.0;

        add_text(
            &mut ops,
            &format!(
                "Total Volume: ${}",
                format_decimal(summary.total_volume_usd(), 2)
            ),
            11.0,
            left_margin,
            y_position,
            &font,
        );
        y_position -= 5.0;

        add_text(
            &mut ops,
            &format!(
                "Base Rebates: ${}",
                format_decimal(summary.total_base_rebates_usd(), 2)
            ),
            11.0,
            left_margin,
            y_position,
            &font,
        );
        y_position -= 5.0;

        add_text(
            &mut ops,
            &format!(
                "Spread Bonuses: ${}",
                format_decimal(summary.total_bonuses_usd(), 2)
            ),
            11.0,
            left_margin,
            y_position,
            &font,
        );
        y_position -= 5.0;

        add_text(
            &mut ops,
            &format!(
                "Net Payout: ${}",
                format_decimal(summary.net_payout_usd(), 2)
            ),
            11.0,
            left_margin,
            y_position,
            &font_bold,
        );
        y_position -= 15.0;

        // Penalties section (if any)
        if let Some(penalties) = report.penalties()
            && penalties.has_penalty()
        {
            add_text(
                &mut ops,
                "Penalties",
                16.0,
                left_margin,
                y_position,
                &font_bold,
            );
            y_position -= 10.0;

            if let Some(reason) = penalties.reason() {
                add_text(
                    &mut ops,
                    &format!("Reason: {}", reason),
                    11.0,
                    left_margin,
                    y_position,
                    &font,
                );
                y_position -= 5.0;
            }

            if penalties.should_reduce_capacity() {
                add_text(
                    &mut ops,
                    &format!(
                        "Capacity Reduction: {}%",
                        format_decimal(penalties.capacity_reduction_pct() * Decimal::from(100), 1)
                    ),
                    11.0,
                    left_margin,
                    y_position,
                    &font,
                );
                y_position -= 5.0;
            }

            if penalties.should_downgrade_tier() {
                add_text(
                    &mut ops,
                    "Tier Downgrade: Yes",
                    11.0,
                    left_margin,
                    y_position,
                    &font,
                );
                y_position -= 5.0;
            }

            y_position -= 10.0;
        }

        // Trade details section (if detailed report)
        if let Some(details) = report.trade_details() {
            add_text(
                &mut ops,
                "Trade Details",
                16.0,
                left_margin,
                y_position,
                &font_bold,
            );
            y_position -= 10.0;

            for (idx, detail) in details.iter().enumerate() {
                if y_position < 30.0 {
                    add_text(
                        &mut ops,
                        "(Additional trades truncated due to space limitations)",
                        9.0,
                        left_margin,
                        y_position,
                        &font,
                    );
                    break;
                }

                add_text(
                    &mut ops,
                    &format!(
                        "{}. Trade {} - ${} - Tier: {} - Rebate: ${}",
                        idx + 1,
                        detail.trade_id(),
                        format_decimal(detail.notional(), 2),
                        detail.tier(),
                        format_decimal(detail.rebate_amount(), 2)
                    ),
                    10.0,
                    left_margin,
                    y_position,
                    &font,
                );
                y_position -= 5.0;
            }
        }

        // Create page with operations
        let page = PdfPage::new(Mm(210.0), Mm(297.0), ops);

        // Save PDF to bytes
        let save_options = PdfSaveOptions {
            subset_fonts: true,
            ..Default::default()
        };

        let mut save_warnings = Vec::new();
        let pdf_bytes = doc
            .with_pages(vec![page])
            .save(&save_options, &mut save_warnings);

        Ok(pdf_bytes)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::domain::entities::mm_incentive::IncentiveTier;
    use crate::domain::entities::settlement::{
        IncentiveSettlement, IncentiveSummary, SettlementPeriod,
    };
    use crate::domain::value_objects::CounterpartyId;
    use crate::domain::value_objects::timestamp::Timestamp;
    use rust_decimal::Decimal;

    fn make_test_report() -> IncentiveReport {
        let mm_id = CounterpartyId::new("mm-test");
        let period = SettlementPeriod::from_month_year(2026, 3).unwrap();
        let settlement = IncentiveSettlement::new(mm_id.clone(), period);

        let summary = IncentiveSummary::new(
            10,
            Decimal::from(10_000_000),
            Decimal::from(-5000),
            Decimal::from(-500),
            Decimal::from(-5500),
        );

        IncentiveReport::new(
            settlement.id(),
            mm_id,
            period,
            Timestamp::now(),
            IncentiveTier::Silver,
            summary,
            None,
            None,
        )
    }

    #[test]
    fn export_to_json_produces_valid_json() {
        let exporter = IncentiveReportExporter::new();
        let report = make_test_report();

        let json = exporter.export_to_json(&report).unwrap();

        assert!(json.contains("mm-test"));
        assert!(json.contains("10000000"));

        // Verify it's valid JSON by parsing it back
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed.is_object());

        // Verify tier is present (serialized as enum variant)
        assert!(json.contains("current_tier"));
    }

    #[test]
    fn export_to_pdf_produces_valid_pdf() {
        let exporter = IncentiveReportExporter::new();
        let report = make_test_report();

        let pdf_bytes = exporter.export_to_pdf(&report).unwrap();

        assert!(!pdf_bytes.is_empty());
        assert!(pdf_bytes.starts_with(b"%PDF"));
    }

    #[test]
    fn json_roundtrip_preserves_data() {
        let exporter = IncentiveReportExporter::new();
        let report = make_test_report();

        let json = exporter.export_to_json(&report).unwrap();
        let parsed: IncentiveReport = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.mm_id(), report.mm_id());
        assert_eq!(parsed.period(), report.period());
        assert_eq!(parsed.current_tier(), report.current_tier());
        assert_eq!(
            parsed.summary().total_trades(),
            report.summary().total_trades()
        );
    }
}
