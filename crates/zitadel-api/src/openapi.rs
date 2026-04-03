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
            {"name": "authorization"},
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
    components.insert("EmptyObject".into(), json!({"type": "object"}));
    components.insert(
        "FGATupleKey".into(),
        json!({
            "type": "object",
            "required": ["user", "relation", "object"],
            "properties": {
                "user": {"type": "string"},
                "relation": {"type": "string"},
                "object": {"type": "string"},
                "condition": {
                    "anyOf": [
                        {"$ref": "#/components/schemas/FGARelationshipCondition"},
                        {"type": "null"}
                    ]
                }
            }
        }),
    );
    components.insert(
        "FGARelationshipCondition".into(),
        json!({
            "type": "object",
            "required": ["name"],
            "properties": {
                "name": {"type": "string"},
                "context": {
                    "type": "object",
                    "additionalProperties": true,
                }
            }
        }),
    );
    components.insert(
        "FGAContextualTuples".into(),
        json!({
            "type": "object",
            "properties": {
                "tuple_keys": {
                    "type": "array",
                    "items": {"$ref": "#/components/schemas/FGATupleKey"}
                }
            }
        }),
    );
    components.insert(
        "FGACheckRequest".into(),
        json!({
            "type": "object",
            "required": ["user", "relation", "object"],
            "properties": {
                "user": {"type": "string"},
                "relation": {"type": "string"},
                "object": {"type": "string"},
            }
        }),
    );
    components.insert(
        "FGACheckResponse".into(),
        json!({
            "type": "object",
            "required": ["allowed", "user", "relation", "object"],
            "properties": {
                "allowed": {"type": "boolean"},
                "user": {"type": "string"},
                "relation": {"type": "string"},
                "object": {"type": "string"},
            }
        }),
    );
    components.insert(
        "FGAStoreCheckRequest".into(),
        json!({
            "type": "object",
            "required": ["tuple_key"],
            "properties": {
                "tuple_key": {"$ref": "#/components/schemas/FGATupleKey"},
                "authorization_model_id": {"type": "string"},
                "contextual_tuples": {"$ref": "#/components/schemas/FGAContextualTuples"},
                "context": {
                    "anyOf": [
                        {"type": "object", "additionalProperties": true},
                        {"type": "array"},
                        {"type": "string"},
                        {"type": "number"},
                        {"type": "boolean"},
                        {"type": "null"}
                    ]
                }
            }
        }),
    );
    components.insert(
        "FGAStoreCheckResponse".into(),
        json!({
            "type": "object",
            "required": ["allowed"],
            "properties": {
                "allowed": {"type": "boolean"}
            }
        }),
    );
    components.insert(
        "FGABatchCheckItem".into(),
        json!({
            "type": "object",
            "required": ["tuple_key"],
            "properties": {
                "tuple_key": {"$ref": "#/components/schemas/FGATupleKey"},
                "correlation_id": {"type": "string"},
            }
        }),
    );
    components.insert(
        "FGABatchCheckRequest".into(),
        json!({
            "type": "object",
            "properties": {
                "checks": {
                    "type": "array",
                    "items": {"$ref": "#/components/schemas/FGABatchCheckItem"}
                },
                "authorization_model_id": {"type": "string"},
                "contextual_tuples": {"$ref": "#/components/schemas/FGAContextualTuples"},
                "context": {
                    "anyOf": [
                        {"type": "object", "additionalProperties": true},
                        {"type": "array"},
                        {"type": "string"},
                        {"type": "number"},
                        {"type": "boolean"},
                        {"type": "null"}
                    ]
                }
            }
        }),
    );
    components.insert(
        "FGABatchCheckResult".into(),
        json!({
            "type": "object",
            "required": ["allowed"],
            "properties": {
                "correlation_id": {"type": "string"},
                "allowed": {"type": "boolean"},
            }
        }),
    );
    components.insert(
        "FGABatchTestRequest".into(),
        json!({
            "type": "object",
            "required": ["assertions"],
            "properties": {
                "assertions": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "required": ["user", "relation", "object", "expected"],
                        "properties": {
                            "user": {"type": "string"},
                            "relation": {"type": "string"},
                            "object": {"type": "string"},
                            "expected": {"type": "boolean"},
                        }
                    }
                }
            }
        }),
    );
    components.insert(
        "FGABatchTestResponse".into(),
        json!({
            "type": "object",
            "required": ["results", "total", "passed", "failed"],
            "properties": {
                "results": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "required": ["user", "relation", "object", "expected", "actual", "pass"],
                        "properties": {
                            "user": {"type": "string"},
                            "relation": {"type": "string"},
                            "object": {"type": "string"},
                            "expected": {"type": "boolean"},
                            "actual": {"type": "boolean"},
                            "pass": {"type": "boolean"},
                            "error": {"type": "string"},
                        }
                    }
                },
                "total": {"type": "integer"},
                "passed": {"type": "integer"},
                "failed": {"type": "integer"},
            }
        }),
    );
    components.insert(
        "FGAStoreBatchCheckResponse".into(),
        json!({
            "type": "object",
            "properties": {
                "results": {
                    "type": "array",
                    "items": {"$ref": "#/components/schemas/FGABatchCheckResult"}
                }
            }
        }),
    );
    components.insert(
        "FGATupleFilter".into(),
        json!({
            "type": "object",
            "properties": {
                "user": {"type": "string"},
                "relation": {"type": "string"},
                "object": {"type": "string"},
            }
        }),
    );
    components.insert(
        "FGAStoreReadRequest".into(),
        json!({
            "type": "object",
            "properties": {
                "tuple_key": {"$ref": "#/components/schemas/FGATupleFilter"},
                "page_size": {"type": "integer"},
                "continuation_token": {"type": "string"},
            }
        }),
    );
    components.insert(
        "FGAStoreTupleRecord".into(),
        json!({
            "type": "object",
            "required": ["key"],
            "properties": {
                "key": {"$ref": "#/components/schemas/FGATupleKey"},
                "timestamp": {"type": "string"},
            }
        }),
    );
    components.insert(
        "FGAReadTuplesResponse".into(),
        json!({
            "type": "object",
            "required": ["tuples"],
            "properties": {
                "tuples": {
                    "type": "array",
                    "items": {"$ref": "#/components/schemas/FGATupleKey"}
                }
            }
        }),
    );
    components.insert(
        "FGAStoreReadResponse".into(),
        json!({
            "type": "object",
            "properties": {
                "tuples": {
                    "type": "array",
                    "items": {"$ref": "#/components/schemas/FGAStoreTupleRecord"}
                },
                "continuation_token": {"type": "string"},
            }
        }),
    );
    components.insert(
        "FGATupleKeySet".into(),
        json!({
            "type": "object",
            "properties": {
                "tuple_keys": {
                    "type": "array",
                    "items": {"$ref": "#/components/schemas/FGATupleKey"}
                }
            }
        }),
    );
    components.insert(
        "FGAWriteTuplesRequest".into(),
        json!({
            "type": "object",
            "required": ["tuples"],
            "properties": {
                "tuples": {
                    "type": "array",
                    "items": {"$ref": "#/components/schemas/FGATupleKey"}
                }
            }
        }),
    );
    components.insert(
        "FGAStoreWriteRequest".into(),
        json!({
            "type": "object",
            "properties": {
                "writes": {"$ref": "#/components/schemas/FGATupleKeySet"},
                "deletes": {"$ref": "#/components/schemas/FGATupleKeySet"},
                "authorization_model_id": {"type": "string"},
            }
        }),
    );
    components.insert(
        "FGAWriteTuplesResponse".into(),
        json!({
            "type": "object",
            "required": ["status", "written"],
            "properties": {
                "status": {"type": "string"},
                "written": {"type": "integer"},
            }
        }),
    );
    components.insert(
        "FGADeleteTuplesResponse".into(),
        json!({
            "type": "object",
            "required": ["status", "deleted"],
            "properties": {
                "status": {"type": "string"},
                "deleted": {"type": "integer"},
            }
        }),
    );
    components.insert(
        "FGAExpandRequest".into(),
        json!({
            "type": "object",
            "required": ["relation", "object"],
            "properties": {
                "relation": {"type": "string"},
                "object": {"type": "string"},
                "authorization_model_id": {"type": "string"},
                "contextual_tuples": {"$ref": "#/components/schemas/FGAContextualTuples"},
            }
        }),
    );
    components.insert(
        "FGAExpandResponse".into(),
        json!({
            "type": "object",
            "required": ["tree"],
            "properties": {
                "tree": {
                    "$ref": "#/components/schemas/FGAExpandNode"
                }
            }
        }),
    );
    components.insert(
        "FGAExpandNode".into(),
        json!({
            "type": "object",
            "required": ["name"],
            "properties": {
                "name": {"type": "string"},
                "children": {
                    "type": "array",
                    "items": {"$ref": "#/components/schemas/FGAExpandNode"}
                },
                "users": {
                    "type": "array",
                    "items": {"type": "string"}
                }
            }
        }),
    );
    components.insert(
        "FGAListObjectsRequest".into(),
        json!({
            "type": "object",
            "required": ["user", "relation", "type"],
            "properties": {
                "user": {"type": "string"},
                "relation": {"type": "string"},
                "type": {"type": "string"},
                "authorization_model_id": {"type": "string"},
                "contextual_tuples": {"$ref": "#/components/schemas/FGAContextualTuples"},
            }
        }),
    );
    components.insert(
        "FGAListObjectsResponse".into(),
        json!({
            "type": "object",
            "required": ["objects"],
            "properties": {
                "objects": {
                    "type": "array",
                    "items": {"type": "string"}
                }
            }
        }),
    );
    components.insert(
        "FGAUserFilter".into(),
        json!({
            "type": "object",
            "required": ["type"],
            "properties": {
                "type": {"type": "string"},
                "relation": {"type": "string"},
            }
        }),
    );
    components.insert(
        "FGAStoreListUsersRequest".into(),
        json!({
            "type": "object",
            "required": ["object", "relation"],
            "properties": {
                "object": {"type": "string"},
                "relation": {"type": "string"},
                "user_filters": {
                    "type": "array",
                    "items": {"$ref": "#/components/schemas/FGAUserFilter"}
                },
                "authorization_model_id": {"type": "string"},
                "contextual_tuples": {"$ref": "#/components/schemas/FGAContextualTuples"},
            }
        }),
    );
    components.insert(
        "FGAStoreListUsersResponse".into(),
        json!({
            "type": "object",
            "properties": {
                "users": {
                    "type": "array",
                    "items": {"type": "string"}
                }
            }
        }),
    );
    components.insert(
        "FGATupleChangeRecord".into(),
        json!({
            "type": "object",
            "required": ["tuple_key", "operation", "timestamp"],
            "properties": {
                "tuple_key": {"$ref": "#/components/schemas/FGATupleKey"},
                "operation": {"type": "string"},
                "timestamp": {"type": "string"},
            }
        }),
    );
    components.insert(
        "FGAStoreReadChangesResponse".into(),
        json!({
            "type": "object",
            "properties": {
                "changes": {
                    "type": "array",
                    "items": {"$ref": "#/components/schemas/FGATupleChangeRecord"}
                },
                "continuation_token": {"type": "string"},
            }
        }),
    );
    components.insert(
        "FGATypeDefinition".into(),
        json!({
            "type": "object",
            "required": ["type"],
            "properties": {
                "type": {"type": "string"},
                "relations": {
                    "type": "object",
                    "additionalProperties": true,
                },
                "metadata": {
                    "anyOf": [
                        {"type": "object", "additionalProperties": true},
                        {"type": "array"},
                        {"type": "string"},
                        {"type": "number"},
                        {"type": "boolean"},
                        {"type": "null"}
                    ]
                }
            }
        }),
    );
    components.insert(
        "FGAAuthorizationModelWriteRequest".into(),
        json!({
            "type": "object",
            "required": ["schema_version"],
            "properties": {
                "schema_version": {"type": "string"},
                "type_definitions": {
                    "type": "array",
                    "items": {"$ref": "#/components/schemas/FGATypeDefinition"}
                },
                "conditions": {
                    "type": "object",
                    "additionalProperties": true,
                }
            }
        }),
    );
    components.insert(
        "FGAAuthorizationModelMetadata".into(),
        json!({
            "type": "object",
            "required": ["authorization_model_id", "schema_version", "type_definitions", "created_at"],
            "properties": {
                "authorization_model_id": {"type": "string"},
                "schema_version": {"type": "string"},
                "type_definitions": {
                    "type": "array",
                    "items": {"$ref": "#/components/schemas/FGATypeDefinition"}
                },
                "conditions": {
                    "type": "object",
                    "additionalProperties": true,
                },
                "created_at": {"type": "string"},
            }
        }),
    );
    components.insert(
        "FGAAuthorizationModelWriteResponse".into(),
        json!({
            "type": "object",
            "required": ["authorization_model_id"],
            "properties": {
                "authorization_model_id": {"type": "string"}
            }
        }),
    );
    components.insert(
        "FGAAuthorizationModelsListResponse".into(),
        json!({
            "type": "object",
            "properties": {
                "authorization_models": {
                    "type": "array",
                    "items": {"$ref": "#/components/schemas/FGAAuthorizationModelMetadata"}
                }
            }
        }),
    );
    components.insert(
        "FGAModelResponse".into(),
        json!({
            "type": "object",
            "required": ["schema_version", "types"],
            "properties": {
                "schema_version": {"type": "string"},
                "types": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "required": ["type", "relations"],
                        "properties": {
                            "type": {"type": "string"},
                            "relations": {
                                "type": "array",
                                "items": {"type": "string"}
                            }
                        }
                    }
                }
            }
        }),
    );
    components.insert(
        "FGAModelGraphResponse".into(),
        json!({
            "type": "object",
            "required": ["nodes", "edges"],
            "properties": {
                "nodes": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "required": ["id", "relations", "permissions"],
                        "properties": {
                            "id": {"type": "string"},
                            "relations": {
                                "type": "array",
                                "items": {"type": "string"}
                            },
                            "permissions": {
                                "type": "array",
                                "items": {"type": "string"}
                            }
                        }
                    }
                },
                "edges": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "required": ["from", "to", "relation", "kind"],
                        "properties": {
                            "from": {"type": "string"},
                            "to": {"type": "string"},
                            "relation": {"type": "string"},
                            "kind": {"type": "string"},
                        }
                    }
                }
            }
        }),
    );
    components.insert(
        "FGAStoreResponse".into(),
        json!({
            "type": "object",
            "required": ["store_id", "name", "instance_id"],
            "properties": {
                "store_id": {"type": "string"},
                "name": {"type": "string"},
                "instance_id": {"type": "string"},
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
        "/v1/fga/store": {
            "get": operation("authorization", "fgaDiscoverStore", "Discover the singleton OpenFGA store for this instance", json_response("FGAStoreResponse"))
        },
        "/v1/fga/check": {
            "post": operation_with_body("authorization", "fgaCheck", "Check authorization using the legacy alias", "FGACheckRequest", json_response("FGACheckResponse"))
        },
        "/v1/fga/tuples": {
            "get": operation_with_params("authorization", "fgaReadTuples", "Read relationship tuples using the legacy alias", json_response("FGAReadTuplesResponse"), vec![
                query_parameter("user", false, json!({"type": "string"})),
                query_parameter("relation", false, json!({"type": "string"})),
                query_parameter("object", false, json!({"type": "string"})),
            ]),
            "post": operation_with_body("authorization", "fgaWriteTuples", "Write relationship tuples using the legacy alias", "FGAWriteTuplesRequest", json_response("FGAWriteTuplesResponse")),
            "delete": operation_with_body("authorization", "fgaDeleteTuples", "Delete relationship tuples using the legacy alias", "FGAWriteTuplesRequest", json_response("FGADeleteTuplesResponse"))
        },
        "/v1/fga/list-objects": {
            "post": operation_with_body("authorization", "fgaListObjects", "List authorized objects using the legacy alias", "FGAListObjectsRequest", json_response("FGAListObjectsResponse"))
        },
        "/v1/fga/model": {
            "get": operation("authorization", "fgaGetModel", "Read the authorization model using the legacy alias", json_response("FGAModelResponse")),
            "post": operation_with_body("authorization", "fgaWriteModel", "Write the authorization model using the legacy alias", "FGAAuthorizationModelWriteRequest", json_response("FGAAuthorizationModelWriteResponse"))
        },
        "/v1/fga/model/graph": {
            "get": operation("authorization", "fgaModelGraph", "Read the authorization model graph using the legacy alias", json_response("FGAModelGraphResponse"))
        },
        "/v1/fga/expand": {
            "post": operation_with_body("authorization", "fgaExpand", "Expand the authorization tree using the legacy alias", "FGAExpandRequest", json_response("FGAExpandResponse"))
        },
        "/v1/fga/test": {
            "post": operation_with_body("authorization", "fgaBatchTest", "Batch test authorization assertions using the legacy alias", "FGABatchTestRequest", json_response("FGABatchTestResponse"))
        },
        "/v1/fga/stores/{store_id}/check": {
            "post": operation_with_body_and_params("authorization", "fgaStoreCheck", "Check authorization in the canonical store-scoped API", "FGAStoreCheckRequest", json_response("FGAStoreCheckResponse"), vec![
                path_parameter("store_id", json!({"type": "string"})),
            ])
        },
        "/v1/fga/stores/{store_id}/batch-check": {
            "post": operation_with_body_and_params("authorization", "fgaStoreBatchCheck", "Batch check authorization in the canonical store-scoped API", "FGABatchCheckRequest", json_response("FGAStoreBatchCheckResponse"), vec![
                path_parameter("store_id", json!({"type": "string"})),
            ])
        },
        "/v1/fga/stores/{store_id}/read": {
            "post": operation_with_body_and_params("authorization", "fgaStoreRead", "Read tuples in the canonical store-scoped API", "FGAStoreReadRequest", json_response("FGAStoreReadResponse"), vec![
                path_parameter("store_id", json!({"type": "string"})),
            ])
        },
        "/v1/fga/stores/{store_id}/write": {
            "post": operation_with_body_and_params("authorization", "fgaStoreWrite", "Write tuples in the canonical store-scoped API", "FGAStoreWriteRequest", json_response("EmptyObject"), vec![
                path_parameter("store_id", json!({"type": "string"})),
            ])
        },
        "/v1/fga/stores/{store_id}/expand": {
            "post": operation_with_body_and_params("authorization", "fgaStoreExpand", "Expand the authorization tree in the canonical store-scoped API", "FGAExpandRequest", json_response("FGAExpandResponse"), vec![
                path_parameter("store_id", json!({"type": "string"})),
            ])
        },
        "/v1/fga/stores/{store_id}/list-objects": {
            "post": operation_with_body_and_params("authorization", "fgaStoreListObjects", "List authorized objects in the canonical store-scoped API", "FGAListObjectsRequest", json_response("FGAListObjectsResponse"), vec![
                path_parameter("store_id", json!({"type": "string"})),
            ])
        },
        "/v1/fga/stores/{store_id}/list-users": {
            "post": operation_with_body_and_params("authorization", "fgaStoreListUsers", "List users in the canonical store-scoped API", "FGAStoreListUsersRequest", json_response("FGAStoreListUsersResponse"), vec![
                path_parameter("store_id", json!({"type": "string"})),
            ])
        },
        "/v1/fga/stores/{store_id}/changes": {
            "get": operation_with_params("authorization", "fgaStoreReadChanges", "Read tuple changes in the canonical store-scoped API", json_response("FGAStoreReadChangesResponse"), vec![
                path_parameter("store_id", json!({"type": "string"})),
                query_parameter("type", false, json!({"type": "string"})),
                query_parameter("page_size", false, json!({"type": "integer"})),
                query_parameter("continuation_token", false, json!({"type": "string"})),
            ])
        },
        "/v1/fga/stores/{store_id}/authorization-models": {
            "get": operation_with_params("authorization", "fgaStoreListAuthorizationModels", "List authorization models in the canonical store-scoped API", json_response("FGAAuthorizationModelsListResponse"), vec![
                path_parameter("store_id", json!({"type": "string"})),
            ]),
            "post": operation_with_body_and_params("authorization", "fgaStoreWriteAuthorizationModel", "Write an authorization model in the canonical store-scoped API", "FGAAuthorizationModelWriteRequest", json_response("FGAAuthorizationModelWriteResponse"), vec![
                path_parameter("store_id", json!({"type": "string"})),
            ])
        },
        "/v1/fga/stores/{store_id}/authorization-models/{model_id}": {
            "get": operation_with_params("authorization", "fgaStoreGetAuthorizationModel", "Fetch an authorization model in the canonical store-scoped API", json_response("FGAAuthorizationModelMetadata"), vec![
                path_parameter("store_id", json!({"type": "string"})),
                path_parameter("model_id", json!({"type": "string"})),
            ])
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

fn operation_with_params(
    tag: &str,
    operation_id: &str,
    summary: &str,
    success: Value,
    parameters: Vec<Value>,
) -> Value {
    let mut operation = operation(tag, operation_id, summary, success);
    if let Some(operation) = operation.as_object_mut() {
        operation.insert("parameters".into(), Value::Array(parameters));
    }
    operation
}

fn operation_with_body_and_params(
    tag: &str,
    operation_id: &str,
    summary: &str,
    body_schema: &str,
    success: Value,
    parameters: Vec<Value>,
) -> Value {
    let mut operation = operation_with_body(tag, operation_id, summary, body_schema, success);
    if let Some(operation) = operation.as_object_mut() {
        operation.insert("parameters".into(), Value::Array(parameters));
    }
    operation
}

fn path_parameter(name: &str, schema: Value) -> Value {
    json!({
        "in": "path",
        "name": name,
        "required": true,
        "schema": schema,
    })
}

fn query_parameter(name: &str, required: bool, schema: Value) -> Value {
    json!({
        "in": "query",
        "name": name,
        "required": required,
        "schema": schema,
    })
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
