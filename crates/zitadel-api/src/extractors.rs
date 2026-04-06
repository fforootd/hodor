//! Custom path parameter extractors that work for both flat routes
//! (`/v1/resource/{id}`) and instance-nested routes
//! (`/v1/instances/{instanceId}/resource/{id}`).
//!
//! When product routes are nested under `/instances/{instanceId}`, axum
//! includes `instanceId` in the path parameters. The standard `Path<String>`
//! extractor rejects this because it expects exactly one parameter. These
//! extractors use `Path<HashMap<...>>` internally and pick the desired
//! parameter by name, ignoring any extra parameters like `instanceId`.

use std::collections::HashMap;

use axum::{
    extract::{FromRequestParts, Path},
    http::request::Parts,
    response::Response,
};

use crate::response::bad_request;

// ─── Generic helpers ────────────────────────────────────────

/// Extract a single path parameter by name from the request.
async fn extract_named<S: Send + Sync>(
    parts: &mut Parts,
    state: &S,
    name: &str,
) -> Result<String, Response> {
    let Path(params): Path<HashMap<String, String>> =
        Path::from_request_parts(parts, state)
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
    let Path(params): Path<HashMap<String, String>> =
        Path::from_request_parts(parts, state)
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
        let (group_id, user_id) =
            extract_named_pair(parts, state, "group_id", "user_id").await?;
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
        let (store_id, model_id) =
            extract_named_pair(parts, state, "store_id", "model_id").await?;
        Ok(Self {
            store_id,
            model_id,
        })
    }
}
