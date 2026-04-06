use crate::{ApiState, extractors::ResourceId, middleware::Identity, response};
use axum::{Extension, Router, extract::State, response::Response, routing::get};
use serde::Serialize;

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

async fn list_sessions(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
) -> Response {
    let ctx = response::build_actor_context(&identity);
    match s.app.list_sessions.execute(&ctx).await {
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
        Err(e) => response::app_error(e),
    }
}

async fn get_session(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    ResourceId(id): ResourceId,
) -> Response {
    let ctx = response::build_actor_context(&identity);
    match s.app.get_session.execute(&ctx, &id).await {
        Ok(r) => response::json_ok(SessionResponse {
            id: r.id,
            user_id: r.user_id,
            org_id: r.org_id,
            created_at: r.created_at,
            expires_at: r.expires_at,
            revoked_at: r.revoked_at,
        }),
        Err(e) => response::app_error(e),
    }
}

async fn revoke_session(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    ResourceId(id): ResourceId,
) -> Response {
    let ctx = response::build_actor_context(&identity);
    match s
        .app
        .runner
        .run(&ctx, "session.revoke", || {
            s.app.revoke_session.execute(&ctx, &id)
        })
        .await
    {
        Ok(()) => response::no_content(),
        Err(e) => response::app_error(e),
    }
}
