use crate::{ApiState, response};
use axum::{
    Router,
    extract::{Path, State},
    response::Response,
    routing::get,
};
use serde::Serialize;
use zitadel_db::DEFAULT_INSTANCE_ID;

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

async fn list_sessions(State(s): State<ApiState>) -> Response {
    match s.transient.list_sessions(DEFAULT_INSTANCE_ID).await {
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

async fn get_session(State(s): State<ApiState>, Path(id): Path<String>) -> Response {
    match s.transient.get_session(DEFAULT_INSTANCE_ID, &id).await {
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

async fn revoke_session(State(s): State<ApiState>, Path(id): Path<String>) -> Response {
    match s.transient.revoke_session(DEFAULT_INSTANCE_ID, &id).await {
        Ok(false) => response::not_found("session not found"),
        Ok(true) => response::no_content(),
        Err(e) => response::internal_error(format!("{e}")),
    }
}
