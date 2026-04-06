use axum::{
    Extension, Json, Router,
    extract::{Path, Query, State},
    response::Response,
    routing::get,
};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use time::{Duration, OffsetDateTime, format_description::well_known::Rfc3339};

use crate::{
    ApiState,
    instances::{
        reconcile_after_mutation, require_instance_relation, require_parent_management,
    },
    middleware::Identity,
    response,
};

pub fn routes() -> Router<ApiState> {
    Router::new()
        .route("/support/grants", get(list_grants).post(create_grant))
        .route("/support/grants/{grant_id}", axum::routing::delete(revoke_grant))
}

#[derive(Deserialize)]
struct CreateGrantRequest {
    #[serde(default)]
    instance_id: String,
    #[serde(default)]
    role: String,
    #[serde(default)]
    reason: String,
    duration_secs: Option<i64>,
    #[serde(default)]
    principal_ref: String,
}

#[derive(Deserialize, Default)]
struct ListGrantQuery {
    instance_id: Option<String>,
    principal_ref: Option<String>,
    #[serde(default)]
    include_revoked: bool,
}

#[derive(Serialize)]
struct GrantResponse {
    grant_id: String,
    instance_id: String,
    principal_ref: String,
    role_key: String,
    source_kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    expires_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    revoked_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    access_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    issuer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    audience: Option<String>,
}

impl GrantResponse {
    fn from_record(record: zitadel_app::repo::RoleAssignmentRecord) -> Self {
        Self {
            grant_id: record.assignment_id,
            instance_id: record.scope_id,
            principal_ref: record.principal_ref,
            role_key: record.role_key,
            source_kind: record.source_kind,
            reason: record.reason,
            expires_at: record.expires_at,
            revoked_at: record.revoked_at,
            access_token: None,
            issuer: None,
            audience: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SupportGrantClaims {
    pub iss: String,
    pub aud: String,
    pub sub: String,
    pub exp: usize,
    pub iat: usize,
    pub grant_id: String,
    pub target_instance_id: String,
    pub role_key: String,
    pub principal_ref: String,
    pub issuer_instance_id: String,
    pub reason: Option<String>,
}

async fn create_grant(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    Json(req): Json<CreateGrantRequest>,
) -> Response {
    let access = match require_parent_management(&s, &identity).await {
        Ok(access) => access,
        Err(response) => return response,
    };
    if req.instance_id.is_empty() {
        return response::bad_request("instance_id is required");
    }
    if !access.operator_admin {
        match require_instance_relation(
            &s,
            &access,
            &identity.principal_ref,
            "admin",
            &req.instance_id,
        )
        .await
        {
            Ok(true) => {}
            Ok(false) => return response::not_found("instance not found"),
            Err(response) => return response,
        }
    }

    let duration_secs = req.duration_secs.unwrap_or(3600);
    if duration_secs <= 0 {
        return response::bad_request("duration_secs must be greater than 0");
    }
    let expires_at = match OffsetDateTime::now_utc()
        .checked_add(Duration::seconds(duration_secs))
        .and_then(|value| value.format(&Rfc3339).ok())
    {
        Some(expires_at) => Some(expires_at),
        None => return response::bad_request("invalid duration_secs"),
    };

    let ctx = response::build_actor_context(&identity);
    let created = match s
        .app
        .runner
        .run(&ctx, "support.grant.create", || {
            s.app.create_support_grant.execute(
                &ctx,
                zitadel_app::support::CreateSupportGrantCommand {
                    target_instance_id: req.instance_id.clone(),
                    principal_ref: (!req.principal_ref.is_empty()).then(|| req.principal_ref.clone()),
                    role: req.role.clone(),
                    reason: (!req.reason.is_empty()).then(|| req.reason.clone()),
                    expires_at,
                },
            )
        })
        .await
    {
        Ok(grant) => grant,
        Err(error) => return response::app_error(error),
    };

    if let Err(response) = reconcile_after_mutation(&s, &access.parent_instance_id).await {
        return response;
    }

    let mut body = GrantResponse::from_record(created.clone());
    if created.source_kind == "support_grant_federated" {
        let audience = support_grant_audience(&created.scope_id);
        let issuer = s.oidc.provider.issuer().into_owned();
        match issue_support_grant_token(
            &s,
            &created,
            &issuer,
            &audience,
            identity.user_id.as_str(),
        ) {
            Ok(token) => {
                body.access_token = Some(token);
                body.issuer = Some(issuer);
                body.audience = Some(audience);
            }
            Err(error) => return response::internal(error),
        }
    }

    response::json_created(body)
}

async fn list_grants(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    Query(query): Query<ListGrantQuery>,
) -> Response {
    let access = match require_parent_management(&s, &identity).await {
        Ok(access) => access,
        Err(response) => return response,
    };
    if let Some(instance_id) = query.instance_id.as_deref()
        && !access.operator_admin
    {
        match require_instance_relation(
            &s,
            &access,
            &identity.principal_ref,
            "viewer",
            instance_id,
        )
        .await
        {
            Ok(true) => {}
            Ok(false) => return response::not_found("instance not found"),
            Err(response) => return response,
        }
    }

    let ctx = response::build_actor_context(&identity);
    let filter = zitadel_app::support::ListSupportGrantFilter {
        target_instance_id: query.instance_id.clone(),
        principal_ref: query.principal_ref.clone(),
        include_revoked: query.include_revoked,
    };
    match s
        .app
        .runner
        .run(&ctx, "support.grant.list", || {
            s.app.list_support_grants.execute(&ctx, &filter)
        })
        .await
    {
        Ok(items) => response::json_ok(response::ListResponse {
            total: Some(items.len() as i64),
            next_cursor: None,
            items: items.into_iter().map(GrantResponse::from_record).collect::<Vec<_>>(),
        }),
        Err(error) => response::app_error(error),
    }
}

async fn revoke_grant(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    Path(grant_id): Path<String>,
) -> Response {
    let access = match require_parent_management(&s, &identity).await {
        Ok(access) => access,
        Err(response) => return response,
    };
    let Some(existing) = (match s.app.repos.authorization.get_role_assignment(&grant_id).await {
        Ok(record) => record,
        Err(error) => return response::internal(error),
    }) else {
        return response::not_found("support grant not found");
    };
    if !zitadel_app::support::is_support_grant_assignment(&existing) {
        return response::not_found("support grant not found");
    }
    if !access.operator_admin {
        match require_instance_relation(
            &s,
            &access,
            &identity.principal_ref,
            "admin",
            &existing.scope_id,
        )
        .await
        {
            Ok(true) => {}
            Ok(false) => return response::not_found("support grant not found"),
            Err(response) => return response,
        }
    }

    let ctx = response::build_actor_context(&identity);
    match s
        .app
        .runner
        .run(&ctx, "support.grant.revoke", || {
            s.app.revoke_support_grant.execute(&ctx, &grant_id)
        })
        .await
    {
        Ok(_) => {
            if let Err(response) = reconcile_after_mutation(&s, &access.parent_instance_id).await {
                return response;
            }
            response::no_content()
        }
        Err(error) => response::app_error(error),
    }
}

pub(crate) fn issue_support_grant_token(
    state: &ApiState,
    grant: &zitadel_app::repo::RoleAssignmentRecord,
    issuer: &str,
    audience: &str,
    subject: &str,
) -> anyhow::Result<String> {
    let now = OffsetDateTime::now_utc();
    let exp = grant
        .expires_at
        .as_deref()
        .and_then(parse_rfc3339_timestamp)
        .unwrap_or_else(|| now + Duration::hours(1));
    let claims = SupportGrantClaims {
        iss: issuer.to_string(),
        aud: audience.to_string(),
        sub: subject.to_string(),
        exp: exp.unix_timestamp().max(0) as usize,
        iat: now.unix_timestamp().max(0) as usize,
        grant_id: grant.assignment_id.clone(),
        target_instance_id: grant.scope_id.clone(),
        role_key: grant.role_key.clone(),
        principal_ref: grant.principal_ref.clone(),
        issuer_instance_id: grant
            .origin_instance_id
            .clone()
            .unwrap_or_else(|| state.oidc.provider.issuer().into_owned()),
        reason: grant.reason.clone(),
    };
    Ok(jsonwebtoken::encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(state.support_grant_secret.as_bytes()),
    )?)
}

pub(crate) fn decode_support_grant_token(
    state: &ApiState,
    token: &str,
) -> anyhow::Result<SupportGrantClaims> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_aud = false;
    validation.validate_exp = true;
    Ok(jsonwebtoken::decode::<SupportGrantClaims>(
        token,
        &DecodingKey::from_secret(state.support_grant_secret.as_bytes()),
        &validation,
    )?
    .claims)
}

pub(crate) fn support_grant_audience(instance_id: &str) -> String {
    format!("instance:{instance_id}")
}

pub(crate) fn parse_rfc3339_timestamp(value: &str) -> Option<OffsetDateTime> {
    OffsetDateTime::parse(value, &Rfc3339).ok()
}
