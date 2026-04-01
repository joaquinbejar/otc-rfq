//! # Export Format Types
//!
//! Defines the formats available for compliance report exports.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Supported formats for compliance report exports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    /// Comma-separated values format.
    Csv,
    /// JavaScript Object Notation format.
    #[default]
    Json,
}

impl ExportFormat {
    /// Returns the file extension for this format.
    #[must_use]
    pub const fn extension(&self) -> &'static str {
        match self {
            Self::Csv => "csv",
            Self::Json => "json",
        }
    }

    /// Returns the MIME type for this format.
    #[must_use]
    pub const fn mime_type(&self) -> &'static str {
        match self {
            Self::Csv => "text/csv",
            Self::Json => "application/json",
        }
    }

    /// Returns all supported export formats.
    #[must_use]
    pub const fn all() -> &'static [ExportFormat] {
        &[ExportFormat::Csv, ExportFormat::Json]
    }
}

impl fmt::Display for ExportFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Csv => write!(f, "CSV"),
            Self::Json => write!(f, "JSON"),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn display_formats_correctly() {
        assert_eq!(ExportFormat::Csv.to_string(), "CSV");
        assert_eq!(ExportFormat::Json.to_string(), "JSON");
    }

    #[test]
    fn extension_returns_correct_value() {
        assert_eq!(ExportFormat::Csv.extension(), "csv");
        assert_eq!(ExportFormat::Json.extension(), "json");
    }

    #[test]
    fn mime_type_returns_correct_value() {
        assert_eq!(ExportFormat::Csv.mime_type(), "text/csv");
        assert_eq!(ExportFormat::Json.mime_type(), "application/json");
    }

    #[test]
    fn serde_roundtrip() {
        for format in ExportFormat::all() {
            let json = serde_json::to_string(format).unwrap();
            let deserialized: ExportFormat = serde_json::from_str(&json).unwrap();
            assert_eq!(*format, deserialized);
        }
    }

    #[test]
    fn default_is_json() {
        assert_eq!(ExportFormat::default(), ExportFormat::Json);
    }

    #[test]
    fn all_returns_all_variants() {
        assert_eq!(ExportFormat::all().len(), 2);
    }
}
