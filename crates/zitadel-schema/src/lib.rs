use serde_json::Value;
use std::collections::HashMap;

const HUMAN_USER_SCHEMA: &str = include_str!("../../zitadel-api/src/schemas/human_user.json");
const PROVIDER_SCHEMA: &str = include_str!("../../zitadel-api/src/schemas/provider.json");

pub fn bundled_schema(schema_type: &str) -> Option<Value> {
    let raw = match schema_type {
        "human_user" => HUMAN_USER_SCHEMA,
        "provider" => PROVIDER_SCHEMA,
        _ => return None,
    };
    serde_json::from_str(raw).ok()
}

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
}
