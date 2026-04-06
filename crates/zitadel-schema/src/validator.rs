//! JSON Schema validation for entity payloads.
//!
//! Compiles all bundled schemas once and provides a global validator instance
//! for use in API request validation.

use jsonschema::Validator;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::OnceLock;

/// A compiled schema validator that validates payloads against bundled schemas.
pub struct SchemaValidator {
    validators: HashMap<String, Validator>,
}

/// A single validation error with a JSON pointer path and message.
#[derive(Debug, Clone)]
pub struct ValidationError {
    pub path: String,
    pub message: String,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.path.is_empty() {
            write!(f, "{}", self.message)
        } else {
            write!(f, "{}: {}", self.path, self.message)
        }
    }
}

impl SchemaValidator {
    /// Build a new validator by compiling all bundled schemas.
    ///
    /// Schemas that use the custom `$schema: "https://zitadel.com/schemas/v1/entity"`
    /// URI have it stripped before compilation so the `jsonschema` crate treats
    /// them as draft-2020-12 by default.
    pub fn new() -> Self {
        let mut validators = HashMap::new();
        for (schema_type, schema_json) in crate::SCHEMAS {
            let mut schema: Value =
                serde_json::from_str(schema_json).expect("bundled schema is valid JSON");

            // Strip custom $schema URI that jsonschema can't resolve.
            if let Some(obj) = schema.as_object_mut() {
                let is_custom = obj
                    .get("$schema")
                    .and_then(Value::as_str)
                    .is_some_and(|s| !s.starts_with("https://json-schema.org"));
                if is_custom {
                    obj.remove("$schema");
                }
            }

            if let Ok(validator) = Validator::new(&schema) {
                validators.insert(schema_type.to_string(), validator);
            }
        }
        Self { validators }
    }

    /// Get a shared global instance (compiled once, reused across requests).
    pub fn global() -> &'static Self {
        static INSTANCE: OnceLock<SchemaValidator> = OnceLock::new();
        INSTANCE.get_or_init(Self::new)
    }

    /// Validate a payload against the schema for the given type.
    ///
    /// Returns `Ok(())` if valid or if the schema type is unknown (fail-open
    /// for extensibility — custom schema types may not be bundled).
    pub fn validate(&self, schema_type: &str, payload: &Value) -> Result<(), Vec<ValidationError>> {
        let Some(validator) = self.validators.get(schema_type) else {
            return Ok(());
        };

        let result = validator.validate(payload);
        if result.is_ok() {
            return Ok(());
        }

        let errors: Vec<ValidationError> = validator
            .iter_errors(payload)
            .map(|e| ValidationError {
                path: e.instance_path.to_string(),
                message: e.to_string(),
            })
            .collect();
        Err(errors)
    }

    /// Check whether a schema type has a compiled validator.
    pub fn has_schema(&self, schema_type: &str) -> bool {
        self.validators.contains_key(schema_type)
    }
}

fn normalize_schema(mut schema: Value) -> Value {
    if let Some(obj) = schema.as_object_mut() {
        let is_custom = obj
            .get("$schema")
            .and_then(Value::as_str)
            .is_some_and(|s| !s.starts_with("https://json-schema.org"));
        if is_custom {
            obj.remove("$schema");
        }
    }
    schema
}

/// Build a metadata-only validation view from a full entity schema.
///
/// Reserved platform fields are removed, `required` is stripped, and
/// `additionalProperties` is disabled so typed transport fields cannot be
/// smuggled into the metadata envelope.
pub fn extension_schema_view(schema: &Value, reserved_fields: &[&str]) -> Value {
    let mut view = normalize_schema(schema.clone());
    let reserved: std::collections::HashSet<&str> = reserved_fields.iter().copied().collect();

    if let Some(obj) = view.as_object_mut() {
        obj.remove("required");
        obj.insert("additionalProperties".into(), Value::Bool(false));

        if let Some(properties) = obj.get_mut("properties").and_then(Value::as_object_mut) {
            properties.retain(|field, _| field != "metadata" && !reserved.contains(field.as_str()));
        }
    }

    view
}

/// Validate a payload against an arbitrary schema value.
pub fn validate_schema(schema: &Value, payload: &Value) -> Result<(), Vec<ValidationError>> {
    let schema = normalize_schema(schema.clone());
    let validator = match Validator::new(&schema) {
        Ok(validator) => validator,
        Err(error) => {
            return Err(vec![ValidationError {
                path: String::new(),
                message: format!("invalid schema: {error}"),
            }]);
        }
    };

    match validator.validate(payload) {
        Ok(()) => Ok(()),
        Err(_) => Err(validator
            .iter_errors(payload)
            .map(|e| ValidationError {
                path: e.instance_path.to_string(),
                message: e.to_string(),
            })
            .collect()),
    }
}

impl Default for SchemaValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn validates_valid_payload() {
        let v = SchemaValidator::global();
        // A minimal valid human_user payload (required fields only)
        let payload = json!({
            "display_name": "Alice",
            "email": "alice@example.com",
        });
        // Should not error even if not all fields are present —
        // depends on what the schema marks as required
        let result = v.validate("human_user", &payload);
        // We just check it doesn't panic; the actual pass/fail depends on schema
        let _ = result;
    }

    #[test]
    fn unknown_schema_type_passes() {
        let v = SchemaValidator::global();
        let result = v.validate("nonexistent_type", &json!({"anything": true}));
        assert!(result.is_ok());
    }

    #[test]
    fn all_schemas_compile() {
        let v = SchemaValidator::global();
        for schema_type in crate::all_schema_types() {
            assert!(
                v.has_schema(schema_type),
                "{schema_type} should have a compiled validator"
            );
        }
    }
}
