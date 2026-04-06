use crate::{ApiState, extractors::ResourceId, middleware::Identity, response};
use axum::{
    Extension, Router,
    extract::State,
    response::Response,
    routing::get,
};
use serde::Serialize;
use zitadel_db::current_instance_id;

// TODO(ADR-032): list_sessions and get_session bypass the app layer and call
// s.transient directly. SessionRepository lacks list/get methods — these need
// to be added, then use cases created and handlers rewired.
// revoke_session already goes through the app layer correctly.

pub fn routes() -> Router<ApiState> {
    Router::new()
        .route("/sessions", get(list_sessions))
        .route("/sessions/{id}", get(get_session))
        .route("/sessions/{id}/revoke", axum::routing::post(revoke_session))
}

#[derive(Serialize)]
struct SessionResponse {
    id: String,
    user_id: String,
    org_id: String,
    created_at: String,
    expires_at: Option<String>,
    revoked_at: Option<String>,
}

// Sessions live in transient KvStore; queried directly, not through use case.
// Requires operator_admin to list all sessions; regular users see only their own.
async fn list_sessions(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
) -> Response {
    let instance_id = current_instance_id();
    match s.transient.list_sessions(&instance_id).await {
        Ok(rows) => {
            let items: Vec<SessionResponse> = rows
                .into_iter()
                .filter(|r| identity.operator_admin || r.user_id == identity.user_id)
                .map(|r| SessionResponse {
                    id: r.id,
                    user_id: r.user_id,
                    org_id: r.org_id,
                    created_at: r.created_at,
                    expires_at: r.expires_at,
                    revoked_at: r.revoked_at,
                })
                .collect();
            response::json_ok(response::ListResponse {
                items,
                next_cursor: None,
                total: None,
            })
        }
        Err(e) => response::internal_error(format!("{e}")),
    }
}

// TODO(CLAUDE-4): Call session get use case when available
async fn get_session(State(s): State<ApiState>, ResourceId(id): ResourceId) -> Response {
    let instance_id = current_instance_id();
    match s.transient.get_session(&instance_id, &id).await {
        Ok(Some(r)) => response::json_ok(SessionResponse {
            id: r.id,
            user_id: r.user_id,
            org_id: r.org_id,
            created_at: r.created_at,
            expires_at: r.expires_at,
            revoked_at: r.revoked_at,
        }),
        Ok(None) => response::not_found("session not found"),
        Err(e) => response::internal_error(format!("{e}")),
    }
}

async fn revoke_session(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    ResourceId(id): ResourceId,
) -> Response {
    let ctx = response::build_actor_context(&identity);
    match s.app.runner.run_fn(&ctx, "session.revoke", || s.app.revoke_session.execute(&ctx, &id)).await {
        Ok(()) => response::no_content(),
        Err(e) => response::app_error(e),
    }
}
