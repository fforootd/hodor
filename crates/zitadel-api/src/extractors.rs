//! Custom extractors for path parameters and validated JSON payloads.
//!
//! ## Path Extractors
//!
//! Work for both flat routes (`/v1/resource/{id}`) and instance-nested routes
//! (`/v1/instances/{instanceId}/resource/{id}`). They use `Path<HashMap<...>>`
//! internally and pick the desired parameter by name.
//!
//! ## ValidatedJson
//!
//! Validates request bodies against bundled JSON schemas before deserialization.
//! Requires a `SchemaType` extension on the route to specify which schema to
//! validate against.

use std::collections::HashMap;

use axum::{
    extract::{FromRequest, FromRequestParts, Path, Request},
    http::request::Parts,
    response::Response,
};
use serde::de::DeserializeOwned;

use crate::response::{self, bad_request};

// ─── Generic helpers ────────────────────────────────────────

/// Extract a single path parameter by name from the request.
async fn extract_named<S: Send + Sync>(
    parts: &mut Parts,
    state: &S,
    name: &str,
) -> Result<String, Response> {
    let Path(params): Path<HashMap<String, String>> = Path::from_request_parts(parts, state)
        .await
        .map_err(|e| bad_request(format!("{e}")))?;
    params
        .get(name)
        .cloned()
        .ok_or_else(|| bad_request(format!("missing path parameter: {name}")))
}

/// Extract two path parameters by name from the request.
async fn extract_named_pair<S: Send + Sync>(
    parts: &mut Parts,
    state: &S,
    name_a: &str,
    name_b: &str,
) -> Result<(String, String), Response> {
    let Path(params): Path<HashMap<String, String>> = Path::from_request_parts(parts, state)
        .await
        .map_err(|e| bad_request(format!("{e}")))?;
    let a = params
        .get(name_a)
        .cloned()
        .ok_or_else(|| bad_request(format!("missing path parameter: {name_a}")))?;
    let b = params
        .get(name_b)
        .cloned()
        .ok_or_else(|| bad_request(format!("missing path parameter: {name_b}")))?;
    Ok((a, b))
}

// ─── Typed extractors ───────────────────────────────────────

/// Extracts `{id}` — the most common single-resource path parameter.
pub struct ResourceId(pub String);

impl<S: Send + Sync> FromRequestParts<S> for ResourceId {
    type Rejection = Response;
    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        extract_named(parts, state, "id").await.map(ResourceId)
    }
}

/// Extracts `{org_id}` for org-scoped sub-resource routes.
pub struct OrgId(pub String);

impl<S: Send + Sync> FromRequestParts<S> for OrgId {
    type Rejection = Response;
    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        extract_named(parts, state, "org_id").await.map(OrgId)
    }
}

/// Extracts `{group_id}` for group-scoped sub-resource routes.
pub struct GroupId(pub String);

impl<S: Send + Sync> FromRequestParts<S> for GroupId {
    type Rejection = Response;
    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        extract_named(parts, state, "group_id").await.map(GroupId)
    }
}

/// Extracts `{store_id}` for FGA store-scoped routes.
pub struct StoreId(pub String);

impl<S: Send + Sync> FromRequestParts<S> for StoreId {
    type Rejection = Response;
    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        extract_named(parts, state, "store_id").await.map(StoreId)
    }
}

/// Extracts `{org_id}` + `{user_id}` for org member removal.
pub struct OrgMemberPath {
    pub org_id: String,
    pub user_id: String,
}

impl<S: Send + Sync> FromRequestParts<S> for OrgMemberPath {
    type Rejection = Response;
    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let (org_id, user_id) = extract_named_pair(parts, state, "org_id", "user_id").await?;
        Ok(Self { org_id, user_id })
    }
}

/// Extracts `{group_id}` + `{user_id}` for group member removal.
pub struct GroupMemberPath {
    pub group_id: String,
    pub user_id: String,
}

impl<S: Send + Sync> FromRequestParts<S> for GroupMemberPath {
    type Rejection = Response;
    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let (group_id, user_id) = extract_named_pair(parts, state, "group_id", "user_id").await?;
        Ok(Self { group_id, user_id })
    }
}

/// Extracts `{store_id}` + `{model_id}` for FGA authorization model detail.
pub struct StoreModelPath {
    pub store_id: String,
    pub model_id: String,
}

impl<S: Send + Sync> FromRequestParts<S> for StoreModelPath {
    type Rejection = Response;
    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let (store_id, model_id) = extract_named_pair(parts, state, "store_id", "model_id").await?;
        Ok(Self { store_id, model_id })
    }
}

// ─── Schema-validated JSON ─────────────────────────────────

/// Route-level extension that declares which JSON schema to validate against.
///
/// Add this as a layer on routes that accept JSON bodies:
/// ```ignore
/// .route("/users", post(create_user).layer(Extension(SchemaType("human_user"))))
/// ```
#[derive(Clone, Debug)]
pub struct SchemaType(pub &'static str);

/// JSON body extractor that validates against a bundled JSON schema.
///
/// If a `SchemaType` extension is present on the route, the payload is
/// validated against the corresponding schema before deserialization.
/// On validation failure, returns 422 with structured error details.
///
/// If no `SchemaType` extension is set, behaves like `Json<T>`.
pub struct ValidatedJson<T>(pub T);

impl<S, T> FromRequest<S> for ValidatedJson<T>
where
    S: Send + Sync,
    T: DeserializeOwned,
{
    type Rejection = Response;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        // Extract schema type from extensions (set by route layer).
        let schema_type = req
            .extensions()
            .get::<SchemaType>()
            .map(|s| s.0);

        let bytes = axum::body::Bytes::from_request(req, state)
            .await
            .map_err(|e| bad_request(format!("failed to read body: {e}")))?;

        let value: serde_json::Value = serde_json::from_slice(&bytes)
            .map_err(|e| bad_request(format!("invalid JSON: {e}")))?;

        // Validate against schema if one is specified.
        if let Some(schema_type) = schema_type {
            if let Err(errors) = zitadel_schema::validator::SchemaValidator::global()
                .validate(schema_type, &value)
            {
                let details: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
                return Err(response::error(
                    axum::http::StatusCode::UNPROCESSABLE_ENTITY,
                    format!("schema validation failed: {}", details.join("; ")),
                ));
            }
        }

        let inner: T = serde_json::from_value(value)
            .map_err(|e| bad_request(format!("deserialization failed: {e}")))?;

        Ok(ValidatedJson(inner))
    }
}
