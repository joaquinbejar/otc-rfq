//! # Schema Generator
//!
//! Generates JSON Schema from Rust types using `schemars`.

use crate::domain::errors::{DomainError, DomainResult};
use schemars::JsonSchema;

/// Generates JSON Schema from a type implementing `JsonSchema`.
///
/// # Errors
///
/// Returns error if schema serialization fails.
///
/// # Examples
///
/// ```
/// use otc_rfq::domain::schema::generate_schema;
/// use schemars::JsonSchema;
/// use serde::{Deserialize, Serialize};
///
/// #[derive(JsonSchema, Serialize, Deserialize)]
/// struct MyEvent {
///     id: String,
///     value: i32,
/// }
///
/// let schema = generate_schema::<MyEvent>().unwrap();
/// assert!(schema.contains("\"type\""));
/// ```
pub fn generate_schema<T: JsonSchema>() -> DomainResult<String> {
    let schema = schemars::schema_for!(T);
    serde_json::to_string_pretty(&schema).map_err(|e| DomainError::SchemaGenerationFailed {
        reason: e.to_string(),
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use schemars::JsonSchema;
    use serde::{Deserialize, Serialize};

    #[derive(JsonSchema, Serialize, Deserialize)]
    struct TestEvent {
        id: String,
        count: u32,
    }

    #[test]
    fn test_generate_schema() {
        let schema = generate_schema::<TestEvent>().unwrap();
        assert!(schema.contains("\"type\""));
        assert!(schema.contains("\"properties\""));
    }

    #[test]
    fn test_schema_is_valid_json() {
        let schema = generate_schema::<TestEvent>().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&schema).unwrap();
        assert!(parsed.is_object());
    }
}
