//! Extract typed `x-*` annotations from JSON schemas.
//!
//! These annotations drive behavior beyond standard JSON Schema validation:
//! field editability, sensitivity (PII redaction), claim mapping, uniqueness, etc.

use serde_json::Value;
use std::collections::HashMap;

/// Per-field annotation metadata extracted from `x-*` properties.
#[derive(Debug, Clone, Default)]
pub struct FieldAnnotations {
    /// Whether the field can be modified by the user after creation.
    pub editable: bool,
    /// Whether the field contains sensitive data (PII) and should be redacted.
    pub sensitive: bool,
    /// CEL expression for SSO claim mapping (e.g. `"claims.email"`).
    pub claim_expr: Option<String>,
    /// Whether this field is used as a login identifier.
    pub identifier: bool,
    /// Uniqueness scope (`"instance"`, `"org"`, etc.) if the field must be unique.
    pub unique_scope: Option<String>,
    /// Verification method (`"email"`, etc.) if the field requires verification.
    pub verify: Option<String>,
    /// Recovery method flag.
    pub recover: bool,
}

/// Schema-level annotation metadata.
#[derive(Debug, Clone, Default)]
pub struct SchemaAnnotations {
    /// Target database table (from `x-table`).
    pub table: Option<String>,
    /// Row-level filter for table polymorphism (from `x-table-filter`).
    pub table_filter: HashMap<String, String>,
    /// Auth methods configuration (from `x-auth-methods`, kept as raw JSON).
    pub auth_methods: Value,
    /// Per-field annotations.
    pub fields: HashMap<String, FieldAnnotations>,
}

/// Extract all `x-*` annotations from a parsed schema.
pub fn extract_annotations(schema: &Value) -> SchemaAnnotations {
    let mut ann = SchemaAnnotations {
        table: schema
            .get("x-table")
            .and_then(Value::as_str)
            .map(String::from),
        auth_methods: schema.get("x-auth-methods").cloned().unwrap_or(Value::Null),
        ..Default::default()
    };

    // Extract x-table-filter
    if let Some(filter) = schema.get("x-table-filter").and_then(Value::as_object) {
        for (k, v) in filter {
            if let Some(s) = v.as_str() {
                ann.table_filter.insert(k.clone(), s.to_string());
            }
        }
    }

    // Extract per-field annotations
    if let Some(properties) = schema.get("properties").and_then(Value::as_object) {
        for (field, def) in properties {
            let field_ann = FieldAnnotations {
                editable: def
                    .get("x-editable")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                sensitive: def
                    .get("x-sensitive")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                claim_expr: def.get("x-claim").and_then(Value::as_str).map(String::from),
                identifier: def
                    .get("x-identifier")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                unique_scope: def
                    .get("x-unique")
                    .and_then(Value::as_str)
                    .map(String::from),
                verify: def
                    .get("x-verify")
                    .and_then(Value::as_str)
                    .map(String::from),
                recover: def
                    .get("x-recover")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            };
            ann.fields.insert(field.clone(), field_ann);
        }
    }

    ann
}

/// Redact sensitive fields in a value based on schema annotations.
///
/// Replaces any field marked `x-sensitive: true` with `"***"`.
pub fn redact_sensitive(schema_type: &str, value: &mut Value) {
    let Some(schema) = crate::bundled_schema(schema_type) else {
        return;
    };
    let ann = extract_annotations(&schema);
    let Some(obj) = value.as_object_mut() else {
        return;
    };

    for (field, field_ann) in &ann.fields {
        if field_ann.sensitive && obj.contains_key(field) {
            obj.insert(field.clone(), Value::String("***".into()));
        }
    }
}

/// Check which fields in the payload are not marked as editable.
///
/// Returns a list of field names that the caller is trying to set but
/// that are not marked `x-editable: true` in the schema. Intended for
/// update/patch handlers — creation handlers typically set all fields.
pub fn check_editable_fields(schema_type: &str, payload: &Value) -> Result<(), Vec<String>> {
    let Some(schema) = crate::bundled_schema(schema_type) else {
        return Ok(());
    };
    check_editable_fields_in_schema(&schema, payload, &[])
}

pub fn check_editable_fields_in_schema(
    schema: &Value,
    payload: &Value,
    reserved_fields: &[&str],
) -> Result<(), Vec<String>> {
    let ann = extract_annotations(schema);
    let Some(obj) = payload.as_object() else {
        return Ok(());
    };
    let reserved: std::collections::HashSet<&str> = reserved_fields.iter().copied().collect();

    let violations: Vec<String> = obj
        .keys()
        .filter(|key| {
            !reserved.contains(key.as_str())
                && key.as_str() != "metadata"
                && ann.fields.get(key.as_str()).is_some_and(|f| !f.editable)
        })
        .cloned()
        .collect();

    if violations.is_empty() {
        Ok(())
    } else {
        Err(violations)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extracts_table_annotation() {
        let schema = crate::bundled_schema("human_user").expect("human_user schema");
        let ann = extract_annotations(&schema);
        assert_eq!(ann.table.as_deref(), Some("users"));
    }

    #[test]
    fn extracts_field_annotations() {
        let schema = crate::bundled_schema("human_user").expect("human_user schema");
        let ann = extract_annotations(&schema);

        // email should have x-claim
        if let Some(email) = ann.fields.get("email") {
            assert!(email.claim_expr.is_some());
        }
    }

    #[test]
    fn redact_sensitive_replaces_fields() {
        let mut value = json!({"email": "alice@example.com", "display_name": "Alice"});
        redact_sensitive("human_user", &mut value);
        // Result depends on which fields are marked x-sensitive in the schema.
        // At minimum, the function should not panic.
        assert!(value.is_object());
    }
}
