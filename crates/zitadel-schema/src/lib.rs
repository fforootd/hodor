//! Schema registry and validation for Zitadel entity types.
//!
//! Bundles all entity JSON schemas from `crates/zitadel-api/src/schemas/`,
//! provides compiled validators, and extracts `x-*` annotation metadata.

pub mod annotations;
pub mod validator;

use serde_json::Value;
use std::collections::HashMap;

/// All bundled schema definitions, keyed by schema type name.
pub const SCHEMAS: &[(&str, &str)] = &[
    (
        "action",
        include_str!("../../zitadel-api/src/schemas/action.json"),
    ),
    (
        "ai_agent",
        include_str!("../../zitadel-api/src/schemas/ai_agent.json"),
    ),
    (
        "app",
        include_str!("../../zitadel-api/src/schemas/app.json"),
    ),
    (
        "group",
        include_str!("../../zitadel-api/src/schemas/group.json"),
    ),
    (
        "human_user",
        include_str!("../../zitadel-api/src/schemas/human_user.json"),
    ),
    (
        "job",
        include_str!("../../zitadel-api/src/schemas/job.json"),
    ),
    (
        "login_flow",
        include_str!("../../zitadel-api/src/schemas/login_flow.json"),
    ),
    (
        "org",
        include_str!("../../zitadel-api/src/schemas/org.json"),
    ),
    (
        "project",
        include_str!("../../zitadel-api/src/schemas/project.json"),
    ),
    (
        "provider",
        include_str!("../../zitadel-api/src/schemas/provider.json"),
    ),
    (
        "service_user",
        include_str!("../../zitadel-api/src/schemas/service_user.json"),
    ),
    (
        "session",
        include_str!("../../zitadel-api/src/schemas/session.json"),
    ),
];

/// Load a bundled schema by type name.
pub fn bundled_schema(schema_type: &str) -> Option<Value> {
    SCHEMAS
        .iter()
        .find(|(name, _)| *name == schema_type)
        .and_then(|(_, raw)| serde_json::from_str(raw).ok())
}

/// List all available schema type names.
pub fn all_schema_types() -> Vec<&'static str> {
    SCHEMAS.iter().map(|(name, _)| *name).collect()
}

/// Extract `x-claim` annotation defaults from a schema's top-level properties.
///
/// Returns a map of `field_name → CEL expression` for SSO claim mapping.
pub fn claim_defaults(schema: &Value) -> HashMap<String, String> {
    let mut defaults = HashMap::new();
    let Some(properties) = schema.get("properties").and_then(Value::as_object) else {
        return defaults;
    };

    for (field, definition) in properties {
        let Some(expr) = definition.get("x-claim").and_then(Value::as_str) else {
            continue;
        };
        defaults.insert(field.clone(), expr.to_string());
    }

    defaults
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_human_user_schema_is_available() {
        let schema = bundled_schema("human_user").expect("human_user schema");
        assert_eq!(schema.get("x-table").and_then(Value::as_str), Some("users"));
    }

    #[test]
    fn extracts_x_claim_defaults() {
        let schema = bundled_schema("human_user").expect("human_user schema");
        let defaults = claim_defaults(&schema);
        assert_eq!(
            defaults.get("email").map(String::as_str),
            Some("claims.email")
        );
        assert!(defaults.contains_key("display_name"));
    }

    #[test]
    fn all_schemas_parse() {
        for schema_type in all_schema_types() {
            bundled_schema(schema_type)
                .unwrap_or_else(|| panic!("{schema_type} schema should parse"));
        }
    }

    #[test]
    fn all_12_schemas_bundled() {
        assert_eq!(SCHEMAS.len(), 12);
    }
}
