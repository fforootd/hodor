use std::collections::{HashMap, HashSet};

use anyhow::Context;
use serde_json::{Map, Value, json};

use crate::dto::*;
use crate::error::FgaError;
use crate::evaluation::*;
use crate::SCHEMA_VERSION_1_1;

pub(crate) fn parse_relation_metadata(
    metadata: Option<&Value>,
) -> Result<HashMap<String, Vec<AllowedDirectUser>>, FgaError> {
    let Some(metadata) = metadata else {
        return Ok(HashMap::new());
    };
    let Some(relations) = metadata.get("relations").and_then(Value::as_object) else {
        return Ok(HashMap::new());
    };
    let mut parsed = HashMap::new();
    for (relation, metadata) in relations {
        let mut allowed = Vec::new();
        if let Some(types) = metadata
            .get("directly_related_user_types")
            .and_then(Value::as_array)
        {
            for type_def in types {
                let Some(object) = type_def.as_object() else {
                    continue;
                };
                let user_type = object
                    .get("type")
                    .and_then(Value::as_str)
                    .ok_or_else(|| FgaError::BadRequest("metadata type must be a string".into()))?;
                if object.get("condition").is_some() {
                    return Err(FgaError::Unsupported(
                        "conditional relation metadata is not supported".into(),
                    ));
                }
                allowed.push(AllowedDirectUser {
                    user_type: user_type.to_string(),
                    relation: object
                        .get("relation")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    wildcard: object.get("wildcard").is_some(),
                });
            }
        }
        parsed.insert(relation.clone(), allowed);
    }
    Ok(parsed)
}

pub(crate) fn parse_relation_expr(value: &Value) -> Result<RelationExpr, FgaError> {
    let Some(object) = value.as_object() else {
        return Err(FgaError::BadRequest(
            "relation definition must be an object".into(),
        ));
    };
    if object.contains_key("this") {
        return Ok(RelationExpr::This);
    }
    if let Some(computed) = object.get("computedUserset") {
        let relation = computed
            .get("relation")
            .and_then(Value::as_str)
            .ok_or_else(|| FgaError::BadRequest("computedUserset.relation is required".into()))?;
        return Ok(RelationExpr::ComputedUserset {
            relation: relation.to_string(),
        });
    }
    if let Some(ttu) = object.get("tupleToUserset") {
        let tupleset = ttu
            .get("tupleset")
            .and_then(Value::as_object)
            .and_then(|value| value.get("relation"))
            .and_then(Value::as_str)
            .ok_or_else(|| {
                FgaError::BadRequest("tupleToUserset.tupleset.relation is required".into())
            })?;
        let computed_userset = ttu
            .get("computedUserset")
            .and_then(Value::as_object)
            .and_then(|value| value.get("relation"))
            .and_then(Value::as_str)
            .ok_or_else(|| {
                FgaError::BadRequest("tupleToUserset.computedUserset.relation is required".into())
            })?;
        return Ok(RelationExpr::TupleToUserset {
            tupleset: tupleset.to_string(),
            computed_userset: computed_userset.to_string(),
        });
    }
    if let Some(union) = object.get("union") {
        let children = union
            .get("child")
            .and_then(Value::as_array)
            .ok_or_else(|| FgaError::BadRequest("union.child must be an array".into()))?
            .iter()
            .map(parse_relation_expr)
            .collect::<Result<Vec<_>, _>>()?;
        return Ok(RelationExpr::Union { children });
    }
    if let Some(intersection) = object.get("intersection") {
        let children = intersection
            .get("child")
            .and_then(Value::as_array)
            .ok_or_else(|| FgaError::BadRequest("intersection.child must be an array".into()))?
            .iter()
            .map(parse_relation_expr)
            .collect::<Result<Vec<_>, _>>()?;
        return Ok(RelationExpr::Intersection { children });
    }
    if let Some(difference) = object.get("difference") {
        let base = difference
            .get("base")
            .ok_or_else(|| FgaError::BadRequest("difference.base is required".into()))?;
        let subtract = difference
            .get("subtract")
            .ok_or_else(|| FgaError::BadRequest("difference.subtract is required".into()))?;
        return Ok(RelationExpr::Difference {
            base: Box::new(parse_relation_expr(base)?),
            subtract: Box::new(parse_relation_expr(subtract)?),
        });
    }
    Err(FgaError::BadRequest(
        "unsupported userset rewrite in relation definition".into(),
    ))
}

pub fn core_authorization_model() -> AuthorizationModelWriteRequest {
    AuthorizationModelWriteRequest {
        schema_version: SCHEMA_VERSION_1_1.into(),
        type_definitions: vec![
            TypeDefinition {
                type_name: "user".into(),
                relations: Map::new(),
                metadata: Some(json!({ "relations": {} })),
            },
            TypeDefinition {
                type_name: "instance".into(),
                relations: Map::from_iter([
                    ("owner".into(), json!({ "this": {} })),
                    (
                        "admin".into(),
                        json!({
                            "union": {
                                "child": [
                                    { "this": {} },
                                    { "computedUserset": { "relation": "owner" } }
                                ]
                            }
                        }),
                    ),
                    (
                        "viewer".into(),
                        json!({
                            "union": {
                                "child": [
                                    { "this": {} },
                                    { "computedUserset": { "relation": "admin" } }
                                ]
                            }
                        }),
                    ),
                    ("parent".into(), json!({ "this": {} })),
                ]),
                metadata: Some(json!({
                    "relations": {
                        "owner": {
                            "directly_related_user_types": [
                                { "type": "user" },
                                { "type": "org", "relation": "owner" }
                            ]
                        },
                        "admin": {
                            "directly_related_user_types": [
                                { "type": "user" },
                                { "type": "org", "relation": "admin" }
                            ]
                        },
                        "viewer": {
                            "directly_related_user_types": [
                                { "type": "user" },
                                { "type": "org", "relation": "viewer" }
                            ]
                        },
                        "parent": {
                            "directly_related_user_types": [
                                { "type": "instance" }
                            ]
                        }
                    }
                })),
            },
            TypeDefinition {
                type_name: "org".into(),
                relations: Map::from_iter([
                    ("owner".into(), json!({ "this": {} })),
                    (
                        "admin".into(),
                        json!({
                            "union": {
                                "child": [
                                    { "this": {} },
                                    { "computedUserset": { "relation": "owner" } }
                                ]
                            }
                        }),
                    ),
                    (
                        "member".into(),
                        json!({
                            "union": {
                                "child": [
                                    { "this": {} },
                                    { "computedUserset": { "relation": "admin" } }
                                ]
                            }
                        }),
                    ),
                    (
                        "viewer".into(),
                        json!({
                            "union": {
                                "child": [
                                    { "this": {} },
                                    { "computedUserset": { "relation": "member" } }
                                ]
                            }
                        }),
                    ),
                ]),
                metadata: Some(json!({
                    "relations": {
                        "owner": { "directly_related_user_types": [{ "type": "user" }] },
                        "admin": { "directly_related_user_types": [{ "type": "user" }] },
                        "member": { "directly_related_user_types": [{ "type": "user" }] },
                        "viewer": { "directly_related_user_types": [{ "type": "user" }] }
                    }
                })),
            },
            simple_direct_type("group", &["member", "admin"]),
            simple_direct_type("project", &["owner", "admin", "member"]),
            simple_direct_type("app", &["admin", "viewer"]),
            simple_direct_type("settings", &["admin", "viewer"]),
            simple_direct_type("session", &["owner"]),
        ],
        conditions: Map::new(),
    }
}

fn simple_direct_type(type_name: &str, relations: &[&str]) -> TypeDefinition {
    let relation_map = relations
        .iter()
        .map(|relation| (relation.to_string(), json!({ "this": {} })))
        .collect::<Map<String, Value>>();
    let metadata_relations = relations
        .iter()
        .map(|relation| {
            (
                relation.to_string(),
                json!({
                    "directly_related_user_types": [
                        { "type": "user" }
                    ]
                }),
            )
        })
        .collect::<Map<String, Value>>();
    TypeDefinition {
        type_name: type_name.into(),
        relations: relation_map,
        metadata: Some(json!({ "relations": metadata_relations })),
    }
}

pub(crate) fn rebuild_model_from_fragments(
    custom_model: &str,
    module_fragments: &str,
) -> Result<AuthorizationModelWriteRequest, FgaError> {
    let mut rebuilt = core_authorization_model();
    let modules: Vec<AuthorizationModelWriteRequest> =
        serde_json::from_str(module_fragments).context("parse module fragments")?;
    for fragment in modules {
        merge_model_fragment(&mut rebuilt, fragment)?;
    }
    let custom = parse_model_fragment(custom_model)?;
    merge_model_fragment(&mut rebuilt, custom)?;
    Ok(rebuilt)
}

fn parse_model_fragment(raw: &str) -> Result<AuthorizationModelWriteRequest, FgaError> {
    let value: Value = serde_json::from_str(raw).context("parse model fragment value")?;
    if value.as_object().is_some_and(|object| object.is_empty()) {
        return Ok(AuthorizationModelWriteRequest {
            schema_version: SCHEMA_VERSION_1_1.into(),
            type_definitions: Vec::new(),
            conditions: Map::new(),
        });
    }
    serde_json::from_value(value)
        .context("parse model fragment")
        .map_err(Into::into)
}

fn merge_model_fragment(
    target: &mut AuthorizationModelWriteRequest,
    fragment: AuthorizationModelWriteRequest,
) -> Result<(), FgaError> {
    if fragment.schema_version != target.schema_version {
        return Err(FgaError::BadRequest(format!(
            "fragment schema_version {} does not match core schema_version {}",
            fragment.schema_version, target.schema_version
        )));
    }
    if !fragment.conditions.is_empty() {
        return Err(FgaError::Unsupported(
            "conditions are not supported by the embedded v1 server".into(),
        ));
    }
    target.type_definitions.extend(fragment.type_definitions);
    Ok(())
}

pub(crate) fn tuple_identity(tuple: &TupleKey) -> String {
    format!("{}|{}|{}", tuple.user, tuple.relation, tuple.object)
}

pub(crate) fn is_managed_root_tuple(tuple: &TupleKey) -> bool {
    match tuple.object.split_once(':') {
        Some(("org", _)) => matches!(
            tuple.relation.as_str(),
            "owner" | "admin" | "member" | "viewer"
        ),
        Some(("instance", _)) => matches!(
            tuple.relation.as_str(),
            "parent" | "owner" | "admin" | "viewer"
        ),
        _ => false,
    }
}

pub(crate) fn validate_sealed_core(model: &CompiledModel) -> Result<(), FgaError> {
    let core = core_authorization_model();
    let core_compiled = CompiledModel::from_request(&core)?;
    for (type_name, expected) in core_compiled.raw_types {
        let actual = model
            .raw_types
            .get(&type_name)
            .ok_or_else(|| FgaError::BadRequest(format!("sealed type {type_name} is missing")))?;
        if actual != &expected {
            return Err(FgaError::BadRequest(format!(
                "sealed type {type_name} cannot be modified"
            )));
        }
    }
    Ok(())
}

pub(crate) fn extract_custom_fragment(request: &AuthorizationModelWriteRequest) -> Value {
    let sealed: HashSet<String> = core_authorization_model()
        .type_definitions
        .into_iter()
        .map(|type_def| type_def.type_name)
        .collect();
    json!({
        "schema_version": request.schema_version,
        "type_definitions": request
            .type_definitions
            .iter()
            .filter(|type_def| !sealed.contains(&type_def.type_name))
            .collect::<Vec<_>>(),
        "conditions": request.conditions,
    })
}
