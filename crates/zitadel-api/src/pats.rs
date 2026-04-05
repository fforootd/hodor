use crate::{ApiState, response};
use axum::{
    Json, Router,
    extract::{Path, State},
    response::Response,
    routing::{delete, get},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zitadel_db::{create_pat as db_create_pat, current_instance_id, list_pats_for_instance, revoke_pat as db_revoke_pat};

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

async fn create_pat(State(s): State<ApiState>, Json(req): Json<CreatePatRequest>) -> Response {
    let id = Uuid::new_v4().to_string();
    let token = format!("zit_pat_{}", zitadel_crypto::random_hex(24));
    let token_hash = zitadel_authn::session::hash_token(&token);
    let scopes = serde_json::to_string(&req.scopes).unwrap_or_else(|_| "[]".to_string());
    match db_create_pat(
        &s.db,
        current_instance_id().as_ref(),
        &id,
        &req.user_id,
        &req.name,
        &token_hash,
        &scopes,
    )
    .await
    {
        Ok(_) => response::json_created(PatResponse {
            id,
            user_id: req.user_id,
            name: req.name,
            token,
            created_at: String::new(),
        }),
        Err(e) => response::bad_request(format!("{e}")),
    }
}

async fn list_pats(State(s): State<ApiState>) -> Response {
    match list_pats_for_instance(&s.db, current_instance_id().as_ref()).await {
        Ok(rows) => {
            let items: Vec<serde_json::Value> = rows
                .into_iter()
                .map(|r| serde_json::json!({"id": r.id, "user_id": r.user_id, "name": r.name, "created_at": r.created_at}))
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

async fn revoke_pat(State(s): State<ApiState>, Path(id): Path<String>) -> Response {
    match db_revoke_pat(&s.db, current_instance_id().as_ref(), &id).await {
        Ok(false) => response::not_found("pat not found"),
        Ok(true) => response::no_content(),
        Err(e) => response::internal_error(format!("{e}")),
    }
}
