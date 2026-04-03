//! Expression evaluator for claim mapping using Google CEL (Common Expression Language).
//!
//! CEL is a non-Turing-complete expression language designed for policy evaluation.
//! We use it for mapping external IdP claims to Zitadel user profiles.
//!
//! Examples:
//! - `claims.email` — dot-path access
//! - `has(claims.email) ? claims.email : claims.upn` — conditional with presence check
//! - `claims.given_name + " " + claims.family_name` — string concat
//! - `claims.groups.exists(g, g == "admin")` — list macros
//! - `has(claims.email)` — presence check
//! - `claims.email.contains("@example.com")` — string methods
//! - `claims.email.startsWith("alice")` — string prefix check
//! - `size(claims.groups) > 0` — collection size

use serde_json::Value;
use std::collections::HashMap;

/// Evaluate a CEL expression against a JSON environment.
///
/// The environment typically contains `{"claims": <raw OIDC claims>}`.
pub fn eval(expr_str: &str, env: &Value) -> Result<Value, EvalError> {
    let program = cel_interpreter::Program::compile(expr_str).map_err(|e| EvalError {
        message: format!("parse: {e}"),
    })?;

    let mut context = cel_interpreter::Context::default();

    // Add all top-level JSON keys as CEL variables.
    if let Value::Object(map) = env {
        for (key, val) in map {
            context.add_variable(key, json_to_cel(val)).ok();
        }
    }

    let result = program.execute(&context).map_err(|e| EvalError {
        message: format!("eval: {e}"),
    })?;

    Ok(cel_to_json(&result))
}

/// Map raw claims through a set of field -> CEL expression mappings.
///
/// `defaults` come from the user schema's `x-claim` annotations.
/// `overrides` come from the provider's `mapping.claims` field (takes priority).
pub fn map_claims(
    defaults: &HashMap<String, String>,
    overrides: &HashMap<String, String>,
    raw_claims: &Value,
) -> HashMap<String, Value> {
    let mut merged: HashMap<String, &str> = HashMap::new();
    for (field, expr) in defaults {
        merged.insert(field.clone(), expr.as_str());
    }
    for (field, expr) in overrides {
        merged.insert(field.clone(), expr.as_str());
    }

    let env = serde_json::json!({ "claims": raw_claims });
    let mut result = HashMap::new();

    for (field, expr_str) in &merged {
        match eval(expr_str, &env) {
            Ok(val) if !val.is_null() => {
                if val.as_str().map(|s| !s.is_empty()).unwrap_or(true) {
                    result.insert(field.clone(), val);
                }
            }
            Ok(_) => {}
            Err(_) => {} // Skip fields that fail to evaluate.
        }
    }

    result
}

#[derive(Debug, Clone, PartialEq)]
pub struct EvalError {
    pub message: String,
}

impl std::fmt::Display for EvalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "cel: {}", self.message)
    }
}

impl std::error::Error for EvalError {}

// ─── JSON <-> CEL Value conversion ────────────────────────

fn json_to_cel(v: &Value) -> cel_interpreter::Value {
    match v {
        Value::Null => cel_interpreter::Value::Null,
        Value::Bool(b) => cel_interpreter::Value::Bool(*b),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                cel_interpreter::Value::Int(i)
            } else if let Some(f) = n.as_f64() {
                cel_interpreter::Value::Float(f)
            } else {
                cel_interpreter::Value::Null
            }
        }
        Value::String(s) => cel_interpreter::Value::String(s.clone().into()),
        Value::Array(arr) => {
            cel_interpreter::Value::List(arr.iter().map(json_to_cel).collect::<Vec<_>>().into())
        }
        Value::Object(map) => {
            let cel_map: HashMap<cel_interpreter::objects::Key, cel_interpreter::Value> = map
                .iter()
                .map(|(k, v)| {
                    (
                        cel_interpreter::objects::Key::String(k.clone().into()),
                        json_to_cel(v),
                    )
                })
                .collect();
            cel_interpreter::Value::Map(cel_map.into())
        }
    }
}

fn cel_to_json(v: &cel_interpreter::Value) -> Value {
    match v {
        cel_interpreter::Value::Null => Value::Null,
        cel_interpreter::Value::Bool(b) => Value::Bool(*b),
        cel_interpreter::Value::Int(i) => Value::Number((*i).into()),
        cel_interpreter::Value::UInt(u) => Value::Number((*u).into()),
        cel_interpreter::Value::Float(f) => serde_json::Number::from_f64(*f)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        cel_interpreter::Value::String(s) => Value::String(s.to_string()),
        cel_interpreter::Value::List(list) => Value::Array(list.iter().map(cel_to_json).collect()),
        cel_interpreter::Value::Map(map) => {
            let obj: serde_json::Map<String, Value> = map
                .map
                .iter()
                .map(|(k, v)| (format!("{k}"), cel_to_json(v)))
                .collect();
            Value::Object(obj)
        }
        cel_interpreter::Value::Bytes(b) => Value::String(String::from_utf8_lossy(b).to_string()),
        _ => Value::Null,
    }
}

// ─── Tests ────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn claims() -> Value {
        serde_json::json!({
            "claims": {
                "sub": "user-123",
                "email": "alice@example.com",
                "email_verified": true,
                "name": "Alice Smith",
                "given_name": "Alice",
                "family_name": "Smith",
                "preferred_username": "alice",
                "picture": "https://example.com/alice.jpg",
                "groups": ["admin", "users"],
                "address": {
                    "street": "123 Main St",
                    "city": "Testville"
                }
            }
        })
    }

    #[test]
    fn simple_path() {
        let env = claims();
        assert_eq!(eval("claims.email", &env).unwrap(), "alice@example.com");
    }

    #[test]
    fn nested_path() {
        let env = claims();
        assert_eq!(eval("claims.address.city", &env).unwrap(), "Testville");
    }

    #[test]
    fn has_presence_check() {
        let env = claims();
        assert_eq!(eval("has(claims.email)", &env).unwrap(), true);
        assert_eq!(eval("has(claims.phone)", &env).unwrap(), false);
    }

    #[test]
    fn ternary_fallback() {
        let env = claims();
        assert_eq!(
            eval("has(claims.phone) ? claims.phone : claims.email", &env).unwrap(),
            "alice@example.com"
        );
    }

    #[test]
    fn chained_ternary() {
        let env = claims();
        assert_eq!(
            eval(
                r#"has(claims.upn) ? claims.upn : has(claims.phone) ? claims.phone : claims.email"#,
                &env,
            )
            .unwrap(),
            "alice@example.com"
        );
    }

    #[test]
    fn string_concat() {
        let env = claims();
        assert_eq!(
            eval(r#"claims.given_name + " " + claims.family_name"#, &env).unwrap(),
            "Alice Smith"
        );
    }

    #[test]
    fn string_literal() {
        let env = claims();
        assert_eq!(eval(r#""hello world""#, &env).unwrap(), "hello world");
    }

    #[test]
    fn boolean_value() {
        let env = claims();
        assert_eq!(eval("claims.email_verified", &env).unwrap(), true);
    }

    #[test]
    fn string_contains() {
        let env = claims();
        assert_eq!(
            eval(r#"claims.email.contains("@example")"#, &env).unwrap(),
            true
        );
    }

    #[test]
    fn string_starts_with() {
        let env = claims();
        assert_eq!(
            eval(r#"claims.email.startsWith("alice")"#, &env).unwrap(),
            true
        );
    }

    #[test]
    fn list_exists() {
        let env = claims();
        assert_eq!(
            eval(r#"claims.groups.exists(g, g == "admin")"#, &env).unwrap(),
            true
        );
        assert_eq!(
            eval(r#"claims.groups.exists(g, g == "superadmin")"#, &env).unwrap(),
            false
        );
    }

    #[test]
    fn size_function() {
        let env = claims();
        assert_eq!(eval("size(claims.groups)", &env).unwrap(), 2);
    }

    #[test]
    fn comparison() {
        let env = claims();
        assert_eq!(
            eval(r#"claims.email == "alice@example.com""#, &env).unwrap(),
            true
        );
        assert_eq!(eval("size(claims.groups) > 1", &env).unwrap(), true);
    }

    #[test]
    fn logical_operators() {
        let env = claims();
        assert_eq!(
            eval("claims.email_verified && size(claims.groups) > 0", &env).unwrap(),
            true
        );
    }

    #[test]
    fn map_claims_basic() {
        let raw = serde_json::json!({
            "email": "alice@example.com",
            "name": "Alice Smith",
        });
        let mut overrides = HashMap::new();
        overrides.insert("email".into(), "claims.email".into());
        overrides.insert("display_name".into(), "claims.name".into());

        let result = map_claims(&HashMap::new(), &overrides, &raw);
        assert_eq!(result.get("email").unwrap(), "alice@example.com");
        assert_eq!(result.get("display_name").unwrap(), "Alice Smith");
    }

    #[test]
    fn map_claims_override_wins() {
        let raw = serde_json::json!({
            "email": "alice@example.com",
            "upn": "alice@corp.local",
        });
        let mut defaults = HashMap::new();
        defaults.insert("email".into(), "claims.email".into());

        let mut overrides = HashMap::new();
        overrides.insert(
            "email".into(),
            "has(claims.upn) ? claims.upn : claims.email".into(),
        );

        let result = map_claims(&defaults, &overrides, &raw);
        assert_eq!(result.get("email").unwrap(), "alice@corp.local");
    }

    #[test]
    fn map_claims_skips_null() {
        let raw = serde_json::json!({ "email": "alice@example.com" });
        let mut overrides = HashMap::new();
        overrides.insert("email".into(), "claims.email".into());
        overrides.insert(
            "phone".into(),
            "has(claims.phone) ? claims.phone : null".into(),
        );

        let result = map_claims(&HashMap::new(), &overrides, &raw);
        assert!(result.contains_key("email"));
        assert!(!result.contains_key("phone"));
    }

    #[test]
    fn entra_id_claim_mapping() {
        let raw = serde_json::json!({
            "preferred_username": "alice@corp.onmicrosoft.com",
            "name": "Alice Smith",
        });
        let mut overrides = HashMap::new();
        overrides.insert(
            "email".into(),
            r#"has(claims.preferred_username) ? claims.preferred_username : has(claims.email) ? claims.email : """#.into(),
        );
        overrides.insert("display_name".into(), "claims.name".into());

        let result = map_claims(&HashMap::new(), &overrides, &raw);
        assert_eq!(result.get("email").unwrap(), "alice@corp.onmicrosoft.com");
    }

    #[test]
    fn number_literal() {
        let env = serde_json::json!({});
        assert_eq!(eval("42", &env).unwrap(), 42);
    }
}
