use crate::{ApiState, middleware::Identity, response};
use axum::{
    Extension, Router,
    extract::{Path, State},
    response::Response,
    routing::get,
};
use serde::Serialize;
use zitadel_db::current_instance_id;

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

// TODO(CLAUDE-4): Call session list use case when available
async fn list_sessions(State(s): State<ApiState>) -> Response {
    let instance_id = current_instance_id();
    match s.transient.list_sessions(&instance_id).await {
        Ok(rows) => {
            let items: Vec<SessionResponse> = rows
                .into_iter()
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
async fn get_session(State(s): State<ApiState>, Path(id): Path<String>) -> Response {
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
    Path(id): Path<String>,
) -> Response {
    let ctx = response::build_actor_context(&identity);
    match s.app.revoke_session.execute(&ctx, &id).await {
        Ok(()) => response::no_content(),
        Err(e) => response::app_error(e),
    }
}
