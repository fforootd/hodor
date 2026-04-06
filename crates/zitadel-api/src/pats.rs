use crate::{ApiState, extractors::ResourceId, middleware::Identity, response};
use axum::{
    Extension, Json, Router,
    extract::State,
    response::Response,
    routing::{delete, get},
};
use serde::{Deserialize, Serialize};
use zitadel_app::pats::{CreatePatCommand, CreatePatResult};
use zitadel_app::repo::PatRecord;

pub fn routes() -> Router<ApiState> {
    Router::new()
        .route("/pats", get(list_pats).post(create_pat))
        .route("/pats/{id}", delete(revoke_pat))
}

#[derive(Deserialize)]
pub struct CreatePatRequest {
    pub user_id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub scopes: Vec<String>,
}

#[derive(Serialize)]
pub struct PatResponse {
    pub id: String,
    pub user_id: String,
    pub name: String,
    pub token: String,
    pub created_at: String,
}

impl From<CreatePatResult> for PatResponse {
    fn from(r: CreatePatResult) -> Self {
        Self {
            id: r.pat_id,
            user_id: String::new(),
            name: String::new(),
            token: r.token,
            created_at: String::new(),
        }
    }
}

async fn create_pat(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    Json(req): Json<CreatePatRequest>,
) -> Response {
    let ctx = response::build_actor_context(&identity);
    let cmd = CreatePatCommand {
        user_id: req.user_id.clone(),
        name: req.name.clone(),
        scopes: req.scopes,
    };
    match s.app.runner.run(&ctx, "pat.create", || s.app.create_pat.execute(&ctx, cmd)).await {
        Ok(result) => response::json_created(PatResponse {
            id: result.pat_id,
            user_id: req.user_id,
            name: req.name,
            token: result.token,
            created_at: String::new(),
        }),
        Err(e) => response::app_error(e),
    }
}

async fn list_pats(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
) -> Response {
    let ctx = response::build_actor_context(&identity);
    match s.app.runner.run(&ctx, "pat.list", || s.app.list_pats.execute(&ctx, &identity.user_id)).await {
        Ok(rows) => {
            let items: Vec<serde_json::Value> = rows
                .into_iter()
                .map(|r: PatRecord| serde_json::json!({"id": r.id, "user_id": r.user_id, "name": r.name, "created_at": r.created_at}))
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

async fn revoke_pat(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    ResourceId(id): ResourceId,
) -> Response {
    let ctx = response::build_actor_context(&identity);
    match s.app.runner.run(&ctx, "pat.revoke", || s.app.revoke_pat.execute(&ctx, &id)).await {
        Ok(()) => response::no_content(),
        Err(e) => response::app_error(e),
    }
}
