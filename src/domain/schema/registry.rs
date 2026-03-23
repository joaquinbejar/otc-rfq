//! # Event Schema Registry
//!
//! In-memory registry for event schemas (documentation purposes only).

use crate::domain::errors::{DomainError, DomainResult};
use crate::domain::schema::SchemaVersion;
use dashmap::DashMap;
use std::sync::Arc;

/// Registry for event schemas (documentation purposes only).
///
/// Stores JSON Schema definitions generated from Rust types.
/// NOT used for runtime validation - schemas are for consumer documentation.
///
/// # Thread Safety
///
/// Uses `DashMap` to provide thread-safe concurrent access.
pub struct EventSchemaRegistry {
    schemas: DashMap<(Arc<str>, SchemaVersion), String>,
}

impl EventSchemaRegistry {
    /// Creates a new empty registry.
    ///
    /// # Examples
    ///
    /// ```
    /// use otc_rfq::domain::schema::EventSchemaRegistry;
    ///
    /// let registry = EventSchemaRegistry::new();
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self {
            schemas: DashMap::new(),
        }
    }

    /// Registers a schema for an event type and version.
    ///
    /// # Errors
    ///
    /// Returns error if schema is already registered for this version.
    ///
    /// # Examples
    ///
    /// ```
    /// use otc_rfq::domain::schema::{EventSchemaRegistry, SchemaVersion};
    ///
    /// let registry = EventSchemaRegistry::new();
    /// let result = registry.register(
    ///     "RfqCreated".to_string(),
    ///     SchemaVersion::V1_0_0,
    ///     r#"{"type": "object"}"#.to_string(),
    /// );
    /// assert!(result.is_ok());
    /// ```
    pub fn register(
        &self,
        event_type: String,
        version: SchemaVersion,
        schema_json: String,
    ) -> DomainResult<()> {
        let key = (Arc::from(event_type.as_str()), version);

        match self.schemas.entry(key) {
            dashmap::mapref::entry::Entry::Vacant(entry) => {
                entry.insert(schema_json);
                Ok(())
            }
            dashmap::mapref::entry::Entry::Occupied(_) => {
                Err(DomainError::SchemaAlreadyRegistered {
                    event_type,
                    version: version.as_string(),
                })
            }
        }
    }

    /// Gets schema for a specific event type and version.
    ///
    /// # Errors
    ///
    /// Returns error if schema not found.
    ///
    /// # Examples
    ///
    /// ```
    /// use otc_rfq::domain::schema::{EventSchemaRegistry, SchemaVersion};
    ///
    /// let registry = EventSchemaRegistry::new();
    /// registry.register(
    ///     "RfqCreated".to_string(),
    ///     SchemaVersion::V1_0_0,
    ///     r#"{"type": "object"}"#.to_string(),
    /// ).unwrap();
    ///
    /// let schema = registry.get("RfqCreated", &SchemaVersion::V1_0_0);
    /// assert!(schema.is_ok());
    /// ```
    pub fn get(&self, event_type: &str, version: &SchemaVersion) -> DomainResult<String> {
        let key = (Arc::from(event_type), *version);

        self.schemas
            .get(&key)
            .map(|entry| entry.value().clone())
            .ok_or_else(|| DomainError::SchemaNotFound {
                event_type: event_type.to_string(),
                version: version.as_string(),
            })
    }

    /// Lists all registered versions for an event type.
    ///
    /// Returns versions in sorted order (oldest to newest).
    ///
    /// # Examples
    ///
    /// ```
    /// use otc_rfq::domain::schema::{EventSchemaRegistry, SchemaVersion};
    ///
    /// let registry = EventSchemaRegistry::new();
    /// registry.register(
    ///     "RfqCreated".to_string(),
    ///     SchemaVersion::V1_0_0,
    ///     r#"{"type": "object"}"#.to_string(),
    /// ).unwrap();
    ///
    /// let versions = registry.list_versions("RfqCreated");
    /// assert_eq!(versions.len(), 1);
    /// ```
    #[must_use]
    pub fn list_versions(&self, event_type: &str) -> Vec<SchemaVersion> {
        let event_type_arc = Arc::from(event_type);
        let mut versions: Vec<SchemaVersion> = self
            .schemas
            .iter()
            .filter_map(|entry| {
                let (key, _) = entry.pair();
                if key.0 == event_type_arc {
                    Some(key.1)
                } else {
                    None
                }
            })
            .collect();

        versions.sort();
        versions
    }

    /// Returns the total number of registered schemas.
    #[must_use]
    pub fn len(&self) -> usize {
        self.schemas.len()
    }

    /// Returns true if the registry is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.schemas.is_empty()
    }
}

impl Default for EventSchemaRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_new_registry() {
        let registry = EventSchemaRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn test_register_schema() {
        let registry = EventSchemaRegistry::new();
        let result = registry.register(
            "RfqCreated".to_string(),
            SchemaVersion::V1_0_0,
            r#"{"type": "object"}"#.to_string(),
        );
        assert!(result.is_ok());
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn test_register_duplicate_fails() {
        let registry = EventSchemaRegistry::new();
        registry
            .register(
                "RfqCreated".to_string(),
                SchemaVersion::V1_0_0,
                r#"{"type": "object"}"#.to_string(),
            )
            .unwrap();

        let result = registry.register(
            "RfqCreated".to_string(),
            SchemaVersion::V1_0_0,
            r#"{"type": "object", "updated": true}"#.to_string(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_get_schema() {
        let registry = EventSchemaRegistry::new();
        let schema_json = r#"{"type": "object"}"#.to_string();

        registry
            .register(
                "RfqCreated".to_string(),
                SchemaVersion::V1_0_0,
                schema_json.clone(),
            )
            .unwrap();

        let retrieved = registry.get("RfqCreated", &SchemaVersion::V1_0_0).unwrap();
        assert_eq!(retrieved, schema_json);
    }

    #[test]
    fn test_get_nonexistent_schema() {
        let registry = EventSchemaRegistry::new();
        let result = registry.get("NonExistent", &SchemaVersion::V1_0_0);
        assert!(result.is_err());
    }

    #[test]
    fn test_list_versions() {
        let registry = EventSchemaRegistry::new();

        registry
            .register(
                "RfqCreated".to_string(),
                SchemaVersion::new(1, 0, 0),
                r#"{"v": "1.0.0"}"#.to_string(),
            )
            .unwrap();

        registry
            .register(
                "RfqCreated".to_string(),
                SchemaVersion::new(1, 1, 0),
                r#"{"v": "1.1.0"}"#.to_string(),
            )
            .unwrap();

        registry
            .register(
                "RfqCreated".to_string(),
                SchemaVersion::new(2, 0, 0),
                r#"{"v": "2.0.0"}"#.to_string(),
            )
            .unwrap();

        let versions = registry.list_versions("RfqCreated");
        assert_eq!(versions.len(), 3);
        assert_eq!(versions.first(), Some(&SchemaVersion::new(1, 0, 0)));
        assert_eq!(versions.get(1), Some(&SchemaVersion::new(1, 1, 0)));
        assert_eq!(versions.get(2), Some(&SchemaVersion::new(2, 0, 0)));
    }

    #[test]
    fn test_list_versions_empty() {
        let registry = EventSchemaRegistry::new();
        let versions = registry.list_versions("NonExistent");
        assert!(versions.is_empty());
    }

    #[test]
    fn test_multiple_event_types() {
        let registry = EventSchemaRegistry::new();

        registry
            .register(
                "RfqCreated".to_string(),
                SchemaVersion::V1_0_0,
                r#"{"type": "rfq"}"#.to_string(),
            )
            .unwrap();

        registry
            .register(
                "TradeExecuted".to_string(),
                SchemaVersion::V1_0_0,
                r#"{"type": "trade"}"#.to_string(),
            )
            .unwrap();

        assert_eq!(registry.len(), 2);
        assert_eq!(registry.list_versions("RfqCreated").len(), 1);
        assert_eq!(registry.list_versions("TradeExecuted").len(), 1);
    }
}
