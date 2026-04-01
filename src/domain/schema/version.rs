//! # Schema Version
//!
//! Semantic versioning for event schemas.

use serde::{Deserialize, Serialize};

/// Semantic version for event schemas.
///
/// Follows semantic versioning (major.minor.patch):
/// - **Major**: Breaking changes (field removal, type changes)
/// - **Minor**: Backward-compatible additions (new optional fields)
/// - **Patch**: Bug fixes, documentation updates
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SchemaVersion {
    /// Major version number.
    pub major: u32,
    /// Minor version number.
    pub minor: u32,
    /// Patch version number.
    pub patch: u32,
}

impl SchemaVersion {
    /// Creates a new schema version.
    ///
    /// # Examples
    ///
    /// ```
    /// use otc_rfq::domain::schema::SchemaVersion;
    ///
    /// let version = SchemaVersion::new(1, 0, 0);
    /// assert_eq!(version.major, 1);
    /// ```
    #[must_use]
    pub const fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    /// Version 1.0.0 constant.
    pub const V1_0_0: Self = Self::new(1, 0, 0);

    /// Formats version as "major.minor.patch".
    ///
    /// # Examples
    ///
    /// ```
    /// use otc_rfq::domain::schema::SchemaVersion;
    ///
    /// let version = SchemaVersion::V1_0_0;
    /// assert_eq!(version.as_string(), "1.0.0");
    /// ```
    #[must_use]
    pub fn as_string(&self) -> String {
        format!("{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl Default for SchemaVersion {
    fn default() -> Self {
        Self::V1_0_0
    }
}

impl std::fmt::Display for SchemaVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_new_version() {
        let version = SchemaVersion::new(1, 2, 3);
        assert_eq!(version.major, 1);
        assert_eq!(version.minor, 2);
        assert_eq!(version.patch, 3);
    }

    #[test]
    fn test_v1_0_0_constant() {
        let version = SchemaVersion::V1_0_0;
        assert_eq!(version.major, 1);
        assert_eq!(version.minor, 0);
        assert_eq!(version.patch, 0);
    }

    #[test]
    fn test_as_string() {
        let version = SchemaVersion::new(2, 5, 7);
        assert_eq!(version.as_string(), "2.5.7");
    }

    #[test]
    fn test_display() {
        let version = SchemaVersion::new(1, 0, 0);
        assert_eq!(format!("{}", version), "1.0.0");
    }

    #[test]
    fn test_default() {
        let version = SchemaVersion::default();
        assert_eq!(version, SchemaVersion::V1_0_0);
    }

    #[test]
    fn test_ordering() {
        let v1 = SchemaVersion::new(1, 0, 0);
        let v2 = SchemaVersion::new(1, 1, 0);
        let v3 = SchemaVersion::new(2, 0, 0);

        assert!(v1 < v2);
        assert!(v2 < v3);
        assert!(v1 < v3);
    }

    #[test]
    fn test_serde_roundtrip() {
        let version = SchemaVersion::new(1, 2, 3);
        let json = serde_json::to_string(&version).unwrap();
        let deserialized: SchemaVersion = serde_json::from_str(&json).unwrap();
        assert_eq!(version, deserialized);
    }
}
