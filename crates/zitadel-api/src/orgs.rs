use crate::{ApiState, response};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    response::Response,
    routing::get,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub fn routes() -> Router<ApiState> {
    Router::new()
        .route("/orgs", get(list).post(create))
        .route("/orgs/{id}", get(get_one).patch(update).delete(delete_one))
}

#[derive(Deserialize)]
pub struct OrgRequest {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub state: String,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

#[derive(Serialize)]
pub struct OrgResponse {
    pub id: String,
    pub name: String,
    pub state: String,
    #[serde(skip_serializing_if = "serde_json::Value::is_null")]
    pub metadata: serde_json::Value,
    pub created_at: String,
    pub updated_at: String,
}

async fn create(State(s): State<ApiState>, Json(req): Json<OrgRequest>) -> Response {
    if req.name.is_empty() {
        return response::bad_request("name is required");
    }
    let scoped = s.db.scoped_default();
    let id = Uuid::new_v4().to_string();
    let meta = response::to_json_string(&req.metadata);
    let sql = format!(
        "INSERT INTO orgs (id, instance_id, name, state, metadata) VALUES ($1, $2, $3, 'active', {})",
        scoped.json_bind(4),
    );
    match sqlx::query(&sql)
        .bind(&id)
        .bind(scoped.instance_id())
        .bind(&req.name)
        .bind(&meta)
        .execute(scoped.pool())
        .await
    {
        Ok(_) => match load(&scoped, &id).await {
            Ok(Some(o)) => response::json_created(o),
            _ => response::internal_error("created but not found"),
        },
        Err(e) => response::bad_request(format!("{e}")),
    }
}

async fn get_one(State(s): State<ApiState>, Path(id): Path<String>) -> Response {
    match load(&s.db.scoped_default(), &id).await {
        Ok(Some(o)) => response::json_ok(o),
        Ok(None) => response::not_found("org not found"),
        Err(e) => response::internal_error(format!("{e}")),
    }
}

async fn list(
    State(s): State<ApiState>,
    Query(p): Query<response::PaginationParams>,
) -> Response {
    let scoped = s.db.scoped_default();
    let cursor = p.cursor.unwrap_or_default();
    let metadata = scoped.as_text("metadata");
    let (created_at, updated_at) = scoped.select_timestamps();
    let sql = format!(
        "SELECT id, name, state, COALESCE({metadata}, '{{}}'), {created_at}, {updated_at} FROM orgs WHERE instance_id = $1 AND id > $2 ORDER BY id LIMIT $3"
    );
    match sqlx::query_as::<_, (String, String, String, String, String, String)>(&sql)
        .bind(scoped.instance_id())
        .bind(&cursor)
        .bind(p.limit.min(200) + 1)
        .fetch_all(scoped.pool())
        .await
    {
        Ok(rows) => {
            let limit = p.limit.min(200);
            let has_more = rows.len() as i64 > limit;
            let items: Vec<OrgResponse> = rows
                .into_iter()
                .take(limit as usize)
                .map(|r| OrgResponse {
                    id: r.0,
                    name: r.1,
                    state: r.2,
                    metadata: serde_json::from_str(&r.3).unwrap_or_default(),
                    created_at: r.4,
                    updated_at: r.5,
                })
                .collect();
            let next_cursor = if has_more {
                items.last().map(|o| o.id.clone())
            } else {
                None
            };
            response::json_ok(response::ListResponse {
                items,
                next_cursor,
                total: None,
            })
        }
        Err(e) => response::internal_error(format!("{e}")),
    }
}

async fn update(
    State(s): State<ApiState>,
    Path(id): Path<String>,
    Json(req): Json<OrgRequest>,
) -> Response {
    let scoped = s.db.scoped_default();
    let mut sets = Vec::new();
    let mut binds: Vec<String> = Vec::new();
    let mut next = 1usize;
    if !req.name.is_empty() {
        sets.push(format!("name = {}", scoped.placeholder(next)));
        binds.push(req.name);
        next += 1;
    }
    if !req.state.is_empty() {
        sets.push(format!("state = {}", scoped.placeholder(next)));
        binds.push(req.state);
        next += 1;
    }
    sets.push("updated_at = CURRENT_TIMESTAMP".into());
    let sql = format!(
        "UPDATE orgs SET {} WHERE instance_id = {} AND id = {}",
        sets.join(", "),
        scoped.placeholder(next),
        scoped.placeholder(next + 1),
    );
    let mut q = sqlx::query(&sql);
    for b in &binds {
        q = q.bind(b);
    }
    q = q.bind(scoped.instance_id()).bind(&id);
    match q.execute(scoped.pool()).await {
        Ok(r) if r.rows_affected() == 0 => response::not_found("org not found"),
        Ok(_) => match load(&scoped, &id).await {
            Ok(Some(o)) => response::json_ok(o),
            _ => response::not_found("org not found"),
        },
        Err(e) => response::internal_error(format!("{e}")),
    }
}

async fn delete_one(State(s): State<ApiState>, Path(id): Path<String>) -> Response {
    response::delete_by_id(&s.db.scoped_default(), "orgs", &id, "org").await
}

async fn load(
    scoped: &zitadel_db::scoped::ScopedDb,
    id: &str,
) -> anyhow::Result<Option<OrgResponse>> {
    let metadata = scoped.as_text("metadata");
    let (created_at, updated_at) = scoped.select_timestamps();
    let sql = format!(
        "SELECT id, name, state, COALESCE({metadata}, '{{}}'), {created_at}, {updated_at} FROM orgs WHERE instance_id = $1 AND id = $2"
    );
    let row: Option<(String, String, String, String, String, String)> = sqlx::query_as(&sql)
        .bind(scoped.instance_id())
        .bind(id)
        .fetch_optional(scoped.pool())
        .await?;
    Ok(row.map(|r| OrgResponse {
        id: r.0,
        name: r.1,
        state: r.2,
        metadata: serde_json::from_str(&r.3).unwrap_or_default(),
        created_at: r.4,
        updated_at: r.5,
    }))
}
