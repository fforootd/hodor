use anyhow::Context;
use serde_json::{Map, Value, json};
use zitadel_db::Db;

const META_SCHEMA: &str = include_str!("meta_schema.json");

pub async fn document(db: &Db, public_origin: &str) -> anyhow::Result<Value> {
    let meta_schema: Value = serde_json::from_str(META_SCHEMA).context("parse meta schema")?;
    let schema_registry = match load_schema_registry(db).await {
        Ok(registry) => registry,
        Err(error) => {
            tracing::warn!(error = %error, "openapi export could not load schema registry");
            Vec::new()
        }
    };

    let mut components = base_components(&meta_schema);
    let mut dynamic_schemas = Vec::with_capacity(schema_registry.len());
    for schema in schema_registry {
        let component = component_name(&schema.type_, &schema.id, schema.version);
        components.insert(component.clone(), schema.schema.clone());
        dynamic_schemas.push(json!({
            "id": schema.id,
            "type": schema.type_,
            "version": schema.version,
            "visibility": schema.visibility,
            "is_default": schema.is_default,
            "component": component,
        }));
    }

    Ok(json!({
        "openapi": "3.1.0",
        "info": {
            "title": "Zitadel API",
            "version": env!("CARGO_PKG_VERSION"),
            "description": "Experimental Zitadel REST+JSON API contract synthesized from built-in handlers and the live schema registry.",
        },
        "servers": [{
            "url": normalize_origin(public_origin),
        }],
        "tags": [
            {"name": "oidc"},
            {"name": "auth"},
            {"name": "users"},
            {"name": "schemas"},
        ],
        "paths": paths(),
        "components": {
            "schemas": Value::Object(components),
        },
        "x-zitadel": {
            "catalog": meta_schema.get("x-catalog").cloned().unwrap_or(Value::Null),
            "dynamic_schemas": dynamic_schemas,
        }
    }))
}

#[derive(Debug)]
struct SchemaRegistryEntry {
    id: String,
    type_: String,
    version: i64,
    visibility: String,
    is_default: bool,
    schema: Value,
}

async fn load_schema_registry(db: &Db) -> anyhow::Result<Vec<SchemaRegistryEntry>> {
    let scoped = db.scoped_default();
    let sql = format!(
        "SELECT id, type, {}, version, visibility, {} FROM schemas ORDER BY type, version DESC",
        scoped.as_text("schema"),
        scoped.bool_as_int("is_default"),
    );
    let rows = sqlx::query_as::<_, (String, String, String, i64, String, i64)>(&sql)
        .fetch_all(scoped.pool())
        .await
        .context("query schema registry")?;

    rows.into_iter()
        .map(|(id, type_, raw_schema, version, visibility, is_default)| {
            let schema = serde_json::from_str(&raw_schema)
                .with_context(|| format!("parse schema registry entry {id}"))?;
            Ok(SchemaRegistryEntry {
                id,
                type_,
                version,
                visibility,
                is_default: is_default != 0,
                schema,
            })
        })
        .collect()
}

fn base_components(meta_schema: &Value) -> Map<String, Value> {
    let mut components = Map::new();
    components.insert(
        "ErrorBody".into(),
        json!({
            "type": "object",
            "required": ["error"],
            "properties": {
                "error": {"type": "string"},
                "code": {"type": ["integer", "null"], "format": "int32"},
            }
        }),
    );
    components.insert(
        "WhoAmIResponse".into(),
        json!({
            "type": "object",
            "required": ["user_id", "session_id", "token_type", "org_id"],
            "properties": {
                "user_id": {"type": "string"},
                "session_id": {"type": "string"},
                "token_type": {"type": "string"},
                "org_id": {"type": "string"},
            }
        }),
    );
    components.insert(
        "UserRequest".into(),
        json!({
            "type": "object",
            "required": ["identifier"],
            "properties": {
                "identifier": {"type": "string"},
                "display_name": {"type": "string"},
                "schema_id": {"type": "string"},
                "state": {"type": "string"},
                "metadata": {"type": "object"},
            }
        }),
    );
    components.insert(
        "UserResponse".into(),
        json!({
            "type": "object",
            "required": ["id", "org_id", "identifier", "user_type", "state", "created_at", "updated_at"],
            "properties": {
                "id": {"type": "string"},
                "org_id": {"type": "string"},
                "identifier": {"type": "string"},
                "display_name": {"type": "string"},
                "user_type": {"type": "string"},
                "state": {"type": "string"},
                "schema_id": {"type": "string"},
                "metadata": {"type": ["object", "null"]},
                "created_at": {"type": "string"},
                "updated_at": {"type": "string"},
            }
        }),
    );
    components.insert("UserListResponse".into(), list_schema_ref("UserResponse"));
    components.insert(
        "PasswordRequest".into(),
        json!({
            "type": "object",
            "required": ["password"],
            "properties": {
                "password": {"type": "string"},
            }
        }),
    );
    components.insert(
        "SchemaResponse".into(),
        json!({
            "type": "object",
            "required": ["id", "type", "version", "is_default", "visibility", "created_at"],
            "properties": {
                "id": {"type": "string"},
                "type": {"type": "string"},
                "version": {"type": "integer"},
                "is_default": {"type": "boolean"},
                "visibility": {"type": "string"},
                "created_at": {"type": "string"},
                "schema": {"type": ["object", "array", "string", "number", "boolean", "null"]},
            }
        }),
    );
    components.insert(
        "SchemaListResponse".into(),
        list_schema_ref("SchemaResponse"),
    );
    components.insert(
        "CreateSchemaRequest".into(),
        json!({
            "type": "object",
            "required": ["type", "schema"],
            "properties": {
                "type": {"type": "string"},
                "schema": {"type": ["object", "array"]},
                "visibility": {"type": "string"},
            }
        }),
    );
    components.insert("EntityMetaSchema".into(), meta_schema.clone());
    components.insert(
        "OpenIdConfiguration".into(),
        json!({
            "type": "object",
            "required": [
                "issuer",
                "authorization_endpoint",
                "token_endpoint",
                "userinfo_endpoint",
                "jwks_uri",
                "response_types_supported",
                "grant_types_supported"
            ],
            "properties": {
                "issuer": {"type": "string"},
                "authorization_endpoint": {"type": "string"},
                "token_endpoint": {"type": "string"},
                "userinfo_endpoint": {"type": "string"},
                "jwks_uri": {"type": "string"},
                "revocation_endpoint": {"type": "string"},
                "end_session_endpoint": {"type": "string"},
                "response_types_supported": {"type": "array", "items": {"type": "string"}},
                "grant_types_supported": {"type": "array", "items": {"type": "string"}},
                "subject_types_supported": {"type": "array", "items": {"type": "string"}},
                "id_token_signing_alg_values_supported": {"type": "array", "items": {"type": "string"}},
                "scopes_supported": {"type": "array", "items": {"type": "string"}},
                "token_endpoint_auth_methods_supported": {"type": "array", "items": {"type": "string"}},
                "code_challenge_methods_supported": {"type": "array", "items": {"type": "string"}},
                "claims_supported": {"type": "array", "items": {"type": "string"}},
            }
        }),
    );
    components.insert(
        "TokenResponse".into(),
        json!({
            "type": "object",
            "required": ["access_token", "token_type", "expires_in"],
            "properties": {
                "access_token": {"type": "string"},
                "token_type": {"type": "string"},
                "expires_in": {"type": "integer"},
                "id_token": {"type": "string"},
                "refresh_token": {"type": "string"},
                "scope": {"type": "string"},
            }
        }),
    );
    components.insert(
        "JsonWebKeySet".into(),
        json!({
            "type": "object",
            "required": ["keys"],
            "properties": {
                "keys": {"type": "array", "items": {"type": "object"}}
            }
        }),
    );
    components
}

fn list_schema_ref(item_component: &str) -> Value {
    json!({
        "type": "object",
        "required": ["items"],
        "properties": {
            "items": {
                "type": "array",
                "items": { "$ref": format!("#/components/schemas/{item_component}") }
            },
            "next_cursor": {"type": ["string", "null"]},
            "total": {"type": ["integer", "null"]},
        }
    })
}

fn paths() -> Value {
    json!({
        "/.well-known/openid-configuration": {
            "get": operation("oidc", "getOpenIdConfiguration", "OIDC discovery document", json_response("OpenIdConfiguration"))
        },
        "/authorize": {
            "get": operation("oidc", "authorize", "OIDC authorization endpoint", empty_response(302, "Redirect to login or callback"))
        },
        "/oauth/token": {
            "post": operation("oidc", "exchangeToken", "OIDC token endpoint", json_response("TokenResponse"))
        },
        "/userinfo": {
            "get": operation("oidc", "userinfo", "OIDC userinfo endpoint", generic_object_response("User claims"))
        },
        "/keys": {
            "get": operation("oidc", "jwks", "OIDC JWKS endpoint", json_response("JsonWebKeySet"))
        },
        "/v1/auth/whoami": {
            "get": operation("auth", "whoami", "Return the current authenticated identity", json_response("WhoAmIResponse"))
        },
        "/v1/users": {
            "get": operation("users", "listUsers", "List users", json_response("UserListResponse")),
            "post": operation_with_body("users", "createUser", "Create user", "UserRequest", json_response_with_status(201, "UserResponse", "Created"))
        },
        "/v1/users/{id}": {
            "get": operation("users", "getUser", "Fetch user by id", json_response("UserResponse")),
            "patch": operation_with_body("users", "updateUser", "Update user", "UserRequest", json_response("UserResponse")),
            "delete": operation("users", "deleteUser", "Delete user", empty_response(204, "Deleted"))
        },
        "/v1/users/{id}/password": {
            "post": operation_with_body("users", "setUserPassword", "Set a user password", "PasswordRequest", generic_object_response("Mutation result"))
        },
        "/v1/schemas/$meta": {
            "get": operation("schemas", "getMetaSchema", "Return the embedded entity meta-schema catalog", json_response("EntityMetaSchema"))
        },
        "/v1/schemas": {
            "get": operation("schemas", "listSchemas", "List registered schemas", json_response("SchemaListResponse")),
            "post": operation_with_body("schemas", "createSchema", "Create schema", "CreateSchemaRequest", json_response_with_status(201, "SchemaResponse", "Created"))
        },
        "/v1/schemas/{id}": {
            "get": operation("schemas", "getSchema", "Get schema by id", json_response("SchemaResponse")),
            "patch": operation_with_body("schemas", "updateSchema", "Update schema", "CreateSchemaRequest", generic_object_response("Mutation result"))
        },
        "/v1/schemas/{id}/promote": {
            "post": operation("schemas", "promoteSchema", "Promote schema as default", generic_object_response("Mutation result"))
        },
        "/v1/schemas/{id}/identity-count": {
            "get": operation("schemas", "schemaIdentityCount", "Count users linked to a schema", generic_object_response("Identity count"))
        }
    })
}

fn operation(tag: &str, operation_id: &str, summary: &str, success: Value) -> Value {
    let mut responses = Map::new();
    responses.insert(success_status_code(&success), strip_status(success));
    responses.extend(default_error_responses(true));

    json!({
        "tags": [tag],
        "operationId": operation_id,
        "summary": summary,
        "responses": responses,
    })
}

fn operation_with_body(
    tag: &str,
    operation_id: &str,
    summary: &str,
    body_schema: &str,
    success: Value,
) -> Value {
    let mut operation = operation(tag, operation_id, summary, success);
    if let Some(operation) = operation.as_object_mut() {
        operation.insert(
            "requestBody".into(),
            json!({
                "required": true,
                "content": {
                    "application/json": {
                        "schema": {
                            "$ref": format!("#/components/schemas/{body_schema}")
                        }
                    }
                }
            }),
        );
    }
    operation
}

fn json_response(component: &str) -> Value {
    json_response_with_status(200, component, "OK")
}

fn json_response_with_status(status: u16, component: &str, description: &str) -> Value {
    json!({
        "status": status,
        "description": description,
        "content": {
            "application/json": {
                "schema": {
                    "$ref": format!("#/components/schemas/{component}")
                }
            }
        }
    })
}

fn generic_object_response(description: &str) -> Value {
    json!({
        "status": 200,
        "description": description,
        "content": {
            "application/json": {
                "schema": {
                    "type": "object"
                }
            }
        }
    })
}

fn empty_response(status: u16, description: &str) -> Value {
    json!({
        "status": status,
        "description": description,
    })
}

fn success_status_code(value: &Value) -> String {
    value
        .get("status")
        .and_then(Value::as_u64)
        .unwrap_or(200)
        .to_string()
}

fn strip_status(mut value: Value) -> Value {
    if let Some(map) = value.as_object_mut() {
        map.remove("status");
    }
    value
}

fn default_error_responses(include_unauthorized: bool) -> Map<String, Value> {
    let mut responses = Map::new();
    responses.insert("400".into(), error_response("Bad request"));
    if include_unauthorized {
        responses.insert("401".into(), error_response("Unauthorized"));
    }
    responses.insert("500".into(), error_response("Internal server error"));
    responses
}

fn error_response(description: &str) -> Value {
    json!({
        "description": description,
        "content": {
            "application/json": {
                "schema": {
                    "$ref": "#/components/schemas/ErrorBody"
                }
            }
        }
    })
}

fn component_name(type_: &str, id: &str, version: i64) -> String {
    format!(
        "Schema_{}_{}_v{}",
        sanitize_component_part(type_),
        sanitize_component_part(id),
        version
    )
}

fn sanitize_component_part(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out
        .chars()
        .next()
        .is_some_and(|first| first.is_ascii_digit())
    {
        out.insert(0, '_');
    }
    out
}

fn normalize_origin(public_origin: &str) -> String {
    public_origin.trim_end_matches('/').to_string()
}
