use crate::{ApiState, middleware::Identity, response};
use axum::{
    Extension, Json, Router,
    extract::{Path, Query, State},
    response::Response,
    routing::get,
};
use serde::{Deserialize, Serialize};
use zitadel_db::current_instance_id;
use zitadel_storage::AnalyticsQuery;

pub fn routes() -> Router<ApiState> {
    Router::new()
        .route("/account/profile", get(get_profile).patch(update_profile))
        .route("/account/sessions", get(list_own_sessions))
        .route(
            "/account/sessions/{id}/revoke",
            axum::routing::post(revoke_own_session),
        )
        .route(
            "/account/sessions/revoke-others",
            axum::routing::post(revoke_other_sessions),
        )
        .route("/account/activity", get(list_own_activity))
}

// ─── Profile ───

#[derive(Serialize)]
struct ProfileResponse {
    identity: IdentityProfile,
    field_permissions: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    schema: Option<serde_json::Value>,
}

#[derive(Serialize)]
struct IdentityProfile {
    id: String,
    identifier: String,
    display_name: String,
    state: String,
    #[serde(skip_serializing_if = "serde_json::Value::is_null")]
    profile: serde_json::Value,
}

async fn get_profile(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
) -> Response {
    let ctx = response::build_actor_context(&identity);
    match s.app.get_user.execute(&ctx, &identity.user_id).await {
        Ok(user) => {
            let profile = user.metadata.get("profile").cloned().unwrap_or_default();
            response::json_ok(ProfileResponse {
                identity: IdentityProfile {
                    id: user.id,
                    identifier: user.identifier,
                    display_name: user.display_name,
                    state: user.state,
                    profile,
                },
                field_permissions: serde_json::json!({}),
                schema: None,
            })
        }
        Err(e) => response::app_error(e),
    }
}

#[derive(Deserialize)]
struct UpdateProfileRequest {
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    profile: Option<serde_json::Value>,
}

async fn update_profile(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    Json(req): Json<UpdateProfileRequest>,
) -> Response {
    let ctx = response::build_actor_context(&identity);

    // Build metadata update with profile nested inside if provided.
    let metadata = req.profile.map(|p| serde_json::json!({ "profile": p }));

    let has_display_name = req.display_name.is_some();
    let cmd = zitadel_app::users::UpdateUserCommand {
        user_id: identity.user_id,
        display_name: req.display_name,
        metadata,
    };
    match s.app.update_user.execute(&ctx, cmd).await {
        Ok(_) => {
            let mut fields_changed = Vec::new();
            if has_display_name {
                fields_changed.push("display_name");
            }
            response::json_ok(serde_json::json!({
                "status": "updated",
                "fields_changed": fields_changed,
            }))
        }
        Err(e) => response::app_error(e),
    }
}

// ─── Sessions (routed through app layer) ───

#[derive(Serialize)]
struct OwnSessionResponse {
    id: String,
    user_agent: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    ip_address: String,
    current: bool,
    created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    expires_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    revoked_at: Option<String>,
}

async fn list_own_sessions(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
) -> Response {
    let ctx = response::build_actor_context(&identity);
    match s.app.list_sessions.execute_self(&ctx).await {
        Ok(rows) => {
            let sessions: Vec<OwnSessionResponse> = rows
                .into_iter()
                .filter(|r| r.revoked_at.is_none())
                .map(|r| {
                    let current = r.id == identity.session_id;
                    OwnSessionResponse {
                        id: r.id,
                        user_agent: r.user_agent,
                        ip_address: r.ip_address,
                        current,
                        created_at: r.created_at,
                        expires_at: r.expires_at,
                        revoked_at: r.revoked_at,
                    }
                })
                .collect();
            let count = sessions.len();
            response::json_ok(serde_json::json!({
                "count": count,
                "sessions": sessions,
            }))
        }
        Err(e) => response::app_error(e),
    }
}

async fn revoke_own_session(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    Path(id): Path<String>,
) -> Response {
    let ctx = response::build_actor_context(&identity);
    match s.app.get_session.execute_self(&ctx, &id).await {
        Ok(_) => {}
        Err(e) => return response::app_error(e),
    }
    match s
        .app
        .runner
        .run(&ctx, "session.revoke", || {
            s.app.revoke_session.execute_self(&ctx, &id)
        })
        .await
    {
        Ok(()) => response::no_content(),
        Err(e) => response::app_error(e),
    }
}

async fn revoke_other_sessions(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
) -> Response {
    let ctx = response::build_actor_context(&identity);
    match s.app.list_sessions.execute_self(&ctx).await {
        Ok(rows) => {
            let mut revoked = 0u32;
            for session in rows {
                if session.id != identity.session_id
                    && session.revoked_at.is_none()
                    && s.app
                        .revoke_session
                        .execute_self(&ctx, &session.id)
                        .await
                        .is_ok()
                {
                    revoked += 1;
                }
            }
            response::json_ok(serde_json::json!({ "revoked": revoked }))
        }
        Err(e) => response::app_error(e),
    }
}

// ─── Activity ───
// Activity listing queries analytics storage directly.
// This is a read-only analytics query and is kept outside the use-case layer
// intentionally — analytics is a separate storage role (ADR-010).

#[derive(Deserialize)]
struct ActivityParams {
    #[serde(default = "default_activity_limit")]
    limit: i64,
}
fn default_activity_limit() -> i64 {
    10
}

async fn list_own_activity(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    Query(p): Query<ActivityParams>,
) -> Response {
    let instance_id = current_instance_id();
    let limit = p.limit.clamp(1, 50);
    let sql = format!(
        "SELECT id, event_type, created_at FROM events \
         WHERE instance_id = $1 AND actor_id = $2 \
         ORDER BY created_at DESC LIMIT {limit}"
    );

    match s
        .analytics
        .query(&AnalyticsQuery {
            sql,
            params: vec![instance_id.to_string(), identity.user_id],
            limit: Some(limit),
        })
        .await
    {
        Ok(result) => {
            if let Some(error) = result.error {
                return response::internal(error);
            }
            let events: Vec<serde_json::Value> = result
                .rows
                .iter()
                .map(|row| {
                    let id = row.first().and_then(|v| v.as_str()).unwrap_or_default();
                    let event_type = row.get(1).and_then(|v| v.as_str()).unwrap_or_default();
                    let created_at = row.get(2).and_then(|v| v.as_str()).unwrap_or_default();
                    serde_json::json!({
                        "id": id,
                        "event_type": event_type,
                        "created_at": created_at,
                    })
                })
                .collect();
            let count = events.len();
            response::json_ok(serde_json::json!({
                "count": count,
                "events": events,
            }))
        }
        Err(e) => response::internal(e),
    }
}
