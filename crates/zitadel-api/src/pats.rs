use crate::{ApiState, response};
use axum::{
    Json, Router,
    extract::{Path, State},
    response::Response,
    routing::{delete, get},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
    let scoped = s.db.scoped_default();
    let id = Uuid::new_v4().to_string();
    let token = format!("zit_pat_{}", zitadel_crypto::random_hex(24));
    let token_hash = zitadel_authn::session::hash_token(&token);
    let scopes = serde_json::to_string(&req.scopes).unwrap_or_else(|_| "[]".to_string());
    let sql = format!(
        "INSERT INTO tokens (id, instance_id, type, token_hash, user_id, name, scopes) VALUES ($1, $2, 'pat', $3, $4, $5, {})",
        scoped.json_bind(6),
    );
    match sqlx::query(&sql)
        .bind(&id)
        .bind(scoped.instance_id())
        .bind(&token_hash)
        .bind(&req.user_id)
        .bind(&req.name)
        .bind(&scopes)
        .execute(scoped.pool())
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
    let scoped = s.db.scoped_default();
    let created_at = scoped.as_text("created_at");
    let sql = format!(
        "SELECT id, user_id, COALESCE(name,''), {created_at} FROM tokens WHERE instance_id = $1 AND type = 'pat' AND revoked_at IS NULL ORDER BY created_at DESC"
    );
    match sqlx::query_as::<_, (String, String, String, String)>(&sql)
        .bind(scoped.instance_id())
        .fetch_all(scoped.pool())
        .await
    {
        Ok(rows) => {
            let items: Vec<serde_json::Value> = rows.into_iter().map(|r| serde_json::json!({"id": r.0, "user_id": r.1, "name": r.2, "created_at": r.3})).collect();
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
    let scoped = s.db.scoped_default();
    match sqlx::query("UPDATE tokens SET revoked_at = CURRENT_TIMESTAMP WHERE instance_id = $1 AND id = $2 AND type = 'pat'")
        .bind(scoped.instance_id()).bind(&id).execute(scoped.pool()).await {
        Ok(r) if r.rows_affected() == 0 => response::not_found("pat not found"), Ok(_) => response::no_content(), Err(e) => response::internal_error(format!("{e}")),
    }
}
